//! Exhaustive routing of the root's journal, with values kept in their owners (service.md, 4).
use crate::{Engine, PartyOutput, clients, stop};
use jig_domain_core as core;
use jig_test_connector as connector;
use jig_test_domain as root;

pub(super) fn request(engine: &mut Engine, request: root::Request) {
    match request {
        root::Request::Commit { number, writes } => {
            if let Some(clients) = &mut engine.clients {
                clients.commit(number, &writes);
            }
            crate::store::request(
                engine,
                jig_test_store::StoreOut::Commit { number, writes: crate::store::writes(writes) },
            );
        }
        root::Request::Restart(step) => restart(engine, step),
        root::Request::Deliver(delivery) => deliver(engine, delivery),
        root::Request::Now(now) => immediate(engine, now),
        root::Request::Busy { event } => {
            if let Some(drain) = &mut engine.drain {
                drain.busy.push(event);
            } else {
                engine.outputs.push(PartyOutput::Busy(event));
            }
        }
        root::Request::ReadResult { payload, outcome } => engine.events.push(crate::tools::result(payload, &outcome)),
        root::Request::Stop => stop::invalid_restore(engine),
    }
}

fn deliver(engine: &mut Engine, delivery: root::Delivery) {
    match delivery {
        root::Delivery::ProjectMade { .. } => {}
        root::Delivery::Restart(step) => restart(engine, step),
        root::Delivery::System { connector: number, call } => {
            let read = match &call {
                connector::SystemRequest::ReadFact { .. } => true,
                connector::SystemRequest::Apply { .. } | connector::SystemRequest::Look { .. } => false,
            };
            let event = system(engine, number).answer(call);
            engine.events.push(root::Event::Connector { number, event: connector::Event::System(event) });
            if read {
                restart_read(engine, number);
            }
        }
        root::Delivery::LostRead { connector, resource, call, .. } => {
            let event = system(engine, connector).answer(call);
            engine.events.push(root::Event::WriterRead { connector, resource, event });
        }
        root::Delivery::Projection { connector, feed } => {
            if feed.closing {
                engine
                    .events
                    .push(root::Event::Core(core::Event::ProjectionSettled { goal: feed.goal.number, connector }));
            }
        }
        root::Delivery::Fleet(event) => engine.events.push(root::Event::Core(core::Event::Fleet(event))),
        root::Delivery::Procedure { .. } => stop::unavailable(engine, b"procedures"),
        root::Delivery::Core(held) => core_held(engine, held),
        held @ (root::Delivery::ApplicationDocument { .. }
        | root::Delivery::ApplicationRead { .. }
        | root::Delivery::Application { .. }) => {
            if let Some(clients) = &mut engine.clients {
                clients.down(clients::application(held).expect("application output"));
            } else {
                engine.outputs.push(PartyOutput::Held(held));
            }
        }
        root::Delivery::Link { channel, down } => {
            if let jig_domain_host::LinkDown::Relayed { call, .. } = &down {
                engine.tools.forget_name(call);
            }
            if let Some(workers) = &mut engine.workers {
                workers.send(engine.env.now, channel, down, &engine.values, &mut engine.events);
            }
        }
        root::Delivery::Assigned { channel, assignment } => {
            if let Some(workers) = &mut engine.workers {
                workers.assignment(engine.env.now, channel, assignment, &engine.values, &mut engine.events);
            }
        }
        root::Delivery::CallBusy { channel, task, attempt, name } => {
            if let Some(workers) = &mut engine.workers {
                workers.send(
                    engine.env.now,
                    channel,
                    jig_domain_host::LinkDown::Relayed {
                        run: skein_lib::Token::new(task),
                        attempt: skein_lib::Token::new(attempt),
                        call: name,
                        answer: jig_domain_host::Relayed::Busy,
                    },
                    &engine.values,
                    &mut engine.events,
                );
            }
        }
        root::Delivery::Message { .. } | root::Delivery::CallAnswer { .. } => {
            stop::unavailable(engine, b"workers");
        }
    }
}

fn restart(engine: &mut Engine, step: core::RestartStep) {
    match step {
        core::RestartStep::Switch => {
            if engine.store.pending {
                crate::store::request(engine, jig_test_store::StoreOut::Restart { step, census: None });
            } else {
                engine.events.push(root::Event::Switched);
            }
        }
        core::RestartStep::ReadAfresh { connector: number } => {
            let paths = system(engine, number).paths();
            let count = u32::try_from(paths.len()).expect("bounded system resources");
            if count == 0 {
                engine.events.push(root::Event::RestartDone(step));
                return;
            }
            engine.restart_reads = Some((number, count));
            for resource in paths {
                engine.events.push(root::Event::Connector {
                    number,
                    event: connector::Event::Restart(connector::RestartStep::FreshRead { resource }),
                });
            }
        }
        core::RestartStep::LoadCore | core::RestartStep::LoadInbox | core::RestartStep::RestoreConnector { .. } => {
            let census = if step == core::RestartStep::LoadInbox {
                engine.root_steps = engine.root_steps.wrapping_add(1);
                Some(root::inbox_census(&mut engine.domain, &engine.env.limits))
            } else {
                None
            };
            crate::store::request(engine, jig_test_store::StoreOut::Restart { step, census });
        }
        core::RestartStep::AdoptRuns | core::RestartStep::SettleOutbox { .. } | core::RestartStep::Open => {
            stop::invalid_restore(engine);
        }
    }
}
fn restart_read(engine: &mut Engine, number: u16) {
    if let Some((owner, left)) = engine.restart_reads
        && owner == number
    {
        let left = left.checked_sub(1).expect("one answer per restart read");
        engine.restart_reads = if left == 0 { None } else { Some((owner, left)) };
        if left == 0 {
            engine.events.push(root::Event::RestartDone(core::RestartStep::ReadAfresh { connector: number }));
        }
    }
}

pub(super) fn system(engine: &mut Engine, number: u16) -> &mut crate::system::System {
    let index = usize::from(number.checked_sub(1).expect("connector numbers start at one"));
    engine.systems.get_mut(index).expect("the test application owns two connectors")
}

fn immediate(engine: &mut Engine, now: core::Now) {
    match now {
        value @ (core::Now::SeenConversation { .. }
        | core::Now::LoadEndedRead { .. }
        | core::Now::HistoricalPersonTask { .. }
        | core::Now::EndedTask { .. }
        | core::Now::PartyInputs { .. }
        | core::Now::HistoricalProposal { .. }
        | core::Now::DecidedProposal { .. }
        | core::Now::HistoricalEscalation { .. }
        | core::Now::StartPreparation { .. }) => {
            crate::store::request(engine, jig_test_store::StoreOut::Now(value));
        }
        // Configuration is parsed whole; inactive account components receive no requests.
        core::Now::Account(request) => {
            if let Some(accounts) = &mut engine.accounts {
                accounts.request(request);
            }
        }
        value @ (core::Now::SignInRefused { .. } | core::Now::WatchRefused { .. } | core::Now::RunText { .. }) => {
            if let Some(clients) = &mut engine.clients {
                let event = clients::immediate(value, &mut engine.admission).expect("immediate client output");
                clients.down(event);
            } else {
                engine.outputs.push(PartyOutput::Now(value));
            }
        }
        core::Now::Call { to, run, attempt, call } => engine.events.push(engine.tools.decode(to, run, attempt, &call)),
        core::Now::SettingsAnswer { to, key, answer } => {
            if let Some(event) = engine.tools.settings_answer(to, key, &answer) {
                engine.events.push(event);
            }
        }
        core::Now::EffectAnswer { to, key, part } => {
            if let Some(event) = engine.tools.render(to, key, &part, false) {
                engine.events.push(event);
            }
        }
        core::Now::RelayBusy { channel, run, attempt, call } => {
            if let Some(workers) = &mut engine.workers {
                workers.send(
                    engine.env.now,
                    channel,
                    jig_domain_host::LinkDown::Relayed { run, attempt, call, answer: jig_domain_host::Relayed::Busy },
                    &engine.values,
                    &mut engine.events,
                );
            }
        }
        core::Now::DropCall { call } => {
            engine.tools.forget_name(&call.name);
        }
        core::Now::NoteBusy { key, .. } | core::Now::SettledCallRefused { key, .. } => engine.tools.forget(key),
        core::Now::CallBusy { .. }
        | core::Now::Relayed { .. }
        | core::Now::Undelivered { .. }
        | core::Now::NotesIndexed { .. }
        | core::Now::NotesRecalled { .. }
        | core::Now::NotesRefused { .. } => stop::unavailable(engine, b"accounts"),
        core::Now::DropPayload { .. }
        | core::Now::DropAssignment { .. }
        | core::Now::TurnPayload { .. }
        | core::Now::AnswerPayload { .. }
        | core::Now::AcceptedTurn { .. }
        | core::Now::RefusedPayload { .. }
        | core::Now::Activate { .. }
        | core::Now::PrepareAgent { .. }
        | core::Now::CompleteBrief { .. }
        | core::Now::BriefCorePlanned { .. }
        | core::Now::WorkspaceRequest { .. }
        | core::Now::RunPreparationFailed { .. }
        | core::Now::RunPrepared { .. }
        | core::Now::RestoreRefused => stop::invalid_restore(engine),
    }
}

fn core_held(engine: &mut Engine, held: core::Held) {
    match held {
        core::Held::Sources { .. } | core::Held::CheckSource { .. } | core::Held::SignOutSource { .. } => {
            unreachable!("link translates host-wide source commands")
        }
        core::Held::Drained => stop::drained(engine),
        core::Held::Load(load) => immediate(engine, *load),
        core::Held::ReadLoad { load, range } => {
            crate::store::request(engine, jig_test_store::StoreOut::ReadLoad { load, range });
        }
        core::Held::NotesLoad { owner, range } => {
            crate::store::request(engine, jig_test_store::StoreOut::NotesLoad { owner, range });
        }
        held @ (core::Held::Read { .. }
        | core::Held::Delivery { .. }
        | core::Held::WatchEnded { .. }
        | core::Held::PeopleReply { .. }
        | core::Held::CarriedUnused { .. }) => {
            if let Some(client) = &mut engine.clients {
                client.down(clients::to_component(held).expect("clients output"));
            } else {
                engine.outputs.push(PartyOutput::Held(root::Delivery::Core(held)));
            }
        }
        held @ (core::Held::Acted { .. } | core::Held::Result { .. } | core::Held::SourceSignedOut { .. }) => {
            engine.outputs.push(PartyOutput::Held(root::Delivery::Core(held)));
        }
        core::Held::MakeEffect { connector: number, entry } => {
            engine.events.push(root::Event::Connector { number, event: connector::Event::Make { entry } });
        }
        core::Held::TurnBusy { channel, run, attempt, turn } => {
            if let Some(workers) = &mut engine.workers {
                workers.down(
                    engine.env.now,
                    jig_protocol_engine_workers::Event::TurnBusy { channel, run, attempt, turn },
                    &mut engine.events,
                );
            }
        }
        core::Held::Refuse { channel } => {
            if let Some(workers) = &mut engine.workers {
                workers.down(
                    engine.env.now,
                    jig_protocol_engine_workers::Event::NotAdmitted {
                        channel,
                        text: b"not admitted".as_slice().into(),
                    },
                    &mut engine.events,
                );
            }
        }
        core::Held::CallAnswer { to, key, part } => {
            if let Some(event) = engine.tools.render(to, key, &part, true) {
                engine.events.push(event);
            }
        }
        core::Held::Assign { .. }
        | core::Held::Grant { .. }
        | core::Held::Relayed { .. }
        | core::Held::Inbound { .. }
        | core::Held::Relay { .. }
        | core::Held::Acknowledge { .. }
        | core::Held::TaskTerminalAcknowledged { .. }
        | core::Held::TaskTurnKept { .. }
        | core::Held::AcknowledgeTurn { .. }
        | core::Held::Cancel { .. }
        | core::Held::Park { .. }
        | core::Held::StopRun { .. }
                | core::Held::ParkRun { .. } => stop::unavailable(engine, b"workers"),
        core::Held::SettledCall { .. } | core::Held::NotesWritten { .. } | core::Held::NotesDeleted { .. } => {
            stop::unavailable(engine, b"accounts");
        }
        core::Held::Applied { .. }
        | core::Held::ApplicationDelivery { .. }
        | core::Held::ApplicationWatchEnded { .. } => stop::invalid_restore(engine),
    }
}
