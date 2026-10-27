use jig_domain_tasks::{
    self as tasks, Active, Amendment, Control, Ending, Event, Executor, Hold, HoldKind, Holding, Kind, MessageKind,
    Name, Party, Phase, ProcedureDecision, ProcedureEnd, Taken, TaskResult, WakePolicy, WakeRule,
};
use jig_tasks_world::{LIMITS, Reply, World, task};
use skein_lib::{Duration, ReplyTo};

const KEPT: u64 = 1;
const DEPENDENCY: u64 = 2;
const OTHER: u64 = 3;
const AMENDER: Party = Party::Person(9);

fn response(world: &mut World, event: impl FnOnce(ReplyTo) -> Event) -> Reply {
    let to = world.to();
    let call = to.into_token();
    world.send(event(ReplyTo::new(call)));
    world.replies[&call.raw()].clone()
}

fn amend(world: &mut World, dependencies: &[u64]) {
    let message = world.record(KEPT).last_message + 1;
    assert_eq!(
        response(world, |reply_to| Event::Amend {
            places: Box::new([]),
            reply_to,
            by: AMENDER,
            task: KEPT,
            message,
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
        }),
        Reply::Done
    );
}

fn quiet() -> WakePolicy {
    WakePolicy {
        markers: 0,
        words: WakeRule::Never,
        notices: WakeRule::Never,
        news: WakeRule::Never,
        questions: false,
        answers: false,
        results: tasks::ResultsWake::Never,
        timers: false,
    }
}

fn pair() -> World {
    let mut world = World::new(870, LIMITS);
    let mut kept = task(KEPT, &[]);
    kept.wake = quiet();
    world.make(AMENDER, vec![kept, task(DEPENDENCY, &[])]);
    world
}

fn done(world: &mut World, number: u64) {
    world.claim_fresh(number);
    world.finish(number);
    world.settle(number);
}

fn met(world: &World, dependency: u64, removed: bool, from: Party, hits: u32) {
    let words: Vec<_> =
        world.record(KEPT).inbox.iter().filter(|word| matches!(word.kind, MessageKind::Met { .. })).collect();
    assert_eq!(words.len(), 1);
    let notice = words.first().expect("one Met");
    assert_eq!(notice.kind, MessageKind::Met { dependency, removed });
    assert_eq!(notice.from, from);
    assert_eq!(notice.hits, hits);
    assert!(notice.words.is_empty());
    assert!(notice.number > 0);
}

fn procedure(world: &mut World, end: ProcedureEnd) {
    let read = world.contexts[&KEPT].inbox.last().map(|word| word.number);
    let step = world.record(KEPT).attempt + 1;
    assert_eq!(
        response(world, |reply_to| Event::Procedure {
            reply_to,
            task: KEPT,
            step,
            read,
            decision: ProcedureDecision::new(end),
        }),
        Reply::Done
    );
}

#[test]
fn a_parked_agents_last_dependency_done_wakes_it_to_finish_with_no_pending() {
    let mut world = pair();
    world.claim_fresh(KEPT);
    amend(&mut world, &[DEPENDENCY]);
    world.finish(KEPT);
    assert_eq!(world.record(KEPT).phase, Phase::Active(Active::Idle));
    assert!(!world.activations.contains(&KEPT));
    done(&mut world, DEPENDENCY);
    met(&world, DEPENDENCY, false, Party::Task(DEPENDENCY), 1);
    assert_eq!(world.record(KEPT).phase, Phase::Active(Active::Due));
    let context = &world.contexts[&KEPT];
    assert!(context.pending.is_empty());
    assert_eq!(context.added.as_ref(), [tasks::Added { task: DEPENDENCY, at: world.env.wall, by: AMENDER }]);
    done(&mut world, KEPT);
    assert!(matches!(world.results[&KEPT], Ending::Done(_)));
}

#[test]
fn removing_a_kept_agents_last_dependency_wakes_it_with_a_met_from_the_amender() {
    let mut world = pair();
    amend(&mut world, &[DEPENDENCY]);
    world.activations.remove(&KEPT);
    amend(&mut world, &[]);
    met(&world, DEPENDENCY, true, AMENDER, 1);
    assert_eq!(world.record(KEPT).phase, Phase::Active(Active::Due));
    assert!(world.activations.contains(&KEPT));
    assert!(world.contexts[&KEPT].pending.is_empty());
}

#[test]
fn only_the_second_of_two_pending_dependencies_done_sends_one_met() {
    let mut world = pair();
    world.make(AMENDER, vec![task(OTHER, &[])]);
    amend(&mut world, &[DEPENDENCY, OTHER]);
    world.activations.remove(&KEPT);
    done(&mut world, DEPENDENCY);
    assert_eq!(world.record(KEPT).waiting_on.as_ref(), [OTHER]);
    assert_eq!(world.record(KEPT).phase, Phase::Active(Active::Idle));
    assert!(!world.activations.contains(&KEPT));
    assert!(!world.record(KEPT).inbox.iter().any(|word| matches!(word.kind, MessageKind::Met { .. })));
    done(&mut world, OTHER);
    met(&world, OTHER, false, Party::Task(OTHER), 1);
    assert_eq!(world.record(KEPT).phase, Phase::Active(Active::Due));
}

#[test]
fn two_mets_merge_after_a_dependency_is_removed_then_readded_before_claim() {
    let mut world = pair();
    amend(&mut world, &[DEPENDENCY]);
    amend(&mut world, &[]);
    met(&world, DEPENDENCY, true, AMENDER, 1);
    amend(&mut world, &[DEPENDENCY]);
    assert_eq!(world.record(KEPT).phase, Phase::Active(Active::Idle));
    world.activations.remove(&KEPT);
    done(&mut world, DEPENDENCY);
    met(&world, DEPENDENCY, false, Party::Task(DEPENDENCY), 2);
    assert_eq!(world.record(KEPT).phase, Phase::Active(Active::Due));
    assert!(world.activations.contains(&KEPT));
    let wakes = world
        .records
        .values()
        .filter(|row| {
            matches!(row,
                tasks::Stored::Milestone(line) if line.task == KEPT && line.phase == Phase::Active(Active::Due)
            )
        })
        .count();
    assert_eq!(wakes, 3, "initial activation and one wake after each kept interval");
}

#[test]
fn a_kept_procedure_wakes_on_met_steps_with_no_pending_and_can_end_done() {
    let mut world = World::new(871, LIMITS);
    let mut kept = task(KEPT, &[]);
    kept.executor = Executor::Procedure { connector: 1, code: 1 };
    kept.wake = quiet();
    world.make(AMENDER, vec![kept, task(DEPENDENCY, &[])]);
    procedure(&mut world, ProcedureEnd::Wait);
    amend(&mut world, &[DEPENDENCY]);
    assert_eq!(world.contexts[&KEPT].pending.as_ref(), [DEPENDENCY]);
    procedure(&mut world, ProcedureEnd::Wait);
    world.activations.remove(&KEPT);
    done(&mut world, DEPENDENCY);
    met(&world, DEPENDENCY, false, Party::Task(DEPENDENCY), 1);
    assert!(world.activations.contains(&KEPT));
    assert!(world.contexts[&KEPT].pending.is_empty());
    procedure(&mut world, ProcedureEnd::Result(TaskResult::Report { words: Box::new([7]) }));
    world.settle(KEPT);
    assert!(matches!(world.results[&KEPT], Ending::Done(_)));
}

#[test]
fn a_held_kept_task_keeps_its_met_until_release_then_is_due() {
    let mut world = pair();
    amend(&mut world, &[DEPENDENCY]);
    world.send(Event::Hold { task: KEPT, why: Hold::Drift });
    world.activations.remove(&KEPT);
    done(&mut world, DEPENDENCY);
    met(&world, DEPENDENCY, false, Party::Task(DEPENDENCY), 1);
    assert!(matches!(world.record(KEPT).phase, Phase::Held { .. }));
    assert!(!world.activations.contains(&KEPT));
    assert_eq!(
        response(&mut world, |reply_to| Event::Control {
            reply_to,
            by: AMENDER,
            task: KEPT,
            control: Control::Release { again: None, message: None },
        }),
        Reply::Done
    );
    assert_eq!(world.record(KEPT).phase, Phase::Active(Active::Due));
    assert!(world.activations.contains(&KEPT));
    met(&world, DEPENDENCY, false, Party::Task(DEPENDENCY), 1);
}

#[test]
fn a_live_met_leaves_all_ordinary_sender_room_available() {
    let mut world = pair();
    amend(&mut world, &[DEPENDENCY]);
    done(&mut world, DEPENDENCY);
    met(&world, DEPENDENCY, false, Party::Task(DEPENDENCY), 1);
    for _ in 0..LIMITS.inbox_messages {
        let message = tasks::Word {
            number: world.record(KEPT).last_message + 1,
            from: AMENDER,
            kind: MessageKind::Words,
            words: Box::new([1]),
            at: world.env.wall,
            hits: 1,
            eligible: false,
        };
        assert_eq!(
            response(&mut world, |reply_to| Event::Message { reply_to, project: 1, task: KEPT, word: message }),
            Reply::Done
        );
    }
    assert_eq!(world.record(KEPT).inbox.len(), usize::try_from(LIMITS.inbox_messages).expect("small limit") + 2);
}

#[test]
fn a_waiters_last_dependency_removed_returns_it_to_its_place_without_a_met() {
    let mut world = World::new(872, LIMITS);
    world.configure_holds(1, vec![Kind { connector: 1, kind: 1, hold: HoldKind::Exclusive { taken: Taken::Waits } }]);
    let resource = Holding::Write { resource: Name { connector: 1, path: Box::new([Box::new([1])]) }, kind: 1 };
    for number in [OTHER, KEPT] {
        let mut new = task(number, &[]);
        new.holdings = Box::new([resource.clone()]);
        world.make(AMENDER, vec![new]);
    }
    world.make(AMENDER, vec![task(DEPENDENCY, &[])]);
    let place = world.record(KEPT).hold_wait_since;
    amend(&mut world, &[DEPENDENCY]);
    world.elapse(Duration::from_millis(1));
    let later = 4;
    let mut new = task(later, &[]);
    new.holdings = Box::new([resource]);
    world.make(AMENDER, vec![new]);
    amend(&mut world, &[]);
    assert_eq!(world.record(KEPT).phase, Phase::Waiting);
    assert_eq!(world.record(KEPT).hold_wait_since, place);
    assert!(place.is_some());
    assert!(!world.record(KEPT).inbox.iter().any(|word| matches!(word.kind, MessageKind::Met { .. })));
    done(&mut world, OTHER);
    assert!(world.record(KEPT).holds_taken);
    assert_eq!(world.record(later).phase, Phase::Waiting);
}

#[test]
fn a_met_revalidates_liveness_phase_pending_and_the_global_message_fence() {
    let mut world = pair();
    amend(&mut world, &[DEPENDENCY]);
    let before = world.records.clone();
    for (task, message) in [(KEPT, 100), (99, 101)] {
        world.send(Event::MetNotice {
            task,
            dependency: DEPENDENCY,
            removed: false,
            by: Party::Task(DEPENDENCY),
            message,
        });
        assert_eq!(world.records, before);
    }
    amend(&mut world, &[]);
    let before = world.records.clone();
    let last = world.record(KEPT).last_message;
    for message in [0, last - 1, last] {
        world.send(Event::MetNotice {
            task: KEPT,
            dependency: DEPENDENCY,
            removed: false,
            by: Party::Task(DEPENDENCY),
            message,
        });
        assert_eq!(world.records, before);
    }
    let waiter = 4;
    world.make(AMENDER, vec![task(waiter, &[DEPENDENCY])]);
    let before = world.records.clone();
    world.send(Event::MetNotice {
        task: waiter,
        dependency: DEPENDENCY,
        removed: false,
        by: Party::Task(DEPENDENCY),
        message: last + 1,
    });
    assert_eq!(world.records, before);
    world.claim_fresh(KEPT);
    world.finish(KEPT);
    let before = world.records.clone();
    world.send(Event::MetNotice {
        task: KEPT,
        dependency: DEPENDENCY,
        removed: false,
        by: Party::Task(DEPENDENCY),
        message: last + 2,
    });
    assert_eq!(world.records, before);
}
