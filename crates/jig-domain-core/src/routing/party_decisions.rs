//! Party goal, proposal and escalation choices routed to their owner
//! (`domain/people.md`, section 5.1). Each proposal maker selects its own
//! current store, falling back to its exact historical identity. Current escalation
//! revisions take precedence over archived decisions, as in the former chain.

use super::PersonEscalation;
use super::refusals::pool_refused;
use crate::{Core, Event, GoalRoute, Limits, Now, PersonProposalRoute, Request};
use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{Env, Queue, ReplyTo, Token};

#[expect(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one holder decision checks and schedules its complete action atomically"
)]
pub(super) fn goal_decide(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    request: Token,
    person: u64,
    _role: Option<people::Role>,
    project: u32,
    proposer: u64,
    number: u64,
    choice: people::ProposalDecision,
) {
    let Some(proposal) = core.tasks.person_proposal(proposer, number) else {
        pool_refused(work, request, people::Refusal::Unknown);
        return;
    };
    if proposal.project != project {
        pool_refused(work, request, people::Refusal::Unknown);
        return;
    }
    if let Err(why) = crate::acts::proposal(
        core,
        &env.limits,
        person,
        proposal_act(&choice),
        crate::documents::Proposer::Party(proposer),
        number,
        env.wall,
    ) {
        pool_refused(work, request, why);
        return;
    }
    let event = match choice {
        people::ProposalDecision::Accept => {
            if super::proposal_revisions::party(core, work, request, &proposal) {
                return;
            }
            let (source, _) = core.person_pool(project, person);
            let (period, pool) = core.open_person_pool(project, person);
            if let Some(event) = period {
                work.push(Event::Tasks(event));
            }
            if let Some(event) = pool {
                work.push(Event::Tasks(event));
            }
            let event = match proposal.action {
                tasks::PersonAction::Settings { .. } => unreachable!("settings acceptance has its own decision"),
                tasks::PersonAction::Batch(mut batch) => {
                    super::deadlines::fund(&mut batch, source, env.wall);
                    tasks::Event::Make {
                        reply_to: ReplyTo::new(request),
                        creator: tasks::Party::Person(proposer),
                        batch,
                    }
                }
                tasks::PersonAction::Amend { task, amendment, .. } => {
                    let Some(current) = core.tasks.task(task) else {
                        pool_refused(work, request, people::Refusal::Unknown);
                        return;
                    };
                    let before = crate::translate::authority_value(&current.authority);
                    let after =
                        crate::translate::authority_value(amendment.authority.as_ref().unwrap_or(&current.authority));
                    let stop_run = !authority::at_most(&before, &after, &core.authority.rules().implies);
                    let Some(message) = crate::fresh(&mut core.counters, crate::Family::Message) else {
                        pool_refused(work, request, core.limit_refusal(people::LimitKind::Arithmetic));
                        return;
                    };
                    tasks::Event::Amend {
                        places: Box::new([]),
                        reply_to: ReplyTo::new(request),
                        by: tasks::Party::Person(proposer),
                        funder: Some(source),
                        task,
                        message,
                        stop_run,
                        amendment: *amendment,
                    }
                }
            };
            assert!(
                core.goal_routes.insert(request, GoalRoute::Accepting { proposer, proposal: number, by: person })
                    == Ok(None),
                "one party acceptance"
            );
            event
        }
        people::ProposalDecision::Reject { reason } => {
            let Some(message) = crate::fresh(&mut core.counters, crate::Family::Message) else {
                pool_refused(work, request, core.limit_refusal(people::LimitKind::Arithmetic));
                return;
            };
            assert!(
                core.goal_routes.insert(request, GoalRoute::Deciding { proposer, proposal: number, by: person })
                    == Ok(None),
                "one goal decision"
            );
            tasks::Event::DecidePersonProposal {
                reply_to: ReplyTo::new(request),
                proposer,
                proposal: number,
                by: tasks::Party::Person(person),
                message: Some(message),
                decision: tasks::ProposalDecision::Reject { reason },
                revision: None,
            }
        }
        people::ProposalDecision::Pass => {
            pool_refused(work, request, people::Refusal::NoFurther);
            return;
        }
    };
    work.push(Event::Tasks(event));
}

#[expect(clippy::too_many_arguments, reason = "a keyed proposal decision carries its holder and exact live proposal")]
pub(super) fn person_proposal_decide(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    request: Token,
    person: u64,
    _role: Option<people::Role>,
    project: u32,
    proposer: u64,
    number: u64,
    choice: people::ProposalDecision,
) {
    let Some(proposal) = core.tasks.proposal(proposer, number) else {
        pool_refused(work, request, people::Refusal::Unknown);
        return;
    };
    if proposal.project != project {
        pool_refused(work, request, people::Refusal::Unknown);
        return;
    }
    if let Err(why) = crate::acts::proposal(
        core,
        &env.limits,
        person,
        proposal_act(&choice),
        crate::documents::Proposer::Task(proposer),
        number,
        env.wall,
    ) {
        pool_refused(work, request, why);
        return;
    }
    let current = match proposal.state {
        tasks::ProposalState::Pending { holder, .. } => holder,
        tasks::ProposalState::Accepted { .. }
        | tasks::ProposalState::Rejected { .. }
        | tasks::ProposalState::Withdrawn => unreachable!("common judgment checked pending holder"),
    };
    let next = match choice {
        people::ProposalDecision::Accept => {
            person_proposal_accept(core, work, request, person, proposal, env.wall);
            return;
        }
        people::ProposalDecision::Reject { reason } => tasks::ProposalDecision::Reject { reason },
        people::ProposalDecision::Pass => match current {
            tasks::ProposalHolder::Person(_) => tasks::ProposalDecision::Pass {
                holder: tasks::ProposalHolder::Policy { project, kind: Core::proposal_kind(&proposal.action).0 },
            },
            tasks::ProposalHolder::Policy { .. } => {
                pool_refused(work, request, people::Refusal::NoFurther);
                return;
            }
            tasks::ProposalHolder::Task(_) => unreachable!("person standing checked"),
        },
    };
    let message = match next {
        tasks::ProposalDecision::Reject { .. } => match crate::fresh(&mut core.counters, crate::Family::Message) {
            Some(message) => Some(message),
            None => {
                pool_refused(work, request, core.limit_refusal(people::LimitKind::Arithmetic));
                return;
            }
        },
        tasks::ProposalDecision::Pass { .. } => None,
        tasks::ProposalDecision::Accept => unreachable!("accept routed separately"),
    };
    assert!(
        core.routing_people_proposals.insert(
            request,
            PersonProposalRoute::Deciding { request, proposer, proposal: number, by: person, moved: None },
        ) == Ok(None),
        "one person proposal route"
    );
    work.push(Event::PersonProposal(tasks::Event::DecideProposal {
        reply_to: ReplyTo::new(request),
        proposer,
        proposal: number,
        message,
        by: tasks::Party::Person(person),
        decision: next,
    }));
}

#[expect(clippy::too_many_lines, reason = "one exhaustive authenticated person proposal dispatcher")]
pub(super) fn person_proposal_accept(
    core: &mut Core,
    work: &mut Queue<Event>,
    request: Token,
    person: u64,
    proposal: tasks::Proposal,
    decision: skein_lib::Wall,
) {
    if super::proposal_revisions::person(core, work, request, person, &proposal) {
        return;
    }
    if crate::proposals::placed_release(&proposal.action) {
        let Some(message) = crate::fresh(&mut core.counters, crate::Family::Message) else {
            pool_refused(work, request, core.limit_refusal(people::LimitKind::Arithmetic));
            return;
        };
        assert!(
            core.routing_people_proposals.insert(
                request,
                PersonProposalRoute::Deciding {
                    request,
                    proposer: proposal.proposer,
                    proposal: proposal.number,
                    by: person,
                    moved: None,
                }
            ) == Ok(None),
            "one release acceptance"
        );
        work.push(Event::PersonProposal(tasks::Event::DecideProposal {
            reply_to: ReplyTo::new(request),
            proposer: proposal.proposer,
            proposal: proposal.number,
            message: Some(message),
            by: tasks::Party::Person(person),
            decision: tasks::ProposalDecision::Accept,
        }));
        return;
    }
    let source = tasks::Funder::Pool { project: proposal.project, person, period: core.period(proposal.project) };
    if let Err(why) = core.action_accounts(&proposal.action, source) {
        pool_refused(work, request, core.account_refusal(why));
        return;
    }
    match &proposal.action {
        tasks::ProposalAction::Amend { .. } | tasks::ProposalAction::Widen { .. } => {
            let (period, pool) = core.open_person_pool(proposal.project, person);
            if let Some(event) = period {
                work.push(Event::Tasks(event));
            }
            if let Some(event) = pool {
                work.push(Event::Tasks(event));
            }
        }
        tasks::ProposalAction::Settings { .. }
        | tasks::ProposalAction::Batch(_)
        | tasks::ProposalAction::Effect { .. }
        | tasks::ProposalAction::ProcedureEffect { .. }
        | tasks::ProposalAction::ReleasePlaced { .. }
        | tasks::ProposalAction::Release { .. } => {}
    }
    let result_followups = core.tasks.result_proposal(proposal.proposer, proposal.number);
    match proposal.action {
        tasks::ProposalAction::Effect { connector, .. } | tasks::ProposalAction::ProcedureEffect { connector, .. } => {
            work.push(Event::EffectStart {
                owner: request,
                connector,
                origin: crate::EffectOrigin::Accept {
                    to: ReplyTo::new(request),
                    key: None,
                    proposal: Box::new(proposal),
                    by: tasks::Party::Person(person),
                },
            });
            return;
        }
        tasks::ProposalAction::Settings { .. }
        | tasks::ProposalAction::Batch(_)
        | tasks::ProposalAction::Amend { .. }
        | tasks::ProposalAction::Widen { .. }
        | tasks::ProposalAction::ReleasePlaced { .. }
        | tasks::ProposalAction::Release { .. } => {}
    }
    let event = match proposal.action {
        tasks::ProposalAction::Settings { .. } => unreachable!("settings has its own acceptance"),
        tasks::ProposalAction::Effect { .. } | tasks::ProposalAction::ProcedureEffect { .. } => {
            unreachable!("effect accepted through its connector")
        }
        tasks::ProposalAction::Batch(mut batch) => {
            super::deadlines::resolve(&mut batch, decision);
            let held = match core.people.role(person, proposal.project) {
                Some(role) => core.authority.escalations_held(proposal.project, role.number()),
                None => false,
            };
            if proposal.as_holder && !held {
                pool_refused(work, request, people::Refusal::NoEscalationHolder);
                return;
            }
            let creator =
                if proposal.as_holder { tasks::Party::Person(person) } else { tasks::Party::Task(proposal.proposer) };
            let (period, pool) = core.open_person_pool(proposal.project, person);
            if let Some(event) = period {
                work.push(Event::Tasks(event));
            }
            if let Some(event) = pool {
                work.push(Event::Tasks(event));
            }
            for member in &mut batch {
                member.funder =
                    tasks::Funder::Pool { project: proposal.project, person, period: core.period(proposal.project) };
            }
            if result_followups {
                tasks::Event::MakeResultFollowups {
                    reply_to: ReplyTo::new(request),
                    proposer: proposal.proposer,
                    proposal: proposal.number,
                    batch,
                }
            } else {
                tasks::Event::Make { reply_to: ReplyTo::new(request), creator, batch }
            }
        }
        tasks::ProposalAction::Amend { task, amendment, .. } => {
            if let Err(why) = core.amendment_accept_revision(proposal.proposer, task, amendment.release) {
                pool_refused(work, request, why);
                return;
            }
            let Some(current) = core.tasks.delegation(task) else {
                pool_refused(work, request, people::Refusal::Unknown);
                return;
            };
            let Some(amend_message) = crate::fresh(&mut core.counters, crate::Family::Message) else {
                pool_refused(work, request, core.limit_refusal(people::LimitKind::Arithmetic));
                return;
            };
            let after = crate::translate::authority_value(amendment.authority.as_ref().unwrap_or(&current.authority));
            let before = crate::translate::authority_value(&current.authority);
            let stop_run = !authority::at_most(&before, &after, &core.authority.rules().implies);
            tasks::Event::Amend {
                places: Box::new([]),
                funder: Some(source),
                reply_to: ReplyTo::new(request),
                by: tasks::Party::Person(person),
                task,
                message: amend_message,
                stop_run,
                amendment,
            }
        }
        tasks::ProposalAction::Widen { task, authority } => {
            let Some(amend_message) = crate::fresh(&mut core.counters, crate::Family::Message) else {
                pool_refused(work, request, core.limit_refusal(people::LimitKind::Arithmetic));
                return;
            };
            tasks::Event::Amend {
                places: Box::new([]),
                funder: Some(source),
                reply_to: ReplyTo::new(request),
                by: tasks::Party::Person(person),
                task,
                message: amend_message,
                stop_run: false,
                amendment: tasks::Amendment {
                    tracked: None,
                    addressed: None,
                    release: None,
                    instructions: None,
                    spec: None,
                    wake: None,
                    dependencies: None,
                    authority: Some(authority),
                    llm: None,
                    reason: proposal.reason,
                },
            }
        }
        tasks::ProposalAction::ReleasePlaced { task, .. } | tasks::ProposalAction::Release { task } => {
            tasks::Event::Control {
                reply_to: ReplyTo::new(request),
                by: tasks::Party::Person(person),
                task,
                control: tasks::Control::Release { again: None, message: None },
            }
        }
    };
    let Some(message) = crate::fresh(&mut core.counters, crate::Family::Message) else {
        pool_refused(work, request, core.limit_refusal(people::LimitKind::Arithmetic));
        return;
    };
    assert!(
        core.routing_people_proposals.insert(
            request,
            PersonProposalRoute::Accepting {
                request,
                person,
                proposer: proposal.proposer,
                proposal: proposal.number,
                message,
            },
        ) == Ok(None),
        "one person acceptance route"
    );
    work.push(Event::PersonProposal(event));
}

#[expect(clippy::too_many_arguments, reason = "one keyed held choice carries its person and exact semantic revision")]
pub(super) fn person_escalation_decide(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    request: Token,
    person: u64,
    _role: Option<people::Role>,
    project: u32,
    task: u64,
    revision: u64,
    decision: people::EscalationDecision,
) {
    if let people::EscalationDecision::Reject { reason } = &decision
        && reason.len() > usize::try_from(env.limits.escalation_reason_bytes).expect("reason bound fits usize")
    {
        pool_refused(
            work,
            request,
            people::Refusal::configured(
                people::LimitKind::Contract,
                u64::from(env.limits.escalation_reason_bytes),
                people::LimitKey::CoreEscalationReasonBytes,
                people::LimitUnit::Bytes,
            ),
        );
        return;
    }
    let Some(context) = core.tasks.escalation(task) else {
        pool_refused(work, request, people::Refusal::Unknown);
        return;
    };
    if context.project != project {
        pool_refused(work, request, people::Refusal::Unknown);
        return;
    }
    let pending = match context.escalation {
        tasks::Escalation::Waiting { revision: current, .. } => current == revision,
        tasks::Escalation::Unheld { .. } | tasks::Escalation::Routing { .. } | tasks::Escalation::Rejected { .. } => {
            false
        }
    };
    if !pending {
        pool_refused(work, request, people::Refusal::NotWaiting);
        return;
    }
    let act = match &decision {
        people::EscalationDecision::Release => crate::acts::Act::Release,
        people::EscalationDecision::Reject { .. } => crate::acts::Act::LeaveHeld,
        people::EscalationDecision::Pass => crate::acts::Act::PassUp,
    };
    if let Err(why) = crate::acts::escalation(core, &env.limits, person, act, task) {
        pool_refused(work, request, why);
        return;
    }
    let semantic = match &decision {
        people::EscalationDecision::Release => tasks::EscalationDecision::Release,
        people::EscalationDecision::Reject { reason } => tasks::EscalationDecision::Reject { reason: reason.clone() },
        people::EscalationDecision::Pass => {
            let Some(holder) = core.escalation_fallback(project) else {
                pool_refused(work, request, people::Refusal::NoFurther);
                return;
            };
            tasks::EscalationDecision::Pass { holder }
        }
    };
    let entry = match semantic {
        tasks::EscalationDecision::Pass { .. } => match crate::fresh(&mut core.counters, crate::Family::Message) {
            Some(entry) => Some(entry),
            None => {
                pool_refused(work, request, core.limit_refusal(people::LimitKind::Arithmetic));
                return;
            }
        },
        tasks::EscalationDecision::Release | tasks::EscalationDecision::Reject { .. } => None,
    };
    assert!(
        core.person_escalations.insert(
            request,
            PersonEscalation { request, requester: context.requester, person, project, task, revision, decision },
        ) == Ok(None),
        "one keyed escalation choice"
    );
    work.push(Event::PersonEscalation(tasks::Event::DecideEscalation {
        reply_to: ReplyTo::new(request),
        task,
        revision,
        by: person,
        entry,
        decision: semantic,
    }));
}

pub(super) fn current_person_escalation(core: &Core, task: u64, revision: u64) -> bool {
    let Some(context) = core.tasks.escalation(task) else { return false };
    match context.escalation {
        tasks::Escalation::Waiting { revision: current, .. } => current == revision,
        tasks::Escalation::Unheld { .. } | tasks::Escalation::Routing { .. } | tasks::Escalation::Rejected { .. } => {
            false
        }
    }
}

#[expect(clippy::too_many_arguments, reason = "the route retains the admitted party and request fields")]
pub(super) fn proposal_request(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    request: Token,
    person: u64,
    project: u32,
    role: Option<people::Role>,
    out: &mut Queue<Request>,
    proposer: people::Proposer,
    proposal: u64,
    decision: people::ProposalDecision,
    running: Option<people::Running>,
) {
    match proposer {
        people::Proposer::Party(number) => {
            if let Some(row) = core.tasks.person_proposal(number, proposal) {
                let settings = match row.action {
                    tasks::PersonAction::Settings { .. } => true,
                    tasks::PersonAction::Batch(_) | tasks::PersonAction::Amend { .. } => false,
                };
                let accepts = decision == people::ProposalDecision::Accept;
                if running.is_some() != (settings && accepts) {
                    pool_refused(
                        work,
                        request,
                        people::Refusal::NotWellFormed { at: 0, why: people::Malformed::Running },
                    );
                    return;
                }
                if settings && accepts {
                    if row.project != project {
                        pool_refused(work, request, people::Refusal::Unknown);
                        return;
                    }
                    if let Err(why) = crate::acts::proposal(
                        core,
                        &env.limits,
                        person,
                        crate::acts::Act::Accept,
                        crate::documents::Proposer::Party(number),
                        proposal,
                        env.wall,
                    ) {
                        pool_refused(work, request, why);
                        return;
                    }
                    let key =
                        (people::Proposer::Party(number), u32::try_from(proposal).expect("settings proposal identity"));
                    let outcome = super::settings::accept(
                        core,
                        env,
                        work,
                        out,
                        person,
                        key,
                        running.expect("settings acceptance"),
                    );
                    match outcome {
                        Ok(revision) => {
                            core.goal_routes
                                .insert(request, GoalRoute::Deciding { proposer: number, proposal, by: person })
                                .expect("one settings acceptance");
                            work.push(Event::Tasks(tasks::Event::DecidePersonProposal {
                                reply_to: ReplyTo::new(request),
                                proposer: number,
                                proposal,
                                by: tasks::Party::Person(person),
                                message: None,
                                revision: Some(revision),
                                decision: tasks::ProposalDecision::Accept,
                            }));
                        }
                        Err(why) => work.push(Event::People(people::Event::Decided {
                            request,
                            outcome: people::Outcome::Refused(why),
                        })),
                    }
                    return;
                }
                goal_decide(core, env, work, request, person, role, project, number, proposal, decision);
                return;
            }
        }
        people::Proposer::Task(number) => {
            if let Some(row) = core.tasks.proposal(number, proposal)
                && let tasks::ProposalAction::Settings { .. } = row.action
            {
                task_settings_decide(core, env, work, out, (request, person, project), row, (decision, running));
                return;
            }
            if running.is_some() {
                pool_refused(work, request, people::Refusal::NotWellFormed { at: 0, why: people::Malformed::Running });
                return;
            }
            if core.tasks.proposal(number, proposal).is_some() {
                person_proposal_decide(core, env, work, request, person, role, project, number, proposal, decision);
                return;
            }
        }
    }
    out.push(Request::Now(Box::new(Now::HistoricalProposal { request, person, project, proposer, proposal })));
}

#[expect(clippy::too_many_arguments, reason = "the route retains the admitted party and request fields")]
pub(super) fn escalation_request(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    request: Token,
    person: u64,
    project: u32,
    role: Option<people::Role>,
    out: &mut Queue<Request>,
    task: u64,
    revision: u64,
    decision: people::EscalationDecision,
) {
    if current_person_escalation(core, task, revision) {
        person_escalation_decide(core, env, work, request, person, role, project, task, revision, decision);
        return;
    }

    if match &decision {
        people::EscalationDecision::Reject { reason } => {
            reason.len() > usize::try_from(env.limits.escalation_reason_bytes).expect("reason bound fits usize")
        }
        people::EscalationDecision::Release | people::EscalationDecision::Pass => false,
    } {
        pool_refused(
            work,
            request,
            people::Refusal::configured(
                people::LimitKind::Contract,
                u64::from(env.limits.escalation_reason_bytes),
                people::LimitKey::CoreEscalationReasonBytes,
                people::LimitUnit::Bytes,
            ),
        );
        return;
    }
    if let Some(context) = core.tasks.escalation(task)
        && (context.project != project || !core.escalation_visible(&context, person, role))
    {
        pool_refused(work, request, people::Refusal::Unknown);
        return;
    }
    if let Some(context) = core.tasks.escalation(task)
        && context.escalation.revision() != revision
    {
        pool_refused(work, request, crate::revisions::moved_hold(context.escalation.revision(), context.why));
        return;
    }
    out.push(Request::Now(Box::new(Now::HistoricalEscalation { request, person, project, task, revision })));
}

fn proposal_act(choice: &people::ProposalDecision) -> crate::acts::Act {
    match choice {
        people::ProposalDecision::Accept => crate::acts::Act::Accept,
        people::ProposalDecision::Reject { .. } => crate::acts::Act::Reject,
        people::ProposalDecision::Pass => crate::acts::Act::PassUp,
    }
}

fn task_settings_decide(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    out: &mut Queue<Request>,
    caller: (Token, u64, u32),
    row: tasks::Proposal,
    choice: (people::ProposalDecision, Option<people::Running>),
) {
    let (request, person, project) = caller;
    let (choice, running) = choice;
    let accepts = choice == people::ProposalDecision::Accept;
    if running.is_some() != accepts {
        return pool_refused(work, request, people::Refusal::NotWellFormed { at: 0, why: people::Malformed::Running });
    }
    if project != people::DEPLOYMENT {
        return pool_refused(work, request, people::Refusal::Unknown);
    }
    if let Err(why) = crate::acts::proposal(
        core,
        &env.limits,
        person,
        proposal_act(&choice),
        crate::documents::Proposer::Task(row.proposer),
        row.number,
        env.wall,
    ) {
        return pool_refused(work, request, why);
    }
    let reason = match &choice {
        people::ProposalDecision::Reject { reason } => reason.clone(),
        people::ProposalDecision::Accept => Box::new([]),
        people::ProposalDecision::Pass => return pool_refused(work, request, people::Refusal::NoFurther),
    };
    if reason.len() > usize::try_from(env.limits.tasks.message_bytes).expect("message bound") {
        return pool_refused(
            work,
            request,
            people::Refusal::configured(
                people::LimitKind::Contract,
                u64::from(env.limits.escalation_reason_bytes),
                people::LimitKey::CoreEscalationReasonBytes,
                people::LimitUnit::Bytes,
            ),
        );
    }
    let messages = u64::from(env.limits.tasks.tasks)
        .checked_mul(2)
        .expect("move messages")
        .checked_add(u64::from(env.limits.settings.proposals))
        .expect("stale messages")
        .checked_add(1)
        .expect("outcome message");
    if core.counters.deployment().messages.checked_add(messages).is_none() {
        return pool_refused(work, request, core.limit_refusal(people::LimitKind::Arithmetic));
    }
    let outcome = if accepts {
        let key =
            (people::Proposer::Task(row.proposer), u32::try_from(row.number).expect("settings proposal identity"));
        match super::settings::accept(core, env, work, out, person, key, running.expect("acceptance running choice")) {
            Ok(revision) => tasks::SettingsOutcome::Accepted { revision },
            Err(why) => return pool_refused(work, request, why),
        }
    } else {
        tasks::SettingsOutcome::Rejected
    };
    let message =
        crate::fresh(&mut core.counters, crate::Family::Message).expect("settings outcome number preflighted");
    core.routing_people_proposals
        .insert(
            request,
            PersonProposalRoute::Deciding {
                request,
                proposer: row.proposer,
                proposal: row.number,
                by: person,
                moved: None,
            },
        )
        .expect("one settings terminal route");
    work.push(Event::PersonProposal(tasks::Event::DecideSettingsProposal {
        reply_to: Some(ReplyTo::new(request)),
        decision: tasks::SettingsDecision {
            proposer: row.proposer,
            proposal: row.number,
            message,
            by: tasks::Party::Person(person),
            outcome,
            reason,
        },
    }));
}
