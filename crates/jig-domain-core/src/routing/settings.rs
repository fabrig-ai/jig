//! Owner settings decisions, atomic with their keyed answer (engine.md, 17.2).
use crate::{Core, Event, Limits, Request, Requests, settings as s};
use alloc::boxed::Box;
use jig_domain_people as people;
use skein_lib::{Env, List, Map, Queue, ReplyTo, Token};

#[expect(clippy::too_many_arguments, reason = "the keyed envelope and its value cross admission together")]
pub(super) fn start(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    reply_to: ReplyTo,
    sign_in: u64,
    key: [u8; 16],
    digest: [u8; 32],
    change: Box<s::Change>,
) -> Requests {
    assert!(core.settings_input.is_none(), "one settings admission at a time");
    let against = change.against;
    let room = room(&env.limits)
        .expect("validated settings room")
        .writes
        .saturating_sub(1)
        .saturating_sub(people::max_out(&env.limits.people));
    core.settings_input = Some((
        match s::change_shape(&change.items, &env.limits) {
            Ok(()) => Ok(change),
            Err(failure) => Err(failure),
        },
        room,
    ));
    let result = super::people_routes::route_people(
        core,
        env,
        work,
        people::Event::Ask {
            reply_to,
            sign_in,
            key,
            ask: people::Ask::ChangeSettings { against, digest },
            models: Box::new([]),
        },
    );
    drop(core.settings_input.take());
    result
}
pub(super) fn admitted(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    out: &mut Queue<Request>,
    request: Token,
    person: u64,
    against: u32,
) {
    let outcome = match core.settings_input.take() {
        Some(_) if against != core.current.newest() => {
            people::Outcome::Refused(people::Refusal::Stale { newest: core.current.newest() })
        }
        Some((Ok(change), room)) => {
            match decide(core, env, work, out, s::Decider::Party(person), None, *change, room) {
                Ok(revision) => people::Outcome::SettingsChanged { revision },
                Err(why) => people::Outcome::Refused(why),
            }
        }
        Some((Err(failure), _)) => malformed(failure),
        None => people::Outcome::Refused(people::Refusal::NotOffered),
    };
    work.push(Event::People(people::Event::Decided { request, outcome }));
}
pub(super) fn malformed(failure: s::Failure) -> people::Outcome {
    people::Outcome::Refused(malformed_refusal(failure))
}
fn malformed_refusal(failure: s::Failure) -> people::Refusal {
    people::Refusal::NotWellFormed {
        at: 0,
        why: people::Malformed::Settings { setting: failure.setting, fault: failure.fault },
    }
}
#[expect(
    clippy::too_many_arguments,
    reason = "the shared whole-set decision carries its decider and proposal identity"
)]
pub(super) fn decide(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    out: &mut Queue<Request>,
    by: s::Decider,
    proposal: Option<(people::Proposer, u32)>,
    change: s::Change,
    room: u32,
) -> Result<u32, people::Refusal> {
    let previous = core.current.newest();
    if !withdrawal_room(core, proposal) {
        return Err(people::Refusal::Limit { which: people::LimitKind::Arithmetic });
    }
    if change.against != previous {
        return Err(people::Refusal::Stale { newest: previous });
    }
    if !s::changes_current(&core.current, &change.items) {
        return Err(people::Refusal::NotWellFormed { at: 0, why: people::Malformed::SettingsUnchanged });
    }
    if let Err(failure) = s::change_shape(&change.items, &env.limits) {
        return Err(malformed_refusal(failure));
    }
    if let Err(failure) = s::check_change(&core.current, &change.items, &core.settings, &core.connectors, &env.limits) {
        return Err(malformed_refusal(failure));
    }
    let charters = s::check::changed_charters(&core.current, &change.items, &core.settings, &env.limits);
    let Some(number) = previous.checked_add(1) else {
        return Err(people::Refusal::Limit { which: people::LimitKind::Arithmetic });
    };
    let mut after = Map::with_capacity(env.limits.settings.revisions.checked_add(1).expect("held names"));
    let moves = change.running == s::Running::Moves;
    if moves {
        core.tasks.held_after_moves(None, number, &charters, &mut after);
    }
    let keep_previous = if moves { after.contains_key(&previous) } else { core.tasks.holders(previous) > 0 };
    if !moves {
        for (&revision, _) in &core.held.heads {
            after.insert(revision, ()).expect("existing held names");
        }
        if keep_previous {
            after.insert(previous, ()).expect("previous newest name");
        }
    }
    let plan = core.held.plan_held(&core.current, &change.items, number, &after, &env.limits);
    let held_count = after.len();
    if held_count > env.limits.settings.revisions || plan.versions() > env.limits.settings.versions {
        let revision = match after.first() {
            Some((&revision, ())) => revision,
            None => previous,
        };
        let (tasks, more) = core.tasks.holding(revision, env.limits.tasks.circle);
        return Err(people::Refusal::Full { revision, tasks, more });
    }
    let mut templates = false;
    let mut rules = false;
    let mut charters = false;
    for item in &change.items {
        if s::changes_current(&core.current, core::slice::from_ref(item)) {
            match item.setting {
                s::Setting::Charter(_) => charters = true,
                s::Setting::Template(_) | s::Setting::Policy(_) => templates = true,
                s::Setting::Rules => rules = true,
            }
        }
    }
    let before = out.len();
    charters |= core.held.retain(&core.current, &plan, out);
    let held_writes = out.len().checked_sub(before).expect("held writes appended");
    drop(plan);
    drop(after);
    if keep_previous {
        core.held.heads.insert(previous, core.current.head.clone()).expect("held heads preflighted");
    }
    let items = core.current.revision_items(&change.items);
    let writes = s::apply(&mut core.current, &change.items, number);
    drop(change.items);
    core.current.head =
        s::Head { number, against: previous, at: env.wall, by, proposal, moved: change.running == s::Running::Moves };
    let revision = s::Revision { head: core.current.head.clone(), items };
    let (writes, oldest) = s::restart::revision_writes_with_hold(
        Some(revision),
        writes,
        core.current.oldest,
        core.current.newest(),
        env.limits.settings.history,
        room.saturating_sub(s::withdrawal_writes(&env.limits).expect("withdrawal write bound"))
            .saturating_sub(held_writes)
            .saturating_sub(s::move_writes(&env.limits).expect("move write bound")),
        keep_previous,
    );
    core.current.oldest = oldest;
    for write in writes.into_boxed() {
        out.push(Request::Write(write));
    }
    withdraw_stale(core, env, out, proposal);
    core.tell_replaced_settings(env, rules, charters);
    if moves {
        move_tasks(core, env, out, None);
    }
    if charters || rules {
        core.touch_deployment();
        core.touch_run_waits();
    }
    super::watch_marks::settings_revised(core, work, templates);
    Ok(number)
}

pub(super) fn room(limits: &Limits) -> Option<skein_lib::JournalRoom> {
    let people = people::max_out(&limits.people);
    Some(skein_lib::JournalRoom {
        writes: s::writes_worst_case(limits)?.checked_add(people)?,
        held: people.checked_add(limits.views.watchers.checked_mul(4)?)?.checked_add(limits.settings.proposals)?,
    })
}

#[expect(clippy::too_many_arguments, reason = "one keyed proposal envelope and its held value")]
pub(super) fn propose_start(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    reply_to: ReplyTo,
    sign_in: u64,
    key: [u8; 16],
    digest: [u8; 32],
    change: s::Proposed,
) -> Requests {
    assert!(core.proposed_input.is_none(), "one settings proposal admission");
    let s::Proposed { against, items, reason } = change;
    prepare_proposal(core, env, against, items);
    let result = super::people_routes::route_people(
        core,
        env,
        work,
        people::Event::Ask {
            reply_to,
            sign_in,
            key,
            models: Box::new([]),
            ask: people::Ask::Propose {
                project: people::DEPLOYMENT,
                action: people::Action::Settings { against, digest },
                reason,
            },
        },
    );
    drop(core.proposed_input.take());
    result
}

pub(super) fn propose_admitted(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    request: Token,
    person: u64,
    against: u32,
    reason: Box<[u8]>,
) {
    let result = proposal_check(core, env, against);
    let change = match result {
        Ok(change) => change,
        Err(why) => {
            work.push(Event::People(people::Event::Decided { request, outcome: people::Outcome::Refused(why) }));
            return;
        }
    };
    let Some(next) = core.counters.deployment().messages.checked_add(1) else {
        return super::refusals::pool_refused(
            work,
            request,
            people::Refusal::Limit { which: people::LimitKind::Arithmetic },
        );
    };
    let Ok(number) = u32::try_from(next) else {
        return super::refusals::pool_refused(
            work,
            request,
            people::Refusal::Limit { which: people::LimitKind::Arithmetic },
        );
    };
    let fresh = crate::fresh(&mut core.counters, crate::Family::Message).expect("proposal number preflighted");
    assert!(fresh == u64::from(number), "same proposal sequence");
    let heads = s::proposal_heads(&change.items);
    assert!(core.settings_offering.is_none(), "one settings offering");
    core.settings_offering = Some((request, person, number, s::Proposal { against, items: change.items }));
    core.goal_routes
        .insert(
            request,
            crate::GoalRoute::Offering { proposal: fresh, to: people::Role::Owner.number(), stubs: Box::new([]) },
        )
        .expect("one offering route");
    work.push(Event::Tasks(jig_domain_tasks::Event::ProposePerson {
        reply_to: ReplyTo::new(request),
        proposal: jig_domain_tasks::PersonProposal {
            number: fresh,
            proposer: person,
            project: people::DEPLOYMENT,
            action: jig_domain_tasks::PersonAction::Settings { against, items: heads },
            reason,
            state: jig_domain_tasks::PersonProposalState::Pending { since: env.wall },
        },
    }));
}
fn proposal_check(core: &mut Core, env: &Env<Limits>, against: u32) -> Result<Box<s::Proposal>, people::Refusal> {
    let input = core.proposed_input.take().ok_or(people::Refusal::NotOffered)?;
    if against != core.current.newest() {
        return Err(people::Refusal::Stale { newest: core.current.newest() });
    }
    let change = match input {
        Ok(change) => change,
        Err(why) => return Err(people::Refusal::NotWellFormed { at: 0, why }),
    };
    if change.against != against {
        return Err(people::Refusal::NotOffered);
    }
    if let Err(failure) = s::check_change(&core.current, &change.items, &core.settings, &core.connectors, &env.limits) {
        return Err(malformed_refusal(failure));
    }
    let mut owner = false;
    for holding in core.people.project_members(people::DEPLOYMENT).unwrap_or(&[]) {
        owner |= holding.role == people::Role::Owner;
    }
    if !owner {
        return Err(people::Refusal::Authority {
            lacking: people::Lack::Approver,
            proposable: Some(people::Accepter::Nobody),
        });
    }
    if core.settings_proposals.len() >= env.limits.settings.proposals {
        return Err(people::Refusal::Limit { which: people::LimitKind::SettingsProposals });
    }
    Ok(change)
}

/// Revision changes and release startup share the exact durable withdrawal path.
pub(crate) fn withdraw_stale(
    core: &mut Core,
    env: &Env<Limits>,
    out: &mut Queue<Request>,
    except: Option<(people::Proposer, u32)>,
) {
    let mut pending = List::with_capacity(env.limits.settings.proposals);
    for (&key, _) in &core.settings_proposals {
        if Some(key) != except {
            pending.push(key).expect("bounded pending sets");
        }
    }
    for (proposer, proposal) in pending.into_boxed() {
        if let people::Proposer::Task(task) = proposer {
            let message = crate::fresh(&mut core.counters, crate::Family::Message).expect("settings outcome sequence");
            super::children::append_tasks(
                core,
                env,
                jig_domain_tasks::Event::DecideSettingsProposal {
                    reply_to: None,
                    decision: jig_domain_tasks::SettingsDecision {
                        proposer: task,
                        proposal: u64::from(proposal),
                        message,
                        by: jig_domain_tasks::Party::Deployment { project: people::DEPLOYMENT },
                        outcome: jig_domain_tasks::SettingsOutcome::Stale { newest: core.current.newest() },
                        reason: Box::new([]),
                    },
                },
                out,
            );
            continue;
        }
        let people::Proposer::Party(person) = proposer else { unreachable!("party or task") };
        super::children::append_tasks(
            core,
            env,
            jig_domain_tasks::Event::WithdrawPersonProposal {
                reply_to: None,
                proposer: person,
                proposal: u64::from(proposal),
            },
            out,
        );
    }
}

/// Keep or erase the held set beside its task-owned proposal row.
pub(super) fn proposal_saved(core: &mut Core, record: &jig_domain_tasks::Stored, out: &mut Queue<Request>) {
    if let jig_domain_tasks::Stored::History(history) = record
        && let Some(row) = &history.proposal
        && let jig_domain_tasks::ProposalAction::Settings { .. } = row.action
    {
        let pending = match row.state {
            jig_domain_tasks::ProposalState::Pending { .. } => true,
            jig_domain_tasks::ProposalState::Accepted { .. }
            | jig_domain_tasks::ProposalState::Rejected { .. }
            | jig_domain_tasks::ProposalState::Withdrawn => false,
        };
        keep_set(core, people::Proposer::Task(row.proposer), row.number, pending, out);
        return;
    }
    let row = match record {
        jig_domain_tasks::Stored::PersonProposal(row) | jig_domain_tasks::Stored::DecidedProposal(row) => row,
        jig_domain_tasks::Stored::Live(_)
        | jig_domain_tasks::Stored::Ended(_)
        | jig_domain_tasks::Stored::Activity(_)
        | jig_domain_tasks::Stored::Milestone(_)
        | jig_domain_tasks::Stored::PlacedHold(_)
        | jig_domain_tasks::Stored::Stub(_)
        | jig_domain_tasks::Stored::Ledger(_)
        | jig_domain_tasks::Stored::Writer(_)
        | jig_domain_tasks::Stored::Pool(_)
        | jig_domain_tasks::Stored::ClosedLedger(_)
        | jig_domain_tasks::Stored::History(_)
        | jig_domain_tasks::Stored::Result(_)
        | jig_domain_tasks::Stored::EndedByParty(_)
        | jig_domain_tasks::Stored::EndedByProject(_)
        | jig_domain_tasks::Stored::EndedDelegate(_)
        | jig_domain_tasks::Stored::Delegate(_)
        | jig_domain_tasks::Stored::Update(_) => return,
    };
    if !row.action.is_settings() {
        return;
    }
    keep_set(core, people::Proposer::Party(row.proposer), row.number, row.state.pending_since().is_some(), out);
}
fn keep_set(core: &mut Core, proposer: people::Proposer, number: u64, pending: bool, out: &mut Queue<Request>) {
    let number = u32::try_from(number).expect("settings proposal fits key");
    let key = (proposer, number);
    if pending {
        if core.settings_proposals.contains_key(&key) {
            return;
        }
        let (_, offered_by, offered, value) = core.settings_offering.take().expect("admitted held set");
        let who = match proposer {
            people::Proposer::Party(n) | people::Proposer::Task(n) => n,
        };
        assert!(offered_by == who && offered == number, "same proposal set");
        out.push(Request::Write(crate::Write::Save(crate::Record::Core(crate::CoreRecord::Settings(
            s::Record::Proposal { proposer, proposal: number, value: value.clone() },
        )))));
        assert!(core.settings_proposals.insert(key, value).expect("settings proposal bound").is_none(), "fresh set");
    } else {
        core.settings_proposals.remove(&key).expect("decided settings set");
        out.push(Request::Write(crate::Write::Erase(crate::Key::Core(crate::CoreKey::Settings(s::Key::Proposal {
            proposer,
            proposal: number,
        })))));
    }
    core.touch_settings();
}

/// Counts decided by an owner's move (engine.md, 17.4).
pub(crate) struct Moved {
    pub moved: u32,
    pub stayed: u32,
}
/// An owner's move of a held revision (engine.md, 17.4).
pub(super) fn move_to_newest(
    core: &mut Core,
    env: &Env<Limits>,
    out: &mut Queue<Request>,
    revision: u32,
) -> Result<Moved, people::Refusal> {
    if revision == core.current.newest() {
        Ok(Moved { moved: 0, stayed: 0 })
    } else if core.held(revision) {
        Ok(move_tasks(core, env, out, Some(revision)))
    } else {
        Err(people::Refusal::Unknown)
    }
}

fn move_tasks(core: &mut Core, env: &Env<Limits>, out: &mut Queue<Request>, from: Option<u32>) -> Moved {
    use jig_domain_tasks as tasks;
    let room = s::task_room(&env.limits);
    let mut child = Queue::with_capacity(room);
    tasks::step(
        &mut core.tasks,
        &Env { now: env.now, wall: env.wall, limits: s::task_limits(&env.limits) },
        tasks::Event::MoveToNewest { from },
        &mut child,
    );
    let mut counts = None;
    for _ in 0..child.len() {
        let request = child.pop().expect("move output count");
        if let tasks::Request::MovedToNewest { moved, stayed } = request {
            counts = Some(Moved { moved, stayed });
        } else {
            child.push(request);
        }
    }
    let Requests::Out(mut routed) = super::tag_tasks(core, env, child, room, false, false, false);
    for _ in 0..routed.len() {
        let request = routed.pop().expect("routed move output count");
        if let Request::Decided = request {
        } else {
            out.push(request);
        }
    }
    core.touch_settings();
    counts.expect("move answers counts")
}

fn prepare_proposal(core: &mut Core, env: &Env<Limits>, against: u32, items: Box<[s::Item]>) {
    core.proposed_input = Some(if !s::changes_current(&core.current, &items) {
        Err(people::Malformed::SettingsUnchanged)
    } else if items.len() > usize::try_from(jig_domain_tasks::SETTINGS_ITEMS).expect("settings item bound") {
        Err(people::Malformed::Settings {
            setting: items.first().expect("past item bound").setting,
            fault: s::Fault::Count,
        })
    } else {
        match s::change_shape(&items, &env.limits) {
            Ok(()) => Ok(Box::new(s::Proposal { against, items })),
            Err(failure) => Err(people::Malformed::Settings { setting: failure.setting, fault: failure.fault }),
        }
    });
}

pub(super) fn propose_run(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    to: ReplyTo,
    key: crate::CallKey,
    change: s::Proposed,
) -> Requests {
    use jig_domain_tasks as tasks;
    let s::Proposed { against, items, reason } = change;
    prepare_proposal(core, env, against, items);
    let checked = proposal_check(core, env, against);
    let set = match checked {
        Ok(set) => *set,
        Err(why) => {
            return super::runs::route_named_answer(core, to, key, crate::CallPart::SettingsRefused(why));
        }
    };
    let Some(next) = core.counters.deployment().messages.checked_add(1) else {
        return super::runs::route_named_answer(
            core,
            to,
            key,
            crate::CallPart::SettingsRefused(people::Refusal::Limit { which: people::LimitKind::Arithmetic }),
        );
    };
    let Ok(number) = u32::try_from(next) else {
        return super::runs::route_named_answer(
            core,
            to,
            key,
            crate::CallPart::SettingsRefused(people::Refusal::Limit { which: people::LimitKind::Arithmetic }),
        );
    };
    let proposal = tasks::Proposal {
        number: next,
        proposer: key.task,
        project: people::DEPLOYMENT,
        action: tasks::ProposalAction::Settings { against, items: s::proposal_heads(&set.items) },
        reason,
        as_holder: false,
        state: tasks::ProposalState::Pending {
            holder: tasks::ProposalHolder::Policy { project: people::DEPLOYMENT, kind: tasks::ProposalKind::Settings },
            since: env.wall,
        },
    };
    // Preflight before retaining the full set; the child repeats its entrance check.
    if let Some(problem) = core.tasks.proposal_problem(&env.limits.tasks, &proposal) {
        return super::runs::route_named_answer(core, to, key, crate::CallPart::ProposalRefused(problem));
    }
    let fresh = crate::fresh(&mut core.counters, crate::Family::Message).expect("preflighted proposal number");
    assert!(fresh == next && core.settings_offering.is_none(), "one fresh settings offering");
    let token = to.into_token();
    core.settings_offering = Some((token, key.task, number, set));
    core.pending_calls.insert(key, true).expect("call room reserved");
    core.routing_calls.insert(token, crate::RoutedCall::Propose { key, proposal: next }).expect("one proposal route");
    work.push(Event::Tasks(tasks::Event::Propose { reply_to: ReplyTo::new(token), proposal }));
    let mut out = Queue::with_capacity(1);
    out.push(Request::Decided);
    Requests::Out(out)
}

/// The same change commits a party's or a task's owner-approved set (engine.md, 17.6).
pub(super) fn accept(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    out: &mut Queue<Request>,
    person: u64,
    key: (people::Proposer, u32),
    running: people::Running,
) -> Result<u32, people::Refusal> {
    let set = core.settings_proposals.get(&key).expect("pending settings set").clone();
    let room = room(&env.limits)
        .expect("settings decision room")
        .writes
        .saturating_sub(1)
        .saturating_sub(people::max_out(&env.limits.people));
    decide(
        core,
        env,
        work,
        out,
        s::Decider::Party(person),
        Some(key),
        s::Change { against: set.against, items: set.items, running },
        room,
    )
}

/// Check every new task notification's number before changing a revision or its defaults.
pub(crate) fn withdrawal_room(core: &Core, except: Option<(people::Proposer, u32)>) -> bool {
    let mut messages = 0_u64;
    for (&key, _) in &core.settings_proposals {
        if Some(key) != except
            && let people::Proposer::Task(_) = key.0
        {
            messages = messages.checked_add(1).expect("bounded pending settings");
        }
    }
    core.counters.deployment().messages.checked_add(messages).is_some()
}
