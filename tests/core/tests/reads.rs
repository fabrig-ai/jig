use jig_core_world::{
    effects::Cut,
    reads::{Fence, ask, complete, latest},
    stories::reads::{conversation, request, world},
};
use jig_domain_core::{self as core, documents, reads};
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_domain as root;

fn a_read_waits_for_the_commit_in_flight_before_it() {
    assert_eq!(Fence(7).answered(6), Err("read preceded its durable prerequisite"));
    assert_eq!(Fence(7).restarted(6), Err("crash retracted an observed read"));
    let mut world = world(761);
    let task = world.assigned.as_ref().expect("chat").task;
    let number = world.store.applied + 1;
    world.store.fault(jig_core_world::Fault::Hold { commits: 1 });
    world.cut = Some(Cut::Submitted(number));
    request(&mut world, 1, people::Ask::Say { project: 1, task, words: b"not yet acknowledged".as_slice().into() });
    assert_eq!(world.reached_cut, Some(Cut::Submitted(number)));
    assert!(world.store.applied < number);
    ask(&mut world, 1, reads::Read::Task { task });
    assert!(world.reads.as_ref().expect("party").receipts.is_empty());
    let acknowledged = world.store.release_held().expect("one held commit completion");
    world.send(root::Event::Committed { number: acknowledged, fill: 0 });
    let receipt = &world.reads.as_ref().expect("party").receipts[0];
    assert_eq!(receipt.1, number);
    assert!(matches!(receipt.2, reads::Answer::Document(_)));
    world.restart(761, false);
    assert_eq!(world.reads.as_ref().expect("party").receipts.len(), 1, "the party keeps the read across a crash");
}

fn a_task_read_after_it_ended_says_how_and_by_whom() {
    let mut world = world(762);
    let task = world.assigned.as_ref().expect("chat").task;
    request(
        &mut world,
        2,
        people::Ask::Cancel { project: 1, task, reason: b"withdrawn".as_slice().into(), grace: None },
    );
    world.end(tasks::End::Parked, 0);
    ask(&mut world, 2, reads::Read::Task { task });
    let reads::Answer::Historical(value) = latest(&world) else { panic!("retained task") };
    assert_eq!(value.by, Some(tasks::Party::Person(1)));
    assert!(value.at.is_some());
    let reads::Past::Task { document, result } = &value.value else { panic!("full retained task") };
    assert_eq!(document.chip.number, task);
    let documents::Ending::Cancelled { by, at, reason, .. } = &result.ending else { panic!("cancellation") };
    assert_eq!(*by, tasks::Party::Person(1));
    assert!(*at > skein_lib::Wall::EPOCH);
    assert_eq!(reason.bytes.as_ref(), b"withdrawn");
}

fn a_conversation_reads_whole_from_its_end_after_its_words_were_taken() {
    let mut world = world(763);
    let task = world.assigned.as_ref().expect("chat").task;
    let message = world.say(42);
    world.turn(1, 1, Some(message), b"took the complete words");
    world.end(
        tasks::End::Finished {
            result: tasks::TaskResult::Report { words: b"finished".as_slice().into() },
            cancel_delegates: false,
        },
        1,
    );
    let first = conversation(&mut world, task, 10);
    assert!(first.windows(2).all(|rows| rows[0].position > rows[1].position));
    let line = first
        .iter()
        .find(|line| matches!(line.line, tasks::Line::Words { message: number, .. } if number == message))
        .expect("the original words survived their unread entry");
    let tasks::Line::Words { words, .. } = &line.line else { panic!() };
    assert_eq!(words.as_ref(), [42]);
    world.restart(763, false);
    assert_eq!(
        conversation(&mut world, task, 50),
        first,
        "a crash cannot retract the immutable conversation the party read"
    );
}

fn an_inbox_longer_than_the_engine_holds_pages_from_the_store() {
    let mut world = world(764);
    let mut tasks = Vec::new();
    for number in 0..3 {
        if number != 0 {
            let proposed_goal = jig_core_world::fixtures::chat(&world, 1, b"next chat".as_slice().into());
            request(&mut world, 10 + number, proposed_goal);
        }
        tasks.push(world.assigned.as_ref().expect("chat").task);
        world.end(
            tasks::End::Finished {
                result: tasks::TaskResult::Report { words: b"result".as_slice().into() },
                cancel_delegates: false,
            },
            0,
        );
    }
    let mut from = None;
    let mut found = Vec::new();
    for page in 0..8 {
        ask(
            &mut world,
            100 + page,
            reads::Read::Inbox {
                group: reads::Group::Updates,
                filter: reads::Filter { project: Some(1), goal: None },
                from: from.take(),
            },
        );
        let reads::Answer::Page(value) = latest(&world) else { panic!("inbox page") };
        assert!(value.rows.len() <= 2);
        for row in &value.rows {
            let reads::PageRow::Inbox(entry) = row else { panic!("inbox owner row") };
            assert_eq!(entry.project, Some(1));
            assert!(entry.at.is_some());
            let documents::TaskLink::Retained { chip, .. } = &entry.task else { panic!("kept ended identity") };
            found.push(chip.number);
        }
        from.clone_from(&value.next);
        if from.is_none() {
            break;
        }
    }
    tasks.reverse();
    assert_eq!(
        found, tasks,
        "the visible cached reference is joined and older results are neither omitted nor repeated"
    );
}

fn a_read_when_loads_are_full_is_busy() {
    let mut world = world(765);
    world.reads.as_mut().expect("party").hold_loads = true;
    ask(&mut world, 200, reads::Read::Task { task: 1000 });
    assert_eq!(world.reads.as_ref().expect("party").loads.len(), 1);
    ask(&mut world, 201, reads::Read::Turn { task: 1001, attempt: 1, turn: 1 });
    assert_eq!(latest(&world), &reads::Answer::None(reads::Why::Busy));
    assert_eq!(world.reads.as_ref().expect("party").loads.len(), 1, "busy keeps no second load");
    complete(&mut world);
    assert_eq!(latest(&world), &reads::Answer::None(reads::Why::Unknown));
    ask(&mut world, 202, reads::Read::Task { task: 1002 });
    assert_eq!(world.reads.as_ref().expect("party").loads.len(), 1, "the terminal freed its load slot");
    complete(&mut world);
}

#[test]
fn five_independent_read_stories_fit_the_shared_process_budget() {
    a_read_waits_for_the_commit_in_flight_before_it();
    a_task_read_after_it_ended_says_how_and_by_whom();
    a_conversation_reads_whole_from_its_end_after_its_words_were_taken();
    an_inbox_longer_than_the_engine_holds_pages_from_the_store();
    a_read_when_loads_are_full_is_busy();
    a_read_after_the_sign_in_ended_is_answered_signed_out();
    a_read_whose_sign_in_ends_while_it_loads_is_signed_out();
    jig_core_world::stories::reads::a_backward_wall_correction_keeps_reply_paging_whole_after_restart();
}

#[test]
fn updates_page_from_the_index_newest_first_after_restart() {
    let mut world = world(780);
    let task = world.assigned.as_ref().expect("chat").task;
    world.turn(1, 1, None, b"first reply");
    world.advance(skein_lib::Duration::from_secs(1));
    world.turn(2, 2, None, b"second reply");
    world.restart(780, false);
    let group = reads::Group::Updates;
    let filter = reads::Filter { project: Some(1), goal: None };
    let mut from = Some(reads::Cursor::Store { scope: reads::Scope::Inbox { group, filter }, after: None });
    let mut found = Vec::new();
    for page in 0..4 {
        ask(&mut world, 700 + page, reads::Read::Inbox { group, filter, from: from.take() });
        let reads::Answer::Page(value) = latest(&world) else { panic!("updates page") };
        for row in &value.rows {
            let reads::PageRow::Inbox(entry) = row else { panic!("update") };
            assert!(matches!(entry.task, documents::TaskLink::Retained { ref chip, .. } if chip.number == task));
            let documents::InboxObject::Reply { message } = entry.object else { panic!("reply") };
            found.push(message);
        }
        from.clone_from(&value.next);
        if from.is_none() {
            break;
        }
    }
    assert_eq!(found, [2, 1]);
}

#[test]
fn a_forgotten_chats_result_leaves_the_updates_index() {
    let mut world = jig_core_world::stories::reads::forgettable_world(781);
    let task = world.assigned.as_ref().expect("chat").task;
    world.turn(1, 1, None, b"reply");
    request(&mut world, 71, people::Ask::MarkSeen { seen: people::Seen::Conversation { task, through: 1 } });
    world.end(
        tasks::End::Finished {
            result: tasks::TaskResult::Report { words: b"done".as_slice().into() },
            cancel_delegates: false,
        },
        1,
    );
    let key = world.store.rows.keys().find(|key| matches!(key, root::Key::Core(core::Key::Tasks(tasks::Key::Update { task: number, object: tasks::UpdateObject::Result, .. })) if number.value() == task)).expect("result index").clone();
    let root::Record::Core(core::Record::Tasks(tasks::Stored::Ended(ended))) =
        &world.store.rows[&root::Key::Core(core::Key::Tasks(tasks::Key::Ended(task)))]
    else {
        panic!("ended chat")
    };
    let through = ended.result_position;
    let reply = world.store.rows.keys().find(|key| matches!(key, root::Key::Core(core::Key::Tasks(tasks::Key::Update { task: number, object: tasks::UpdateObject::Reply { .. }, .. })) if number.value() == task)).expect("reply index").clone();
    request(&mut world, 70, people::Ask::MarkSeen { seen: people::Seen::Results { through } });
    assert!(!world.store.rows.contains_key(&key));
    assert!(world.store.rows.contains_key(&reply));
    let group = reads::Group::Updates;
    let filter = reads::Filter { project: Some(1), goal: None };
    let scope = reads::Scope::Inbox { group, filter };
    ask(&mut world, 730, reads::Read::Inbox { group, filter, from: Some(reads::Cursor::Store { scope, after: None }) });
    let reads::Answer::Page(page) = latest(&world) else { panic!("forgotten updates page") };
    assert_eq!(page.rows.len(), 0);
    assert_eq!(page.next, None);
}

#[test]
fn the_results_group_pages_results_only_from_the_index() {
    let mut world = world(782);
    world.turn(1, 1, None, b"reply");
    world.end(
        tasks::End::Finished {
            result: tasks::TaskResult::Report { words: b"done".as_slice().into() },
            cancel_delegates: false,
        },
        1,
    );
    world.restart(782, false);
    let scope =
        reads::Scope::Inbox { group: reads::Group::Results, filter: reads::Filter { project: Some(1), goal: None } };
    ask(
        &mut world,
        720,
        reads::Read::Inbox {
            group: reads::Group::Results,
            filter: reads::Filter { project: Some(1), goal: None },
            from: Some(reads::Cursor::Store { scope, after: None }),
        },
    );
    let reads::Answer::Page(value) = latest(&world) else { panic!("results page") };
    assert_eq!(value.rows.len(), 1);
    let reads::PageRow::Inbox(entry) = value.rows.get(0).expect("result") else { panic!("inbox result") };
    assert!(matches!(entry.object, documents::InboxObject::Result { .. }));
}

fn a_read_after_the_sign_in_ended_is_answered_signed_out() {
    let mut world = jig_core_world::stories::reads::forgettable_world(784);
    end_sign_in(&mut world);
    ask(&mut world, 800, reads::Read::Frame);
    assert_eq!(latest(&world), &reads::Answer::None(reads::Why::SignedOut));
}

fn a_read_whose_sign_in_ends_while_it_loads_is_signed_out() {
    let mut world = world(785);
    world.reads.as_mut().expect("party").hold_loads = true;
    ask(&mut world, 801, reads::Read::Task { task: 1000 });
    assert_eq!(world.reads.as_ref().expect("party").loads.len(), 1);
    end_sign_in(&mut world);
    complete(&mut world);
    assert_eq!(latest(&world), &reads::Answer::None(reads::Why::SignedOut));
    assert!(world.reads.as_ref().expect("party").loads.is_empty());
}

fn end_sign_in(world: &mut jig_core_world::effects::World) {
    world.send(root::Event::Core(core::Event::People(people::Event::SignOut {
        reply_to: skein_lib::ReplyTo::new(skein_lib::Token::new(80_000)),
        sign_in: world.sign_in.expect("original sign-in"),
    })));
}

fn chat_task(world: &jig_core_world::effects::World) -> u64 {
    world.assigned.as_ref().expect("chat").task
}

fn message(
    world: &mut jig_core_world::effects::World,
    source: u64,
    target: u64,
    completion: u32,
    kind: tasks::MessageKind,
    words: &[u8],
) -> u64 {
    world.send(root::Event::Core(core::Event::NamedAction {
        to: skein_lib::ReplyTo::new(skein_lib::Token::new(40_000 + u64::from(completion))),
        key: run_key(world, source, completion),
        action: core::NamedAction::Message { target, kind, words: words.into() },
    }));
    match world.answers.last() {
        Some((_, _, core::CallPart::Sent { message })) => *message,
        other => panic!("message admitted: {other:?}"),
    }
}

fn question(world: &mut jig_core_world::effects::World, target: u64, completion: u32) -> u64 {
    message(world, chat_task(world), target, completion, tasks::MessageKind::Question, b"original question?")
}

fn finish_chat(world: &mut jig_core_world::effects::World, task: u64) {
    let key = run_key(world, task, 0);
    world.send(root::Event::Answer {
        read: None,
        refused: Box::new([]),
        channel: skein_lib::Token::new(7),
        task,
        attempt: key.attempt,
        cumulative: 0,
        end: tasks::End::Finished {
            result: tasks::TaskResult::Report { words: b"done".as_slice().into() },
            cancel_delegates: false,
        },
    });
}

fn past(world: &jig_core_world::effects::World) -> &reads::Past {
    let reads::Answer::Historical(value) = latest(world) else { panic!("retained document: {:?}", latest(world)) };
    &value.value
}

#[test]
fn a_question_read_after_its_task_ended_finds_both_lines_by_key() {
    let (mut world, task) = question_world(791, None);
    let sender = chat_task(&world);
    let message = question(&mut world, task, 1);
    request(
        &mut world,
        91,
        people::Ask::AnswerQuestion { project: 1, task, question: message, words: b"the answer".as_slice().into() },
    );
    finish_chat(&mut world, sender);
    finish_chat(&mut world, task);
    ask(&mut world, 901, reads::Read::Question { task, message });
    let reads::Past::Question { question, answer, ending } = past(&world) else { panic!("question") };
    assert_eq!(question.words.bytes.as_ref(), b"original question?");
    let answer = answer.as_ref().expect("answer by key");
    assert!(matches!(&answer.line, tasks::Line::Words { kind: tasks::MessageKind::Answer { question }, words, .. }
        if *question == message && words.as_ref() == b"the answer"));
    assert!(ending.is_some());
}

#[test]
fn an_escalation_read_shows_its_latest_decision_and_its_hold() {
    let mut world = world(792);
    let task = chat_task(&world);
    world.send(root::Event::Core(core::Event::Tasks(tasks::Event::Hold { task, why: tasks::Hold::Stalled })));
    world.end(tasks::End::Parked, 0);
    request(
        &mut world,
        93,
        people::Ask::DecideEscalation {
            project: 1,
            task,
            revision: 1,
            decision: people::EscalationDecision::Reject { reason: b"stay stopped".as_slice().into() },
        },
    );
    request(&mut world, 94, people::Ask::Cancel { project: 1, task, reason: b"end".as_slice().into(), grace: None });
    ask(&mut world, 902, reads::Read::Escalation { task });
    let reads::Past::Escalation { decision, hold, .. } = past(&world) else { panic!("escalation") };
    assert_eq!(decision.revision, 1);
    assert_eq!(decision.decision, people::EscalationDecision::Reject { reason: b"stay stopped".as_slice().into() });
    assert!(hold.is_some());
}

#[test]
fn a_task_proposal_read_finds_its_history_row_by_key() {
    let mut world = world(793);
    let task = chat_task(&world);
    let proposal = world.propose(1);
    world.decide_proposal(proposal, false, 95);
    finish_chat(&mut world, task);
    ask(&mut world, 903, reads::Read::Proposal { proposer: documents::Proposer::Task(task), proposal });
    let reads::Past::Proposal(value) = past(&world) else { panic!("proposal") };
    assert_eq!(value.number, proposal);
    assert!(matches!(&value.state, documents::ProposalHistoryState::Rejected { by, reason }
        if *by == tasks::Party::Person(1) && reason.bytes.as_ref() == b"not now"));
}

#[test]
fn a_question_older_than_the_kept_lines_reads_as_gone() {
    let bound = 2;
    let (mut world, sender) = question_world(794, Some(bound));
    let task = chat_task(&world);
    let oldest = message(&mut world, sender, task, 2, tasks::MessageKind::Question, b"original question?");
    let answer = reply(&mut world, task, sender, oldest, bound + 2);
    take_message(&mut world, task, 1, oldest);
    take_message(&mut world, sender, 1, answer);
    for number in 2..=bound + 1 {
        let message =
            message(&mut world, sender, task, number + 1, tasks::MessageKind::Question, b"original question?");
        let answer = reply(&mut world, task, sender, message, number + bound + 1);
        take_message(&mut world, task, number, message);
        take_message(&mut world, sender, number, answer);
    }
    ask(&mut world, 904, reads::Read::Question { task, message: oldest });
    assert_eq!(latest(&world), &reads::Answer::None(reads::Why::Gone));
}

fn reply(world: &mut jig_core_world::effects::World, source: u64, target: u64, question: u64, completion: u32) -> u64 {
    message(world, source, target, completion, tasks::MessageKind::Answer { question }, b"answer")
}

fn run_key(world: &jig_core_world::effects::World, task: u64, completion: u32) -> core::CallKey {
    let (_, attempt) = world.assignments.iter().rev().find(|(number, _)| *number == task).expect("assigned task");
    core::CallKey { task, attempt: *attempt, completion, position: 0 }
}

fn take_message(world: &mut jig_core_world::effects::World, task: u64, turn: u32, message: u64) {
    world.send(root::Event::Turn {
        opens_window: turn == 1,
        refused: Box::new([]),
        channel: skein_lib::Token::new(7),
        task,
        attempt: run_key(world, task, 0).attempt,
        turn,
        cumulative: 0,
        read: Some(message),
        transcript: b"took the message".as_slice().into(),
    });
}

fn question_world(seed: u64, bound: Option<u32>) -> (jig_core_world::effects::World, u64) {
    question_world_with_text_bound(seed, bound, None)
}

fn question_world_with_text_bound(
    seed: u64,
    bound: Option<u32>,
    text: Option<u32>,
) -> (jig_core_world::effects::World, u64) {
    let (configuration, mut limits) = jig_core_world::faults::plan_fixture(seed);
    if let Some(text) = text {
        limits.core.documents.text_bytes = text;
    }
    if let Some(bound) = bound {
        limits.core.tasks.inbox_messages = bound;
        limits.core.tasks.run_inbox = bound;
    }
    jig_core_world::peers::journal(&mut limits);
    let mut world = jig_core_world::effects::World::configured(seed, false, configuration, limits);
    world.reads = Some(jig_core_world::reads::Parties::new(world.store.applied));
    let parent = chat_task(&world);
    let mut authority = world.assigned.as_ref().expect("parent").run.authority.clone();
    authority.budget.spend = 20;
    authority.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
    world.send(root::Event::Core(core::Event::DelegateValidated {
        to: skein_lib::ReplyTo::new(skein_lib::Token::new(49_001)),
        key: run_key(&world, parent, 1),
        batch: Box::new([core::Delegate {
            deadline: jig_domain_tasks::Deadline::Named,
            label: 0,
            flags: jig_domain_tasks::Flags::PLAIN,
            tracked: None,
            budget: jig_domain_people::Budget::Named,
            executor: tasks::Executor::Agent { charter: 1 },
            spec: tasks::Spec {
                asks: false,
                words: b"ask the parent".as_slice().into(),
                parameters: Box::new([]),
                inputs: Box::new([]),
            },
            contract: tasks::Contract::Report { words: 128 },
            authority,
            llm: None,
            symbolic_grants: Box::new([]),
            dependencies: Box::new([]),
            wake: tasks::WakePolicy::DEFAULT,
        }]),
        stubs: Box::new([]),
    }));
    assert_ne!(chat_task(&world), parent, "the delegate runs");
    (world, parent)
}

#[test]
fn a_persons_answer_reads_the_line_in_the_delegates_activity() {
    let (mut world, task) = question_world(795, None);
    let delegate = chat_task(&world);
    let message = question(&mut world, task, 1);
    request(
        &mut world,
        124,
        people::Ask::AnswerQuestion { project: 1, task, question: message, words: b"answer".as_slice().into() },
    );
    let sent = live_task(&world, delegate)
        .inbox
        .iter()
        .find(|word| word.kind == tasks::MessageKind::Answer { question: message })
        .expect("person's answer")
        .number;
    finish_chat(&mut world, delegate);
    finish_chat(&mut world, task);
    ask(&mut world, 905, reads::Read::Question { task, message });
    let reads::Past::Question { answer, .. } = past(&world) else { panic!("question") };
    let answer = answer.as_ref().expect("delegate's answer by key");
    assert_eq!(answer.task, delegate);
    assert!(
        matches!(&answer.line, tasks::Line::Words { message: number, kind: tasks::MessageKind::Answer { question }, words, .. }
        if *number == sent && *question == message && words.as_ref() == b"answer")
    );
}

#[test]
fn a_moved_question_waits_for_its_person_after_the_chat_closes() {
    const SEED: u64 = 810;
    // Closing consists of moving the goal and cancelling its former requester.
    const OPERATIONS: u32 = 2;
    let (mut world, chat) = question_world(SEED, None);
    let goal = chat_task(&world);
    let message = question(&mut world, chat, 1);
    finish_chat(&mut world, goal);
    let mut operations = skein_lib::List::with_capacity(OPERATIONS);
    operations
        .push(people::Operation::Move { task: goal, reason: b"continue".as_slice().into() })
        .expect("move fixture");
    operations
        .push(people::Operation::Cancel { task: chat, reason: b"close".as_slice().into(), grace: None })
        .expect("close fixture");
    request(&mut world, 125, people::Ask::Compound { project: 1, operations });
    ask(&mut world, 998, reads::Read::Question { task: goal, message });
    let question = question_read(&world);
    assert_eq!(question.words.bytes.as_ref(), b"original question?");
    world.restart_with(jig_core_world::faults::plan_fixture(SEED).0);
    let row = live_task(&world, goal);
    assert_eq!(row.requester, tasks::Party::Person(1));
    assert_eq!(row.phase, tasks::Phase::Active(tasks::Active::Idle));
    assert_eq!(row.question_lines[0].message, message);
    assert_eq!(row.question_lines[0].line, row.questions[0].line);
    ask(&mut world, 999, reads::Read::Question { task: goal, message });
    let question = question_read(&world);
    assert_eq!(question.words.bytes.as_ref(), b"original question?");
    request(
        &mut world,
        126,
        people::Ask::AnswerQuestion { project: 1, task: goal, question: message, words: b"proceed".as_slice().into() },
    );
    assert!(live_task(&world, goal).questions.is_empty());
    assert!(
        live_task(&world, goal).inbox.iter().any(|word| word.from == tasks::Party::Person(1)
            && word.kind == tasks::MessageKind::Answer { question: message })
    );
}

fn question_read(world: &jig_core_world::effects::World) -> &documents::Question {
    if let reads::Answer::Document(document) = latest(world)
        && let documents::Document::Question(question) = document.as_ref()
    {
        return question;
    }
    if let reads::Answer::Historical(document) = latest(world)
        && let reads::Past::Question { question, .. } = &document.value
    {
        return question;
    }
    panic!("question by its original activity key: {:?}", latest(world));
}

fn live_task(world: &jig_core_world::effects::World, task: u64) -> &tasks::TaskRecord {
    let root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) =
        &world.store.rows[&root::Key::Core(core::Key::Tasks(tasks::Key::Live(task)))]
    else {
        panic!("live task")
    };
    row
}

fn ended_delegate_world(seed: u64) -> (jig_core_world::effects::World, u64, u64) {
    let mut world = world(seed);
    let chat = chat_task(&world);
    finish_chat(&mut world, chat);
    let ask = jig_core_world::fixtures::goal(
        &world,
        1,
        b"goal with a delegate".as_slice().into(),
        people::TaskExecutor::Agent { charter: 1 },
        jig_core_world::fixtures::Goal { budget: 300, priority: 1, ..Default::default() },
    );
    request(&mut world, 124, ask);
    let goal = chat_task(&world);
    world.delegate();
    world.observed(13);
    let delegate = world.procedure.expect("ended delegate");
    let message = live_task(&world, goal).inbox.last().expect("delegate result").number;
    take_message(&mut world, goal, 1, message);
    finish_chat(&mut world, goal);
    (world, goal, delegate)
}

#[test]
fn ended_delegates_page_from_the_index() {
    let (mut world, goal, delegate) = ended_delegate_world(800);
    world.restart(800, false);
    ask(&mut world, 950, reads::Read::EndedDelegates { task: goal, from: None });
    let reads::Answer::Page(page) = latest(&world) else { panic!("ended delegates") };
    assert_eq!(page.rows.len(), 1);
    let reads::PageRow::EndedDelegate(row) = page.rows.get(0).expect("one delegate") else { panic!("delegate") };
    assert_eq!(row.chip.number, delegate);
    assert!(matches!(row.ended, documents::Ending::Done(_)));
    assert!(page.next.is_none());
}

fn seed_history(world: &mut jig_core_world::effects::World, task: u64) {
    let row = tasks::History {
        task,
        revision: 1,
        by: tasks::Party::Person(1),
        change: tasks::Change::Amended,
        reason: Box::new([]),
        proposal: None,
        at: skein_lib::Wall::EPOCH,
        amendment: None,
    };
    world.seed(Box::new([root::Record::Core(core::Record::Tasks(tasks::Stored::History(row)))]));
}

fn subtree_history(world: &mut jig_core_world::effects::World, task: u64) -> Vec<(u8, u64, u64)> {
    let mut from = None;
    let mut rows = Vec::new();
    for number in 0..24 {
        ask(world, 960 + number, reads::Read::History { task, subtree: true, from: from.take() });
        let reads::Answer::Page(page) = latest(world) else { panic!("history") };
        assert!(page.rows.len() <= 2);
        for row in &page.rows {
            match row {
                reads::PageRow::History(line) => rows.push((0, line.task, line.revision)),
                reads::PageRow::Milestone(line) => rows.push((1, line.task, line.position)),
                reads::PageRow::EndedDelegate(_)
                | reads::PageRow::Activity { .. }
                | reads::PageRow::Inbox(_)
                | reads::PageRow::Line(_)
                | reads::PageRow::Note(_)
                | reads::PageRow::Spend(_) => panic!("history family"),
            }
        }
        from.clone_from(&page.next);
        if from.is_none() {
            return rows;
        }
    }
    panic!("bounded history exhausted");
}

#[test]
fn a_subtree_history_reads_each_delegates_rows_found_by_index() {
    let (mut world, goal, delegate) = ended_delegate_world(801);
    world.restart(801, false);
    seed_history(&mut world, goal);
    seed_history(&mut world, delegate);
    let rows = subtree_history(&mut world, goal);
    assert!(rows.iter().any(|row| row.1 == goal));
    assert!(rows.iter().any(|row| row.1 == delegate));
    assert_eq!(&rows[..2], &[(0, goal, 1), (1, goal, 1)]);
    let mut members: Vec<_> = rows.iter().map(|row| row.1).collect();
    members.dedup();
    assert_eq!(members, [goal, delegate]);
    assert!(rows.windows(2).all(|pair| pair[0].1 != pair[1].1 || (pair[0].0, pair[0].2) < (pair[1].0, pair[1].2)));
    assert!(
        world
            .erase(root::Key::Core(core::Key::Tasks(tasks::Key::Delegate { requester: goal, task: delegate })))
            .is_some()
    );
    let without_edge = subtree_history(&mut world, goal);
    assert!(!without_edge.is_empty());
    assert!(without_edge.iter().all(|row| row.1 == goal));
}

#[test]
fn a_turn_past_the_read_bound_reports_its_size() {
    use jig_core_world::stories::reads::{LARGE_TURN_BYTES, TURN_READ_BYTES, world_with_large_turn};
    let mut world = world_with_large_turn(802);
    let task = world.assigned.as_ref().expect("chat").task;
    let body = vec![b'x'; usize::try_from(LARGE_TURN_BYTES).expect("bounded fixture")];
    assert!(u64::try_from(body.len()).expect("payload length") > u64::from(TURN_READ_BYTES));
    world.turn(1, 1, None, &body);
    ask(&mut world, 980, reads::Read::Turn { task, attempt: 1, turn: 1 });
    assert_eq!(latest(&world), &reads::Answer::None(reads::Why::TooLarge { bytes: u64::from(LARGE_TURN_BYTES) }));
    assert_eq!(world.reads.as_ref().expect("party").receipts.len(), 1);
}

#[test]
fn a_turn_load_preserves_the_gone_terminal() {
    let mut world = world(803);
    world.reads.as_mut().expect("party").hold_loads = true;
    let task = chat_task(&world);
    ask(&mut world, 981, reads::Read::Turn { task, attempt: 1, turn: 1 });
    let (load, _) = world.reads.as_mut().expect("party").loads.pop().expect("one turn load");
    world.send(root::Event::Core(core::Event::ReadLoaded { load, result: reads::Loaded::Gone }));
    assert_eq!(latest(&world), &reads::Answer::None(reads::Why::Gone));
    assert_eq!(world.reads.as_ref().expect("party").receipts.len(), 1);
}

#[test]
fn a_delegates_question_waits_for_the_person_after_the_director_reads_it() {
    let (mut world, director) = question_world(796, None);
    let delegate = chat_task(&world);
    let message = question(&mut world, director, 1);
    let original_at = jig_core_world::fixtures::live_row(&world, director).inbox[0].at;
    take_message(&mut world, director, 1, message);
    assert!(jig_core_world::fixtures::live_row(&world, director).inbox.is_empty());
    for restart in [false, true] {
        if restart {
            world.restart_with(jig_core_world::faults::plan_fixture(796).0);
        }
        ask(
            &mut world,
            906,
            reads::Read::Inbox {
                group: reads::Group::Questions,
                filter: reads::Filter { project: Some(1), goal: None },
                from: None,
            },
        );
        let reads::Answer::Page(page) = latest(&world) else { panic!("questions inbox") };
        assert_eq!(page.rows.len(), 1);
        let reads::PageRow::Inbox(entry) = page.rows.get(0).expect("one question entry") else {
            panic!("question entry")
        };
        assert_eq!(entry.object, documents::InboxObject::Question { message });
        assert_eq!(entry.at, Some(original_at));
        ask(&mut world, 907, reads::Read::Question { task: director, message });
        let reads::Answer::Document(frame) = latest(&world) else { panic!("live question: {:?}", latest(&world)) };
        let documents::Document::Question(question) = frame.as_ref() else { panic!("question document") };
        assert_eq!(question.words.bytes.as_ref(), b"original question?");
        assert_eq!(question.at, original_at);
        assert!(
            question
                .acts
                .as_slice()
                .iter()
                .any(|offer| offer.act == core::acts::Act::Answer && offer.admission == core::acts::Admission::Allowed)
        );
    }
    request(
        &mut world,
        96,
        people::Ask::AnswerQuestion {
            project: 1,
            task: director,
            question: message,
            words: b"the person's answer".as_slice().into(),
        },
    );
    let row = jig_core_world::fixtures::live_row(&world, director);
    assert!(row.inbox.iter().any(|word| word.kind == tasks::MessageKind::QuestionAnswered { question: message }));
    assert!(
        jig_core_world::fixtures::live_row(&world, delegate)
            .inbox
            .iter()
            .any(|word| word.kind == tasks::MessageKind::Answer { question: message }
                && word.words.as_ref() == b"the person's answer")
    );
    assert!(row.question_lines[0].answer.is_some());
    assert_eq!(
        jig_core_world::fixtures::live_row(&world, delegate).questions.len(),
        0,
        "the person's answer consumed the delegate's reserved answer credit"
    );
    ask(
        &mut world,
        908,
        reads::Read::Inbox {
            group: reads::Group::Questions,
            filter: reads::Filter { project: Some(1), goal: None },
            from: None,
        },
    );
    let reads::Answer::Page(page) = latest(&world) else { panic!("questions inbox") };
    assert!(page.rows.is_empty());
    ask(&mut world, 909, reads::Read::Question { task: director, message });
    let reads::Past::Question { question, answer, .. } = past(&world) else { panic!("historical question") };
    assert!(question.acts.is_empty());
    assert!(answer.is_some());
}

#[test]
fn a_stored_question_past_the_text_bound_is_cut_and_marked() {
    stored_cut_question(b"plan?", 4, b"plan");
}

#[test]
fn a_stored_question_is_cut_before_a_character_straddling_the_text_bound() {
    stored_cut_question("plané?".as_bytes(), 5, b"plan");
}

fn stored_cut_question(words: &[u8], bound: u32, expected: &[u8]) {
    let (mut world, task) = question_world_with_text_bound(799, None, Some(bound));
    let sender = chat_task(&world);
    let message = message(&mut world, sender, task, 1, tasks::MessageKind::Question, words);
    request(
        &mut world,
        91,
        people::Ask::AnswerQuestion { project: 1, task, question: message, words: b"Go ahead.".as_slice().into() },
    );
    finish_chat(&mut world, sender);
    finish_chat(&mut world, task);
    ask(&mut world, 901, reads::Read::Question { task, message });
    let reads::Past::Question { question, answer, .. } = past(&world) else { panic!("stored question") };
    assert_eq!(question.words.bytes.as_ref(), expected);
    assert!(question.words.cut);
    assert!(answer.is_some(), "the full stored question remains answerable");
}
