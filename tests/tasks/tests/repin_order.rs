use jig_domain_tasks::{
    Accepted, Active, Added, Amendment, Control, End, EndKind, Ending, Event, Funder, Key, Line, MessageKind, Party,
    Phase, Refusal, Stored,
};
use jig_tasks_world::{LIMITS, Reply, World, task};
use skein_lib::ReplyTo;

const DIRECTOR: Party = Party::Person(9);
const T4B: u64 = 1;
const T4R: u64 = 2;
const FIRST_RUN: u64 = 3;
const DIRECTORIES: u64 = 4;
const FOLLOWUP: u64 = 5;

fn response(world: &mut World, event: impl FnOnce(ReplyTo) -> Event) -> Reply {
    let to = world.to();
    let call = to.into_token();
    world.send(event(ReplyTo::new(call)));
    world.replies.get(&call.raw()).expect("request answered").clone()
}

fn amend(world: &mut World, by: Party, task: u64, dependencies: &[u64]) -> Reply {
    let message = world.record(task).last_message + 1;
    response(world, |reply_to| Event::Amend {
        places: Box::new([]),
        reply_to,
        by,
        task,
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
    })
}

fn refused(reply: Reply, task: u64, why: Refusal, blocked: &[u64]) {
    let Reply::Refused(problem) = reply else { panic!("expected refusal, got {reply:?}") };
    assert_eq!(problem.task, Some(task));
    assert_eq!(problem.why, why);
    assert_eq!(problem.blocked_by.as_deref(), Some(blocked));
}

fn parked(world: &mut World, task: u64, dependency: u64) {
    let attempt = world.record(task).attempt;
    world.finish(task);
    assert_eq!(world.record(task).phase, Phase::Active(Active::Idle));
    assert!(!world.runs.contains_key(&task));
    assert!(!world.results.contains_key(&task));
    assert!(world.activations.is_empty());
    let mut lines = Vec::new();
    for row in world.records.values() {
        if let Stored::Activity(line) = row
            && line.task == task
        {
            lines.push(&line.line);
        }
    }
    let mut ended = None;
    for (position, line) in lines.iter().enumerate() {
        if matches!(line, Line::RunEnded { attempt: run, end: EndKind::Parked, .. } if *run == attempt) {
            ended = Some(position);
        }
    }
    let ended = ended.expect("parked run activity");
    assert!(matches!(lines.get(ended + 1), Some(Line::Kept { attempt: run, pending, question: None })
        if *run == attempt && pending.as_ref() == [dependency]));
}

fn met(world: &World, task: u64, dependency: u64) {
    let mut notices = Vec::new();
    for word in &world.record(task).inbox {
        if matches!(word.kind, MessageKind::Met { .. }) {
            notices.push(word);
        }
    }
    assert_eq!(notices.len(), 1);
    let notice = notices.first().expect("one Met");
    assert_eq!(notice.kind, MessageKind::Met { dependency, removed: false });
    assert_eq!(notice.from, Party::Task(dependency));
    assert_eq!(world.record(task).phase, Phase::Active(Active::Due));
    assert_eq!(world.activations.iter().copied().collect::<Vec<_>>(), [task]);
}

fn finish(world: &mut World, task: u64) {
    world.finish(task);
    world.settle(task);
    assert!(matches!(world.results.get(&task), Some(Ending::Done(_))));
}

// domain/tasks.md, 5.7, 7, 8.1 and 12.
// Story 3's queue order is already covered in added.rs by
// a_waiter_given_a_dependency_leaves_the_queue_and_returns_ahead_of_a_later_waiter.
#[test]
fn a_running_repin_given_a_dependency_lands_after_the_other_and_starts_its_dependents() {
    let mut world = World::new(880, LIMITS);
    assert_eq!(
        world.make(DIRECTOR, vec![task(T4B, &[]), task(T4R, &[]), task(FIRST_RUN, &[])]),
        Reply::Made(vec![T4B, T4R, FIRST_RUN])
    );
    for task in [T4B, T4R, FIRST_RUN] {
        world.claim(task, 1);
    }
    assert_eq!(
        response(&mut world, |reply_to| Event::Control {
            reply_to,
            by: DIRECTOR,
            task: FIRST_RUN,
            control: Control::Cancel { reason: Box::new([1]), grace: None, message: None },
        }),
        Reply::Done
    );
    assert_eq!(world.terminal(FIRST_RUN, End::Parked), Reply::Acknowledged(Accepted::New));
    world.settle(FIRST_RUN);
    assert!(matches!(world.results.get(&FIRST_RUN), Some(Ending::Cancelled { .. })));

    let added_at = world.env.wall;
    assert_eq!(amend(&mut world, DIRECTOR, T4R, &[T4B]), Reply::Done);
    assert_eq!(world.record(T4R).waiting_on.as_ref(), [T4B]);
    assert_eq!(world.runs.get(&T4R), Some(&1));
    assert_eq!(world.record(T4R).attempt, 1);
    assert!(world.activations.is_empty());
    assert_eq!(
        world.make(DIRECTOR, vec![task(DIRECTORIES, &[T4R]), task(FOLLOWUP, &[T4R])]),
        Reply::Made(vec![DIRECTORIES, FOLLOWUP])
    );
    for task in [DIRECTORIES, FOLLOWUP] {
        assert_eq!(world.record(task).phase, Phase::Waiting);
    }
    assert!(world.activations.is_empty());
    refused(amend(&mut world, DIRECTOR, T4B, &[T4R]), T4B, Refusal::Circle { cut: false }, &[T4B, T4R]);

    parked(&mut world, T4R, T4B);
    for task in [DIRECTORIES, FOLLOWUP] {
        assert_eq!(world.record(task).phase, Phase::Waiting);
    }
    finish(&mut world, T4B);
    met(&world, T4R, T4B);
    world.claim(T4R, 2);
    let context = world.contexts.get(&T4R).expect("second run context");
    assert_eq!(world.record(T4R).attempt, 2);
    assert!(context.pending.is_empty());
    assert_eq!(context.added.as_ref(), [Added { task: T4B, at: added_at, by: DIRECTOR }]);
    assert_eq!(context.dependencies.as_ref(), [T4B]);
    finish(&mut world, T4R);

    // The ready set contains only the dependents; claim them in creation order.
    assert_eq!(world.activations.iter().copied().collect::<Vec<_>>(), [DIRECTORIES, FOLLOWUP]);
    for task in [DIRECTORIES, FOLLOWUP] {
        assert_eq!(world.record(task).phase, Phase::Active(Active::Due));
        world.claim(task, 1);
        finish(&mut world, task);
    }
    assert_eq!(world.live(), 0);
    assert_eq!(world.results.len(), 5);
    world.complete_cancel();
}

// domain/tasks.md, 5.7, 7, 8.1 and 12.
#[test]
fn an_owner_adds_a_dependency_its_child_found_while_working() {
    const OWNER: u64 = 1;
    const CHILD: u64 = 2;
    const FOUND: u64 = 3;
    let mut world = World::new(881, LIMITS);
    assert_eq!(world.make(DIRECTOR, vec![task(OWNER, &[])]), Reply::Made(vec![OWNER]));
    let mut children = Vec::new();
    for number in [CHILD, FOUND] {
        let mut child = task(number, &[]);
        child.funder = Funder::Task(OWNER);
        child.numbers.budget = 30;
        child.authority.budget.spend = 30;
        children.push(child);
    }
    assert_eq!(world.make(Party::Task(OWNER), children), Reply::Made(vec![CHILD, FOUND]));
    world.claim(OWNER, 1);
    world.claim(CHILD, 1);
    world.claim(FOUND, 1);
    // The child that found the dependency may not add it: only a requester, a
    // task above and a party may amend (section 7, "Who may"), and the owner's
    // subtree reaches both children. One that may amend but not reach is
    // a_task_amender_may_name_any_live_task_below_it_but_not_an_outside_task.
    let Reply::Refused(problem) = amend(&mut world, Party::Task(CHILD), CHILD, &[FOUND]) else {
        panic!("a task amending itself is refused");
    };
    assert_eq!((problem.task, problem.why), (Some(CHILD), Refusal::Reference));
    assert!(world.record(CHILD).dependencies.is_empty());
    assert_eq!(amend(&mut world, Party::Task(OWNER), CHILD, &[FOUND]), Reply::Done);
    assert_eq!(world.runs.get(&CHILD), Some(&1));
    parked(&mut world, CHILD, FOUND);
    finish(&mut world, FOUND);
    met(&world, CHILD, FOUND);
    world.claim(CHILD, 2);
    finish(&mut world, CHILD);
    finish(&mut world, OWNER);
    assert_eq!(world.live(), 0);
    assert_eq!(world.results.len(), 3);
    assert!(!world.records.contains_key(&Key::Live(OWNER)));
    world.complete_cancel();
}
