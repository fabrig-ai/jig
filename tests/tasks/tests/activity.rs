use jig_domain_tasks::{Accepted, ActivityLine, Event, Key, Line, MessageKind, Party, Stored, Word};
use jig_tasks_world::{LIMITS, Reply, World, task};
use skein_lib::Wall;

fn lines(world: &World, task: u64) -> Vec<ActivityLine> {
    world
        .records
        .values()
        .filter_map(|row| match row {
            Stored::Activity(row) if row.task == task => Some(row.clone()),
            Stored::Activity(_)
            | Stored::Live(_)
            | Stored::Ended(_)
            | Stored::Ledger(_)
            | Stored::Writer(_)
            | Stored::Result(_)
            | Stored::EndedByParty(_)
            | Stored::EndedByProject(_)
            | Stored::EndedDelegate(_)
            | Stored::Delegate(_)
            | Stored::Update(_)
            | Stored::Pool(_)
            | Stored::ClosedLedger(_)
            | Stored::DecidedProposal(_)
            | Stored::Stub(_)
            | Stored::Milestone(_)
            | Stored::History(_)
            | Stored::PlacedHold(_)
            | Stored::PersonProposal(_) => None,
        })
        .collect()
}

fn write(world: &mut World, number: u64, words: &[u8]) {
    let reply_to = world.to();
    world.send(Event::Message {
        reply_to,
        project: 1,
        task: 1,
        word: Word {
            number,
            from: Party::Person(9),
            kind: MessageKind::Words,
            words: words.into(),
            at: world.env.wall,
            hits: 1,
            eligible: true,
        },
    });
}

fn take(world: &mut World, turn: u32, read: u64, cumulative: u64) {
    let reply_to = world.to();
    let request = reply_to.into_token();
    world.send(Event::Turn {
        refused: Box::new([]),
        reply_to: skein_lib::ReplyTo::new(request),
        task: 1,
        attempt: 7,
        turn,
        read: Some(read),
        offered: Some(read),
        outstanding: Box::new([]),
        cumulative,
    });
    assert_eq!(world.replies[&request.raw()], Reply::Turn(Accepted::New));
    assert!(world.record(1).inbox.is_empty());
}

#[test]
fn a_task_s_activity_survives_its_words_being_taken_and_a_restart() {
    let mut world = World::new(505, LIMITS);
    world.make(Party::Person(9), vec![task(1, &[])]);
    world.env.wall = Wall::from_nanos(500);
    write(&mut world, 1, b"the complete first message");
    world.claim(1, 7);
    take(&mut world, 1, 1, 3);
    let before = lines(&world, 1);
    let position = world.record(1).activity;
    assert_eq!(before.last().expect("activity before restart").position, position);
    world.restart();
    assert_eq!(world.record(1).activity, position);
    assert_eq!(lines(&world, 1), before);
    // A wall correction does not reorder immutable committed positions.
    world.env.wall = Wall::from_nanos(100);
    write(&mut world, 2, b"the complete second message");
    take(&mut world, 2, 2, 5);
    world.finish(1);
    world.settle(1);
    let after = lines(&world, 1);
    assert_eq!(&after[..before.len()], before.as_slice());
    for (index, row) in after.iter().enumerate() {
        assert_eq!(row.position, u64::try_from(index).expect("activity count fits u64") + 1);
    }
    for (message, words, at) in
        [(1, b"the complete first message".as_slice(), 500), (2, b"the complete second message".as_slice(), 100)]
    {
        assert!(after.iter().any(|row| row.at == Wall::from_nanos(at) && matches!(&row.line, Line::Words { message: kept, by: Party::Person(9), words: kept_words, .. } if *kept == message && kept_words.as_ref() == words)));
    }
    assert!(
        after
            .iter()
            .any(|row| row.line == Line::RunEnded { attempt: 7, end: jig_domain_tasks::EndKind::Finished, spent: 5 })
    );
    let Stored::Ended(ended) = &world.records[&Key::Ended(1)] else { panic!("ended row kept") };
    assert_eq!(ended.activity, after.last().expect("ended activity").position);
    world.restart();
    assert_eq!(world.live(), 0);
    assert_eq!(lines(&world, 1), after);
}

#[test]
fn a_tasks_label_and_flags_are_kept_as_made() {
    let mut world = World::new(331, LIMITS);
    let mut made = task(1, &[]);
    made.label = 7;
    let flags = jig_domain_tasks::Flags {
        ended_by: jig_domain_tasks::EndedBy::Result,
        conversation: true,
        listed: false,
        retention: jig_domain_tasks::Retention::ForgottenOnceRead,
    };
    made.flags = flags;
    assert_eq!(world.make(Party::Person(9), vec![made]), Reply::Made(vec![1]));
    world.claim(1, 7);
    world.restart();
    assert_eq!(world.record(1).label, 7);
    assert_eq!(world.record(1).flags, flags);
}

#[test]
fn a_tasks_last_activity_is_when_it_was_made() {
    let mut world = World::new(332, LIMITS);
    world.env.wall = Wall::from_nanos(500);
    assert_eq!(world.make(Party::Person(9), vec![task(1, &[])]), Reply::Made(vec![1]));
    assert_eq!(world.record(1).last_activity, Wall::from_nanos(500));
}

#[test]
fn a_tasks_last_activity_moves_when_it_hears_words() {
    let mut world = World::new(333, LIMITS);
    world.make(Party::Person(9), vec![task(1, &[])]);
    world.env.wall = Wall::from_nanos(500);
    write(&mut world, 1, b"heard later");
    assert_eq!(world.record(1).last_activity, Wall::from_nanos(500));
}

#[test]
fn a_tasks_last_activity_moves_when_its_run_commits_a_turn() {
    let mut world = World::new(334, LIMITS);
    world.make(Party::Person(9), vec![task(1, &[])]);
    write(&mut world, 1, b"heard earlier");
    world.claim(1, 7);
    world.env.wall = Wall::from_nanos(500);
    take(&mut world, 1, 1, 3);
    assert_eq!(world.record(1).last_activity, Wall::from_nanos(500));
}

#[test]
fn a_requesters_last_activity_moves_when_a_delegate_ends() {
    let mut world = World::new(335, LIMITS);
    world.make(Party::Person(9), vec![task(1, &[])]);
    assert_eq!(world.make(Party::Task(1), vec![task(2, &[])]), Reply::Made(vec![2]));
    world.claim(2, 7);
    world.env.wall = Wall::from_nanos(500);
    world.finish(2);
    world.settle(2);
    assert_eq!(world.record(1).last_activity, Wall::from_nanos(500));
}
