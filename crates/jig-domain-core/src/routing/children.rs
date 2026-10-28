//! Brief, fleet and account outputs translated for the root, with sibling continuations
//! (`domain/engine.md`, section 4.4).

use super::people_routes::tag_people;
use super::route_room;
use super::task_routes::tag_tasks;
use crate::{Ask, Core, CoreRecord, Event, Held, Key, Limits, Now, Record, Request, Requests, Write};
use alloc::boxed::Box;
use jig_domain_accounts as accounts;
use jig_domain_brief as brief;
use jig_domain_fleet as fleet;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{Duration, Env, Queue, ReplyTo, Time, Token};

pub(super) fn append_people(core: &mut Core, env: &Env<Limits>, event: people::Event, out: &mut Queue<Request>) {
    super::view_routes::touch_people_event(core, &env.limits, &event);
    let mut child = Queue::with_capacity(people::max_out(&env.limits.people));
    people::step(&mut core.people, &Env { now: env.now, wall: env.wall, limits: env.limits.people }, event, &mut child);
    let mut work = Queue::with_capacity(0);
    let Requests::Out(mut routed) = tag_people(core, env, child, &mut work);
    assert!(work.is_empty(), "a keyed party terminal does not start a task route");
    for _ in 0..routed.len() {
        match routed.pop().expect("party route count") {
            Request::Decided => {}
            request @ (Request::ProjectMade { .. }
            | Request::Write(_)
            | Request::Ask { .. }
            | Request::Held(_)
            | Request::Now(_)) => {
                out.push(request);
            }
        }
    }
}

pub(super) fn append_tasks(core: &mut Core, env: &Env<Limits>, event: tasks::Event, out: &mut Queue<Request>) {
    let room = crate::settings::task_room(&env.limits);
    let mut child = Queue::with_capacity(room);
    tasks::step(&mut core.tasks, &Env { now: env.now, wall: env.wall, limits: env.limits.tasks }, event, &mut child);
    let Requests::Out(mut routed) = tag_tasks(core, env, child, room, false, false, false);
    for _ in 0..routed.len() {
        match routed.pop().expect("task route count") {
            Request::Decided => {}
            request @ (Request::ProjectMade { .. }
            | Request::Write(_)
            | Request::Ask { .. }
            | Request::Held(_)
            | Request::Now(_)) => {
                out.push(request);
            }
        }
    }
}

pub(super) fn tag_brief(
    core: &mut Core,
    env: &Env<Limits>,
    mut child: Queue<brief::GatherRequest>,
    room: u32,
) -> Requests {
    let task_room = crate::settings::task_route_room(&env.limits).checked_mul(8).expect("bounded task routes");
    let party_room = people::max_out(&env.limits.people).checked_mul(4).expect("bounded party routes");
    let total = room.checked_add(task_room).expect("brief and task room");
    let total = total
        .checked_add(party_room)
        .expect("brief output room")
        .checked_add(crate::settings::release_writes(&env.limits).expect("release writes"))
        .expect("brief release room");
    let mut out = Queue::with_capacity(total.checked_add(1).expect("brief output mark"));
    for _ in 0..child.len() {
        let request = match child.pop().expect("brief output count") {
            brief::GatherRequest::Gather { connector, section, budget } => {
                Request::Ask { connector, ask: Ask::Gather { section, budget } }
            }
            brief::GatherRequest::CutTo { connector, section, size } => {
                Request::Ask { connector, ask: Ask::CutTo { section, size } }
            }
            brief::GatherRequest::Drop { connector, section } => Request::Ask { connector, ask: Ask::Drop { section } },
            brief::GatherRequest::Failed { brief, .. } | brief::GatherRequest::Refused { brief } => {
                let task = brief.raw();
                super::result_reads::clear(core, task);
                drop(core.contexts.remove(&task));
                drop(core.dependency_results.remove(&task));
                drop(core.transcripts.remove(&task));
                append_tasks(core, env, tasks::Event::PreparationFailed { task }, &mut out);
                continue;
            }
            brief::GatherRequest::Complete { brief, order } => {
                let task = brief.raw();
                let current = match core.contexts.get(&task) {
                    Some(context) => match core.tasks.task(task) {
                        Some(row) => {
                            row.phase == tasks::Phase::Active(tasks::Active::Preparing)
                                && core.tasks.run_message(task) == Some(context.last_message)
                        }
                        None => false,
                    },
                    None => false,
                };
                if !current {
                    for placed in order {
                        if let brief::GatherPlaced::Connector { connector, token, .. } = placed {
                            out.push(Request::Ask { connector, ask: Ask::Drop { section: token } });
                        }
                    }
                    continue;
                }
                Request::Now(Box::new(Now::CompleteBrief { brief, order }))
            }
        };
        out.push(request);
    }
    out.push(Request::Decided);
    Requests::Out(out)
}

#[expect(clippy::too_many_lines, reason = "one exhaustive host vocabulary routes its bounded child decision")]
pub(super) fn tag_fleet(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    mut child: Queue<fleet::Request>,
    room: u32,
) -> Requests {
    let marked = room.checked_mul(4).expect("bounded fleet routes");
    let task_room = crate::settings::task_route_room(&env.limits).checked_mul(8).expect("bounded task routes");
    let party_room = people::max_out(&env.limits.people).checked_mul(4).expect("bounded party routes");
    let total = marked.checked_add(task_room).expect("fleet and task route room");
    let total = total
        .checked_add(party_room)
        .expect("fleet and party route room")
        .checked_add(crate::settings::release_writes(&env.limits).expect("release writes"))
        .expect("fleet release room");
    let mut out = Queue::with_capacity(total.checked_add(1).expect("fleet output room"));
    let mut pending = Queue::with_capacity(marked);
    for _ in 0..child.len() {
        pending.push(child.pop().expect("fleet output count"));
    }
    for _ in 0..marked {
        let Some(next) = pending.pop() else { break };
        let request = match next {
            fleet::Request::ProgramTargets { source, workers } => {
                work.push(Event::Account(accounts::Event::ProgramTargets { source, hosts: workers }));
                continue;
            }
            fleet::Request::ProgramSignedOut { source, worker, result } => {
                let result = match result {
                    fleet::ProgramResult::SignedOut => accounts::ProgramResult::SignedOut,
                    fleet::ProgramResult::NotInContact => accounts::ProgramResult::NotInContact,
                    fleet::ProgramResult::Lost => accounts::ProgramResult::Lost,
                    fleet::ProgramResult::Failed => accounts::ProgramResult::Failed,
                };
                work.push(Event::Account(accounts::Event::ProgramSignedOut { source, host: worker, result }));
                continue;
            }
            fleet::Request::Sources { channel, sources } => Request::Held(Box::new(Held::Sources { channel, sources })),
            fleet::Request::Check { channel, name } => Request::Held(Box::new(Held::CheckSource { channel, name })),
            fleet::Request::SignOut { channel, name } => Request::Held(Box::new(Held::SignOutSource { channel, name })),
            fleet::Request::SignedOut { to, channel, name, result } => {
                Request::Held(Box::new(Held::SourceSignedOut { to, channel, name, result }))
            }
            fleet::Request::Acknowledge { channel, run, attempt } => {
                Request::Held(Box::new(Held::Acknowledge { channel, run, attempt }))
            }
            fleet::Request::AcknowledgeTurn { channel, run, attempt, turn } => {
                Request::Held(Box::new(Held::AcknowledgeTurn { channel, run, attempt, turn }))
            }
            fleet::Request::Cancel { channel, run, attempt } => {
                Request::Held(Box::new(Held::Cancel { channel, run, attempt }))
            }
            fleet::Request::Park { channel, run, attempt } => {
                Request::Held(Box::new(Held::Park { channel, run, attempt }))
            }
            fleet::Request::Refuse { channel } => Request::Held(Box::new(Held::Refuse { channel })),
            fleet::Request::TurnBusy { channel, run, attempt, turn } => {
                Request::Held(Box::new(Held::TurnBusy { channel, run, attempt, turn }))
            }
            fleet::Request::Drop { payload } => Request::Now(Box::new(Now::DropPayload { payload })),
            fleet::Request::Listed { run, attempt } => {
                let claimed = match core.proofs.get(&run.raw()) {
                    Some(proof) => proof.attempt == attempt.raw() && proof.terminal.is_none(),
                    None => false,
                };
                // Listings precede adoption during a cold start. Only an open
                // core has the complete proofs needed to reject a stale run.
                if core.restart_ready() && !claimed {
                    let mut follow = Queue::with_capacity(room);
                    fleet::step(
                        &mut core.fleet,
                        &Env { now: env.now, wall: env.wall, limits: env.limits.fleet },
                        fleet::Event::Cancel { run, attempt },
                        &mut follow,
                    );
                    for _ in 0..follow.len() {
                        pending.push(follow.pop().expect("fenced listing output count"));
                    }
                }
                continue;
            }
            fleet::Request::Observed { run, attempt: _ } => {
                core.touch_deployment();
                super::view_routes::touch_number(core, &env.limits, run.raw());
                continue;
            }
            fleet::Request::Placed { run, attempt } => {
                if core.unreported_restored.get(&run.raw()) == Some(&attempt.raw()) {
                    let _: Option<u64> = core.unreported_restored.remove(&run.raw());
                }
                append_tasks(core, env, tasks::Event::Started { task: run.raw(), attempt: attempt.raw() }, &mut out);
                continue;
            }
            fleet::Request::Refused { to, run, attempt, refusal: fleet::Refusal::Recheck } => {
                let _answered = to.into_token();
                core.remember_unpriced_terminal(run, attempt, tasks::End::Recheck);
                out.push(Request::Now(Box::new(Now::DropAssignment { task: run.raw() })));
                append_tasks(
                    core,
                    env,
                    tasks::Event::Activation {
                        reply_to: ReplyTo::new(Token::new(u64::MAX)),
                        task: run.raw(),
                        attempt: attempt.raw(),
                        cause: tasks::Cause::Unpriced,
                        end: tasks::End::Recheck,
                        saved: None,
                    },
                    &mut out,
                );
                continue;
            }
            fleet::Request::NotStarted { to, run, attempt }
            | fleet::Request::Withdrawn { to, run, attempt, .. }
            | fleet::Request::Refused {
                to,
                run,
                attempt,
                refusal: fleet::Refusal::Busy | fleet::Refusal::Workstream | fleet::Refusal::Duplicate,
            } => {
                let _answered = to.into_token();
                let _: Option<u64> = core.unreported_restored.remove(&run.raw());
                core.remember_unpriced_terminal(run, attempt, tasks::End::Refused);
                out.push(Request::Now(Box::new(Now::DropAssignment { task: run.raw() })));
                append_tasks(
                    core,
                    env,
                    tasks::Event::Activation {
                        reply_to: ReplyTo::new(Token::new(u64::MAX)),
                        task: run.raw(),
                        attempt: attempt.raw(),
                        end: tasks::End::Refused,
                        saved: None,
                        cause: tasks::Cause::Unpriced,
                    },
                    &mut out,
                );
                continue;
            }
            fleet::Request::Lost { to, run, attempt } => {
                let _answered = to.into_token();
                let proof = core.proofs.get(&run.raw()).expect("lost claim has durable evidence");
                assert!(proof.attempt == attempt.raw(), "lost callback belongs to current proof");
                let never_reported = core.unreported_restored.remove(&run.raw()) == Some(attempt.raw());
                let mut committed_call = false;
                for (key, _) in &core.call_parts {
                    if key.task == run.raw() && key.attempt == attempt.raw() {
                        committed_call = true;
                        break;
                    }
                }
                // The fleet has already classified a crossed claim's end.
                let end = if !proof.crossed && never_reported && proof.turn.is_none() && !committed_call {
                    tasks::End::Refused
                } else {
                    tasks::End::Failed(tasks::Class::Lost)
                };
                core.remember_unpriced_terminal(run, attempt, end.clone());
                append_tasks(
                    core,
                    env,
                    tasks::Event::Activation {
                        reply_to: ReplyTo::new(Token::new(u64::MAX)),
                        task: run.raw(),
                        attempt: attempt.raw(),
                        end,
                        saved: None,
                        cause: tasks::Cause::Unpriced,
                    },
                    &mut out,
                );
                continue;
            }
            fleet::Request::Assign { channel, kind, run, attempt, activation, assignment } => {
                let proof = core.proofs.get(&run.raw()).expect("assigned run has a claim");
                assert!(proof.attempt == attempt.raw() && proof.host == kind, "placement follows the claimed kind");
                if let Some(selected) = proof.placement {
                    assert_eq!(selected, channel, "assignment retains checked host");
                }
                if !core.claim_bindings_usable(run.raw(), attempt.raw()) {
                    core.remember_unpriced_terminal(run, attempt, tasks::End::Recheck);
                    append_tasks(
                        core,
                        env,
                        tasks::Event::Activation {
                            reply_to: ReplyTo::new(Token::new(u64::MAX)),
                            task: run.raw(),
                            attempt: attempt.raw(),
                            cause: tasks::Cause::Unpriced,
                            end: tasks::End::Recheck,
                            saved: None,
                        },
                        &mut out,
                    );
                    let mut cancelled = Queue::with_capacity(fleet::max_out(&env.limits.fleet));
                    fleet::step(
                        &mut core.fleet,
                        &Env { now: env.now, wall: env.wall, limits: env.limits.fleet },
                        fleet::Event::Cancel { run, attempt },
                        &mut cancelled,
                    );
                    for _ in 0..cancelled.len() {
                        pending.push(cancelled.pop().expect("cancelled placement outputs"));
                    }
                    out.push(Request::Now(Box::new(Now::DropAssignment { task: run.raw() })));
                    continue;
                }
                super::view_routes::touch_number(core, &env.limits, run.raw());
                Request::Held(Box::new(Held::Assign { channel, run, attempt, activation, assignment }))
            }
            fleet::Request::Inbound { channel, run, attempt, message } => {
                let deliver = match core.relaying.remove(&run.raw()) {
                    Some(relay) => {
                        assert!(message.name.raw() == relay.message, "relay name identifies word");
                        match core.proofs.get(&run.raw()) {
                            Some(proof) => proof.attempt == attempt.raw() && proof.offered == relay.previous,
                            None => false,
                        }
                    }
                    None => true,
                };
                if !deliver {
                    out.push(Request::Now(Box::new(Now::DropPayload { payload: message.words })));
                    continue;
                }
                if !core.stopping
                    && let Some(proof) = core.proofs.get_mut(&run.raw())
                    && proof.attempt == attempt.raw()
                {
                    let mut outstanding = skein_lib::List::with_capacity(env.limits.tasks.run_inbox);
                    for &number in &proof.outstanding {
                        outstanding.push(number).expect("bounded outstanding offers");
                    }
                    if !proof.outstanding.contains(&message.name.raw()) {
                        outstanding.push(message.name.raw()).expect("relay reserved run inbox room");
                    }
                    proof.outstanding = outstanding.into_boxed();
                    proof.offered = Some(match proof.offered {
                        Some(previous) => previous.max(message.name.raw()),
                        None => message.name.raw(),
                    });
                    out.push(Request::Write(Write::Save(Record::Core(CoreRecord::RunProof(proof.clone())))));
                }
                out.push(Request::Held(Box::new(Held::Inbound { channel, run, attempt, message })));
                if let Some(relay) = crate::relays::next(core, run.raw(), env.limits.tasks.run_inbox) {
                    out.push(Request::Held(Box::new(relay)));
                }
                continue;
            }
            fleet::Request::Relay { reply_to, .. } if core.stopping => {
                Request::Now(Box::new(Now::CallBusy { to: reply_to }))
            }
            fleet::Request::Relay { reply_to, run, attempt, call } => {
                Request::Now(Box::new(Now::Call { to: reply_to, run, attempt, call }))
            }
            fleet::Request::Relayed { channel, run, attempt, call, answer, writes } => {
                if writes {
                    Request::Held(Box::new(Held::Relayed { channel, run, attempt, call, answer }))
                } else {
                    Request::Now(Box::new(Now::Relayed { channel, run, attempt, call, answer }))
                }
            }
            fleet::Request::RelayBusy { channel, run, attempt, call } => {
                Request::Now(Box::new(Now::RelayBusy { channel, run, attempt, call }))
            }
            fleet::Request::DropCall { call } => Request::Now(Box::new(Now::DropCall { call })),
            fleet::Request::Undelivered { run, attempt, message, undelivered: _ } => {
                let _pending = core.relaying.remove(&run.raw());
                Request::Now(Box::new(Now::Undelivered { run, attempt, message }))
            }
            fleet::Request::Unauthorized { run, attempt, account, generation } => {
                if !claim_source(core, run, attempt, account, false, &mut out) {
                    continue;
                }
                append_account(core, env, accounts::Event::Unauthorized { account, generation }, &mut out);
                continue;
            }
            fleet::Request::Limited { run, attempt, account, reset } => {
                if !claim_source(core, run, attempt, account, false, &mut out) {
                    continue;
                }
                let reset = reset_at(env.now, reset);
                append_account(core, env, accounts::Event::Limited { account, reset }, &mut out);
                continue;
            }
            fleet::Request::AccountRefused { run, attempt, account, code, reason } => {
                if !claim_source(core, run, attempt, account, false, &mut out) {
                    continue;
                }
                append_account(core, env, accounts::Event::Refused { account, code, reason }, &mut out);
                continue;
            }
            fleet::Request::Grant { channel, run, attempt, grant } => {
                Request::Held(Box::new(Held::Grant { channel, run, attempt, grant }))
            }
            fleet::Request::Turned { run, attempt, turn, body } if core.stopping => {
                let mut follow = Queue::with_capacity(fleet::max_out(&env.limits.fleet));
                fleet::step(
                    &mut core.fleet,
                    &Env { now: env.now, wall: env.wall, limits: env.limits.fleet },
                    fleet::Event::TurnBusy { run, attempt, turn },
                    &mut follow,
                );
                for _ in 0..follow.len() {
                    pending.push(follow.pop().expect("busy turn outputs"));
                }
                Request::Now(Box::new(Now::DropPayload { payload: body }))
            }
            fleet::Request::Turned { run, attempt, turn, body } => {
                assert!(core.current_proof(run.raw(), attempt.raw()), "current turn has reserved proof");
                Request::Now(Box::new(Now::TurnPayload { run, attempt, turn, body }))
            }
            fleet::Request::Answered { payload, .. } if core.stopping => {
                Request::Now(Box::new(Now::DropPayload { payload }))
            }
            fleet::Request::Answered { run, attempt, payload, to, answer } => {
                let _answered = to.into_token();
                let _: Option<u64> = core.unreported_restored.remove(&run.raw());
                assert!(core.current_proof(run.raw(), attempt.raw()), "current answer has reserved proof");
                if let fleet::Answer::Failed { turned: Some(turned) } = answer {
                    let account = match turned {
                        fleet::Turned::Limited { account, .. } | fleet::Turned::Refused { account } => account,
                    };
                    if claim_source(core, run, attempt, account, true, &mut out) {
                        append_account(core, env, run_failed(env.now, turned), &mut out);
                    }
                }
                Request::Now(Box::new(Now::AnswerPayload { run, attempt, payload }))
            }
        };
        out.push(request);
    }
    out.push(Request::Decided);
    Requests::Out(out)
}

/// A worker's limit reset, a span from its notice, as a time on the
/// engine's clock.
#[expect(clippy::manual_map, reason = "the strict subset uses an exhaustive match rather than a closure")]
fn reset_at(now: Time, reset: Option<Duration>) -> Option<Time> {
    match reset {
        Some(reset) => Some(now.saturating_add(reset)),
        None => None,
    }
}

/// The account a failed run turned on, for the accounts child.
fn run_failed(now: Time, turned: fleet::Turned) -> accounts::Event {
    match turned {
        fleet::Turned::Limited { account, reset } => {
            accounts::Event::RunFailed { account, turned: accounts::Turned::Limited { reset: reset_at(now, reset) } }
        }
        fleet::Turned::Refused { account } => accounts::Event::RunFailed { account, turned: accounts::Turned::Refused },
    }
}

fn append_account(core: &mut Core, env: &Env<Limits>, event: accounts::Event, out: &mut Queue<Request>) {
    let mut work = Queue::with_capacity(0);
    let Requests::Out(mut routed) = route_account(core, env, &mut work, event);
    assert!(work.is_empty(), "a run's account turn answers no party");
    for _ in 0..routed.len() {
        match routed.pop().expect("account route count") {
            Request::Decided => {}
            request @ (Request::ProjectMade { .. }
            | Request::Write(_)
            | Request::Ask { .. }
            | Request::Held(_)
            | Request::Now(_)) => {
                out.push(request);
            }
        }
    }
}

pub(super) fn tag_account(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    mut child: Queue<accounts::Request>,
) -> Requests {
    let room = child
        .capacity()
        .checked_mul(fleet::max_out(&env.limits.fleet).checked_add(1).expect("grant routes"))
        .expect("account output room");
    let mut out = Queue::with_capacity(room.checked_add(1).expect("account decision"));
    for _ in 0..child.len() {
        match child.pop().expect("account output count") {
            accounts::Request::Programs { names } => {
                let mut sources = skein_lib::List::with_capacity(env.limits.accounts.program_names);
                let mut cleanup = skein_lib::List::with_capacity(env.limits.accounts.program_names);
                for row in names {
                    let name = row.name;
                    sources.push(name).expect("one per reservation");
                    if row.cleanup {
                        cleanup.push(name).expect("subset of issued names");
                    }
                }
                append_fleet(core, env, work, fleet::Event::CleanupPrograms { names: cleanup.into_boxed() }, &mut out);
                append_fleet(core, env, work, fleet::Event::Sources { sources: sources.into_boxed() }, &mut out);
            }
            accounts::Request::CheckProgram { name } => {
                append_fleet(core, env, work, fleet::Event::CheckProgram { name }, &mut out);
            }
            accounts::Request::SignOutProgram { source, name } => {
                append_fleet(core, env, work, fleet::Event::SignOutProgram { source, name }, &mut out);
            }
            accounts::Request::Granted { grant } => {
                let Requests::Out(mut routed) = route_fleet(
                    core,
                    env,
                    work,
                    fleet::Event::Grant { grant: crate::sibling_routes::fleet_grant(grant) },
                );
                for _ in 0..routed.len() {
                    match routed.pop().expect("grant output count") {
                        Request::Decided => {}
                        request @ (Request::ProjectMade { .. }
                        | Request::Write(_)
                        | Request::Ask { .. }
                        | Request::Held(_)
                        | Request::Now(_)) => {
                            out.push(request);
                        }
                    }
                }
            }
            // The accounts' component starts a party's sign-in and revokes a signed-out
            // record (accounts.md, 3.3 and section 5), as it grants and releases.
            request @ (accounts::Request::Component(_) | accounts::Request::Signing(_) | accounts::Request::Closed) => {
                if let accounts::Request::Signing(
                    accounts::Signing::Return { source } | accounts::Signing::Forget { source },
                ) = &request
                {
                    core.people.cancel_moves(Some(*source));
                } else if let accounts::Request::Component(accounts::ToComponent::Close) = &request {
                    core.people.cancel_moves(None);
                }
                out.push(Request::Now(Box::new(Now::Account(request))));
            }
            // The accounts family (protocol/store.md, 4.4 item 9) keeps each record in the
            // decision that changes it.
            accounts::Request::Save { record } => out.push(Request::Write(Write::Save(Record::Accounts(record)))),
            accounts::Request::Erase { key } => out.push(Request::Write(Write::Erase(Key::Accounts(key)))),
            // People keeps the answer under the party's key, and replies once it is durable.
            accounts::Request::Answer { request, answer } => {
                let outcome = super::account_requests::outcome(core, answer);
                work.push(Event::People(people::Event::Decided { request, outcome }));
            }
        }
    }
    out.push(Request::Decided);
    Requests::Out(out)
}

pub(super) fn remember_due(core: &mut Core, context: Box<tasks::RunContext>) {
    for task in &core.due {
        if task.task == context.task {
            return;
        }
    }
    core.due.push(context);
}

/// Determine the concerned party before the step removes an ended sign-in.
// Pending sign-in facts name their asker; account state can serve other parties through uses.
fn account_party(core: &Core, event: &accounts::Event) -> Option<u64> {
    match event {
        accounts::Event::SignIn { by, .. } => Some(*by),
        accounts::Event::SignInFailed { source, .. } => {
            for pending in &core.accounts.pending() {
                if pending.source == *source {
                    return match pending.holder {
                        accounts::Holder::Party(_) => Some(pending.by),
                        accounts::Holder::Deployment => None,
                    };
                }
            }
            None
        }
        accounts::Event::Withdrawn { .. }
        | accounts::Event::Opened { .. }
        | accounts::Event::NotOurs { .. }
        | accounts::Event::Adopted { .. }
        | accounts::Event::ProgramTargets { .. }
        | accounts::Event::ProgramSignedOut { .. }
        | accounts::Event::SignedIn { .. }
        | accounts::Event::SignOut { .. }
        | accounts::Event::Change { .. }
        | accounts::Event::Restore { .. }
        | accounts::Event::Want
        | accounts::Event::Component(_)
        | accounts::Event::Unauthorized { .. }
        | accounts::Event::Limited { .. }
        | accounts::Event::Refused { .. }
        | accounts::Event::RunFailed { .. }
        | accounts::Event::Credential { .. }
        | accounts::Event::Finishing { .. }
        | accounts::Event::FinishRefused { .. }
        | accounts::Event::Visited { .. }
        | accounts::Event::SignedOut { .. }
        | accounts::Event::Close => None,
    }
}

pub(super) fn route_account(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    event: accounts::Event,
) -> Requests {
    let party = account_party(core, &event);
    let mut out = Queue::with_capacity(accounts::max_out(&env.limits.accounts));
    accounts::step(
        &mut core.accounts,
        &Env { now: env.now, wall: env.wall, limits: env.limits.accounts },
        event,
        &mut out,
    );
    core.touch_deployment();
    core.touch_run_waits();
    core.touch_accounts(party);
    tag_account(core, env, work, out)
}

pub(super) fn route_brief(core: &mut Core, env: &Env<Limits>, event: brief::GatherEvent) -> Requests {
    let mut out = Queue::with_capacity(brief::gather_max_out(&env.limits.brief));
    brief::gather_step(
        &mut core.brief,
        &Env { now: env.now, wall: env.wall, limits: env.limits.brief },
        event,
        &mut out,
    );
    tag_brief(core, env, out, brief::gather_max_out(&env.limits.brief))
}

pub(super) fn route_fleet(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    mut event: fleet::Event,
) -> Requests {
    if let fleet::Event::Start { run, attempt, placement: Some(placement), .. } = &event
        && !core.fleet.placement_available(*placement)
    {
        let mut out = Queue::with_capacity(route_room(&env.limits));
        core.remember_unpriced_terminal(*run, *attempt, tasks::End::Recheck);
        append_tasks(
            core,
            env,
            tasks::Event::Activation {
                reply_to: ReplyTo::new(Token::new(u64::MAX)),
                task: run.raw(),
                attempt: attempt.raw(),
                cause: tasks::Cause::Unpriced,
                end: tasks::End::Recheck,
                saved: None,
            },
            &mut out,
        );
        out.push(Request::Now(Box::new(Now::DropAssignment { task: run.raw() })));
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    if let fleet::Event::Start { grants, .. } = &mut event {
        for grant in &mut **grants {
            match grant.form.table() {
                fleet::GrantTable::Accounts | fleet::GrantTable::Name => {
                    if let Some(current) = core.accounts.source_grant(grant.account, env.now) {
                        *grant = crate::sibling_routes::fleet_grant(current);
                    }
                }
                fleet::GrantTable::Application => {
                    if let Some(current) = core.accounts.credential_grant(grant.account, env.now) {
                        *grant = crate::sibling_routes::fleet_grant(current);
                    }
                }
            }
        }
    }
    let touched = match &event {
        fleet::Event::CleanupPrograms { .. }
        | fleet::Event::CheckProgram { .. }
        | fleet::Event::SignOutProgram { .. }
        | fleet::Event::Sources { .. }
        | fleet::Event::Logins { .. }
        | fleet::Event::Check { .. }
        | fleet::Event::SignOut { .. }
        | fleet::Event::SignedOut { .. }
        | fleet::Event::Start { .. }
        | fleet::Event::Adopt { .. }
        | fleet::Event::Cross { .. }
        | fleet::Event::Cancel { .. }
        | fleet::Event::Park { .. }
        | fleet::Event::Acknowledge { .. }
        | fleet::Event::Loaded
        | fleet::Event::Hello { .. }
        | fleet::Event::Lost { .. }
        | fleet::Event::Answer { .. } => true,
        fleet::Event::Inbound { .. }
        | fleet::Event::Relay { .. }
        | fleet::Event::Relayed { .. }
        | fleet::Event::Grant { .. }
        | fleet::Event::Unauthorized { .. }
        | fleet::Event::Limited { .. }
        | fleet::Event::AccountRefused { .. }
        | fleet::Event::Turn { .. }
        | fleet::Event::TurnKept { .. }
        | fleet::Event::TurnBusy { .. }
        | fleet::Event::Observe { .. }
        | fleet::Event::Needs(_) => false,
    };
    if touched {
        core.touch_deployment();
        core.touch_run_waits();
    }
    let shape = host_shape(&event);
    let draining = draining_hosts(core);
    let mut out = Queue::with_capacity(fleet::max_out(&env.limits.fleet));
    fleet::step(&mut core.fleet, &Env { now: env.now, wall: env.wall, limits: env.limits.fleet }, event, &mut out);
    if shape || draining != draining_hosts(core) {
        core.touch_accounts(None);
    }
    tag_fleet(core, env, work, out, fleet::max_out(&env.limits.fleet))
}

pub(super) fn route_projection_settled(core: &mut Core, env: &Env<Limits>, goal: u64, connector: u16) -> Requests {
    let mut out = Queue::with_capacity(route_room(&env.limits).checked_add(1).expect("settlement mark"));
    crate::projections::settled(core, goal, connector, &mut out);
    out.push(Request::Decided);
    Requests::Out(out)
}

/// Fleet authenticates the worker; the core authenticates the recorded source binding too.
fn claim_source(
    core: &mut Core,
    run: Token,
    attempt: Token,
    source: u32,
    terminal: bool,
    out: &mut Queue<Request>,
) -> bool {
    let Some(proof) = core.proofs.get_mut(&run.raw()) else { return false };
    if proof.attempt != attempt.raw() {
        return false;
    }
    let mut held = false;
    for binding in &proof.accounts {
        if binding.source == source {
            held = true;
        }
    }
    if !held {
        return false;
    }
    if proof.account_failure != Some(source) || (terminal && !proof.account_terminal) {
        proof.account_failure = Some(source);
        if terminal {
            proof.account_terminal = true;
        }
        out.push(Request::Write(Write::Save(Record::Core(CoreRecord::RunProof(proof.clone())))));
    }
    true
}

/// Account readiness ignores attempt occupancy; only host eligibility can change it.
fn host_shape(event: &fleet::Event) -> bool {
    match event {
        fleet::Event::Sources { .. }
        | fleet::Event::CleanupPrograms { .. }
        | fleet::Event::CheckProgram { .. }
        | fleet::Event::SignOutProgram { .. }
        | fleet::Event::Logins { .. }
        | fleet::Event::Check { .. }
        | fleet::Event::SignOut { .. }
        | fleet::Event::SignedOut { .. }
        | fleet::Event::Loaded
        | fleet::Event::Hello { .. }
        | fleet::Event::Lost { .. } => true,
        fleet::Event::Start { .. }
        | fleet::Event::Adopt { .. }
        | fleet::Event::Cross { .. }
        | fleet::Event::Cancel { .. }
        | fleet::Event::Park { .. }
        | fleet::Event::Acknowledge { .. }
        | fleet::Event::Answer { .. }
        | fleet::Event::Inbound { .. }
        | fleet::Event::Relay { .. }
        | fleet::Event::Relayed { .. }
        | fleet::Event::Grant { .. }
        | fleet::Event::Unauthorized { .. }
        | fleet::Event::Limited { .. }
        | fleet::Event::AccountRefused { .. }
        | fleet::Event::Turn { .. }
        | fleet::Event::TurnKept { .. }
        | fleet::Event::TurnBusy { .. }
        | fleet::Event::Observe { .. }
        | fleet::Event::Needs(_) => false,
    }
}

// An attempt may start or lift a host drain. Ignore occupancy, which readiness never uses.
// This temporary HostView list fits the document scratch envelope and is dropped before routing output.
fn draining_hosts(core: &Core) -> u32 {
    let mut draining = 0_u32;
    for host in &core.fleet.host_views() {
        if host.connected && host.draining {
            draining = draining.saturating_add(1);
        }
    }
    draining
}

fn append_fleet(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    event: fleet::Event,
    out: &mut Queue<Request>,
) {
    let Requests::Out(mut routed) = route_fleet(core, env, work, event);
    for _ in 0..routed.len() {
        match routed.pop().expect("fleet output count") {
            Request::Decided => {}
            request @ (Request::ProjectMade { .. }
            | Request::Write(_)
            | Request::Ask { .. }
            | Request::Held(_)
            | Request::Now(_)) => {
                out.push(request);
            }
        }
    }
}
