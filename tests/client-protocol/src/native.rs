//! The test application's native client over simulated or real io.
use jig_client_shell_native::{Settings, Shell};
use jig_client_test_domain::{self as client, Request};
use jig_client_view::address::{Address, TaskSection};
use jig_client_world::{App, Test};
use skein_fake_person::{Doing, Find, Next, Person, Step, TreeFace};
use skein_io::kernel::{Complete, Fd, Submit};
use skein_lib::{Duration, Env, Queue, Time, Wall};
use skein_shell::Host;
/// The fake person's finite script, shared by simulated and real worlds.
pub struct Script {
    steps: Box<[Step<client::Address>]>,
}
impl Script {
    /// Empty UI script for a referee-controlled LLM account sign-in story.
    #[must_use]
    pub fn account_sign_in() -> Self {
        Self { steps: Box::new([]) }
    }
    /// Start a chat and keep reading until its answer is visible.
    #[must_use]
    pub fn answered_chat(words: &[u8], answer: &[u8]) -> Self {
        let mut steps = Self::start_chat(words).steps.into_vec();
        steps.push(Step::See {
            find: Find::named(skein_ui::Role::Heading { level: 1 }, b"Task 1"),
            within: Duration::from_secs(5),
        });
        steps.push(Step::Go {
            address: client::Address::Jig(Address::Task { task: 1, section: TaskSection::Activity }),
        });
        steps.push(Step::See { find: Find::text(answer), within: Duration::from_secs(5) });
        Self { steps: steps.into_boxed_slice() }
    }
    /// Read this fixture's answer card, excluding its request and instructions.
    #[must_use]
    pub fn answered_report(words: &[u8], answer: &[u8], within: Duration) -> Self {
        let mut steps = Self::start_chat(words).steps.into_vec();
        steps.push(Step::See {
            find: Find::named(skein_ui::Role::Paragraph, answer).within(Box::new([
                skein_fake_person::Target::ExactRole {
                    role: skein_ui::Role::Region,
                    name: b"Historical object".as_slice().into(),
                },
                skein_fake_person::Target::ExactRole {
                    role: skein_ui::Role::Region,
                    name: b"Project: 1".as_slice().into(),
                },
            ])),
            within,
        });
        Self { steps: steps.into_boxed_slice() }
    }
    /// Open the durable conversation and read its answer after a restart.
    #[must_use]
    pub fn found_chat(answer: &[u8]) -> Self {
        Self {
            steps: Box::new([
                Step::Go { address: client::Address::Jig(Address::Task { task: 1, section: TaskSection::Activity }) },
                Step::See { find: Find::text(answer), within: Duration::from_secs(5) },
            ]),
        }
    }
    #[must_use]
    pub fn start_chat(words: &[u8]) -> Self {
        Self {
            steps: vec![
                Step::See {
                    find: Find::named(skein_ui::Role::TextArea, b"First words"),
                    within: Duration::from_secs(1),
                },
                Step::Type { find: Find::named(skein_ui::Role::TextArea, b"First words"), words: words.into() },
                Step::Press { find: Find::named(skein_ui::Role::Button, b"Start a conversation") },
            ]
            .into_boxed_slice(),
        }
    }
}
/// Decoded facts retained outside the client domain for protocol referees.
#[derive(Clone, Debug)]
pub enum Received {
    SignedIn { person: u64 },
    Sent { attempt: skein_lib::Token, key: skein_lib::RequestKey },
    Answer { attempt: skein_lib::Token, answer: jig_client_domain::contract::Answer },
    Delivery(jig_client_domain::contract::Delivery),
    Read(jig_client_domain::contract::ReadResult),
}
/// A native client as a process of a skein world.
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent native lifecycle and externally driven account sign-in script"
)]
pub struct NativeClient {
    shell: Shell,
    wire: wire::Wire,
    credential: Option<skein_lib::Secret>,
    terminal: Option<skein_ui_text::Terminal>,
    #[expect(clippy::disallowed_types, reason = "scenario input hands inherited pipes to the host between iterations")]
    pipes: core::cell::Cell<Option<(Fd, Fd)>>,
    complete: Queue<Complete>,
    root: client::Domain,
    view: jig_client_test_view::View<'static>,
    person: Person<client::Address>,
    env: Env<client::Limits>,
    out: Queue<Request>,
    patches: Queue<skein_ui::Patch>,
    address: client::Address,
    started: bool,
    person_done: bool,
    hold_after_script: bool,
    account_sign_in_controlled: bool,
    wake: Option<Time>,
    closing: bool,
    pub received: Vec<Received>,
    pub signed_in: u32,
    pub chats_made: u32,
}
impl NativeClient {
    /// The provisional text retained by the client's run watch.
    pub fn text(&self, handle: jig_client::Handle) -> Option<&jig_client::contract::RunText> {
        self.root.client().text(handle)
    }
    #[must_use]
    pub fn new(settings: Settings, root: Fd, user: u32, seed: u64, script: Script) -> Self {
        let mut shell = Shell::new(settings);
        shell.adopt(root, user, seed);
        let mut limits = jig_client_world::test::limits();
        limits.client.domain.text = 512;
        limits.client.domain.turn_text = 512;
        limits.client.domain.turn_bytes = 512;
        let (root, view) = jig_client_world::test::linked_with(&limits, seed);
        let patches = Queue::with_capacity(Test::patch_limit(&view));
        let complete = Queue::with_capacity(shell.operations() + 10);
        Self {
            terminal: None,
            pipes: None.into(),
            complete,
            shell,
            wire: wire::Wire::default(),
            credential: None,
            root,
            view,
            person: Person::new(script.steps),
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            out: Queue::uncapped(),
            patches,
            address: jig_client_world::stories::conversation(),
            started: false,
            person_done: false,
            hold_after_script: false,
            account_sign_in_controlled: false,
            wake: None,
            closing: false,
            received: Vec::new(),
            signed_in: 0,
            chats_made: 0,
        }
    }
    /// Keep the native client open while a process referee drives LLM account sign-in asks.
    pub fn hold_for_account_sign_in(&mut self) {
        self.account_sign_in_controlled = true;
    }
    /// The own `me` watch confirmed this account sign-in client's party.
    #[must_use]
    pub fn account_sign_in_ready(&self) -> bool {
        self.root.client().link().state == jig_client::Link::Live
    }
    /// Supply an LLM account sign-in ask through the real client's request table and link.
    pub fn ask_account_sign_in(&mut self, key: jig_client::Key, ask: jig_client::contract::Ask) {
        self.callback(client::Event::Client {
            event: jig_client::Event::Send { key, name: None, request: jig_client::Ask::Jig(Box::new(ask)) },
        });
        self.wake = Some(self.env.now);
    }
    /// Drive an account request constructor through the real client's value envelope.
    pub fn account_request(&mut self, event: jig_client::Event) {
        self.callback(client::Event::Client { event });
        self.wake = Some(self.env.now);
    }
    /// Stop presentation watches while a referee drives requests directly.
    pub fn idle_fixture_view(&mut self) {
        self.callback(client::Event::Jig(jig_client_view::Event::Left));
    }
    /// Resume its ordinary UI script after fixture account setup.
    pub fn resume_after_account_setup(&mut self) {
        self.account_sign_in_controlled = false;
        self.wake = Some(self.env.now);
    }
    /// Keep the answered script's client open for the referee's account sign-out.
    pub fn hold_after_script(&mut self) {
        self.hold_after_script = true;
    }
    #[must_use]
    pub fn script_finished(&self) -> bool {
        self.person_done
    }
    /// Drain the native client after the account sign-in referee saw its answers.
    pub fn finish_account_sign_in(&mut self) {
        self.hold_after_script = false;
        self.account_sign_in_controlled = false;
        self.person_done = true;
        self.wake = Some(self.env.now);
    }
    /// # Errors
    /// Refuses a link outside the configured loopback engine's address.
    pub fn sign_in(&mut self, printed: &[u8]) -> Result<(), jig_client_protocol_http::Unlinked> {
        let secret = self.shell.sign_in(printed)?;
        self.callback(client::Event::Client {
            event: jig_client::Event::SignIn { with: jig_client::With::Link { secret, held: None } },
        });
        Ok(())
    }
    fn callback(&mut self, event: client::Event) {
        if let client::Event::Client { event } = &event {
            let received = self.wire.received(event);
            for fact in &received {
                match fact {
                    Received::SignedIn { .. } => self.signed_in += 1,
                    Received::Answer {
                        answer:
                            jig_client::contract::Answer::Done(jig_client::contract::Outcome::Owner(
                                jig_client::contract::OwnerOutcome::Created { .. },
                            )),
                        ..
                    } => self.chats_made += 1,
                    Received::Sent { .. } | Received::Answer { .. } | Received::Delivery(_) | Received::Read(_) => {}
                }
            }
            self.received.extend(received);
        }
        Test::step(&mut self.root, &self.env, event, &mut self.out);
        let credential = self.root.client().link().credential.cloned();
        if credential != self.credential {
            self.shell.keep(credential.clone());
            self.credential = credential;
        }
    }
    fn outputs(&mut self) {
        while let Some(request) = self.out.pop() {
            match request {
                Request::Save { record } => {
                    self.shell.down(jig_client::Request::Save {
                        records: jig_client_world::test::list([crate::front::persistence::encode_one(record)]),
                    });
                }
                Request::Erase { key } => {
                    self.shell.down(jig_client::Request::Erase {
                        records: jig_client_world::test::list([crate::front::persistence::erased(key)]),
                    });
                }
                Request::Show { address, .. } => {
                    if self.address != address {
                        self.address = address;
                        self.callback(Test::went(address));
                    }
                }
                Request::SignOut => self.callback(client::Event::Client { event: jig_client::Event::SignOut }),
                Request::SignIn { .. } => panic!("printed link supplied by caller"),
                Request::Client { request } => {
                    self.received.extend(self.wire.sent(&request));
                    self.shell.down(request);
                }
                Request::First(_) | Request::Fact { .. } => {}
            }
        }
    }
    fn text_step(&mut self, now: Time, changed: bool) {
        if let Some(mut terminal) = self.terminal.take() {
            terminal.iterate(now, Test::tree(&self.view), changed);
            while let Some(command) = terminal.commands().pop() {
                match command {
                    skein_ui_text::Command::Act { node, first, submit } => {
                        let event = Test::act(&mut self.view, node, first).expect("terminal act");
                        self.callback(event);
                        if submit {
                            let event =
                                Test::act(&mut self.view, node, skein_ui::Act::Submit).expect("terminal submit");
                            self.callback(event);
                        }
                    }
                    skein_ui_text::Command::Print
                    | skein_ui_text::Command::Close
                    | skein_ui_text::Command::Nothing
                    | skein_ui_text::Command::Refused { .. } => unreachable!("terminal queues acts only"),
                }
            }
            while let Some(submit) = terminal.submissions().pop() {
                self.shell.submissions().push(submit);
            }
            self.person_done = terminal.closed();
            self.terminal = Some(terminal);
        }
    }
}
impl Host for NativeClient {
    fn iterate(&mut self, now: Time, wall: Wall) {
        self.env.now = now;
        self.env.wall = wall;
        let mut changed = false;
        if let Some((input, output)) = self.pipes.take() {
            self.terminal.as_mut().expect("terminal supplied").adopt(input, output).expect("pipe room");
        }
        while let Some(complete) = self.complete.pop() {
            if skein_ui_text::Terminal::owns(complete.op) {
                self.terminal.as_mut().expect("terminal operation").completions().push(complete);
            } else {
                self.shell.completions().push(complete);
            }
        }
        self.shell.iterate(now, wall);
        if let Some((credential, kept)) = self.shell.loaded() {
            let mut client_records = Vec::new();
            for record in kept {
                if record.key.len() == 16 || record.key.as_ref() == jig_client::STREAM_KEY {
                    client_records.push(client::store::Record::Client(record));
                } else if let Some(record) = crate::front::persistence::child_record(&record) {
                    client_records.push(record);
                }
            }
            self.started = true;
            if let Some(credential) = credential {
                self.callback(client::Event::Client {
                    event: jig_client::Event::SignIn { with: jig_client::With::Held(credential) },
                });
            }
            self.callback(Test::start(self.address, client_records, 0));
        }
        if let Some(ok) = self.shell.kept() {
            assert!(ok, "native credential is durable");
        }
        while let Some(event) = self.shell.events().pop() {
            self.callback(client::Event::Client { event });
        }
        self.outputs();
        if !self.closing && self.started {
            if self.account_sign_in_controlled {
                self.wake = None;
            }
            Test::fire(&mut self.root, &self.env, &mut self.out);
            Test::take(&mut self.root, &self.env, &mut self.out);
            self.outputs();
            if !self.account_sign_in_controlled {
                Test::build(&mut self.view, &self.root, &mut self.patches);
                changed = !self.patches.is_empty();
                while self.patches.pop().is_some() {}
            }
            if self.terminal.is_none() && !self.account_sign_in_controlled {
                self.wake = None;
                let before = self.person.at();
                match self.person.next(&mut TreeFace::new(Test::tree(&self.view)), now) {
                    Next::Do(Doing::Act { node, act }) => {
                        let event = Test::act(&mut self.view, node, act).expect("offered person action");
                        self.callback(event);
                    }
                    Next::Do(Doing::Go { address }) => {
                        self.address = address;
                        self.callback(Test::went(address));
                    }
                    Next::Do(Doing::Reload) => panic!("this native script has no reload"),
                    Next::Wait { until } => self.wake = until,
                    Next::Done => self.person_done = true,
                    Next::Failed { step, why } => panic!("native person step {step}: {why:?}"),
                }
                if self.person.at() != before {
                    self.wake = Some(now);
                }
            }
            self.outputs();
        }
        self.text_step(now, changed);
        if !self.closing && !self.hold_after_script && self.person_done && self.root.jig().chats().starting.is_none() {
            self.closing = true;
            self.shell.close();
        }
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        &mut self.complete
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        self.shell.submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        self.shell.work_pending(now)
            || !self.complete.is_empty()
            || self.pipes.get().is_some()
            || self.terminal.as_ref().is_some_and(|terminal| terminal.work_pending(now))
            || !self.out.is_empty()
            || (!self.closing && self.started && self.wake.is_some_and(|at| at <= now))
    }
    fn next_deadline(&self) -> Option<Time> {
        [
            self.shell.next_deadline(),
            self.next_policy_deadline(),
            self.terminal.as_ref().and_then(skein_ui_text::Terminal::next_deadline),
        ]
        .into_iter()
        .flatten()
        .min()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        if self.closing {
            None
        } else {
            let root = if self.account_sign_in_controlled {
                self.root.client().next_deadline()
            } else {
                Test::next_deadline(&self.root)
            };
            [root, self.wake].into_iter().flatten().min()
        }
    }
    fn is_empty(&self) -> bool {
        self.closing && self.shell.is_empty() && self.terminal.as_ref().is_none_or(skein_ui_text::Terminal::closed)
    }
    fn operations(&self) -> u32 {
        self.shell.operations() + if self.terminal.is_some() { 10 } else { 0 }
    }
}

pub mod wire;
mod world;
pub use world::{Outcome, run, run_terminal, settings};
