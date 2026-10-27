//! Translations between the test root and independent scripted peers. The
//! workers and parties retain their own observations; this world reads only
//! released root outputs and durable store rows (`domain/testing.md`, 4, 7).
use crate::Store;
mod delegation;
mod worker;
use delegation::delegate;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_fake_parties as parties;
use jig_fake_workers as workers;
use jig_test_domain as root;
use skein_lib::{Duration, ReplyTo, Time, Token};
use std::collections::BTreeMap;

type OpaqueName = (u64, u64, Box<[u8]>);

/// Scripted peers and the ordinary world's correlation ledger.
#[derive(Clone, Debug)]
pub struct Peers {
    pub workers: Vec<workers::Worker>,
    pub parties: Vec<parties::Party>,
    pub upstream: Vec<workers::Up>,
    pub requests: Vec<parties::Up>,
    pub creating: Option<crate::fixtures::Creating>,
    party_flights: BTreeMap<u64, usize>,
    party_marks: BTreeMap<usize, (u64, Option<u64>, parties::Role)>,
    call_hosts: BTreeMap<core::CallKey, (u64, u32)>,
    pub protocol: crate::protocol::Stand,
    opaque: BTreeMap<OpaqueName, (u64, u32)>,
}

impl Peers {
    #[must_use]
    pub fn new(workers: Vec<workers::Worker>, parties: Vec<parties::Party>) -> Self {
        Self {
            workers,
            parties,
            upstream: Vec::new(),
            requests: Vec::new(),
            creating: None,
            party_flights: BTreeMap::new(),
            party_marks: BTreeMap::new(),
            call_hosts: BTreeMap::new(),
            protocol: crate::protocol::Stand::new(crate::world::limits().core),
            opaque: BTreeMap::new(),
        }
    }

    /// Reconnect the independently retained peers to a cold engine.
    pub fn restart(&mut self) {
        self.party_flights.clear();
        self.party_marks.clear();
        for worker in &mut self.workers {
            worker.restart_engine();
        }
        for party in &mut self.parties {
            party.restart_engine();
        }
    }

    /// Scripts progress only from independently received outputs and store rows.
    pub fn tick(&mut self, store: &Store, now: Time) -> Vec<root::Event> {
        let reports: Vec<_> = self.workers.iter_mut().flat_map(|worker| worker.tick(now)).collect();
        let mut events: Vec<_> = reports.into_iter().map(|up| self.worker_event(up)).collect();
        for index in 0..self.parties.len() {
            let party = &self.parties[index];
            let mark = (store.applied, party.person, party.role);
            if self.party_marks.get(&index) != Some(&mark) {
                let entry = waiting(store, party);
                self.parties[index].waiting(entry);
                self.party_marks.insert(index, mark);
            }
            if let Some(up) = self.parties[index].tick(now) {
                events.push(self.party_event(index, up));
            }
        }
        events
    }

    fn party_event(&mut self, index: usize, up: parties::Up) -> root::Event {
        self.requests.push(up.clone());
        match up {
            parties::Up::SignIn { to, provider, subject } => {
                assert!(self.party_flights.insert(to, index).is_none());
                root::Event::Core(core::Event::SignIn {
                    held: None,
                    reply_to: ReplyTo::new(Token::new(to)),
                    identity: people::Identity {
                        key: people::IdentityKey { provider, subject },
                        login: b"person".as_slice().into(),
                        name: b"Person".as_slice().into(),
                    },
                })
            }
            parties::Up::Request { to, sign_in, project, key, ask } => {
                assert!(self.party_flights.insert(to, index).is_none());
                let ask = match ask {
                    parties::Ask::SeeResults { through } => {
                        people::Ask::MarkSeen { seen: people::Seen::Results { through } }
                    }
                    parties::Ask::SeeConversation { task, through } => {
                        people::Ask::MarkSeen { seen: people::Seen::Conversation { task: task_number(task), through } }
                    }
                    parties::Ask::Chat { words } => crate::fixtures::create_chat(
                        self.creating.as_ref().expect("fixture creating authority"),
                        project,
                        role_number(self.parties[index].role),
                        words,
                    ),
                    parties::Ask::Goal { words, budget, priority, deadline } => {
                        let people::Ask::Create { mut batch, .. } = crate::fixtures::create_chat(
                            self.creating.as_ref().expect("fixture creating authority"),
                            project,
                            role_number(self.parties[index].role),
                            words,
                        ) else {
                            unreachable!()
                        };
                        let member = &mut batch[0];
                        member.budget = people::Budget::Named;
                        member.authority.spend = budget;
                        member.authority.deadline = deadline;
                        member.flags.conversation = false;
                        member.flags.listed = true;
                        member.label = jig_client_test_records::GOAL;
                        member.tracked = Some(priority);
                        people::Ask::Create { project, batch }
                    }
                    parties::Ask::Say { task, words, release, .. } => say(project, task_number(task), words, release),
                    parties::Ask::Cancel { task, grace } => people::Ask::Cancel {
                        project,
                        task: task_number(task),
                        reason: b"cancelled".as_slice().into(),
                        grace,
                    },
                    parties::Ask::Stop { task } => people::Ask::Stop { project, task: task_number(task) },
                    parties::Ask::Release { task, revision, again } => {
                        people::Ask::Release { project, task: task_number(task), revision, again }
                    }
                    parties::Ask::Close { task, disposals } => closing(project, task, disposals),
                    parties::Ask::Take { task } => people::Ask::TakePerson { project, task: task_number(task) },
                    parties::Ask::Choose { task, revision, code, words } => people::Ask::AnswerPerson {
                        project,
                        task: task_number(task),
                        revision,
                        result: people::PersonResult::Verdict { code, words },
                    },
                    parties::Ask::Decide { .. } => panic!("party resolves its visible proposal"),
                    parties::Ask::Decision { proposer, proposal, accept } => people::Ask::DecideProposal {
                        project,
                        proposer: match proposer {
                            parties::Proposer::Task(number) => people::Proposer::Task(number),
                            parties::Proposer::Party(number) => people::Proposer::Party(number),
                        },
                        proposal,
                        decision: if accept {
                            people::ProposalDecision::Accept
                        } else {
                            people::ProposalDecision::Reject { reason: b"not now".as_slice().into() }
                        },
                        running: None,
                    },
                };
                root::Event::Core(core::Event::People(people::Event::Ask {
                    models: Box::new([]),
                    reply_to: ReplyTo::new(Token::new(to)),
                    sign_in,
                    key,
                    ask,
                }))
            }
        }
    }

    /// Deliver post-commit observations to the peer that owns the flight.
    pub fn delivered(&mut self, delivery: &root::Delivery) {
        match delivery {
            root::Delivery::Assigned { channel, assignment } => {
                self.worker(channel.raw()).assign(workers::Assignment {
                    task: assignment.task,
                    attempt: assignment.attempt,
                    conversation: assignment.conversation,
                    budget: assignment.run.budget,
                    transcript: assignment.transcript.clone(),
                    messages: assignment.messages.iter().map(|word| word.number).collect(),
                });
            }
            root::Delivery::Core(core::Held::AcknowledgeTurn { channel, run, attempt, turn }) => {
                self.worker(channel.raw()).acknowledge_turn(run.raw(), attempt.raw(), *turn);
            }
            root::Delivery::Core(core::Held::Acknowledge { channel, run, attempt }) => {
                self.worker(channel.raw()).acknowledge_answer(run.raw(), attempt.raw());
            }
            root::Delivery::Core(core::Held::Cancel { channel, run, attempt }) => {
                self.worker(channel.raw()).cancel(run.raw(), attempt.raw());
            }
            root::Delivery::Message { channel, task, attempt, word } => {
                self.worker(channel.raw()).message(*task, *attempt, word.number, word.words.clone());
            }
            root::Delivery::Core(core::Held::CallAnswer { key, part, .. }) => {
                if let Some((channel, number)) = self.call_hosts.remove(key) {
                    let bytes = format!("{part:?}").into_bytes().into_boxed_slice();
                    self.worker(channel).call_answer(key.task, key.attempt, number, bytes);
                }
            }
            root::Delivery::CallAnswer { task, attempt, name, call, .. } => {
                if let Some((channel, number)) = self.opaque.remove(&(*task, *attempt, name.clone())) {
                    self.worker(channel).call_answer(
                        *task,
                        *attempt,
                        number,
                        match &call.answer {
                            core::SettledAnswer::Host { body, .. } => body.clone(),
                            core::SettledAnswer::Delivery { evidence, .. } => evidence.clone(),
                        },
                    );
                }
            }
            root::Delivery::Application { .. }
            | root::Delivery::LostRead { .. }
            | root::Delivery::Restart(_)
            | root::Delivery::Core(_)
            | root::Delivery::Fleet(_)
            | root::Delivery::ApplicationDocument { .. }
            | root::Delivery::ApplicationRead { .. }
            | root::Delivery::Projection { .. }
            | root::Delivery::Link { .. }
            | root::Delivery::ProjectMade { .. }
            | root::Delivery::CallBusy { .. }
            | root::Delivery::System { .. }
            | root::Delivery::Procedure { .. } => {}
        }
    }

    pub fn reply(&mut self, to: ReplyTo, reply: &people::Reply) {
        let number = to.into_token().raw();
        if let Some(index) = self.party_flights.remove(&number) {
            let translated = match reply {
                people::Reply::Outcome(people::Outcome::Compound { .. }) => {
                    match self
                        .requests
                        .iter()
                        .rev()
                        .find(|up| matches!(up, parties::Up::Request { to, .. } if *to == number))
                    {
                        Some(parties::Up::Request { ask: parties::Ask::Close { task, disposals }, .. }) => {
                            parties::Reply::Closed {
                                chat: task_number(*task),
                                cancelled: u32::try_from(
                                    disposals.iter().filter(|d| d.keep == parties::Keep::Cancel).count(),
                                )
                                .expect("bounded disposals"),
                                kept: u32::try_from(
                                    disposals.iter().filter(|d| d.keep == parties::Keep::MakeMine).count(),
                                )
                                .expect("bounded disposals"),
                            }
                        }
                        _ => parties::Reply::Done,
                    }
                }
                reply @ (people::Reply::Signer(_)
                | people::Reply::Full { .. }
                | people::Reply::SignedIn { .. }
                | people::Reply::SignedOut
                | people::Reply::Outcome(_)
                | people::Reply::Refused(_)
                | people::Reply::Again(_)) => party_reply(reply),
            };
            self.parties[index].reply(number, translated);
        }
    }

    fn worker(&mut self, channel: u64) -> &mut workers::Worker {
        self.workers.iter_mut().find(|worker| worker.channel == channel).expect("configured host")
    }

    /// The world's protocol adapter decodes core tools before application calls.
    pub fn opaque_answer(&mut self, to: ReplyTo, run: Token, attempt: Token, call: &fleet::Call) -> Vec<root::Event> {
        let number = self.opaque.get(&(run.raw(), attempt.raw(), call.name.clone())).expect("retained opaque call").1;
        let key = core::CallKey { task: run.raw(), attempt: attempt.raw(), completion: number, position: 0 };
        self.call_hosts.remove(&key);
        let token = to.into_token();
        if let Some(event) = self.protocol.decode(token, run, attempt, call) {
            return vec![event];
        }
        if !call.writes {
            return vec![root::Event::CallAnswered {
                to: ReplyTo::new(token),
                call: core::SettledCall {
                    serial: 0,
                    tool: call.tool.clone(),
                    answer: core::SettledAnswer::Host { error: false, body: call.input.clone() },
                },
            }];
        }
        vec![
            root::Event::Core(core::Event::NamedAnswer {
                to: ReplyTo::new(token),
                key,
                part: core::CallPart::Unavailable,
            }),
            root::Event::Core(core::Event::SettledCall {
                to: ReplyTo::new(token),
                key,
                name: call.name.clone(),
                call: core::SettledCall {
                    serial: 0,
                    tool: call.tool.clone(),
                    answer: core::SettledAnswer::Host { error: false, body: call.input.clone() },
                },
            }),
        ]
    }

    #[must_use]
    pub fn quiescent(&self) -> bool {
        self.workers.iter().all(workers::Worker::quiescent) && self.parties.iter().all(parties::Party::quiescent)
    }
}

fn task_number(task: parties::Task) -> u64 {
    match task {
        parties::Task::Number(number) => number,
        parties::Task::Last | parties::Task::Waiting => panic!("party resolved its task reference"),
    }
}

fn party_reply(reply: &people::Reply) -> parties::Reply {
    match reply {
        people::Reply::Signer(_) | people::Reply::Full { .. } => parties::Reply::Again(parties::Again::Busy),
        people::Reply::SignedIn { person, sign_in, .. } => {
            parties::Reply::SignedIn { person: *person, sign_in: *sign_in }
        }
        people::Reply::Outcome(people::Outcome::Created { first: task, .. }) => parties::Reply::Started { task: *task },
        people::Reply::Outcome(people::Outcome::Said { task, message }) => {
            parties::Reply::Said { task: *task, message: *message }
        }
        people::Reply::Outcome(people::Outcome::Proposed { proposal, .. }) => {
            parties::Reply::Proposed { proposal: *proposal }
        }
        people::Reply::Outcome(people::Outcome::Stopped { task } | people::Outcome::Released { task }) => {
            parties::Reply::Changed { task: *task }
        }
        people::Reply::Again(again) | people::Reply::Outcome(people::Outcome::Again(again)) => {
            parties::Reply::Again(match again {
                people::Again::Busy => parties::Again::Busy,
                people::Again::NotReady => parties::Again::NotReady,
            })
        }
        people::Reply::Refused(why) | people::Reply::Outcome(people::Outcome::Refused(why)) => {
            parties::Reply::Refused(party_refusal(why.clone()))
        }
        people::Reply::SignedOut | people::Reply::Outcome(people::Outcome::SignedOut) => parties::Reply::SignedOut,
        people::Reply::Outcome(people::Outcome::ProposalDecided { proposer, proposal, by, at, choice }) => {
            parties::Reply::ProposalDecided {
                proposer: match proposer {
                    people::Proposer::Task(number) => parties::Proposer::Task(*number),
                    people::Proposer::Party(number) => parties::Proposer::Party(*number),
                },
                proposal: *proposal,
                by: *by,
                at: *at,
                choice: match choice {
                    people::ProposalChoice::Accepted => parties::Choice::Accepted,
                    people::ProposalChoice::Rejected => parties::Choice::Rejected,
                    people::ProposalChoice::Passed => parties::Choice::Passed,
                    people::ProposalChoice::Withdrawn => parties::Choice::Withdrawn,
                    people::ProposalChoice::Stale => parties::Choice::Stale,
                },
            }
        }
        people::Reply::Outcome(people::Outcome::EscalationDecided { task, revision, by, at, choice }) => {
            parties::Reply::EscalationDecided {
                task: *task,
                revision: *revision,
                by: *by,
                at: *at,
                choice: match choice {
                    people::EscalationChoice::Released => parties::Choice::Released,
                    people::EscalationChoice::Rejected => parties::Choice::Rejected,
                    people::EscalationChoice::Passed => parties::Choice::Passed,
                },
            }
        }
        people::Reply::Outcome(_) => parties::Reply::Done,
    }
}

fn waiting(store: &Store, party: &parties::Party) -> Option<parties::Waiting> {
    let person = party.person?;
    for record in store.rows.values() {
        let root::Record::Core(core::Record::Tasks(tasks::Stored::Live(task))) = record else { continue };
        if let Some(proposal) = &task.proposal {
            let visible = match proposal.state {
                tasks::ProposalState::Pending { holder: tasks::ProposalHolder::Person(holder), .. } => holder == person,
                tasks::ProposalState::Pending { holder: tasks::ProposalHolder::Policy { .. }, .. } => {
                    matches!(party.role, parties::Role::Owner | parties::Role::Maintainer)
                }
                tasks::ProposalState::Pending { .. }
                | tasks::ProposalState::Accepted { .. }
                | tasks::ProposalState::Rejected { .. }
                | tasks::ProposalState::Withdrawn => false,
            };
            if visible {
                return Some(parties::Waiting::Proposal {
                    proposer: parties::Proposer::Task(task.number),
                    proposal: proposal.number,
                });
            }
        }
        let visible = match task.executor {
            tasks::Executor::Person(tasks::PersonAddress::Person(holder)) => holder == person,
            tasks::Executor::Person(tasks::PersonAddress::Role(role)) => role == role_number(party.role),
            tasks::Executor::Agent { .. } | tasks::Executor::Procedure { .. } => false,
        };
        if visible
            && task.taken_by.is_none_or(|holder| holder == person)
            && matches!(task.phase, tasks::Phase::Active(_))
        {
            return Some(parties::Waiting::Choice { task: task.number, revision: task.revisions.person });
        }
    }
    None
}

fn role_number(role: parties::Role) -> u32 {
    match role {
        parties::Role::Owner => 0,
        parties::Role::Maintainer => 1,
        parties::Role::Member => 2,
        parties::Role::Observer => 3,
    }
}

fn authority(budget: u64, agent: bool) -> tasks::Authority {
    tasks::Authority {
        tools: tasks::Tools(if agent { 1023 } else { 0 }),
        models: tasks::Models::NONE,
        grants: Box::new([]),
        delegation: tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 },
        budget: tasks::Budget { spend: budget, deadline: None },
        notes: tasks::Scopes(0),
        note_resources: Box::new([]),
    }
}

fn end_value(end: workers::End) -> tasks::End {
    match end {
        workers::End::Finished(words) => {
            tasks::End::Finished { result: tasks::TaskResult::Report { words }, cancel_delegates: false }
        }
        workers::End::Parked => tasks::End::Parked,
        workers::End::Failed(why) => tasks::End::Failed(match why {
            workers::Failure::Transient => tasks::Class::Transient,
            workers::Failure::Permanent => tasks::Class::Permanent,
            workers::Failure::Run => tasks::Class::Run,
            workers::Failure::Agent => tasks::Class::Agent,
            workers::Failure::Lost => tasks::Class::Lost,
            workers::Failure::Invalid => tasks::Class::Invalid,
        }),
    }
}

/// Tiny limits with enough roles, hosts and person verdicts for the peer stories.
#[must_use]
pub fn fixture(seed: u64, engine_slots: u32) -> (root::Config, root::Limits) {
    use jig_domain_authority as policy;
    let (mut configuration, mut limits) = crate::effects::fixture(seed, false);
    limits.core.tasks.tasks = 5;
    limits.core.tasks.project_tasks = 5;
    limits.core.tasks.tree_tasks = 5;
    limits.core.tasks.depth = 2;
    limits.core.tasks.delegates = 3;
    limits.core.tasks.funders = 16;
    limits.core.tasks.executor_kinds = 3;
    limits.core.tasks.contract_choices = 2;
    limits.core.people.people = 4;
    limits.core.people.sign_ins = 4;
    limits.core.people.holdings = 4;
    limits.core.people.inbox_entries = 4;
    limits.core.people.inbox_recipients = 16;
    limits.core.people.requests = 32;
    limits.core.people.operations = 4;
    limits.core.people.amendment_bytes = 256;
    limits.core.authority.roles = 4;
    limits.core.authority.executors = 3;
    limits.core.documents.executors = 3;
    limits.core.fleet.workers = 3;
    limits.core.fleet.slots = 1;
    limits.core.fleet.engine_slots = engine_slots;
    limits.core.fleet.attempts = 5;
    limits.core.fleet.calls = 5;
    limits.core.brief.briefs = 5;
    limits.core.views.runs = 5;
    limits.core.call_records = 16;
    journal(&mut limits);
    configuration.core.defaults.charters[0].hosts = fleet::Kinds::Both;
    let kinds = Box::new([policy::Executor::Charter(1), policy::Executor::Procedure(1), policy::Executor::Role(1)]);
    let mut rules = configuration.core.defaults.rules.clone();
    rules.ceiling.delegation = policy::Delegation { kinds, tasks: 6, depth: 3 };
    rules.maximum_run_spend = 20;
    let mut project = configuration.core.seed_projects[0].policy.policy.clone();
    project.ceiling = rules.ceiling.clone();
    let mut owner = project.roles[0].clone();
    owner.authority.delegation =
        policy::Delegation { kinds: rules.ceiling.delegation.kinds.clone(), tasks: 4, depth: 3 };
    owner.authority.budget.spend = 40;
    let mut maintainer = owner.clone();
    maintainer.number = 1;
    maintainer.authority.budget.spend = 500;
    let mut member = owner.clone();
    member.number = 2;
    member.authority.budget.spend = 50;
    member.decides = policy::Proposals(0);
    let mut observer = owner.clone();
    observer.number = 3;
    observer.requests = policy::Requests(128);
    observer.decides = policy::Proposals(0);
    project.roles = Box::new([owner, maintainer, member, observer]);
    let mut checked = policy::Domain::new(rules, limits.core.authority).expect("peer policy limits");
    let mut out = skein_lib::Queue::with_capacity(policy::POLICY_MAX_OUT);
    policy::step(&mut checked, policy::Event::Policy { project: 1, policy: project }, &mut out);
    assert_eq!(out.pop(), Some(policy::PolicyFact::Added { project: 1 }));
    configuration.core.seed_projects[0].policy.policy = checked.policy(1).expect("checked seed policy").clone();
    configuration.core.defaults.rules = (jig_domain_authority::Domain::new(checked.rules().clone(), *checked.limits())
        .expect("checked rules"))
    .rules()
    .clone();
    (configuration, limits)
}

fn party_refusal(why: people::Refusal) -> parties::Refusal {
    match why {
        people::Refusal::Circle { tasks, cut } => parties::Refusal::Circle { tasks, cut },
        people::Refusal::Kept { pending } => parties::Refusal::Kept { pending },
        people::Refusal::Dependency { dependency, how } => parties::Refusal::Dependency {
            dependency,
            how: match how {
                people::DependencyStatus::Done => parties::DependencyStatus::Done,
                people::DependencyStatus::Failed => parties::DependencyStatus::Failed,
                people::DependencyStatus::Cancelled => parties::DependencyStatus::Cancelled,
            },
        },
        people::Refusal::Role { held } => parties::Refusal::Role { held: held.map(people::Role::number) },
        people::Refusal::Authority { proposable, lacking } => parties::Refusal::Authority {
            proposable: match proposable {
                Some(people::Accepter::Role(_)) => Some(true),
                Some(people::Accepter::Nobody) => Some(false),
                None => None,
            },
            lacking: party_lack(lacking),
        },
        people::Refusal::Limit { which } => parties::Refusal::Limit { which: party_limit(which) },
        people::Refusal::Moved { now } => parties::Refusal::Moved { now: party_moved(now) },
        people::Refusal::Ended { how } => parties::Refusal::Ended { how: party_ending(how) },
        people::Refusal::Taken { by } => parties::Refusal::Taken { by },
        people::Refusal::NotWellFormed { why: people::Malformed::Cycle, .. } => {
            parties::Refusal::Limit { which: parties::LimitKind::Cycle }
        }
        people::Refusal::NotWellFormed {
            why: people::Malformed::AsAllowedTwice | people::Malformed::ProposedAsAllowed,
            ..
        } => parties::Refusal::Limit { which: parties::LimitKind::Batch },
        people::Refusal::NotWellFormed {
            why: people::Malformed::Conversation | people::Malformed::EndedBy, ..
        } => parties::Refusal::Limit { which: parties::LimitKind::Spec },
        people::Refusal::NotWaiting => parties::Refusal::NotWaiting,
        people::Refusal::NoFurther => parties::Refusal::NoFurther,
        people::Refusal::SignInRedirect { .. } | people::Refusal::AccountMove { .. } => {
            unreachable!("fake parties ask for no account moves")
        }
        people::Refusal::Forgotten { .. }
        | people::Refusal::Stale { .. }
        | people::Refusal::Full { .. }
        | people::Refusal::NotWellFormed {
            why: people::Malformed::Settings { .. } | people::Malformed::SettingsUnchanged | people::Malformed::Running,
            ..
        }
        | people::Refusal::NameTaken
        | people::Refusal::NotWellFormed { why: people::Malformed::Name(_), .. } => {
            unreachable!("fake parties ask for no project changes")
        }
        people::Refusal::Unknown => parties::Refusal::Unknown,
        people::Refusal::Account { .. }
        | people::Refusal::Provider
        | people::Refusal::NotOffered
        | people::Refusal::NoEscalationHolder => parties::Refusal::NotOffered,
        people::Refusal::KeyConflict => parties::Refusal::KeyConflict,
        people::Refusal::Llm { model, .. } => parties::Refusal::Llm { model },
    }
}

fn party_lack(value: people::Lack) -> parties::Lack {
    match value {
        people::Lack::Approver => parties::Lack::Approver,
        people::Lack::Budget { needs, left } => parties::Lack::Budget { needs, left },
        people::Lack::Grant { kind } => parties::Lack::Grant { kind },
        people::Lack::Funding => parties::Lack::Funding,
        people::Lack::TaskKind { .. } => parties::Lack::TaskKind,
        people::Lack::Depth { most } => parties::Lack::Depth { most },
        people::Lack::Deadline => parties::Lack::Deadline,
        people::Lack::Tasks { left } => parties::Lack::Tasks { left },
        people::Lack::Tool => parties::Lack::Tool,
        people::Lack::Scope => parties::Lack::Scope,
        people::Lack::Models => parties::Lack::Models,
        people::Lack::Reference => parties::Lack::Reference,
        people::Lack::ResourceAccess => parties::Lack::ResourceAccess,
        people::Lack::ResourceUnavailable => parties::Lack::ResourceUnavailable,
        people::Lack::Shape => parties::Lack::Shape,
        people::Lack::Again { required } => parties::Lack::Again { required },
        people::Lack::Account => parties::Lack::Account,
        people::Lack::Writer => parties::Lack::Writer,
        people::Lack::Required { connector, kind } => parties::Lack::Required { connector, kind },
        people::Lack::Failed { connector, kind } => parties::Lack::Failed { connector, kind },
        people::Lack::Unguarded { connector, kind } => parties::Lack::Unguarded { connector, kind },
    }
}

fn party_limit(value: people::LimitKind) -> parties::LimitKind {
    match value {
        people::LimitKind::Operations => parties::LimitKind::Operations,
        people::LimitKind::Words => parties::LimitKind::Words,
        people::LimitKind::Identity => parties::LimitKind::Identity,
        people::LimitKind::Roles => parties::LimitKind::Roles,
        people::LimitKind::Arithmetic => parties::LimitKind::Arithmetic,
        people::LimitKind::Duplicate => parties::LimitKind::Duplicate,
        people::LimitKind::Empty => parties::LimitKind::Empty,
        people::LimitKind::SettingsProposals => parties::LimitKind::SettingsProposals,
        people::LimitKind::Batch => parties::LimitKind::Batch,
        people::LimitKind::ProjectLiveTasks => parties::LimitKind::ProjectLiveTasks,
        people::LimitKind::LiveTasks => parties::LimitKind::LiveTasks,
        people::LimitKind::Tree => parties::LimitKind::Tree,
        people::LimitKind::Depth => parties::LimitKind::Depth,
        people::LimitKind::Delegates => parties::LimitKind::Delegates,
        people::LimitKind::Subscription => parties::LimitKind::Subscription,
        people::LimitKind::Dependencies => parties::LimitKind::Dependencies,
        people::LimitKind::Cycle => parties::LimitKind::Cycle,
        people::LimitKind::Spec => parties::LimitKind::Spec,
        people::LimitKind::Contract => parties::LimitKind::Contract,
        people::LimitKind::Authority => parties::LimitKind::Authority,
        people::LimitKind::Inputs => parties::LimitKind::Inputs,
        people::LimitKind::Attempts => parties::LimitKind::Attempts,
        people::LimitKind::LiveDelegates => parties::LimitKind::LiveDelegates,
        people::LimitKind::Restore => parties::LimitKind::Restore,
        people::LimitKind::Read => parties::LimitKind::Read,
        people::LimitKind::Turn => parties::LimitKind::Turn,
        people::LimitKind::HoldKind => parties::LimitKind::HoldKind,
        people::LimitKind::HoldTaken => parties::LimitKind::HoldTaken,
        people::LimitKind::Holds => parties::LimitKind::Holds,
        people::LimitKind::InboxFull => parties::LimitKind::InboxFull,
        people::LimitKind::Notes => parties::LimitKind::Notes,
        people::LimitKind::Revision => parties::LimitKind::Revision,
        people::LimitKind::Question => parties::LimitKind::Question,
        people::LimitKind::Priority => parties::LimitKind::Priority,
        people::LimitKind::Policy => parties::LimitKind::Policy,
        people::LimitKind::Resource => parties::LimitKind::Resource,
        people::LimitKind::Grace => parties::LimitKind::Grace,
        people::LimitKind::Goals => parties::LimitKind::Goals,
        people::LimitKind::Service => parties::LimitKind::Service,
        people::LimitKind::Projects
        | people::LimitKind::Accounts
        | people::LimitKind::HolderAccounts
        | people::LimitKind::Sources
        | people::LimitKind::SignIns => unreachable!("a fake party asks for no account"),
    }
}

fn party_moved(value: people::Moved) -> parties::Moved {
    match value {
        people::Moved::Hold { revision, reason } => parties::Moved::Hold { revision, reason: party_hold(reason) },
        people::Moved::Spec { revision } => parties::Moved::Spec { revision },
        people::Moved::Person { revision } => parties::Moved::Person { revision },
        people::Moved::Note { revision } => parties::Moved::Note { revision },
    }
}

fn party_hold(value: people::HoldReason) -> parties::HoldReason {
    match value {
        people::HoldReason::Exceeded(exceeded) => parties::HoldReason::Exceeded(parties::Exceeded {
            bound: match exceeded.bound {
                people::Bound::Reasoning => parties::Bound::Reasoning,
                people::Bound::Metadata => parties::Bound::Metadata,
                people::Bound::Items => parties::Bound::Items,
                people::Bound::Answer => parties::Bound::Answer,
                people::Bound::Tools => parties::Bound::Tools,
                people::Bound::HistoryItems => parties::Bound::HistoryItems,
                people::Bound::History => parties::Bound::History,
                people::Bound::Window => parties::Bound::Window,
                people::Bound::Output => parties::Bound::Output,
            },
            value: exceeded.value,
        }),
        people::HoldReason::Failures { class } => parties::HoldReason::Failures { class: party_failure(class) },
        people::HoldReason::Stalled => parties::HoldReason::Stalled,
        people::HoldReason::Dependency { task } => parties::HoldReason::Dependency { task },
        people::HoldReason::Stopped => parties::HoldReason::Stopped,
        people::HoldReason::StoppedBy { party } => parties::HoldReason::StoppedBy { party },
        people::HoldReason::Drift => parties::HoldReason::Drift,
        people::HoldReason::Effects => parties::HoldReason::Effects,
        people::HoldReason::EffectFailed => parties::HoldReason::EffectFailed,
        people::HoldReason::Uncertain { entry } => parties::HoldReason::Uncertain { entry },
        people::HoldReason::Budget => parties::HoldReason::Budget,
        people::HoldReason::Deadline => parties::HoldReason::Deadline,
        people::HoldReason::HoldsWaited => parties::HoldReason::HoldsWaited,
        people::HoldReason::ResourceUnavailable => parties::HoldReason::ResourceUnavailable,
        people::HoldReason::ResourceAccess => parties::HoldReason::ResourceAccess,
        people::HoldReason::PoolLost { pool } => parties::HoldReason::PoolLost { pool },
        people::HoldReason::Procedure => parties::HoldReason::Procedure,
        people::HoldReason::Circle => parties::HoldReason::Circle,
    }
}

fn party_failure(value: people::FailureClass) -> parties::FailureClass {
    match value {
        people::FailureClass::Transient => parties::FailureClass::Transient,
        people::FailureClass::Permanent => parties::FailureClass::Permanent,
        people::FailureClass::Run => parties::FailureClass::Run,
        people::FailureClass::Agent => parties::FailureClass::Agent,
        people::FailureClass::Lost => parties::FailureClass::Lost,
        people::FailureClass::Invalid => parties::FailureClass::Invalid,
    }
}

fn party_ending(value: people::Ending) -> parties::Ending {
    match value {
        people::Ending::Done => parties::Ending::Done,
        people::Ending::Failed => parties::Ending::Failed,
        people::Ending::Cancelled { by, at } => parties::Ending::Cancelled { by: party_ending_party(by), at },
        people::Ending::Decided { by, at } => parties::Ending::Decided { by, at },
        people::Ending::Deleted => parties::Ending::Deleted,
    }
}

fn party_ending_party(value: people::Party) -> parties::EndingParty {
    match value {
        people::Party::Person(person) => parties::EndingParty::Person(person),
        people::Party::Task(task) => parties::EndingParty::Task(task),
        people::Party::Deployment { project } => parties::EndingParty::Deployment { project },
    }
}

/// Independent consumers of contract documents, alongside the scripted peers.
pub use crate::watches::Parties as WatchParties;

/// Derive the admitted journal room after a peer fixture's final limits.
pub fn journal(limits: &mut root::Limits) {
    let room = core::room_max(&limits.core).expect("peer root room");
    let connectors = jig_test_connector::MAX_OUT * 2;
    limits.journal.writes = (room.writes + connectors).max(2000);
    limits.journal.held = ((room.held + connectors) * limits.journal.commits).max(2000);
}

// Words and a named release share one atomic party request.
fn say(project: u32, task: u64, words: Box<[u8]>, release: Option<u64>) -> people::Ask {
    match release {
        Some(revision) => {
            let mut operations = skein_lib::List::with_capacity(2);
            operations.push(people::Operation::Message { task, words }).expect("two word and release operations");
            operations
                .push(people::Operation::Release { task, revision, again: None })
                .expect("two word and release operations");
            people::Ask::Compound { project, operations }
        }
        None => people::Ask::Say { project, task, words },
    }
}

fn closing(project: u32, task: parties::Task, disposals: Box<[parties::DelegateChoice]>) -> people::Ask {
    let mut operations = skein_lib::List::with_capacity(u32::try_from(disposals.len() + 1).expect("scripted cohort"));
    for disposal in disposals {
        let task = task_number(disposal.task);
        let reason = b"Finish this chat.".as_slice().into();
        operations
            .push(match disposal.keep {
                parties::Keep::Cancel => people::Operation::Cancel { task, reason, grace: None },
                parties::Keep::MakeMine => people::Operation::Move { task, reason },
            })
            .expect("cohort room");
    }
    operations
        .push(people::Operation::Cancel {
            task: task_number(task),
            reason: b"Finish this chat.".as_slice().into(),
            grace: Some(Duration::from_secs(5)),
        })
        .expect("chat last");
    people::Ask::Compound { project, operations }
}
