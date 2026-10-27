use jig_domain_tasks::{
    Accepted, Active, Amendment, Control, DependencyPlace, End, Ending, Event, Funder, Hold, HoldKind, Holding, Key,
    Kind, Line, Name, Party, Phase, Refusal, Status, Stored, Taken, TaskResult,
};
use jig_tasks_world::{LIMITS, Reply, World, task};
use skein_lib::{ReplyTo, Rng, Token};
use std::collections::BTreeSet;

const DRAW_SOURCE: u64 = 731;
const DRAWN_SEEDS: usize = 64;
const STEPS: usize = 96;
const PERSON: Party = Party::Person(9);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Reached {
    Amended,
    Circle,
    Kept,
    Parked,
    Met,
    QueueLeft,
    QueueReturned,
    CompoundAmended,
}
const MARKERS: [Reached; 8] = [
    Reached::Amended,
    Reached::Circle,
    Reached::Kept,
    Reached::Parked,
    Reached::Met,
    Reached::QueueLeft,
    Reached::QueueReturned,
    Reached::CompoundAmended,
];

// Each operation carries its complete draw.
#[derive(Debug)]
enum Operation {
    Amend { task: u64, dependencies: Vec<u64> },
    HoldRelease { task: u64 },
    Claim { task: u64 },
    End { task: u64, failed: bool },
    Cancel { task: u64 },
    Restart,
    Compound(Vec<jig_domain_tasks::Operation>),
}

fn name(value: u8) -> Name {
    Name { connector: 1, path: Box::new([Box::new([value])]) }
}
fn write(value: u8, kind: u16) -> Holding {
    Holding::Write { resource: name(value), kind }
}
fn slot() -> Holding {
    Holding::Slot { pool: name(2), kind: 2 }
}

fn population(seed: u64) -> World {
    let mut world = World::new(seed, LIMITS);
    world.configure_holds(
        1,
        vec![
            Kind { connector: 1, kind: 1, hold: HoldKind::Exclusive { taken: Taken::Waits } },
            Kind { connector: 1, kind: 2, hold: HoldKind::Pooled { taken: Taken::Waits } },
            Kind { connector: 1, kind: 3, hold: HoldKind::Shared },
        ],
    );
    world.send(Event::Slots { pool: name(2), slots: 2 });
    let mut first = task(1, &[]);
    first.numbers.budget = 400;
    first.authority.budget.spend = 400;
    first.holdings = Box::new([write(1, 1), write(3, 3)]);
    let mut second = task(2, &[]);
    second.holdings = Box::new([write(1, 1)]);
    let mut third = task(3, &[]);
    third.holdings = Box::new([slot(), write(3, 3)]);
    let mut fourth = task(4, &[]);
    fourth.holdings = Box::new([slot()]);
    assert_eq!(world.make(PERSON, vec![first, second, third, fourth]), Reply::Made(vec![1, 2, 3, 4]));
    let mut fifth = task(5, &[]);
    fifth.funder = Funder::Task(1);
    let mut sixth = task(6, &[]);
    sixth.funder = Funder::Task(1);
    sixth.holdings = Box::new([slot()]);
    assert_eq!(world.make(Party::Task(1), vec![fifth, sixth]), Reply::Made(vec![5, 6]));
    assert_eq!(world.record(2).phase, Phase::Waiting);
    assert_eq!(world.record(6).phase, Phase::Waiting);
    world
}

fn live(world: &World) -> Vec<u64> {
    let mut live = Vec::new();
    for (key, _) in world.records.range(Key::Live(0)..=Key::Live(u64::MAX)) {
        if let Key::Live(number) = key {
            live.push(*number);
        }
    }
    live
}

fn pick(random: &mut Rng, choices: &[u64]) -> u64 {
    *choices.get(usize::try_from(random.below(choices.len() as u64)).expect("small draw")).expect("nonempty choices")
}

// A done stub can disappear at the next removal of its last live name.
// Draw a new add while it is available, across all possible dependents.
fn draw_met(world: &World, random: &mut Rng) -> Option<Operation> {
    let mut choices = Vec::new();
    for (_, record) in world.records.range(Key::Stub(0)..=Key::Stub(u64::MAX)) {
        if let Stored::Stub(stub) = record
            && stub.phase == Status::Done
        {
            for task in live(world) {
                if !world.record(task).dependencies.contains(&stub.task) {
                    choices.push((task, stub.task));
                }
            }
        }
    }
    if choices.is_empty() {
        return None;
    }
    let (task, dependency) = *choices
        .get(usize::try_from(random.below(choices.len() as u64)).expect("small draw"))
        .expect("nonempty choices");
    let mut dependencies = world.record(task).dependencies.to_vec();
    dependencies.push(dependency);
    Some(Operation::Amend { task, dependencies })
}

fn done(world: &World, dependency: u64) -> bool {
    if let Some(Stored::Stub(stub)) = world.records.get(&Key::Stub(dependency)) {
        return stub.phase == Status::Done;
    }
    matches!(world.records.get(&Key::Ended(dependency)),
        Some(Stored::Ended(row)) if matches!(row.phase, Phase::Ended(jig_domain_tasks::Ending::Done(_))))
}

fn draw(world: &World, random: &mut Rng) -> Operation {
    let task = pick(random, &live(world));
    match random.below(13) {
        0..=5 => {
            let mut dependencies = world.record(task).dependencies.to_vec();
            match random.below(3) {
                0 => {
                    let candidate = pick(random, &live(world));
                    if !dependencies.contains(&candidate) {
                        dependencies.push(candidate);
                    }
                }
                1 => {
                    dependencies.pop();
                }
                2 => {
                    if random.chance(500)
                        && let Some(operation) = draw_met(world, random)
                    {
                        return operation;
                    }
                    dependencies.clear();
                    // Ended names deliberately exercise both met and refusal.
                    dependencies.push(random.below(6) + 1);
                }
                _ => unreachable!(),
            }
            Operation::Amend { task, dependencies }
        }
        6 => Operation::HoldRelease { task },
        7 | 8 => Operation::Claim { task },
        9 => Operation::End { task, failed: random.chance(200) },
        10 => Operation::Cancel { task },
        11 => Operation::Restart,
        12 => {
            let mut operations = Vec::new();
            for at in 0..=random.below(3) {
                let number = pick(random, &live(world));
                let row = world.record(number);
                if random.chance(650) {
                    let mut dependencies = row.dependencies.to_vec();
                    if random.chance(500) {
                        dependencies.pop();
                    }
                    operations.push(jig_domain_tasks::Operation::Amend {
                        task: number,
                        message: row.last_message + at + 1,
                        stop_run: false,
                        amendment: Box::new(Amendment {
                            tracked: None,
                            addressed: None,
                            instructions: None,
                            release: if random.chance(500) { Some(row.revisions.hold) } else { None },
                            spec: None,
                            wake: None,
                            dependencies: Some(dependencies.into()),
                            authority: None,
                            llm: None,
                            reason: Box::new([1]),
                        }),
                    });
                } else {
                    operations.push(jig_domain_tasks::Operation::Release { task: number, again: None, message: None });
                }
            }
            Operation::Compound(operations)
        }
        _ => unreachable!(),
    }
}

// The places core would load: each newly added dependency's stored ended row.
fn places(world: &World, task: u64, dependencies: &[u64]) -> Box<[DependencyPlace]> {
    let mut places = Vec::new();
    for dependency in dependencies {
        if world.record(task).dependencies.contains(dependency) {
            continue;
        }
        let Some(Stored::Ended(row)) = world.records.get(&Key::Ended(*dependency)) else { continue };
        let status = match row.phase {
            Phase::Ended(Ending::Done(_)) => Status::Done,
            Phase::Ended(Ending::Failed { .. }) => Status::Failed,
            Phase::Ended(Ending::Cancelled { .. }) => Status::Cancelled,
            Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Held { .. } => panic!("ended row"),
        };
        places.push(DependencyPlace {
            number: row.number,
            project: row.project,
            requester: row.requester,
            root: row.root,
            depth: row.depth,
            status,
        });
    }
    places.into()
}

fn amend(world: &mut World, task: u64, dependencies: &[u64], seen: &mut BTreeSet<Reached>) {
    let before = world.record(task).clone();
    let places = places(world, task, dependencies);
    let reply_to = world.to();
    let call = reply_to.into_token().raw();
    let reply_to = ReplyTo::new(Token::new(call));
    world.send(Event::Amend {
        places,
        reply_to,
        by: PERSON,
        task,
        message: before.last_message + 1,
        stop_run: false,
        funder: None,
        amendment: Amendment {
            tracked: None,
            addressed: None,
            instructions: None,
            release: None,
            spec: None,
            wake: None,
            dependencies: Some(dependencies.into()),
            authority: None,
            llm: None,
            reason: Box::new([1]),
        },
    });
    match world.replies.get(&call).expect("amendment reply") {
        Reply::Done => {
            seen.insert(Reached::Amended);
            let after = world.record(task);
            if matches!(after.phase, Phase::Active(_)) && !after.waiting_on.is_empty() {
                seen.insert(Reached::Kept);
            }
            if before.phase == Phase::Waiting && before.waiting_on.is_empty() && !after.waiting_on.is_empty() {
                seen.insert(Reached::QueueLeft);
            }
            for dependency in dependencies {
                if !before.dependencies.contains(dependency) && done(world, *dependency) {
                    assert!(!after.waiting_on.contains(dependency));
                    let mut recorded = false;
                    for added in &after.added {
                        if added.task == *dependency {
                            recorded = true;
                        }
                    }
                    assert!(recorded, "done dependency's add is recorded");
                    seen.insert(Reached::Met);
                }
            }
        }
        Reply::Refused(problem) => {
            if let Refusal::Circle { .. } = problem.why {
                let members = problem.blocked_by.as_ref().expect("circle names its members");
                assert!(!members.is_empty(), "circle names at least one member");
                seen.insert(Reached::Circle);
            } else {
                assert!(
                    matches!(
                        problem.why,
                        Refusal::State
                            | Refusal::Unknown
                            | Refusal::Dependencies
                            | Refusal::DependencyEnded { how: Status::Failed | Status::Cancelled }
                    ),
                    "unexpected amendment refusal: {:?}",
                    problem.why
                );
            }
        }
        reply @ (Reply::Made(_)
        | Reply::Lifted { .. }
        | Reply::Acknowledged(_)
        | Reply::Turn(_)
        | Reply::WriterWaiting(_)) => {
            panic!("unexpected amendment reply: {reply:?}");
        }
    }
}

fn drain(world: &mut World) {
    for (task, _) in world.stops.clone() {
        if world.runs.contains_key(&task) {
            assert_eq!(world.terminal(task, End::Parked), Reply::Acknowledged(Accepted::New));
        }
    }
    while let Some(task) = world.closing.first().copied() {
        world.settle(task);
    }
}

fn control(world: &mut World, task: u64, control: Control) {
    let reply_to = world.to();
    let call = reply_to.into_token().raw();
    let reply_to = ReplyTo::new(Token::new(call));
    world.send(Event::Control { reply_to, by: PERSON, task, control });
    match world.replies.get(&call).expect("control reply") {
        Reply::Done => {}
        Reply::Refused(problem) if problem.why == Refusal::State => {}
        reply @ (Reply::Made(_)
        | Reply::Lifted { .. }
        | Reply::Refused(_)
        | Reply::Acknowledged(_)
        | Reply::Turn(_)
        | Reply::WriterWaiting(_)) => {
            panic!("unexpected control reply: {reply:?}");
        }
    }
}

fn compound(world: &mut World, operations: Vec<jig_domain_tasks::Operation>, seen: &mut BTreeSet<Reached>) {
    let mut amended = false;
    let mut staged = skein_lib::List::with_capacity(u32::try_from(operations.len()).expect("small draw"));
    for operation in operations {
        if matches!(operation, jig_domain_tasks::Operation::Amend { .. }) {
            amended = true;
        }
        staged.push(operation).expect("drawn compound bound");
    }
    let reply_to = world.to();
    let call = reply_to.into_token();
    world.send(Event::Compound {
        reply_to: ReplyTo::new(call),
        by: jig_domain_tasks::Party::Person(9),
        project: 1,
        operations: staged,
        period: 0,
        pool_budget: 1_000,
        period_budget: 10_000,
    });
    match world.replies.get(&call.raw()).expect("compound reply") {
        Reply::Done => {
            if amended {
                seen.insert(Reached::CompoundAmended);
            }
        }
        Reply::Refused(problem) => {
            if let Refusal::Circle { .. } = problem.why {
                let members = problem.blocked_by.as_ref().expect("circle names its members");
                assert!(!members.is_empty(), "circle names at least one member");
                seen.insert(Reached::Circle);
            } else if problem.why == Refusal::Unknown {
                // A compound carries no loaded places (tasks.md 7): an earlier
                // operation's dropped dependency re-added once it has ended is unknown.
                let dependency = problem.task.expect("unknown dependency is named");
                assert!(!live(world).contains(&dependency), "only an ended dependency is unknown: {problem:?}");
            } else {
                assert!(
                    matches!(problem.why, Refusal::State | Refusal::Read),
                    "unexpected compound refusal: {problem:?}"
                );
            }
        }
        reply @ (Reply::Made(_)
        | Reply::Lifted { .. }
        | Reply::Acknowledged(_)
        | Reply::Turn(_)
        | Reply::WriterWaiting(_)) => {
            panic!("unexpected compound reply: {reply:?}");
        }
    }
}

fn apply(world: &mut World, operation: Operation, seen: &mut BTreeSet<Reached>, attempt: &mut u64) {
    match operation {
        Operation::Amend { task, dependencies } => {
            amend(world, task, &dependencies, seen);
        }
        Operation::HoldRelease { task } => {
            world.send(Event::Hold { task, why: Hold::Budget });
            drain(world);
            if world.records.contains_key(&Key::Live(task)) {
                control(world, task, Control::Release { again: None, message: None });
            }
        }
        Operation::Claim { task } => {
            if matches!(world.record(task).phase, Phase::Active(Active::Due)) && !world.runs.contains_key(&task) {
                *attempt += 1;
                match world.claim_budget_writing(task, *attempt, 1, Box::new([])) {
                    Reply::Done => {}
                    Reply::Refused(problem) if problem.why == Refusal::Funding => {}
                    reply @ (Reply::Made(_)
                    | Reply::Lifted { .. }
                    | Reply::Refused(_)
                    | Reply::Acknowledged(_)
                    | Reply::Turn(_)
                    | Reply::WriterWaiting(_)) => {
                        panic!("unexpected claim reply: {reply:?}");
                    }
                }
            }
        }
        Operation::End { task, failed } => {
            if world.runs.contains_key(&task) {
                let kept = !world.record(task).waiting_on.is_empty();
                let result = if failed {
                    TaskResult::Failure { reason: Box::new([1]) }
                } else {
                    TaskResult::Report { words: Box::new([1]) }
                };
                match world.terminal(task, End::Finished { result, cancel_delegates: false }) {
                    Reply::Acknowledged(Accepted::New) => {
                        if kept && !failed {
                            assert!(matches!(world.record(task).phase, Phase::Active(Active::Idle)));
                            let mut parked = false;
                            for record in world.records.values() {
                                if let Stored::Activity(row) = record
                                    && row.task == task
                                    && matches!(row.line, Line::Kept { .. })
                                {
                                    parked = true;
                                }
                            }
                            assert!(parked, "kept done finish has a durable parked line");
                            seen.insert(Reached::Parked);
                        }
                    }
                    Reply::Refused(problem) if problem.why == Refusal::LiveDelegates => {}
                    reply @ (Reply::Made(_)
                    | Reply::Lifted { .. }
                    | Reply::Done
                    | Reply::Refused(_)
                    | Reply::Acknowledged(_)
                    | Reply::Turn(_)
                    | Reply::WriterWaiting(_)) => {
                        panic!("unexpected terminal reply: {reply:?}");
                    }
                }
            }
        }
        Operation::Cancel { task } => {
            control(world, task, Control::Cancel { reason: Box::new([1]), grace: None, message: None });
        }
        Operation::Restart => world.restart(),
        Operation::Compound(operations) => compound(world, operations, seen),
    }
    drain(world);
}

#[test]
fn random_amendments_preserve_the_promise_and_leave_no_circle() {
    let mut random = Rng::new(DRAW_SOURCE);
    let mut seeds = BTreeSet::new();
    while seeds.len() < DRAWN_SEEDS {
        seeds.insert(random.below(65_536));
    }
    eprintln!("amendment drawn seeds={seeds:?}, steps={STEPS}");
    let mut seen = BTreeSet::new();
    let tracing = std::env::var_os("JIG_WORLD_TRACE").is_some();
    for seed in seeds {
        eprintln!("amendment seed={seed}");
        let mut world = population(seed);
        let mut random = Rng::new(seed);
        let mut attempt = 0;
        let mut left = BTreeSet::new();
        for step in 0..STEPS {
            if world.live() == 0 {
                break;
            }
            let mut queued = BTreeSet::new();
            for task in live(&world) {
                let row = world.record(task);
                if row.phase == Phase::Waiting && row.hold_wait_since.is_some() && row.waiting_on.is_empty() {
                    queued.insert(task);
                }
            }
            let operation = draw(&world, &mut random);
            if tracing {
                eprintln!("step {step}: {operation:?}");
            }
            apply(&mut world, operation, &mut seen, &mut attempt);
            for task in live(&world) {
                let row = world.record(task);
                if queued.contains(&task) && !row.waiting_on.is_empty() {
                    left.insert(task);
                }
                if left.contains(&task)
                    && row.phase == Phase::Waiting
                    && row.waiting_on.is_empty()
                    && row.hold_wait_since.is_some()
                {
                    seen.insert(Reached::QueueReturned);
                }
            }
            world.referee.assert_holding(seed);
        }
        for task in live(&world) {
            if world.records.contains_key(&Key::Live(task)) {
                control(&mut world, task, Control::Cancel { reason: Box::new([1]), grace: None, message: None });
                drain(&mut world);
            }
        }
        world.complete_cancel();
        assert_eq!(world.results.len(), 6, "seed {seed}: every task ends");
        assert_eq!(world.live(), 0);
    }
    eprintln!("amendment reach={seen:?}");
    for marker in MARKERS {
        assert!(seen.contains(&marker), "sweep reaches {marker:?}");
    }
}
