use jig_core_world::Fault;
use jig_core_world::effects::World;
use jig_core_world::fixtures::finish_chat;
use jig_domain_core as core;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{ReplyTo, Token};

#[test]
fn a_held_effect_commit_holds_the_system_and_the_call_answer() {
    let mut world = World::new(121, false);
    let answers = world.answers.len();
    world.store.fault(Fault::Hold { commits: 1 });
    world.call(1, 800);
    assert!(!world.store.held.is_empty());
    assert!(world.systems[0].observed().is_empty());
    assert_eq!(world.answers.len(), answers);
    world.release_commits();
    assert_eq!(world.systems[0].observed().iter().filter(|row| row.applied).count(), 1);
    assert_eq!(world.answers.len(), answers + 1);
}

#[test]
fn a_failed_commit_stops_the_root_before_its_effect_or_answer_leaves() {
    let mut world = World::new(122, false);
    let durable = world.store.applied;
    let rows = world.store.rows.clone();
    let answers = world.answers.len();
    world.store.fault(Fault::Fail { commit: durable + 1 });
    world.call(1, 801);
    assert!(world.stopped);
    assert_eq!(world.store.applied, durable);
    assert_eq!(world.store.rows, rows);
    assert!(world.systems[0].observed().is_empty());
    assert_eq!(world.answers.len(), answers);
    world.restart(122, false);
    assert!(world.domain.ready());
    assert!(!world.stopped);
    assert!(world.systems[0].observed().is_empty());
}

#[test]
fn slow_pages_delay_a_cold_start_without_skipping_the_live_rows() {
    let mut world = World::new(123, false);
    world.enable_tracing();
    world.turn(1, 3, None, b"kept before slow restart");
    world.call(1, 802);
    let durable = world.store.applied;
    let started = world.wall_time();
    world.store.fault(Fault::SlowPages { by: 3 });
    world.restart(123, false);
    assert!(world.domain.ready());
    assert!(world.wall_time().as_nanos() > started.as_nanos() + 10_000_000);
    assert!(world.store.applied >= durable);
    assert!(world.store.rows.values().any(|row| matches!(row,
        root::Record::Core(core::Record::Core(core::CoreRecord::Turn(turn))) if turn.transcript.as_ref() == b"kept before slow restart"
    )));
    assert!(world.trace.iter().any(|line| line.starts_with("store page LoadCore")));
    assert_eq!(world.systems[0].observed().iter().filter(|row| row.applied).count(), 1);
}

#[test]
fn a_crash_before_a_turn_is_durable_loses_only_its_uncommitted_turn() {
    let mut world = World::new(124, false);
    let assignment = world.assigned.as_ref().expect("assigned caller");
    let task = assignment.task;
    let attempt = assignment.attempt;
    world.store_delay = 500;
    submit_delayed_turn(&mut world, task, attempt);
    assert!(!world.store.pending.is_empty());
    world.store_delay = 0;
    world.restart(124, false);
    assert!(world.store.pending.is_empty());
    assert!(!world.store.rows.contains_key(&root::Key::Core(core::Key::Core(core::CoreKey::Turn {
        task,
        attempt,
        turn: 1
    }))));
    assert!(world.domain.ready());
}

#[test]
fn a_crash_after_durability_before_acknowledgement_restores_the_turn() {
    let mut world = World::new(125, false);
    world.store.fault(Fault::Hold { commits: 1 });
    world.turn(1, 3, None, b"durable without acknowledgement");
    assert!(!world.store.held.is_empty());
    let durable = world.store.applied;
    world.restart(125, false);
    assert!(world.store.held.is_empty());
    assert!(world.store.applied >= durable);
    assert!(world.store.rows.values().any(|row| matches!(row,
        root::Record::Core(core::Record::Core(core::CoreRecord::Turn(turn))) if turn.transcript.as_ref() == b"durable without acknowledgement"
    )));
}

#[test]
fn parties_run_ahead_of_held_completions_but_every_answer_waits() {
    let mut world = World::new(126, false);
    let task = world.call_key(1).task;
    let answers = world.people_answers.len();
    world.store.fault(Fault::Hold { commits: 1 });
    for request in [70, 71] {
        world.send(root::Event::Core(core::Event::People(jig_domain_people::Event::Ask {
            models: Box::new([]),
            reply_to: ReplyTo::new(Token::new(u64::from(request))),
            sign_in: world.sign_in.expect("signed in"),
            key: [request; 16],
            ask: jig_domain_people::Ask::Say { project: 1, task, words: Box::from([request]) },
        })));
    }
    assert!(world.store.held.len() >= 2);
    assert_eq!(world.people_answers.len(), answers);
    assert!(
        !world
            .store
            .rows
            .values()
            .any(|row| matches!(row, root::Record::Core(core::Record::Tasks(tasks::Stored::Ended(_)))))
    );
    world.release_commits();
    assert_eq!(world.people_answers.len(), answers + 2);
}

#[test]
fn a_runs_read_is_answered_while_an_unrelated_commit_is_in_flight() {
    let mut world = World::new(128, false);
    world.store.fault(Fault::Hold { commits: 1 });
    world.turn(1, 1, None, b"unrelated turn");
    let held = world.store.held.clone();
    assert!(!held.is_empty());
    let commits = world.commits.len();
    let right = world.host_call(b"read-now", b"read", b"{}", false);
    let answer = core::SettledCall {
        serial: 0,
        tool: b"read".as_slice().into(),
        answer: core::SettledAnswer::Host { error: false, body: b"fresh result".as_slice().into() },
    };
    world.send(root::Event::CallAnswered { to: ReplyTo::new(right), call: answer.clone() });
    assert_eq!(world.store.held, held, "the unrelated commit still waits");
    assert_eq!(world.commits.len(), commits, "a read commits nothing");
    assert_eq!(world.host_answers, [(b"read-now".as_slice().into(), answer)]);
    world.release_commits();
    assert_eq!(world.host_answers.len(), 1, "the read is answered once");
}

#[test]
fn a_runs_write_answer_waits_for_an_unrelated_commit_in_flight() {
    let mut world = World::new(129, false);
    world.store.fault(Fault::Hold { commits: 1 });
    world.turn(1, 1, None, b"unrelated turn");
    let right = world.host_call(b"write-held", b"write", b"{}", true);
    let key = world.call_key(2);
    world.send(root::Event::Core(core::Event::NamedAnswer {
        to: ReplyTo::new(right),
        key,
        part: core::CallPart::Unavailable,
    }));
    world.send(root::Event::Core(core::Event::SettledCall {
        to: ReplyTo::new(right),
        key,
        name: b"write-held".as_slice().into(),
        call: core::SettledCall {
            serial: 0,
            tool: b"write".as_slice().into(),
            answer: core::SettledAnswer::Host { error: false, body: b"done".as_slice().into() },
        },
    }));
    assert!(world.host_answers.is_empty(), "writes wait behind the earlier commit");
    world.release_commits();
    assert_eq!(world.host_answers.len(), 1);
}

fn start_chat(world: &mut World, number: u8) {
    jig_core_world::stories::reads::request(
        world,
        number,
        jig_core_world::fixtures::chat(world, 1, b"new chat".as_slice().into()),
    );
}

fn filled_chat(seed: u64) -> World {
    let mut world = World::new(seed, false);
    world.fill = jig_core_world::world::limits().core.fill_mark + 1;
    world.turn(1, 1, None, b"fill reported with this commit");
    world
}

#[test]
fn a_running_chat_goes_on_above_the_fill_mark() {
    let mut world = filled_chat(130);
    let commits = world.store.applied;
    start_chat(&mut world, 80);
    assert_eq!(world.people_answers.last(), Some(&jig_domain_people::Reply::Again(jig_domain_people::Again::Busy)));
    assert_eq!(world.store.applied, commits, "a fill refusal makes no commit");
    world.turn(2, 2, None, b"the running chat continues");
    assert!(world.store.rows.values().any(|row| matches!(row,
        root::Record::Core(core::Record::Core(core::CoreRecord::Turn(turn)))
            if turn.turn == 2 && turn.transcript.as_ref() == b"the running chat continues"
    )));
    assert!(world.busy_turns.is_empty());
}

#[test]
fn a_fill_below_the_mark_admits_a_chat_again() {
    let mut world = filled_chat(131);
    world.fill = jig_core_world::world::limits().core.fill_mark - 1;
    finish_chat(&mut world, 1, b"finished chat");
    start_chat(&mut world, 81);
    assert!(
        matches!(
            world.people_answers.last(),
            Some(jig_domain_people::Reply::Outcome(jig_domain_people::Outcome::Created { .. }))
        ),
        "{:?}",
        world.people_answers.last()
    );
}

#[test]
fn a_seen_conversation_is_read_back_through_the_store_after_restart() {
    let mut world = World::new(127, false);
    let task = world.assigned.as_ref().expect("the world's chat").task;
    world.turn(1, 3, None, b"the chat's reply");
    finish_chat(&mut world, 3, b"done");
    world.restart(127, false);
    let seen = jig_domain_people::Seen::Conversation { task, through: 1 };
    jig_core_world::stories::reads::request(&mut world, 62, jig_domain_people::Ask::MarkSeen { seen });
    assert_eq!(
        world.people_answers.last(),
        Some(&jig_domain_people::Reply::Outcome(jig_domain_people::Outcome::MarkedSeen { seen }))
    );
}
#[test]
fn a_historical_escalation_is_read_through_the_store() {
    let mut world = World::new(132, false);
    let task = world.assigned.as_ref().expect("the world's chat").task;
    world.send(root::Event::Core(core::Event::Tasks(tasks::Event::Hold { task, why: tasks::Hold::Stalled })));
    world.end(tasks::End::Parked, 0);
    let root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) =
        &world.store.rows[&root::Key::Core(core::Key::Tasks(tasks::Key::Live(task)))]
    else {
        panic!("held chat");
    };
    let revision = row.revisions.hold;
    release_escalation(&mut world, 64, task, revision);
    let answer = world.people_answers.last().cloned();
    assert!(matches!(
        answer,
        Some(jig_domain_people::Reply::Outcome(jig_domain_people::Outcome::EscalationDecided { .. }))
    ));
    world.restart(132, false);
    world.enable_tracing();
    release_escalation(&mut world, 65, task, revision);
    assert!(
        world.trace.iter().any(|line| line.starts_with("output Now(HistoricalEscalation")),
        "historical store lookup"
    );
    assert_eq!(world.people_answers.last().cloned(), answer);
}

fn release_escalation(world: &mut World, key: u8, task: u64, revision: u64) {
    jig_core_world::stories::reads::request(
        world,
        key,
        jig_domain_people::Ask::DecideEscalation {
            project: 1,
            task,
            revision,
            decision: jig_domain_people::EscalationDecision::Release,
        },
    );
}

fn submit_delayed_turn(world: &mut World, task: u64, attempt: u64) {
    // One iteration is driven directly so the crash precedes storage application.
    let env = skein_lib::Env {
        now: skein_lib::Time::from_nanos(world.wall_time().as_nanos()),
        wall: skein_lib::Wall::from_nanos(world.wall_time().as_nanos()),
        limits: jig_core_world::world::limits(),
    };
    root::step(
        &mut world.domain,
        &env,
        root::Event::Turn {
            opens_window: false,
            refused: Box::new([]),
            channel: Token::new(7),
            task,
            attempt,
            turn: 1,
            cumulative: 3,
            read: None,
            transcript: b"not durable".as_slice().into(),
        },
    );
    let mut out = skein_lib::Queue::with_capacity(128);
    root::release(&mut world.domain, &env, &mut out);
    while let Some(request) = out.pop() {
        match request {
            root::Request::Commit { number, mut writes } => {
                let mut rows = Vec::new();
                while let Some(row) = writes.pop() {
                    rows.push(row);
                }
                world.store.submit(number, rows.into_boxed_slice(), 500);
            }
            other @ (root::Request::Restart(_)
            | root::Request::Deliver(_)
            | root::Request::ReadResult { .. }
            | root::Request::Now(_)
            | root::Request::Busy { .. }
            | root::Request::Stop) => panic!("nothing leaves before the turn commit: {other:?}"),
        }
    }
}
