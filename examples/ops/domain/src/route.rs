//! Each variant fixes its destination. Replace connector arms when copying;
//! retain the journal marks and engine's permanent host link (domain/root.md, 5).
use crate::boundary::Payload;
use crate::domain::{Work, child, held, now, write};
use crate::translate;
use crate::{Domain, Event, Key, Limits, Output, Record, Released, Write};
use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_host as host;
use jig_domain_tasks as tasks;
use jig_ops_domain_connectors::Connector;
use jig_ops_domain_infrastructure as infrastructure;
use jig_ops_domain_observability as observability;
use skein_lib::{Decision, Env, List, Queue, ReplyTo, Token};

pub(crate) fn work(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision<Write, Output>,
    item: Work,
    work: &mut Queue<Work>,
) {
    match item {
        Work::Event(event) => event_in(domain, env, decision, event, work),
        Work::Core(event) => {
            let requests =
                core::step(&mut domain.core, &Env { now: env.now, wall: env.wall, limits: env.limits.core }, event);
            core_out(domain, env, decision, requests, work);
        }
        Work::Finish => {}
        Work::ResumeFleet => {
            let requests =
                core::resume_fleet(&mut domain.core, &Env { now: env.now, wall: env.wall, limits: env.limits.core });
            core_out(domain, env, decision, requests, work);
            let mut hub_out = Queue::with_capacity(host::max_out(&env.limits.host));
            host::resume(
                &mut domain.host,
                &Env { now: env.now, wall: env.wall, limits: env.limits.host },
                &mut hub_out,
            );
            host_out(domain, env, decision, &mut hub_out, work);
        }
        Work::Infrastructure(event) => {
            let mut out = Queue::with_capacity(infrastructure::MAX_OUT);
            infrastructure::step(
                &mut domain.infrastructure,
                &Env { now: env.now, wall: env.wall, limits: env.limits.infrastructure },
                event,
                &mut out,
            );
            infrastructure_out(domain, env, decision, &mut out, work);
        }
        Work::Observability(event) => {
            let mut out = Queue::with_capacity(observability::MAX_OUT);
            observability::step(
                &mut domain.observability,
                &Env { now: env.now, wall: env.wall, limits: env.limits.observability },
                event,
                &mut out,
            );
            observability_out(domain, env, decision, &mut out, work);
        }
        Work::Host(event) => {
            let mut out = Queue::with_capacity(host::max_out(&env.limits.host));
            host::step(
                &mut domain.host,
                &Env { now: env.now, wall: env.wall, limits: env.limits.host },
                event,
                &mut out,
            );
            host_out(domain, env, decision, &mut out, work);
        }
        Work::Restart(request) => crate::restart::route(domain, env, decision, request, work),
        Work::SwitchChecked => {
            if domain.core.restart_checked() {
                held(decision, Output::Switch);
            }
        }
        Work::AdoptClaims => {
            // Tasks's restoration terminal produces the claims. Feed it first,
            // then carry the complete range to the fleet as the core asks.
            if domain.core.restart_admit_claims() {
                for _ in 0..domain.core.adopted.len() {
                    work.push(Work::Core(core::Event::Fleet(domain.core.adopted.pop().expect("loaded claim count"))));
                }
                work.push(Work::Core(core::Event::Fleet(fleet::Event::Loaded)));
                work.push(Work::AdoptDone);
            }
        }
        Work::AdoptDone => {
            let request = domain.core.restart_done(core::RestartStep::AdoptRuns);
            work.push(Work::Restart(request));
        }
    }
}

#[expect(clippy::too_many_lines, reason = "the application input vocabulary is routed exhaustively")]
fn event_in(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision<Write, Output>,
    event: Event,
    work: &mut Queue<Work>,
) {
    match event {
        Event::WatchApplication { watcher, sign_in, scope, subject, window } => {
            let token = domain.token();
            let number = crate::client_documents::prepare(domain, token, subject, work);
            work.push(Work::Core(core::Event::Watch {
                watcher,
                sign_in,
                subject: core::documents::Watched::Application { scope, connector: number, subject: token },
                window,
            }));
        }
        Event::ReadApplication { reply, sign_in, scope, subject } => {
            let token = domain.token();
            let number = crate::client_documents::prepare(domain, token, subject, work);
            work.push(Work::Core(core::Event::Read {
                reply,
                sign_in,
                read: core::reads::Read::Application {
                    scope,
                    connector: number,
                    read: token,
                    window: core::documents::Window { rows: env.limits.core.documents.rows, after: None },
                },
            }));
        }
        Event::Application { reply_to, sign_in, key, project, digest, value } => {
            let owner = domain.token();
            work.push(Work::Infrastructure(infrastructure::Event::ApplicationPrepare { owner, value }));
            work.push(Work::Core(core::Event::Application {
                reply_to,
                sign_in,
                key,
                scope: jig_domain_people::ApplicationScope::Project(project),
                connector: jig_ops_domain_connectors::number(Connector::Infrastructure),
                digest,
                owner,
                carried: None,
            }));
        }
        Event::Core(event) => work.push(Work::Core(event)),
        Event::Infrastructure(event) => work.push(Work::Infrastructure(event)),
        Event::Observability(event) => work.push(Work::Observability(event)),
        Event::Host(event) => work.push(Work::Host(event)),
        Event::Restart { pending } => {
            domain.restarting = true;
            let request = domain.core.restart_begin(pending);
            work.push(Work::Restart(request));
        }
        Event::Store(store) => crate::restart::store(domain, env, decision, store, work),
        Event::Released(released) => match released {
            Released::Core(event) => work.push(Work::Core(event)),
            Released::Fleet(event) => work.push(Work::Core(core::Event::Fleet(event))),
            Released::Host(event) => work.push(Work::Host(event)),
            Released::Infrastructure(event) => work.push(Work::Infrastructure(event)),
            Released::Observability(event) => work.push(Work::Observability(event)),
            Released::RestartDone(step) => {
                if !domain.core.stopping() {
                    let request = domain.core.restart_done(step);
                    work.push(Work::Restart(request));
                }
            }
            Released::Restart(step) => work.push(Work::Restart(core::RestartRequest::Step(step))),
            Released::Procedure { task, step: _, connector, code: _ } => {
                match jig_ops_domain_connectors::connector(connector) {
                    Some(Connector::Infrastructure) => {
                        work.push(Work::Infrastructure(infrastructure::Event::Procedure {
                            task,
                            signal: infrastructure::ProcedureSignal::Step,
                        }));
                    }
                    Some(Connector::Observability) | None => {}
                }
            }
        },
        Event::ResultRead { payload, end } => crate::assemble::result_read(domain, payload, end, work),
        Event::CallAnswered { to, call } => {
            let answer = domain.payload(Payload::Settled(call));
            work.push(Work::Core(core::Event::Fleet(fleet::Event::Relayed { to, answer })));
        }
        Event::ConnectorRead { to, task, name, tool, read } => {
            crate::assemble::connector_read(domain, env, to, task, name, tool, read, work);
        }
        Event::Effect { to, key, effect, deadline, proposal } => {
            let owner = domain.token();
            domain.effects.insert(owner, effect).expect("admitted decoded effect");
            let origin = match proposal {
                Some(reason) => core::EffectOrigin::Propose { to, key, reason, as_holder: false },
                None => core::EffectOrigin::Call { to, key, deadline },
            };
            work.push(Work::Core(core::Event::EffectStart {
                owner,
                connector: jig_ops_domain_connectors::number(Connector::Infrastructure),
                origin,
            }));
        }
        Event::Timer(timer) => {
            let requests =
                core::fire(&mut domain.core, &Env { now: env.now, wall: env.wall, limits: env.limits.core }, timer);
            core_out(domain, env, decision, requests, work);
        }
        Event::TurnTimer => crate::turn_backoff::fire(domain, env, decision, work),
        Event::InfrastructureTimer => {
            let mut out = Queue::with_capacity(infrastructure::MAX_OUT);
            infrastructure::fire(
                &mut domain.infrastructure,
                &Env { now: env.now, wall: env.wall, limits: env.limits.infrastructure },
                &mut out,
            );
            infrastructure_out(domain, env, decision, &mut out, work);
        }
        Event::ObservabilityTimer => {
            let mut out = Queue::with_capacity(observability::MAX_OUT);
            observability::fire(
                &mut domain.observability,
                &Env { now: env.now, wall: env.wall, limits: env.limits.observability },
                &mut out,
            );
            observability_out(domain, env, decision, &mut out, work);
        }
        Event::Agent(report) => work.push(Work::Host(host::Event::from_agent(report))),
        Event::AgentContent { owner, content } => agent_content(domain, env, decision, work, owner, content),
    }
}

pub(crate) fn core_out(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision<Write, Output>,
    requests: core::Requests,
    work: &mut Queue<Work>,
) {
    let core::Requests::Out(mut out) = requests;
    for _ in 0..out.len() {
        match out.pop().expect("core request count") {
            core::Request::Write(core::Write::CopySettingsRevision(number)) => {
                write(domain, decision, Write::CopySettingsRevision(number));
            }
            core::Request::Write(core::Write::Save(record)) => {
                write(domain, decision, Write::Save(Record::Core(record)));
            }
            core::Request::Write(core::Write::Erase(key)) => write(domain, decision, Write::Erase(Key::Core(key))),
            core::Request::Held(value) => core_held(domain, env, decision, *value, work),
            core::Request::Now(value) => core_now(domain, env, *value, work),
            core::Request::Ask { connector, ask } => core_ask(domain, env, decision, connector, ask, work),
            core::Request::ProjectMade { project } => held(decision, Output::ProjectMade { project }),
            core::Request::Decided => {}
        }
    }
}
#[expect(clippy::too_many_lines, reason = "one dispatcher preserves the full held output vocabulary")]
fn core_held(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision<Write, Output>,
    value: core::Held,
    work: &mut Queue<Work>,
) {
    match value {
        core::Held::Load(load) => held(decision, Output::CoreLoad(*load)),
        core::Held::ApplicationDelivery { .. } => unreachable!("document assembly owns this synchronous output"),
        core::Held::ApplicationWatchEnded { watcher, connector, subject, why } => {
            crate::client_documents::forget(domain, connector, subject, work);
            held(decision, Output::Core(core::Held::WatchEnded { watcher, why }));
        }
        core::Held::Applied { to, connector, token } => {
            assert!(
                connector == jig_ops_domain_connectors::number(Connector::Infrastructure),
                "application request owner"
            );
            let outcome = domain
                .infrastructure
                .application_outcome(token)
                .expect("kept people token has its retained connector outcome");
            held(decision, Output::Application { to, outcome });
        }
        core::Held::Acted { to, outcome } => {
            let token = to.into_token();
            let key = domain.close_replies.remove(&token).expect("infrastructure act reply");
            let translated = match &outcome {
                jig_domain_people::Outcome::Compound { .. } => infrastructure::CloseOutcome::Closed,
                jig_domain_people::Outcome::Again(_) => infrastructure::CloseOutcome::Again,
                jig_domain_people::Outcome::Refused(jig_domain_people::Refusal::NotOffered) => {
                    infrastructure::CloseOutcome::NotOffered
                }
                jig_domain_people::Outcome::Refused(jig_domain_people::Refusal::Role { held: None }) => {
                    infrastructure::CloseOutcome::NoOwner
                }
                jig_domain_people::Outcome::Refused(jig_domain_people::Refusal::Ended { .. }) => {
                    infrastructure::CloseOutcome::Ended
                }
                jig_domain_people::Outcome::CompoundRefused { .. }
                | jig_domain_people::Outcome::Created { .. }
                | jig_domain_people::Outcome::MarkedSeen { .. }
                | jig_domain_people::Outcome::Applied { .. }
                | jig_domain_people::Outcome::SignedOut
                | jig_domain_people::Outcome::NoteEdited { .. }
                | jig_domain_people::Outcome::ServiceMade { .. }
                | jig_domain_people::Outcome::Adopted { .. }
                | jig_domain_people::Outcome::Proposed { .. }
                | jig_domain_people::Outcome::Withdrawn { .. }
                | jig_domain_people::Outcome::Stopped { .. }
                | jig_domain_people::Outcome::Cancelled { .. }
                | jig_domain_people::Outcome::Released { .. }
                | jig_domain_people::Outcome::PersonTaken { .. }
                | jig_domain_people::Outcome::PersonHandedBack { .. }
                | jig_domain_people::Outcome::PersonAnswered { .. }
                | jig_domain_people::Outcome::Moved { .. }
                | jig_domain_people::Outcome::ProposalDecided { .. }
                | jig_domain_people::Outcome::Said { .. }
                | jig_domain_people::Outcome::QuestionAnswered { .. }
                | jig_domain_people::Outcome::Amended { .. }
                | jig_domain_people::Outcome::RolesSet { .. }
                | jig_domain_people::Outcome::PolicyChanged { .. }
                | jig_domain_people::Outcome::PoolSet { .. }
                | jig_domain_people::Outcome::Renamed { .. }
                | jig_domain_people::Outcome::ProjectMade { .. }
                | jig_domain_people::Outcome::SettingsChanged { .. }
                | jig_domain_people::Outcome::EscalationDecided { .. }
                | jig_domain_people::Outcome::AccountSignInStarted { .. }
                | jig_domain_people::Outcome::AccountSignedOut { .. }
                | jig_domain_people::Outcome::AccountKeyKept { .. }
                | jig_domain_people::Outcome::AccountProgramDeclared { .. }
                | jig_domain_people::Outcome::AccountChanged
                | jig_domain_people::Outcome::AccountMovingOut { .. }
                | jig_domain_people::Outcome::AccountFinishing { .. }
                | jig_domain_people::Outcome::AccountMovedIn { .. }
                | jig_domain_people::Outcome::AccountRemoved { .. }
                | jig_domain_people::Outcome::Refused(_) => infrastructure::CloseOutcome::Refused,
            };
            // Keep the complete answer as a report fact, including saved
            // refusals; the connector owns retention and retry decisions.
            held(decision, Output::Core(core::Held::Acted { to: ReplyTo::new(token), outcome }));
            child(decision, Released::Infrastructure(infrastructure::Event::Closed { key, outcome: translated }));
        }
        core::Held::MakeEffect { connector, entry } => match jig_ops_domain_connectors::connector(connector) {
            Some(Connector::Infrastructure) => {
                child(decision, Released::Infrastructure(infrastructure::Event::MakeEntry { entry }));
            }
            Some(Connector::Observability) => {
                child(decision, Released::Observability(observability::Event::MakeEntry { entry }));
            }
            None => {}
        },
        core::Held::Assign { channel, run, attempt, .. } => {
            assert!(channel == Token::new(0), "ops engine's permanent link");
            let assignment = domain.assignments.remove(&run.raw()).expect("core asked to retain its assignment");
            assert!(assignment.attempt == attempt.raw(), "assignment fence");
            child(decision, Released::Host(host::Event::Down(host::LinkDown::Assign { assignment: assignment.value })));
        }
        core::Held::SettledCall { to, key: _, call } => {
            let answer = domain.payload(Payload::Settled(call));
            work.push(Work::Core(core::Event::Fleet(fleet::Event::Relayed { to, answer })));
        }
        core::Held::Grant { channel, run, attempt, grant } => {
            assert!(channel == Token::new(0), "ops engine's permanent link");
            child(
                decision,
                Released::Host(host::Event::Down(host::LinkDown::Grant {
                    run,
                    attempt,
                    grant: crate::assemble::host_grant(grant),
                })),
            );
        }
        core::Held::Relayed { run, attempt, call, answer, .. } => {
            if let Some(released) = crate::assemble::relayed(domain, run, attempt, call, answer) {
                child(decision, released);
            }
        }
        core::Held::Inbound { run, attempt, message, .. } => {
            let value = domain.payloads.remove(&message.words).expect("message token");
            let message_word = match value {
                Payload::Word(message_word) => message_word,
                Payload::Turn { .. } | Payload::Answer { .. } | Payload::Settled(_) => {
                    unreachable!("inbox word family")
                }
            };
            child(
                decision,
                Released::Host(host::Event::Down(host::LinkDown::Message {
                    run,
                    attempt,
                    name: message.name,
                    sender: crate::assemble::sender(message_word.from),
                    finish_requested: message_word.finish_requested(),
                    words: jig_protocol_tools::words(&message_word),
                })),
            );
        }
        core::Held::Relay { task, attempt, word, .. } => {
            let name = Token::new(word.number);
            let words = domain.payload(Payload::Word(word));
            child(
                decision,
                Released::Fleet(fleet::Event::Inbound {
                    run: Token::new(task),
                    attempt: Token::new(attempt),
                    message: fleet::Message { name, sender: Token::new(0), words },
                }),
            );
        }
        core::Held::TaskTurnKept { task: run, attempt, turn } => {
            child(
                decision,
                Released::Fleet(fleet::Event::TurnKept { run: Token::new(run), attempt: Token::new(attempt), turn }),
            );
        }
        core::Held::TurnBusy { channel, run, attempt, turn } if channel == Token::new(0) => {
            crate::turn_backoff::busy(domain, env, run, attempt, turn);
        }
        core::Held::AcknowledgeTurn { run, attempt, turn, .. } => {
            domain.turn_backoffs.cancel((run, attempt, turn));
            child(decision, Released::Host(host::Event::Down(host::LinkDown::TurnKept { run, attempt, turn })));
        }
        core::Held::Cancel { run, attempt, .. } => {
            child(decision, Released::Host(host::Event::Down(host::LinkDown::Cancel { run, attempt })));
        }
        core::Held::StopRun { task, attempt } => child(
            decision,
            Released::Fleet(fleet::Event::Cancel { run: Token::new(task), attempt: Token::new(attempt) }),
        ),
        core::Held::Acknowledge { run, attempt, .. } => {
            child(decision, Released::Host(host::Event::Down(host::LinkDown::Acknowledge { run, attempt })));
        }
        core::Held::TaskTerminalAcknowledged { task, attempt } => child(
            decision,
            Released::Fleet(fleet::Event::Acknowledge { run: Token::new(task), attempt: Token::new(attempt) }),
        ),
        other @ (core::Held::CallAnswer { .. }
        | core::Held::ReadLoad { .. }
        | core::Held::Read { .. }
        | core::Held::Delivery { .. }
        | core::Held::WatchEnded { .. }
        | core::Held::PeopleReply { .. }
        | core::Held::CarriedUnused { .. }
        | core::Held::Result { .. }
        | core::Held::Drained
        | core::Held::Sources { .. }
        | core::Held::CheckSource { .. }
        | core::Held::SignOutSource { .. }
        | core::Held::SourceSignedOut { .. }
        | core::Held::Refuse { .. }
        | core::Held::TurnBusy { .. }
        | core::Held::NotesLoad { .. }
        | core::Held::NotesWritten { .. }
        | core::Held::NotesDeleted { .. }) => held(decision, Output::Core(other)),
    }
}

#[expect(clippy::too_many_lines, reason = "one exhaustive dispatcher translates this child boundary")]
fn core_ask(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision<Write, Output>,
    number: u16,
    ask: core::Ask,
    work: &mut Queue<Work>,
) {
    match ask {
        core::Ask::StepMade { task, step, first: Some(first), .. } => {
            crate::steps::made(number, task, step, Some(first), work);
        }
        core::Ask::StepRefused { task, step, .. } => crate::steps::made(number, task, step, None, work),
        core::Ask::DropDocument { subject } => crate::client_documents::forget(domain, number, subject, work),
        core::Ask::Document { watcher, scope, subject, party, window: _ } => {
            crate::client_documents::ask_document(domain, number, watcher, scope, subject, party, work);
        }
        core::Ask::Read { reply, scope, read, party, window: _ } => {
            crate::client_documents::read(domain, env, decision, number, reply, scope, read, party, work);
        }
        core::Ask::Application { request, token, owner, person, scope, role, retention } => {
            let (jig_domain_people::ApplicationScope::Project(project), Some(role)) = (scope, role) else {
                work.push(Work::Core(core::Event::People(jig_domain_people::Event::Decided {
                    request,
                    outcome: jig_domain_people::Outcome::Refused(jig_domain_people::Refusal::NotOffered),
                })));
                return;
            };
            assert!(number == jig_ops_domain_connectors::number(Connector::Infrastructure), "application owner");
            work.push(Work::Infrastructure(infrastructure::Event::Application {
                request,
                token,
                owner,
                person,
                project,
                role: role.number(),
                retention,
            }));
        }
        core::Ask::DropApplication { owner } => match jig_ops_domain_connectors::connector(number) {
            Some(Connector::Infrastructure) => {
                work.push(Work::Infrastructure(infrastructure::Event::ApplicationDrop { owner }));
            }
            Some(Connector::Observability) | None => {}
        },
        core::Ask::ApplicationPartAnswered { .. } => unreachable!("infrastructure requests ask for no core part"),
        core::Ask::Effect(ask) => effect_ask(domain, env, number, ask, work),
        core::Ask::TaskHoldings { request, executor, spec, .. } => work.push(Work::Core(core::Event::Holdings {
            request,
            connector: number,
            holdings: translate::holdings(number, executor, &spec),
        })),
        core::Ask::DelegateHoldings { request, members, .. } => {
            let mut holdings = List::with_capacity(u32::try_from(members.len()).expect("bounded batch"));
            let mut valid = true;
            for member in members {
                match translate::holdings(number, member.executor, &member.spec) {
                    Some(value) => holdings.push(value).expect("one holdings row"),
                    None => valid = false,
                }
            }
            work.push(Work::Core(core::Event::DelegateHoldings {
                request,
                connector: number,
                holdings: if valid { Some(holdings.into_boxed()) } else { None },
            }));
        }
        core::Ask::ProcedureHoldings { task, step, members } => {
            let mut holdings = List::with_capacity(u32::try_from(members.len()).expect("bounded batch"));
            let mut valid = true;
            for member in members {
                match translate::holdings(number, member.executor, &member.spec) {
                    Some(value) => holdings.push(value).expect("one holdings row"),
                    None => valid = false,
                }
            }
            work.push(Work::Core(core::Event::ProcedureHoldings {
                task,
                step,
                connector: number,
                holdings: if valid { Some(holdings.into_boxed()) } else { None },
            }));
        }
        core::Ask::StartProcedure { context, step } => {
            let task = context.task;
            let code = match context.executor {
                tasks::Executor::Procedure { code, .. } => code,
                tasks::Executor::Agent { .. } | tasks::Executor::Person(_) => {
                    unreachable!("core-selected procedure executor")
                }
            };
            domain.procedures.insert(task, (step, number, code)).expect("core-selected procedure continuation");
            match jig_ops_domain_connectors::connector(number) {
                Some(Connector::Infrastructure) => {
                    match translate::procedure(code, &context.spec, domain.core.counters.deployment().id) {
                        Some(procedure) => {
                            work.push(Work::Infrastructure(infrastructure::Event::StartProcedure {
                                task,
                                project: context.project,
                                chat: served_chat(domain, task),
                                pending: context.pending,
                                added: translate::infrastructure_added(context.added),
                                procedure,
                            }));
                        }
                        None => work.push(Work::Core(core::Event::ProcedureStep {
                            task,
                            step,
                            connector: number,
                            code,
                            decision: core::ProcedureStep::new(core::StepEnd::Hold(tasks::Hold::Procedure)),
                        })),
                    }
                    child(decision, Released::Procedure { task, step, connector: number, code });
                }
                Some(Connector::Observability) => match translate::spec_service(&context.spec) {
                    Some(service) => {
                        work.push(Work::Observability(observability::Event::StartWatch {
                            task,
                            project: context.project,
                            services: Box::new([observability::Service::new(service.environment, service.name)]),
                            template: u16::try_from(translate::spec_number(&context.spec, 3).unwrap_or(1))
                                .expect("watch template"),
                        }));
                        if let Some(row) = domain.core.tasks.task(task) {
                            for subscription in &row.subscriptions {
                                match subscription.kind {
                                    tasks::SubscriptionKind::Topic { connector, topic } => {
                                        if connector == number && topic == task {
                                            work.push(Work::Observability(observability::Event::LinkWatch {
                                                task,
                                                subscription: subscription.number,
                                            }));
                                        }
                                    }
                                    tasks::SubscriptionKind::Task { .. } | tasks::SubscriptionKind::Timer { .. } => {}
                                }
                            }
                        }
                        let mut messages = List::with_capacity(env.limits.core.tasks.inbox_messages);
                        for word in &context.inbox {
                            match &word.kind {
                                tasks::MessageKind::News { .. } => {
                                    messages.push(observability::WatchMessage::News).expect("offered inbox bound");
                                }
                                tasks::MessageKind::Result(result) => match word.from {
                                    tasks::Party::Task(child) => messages
                                        .push(translate::watch_result(child, result, &env.limits.observability))
                                        .expect("offered inbox bound"),
                                    tasks::Party::Person(_) | tasks::Party::Deployment { .. } => {}
                                },
                                tasks::MessageKind::Escalation { .. }
                                | tasks::MessageKind::Proposal { .. }
                                | tasks::MessageKind::ProposalDecision { .. }
                                | tasks::MessageKind::EffectNotMade { .. }
                                | tasks::MessageKind::Words
                                | tasks::MessageKind::Met { .. }
                                | tasks::MessageKind::FinishRequested { .. }
                                | tasks::MessageKind::Amendment { .. }
                                | tasks::MessageKind::Question
                                | tasks::MessageKind::Answer { .. }
                                | tasks::MessageKind::Notice { .. }
                                | tasks::MessageKind::Timer { .. }
                                | tasks::MessageKind::PersonStalled { .. } => {}
                            }
                        }
                        work.push(Work::Observability(observability::Event::WatchStep {
                            task,
                            step,
                            batch: context.last_message,
                            messages: messages.into_boxed(),
                        }));
                    }
                    None => work.push(Work::Core(core::Event::ProcedureStep {
                        task,
                        step,
                        connector: number,
                        code,
                        decision: core::ProcedureStep::new(core::StepEnd::Hold(tasks::Hold::Procedure)),
                    })),
                },
                None => {}
            }
        }
        core::Ask::ClaimDone { task, attempt, kinds, charter } => match jig_ops_domain_connectors::connector(number) {
            Some(Connector::Infrastructure) => {
                child(
                    decision,
                    Released::Fleet(fleet::Event::Start {
                        placement: Some(domain.assignments.get(&task).expect("prepared assignment").placement),
                        grants: crate::assemble::fleet_grants(
                            &domain.assignments.get(&task).expect("prepared assignment").value.assignment.grants,
                        ),
                        reply_to: ReplyTo::new(Token::new(task)),
                        run: Token::new(task),
                        attempt: Token::new(attempt),
                        workstream: task,
                        assignment: fleet::Assignment { turns: Token::new(task), answered: Token::new(task) },
                        kinds,
                        charter,
                    }),
                );
            }
            Some(Connector::Observability) | None => {}
        },
        core::Ask::Close { task, .. } => match jig_ops_domain_connectors::connector(number) {
            Some(Connector::Infrastructure) => {
                work.push(Work::Infrastructure(infrastructure::Event::Close { task }));
            }
            Some(Connector::Observability) => {
                work.push(Work::Observability(observability::Event::Close { task }));
            }
            None => {}
        },
        core::Ask::Release { task, .. } => match jig_ops_domain_connectors::connector(number) {
            Some(Connector::Infrastructure) => {
                work.push(Work::Infrastructure(infrastructure::Event::Release { task }));
            }
            Some(Connector::Observability) => {
                work.push(Work::Observability(observability::Event::Release { task }));
            }
            None => {}
        },
        core::Ask::Gather { section, .. } | core::Ask::CutTo { section, .. } => {
            work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::SectionReady {
                section,
                size: None,
            })));
        }
        core::Ask::ProjectGoal { feed } => {
            // These two connectors have no goal projection. Their closing
            // hand-off has no pending writes to settle.
            if feed.closing {
                work.push(Work::Core(core::Event::ProjectionSettled { goal: feed.goal.number, connector: number }));
            }
        }
        core::Ask::SubscriptionDone { request, key, subscription } => {
            match jig_ops_domain_connectors::connector(number) {
                Some(Connector::Observability) => {
                    work.push(Work::Observability(observability::Event::LinkWatch { task: key.task, subscription }));
                }
                Some(Connector::Infrastructure) | None => {}
            }
            // The core broadcasts this completed task operation to every
            // connector. The permanent hub receives one assembled answer.
            match jig_ops_domain_connectors::connector(number) {
                Some(Connector::Infrastructure) => {
                    work.push(Work::Core(core::Event::NamedAnswer {
                        to: ReplyTo::new(request),
                        key,
                        part: core::CallPart::Subscribed { subscription },
                    }));
                }
                Some(Connector::Observability) | None => {}
            }
        }
        core::Ask::UnsubscriptionDone { request, key } => match jig_ops_domain_connectors::connector(number) {
            Some(Connector::Infrastructure) => {
                work.push(Work::Core(core::Event::NamedAnswer {
                    to: ReplyTo::new(request),
                    key,
                    part: core::CallPart::Unsubscribed,
                }));
            }
            Some(Connector::Observability) | None => {}
        },
        // These proof-of-concept connectors provide no projection, brief
        // section, adoption, workspace or writer-handoff state. A copying
        // application fills these arms when its connectors expose it.
        core::Ask::StepMade { first: None, .. }
        | core::Ask::ForgetProjection { .. }
        | core::Ask::Adopt { .. }
        | core::Ask::Hold { .. }
        | core::Ask::EndTopic { .. }
        | core::Ask::Drop { .. }
        | core::Ask::Lost { .. }
        | core::Ask::DropSubscription { .. }
        | core::Ask::RepairRefused { .. }
        | core::Ask::DelegateRefused { .. } => {}
    }
}
#[expect(clippy::too_many_lines, reason = "one exhaustive dispatcher translates this child boundary")]
#[expect(clippy::manual_map, reason = "step code copies owner scope without closure-taking methods")]
fn effect_ask(domain: &mut Domain, env: &Env<Limits>, number: u16, ask: core::connector::Ask, work: &mut Queue<Work>) {
    match ask {
        core::connector::Ask::Describe { owner } => match domain.effects.remove(&owner) {
            Some(effect) => work.push(Work::Infrastructure(infrastructure::Event::Describe { token: owner, effect })),
            None => {
                work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::DescribeRefused { owner })));
            }
        },
        core::connector::Ask::DescribeProposal { owner, proposal } => {
            work.push(Work::Infrastructure(infrastructure::Event::DescribeProposal { token: owner, number: proposal }));
        }
        core::connector::Ask::KeepProposal { owner, proposal, task, project: _ } => {
            domain.effect_procedures.remove(&owner);
            work.push(Work::Infrastructure(infrastructure::Event::KeepProposal {
                token: owner,
                number: proposal,
                task,
            }));
        }
        core::connector::Ask::DropProposal { proposal } => {
            work.push(Work::Infrastructure(infrastructure::Event::DropProposal { number: proposal }));
        }
        core::connector::Ask::Keep { owner, entry, key, .. } => {
            domain.effect_procedures.remove(&owner);
            match jig_ops_domain_connectors::connector(number) {
                Some(Connector::Infrastructure) => {
                    work.push(Work::Infrastructure(infrastructure::Event::Keep {
                        token: owner,
                        entry,
                        key: translate::infrastructure_key(key),
                        project: match domain.core.tasks.task(key.task) {
                            Some(task) => Some(task.project),
                            None => None,
                        },
                    }));
                }
                Some(Connector::Observability) => {
                    work.push(Work::Observability(observability::Event::Keep {
                        token: owner,
                        entry,
                        key: translate::observability_key(key),
                        project: match domain.core.tasks.task(key.task) {
                            Some(task) => Some(task.project),
                            None => None,
                        },
                    }));
                }
                None => {}
            }
        }
        core::connector::Ask::Drop { owner, answer } => {
            if let Some(task) = domain.effect_procedures.remove(&owner) {
                let signal = match answer {
                    authority::Answer::Allow => infrastructure::ProcedureSignal::EffectMade,
                    authority::Answer::Wait => infrastructure::ProcedureSignal::EffectWaiting,
                    authority::Answer::Propose | authority::Answer::Refuse => {
                        infrastructure::ProcedureSignal::EffectFailed
                    }
                };
                work.push(Work::Infrastructure(infrastructure::Event::Procedure { task, signal }));
            }
            domain.effects.remove(&owner);
            match jig_ops_domain_connectors::connector(number) {
                Some(Connector::Infrastructure) => {
                    work.push(Work::Infrastructure(infrastructure::Event::Drop { token: owner }));
                }
                Some(Connector::Observability) => {
                    work.push(Work::Observability(observability::Event::Drop { token: owner }));
                }
                None => {}
            }
        }
        core::connector::Ask::Judge { owner, judge, state, resources } => {
            let token = domain.token();
            match translate::service(&resources) {
                Some(service) => match translate::requirement(judge.requirement) {
                    Some(requirement) => {
                        domain.judges.insert(token, (owner, judge, state)).expect("core-selected judge continuation");
                        work.push(Work::Observability(observability::Event::Judge {
                            token,
                            requirement,
                            service,
                            freshness: 30,
                        }));
                    }
                    None => work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::Verdict {
                        owner,
                        judge,
                        verdict: authority::Verdict::Refuse,
                        at: env.wall,
                        guarded: false,
                        state,
                    }))),
                },
                None => work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::Verdict {
                    owner,
                    judge,
                    verdict: authority::Verdict::Refuse,
                    at: env.wall,
                    guarded: false,
                    state,
                }))),
            }
        }
        core::connector::Ask::ResolveUncertain { entry, again } => match jig_ops_domain_connectors::connector(number) {
            Some(Connector::Infrastructure) => {
                work.push(Work::Infrastructure(infrastructure::Event::ResolveEntry { entry, repeat: again }));
            }
            Some(Connector::Observability) | None => {}
        },
        core::connector::Ask::Make { entry } => match jig_ops_domain_connectors::connector(number) {
            Some(Connector::Infrastructure) => {
                work.push(Work::Infrastructure(infrastructure::Event::MakeEntry { entry }));
            }
            Some(Connector::Observability) => {
                work.push(Work::Observability(observability::Event::MakeEntry { entry }));
            }
            None => {}
        },
        core::connector::Ask::ReadAfresh => match jig_ops_domain_connectors::connector(number) {
            Some(Connector::Infrastructure) => {
                work.push(Work::Infrastructure(infrastructure::Event::ReadAfresh));
            }
            Some(Connector::Observability) => {
                work.push(Work::Observability(observability::Event::ReadAfresh));
            }
            None => {}
        },
        core::connector::Ask::SettleOutbox => match jig_ops_domain_connectors::connector(number) {
            Some(Connector::Infrastructure) => {
                work.push(Work::Infrastructure(infrastructure::Event::Restart));
            }
            Some(Connector::Observability) => {
                work.push(Work::Observability(observability::Event::Restart));
            }
            None => {}
        },
        core::connector::Ask::Names { .. }
        | core::connector::Ask::Unname { .. }
        | core::connector::Ask::Hold { .. }
        | core::connector::Ask::ReleaseHold { .. }
        | core::connector::Ask::Writer { .. }
        | core::connector::Ask::ProcedureMade { .. }
        | core::connector::Ask::ProcedureActivate { .. }
        | core::connector::Ask::ProcedureMessage { .. }
        | core::connector::Ask::ProcedureClose { .. }
        | core::connector::Ask::Project { .. }
        | core::connector::Ask::Release { .. }
        | core::connector::Ask::Subscribe { .. }
        | core::connector::Ask::Unsubscribe { .. }
        | core::connector::Ask::Read { .. }
        | core::connector::Ask::Gather { .. }
        | core::connector::Ask::CutTo { .. }
        | core::connector::Ask::Take { .. }
        | core::connector::Ask::Prepare { .. }
        | core::connector::Ask::Left { .. }
        | core::connector::Ask::Adopt { .. }
        | core::connector::Ask::Restore { .. } => {}
    }
}

#[expect(clippy::too_many_lines, reason = "one exhaustive dispatcher translates this child boundary")]
pub(crate) fn infrastructure_out(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision<Write, Output>,
    out: &mut Queue<infrastructure::Request>,
    work: &mut Queue<Work>,
) {
    for _ in 0..out.len() {
        match out.pop().expect("infrastructure requests") {
            infrastructure::Request::Document { watcher, subject, document } => {
                let document = match document {
                    Ok(document) => Ok(crate::ClientDocument::Infrastructure(Box::new(document))),
                    Err(why) => Err(match why {
                        infrastructure::DocumentMissing::Unknown => jig_domain_views::End::Gone,
                        infrastructure::DocumentMissing::NotAllowed => jig_domain_views::End::NotAllowed,
                    }),
                };
                crate::client_documents::document(
                    domain,
                    env,
                    decision,
                    jig_ops_domain_connectors::number(Connector::Infrastructure),
                    watcher,
                    subject,
                    document,
                    work,
                );
            }
            infrastructure::Request::ReadAnswered { .. } => {
                unreachable!("client read completes in its synchronous route")
            }
            infrastructure::Request::Marked { subjects } => {
                work.push(Work::Core(core::Event::Marked {
                    connector: jig_ops_domain_connectors::number(Connector::Infrastructure),
                    subjects,
                }));
            }
            infrastructure::Request::ApplicationDone { request, token } => {
                work.push(Work::Core(core::Event::ApplicationFinished { request, outcome: Ok(token) }));
            }
            infrastructure::Request::ApplicationBusy { request } => {
                work.push(Work::Core(core::Event::ApplicationFinished {
                    request,
                    outcome: Err(jig_domain_people::Again::Busy),
                }));
            }
            infrastructure::Request::Closed { task } => {
                work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::Closed {
                    task,
                    connector: jig_ops_domain_connectors::number(Connector::Infrastructure),
                })));
            }
            infrastructure::Request::Released { task } => {
                work.push(Work::Core(core::Event::Tasks(tasks::Event::Settled { task })));
            }
            infrastructure::Request::Save { record } => {
                write(domain, decision, Write::Save(Record::Infrastructure(record)));
            }
            infrastructure::Request::Erase { key } => write(domain, decision, Write::Erase(Key::Infrastructure(key))),
            infrastructure::Request::System(request) => held(decision, Output::Infrastructure(request)),
            infrastructure::Request::Described { token, description, waits_for_dependencies } => {
                work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::Described {
                    owner: token,
                    description: Box::new(translate::infrastructure_description(
                        jig_ops_domain_connectors::number(Connector::Infrastructure),
                        description,
                        waits_for_dependencies,
                    )),
                })));
            }
            infrastructure::Request::Refused { token } => {
                work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::DescribeRefused {
                    owner: token,
                })));
            }
            infrastructure::Request::Make { key } => {
                child(decision, Released::Infrastructure(infrastructure::Event::Make { key }));
            }
            infrastructure::Request::Outcome { entry, key, outcome } => {
                work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::Outbox {
                    connector: jig_ops_domain_connectors::number(Connector::Infrastructure),
                    entry,
                    task: key.task,
                    outcome: translate::infrastructure_outcome(outcome),
                })));
                match key.origin {
                    infrastructure::Purpose::Procedure { .. } => {
                        work.push(Work::Infrastructure(infrastructure::Event::Procedure {
                            task: key.task,
                            signal: match outcome {
                                infrastructure::Outcome::Made => infrastructure::ProcedureSignal::EffectMade,
                                infrastructure::Outcome::Failed | infrastructure::Outcome::Skipped => {
                                    infrastructure::ProcedureSignal::EffectFailed
                                }
                                infrastructure::Outcome::Uncertain => infrastructure::ProcedureSignal::Step,
                            },
                        }));
                    }
                    infrastructure::Purpose::Call { .. } | infrastructure::Purpose::Projection { .. } => {}
                }
            }
            infrastructure::Request::Step { task, decision: action } => {
                let Some((step, connector, code)) = domain.procedures.remove(&task) else { continue };
                match action {
                    infrastructure::StepDecision::Effect(effect) => {
                        let owner = domain.token();
                        domain.effects.insert(owner, effect).expect("admitted procedure effect");
                        domain.effect_procedures.insert(owner, task).expect("procedure effect continuation");
                        work.push(Work::Core(core::Event::EffectStart {
                            owner,
                            connector,
                            origin: core::EffectOrigin::Procedure { task, step, entry: None },
                        }));
                    }
                    infrastructure::StepDecision::Wait { .. } => work.push(Work::Core(core::Event::ProcedureStep {
                        task,
                        step,
                        connector,
                        code,
                        decision: core::ProcedureStep::new(core::StepEnd::Wait),
                    })),
                    infrastructure::StepDecision::Finish => work.push(Work::Core(core::Event::ProcedureStep {
                        task,
                        step,
                        connector,
                        code,
                        decision: core::ProcedureStep::new(core::StepEnd::Result(tasks::TaskResult::Report {
                            words: Box::new([]),
                        })),
                    })),
                    infrastructure::StepDecision::FinishReport { words } => {
                        work.push(Work::Core(core::Event::ProcedureStep {
                            task,
                            step,
                            connector,
                            code,
                            decision: core::ProcedureStep::new(core::StepEnd::Result(tasks::TaskResult::Report {
                                words,
                            })),
                        }));
                    }
                    infrastructure::StepDecision::Hold { .. } => work.push(Work::Core(core::Event::ProcedureStep {
                        task,
                        step,
                        connector,
                        code,
                        decision: core::ProcedureStep::new(core::StepEnd::Hold(tasks::Hold::Effects)),
                    })),
                }
            }
            infrastructure::Request::Drift { task, resource } => {
                work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::Drift {
                    task,
                    resource: tasks::Name {
                        connector: jig_ops_domain_connectors::number(Connector::Infrastructure),
                        path: resource.segments(),
                    },
                })));
            }
            infrastructure::Request::Slots { pool, quota, used: _ } => {
                work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::PoolSlots {
                    name: tasks::Name {
                        connector: jig_ops_domain_connectors::number(Connector::Infrastructure),
                        path: infrastructure::Resource::Pool(pool).segments(),
                    },
                    slots: quota,
                })));
            }
            infrastructure::Request::Wake { task } => {
                work.push(Work::Core(core::Event::Tasks(tasks::Event::WakeProcedure { task })));
            }
            infrastructure::Request::Changed { resource } => {
                for task in domain.infrastructure.readers(&resource) {
                    work.push(Work::Core(core::Event::Tasks(tasks::Event::WakeProcedure { task })));
                }
            }
            infrastructure::Request::Named { project, resources, .. } => {
                for named in resources {
                    let hold = match named.hold {
                        infrastructure::Hold::None => core::connector::HoldKind::Shared,
                        infrastructure::Hold::ExclusiveWait => core::connector::HoldKind::Exclusive { wait: true },
                        infrastructure::Hold::PooledWait => core::connector::HoldKind::Pooled { slots: 0, wait: true },
                    };
                    work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::Resource {
                        project,
                        role: translate::resource_role(named.role),
                        name: tasks::Name {
                            connector: jig_ops_domain_connectors::number(Connector::Infrastructure),
                            path: named.resource.segments(),
                        },
                        hold,
                    })));
                }
            }
            infrastructure::Request::Close { key, project, chat } => {
                let token = domain.token();
                domain.close_replies.insert(token, key).expect("bounded infrastructure close replies");
                let mut grace = None;
                for setting in &domain.core.current.offered() {
                    let number = match setting {
                        core::settings::Setting::Template(number) => *number,
                        core::settings::Setting::Charter(_)
                        | core::settings::Setting::Policy(_)
                        | core::settings::Setting::Rules => continue,
                    };
                    let template = domain.core.current.template(number).expect("offered template");
                    if template.flags.conversation {
                        grace = template.grace;
                        break;
                    }
                }
                work.push(Work::Core(core::Event::Act {
                    reply_to: ReplyTo::new(token),
                    connector: jig_ops_domain_connectors::number(Connector::Infrastructure),
                    key,
                    act: Box::new(jig_domain_people::Act::Close {
                        project,
                        task: chat,
                        grace,
                        reason: b"environment torn down".as_slice().into(),
                    }),
                    carried: None,
                }));
            }
            infrastructure::Request::Restarted { stage } => {
                let step = match stage {
                    infrastructure::RestartStage::ReadAfresh => core::RestartStep::ReadAfresh {
                        connector: jig_ops_domain_connectors::number(Connector::Infrastructure),
                    },
                    infrastructure::RestartStage::Outbox => core::RestartStep::SettleOutbox {
                        connector: jig_ops_domain_connectors::number(Connector::Infrastructure),
                    },
                };
                let request = domain.core.restart_done(step);
                work.push(Work::Restart(request));
            }
        }
    }
}
#[expect(clippy::too_many_lines, reason = "one exhaustive dispatcher translates this child boundary")]
pub(crate) fn observability_out(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision<Write, Output>,
    out: &mut Queue<observability::Request>,
    work: &mut Queue<Work>,
) {
    for _ in 0..out.len() {
        match out.pop().expect("observability requests") {
            observability::Request::Document { watcher, subject, document } => {
                let document = match document {
                    Ok(document) => Ok(crate::ClientDocument::Observability(Box::new(document))),
                    Err(why) => Err(match why {
                        observability::DocumentMissing::Unknown => jig_domain_views::End::Gone,
                        observability::DocumentMissing::NotAllowed => jig_domain_views::End::NotAllowed,
                    }),
                };
                crate::client_documents::document(
                    domain,
                    env,
                    decision,
                    jig_ops_domain_connectors::number(Connector::Observability),
                    watcher,
                    subject,
                    document,
                    work,
                );
            }
            observability::Request::ReadAnswered { .. } => {
                unreachable!("client read completes in its synchronous route")
            }
            observability::Request::Marked { subjects } => {
                work.push(Work::Core(core::Event::Marked {
                    connector: jig_ops_domain_connectors::number(Connector::Observability),
                    subjects,
                }));
            }
            observability::Request::Closed { task } => {
                work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::Closed {
                    task,
                    connector: jig_ops_domain_connectors::number(Connector::Observability),
                })));
            }
            observability::Request::Released { task } => {
                work.push(Work::Core(core::Event::Tasks(tasks::Event::Settled { task })));
            }
            observability::Request::Save { record } => {
                write(domain, decision, Write::Save(Record::Observability(record)));
            }
            observability::Request::Erase { key } => write(domain, decision, Write::Erase(Key::Observability(key))),
            observability::Request::System(request @ observability::SystemRequest::Read { .. }) => {
                now(domain, Output::Observability(request));
            }
            observability::Request::System(
                request @ (observability::SystemRequest::Facts { .. }
                | observability::SystemRequest::Silence { .. }
                | observability::SystemRequest::FindSilence { .. }),
            ) => held(decision, Output::Observability(request)),
            observability::Request::Answer { token, bytes } => {
                if let Some(call) = domain.read_calls.remove(&token) {
                    crate::assemble::read_answer(domain, call, false, bytes, work);
                } else {
                    now(domain, Output::Read { owner: token, bytes });
                }
            }
            observability::Request::Verdict { token, verdict } => {
                let Some((owner, judge, state)) = domain.judges.remove(&token) else { continue };
                let (verdict, at) = translate::verdict(verdict, env.wall);
                work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::Verdict {
                    owner,
                    judge,
                    verdict,
                    at,
                    guarded: false,
                    state,
                })));
            }
            observability::Request::Described { token, effect, waits_for_dependencies } => {
                work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::Described {
                    owner: token,
                    description: Box::new(translate::observability_description(
                        jig_ops_domain_connectors::number(Connector::Observability),
                        effect,
                        waits_for_dependencies,
                    )),
                })));
            }
            observability::Request::Refused { token } => {
                if let Some(call) = domain.read_calls.remove(&token) {
                    crate::assemble::read_answer(domain, call, true, Box::from(&b"unavailable"[..]), work);
                } else {
                    work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::DescribeRefused {
                        owner: token,
                    })));
                }
            }
            observability::Request::Make { key } => {
                child(decision, Released::Observability(observability::Event::Make { key }));
            }
            observability::Request::Outcome { entry, key, outcome } => {
                work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::Outbox {
                    connector: jig_ops_domain_connectors::number(Connector::Observability),
                    entry,
                    task: key.task,
                    outcome: translate::observability_outcome(outcome),
                })));
            }
            observability::Request::News { alert, subscribers } => {
                for subscriber in subscribers {
                    let class = translate::news_class(subscriber.class);
                    if let Some(event) = domain.core.connector_news(
                        subscriber.task,
                        subscriber.subscription,
                        class,
                        translate::alert_words(&alert),
                        env.wall,
                    ) {
                        work.push(Work::Core(core::Event::Tasks(event)));
                    }
                }
            }
            observability::Request::Health { service, healthy, subscribers } => {
                for subscriber in subscribers {
                    let class = translate::news_class(subscriber.class);
                    if let Some(event) = domain.core.connector_news(
                        subscriber.task,
                        subscriber.subscription,
                        class,
                        translate::health_words(&service, healthy),
                        env.wall,
                    ) {
                        work.push(Work::Core(core::Event::Tasks(event)));
                    }
                }
            }
            observability::Request::WatchInterest { task, subscription, topic } => {
                work.push(Work::Core(core::Event::Tasks(tasks::Event::SubscribeTopic {
                    reply_to: ReplyTo::new(Token::new(subscription)),
                    task,
                    subscription: tasks::Subscription {
                        number: subscription,
                        kind: tasks::SubscriptionKind::Topic {
                            connector: jig_ops_domain_connectors::number(Connector::Observability),
                            topic,
                        },
                    },
                })));
            }
            observability::Request::WatchDecision { task, step, decision: chosen } => {
                let Some((offered, connector, code)) = domain.procedures.remove(&task) else { continue };
                assert_eq!(offered, step, "owner answers its offered step");
                let action = match chosen {
                    observability::WatchDecision::Triage { delegate, alerts } => {
                        match translate::triage(&delegate, &alerts) {
                            Some(batch) => {
                                core::ProcedureStep { batch, ..core::ProcedureStep::new(core::StepEnd::Wait) }
                            }
                            None => core::ProcedureStep::new(core::StepEnd::Hold(tasks::Hold::Procedure)),
                        }
                    }
                    observability::WatchDecision::Lead { delegate } => match translate::lead(&delegate) {
                        Some(batch) => core::ProcedureStep { batch, ..core::ProcedureStep::new(core::StepEnd::Wait) },
                        None => core::ProcedureStep::new(core::StepEnd::Hold(tasks::Hold::Procedure)),
                    },
                    observability::WatchDecision::Wait => core::ProcedureStep::new(core::StepEnd::Wait),
                    observability::WatchDecision::Hold => {
                        core::ProcedureStep::new(core::StepEnd::Hold(tasks::Hold::Procedure))
                    }
                };
                work.push(Work::Core(core::Event::ProcedureStep { task, step, connector, code, decision: action }));
            }
            observability::Request::WakeProcedure { task } => {
                child(decision, Released::Core(core::Event::Tasks(tasks::Event::WakeProcedure { task })));
            }
            observability::Request::AssociationRefused { task, step, reason } => {
                held(decision, Output::IncidentAssociationRefused { task, step, reason });
            }
            observability::Request::Triage { watch, delegate, alerts, .. } => {
                let Some((step, connector, code)) = domain.procedures.remove(&watch) else { continue };
                let action = match translate::triage(&delegate, &alerts) {
                    Some(batch) => core::ProcedureStep { batch, ..core::ProcedureStep::new(core::StepEnd::Wait) },
                    None => core::ProcedureStep::new(core::StepEnd::Hold(tasks::Hold::Procedure)),
                };
                work.push(Work::Core(core::Event::ProcedureStep {
                    task: watch,
                    step,
                    connector,
                    code,
                    decision: action,
                }));
            }
            observability::Request::Changed { service } => {
                let resource =
                    infrastructure::Resource::Service(infrastructure::Service::new(service.environment, service.name));
                for task in domain.infrastructure.readers(&resource) {
                    work.push(Work::Core(core::Event::Tasks(tasks::Event::WakeProcedure { task })));
                }
            }
            observability::Request::Restarted { stage } => {
                let step = match stage {
                    observability::RestartStage::ReadAfresh => core::RestartStep::ReadAfresh {
                        connector: jig_ops_domain_connectors::number(Connector::Observability),
                    },
                    observability::RestartStage::Outbox => core::RestartStep::SettleOutbox {
                        connector: jig_ops_domain_connectors::number(Connector::Observability),
                    },
                };
                let request = domain.core.restart_done(step);
                work.push(Work::Restart(request));
            }
        }
    }
}

#[expect(clippy::too_many_lines, reason = "one exhaustive dispatcher translates this child boundary")]
fn core_now(domain: &mut Domain, env: &Env<Limits>, value: core::Now, work: &mut Queue<Work>) {
    match value {
        core::Now::Relayed { run, attempt, call, answer, .. } => {
            if let Some(released) = crate::assemble::relayed(domain, run, attempt, call, answer) {
                now(domain, Output::ToChild(released));
            }
        }
        core::Now::Undelivered { message, .. } => {
            drop(domain.payloads.remove(&message.words));
        }

        core::Now::Activate { context } => {
            work.push(Work::Core(core::Event::Activate { context, ready: domain.ready() }));
        }
        core::Now::PrepareAgent { context } => {
            let transcript_waiter = if context.ever_turned { Some(Token::new(context.task)) } else { None };
            work.push(Work::Core(core::Event::PreparedAgent { context, transcript_waiter, busy: false }));
        }
        core::Now::StartPreparation { task, from: None, .. } => {
            work.push(Work::Core(core::Event::Tasks(tasks::Event::Prepare {
                reply_to: ReplyTo::new(Token::new(task)),
                task,
            })));
            work.push(Work::Core(core::Event::StartBrief { task }));
        }
        value @ core::Now::StartPreparation { task, from: Some(_), .. } => {
            work.push(Work::Core(core::Event::Tasks(tasks::Event::Prepare {
                reply_to: ReplyTo::new(Token::new(task)),
                task,
            })));
            now(domain, Output::CoreLoad(value));
        }
        core::Now::BriefCorePlanned { task, sections, .. } => {
            work.push(Work::Core(core::Event::Brief(jig_domain_brief::GatherEvent::Plan {
                brief: Token::new(task),
                budget: env.limits.core.brief.brief_bytes,
                deadline: env.now.saturating_add(skein_lib::Duration::from_secs(30)),
                sections: sections.into_boxed(),
            })));
        }
        core::Now::CompleteBrief { brief, order } => {
            let sections = crate::assemble::brief(order);
            domain.briefs.insert(brief.raw(), sections).expect("core-selected brief continuation");
            work.push(Work::Core(core::Event::BriefAssembled { task: brief.raw(), ready: true }));
        }
        core::Now::WorkspaceRequest { task, attempt, .. } => {
            work.push(Work::Core(core::Event::WorkspacePrepared {
                task,
                attempt,
                credentials: Box::new([]),
                writes: Some(Box::new([])),
            }));
        }
        core::Now::RunPrepared { live, task, attempt, conversation, run, messages, transcript, grants, .. } => {
            let budget = run.budget;
            let placement = run.placement;
            let sections = domain.briefs.remove(&task).expect("completed brief parts");
            match crate::assemble::assignment(
                domain,
                env,
                task,
                attempt,
                conversation,
                live,
                *run,
                sections,
                transcript,
                messages,
                grants,
            ) {
                Some(value) => {
                    domain
                        .assignments
                        .insert(task, crate::domain::Assignment { placement, attempt, value })
                        .expect("core-selected assignment");
                    work.push(Work::Core(core::Event::ClaimPrepared { task, attempt, budget, writes: Box::new([]) }));
                }
                None => work.push(Work::Core(core::Event::PreparationFailed { task })),
            }
        }
        core::Now::RunPreparationFailed { task } | core::Now::DropAssignment { task } => {
            drop(domain.assignments.remove(&task));
            drop(domain.briefs.remove(&task));
        }
        core::Now::DropPayload { payload } => {
            drop(domain.payloads.remove(&payload));
        }
        core::Now::TurnPayload { run, attempt, turn, body } => {
            let payload = domain.payloads.get(&body).expect("fleet retained the turn body");
            let (spent, read, refused) = match payload {
                Payload::Turn { spent, read, refused, .. } => (*spent, *read, crate::assemble::refusal_names(refused)),
                Payload::Word(_) | Payload::Settled(_) | Payload::Answer { .. } => {
                    unreachable!("turn body family")
                }
            };
            work.push(Work::Core(core::Event::TurnPayload {
                run,
                attempt,
                turn,
                body,
                cumulative: spent,
                read,
                refused,
            }));
        }
        core::Now::AnswerPayload { run, attempt, payload } => {
            crate::assemble::answer_payload(domain, run, attempt, payload, work);
        }
        core::Now::AcceptedTurn { payload, task, attempt, turn, accepted } => {
            let body = domain.payloads.remove(&payload).expect("accepted turn body");
            let (spent, read, transcript, opens_window) = match body {
                Payload::Turn { spent, read, body: transcript, opens_window, .. } => {
                    (spent, read, transcript, opens_window)
                }
                Payload::Word(_) | Payload::Settled(_) | Payload::Answer { .. } => {
                    unreachable!("accepted turn body family")
                }
            };
            work.push(Work::Core(core::Event::AcceptedTurn {
                task,
                attempt,
                turn,
                accepted,
                opens_window,
                cumulative: spent,
                read,
                transcript,
            }));
        }
        core::Now::RefusedPayload { request, problem } => {
            let payload = match domain.payloads.remove(&request) {
                Some(Payload::Turn { task, attempt, turn, .. }) => {
                    Some(core::PayloadRefusal::Turn { task, attempt, turn })
                }
                Some(Payload::Answer { task, attempt, .. }) => Some(core::PayloadRefusal::Answer { task, attempt }),
                Some(Payload::Word(_) | Payload::Settled(_)) => {
                    unreachable!("answer is not a turn or terminal")
                }
                None => None,
            };
            work.push(Work::Core(core::Event::RefusedPayload { request, problem, payload }));
        }
        load @ (core::Now::SeenConversation { .. }
        | core::Now::LoadEndedRead { .. }
        | core::Now::HistoricalProposal { .. }
        | core::Now::DecidedProposal { .. }
        | core::Now::HistoricalEscalation { .. }
        | core::Now::HistoricalPersonTask { .. }
        | core::Now::EndedTask { .. }
        | core::Now::PartyInputs { .. }) => {
            now(domain, Output::CoreLoad(load));
        }

        core::Now::RestoreRefused => {
            let _request = domain.core.restart_refuse();
        }
        core::Now::RelayBusy { channel: _, run, attempt, call } => {
            domain.host_calls.remove(&call);
            now(
                domain,
                Output::ToChild(Released::Host(host::Event::Down(host::LinkDown::Relayed {
                    run,
                    attempt,
                    call,
                    answer: host::Relayed::Busy,
                }))),
            );
        }
        core::Now::DropCall { call } => {
            if let Some(binding) = domain.host_calls.remove(&call.name) {
                now(
                    domain,
                    Output::ToChild(Released::Host(host::Event::Down(host::LinkDown::Relayed {
                        run: binding.run,
                        attempt: binding.attempt,
                        call: call.name,
                        answer: crate::assemble::host_answer(core::SettledAnswer::Host {
                            error: true,
                            body: Box::from(&b"unavailable"[..]),
                        }),
                    }))),
                );
            }
        }
        other @ (core::Now::Call { .. }
        | core::Now::EffectAnswer { .. }
        | core::Now::NoteBusy { .. }
        | core::Now::SettledCallRefused { .. }
        | core::Now::CallBusy { .. }
        | core::Now::SignInRefused { .. }
        | core::Now::WatchRefused { .. }
        | core::Now::Account(_)
        | core::Now::RunText { .. }
        | core::Now::NotesIndexed { .. }
        | core::Now::NotesRecalled { .. }
        | core::Now::NotesRefused { .. }) => now(domain, Output::Now(other)),
    }
}

#[expect(clippy::manual_map, reason = "the strict subset uses a match instead of closure-taking methods")]
#[expect(clippy::too_many_lines, reason = "one exhaustive dispatcher translates the complete hub boundary")]
pub(crate) fn host_out(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision<Write, Output>,
    out: &mut Queue<host::Request>,
    work: &mut Queue<Work>,
) {
    for _ in 0..out.len() {
        let request = out.pop().expect("host requests");
        match request.to_agent() {
            Ok(request) => held(decision, Output::Agent(request)),
            Err(request) => match request {
                host::Request::Source { .. }
                | host::Request::SourceFault
                | host::Request::Up(host::LinkUp::Logins { .. } | host::LinkUp::SignedOut { .. }) => {
                    unreachable!("engine hub has no program sources")
                }
                host::Request::Up(host::LinkUp::Call { run, attempt, call, tool, writes, input, deadline }) => {
                    domain
                        .host_calls
                        .insert(call.clone(), crate::domain::HostCall { run, attempt })
                        .expect("host bounded its relays");
                    work.push(Work::Core(core::Event::Fleet(fleet::Event::Relay {
                        channel: Token::new(0),
                        run,
                        attempt,
                        call: fleet::Call { name: call, tool, writes, input, deadline },
                    })));
                }
                host::Request::Up(host::LinkUp::Turn { run, attempt, turn, .. }) => {
                    let body = domain.payload(Payload::Turn {
                        task: run.raw(),
                        attempt: attempt.raw(),
                        turn: turn.turn,
                        spent: turn.spent,
                        read: match turn.read {
                            Some(read) => Some(read.raw()),
                            None => None,
                        },
                        body: turn.body,
                        opens_window: turn.opens_window,
                        refused: crate::assemble::refusals(turn.refused),
                    });
                    work.push(Work::Core(core::Event::Fleet(fleet::Event::Turn {
                        channel: Token::new(0),
                        run,
                        attempt,
                        turn: match domain.payloads.get(&body).expect("turn token") {
                            Payload::Turn { turn, .. } => *turn,
                            Payload::Answer { .. } | Payload::Word(_) | Payload::Settled(_) => {
                                unreachable!("turn token family");
                            }
                        },
                        body,
                    })));
                }
                host::Request::Up(host::LinkUp::Answer { run, attempt, answer, .. }) => {
                    let payload = domain.payload(Payload::Answer {
                        task: run.raw(),
                        attempt: attempt.raw(),
                        spent: answer.spent,
                        ending: answer.ending,
                        read: crate::assemble::message_read(answer.read),
                        refused: crate::assemble::refusals(answer.refused),
                    });
                    work.push(Work::Core(core::Event::Fleet(fleet::Event::Answer {
                        channel: Token::new(0),
                        run,
                        attempt,
                        answer: fleet::Answer::Ended,
                        payload,
                    })));
                }
                host::Request::Up(host::LinkUp::Unauthorized { run, attempt, account, generation }) => {
                    work.push(Work::Core(core::Event::Fleet(fleet::Event::Unauthorized {
                        channel: Token::new(0),
                        run,
                        attempt,
                        account,
                        generation,
                    })));
                }
                host::Request::Up(host::LinkUp::Limited { run, attempt, account, reset }) => {
                    work.push(Work::Core(core::Event::Fleet(fleet::Event::Limited {
                        channel: Token::new(0),
                        run,
                        attempt,
                        account,
                        reset,
                    })));
                }
                host::Request::Up(host::LinkUp::AccountRefused { run, attempt, account, code, reason }) => {
                    work.push(Work::Core(core::Event::Fleet(fleet::Event::AccountRefused {
                        channel: Token::new(0),
                        run,
                        attempt,
                        account,
                        code,
                        reason,
                    })));
                }
                host::Request::Up(host::LinkUp::Activity { .. }) => unreachable!("activity is drained separately"),
                host::Request::CancelRelay { run, attempt, call } => {
                    domain.host_calls.remove(&call);
                    work.push(Work::Host(host::Event::RelayCancelled { run, attempt, call }));
                }

                host::Request::Up(host::LinkUp::Piece { .. }) | host::Request::Hello { .. } => {}
                host::Request::Prepare { owner, .. } => work.push(Work::Host(host::Event::Unprepared {
                    owner,
                    failure: host::Preparation::Permanent { resource: None },
                    detail: Box::new([]),
                })),
                host::Request::Deliver { .. }
                | host::Request::Abort { .. }
                | host::Request::Save { .. }
                | host::Request::Release { .. } => unreachable!("engine hub has no workspace capability"),
                host::Request::Message { .. }
                | host::Request::Reply { .. }
                | host::Request::Start { .. }
                | host::Request::AcknowledgeAgentTurn { .. }
                | host::Request::Stop { .. }
                | host::Request::Grant { .. } => unreachable!("to_agent classified the agent capability"),
            },
        }
    }
    crate::host_activity::drain(&mut domain.host, env.limits.host.told, work);
}

/// A run's live content goes to the core under its hub's hosting; text is
/// carried in pieces the views' bound allows (client/domain/contract.md, 7.3).
fn agent_content(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision<Write, Output>,
    work: &mut Queue<Work>,
    owner: Token,
    content: crate::AgentContent,
) {
    let Some(hosting) = domain.host.hosting(owner) else { return };
    let core_env = Env { now: env.now, wall: env.wall, limits: env.limits.core };
    match content {
        crate::AgentContent::Text { turn, retry, mut offset, text } => {
            if text.is_empty() {
                let requests = core::step(
                    &mut domain.core,
                    &core_env,
                    core::Event::RunText {
                        run: hosting.run,
                        attempt: hosting.attempt,
                        turn,
                        retry,
                        offset,
                        text,
                        call: None,
                    },
                );
                core_out(domain, env, decision, requests, work);
                return;
            }
            let bound = usize::try_from(env.limits.core.views.text_bytes).expect("stream byte limit");
            for piece in text.chunks(bound) {
                let requests = core::step(
                    &mut domain.core,
                    &core_env,
                    core::Event::RunText {
                        run: hosting.run,
                        attempt: hosting.attempt,
                        turn,
                        retry,
                        offset,
                        text: skein_lib::bytes::copy_of(piece),
                        call: None,
                    },
                );
                core_out(domain, env, decision, requests, work);
                let length = u32::try_from(piece.len()).expect("stream piece byte limit");
                offset = offset.checked_add(length).expect("supplied text position");
            }
        }
        crate::AgentContent::Call(call) => {
            let requests = core::step(
                &mut domain.core,
                &core_env,
                core::Event::RunCall { run: hosting.run, attempt: hosting.attempt, call },
            );
            core_out(domain, env, decision, requests, work);
        }
    }
}

// Translate the procedure's requester context; authority and closing stay below
// the root, in the connector and core respectively (examples.md, 7.2–7.3).
fn served_chat(domain: &Domain, task: u64) -> Option<u64> {
    let row = domain.core.tasks.task(task)?;
    let chat = match row.requester {
        tasks::Party::Task(chat) => chat,
        tasks::Party::Person(_) | tasks::Party::Deployment { .. } => return None,
    };
    let row = domain.core.tasks.task(chat)?;
    match row.requester {
        tasks::Party::Person(_) if row.flags.conversation => Some(chat),
        tasks::Party::Person(_) | tasks::Party::Task(_) | tasks::Party::Deployment { .. } => None,
    }
}
