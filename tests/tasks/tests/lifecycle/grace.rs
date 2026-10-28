use jig_domain_tasks::{
    Amendment, Control, Ending, Event, Executor, Key, MessageKind, Party, PersonAddress, Phase, Problem, Refusal,
    Stored, Word,
};
use jig_tasks_world::{LIMITS, Reply, World, task};
use skein_lib::Duration;

const TASK: u64 = 1;
const DEPENDENCY: u64 = 2;
const REQUESTER: u64 = 1;
const HOLDER: u64 = 3;
const FIRST_WORD: u64 = 1;

fn cancel(world: &mut World, grace: Option<Duration>, reason: &[u8]) -> Reply {
    let reply_to = world.to();
    let message = world.record(TASK).last_message + 1;
    world.send(Event::Control {
        reply_to,
        by: Party::Person(REQUESTER),
        task: TASK,
        control: Control::Cancel { reason: reason.into(), grace, message: grace.map(|_| message) },
    });
    world.replies.last_key_value().expect("cancel answered").1.clone()
}

fn world() -> World {
    let mut world = World::new(190, LIMITS);
    world.make(Party::Person(REQUESTER), vec![task(TASK, &[])]);
    world
}

fn ended(world: &World) -> &Ending {
    let Stored::Ended(row) = &world.records[&Key::Ended(TASK)] else { panic!("ended row") };
    let Phase::Ended(ending) = &row.phase else { panic!("ended phase") };
    ending
}

fn amend(world: &mut World) {
    let reply_to = world.to();
    let message = world.record(TASK).last_message + 1;
    world.send(Event::Amend {
        places: Box::new([]),
        funder: None,
        reply_to,
        by: Party::Person(REQUESTER),
        task: TASK,
        message,
        stop_run: false,
        amendment: Amendment {
            tracked: None,
            addressed: None,
            instructions: None,
            release: None,
            spec: None,
            wake: None,
            dependencies: None,
            authority: None,
            llm: None,
            reason: b"ordinary amendment".as_slice().into(),
        },
    });
}

#[test]
fn a_grace_kept_with_its_task_ends_on_time_after_a_restart() {
    let mut world = world();
    cancel(&mut world, Some(Duration::from_secs(5)), b"wrap up");
    world.restart();
    world.elapse(Duration::from_secs(4));
    assert!(matches!(world.record(TASK).phase, Phase::Active(_)));
    world.elapse(Duration::from_secs(1));
    world.settle(TASK);
    assert!(matches!(ended(&world), Ending::Cancelled { by, reason, .. }
        if *by == Party::Person(REQUESTER) && reason.as_ref() == b"wrap up"));
}

#[test]
fn a_cancel_with_a_grace_asks_the_task_to_finish_first() {
    let mut world = world();
    assert_eq!(cancel(&mut world, Some(Duration::from_secs(5)), b"wrap up"), Reply::Done);
    let row = world.record(TASK);
    assert!(matches!(row.phase, Phase::Active(_)));
    assert_eq!(row.inbox.len(), 1);
    assert_eq!(row.inbox[0].kind, MessageKind::FinishRequested { revision: 1 });
    assert_eq!(row.inbox[0].number, FIRST_WORD);
    assert_eq!(row.inbox[0].words.as_ref(), b"wrap up");
}

#[test]
fn a_grace_longer_than_the_limit_is_refused_and_changes_nothing() {
    let mut world = world();
    let before = world.records.clone();
    assert_eq!(
        cancel(&mut world, Some(Duration::from_secs(601)), b"wrap up"),
        Reply::Refused(Problem::new(Some(TASK), Refusal::Grace))
    );
    assert_eq!(world.records, before);
}

#[test]
fn a_waiting_task_cancelled_with_a_grace_ends_at_once() {
    let mut world = World::new(191, LIMITS);
    world.make(Party::Person(REQUESTER), vec![task(TASK, &[DEPENDENCY]), task(DEPENDENCY, &[])]);
    assert_eq!(world.record(TASK).phase, Phase::Waiting);
    cancel(&mut world, Some(Duration::from_secs(5)), b"wrap up");
    world.settle(TASK);
    assert!(matches!(ended(&world), Ending::Cancelled { reason, .. } if reason.as_ref() == b"wrap up"));
}

#[test]
fn a_cancel_without_a_grace_during_one_cancels_at_once() {
    let mut world = world();
    cancel(&mut world, Some(Duration::from_secs(5)), b"wrap up");
    cancel(&mut world, None, b"now");
    world.settle(TASK);
    assert!(matches!(ended(&world), Ending::Cancelled { reason, .. } if reason.as_ref() == b"now"));
}

#[test]
fn a_second_grace_never_ends_the_task_later_than_the_first() {
    let mut world = world();
    cancel(&mut world, Some(Duration::from_secs(5)), b"wrap up");
    let before = world.records.clone();
    cancel(&mut world, Some(Duration::from_secs(10)), b"later");
    assert_eq!(world.records, before);
    assert_eq!(world.record(TASK).inbox.len(), 1);
    world.elapse(Duration::from_secs(5));
    world.settle(TASK);
    assert!(matches!(ended(&world), Ending::Cancelled { reason, .. } if reason.as_ref() == b"wrap up"));
}

#[test]
fn a_sooner_second_grace_ends_the_task_with_its_own_reason() {
    let mut world = world();
    cancel(&mut world, Some(Duration::from_secs(5)), b"wrap up");
    cancel(&mut world, Some(Duration::from_secs(2)), b"now");
    assert_eq!(world.record(TASK).inbox.len(), 1);
    assert_eq!(world.record(TASK).inbox[0].words.as_ref(), b"wrap up");
    world.restart();
    world.elapse(Duration::from_secs(2));
    world.settle(TASK);
    assert!(matches!(ended(&world), Ending::Cancelled { reason, .. } if reason.as_ref() == b"now"));
}

#[test]
fn a_graces_ask_to_finish_is_admitted_into_a_full_inbox() {
    let mut world = world();
    for message in FIRST_WORD..=u64::from(LIMITS.inbox_messages) {
        let reply_to = world.to();
        world.send(Event::Message {
            reply_to,
            project: 1,
            task: TASK,
            word: Word {
                number: message,
                from: Party::Person(REQUESTER),
                kind: MessageKind::Words,
                words: b"waiting".as_slice().into(),
                at: world.env.wall,
                hits: 1,
                eligible: true,
            },
        });
    }
    assert_eq!(world.record(TASK).inbox.len(), usize::try_from(LIMITS.inbox_messages).expect("small inbox bound"));
    assert_eq!(cancel(&mut world, Some(Duration::from_secs(5)), b"wrap up"), Reply::Done);
    assert_eq!(cancel(&mut world, Some(Duration::from_secs(10)), b"later"), Reply::Done);
    assert_eq!(world.record(TASK).inbox.len(), usize::try_from(LIMITS.inbox_messages).expect("small inbox bound") + 1);
    world.restart();
    assert_eq!(world.record(TASK).inbox.len(), usize::try_from(LIMITS.inbox_messages).expect("small inbox bound") + 1);
}

#[test]
fn a_graces_ask_never_merges_into_a_waiting_amendment() {
    let mut world = world();
    amend(&mut world);
    cancel(&mut world, Some(Duration::from_secs(5)), b"wrap up");
    amend(&mut world);
    world.restart();
    let inbox = &world.record(TASK).inbox;
    assert_eq!(inbox.len(), 2);
    assert_eq!(inbox[0].kind, MessageKind::FinishRequested { revision: 2 });
    assert_eq!(inbox[1].kind, MessageKind::Amendment { revision: 3 });
    assert_eq!(inbox[0].words.as_ref(), b"wrap up");
    assert_eq!(inbox[1].words.as_ref(), b"ordinary amendment");
}

#[test]
fn a_person_task_cancelled_with_a_grace_shows_its_holder_the_ask() {
    let mut world = World::new(192, LIMITS);
    let mut new = task(TASK, &[]);
    new.executor = Executor::Person(PersonAddress::Role(1));
    world.make(Party::Person(REQUESTER), vec![new]);
    let reply_to = world.to();
    world.send(Event::TakePerson { reply_to, task: TASK, person: HOLDER });
    cancel(&mut world, Some(Duration::from_secs(5)), b"wrap up");
    assert_eq!(world.record(TASK).taken_by, Some(HOLDER));
    assert_eq!(world.record(TASK).inbox[0].kind, MessageKind::FinishRequested { revision: 1 });
    assert_eq!(world.record(TASK).inbox[0].words.as_ref(), b"wrap up");
    world.elapse(Duration::from_secs(5));
    world.settle(TASK);
    assert!(matches!(ended(&world), Ending::Cancelled { reason, .. } if reason.as_ref() == b"wrap up"));
}

#[test]
fn a_graces_ask_keeps_room_for_the_whole_cancellation_reason() {
    let mut limits = LIMITS;
    limits.message_bytes = 16;
    let mut world = World::new(193, limits);
    world.make(Party::Person(REQUESTER), vec![task(TASK, &[])]);
    assert_eq!(cancel(&mut world, Some(Duration::from_secs(5)), &[b'x'; 32]), Reply::Done);
    world.restart();
    assert_eq!(world.record(TASK).inbox[0].words.as_ref(), &[b'x'; 32]);
}

#[test]
fn a_restored_finish_word_loads_without_judging_its_grace() {
    const RESTORE_SEED: u64 = 194;
    let mut world = world();
    cancel(&mut world, Some(Duration::from_secs(5)), b"wrap up");
    let original = world.record(TASK).clone();
    let mut missing = original.clone();
    missing.grace = None;
    let mut wrong_name = original.clone();
    wrong_name.grace.as_mut().expect("grace fixture").message += 1;
    let mut ordinary = original;
    ordinary.inbox[0].kind = MessageKind::Words;
    for record in [missing, wrong_name, ordinary] {
        let mut domain = jig_tasks_world::domain(&LIMITS, RESTORE_SEED);
        let mut out = skein_lib::Queue::with_capacity(jig_domain_tasks::max_out(&LIMITS));
        let record = Stored::Live(Box::new(record));
        jig_domain_tasks::step(&mut domain, &world.env, Event::Restore { record: record.clone() }, &mut out);
        assert!(out.is_empty());
        assert_eq!(domain.stored_record(&record.key()), Some(record));
    }
}
