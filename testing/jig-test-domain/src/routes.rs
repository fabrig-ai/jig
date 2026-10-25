//! Total boundary translations for the testing application's tools, effects
//! and restart script (`domain/root.md`, sections 3 and 11;
//! `domain/engine.md`, sections 6–7).

use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_connector as connector;
use skein_lib::{Decision, Env, List, Queue, ReplyTo, Token};

mod brief;
mod workspace;

use crate::{Delivery, Domain, Key, Limits, Record, Work, Write, hold, write};

pub(super) fn route_core(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision<Write, Delivery>,
    requests: core::Requests,
    work: &mut Queue<Work>,
) {
    let core::Requests::Out(mut requests) = requests;
    for _ in 0..requests.len() {
        match requests.pop().expect("core request count") {
            core::Request::Write(core::Write::CopySettingsRevision(number)) => {
                write(domain, decision, Write::CopySettingsRevision(number));
            }
            core::Request::Write(core::Write::Save(record)) => {
                write(domain, decision, Write::Save(Record::Core(record)));
            }
            core::Request::Write(core::Write::Erase(key)) => write(domain, decision, Write::Erase(Key::Core(key))),
            core::Request::Held(held) => match *held {
                core::Held::ApplicationDelivery { .. } => {
                    unreachable!("document assembly owns this synchronous output")
                }
                core::Held::ApplicationWatchEnded { watcher, connector: number, subject, why } => {
                    work.push(Work::Connector { number, event: connector::Event::DocumentForget { subject } });
                    hold(decision, Delivery::Core(core::Held::WatchEnded { watcher, why }));
                }
                core::Held::Applied { to, connector: number, token } => {
                    let outcome = crate::numbered(domain, number)
                        .application_outcome(token)
                        .expect("the connector retains every kept people token");
                    hold(decision, Delivery::Application { to, number, outcome });
                }
                core::Held::Assign { channel, run, attempt, .. } => {
                    let assignment = domain.assignments.remove(&run.raw()).expect("prepared claim has assignment");
                    assert!(assignment.attempt == attempt.raw(), "current assignment fence");
                    hold(decision, Delivery::Assigned { channel, assignment });
                }
                core::Held::SettledCall { to, call, .. } => {
                    let answer = crate::payload(domain, crate::Payload::SettledCall(call));
                    hold(decision, Delivery::Fleet(fleet::Event::Relayed { to, answer }));
                }
                core::Held::Relayed { channel, run, attempt, call: name, answer } => {
                    let delivery = crate::stopping::relayed(domain, channel, run, attempt, name, answer, true);
                    hold(decision, crate::link::down(domain, delivery));
                }
                core::Held::MakeEffect { connector, entry } => {
                    hold(decision, Delivery::Core(core::Held::MakeEffect { connector, entry }));
                }
                core::Held::Inbound { channel, run, attempt, message } => {
                    let Some(crate::Payload::InboxWord(line)) = domain.payloads.remove(&message.words) else {
                        unreachable!("inbox word payload")
                    };
                    hold(
                        decision,
                        crate::link::down(
                            domain,
                            Delivery::Message { channel, task: run.raw(), attempt: attempt.raw(), word: line },
                        ),
                    );
                }
                other @ (core::Held::ReadLoad { .. }
                | core::Held::Read { .. }
                | core::Held::ProjectionRecovery { .. }
                | core::Held::Delivery { .. }
                | core::Held::WatchEnded { .. }
                | core::Held::Relay { .. }
                | core::Held::PeopleReply { .. }
                | core::Held::CarriedUnused { .. }
                | core::Held::CallAnswer { .. }
                | core::Held::Result { .. }
                | core::Held::Acknowledge { .. }
                | core::Held::TaskTerminalAcknowledged { .. }
                | core::Held::TaskTurnKept { .. }
                | core::Held::AcknowledgeTurn { .. }
                | core::Held::Drained
                | core::Held::Grant { .. }
                | core::Held::Cancel { .. }
                | core::Held::Sources { .. }
                | core::Held::CheckSource { .. }
                | core::Held::SignOutSource { .. }
                | core::Held::SourceSignedOut { .. }
                | core::Held::Refuse { .. }
                | core::Held::TurnBusy { .. }
                | core::Held::StopRun { .. }
                | core::Held::Load(..)
                | core::Held::NotesLoad { .. }
                | core::Held::NotesWritten { .. }
                | core::Held::Acted { .. }
                | core::Held::NotesDeleted { .. }) => hold(decision, crate::link::down(domain, Delivery::Core(other))),
            },
            core::Request::Now(now) => route_now(domain, env, decision, *now, work),
            core::Request::Ask { connector, ask } => route_ask(domain, env, decision, connector, ask, work),
            core::Request::ProjectMade { project } => hold(decision, Delivery::ProjectMade { project }),
            core::Request::Decided => {}
        }
    }
}

#[expect(clippy::too_many_lines, reason = "one dispatcher translates the complete connector ask vocabulary")]
fn route_ask(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision<Write, Delivery>,
    number: u16,
    ask: core::Ask,
    work: &mut Queue<Work>,
) {
    match ask {
        core::Ask::StepMade { task, step, first, revisions } => {
            domain.procedure_steps.remove(&task);
            work.push(Work::Connector { number, event: connector::Event::StepMade { task, step, first, revisions } });
        }
        core::Ask::StepRefused { task, step, why } => crate::steps::refused(number, task, step, why, work),

        core::Ask::DropDocument { subject } => {
            if number == 1 || number == 2 {
                work.push(Work::Connector { number, event: connector::Event::DocumentForget { subject } });
            }
        }
        core::Ask::Document { watcher, scope, subject, party, window } => work.push(Work::Connector {
            number,
            event: connector::Event::Document {
                watcher,
                scope: crate::client_documents::scope(scope),
                subject,
                party,
                window: crate::client_documents::window(window),
            },
        }),
        core::Ask::Read { reply, scope, read, party, window } => crate::client_documents::read(
            domain,
            env,
            decision,
            number,
            reply,
            crate::client_documents::scope(scope),
            read,
            party,
            window,
            work,
        ),
        core::Ask::Application { request, token, owner, person, scope, role, retention } => {
            work.push(Work::Connector {
                number,
                event: connector::Event::Application {
                    request,
                    token,
                    owner,
                    person,
                    scope: crate::client_documents::scope(scope),
                    role: crate::client_documents::role_number(role),
                    retention,
                },
            });
        }
        core::Ask::DropApplication { owner } => {
            work.push(Work::Connector { number, event: connector::Event::ApplicationDrop { owner } });
        }
        core::Ask::ApplicationPartAnswered { request, outcome } => work.push(Work::Connector {
            number,
            event: connector::Event::ApplicationCore { request, answer: application_answer(outcome) },
        }),
        core::Ask::Effect(ask) => route_effect_ask(domain, number, ask, work),
        core::Ask::TaskHoldings { request, project, spec, .. } => {
            let holdings = specification_holdings(domain, env, number, project, &spec, work);
            work.push(Work::Core(core::Event::Holdings { request, connector: number, holdings }));
        }
        core::Ask::DelegateHoldings { request, members, .. } => {
            let mut holdings: List<Box<[tasks::Holding]>> =
                List::with_capacity(u32::try_from(members.len()).expect("bounded batch"));
            let mut valid = true;
            for member in members {
                match specification_holdings(domain, env, number, member.project, &member.spec, work) {
                    Some(named) => holdings.push(named).expect("batch room"),
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
            let mut holdings: List<Box<[tasks::Holding]>> =
                List::with_capacity(u32::try_from(members.len()).expect("bounded batch"));
            let mut valid = true;
            for member in members {
                match specification_holdings(domain, env, number, member.project, &member.spec, work) {
                    Some(named) => holdings.push(named).expect("batch room"),
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
            let code = match context.executor {
                tasks::Executor::Procedure { code, .. } => u16::try_from(code).expect("test procedure code"),
                tasks::Executor::Agent { .. } | tasks::Executor::Person(_) => {
                    unreachable!("procedure context has a procedure executor")
                }
            };
            assert!(domain.procedure_steps.insert(context.task, (step, code)).is_ok(), "one procedure step flight");
            hold(decision, Delivery::Procedure { task: context.task, step, connector: number, code, context });
        }
        core::Ask::ClaimDone { task, attempt, kinds, charter } => {
            if number == 1 {
                hold(
                    decision,
                    Delivery::Fleet(fleet::Event::Start {
                        placement: Some(domain.assignments.get(&task).expect("prepared assignment").run.placement),
                        grants: grants::fleet(&domain.assignments.get(&task).expect("prepared assignment").grants),
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
        }
        core::Ask::Close { task, .. } => {
            work.push(Work::Connector { number, event: connector::Event::Close { task } });
        }
        core::Ask::Release { task, .. } => {
            work.push(Work::Connector { number, event: connector::Event::Unnamed { task } });
            if number == 1 {
                work.push(Work::Core(core::Event::Tasks(tasks::Event::Settled { task })));
            }
        }
        core::Ask::Adopt { request: _, project, adoption } => {
            let segments = adoption.resource.path;
            let path = path(segments);
            let role = match adoption.role {
                people::ResourceRole::Owned => connector::ResourceRole::Owned,
                people::ResourceRole::Fork => connector::ResourceRole::Participating,
                people::ResourceRole::Context => connector::ResourceRole::Context,
            };
            work.push(Work::Connector { number, event: connector::Event::Adopt { project, resource: path, role } });
        }
        core::Ask::Gather { section, budget } => {
            work.push(Work::Connector {
                number,
                event: connector::Event::Gather { token: section, task: section.raw(), budget },
            });
        }
        core::Ask::CutTo { section, size } => {
            work.push(Work::Connector { number, event: connector::Event::Cut { token: section, size } });
        }
        core::Ask::Drop { section } => {
            work.push(Work::Connector { number, event: connector::Event::HandOver { token: section } });
        }
        core::Ask::Lost { task, attempt } => {
            for resource in domain.core.tasks.lost_writes(task, attempt) {
                if resource.connector == number {
                    let call = connector::SystemRequest::ReadFact {
                        resource: path(resource.path.clone()),
                        observed: env.wall,
                    };
                    hold(decision, Delivery::LostRead { connector: number, task, attempt, resource, call });
                }
            }
        }
        core::Ask::ProjectionBegun { owner, source } => {
            if let Some(source) = source {
                let goal = domain.effects.get(&owner).expect("projection payload").goal.expect("projection goal");
                work.push(Work::Core(core::Event::EffectStart {
                    owner,
                    connector: number,
                    origin: core::EffectOrigin::Projection { goal, source, entry: None },
                }));
            } else {
                drop(domain.effects.remove(&owner));
            }
        }
        core::Ask::CommitProjectionRecovery { .. } => {}
        core::Ask::ProjectGoal { feed } => hold(decision, Delivery::Projection { connector: number, feed }),
        core::Ask::EndTopic { task, .. } => crate::topics::sync(domain, number, task, work),
        core::Ask::SubscriptionDone { request, key, subscription } => {
            crate::topics::subscribed(domain, number, request, key, subscription, work);
        }
        core::Ask::UnsubscriptionDone { request, key } => {
            crate::topics::unsubscribed(domain, number, request, key, work);
        }
        core::Ask::DropSubscription { .. }
        | core::Ask::ForgetProjection { .. }
        | core::Ask::Hold { .. }
        | core::Ask::RepairRefused { .. }
        | core::Ask::DelegateRefused { .. } => {}
    }
}

/// Translate the connector's one-based resource IDs. Kinds one and two are
/// respectively exclusive and pooled in this testing application's charter.
fn specification_holdings(
    domain: &mut Domain,
    env: &Env<Limits>,
    number: u16,
    project: u32,
    spec: &tasks::Spec,
    work: &mut Queue<Work>,
) -> Option<Box<[tasks::Holding]>> {
    let mut holdings = List::with_capacity(env.limits.core.tasks.holdings);
    for parameter in &spec.parameters {
        match parameter {
            tasks::Parameter::Resource { connector: owner, resource, .. } => {
                if *owner == number {
                    let described = crate::numbered(domain, number).specification_resource(*resource)?.clone();
                    let role = crate::numbered(domain, number).resource_role(project, &described.path);
                    let role = match role {
                        Some(connector::ResourceRole::Owned) => core::connector::ResourceRole::Owned,
                        Some(connector::ResourceRole::Participating) => core::connector::ResourceRole::Participant,
                        Some(connector::ResourceRole::Context) => core::connector::ResourceRole::Context,
                        Some(connector::ResourceRole::Unavailable) | None => core::connector::ResourceRole::Unavailable,
                    };
                    let hold = report_hold(domain, number, &described.path, described.hold);
                    let name = tasks::Name { connector: number, path: name(described.path.clone()).segments };
                    work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::Resource {
                        project,
                        name: name.clone(),
                        role,
                        hold,
                    })));
                    let holding = match described.hold {
                        connector::Hold::Exclusive { .. } => Some(tasks::Holding::Write { resource: name, kind: 1 }),
                        connector::Hold::Pooled { .. } => Some(tasks::Holding::Slot { pool: name, kind: 2 }),
                        connector::Hold::None => None,
                    };
                    if let Some(holding) = holding {
                        holdings.push(holding).ok()?;
                    }
                }
            }
            tasks::Parameter::Number { .. } | tasks::Parameter::Bytes { .. } => {}
        }
    }
    Some(holdings.into_boxed())
}

fn path(segments: Box<[Box<[u8]>]>) -> connector::Path {
    let capacity = u32::try_from(segments.len()).expect("bounded path segments");
    let mut path = List::with_capacity(capacity);
    for segment in segments {
        path.push(segment).expect("path room");
    }
    connector::Path::new(path, u32::MAX).expect("admitted nonempty path")
}

#[expect(clippy::too_many_lines, reason = "one exhaustive root continuation boundary")]
fn route_now(
    domain: &mut Domain,
    env: &Env<Limits>,
    _decision: &mut Decision<Write, Delivery>,
    now: core::Now,
    work: &mut Queue<Work>,
) {
    match now {
        core::Now::RelayBusy { channel, run, attempt, call } => {
            let delivery = crate::link::down(
                domain,
                Delivery::CallBusy { channel, task: run.raw(), attempt: attempt.raw(), name: call },
            );
            assert!(domain.journal.now(delivery).is_ok(), "reserved busy answer room");
        }
        core::Now::CallBusy { to } | core::Now::NoteBusy { to, .. } => crate::stopping::call(domain, to, work),
        core::Now::Relayed { channel, run, attempt, call: name, answer } => {
            let delivery = crate::stopping::relayed(domain, channel, run, attempt, name, answer, false);
            let delivery = crate::link::down(domain, delivery);
            assert!(domain.journal.now(delivery).is_ok(), "reserved relay answer room");
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
            domain.now.push(crate::Request::Now(value));
        }
        core::Now::BriefCorePlanned { task, sections, .. } => brief::plan(env, task, sections, work),
        core::Now::CompleteBrief { brief: token, order } => brief::complete(domain, token, order, work),
        core::Now::WorkspaceRequest { task, attempt, context } => {
            let mut names = List::with_capacity(env.limits.core.tasks.holdings);
            let mut checks = List::with_capacity(env.limits.core.authority.writes);
            for number in [1, 2] {
                let Some(holdings) = specification_holdings(domain, env, number, context.project, &context.spec, work)
                else {
                    work.push(Work::Core(core::Event::WorkspacePrepared {
                        task,
                        attempt,
                        credentials: Box::new([]),
                        writes: None,
                    }));
                    return;
                };
                for holding in holdings {
                    match holding {
                        tasks::Holding::Write { resource, .. } => {
                            checks
                                .push(authority::Write {
                                    effect: authority::Effect {
                                        connector: resource.connector,
                                        kind: 5,
                                        name: authority::Name { segments: resource.path.clone() },
                                        state: [0; 32],
                                        price: None,
                                        access: authority::EffectAccess::Owned,
                                        additional: Box::new([]),
                                        guards: Box::new([]),
                                    },
                                    held: authority::Writer::Task,
                                })
                                .expect("workspace write check room");
                            names.push(resource).expect("admitted holding count");
                        }
                        tasks::Holding::Slot { .. } => {}
                    }
                }
            }
            assert!(domain.workspace_writes.insert(task, names.into_boxed()).is_ok(), "workspace per task");
            let prepared = workspace::credentials(domain, &env.limits, context.project, &context.spec);
            let (credentials, writes) = match prepared {
                Some(credentials) => (credentials, Some(checks.into_boxed())),
                None => (Box::<[u32]>::from([]), None),
            };
            work.push(Work::Core(core::Event::WorkspacePrepared { task, attempt, credentials, writes }));
        }
        core::Now::RunPrepared {
            live,
            task,
            attempt,
            conversation,
            charter,
            run,
            messages,
            transcript,
            grants,
            ..
        } => {
            let budget = run.budget;
            assert!(
                domain
                    .assignments
                    .insert(
                        task,
                        crate::Assignment {
                            live,
                            task,
                            attempt,
                            conversation,
                            brief: domain.briefs.remove(&task).expect("completed brief"),
                            charter,
                            run,
                            messages,
                            transcript,
                            answered: domain.core.settled_calls(task, attempt),
                            grants
                        }
                    )
                    .is_ok(),
                "one prepared assignment per task"
            );
            let writes = domain.workspace_writes.remove(&task).expect("assembled workspace writes");
            work.push(Work::Core(core::Event::ClaimPrepared { task, attempt, budget, writes }));
        }
        core::Now::RunPreparationFailed { task } | core::Now::DropAssignment { task } => {
            drop(domain.assignments.remove(&task));
            drop(domain.briefs.remove(&task));
            drop(domain.workspace_writes.remove(&task));
        }
        core::Now::DropPayload { payload } => {
            drop(domain.payloads.remove(&payload));
        }
        core::Now::TurnPayload { run, attempt, turn, body } => {
            let payload = domain.payloads.get(&body).expect("fleet retained the turn body");
            let (cumulative, read, refused) = match payload {
                crate::Payload::Turn { cumulative, read, refused, .. } => (*cumulative, *read, refused.clone()),
                crate::Payload::Busy
                | crate::Payload::InboxWord(_)
                | crate::Payload::SettledCall(_)
                | crate::Payload::Answer { .. } => {
                    unreachable!("turn body family")
                }
            };
            work.push(Work::Core(core::Event::TurnPayload { run, attempt, turn, body, cumulative, read, refused }));
        }
        core::Now::AnswerPayload { run, attempt, payload } => crate::link::answer(domain, run, attempt, payload, work),
        core::Now::AcceptedTurn { payload, task, attempt, turn, accepted } => {
            let body = domain.payloads.remove(&payload).expect("accepted turn body");
            let (cumulative, read, transcript, opens_window) = match body {
                crate::Payload::Turn { cumulative, read, transcript, opens_window, .. } => {
                    (cumulative, read, transcript, opens_window)
                }
                crate::Payload::Busy
                | crate::Payload::InboxWord(_)
                | crate::Payload::SettledCall(_)
                | crate::Payload::Answer { .. } => {
                    unreachable!("accepted turn body family")
                }
            };
            work.push(Work::Core(core::Event::AcceptedTurn {
                task,
                attempt,
                turn,
                accepted,
                opens_window,
                cumulative,
                read,
                transcript,
            }));
        }
        core::Now::RefusedPayload { request, problem } => {
            let payload = match domain.payloads.remove(&request) {
                Some(crate::Payload::Turn { task, attempt, turn, .. }) => {
                    Some(core::PayloadRefusal::Turn { task, attempt, turn })
                }
                Some(crate::Payload::Answer { task, attempt, .. }) => {
                    Some(core::PayloadRefusal::Answer { task, attempt })
                }
                Some(crate::Payload::Busy | crate::Payload::InboxWord(_) | crate::Payload::SettledCall(_)) => {
                    unreachable!("answer is not a turn or terminal")
                }
                None => None,
            };
            work.push(Work::Core(core::Event::RefusedPayload { request, problem, payload }));
        }
        core::Now::RestoreRefused => {
            assert!(domain.core.restart_failure().is_some(), "a refused restore has stopped the core");
        }

        other @ (core::Now::SeenConversation { .. }
        | core::Now::LoadEndedRead { .. }
        | core::Now::HistoricalPersonTask { .. }
        | core::Now::EndedTask { .. }
        | core::Now::PartyInputs { .. }
        | core::Now::HistoricalProposal { .. }
        | core::Now::DecidedProposal { .. }
        | core::Now::HistoricalEscalation { .. }
        | core::Now::SettledCallRefused { .. }
        | core::Now::Call { .. }
        | core::Now::DropCall { .. }
        | core::Now::EffectAnswer { .. }
        | core::Now::SignInRefused { .. }
        | core::Now::WatchRefused { .. }
        | core::Now::Account(_)
        | core::Now::RunText { .. }
        | core::Now::NotesIndexed { .. }
        | core::Now::NotesRecalled { .. }
        | core::Now::NotesRefused { .. }) => domain.now.push(crate::Request::Now(other)),
    }
}

#[expect(clippy::too_many_lines, reason = "one exhaustive connector boundary dispatcher")]
pub(super) fn route_connector(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision<Write, Delivery>,
    number: u16,
    requests: &mut Queue<connector::Request>,
    work: &mut Queue<Work>,
) {
    for _ in 0..requests.len() {
        match requests.pop().expect("connector request count") {
            connector::Request::Document { watcher, subject, document } => {
                crate::client_documents::document(domain, env, decision, number, watcher, subject, document, work);
            }
            connector::Request::Topic { token, resources } => {
                crate::topics::described(domain, number, token, Some(resources), work);
            }
            connector::Request::TopicRefused { token } => crate::topics::described(domain, number, token, None, work),
            connector::Request::News { topic, subscribers } => {
                crate::topics::news(domain, number, topic, subscribers, work);
            }
            connector::Request::ReadAnswered { .. } => unreachable!("client read completes in its synchronous route"),
            connector::Request::Marked { subjects } => {
                work.push(Work::Core(core::Event::Marked { connector: number, subjects }));
            }
            connector::Request::ApplicationDone { request, token } => {
                work.push(Work::Core(core::Event::ApplicationFinished { request, outcome: Ok(token) }));
            }
            connector::Request::ApplicationAgain { request, why } => {
                let why = match why {
                    connector::application_core::Again::Busy => people::Again::Busy,
                    connector::application_core::Again::NotReady => people::Again::NotReady,
                };
                work.push(Work::Core(core::Event::ApplicationFinished { request, outcome: Err(why) }));
            }
            connector::Request::ApplicationRelease { request, project, release } => {
                work.push(Work::Core(core::Event::ApplicationPart {
                    request,
                    ask: Box::new(people::Ask::Release {
                        project,
                        task: release.task,
                        revision: release.revision,
                        again: release.again,
                    }),
                }));
            }
            connector::Request::Closed { task } => {
                work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::Closed {
                    task,
                    connector: number,
                })));
            }
            connector::Request::Drift { pool: resource, tasks }
            | connector::Request::DriftResource { tasks, resource } => {
                let resource = name(resource);
                for task in tasks {
                    work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::Drift {
                        task,
                        resource: tasks::Name { connector: number, path: resource.segments.clone() },
                    })));
                }
            }
            connector::Request::RestartDone => {
                let step = core::RestartStep::SettleOutbox { connector: number };
                let request = domain.core.restart_done(step);
                work.push(Work::Restart(request));
            }
            connector::Request::Save { record } => {
                match &record {
                    connector::Record::Outbox(row) => {
                        assert!(crate::remember_outbox(domain, number, row), "bounded outbox owner routes");
                    }
                    connector::Record::ApplicationObject { .. }
                    | connector::Record::ApplicationAnswer { .. }
                    | connector::Record::Proposal { .. }
                    | connector::Record::Task { .. }
                    | connector::Record::Adoption { .. }
                    | connector::Record::Subscription { .. }
                    | connector::Record::Pool { .. }
                    | connector::Record::Procedure(_)
                    | connector::Record::ProcedureDelegateOutcome { .. }
                    | connector::Record::Result { .. }
                    | connector::Record::Made { .. } => {}
                }
                write(domain, decision, Write::Save(Record::Connector { number, record }));
            }
            connector::Request::Erase { key } => {
                write(domain, decision, Write::Erase(Key::Connector { number, key }));
            }
            connector::Request::Step { task, decision: step } => {
                let (numbered_step, code) = *domain.procedure_steps.get(&task).expect("procedure chose a step");
                let Some(action) = crate::steps::translate(domain, task, numbered_step, number, step, work) else {
                    continue;
                };
                work.push(Work::Core(core::Event::ProcedureStep {
                    task,
                    step: numbered_step,
                    connector: number,
                    code: u32::from(code),
                    decision: action,
                }));
            }
            connector::Request::System(call) => hold(decision, Delivery::System { connector: number, call }),
            connector::Request::Described { token, description, waits_for_dependencies } => {
                work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::Described {
                    owner: token,
                    description: Box::new(describe(number, description, waits_for_dependencies)),
                })));
            }
            connector::Request::EffectBusy { token } => work
                .push(Work::Core(core::Event::EffectConnector(core::connector::Event::DescribeBusy { owner: token }))),
            connector::Request::EffectRefused { token } => {
                work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::DescribeRefused {
                    owner: token,
                })));
            }
            connector::Request::Verdict { token, verdict } => {
                let Some((owner, judge, asked_state)) = domain.judges.remove(&token) else { continue };
                let (verdict, at, guarded, state) = match verdict {
                    connector::Verdict::Met { state, observed, guarded } => {
                        let pin = state_pin(state);
                        (authority::Verdict::Met, observed, guarded, pin)
                    }
                    connector::Verdict::Wait => (authority::Verdict::Wait, env.wall, false, asked_state),
                    connector::Verdict::Refuse { .. } => (authority::Verdict::Refuse, env.wall, false, asked_state),
                };
                work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::Verdict {
                    owner,
                    judge,
                    verdict,
                    at,
                    guarded,
                    state,
                })));
            }
            connector::Request::Outcome { entry, outcome } => {
                let owner = match domain.outbox_owners.get(&entry) {
                    Some(owner) => *owner,
                    None => continue,
                };
                let made = match outcome {
                    connector::Outcome::Made { .. } => true,
                    connector::Outcome::Failed
                    | connector::Outcome::Uncertain
                    | connector::Outcome::Skipped
                    | connector::Outcome::Withdrawn
                    | connector::Outcome::Held => false,
                };
                let settled = match outcome {
                    connector::Outcome::Made { .. }
                    | connector::Outcome::Failed
                    | connector::Outcome::Skipped
                    | connector::Outcome::Withdrawn => true,
                    connector::Outcome::Uncertain | connector::Outcome::Held => false,
                };
                match domain.outbox_procedures.get(&entry) {
                    Some(number) if settled => work.push(Work::Connector {
                        number: *number,
                        event: connector::Event::EffectDecision { task: owner.task, made },
                    }),
                    Some(_) | None => {}
                }
                if settled {
                    domain.outbox_owners.remove(&entry);
                    domain.outbox_procedures.remove(&entry);
                }
                let outcome = match outcome {
                    connector::Outcome::Made { .. } => core::connector::OutboxOutcome::Made,
                    connector::Outcome::Failed => core::connector::OutboxOutcome::Failed,
                    connector::Outcome::Uncertain => core::connector::OutboxOutcome::Uncertain,
                    connector::Outcome::Held => core::connector::OutboxOutcome::Held { entry },
                    connector::Outcome::Withdrawn => core::connector::OutboxOutcome::Withdrawn,
                    connector::Outcome::Skipped => core::connector::OutboxOutcome::NotMade,
                };
                let event = if owner.projection {
                    core::connector::Event::ProjectionOutbox { connector: number, entry, outcome }
                } else {
                    core::connector::Event::Outbox { connector: number, entry, task: owner.task, outcome }
                };
                work.push(Work::Core(core::Event::EffectConnector(event)));
                if owner.projection
                    && match outcome {
                        core::connector::OutboxOutcome::Failed
                        | core::connector::OutboxOutcome::Held { .. }
                        | core::connector::OutboxOutcome::Withdrawn => true,
                        core::connector::OutboxOutcome::Made
                        | core::connector::OutboxOutcome::NotMade
                        | core::connector::OutboxOutcome::Uncertain => false,
                    }
                {
                    work.push(Work::Core(core::Event::ProjectionFailed {
                        goal: owner.task,
                        connector: number,
                        source: entry,
                    }));
                }
            }
            connector::Request::Slots { pool, slots } => {
                let name = tasks::Name { connector: number, path: name(pool).segments };
                work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::PoolSlots { name, slots })));
            }
            connector::Request::Named { task: _, project, resources } => {
                resource_reports(domain, number, project, resources, work);
            }
            connector::Request::Resource { project, resource } => {
                resource_reports(domain, number, project, Box::new([resource]), work);
            }
            connector::Request::Make { entry: _ }
            | connector::Request::Adopted { project: _, resource: _, result: _ }
            | connector::Request::Unknown { task: _, resource: _ }
            | connector::Request::Refused { task: _ }
            | connector::Request::Changed { resource: _ }
            | connector::Request::Answer { token: _, bytes: _ }
            | connector::Request::Ready { token: _, size: _ }
            | connector::Request::Section { token: _, bytes: _ }
            | connector::Request::Workspace { token: _, items: _ } => {}
        }
    }
}

/// Translate the core's choice without choosing a judge or admitting a write.
fn route_effect_ask(domain: &mut Domain, number: u16, ask: core::connector::Ask, work: &mut Queue<Work>) {
    let event = match ask {
        core::connector::Ask::ResolveUncertain { entry, again } => connector::Event::Resolve { entry, repeat: again },
        core::connector::Ask::Describe { owner } => connector::Event::Describe {
            token: owner,
            effect: domain.effects.remove(&owner).expect("connector-owned decoded effect").effect,
        },
        core::connector::Ask::DescribeProposal { owner, proposal } => {
            connector::Event::DescribeProposal { token: owner, number: proposal }
        }
        core::connector::Ask::KeepProposal { owner, proposal, task, project } => {
            connector::Event::KeepProposal { token: owner, number: proposal, task, project }
        }
        core::connector::Ask::DropProposal { proposal } => connector::Event::DropProposal { number: proposal },
        core::connector::Ask::Keep { owner, entry, task, key } => {
            domain.effect_procedures.remove(&owner);
            let (attempt, completion, position) = match key.origin {
                core::EffectPurpose::Call { attempt, completion, position } => (attempt, completion, position),
                core::EffectPurpose::Procedure { .. } => (0, 0, 0),
                core::EffectPurpose::Projection { .. } => (0, 0, 1),
            };
            connector::Event::Keep {
                token: owner,
                entry,
                task,
                key: connector::Key {
                    deployment: key.deployment,
                    task: key.task,
                    purpose: key.purpose,
                    attempt,
                    completion,
                    position,
                },
            }
        }
        core::connector::Ask::Drop { owner, .. } => {
            if let Some(task) = domain.effect_procedures.remove(&owner) {
                work.push(Work::Connector { number, event: connector::Event::EffectDecision { task, made: false } });
            }
            drop(domain.effects.remove(&owner));
            connector::Event::Drop { token: owner }
        }
        core::connector::Ask::Judge { owner, judge, state, resources } => {
            let token = Token::new(domain.next_payload);
            domain.next_payload = domain.next_payload.checked_add(1).expect("bounded judge owner numbers");
            assert!(domain.judges.insert(token, (owner, judge, state)).is_ok(), "one route per core-selected judge");
            let mut names = List::with_capacity(u32::try_from(resources.len()).expect("bounded resource names"));
            for name in resources {
                names.push(path(name.segments)).expect("one path per name");
            }
            let bytes = [state[24], state[25], state[26], state[27], state[28], state[29], state[30], state[31]];
            connector::Event::Judge {
                token,
                requirement: judge.requirement,
                resources: names.into_boxed(),
                state: u64::from_be_bytes(bytes),
            }
        }
        core::connector::Ask::Names { .. }
        | core::connector::Ask::Unname { .. }
        | core::connector::Ask::Hold { .. }
        | core::connector::Ask::ReleaseHold { .. }
        | core::connector::Ask::Writer { .. }
        | core::connector::Ask::Make { .. }
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
        | core::connector::Ask::Restore { .. }
        | core::connector::Ask::ReadAfresh
        | core::connector::Ask::SettleOutbox => unreachable!("effect handoff vocabulary"),
    };
    work.push(Work::Connector { number, event });
}

fn name(path: connector::Path) -> authority::Name {
    let mut segments = List::with_capacity(u32::try_from(path.segments().len()).expect("bounded segments"));
    for segment in path.segments() {
        segments.push(segment.clone()).expect("one segment per path");
    }
    authority::Name { segments: segments.into_boxed() }
}

fn describe(
    number: u16,
    description: connector::Description,
    waits_for_dependencies: bool,
) -> core::connector::EffectDescription {
    let form = match description.form {
        connector::Form::Creation => core::connector::EffectForm::Creation,
        connector::Form::Transition => core::connector::EffectForm::Transition,
        connector::Form::Set => core::connector::EffectForm::Set,
    };
    let recovery = match description.recovery {
        connector::Recovery::Keyed => core::connector::Recovery::Keyed,
        connector::Recovery::Conditional => core::connector::Recovery::Conditional,
        connector::Recovery::Idempotent => core::connector::Recovery::Idempotent,
        connector::Recovery::Unrecoverable => core::connector::Recovery::Unrecoverable,
    };
    let mut resources = List::with_capacity(u32::try_from(description.resources.len()).expect("bounded resources"));
    for resource in description.resources {
        resources.push(name(resource)).expect("one name per resource");
    }
    let mut additional = List::with_capacity(resources.len().saturating_sub(1));
    let first = resources.as_slice().first().expect("connector described at least one resource").clone();
    for resource in resources.as_slice().get(1..).expect("at least one resource") {
        additional
            .push(authority::EffectResource { name: resource.clone(), access: authority::EffectAccess::Owned })
            .expect("additional resources");
    }
    let state = state_pin(description.state);
    core::connector::EffectDescription {
        waits_for_dependencies,
        connector: number,
        purpose: description.purpose,
        form,
        recovery,
        effect: authority::Effect {
            connector: number,
            kind: description.kind,
            name: first,
            state,
            price: description.price,
            access: authority::EffectAccess::Owned,
            additional: additional.into_boxed(),
            guards: Box::new([]),
        },
    }
}

fn state_pin(state: u64) -> [u8; 32] {
    let bytes = state.to_be_bytes();
    let mut pin = [0; 32];
    for offset in 0..8 {
        *pin.get_mut(24usize.checked_add(offset).expect("pin offset")).expect("eight bytes fit the pin") =
            *bytes.get(offset).expect("eight-byte integer");
    }
    pin
}

/// Hold modes and current pool facts are translated without choosing admission.
fn report_hold(
    domain: &mut Domain,
    number: u16,
    path: &connector::Path,
    hold: connector::Hold,
) -> core::connector::HoldKind {
    match hold {
        connector::Hold::None => core::connector::HoldKind::Shared,
        connector::Hold::Exclusive { mode } => core::connector::HoldKind::Exclusive {
            wait: match mode {
                connector::HoldMode::Wait => true,
                connector::HoldMode::Refuse => false,
            },
        },
        connector::Hold::Pooled { mode } => core::connector::HoldKind::Pooled {
            slots: crate::numbered(domain, number).slots(path).unwrap_or(0),
            wait: match mode {
                connector::HoldMode::Wait => true,
                connector::HoldMode::Refuse => false,
            },
        },
    }
}

/// Translate current roles and holds together, scoped to the adopting project.
fn resource_reports(
    domain: &mut Domain,
    number: u16,
    project: u32,
    resources: Box<[connector::Named]>,
    work: &mut Queue<Work>,
) {
    for resource in resources {
        let hold = report_hold(domain, number, &resource.path, resource.hold);
        let role = match resource.role {
            connector::ResourceRole::Owned => core::connector::ResourceRole::Owned,
            connector::ResourceRole::Participating => core::connector::ResourceRole::Participant,
            connector::ResourceRole::Context => core::connector::ResourceRole::Context,
            connector::ResourceRole::Unavailable => core::connector::ResourceRole::Unavailable,
        };
        let name = tasks::Name { connector: number, path: name(resource.path).segments };
        work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::Resource {
            project,
            name,
            role,
            hold,
        })));
    }
}

fn application_answer(outcome: people::Outcome) -> connector::ApplicationCoreAnswer {
    match outcome {
        people::Outcome::Released { .. } => connector::ApplicationCoreAnswer::Released,
        people::Outcome::Refused(why) => {
            connector::ApplicationCoreAnswer::Refused(crate::application_facts::refusal(why))
        }
        people::Outcome::Again(why) => connector::ApplicationCoreAnswer::Again(crate::application_facts::again(why)),
        people::Outcome::Compound { .. }
        | people::Outcome::CompoundRefused { .. }
        | people::Outcome::Applied { .. }
        | people::Outcome::SignedOut
        | people::Outcome::NoteEdited { .. }
        | people::Outcome::ServiceMade { .. }
        | people::Outcome::Adopted { .. }
        | people::Outcome::Created { .. }
        | people::Outcome::Proposed { .. }
        | people::Outcome::Withdrawn { .. }
        | people::Outcome::Stopped { .. }
        | people::Outcome::Cancelled { .. }
        | people::Outcome::PersonTaken { .. }
        | people::Outcome::PersonHandedBack { .. }
        | people::Outcome::PersonAnswered { .. }
        | people::Outcome::Moved { .. }
        | people::Outcome::ProposalDecided { .. }
        | people::Outcome::Said { .. }
        | people::Outcome::QuestionAnswered { .. }
        | people::Outcome::Amended { .. }
        | people::Outcome::RolesSet { .. }
        | people::Outcome::Renamed { .. }
        | people::Outcome::ProjectMade { .. }
        | people::Outcome::SettingsChanged { .. }
        | people::Outcome::PolicyChanged { .. }
        | people::Outcome::PoolSet { .. }
        | people::Outcome::EscalationDecided { .. }
        | people::Outcome::AccountSignInStarted { .. }
        | people::Outcome::AccountProgramDeclared { .. }
        | people::Outcome::AccountKeyKept { .. }
        | people::Outcome::AccountMovingOut { .. }
        | people::Outcome::AccountFinishing { .. }
        | people::Outcome::AccountMovedIn { .. }
        | people::Outcome::AccountChanged
        | people::Outcome::AccountRemoved { .. }
        | people::Outcome::AccountSignedOut { .. }
        | people::Outcome::MarkedSeen { .. } => unreachable!("release core part returns a release terminal"),
    }
}

mod grants;
