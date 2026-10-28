//! Party release proposals wait only for the placed hold owner (tasks.md, 5.9).
use crate::{CallKey, Core, Event, GoalRoute, Limits, ProposalChoice, Request, Requests, RoutedCall};
use alloc::boxed::Box;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{Env, Queue, ReplyTo, Token};

pub(super) struct Offer {
    pub request: Token,
    pub person: u64,
    pub project: u32,
    pub role: Option<people::Role>,
    pub task: u64,
    pub hold: u64,
    pub revision: u64,
    pub reason: Box<[u8]>,
}
pub(super) fn offer(core: &mut Core, env: &Env<Limits>, work: &mut Queue<Event>, value: Offer) {
    let request = value.request;
    if let Err(why) = checked_offer(core, env, work, value) {
        super::refusals::pool_refused(work, request, why);
    }
}
fn checked_offer(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    value: Offer,
) -> Result<(), people::Refusal> {
    let Offer { request, person, project, role, task, hold, revision, reason } = value;
    let ask = people::Ask::Release { project, task, revision: 0, again: None };
    match core.control_admit(&ask, person, project, role, &env.limits.tasks) {
        Err(people::Refusal::Authority {
            lacking: people::Lack::HoldOwner { .. },
            proposable: Some(people::Accepter::HoldOwner),
        }) => {}
        Ok(_) => return Err(people::Refusal::NotOffered),
        Err(why) => return Err(why),
    }
    let current = core.tasks.placed_on(task).ok_or(people::Refusal::NotWaiting)?;
    if current.number != hold || current.project != project || current.lifted {
        return Err(people::Refusal::NotWaiting);
    }
    if current.revision != revision {
        return Err(people::Refusal::Moved { now: people::Moved::PlacedHold { revision: current.revision } });
    }
    let number = crate::fresh(&mut core.counters, crate::Family::Message)
        .ok_or(people::Refusal::Limit { which: people::LimitKind::Arithmetic })?;
    assert!(
        core.goal_routes.insert(request, GoalRoute::OfferingRelease { proposal: number }) == Ok(None),
        "one release offer"
    );
    work.push(Event::Tasks(tasks::Event::ProposePerson {
        reply_to: ReplyTo::new(request),
        proposal: tasks::PersonProposal {
            number,
            proposer: person,
            project,
            action: tasks::PersonAction::ReleasePlaced { task, hold, revision },
            reason,
            state: tasks::PersonProposalState::Pending { since: env.wall },
        },
    }));
    Ok(())
}

/// Judge acceptance by the accepter's current release authority, never the proposer's.
pub(crate) fn judge(
    core: &Core,
    limits: &Limits,
    person: u64,
    act: crate::acts::Act,
    proposal: &tasks::PersonProposal,
) -> Result<(), people::Refusal> {
    let tasks::PersonAction::ReleasePlaced { task, hold, revision } = proposal.action else {
        return Err(people::Refusal::NotOffered);
    };
    let current = core.tasks.placed_hold(hold).ok_or(people::Refusal::Unknown)?;
    core.party_release_admit(person, current)?;
    if act == crate::acts::Act::Accept {
        if current.lifted || current.revision != revision {
            return Err(people::Refusal::Moved { now: people::Moved::PlacedHold { revision: current.revision } });
        }
        let row = core.tasks.task(task).ok_or(people::Refusal::Unknown)?;
        if row.placed != Some(hold) {
            return Err(people::Refusal::NotWaiting);
        }
        let ask = people::Ask::Release { project: proposal.project, task, revision: row.revisions.hold, again: None };
        core.control_admit(&ask, person, proposal.project, core.people.role(person, proposal.project), &limits.tasks)?;
        Ok(())
    } else if act == crate::acts::Act::Reject {
        Ok(())
    } else if act == crate::acts::Act::PassUp {
        Err(people::Refusal::NoFurther)
    } else {
        Err(people::Refusal::NotOffered)
    }
}

pub(super) fn accept(
    core: &mut Core,
    work: &mut Queue<Event>,
    request: Token,
    person: u64,
    proposal: tasks::PersonProposal,
) {
    assert!(
        core.goal_routes.insert(
            request,
            GoalRoute::Deciding { proposer: proposal.proposer, proposal: proposal.number, by: person }
        ) == Ok(None),
        "one release decision"
    );
    work.push(Event::Tasks(tasks::Event::DecidePersonProposal {
        reply_to: ReplyTo::new(request),
        proposer: proposal.proposer,
        proposal: proposal.number,
        by: tasks::Party::Person(person),
        message: None,
        decision: tasks::ProposalDecision::Accept,
        revision: None,
    }));
}

pub(super) fn task_decide(
    core: &mut Core,
    _env: &Env<Limits>,
    work: &mut Queue<Event>,
    to: ReplyTo,
    key: CallKey,
    proposal: tasks::PersonProposal,
    choice: ProposalChoice,
) -> Requests {
    if proposal.state.pending_since().is_none()
        || core.tasks.person_proposal_owner(&proposal) != Some(tasks::Owner::Task { task: key.task })
    {
        return super::runs::proposal_refused(core, to, key, tasks::Refusal::Reference);
    }
    let (decision, message) = match choice {
        ProposalChoice::Accept => (tasks::ProposalDecision::Accept, None),
        ProposalChoice::Reject { reason } => {
            let Some(message) = crate::fresh(&mut core.counters, crate::Family::Message) else {
                return super::runs::proposal_refused(core, to, key, tasks::Refusal::Busy);
            };
            (tasks::ProposalDecision::Reject { reason }, Some(message))
        }
        ProposalChoice::Pass => return super::runs::proposal_refused(core, to, key, tasks::Refusal::Reference),
    };
    let token = to.into_token();
    assert!(core.pending_calls.insert(key, true).is_ok(), "release decision call room");
    assert!(
        core.routing_calls.insert(token, RoutedCall::Decide { key, proposal: proposal.number }) == Ok(None),
        "one release decision route"
    );
    work.push(Event::Tasks(tasks::Event::DecidePersonProposal {
        reply_to: ReplyTo::new(token),
        proposer: proposal.proposer,
        proposal: proposal.number,
        by: tasks::Party::Task(key.task),
        message,
        decision,
        revision: None,
    }));
    let mut out = Queue::with_capacity(1);
    out.push(Request::Decided);
    Requests::Out(out)
}
