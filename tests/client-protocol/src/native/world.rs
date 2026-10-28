//! One actual reference service and native client, joined by simulated sockets.
use super::{NativeClient, Script};
use crate::fixtures;
use jig_client_world::App;
use skein_io::kernel::{Complete, Submit};
use skein_lib::{Duration, Queue, Secret, Time, Wall};
use skein_shell::Host;
use skein_world::{Memory, Referee};
#[path = "config.rs"]
mod config;
/// What the person received through bytes, after both hosts drained.
#[derive(Debug)]
pub struct Outcome {
    pub signed_in: u32,
    pub chats_made: u32,
    pub drained: bool,
}
struct Service(jig_test_service::Service);
impl Host for Service {
    fn iterate(&mut self, now: Time, wall: Wall) {
        jig_test_service::iterate(&mut self.0, now, wall);
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        self.0.completions()
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        self.0.submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        self.0.work_pending(now)
    }
    fn next_deadline(&self) -> Option<Time> {
        self.0.next_deadline()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.0.next_policy_deadline()
    }
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    fn operations(&self) -> u32 {
        self.0.operations()
    }
}
enum Process {
    Service(Box<Service>),
    Client(Box<NativeClient>),
}
impl Process {
    fn host(&self) -> &dyn Host {
        match self {
            Self::Service(host) => host.as_ref(),
            Self::Client(host) => host.as_ref(),
        }
    }
    fn host_mut(&mut self) -> &mut dyn Host {
        match self {
            Self::Service(host) => host.as_mut(),
            Self::Client(host) => host.as_mut(),
        }
    }
}
impl Host for Process {
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
    fn operations(&self) -> u32 {
        self.host().operations()
    }
}
// The world plays the application calendar child (service.md, 7; ruling 275).
struct PeriodSource {
    supplied: bool,
}
impl PeriodSource {
    fn step(&mut self, service: &mut jig_test_service::Service, client: &NativeClient) {
        const PROJECT: u32 = 1;
        const FIRST_PERIOD: u64 = 1;
        const PERIOD_BUDGET: u64 = 100;
        if !self.supplied && client.signed_in > 0 {
            service.engine().expect("engine-only service").parties_in().push(jig_domain_core::Event::Period {
                project: PROJECT,
                period: FIRST_PERIOD,
                budget: PERIOD_BUDGET,
                next: None,
            });
            self.supplied = true;
        }
    }
}
struct Judge {
    period: PeriodSource,
    handed: bool,
    done: bool,
}
impl Referee<Process> for Judge {
    fn act(&mut self, _now: Time, procs: &mut [Process]) {
        let (service, client) = procs.split_at_mut(1);
        let Process::Service(service) = &mut service[0] else { panic!("service") };
        let Process::Client(client) = &mut client[0] else { panic!("client") };
        self.period.step(&mut service.0, client);
        if !self.handed
            && let Some(link) = service.0.links().pop()
        {
            client.sign_in(&link).expect("service's link");
            self.handed = true;
        }
        if !self.done && client.is_empty() {
            service.0.shutdown();
            self.done = true;
        }
    }
    fn observe(&mut self, _now: Time, _procs: &[Process]) {}
    fn next_deadline(&self) -> Option<Time> {
        if self.done { None } else { Some(Time::ZERO.saturating_add(Duration::from_secs(5))) }
    }
    fn overdue(&self, now: Time) -> Option<String> {
        if !self.done && now >= Time::ZERO.saturating_add(Duration::from_secs(5)) {
            Some("native chat did not drain".to_owned())
        } else {
            None
        }
    }
    fn passed(&self) -> bool {
        self.done
    }
}
struct Machine(skein_fake_machine::Machine);
impl skein_world::Machine for Machine {
    fn step(&mut self, call: skein_sim::Call, answers: &mut Queue<skein_sim::Answer>) {
        skein_fake_machine::step(&mut self.0, call, answers);
    }
}
fn files() -> jig_test_service::Files {
    jig_test_service::Files {
        entries: jig_client_world::test::list([
            (b"charters/chat.txt".as_slice().into(), Secret::new(b"Reply to the person".as_slice().into())),
            (b"app.secret".as_slice().into(), Secret::new(b"application-token".as_slice().into())),
            (
                b"accounts/chat.json".as_slice().into(),
                Secret::new(br#"{"access_token":"fake","expires_unix_seconds":4102444800}"#.as_slice().into()),
            ),
            (
                b"secrets/worker-a.digest".as_slice().into(),
                Secret::new(b"YWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWE".as_slice().into()),
            ),
            (
                b"secrets/worker-b.digest".as_slice().into(),
                Secret::new(b"YmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmI".as_slice().into()),
            ),
        ]),
    }
}
#[must_use]
pub fn settings() -> jig_client_shell_native::Settings {
    jig_client_shell_native::Settings {
        engine: skein_io::Addr::Ip((std::net::Ipv4Addr::LOCALHOST, 7100).into()),
        directory: b"private".as_slice().into(),
        limits: fixtures::connections_limits(),
        io: skein_io::Limits {
            sockets: 32,
            names: 1,
            refusals: 8,
            intake: 4096,
            receive: 1024,
            output: 4096,
            sends: 8,
            accepts: 4,
            backlog: 8,
            close_timeout: Duration::from_secs(5),
            retry: Duration::from_millis(1),
        },
    }
}
#[must_use]
pub fn run(seed: u64, script: Script) -> Outcome {
    run_mode(seed, script, false).0
}
/// Start the chat by the terminal's printed numbers, then close its input.
#[must_use]
pub fn run_terminal(seed: u64) -> (Outcome, Box<[u8]>) {
    run_mode(seed, Script::start_chat(b"Hello"), true)
}
fn run_mode(seed: u64, script: Script, terminal: bool) -> (Outcome, Box<[u8]>) {
    let mut machine = skein_fake_machine::Machine::for_user(1000);
    let engine_root = machine.lay(&[]);
    let client_root = machine.lay(&[]);
    let mut world = skein_world::World::new(
        seed,
        skein_sim::Config::calm(),
        Judge { period: PeriodSource { supplied: false }, handed: false, done: false },
        Memory::Unchecked,
    )
    .with_machine(Machine(machine));
    world.spawn_root(skein_sim::Handle::new(engine_root.raw()), |root| {
        let document = config::document();
        let seeds = jig_test_service::Seeds {
            deployment: [7; 16],
            installation: [9; 16],
            seed,
            crypto: skein_world::crypto_seed(seed),
        };
        let mut service =
            jig_test_service::startup(&document, &files(), None, &seeds, None, None).expect("engine configuration");
        service.adopt_store(root, 1000).expect("real store directory");
        Process::Service(Box::new(Service(service)))
    });
    world.spawn_root(skein_sim::Handle::new(client_root.raw()), |root| {
        let mut client = NativeClient::new(settings(), root, 1000, seed, script);
        if terminal {
            client.terminal = Some(
                skein_ui_text::Terminal::new(
                    skein_ui_text::Settings {
                        name: b"jig".as_slice().into(),
                        longest: 1024,
                        lines: 0,
                        line_bytes: 0,
                        interval: Duration::from_millis(1),
                        status: Some(jig_client_view::regions::JigRegion::Link.region()),
                    },
                    &super::Test::view_limits(&client.view),
                )
                .expect("terminal bounds"),
            );
        }
        Process::Client(Box::new(client))
    });
    let mut pipes = None;
    let mut pages = Vec::new();
    let mut sent = false;
    let mut ended = false;
    let outcome = world.run_with_inputs(|sim, pids, processes| {
        if !terminal {
            return;
        }
        let Process::Client(client) = &processes[1] else { panic!("native client") };
        let (input, output) = *pipes.get_or_insert_with(|| {
            let pair = (sim.open_inherited_read(pids[1]), sim.open_inherited_write(pids[1]));
            client.pipes.set(Some(pair));
            pair
        });
        pages.extend(sim.peer_drain(pids[1], output, 65536));
        if client.is_empty() {
            sim.assert_no_open_fds(pids[1]);
        }
        if !sent
            && let (Some(field), Some(button)) =
                (number(&pages, "First words:"), number(&pages, "Start a conversation"))
        {
            let line = format!("{field} Hello\n{button}\n");
            assert_eq!(sim.peer_feed(pids[1], input, line.as_bytes()), line.len());
            sent = true;
        }
        if !ended && client.chats_made == 1 && String::from_utf8_lossy(&pages).contains("Task 1") {
            sim.peer_close(pids[1], input);
            ended = true;
        }
    });
    let Process::Client(client) = &outcome.procs[1] else { panic!("native client") };
    (
        Outcome {
            signed_in: client.signed_in,
            chats_made: client.chats_made,
            drained: outcome.procs.iter().all(Host::is_empty),
        },
        pages.into_boxed_slice(),
    )
}
fn number(page: &[u8], label: &str) -> Option<u32> {
    for line in std::str::from_utf8(page).expect("terminal prints UTF-8").lines().rev() {
        let line = line.trim_start();
        if line.contains(label)
            && let Some(end) = line.find(']')
        {
            return line.strip_prefix('[')?.get(..end - 1)?.parse().ok();
        }
    }
    None
}
