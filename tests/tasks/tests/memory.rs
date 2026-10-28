use jig_domain_tasks::{
    self as tasks, Active, AuthorityExecutor, Contract, Domain, End, Event, Funder, Grant, Hold, Last, Limits,
    Parameter, Party, Pattern, Request, Stored, TaskResult, Verdict,
};
use jig_tasks_world::{LIMITS, task};
use skein_lib::{Env, Queue, ReplyTo, Time, Token, Wall};
use skein_world::domain::heap::{self, Meter};

#[global_allocator]
static HEAP: heap::Counting = heap::Counting;

struct Measured {
    d: Domain,
    env: Env<Limits>,
    out: Queue<Request>,
    meter: Meter,
    bound: u64,
    serial: u64,
    refused: bool,
}

impl Measured {
    fn new(limits: &Limits) -> Measured {
        let limits = *limits;
        let out = Queue::with_capacity(tasks::max_out(&limits));
        let meter = Meter::new();
        let d = jig_tasks_world::domain(&limits, 1);
        Measured {
            d,
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            out,
            meter,
            bound: tasks::worst_case(&limits).expect("admitted bounds"),
            serial: 0,
            refused: false,
        }
    }

    fn to(&mut self) -> ReplyTo {
        self.serial += 1;
        ReplyTo::new(Token::new(self.serial))
    }

    fn event(&mut self, event: Event) {
        self.meter.start();
        tasks::step(&mut self.d, &self.env, event, &mut self.out);
        let _history = self.d.capacity_refusals();
        let measured = self.meter.end();
        self.refused = false;
        while let Some(request) = self.out.pop() {
            self.refused |= matches!(request, Request::Refused { .. });
            drop(request);
        }
        self.meter.check(measured, self.bound, &self.env.limits);
        self.d.reclaim();
    }

    fn bootstrap(&mut self) {
        self.event(Event::Check);
        self.event(Event::Restored);
        for period in [0, 7] {
            let reply_to = self.to();
            self.event(Event::OpenPeriod { reply_to, project: 1, period, budget: 100_000 });
        }
        let reply_to = self.to();
        self.event(Event::CarvePool { reply_to, project: 1, person: 9, period: 7, budget: 100_000 });
    }

    fn fire(&mut self) {
        self.env.now = self.env.now.saturating_add(skein_lib::Duration::from_secs(1));
        self.env.wall = Wall::from_nanos(self.env.now.as_nanos());
        self.meter.start();
        tasks::fire(&mut self.d, &self.env, &mut self.out);
        let measured = self.meter.end();
        while let Some(request) = self.out.pop() {
            drop(request);
        }
        self.meter.check(measured, self.bound, &self.env.limits);
        self.d.reclaim();
    }

    fn claim(&mut self, number: u64) {
        let reply_to = self.to();
        self.event(Event::Prepare { reply_to, task: number });
        let reply_to = self.to();
        self.event(Event::Claim {
            account: None,
            reply_to,
            task: number,
            attempt: number,
            resumed: false,
            budget: 100,
            writes: Box::new([]),
        });
        self.event(Event::Started { task: number, attempt: number });
    }
}

#[test]
#[expect(clippy::too_many_lines, reason = "one allocation family fills task payloads and every graph phase")]
fn saturated_payloads_graph_backoff_held_closing_retirement_and_restore_fit() {
    for l in [
        Limits {
            tasks: 4,
            tree_tasks: 4,
            depth: 2,
            delegates: 3,
            batch: 4,
            dependencies: 3,
            inputs: 4,
            parameters: 4,
            ..LIMITS
        },
        Limits {
            tasks: 1,
            tree_tasks: 1,
            depth: 0,
            delegates: 0,
            batch: 1,
            dependencies: 0,
            inputs: 1,
            parameters: 1,
            facts: 0,
            ..LIMITS
        },
    ] {
        let mut m = Measured::new(&l);
        m.bootstrap();
        let batch = (1..=u64::from(l.tasks))
            .map(|number| {
                let mut new = filled(&l, number);
                new.funder = Funder::Pool { project: 1, person: 9, period: 7 };
                new
            })
            .collect::<Vec<_>>();
        let reply_to = m.to();
        m.event(Event::Make { reply_to, creator: Party::Person(1), batch: batch.into_boxed_slice() });
        assert!(!m.refused);
        for _ in 0..=l.room_refusals {
            let reply_to = m.to();
            m.event(Event::Make { reply_to, creator: Party::Person(1), batch: Box::new([filled(&l, 100)]) });
            assert!(m.refused);
        }
        assert_eq!(m.d.capacity_refusals().len(), l.room_refusals);
        assert_eq!(m.d.capacity_refusals_lost(), 1);
        for number in 1..=u64::from(l.tasks) {
            m.claim(number);
            let reply_to = m.to();
            m.event(Event::Activation {
                saved: None,
                reply_to,
                task: number,
                attempt: number,
                end: End::Refused,
                cause: tasks::Cause::Unpriced,
            });
            m.fire();
            let reply_to = m.to();
            m.event(Event::Prepare { reply_to, task: number });
            let reply_to = m.to();
            m.event(Event::Claim {
                account: None,
                reply_to,
                task: number,
                attempt: number + 10,
                resumed: false,
                budget: 100,
                writes: Box::new([]),
            });
            let reply_to = m.to();
            m.event(Event::Turn {
                refused: Box::new([]),
                reply_to,
                task: number,
                attempt: number + 10,
                turn: 1,
                read: None,
                offered: None,
                outstanding: Box::new([]),
                cumulative: 7,
            });
            for turn in 2..=l.inbox_messages {
                let reply_to = m.to();
                m.event(Event::Turn {
                    refused: Box::new([]),
                    reply_to,
                    task: number,
                    attempt: number + 10,
                    turn,
                    read: None,
                    offered: None,
                    outstanding: Box::new([]),
                    cumulative: 7,
                });
                assert!(!m.refused);
            }
            assert_eq!(m.d.task(number).expect("live task").replies.len(), l.inbox_messages as usize);
            let reply_to = m.to();
            m.event(Event::Activation {
                saved: None,
                reply_to,
                task: number,
                attempt: number + 10,
                end: End::Finished {
                    result: TaskResult::Verdict {
                        fields: Box::new([]),
                        code: 0,
                        words: vec![1; l.result_bytes as usize].into_boxed_slice(),
                    },
                    cancel_delegates: false,
                },
                cause: tasks::Cause::Priced { cumulative: 10 },
            });
            let mut remaining = l.hold_bytes as usize;
            let mut path = Vec::new();
            for segment in 0..l.hold_segments {
                let length = if segment + 1 == l.hold_segments { remaining } else { remaining / 2 };
                path.push(vec![1; length].into_boxed_slice());
                remaining -= length;
            }
            m.event(Event::HoldFrom {
                task: number,
                why: Hold::Drift,
                source: Box::new(tasks::HoldSource::Resource(tasks::Name {
                    connector: 1,
                    path: path.into_boxed_slice(),
                })),
            });
            assert!(matches!(
                &m.d.task(number).expect("live held task").phase,
                tasks::Phase::Held { source: Some(_), .. }
            ));
            m.event(Event::Hold { task: number, why: Hold::Effects });
            m.event(Event::EffectsSettled { task: number });
            m.event(Event::Settled { task: number });
        }
    }
}

#[test]
fn full_delegate_tree_dependency_edges_and_cold_restored_claims_fit() {
    let l = Limits { tasks: 4, tree_tasks: 4, delegates: 3, batch: 3, dependencies: 3, inputs: 0, ..LIMITS };
    let mut source = jig_tasks_world::World::new(2, l);
    source.make(Party::Person(1), vec![task(1, &[])]);
    source.claim(1, 1);
    source.make(Party::Task(1), vec![task(2, &[]), task(3, &[2]), task(4, &[2, 3])]);
    source.claim(2, 2);
    let mut m = Measured::new(&l);
    for row in source.records.values() {
        m.event(Event::Restore { record: row.clone() });
    }
    m.event(Event::Check);
    m.event(Event::Restored);
    let reply_to = m.to();
    m.event(Event::Activation {
        saved: None,
        reply_to,
        task: 1,
        attempt: 1,
        end: End::Finished { result: TaskResult::Report { words: Box::new([1]) }, cancel_delegates: true },
        cause: tasks::Cause::Unpriced,
    });
    let reply_to = m.to();
    m.event(Event::Activation {
        reply_to,
        task: 2,
        attempt: 2,
        end: End::Parked,
        saved: None,
        cause: tasks::Cause::Unpriced,
    });
    for task in [3, 4, 2, 1] {
        m.event(Event::EffectsSettled { task });
        m.event(Event::Settled { task });
    }
    assert!(tasks::worst_case(&Limits { tasks: u32::MAX, ..l }).is_none());
    let mut w = jig_tasks_world::World::new(3, l);
    w.make(Party::Person(1), vec![task(10, &[])]);
    w.claim(10, 10);
    w.finish(10);
    w.settle(10);
    assert!(
        matches!(w.records.get(&tasks::Key::Ended(10)),Some(Stored::Ended(record)) if record.phase!=tasks::Phase::Active(Active::Due))
    );
}

fn filled(limits: &Limits, number: u64) -> jig_domain_tasks::New {
    let limits = *limits;
    let mut new = task(number, &[]);
    new.spec.words = vec![1; usize::try_from(limits.spec_bytes).expect("small bound")].into_boxed_slice();
    new.spec.parameters =
        vec![Parameter::Number { name: 1, value: 1 }; usize::try_from(limits.parameters).expect("small bound")]
            .into_boxed_slice();

    new.contract = Contract::Verdict {
        choices: (0..limits.contract_choices)
            .map(|code| Verdict { code, words: limits.result_bytes, followups: 0, fields: Box::new([]) })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    };
    new.authority.grants = (0..limits.authority_grants)
        .map(|kind| Grant {
            connector: 0,
            kind: u16::try_from(kind).expect("small bound"),
            pattern: Pattern {
                segments: (0..limits.authority_segments)
                    .map(|_| Box::new([]) as Box<[u8]>)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                last: Last::Exact(if kind == 0 {
                    vec![1; usize::try_from(limits.authority_bytes).expect("small bound")].into_boxed_slice()
                } else {
                    Box::new([])
                }),
            },
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    new.authority.delegation.kinds =
        vec![AuthorityExecutor::Charter(1); usize::try_from(limits.executor_kinds).expect("small bound")]
            .into_boxed_slice();
    new
}

#[test]
fn saturated_finite_sources_and_oversized_refusals_fit_without_input_copies() {
    let l = Limits { tasks: 1, funders: 3, ..LIMITS };
    let mut m = Measured::new(&l);
    m.bootstrap();
    let reply_to = m.to();
    let mut new = task(1, &[]);
    new.funder = Funder::Pool { project: 1, person: 9, period: 7 };
    m.event(Event::Make { reply_to, creator: Party::Person(1), batch: Box::new([new]) });
    let reply_to = m.to();
    m.event(Event::OpenPeriod { reply_to, project: 1, period: 8, budget: 100 });
    let reply_to = m.to();
    m.event(Event::OpenPeriod { reply_to, project: 1, period: 9, budget: 100 });
    assert!(m.refused);
    m.claim(1);
    let reply_to = m.to();
    m.event(Event::Activation {
        saved: None,
        reply_to,
        task: 1,
        attempt: 1,
        end: End::Finished {
            result: TaskResult::Report { words: vec![1; 1024].into_boxed_slice() },
            cancel_delegates: false,
        },
        cause: tasks::Cause::Priced { cumulative: 5 },
    });
    assert!(m.refused);
    let reply_to = m.to();
    m.event(Event::Turn {
        refused: Box::new([]),
        reply_to,
        task: 1,
        attempt: 1,
        turn: 1,
        read: Some(1),
        offered: None,
        outstanding: Box::new([]),
        cumulative: 5,
    });
    assert!(m.refused);
}

#[test]
fn borrowed_stored_bytes_matches_allocator_for_every_retained_row() {
    let mut world = jig_tasks_world::World::new(80, LIMITS);
    world.make(Party::Person(1), vec![task(1, &[])]);
    let mut record = world.record(1).clone();
    let new = filled(&LIMITS, 1);
    record.grace = Some(tasks::Grace {
        by: Party::Person(1),
        until: Wall::from_nanos(5),
        reason: b"finish".as_slice().into(),
        message: 1,
    });
    record.refused = Box::new([1, 2]);
    record.spec = new.spec;
    record.spec.parameters = Box::new([
        Parameter::Bytes { name: 1, value: vec![1; 4096].into_boxed_slice() },
        Parameter::Number { name: 2, value: 5 },
        Parameter::Resource { name: 3, connector: 1, resource: 2 },
    ]);
    record.authority = new.authority;
    record.authority.grants[0].pattern.segments = Box::new([Box::new([1_u8, 2]) as Box<[u8]>, Box::new([3])]);
    record.authority.grants[1].pattern.last = Last::Open(Box::new([4, 5]));
    record.contract = new.contract;
    record.dependencies = Box::new([1, 2, 3]);
    record.waiting_on = Box::new([2, 3]);
    record.delegates = Box::new([4, 5]);
    record.result_followups = Box::new([6, 7]);
    let ending = tasks::Ending::Cancelled {
        by: tasks::Party::Person(1),
        at: skein_lib::Wall::from_nanos(0),
        reason: Box::new([1, 2]),
        result: Some(TaskResult::Report { words: Box::new([3, 4, 5]) }),
    };
    record.phase = tasks::Phase::Held {
        was: tasks::Was::Closing(tasks::Closing { stage: tasks::Stage::Delegates, ending: ending.clone() }),
        why: Hold::Drift,
        source: Some(Box::new(tasks::HoldSource::Resource(tasks::Name {
            connector: 1,
            path: Box::new([Box::new([8, 9]), Box::new([10])]),
        }))),
    };
    record.escalation = tasks::Escalation::Rejected { revision: 1, by: 1, reason: Box::new([6, 7, 8, 9]) };
    let mut rows = world.records.values().cloned().collect::<Vec<_>>();
    rows.push(Stored::Live(Box::new(record.clone())));
    rows.push(Stored::Milestone(tasks::Milestone {
        task: 1,
        position: 1,
        phase: record.phase.clone(),
        at: skein_lib::Wall::EPOCH,
    }));
    record.phase = tasks::Phase::Ended(ending);
    rows.push(Stored::Ended(Box::new(record)));
    for row in rows {
        let meter = Meter::new();
        let cloned = row.clone();
        let actual = meter.held();
        assert_eq!(tasks::stored_bytes(&cloned), Some(actual));
        assert_eq!(meter.held(), actual, "borrowed row measurement allocates nothing");
    }
}

#[test]
fn the_complete_maximum_recurring_batch_and_retained_ids_fit_the_owner_bound() {
    let limits = Limits { tasks: 5, project_tasks: 5, tree_tasks: 5, depth: 2, delegates: 4, batch: 4, ..LIMITS };
    let mut measured = Measured::new(&limits);
    measured.event(Event::Check);
    measured.event(Event::Restored);
    let reply_to = measured.to();
    measured.event(Event::OpenPeriod { reply_to, project: 1, period: 7, budget: 100_000 });
    let mut template = Vec::new();
    for number in 1..=u64::from(limits.batch) {
        let mut member = task(number, &[]);
        member.numbers.budget = 10;
        member.authority.budget.spend = 10;
        template.push(member);
    }
    let mut master = task(1, &[]);
    master.executor = tasks::Executor::Procedure { connector: 1, code: 1 };
    master.numbers = tasks::Numbers { budget: 0, spent: 0, spent_below: 0, reserved: 0 };
    master.funder = Funder::Period { project: 1, period: 7 };
    master.recurring = Some(Box::new(tasks::RecurringTemplate {
        key: 1,
        batch: template.into_boxed_slice(),
        overlap: tasks::RecurringOverlap::Skip,
    }));
    let reply_to = measured.to();
    measured.event(Event::Make { reply_to, creator: Party::Deployment { project: 1 }, batch: Box::new([master]) });
    assert!(!measured.refused);
    measured.event(Event::TickRecurring { task: 1, period: 7 });
    measured.event(Event::RecurringBatch { task: 1, period: 7, numbers: Box::new([2, 3, 4, 5]) });
    assert_eq!(
        measured
            .d
            .task(1)
            .expect("recurring master live")
            .recurring
            .as_ref()
            .expect("recurring template retained")
            .last_batch
            .as_ref(),
        [2, 3, 4, 5]
    );
    for number in 2..=5 {
        let reply_to = measured.to();
        measured.event(Event::Control {
            reply_to,
            by: Party::Task(1),
            task: number,
            control: tasks::Control::Cancel { reason: Box::new([]), grace: None, message: None },
        });
        measured.event(Event::EffectsSettled { task: number });
        measured.event(Event::Settled { task: number });
    }
    assert!(measured.d.task(1).expect("recurring master live").delegates.is_empty());
    assert_eq!(
        measured
            .d
            .task(1)
            .expect("recurring master live")
            .recurring
            .as_ref()
            .expect("recurring template retained")
            .last_batch
            .as_ref(),
        [2, 3, 4, 5]
    );
}

#[test]
fn saturated_approval_states_parts_and_results_fit_the_owner_bound() {
    let l = LIMITS;
    let mut m = Measured::new(&l);
    m.bootstrap();
    let mut path = skein_lib::List::with_capacity(l.hold_segments);
    for _ in 0..l.hold_segments {
        path.push(vec![7; usize::try_from(l.hold_bytes / l.hold_segments).expect("small limit")].into_boxed_slice())
            .expect("bounded path");
    }
    let batch = (1..=u64::from(l.tasks))
        .map(|number| {
            let mut new = filled(&l, number);
            new.executor = tasks::Executor::Person(tasks::PersonAddress::Person(9));
            new.funder = Funder::Pool { project: 1, person: 9, period: 7 };
            new.contract = Contract::Approval {
                state: vec![5; usize::try_from(l.result_bytes).expect("small limit")].into_boxed_slice(),
                part: tasks::Part {
                    connector: 1,
                    kind: tasks::PartKind::Resource,
                    key: tasks::PartKey::Path(path.clone()),
                },
            };
            new
        })
        .collect::<Vec<_>>();
    let reply_to = m.to();
    m.event(Event::Make {
        reply_to,
        creator: Party::Person(9),
        batch: batch[..usize::try_from(l.batch).expect("small limit")].to_vec().into_boxed_slice(),
    });
    assert!(!m.refused);
    for number in 1..=u64::from(l.batch) {
        let reply_to = m.to();
        m.event(Event::AnswerPerson {
            reply_to,
            task: number,
            person: 9,
            result: TaskResult::Approved {
                state: vec![5; usize::try_from(l.result_bytes).expect("small limit")].into_boxed_slice(),
                words: Some(Box::new([])),
            },
        });
        assert!(!m.refused);
        m.event(Event::EffectsSettled { task: number });
        m.event(Event::Settled { task: number });
    }
    let approval = Contract::Approval {
        state: vec![5; usize::try_from(l.result_bytes + 1).expect("small limit")].into_boxed_slice(),
        part: tasks::Part { connector: 1, kind: tasks::PartKind::Resource, key: tasks::PartKey::Number(1) },
    };
    assert!(!tasks::valid_contract(&l, &approval), "oversized whole state cannot be admitted");
}

#[test]
fn saturated_verdict_machine_field_arrays_fit_their_declared_owner_bound() {
    let l = LIMITS;
    let mut measured = Measured::new(&l);
    measured.bootstrap();
    let rules: Box<[tasks::FieldRule]> = (0..l.result_bytes)
        .map(|name| tasks::FieldRule { name: Box::new([u8::try_from(name).expect("small field count")]), max: 0 })
        .collect();
    let fields: Box<[tasks::ResultField]> =
        rules.iter().map(|rule| tasks::ResultField { name: rule.name.clone(), value: Box::new([]) }).collect();
    let choices: Box<[tasks::Verdict]> = (0..l.contract_choices)
        .map(|code| tasks::Verdict { code, words: 0, followups: 0, fields: rules.clone() })
        .collect();
    let batch: Box<[tasks::New]> = (1..=u64::from(l.batch))
        .map(|number| {
            let mut new = filled(&l, number);
            new.executor = tasks::Executor::Person(tasks::PersonAddress::Person(9));
            new.funder = Funder::Pool { project: 1, person: 9, period: 7 };
            new.contract = Contract::Verdict { choices: choices.clone() };
            new
        })
        .collect();
    let reply_to = measured.to();
    measured.event(Event::Make { reply_to, creator: Party::Person(9), batch });
    assert!(!measured.refused);
    for task in 1..=u64::from(l.batch) {
        let reply_to = measured.to();
        measured.event(Event::AnswerPerson {
            reply_to,
            task,
            person: 9,
            result: TaskResult::Verdict { code: 0, words: Box::new([]), fields: fields.clone() },
        });
        assert!(!measured.refused);
        measured.event(Event::EffectsSettled { task });
        measured.event(Event::Settled { task });
    }
}

#[test]
fn a_wide_back_walk_with_ancestors_and_dependents_fits_its_priced_scratch() {
    const ROOT: u64 = 1;
    const MIDDLE: u64 = 2;
    const SOURCE: u64 = 3;
    const FIRST_LEAF: u64 = 10;
    const PROPOSED: u64 = 18;
    const WIDTH: u64 = 8;
    let limits =
        Limits { tasks: 24, tree_tasks: 24, project_tasks: 24, inbox_messages: 16, inbox_bytes: 512, ..LIMITS };
    let mut measured = Measured::new(&limits);
    measured.bootstrap();
    let reply_to = measured.to();
    measured.event(Event::Make { reply_to, creator: Party::Person(9), batch: Box::new([pool_task(ROOT, &[])]) });
    let batch = (MIDDLE..FIRST_LEAF).map(|number| pool_task(number, &[])).collect();
    let reply_to = measured.to();
    measured.event(Event::Make { reply_to, creator: Party::Task(ROOT), batch });
    for sibling in SOURCE..FIRST_LEAF {
        let reply_to = measured.to();
        measured.event(Event::Introduce { reply_to, by: ROOT, left: MIDDLE, right: sibling });
    }
    let dependencies: Vec<_> = (SOURCE..FIRST_LEAF).collect();
    let batch = (FIRST_LEAF..FIRST_LEAF + WIDTH).map(|number| pool_task(number, &dependencies)).collect();
    let reply_to = measured.to();
    measured.event(Event::Make { reply_to, creator: Party::Task(MIDDLE), batch });
    let reply_to = measured.to();
    measured.event(Event::Introduce { reply_to, by: MIDDLE, left: SOURCE, right: FIRST_LEAF });
    let reply_to = measured.to();
    measured.event(Event::Make {
        reply_to,
        creator: Party::Task(SOURCE),
        batch: Box::new([pool_task(PROPOSED, &[FIRST_LEAF])]),
    });
    assert!(measured.refused, "the creator's eight waiting dependents close the proposed circle");
    let width = usize::try_from(WIDTH).expect("small tree width");
    assert_eq!(measured.d.task(ROOT).expect("root").delegates.len(), width);
    assert_eq!(measured.d.task(MIDDLE).expect("middle").delegates.len(), width);
}

#[test]
fn the_wait_index_and_walk_have_an_explicit_memory_delta() {
    // abf728e0's bound for this profile; the added index owns resource keys,
    // per-task comparison payloads, reversed edges and all simultaneous scratch.
    // An amendment's staged plan carries its list, its provenance and its Met.
    const SEED_BYTES: u64 = 9_808_904;
    const WAIT_BYTES: u64 = 1_485_368;
    const PROMISE_BYTES: u64 = 768;
    // 128 reverse dependency pairs * 16 bytes; 1,024 queue pairs *
    // (32 inline bytes + 128 name bytes), rebuilt once for each staged walk.
    const STAGED_EDGE_BYTES: u64 = 165_888;
    // 16 optional (task, circle) slots * 32 bytes, plus a 32-byte source
    // and eight named u64 tasks per projected return.
    const STAGED_RETURN_BYTES: u64 = 2_048;
    // Delegate comparison rows: 704 inline + 2,048 payload; a simultaneous
    // changes set: 800; two u32 pending maps replace two u64 maps: -320.
    const JUDGEMENT_BYTES: u64 = 3_232;
    // A bounded map ordered by answer state and reversed activity, and the
    // replacement list of question positions, during a first answer.
    const QUESTION_POSITION_BYTES: u64 = 1_776;
    const ENDING_AUTHORITY_BYTES: u64 = 384;
    // O(1) live agent count, including alignment room.
    const SETTINGS_HOLDERS_BYTES: u64 = 8;
    // Settings action carriers enlarge this fixture's bounded party-proposal map.
    const SETTINGS_PROPOSAL_BYTES: u64 = 352;
    // T1's eight held slots and T3's two extra offered/removal slots,
    // each 24 inline CharterModels bytes plus 64 model bytes. The original
    // two offered slots are already in the baseline above.
    // T1's five-entry holder map costs 416 bytes. Its SettingsRevision
    // grows each of 16 tasks' three retained/scratch records by eight bytes.
    // T3 also keeps five u32 revision names for the net-release snapshot.
    const SETTINGS_REVISIONS_BYTES: u64 = 10 * (24 + 64) + 416 + 16 * 3 * 8 + 5 * 4;
    // One bounded snapshot of the sixteen task numbers moved in a settings decision.
    const SETTINGS_MOVES_BYTES: u64 = 16 * 8;
    // Placed table and payloads, per-task placed identity, and bounded compound/walk scratch.
    const PLACED_HOLDS_BYTES: u64 = 16_032;
    // Eight credits per task keep 48 inline bytes instead of 16: number,
    // two Parties and the original activity position; no copied time or words.
    // Three retained/scratch copies per live task, four per compound member
    // and one shared copy preserve the existing row-copy accounting.
    const QUESTION_RAILS_BYTES: u64 = 8 * (48 - 16) * (16 * 3 + 16 * 4 + 1);
    // One dirty set and its draining copy, one per-task continuation map,
    // and eight retired question identifiers per task.
    let question_batches = 2 * skein_lib::List::<u64>::worst_case(16).expect("bounded dirty set")
        + skein_lib::Map::<u64, skein_lib::List<u64>>::worst_case(16).expect("bounded notice map")
        + 16 * 8 * 8;
    assert_eq!(
        tasks::worst_case(&LIMITS),
        Some(
            SEED_BYTES
                + WAIT_BYTES
                + PROMISE_BYTES
                + STAGED_EDGE_BYTES
                + STAGED_RETURN_BYTES
                + JUDGEMENT_BYTES
                + QUESTION_POSITION_BYTES
                + ENDING_AUTHORITY_BYTES
                + SETTINGS_HOLDERS_BYTES
                + SETTINGS_PROPOSAL_BYTES
                + SETTINGS_REVISIONS_BYTES
                + SETTINGS_MOVES_BYTES
                + PLACED_HOLDS_BYTES
                + QUESTION_RAILS_BYTES
                + question_batches
        )
    );
}

#[test]
fn retained_root_holds_and_new_live_holds_fit_the_index_together() {
    const ROOT: u64 = 1;
    const LIVE_CHILD: u64 = 5;
    const LIVE_TASKS: u32 = 3;
    let limits = Limits { tasks: LIVE_TASKS, holdings: 1, tree_tasks: 6, ..LIMITS };
    let mut world = jig_tasks_world::World::new(321, limits);
    let kind = tasks::Kind { connector: 1, kind: 1, hold: tasks::HoldKind::Exclusive { taken: tasks::Taken::Waits } };
    world.configure_holds(1, vec![kind]);
    world.make(Party::Person(9), vec![task(ROOT, &[])]);
    for child in 2..=LIVE_CHILD {
        let holding = tasks::Holding::Write {
            resource: tasks::Name {
                connector: 1,
                path: Box::new([Box::new([u8::try_from(child).expect("small task identity")])]),
            },
            kind: 1,
        };
        let mut new = task(child, &[]);
        new.holdings = Box::new([holding.clone()]);
        world.make(Party::Task(ROOT), vec![new]);
        if child == LIVE_CHILD {
            break;
        }
        world.claim(child, child);
        world.terminal(
            child,
            End::Finished { result: TaskResult::Failure { reason: Box::new([1]) }, cancel_delegates: false },
        );
        world.settle_effects(child);
        world.send(Event::Retained { task: child, root: ROOT, holding });
        world.release(child);
    }
    let mut measured = Measured::new(&limits);
    measured.event(Event::Kinds { connector: 1, kinds: Box::new([kind]) });
    for row in world.records.values().filter(|row| row.key().live()) {
        measured.event(Event::Restore { record: row.clone() });
    }
    measured.event(Event::Check);
    measured.event(Event::Restored);
    assert_eq!(measured.d.task(ROOT).expect("retaining root").holdings.len(), LIVE_TASKS as usize);
    assert!(measured.d.task(LIVE_CHILD).expect("new child").holds_taken);
}

fn pool_task(number: u64, dependencies: &[u64]) -> tasks::New {
    let mut new = task(number, dependencies);
    new.funder = Funder::Pool { project: 1, person: 9, period: 7 };
    new
}

#[test]
fn saturated_placed_records_reasons_and_exemptions_fit_and_measure_exactly() {
    let limits =
        Limits { tasks: 4, placed_holds: 4, project_placed_holds: 4, party_placed_holds: 4, exemptions: 3, ..LIMITS };
    let mut source = jig_tasks_world::World::new(322, limits);
    source.make(Party::Person(1), vec![task(1, &[])]);
    source.make(Party::Task(1), vec![task(2, &[]), task(3, &[]), task(4, &[])]);
    for number in 100..104 {
        let reply_to = source.to();
        source.send(Event::Place {
            reply_to,
            hold: tasks::PlacedHold {
                number,
                project: 1,
                owner: tasks::Owner::Party { person: 1 },
                mode: tasks::Mode::Stop,
                scope: tasks::Scope::Subtree { task: 1 },
                reason: vec![1; limits.message_bytes as usize].into_boxed_slice(),
                placed_at: 0,
                revision: 1,
                exempt: Box::new([]),
                lifted: false,
            },
        });
    }
    for task in 2..=4 {
        for _ in 100..104 {
            let reply_to = source.to();
            source.send(Event::Control {
                reply_to,
                by: Party::Person(1),
                task,
                control: tasks::Control::Release { again: None, message: None },
            });
        }
    }
    let mut measured = Measured::new(&limits);
    for row in source.records.values().filter(|row| row.key().live()) {
        measured.event(Event::Restore { record: row.clone() });
        if let Stored::PlacedHold(hold) = row {
            assert_eq!(hold.exempt.len(), limits.exemptions as usize);
            let meter = Meter::new();
            meter.start();
            let cloned = row.clone();
            let _peak = meter.end();
            assert_eq!(tasks::stored_bytes(&cloned), Some(meter.held()));
        }
    }
    measured.event(Event::Check);
    measured.event(Event::Restored);
    let revision = measured.d.placed_hold(100).expect("restored hold").revision;
    let reply_to = measured.to();
    measured.event(Event::Lift { reply_to, by: Party::Person(1), hold: 100, revision, with_tasks: true });
    assert!(!measured.refused);
}
