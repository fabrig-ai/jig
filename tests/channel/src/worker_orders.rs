//! Worker FIFO and hand-out orders over the actual services and agent processes.
use crate::{
    account_setup, fixtures,
    processes::{self, Proc},
    referee::Rules,
    stories::Story,
    world::{self, Store, Views},
};
use jig_domain_core as core;
use jig_protocol_worker_wire as wire;
use skein_lib::{Duration, Reader, Time};
use std::{cell::RefCell, rc::Rc};

/// One run, one final turn and one result, as scripted by `one_chat`.
#[derive(Default, Debug)]
pub struct Outcome {
    pub records: Vec<u16>,
    pub turns: Vec<wire::Turn>,
    pub answers: Vec<wire::Answer>,
    pub acknowledgements: usize,
    pub starts: usize,
    pub starts_after_cancel: usize,
    pub admission_waits: u64,
}

enum Cancel {
    None,
    Ready,
    Sent,
}

struct Judge {
    accounts: account_setup::Setup,
    party: jig_fake_parties::Party,
    store: Store,
    rules: Rules,
    views: Views,
    period: bool,
    cancel: Cancel,
    done: bool,
    outcome: Rc<RefCell<Outcome>>,
    starts_at_cancel: Option<usize>,
}
impl skein_world::Referee<Proc> for Judge {
    fn act(&mut self, now: Time, procs: &mut [Proc]) {
        self.accounts.act(procs, Story::Chat, self.done);
        if !self.done {
            self.accounts.party(now, procs, &mut self.party, &mut self.views, Story::Chat);
        }
        if matches!(self.cancel, Cancel::Ready) {
            let assignment = procs.iter().find_map(|proc| {
                let Proc::Engine(host) = proc else { return None };
                host.wire.frames.iter().find_map(|(_, kind, bytes)| {
                    (*kind == wire::Kind::Assignment.number()).then(|| {
                        wire::Assignment::decode(&wire::CEILINGS, &mut Reader::new(bytes)).expect("assignment")
                    })
                })
            });
            if let Some(assignment) = assignment {
                let client = procs
                    .iter_mut()
                    .find_map(|proc| if let Proc::Client(client) = proc { Some(client) } else { None })
                    .expect("native owner");
                client.account_request(jig_client::Event::Send {
                    key: jig_client::Key { asker: jig_client::Asker(78), number: 99 },
                    name: Some([99; 16]),
                    request: jig_client::Ask::Jig(Box::new(jig_client::contract::Ask::Cancel {
                        project: 1,
                        task: assignment.task(),
                        reason: jig_client::contract::Text::new(b"Stop".as_slice().into(), 512)
                            .expect("bounded reason"),
                        grace: None,
                    })),
                });
                self.cancel = Cancel::Sent;
            }
        }
        for proc in procs {
            match proc {
                Proc::Engine(host) => {
                    world::answer_store(host, &mut self.store, &mut self.rules, false, Story::Chat, now);
                    let engine = host.service.engine().expect("engine");
                    world::answer_parties(engine, &mut self.party, &mut self.views);
                    if !self.period && self.party.sign_in.is_some() {
                        engine.parties_in().push(core::Event::Period {
                            project: 1,
                            period: 1,
                            budget: 100,
                            next: None,
                        });
                        self.period = true;
                    }
                    if self.done && !host.stopping {
                        host.stopping = true;
                        host.service.shutdown();
                    }
                }
                Proc::Worker(host) => {
                    self.outcome.borrow_mut().admission_waits =
                        host.service.worker(0).expect("worker").admission_waits();
                    if self.done && !host.stopping {
                        host.stopping = true;
                        host.service.shutdown();
                    }
                }
                Proc::Peer(peer) if self.done => peer.shutdown(),
                Proc::Peer(_) | Proc::Agent { .. } | Proc::Client(_) | Proc::Auth(_) | Proc::Probe(_) => {}
            }
        }
    }
    fn observe(&mut self, _now: Time, procs: &[Proc]) {
        let worker = world::worker_wire(procs);
        let mut outcome = self.outcome.borrow_mut();
        outcome.records = worker.frames.iter().map(|(_, kind, _)| *kind).collect();
        outcome.turns = worker
            .frames
            .iter()
            .filter(|(_, kind, _)| *kind == wire::Kind::Turn.number())
            .map(|(_, _, bytes)| wire::Turn::decode(&wire::CEILINGS, &mut Reader::new(bytes)).expect("turn"))
            .collect();
        outcome.answers = worker
            .frames
            .iter()
            .filter(|(_, kind, _)| *kind == wire::Kind::Answer.number())
            .map(|(_, _, bytes)| wire::Answer::decode(&wire::CEILINGS, &mut Reader::new(bytes)).expect("answer"))
            .collect();
        outcome.acknowledgements = worker.acknowledgements.len();
        outcome.starts = procs.iter().filter(|proc| matches!(proc, Proc::Agent { .. })).count();
        if worker.frames.iter().any(|(_, kind, _)| *kind == wire::Kind::Cancel.number()) {
            let before = *self.starts_at_cancel.get_or_insert(outcome.starts);
            outcome.starts_after_cancel = outcome.starts.saturating_sub(before);
        }
        self.done = outcome.acknowledgements == 1;
    }
    fn next_deadline(&self) -> Option<Time> {
        None
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (now > Time::ZERO.saturating_add(Duration::from_secs(45)))
            .then(|| format!("worker order settles: {:?}; {}", self.outcome.borrow(), world::diagnostics(&[])))
    }
    fn passed(&self) -> bool {
        self.done
    }
    fn last_word(&self) -> skein_world::LastWord {
        skein_world::LastWord::LastExpectation
    }
}

// One admitted io request holds the next waiting record until dispatch drains it.
const WATERMARK: u32 = 1;

fn worker(root: skein_io::kernel::Fd) -> Proc {
    let mut proc = processes::worker(root);
    let Proc::Worker(host) = &mut proc else { unreachable!("worker factory") };
    host.service.worker(0).expect("worker").set_watermark(WATERMARK);
    proc
}

/// Run the original order stories with a one-entry admission watermark.
#[must_use]
pub fn run(cancel: bool) -> Outcome {
    let mut script = fixtures::one_chat(b"Hello");
    if cancel {
        script.latency = Duration::from_secs(1);
    }
    let outcome = Rc::new(RefCell::new(Outcome::default()));
    let judge = Judge {
        accounts: account_setup::Setup::default(),
        party: jig_fake_parties::Party::new(1, jig_fake_parties::Role::Owner, 100, fixtures::party_script(&script)),
        store: Store::default(),
        rules: Rules::default(),
        views: Views::default(),
        period: false,
        cancel: if cancel { Cancel::Ready } else { Cancel::None },
        done: false,
        outcome: outcome.clone(),
        starts_at_cancel: None,
    };
    let mut machine = skein_fake_machine::Machine::new();
    let _state = machine.lay(&[]);
    let worker_root = machine.lay(&[]);
    let client_root = machine.lay(&[]);
    let mut harness =
        skein_world::World::new(fixtures::SEED, skein_sim::Config::calm(), judge, skein_world::Memory::Unchecked)
            .with_machine(processes::Machine(machine));
    harness.host(skein_world::HostedProgram {
        program: b"agent-program".as_slice().into(),
        make: processes::agent,
        instances: 1,
        operations: smith_agent_process_world::limits().routes.checked_add(2).expect("agent operations"),
    });
    harness.spawn_restartable(
        vec![skein_world::StartupRoot { name: b"state".as_slice().into(), path: b"state".as_slice().into() }],
        processes::engine,
    );
    harness.spawn_root(skein_sim::Handle::new(worker_root.raw()), worker);
    harness.spawn_root(skein_sim::Handle::new(client_root.raw()), |root| {
        let mut client = jig_client_protocol_world::native::NativeClient::new(
            jig_client_protocol_world::native::settings(),
            root,
            1000,
            fixtures::SEED,
            jig_client_protocol_world::native::Script::account_sign_in(),
        );
        client.hold_for_account_sign_in();
        Proc::Client(Box::new(client))
    });
    harness.spawn(|| {
        Proc::Peer(Box::new(fixtures::provider(
            Box::new([skein_fake_llm_domain::api::Script {
                cue: Box::default(),
                turns: fixtures::provider_turns(&script),
            }]),
            skein_llm::Credential::bearer(fixtures::TOKEN.into()),
            script.latency,
            skein_fake_peers::Transport::Plaintext,
            skein_fake_llm_protocol::documents::Provider::OpenAi,
        )))
    });
    let settled = harness.run();
    crate::referee::roots_at_end(&settled.procs);
    Rc::try_unwrap(outcome).expect("judge released").into_inner()
}
