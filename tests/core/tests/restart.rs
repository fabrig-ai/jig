const FIRST_GOAL: u8 = 151;
const CHAT: u8 = 152;
const TURN: u32 = 1;
const BEFORE: u64 = 153;
const AFTER: u64 = 154;
use jig_core_world::effects::World;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_tasks as tasks;
use jig_test_connector as connector;
use jig_test_domain as root;
use jig_test_system::Fault;
use skein_lib::{Duration, Token};

fn hello(world: &mut World, hosting: Box<[fleet::Hosted]>) {
    world.send(root::Event::Core(core::Event::Fleet(fleet::Event::Hello {
        worker: 0,
        channel: Token::new(7),
        hello: fleet::Hello {
            bounds: jig_core_world::fixtures::bounds(),
            stop_bound: Duration::from_millis(100),
            slots: 1,
            workstreams: Box::new([]),
            hosting,
        },
    })));
}

fn account(world: &mut World) {
    grant_account(world, 1, 1);
}

fn grant_account(world: &mut World, account: u32, generation: u64) {
    world.send(root::Event::Core(core::Event::Account(jig_domain_accounts::Event::Component(
        jig_domain_accounts::FromComponent::Granted {
            form: jig_domain_accounts::Form::Credential(jig_domain_accounts::Scheme::Bearer),
            account,
            generation,
            valid: Duration::from_secs(60),
        },
    ))));
}

fn hosting(world: &World) -> Box<[fleet::Hosted]> {
    let assigned = world.assigned.as_ref().expect("a live chat");
    Box::new([fleet::Hosted {
        run: Token::new(assigned.task),
        attempt: Token::new(assigned.attempt),
        phase: fleet::Phase::Active,
    }])
}

fn received(world: &World) -> fleet::Grant {
    world.received_grant().expect("host received a grant").3
}

fn applied_without_answer(world: &mut World) {
    world.hold_writes = true;
    world.call(1, 400);
    let (number, call) = world.pending_writes.remove(0);
    let _lost = world.systems[usize::from(number - 1)].answer(call, Fault::AfterApply);
    world.hold_writes = false;
}

#[test]
fn the_restart_script_waits_for_each_step_and_each_connector() {
    let limits = jig_core_world::world::limits().core;
    let config = jig_core_world::world::config(111).core;
    let mut core = core::Core::new(config, &limits).expect("fixture core configuration");
    let steps = [
        core::RestartStep::LoadCore,
        core::RestartStep::LoadInbox,
        core::RestartStep::RestoreConnector { connector: 1 },
        core::RestartStep::RestoreConnector { connector: 2 },
        core::RestartStep::Switch,
        core::RestartStep::AdoptRuns,
        core::RestartStep::ReadAfresh { connector: 1 },
        core::RestartStep::ReadAfresh { connector: 2 },
        core::RestartStep::SettleOutbox { connector: 1 },
        core::RestartStep::SettleOutbox { connector: 2 },
        core::RestartStep::Open,
    ];
    let mut request = core.restart_begin(false);
    for step in steps {
        assert_eq!(request, core::RestartRequest::Step(step));
        assert_eq!(core.restart_step(), Some(step));
        assert!(!core.restart_ready());
        assert_eq!(core.restart_begin(false), core::RestartRequest::Idle);
        request = core.restart_done(step);
    }
    assert_eq!(request, core::RestartRequest::Idle);
    assert!(core.restart_ready());
    assert_eq!(core.restart_done(core::RestartStep::Open), core::RestartRequest::Idle);
}

#[test]
fn an_out_of_order_restart_completion_refuses_the_start_naming_its_step() {
    let limits = jig_core_world::world::limits().core;
    let mut core =
        core::Core::new(jig_core_world::world::config(112).core, &limits).expect("fixture core configuration");
    assert_eq!(core.restart_begin(false), core::RestartRequest::Step(core::RestartStep::LoadCore));
    assert_eq!(
        core.restart_done(core::RestartStep::Open),
        core::RestartRequest::Refused { step: core::RestartStep::LoadCore }
    );
    assert_eq!(core.restart_failure(), Some(core::RestartStep::LoadCore));
    assert!(!core.restart_ready());
}

#[test]
fn an_uncertain_effect_across_restarts_is_made_before_its_retry_deadline_and_once() {
    let mut world = World::new(113, false);
    world.enable_tracing();
    applied_without_answer(&mut world);
    let (key, deadline) = world
        .store
        .rows
        .values()
        .find_map(|record| match record {
            root::Record::Connector { number: 1, record: connector::Record::Outbox(row) } => {
                Some((row.key, row.attempt.expect("attempt committed").deadline))
            }
            root::Record::Core(_) | root::Record::Connector { .. } => None,
        })
        .expect("sent outbox record");
    world.restart(113, false);
    assert!(world.domain.ready());
    assert!(world.store.rows.values().any(|record| matches!(record, root::Record::Connector { number: 1, record: connector::Record::Made { key: found, .. } } if *found==key)));
    assert!(
        world
            .store
            .rows
            .values()
            .any(|record| matches!(record, root::Record::Core(core::Record::Core(core::CoreRecord::Deployment(_)))))
    );
    let applied = world.systems[0].observed();
    assert_eq!(applied.iter().filter(|row| row.applied).count(), 1);
    assert!(world.trace.iter().any(|line| line.contains("Look")));
    assert!(world.store.rows.values().any(|record| matches!(record, root::Record::Core(core::Record::Core(core::CoreRecord::Call(row))) if matches!(row.part, core::CallPart::Effect { outcome: Some(core::connector::OutboxOutcome::Made), .. }))));
    assert!(deadline > world.wall_time(), "lookup resolved long before the saved retry deadline");
    world.restart(113, false);
    assert_eq!(world.systems[0].observed().iter().filter(|row| row.applied).count(), 1);
}

#[test]
fn nothing_is_decided_before_every_connector_has_settled_its_outbox() {
    let mut world = World::new(115, false);
    applied_without_answer(&mut world);
    world.end(tasks::End::Parked, 0);
    let mut seeded = Vec::new();
    for record in world.store.rows.values().cloned() {
        let mut record = record;
        if let root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) = &mut record {
            row.phase = tasks::Phase::Active(tasks::Active::Due);
            seeded.push(record);
        }
    }
    world.seed(seeded.into_boxed_slice());
    world.assigned = None;
    world.hold_lookups = true;
    world.restart(115, false);
    assert!(!world.domain.ready());
    assert_eq!(world.pending_lookups.len(), 1);
    hello(&mut world, Box::new([]));
    account(&mut world);
    assert!(world.assigned.is_none(), "a due run cannot be assigned while recovery waits");
    world.finish_lookups();
    assert!(world.domain.ready());
    assert!(world.assigned.is_some(), "the due run starts after every connector settled");
}

#[test]
fn a_restart_after_a_turn_committed_resumes_from_it() {
    let mut world = World::new(116, false);
    world.turn(1, 3, None, b"committed whole turn");
    let assignment = world.assigned.as_ref().expect("live worker");
    let hosted = fleet::Hosted {
        run: Token::new(assignment.task),
        attempt: Token::new(assignment.attempt),
        phase: fleet::Phase::Active,
    };
    world.restart(116, false);
    hello(&mut world, Box::new([hosted]));
    world.end(tasks::End::Parked, 3);
    account(&mut world);
    world.say(90);
    assert_eq!(
        world.assigned.as_ref().expect("resumed").transcript.as_ref(),
        [Box::<[u8]>::from(&b"committed whole turn"[..])]
    );
    assert_eq!(world.spent(), 3);
}

#[test]
fn an_engine_claim_is_settled_at_once_instead_of_waiting_for_a_worker() {
    let mut world = World::new(117, false);
    world.turn(1, 2, None, b"engine turn");
    let mut seeded = Vec::new();
    for record in world.store.rows.values().cloned() {
        let mut record = record;
        if let root::Record::Core(core::Record::Core(core::CoreRecord::RunProof(proof))) = &mut record {
            proof.host = fleet::HostKind::Engine;
            proof.placement = Some(Token::new(0));
            seeded.push(record);
        }
    }
    world.seed(seeded.into_boxed_slice());
    world.restart(117, false);
    assert!(world.store.rows.values().any(|record| matches!(record, root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) if row.tries.lost==1)));
    assert!(world.domain.ready());
}

#[test]
fn an_unfound_uncertain_entry_keeps_its_saved_retry_deadline_across_restarts() {
    let mut world = World::new(118, false);
    world.hold_writes = true;
    world.call(1, 401);
    let attempt = world
        .store
        .rows
        .values()
        .find_map(|record| match record {
            root::Record::Connector { record: connector::Record::Outbox(row), .. } => row.attempt,
            root::Record::Core(_) | root::Record::Connector { .. } => None,
        })
        .expect("durable attempt");
    world.restart(118, false);
    assert!(world.domain.ready());
    assert!(world.systems[0].observed().is_empty(), "not found does not retry before the saved deadline");
    assert!(world.store.rows.values().any(|record| matches!(record, root::Record::Connector { record: connector::Record::Outbox(row), .. } if row.attempt==Some(attempt))));
    world.restart(118, false);
    assert!(world.systems[0].observed().is_empty());
    assert!(world.store.rows.values().any(|record| matches!(record, root::Record::Connector { record: connector::Record::Outbox(row), .. } if row.attempt==Some(attempt))));
}

#[test]
fn an_uncertain_effect_released_without_making_it_again_tells_its_executor() {
    use jig_domain_people as people;
    use skein_lib::ReplyTo;
    let mut world = World::new(119, false);
    world.recovery = connector::Recovery::Unrecoverable;
    world.restart(119, false);
    let task = world.call_key(1).task;
    applied_without_answer(&mut world);
    world.end(tasks::End::Parked, 0);
    world.restart(119, false);
    assert!(world.domain.ready());
    assert!(world.store.rows.values().any(|record| matches!(record, root::Record::Connector { record: connector::Record::Outbox(row), .. } if row.phase==connector::EffectPhase::Held)));
    assert!(world.store.rows.values().any(|record| matches!(record, root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) if matches!(row.phase, tasks::Phase::Held { why: tasks::Hold::Uncertain { .. }, .. }))));
    assert_eq!(world.systems[0].observed().iter().filter(|row| row.applied).count(), 1);
    let root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) =
        &world.store.rows[&root::Key::Core(core::Key::Tasks(tasks::Key::Live(task)))]
    else {
        panic!("live held task")
    };
    let revision = row.revisions.hold;
    let sign_in = world.sign_in.expect("restored sign-in");
    world.send(root::Event::Core(core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(7001)),
        sign_in,
        key: [80; 16],
        ask: people::Ask::Release { project: 1, task, revision, again: None },
    })));
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Authority {
            proposable: None,
            lacking: people::Lack::Again { required: true }
        })))
    );
    let made = world.systems[0].observed().iter().filter(|row| row.applied).count();
    world.store.fault(jig_core_world::Fault::Hold { commits: 1 });
    let answers = world.people_answers.len();
    world.send(root::Event::Core(core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(7002)),
        sign_in,
        key: [81; 16],
        ask: people::Ask::Release { project: 1, task, revision, again: Some(false) },
    })));
    assert_eq!(world.people_answers.len(), answers, "release answer waits for the outbox decision");
    world.release_commits();
    assert_eq!(world.people_answers.last(), Some(&people::Reply::Outcome(people::Outcome::Released { task })));
    assert_eq!(world.systems[0].observed().iter().filter(|row| row.applied).count(), made);
    let root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) =
        &world.store.rows[&root::Key::Core(core::Key::Tasks(tasks::Key::Live(task)))]
    else {
        panic!("live released task")
    };
    assert!(
        row.inbox.iter().any(|word| matches!(word.kind, tasks::MessageKind::EffectNotMade { .. })),
        "the resumed executor reads the not-made outcome"
    );
    assert!(!world.store.rows.values().any(
        |row| matches!(row, root::Record::Connector { record: connector::Record::Outbox(row), .. } if row.task == task)
    ));
    assert!(world.store.rows.values().any(|row| matches!(row, root::Record::Core(core::Record::Core(core::CoreRecord::Call(row))) if row.key.task == task && matches!(row.part, core::CallPart::Effect { outcome: Some(core::connector::OutboxOutcome::NotMade), .. }))));
}

#[test]
fn a_refused_claim_keeps_the_conversation_for_the_next_resume() {
    let mut world = World::new(120, false);
    world.turn(1, 1, None, b"durable conversation");
    world.end(tasks::End::Parked, 1);
    let assignment = world.assigned.as_ref().expect("first assignment");
    let task = assignment.task;
    let key = root::Key::Core(core::Key::Core(core::CoreKey::RunProof(task)));
    let root::Record::Core(core::Record::Core(core::CoreRecord::RunProof(saved))) = &world.store.rows[&key] else {
        panic!("durable proof")
    };
    let limits = jig_core_world::world::limits().core;
    let mut core =
        core::Core::new(jig_core_world::world::config(120).core, &limits).expect("fixture core configuration");
    core.replaced.insert(task, saved.clone()).expect("one durable replacement");
    let mut pending = saved.clone();
    pending.attempt += 1;
    pending.conversation = pending.attempt;
    core.proofs.insert(task, pending.clone()).expect("one pending proof");
    core.claiming.insert(task, pending.attempt).expect("one pending claim");
    let env = skein_lib::Env { now: skein_lib::Time::ZERO, wall: skein_lib::Wall::EPOCH, limits };
    let _decision = core::step(&mut core, &env, core::Event::ClaimRefused { task });
    assert_eq!(core.proofs.get(&task), Some(saved));
    assert!(core.replaced.is_empty());
    core.begin_transcript(task, saved.attempt);
    assert_eq!(core.transcripts.get(&task).expect("next resume").from, saved.conversation);
}

#[test]
fn a_restart_restores_people_before_live_tasks_and_core_rows() {
    let mut world = World::new(120, false);
    world.restart(120, false);
    assert_eq!(
        world.restore_ranges,
        [
            core::RestoreRange::Deployment,
            core::RestoreRange::Settings,
            core::RestoreRange::Accounts,
            core::RestoreRange::People,
            core::RestoreRange::Tasks,
            core::RestoreRange::Core,
        ]
    );
    assert!(world.domain.ready());
}

#[test]
fn a_limited_source_keeps_its_reset_across_a_restart() {
    let mut world = World::new(121, false);
    account(&mut world);
    let (now, wall) = world.clocks();
    let reset = now.saturating_add(Duration::from_secs(300));
    world.send(root::Event::Core(core::Event::Account(jig_domain_accounts::Event::Limited {
        account: 1,
        reset: Some(reset),
    })));
    let limited = jig_domain_accounts::State::Limited {
        until: Some(jig_domain_accounts::Deadline {
            at: reset,
            wall: skein_lib::Wall::from_nanos(wall.as_nanos() + Duration::from_secs(300).as_nanos()),
        }),
    };
    assert_eq!(world.domain.accounts().source(1).expect("the configured source").state, limited);
    let row = root::Key::Core(core::Key::Accounts(jig_domain_accounts::Key::Source { account: 0, number: 1 }));
    assert!(world.store.rows.contains_key(&row), "the hold is committed with the decision that limits it");
    world.restart(121, false);
    assert!(world.domain.ready());
    let restored = world.domain.accounts();
    assert_eq!(restored.source(1).expect("the configured source").state, limited, "the same reset after the restart");
    assert_eq!(restored.unrestored(), 0);
}

fn restored_task(world: &World, task: u64) -> &tasks::TaskRecord {
    let row = &world.store.rows[&root::Key::Core(core::Key::Tasks(tasks::Key::Live(task)))];
    let root::Record::Core(core::Record::Tasks(tasks::Stored::Live(task))) = row else { panic!("live task") };
    task
}

#[test]
fn a_restart_with_workers_in_process_settles_their_claims_at_once() {
    let mut world = World::new(121, false);
    world.turn(1, 2, None, b"worker turn");
    let task = world.task();
    let before = world.wall_time().as_nanos();
    let mut config = jig_core_world::effects::fixture(121, false).0;
    config.core.workers_in_process = true;
    world.restart_with(config);
    assert!(world.domain.ready());
    assert_eq!(restored_task(&world, task).tries.lost, 1);
    assert!(world.wall_time().as_nanos() - before < Duration::from_secs(5).as_nanos());
}

#[test]
fn a_restart_with_workers_in_process_assigns_without_a_grace() {
    let mut world = World::new(122, false);
    let (task, attempt) = (world.task(), world.attempt());
    let before = world.wall_time().as_nanos();
    let mut config = jig_core_world::effects::fixture(122, false).0;
    config.core.workers_in_process = true;
    world.assigned = None;
    world.restart_with(config);
    assert_eq!(restored_task(&world, task).refusals, 1, "an unworked claim was not started");
    assert_eq!(restored_task(&world, task).tries.lost, 0);
    hello(&mut world, Box::new([]));
    account(&mut world);
    world.advance(Duration::from_millis(100));
    let assigned = world.assigned.as_ref().expect("next attempt passes only its retry backoff");
    assert_eq!(assigned.task, task);
    assert!(assigned.attempt > attempt);
    assert!(world.wall_time().as_nanos() - before < Duration::from_secs(5).as_nanos());
}

#[test]
fn a_restart_with_workers_elsewhere_still_waits_for_them() {
    let mut world = World::new(123, false);
    world.turn(1, 2, None, b"remote turn");
    let (task, attempt) = (world.task(), world.attempt());
    world.assigned = None;
    world.restart(123, false);
    account(&mut world);
    world.advance(Duration::from_millis(100));
    assert!(world.assigned.is_none());
    assert_eq!(restored_task(&world, task).tries.lost, 0);
    hello(
        &mut world,
        Box::new([fleet::Hosted { run: Token::new(task), attempt: Token::new(attempt), phase: fleet::Phase::Active }]),
    );
    assert_eq!(restored_task(&world, task).attempt, attempt);
    assert_eq!(restored_task(&world, task).phase, tasks::Phase::Active(tasks::Active::Running { attempt }));
    assert_eq!(restored_task(&world, task).tries.lost, 0);
}

#[test]
fn a_restart_keeps_the_window_a_turn_opened() {
    let mut world = World::new(125, false);
    world.turn(1, 1, None, b"earlier window");
    world.window_turn(2, 2, None, b"current window");
    let assignment = world.assigned.as_ref().expect("live worker");
    let (task, attempt) = (assignment.task, assignment.attempt);
    let hosted = fleet::Hosted { run: Token::new(task), attempt: Token::new(attempt), phase: fleet::Phase::Active };
    world.restart(125, false);
    hello(&mut world, Box::new([hosted]));
    world.end(tasks::End::Parked, 2);
    account(&mut world);
    world.say(92);
    let key = root::Key::Core(core::Key::Core(core::CoreKey::RunProof(task)));
    let root::Record::Core(core::Record::Core(core::CoreRecord::RunProof(proof))) = &world.store.rows[&key] else {
        panic!("restored conversation proof")
    };
    assert_eq!(proof.window, Some(core::CurrentWindow { attempt, turn: 2, closed: false }));
}

#[test]
fn a_restart_between_a_window_turn_and_its_resume_sends_the_window() {
    let mut world = World::new(124, false);
    world.turn(1, 1, None, b"earlier window");
    world.window_turn(2, 2, None, b"window to resume");
    let assignment = world.assigned.as_ref().expect("live worker");
    let hosted = fleet::Hosted {
        run: Token::new(assignment.task),
        attempt: Token::new(assignment.attempt),
        phase: fleet::Phase::Active,
    };
    world.restart(124, false);
    hello(&mut world, Box::new([hosted]));
    world.end(tasks::End::Parked, 2);
    account(&mut world);
    world.say(91);
    assert_eq!(
        world.assigned.as_ref().expect("resumed window").transcript.as_ref(),
        [Box::<[u8]>::from(&b"window to resume"[..])]
    );
}

fn census_world(seed: u64) -> World {
    let (configuration, mut limits) = jig_core_world::effects::fixture(seed, false);
    limits.core.people.inbox_entries = 8;
    limits.core.tasks.inbox_messages = 8;
    limits.core.tasks.run_inbox = 8;
    limits.core.documents.rows = 2;
    let room = core::room_max(&limits.core).expect("census world room");
    let connectors = connector::MAX_OUT * 2;
    limits.journal.commits = 2;
    limits.watermark = 2;
    limits.journal.writes = room.writes + connectors;
    limits.journal.held = (room.held + connectors) * 2;
    let mut world = World::configured(seed, false, configuration, limits);
    world.reads = Some(jig_core_world::reads::Parties::new(world.store.applied));
    world
}
fn finish_census_task(world: &mut World, cumulative: u64) {
    world.end(
        tasks::End::Finished {
            result: tasks::TaskResult::Report { words: b"census result".as_slice().into() },
            cancel_delegates: false,
        },
        cumulative,
    );
}
fn census_ended_key(task: u64) -> root::Key {
    root::Key::Core(core::Key::Tasks(tasks::Key::Ended(task)))
}
fn census_result(world: &World, task: u64) -> &tasks::TaskRecord {
    let root::Record::Core(core::Record::Tasks(tasks::Stored::Ended(row))) = &world.store.rows[&census_ended_key(task)]
    else {
        panic!("ended census result")
    };
    row
}
fn census_goal(world: &mut World, key: u8) -> u64 {
    jig_core_world::stories::reads::request(world, key, jig_core_world::stories::offers::goal(100));
    let task = world.assigned.as_ref().expect("goal assigned").task;
    finish_census_task(world, 0);
    assert!(census_result(world, task).tracked.is_some());
    task
}
fn census_mark_results(world: &mut World, through: u64, key: u8) {
    jig_core_world::stories::reads::request(
        world,
        key,
        jig_domain_people::Ask::MarkSeen { seen: jig_domain_people::Seen::Results { through } },
    );
}
#[test]
fn a_restart_reads_no_ended_task_whose_result_was_read() {
    let mut world = census_world(790);
    let chat = world.assigned.as_ref().expect("initial chat").task;
    world.turn(1, 1, None, b"initial reply");
    finish_census_task(&mut world, 1);
    jig_core_world::stories::reads::request(
        &mut world,
        99,
        jig_domain_people::Ask::MarkSeen { seen: jig_domain_people::Seen::Conversation { task: chat, through: 1 } },
    );
    let through = census_result(&world, chat).result_position;
    census_mark_results(&mut world, through, 100);
    let first = census_goal(&mut world, 101);
    let _second = census_goal(&mut world, 102);
    let _third = census_goal(&mut world, 103);
    let through = census_result(&world, first).result_position;
    census_mark_results(&mut world, through, 104);
    world.restart(790, false);
    assert_eq!(world.census_ended, 3);
    assert!(world.store.rows.contains_key(&census_ended_key(first)));
}
fn census_updates(world: &mut World, reply: u64) -> core::documents::Count {
    jig_core_world::reads::ask(world, reply, core::reads::Read::Frame);
    let core::reads::Answer::Document(document) = jig_core_world::reads::latest(world) else { panic!("frame") };
    let core::documents::Document::Frame(frame) = &**document else { panic!("frame document") };
    frame.updates
}
fn census_unread_results(world: &mut World) -> Vec<u64> {
    let mut from = None;
    let mut tasks = Vec::new();
    for page in 0..4 {
        jig_core_world::reads::ask(
            world,
            810 + page,
            core::reads::Read::Inbox {
                group: core::reads::Group::Results,
                filter: core::reads::Filter { project: Some(1), goal: None },
                from: from.take(),
            },
        );
        let core::reads::Answer::Page(rows) = jig_core_world::reads::latest(world) else { panic!("results page") };
        for row in &rows.rows {
            let core::reads::PageRow::Inbox(entry) = row else { panic!("result row") };
            let core::documents::TaskLink::Retained { chip, .. } = &entry.task else { panic!("retained task") };
            tasks.push(chip.number);
        }
        from.clone_from(&rows.next);
        if from.is_none() {
            break;
        }
    }
    tasks
}
#[test]
fn a_restart_restores_unread_results_and_chat_totals() {
    let mut world = census_world(791);
    let mut chats = Vec::new();
    for number in 0..3 {
        if number != 0 {
            let request = jig_core_world::fixtures::chat(&world, 1, b"next census chat".as_slice().into());
            jig_core_world::stories::reads::request(&mut world, 110 + number, request);
        }
        chats.push(world.assigned.as_ref().expect("chat").task);
        world.turn(1, 1, None, b"unread reply");
        finish_census_task(&mut world, 1);
    }
    assert_eq!(census_updates(&mut world, 800), core::documents::Count::Known(6));
    world.restart(791, false);
    assert_eq!(census_updates(&mut world, 801), core::documents::Count::Known(6));
    chats.reverse();
    assert_eq!(census_unread_results(&mut world), chats);
}

fn both_kinds_engine_fixture(seed: u64) -> (root::Config, root::Limits) {
    let (mut config, mut limits) = jig_core_world::effects::fixture(seed, false);
    config.core.defaults.charters[0].hosts = fleet::Kinds::Both;
    limits.core.fleet.engine_slots = 2;
    limits.core.fleet.slots = 1;
    (config, limits)
}

#[test]
fn a_restart_settles_an_engine_claim_of_a_both_kinds_charter_at_once() {
    let (config, limits) = both_kinds_engine_fixture(125);
    let mut world = World::configured(125, false, config, limits);
    let assignment = world.assigned.as_ref().expect("engine claim");
    let task = assignment.task;
    let attempt = assignment.attempt;
    world.send(root::Event::Turn {
        channel: Token::new(0),
        task,
        attempt,
        turn: 1,
        cumulative: 2,
        read: None,
        opens_window: true,
        refused: Box::new([]),
        transcript: b"engine window".as_slice().into(),
    });
    world.restart_with(both_kinds_engine_fixture(125).0);
    assert!(world.domain.ready(), "engine claim settles before any worker hello");
    assert!(world.store.rows.values().any(|record| matches!(record,
        root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row)))
        if row.number == task && row.tries.lost == 1)));
}

#[test]
fn a_new_generation_reaches_the_live_chat() {
    let mut world = World::new(161, false);
    grant_account(&mut world, 1, 2);
    let grant = received(&world);
    assert_eq!((grant.form.table(), grant.account, grant.generation), (fleet::GrantTable::Accounts, 1, 2));
}

#[test]
fn a_restart_sends_an_adopted_chat_its_current_grant() {
    let mut world = World::new(162, false);
    let live = hosting(&world);
    world.restart(162, false);
    grant_account(&mut world, 1, 7);
    hello(&mut world, live);
    let grant = received(&world);
    assert_eq!((grant.form.table(), grant.account, grant.generation), (fleet::GrantTable::Accounts, 1, 7));
    assert!(world.grant_follows_assignment());
}

#[test]
fn a_generation_renewed_while_the_assignment_is_held_reaches_the_attempt_after_it() {
    const SECOND_ANALYST: u32 = 2;
    let mut world = jig_core_world::fixtures::waiting_on_source(163, SECOND_ANALYST);
    assert!(world.assigned.is_none());
    world.renew_on_assignment(
        SECOND_ANALYST,
        2,
        jig_domain_accounts::Form::Credential(jig_domain_accounts::Scheme::Bearer),
    );
    grant_account(&mut world, SECOND_ANALYST, 1);
    assert_eq!(world.assigned.as_ref().expect("assigned after commit").grants[0].generation, 1);
    let grant = received(&world);
    assert_eq!((grant.account, grant.generation), (SECOND_ANALYST, 2));
    assert!(world.grant_follows_assignment());
}

#[test]
fn a_generation_renewed_before_a_waiting_chat_is_placed_reaches_its_host() {
    let mut world = World::new(165, false);
    world.end(tasks::End::Parked, 0);
    world.send(root::Event::Core(core::Event::Fleet(fleet::Event::Lost { channel: Token::new(7) })));
    world.say(99);
    grant_account(&mut world, 1, 2);
    hello(&mut world, Box::new([]));
    assert_eq!(world.assigned.as_ref().expect("resumed assignment").grants[0].generation, 2);
    assert_eq!(received(&world).generation, 2);
    assert!(world.grant_follows_assignment());
}

#[test]
fn a_restart_keeps_an_unread_goal_older_than_an_unread_conversation() {
    let mut world = census_world(839);
    finish_census_task(&mut world, 0);
    let goal = census_goal(&mut world, FIRST_GOAL);
    let request = jig_core_world::fixtures::chat(&world, 1, b"newer chat".as_slice().into());
    jig_core_world::stories::reads::request(&mut world, CHAT, request);
    let chat = world.assigned.as_ref().expect("chat").task;
    world.turn(TURN, 1, None, b"reply");
    finish_census_task(&mut world, 1);
    let updates = census_updates(&mut world, BEFORE);
    world.restart(839, false);
    let results = census_unread_results(&mut world);
    assert!(results.contains(&goal));
    assert_eq!(results[0], chat);
    assert_eq!(census_updates(&mut world, AFTER), updates);
}

#[test]
fn a_restart_without_connectors_still_switches_before_adoption() {
    for pending in [false, true] {
        let limits = jig_core_world::world::limits().core;
        let mut config = jig_core_world::world::config(111).core;
        config.connectors = Box::new([]);
        config.grants = Box::new([]);
        let mut core = core::Core::new(config, &limits).expect("no-connector fixture");
        let mut request = core.restart_begin(pending);
        for step in [
            core::RestartStep::LoadCore,
            core::RestartStep::LoadInbox,
            core::RestartStep::Switch,
            core::RestartStep::AdoptRuns,
            core::RestartStep::Open,
        ] {
            assert_eq!(request, core::RestartRequest::Step(step));
            request = core.restart_done(step);
        }
        assert!(core.restart_ready());
    }
}

#[test]
fn a_durable_placed_stop_restores_with_its_task_and_releases_after_restart() {
    use jig_core_world::fixtures::{ask_as, live_row};
    use jig_domain_people as people;
    let mut world = World::new(2210, false);
    let (task, attempt) = world.assignments[0];
    let sign_in = world.sign_in.expect("signed in requester");
    ask_as(&mut world, sign_in, 71, people::Ask::Stop { project: 1, task });
    world.end(tasks::End::Parked, 0);
    let hold = live_row(&world, task).placed.expect("placed stop");
    let revision = live_row(&world, task).revisions.hold;
    let key = root::Key::Core(core::Key::Tasks(tasks::Key::PlacedHold(hold)));
    let durable = world.store.rows[&key].clone();
    let assignments = world.assignments.len();
    world.restart(2210, false);
    assert_eq!(world.domain.restart_failure(), None);
    assert!(world.domain.ready());
    assert_eq!(world.assignments.len(), assignments);
    assert_eq!(world.store.rows.get(&key), Some(&durable));
    assert_eq!(live_row(&world, task).revisions.hold, revision);
    hello(&mut world, Box::new([]));
    account(&mut world);
    ask_as(&mut world, sign_in, 72, people::Ask::Release { project: 1, task, revision, again: None });
    assert_eq!(world.people_answers.last(), Some(&people::Reply::Outcome(people::Outcome::Released { task })));
    assert!(!world.store.rows.contains_key(&key));
    assert_eq!(live_row(&world, task).placed, None);
    assert!(world.assignments.iter().any(|(number, next)| *number == task && *next > attempt));
}
