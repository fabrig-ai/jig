//! Inbox selection contexts preserve the source's membership and original rows.
use super::*;
use tasks::{Newest, Update, UpdateObject};

fn fixture() -> (Core, Limits) {
    let (mut core, mut limits) = watch_core();
    let template = core.tasks.task(1).unwrap().clone();
    limits.tasks.tasks = 16;
    limits.tasks.project_tasks = 16;
    limits.tasks.tree_tasks = 16;
    limits.tasks.depth = 4;
    limits.tasks.inbox_messages = 16;
    limits.people.inbox_tasks = 32;
    limits.people.requests = 4;
    limits.people.inbox_entries = 16;
    limits.documents.rows = 16;
    let ledgers: Vec<_> = core.tasks.funding_records().iter().map(|(_, row)| *row).collect();
    core.tasks = fixtures::fresh_tasks(&core, &limits);
    for row in ledgers {
        send_task(&mut core, &limits, tasks::Event::Restore { record: tasks::Stored::Ledger(row) });
    }
    for (number, requester) in [
        (1, tasks::Party::Person(1)),
        (2, tasks::Party::Task(1)),
        (3, tasks::Party::Task(2)),
        (4, tasks::Party::Deployment { project: 1 }),
        (5, tasks::Party::Task(99)),
        (99, tasks::Party::Person(2)),
    ] {
        let mut row = template.clone();
        row.number = number;
        row.requester = requester;
        send_task(&mut core, &limits, tasks::Event::Restore { record: tasks::Stored::Live(Box::new(row)) });
        assert!(core.tasks.task(number).is_some());
    }
    send_task(&mut core, &limits, tasks::Event::Check);
    send_task(&mut core, &limits, tasks::Event::Restored);
    let identity = core.people.identity(1).unwrap().clone();
    core.people = people::Domain::new(&limits.people, Box::new([]), core.settings.deployment_provider, Box::new([]));
    let mut out = Queue::with_capacity(people::max_out(&limits.people));
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people };
    for record in [
        people::Stored::Person { number: 1, identity },
        people::Stored::Roles {
            project: 1,
            holdings: Box::new([people::Holding { person: 1, role: people::Role::Owner }]),
        },
        people::Stored::SignIn { number: 10, person: 1, expires: Wall::from_nanos(Duration::from_secs(60).as_nanos()) },
    ] {
        people::step(&mut core.people, &env, people::Event::Restore { record }, &mut out);
        while out.pop().is_some() {}
    }
    people::step(&mut core.people, &env, people::Event::Restored, &mut out);
    assert!(core.people.identity(1).is_some());
    assert_eq!(core.people.person(10, Time::ZERO, Wall::EPOCH), Some(1));
    (core, limits)
}

fn waiting(core: &mut Core, limits: &Limits, task: u64, kinds: &[people::EntryKind]) {
    let mut entries = List::with_capacity(u32::try_from(kinds.len()).unwrap());
    for (index, kind) in kinds.iter().enumerate() {
        entries
            .push(people::Entry {
                task,
                project: 1,
                whom: people::Whom::Person(1),
                kind: *kind,
                at: Wall::from_nanos(u64::try_from(index).unwrap().checked_add(10).unwrap()),
            })
            .unwrap();
    }
    let mut out = Queue::with_capacity(people::max_out(&limits.people));
    people::step(
        &mut core.people,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people },
        people::Event::Waiting { task, entries: entries.into_boxed() },
        &mut out,
    );
}

fn inbox(core: &Core, limits: &Limits, rows: u32) -> Inbox {
    let Document::Inbox(value) =
        make(core, limits, 1, &Subject::Inbox { party: 1 }, &Window { rows, after: None }, Wall::EPOCH).unwrap()
    else {
        panic!("whole inbox document")
    };
    value
}

fn stored_page(
    core: &mut Core,
    limits: &Limits,
    group: reads::Group,
    goal: Option<u64>,
    rows: Box<[reads::Row]>,
) -> reads::Answer {
    core.reclaim();
    let filter = reads::Filter { project: Some(1), goal };
    let Held::ReadLoad { load, .. } = read_start(
        core,
        limits,
        10,
        reads::Read::Inbox {
            group,
            filter,
            from: Some(reads::Cursor::Store { scope: reads::Scope::Inbox { group, filter }, after: None }),
        },
    ) else {
        panic!("original scoped Inbox load")
    };
    loaded_answer(core, limits, load, reads::Loaded::Page { rows, next: None }).unwrap()
}

fn ended_row(core: &Core, number: u64, ancestors: &[u64]) -> reads::Row {
    let mut owner = core.tasks.task(1).unwrap().clone();
    owner.number = number;
    owner.requester = tasks::Party::Person(1);
    owner.phase = tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Report { words: Box::new([]) }));
    owner.result_position = u64::MAX;
    owner.ended_at = None;
    let object = InboxObject::Result { position: u64::MAX };
    let update = Update { party: 1, at: Wall::EPOCH, task: number, object: UpdateObject::Result };
    let mut row = stored_row(Record::Tasks(tasks::Stored::Update(update)), Some(owner));
    row.ancestors = ancestors.into();
    row.inbox = Some(Cursor::Inbox { task: number, object, at: None });
    row
}

fn context_for(contexts: &List<InboxSelection>, cursor: Cursor) -> &InboxSelection {
    let mut found = None;
    for context in contexts {
        if context.cursor == cursor {
            assert!(found.is_none(), "one complete context for each original cursor");
            found = Some(context);
        }
    }
    found.unwrap()
}

const CHAT: u64 = 1;
const CHAT_PARTY: u64 = 1;
const CHAT_SIGN_IN: u64 = 10;
const ORDINARY_REPLY: u64 = 1;
const FINAL_REPLY: u64 = 2;
const ORDINARY_TURN: u32 = 1;
const FINAL_TURN: u32 = 2;
const RUN_ALLOWANCE: u64 = 10; // The existing claim fixture's allowance; both turns cost zero.
const CHAT_REPLIES: u32 = 2; // The ordinary turn and the final turn.
const CHAT_UPDATES: u32 = CHAT_REPLIES + 1; // Plus the independently positioned result.
const CHAT_RECORDS: u32 = CHAT_REPLIES * 2 + 4; // Activity/index pairs, owner, result/index and ended position.

/// Keep the real task child's turns and closing writes, without a store or host harness.
fn finished_chat() -> (Core, Limits, Box<[Record]>) {
    let (mut core, limits) = fixture();
    // The membership fixture restores disconnected rows for selection only.
    // Its existing make_task helper supplies a funded, kept conversation instead.
    core.tasks = fixtures::fresh_tasks(&core, &limits);
    drop(core.proofs.remove(&CHAT));
    let task = fresh(&mut core.counters, Family::Task).unwrap();
    let attempt = fresh(&mut core.counters, Family::Run).unwrap();
    make_task(&mut core, &limits, task);
    send_task(&mut core, &limits, tasks::Event::Prepare { reply_to: ReplyTo::new(Token::new(task)), task });
    send_task(
        &mut core,
        &limits,
        tasks::Event::Claim {
            reply_to: ReplyTo::new(Token::new(task)),
            task,
            attempt,
            resumed: false,
            budget: RUN_ALLOWANCE,
            account: None,
            writes: Box::new([]),
        },
    );
    let mut kept = List::with_capacity(CHAT_RECORDS);
    for event in [
        tasks::Event::Turn {
            reply_to: ReplyTo::new(Token::new(task)),
            task,
            attempt,
            turn: ORDINARY_TURN,
            read: None,
            refused: Box::new([]),
            offered: None,
            outstanding: Box::new([]),
            cumulative: 0,
        },
        tasks::Event::Turn {
            reply_to: ReplyTo::new(Token::new(task)),
            task,
            attempt,
            turn: FINAL_TURN,
            read: None,
            refused: Box::new([]),
            offered: None,
            outstanding: Box::new([]),
            cumulative: 0,
        },
        // The final turn is kept before its finish is admitted and closing settles.
        tasks::Event::Activation {
            reply_to: ReplyTo::new(Token::new(task)),
            task,
            attempt,
            end: tasks::End::Finished {
                result: tasks::TaskResult::Report { words: Box::new(*b"final report") },
                cancel_delegates: false,
            },
            saved: None,
            cause: tasks::Cause::Priced { cumulative: 0 },
        },
        tasks::Event::EffectsSettled { task },
        tasks::Event::Settled { task },
    ] {
        for output in watch_outputs(&mut core, &limits, Event::Tasks(event)) {
            if let Request::Write(Write::Save(record)) = output
                && let Record::Tasks(
                    tasks::Stored::Activity(tasks::ActivityLine { line: tasks::Line::Turn { .. }, .. })
                    | tasks::Stored::Update(_)
                    | tasks::Stored::Ended(_)
                    | tasks::Stored::Result(_),
                )
                | Record::People(people::Stored::EndedRead { .. }) = &record
            {
                kept.push(record).unwrap();
            }
        }
    }
    core.reclaim();
    (core, limits, kept.into_boxed())
}

fn chat_owner(records: &[Record]) -> &tasks::TaskRecord {
    for record in records {
        if let Record::Tasks(tasks::Stored::Ended(owner)) = record {
            return owner;
        }
    }
    panic!("the real closing decision saved its ended owner")
}

fn chat_rows(records: &[Record], through: u64, replies: bool) -> Box<[reads::Row]> {
    let owner = chat_owner(records);
    let mut rows = List::with_capacity(CHAT_UPDATES);
    for record in records.iter().rev() {
        let Record::Tasks(tasks::Stored::Update(update)) = record else { continue };
        let (object, related) = match update.object {
            UpdateObject::Reply { message, activity } if replies => {
                let mut found = None;
                for record in records {
                    if let Record::Tasks(tasks::Stored::Activity(line)) = record
                        && line.position == activity.value()
                    {
                        found = Some(record);
                        break;
                    }
                }
                let line = found.expect("the reply index names its emitted activity");
                (InboxObject::Reply { message: message.value() }, Some(line.clone()))
            }
            UpdateObject::Result if !replies => (InboxObject::Result { position: owner.result_position }, None),
            UpdateObject::Result | UpdateObject::Reply { .. } | UpdateObject::Proposal { .. } => continue,
        };
        let mut row = stored_row(record.clone(), Some(owner.clone()));
        row.related = related;
        row.ended_read = Some(through);
        row.inbox = Some(Cursor::Inbox { task: owner.number, object, at: Some(update.at) });
        rows.push(row).unwrap();
    }
    rows.into_boxed()
}

fn chat_entry(entries: &[InboxEntry], object: InboxObject) -> &InboxEntry {
    for entry in entries {
        if entry.object == object {
            return entry;
        }
    }
    panic!("chat inbox object")
}

fn assert_chat_replies(entries: &[InboxEntry], records: &[Record]) {
    let owner = chat_owner(records);
    let mut replies = 0_u32;
    for entry in entries {
        if let InboxObject::Reply { .. } = entry.object {
            replies = replies.checked_add(1).expect("bounded chat reply count");
        }
    }
    assert_eq!(replies, CHAT_REPLIES);
    for message in [ORDINARY_REPLY, FINAL_REPLY] {
        let entry = chat_entry(entries, InboxObject::Reply { message });
        let mut activity = None;
        for record in records {
            if let Record::Tasks(tasks::Stored::Activity(line)) = record
                && let tasks::Line::Turn { reply, .. } = line.line
                && reply == message
            {
                activity = Some(line);
                break;
            }
        }
        let activity = activity.expect("the reply has an emitted turn activity");
        assert_eq!((entry.activity, entry.at, entry.unread), (Some(activity.position), Some(activity.at), Some(true)));
        assert_eq!(
            entry.cursor,
            Cursor::Inbox { task: CHAT, object: InboxObject::Reply { message }, at: Some(activity.at) }
        );
        let TaskLink::Retained { chip, .. } = &entry.task else { panic!("retained chat chip") };
        assert_eq!((chip.number, &chip.phase), (CHAT, &PhaseLine::Ended));
        let line = entry.reply.as_ref().expect("conversation line on a Reply");
        assert_eq!(
            (line.chip.number, &line.chip.phase, line.project, line.requester),
            (CHAT, &PhaseLine::Ended, 1, tasks::Party::Person(CHAT_PARTY))
        );
        assert_eq!(line.flags.retention, tasks::Retention::Kept);
        assert_eq!(line.conversation, Some(ConversationRead { seen: 0, unread: Count::Known(CHAT_REPLIES) }));
    }
    assert_eq!(owner.last_reply, FINAL_REPLY);
}

fn start_chat_mark(core: &mut Core, limits: &Limits, seen: people::Seen, key: u8) -> Box<[Request]> {
    core.reclaim();
    watch_outputs(
        core,
        limits,
        Event::People(people::Event::Ask {
            models: Box::new([]),
            reply_to: ReplyTo::new(Token::new(91)),
            sign_in: CHAT_SIGN_IN,
            key: [key; 16],
            ask: people::Ask::MarkSeen { seen },
        }),
    )
}

fn mark_chat_seen(core: &mut Core, limits: &Limits, records: &[Record], seen: people::Seen) -> Box<[Request]> {
    let outputs = start_chat_mark(core, limits, seen, 1);
    let mut request = None;
    for output in &outputs {
        if let Request::Held(held) = output
            && let Held::Load(load) = held.as_ref()
            && let Now::SeenConversation { request: wanted, .. } = load.as_ref()
        {
            request = Some(*wanted);
            break;
        }
    }
    let Some(request) = request else { return outputs };
    let loaded = watch_outputs(
        core,
        limits,
        Event::SeenConversationLoaded { request, row: Some(Box::new(chat_owner(records).clone())), busy: false },
    );
    let mut joined = false;
    for output in &loaded {
        if let Request::Held(held) = output
            && let Held::Load(load) = held.as_ref()
            && let Now::LoadEndedRead { request: wanted, task: CHAT, party: CHAT_PARTY } = load.as_ref()
            && *wanted == request
        {
            joined = true;
            break;
        }
    }
    assert!(joined, "the owner join requests the matching ended position");
    watch_outputs(
        core,
        limits,
        Event::EndedReadLoaded { request, task: CHAT, party: CHAT_PARTY, through: 0, busy: false },
    )
}

fn repeated_chat_mark() -> (Core, Limits, tasks::TaskRecord, Token) {
    let (mut core, limits, records) = finished_chat();
    let seen = people::Seen::Conversation { task: CHAT, through: FINAL_REPLY };
    let _marked = mark_chat_seen(&mut core, &limits, &records, seen);
    let outputs = start_chat_mark(&mut core, &limits, seen, 1);
    let mut request = None;
    for output in &outputs {
        if let Request::Held(held) = output
            && let Held::Load(load) = held.as_ref()
            && let Now::SeenConversation { request: wanted, .. } = load.as_ref()
        {
            request = Some(*wanted);
            break;
        }
    }
    let request = request.expect("the completed key reprocesses through a fresh header load");
    (core, limits, chat_owner(&records).clone(), request)
}

fn assert_refused_chat_mark(
    core: &mut Core,
    limits: &Limits,
    row: tasks::TaskRecord,
    request: Token,
    held: Option<people::Role>,
) {
    let outputs =
        watch_outputs(core, limits, Event::SeenConversationLoaded { request, row: Some(Box::new(row)), busy: false });
    assert_mark_without_writes(
        &outputs,
        people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Role { held })),
    );
}

fn assert_mark_without_writes(outputs: &[Request], expected: people::Reply) {
    let mut wrote = false;
    let mut replies = List::with_capacity(1);
    for output in outputs {
        if let Request::Write(_) = output {
            wrote = true;
        }
        if let Request::Held(held) = output
            && let Held::PeopleReply { reply, .. } = held.as_ref()
        {
            replies.push(reply.clone()).unwrap();
        }
    }
    assert!(!wrote, "a refused repeat writes neither an answer nor a position");
    assert_eq!(replies.as_slice(), [expected]);
}

#[test]
fn a_repeated_seen_rechecks_role_after_its_header_load() {
    let (mut core, limits, row, request) = repeated_chat_mark();
    let _roles = watch_outputs(
        &mut core,
        &limits,
        Event::People(people::Event::Roles { project: row.project, holdings: Box::new([]) }),
    );
    assert_refused_chat_mark(&mut core, &limits, row, request, None);
}

#[test]
fn a_repeated_seen_rechecks_the_loaded_conversations_requester() {
    let (mut core, limits, mut row, request) = repeated_chat_mark();
    row.requester = tasks::Party::Person(2);
    assert_refused_chat_mark(&mut core, &limits, row, request, Some(people::Role::Owner));
}

#[test]
fn overlapping_seen_stays_busy_during_header_and_position_loads() {
    let (mut core, limits, row, request) = repeated_chat_mark();
    let seen = people::Seen::Conversation { task: CHAT, through: FINAL_REPLY };
    let outputs = start_chat_mark(&mut core, &limits, seen, 2);
    assert_mark_without_writes(&outputs, people::Reply::Again(people::Again::Busy));
    let outputs = watch_outputs(
        &mut core,
        &limits,
        Event::SeenConversationLoaded { request, row: Some(Box::new(row)), busy: false },
    );
    let mut loading = false;
    for output in &outputs {
        if let Request::Held(held) = output
            && let Held::Load(load) = held.as_ref()
            && let Now::LoadEndedRead { request: wanted, task: CHAT, party: CHAT_PARTY } = load.as_ref()
        {
            assert_eq!(*wanted, request);
            loading = true;
        }
    }
    assert!(loading);
    let outputs = start_chat_mark(&mut core, &limits, seen, 2);
    assert_mark_without_writes(&outputs, people::Reply::Again(people::Again::Busy));
    let outputs = watch_outputs(
        &mut core,
        &limits,
        Event::EndedReadLoaded { request, task: CHAT, party: CHAT_PARTY, through: 0, busy: true },
    );
    assert_mark_without_writes(&outputs, people::Reply::Again(people::Again::Busy));
}

#[test]
fn ending_a_kept_chat_preserves_cached_and_stored_replies() {
    let (mut core, limits, records) = finished_chat();
    assert!(records.contains(&Record::People(people::Stored::EndedRead { task: CHAT, party: CHAT_PARTY, through: 0 })));
    let cached = inbox(&core, &limits, limits.documents.rows);
    assert_eq!((cached.results_seen, cached.updates.len()), (0, CHAT_UPDATES));
    assert_chat_replies(cached.updates.as_slice(), &records);
    let result =
        chat_entry(cached.updates.as_slice(), InboxObject::Result { position: chat_owner(&records).result_position });
    assert_eq!((result.reply.as_ref(), result.activity, result.unread), (None, None, Some(true)));
    let reads::Answer::Page(page) =
        stored_page(&mut core, &limits, reads::Group::Replies, Some(CHAT), chat_rows(&records, 0, true))
    else {
        panic!("stored replies")
    };
    let mut entries = List::with_capacity(CHAT_REPLIES);
    for row in &page.rows {
        let reads::PageRow::Inbox(entry) = row else { panic!("Reply row") };
        entries.push(entry.clone()).unwrap();
    }
    assert_chat_replies(entries.as_slice(), &records);
}

#[test]
fn reading_results_leaves_ended_chat_turns_unread() {
    let (mut core, limits, records) = finished_chat();
    let through = chat_owner(&records).result_position;
    let _marked = mark_chat_seen(&mut core, &limits, &records, people::Seen::Results { through });
    let cached = inbox(&core, &limits, limits.documents.rows);
    assert_eq!((cached.results_seen, cached.updates.len()), (through, CHAT_REPLIES));
    assert_chat_replies(cached.updates.as_slice(), &records);
    let reads::Answer::Page(page) =
        stored_page(&mut core, &limits, reads::Group::Results, Some(CHAT), chat_rows(&records, 0, false))
    else {
        panic!("stored result selection")
    };
    assert!(page.rows.is_empty());
}

#[test]
fn reading_the_last_turn_preserves_kept_result_and_history() {
    let (mut core, limits, records) = finished_chat();
    let marked =
        mark_chat_seen(&mut core, &limits, &records, people::Seen::Conversation { task: CHAT, through: FINAL_REPLY });
    let mut saved = false;
    for output in &marked {
        if let Request::Write(Write::Save(Record::People(people::Stored::EndedRead {
            task: CHAT,
            party: CHAT_PARTY,
            through: FINAL_REPLY,
        }))) = output
        {
            saved = true;
            break;
        }
    }
    assert!(saved);
    let cached = inbox(&core, &limits, limits.documents.rows);
    assert_eq!((cached.results_seen, cached.updates.len()), (0, 1));
    assert_eq!(
        cached.updates.get(0).unwrap().object,
        InboxObject::Result { position: chat_owner(&records).result_position }
    );
    let reads::Answer::Page(page) =
        stored_page(&mut core, &limits, reads::Group::Replies, Some(CHAT), chat_rows(&records, FINAL_REPLY, true))
    else {
        panic!("stored reply boundary")
    };
    assert!(page.rows.is_empty());
    core.reclaim();
    let Held::ReadLoad { load, .. } = read_start(&mut core, &limits, CHAT_SIGN_IN, reads::Read::Task { task: CHAT })
    else {
        panic!("kept task load")
    };
    let row = stored_row(Record::Tasks(tasks::Stored::Ended(Box::new(chat_owner(&records).clone()))), None);
    let Some(reads::Answer::Historical(history)) =
        loaded_answer(&mut core, &limits, load, reads::Loaded::Page { rows: Box::new([row]), next: None })
    else {
        panic!("kept task history")
    };
    let reads::Past::Task { document, result } = &history.value else { panic!("whole kept task") };
    assert_eq!(
        (document.chip.number, document.flags.retention, result.task.number),
        (CHAT, tasks::Retention::Kept, CHAT)
    );
    let Ending::Done(ResultValue::Report(report)) = &result.ending else { panic!("kept final report") };
    assert_eq!(report.bytes.as_ref(), b"final report");
    core.reclaim();
    let Held::ReadLoad { load, .. } =
        read_start(&mut core, &limits, CHAT_SIGN_IN, reads::Read::Activity { task: CHAT, before: None })
    else {
        panic!("kept activity load")
    };
    let mut rows = List::with_capacity(CHAT_REPLIES);
    for record in records.iter().rev() {
        if let Record::Tasks(tasks::Stored::Activity(_)) = record {
            rows.push(stored_row(record.clone(), Some(chat_owner(&records).clone()))).unwrap();
        }
    }
    let Some(reads::Answer::Page(page)) =
        loaded_answer(&mut core, &limits, load, reads::Loaded::Page { rows: rows.into_boxed(), next: None })
    else {
        panic!("kept turn history")
    };
    assert_eq!(page.rows.len(), CHAT_REPLIES);
    for (row, expected) in page.rows.iter().zip([FINAL_REPLY, ORDINARY_REPLY]) {
        let reads::PageRow::Activity { line, .. } = row else { panic!("kept turn activity") };
        let tasks::Line::Turn { reply, .. } = line.line else { panic!("committed turn") };
        assert_eq!(reply, expected);
    }
}

#[test]
fn inbox_membership_matches_the_original_read_predicate_at_every_requester_boundary() {
    let (mut core, mut limits) = fixture();
    for task in [1, 2, 3, 4, 5] {
        waiting(&mut core, &limits, task, &[people::EntryKind::Question { message: task }]);
    }
    for depth in [0, 1, 2, 4] {
        limits.tasks.depth = depth;
        let supplied = inbox(&core, &limits, 16);
        for goal in [1, 2, 3, 4, 5, 99, u64::MAX] {
            let contexts = supplied.selection.as_ref().unwrap();
            let mut expected = List::with_capacity(16);
            for entry in &supplied.needs {
                if context_for(contexts, entry.cursor).goals.as_slice().contains(&goal) {
                    expected.push(entry.clone()).unwrap();
                }
            }
            let Held::Read { answer: reads::Answer::Page(page), .. } = read_start(
                &mut core,
                &limits,
                10,
                reads::Read::Inbox {
                    group: reads::Group::Questions,
                    filter: reads::Filter { project: Some(1), goal: Some(goal) },
                    from: None,
                },
            ) else {
                panic!("live Inbox read")
            };
            let mut actual = List::with_capacity(16);
            for row in &page.rows {
                let reads::PageRow::Inbox(entry) = row else { panic!("whole selected entry") };
                actual.push(entry.clone()).unwrap();
            }
            assert_eq!(actual, expected, "goal {goal}, depth {depth}");
            assert_eq!(page.inbox_selection.as_ref().unwrap().len(), page.rows.len());
        }
        let mut selected = None;
        for entry in &supplied.needs {
            let number = match &entry.task {
                TaskLink::Live(chip) | TaskLink::Retained { chip, .. } => chip.number,
                TaskLink::Stored { number, .. } => *number,
            };
            if number == 3 {
                selected = Some(entry.cursor);
            }
        }
        let context = context_for(supplied.selection.as_ref().unwrap(), selected.unwrap());
        let expected: &[u64] = match depth {
            0 => &[3],
            1 => &[3, 2],
            _ => &[3, 2, 1],
        };
        assert_eq!(context.goals.as_slice(), expected);
    }
}

#[test]
fn inbox_context_keeps_all_six_raw_objects_groups_counts_and_full_width_identities() {
    let (mut core, limits) = fixture();
    let kinds = [
        people::EntryKind::Question { message: u64::MAX },
        people::EntryKind::Proposal { number: u64::MAX },
        people::EntryKind::Escalation { revision: u64::MAX },
        people::EntryKind::PersonTask,
        people::EntryKind::Reply { message: u64::MAX },
    ];
    waiting(&mut core, &limits, 3, &kinds);
    core.people.remember_result(
        &limits.people,
        1,
        people::ResultRef {
            task: u64::MAX,
            project: 1,
            at: Wall::from_nanos(u64::MAX),
            position: u64::MAX,
            forgettable: None,
        },
    );
    let value = inbox(&core, &limits, 16);
    assert_eq!(
        (
            value.party,
            value.needs.len(),
            value.updates.len(),
            value.cached_needs,
            value.cached_updates,
            value.results_seen,
            value.updates_time_complete
        ),
        (1, 4, 2, 4, Count::Known(2), 0, true)
    );
    let contexts = value.selection.as_ref().unwrap();
    assert_eq!(contexts.len(), 6);
    for entry in value.needs.iter().chain(value.updates.iter()) {
        let context = context_for(contexts, entry.cursor);
        let expected: &[u64] = match entry.object {
            InboxObject::Result { position } => {
                assert_eq!(position, u64::MAX);
                &[u64::MAX]
            }
            InboxObject::Question { .. }
            | InboxObject::Proposal { .. }
            | InboxObject::Escalation { .. }
            | InboxObject::PersonTask
            | InboxObject::Reply { .. } => &[3, 2, 1],
        };
        assert_eq!(context.goals.as_slice(), expected);
        assert_eq!(entry.project, Some(1));
        let Cursor::Inbox { object, task, at } = entry.cursor else { panic!("original raw cursor") };
        assert_eq!(object, entry.object);
        assert_eq!(at, entry.at);
        assert_eq!(task, *context.goals.get(0).unwrap());
        assert_eq!(context.goals.capacity(), limits.tasks.depth + 1);
    }
    assert_eq!(value.updates.get(0).unwrap().at, Some(Wall::from_nanos(u64::MAX)));
    assert_eq!(value.updates.get(0).unwrap().task, TaskLink::Stored { number: u64::MAX, status: None });
}

#[test]
fn retained_inbox_context_keeps_original_ancestors_and_missing_clocks_without_current_joins() {
    let (mut core, limits) = fixture();
    for goal in [3, 1, u64::MAX, 2] {
        let row = ended_row(&core, 3, &[u64::MAX]);
        let reads::Answer::Page(page) =
            stored_page(&mut core, &limits, reads::Group::Results, Some(goal), Box::new([row]))
        else {
            panic!("whole retained page")
        };
        assert_eq!(page.rows.len(), u32::from(goal == 3 || goal == u64::MAX));
        if let Some(reads::PageRow::Inbox(entry)) = page.rows.get(0) {
            assert_eq!(entry.at, None);
            assert_eq!(entry.unread, Some(true));
            assert_eq!(entry.activity, None);
            let TaskLink::Retained { chip, at } = &entry.task else { panic!("retained owner link") };
            assert_eq!((chip.number, *at), (3, None));
            let context = page.inbox_selection.as_ref().unwrap().get(0).unwrap();
            assert_eq!(context.cursor, entry.cursor);
            assert_eq!(context.goals.as_slice(), &[3, u64::MAX]);
            assert!(!context.goals.as_slice().contains(&1));
        }
    }
}

#[test]
fn stored_waiting_and_reply_entries_keep_separate_original_contexts_and_activity_identity() {
    let (mut core, limits) = fixture();
    let mut owner = core.tasks.task(1).unwrap().clone();
    owner.inbox = Box::new([tasks::Word {
        number: u64::MAX,
        from: tasks::Party::Task(2),
        kind: tasks::MessageKind::Question,
        words: Box::new(*b"question"),
        at: Wall::EPOCH,
        hits: 0,
        eligible: true,
    }]);
    owner.question_lines = Box::new([tasks::QuestionLine { message: u64::MAX, line: owner.activity, answer: None }]);
    core.question_activity
        .insert(
            (owner.number, owner.activity),
            tasks::ActivityLine {
                task: owner.number,
                position: owner.activity,
                at: Wall::EPOCH,
                line: tasks::Line::Words {
                    message: u64::MAX,
                    by: tasks::Party::Task(2),
                    kind: tasks::MessageKind::Question,
                    words: Box::new(*b"question"),
                },
            },
        )
        .unwrap();
    let object = InboxObject::Question { message: u64::MAX };
    let mut row = stored_row(Record::Tasks(tasks::Stored::Live(Box::new(owner.clone()))), None);
    row.inbox = Some(Cursor::Inbox { task: 1, object, at: Some(Wall::EPOCH) });
    row.ancestors = Box::new([u64::MAX]);
    let reads::Answer::Page(needs) =
        stored_page(&mut core, &limits, reads::Group::Questions, Some(u64::MAX), Box::new([row]))
    else {
        panic!("waiting owner page")
    };
    assert_eq!(needs.rows.len(), 1);
    assert_eq!(needs.inbox_selection.as_ref().unwrap().get(0).unwrap().goals.as_slice(), &[1, u64::MAX]);
    let word = tasks::Word {
        number: u64::MAX - 1,
        from: tasks::Party::Task(2),
        kind: tasks::MessageKind::Words,
        words: Box::new(*b"reply"),
        at: Wall::from_nanos(9),
        hits: 0,
        eligible: true,
    };
    let line = tasks::ActivityLine {
        task: 1,
        position: u64::MAX,
        at: word.at,
        line: tasks::Line::Notified { recipient: tasks::Party::Person(1), word: word.clone() },
    };
    let object = UpdateObject::Reply { message: Newest::of(word.number), activity: Newest::of(line.position) };
    let update = Update { party: 1, at: word.at, task: 1, object };
    let mut row = stored_row(Record::Tasks(tasks::Stored::Update(update)), Some(owner));
    row.related = Some(Record::Tasks(tasks::Stored::Activity(line)));
    row.inbox = Some(Cursor::Inbox { task: 1, object: InboxObject::Reply { message: word.number }, at: Some(word.at) });
    row.ancestors = Box::new([99]);
    let reads::Answer::Page(updates) =
        stored_page(&mut core, &limits, reads::Group::Replies, Some(99), Box::new([row]))
    else {
        panic!("reply owner page")
    };
    let reads::PageRow::Inbox(entry) = updates.rows.get(0).unwrap() else { panic!("reply entry") };
    assert_eq!(
        (entry.object, entry.activity, entry.at),
        (InboxObject::Reply { message: u64::MAX - 1 }, Some(u64::MAX), Some(word.at))
    );
    assert_eq!(updates.inbox_selection.as_ref().unwrap().get(0).unwrap().goals.as_slice(), &[1, 99]);
}

#[test]
fn a_party_proposals_stored_goal_context_uses_only_its_supplied_original_goal() {
    let (mut core, limits) = fixture();
    let proposal = tasks::PersonProposal {
        number: u64::MAX,
        proposer: 1,
        project: 1,
        action: tasks::PersonAction::Batch(Box::new([new_row(
            &core,
            u64::MAX - 1,
            tasks::Funder::Pool { project: 1, person: 1, period: 1 },
            Some(4),
        )])),
        reason: Box::from(b"proposed batch".as_slice()),
        state: tasks::PersonProposalState::Pending { since: Wall::from_nanos(7) },
    };
    assert!(!core.person_proposal_entries(&proposal).is_empty());
    for goal in [None, Some(proposal.first()), Some(99)] {
        let mut row = stored_row(Record::Tasks(tasks::Stored::PersonProposal(Box::new(proposal.clone()))), None);
        row.ancestors = Box::new([99]);
        row.inbox = Some(Cursor::Inbox {
            task: proposal.first(),
            object: InboxObject::Proposal { proposer: Proposer::Party(1), number: proposal.number },
            at: Some(Wall::from_nanos(7)),
        });
        let reads::Answer::Page(page) = stored_page(&mut core, &limits, reads::Group::Proposals, goal, Box::new([row]))
        else {
            panic!("party-origin proposal page")
        };
        assert_eq!(page.rows.len(), u32::from(goal != Some(99)));
        if let Some(reads::PageRow::Inbox(entry)) = page.rows.get(0) {
            assert_eq!(entry.task, TaskLink::Stored { number: proposal.first(), status: None });
            assert_eq!(entry.object, InboxObject::Proposal { proposer: Proposer::Party(1), number: u64::MAX });
            assert_eq!(page.inbox_selection.as_ref().unwrap().get(0).unwrap().goals.as_slice(), &[proposal.first()]);
        }
    }
}

#[test]
fn whole_inbox_context_distinguishes_unsupplied_from_empty_and_refuses_depth_overflow() {
    let (mut core, mut limits) = fixture();
    let value = inbox(&core, &limits, 16);
    assert!(value.selection.as_ref().unwrap().is_empty());
    assert_eq!(value.selection.as_ref().unwrap().capacity(), 16);
    let mut absent = value.clone();
    absent.selection = None;
    assert_ne!(absent, value);
    let reads::Answer::Page(page) = stored_page(&mut core, &limits, reads::Group::Needs, None, Box::new([])) else {
        panic!("complete empty page")
    };
    assert!(page.inbox_selection.as_ref().unwrap().is_empty());
    limits.tasks.depth = u32::MAX;
    assert_eq!(inbox_selection_worst_case(16, limits.tasks.depth), None);
    assert_eq!(document_with_context_worst_case(&limits), None);
    assert_eq!(
        make(&core, &limits, 1, &Subject::Inbox { party: 1 }, &Window { rows: 16, after: None }, Wall::EPOCH),
        Err(Missing::TooLarge)
    );
}

#[test]
fn original_stored_ancestry_fits_at_depth_and_one_under_refuses_the_whole_page() {
    let (mut core, mut limits) = fixture();
    limits.tasks.depth = 2;
    let row = ended_row(&core, 100, &[99, u64::MAX]);
    let second = ended_row(&core, 100, &[99, u64::MAX]);
    let reads::Answer::Page(page) = stored_page(&mut core, &limits, reads::Group::Results, Some(99), Box::new([row]))
    else {
        panic!("whole depth boundary")
    };
    assert_eq!(page.inbox_selection.as_ref().unwrap().get(0).unwrap().goals.as_slice(), &[100, 99, u64::MAX]);
    assert_eq!(page.inbox_selection.as_ref().unwrap().get(0).unwrap().goals.capacity(), 3);
    limits.tasks.depth = 1;
    assert_eq!(
        stored_page(&mut core, &limits, reads::Group::Results, Some(99), Box::new([second])),
        reads::Answer::None(reads::Why::Busy)
    );
    assert_eq!(core.tasks.task(1).unwrap().requester, tasks::Party::Person(1));
}

#[test]
fn inbox_context_memory_prices_every_reserved_container_and_complete_document_envelope() {
    // The template envelopes now carry each optional account preference:
    // 16 document lines × 16 templates × 8 bytes adds 2,048 to 987,136.
    const TEMPLATE_ENVELOPES: u64 = 989_184;
    // A settings inventory document is priced beside every document (engine.md 17.7).
    // Four pending inventory heads add 96 bytes; the two value sets and
    // bounded review/item-head carriers add 115,248 bytes at this fixture.
    // Four older held revision heads, each with its holder count, add 288 bytes.
    const SETTINGS_DOCUMENT: u64 = 120_496 + 4 * 72;

    let (mut core, limits) = fixture();
    waiting(
        &mut core,
        &limits,
        3,
        &[people::EntryKind::Question { message: 1 }, people::EntryKind::Question { message: 2 }],
    );
    let value = inbox(&core, &limits, 16);
    let contexts = value.selection.as_ref().unwrap();
    let reserved = List::<InboxSelection>::worst_case(contexts.capacity()).unwrap();
    let roots = List::<u64>::worst_case(limits.tasks.depth + 1).unwrap();
    let bound = inbox_selection_worst_case(16, limits.tasks.depth).unwrap();
    assert_eq!(bound, reserved + 16 * roots);
    let mut held = reserved;
    for context in contexts {
        assert_eq!(context.owned_bytes(), Some(roots));
        held += context.owned_bytes().unwrap();
    }
    assert!(held <= bound);
    assert!(roots > u64::try_from(size_of_val(contexts.get(0).unwrap().goals.as_slice())).unwrap());
    assert_eq!(
        document_with_context_worst_case(&limits),
        Some(document_worst_case(&limits.documents).unwrap() + bound + TEMPLATE_ENVELOPES + SETTINGS_DOCUMENT)
    );
    assert!(reads::answer_worst_case(&limits).unwrap() >= document_with_context_worst_case(&limits).unwrap());
    assert!(worst_case(&limits).is_some());
}
