//! Restoration precedes deployment allocation and transient admission
//! (`domain/engine.md`, 5 and 6; `client/domain/contract.md`, 3.1 and 4).
const PARTY: u64 = 1;
const OTHER_TASK: u64 = 3;
use super::*;

const UTF8_IDENTITY_WORDS: (&[u8], &[u8]) = (b"~\0\x01", b"\0^!");

fn env() -> Env<Limits> {
    Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits() }
}

fn identity() -> people::Identity {
    people::Identity {
        key: people::IdentityKey { provider: 0, subject: Box::new([0, 255, 128, 7]) },
        login: Box::from(UTF8_IDENTITY_WORDS.0),
        name: Box::from(UTF8_IDENTITY_WORDS.1),
    }
}

fn signin(core: &mut Core, env: &Env<Limits>, token: u64) {
    let Requests::Out(mut out) =
        step(core, env, Event::SignIn { held: None, reply_to: ReplyTo::new(Token::new(token)), identity: identity() });
    assert_eq!(out.len(), 2, "one answer and the completed decision, with no write");
    let Some(Request::Held(reply)) = out.pop() else { panic!("held sign-in answer") };
    let Held::PeopleReply { to, reply } = *reply else { panic!("original people reply") };
    assert_eq!(to.into_token(), Token::new(token));
    assert_eq!(reply, people::Reply::Again(people::Again::NotReady));
    let Some(Request::Decided) = out.pop() else { panic!("completed sign-in decision") };
    assert!(out.is_empty());
}

#[test]
fn an_empty_load_commits_the_original_bootstrap_only_after_loading_completes() {
    let env = env();
    let mut core = Core::new(config(47), &env.limits).expect("fixture core configuration");
    let mut expected = core.counters.deployment();
    assert!(core.counters.dirty(), "ordinary first start still needs its header");
    assert_eq!(core.restart_begin(), RestartRequest::Step(RestartStep::LoadCore));
    assert!(!core.counters.dirty(), "no header commit while its durable source is unknown");
    assert_eq!(core.restart_begin(), RestartRequest::Idle);
    assert_eq!(core.counters.deployment(), expected);
    for token in [7101, 7102] {
        signin(&mut core, &env, token);
        assert!(!core.counters.dirty());
        assert_eq!(core.counters.deployment(), expected);
    }
    assert_eq!(core.restart_done(RestartStep::LoadCore), RestartRequest::Step(RestartStep::LoadInbox));
    assert!(core.counters.dirty());
    expected.commits = 1;
    assert_eq!(core.counters.next_commit(), expected, "the first real header keeps the supplied deployment identity");
    assert!(!core.counters.dirty());
}

#[test]
fn a_restored_header_preserves_every_counter_and_refused_signins_allocate_nothing() {
    let env = env();
    let mut core = Core::new(config(48), &env.limits).expect("fixture core configuration");
    let mut expected = core.counters.deployment();
    expected.id = [239; 16];
    expected.tasks = 11;
    expected.people = 12;
    expected.sign_ins = 13;
    expected.messages = 14;
    expected.runs = 15;
    expected.calls = 16;
    expected.connector_rows = 17;
    expected.applications = 18;
    expected.commits = 19;
    assert_eq!(core.restart_begin(), RestartRequest::Step(RestartStep::LoadCore));
    assert_eq!(core.restore_core(CoreRecord::Deployment(expected), &env.limits), Restored::Deployment { commits: 19 });
    for token in [7103, 7104] {
        signin(&mut core, &env, token);
        assert_eq!(core.counters.deployment(), expected);
        assert!(!core.counters.dirty());
    }
    assert_eq!(core.restart_done(RestartStep::LoadCore), RestartRequest::Step(RestartStep::LoadInbox));
    signin(&mut core, &env, 7105);
    assert_eq!(core.counters.deployment(), expected);
    assert!(!core.counters.dirty(), "restoring an existing deployment never bootstrap-writes it");
    assert_eq!(fresh(&mut core.counters, Family::Task), Some(12));
    expected.tasks = 12;
    expected.commits = 20;
    assert_eq!(core.counters.next_commit(), expected, "the first new allocation advances the actual durable header");
}

#[test]
fn an_unknown_header_cannot_allocate_any_family_or_become_dirty_on_time() {
    let mut env = env();
    let mut core = Core::new(config(49), &env.limits).expect("fixture core configuration");
    let expected = core.counters.deployment();
    assert_eq!(core.restart_begin(), RestartRequest::Step(RestartStep::LoadCore));
    for family in [
        Family::Project,
        Family::Task,
        Family::Person,
        Family::SignIn,
        Family::Message,
        Family::Run,
        Family::Call,
        Family::ConnectorRow,
        Family::Application,
    ] {
        assert_eq!(fresh(&mut core.counters, family), None);
        assert_eq!(core.counters.deployment(), expected);
        assert!(!core.counters.dirty());
    }
    env.now = Time::ZERO.saturating_add(Duration::from_secs(75));
    env.wall = Wall::from_nanos(Duration::from_secs(65).as_nanos());
    let Requests::Out(mut out) = fire(&mut core, &env, Timer::People);
    let Some(Request::Decided) = out.pop() else { panic!("completed timer decision") };
    assert!(out.is_empty());
    assert_eq!(core.counters.deployment(), expected);
    assert!(!core.counters.dirty());
}

fn core_keys() -> [CoreKey; 7] {
    [
        CoreKey::Projection(ProjectionKey::Goal(0)),
        CoreKey::RunProof(0),
        CoreKey::Call(CallKey { task: 0, attempt: 0, completion: 0, position: 0 }),
        CoreKey::Turn { task: 0, attempt: 0, turn: 0 },
        CoreKey::Terminal { task: 0, attempt: 0 },
        CoreKey::ProposalDecision { proposer: Proposer::Task(0), proposal: 0 },
        CoreKey::EscalationDecision { task: 0, revision: 0 },
    ]
}

#[test]
fn the_deployment_key_sorts_before_every_other_core_key() {
    for key in core_keys() {
        assert!(CoreKey::Deployment < key);
    }
}

#[test]
fn restore_ranges_admit_no_turn_terminal_history_or_note() {
    let mut history = List::with_capacity(9);
    for key in core_keys().into_iter().skip(3) {
        history.push(Key::Core(key)).expect("history fixture capacity");
    }
    for key in [
        Key::Tasks(tasks::Key::Ended(0)),
        Key::Tasks(tasks::Key::History { task: 0, revision: 0 }),
        Key::Tasks(tasks::Key::Milestone { task: 0, position: 0 }),
        Key::Tasks(tasks::Key::Activity { task: 0, position: 0 }),
        Key::Notes(notes::Key::Entry { name: 0 }),
    ] {
        history.push(key).expect("history fixture capacity");
    }
    for key in &history {
        for range in RestoreRange::LIVE {
            assert!(!range.admits(key), "{range:?} excludes {key:?}");
        }
    }
}

#[test]
fn restore_ranges_admit_each_live_family_once() {
    let mut live = List::with_capacity(17);
    for key in core_keys().into_iter().take(3) {
        live.push(Key::Core(key)).expect("live fixture capacity");
    }
    for key in [
        Key::Core(CoreKey::Deployment),
        Key::Tasks(tasks::Key::Live(0)),
        Key::Tasks(tasks::Key::Stub(0)),
        Key::Tasks(tasks::Key::Ledger(tasks::Funder::Task(0))),
        Key::Tasks(tasks::Key::Writer(0)),
        Key::Tasks(tasks::Key::Pool(0)),
        Key::Tasks(tasks::Key::PersonProposal(0)),
        Key::People(people::Key::Person(0)),
        Key::People(people::Key::ReadResults { party: 0 }),
        Key::People(people::Key::ReadConversation { party: 0, task: 0 }),
        Key::People(people::Key::SignIn(0)),
        Key::People(people::Key::Roles(0)),
        Key::People(people::Key::Policy(0)),
        Key::People(people::Key::Answer(people::RequestKey { person: 0, key: [0; 16] })),
    ] {
        live.push(key).expect("live fixture capacity");
    }
    for key in &live {
        let mut count = 0;
        for range in RestoreRange::LIVE {
            count += u32::from(range.admits(key));
        }
        assert_eq!(count, 1, "{key:?}");
    }
}

fn record_key_rows() -> [(CoreRecord, CoreKey); 8] {
    [
        (
            CoreRecord::Deployment(
                Core::new(config(50), &limits()).expect("fixture core configuration").counters.deployment(),
            ),
            CoreKey::Deployment,
        ),
        (
            CoreRecord::Projection(ProjectionRecord::Goal {
                goal: ProjectionGoal {
                    number: 7,
                    project: 1,
                    priority: 1,
                    words: Box::new([]),
                    phase: tasks::Phase::Waiting,
                },
                pending: Box::new([]),
                changed: false,
                fed_at: None,
                closing_at: None,
            }),
            CoreKey::Projection(ProjectionKey::Goal(7)),
        ),
        (
            CoreRecord::Call(CallRecord {
                key: CallKey { task: 11, attempt: 13, completion: 17, position: 19 },
                part: CallPart::Controlled,
                settled: None,
            }),
            CoreKey::Call(CallKey { task: 11, attempt: 13, completion: 17, position: 19 }),
        ),
        (
            CoreRecord::RunProof(RunProof::claimed(23, 29, 29, None, jig_domain_fleet::HostKind::Worker)),
            CoreKey::RunProof(23),
        ),
        (
            CoreRecord::Turn(TurnRecord {
                opens_window: false,
                task: 31,
                attempt: 37,
                turn: 41,
                spent: 0,
                read: None,
                at: Wall::EPOCH,
                transcript: Box::new([]),
            }),
            CoreKey::Turn { task: 31, attempt: 37, turn: 41 },
        ),
        (
            CoreRecord::Terminal(TerminalRecord { task: 43, attempt: 47, cumulative: 0, end: tasks::End::Parked }),
            CoreKey::Terminal { task: 43, attempt: 47 },
        ),
        (
            CoreRecord::ProposalDecision(ProposalDecisionRecord {
                project: 1,
                proposer: Proposer::Task(53),
                proposal: 59,
                kind: tasks::ProposalKind::Batch,
                by: 1,
                at: Wall::EPOCH,
                choice: people::ProposalChoice::Accepted,
                created: None,
            }),
            CoreKey::ProposalDecision { proposer: Proposer::Task(53), proposal: 59 },
        ),
        (
            CoreRecord::EscalationDecision(EscalationDecisionRecord {
                project: 1,
                requester: 1,
                task: 61,
                revision: 67,
                by: 1,
                at: Wall::EPOCH,
                decision: people::EscalationDecision::Release,
            }),
            CoreKey::EscalationDecision { task: 61, revision: 67 },
        ),
    ]
}

#[test]
fn a_records_key_is_the_key_its_row_is_saved_under() {
    for (row, key) in record_key_rows() {
        assert_eq!(row.key(), key, "the row owns its address");
        assert_eq!(Record::Core(row).key(), Key::Core(key), "the child tag wraps the same address");
    }
}

fn census_fixture() -> (Core, Env<Limits>) {
    let mut env = env();
    env.limits.people.inbox_entries = 8;
    let mut configuration = config(79);
    configuration.owners = Box::new([]);
    let mut core = Core::new(configuration, &env.limits).expect("fixture core configuration");
    assert_eq!(core.restart_begin(), RestartRequest::Step(RestartStep::LoadCore));
    let mut header = core.counters.deployment();
    header.tasks = 20;
    header.people = 7;
    header.messages = 5;
    header.runs = 1;
    core.counters = Counters::new(header);
    restore_census_people(&mut core, &env);
    assert_eq!(core.restart_done(RestartStep::LoadCore), RestartRequest::Step(RestartStep::LoadInbox));
    (core, env)
}
#[test]
fn the_census_lists_each_party_after_its_read_position() {
    let (mut core, env) = census_fixture();
    assert_eq!(
        core.census(&env.limits).results.as_ref(),
        [ReadResults { party: 1, after: 3 }, ReadResults { party: 7, after: 0 }]
    );
}
fn ended_census_chat() -> tasks::Stored {
    let (live, _) = watch_core();
    let mut row = live.tasks.task(1).expect("fixture chat").clone();
    row.number = 2;
    row.root = 2;
    row.depth = 0;
    row.result_position = 5;
    row.last_reply = 1;
    row.activity = 1;
    row.attempt = 1;
    row.turn = 1;
    row.replies =
        Box::new([tasks::ReplyHint { number: 1, activity: 1, attempt: 1, turn: 1, at: Wall::from_nanos(50) }]);
    row.ended_at = Some(Wall::from_nanos(100));
    row.phase = tasks::Phase::Ended(tasks::Ending::Cancelled {
        by: tasks::Party::Person(1),
        at: Wall::from_nanos(100),
        reason: Box::new([1]),
        result: None,
    });
    tasks::Stored::Ended(Box::new(row))
}
#[test]
fn an_ended_chat_handed_twice_to_the_census_changes_nothing() {
    let (mut core, env) = census_fixture();
    let _census = core.census(&env.limits);
    let chat = ended_census_chat();
    assert!(core.restore_inbox_row(&env.limits, &Record::Tasks(chat.clone())));
    let before = core.people.counts(&env.limits.people, 1).expect("party counts");
    assert!(core.restore_inbox_row(&env.limits, &Record::Tasks(chat.clone())));
    assert_eq!(core.people.counts(&env.limits.people, 1), Some(before));
    assert_eq!(core.people.cached_results(1).expect("party results").len(), 1);
    let _next = core.restart_done(RestartStep::LoadInbox);
    assert!(core.census_progress.is_empty());
}

fn restore_census_people(core: &mut Core, env: &Env<Limits>) {
    let people_env = Env { now: env.now, wall: env.wall, limits: env.limits.people };
    let mut out = Queue::with_capacity(people::max_out(&env.limits.people));
    for party in [1, 7] {
        let mut who = identity();
        who.key.subject = Box::new([u8::try_from(party).expect("fixture party")]);
        people::step(
            &mut core.people,
            &people_env,
            people::Event::Restore { record: people::Stored::Person { number: party, identity: who } },
            &mut out,
        );
    }
    for record in [
        people::Stored::ReadResults { party: 1, through: 3 },
        people::Stored::Roles {
            project: 1,
            holdings: Box::new([people::Holding { person: 1, role: people::Role::Owner }]),
        },
    ] {
        people::step(&mut core.people, &people_env, people::Event::Restore { record }, &mut out);
    }
    people::step(&mut core.people, &people_env, people::Event::Restored, &mut out);
    assert!(out.is_empty());
}

#[test]
fn a_census_position_followed_by_another_task_is_refused() {
    let (mut core, env) = census_fixture();
    let _census = core.census(&env.limits);
    assert!(core.restore_inbox_row(
        &env.limits,
        &Record::People(people::Stored::EndedRead { task: OTHER_TASK, party: PARTY, through: 0 })
    ));
    assert!(!core.restore_inbox_row(&env.limits, &Record::Tasks(ended_census_chat())));
}

#[test]
fn the_census_restores_unread_ended_replies_beside_the_result() {
    const CHAT: u64 = 2; // The existing ended_census_chat owner.
    const SIGN_IN: u64 = 10;
    const ORDINARY_REPLY: u64 = 1;
    const FINAL_REPLY: u64 = 2;
    const REPLIES: u32 = 2; // The ordinary turn and the final kept turn.
    const UPDATES: u32 = REPLIES + 1; // Plus the separately positioned result.
    let (mut core, env) = census_fixture();
    let tasks::Stored::Ended(mut owner) = ended_census_chat() else { panic!("ended census fixture") };
    // Durable records for a kept conversation: RunBegan at 1, these two
    // turns at 2/3, then RunEnded at 4 and the settled owner at time 100.
    owner.flags =
        tasks::Flags { conversation: true, listed: false, retention: tasks::Retention::Kept, ..tasks::Flags::PLAIN };
    owner.phase =
        tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Report { words: Box::new(*b"final report") }));
    owner.last_reply = FINAL_REPLY;
    owner.turn = 2;
    owner.activity = 4;
    owner.ever_turned = true;
    owner.last_answer = Some(owner.attempt);
    owner.last_activity = Wall::from_nanos(75);
    owner.changed = Wall::from_nanos(100);
    let turns = [
        tasks::ActivityLine {
            task: CHAT,
            position: 3,
            at: Wall::from_nanos(75),
            line: tasks::Line::Turn { reply: FINAL_REPLY, attempt: 1, turn: 2, read: None },
        },
        tasks::ActivityLine {
            task: CHAT,
            position: 2,
            at: Wall::from_nanos(50),
            line: tasks::Line::Turn { reply: ORDINARY_REPLY, attempt: 1, turn: 1, read: None },
        },
    ];
    owner.replies = Box::new([
        tasks::ReplyHint { number: FINAL_REPLY, activity: 3, attempt: 1, turn: 2, at: Wall::from_nanos(75) },
        tasks::ReplyHint { number: ORDINARY_REPLY, activity: 2, attempt: 1, turn: 1, at: Wall::from_nanos(50) },
    ]);
    let _census = core.census(&env.limits);
    assert!(core.restore_inbox_row(
        &env.limits,
        &Record::People(people::Stored::EndedRead { task: CHAT, party: PARTY, through: 0 })
    ));
    assert!(core.restore_inbox_row(&env.limits, &Record::Tasks(tasks::Stored::Ended(owner.clone()))));
    // The result-range pass point-loads the same owner; it must not replace
    // the conversation projection reconstructed by the position/owner pair.
    assert!(core.restore_inbox_row(&env.limits, &Record::Tasks(tasks::Stored::Ended(owner.clone()))));
    let _next = core.restart_done(RestartStep::LoadInbox);
    let Document::Inbox(cached) = make(
        &core,
        &env.limits,
        PARTY,
        &Subject::Inbox { party: PARTY },
        &Window { rows: UPDATES, after: None },
        env.wall,
    )
    .unwrap() else {
        panic!("restored inbox")
    };
    assert_eq!((cached.results_seen, cached.updates.len()), (3, UPDATES));
    let result = cached.updates.get(0).unwrap();
    assert_eq!(
        (result.object, result.activity, result.reply.as_ref()),
        (InboxObject::Result { position: 5 }, None, None)
    );

    // Authenticate the existing restored party through the people interface.
    let mut out = Queue::with_capacity(people::max_out(&env.limits.people));
    let identity = core.people.identity(PARTY).unwrap().clone();
    people::step(
        &mut core.people,
        &Env { now: env.now, wall: env.wall, limits: env.limits.people },
        people::Event::SignedIn {
            held: None,
            reply_to: ReplyTo::new(Token::new(SIGN_IN)),
            person: PARTY,
            sign_in: SIGN_IN,
            identity,
            kind: people::Kind::Person,
        },
        &mut out,
    );
    let mut rows = List::with_capacity(REPLIES);
    for line in &turns {
        let tasks::Line::Turn { reply, .. } = line.line else { panic!("census turn fixture") };
        let object = InboxObject::Reply { message: reply };
        let update = tasks::Update {
            party: PARTY,
            at: line.at,
            task: CHAT,
            object: tasks::UpdateObject::Reply {
                message: tasks::Newest::of(reply),
                activity: tasks::Newest::of(line.position),
            },
        };
        let mut row = stored_row(Record::Tasks(tasks::Stored::Update(update)), Some(*owner.clone()));
        row.related = Some(Record::Tasks(tasks::Stored::Activity(line.clone())));
        row.ended_read = Some(0);
        row.inbox = Some(Cursor::Inbox { task: CHAT, object, at: Some(line.at) });
        rows.push(row).unwrap();
    }
    let filter = reads::Filter { project: Some(1), goal: Some(CHAT) };
    let group = reads::Group::Replies;
    let Held::ReadLoad { load, .. } = read_start(
        &mut core,
        &env.limits,
        SIGN_IN,
        reads::Read::Inbox {
            group,
            filter,
            from: Some(reads::Cursor::Store { scope: reads::Scope::Inbox { group, filter }, after: None }),
        },
    ) else {
        panic!("restored stored reply selection")
    };
    let Some(reads::Answer::Page(page)) =
        loaded_answer(&mut core, &env.limits, load, reads::Loaded::Page { rows: rows.into_boxed(), next: None })
    else {
        panic!("restored stored replies")
    };
    assert_eq!(page.rows.len(), REPLIES);
    for (stored, line) in page.rows.iter().zip(&turns) {
        let reads::PageRow::Inbox(entry) = stored else { panic!("stored Reply") };
        let tasks::Line::Turn { reply, .. } = line.line else { panic!("expected turn") };
        let mut cached_reply = None;
        for entry in &cached.updates {
            if entry.object == (InboxObject::Reply { message: reply }) {
                cached_reply = Some(entry);
                break;
            }
        }
        let cached = cached_reply.expect("cached Reply after census join");
        for supplied in [entry, cached] {
            assert_eq!(
                (supplied.object, supplied.activity, supplied.at, supplied.unread),
                (InboxObject::Reply { message: reply }, Some(line.position), Some(line.at), Some(true))
            );
            assert_eq!(
                supplied.cursor,
                Cursor::Inbox { task: CHAT, object: InboxObject::Reply { message: reply }, at: Some(line.at) }
            );
            let TaskLink::Retained { chip, .. } = &supplied.task else { panic!("retained ended owner") };
            assert_eq!((chip.number, &chip.phase), (CHAT, &PhaseLine::Ended));
            let conversation = supplied.reply.as_ref().expect("ended conversation line");
            assert_eq!(
                (conversation.chip.number, &conversation.chip.phase, conversation.flags.retention),
                (CHAT, &PhaseLine::Ended, tasks::Retention::Kept)
            );
            assert_eq!((conversation.project, conversation.requester), (1, tasks::Party::Person(PARTY)));
            assert_eq!(conversation.conversation, Some(ConversationRead { seen: 0, unread: Count::Known(REPLIES) }));
        }
    }
}

#[test]
fn a_restored_projects_mark_precedes_the_next_project_allocation() {
    let limits = env().limits;
    let mut core = Core::new(config(50), &limits).expect("fixture core");
    let mut header = core.counters.deployment();
    header.projects = 5;
    core.restart_begin();
    assert_eq!(core.restore_core(CoreRecord::Deployment(header), &limits), Restored::Deployment { commits: 0 });
    assert_eq!(fresh(&mut core.counters, Family::Project), Some(6));
}

// A two-slot run inbox exercises both the exact bound and one past it.
const OFFER_SLOTS: u32 = 2;
const PROOF_ATTEMPT: u64 = 7;

fn restore_offers(ids: &[u64], offered: Option<u64>, read: Option<u64>, attempt: u64) -> bool {
    let mut limits = limits();
    limits.tasks.run_inbox = OFFER_SLOTS;
    let mut core = Core::new(config(463), &limits).expect("fixture core");
    core.restoring_proofs
        .insert(1, RestoringProof { attempt: PROOF_ATTEMPT, turn: 1, run_spent: 0, last_answer: None })
        .unwrap();
    let mut proof = RunProof::claimed(1, attempt, PROOF_ATTEMPT, offered, fleet::HostKind::Worker);
    proof.accounts = Box::new([jig_domain_authority::AccountBinding { provider: 1, account: 1, source: 1 }]);
    proof.placement = Some(Token::new(7));
    proof.turn = Some(TurnProof { turn: 1, cumulative: 0, read });
    proof.outstanding = ids.into();
    core.restore_proof(proof, limits.tasks.result_bytes)
}

#[test]
fn valid_unconsumed_offers_and_an_empty_consumed_set_restore() {
    for (ids, offered, read) in [(&[11, 13][..], Some(13), Some(10)), (&[][..], Some(13), Some(13))] {
        assert!(restore_offers(ids, offered, read, PROOF_ATTEMPT));
    }
}

#[test]
fn malformed_oversized_consumed_and_wrong_attempt_offers_do_not_restore() {
    for (ids, offered, read, attempt) in [
        (&[0][..], Some(13), None, PROOF_ATTEMPT),
        (&[11, 11][..], Some(13), Some(10), PROOF_ATTEMPT),
        (&[14][..], Some(13), Some(10), PROOF_ATTEMPT),
        (&[10][..], Some(13), Some(10), PROOF_ATTEMPT),
        (&[9][..], Some(13), Some(10), PROOF_ATTEMPT),
        (&[11][..], None, None, PROOF_ATTEMPT),
        (&[11][..], Some(13), Some(10), PROOF_ATTEMPT + 1),
        (&[11, 12, 13][..], Some(13), Some(10), PROOF_ATTEMPT),
        (&[][..], Some(0), None, PROOF_ATTEMPT),
        (&[][..], Some(13), Some(0), PROOF_ATTEMPT),
    ] {
        assert!(!restore_offers(ids, offered, read, attempt));
    }
}
