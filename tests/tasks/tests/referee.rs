use jig_domain_tasks::{Party, SettingsRevision, Status};
use jig_tasks_world::referee::{Seen, Tasks};
use skein_lib::Time;
use skein_world::domain::{Referee, Verdict};

fn rejects(seen: Vec<Seen>) {
    let mut referee = Referee::new(Tasks::default());
    let mut out = Vec::new();
    for observation in seen {
        referee.observe(Time::ZERO, observation, &mut out);
    }
    assert!(matches!(referee.verdict(), Verdict::Failed(_)), "referee catches violated invariant");
}

fn made(task: u64) -> Seen {
    Seen::Made {
        task,
        parent: Party::Person(1),
        dependencies: vec![],
        depth: 0,
        settings: SettingsRevision { task: 1, run: Some(1) },
        newest: 1,
    }
}

fn assigned(task: u64, attempt: u64) -> Seen {
    Seen::Assigned { task, attempt, after: 0, adopted: false, settings: 1 }
}

fn partial_batch() {
    rejects(vec![Seen::Batch { members: vec![1, 2], accepted: true, made: vec![1] }]);
}

fn refused_batch_created_task() {
    rejects(vec![Seen::Batch { members: vec![1], accepted: false, made: vec![1] }]);
}

fn reused_number() {
    rejects(vec![made(1), made(1)]);
}

fn dependency_on_self() {
    rejects(vec![Seen::Made {
        task: 1,
        parent: Party::Person(1),
        dependencies: vec![1],
        depth: 0,
        settings: SettingsRevision { task: 1, run: None },
        newest: 1,
    }]);
}

fn delegate_without_parent() {
    rejects(vec![Seen::Made {
        task: 2,
        parent: Party::Task(1),
        dependencies: vec![],
        depth: 1,
        settings: SettingsRevision { task: 1, run: None },
        newest: 1,
    }]);
}

fn assignment_before_dependency_done() {
    rejects(vec![
        made(1),
        Seen::Made {
            task: 2,
            parent: Party::Person(1),
            dependencies: vec![1],
            depth: 0,
            settings: SettingsRevision { task: 1, run: None },
            newest: 1,
        },
        assigned(2, 1),
    ]);
}

fn overlapping_runs() {
    rejects(vec![made(1), assigned(1, 1), assigned(1, 2)]);
}

fn nonmonotonic_attempt() {
    rejects(vec![made(1), assigned(1, 2), Seen::Terminal { task: 1, attempt: 2 }, assigned(1, 1)]);
}

fn terminal_of_wrong_attempt() {
    rejects(vec![made(1), assigned(1, 1), Seen::Terminal { task: 1, attempt: 2 }]);
}

fn closing_before_run_ended() {
    rejects(vec![made(1), assigned(1, 1), Seen::Closing { task: 1 }]);
}

fn closing_before_delegate_ended() {
    rejects(vec![
        made(1),
        Seen::Made {
            task: 2,
            parent: Party::Task(1),
            dependencies: vec![],
            depth: 1,
            settings: SettingsRevision { task: 1, run: None },
            newest: 1,
        },
        Seen::Closing { task: 1 },
    ]);
}

fn settlement_before_close() {
    rejects(vec![Seen::Settled { task: 1 }]);
}

fn result_before_settlement() {
    rejects(vec![made(1), Seen::Ended { task: 1, status: Status::Done, after: 0 }]);
}

fn duplicate_result() {
    rejects(vec![
        made(1),
        Seen::Closing { task: 1 },
        Seen::Settled { task: 1 },
        Seen::Ended { task: 1, status: Status::Done, after: 0 },
        Seen::Ended { task: 1, status: Status::Done, after: 0 },
    ]);
}

fn parent_result_before_child() {
    rejects(vec![
        made(1),
        Seen::Closing { task: 1 },
        Seen::Settled { task: 1 },
        Seen::Made {
            task: 2,
            parent: Party::Task(1),
            dependencies: vec![],
            depth: 1,
            settings: SettingsRevision { task: 1, run: None },
            newest: 1,
        },
        Seen::Ended { task: 1, status: Status::Done, after: 0 },
    ]);
}

fn wrong_cancelled_ending() {
    rejects(vec![
        made(1),
        Seen::Cancelled { task: 1 },
        Seen::Closing { task: 1 },
        Seen::Settled { task: 1 },
        Seen::Ended { task: 1, status: Status::Done, after: 0 },
    ]);
}

fn cancelled_descendant_missing() {
    rejects(vec![made(1), Seen::Cancelled { task: 1 }, Seen::Finished]);
}

fn live_limit() {
    rejects(vec![Seen::Limit { live: 2, cap: 1 }]);
}

fn reply_before_durable() {
    rejects(vec![Seen::Replied { call: 1, after: 1 }]);
}

fn duplicate_reply() {
    rejects(vec![Seen::Replied { call: 1, after: 0 }, Seen::Replied { call: 1, after: 0 }]);
}

fn assignment_before_durable() {
    rejects(vec![made(1), Seen::Assigned { task: 1, attempt: 1, after: 1, adopted: false, settings: 1 }]);
}

fn result_before_durable() {
    rejects(vec![
        made(1),
        Seen::Closing { task: 1 },
        Seen::Settled { task: 1 },
        Seen::Ended { task: 1, status: Status::Done, after: 1 },
    ]);
}

#[test]
fn resource_referee_rejects_duplicate_holds_partial_holds_pool_overadmission_and_writers() {
    use jig_domain_tasks::{Holding, Name as ResourceName, PoolSlots, Writer, WriterSlot};
    use jig_tasks_world::{LIMITS, World, task};
    let resource = ResourceName { connector: 1, path: Box::new([Box::new([1])]) };
    let holding = Holding::Write { resource: resource.clone(), kind: 1 };
    let mut world = World::new(500, LIMITS);
    world.make(Party::Person(1), vec![task(1, &[]), task(2, &[])]);
    let mut first = world.record(1).clone();
    first.holdings = Box::new([holding.clone()]);
    first.holds_taken = true;
    let mut second = world.record(2).clone();
    second.holdings = Box::new([holding]);
    second.holds_taken = true;
    rejects(vec![Seen::Resources {
        live: vec![first.clone(), second.clone()],
        pools: vec![],
        writers: vec![],
        kinds: rules(),
        reports: vec![],
    }]);
    second.phase = jig_domain_tasks::Phase::Waiting;
    rejects(vec![Seen::Resources {
        live: vec![second],
        pools: vec![],
        writers: vec![],
        kinds: rules(),
        reports: vec![],
    }]);

    let pool = ResourceName { connector: 1, path: Box::new([Box::new([2])]) };
    let slot = Holding::Slot { pool: pool.clone(), kind: 2 };
    first.holdings = Box::new([slot.clone()]);
    let mut second = world.record(2).clone();
    second.holdings = Box::new([slot]);
    second.holds_taken = true;
    let slots = vec![PoolSlots { number: 1, pool, slots: 1 }];
    rejects(vec![
        Seen::Resources {
            live: vec![first.clone()],
            pools: slots.clone(),
            writers: vec![],
            kinds: rules(),
            reports: vec![],
        },
        Seen::Resources { live: vec![first, second], pools: slots, writers: vec![], kinds: rules(), reports: vec![] },
    ]);

    let writer =
        WriterSlot { number: 1, resource: resource.clone(), writer: Writer::Run { task: 1, attempt: 1 }, lost: false };
    let other = WriterSlot { number: 2, resource, writer: Writer::Effect { entry: 2 }, lost: false };
    rejects(vec![Seen::Resources {
        live: vec![],
        pools: vec![],
        writers: vec![writer, other],
        kinds: rules(),
        reports: vec![],
    }]);
}

#[test]
fn each_initial_invariant_catches_an_independent_violation() {
    dependency_cycle();
    cycle_through_delegate();
    missing_end_deadline();
    stored_limits();
    partial_batch();
    refused_batch_created_task();
    reused_number();
    dependency_on_self();
    delegate_without_parent();
    assignment_before_dependency_done();
    overlapping_runs();
    nonmonotonic_attempt();
    terminal_of_wrong_attempt();
    closing_before_run_ended();
    closing_before_delegate_ended();
    settlement_before_close();
    result_before_settlement();
    duplicate_result();
    parent_result_before_child();
    wrong_cancelled_ending();
    cancelled_descendant_missing();
    live_limit();
    reply_before_durable();
    duplicate_reply();
    assignment_before_durable();
    result_before_durable();
}

fn dependency_cycle() {
    use jig_tasks_world::{LIMITS, World, task};
    let mut world = World::new(0, LIMITS);
    world.make(Party::Person(1), vec![task(1, &[]), task(2, &[])]);
    let mut first = world.record(1).clone();
    let mut second = world.record(2).clone();
    first.waiting_on = Box::new([2]);
    second.waiting_on = Box::new([1]);
    rejects(vec![resources(vec![first, second], vec![])]);
}

fn cycle_through_delegate() {
    use jig_tasks_world::{LIMITS, World, task};
    let mut world = World::new(0, LIMITS);
    world.make(Party::Person(1), vec![task(1, &[]), task(2, &[])]);
    let mut parent = world.record(1).clone();
    let mut child = world.record(2).clone();
    parent.delegates = Box::new([2]);
    child.waiting_on = Box::new([1]);
    rejects(vec![resources(vec![parent, child], vec![])]);
}

fn missing_end_deadline() {
    let mut referee = Referee::new(Tasks::default());
    let mut out = Vec::new();
    referee.observe(Time::ZERO, made(1), &mut out);
    referee.observe(Time::ZERO, Seen::Cancelled { task: 1 }, &mut out);
    referee.fire(Time::ZERO.saturating_add(skein_lib::Duration::from_secs(10)), &mut out);
    assert!(matches!(referee.verdict(), Verdict::Failed(_)), "missing cancelled result expires");
}

fn stored_limits() {
    use jig_tasks_world::{LIMITS, World, task};
    let mut w = World::new(0, LIMITS);
    w.make(Party::Person(1), vec![task(1, &[])]);
    for field in 0..10 {
        let mut record = w.record(1).clone();
        match field {
            0 => record.depth = LIMITS.depth + 1,
            1 => record.made = LIMITS.tree_tasks + 1,
            2 => record.delegates = vec![2; 9].into_boxed_slice(),
            3 => record.dependencies = vec![2; 9].into_boxed_slice(),
            4 => record.spec.words = vec![1; 65].into_boxed_slice(),
            5 => record.spec.inputs = vec![2; 5].into_boxed_slice(),
            6 => {
                record.spec.parameters =
                    vec![jig_domain_tasks::Parameter::Number { name: 1, value: 1 }; 5].into_boxed_slice();
            }
            7 => record.contract = jig_domain_tasks::Contract::Report { words: 33 },
            8 => {
                record.authority.delegation.kinds =
                    vec![jig_domain_tasks::AuthorityExecutor::Role(1); 4].into_boxed_slice();
            }
            9 => {
                record.authority.grants = vec![
                    jig_domain_tasks::Grant {
                        connector: 0,
                        kind: 0,
                        pattern: jig_domain_tasks::Pattern {
                            segments: Box::new([]),
                            last: jig_domain_tasks::Last::Exact(Box::from(b"resource".as_slice()))
                        }
                    };
                    5
                ]
                .into_boxed_slice();
            }
            _ => unreachable!(),
        }
        rejects(vec![Seen::Stored { live: vec![record], limits: Box::new(LIMITS) }]);
    }
    let record = w.record(1).clone();
    rejects(vec![Seen::Stored {
        live: vec![record.clone(), record],
        limits: Box::new(jig_domain_tasks::Limits { project_tasks: 1, ..LIMITS }),
    }]);
}

#[test]
fn accounting_referee_rejects_expense_reservation_and_settlement_corruption() {
    use jig_domain_tasks::{Cause, End, Funder, Key, Stored};
    use jig_tasks_world::{LIMITS, World, accounting_referee::Accounting, task};
    let mut w = World::new(80, LIMITS);
    w.make(Party::Person(1), vec![task(1, &[])]);
    w.claim(1, 1);
    let before = w.records.clone();
    w.terminal_cause(
        1,
        End::Finished {
            result: jig_domain_tasks::TaskResult::Report { words: Box::new([1]) },
            cancel_delegates: false,
        },
        Cause::Priced { cumulative: 7 },
    );
    for fault in 0..3 {
        let mut bad = w.records.clone();
        if let Some(Stored::Live(task)) = bad.get_mut(&Key::Live(1)) {
            match fault {
                0 => task.numbers.spent = 0,
                1 => task.run_spent = 0,
                2 => task.numbers.reserved = 1,
                _ => unreachable!(),
            }
        }
        let mut judge = Accounting::default();
        judge.reset(&before);
        judge.charged(1, 7);
        assert!(judge.committed(&bad, &[]).is_err(), "expense fault {fault}");
    }
    let before = w.records.clone();
    w.settle(1);
    for fault in 0..6 {
        let mut bad = w.records.clone();
        match fault {
            0 => {
                bad.remove(&Key::Ended(1));
            }
            1 => {
                if let Some(Stored::Ended(row)) = bad.get_mut(&Key::Ended(1)) {
                    row.numbers.spent = 6;
                }
            }
            2 => {
                if let Some(Stored::Ended(row)) = bad.get_mut(&Key::Ended(1)) {
                    row.funder = Funder::Task(99);
                }
            }
            3 => {
                if let Some(Stored::Ledger(row)) = bad.get_mut(&Key::Ledger(Funder::Period { project: 1, period: 0 })) {
                    row.numbers.reserved = 100;
                }
            }
            4 => {
                if let Some(Stored::Ledger(row)) = bad.get_mut(&Key::Ledger(Funder::Period { project: 1, period: 0 })) {
                    row.numbers.spent_below = 0;
                }
            }
            5 => {
                if let Some(Stored::Ended(row)) = bad.get_mut(&Key::Ended(1)) {
                    row.numbers.budget = 99;
                }
            }
            _ => unreachable!(),
        }
        let mut judge = Accounting::default();
        judge.reset(&before);
        assert!(judge.committed(&bad, &[]).is_err(), "settlement fault {fault}");
    }
    let mut bad = w.records.clone();
    bad.remove(&Key::Ended(1));
    let mut judge = Accounting::default();
    judge.reset(&w.records);
    assert_eq!(judge.committed(&bad, &[]), Err("immutable accounting row changed"));
}

#[test]
fn message_and_escalation_referee_rejects_lost_delivery_and_missing_holder() {
    use jig_domain_tasks::{Escalation, EscalationHolder, Key, MessageKind, Request, Stored, Word};
    use jig_tasks_world::{LIMITS, World, accounting_referee::Accounting, task};
    use skein_lib::{ReplyTo, Token, Wall};

    let mut world = World::new(216, LIMITS);
    world.make(Party::Person(9), vec![task(1, &[])]);
    let before = world.records.clone();
    let message = Word {
        number: 1,
        from: Party::Person(9),
        kind: MessageKind::Words,
        words: Box::new([1]),
        at: Wall::EPOCH,
        hits: 1,
        eligible: false,
    };
    let mut judge = Accounting::default();
    judge.reset(&before);
    let sent = Request::Sent { reply_to: ReplyTo::new(Token::new(1)), task: 1, word: message };
    assert_eq!(judge.committed(&before, &[sent]), Err("accepted message missing from durable inbox"));

    let mut bad = before.clone();
    if let Some(Stored::Live(row)) = bad.get_mut(&Key::Live(1)) {
        row.escalation =
            Escalation::Waiting { revision: 1, holder: EscalationHolder::Task(99), entry: 1, since: Wall::EPOCH };
    }
    assert_eq!(judge.committed(&bad, &[]), Err("escalation has no deciding holder"));
}

const INDEX_REQUESTER: u64 = 1;
const INDEX_TASK: u64 = 1;

#[test]
fn an_ended_task_without_its_party_index_row_is_refused() {
    use jig_domain_tasks::{Key, Request};
    use jig_tasks_world::{LIMITS, World, accounting_referee::Accounting, task};
    let mut world = World::new(822, LIMITS);
    world.make(Party::Person(INDEX_REQUESTER), vec![task(INDEX_TASK, &[])]);
    world.claim(INDEX_TASK, 1);
    world.finish(INDEX_TASK);
    world.settle(INDEX_TASK);
    let ended = world.records[&Key::Ended(INDEX_TASK)].clone();
    let index = world.records.keys().find(|key| matches!(key, Key::EndedByParty { .. })).copied().expect("party row");
    let row = world.records.remove(&index).expect("saved index");
    let requests = [Request::Save { record: ended }, Request::Save { record: row }];
    assert_eq!(
        Accounting::default().committed(&world.records, &requests),
        Err("ended task missing its party index row")
    );
}

#[test]
fn an_ended_listed_task_without_its_project_index_row_is_refused() {
    use jig_domain_tasks::{Key, Request};
    use jig_tasks_world::{LIMITS, World, accounting_referee::Accounting, task};
    let mut world = World::new(823, LIMITS);
    let mut listed = task(INDEX_TASK, &[]);
    listed.flags.listed = true;
    world.make(Party::Person(INDEX_REQUESTER), vec![listed]);
    world.claim(INDEX_TASK, 1);
    world.finish(INDEX_TASK);
    world.settle(INDEX_TASK);
    let ended = world.records[&Key::Ended(INDEX_TASK)].clone();
    let index =
        world.records.keys().find(|key| matches!(key, Key::EndedByProject { .. })).copied().expect("project row");
    let row = world.records.remove(&index).expect("saved index");
    let requests = [Request::Save { record: ended }, Request::Save { record: row }];
    assert_eq!(
        Accounting::default().committed(&world.records, &requests),
        Err("ended task missing its project index row")
    );
}

#[test]
fn live_queue_and_dependency_circles_are_judged_from_rows() {
    use jig_domain_tasks::{Active, Holding, Name, Phase, PoolSlots};
    use jig_tasks_world::{LIMITS, World, task};
    let mut world = World::new(0, LIMITS);
    world.make(Party::Person(1), vec![task(1, &[]), task(2, &[])]);
    let mut holder = world.record(1).clone();
    let mut waiter = world.record(2).clone();
    let resource = Name { connector: 1, path: Box::new([Box::new([1])]) };
    holder.waiting_on = Box::new([waiter.number]);
    holder.phase = Phase::Active(Active::Idle);
    waiter.phase = Phase::Waiting;
    waiter.holds_taken = false;
    waiter.hold_wait_since = Some(skein_lib::Wall::EPOCH);
    for pooled in [false, true] {
        let holding = if pooled {
            Holding::Slot { pool: resource.clone(), kind: 2 }
        } else {
            Holding::Write { resource: resource.clone(), kind: 1 }
        };
        holder.holdings = Box::new([holding.clone()]);
        waiter.holdings = Box::new([holding]);
        let pools = if pooled { vec![PoolSlots { number: 1, pool: resource.clone(), slots: 0 }] } else { vec![] };
        rejects(vec![Seen::Resources {
            live: vec![holder.clone(), waiter.clone()],
            pools,
            writers: vec![],
            kinds: rules(),
            reports: vec![],
        }]);
    }
    holder.holdings = Box::new([]);
    waiter.hold_wait_since = None;
    waiter.holdings = Box::new([]);
    waiter.waiting_on = Box::new([holder.number]);
    rejects(vec![Seen::Resources {
        live: vec![holder, waiter],
        pools: vec![],
        writers: vec![],
        kinds: rules(),
        reports: vec![],
    }]);
}

fn rules() -> Vec<jig_domain_tasks::Kind> {
    use jig_domain_tasks::{HoldKind, Kind, Taken};
    vec![
        Kind { connector: 1, kind: 1, hold: HoldKind::Exclusive { taken: Taken::Waits } },
        Kind { connector: 1, kind: 2, hold: HoldKind::Pooled { taken: Taken::Waits } },
    ]
}

fn resources(
    live: Vec<jig_domain_tasks::TaskRecord>,
    reports: Vec<(jig_domain_tasks::Name, jig_domain_tasks::HoldKind)>,
) -> Seen {
    Seen::Resources { live, pools: vec![], writers: vec![], kinds: rules(), reports }
}

fn passes(seen: Vec<Seen>) {
    let mut referee = Referee::new(Tasks::default());
    let mut out = Vec::new();
    for observation in seen {
        referee.observe(Time::ZERO, observation, &mut out);
    }
    referee.assert_passed(0);
}

#[test]
fn shared_writes_allow_two_holders_but_exclusive_writes_do_not() {
    use jig_domain_tasks::{HoldKind, Holding, Name};
    use jig_tasks_world::{LIMITS, World, task};
    let mut world = World::new(0, LIMITS);
    world.make(Party::Person(1), vec![task(1, &[]), task(2, &[])]);
    let name = Name { connector: 1, path: Box::new([Box::new([1])]) };
    let mut live = Vec::new();
    for number in [1, 2] {
        let mut row = world.record(number).clone();
        row.holdings = Box::new([Holding::Write { resource: name.clone(), kind: 1 }]);
        live.push(row);
    }
    // A named report wins over the configured Exclusive kind.
    passes(vec![resources(live.clone(), vec![(name, HoldKind::Shared)])]);
    rejects(vec![resources(live, vec![])]);
}

#[test]
fn a_shared_waiter_adds_no_queue_edge_even_when_its_holder_waits_for_it() {
    use jig_domain_tasks::{HoldKind, Holding, Kind, Name, Phase};
    use jig_tasks_world::{LIMITS, World, task};
    let mut world = World::new(0, LIMITS);
    world.make(Party::Person(1), vec![task(1, &[]), task(2, &[])]);
    let name = Name { connector: 1, path: Box::new([Box::new([1])]) };
    let mut holder = world.record(1).clone();
    let mut waiter = world.record(2).clone();
    holder.waiting_on = Box::new([2]);
    holder.holdings = Box::new([Holding::Write { resource: name.clone(), kind: 1 }]);
    waiter.phase = Phase::Waiting;
    waiter.holds_taken = false;
    waiter.hold_wait_since = Some(skein_lib::Wall::EPOCH);
    waiter.holdings = holder.holdings.clone();
    let live = vec![holder, waiter];
    passes(vec![resources(live.clone(), vec![(name, HoldKind::Shared)])]);
    for kinds in [vec![], vec![Kind { connector: 1, kind: 1, hold: HoldKind::Shared }]] {
        passes(vec![Seen::Resources { live: live.clone(), pools: vec![], writers: vec![], kinds, reports: vec![] }]);
    }
}

fn end_observations(status: Status) -> Vec<Seen> {
    vec![
        made(1),
        made(2),
        Seen::Closing { task: 2 },
        Seen::Settled { task: 2 },
        Seen::Ended { task: 2, status, after: 0 },
    ]
}

#[test]
fn an_amendment_cannot_name_an_already_failed_or_cancelled_dependency() {
    for status in [Status::Failed, Status::Cancelled] {
        let mut seen = end_observations(status);
        seen.push(Seen::Amended { task: 1, dependencies: vec![2] });
        rejects(seen);
    }
}

#[test]
fn a_done_dependency_is_met_and_a_later_failure_in_the_same_commit_is_allowed() {
    let mut seen = end_observations(Status::Done);
    seen.push(Seen::Amended { task: 1, dependencies: vec![2] });
    passes(seen);
    for status in [Status::Failed, Status::Cancelled] {
        passes(vec![
            made(1),
            made(2),
            Seen::Amended { task: 1, dependencies: vec![2] },
            Seen::Closing { task: 2 },
            Seen::Settled { task: 2 },
            Seen::Ended { task: 2, status, after: 0 },
        ]);
    }
}

#[test]
fn retaining_a_dependency_that_failed_later_is_not_a_new_add() {
    passes(vec![
        made(1),
        made(2),
        Seen::Amended { task: 1, dependencies: vec![2] },
        Seen::Closing { task: 2 },
        Seen::Settled { task: 2 },
        Seen::Ended { task: 2, status: Status::Failed, after: 0 },
        Seen::Amended { task: 1, dependencies: vec![2] },
    ]);
}

#[test]
fn the_referee_catches_a_task_made_on_an_older_revision() {
    rejects(vec![Seen::Made {
        task: 1,
        parent: Party::Person(1),
        dependencies: vec![],
        depth: 0,
        settings: SettingsRevision { task: 1, run: None },
        newest: 2,
    }]);
}

#[test]
fn the_referee_catches_a_task_moved_to_an_older_revision() {
    rejects(vec![made(1), Seen::Newest { revision: 3 }, Seen::MovedToNewest { task: 1, revision: 2 }]);
}

#[test]
fn the_referee_catches_a_run_prepared_on_another_revision() {
    rejects(vec![made(1), Seen::PreparationAsked { task: 1 }, Seen::Prepared { task: 1, revision: 2 }]);
}

#[test]
fn the_referee_catches_an_assignment_on_another_revision() {
    rejects(vec![made(1), Seen::Assigned { task: 1, attempt: 1, after: 0, adopted: false, settings: 2 }]);
}

#[test]
fn a_move_keeps_the_prepared_run_on_its_revision_until_the_next_preparation() {
    passes(vec![
        made(1),
        Seen::Newest { revision: 2 },
        Seen::MovedToNewest { task: 1, revision: 2 },
        assigned(1, 1),
        Seen::Terminal { task: 1, attempt: 1 },
        Seen::PreparationAsked { task: 1 },
        Seen::Prepared { task: 1, revision: 2 },
        Seen::Assigned { task: 1, attempt: 2, after: 0, adopted: false, settings: 2 },
    ]);
}

#[test]
fn the_world_observes_settings_moves_separately_from_requester_moves() {
    use jig_domain_tasks::Event;
    use jig_tasks_world::{EFFORTS, LIMITS, World, charters, task};
    let mut world = World::new(713, LIMITS);
    world.make(Party::Person(1), vec![task(1, &[])]);
    world.claim(1, 1);
    let mut versions = charters().into_vec();
    let mut newest = versions.clone();
    for charter in &mut newest {
        charter.revision = 2;
    }
    versions.extend(newest);
    world.send(Event::Charters { newest: 2, charters: versions.into_boxed_slice(), efforts: Box::new(EFFORTS) });
    world.send(Event::MoveToNewest { from: Some(1) });
    assert_eq!(world.record(1).settings, SettingsRevision { task: 2, run: Some(1) });
    world.make(Party::Person(1), vec![task(2, &[])]);
    world.claim(2, 2);
    assert_eq!(world.record(2).settings, SettingsRevision { task: 2, run: Some(2) });
}
