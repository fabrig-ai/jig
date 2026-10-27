//! A single Service owns both layers; the world owns its external parties and machine.
use crate::world_signals::SignalHost;
use crate::{
    Script, fixtures,
    referee::{Referee as Judge, Scenario},
};
use jig_channel_world::{
    fixtures as channel,
    world::store::{Capture, Store},
};
use jig_client_protocol_world::native::NativeClient;
use skein_io::kernel::{Complete, Submit};
use skein_lib::{List, Queue, Time, Wall};
use skein_shell::Host;
use skein_world::{Inherited, Memory};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub seed: u64,
    pub config: skein_sim::Config,
    pub chat: fixtures::Chat,
}
#[derive(Debug)]
pub struct Outcome {
    pub verdict: skein_world::domain::Verdict,
    pub chat: Option<crate::referee::ChatOutcome>,
    pub chats_made: u32,
    pub runs_answered: u32,
    pub drained: bool,
    pub restarts: u32,
    pub claims_settled_at_restart: u32,
    pub mark_gate_checked: bool,
}
pub(super) struct ServiceHost {
    pub(super) service: jig_test_service::Service,
    capture: Capture,
    pub(super) store: Rc<RefCell<Store>>,
    pub(super) attached: bool,
    pub(super) wire: crate::referee::agent::Wire,
    pub(super) commits: Vec<jig_channel_world::world::store::Commit>,
    pub(super) hold_mark: bool,
    pub(super) held_mark: Vec<Complete>,
    pub(super) mark_gate_checked: bool,
    pub(super) early_connections: u32,
    codec: jig_protocol_store::codec::Limits,
    connector: jig_test_connector::Limits,
}
impl Host for ServiceHost {
    fn iterate(&mut self, now: Time, wall: Wall) {
        if !self.hold_mark {
            for complete in self.held_mark.drain(..) {
                self.service.completions().push(complete);
            }
        } else if self.service.account_startup(0, 1, now).is_some_and(|state| state.pending_mark.is_some()) {
            for _ in 0..self.service.completions().len() {
                let complete = self.service.completions().pop().expect("completion count");
                if matches!(complete.kind, skein_io::kernel::Op::Sync { .. }) {
                    self.held_mark.push(complete);
                } else {
                    self.service.completions().push(complete);
                }
            }
        }
        for complete in self.service.completions().iter() {
            self.capture.complete(complete);
            self.wire.complete(complete);
        }
        let commits = self.store.borrow_mut().observe(
            &mut self.capture.events,
            &self.codec,
            crate::fixtures::store_names(),
            &self.connector,
        );
        self.commits.extend(commits);
        jig_test_service::iterate(&mut self.service, now, wall);
        for submit in self.service.submissions().iter() {
            self.capture.pending(&submit.kind);
            if self.hold_mark && matches!(submit.kind, skein_io::kernel::Op::Connect { .. }) {
                self.early_connections += 1;
            }
        }
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        self.service.completions()
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        self.service.submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        self.service.work_pending(now)
    }
    fn next_deadline(&self) -> Option<Time> {
        self.service.next_deadline()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.service.next_policy_deadline()
    }
    fn is_empty(&self) -> bool {
        self.service.is_empty()
    }
    fn worst_case(&self) -> u64 {
        self.service.worst_case() + 8 * 1024 * 1024
    }
    fn operations(&self) -> u32 {
        self.service.operations()
    }
}
pub(super) enum Proc {
    Service(Box<ServiceHost>),
    Client(Box<SignalHost<NativeClient>>),
    Agent { host: Box<skein_world::PipeHost<smith_agent_shell::Agent>> },
    Peer(Box<SignalHost<skein_fake_peers::llm::Peer>>),
}
impl Proc {
    fn host(&self) -> &dyn Host {
        match self {
            Self::Service(host) => host.as_ref(),
            Self::Client(host) => host.as_ref(),
            Self::Agent { host, .. } => host.as_ref(),
            Self::Peer(host) => host.as_ref(),
        }
    }
    fn host_mut(&mut self) -> &mut dyn Host {
        match self {
            Self::Service(host) => host.as_mut(),
            Self::Client(host) => host.as_mut(),
            Self::Agent { host, .. } => host.as_mut(),
            Self::Peer(host) => host.as_mut(),
        }
    }
}
impl Host for Proc {
    fn drain(&mut self) {
        self.host_mut().drain();
    }
    fn iterate(&mut self, now: Time, wall: Wall) {
        self.host_mut().iterate(now, wall);
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        self.host_mut().completions()
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        self.host_mut().submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        self.host().work_pending(now)
    }
    fn next_deadline(&self) -> Option<Time> {
        self.host().next_deadline()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.host().next_policy_deadline()
    }
    fn is_empty(&self) -> bool {
        self.host().is_empty()
    }
    fn worst_case(&self) -> u64 {
        self.host().worst_case()
    }
    fn operations(&self) -> u32 {
        self.host().operations()
    }
}
fn service(inherited: &Inherited) -> Proc {
    service_chat(inherited, fixtures::Chat::Finite)
}
fn open_service(inherited: &Inherited) -> Proc {
    service_chat(inherited, fixtures::Chat::OpenText)
}
fn service_chat(inherited: &Inherited, chat: fixtures::Chat) -> Proc {
    let root = inherited.roots.iter().find(|(name, _)| name.as_ref() == b"state").expect("store root").1;
    let worker = inherited.roots.iter().find(|(name, _)| name.as_ref() == b"worker").expect("worker root").1;
    let mut roots = List::with_capacity(1);
    roots.push(worker).expect("worker root");
    let mut service = jig_test_service::startup(
        &fixtures::document(chat),
        &fixtures::files(),
        Some(&channel::agent()),
        &channel::seeds(),
        None,
        Some(jig_test_service::WorkerRuntime { roots }),
    )
    .expect("one-process startup");
    service.adopt_store(root, 1000).expect("actual deployment store");
    service.adopt_signals(inherited.signal).expect("service signals");
    let mut capture = Capture::default();
    capture.root(root);
    let limits = fixtures::store_limits();
    let codec = jig_protocol_store::codec::limits(&limits.root.core).expect("admitted fixture codecs");
    Proc::Service(Box::new(ServiceHost {
        service,
        capture,
        store: Rc::new(RefCell::new(Store::new())),
        attached: false,
        wire: crate::referee::agent::Wire::default(),
        commits: Vec::new(),
        hold_mark: false,
        held_mark: Vec::new(),
        mark_gate_checked: false,
        early_connections: 0,
        codec,
        connector: limits.root.connector,
    }))
}
fn agent(_spawn: &skein_io::kernel::Spawn, inherited: &Inherited) -> Proc {
    let stderr = inherited.pipes.iter().find(|(child, _)| *child == 2).expect("agent stderr").1;
    let (errors, capture) = smith_agent_process_world::AgentErrors::pipe(stderr);
    let host = smith_agent_process_world::configured(channel::agent_configuration(), fixtures::SEED, inherited, errors);
    Proc::Agent { host: Box::new(capture.host(host)) }
}
struct Machine(skein_fake_machine::Machine);
impl skein_world::Machine for Machine {
    fn cut(&mut self, cut: skein_world::Cut, held: &[skein_sim::Handle], seed: u64) {
        match cut {
            skein_world::Cut::Kill => {
                for handle in held {
                    self.0.close(skein_fake_machine::Opened::new(handle.raw()));
                }
            }
            skein_world::Cut::PowerLoss => self.0.crash(seed),
        }
    }
    fn open_root(&mut self, path: &[u8]) -> Result<skein_sim::Handle, skein_io::kernel::Error> {
        let index = match path {
            b"state" => 0,
            b"agent-root" => 1,
            b"client" => 2,
            _ => return Err(skein_io::kernel::Error::NotFound),
        };
        Ok(skein_sim::Handle::new(self.0.reopen_root(index).raw()))
    }
    fn close_root(&mut self, root: skein_sim::Handle) {
        self.0.close(skein_fake_machine::Opened::new(root.raw()));
    }
    fn step(&mut self, call: skein_sim::Call, answers: &mut Queue<skein_sim::Answer>) {
        skein_fake_machine::step(&mut self.0, call, answers);
    }
}
#[must_use]
pub fn run(settings: Settings, people: Box<[Script]>) -> Outcome {
    run_phase(settings, people, machine(), Store::new(), crate::referee::conformance(), Scenario::Answer).0
}
#[must_use]
pub fn drain_mid_run(settings: Settings, restart: bool) -> Outcome {
    let (mut outcome, machine, store, reference) = run_phase(
        settings,
        Box::new([Script::start_chat(b"Hello")]),
        machine(),
        Store::new(),
        crate::referee::conformance(),
        Scenario::MidRun,
    );
    if restart {
        let (restarted, _, _, _) = run_phase(settings, Box::new([]), machine, store, reference, Scenario::Restart);
        outcome.claims_settled_at_restart = restarted.claims_settled_at_restart;
        outcome.drained &= restarted.drained;
    }
    outcome
}
#[must_use]
pub fn crash(settings: Settings, cut: skein_world::Cut, before: u32) -> Outcome {
    run_phase(
        settings,
        Box::new([]),
        machine(),
        Store::new(),
        crate::referee::conformance(),
        Scenario::Fault { cut, before },
    )
    .0
}
/// Restore a real saved source with the service's mark acknowledgement held.
#[must_use]
pub fn accounts_mark_gate(settings: Settings) -> Outcome {
    let (_, machine, store, reference) = run_phase(
        settings,
        fixtures::one_person_chat(b"Hello"),
        machine(),
        Store::new(),
        crate::referee::conformance(),
        Scenario::Answer,
    );
    run_phase(settings, Box::new([]), machine, store, reference, Scenario::MarkGate).0
}

fn client(inherited: &Inherited) -> Proc {
    let root = inherited.roots.first().expect("client root").1;
    let mut client =
        NativeClient::new(fixtures::client_settings(), root, 1000, fixtures::SEED, Script::start_chat(b"Hello"));
    client.hold_for_account_sign_in();
    Proc::Client(Box::new(SignalHost::new(client, Some(inherited.signal))))
}
fn peer(inherited: &Inherited) -> Proc {
    Proc::Peer(Box::new(SignalHost::new(fixtures::provider(fixtures::Chat::Finite), Some(inherited.signal))))
}
fn machine() -> skein_fake_machine::Machine {
    let mut machine = skein_fake_machine::Machine::for_user(1000);
    for _ in 0..3 {
        let _root = machine.lay(&[]);
    }
    machine
}
fn run_phase(
    settings: Settings,
    people: Box<[Script]>,
    mut machine: skein_fake_machine::Machine,
    durable_observer: Store,
    reference: Rc<RefCell<jig_conformance::referee::Referee>>,
    scenario: Scenario,
) -> (Outcome, skein_fake_machine::Machine, Store, Rc<RefCell<jig_conformance::referee::Referee>>) {
    let client_root = machine.reopen_root(2);
    let restarts = Rc::new(RefCell::new(0));
    let chat_outcome = Rc::new(RefCell::new(None));
    let mut world = skein_world::World::new_controlled(settings.seed, settings.config, Memory::Unchecked, |controls| {
        Judge::new(
            scenario,
            controls,
            durable_observer,
            reference.clone(),
            restarts.clone(),
            settings.chat,
            chat_outcome.clone(),
        )
    })
    .with_machine(Machine(machine));
    let agent_limits = smith_agent_process_world::limits();
    // smith's agent_shell.rs extra_operations reserves one hosted open per permitted root.
    let root_opens = agent_limits.domain.run.directories;
    world.host(skein_world::HostedProgram {
        program: b"agent-program".as_slice().into(),
        make: agent,
        instances: 1,
        operations: agent_limits
            .routes
            .checked_add(2)
            .and_then(|count| count.checked_add(root_opens))
            .expect("agent operations and hosted root opens"),
    });
    world.spawn_restartable(
        vec![
            skein_world::StartupRoot { name: b"state".as_slice().into(), path: b"state".as_slice().into() },
            skein_world::StartupRoot { name: b"worker".as_slice().into(), path: b"agent-root".as_slice().into() },
        ],
        if settings.chat == fixtures::Chat::OpenText { open_service } else { service },
    );
    if let Scenario::Fault { cut, before } = scenario {
        world.spawn_restartable(
            vec![skein_world::StartupRoot { name: b"private".as_slice().into(), path: b"client".as_slice().into() }],
            client,
        );
        world.spawn_restartable(vec![], peer);
        world.cut(0, before, cut);
    } else {
        for script in people {
            world.spawn_root(skein_sim::Handle::new(client_root.raw()), |root| {
                let mut client = NativeClient::new(fixtures::client_settings(), root, 1000, settings.seed, script);
                client.hold_for_account_sign_in();
                Proc::Client(Box::new(SignalHost::new(client, None)))
            });
        }
        world.spawn(|| Proc::Peer(Box::new(SignalHost::new(fixtures::provider(settings.chat), None))));
    }
    let mut outcome = world.run();
    let chats_made = outcome
        .procs
        .iter()
        .filter_map(|proc| match proc {
            Proc::Client(client) => Some(client.chats_made),
            Proc::Service(_) | Proc::Agent { .. } | Proc::Peer(_) => None,
        })
        .sum();
    let result = Outcome {
        verdict: skein_world::domain::Verdict::Passed,
        chat: chat_outcome.borrow().clone(),
        mark_gate_checked: outcome.procs.iter().any(|proc| match proc {
            Proc::Service(service) => service.mark_gate_checked,
            Proc::Client(_) | Proc::Agent { .. } | Proc::Peer(_) => false,
        }),
        chats_made,
        restarts: *restarts.borrow(),
        runs_answered: outcome
            .procs
            .iter()
            .find_map(|proc| match proc {
                Proc::Service(service) => Some(crate::referee::terminals(&service.store.borrow())),
                Proc::Client(_) | Proc::Agent { .. } | Proc::Peer(_) => None,
            })
            .expect("one service"),
        drained: outcome.procs.iter().all(Host::is_empty)
            && outcome.procs.iter().all(|proc| match proc {
                Proc::Service(service) => service.store.borrow().pending.is_empty(),
                Proc::Client(_) | Proc::Agent { .. } | Proc::Peer(_) => true,
            }),
        claims_settled_at_restart: outcome
            .procs
            .iter()
            .find_map(|proc| match proc {
                Proc::Service(service) => Some(crate::referee::settled(&service.store.borrow())),
                Proc::Client(_) | Proc::Agent { .. } | Proc::Peer(_) => None,
            })
            .expect("one service"),
    };
    let store = outcome
        .procs
        .iter_mut()
        .find_map(|proc| match proc {
            Proc::Service(service) => Some(std::mem::take(&mut *service.store.borrow_mut())),
            Proc::Client(_) | Proc::Agent { .. } | Proc::Peer(_) => None,
        })
        .expect("one service observer");
    (result, std::mem::replace(&mut outcome.machine.0, skein_fake_machine::Machine::for_user(1000)), store, reference)
}
