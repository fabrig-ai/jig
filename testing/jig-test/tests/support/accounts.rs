//! LLM account sign-in against the shipped process and the existing independent issuer/browser.
use crate::support::{Person, Process, issuer, private, start};
use jig_client::{Asker, Key, contract as c};
use jig_client_protocol_world::native::Received;
use jig_client_protocol_world::native::{NativeClient, Script};
use skein_fake_peers::oauth;
use skein_io::kernel::{Exit, Signal};
use skein_lib::{Duration, Queue, Time};
use skein_shell::{Clock, Host};
use skein_world::{Referee, end_to_end::Streams, real};
use std::{fs, path::Path};

const ACCOUNT_SIGN_IN: Key = Key { asker: Asker(7), number: 1 };
const ACCOUNT_SIGN_OUT: Key = Key { asker: Asker(7), number: 2 };

pub fn configuration(directory: &Path) {
    crate::support::configuration(directory);
    let path = directory.join("config.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).expect("configuration")).expect("JSON");
    value["engine"]["llm"]["providers"] = serde_json::json!([{
        "name":"Fake", "signed_in_plan":true, "manage_usage":"https://usage.example/", "number":3, "models":["fake"], "sources":{"sign_in":{
            "issuer":"http://127.0.0.1:31000", "client":"client", "scopes":["openid","read"],
            "name_hint":"jig-test", "authorization":"http://127.0.0.1:31000/authorize",
            "token":"http://127.0.0.1:31000/token", "revocation":"http://127.0.0.1:31000/revoke",
            "keys":"http://127.0.0.1:31000/keys", "transport":"plaintext",
            "must_grant":["openid","read"], "resource":"api", "redirect_path":"callback"
        }}
    }]);
    private(&path, &serde_json::to_vec(&value).expect("configuration JSON"));
}
#[derive(Debug, Default)]
enum Phase {
    #[default]
    AccountSignIn,
    Address,
    AccountSignOut {
        source: u32,
    },
    Confirmed,
    Closing,
    Ended,
}
struct Judge {
    status: String,
    phase: Phase,
    deadline: Time,
}
fn client(procs: &mut [Process]) -> &mut NativeClient {
    let Process::Person(person) = &mut procs[1] else { panic!("native person") };
    person.client.as_mut().expect("native client")
}
fn answer(procs: &[Process]) -> Option<c::Answer> {
    let Process::Person(person) = &procs[1] else { panic!("native person") };
    person.client.as_ref()?.received.iter().rev().find_map(|event| match event {
        Received::Answer { answer, .. } => Some(answer.clone()),
        Received::SignedIn { .. } | Received::Sent { .. } | Received::Delivery(_) | Received::Read(_) => None,
    })
}
impl Referee<Process> for Judge {
    fn act(&mut self, _now: Time, procs: &mut [Process]) {
        match self.phase {
            Phase::AccountSignIn => {
                let Process::Person(person) = &mut procs[1] else { panic!("native person") };
                if let Some(client) = &mut person.client
                    && client.signed_in == 1
                    && client.account_sign_in_ready()
                {
                    client.ask_account_sign_in(
                        ACCOUNT_SIGN_IN,
                        c::Ask::SignIn { holder: c::AccountHolder::Own, provider: 3 },
                    );
                    self.phase = Phase::Address;
                }
            }
            Phase::Address => {
                if let Some(answer) = answer(procs) {
                    let c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::AccountSignInStarted { source, address })) =
                        answer
                    else {
                        panic!("account sign-in answer: {answer:?}")
                    };
                    assert!(!address.is_empty());
                    let Process::Visitor(visitor) = &mut procs[3] else { panic!("browser") };
                    visitor.hand(&address);
                    self.phase = Phase::AccountSignOut { source };
                }
            }
            Phase::AccountSignOut { source } => {
                let Process::Visitor(visitor) = &procs[3] else { panic!("browser") };
                let Process::Issuer(issuer) = &procs[2] else { panic!("issuer") };
                // The browser's callback finishes after the driver's exchange. Wait for its
                // sockets too, so sign-out cannot race the account sign-in exchange teardown.
                let token = issuer.observations().iter().find_map(|event| match event {
                    oauth::Observation::Post { connection } => Some(*connection),
                    oauth::Observation::Accepted { .. }
                    | oauth::Observation::Authorize { .. }
                    | oauth::Observation::Document { .. }
                    | oauth::Observation::Revoke { .. }
                    | oauth::Observation::Redirect { .. }
                    | oauth::Observation::Answered { .. }
                    | oauth::Observation::Closed { .. } => None,
                });
                if visitor.is_empty()
                    && token.is_some_and(|token| {
                        issuer.observations().iter().any(
                            |event| matches!(event, oauth::Observation::Closed { connection } if *connection == token),
                        )
                    })
                {
                    client(procs).ask_account_sign_in(
                        ACCOUNT_SIGN_OUT,
                        c::Ask::SignOut { holder: c::AccountHolder::Own, source },
                    );
                    self.phase = Phase::Confirmed;
                }
            }
            Phase::Confirmed => {
                if let Some(answer) = answer(procs)
                    && !matches!(
                        &answer,
                        c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::AccountSignInStarted { .. }))
                    )
                {
                    assert_eq!(
                        answer,
                        c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::AccountSignedOut {
                            confirmed: true,
                            failed_hosts: Box::new([])
                        }))
                    );
                    client(procs).finish_account_sign_in();
                    self.phase = Phase::Closing;
                }
            }
            Phase::Closing => {
                if client(procs).is_empty() {
                    let Process::Binary(binary) = &mut procs[0] else { panic!("binary") };
                    binary.signal(Signal::Kill);
                    let Process::Issuer(issuer) = &mut procs[2] else { panic!("issuer") };
                    issuer.shutdown();
                    self.phase = Phase::Ended;
                }
            }
            Phase::Ended => {}
        }
    }
    fn observe(&mut self, _now: Time, procs: &[Process]) {
        let Process::Visitor(visitor) = &procs[3] else { panic!("browser") };
        let Process::Issuer(issuer) = &procs[2] else { panic!("issuer") };
        let events: Vec<_> = issuer
            .observations()
            .iter()
            .map(|event| match event {
                oauth::Observation::Accepted { .. } => "accepted",
                oauth::Observation::Authorize { .. } => "authorize",
                oauth::Observation::Document { .. } => "document",
                oauth::Observation::Post { .. } => "post",
                oauth::Observation::Revoke { .. } => "revoke",
                oauth::Observation::Redirect { .. } => "redirect",
                oauth::Observation::Answered { .. } => "answered",
                oauth::Observation::Closed { .. } => "closed",
            })
            .collect();
        let Process::Person(person) = &procs[1] else { panic!("native person") };
        self.status = format!(
            "phase {:?}, signed in {:?}, browser handed {}, empty {}, status {:?}, issuer {:?}, answer {:?}",
            self.phase,
            person.client.as_ref().map(|c| c.signed_in),
            visitor.handed(),
            visitor.is_empty(),
            visitor.status(),
            events,
            answer(procs)
        );
        let Process::Binary(binary) = &procs[0] else { panic!("binary") };
        assert!(
            binary.exit_status().is_none() || matches!(self.phase, Phase::Ended),
            "jig-test failed: {}",
            String::from_utf8_lossy(binary.stderr())
        );
    }
    fn next_deadline(&self) -> Option<Time> {
        (!matches!(self.phase, Phase::Ended)).then_some(self.deadline)
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (now >= self.deadline && !matches!(self.phase, Phase::Ended))
            .then(|| format!("account sign-in process story did not finish: {}", self.status))
    }
    fn passed(&self) -> bool {
        matches!(self.phase, Phase::Ended)
    }
}
pub fn run(directory: &Path) {
    let clock = Clock::new();
    let judge = Judge {
        status: String::new(),
        phase: Phase::default(),
        deadline: clock.now().now.saturating_add(Duration::from_secs(10)),
    };
    let outcome = run_world(directory, judge, &clock, Duration::from_secs(15));
    assert!(outcome.procs.iter().all(Host::is_empty), "all real account sign-in io settles");
}

fn run_world<R: Referee<Process>>(
    directory: &Path,
    judge: R,
    clock: &Clock,
    patience: Duration,
) -> real::Outcome<Process> {
    let mut binary = start(directory);
    let streams = binary.take_streams().expect("binary pipes");
    let fds = streams.descriptors();
    let Streams::Pipes { input, output } = streams else { panic!("pipes mode") };
    let client_directory = tempfile::tempdir_in(directory).expect("client directory");
    let root = skein_shell::open_root(client_directory.path()).expect("client root");
    let mut world = real::World::new(judge);
    world.spawn_with_fds(binary.descriptors(), || Process::Binary(Box::new(binary)));
    world.spawn_with_fds(fds.into_iter().chain([root]).collect(), || {
        Process::Person(Box::new(Person {
            client: None,
            root: Some(root),
            script: Some(Script::account_sign_in()),
            account_sign_in: true,
            llm_key: None,
            input: Some(input),
            output: Some(output),
            reading: false,
            closing: 0,
            bytes: Vec::new(),
            completions: Queue::with_capacity(512),
            submissions: Queue::with_capacity(512),
        }))
    });
    world.spawn(|| Process::Issuer(Box::new(issuer())));
    world.spawn(|| Process::Visitor(Box::default()));
    world.run(clock, patience)
}

// A diagnostic bound, independent of the shipped 300-second sign-in deadline.
const SHUTDOWN_WAIT: Duration = Duration::from_secs(5);
const SETUP_WAIT: Duration = Duration::from_secs(10);
const WORLD_WAIT: Duration = Duration::from_secs(20);

struct Shutdown {
    pending: bool,
    asked: bool,
    sent: Option<Time>,
    killed: bool,
    exit: Option<Exit>,
    deadline: Time,
}
impl Referee<Process> for Shutdown {
    fn act(&mut self, now: Time, procs: &mut [Process]) {
        if self.exit.is_some() {
            return;
        }
        if let Some(sent) = self.sent {
            let Process::Binary(binary) = &mut procs[0] else { panic!("binary") };
            if let Some(status) = binary.exit_status() {
                let elapsed = now.saturating_since(sent);
                eprintln!(
                    "SIGTERM pending={} elapsed={elapsed:?} killed={} status={status:?}",
                    self.pending, self.killed
                );
                let Process::Issuer(issuer) = &mut procs[2] else { panic!("issuer") };
                // Keep the fake alive until the binary exits; it cannot help teardown.
                issuer.shutdown();
                self.exit = Some(status);
            } else if !self.killed && now >= sent.saturating_add(SHUTDOWN_WAIT) {
                eprintln!(
                    "SIGTERM pending={} still running after {SHUTDOWN_WAIT:?}; killing owned child",
                    self.pending
                );
                binary.signal(Signal::Kill);
                self.killed = true;
            }
            return;
        }
        let Process::Person(person) = &mut procs[1] else { panic!("native person") };
        let Some(native) = &mut person.client else { return };
        if native.signed_in != 1 || !native.account_sign_in_ready() {
            return;
        }
        if self.pending && !self.asked {
            native.ask_account_sign_in(ACCOUNT_SIGN_IN, c::Ask::SignIn { holder: c::AccountHolder::Own, provider: 3 });
            self.asked = true;
            return;
        }
        if self.pending {
            let Some(answer) = answer(procs) else { return };
            let c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::AccountSignInStarted { address, .. })) = answer
            else {
                panic!("account sign-in answer: {answer:?}")
            };
            assert!(!address.is_empty(), "the pending sign-in has its authorization address");
        }
        // Deliberately never hand the address to Visitor.
        client(procs).finish_account_sign_in();
        let Process::Binary(binary) = &mut procs[0] else { panic!("binary") };
        binary.signal(Signal::Terminate);
        self.sent = Some(now);
    }
    fn observe(&mut self, _now: Time, procs: &[Process]) {
        let Process::Visitor(visitor) = &procs[3] else { panic!("browser") };
        assert!(!visitor.handed(), "no authorization visit");
        let Process::Binary(binary) = &procs[0] else { panic!("binary") };
        assert!(binary.exit_status().is_none() || self.sent.is_some(), "binary exited before SIGTERM");
    }
    fn next_deadline(&self) -> Option<Time> {
        if self.exit.is_some() {
            None
        } else if let Some(sent) = self.sent {
            Some(if self.killed { self.deadline } else { sent.saturating_add(SHUTDOWN_WAIT) })
        } else {
            Some(self.deadline)
        }
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (self.exit.is_none() && now >= self.deadline).then(|| "SIGTERM process setup or cleanup did not settle".into())
    }
    fn passed(&self) -> bool {
        self.exit.is_some()
    }
}

/// Uses the shipped deadline unless this scenario explicitly varies it to identify the wait.
pub fn sigterm(pending: bool, sign_in_deadline_ms: Option<u64>) {
    let directory = crate::support::scratch();
    configuration(directory.path());
    if let Some(millis) = sign_in_deadline_ms {
        let path = directory.path().join("config.json");
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).expect("configuration")).expect("JSON");
        value["engine"]["limits"]["overrides"]["accounts.sign_in_deadline_ms"] = millis.into();
        private(&path, &serde_json::to_vec(&value).expect("configuration JSON"));
    }
    let clock = Clock::new();
    let judge = Shutdown {
        pending,
        asked: false,
        sent: None,
        killed: false,
        exit: None,
        deadline: clock.now().now.saturating_add(SETUP_WAIT),
    };
    let outcome = run_world(directory.path(), judge, &clock, WORLD_WAIT);
    assert!(outcome.procs.iter().all(Host::is_empty), "all SIGTERM test io settles");
    let Process::Binary(binary) = &outcome.procs[0] else { panic!("binary") };
    assert_eq!(binary.exit_status(), Some(Exit::Code(0)), "SIGTERM must end without SIGKILL");
}
