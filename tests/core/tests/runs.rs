use jig_core_world::effects::World;
use jig_domain_core as core;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{ReplyTo, Token};

fn a_chat_answers_parks_and_resumes() {
    let mut world = World::new(101, false);
    let first = world.assigned.as_ref().expect("chat assigned").attempt;
    world.turn(1, 3, None, b"first opaque turn");
    world.end(tasks::End::Parked, 3);
    let parked = world.store.applied;
    world.end(tasks::End::Parked, 3);
    assert_eq!(world.store.applied, parked, "duplicate terminal changes nothing");
    let read = world.say(71);
    let second = world.assigned.as_ref().expect("woken chat");
    assert!(second.attempt > first);
    assert_eq!(second.transcript.as_ref(), [Box::<[u8]>::from(&b"first opaque turn"[..])]);
    world.turn(1, 2, Some(read), b"second opaque turn");
    assert!(world.store.rows.values().any(|row| matches!(row, root::Record::Core(core::Record::Core(core::CoreRecord::Turn(turn))) if turn.transcript.as_ref()==b"second opaque turn")), "{:?}", world.trace);
    world.end(tasks::End::Parked, 2);
    world.say(72);
    let third = world.assigned.as_ref().expect("chat resumes across attempts");
    assert!(third.transcript.len() == 2, "{:?}", world.trace);
    assert_eq!(
        third.transcript.as_ref(),
        [Box::<[u8]>::from(&b"first opaque turn"[..]), Box::<[u8]>::from(&b"second opaque turn"[..])]
    );
    let task = third.task;
    assert_eq!(world.spent(), 5, "each attempt's accepted spend is charged once");
    world.end(
        tasks::End::Finished {
            result: tasks::TaskResult::Report { words: b"done".as_slice().into() },
            cancel_delegates: false,
        },
        0,
    );
    assert!(world.store.rows.contains_key(&root::Key::Core(core::Key::Tasks(tasks::Key::Ended(task)))));
}

fn a_turn_admits_its_read_fence_and_spend_together_and_duplicates_change_nothing() {
    let mut world = World::new(102, false);
    let message = world.say(73);
    let rows = world.store.rows.clone();
    let commits = world.store.applied;
    world.turn(1, 5, Some(message + 1), b"invalid offered read");
    assert_eq!(world.store.rows, rows, "an unoffered read cannot charge or take words");
    assert_eq!(world.store.applied, commits);
    world.turn(1, 5, Some(message), b"valid turn");
    assert_eq!(world.spent(), 5);
    let rows = world.store.rows.clone();
    let commits = world.store.applied;
    world.turn(1, 5, Some(message), b"valid turn");
    assert_eq!(world.store.rows, rows, "duplicate accepted turn preserves the exact proof and transcript");
    assert_eq!(world.store.applied, commits);
}

fn settled_delivery_evidence_survives_restart_and_resume_until_a_turn_carries_it() {
    let mut world = World::new(103, false);
    world.turn(1, 1, None, b"turn");
    let right = world.host_call(b"call-name", b"deliver", b"opaque-input", true);
    let key = world.call_key(2);
    assert_eq!(world.host_calls[0].3.name.as_ref(), b"call-name");
    assert_eq!(world.host_calls[0].3.tool.as_ref(), b"deliver");
    assert_eq!(world.host_calls[0].3.input.as_ref(), b"opaque-input");
    assert!(world.host_calls[0].3.writes);
    world.send(root::Event::Core(core::Event::NamedAnswer {
        to: ReplyTo::new(right),
        key,
        part: core::CallPart::Unavailable,
    }));
    let evidence = Box::<[u8]>::from(&b"\x01opaque delivery evidence\x00"[..]);
    let settled = core::SettledCall {
        serial: 0,
        tool: b"deliver".as_slice().into(),
        answer: core::SettledAnswer::Delivery { outcome: core::DeliveryOutcome::Delivered, evidence: evidence.clone() },
    };
    world.send(root::Event::Core(core::Event::SettledCall {
        to: ReplyTo::new(right),
        key,
        name: b"call-name".as_slice().into(),
        call: settled,
    }));
    assert_eq!(world.host_answers.len(), 1);
    let saved = world.host_answers[0].1.clone();
    assert_ne!(saved.serial, 0);
    world.end(tasks::End::Parked, 1);
    world.restart(103, false);
    world.send(root::Event::Core(core::Event::Fleet(jig_domain_fleet::Event::Hello {
        worker: 0,
        channel: Token::new(7),
        hello: jig_domain_fleet::Hello {
            bounds: jig_core_world::fixtures::bounds(),
            stop_bound: skein_lib::Duration::from_millis(100),
            slots: 1,
            workstreams: Box::new([]),
            hosting: Box::new([]),
        },
    })));
    world.grant_account(1);
    let read = world.say(74);
    assert!(!world.assigned.as_ref().expect("resumed chat").answered.is_empty(), "{:?}", world.trace);
    assert_eq!(world.assigned.as_ref().expect("resumed chat").answered.as_ref(), [core::Settled { key, call: saved }]);
    world.turn(1, 1, Some(read), b"answers carried by this turn");
    assert!(!world.store.rows.contains_key(&root::Key::Core(core::Key::Core(core::CoreKey::Call(key)))));
    world.end(tasks::End::Parked, 1);
    world.say(75);
    assert!(world.assigned.as_ref().expect("resumed again").answered.is_empty());
}

fn a_chat_past_the_resume_limit_starts_fresh() {
    let mut world = World::new(104, false);
    world.turn(1, 1, None, &[b'a'; 200]);
    world.turn(2, 2, None, &[b'b'; 100]);
    world.end(tasks::End::Parked, 2);
    world.say(76);
    assert!(world.assigned.as_ref().expect("fresh chat").transcript.is_empty());
}

fn transcript_core(window: (u64, u32)) -> core::Core {
    let limits = jig_core_world::world::limits().core;
    let config = jig_core_world::world::config(105).core;
    let mut core = core::Core::new(config, &limits).expect("fixture core configuration");
    core.begin_transcript(1, 3);
    core.transcripts.get_mut(&1).expect("begun transcript").window = Some(window);
    core
}

fn stored_turn(attempt: u64, turn: u32, body: &[u8]) -> core::TurnRecord {
    core::TurnRecord {
        opens_window: true,
        task: 1,
        attempt,
        turn,
        spent: 0,
        read: None,
        at: skein_lib::Wall::EPOCH,
        transcript: body.into(),
    }
}

fn loaded_transcript_pages_reject_repeated_and_reversed_turns() {
    let mut core = transcript_core((2, 2));
    assert!(!core.append_transcript(1, stored_turn(2, 1, b"before window"), None));
    assert!(core.append_transcript(1, stored_turn(2, 2, b"turn"), None));
    assert!(!core.append_transcript(1, stored_turn(2, 2, b"turn"), None));
    assert!(!core.append_transcript(1, stored_turn(1, 2, b"old"), None));
    assert_eq!(core.transcripts.get(&1).expect("loaded transcript").bytes, 8);
}

fn a_chat_resumed_after_a_compaction_is_sent_only_its_window() {
    let mut world = World::new(115, false);
    world.turn(1, 1, None, &[b'a'; 200]);
    park_chat(&mut world, 1);
    world.say(86);
    world.window_turn(1, 1, None, b"compacted window");
    world.turn(2, 2, None, b"window continuation");
    park_chat(&mut world, 2);
    world.say(87);
    assert_eq!(
        world.assigned.as_ref().expect("resumed compacted chat").transcript.as_ref(),
        [Box::<[u8]>::from(&b"compacted window"[..]), Box::<[u8]>::from(&b"window continuation"[..])]
    );
}

fn a_window_past_the_resume_limit_starts_fresh_with_its_tail() {
    let mut world = World::new(116, false);
    world.turn(1, 1, None, b"earlier window");
    world.window_turn(2, 2, None, &[b'b'; 200]);
    park_chat(&mut world, 2);
    world.say(88);
    world.turn(1, 1, None, &[b'c'; 100]);
    park_chat(&mut world, 1);
    world.say(90);
    let fresh = world.assigned.as_ref().expect("fresh chat");
    assert!(!fresh.run.policy.resume);
    assert!(fresh.transcript.is_empty());
    let mut core = transcript_core((2, 2));
    assert!(core.append_transcript(1, stored_turn(2, 2, &[b'b'; 200]), Some(Box::from(&[b'b'; 200][..]))));
    assert!(core.append_transcript(1, stored_turn(3, 1, &[b'c'; 100]), Some(Box::from(&[b'c'; 100][..]))));
    let tail = core::read_brief_part(
        &core,
        core::BriefTextLimits { parts: 2, read_bytes: 128, brief_bytes: 256 },
        1,
        core::BriefPart::TranscriptTail,
    )
    .expect("window tail");
    assert_eq!(tail.as_ref(), [b'c'; 100]);
}

#[path = "support/fresh_windows.rs"]
mod fresh_windows;

#[test]
fn a_window_closed_by_a_fresh_mark_starts_fresh_with_its_tail() {
    use fresh_windows as f;
    let mut world = f::parked();
    let mut control = f::parked();
    f::restart(&mut world, f::AFTER);
    f::restart(&mut control, f::BEFORE);
    world.say(87);
    control.say(87);
    f::fresh(&world);
    assert_eq!(f::tail(&world), Some(f::TAIL));
    assert_eq!(f::tries(&world), f::tries(&control));
    let mut activity = f::activity(&world);
    let Some(tasks::Line::RunBegan { resumed, .. }) = activity.last_mut() else { panic!("fresh run activity") };
    *resumed = true;
    assert_eq!(activity, f::activity(&control));
    assert!(control.assigned.as_ref().expect("control resumes").run.policy.resume);
    assert_eq!(
        control.assigned.as_ref().expect("control resumes").transcript.as_ref(),
        [f::turn(b"closed window"), f::turn(b"window continuation"),]
    );
    assert_eq!(f::tail(&control), None);
}

#[test]
fn a_closed_window_keeps_its_tail_after_a_transient_try() {
    use fresh_windows as f;
    let mut world = f::parked();
    let mut control = f::parked();
    for run in [&mut world, &mut control] {
        // The failed attempt resumed the parked window but wrote no new turn.
        run.say(87);
        run.end(tasks::End::Failed(tasks::Class::Transient), 0);
    }
    f::restart(&mut world, f::AFTER);
    f::restart(&mut control, f::BEFORE);
    world.retry_due();
    control.retry_due();
    f::fresh(&world);
    assert_eq!(f::tail(&world), Some(f::TAIL));
    assert_eq!(f::tries(&world).transient, 1);
    assert_eq!(f::tries(&world), f::tries(&control));
    assert_eq!(f::activity(&world), f::activity(&control));
}

#[test]
fn a_closed_windows_unread_tail_starts_fresh_without_counting() {
    use fresh_windows as f;
    let mut world = f::parked();
    let mut control = f::parked();
    f::restart(&mut world, f::AFTER);
    f::restart(&mut control, f::BEFORE);
    world.store.unread_transcript(f::proof(&world).task);
    world.say(87);
    control.say(87);
    f::fresh(&world);
    assert_eq!(f::tail(&world), None);
    assert_eq!(f::tries(&world), f::tries(&control));
    let mut activity = f::activity(&world);
    let Some(tasks::Line::RunBegan { resumed, .. }) = activity.last_mut() else { panic!("fresh run activity") };
    *resumed = true;
    assert_eq!(activity, f::activity(&control));
}

fn an_unusable_transcript_fails_transient_and_the_next_conversation_starts_fresh() {
    let mut world = World::new(106, false);
    world.turn(1, 1, None, b"unusable bytes");
    world.end(tasks::End::Parked, 1);
    let read = world.say(77);
    assert_eq!(world.assigned.as_ref().expect("resumed attempt").transcript.len(), 1);
    world.end(tasks::End::Failed(tasks::Class::Transient), 0);
    world.retry_due();
    let fresh = world.assigned.as_ref().expect("fresh retry");
    assert!(!fresh.run.policy.resume);
    assert!(fresh.transcript.is_empty());
    world.turn(1, 1, Some(read), b"fresh conversation");
    world.end(tasks::End::Parked, 1);
    world.say(78);
    assert_eq!(
        world.assigned.as_ref().expect("resumed usable conversation").transcript.as_ref(),
        [Box::<[u8]>::from(&b"fresh conversation"[..])]
    );
}

fn settled_answers_resume_in_commit_order_and_a_replay_preserves_exact_bytes() {
    let mut world = World::new(107, false);
    for (completion, name) in [(4, &b"first"[..]), (2, &b"second"[..])] {
        let right = world.host_call(name, b"read", b"{}", false);
        let key = world.call_key(completion);
        world.send(root::Event::Core(core::Event::NamedAnswer {
            to: ReplyTo::new(right),
            key,
            part: core::CallPart::Unavailable,
        }));
        world.send(root::Event::Core(core::Event::SettledCall {
            to: ReplyTo::new(right),
            key,
            name: name.into(),
            call: core::SettledCall {
                serial: 0,
                tool: b"read".as_slice().into(),
                answer: core::SettledAnswer::Host { error: true, body: b"exact original".as_slice().into() },
            },
        }));
    }
    let first = world.host_answers[0].1.clone();
    let key = world.call_key(4);
    let right = world.host_call(b"first", b"read", b"{}", false);
    let before = world.store.applied;
    world.send(root::Event::Core(core::Event::SettledCall {
        to: ReplyTo::new(right),
        key,
        name: b"first".as_slice().into(),
        call: core::SettledCall {
            serial: 0,
            tool: b"read".as_slice().into(),
            answer: core::SettledAnswer::Host { error: false, body: b"different rendering".as_slice().into() },
        },
    }));
    assert_eq!(world.host_answers.last().expect("replayed answer").1, first);
    assert_eq!(world.store.applied, before);
    world.end(tasks::End::Parked, 0);
    world.say(79);
    let answers = &world.assigned.as_ref().expect("resumed answers").answered;
    assert_eq!(answers.len(), 2);
    assert_eq!(answers[0].key, key);
    assert_eq!(answers[1].key, core::CallKey { completion: 2, ..key });
    assert!(answers[0].call.serial < answers[1].call.serial);
}

fn a_woken_chats_start_carries_its_waiting_words_oldest_first() {
    let mut world = World::new(141, false);
    let first = world.say(84);
    let second = world.say(85);
    park_chat(&mut world, 0);
    let messages = &world.assigned.as_ref().expect("woken chat").messages;
    assert_eq!(messages.iter().map(|word| word.number).collect::<Vec<_>>(), [first, second]);
    assert_eq!(messages[0].words.as_ref(), [84]);
    assert_eq!(messages[1].words.as_ref(), [85]);
}

fn a_start_carries_at_most_the_runs_inbox() {
    let (config, mut limits) = jig_core_world::effects::fixture(142, false);
    limits.core.tasks.run_inbox = 1;
    let mut world = World::configured(142, false, config, limits);
    let first = world.say(86);
    let second = world.say(87);
    park_chat(&mut world, 0);
    let assigned = world.assigned.as_ref().expect("woken chat");
    assert_eq!(assigned.messages.iter().map(|word| word.number).collect::<Vec<_>>(), [first]);
    let key = root::Key::Core(core::Key::Core(core::CoreKey::RunProof(assigned.task)));
    let root::Record::Core(core::Record::Core(core::CoreRecord::RunProof(proof))) = &world.store.rows[&key] else {
        panic!("durable claim proof")
    };
    assert_eq!(proof.offered, Some(first));
    assert!(world.store.rows.values().any(|row| matches!(row,
        root::Record::Core(core::Record::Tasks(tasks::Stored::Live(task)))
        if task.inbox.iter().any(|word| word.number == second))));
}

fn relays_wait_while_the_runs_inbox_is_full() {
    let (config, mut limits) = jig_core_world::effects::fixture(143, false);
    limits.core.tasks.run_inbox = 1;
    let mut world = World::configured(143, false, config, limits);
    let first = world.say(88);
    let second = world.say(89);
    assert_eq!(world.inbound.iter().map(|word| word.number).collect::<Vec<_>>(), [first]);
    world.turn(1, 1, Some(first), b"reads the first words");
    assert_eq!(world.inbound.iter().map(|word| word.number).collect::<Vec<_>>(), [first, second]);
}

fn a_run_reading_nothing_is_relayed_nothing_more() {
    let (config, mut limits) = jig_core_world::effects::fixture(144, false);
    limits.core.tasks.run_inbox = 1;
    let mut world = World::configured(144, false, config, limits);
    let first = world.say(90);
    world.say(91);
    world.turn(1, 1, None, b"reads no message");
    assert_eq!(world.inbound.iter().map(|word| word.number).collect::<Vec<_>>(), [first]);
}

fn answer_with_messages(world: &mut World, read: Option<u64>, refused: &[u64]) {
    let assignment = world.assigned.as_ref().expect("assigned chat");
    world.send(root::Event::Answer {
        channel: Token::new(7),
        task: assignment.task,
        attempt: assignment.attempt,
        cumulative: 1,
        end: tasks::End::Parked,
        read,
        refused: refused.into(),
    });
}

fn invalid_answer_recorded(world: &World) -> bool {
    world.store.rows.values().any(|row| {
        matches!(row,
        root::Record::Core(core::Record::Core(core::CoreRecord::RunProof(proof)))
        if matches!(&proof.terminal, Some(terminal)
            if terminal.end == tasks::End::Failed(tasks::Class::Invalid)))
    })
}

fn an_answer_whose_fence_passes_its_last_turn_fails_invalid() {
    let mut world = World::new(145, false);
    let first = world.say(92);
    world.turn(1, 1, Some(first), b"reads the first words");
    let second = world.say(93);
    answer_with_messages(&mut world, Some(second), &[]);
    assert!(invalid_answer_recorded(&world), "{:?}", world.trace);
}

fn an_answer_refusing_an_unoffered_word_fails_invalid() {
    let mut world = World::new(146, false);
    world.turn(1, 1, None, b"reads no words");
    answer_with_messages(&mut world, None, &[999]);
    assert!(invalid_answer_recorded(&world), "{:?}", world.trace);
}

fn a_refusal_frees_the_runs_inbox_for_its_next_waiting_word() {
    let (config, mut limits) = jig_core_world::effects::fixture(147, false);
    limits.core.tasks.run_inbox = 1;
    let mut world = World::configured(147, false, config, limits);
    let first = world.say(94);
    let second = world.say(95);
    world.turn_refusing(1, 1, None, &[first], b"refuses the first words");
    assert_eq!(world.inbound.iter().map(|word| word.number).collect::<Vec<_>>(), [first, second]);
}

fn a_refused_message_survives_a_later_fence_and_wakes_its_task_as_the_run_parks() {
    let mut world = World::new(148, false);
    let first = world.say(80);
    let second = world.say(81);
    let attempt = assigned_chat(&world).attempt;
    world.turn_refusing(1, 1, Some(second), &[first], b"reads the second words only");
    park_chat(&mut world, 1);
    let woken = assigned_chat(&world);
    assert!(woken.attempt > attempt, "{:?}", world.trace);
    assert_eq!(woken.messages.first().map(|word| word.number), Some(first));
}

fn a_message_left_unread_wakes_its_task_as_the_run_parks() {
    let mut world = World::new(149, false);
    let first = world.say(82);
    world.turn(1, 1, Some(first), b"reads the first words");
    let unread = world.say(83);
    let attempt = assigned_chat(&world).attempt;
    park_chat(&mut world, 1);
    let woken = assigned_chat(&world);
    assert!(woken.attempt > attempt, "{:?}", world.trace);
    assert_eq!(woken.messages.first().map(|word| word.number), Some(unread));
}

fn quiet_news_left_unread_waits_for_the_next_activation() {
    let (config, mut limits) = jig_core_world::effects::fixture(150, false);
    limits.core.tasks.inbox_bytes = 256;
    let mut world = World::configured(150, false, config, limits);
    let task = assigned_chat(&world).task;
    subscribe_topic(&mut world, task, 10);
    subscribe_topic(&mut world, task, 11);
    world.send(root::Event::Core(core::Event::Tasks(tasks::Event::Notice {
        task,
        word: inbox_notice(9001, tasks::MessageKind::News { subscription: 10, class: tasks::NewsClass::Kept }),
    })));
    let assigned = world.assignments.len();
    park_chat(&mut world, 0);
    assert_eq!(world.assignments.len(), assigned, "quiet news does not wake a parked chat");
    world.send(root::Event::Core(core::Event::Tasks(tasks::Event::Notice {
        task,
        word: inbox_notice(9002, tasks::MessageKind::News { subscription: 11, class: tasks::NewsClass::Wakes }),
    })));
    assert_eq!(world.assignments.len(), assigned + 1);
    assert!(world.assigned.as_ref().expect("next activation").messages.iter().any(|word| word.number == 9001));
}

fn inbox_notice(number: u64, kind: tasks::MessageKind) -> tasks::Word {
    tasks::Word {
        number,
        from: tasks::Party::Deployment { project: 1 },
        kind,
        words: b"inbox notice".as_slice().into(),
        at: skein_lib::Wall::EPOCH,
        hits: 1,
        eligible: false,
    }
}

fn subscribe_topic(world: &mut World, task: u64, number: u64) {
    world.send(root::Event::Core(core::Event::Tasks(tasks::Event::SubscribeTopic {
        reply_to: ReplyTo::new(Token::new(9000 + number)),
        task,
        subscription: tasks::Subscription {
            number,
            kind: tasks::SubscriptionKind::Topic { connector: 1, topic: number },
        },
    })));
    let key = root::Key::Core(core::Key::Tasks(tasks::Key::Live(task)));
    assert!(matches!(&world.store.rows[&key],
        root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row)))
        if row.subscriptions.iter().any(|subscription| subscription.number == number)));
}

fn assigned_chat(world: &World) -> &root::Assignment {
    world.assigned.as_ref().expect("assigned chat")
}

// Each scenario keeps its own fresh world and all original assertions.
// A shared entrypoint avoids one process startup per short scenario.
#[test]
fn runs_scenarios() {
    a_refused_message_survives_a_later_fence_and_wakes_its_task_as_the_run_parks();
    a_message_left_unread_wakes_its_task_as_the_run_parks();
    quiet_news_left_unread_waits_for_the_next_activation();
    an_answer_whose_fence_passes_its_last_turn_fails_invalid();
    an_answer_refusing_an_unoffered_word_fails_invalid();
    a_refusal_frees_the_runs_inbox_for_its_next_waiting_word();
    relays_wait_while_the_runs_inbox_is_full();
    a_run_reading_nothing_is_relayed_nothing_more();
    a_woken_chats_start_carries_its_waiting_words_oldest_first();
    a_start_carries_at_most_the_runs_inbox();
    a_chat_answers_parks_and_resumes();
    a_turn_admits_its_read_fence_and_spend_together_and_duplicates_change_nothing();
    settled_delivery_evidence_survives_restart_and_resume_until_a_turn_carries_it();
    a_chat_past_the_resume_limit_starts_fresh();
    a_chat_resumed_after_a_compaction_is_sent_only_its_window();
    a_window_past_the_resume_limit_starts_fresh_with_its_tail();
    loaded_transcript_pages_reject_repeated_and_reversed_turns();
    an_unusable_transcript_fails_transient_and_the_next_conversation_starts_fresh();
    settled_answers_resume_in_commit_order_and_a_replay_preserves_exact_bytes();
}

#[test]
fn a_resuming_charters_first_claim_records_its_own_attempt() {
    let world = World::new(108, false);
    let assigned = world.assigned.as_ref().expect("first chat assignment");
    let key = root::Key::Core(core::Key::Core(core::CoreKey::RunProof(assigned.task)));
    let root::Record::Core(core::Record::Core(core::CoreRecord::RunProof(proof))) = &world.store.rows[&key] else {
        panic!("durable claim proof")
    };
    assert_eq!(proof.conversation, assigned.attempt);
}

#[test]
fn a_conversations_attempts_name_one_first_attempt() {
    let mut world = World::new(109, false);
    let first = world.assigned.as_ref().expect("first assignment").attempt;
    for key in [81, 82] {
        world.turn(1, 1, None, b"conversation turn");
        world.end(tasks::End::Parked, 1);
        world.say(key);
        let assigned = world.assigned.as_ref().expect("resumed chat");
        let key = root::Key::Core(core::Key::Core(core::CoreKey::RunProof(assigned.task)));
        let root::Record::Core(core::Record::Core(core::CoreRecord::RunProof(proof))) = &world.store.rows[&key] else {
            panic!("durable resumed claim")
        };
        assert_eq!(proof.conversation, first);
        assert!(proof.attempt > first);
    }
}

fn reserve_fixture(seed: u64, spend: u64) -> (root::Config, root::Limits) {
    let (mut config, limits) = jig_core_world::effects::fixture(seed, false);
    config.core.defaults.charters[0].policy.model.price_unit = 1_000;
    config.core.settings.models[0].price_unit = 1_000;
    let mut rules = config.core.defaults.rules.clone();
    rules.maximum_run_spend = spend;
    let policy = config.core.seed_projects[0].policy.policy.clone();
    config.core.defaults.rules =
        (jig_domain_authority::Domain::new(rules, limits.core.authority).expect("authority bounds")).rules().clone();
    config.core.seed_projects[0].policy.policy = policy;
    (config, limits)
}

#[test]
fn a_chat_whose_spend_is_below_its_reserve_is_held_for_budget() {
    let (config, limits) = reserve_fixture(139, 19);
    let world = World::configured_unassigned(139, false, config, limits);
    assert!(world.assigned.is_none());
    assert!(world.store.rows.values().any(|row| matches!(row,
        root::Record::Core(core::Record::Tasks(tasks::Stored::Live(task)))
        if matches!(task.phase, tasks::Phase::Held { why: tasks::Hold::Budget, .. }))));
}

#[test]
fn a_chat_whose_spend_holds_its_reserve_is_assigned() {
    let (config, limits) = reserve_fixture(140, 20);
    let world = World::configured(140, false, config, limits);
    assert_eq!(world.assigned.as_ref().expect("chat assigned at floor").run.budget, 20);
}

#[test]
fn a_resumed_run_names_its_first_runs_attempt() {
    let mut world = World::new(110, false);
    let first = world.assigned.as_ref().expect("first assignment").attempt;
    for key in [83, 84] {
        world.turn(1, 1, None, b"conversation turn");
        world.end(tasks::End::Parked, 1);
        world.say(key);
        let assignment = world.assigned.as_ref().expect("resumed assignment");
        assert_eq!(assignment.conversation, first);
        assert!(assignment.attempt > first);
    }
}

#[test]
fn a_fresh_start_after_a_transient_failure_names_its_own() {
    let mut world = World::new(111, false);
    let first = world.assigned.as_ref().expect("first assignment").attempt;
    world.turn(1, 1, None, b"old conversation");
    world.end(tasks::End::Failed(tasks::Class::Transient), 1);
    world.retry_due();
    let assignment = world.assigned.as_ref().expect("fresh assignment");
    assert!(assignment.attempt > first);
    assert_eq!(assignment.conversation, assignment.attempt);
    assert!(assignment.transcript.is_empty());
}

fn account_failed(world: &mut World) {
    world.send(root::Event::Core(core::Event::Account(jig_domain_accounts::Event::Component(
        jig_domain_accounts::FromComponent::Failed { account: 1, failure: jig_domain_accounts::Failure::SignInAgain },
    ))));
}
fn account_granted(world: &mut World) {
    world.send(root::Event::Core(core::Event::Account(jig_domain_accounts::Event::Component(
        jig_domain_accounts::FromComponent::Granted {
            form: jig_domain_accounts::Form::Credential(jig_domain_accounts::Scheme::Bearer),
            account: 1,
            generation: 9,
            valid: skein_lib::Duration::from_secs(60),
        },
    ))));
}
#[test]
fn a_run_waits_while_its_account_must_sign_in_again() {
    let mut world = World::new(141, false);
    world.end(tasks::End::Parked, 0);
    account_failed(&mut world);
    let assigned = world.assignments.len();
    world.say(83);
    assert_eq!(world.assignments.len(), assigned);
    assert!(world.store.rows.values().any(|row| matches!(row,
        root::Record::Core(core::Record::Tasks(tasks::Stored::Live(task)))
        if matches!(task.phase, tasks::Phase::Active(tasks::Active::Due)))));
}
#[test]
fn a_run_waiting_for_its_account_starts_once_granted() {
    let mut world = World::new(142, false);
    world.end(tasks::End::Parked, 0);
    account_failed(&mut world);
    let assigned = world.assignments.len();
    world.say(84);
    assert_eq!(world.assignments.len(), assigned);
    account_granted(&mut world);
    assert_eq!(world.assignments.len(), assigned + 1);
    assert_eq!(world.assigned.as_ref().expect("assigned after grant").grants[0].generation, 9);
}
#[test]
fn an_account_unusable_at_the_second_check_waits_and_is_not_held() {
    let mut world = World::new(143, false);
    world.turn(1, 1, None, b"turn to resume");
    world.end(tasks::End::Parked, 1);
    world.fail_account_before_workspace = true;
    let assigned = world.assignments.len();
    world.say(85);
    assert_eq!(world.assignments.len(), assigned);
    assert!(world.store.rows.values().any(|row| matches!(row,
        root::Record::Core(core::Record::Tasks(tasks::Stored::Live(task)))
        if matches!(task.phase, tasks::Phase::Active(tasks::Active::Due)))));
    account_granted(&mut world);
    assert_eq!(world.assignments.len(), assigned + 1);
}

fn current_window_proof(world: &World) -> &core::RunProof {
    let task = world.assigned.as_ref().expect("assigned chat").task;
    let key = root::Key::Core(core::Key::Core(core::CoreKey::RunProof(task)));
    let root::Record::Core(core::Record::Core(core::CoreRecord::RunProof(proof))) = &world.store.rows[&key] else {
        panic!("durable window proof")
    };
    proof
}

#[test]
fn a_window_turn_moves_the_tasks_window_in_its_commit() {
    let mut world = World::new(113, false);
    let attempt = world.assigned.as_ref().expect("assigned chat").attempt;
    world.turn(1, 1, None, b"opening turn");
    assert_eq!(current_window_proof(&world).window, Some(core::CurrentWindow { attempt, turn: 1, closed: false }));
    world.window_turn(2, 2, None, b"compacted window");
    assert_eq!(current_window_proof(&world).window, Some(core::CurrentWindow { attempt, turn: 2, closed: false }));
    let proof = current_window_proof(&world);
    let key = root::Key::Core(core::Key::Core(core::CoreKey::Turn { task: proof.task, attempt, turn: 2 }));
    let root::Record::Core(core::Record::Core(core::CoreRecord::Turn(turn))) = &world.store.rows[&key] else {
        panic!("committed window turn")
    };
    assert!(turn.opens_window);
    world.end(tasks::End::Parked, 2);
    world.say(85);
    assert_eq!(current_window_proof(&world).window, Some(core::CurrentWindow { attempt, turn: 2, closed: false }));
}

#[test]
fn a_fresh_start_clears_the_window_until_its_first_turn() {
    let mut world = World::new(114, false);
    world.window_turn(1, 1, None, b"old window");
    world.end(tasks::End::Failed(tasks::Class::Transient), 1);
    world.retry_due();
    let attempt = current_window_proof(&world).attempt;
    assert_eq!(current_window_proof(&world).window, None);
    world.turn(1, 1, None, b"fresh first turn");
    assert_eq!(current_window_proof(&world).window, Some(core::CurrentWindow { attempt, turn: 1, closed: false }));
}

#[test]
fn a_transcript_read_as_unread_fails_the_run_and_the_next_starts_fresh() {
    let mut world = World::new(117, false);
    world.turn(1, 1, None, b"unread window");
    park_chat(&mut world, 1);
    let task = current_window_proof(&world).task;
    world.store.unread_transcript(task);
    let assigned = world.assignments.len();
    world.say(89);
    assert_eq!(world.assignments.len(), assigned);
    world.retry_due();
    let fresh = world.assigned.as_ref().expect("fresh retry after unread transcript");
    assert_eq!(world.assignments.len(), assigned + 1);
    assert!(!fresh.run.policy.resume);
    assert!(fresh.transcript.is_empty());
}

fn park_chat(world: &mut World, spent: u64) {
    world.end(tasks::End::Parked, spent);
}
