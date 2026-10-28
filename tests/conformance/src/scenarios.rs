//! The shared scenarios bound to two test connectors and independent hosts.
mod conversations;
pub(crate) mod lifecycle;
pub(crate) mod queries;
use crate::{Config, Testing, actions::World};
use jig_conformance::referee::{Promise, Violation};
use jig_conformance::scenarios::{Action, Scenario, ScenarioApplication};
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_connector as connector;
use jig_test_domain as root;
use skein_lib::{ReplyTo, Token, Wall};

fn check(holds: bool, why: &str) -> Result<(), Violation> {
    if holds { Ok(()) } else { Err(Violation { promise: Promise::Lost, why: why.into() }) }
}

fn parent(world: &World) -> (u64, u64) {
    world.peers.assignments[0]
}

fn authority(world: &World) -> tasks::Authority {
    let task = parent(world).0;
    match world.store.rows.get(&root::Key::Core(core::Key::Tasks(tasks::Key::Live(task)))) {
        Some(root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row)))) => row.authority.clone(),
        row => panic!("scripted parent's durable authority: {row:?}"),
    }
}

const PARTY_REPLY_BASE: u64 = 6000;

fn ask(world: &mut World, key: u8, ask: people::Ask) {
    world.send(root::Event::Core(core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(PARTY_REPLY_BASE + u64::from(key))),
        sign_in: world.peers.peers.parties[0].sign_in.expect("scripted sign-in"),
        key: [key; 16],
        ask,
    })));
}

fn effect(world: &mut World, completion: u32, proposal: bool) {
    let (task, attempt) = parent(world);
    let mut effect = jig_core_world::effects::World::effect();
    if world.peers.scenario == Some(Scenario::OtherHand) {
        effect.condition = Some(0);
    }
    world.send(root::Event::EffectCall {
        to: ReplyTo::new(Token::new(7000 + u64::from(completion))),
        key: core::CallKey { task, attempt, completion, position: 0 },
        number: 1,
        effect,
        deadline: Wall::from_nanos(world.clock().now + 1_000_000_000),
        proposal: proposal.then(|| Box::from(&b"ask the holder"[..])),
    });
}

fn named_part(world: &World, completion: u32) -> Option<&core::CallPart> {
    let (task, attempt) = parent(world);
    let key = core::CallKey { task, attempt, completion, position: 0 };
    match world.store.rows.get(&root::Key::Core(core::Key::Core(core::CoreKey::Call(key)))) {
        Some(root::Record::Core(core::Record::Core(core::CoreRecord::Call(row)))) => Some(&row.part),
        Some(root::Record::Core(_) | root::Record::Connector { .. }) | None => None,
    }
}

fn pool(world: &mut World, slots: u32) {
    world.send(root::Event::Connector {
        number: 1,
        event: connector::Event::System(connector::SystemEvent::Pool {
            path: jig_test_connector_world::path(1, 2),
            slots,
            lost: Box::new([]),
        }),
    });
}

fn release_writes(world: &mut World) {
    world.peers.hold_writes = false;
    for (number, call) in std::mem::take(&mut world.peers.pending_writes) {
        let event = world.systems[usize::from(number - 1)].answer(call, jig_test_system::Fault::None);
        world.send(root::Event::Connector { number, event: connector::Event::System(event) });
    }
}

fn message(world: &mut World, task: u64) {
    let (task, attempt) =
        *world.peers.assignments.iter().rev().find(|(number, _)| *number == task).expect("independent assignment");
    let worker = world
        .peers
        .peers
        .workers
        .iter_mut()
        .find(|worker| worker.seen.iter().any(|run| run.task == task && run.attempt == attempt))
        .expect("assigned host");
    worker.message(task, attempt, 100, b"complete scripted work".as_slice().into());
}

fn ended(world: &World, task: u64) -> bool {
    world.store.rows.contains_key(&root::Key::Core(core::Key::Tasks(tasks::Key::Ended(task))))
}

impl ScenarioApplication for Testing {
    fn configuration(scenario: Scenario, seed: u64) -> Option<Config> {
        Some(Config::new(
            seed,
            false,
            if matches!(
                scenario,
                Scenario::Compound
                    | Scenario::ShrinkingPool
                    | Scenario::ConversationEndsWhileRead
                    | Scenario::QueryWatchedWhileTasksEnd
            ) {
                3
            } else {
                2
            },
            Some(scenario),
        ))
    }

    fn action(world: &mut World, action: Action) -> Result<(), Violation> {
        let mut continuation = Continuation::new(action);
        while continuation.advance(world)? {
            world.drain()?;
        }
        Ok(())
    }

    fn check(world: &World, scenario: Scenario) -> Result<(), Violation> {
        let applied = world.systems[0].observed().iter().filter(|effect| effect.applied).count();
        match scenario {
            Scenario::Compound | Scenario::Grace => lifecycle::check_ending(world, scenario),
            Scenario::QueryWatchedWhileTasksEnd => queries::check_ending(world),
            Scenario::ConversationEndsWhileRead => conversations::check_ending(world),
            Scenario::RacingDecisions => crate::actions::check_racing_decisions(world),
            Scenario::LateCopy => check(
                applied == 1 && world.systems[0].observed().len() == 2,
                "late and retried copies did not make one effect",
            ),
            Scenario::OtherHand => check(
                applied == 0 && world.systems[0].state(&jig_test_connector_world::path(1, 1)) == Some(13),
                "conditional retry overwrote another participant",
            ),
            Scenario::UncertainRestarts => {
                check(applied == 1, "repeated cold starts did not resolve the uncertain entry")
            }
            Scenario::ChangedJudge => check(
                applied == 1
                    && world.peers.calls.iter().any(|part| matches!(part, core::CallPart::EffectDenied { .. })),
                "a new effect ignored the changed judge",
            ),
            Scenario::NarrowingPolicy => check(
                applied == 1
                    && world.peers.pending_proposal.is_some_and(|number| {
                        world.store.rows.contains_key(&root::Key::Connector {
                            number: 1,
                            key: connector::RecordKey::Proposal(number),
                        })
                    }),
                "narrowing changed a committed flight or consumed its waiting proposal",
            ),
            Scenario::ShrinkingPool => check(
                world.peers.assignments.len() == 4 && world.peers.children.iter().all(|task| ended(world, *task)),
                "pool did not retain its holders and eventually serve its waiter",
            ),
            Scenario::StandingTask => {
                check(world.peers.assignments.len() == 6, "standing task did not run across five periods")
            }
            Scenario::RunBudget => {
                let spent: u64 = world.peers.peers.workers.iter().map(|worker| worker.actual_spent).sum();
                check(
                    spent == 20 && world.peers.peers.workers.iter().map(|worker| worker.turn_acks).sum::<u32>() == 12,
                    &format!(
                        "sessions and sub-agents did not consume the one run allowance: spent={spent}, acks={}, assignments={:?}",
                        world.peers.peers.workers.iter().map(|worker| worker.turn_acks).sum::<u32>(),
                        world.peers.assignments
                    ),
                )
            }
        }
    }
}

/// Concrete script position across nested drains, with every live local retained.
/// Both fresh actions and cold-cut continuations execute this same code.
#[derive(Clone, Debug)]
pub(crate) struct Continuation {
    action: Action,
    phase: u8,
    completion: u32,
    tries: u8,
    task: u64,
    before: usize,
}
impl Continuation {
    pub(crate) fn new(action: Action) -> Self {
        Self { action, phase: 0, completion: 1, tries: 0, task: 0, before: 0 }
    }

    // Phases 1–3 retain wake's original two conditional waits and message.
    fn wake(&mut self, world: &mut World) -> bool {
        if self.phase == 1 {
            if world.peers.assignments.iter().any(|(number, _)| *number == self.task) {
                self.phase = 3;
            } else {
                world.advance(6_000_000_000);
                self.phase = 2;
                return true;
            }
        }
        if self.phase == 2 {
            world.advance(1_000_000_000);
            self.phase = 3;
            return true;
        }
        message(world, self.task);
        self.phase = 4;
        false
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one concrete exhaustive continuation preserves every nested action drain"
    )]
    pub(crate) fn advance(&mut self, world: &mut World) -> Result<bool, Violation> {
        loop {
            match self.action {
                Action::Compound | Action::CompoundAgain | Action::CancelWithGrace | Action::Finish => {
                    return lifecycle::advance(world, self.action, &mut self.phase);
                }
                Action::WatchQuery | Action::EndTask | Action::ReadEnded => {
                    return queries::advance(world, self.action, &mut self.phase);
                }
                Action::Conversation { .. } | Action::Turn | Action::MarkTurns | Action::MarkResults => {
                    conversations::action(world, self.action);
                    return Ok(false);
                }
                Action::EndConversations => {
                    if self.phase < 2 {
                        conversations::finish(world, usize::from(self.phase));
                        self.phase += 1;
                        return Ok(true);
                    }
                    return Ok(false);
                }
                Action::PartyAmend => {
                    if self.phase == 0 {
                        crate::actions::prepare_racing_decisions(world);
                        self.phase = 1;
                        return Ok(true);
                    }
                    let core::CallPart::Delegated(numbers) = named_part(world, 1).expect("delegated race target")
                    else {
                        panic!("race delegation refused");
                    };
                    let task = numbers[0].task;
                    world.peers.children.push(task);
                    ask(
                        world,
                        90,
                        people::Ask::Amend {
                            project: 1,
                            task,
                            revision: 0,
                            release: None,
                            amendment: people::Amendment {
                                spec: Some(people::Spec {
                                    asks: false,
                                    words: b"party choice".as_slice().into(),
                                    parameters: Box::new([]),
                                    inputs: Box::new([]),
                                }),
                                ..jig_core_world::fixtures::person_unchanged(b"party evidence")
                            },
                        },
                    );
                    return Ok(false);
                }
                Action::RunAmend => {
                    crate::actions::run_racing_amendment(world);
                    return Ok(false);
                }
                Action::Ready => match self.phase {
                    0 if world.peers.assignments.is_empty() => {
                        world.advance(6_000_000_000);
                        self.phase = 1;
                        return Ok(true);
                    }
                    1 => {
                        world.advance(1_000_000_000);
                        self.phase = 2;
                        return Ok(true);
                    }
                    _ => {
                        check(!world.peers.assignments.is_empty(), "caller was not assigned")?;
                        if world.peers.scenario == Some(Scenario::Compound)
                            && lifecycle::prepare(world, &mut self.phase)?
                        {
                            return Ok(true);
                        }
                        world.peers.initial_assignments = world.peers.assignments.len();
                        world.peers.lifecycle.ready = world.outcome().commits;
                        return Ok(false);
                    }
                },
                Action::FillPool => match self.phase {
                    0 => {
                        world.send(root::Event::Connector {
                            number: 1,
                            event: connector::Event::Adopt {
                                project: 1,
                                resource: jig_test_connector_world::path(1, 2),
                                role: connector::ResourceRole::Owned,
                            },
                        });
                        pool(world, 2);
                        self.phase = 1;
                        return Ok(true);
                    }
                    1 => {
                        let completion = self.completion;
                        let mut authority = authority(world);
                        authority.budget.spend = 10;
                        authority.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
                        let (task, attempt) = parent(world);
                        world.send(root::Event::Core(core::Event::DelegateValidated {
                            to: ReplyTo::new(Token::new(8000 + u64::from(completion))),
                            key: core::CallKey { task, attempt, completion, position: 0 },
                            batch: Box::new([core::Delegate {
                                deadline: jig_domain_tasks::Deadline::Named,
                                label: 0,
                                flags: jig_domain_tasks::Flags::PLAIN,
                                tracked: None,
                                budget: jig_domain_people::Budget::Named,
                                executor: tasks::Executor::Agent { charter: 1 },
                                spec: tasks::Spec {
                                    asks: false,
                                    words: b"pool work".as_slice().into(),
                                    parameters: Box::new([tasks::Parameter::Resource {
                                        name: 1,
                                        connector: 1,
                                        resource: 2,
                                    }]),
                                    inputs: Box::new([]),
                                },
                                contract: tasks::Contract::Report { words: 128 },
                                authority,
                                llm: None,
                                symbolic_grants: Box::new([]),
                                dependencies: Box::new([]),
                                wake: tasks::WakePolicy::DEFAULT,
                            }]),
                            stubs: Box::new([]),
                        }));

                        self.tries += 1;
                        self.phase = 2;
                        return Ok(true);
                    }
                    2 => {
                        if named_part(world, self.completion).is_none() && self.tries < 2 {
                            self.phase = 1;
                            continue;
                        }
                        let core::CallPart::Delegated(members) =
                            named_part(world, self.completion).expect("durable delegate reply")
                        else {
                            panic!("delegate refused: {:?}", named_part(world, self.completion));
                        };
                        world.peers.children.push(members[0].task);
                        self.completion += 1;
                        self.tries = 0;
                        self.phase = if self.completion <= 3 { 1 } else { 3 };
                    }
                    3 if world
                        .peers
                        .children
                        .iter()
                        .filter(|child| world.peers.assignments.iter().any(|(task, _)| task == *child))
                        .count()
                        < 2 =>
                    {
                        world.advance(6_000_000_000);
                        self.phase = 4;
                        return Ok(true);
                    }
                    4 => {
                        world.advance(1_000_000_000);
                        self.phase = 5;
                        return Ok(true);
                    }
                    _ => {
                        check(world.peers.assignments.len() == 3, "two holders and parent were not assigned")?;
                        return Ok(false);
                    }
                },
                Action::FinishHolder(index) => {
                    if self.phase == 0 {
                        self.task = world.peers.children[usize::try_from(index).expect("three holders")];
                        self.phase = 1;
                    }
                    if self.phase < 4 && self.wake(world) {
                        return Ok(true);
                    }
                    if self.phase == 4 {
                        self.phase = 5;
                        return Ok(true);
                    }
                    check(ended(world, self.task), "pool holder did not end")?;
                    if index == 0 {
                        check(world.peers.assignments.len() == 3, "shrinking pool admitted its waiter too soon")?;
                    }
                    return Ok(false);
                }
                Action::Propose => match self.phase {
                    0 => {
                        effect(world, 2, true);
                        self.phase = 1;
                        return Ok(true);
                    }
                    1 if named_part(world, 2).is_none() => {
                        effect(world, 2, true);
                        self.phase = 2;
                        return Ok(true);
                    }
                    _ => {
                        let core::CallPart::Proposed { proposal, .. } =
                            named_part(world, 2).expect("durable proposal reply")
                        else {
                            panic!("proposal refused: {:?}", world.peers.calls);
                        };
                        world.peers.pending_proposal = Some(*proposal);
                        return Ok(false);
                    }
                },
                Action::StartStanding => {
                    if self.phase == 0 {
                        let (task, attempt) = parent(world);
                        let worker = world
                            .peers
                            .peers
                            .workers
                            .iter_mut()
                            .find(|worker| worker.seen.iter().any(|run| run.task == task))
                            .expect("parent host");
                        worker.cancel(task, attempt);
                        self.phase = 1;
                        return Ok(true);
                    }
                    let mut authority = authority(world);
                    authority.budget.spend = 30;
                    authority.delegation = tasks::Delegation {
                        kinds: Box::new([tasks::AuthorityExecutor::Charter(1)]),
                        tasks: 1,
                        depth: 1,
                    };
                    let mut child = authority.clone();
                    child.budget.spend = 10;
                    child.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
                    world.send(root::Event::Core(core::Event::StartRecurring {
                        project: 1,
                        authority,
                        template: tasks::RecurringTemplate {
                            key: 1,
                            overlap: tasks::RecurringOverlap::Skip,
                            batch: Box::new([tasks::New {
                                deadline: jig_domain_tasks::Deadline::Named,
                                label: 0,
                                flags: jig_domain_tasks::Flags::PLAIN,
                                number: 1,
                                project: 1,
                                executor: tasks::Executor::Agent { charter: 1 },
                                llm: None,
                                spec: tasks::Spec {
                                    asks: false,
                                    words: b"period work".as_slice().into(),
                                    parameters: Box::new([]),
                                    inputs: Box::new([]),
                                },
                                contract: tasks::Contract::Report { words: 128 },
                                authority: child,
                                numbers: tasks::Numbers { budget: 10, spent: 0, spent_below: 0, reserved: 0 },
                                funder: tasks::Funder::Period { project: 1, period: 0 },
                                dependencies: Box::new([]),
                                holdings: Box::new([]),
                                wake: tasks::WakePolicy::DEFAULT,
                                recurring: None,
                                tracked: None,
                            }]),
                        },
                    }));
                    return Ok(false);
                }
                Action::Period(period) => {
                    if self.phase == 0 {
                        self.task = world
                            .store
                            .rows
                            .values()
                            .filter_map(|row| match row {
                                root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row)))
                                    if row.spec.words.as_ref() == b"period work" =>
                                {
                                    Some(row.number)
                                }
                                root::Record::Core(_) | root::Record::Connector { .. } => None,
                            })
                            .max()
                            .expect("durable standing batch");
                        self.phase = 1;
                    }
                    if self.phase < 4 && self.wake(world) {
                        return Ok(true);
                    }
                    match self.phase {
                        4 => {
                            self.phase = 5;
                            return Ok(true);
                        }
                        5 => {
                            check(ended(world, self.task), "standing batch did not finish")?;
                            self.before = world.peers.assignments.len();
                            world.send(root::Event::Core(core::Event::Period {
                                project: 1,
                                period,
                                budget: 1000,
                                next: None,
                            }));
                            self.phase = 6;
                            return Ok(true);
                        }
                        6 => {
                            world.send(root::Event::Core(core::Event::Period {
                                project: 1,
                                period,
                                budget: 1000,
                                next: None,
                            }));
                            self.phase = 7;
                            return Ok(true);
                        }
                        7 if world.peers.assignments.len() == self.before => {
                            world.advance(6_000_000_000);
                            self.phase = 8;
                            return Ok(true);
                        }
                        8 => {
                            world.advance(1_000_000_000);
                            self.phase = 9;
                            return Ok(true);
                        }
                        _ => {
                            check(
                                world.peers.assignments.len() == self.before + 1,
                                "period did not make exactly one batch",
                            )?;
                            return Ok(false);
                        }
                    }
                }
                Action::Resume => {
                    if self.phase == 0 {
                        self.task = parent(world).0;
                        self.phase = 1;
                    }
                    if self.phase < 4 && self.wake(world) {
                        return Ok(true);
                    }
                    return Ok(false);
                }
                Action::Late
                | Action::Effect(_)
                | Action::DeliverLate
                | Action::OtherHand
                | Action::HoldWrites
                | Action::ReleaseWrites
                | Action::CheckDeadline
                | Action::ChangeJudge
                | Action::ShrinkPool
                | Action::NarrowPolicy
                | Action::AcceptProposal => {
                    atomic(world, self.action)?;
                    return Ok(false);
                }
            }
        }
    }
}

fn atomic(world: &mut World, action: Action) -> Result<(), Violation> {
    match action {
        Action::Late => world.peers.next_fault = jig_test_system::Fault::Late,
        Action::Effect(completion) => effect(world, completion, false),
        Action::DeliverLate => {
            for index in 0..2 {
                while let Some(event) = world.systems[index].deliver_late() {
                    world.send(root::Event::Connector {
                        number: u16::try_from(index + 1).expect("two systems"),
                        event: connector::Event::System(event),
                    });
                }
            }
        }
        Action::OtherHand => world.systems[0].other_hand(&jig_test_connector_world::path(1, 1), 13),
        Action::HoldWrites => world.peers.hold_writes = true,
        Action::ReleaseWrites => release_writes(world),
        Action::CheckDeadline => check_deadline(world)?,
        Action::ChangeJudge => {
            world.systems[1].other_hand(&jig_test_connector_world::path(1, 1), 14);
            let event = world.systems[1].answer(
                connector::SystemRequest::ReadFact {
                    resource: jig_test_connector_world::path(1, 1),
                    observed: Wall::from_nanos(world.clock().now),
                },
                jig_test_system::Fault::None,
            );
            world.send(root::Event::Connector { number: 2, event: connector::Event::System(event) });
        }
        Action::ShrinkPool => {
            pool(world, 1);
        }
        Action::NarrowPolicy => ask(
            world,
            43,
            people::Ask::ChangePolicy {
                project: 1,
                change: people::PolicyChange::Requirements {
                    requirements: Box::new([people::Requirement {
                        connector: 1,
                        kind: 5,
                        pattern: people::Pattern { segments: Box::new([]), last: people::Last::Open(Box::new([])) },
                        judge: people::Judge { connector: 2, requirement: 9, parameters: 0 },
                        guard: people::Guard::Guarded,
                        must_be_guarded: true,
                    }]),
                },
            },
        ),
        Action::AcceptProposal => {
            let proposal = world.peers.pending_proposal.expect("waiting proposal");
            ask(
                world,
                44,
                people::Ask::DecideProposal {
                    project: 1,
                    proposer: people::Proposer::Task(parent(world).0),
                    proposal,
                    decision: people::ProposalDecision::Accept,
                    running: None,
                },
            );
        }

        Action::Conversation { .. }
        | Action::Turn
        | Action::MarkTurns
        | Action::EndConversations
        | Action::MarkResults
        | Action::Compound
        | Action::CompoundAgain
        | Action::CancelWithGrace
        | Action::Finish
        | Action::Ready
        | Action::FillPool
        | Action::FinishHolder(_)
        | Action::Propose
        | Action::StartStanding
        | Action::Period(_)
        | Action::Resume
        | Action::PartyAmend
        | Action::RunAmend
        | Action::WatchQuery
        | Action::EndTask
        | Action::ReadEnded => unreachable!("continued action"),
    }
    Ok(())
}

fn check_deadline(world: &mut World) -> Result<(), Violation> {
    let deadline = world
        .store
        .rows
        .values()
        .find_map(|row| match row {
            root::Record::Connector { number: 1, record: connector::Record::Outbox(entry) } => {
                entry.attempt.map(|attempt| attempt.deadline)
            }
            root::Record::Core(_) | root::Record::Connector { .. } => None,
        })
        .expect("durable uncertain attempt");
    if let Some(saved) = world.peers.saved_deadline {
        check(deadline == saved, "cold restart changed the uncertain deadline")?;
        check(world.clock().now < saved.as_nanos(), "restarts passed the uncertain deadline")?;
    } else {
        world.peers.saved_deadline = Some(deadline);
    }
    Ok(())
}
