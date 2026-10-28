//! Routes between the core's children (domain/engine.md, 4.4). These
//! translations use only the children's vocabularies; the application root
//! carries their events and handles the resulting store and host requests.

use alloc::boxed::Box;
use jig_domain_accounts as accounts;
use jig_domain_authority as authority;
use jig_domain_fleet as fleet;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{Env, List, Queue, Token, Wall};

use crate::{CallKey, Core, Family, GoalRoute, Limits, PersonProposalRoute, RoutedCall, fresh};

pub(crate) fn fleet_grant(grant: accounts::Grant) -> fleet::Grant {
    fleet::Grant {
        form: fleet_form(grant.form),
        account: grant.account,
        generation: grant.generation,
        valid: grant.valid,
    }
}

/// Destination of an accepted message after the task child saved it.
#[derive(Debug)]
pub enum SentRoute {
    /// A named host call receives its durable answer.
    Call { key: CallKey, message: u64 },
    /// A party's keyed request receives its answer from the people child.
    Person(Box<people::Event>),
}

/// The next core route after the task hub admitted a numbered batch.
#[derive(Debug)]
pub enum MadeRoute {
    /// Internal connector or deployment admission needs no keyed reply.
    Internal,
    /// The accepted goal needs its semantic proposal decision.
    Tasks(tasks::Event),
    /// A person proposal needs its semantic task decision.
    PersonProposal(tasks::Event),
    /// A person request receives its keyed result.
    Person { event: Box<people::Event>, stubs: Box<[tasks::Stub]> },
    /// A named run call receives its batch, after remembering historical stubs.
    Delegated { key: CallKey, tasks: Box<[u64]>, stubs: Box<[tasks::Stub]>, given: Option<people::Given> },
}

/// One authenticated person word awaiting a durable task admission.
#[derive(Debug)]
pub struct PersonMessage {
    pub request: Token,
    pub person: u64,
    pub project: u32,
    pub task: u64,
    pub question: Option<u64>,
    pub words: Box<[u8]>,
    pub at: Wall,
}

impl Core {
    /// Number an authenticated person's word and route it to the task hub,
    /// retaining the keyed reply correlation until admission completes.
    pub fn person_message(&mut self, limits: &Limits, message: PersonMessage) -> Result<tasks::Event, people::Refusal> {
        let PersonMessage { request, person, project, task, question, words, at } = message;
        let row = self.tasks.task(task).ok_or(people::Refusal::Unknown)?;
        if row.project != project {
            return Err(people::Refusal::Unknown);
        }
        match question {
            Some(number) => crate::acts::question(self, limits, person, crate::acts::Act::Answer, task, number)?,
            None => crate::acts::write(self, limits, person, task)?,
        }
        let Some(message) = fresh(&mut self.counters, Family::Message) else {
            return Err(people::Refusal::Limit { which: people::LimitKind::Arithmetic });
        };
        let kind = match question {
            Some(number) => tasks::MessageKind::Answer { question: number },
            None => tasks::MessageKind::Words,
        };
        let destination = match question {
            Some(question) => self.tasks.question_asker(task, person, question).ok_or(people::Refusal::Unknown)?,
            None => task,
        };
        let notice = match question {
            Some(_) => fresh(&mut self.counters, Family::Message)
                .ok_or(people::Refusal::Limit { which: people::LimitKind::Arithmetic })?,
            None => 0,
        };
        assert!(self.saying.insert(request, (destination, question)) == Ok(None), "one person message flight");
        let event = match question {
            Some(question) => tasks::Event::PersonQuestionAnswer {
                reply_to: skein_lib::ReplyTo::new(request),
                project,
                asked: task,
                person,
                question,
                notice,
                word: tasks::Word {
                    number: message,
                    from: tasks::Party::Person(person),
                    kind,
                    words,
                    at,
                    hits: 1,
                    eligible: false,
                },
            },
            None => tasks::Event::AuthorizedMessage {
                reply_to: skein_lib::ReplyTo::new(request),
                project,
                task,
                word: tasks::Word {
                    number: message,
                    from: tasks::Party::Person(person),
                    kind,
                    words,
                    at,
                    hits: 1,
                    eligible: false,
                },
            },
        };
        Ok(event)
    }

    /// Project a person's durable goal proposal into the party inbox.
    #[must_use]
    pub fn person_proposal_entries(&self, row: &tasks::PersonProposal) -> Box<[people::Entry]> {
        let mut entries = List::with_capacity(self.authority.limits().roles.max(1));
        if row.action.is_settings() {
            if let Some(since) = row.state.pending_since() {
                entries
                    .push(people::Entry {
                        task: tasks::PROPOSAL_WITHOUT_TASK,
                        project: 0,
                        whom: people::Whom::Role { project: 0, role: people::Role::Owner.number() },
                        kind: people::EntryKind::Proposal { number: row.number },
                        at: since,
                    })
                    .expect("one owner recipient");
            }
            return entries.into_boxed();
        }
        match &row.state {
            tasks::PersonProposalState::Pending { since } => {
                if let Some(project) = self.authority.policy(row.project) {
                    for policy in &project.roles {
                        let kind = match row.action {
                            tasks::PersonAction::Settings { .. } => {
                                unreachable!("settings recipient is the deployment owner")
                            }
                            tasks::PersonAction::Batch(_) => authority::ProposalKind::Batch,
                            tasks::PersonAction::Amend { .. } => authority::ProposalKind::Amend,
                        };
                        if !policy.decides.allows(kind) {
                            continue;
                        }
                        entries
                            .push(people::Entry {
                                task: row.first(),
                                project: row.project,
                                whom: people::Whom::Role { project: row.project, role: policy.number },
                                kind: people::EntryKind::Proposal { number: row.number },
                                at: *since,
                            })
                            .expect("one entry per policy role");
                    }
                }
            }
            tasks::PersonProposalState::Rejected { .. }
            | tasks::PersonProposalState::SettingsAccepted { .. }
            | tasks::PersonProposalState::Accepted { .. }
            | tasks::PersonProposalState::Withdrawn { .. } => {}
        }
        entries.into_boxed()
    }

    /// A lost channel changes fleet topology without an external output.
    pub fn lost_channel(&mut self, env: &Env<Limits>, channel: Token) {
        let mut out = Queue::with_capacity(fleet::max_out(&env.limits.fleet));
        fleet::step(
            &mut self.fleet,
            &Env { now: env.now, wall: env.wall, limits: env.limits.fleet },
            fleet::Event::Lost { channel },
            &mut out,
        );
        assert!(out.is_empty(), "loss changes topology and deadlines without child effects");
        self.touch_deployment();
    }

    /// Route a restored task's current claim to views and the fleet, holding
    /// host adoption until the application's live-range load finishes.
    pub fn adopt(&mut self, env: &Env<Limits>, task: u64, attempt: u64, kept: u32) {
        assert!(self.unreported_restored.insert(task, attempt) == Ok(None), "one restored claim per task");
        let proof = self.proofs.get(&task).expect("restored claim proof");
        let kind = if self.workers_in_process { fleet::HostKind::Engine } else { proof.host };
        let mut worked = kept > 0;
        for (key, _) in &self.call_parts {
            if key.task == task && key.attempt == attempt {
                worked = true;
            }
        }
        if proof.crossed && kind == fleet::HostKind::Worker {
            self.adopted.push(fleet::Event::Cross {
                reply_to: skein_lib::ReplyTo::new(Token::new(task)),
                run: Token::new(task),
                attempt: Token::new(attempt),
                worked,
            });
            return;
        }
        let mut grants = List::with_capacity(env.limits.fleet.grants);
        let mut holds = List::with_capacity(env.limits.fleet.grants);
        for binding in &self.proofs.get(&task).expect("restored claim proof").accounts {
            let Some(source) = self.accounts.source(binding.source) else { continue };
            if source.account != binding.account {
                continue;
            }
            if source.kind == accounts::Kind::None {
                continue;
            }
            let table = match source.kind {
                accounts::Kind::Program => fleet::GrantTable::Name,
                accounts::Kind::SignIn | accounts::Kind::Key | accounts::Kind::None => fleet::GrantTable::Accounts,
            };
            holds.push(fleet::GrantName { table, account: binding.source }).expect("provider names");
            if let Some(grant) = self.accounts.source_grant(binding.source, env.now) {
                grants.push(fleet_grant(grant)).expect("provider grants");
            }
        }
        for number in &self.proofs.get(&task).expect("restored claim proof").credentials {
            holds
                .push(fleet::GrantName { table: fleet::GrantTable::Application, account: *number })
                .expect("credential name room");
            if let Some(grant) = self.accounts.credential_grant(*number, env.now) {
                grants.push(fleet_grant(grant)).expect("credential grant room");
            }
        }
        self.adopted.push(fleet::Event::Adopt {
            holds: holds.into_boxed(),
            grants: grants.into_boxed(),
            kind,
            worked,
            reply_to: skein_lib::ReplyTo::new(Token::new(task)),
            run: Token::new(task),
            attempt: Token::new(attempt),
            kept,
        });
    }

    /// Translate the task hub's stop into the fleet's fenced cancellation.
    #[must_use]
    pub fn stop_run(task: u64, attempt: u64) -> fleet::Event {
        fleet::Event::Cancel { run: Token::new(task), attempt: Token::new(attempt) }
    }

    /// Route a refused goal flight back to the requesting party. The hub's
    /// structural reasons are translated at the core's sibling boundary.
    pub fn goal_refused(&mut self, request: Token, why: tasks::Refusal) -> Option<people::Event> {
        self.goal_routes.remove(&request)?;
        let outcome = crate::routing::refusals::person_task_refusal(self, None, why);
        Some(people::Event::Decided { request, outcome })
    }

    /// Advance a successful batch to the child or caller that owns its
    /// continuation. The root only carries the returned event or answer.
    pub fn made(&mut self, request: Token, tasks: Box<[u64]>, person_proposal: bool) -> MadeRoute {
        if request.raw() >= u64::MAX - 3 {
            return MadeRoute::Internal;
        }
        if let Some(GoalRoute::Accepting { proposer, proposal, by }) = self.goal_routes.get(&request).cloned() {
            let row = self.tasks.person_proposal(proposer, proposal).expect("accepted party proposal");
            let tasks::PersonAction::Batch(batch) = &row.action else {
                unreachable!("only a party batch makes tasks");
            };
            assert!(batch.len() == tasks.len(), "accepted batch length");
            for (member, number) in batch.iter().zip(tasks.iter()) {
                assert!(member.number == *number, "accepted batch made");
            }
            let inserted = self.goal_routes.insert(request, GoalRoute::Deciding { proposer, proposal, by });
            assert!(inserted.is_ok(), "goal route replaced");
            return MadeRoute::Tasks(tasks::Event::DecidePersonProposal {
                reply_to: skein_lib::ReplyTo::new(request),
                proposer,
                proposal,
                by: tasks::Party::Person(by),
                message: None,
                decision: tasks::ProposalDecision::Accept,
                revision: None,
            });
        }
        let person_route = if person_proposal { self.routing_people_proposals.remove(&request) } else { None };
        if let Some(PersonProposalRoute::Accepting { request: named, person, proposer, proposal, message }) =
            person_route
        {
            assert!(request == named, "person acceptance correlation");
            self.routing_people_proposals
                .insert(request, PersonProposalRoute::Deciding { request, proposer, proposal, by: person, moved: None })
                .expect("person route room");
            return MadeRoute::PersonProposal(tasks::Event::DecideProposal {
                reply_to: skein_lib::ReplyTo::new(request),
                proposer,
                proposal,
                message: Some(message),
                by: tasks::Party::Person(person),
                decision: tasks::ProposalDecision::Accept,
            });
        }
        if let Some(RoutedCall::Accepting { key, proposer, proposal, message }) = self.routing_calls.remove(&request) {
            self.routing_calls.insert(request, RoutedCall::Decide { key, proposal }).expect("acceptance route room");
            return MadeRoute::Tasks(tasks::Event::DecideProposal {
                reply_to: skein_lib::ReplyTo::new(request),
                proposer,
                proposal,
                message: Some(message),
                by: tasks::Party::Task(key.task),
                decision: tasks::ProposalDecision::Accept,
            });
        }
        match self.delegating.remove(&request) {
            Some((key, stubs, given)) => MadeRoute::Delegated { key, tasks, stubs, given },
            None => {
                let crate::routing::Made { first: expected, given, stubs } =
                    self.made.remove(&request).expect("pending make route");
                MadeRoute::Person {
                    stubs,
                    event: Box::new(people::Event::Decided {
                        request,
                        outcome: people::Outcome::Created {
                            first: expected,
                            given,
                            last: *tasks.last().expect("nonempty created batch"),
                        },
                    }),
                }
            }
        }
    }

    /// Translate the task child's successful person-origin proposal into the
    /// people child's keyed reply, consuming the in-flight correlation.
    pub fn person_proposed(&mut self, request: Token, proposal: u64) -> (people::Event, Box<[tasks::Stub]>) {
        let route = self.goal_routes.remove(&request).expect("person proposal route");
        let GoalRoute::Offering { proposal: named, to, stubs } = route else {
            unreachable!("proposal admission route")
        };
        assert!(named == proposal, "proposal identity");
        (people::Event::Decided { request, outcome: people::Outcome::Proposed { proposal, to } }, stubs)
    }

    /// Translate the task child's person-origin proposal decision into the
    /// people child's keyed reply.
    pub fn person_proposal_decided(
        &mut self,
        request: Token,
        proposer: u64,
        number: u64,
        outcome: tasks::ProposalOutcome,
        at: Wall,
    ) -> people::Event {
        let route = self.goal_routes.remove(&request).expect("goal decision route");
        let (named, proposal, by) = match route {
            GoalRoute::Withdrawing { proposer: named, proposal, moved } => {
                assert!(named == proposer && proposal == number, "withdrawal identity");
                return people::Event::Decided {
                    request,
                    outcome: match moved {
                        Some(now) => people::Outcome::Refused(people::Refusal::Moved { now }),
                        None => people::Outcome::Withdrawn { proposal: number },
                    },
                };
            }
            GoalRoute::Deciding { proposer, proposal, by } => (proposer, proposal, by),
            GoalRoute::Offering { .. } | GoalRoute::Accepting { .. } => unreachable!("proposal decision stage"),
        };
        assert!(named == proposer && proposal == number, "party decision identity");
        let choice = match outcome {
            tasks::ProposalOutcome::Accepted => people::ProposalChoice::Accepted,
            tasks::ProposalOutcome::Rejected => people::ProposalChoice::Rejected,
            tasks::ProposalOutcome::Passed => people::ProposalChoice::Passed,
            tasks::ProposalOutcome::Withdrawn => people::ProposalChoice::Withdrawn,
            tasks::ProposalOutcome::Stale => people::ProposalChoice::Stale,
        };
        people::Event::Decided {
            request,
            outcome: people::Outcome::ProposalDecided {
                proposer: people::Proposer::Party(proposer),
                proposal: number,
                by,
                at,
                choice,
                repeated: false,
            },
        }
    }

    /// Complete a task proposal decided on a person's keyed request. Other
    /// task proposal decisions continue to the named host-call route.
    pub fn task_proposal_decided_for_person(
        &mut self,
        request: Token,
        proposer: u64,
        number: u64,
        outcome: tasks::ProposalOutcome,
        at: Wall,
        person_proposal: bool,
    ) -> Option<people::Event> {
        let route = if person_proposal { self.routing_people_proposals.remove(&request) } else { None };
        match route {
            Some(PersonProposalRoute::Deciding { request: named, proposer: expected, proposal, by, moved })
                if expected == proposer && proposal == number =>
            {
                if let Some(now) = moved {
                    assert!(outcome == tasks::ProposalOutcome::Withdrawn, "stale proposal withdrawn");
                    return Some(people::Event::Decided {
                        request: named,
                        outcome: people::Outcome::Refused(people::Refusal::Moved { now }),
                    });
                }
                let choice = match outcome {
                    tasks::ProposalOutcome::Accepted => people::ProposalChoice::Accepted,
                    tasks::ProposalOutcome::Rejected => people::ProposalChoice::Rejected,
                    tasks::ProposalOutcome::Passed => people::ProposalChoice::Passed,
                    tasks::ProposalOutcome::Withdrawn => people::ProposalChoice::Withdrawn,
                    tasks::ProposalOutcome::Stale => people::ProposalChoice::Stale,
                };
                Some(people::Event::Decided {
                    request: named,
                    outcome: people::Outcome::ProposalDecided {
                        proposer: people::Proposer::Task(proposer),
                        proposal: number,
                        by,
                        at,
                        choice,
                        repeated: false,
                    },
                })
            }
            Some(PersonProposalRoute::Deciding { .. } | PersonProposalRoute::Accepting { .. }) => {
                unreachable!("matching person proposal decision")
            }
            None => None,
        }
    }

    /// Number one subscribed state notice and route it back to the task hub.
    pub fn notice(
        &mut self,
        task: u64,
        subscription: u64,
        target: u64,
        state: tasks::NoticeState,
        words: Box<[u8]>,
        at: Wall,
    ) -> tasks::Event {
        let number = fresh(&mut self.counters, Family::Message).expect("notification number admitted");
        tasks::Event::Notice {
            task,
            word: tasks::Word {
                number,
                from: tasks::Party::Task(target),
                kind: tasks::MessageKind::Notice { subscription, target, state },
                words,
                at,
                hits: 1,
                eligible: false,
            },
        }
    }

    /// Number one due subscription timer and route it back to the task hub.
    pub fn notice_timer(&mut self, task: u64, subscription: u64, at: Wall) -> tasks::Event {
        let number = fresh(&mut self.counters, Family::Message).expect("timer number admitted");
        tasks::Event::Notice {
            task,
            word: tasks::Word {
                number,
                from: tasks::Party::Task(task),
                kind: tasks::MessageKind::Timer { subscription },
                words: Box::new([]),
                at,
                hits: 1,
                eligible: false,
            },
        }
    }

    /// Keep the newly committed word in a preparing run's brief and route
    /// the terminal to the caller who asked the task child to send it.
    pub fn sent(
        &mut self,
        request: Token,
        task: u64,
        word: &tasks::Word,
        inbox_messages: u32,
        tasks: u32,
    ) -> SentRoute {
        if let Some(context) = self.contexts.get_mut(&task) {
            let capacity = inbox_messages
                .checked_add(tasks.checked_mul(2).expect("two decision kinds per task"))
                .expect("validated virtual proposal room");
            let mut inbox = List::with_capacity(capacity);
            for old in &context.inbox {
                inbox.push(old.clone()).expect("accepted inbox count");
            }
            inbox.push(word.clone()).expect("accepted inbox count");
            context.inbox = inbox.into_boxed();
            context.last_message = word.number;
        }
        match self.routing_calls.remove(&request) {
            Some(RoutedCall::Message(key)) => SentRoute::Call { key, message: word.number },
            Some(
                RoutedCall::Introduce(_)
                | RoutedCall::Propose { .. }
                | RoutedCall::Decide { .. }
                | RoutedCall::Escalation { .. }
                | RoutedCall::Withdraw { .. }
                | RoutedCall::Accepting { .. }
                | RoutedCall::Subscribe { .. }
                | RoutedCall::Unsubscribe(_)
                | RoutedCall::Control { .. },
            ) => unreachable!("only message calls produce Sent"),
            None => {
                let route = self.saying.remove(&request);
                let outcome = match route {
                    Some((named, None)) if named == task => people::Outcome::Said { task, message: word.number },
                    Some((named, Some(question))) if named == task => {
                        let asked = match self.people.pending_ask(request) {
                            Some(people::Ask::AnswerQuestion { task, .. }) => *task,
                            Some(_) | None => task,
                        };
                        people::Outcome::QuestionAnswered { task: asked, question, message: word.number }
                    }
                    Some(_) | None => unreachable!("matching pending person message flight"),
                };
                SentRoute::Person(Box::new(people::Event::Decided { request, outcome }))
            }
        }
    }
}

fn fleet_form(form: accounts::Form) -> fleet::Form {
    match form {
        accounts::Form::Credential(scheme) => fleet::Form::Credential(match scheme {
            accounts::Scheme::Bearer => fleet::Scheme::Bearer,
            accounts::Scheme::Key => fleet::Scheme::Key,
        }),
        accounts::Form::Application => fleet::Form::Application,
        accounts::Form::Program(name) => fleet::Form::Program(name),
    }
}
