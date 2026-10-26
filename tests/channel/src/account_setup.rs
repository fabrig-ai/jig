//! A real client keeps the fixture's LLM key before its task is admitted.
use crate::{processes::Proc, stories::Story};
use jig_client::{Asker, Key, accounts, contract as c};
use jig_client_protocol_world::native::Received;
use skein_lib::Secret;

#[derive(Default)]
enum SourceKind {
    #[default]
    Key,
    Program,
}
#[derive(Default)]
pub(crate) struct Setup {
    kind: SourceKind,
    phase: u32,
    answers: usize,
    source: Option<u32>,
    linked: bool,
    finished: bool,
    quiet: bool,
    work_answers: usize,
    work_to: Option<u64>,
    work_pending: Option<jig_fake_parties::Up>,
}
impl Setup {
    pub(crate) fn program() -> Self {
        Self { kind: SourceKind::Program, ..Self::default() }
    }

    pub(crate) fn source(&self) -> Option<u32> {
        self.source
    }
    pub(crate) fn ready(&self) -> bool {
        self.phase == 3
    }
    pub(crate) fn act(&mut self, procs: &mut [Proc], story: Story, done: bool) {
        let link = if self.linked { None } else { ready_link(procs) };
        let Some(client) = procs.iter_mut().find_map(|p| if let Proc::Client(c) = p { Some(c) } else { None }) else {
            return;
        };
        if let Some(link) = link {
            client.sign_in(&link).expect("printed fixture link");
            self.linked = true;
        }
        if done && !self.finished {
            client.finish_account_sign_in();
            self.finished = true;
            return;
        }
        if self.phase == 0 && client.account_sign_in_ready() {
            let value = if matches!(story, Story::Key { .. }) { crate::fixtures::KEY } else { crate::fixtures::TOKEN };
            if matches!(self.kind, SourceKind::Program) {
                client.account_request(accounts::declare_program(
                    key(1),
                    [71; 16],
                    c::AccountHolder::Own,
                    1,
                    None,
                    b"alice".as_slice().into(),
                ));
            } else {
                client.account_request(accounts::add_key(
                    key(1),
                    [71; 16],
                    c::AccountHolder::Deployment,
                    1,
                    None,
                    Secret::new(value.into()),
                ));
            }
            self.phase = 1;
        }
        let replies: Vec<_> = client
            .received
            .iter()
            .filter_map(|r| {
                if let Received::Answer {
                    answer:
                        answer @ c::Answer::Done(c::Outcome::Owner(
                            c::OwnerOutcome::AccountKeyKept { .. } | c::OwnerOutcome::AccountProgramDeclared { .. },
                        )),
                    ..
                } = r
                {
                    Some(answer.clone())
                } else {
                    None
                }
            })
            .collect();
        for answer in &replies[self.answers..] {
            let c::Answer::Done(c::Outcome::Owner(
                c::OwnerOutcome::AccountKeyKept { source } | c::OwnerOutcome::AccountProgramDeclared { source, .. },
            )) = answer
            else {
                panic!("fixture source admitted: {answer:?}");
            };
            self.source = Some(*source);
            if self.phase == 1 && matches!(story, Story::Providers) {
                client.account_request(accounts::add_key(
                    key(2),
                    [72; 16],
                    c::AccountHolder::Deployment,
                    2,
                    None,
                    Secret::new(crate::fixtures::NEXT_KEY.into()),
                ));
                self.phase = 2;
            } else if self.phase == 1 && matches!(story, Story::Key { .. }) {
                client.account_request(accounts::replace_key(
                    key(2),
                    [72; 16],
                    c::AccountHolder::Deployment,
                    *source,
                    Secret::new(crate::fixtures::KEY.into()),
                ));
                self.phase = 2;
            } else {
                self.phase = 3;
            }
        }
        self.answers = replies.len();
        if self.ready() && !self.quiet {
            client.idle_fixture_view();
            self.quiet = true;
        }
    }
    pub(crate) fn party(
        &mut self,
        now: skein_lib::Time,
        procs: &mut [Proc],
        party: &mut jig_fake_parties::Party,
        views: &mut crate::world::Views,
        story: Story,
    ) {
        use jig_fake_parties::{Ask, Reply, Task, Up};
        if !self.ready() {
            return;
        }
        let client =
            procs.iter_mut().find_map(|p| if let Proc::Client(c) = p { Some(c) } else { None }).expect("native client");
        let answers: Vec<_> = client
            .received
            .iter()
            .filter_map(|r| match r {
                Received::Answer {
                    answer: c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::Created { first, .. })),
                    ..
                } => Some(Reply::Started { task: *first }),
                Received::Answer {
                    answer: c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::Said { task, message })),
                    ..
                } => Some(Reply::Said { task: *task, message: *message }),
                Received::SignedIn { .. }
                | Received::Sent { .. }
                | Received::Answer { .. }
                | Received::Delivery(_)
                | Received::Read(_) => None,
            })
            .collect();
        for reply in &answers[self.work_answers..] {
            party.reply(self.work_to.take().expect("one scripted request"), reply.clone());
        }
        self.work_answers = answers.len();
        observe(client, views);
        let Some(up) = self.work_pending.take().or_else(|| party.tick(now)) else {
            return;
        };
        let (to, name, ask) = match &up {
            Up::SignIn { to, .. } => {
                let person = client
                    .received
                    .iter()
                    .find_map(|r| if let Received::SignedIn { person } = r { Some(*person) } else { None })
                    .expect("real native sign-in");
                // The script only needs an authenticated marker; the native client owns the credential.
                party.reply(*to, Reply::SignedIn { person, sign_in: 1 });
                return;
            }
            Up::Request { to, project, key, ask: Ask::Chat { words }, .. } => {
                let Some(me) = client
                    .received
                    .iter()
                    .rev()
                    .find_map(|r| if let Received::Delivery(c::Delivery::Me(me)) = r { Some(me) } else { None })
                else {
                    self.work_pending = Some(up);
                    return;
                };
                let Ok(mut ask) = jig_client::shapes::chat(me, *project, text(words)) else {
                    self.work_pending = Some(up);
                    return;
                };
                if let c::Ask::Create { tasks, .. } = &mut ask {
                    let task = tasks.get_mut(0).expect("chat");
                    if matches!(story, Story::ApplicationGeneration) {
                        task.spec.parameters = skein_lib::List::with_capacity(1);
                        task.spec
                            .parameters
                            .push(c::Parameter::Resource { name: 1, connector: 1, resource: 1 })
                            .expect("bounded fixture value");
                    }
                    if matches!(story, Story::Providers) {
                        let mut subagents = skein_lib::List::with_capacity(1);
                        subagents
                            .push(c::ModelEffort { model: text(b"other"), effort: None })
                            .expect("bounded fixture value");
                        task.llm = Some(c::LlmChoice {
                            account: None,
                            main: c::ModelEffort { model: text(b"fake"), effort: None },
                            subagents,
                        });
                    }
                }
                (*to, *key, ask)
            }
            Up::Request { to, project, key, ask: Ask::Say { task, words, release: None }, .. } => {
                let task = match task {
                    Task::Number(number) => *number,
                    Task::Last => party.last_task.expect("created chat"),
                    Task::Waiting => panic!("channel has no person tasks"),
                };
                (*to, *key, c::Ask::Message { project: *project, task, words: text(words) })
            }
            Up::Request { .. } => panic!("channel public request {up:?}"),
        };
        client.account_request(jig_client::Event::Send {
            key: Key { asker: Asker(78), number: to },
            name: Some(name),
            request: jig_client::Ask::Jig(Box::new(ask)),
        });
        self.work_to = Some(to);
    }
    pub(crate) fn replace(&self, procs: &mut [Proc]) {
        let Some(client) = procs.iter_mut().find_map(|p| if let Proc::Client(c) = p { Some(c) } else { None }) else {
            panic!("fixture client");
        };
        client.account_request(accounts::replace_key(
            key(3),
            [73; 16],
            c::AccountHolder::Deployment,
            self.source.expect("kept key"),
            Secret::new(crate::fixtures::NEXT_KEY.into()),
        ));
    }
}
fn ready_link(procs: &mut [Proc]) -> Option<Box<[u8]>> {
    procs.iter_mut().find_map(|p| {
        if let Proc::Engine(e) = p
            // A printed link may precede the mark; this fixture signs in once.
            && e.service.account_startup(0, 1, skein_lib::Time::ZERO).is_some_and(|state| state.durable)
        {
            e.service.links().pop()
        } else {
            None
        }
    })
}

fn key(number: u64) -> Key {
    Key { asker: Asker(77), number }
}

fn text(value: &[u8]) -> c::Text {
    c::Text::new(value.into(), 512).expect("bounded fixture value")
}

#[expect(clippy::wildcard_enum_match_arm, reason = "this fixture checks only run activity and provisional text")]
fn observe(client: &jig_client_protocol_world::native::NativeClient, views: &mut crate::world::Views) {
    if let Some(text) = client.text(crate::live_text::RUN) {
        views.text_bytes = views.text_bytes.max(text.text.bytes().len());
    }
    for received in &client.received {
        match received {
            Received::Delivery(c::Delivery::Task(task)) => match &*task.chip.details {
                c::ChipPhase::Owner { details, .. } => views.activity_seen |= details.observation.is_some(),
                c::ChipPhase::Active(run) => views.activity_seen |= run.observation.is_some(),
                _ => {}
            },
            Received::Delivery(c::Delivery::Run(text)) => {
                views.text_bytes = views.text_bytes.max(text.text.bytes().len());
            }
            Received::Delivery(c::Delivery::RunState(state)) => {
                if let Some(writing) = &state.writing {
                    assert!(writing.turn.number > state.committed_turn, "committed turns have no writing");
                }
            }
            _ => {}
        }
    }
}
