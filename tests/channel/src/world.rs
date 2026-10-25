//! Two reference services joined by loopback, with independent parties and durable rows.
//! Contract: protocol/channel.md, 12, on the real deployment store.
use crate::{
    fixtures,
    processes::{self, Machine, Proc},
};
use jig_domain_core as core;
use jig_domain_people as people;
use jig_fake_parties as parties;
use jig_test_domain as root;
use jig_test_service::PartyOutput;
use skein_lib::Duration;
use skein_world::domain::Verdict;
use std::{cell::RefCell, rc::Rc};

pub mod store;
pub(crate) use store::{Capture, Store};

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub seed: u64,
    pub config: skein_sim::Config,
}
#[derive(Clone, Debug)]
pub struct Script {
    pub words: Box<[u8]>,
    pub result: Box<[u8]>,
    pub leading_turns: u32,
    pub latency: Duration,
    pub story: crate::stories::Story,
    pub recorded: Option<Recorded>,
}
#[derive(Clone, Debug)]
pub struct Recorded {
    pub(crate) turn: jig_protocol_worker_wire::Turn,
    pub(crate) answer: jig_protocol_worker_wire::Answer,
}
#[derive(Debug)]
#[expect(clippy::struct_excessive_bools, reason = "independent protocol observations for story assertions")]
pub struct Outcome {
    pub verdict: Verdict,
    pub turns_committed: u32,
    pub answers_acknowledged: u32,
    pub calls_answered: u32,
    pub calls_busy: u32,
    pub named_call_error_observed: bool,
    pub services_empty: bool,
    pub messages_ended: u32,
    pub refusal: Option<u16>,
    pub reset_injected: bool,
    pub adopting_hellos: u32,
    pub agent_starts: u32,
    pub activity_seen: bool,
    pub text_bytes: usize,
    pub text_prefixes: Vec<Box<[u8]>>,
    pub text_committed: bool,
    pub start_messages: u32,
    pub waited: bool,
    pub windows: u32,
    pub resumed_turns: Option<u32>,
    pub identity_kept: bool,
    pub heartbeats: u32,
    pub channel_closed: bool,
    pub lost: u32,
    pub shut_down: bool,
    pub contact_cancelled: bool,
    pub shutdown_cancelled: bool,
    pub refused_messages: Vec<jig_protocol_worker_wire::MessageRefusal>,
    pub recorded: Option<Recorded>,
    pub refusals_kept: bool,
    pub engine_drained: bool,
    pub commits_waiting_at_drain: usize,
    pub durable_at_restart: u64,
    pub held_at_restart: Option<u64>,
    pub recovered_commits: Vec<u64>,
    pub query_submissions: Option<u32>,
    pub held_turn_submissions: Option<u32>,
    pub providers_before: (Option<u32>, Vec<(u32, u32, u32)>),
    pub providers_after: (Option<u32>, Vec<(u32, u32, u32)>),
}

/// Run one person's chat through the actual agent and all channel layers.
#[must_use]
pub fn run(settings: Settings, script: Script) -> Outcome {
    let held_turn = if matches!(script.story, crate::stories::Story::DurableRestart) {
        // Replay the identical prefix, releasing its first observed held turn instead of killing it.
        Some(run_at(settings, script.clone(), None).held_turn_submissions.expect("first observed held turn"))
    } else {
        None
    };
    run_at(settings, script, held_turn)
}

fn run_at(mut settings: Settings, script: Script, held_turn: Option<u32>) -> Outcome {
    if matches!(script.story, crate::stories::Story::MidFrame) {
        settings.config.buffer = 64;
    }
    let store = Rc::new(RefCell::new(Store::new()));
    let views = Rc::new(RefCell::new(Views::default()));
    let referee = make_referee(
        &script,
        store.clone(),
        views.clone(),
        matches!(script.story, crate::stories::Story::DurableRestart) && held_turn.is_none(),
    );
    let mut machine = skein_fake_machine::Machine::new();
    let _store_root = machine.lay(&[]);
    let client_root = machine.lay(&[]);
    let root = (!matches!(script.story, crate::stories::Story::Opening(_))).then(|| machine.lay(&[]));
    let mut harness = skein_world::World::new(settings.seed, settings.config, referee, skein_world::Memory::Unchecked)
        .with_machine(Machine(machine));
    launch(&mut harness, root, &script, held_turn);
    harness.spawn_root(skein_sim::Handle::new(client_root.raw()), |root| {
        let mut client = jig_client_protocol_world::native::NativeClient::new(
            jig_client_protocol_world::native::settings(),
            root,
            1000,
            settings.seed,
            jig_client_protocol_world::native::Script::account_sign_in(),
        );
        client.hold_for_account_sign_in();
        Proc::Client(Box::new(client))
    });
    let scenario = script.story;
    drop(script);
    let mut reset_injected = false;
    let settled = harness.run_with_faults(|at, _, submissions| {
        let reset = !reset_injected && (matches!(scenario, crate::stories::Story::MidFrame) && at == 0
            && submissions.iter().any(|submission| matches!(&submission.kind,
                skein_io::kernel::Op::Send { bytes, from, .. }
                if *from > 8 && bytes.len() >= 8 && skein_channel::parse_header(&bytes[..8]).is_ok_and(|head| head.kind == 0x0180))) || matches!(scenario, crate::stories::Story::Reconnect) && at == 1 && submissions.iter().any(|submission| matches!(&submission.kind, skein_io::kernel::Op::Send { bytes, .. } if bytes.len() >= 8 && skein_channel::parse_header(&bytes[..8]).is_ok_and(|head| head.kind == 0x0104))));
        if reset { reset_injected = true; }
        Some(skein_sim::Faults { reset: if reset { 1000 } else { 0 }, ..settings.config.faults })
    });
    crate::referee::roots_at_end(&settled.procs);
    outcome(&settled.procs, &store.borrow(), &views.borrow(), reset_injected, settled.killed.len())
}

fn outcome(procs: &[Proc], store: &Store, views: &Views, reset_injected: bool, killed: usize) -> Outcome {
    let worker = procs.iter().find_map(|proc| match proc {
        Proc::Worker(host) => Some(&host.wire),
        Proc::Probe(peer) => Some(&peer.wire),
        Proc::Engine(_) | Proc::Agent { .. } | Proc::Peer(_) | Proc::Auth(_) | Proc::Client(_) => None,
    });
    let refusal = procs.iter().find_map(|proc| if let Proc::Probe(peer) = proc { peer.refused } else { None });
    let turns = turns(store);
    let assignments: Vec<_> = worker
        .into_iter()
        .flat_map(|wire| wire.frames.iter())
        .filter(|(_, kind, _)| *kind == 0x0180)
        .map(|(_, _, bytes)| {
            jig_protocol_worker_wire::Assignment::decode(
                &jig_protocol_worker_wire::CEILINGS,
                &mut skein_lib::Reader::new(bytes),
            )
            .expect("received assignment")
        })
        .collect();
    Outcome {
        durable_at_restart: views.durable_at_restart,
        held_at_restart: views.held_at_restart,
        query_submissions: views.query_submissions,
        held_turn_submissions: views.held_turn_submissions,
        providers_before: views.providers_before.clone(),
        providers_after: views.providers_after.clone(),
        recovered_commits: store.recovered.iter().copied().collect(),
        engine_drained: views.engine_drained,
        commits_waiting_at_drain: views.commits_waiting_at_drain,
        recorded: recorded(worker),
        refusals_kept: views.refusals_kept,
        refused_messages: collect_refusals(procs),
        contact_cancelled: cancelled(worker, &jig_protocol_worker_wire::CancelReason::Contact),
        shutdown_cancelled: cancelled(worker, &jig_protocol_worker_wire::CancelReason::Shutdown),
        lost: u32::try_from(store.rows.values().filter(|row| matches!(row, root::Record::Core(core::Record::Core(core::CoreRecord::Terminal(answer))) if answer.end == jig_domain_tasks::End::Failed(jig_domain_tasks::Class::Lost))).count()).expect("lost terminals"),
        shut_down: procs.iter().any(|proc| matches!(proc, Proc::Worker(host) if host.stopping)),
        heartbeats: procs.iter().find_map(|proc| if let Proc::Probe(peer) = proc { Some(peer.heartbeats) } else { None }).unwrap_or(0),
        channel_closed: procs.iter().any(|proc| matches!(proc, Proc::Engine(host) if host.wire.closed)),
        windows: u32::try_from(turns.iter().filter(|turn| turn.opens_window).count().saturating_sub(1)).expect("window count"),
        resumed_turns: assignments.last().map(|assignment| transcript_count(assignment.transcript())),
        identity_kept: assignments.last().is_some_and(|next| assignments[0].identity() == next.identity()),
        activity_seen: views.activity_seen,
        text_bytes: views.text_bytes,
        text_prefixes: views.reply.prefixes.clone(),
        text_committed: views.reply.committed,
        verdict: Verdict::Passed,
        turns_committed: u32::try_from(turns.len()).expect("bounded turns"),
        answers_acknowledged: worker.map_or(0, |wire| u32::try_from(wire.acknowledgements.len()).expect("bounded answers")),
        calls_answered: worker.map_or(0, |wire| wire.calls_answered),
        calls_busy: worker.map_or(0, |wire| u32::try_from(wire.frames.iter().filter(|(_, kind, bytes)| {
            *kind == 0x0182 && jig_protocol_worker_wire::Relayed::decode(&jig_protocol_worker_wire::CEILINGS,
                &mut skein_lib::Reader::new(bytes)).is_ok_and(|relay| matches!(relay.answer(), jig_protocol_worker_wire::RelayAnswer::Busy))
        }).count()).expect("busy count")),
        named_call_error_observed: procs.iter().any(|proc| {
            let Proc::Peer(peer) = proc else { return false };
            smith_agent_process_world::fake::queries(peer).any(|query| {
                use skein_fake_llm_domain::api::Part;
                query.messages.iter().flat_map(|message| message.parts.iter()).any(|part| {
                    let Part::ToolOutput { id, output, .. } = part else { return false };
                    output.as_ref() == b"Error: call too large" && query.messages.iter().flat_map(|message| message.parts.iter()).any(|part|
                        matches!(part, Part::ToolCall { id: original, name, arguments } if original == id && name.as_ref() == b"recall" && arguments.len() == 65))
                })
            })
        }),
        services_empty: procs.iter().all(|proc| match proc {
            Proc::Engine(host) | Proc::Worker(host) => host.service.is_empty(),
            Proc::Agent { .. } | Proc::Peer(_) | Proc::Auth(_) | Proc::Client(_) | Proc::Probe(_) => true,
        }),
        refusal,
        reset_injected,
        adopting_hellos: worker.map_or(0, |wire| wire.adopting_hellos),
        agent_starts: u32::try_from(killed + procs.iter().filter(|proc| matches!(proc, Proc::Agent { .. })).count()).expect("agent count"),
        messages_ended: u32::try_from(views.messages_ended).expect("bounded messages"),
        start_messages: worker.map_or(0, |wire| wire.start_messages),
        waited: worker.is_some_and(|wire| wire.waited),
    }
}

pub(crate) fn turns(store: &Store) -> Vec<&core::TurnRecord> {
    store
        .rows
        .values()
        .filter_map(|row| match row {
            root::Record::Core(core::Record::Core(core::CoreRecord::Turn(turn))) => Some(turn),
            root::Record::Core(_) | root::Record::Connector { .. } => None,
        })
        .collect()
}
pub(crate) fn terminal(store: &Store, task: u64, attempt: u64, scenario: crate::stories::Story) -> bool {
    store.rows.values().any(|row| matches!(row,
        root::Record::Core(core::Record::Core(core::CoreRecord::Terminal(answer)))
        if answer.task == task && answer.attempt == attempt && if matches!(scenario, crate::stories::Story::InvalidCharter) || matches!(scenario, crate::stories::Story::InvalidTranscript) && attempt != 1 {
            answer.end == jig_domain_tasks::End::Failed(jig_domain_tasks::Class::Invalid)
        } else if matches!(scenario, crate::stories::Story::Window | crate::stories::Story::InvalidTranscript) && (attempt == 1 || matches!(scenario, crate::stories::Story::Window) && attempt == 2) {
            answer.end == jig_domain_tasks::End::Parked
        } else if matches!(scenario, crate::stories::Story::BeyondGrace | crate::stories::Story::Shutdown) { answer.end == jig_domain_tasks::End::Failed(jig_domain_tasks::Class::Agent) } else { matches!(answer.end, jig_domain_tasks::End::Finished { .. }) }
    ))
}
pub(crate) fn pending_turn(store: &Store) -> bool {
    store.pending.iter().any(|pending| {
        pending.writes.iter().any(|write| {
            matches!(write, root::Write::Save(root::Record::Core(core::Record::Core(core::CoreRecord::Turn(_)))))
        })
    })
}

pub(crate) fn answer_store(
    host: &mut processes::ServiceHost,
    store: &mut Store,
    rules: &mut crate::referee::Rules,
    hold: bool,
    scenario: crate::stories::Story,
    now: skein_lib::Time,
) {
    host.holding = hold;
    host.corrupt_transcript = matches!(scenario, crate::stories::Story::InvalidTranscript);
    let names = if matches!(scenario, crate::stories::Story::Providers) {
        fixtures::providers_store_names()
    } else {
        fixtures::store_names()
    };
    for commit in store.observe(&mut host.files.events, &host.codec, names, &host.store_limits.root.connector) {
        for write in &commit.writes {
            if let root::Write::Save(row) = write {
                match row {
                    root::Record::Core(core::Record::Core(core::CoreRecord::Turn(turn))) => {
                        rules.turned(turn.task, turn.attempt, turn.turn);
                        rules.read(turn.task, turn.attempt, turn.read);
                    }
                    root::Record::Core(core::Record::Core(core::CoreRecord::Terminal(answer))) => {
                        answer_refusals(host, store, answer, rules);
                        if answer.end == jig_domain_tasks::End::Failed(jig_domain_tasks::Class::Lost) {
                            rules.lost_ended(answer.task, answer.attempt, now);
                        } else {
                            rules.answered(answer.task, answer.attempt);
                        }
                    }
                    root::Record::Core(core::Record::Tasks(
                        jig_domain_tasks::Stored::Live(task) | jig_domain_tasks::Stored::Ended(task),
                    )) => {
                        for &name in &task.refused {
                            rules.message_refused(task.number, task.attempt, name);
                        }
                    }
                    root::Record::Core(_) | root::Record::Connector { .. } => {}
                }
            }
        }
    }
}

// An ended attempt drops its refusal marks; its refused messages remain in the durable inbox (tasks.md, 8.2).
fn answer_refusals(
    host: &processes::ServiceHost,
    store: &Store,
    terminal: &core::TerminalRecord,
    rules: &mut crate::referee::Rules,
) {
    for (_, kind, bytes) in &host.wire.frames {
        if *kind != 0x0103 {
            continue;
        }
        let answer = jig_protocol_worker_wire::Answer::decode(
            &jig_protocol_worker_wire::CEILINGS,
            &mut skein_lib::Reader::new(bytes),
        )
        .expect("received terminal");
        if (answer.task(), answer.attempt()) != (terminal.task, terminal.attempt) {
            continue;
        }
        for refused in answer.refused() {
            let retained = store.rows.values().any(|row| match row {
                root::Record::Core(core::Record::Tasks(
                    jig_domain_tasks::Stored::Live(task) | jig_domain_tasks::Stored::Ended(task),
                )) => {
                    task.number == terminal.task
                        && task.attempt == terminal.attempt
                        && task.inbox.iter().any(|word| word.number == refused.name())
                }
                root::Record::Core(_) | root::Record::Connector { .. } => false,
            });
            assert!(retained, "refused message {} is retained in the durable terminal inbox", refused.name());
            rules.message_refused(terminal.task, terminal.attempt, refused.name());
        }
    }
}

pub(crate) fn answer_parties(engine: &mut jig_test_service::Engine, party: &mut parties::Party, views: &mut Views) {
    while let Some(output) = engine.parties_out().pop() {
        match output {
            PartyOutput::Held(root::Delivery::Core(core::Held::PeopleReply { to, reply })) => {
                let reply = match reply {
                    people::Reply::SignedIn { person, sign_in, .. } => parties::Reply::SignedIn { person, sign_in },
                    people::Reply::Outcome(people::Outcome::Created { first: task, .. }) => {
                        parties::Reply::Started { task }
                    }
                    people::Reply::Outcome(people::Outcome::Said { task, message }) => {
                        parties::Reply::Said { task, message }
                    }
                    people::Reply::Signer(_) | people::Reply::Full { .. } => {
                        parties::Reply::Again(parties::Again::Busy)
                    }
                    people::Reply::Again(reason) => parties::Reply::Again(match reason {
                        people::Again::Busy => parties::Again::Busy,
                        people::Again::NotReady => parties::Again::NotReady,
                    }),
                    other @ (people::Reply::SignedOut | people::Reply::Outcome(_) | people::Reply::Refused(_)) => {
                        panic!("one-chat reply {other:?}")
                    }
                };
                party.reply(to.into_token().raw(), reply);
            }
            PartyOutput::Now(core::Now::SignInRefused { to }) => {
                party.reply(to.into_token().raw(), parties::Reply::Again(parties::Again::NotReady));
            }
            PartyOutput::Held(root::Delivery::Core(core::Held::Delivery { watcher, document })) => {
                if let core::documents::Document::Task(task) = &*document {
                    views.activity_seen |= task.chip.details.observation.is_some();
                }
                if let core::documents::Document::Run(run) = &*document
                    && let Some(writing) = &run.writing
                {
                    assert!(writing.turn.turn > run.committed_turn, "committed turns have no writing");
                }
                engine.parties_in().push(core::Event::Delivered { watcher });
            }
            PartyOutput::Now(core::Now::RunText { watcher, text, .. }) => {
                views.text_bytes += text.len();
                engine.parties_in().push(core::Event::StreamDelivered { watcher });
            }
            PartyOutput::Busy(event) => {
                let root::Event::Core(event) = *event else { panic!("party port busy with {event:?}") };
                engine.parties_in().push(event);
            }
            PartyOutput::Held(_) | PartyOutput::Now(_) => {}
        }
    }
}

pub(crate) fn diagnostics(procs: &[Proc]) -> String {
    procs
        .iter()
        .filter_map(|proc| match proc {
            Proc::Agent { errors, .. } => Some(String::from_utf8_lossy(&errors.bytes()).into_owned()),
            Proc::Worker(host) => Some(format!(
                "{:?}",
                host.wire
                    .frames
                    .iter()
                    .filter_map(|(_, kind, bytes)| if *kind == 0x0103 {
                        Some(
                            jig_protocol_worker_wire::Answer::decode(
                                &jig_protocol_worker_wire::CEILINGS,
                                &mut skein_lib::Reader::new(bytes),
                            )
                            .expect("answer")
                            .ending()
                            .clone(),
                        )
                    } else {
                        None
                    })
                    .collect::<Vec<_>>()
            )),
            Proc::Peer(peer) => Some(format!(
                "{:?}",
                peer.observations()
                    .iter()
                    .filter(|o| matches!(o, skein_fake_peers::llm::Observation::Answered { .. }))
                    .collect::<Vec<_>>()
            )),
            Proc::Engine(host) => Some(format!(
                "engine turns {:?}",
                host.wire
                    .frames
                    .iter()
                    .filter_map(|(_, kind, bytes)| if *kind == 0x0101 {
                        let t = jig_protocol_worker_wire::Turn::decode(
                            &jig_protocol_worker_wire::CEILINGS,
                            &mut skein_lib::Reader::new(bytes),
                        )
                        .expect("wire turn");
                        Some((t.task(), t.attempt(), t.number(), t.spent(), *t.read(), t.opens_window()))
                    } else {
                        None
                    })
                    .collect::<Vec<_>>()
            )),
            Proc::Probe(_) | Proc::Auth(_) | Proc::Client(_) => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn worker_wire(procs: &[Proc]) -> &processes::Wire {
    procs
        .iter()
        .find_map(|proc| match proc {
            Proc::Worker(worker) => Some(&worker.wire),
            Proc::Probe(peer) => Some(&peer.wire),
            Proc::Engine(_) | Proc::Agent { .. } | Proc::Peer(_) | Proc::Auth(_) | Proc::Client(_) => None,
        })
        .expect("worker's wire observations")
}

#[derive(Default)]
pub(crate) struct Views {
    pub(crate) activity_seen: bool,
    pub(crate) text_bytes: usize,
    pub(crate) reply: crate::live_text::Reply,
    pub(crate) messages_ended: usize,
    pub(crate) refusals_kept: bool,
    pub(crate) engine_drained: bool,
    pub(crate) commits_waiting_at_drain: usize,
    pub(crate) durable_at_restart: u64,
    pub(crate) held_at_restart: Option<u64>,
    pub(crate) query_submissions: Option<u32>,
    pub(crate) held_turn_submissions: Option<u32>,
    pub(crate) providers_before: (Option<u32>, Vec<(u32, u32, u32)>),
    pub(crate) providers_after: (Option<u32>, Vec<(u32, u32, u32)>),
}

fn transcript_count(bytes: &[u8]) -> u32 {
    let mut reader = skein_lib::Reader::new(bytes);
    let mut count = 0;
    while !reader.is_empty() {
        let length = reader.u32().expect("transcript piece length");
        reader.skip(length).expect("transcript piece body");
        count += 1;
    }
    count
}

fn make_referee(
    script: &Script,
    store: Rc<RefCell<Store>>,
    views: Rc<RefCell<Views>>,
    calibrating_held_turn: bool,
) -> crate::referee::Referee {
    crate::referee::Referee {
        calibrating_held_turn,
        store,
        accounts: crate::account_setup::Setup::default(),
        party: parties::Party::new(1, parties::Role::Owner, 100, fixtures::party_script(script)),
        period: false,
        passed: false,
        stopped: false,
        rules: crate::referee::Rules::default(),
        observed: 0,
        expected_turns: match script.story {
            crate::stories::Story::InvalidCharter
            | crate::stories::Story::BeyondGrace
            | crate::stories::Story::Shutdown => 0,
            crate::stories::Story::InvalidTranscript => 1,
            crate::stories::Story::Window => 7,
            crate::stories::Story::Facts => 2,
            crate::stories::Story::Drain
            | crate::stories::Story::ApplicationGeneration
            | crate::stories::Story::Key { .. }
            | crate::stories::Story::Providers
            | crate::stories::Story::Chat
            | crate::stories::Story::Stalled
            | crate::stories::Story::Quiet
            | crate::stories::Story::Replaced
            | crate::stories::Story::Reconnect
            | crate::stories::Story::Refusals(_)
            | crate::stories::Story::Opening(_)
            | crate::stories::Story::Progress
            | crate::stories::Story::Inbox
            | crate::stories::Story::Call
            | crate::stories::Story::Wait
            | crate::stories::Story::MidFrame
            | crate::stories::Story::Restart { .. }
            | crate::stories::Story::HeldCommit
            | crate::stories::Story::DurableRestart
            | crate::stories::Story::Partition { .. } => script.leading_turns + 1,
        },
        expected_answers: if matches!(script.story, crate::stories::Story::Window) { 3 } else { 1 },
        story: script.story,
        injected: false,
        resumed_attempt: 0,
        engine_observed: 0,
        roots_observed: std::collections::BTreeMap::new(),
        formatted: String::new(),
        grants_observed: 0,
        engine_instance: None,
        views,
    }
}

fn launch(
    harness: &mut skein_world::World<Proc, crate::referee::Referee, Machine>,
    root: Option<skein_fake_machine::Opened>,
    script: &Script,
    held_turn: Option<u32>,
) {
    harness.host(skein_world::HostedProgram {
        program: b"agent-program".as_slice().into(),
        make: if matches!(script.story, crate::stories::Story::BeyondGrace) {
            processes::agent_slow
        } else if matches!(script.story, crate::stories::Story::Providers) {
            processes::agent_providers
        } else if matches!(script.story, crate::stories::Story::Key { .. }) {
            processes::agent_anthropic
        } else {
            processes::agent
        },
        instances: 2,
        operations: smith_agent_process_world::limits().routes.checked_add(2).expect("agent operations"),
    });
    let engine_factory = if matches!(script.story, crate::stories::Story::Providers) {
        processes::engine_providers
    } else if matches!(
        script.story,
        crate::stories::Story::Refusals(
            crate::stories::MessageRefusal::TooLarge | crate::stories::MessageRefusal::Replay
        )
    ) {
        processes::engine_message
    } else if matches!(script.story, crate::stories::Story::Drain | crate::stories::Story::Providers) {
        processes::engine_drain
    } else if matches!(script.story, crate::stories::Story::Call) {
        processes::engine_calls
    } else if matches!(script.story, crate::stories::Story::Window) {
        processes::engine_window
    } else if matches!(script.story, crate::stories::Story::ApplicationGeneration) {
        processes::engine_application_generation
    } else {
        processes::engine
    };
    let mut roots =
        vec![skein_world::StartupRoot { name: b"state".as_slice().into(), path: b"state".as_slice().into() }];
    if matches!(script.story, crate::stories::Story::Drain | crate::stories::Story::Providers) {
        roots.push(skein_world::StartupRoot {
            name: b"restart-state".as_slice().into(),
            path: b"state".as_slice().into(),
        });
    }
    harness.spawn_restartable(roots, engine_factory);
    // The real key request is kept before these channel-story cuts.
    if matches!(script.story, crate::stories::Story::Refusals(crate::stories::MessageRefusal::Replay)) {
        // Counts the engine's submissions from spawn, including generation startup
        // and generation 1's manifest (ten). The trace sends the assignment at 177
        // and the message at 193; cut before receive 200 brings the native run's
        // first turn carrying its refusal.
        const BEFORE_CARRIED_REFUSAL: u32 = 200;
        harness.cut(0, BEFORE_CARRIED_REFUSAL, skein_world::Cut::Kill);
    }
    if let Some(before) = held_turn {
        harness.cut(0, before, skein_world::Cut::Kill);
    }
    if let crate::stories::Story::Restart { before } = script.story {
        harness.cut(0, before, skein_world::Cut::Kill);
    }

    spawn_channel_side(harness, root, script);
    if !matches!(
        script.story,
        crate::stories::Story::Opening(_)
            | crate::stories::Story::Stalled
            | crate::stories::Story::Quiet
            | crate::stories::Story::Replaced
            | crate::stories::Story::Refusals(
                crate::stories::MessageRefusal::Full | crate::stories::MessageRefusal::NameInUse
            )
    ) {
        harness.spawn(|| {
            let scripts = Box::new([skein_fake_llm_domain::api::Script {
                cue: Box::default(),
                turns: fixtures::provider_turns(script),
            }]);
            let peer = if matches!(script.story, crate::stories::Story::Window) {
                processes::window_provider(scripts)
            } else if matches!(script.story, crate::stories::Story::Key { .. }) {
                let mut peer = fixtures::provider(
                    scripts,
                    skein_llm::Credential::key(fixtures::KEY.into()),
                    script.latency,
                    skein_fake_peers::Transport::Plaintext,
                    skein_fake_llm_protocol::documents::Provider::Anthropic,
                );
                peer.accept(skein_llm::Credential::key(fixtures::NEXT_KEY.into())).expect("the next key");
                peer
            } else {
                fixtures::provider(
                    scripts,
                    skein_llm::Credential::bearer(fixtures::TOKEN.into()),
                    script.latency,
                    skein_fake_peers::Transport::Plaintext,
                    skein_fake_llm_protocol::documents::Provider::OpenAi,
                )
            };
            Proc::Peer(Box::new(peer))
        });
    }
}

fn spawn_channel_side(
    harness: &mut skein_world::World<Proc, crate::referee::Referee, Machine>,
    root: Option<skein_fake_machine::Opened>,
    script: &Script,
) {
    if matches!(
        script.story,
        crate::stories::Story::Refusals(
            crate::stories::MessageRefusal::Full | crate::stories::MessageRefusal::NameInUse
        )
    ) {
        harness.spawn(|| {
            Proc::Probe(Box::new(crate::relay::Peer::refusing(
                if matches!(script.story, crate::stories::Story::Refusals(crate::stories::MessageRefusal::Full)) {
                    jig_protocol_worker_wire::MessageRefusal::Full
                } else {
                    jig_protocol_worker_wire::MessageRefusal::NameInUse
                },
                script.recorded.clone().expect("recorded native wire"),
            )))
        });
    } else if matches!(script.story, crate::stories::Story::Replaced) {
        harness.spawn(|| Proc::Probe(Box::new(crate::relay::Peer::silent(false))));
        harness.spawn(|| {
            let mut peer = crate::relay::Peer::silent(false);
            peer.start_at = skein_lib::Time::from_nanos(1_000_000_000);
            Proc::Probe(Box::new(peer))
        });
    } else if matches!(script.story, crate::stories::Story::Stalled | crate::stories::Story::Quiet) {
        harness.spawn(|| {
            Proc::Probe(Box::new(crate::relay::Peer::silent(matches!(script.story, crate::stories::Story::Stalled))))
        });
    } else if let crate::stories::Story::Opening(case) = script.story {
        harness.spawn(|| Proc::Probe(Box::new(crate::relay::Peer::opening(case))));
    } else {
        let factory = if matches!(script.story, crate::stories::Story::Providers) {
            processes::worker_providers
        } else if matches!(
            script.story,
            crate::stories::Story::Refusals(
                crate::stories::MessageRefusal::TooLarge | crate::stories::MessageRefusal::Replay
            )
        ) {
            processes::worker_message
        } else if matches!(script.story, crate::stories::Story::InvalidCharter) {
            processes::worker_bad_charter
        } else if matches!(script.story, crate::stories::Story::Call) {
            processes::worker_calls
        } else {
            processes::worker
        };
        harness.spawn_root(skein_sim::Handle::new(root.expect("worker root").raw()), factory);
    }
}

fn cancelled(worker: Option<&processes::Wire>, reason: &jig_protocol_worker_wire::CancelReason) -> bool {
    worker.is_some_and(|wire| wire.frames.iter().any(|(_, kind, bytes)| {
        *kind == 0x0103 && jig_protocol_worker_wire::Answer::decode(&jig_protocol_worker_wire::CEILINGS,
            &mut skein_lib::Reader::new(bytes)).is_ok_and(|answer| matches!(answer.ending(), jig_protocol_worker_wire::Ending::Failed(failed)
                if matches!(failed.failure(), jig_protocol_worker_wire::Failure::Cancelled(value) if value.value() == reason)))
    }))
}

fn collect_refusals(procs: &[Proc]) -> Vec<jig_protocol_worker_wire::MessageRefusal> {
    procs
        .iter()
        .filter_map(|proc| if let Proc::Engine(host) = proc { Some(host) } else { None })
        .flat_map(|host| host.wire.frames.iter())
        .flat_map(|(_, kind, bytes)| match *kind {
            0x0101 => jig_protocol_worker_wire::Turn::decode(
                &jig_protocol_worker_wire::CEILINGS,
                &mut skein_lib::Reader::new(bytes),
            )
            .expect("turn")
            .refused()
            .iter()
            .map(|r| r.reason().clone())
            .collect::<Vec<_>>(),
            0x0103 => jig_protocol_worker_wire::Answer::decode(
                &jig_protocol_worker_wire::CEILINGS,
                &mut skein_lib::Reader::new(bytes),
            )
            .expect("answer")
            .refused()
            .iter()
            .map(|r| r.reason().clone())
            .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect()
}

fn recorded(worker: Option<&processes::Wire>) -> Option<Recorded> {
    let wire = worker?;
    let turn = wire.frames.iter().find_map(|(_, kind, bytes)| {
        if *kind == 0x0101 {
            Some(
                jig_protocol_worker_wire::Turn::decode(
                    &jig_protocol_worker_wire::CEILINGS,
                    &mut skein_lib::Reader::new(bytes),
                )
                .expect("native turn"),
            )
        } else {
            None
        }
    })?;
    let answer = wire.frames.iter().find_map(|(_, kind, bytes)| {
        if *kind == 0x0103 {
            Some(
                jig_protocol_worker_wire::Answer::decode(
                    &jig_protocol_worker_wire::CEILINGS,
                    &mut skein_lib::Reader::new(bytes),
                )
                .expect("native answer"),
            )
        } else {
            None
        }
    })?;
    Some(Recorded { turn, answer })
}

pub(crate) fn provider_choices(store: &Store) -> (Option<u32>, Vec<(u32, u32, u32)>) {
    let main = store.rows.values().find_map(|row| {
        if let root::Record::Core(core::Record::Tasks(
            jig_domain_tasks::Stored::Live(task) | jig_domain_tasks::Stored::Ended(task),
        )) = row
        {
            task.kept_account
        } else {
            None
        }
    });
    let bindings = store
        .rows
        .values()
        .find_map(|row| {
            if let root::Record::Core(core::Record::Core(core::CoreRecord::RunProof(proof))) = row {
                Some(proof.accounts.iter().map(|a| (a.provider, a.account, a.source)).collect())
            } else {
                None
            }
        })
        .unwrap_or_default();
    (main, bindings)
}
