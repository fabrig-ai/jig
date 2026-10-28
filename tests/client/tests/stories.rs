#[path = "stories/closing.rs"]
mod closing;
#[path = "stories/goals.rs"]
mod goals;
use jig_client_world::{Until, stories, test};
use skein_fake_person::{Find, Step, Target};
use skein_lib::Duration;
use skein_ui::Role;
const WITHIN: Duration = Duration::from_secs(1);
#[test]
fn signing_in_and_starting_a_chat() {
    let mut w = stories::chat_world(1);
    w.front()
        .engine
        .act(
            skein_lib::Time::ZERO,
            skein_lib::Wall::EPOCH,
            jig_client_scripted_engine::Change::SignedOut { sign_in: 7 },
        )
        .expect("sign out");
    let c = w.client(7, stories::conversation());
    w.person(
        c,
        vec![
            Step::See { find: Find::named(Role::Button, b"Sign in with Test"), within: WITHIN },
            Step::Press { find: Find::named(Role::Button, b"Sign in with Test") },
            Step::See { find: Find::named(Role::TextArea, b"First words"), within: WITHIN },
            Step::Type { find: Find::named(Role::TextArea, b"First words"), words: Box::from(*b"Hello") },
            Step::Press { find: Find::named(Role::Button, b"Start a conversation") },
            Step::See { find: Find::text(b"Chat nine"), within: WITHIN },
        ],
    );
    let outcome = w.run(Until::Done);
    assert!(outcome.callbacks > 4);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert!(outcome.judged.0 > 0);
    assert_eq!(w.front().engine.commits().iter().filter(|commit| commit.key.is_some()).count(), 1);
}
#[test]
fn a_draft_is_kept_across_a_reload() {
    let mut w = stories::chat_ready(2);
    let c = w.client(7, stories::chat());
    w.person(
        c,
        vec![
            Step::See { find: Find::named(Role::Button, b"Write"), within: WITHIN },
            Step::Press { find: Find::named(Role::Button, b"Write") },
            Step::Type { find: Find::named(Role::TextArea, b"Your answer"), words: Box::from(*b"Unsent draft") },
            Step::Pause { span: Duration::from_millis(30) },
            Step::Reload,
            Step::See { find: Find::named(Role::Button, b"Write"), within: WITHIN },
            Step::Press { find: Find::named(Role::Button, b"Write") },
        ],
    );
    w.run(Until::Done);
    assert!(w.kept(c).iter().any(|r| matches!(r,jig_client_test_domain::store::Record::Jig(jig_client_view::store::Record::Draft { words,.. }) if words.bytes()==b"Unsent draft")));
    assert!(w.tree(c).nodes().iter().any(|n| n.value.as_ref().is_some_and(|v| v.text.as_ref() == b"Unsent draft")));
}

#[test]
fn a_held_chat_is_released() {
    for seed in [11, 12] {
        let mut w = stories::held_chat(seed);
        let c = w.client(7, stories::chat());
        w.person(
            c,
            vec![
                Step::See { find: Find::named(Role::Button, b"Release"), within: WITHIN },
                Step::Press { find: Find::named(Role::Button, b"Release") },
                Step::See { find: Find::text(b"Being decided"), within: WITHIN },
                Step::Press { find: Find::named(Role::Button, b"Confirm") },
                Step::Press { find: Find::named(Role::Tab, b"Overview") },
                Step::See { find: Find::text(b"Starting"), within: WITHIN },
            ],
        );
        w.run(Until::Done);
        assert_eq!(w.front().engine.commits().iter().filter(|c| c.key.is_some()).count(), 1);
    }
}

const CHECK_PARTY: u64 = 7;
const HELD_TASK: u64 = 9;
const RELEASE_ATTEMPT: skein_lib::Token = skein_lib::Token::new(9000);
const RELEASE_KEY: [u8; 16] = [91; 16];
const DELIVERED_HOLD: jig_client::contract::Revision = jig_client::contract::Revision { value: 4 };
const UNDELIVERED_HOLD: jig_client::contract::Revision = jig_client::contract::Revision { value: 99 };
fn raw_release(revision: jig_client::contract::Revision) {
    let mut world = stories::held_chat(11);
    let client = world.client(CHECK_PARTY, stories::chat());
    world.person(client, vec![Step::See { find: Find::named(Role::Button, b"Release"), within: WITHIN }]);
    world.run(Until::Done);
    world.send_raw(
        client,
        jig_client_test_domain::Request::Client {
            request: jig_client::Request::Down {
                down: jig_client::Down::Send {
                    attempt: RELEASE_ATTEMPT,
                    key: RELEASE_KEY,
                    request: jig_client::Ask::Jig(Box::new(jig_client::contract::Ask::Release {
                        project: 1,
                        task: HELD_TASK,
                        revision,
                        again: None,
                    })),
                    value: None,
                },
            },
        },
    );
    assert_eq!(world.run(Until::Done).verdict, skein_world::domain::Verdict::Passed);
}
#[test]
#[should_panic(expected = "jig's check: a decision names a revision never delivered")]
fn jig_s_check_fails_a_release_naming_a_revision_never_delivered() {
    raw_release(UNDELIVERED_HOLD);
}
#[test]
fn jig_s_check_passes_a_release_naming_the_delivered_revision() {
    raw_release(DELIVERED_HOLD);
}

#[test]
fn a_proposal_accepted_in_the_inbox_leaves_its_chat_too() {
    for seed in [13, 14] {
        let mut w = stories::proposal_world(seed);
        let inbox = w.client(7, stories::inbox());
        let chat = w.client(7, stories::chat());
        w.person(
            inbox,
            vec![
                Step::See { find: Find::named(Role::Button, b"Accept"), within: WITHIN },
                Step::Press { find: Find::named(Role::Button, b"Accept") },
                Step::Press { find: Find::named(Role::Button, b"Confirm") },
                Step::See { find: Find::text(b"Accepted"), within: WITHIN },
            ],
        );
        w.person(
            chat,
            vec![
                Step::See { find: Find::text(b"Resolve the incident"), within: WITHIN },
                Step::See { find: Find::text(b"Accepted"), within: WITHIN },
            ],
        );
        w.run(Until::Done);
        assert_eq!(w.front().engine.commits().iter().filter(|c| c.key.is_some()).count(), 1);
    }
}

#[test]
fn two_tabs_deciding_one_thing_are_told_who_decided_first() {
    for seed in [15, 16] {
        let mut w = stories::proposal_world(seed);
        let a = w.client(7, stories::inbox());
        let b = w.tab(a);
        for c in [a, b] {
            w.person(
                c,
                vec![
                    Step::See { find: Find::named(Role::Button, b"Accept"), within: WITHIN },
                    Step::Press { find: Find::named(Role::Button, b"Accept") },
                    Step::Press { find: Find::named(Role::Button, b"Confirm") },
                    Step::See { find: Find::text(b"Accepted"), within: WITHIN },
                ],
            );
        }
        w.run(Until::Done);
        assert_eq!(w.front().engine.commits().iter().filter(|c| c.key.is_some()).count(), 2);
        assert!([a, b].into_iter().any(|c| {
            w.tree(c)
                .nodes()
                .iter()
                .flat_map(|n| &n.text)
                .any(|r| r.text.windows(b"Decided first".len()).any(|part| part == b"Decided first"))
        }));
    }
}

fn start_with_fault(seed: u64, fault: Option<jig_client_world::FrontFault>, reload: bool) {
    let mut w = stories::chat_world(seed);
    let c = w.client(7, stories::conversation());
    w.person(c, stories::start_chat_steps());
    w.run(Until::Done);
    if let Some(fault) = fault {
        w.fault(fault);
    }
    if reload {
        w.reload(c);
    }
    w.person(c, vec![Step::See { find: Find::text(b"Chat nine"), within: WITHIN }]);
    w.run(Until::Done);
    assert_eq!(w.front().engine.commits().iter().filter(|c| c.key.is_some()).count(), 1);
}
#[test]
fn a_reload_with_requests_pending_sends_them_again_under_their_keys() {
    for seed in [17, 18] {
        start_with_fault(seed, None, true);
    }
}
#[test]
fn the_engine_restarting_under_a_pending_request_answers_it_once() {
    for seed in [19, 20] {
        start_with_fault(
            seed,
            Some(jig_client_world::FrontFault::Restart { offline: Duration::from_millis(120) }),
            false,
        );
    }
}
#[test]
fn a_dropped_watch_shows_the_page_behind_until_it_reopens() {
    for seed in [21, 22] {
        let mut w = stories::chat_ready(seed);
        let c = w.client(7, stories::chat());
        w.person(c, vec![Step::See { find: Find::text(b"Chat nine"), within: WITHIN }]);
        w.run(Until::Done);
        w.fault(jig_client_world::FrontFault::DropWatch);
        w.person(
            c,
            vec![
                Step::See { find: Find::text(b"Behind"), within: WITHIN },
                Step::Gone { find: Find::text(b"Behind"), within: WITHIN },
            ],
        );
        w.run(Until::Done);
    }
}
#[test]
fn a_page_left_with_reads_in_flight_drops_their_answers() {
    for seed in [23, 24] {
        let mut w = stories::proposal_world(seed);
        let c = w.client(7, stories::inbox());
        w.person(c, vec![Step::See { find: Find::text(b"Live"), within: WITHIN }]);
        w.run(Until::Done);
        w.fault(jig_client_world::FrontFault::Slow { span: Duration::from_millis(50) });
        w.go(c, stories::chat());
        w.go(c, stories::conversation());
        w.run(Until::Time(w.now().saturating_add(Duration::from_millis(80))));
        assert!(w.tree(c).nodes().iter().all(|n| n.name.as_deref() != Some(b"Accept".as_slice())));
        assert!(w.front().engine.commits().iter().all(|c| c.key.is_none()));
    }
}

#[test]
fn a_goal_beyond_the_allotment_is_proposed_as_the_confirmation_said() {
    for seed in [25, 26] {
        let mut w = stories::goal_world(seed);
        let c = w.client(
            7,
            jig_client_test_domain::Address::Jig(jig_client_view::address::Address::Project {
                project: 1,
                section: jig_client_view::address::ProjectSection::Goals,
            }),
        );
        w.person(
            c,
            vec![
                Step::See { find: Find::named(Role::TextArea, b"Goal specification"), within: WITHIN },
                Step::Type {
                    find: Find::named(Role::TextArea, b"Goal specification"),
                    words: Box::from(*b"Resolve incident"),
                },
                Step::Press { find: Find::named(Role::Button, b"Charter 1") },
                Step::Type { find: Find::named(Role::TextField, b"Goal budget"), words: Box::from(*b"2") },
                Step::See { find: Find::text(b"Beyond your pool, this will be proposed"), within: WITHIN },
                Step::Press { find: Find::named(Role::Button, b"Set goal") },
                Step::See { find: Find::text(b"proposed"), within: WITHIN },
                Step::Press { find: Find::named(Role::Button, b"Confirm") },
                Step::See { find: Find::text(b"Proposed by task"), within: WITHIN },
            ],
        );
        w.run(Until::Done);
        let commits = w.front().engine.commits();
        let requests: Vec<_> = commits.iter().filter_map(|c| c.ask.as_ref()).collect();
        assert_eq!(requests.len(), 1);
        assert!(
            matches!(requests[0], jig_client_domain::contract::Ask::Propose { action: jig_client_domain::contract::ProposeAction::Batch { tasks }, .. } if tasks.len() == 1 && tasks.get(0).is_some_and(|task| task.authority.spend == 2 && task.spec.words.bytes() == b"Resolve incident"))
        );
        assert!(matches!(
            commits.iter().find(|c| c.key.is_some()).expect("incident fixture fits and exists").answer,
            Some(jig_client_domain::contract::Answer::Done(jig_client_domain::contract::Outcome::Proposed { .. }))
        ));
    }
}

#[test]
fn a_procedure_only_goal_form_confirms_and_sends_the_supplied_executor_once() {
    for seed in [51, 52] {
        let mut w = stories::procedure_goal_world(seed);
        let c = w.client(
            7,
            jig_client_test_domain::Address::Jig(jig_client_view::address::Address::Project {
                project: 1,
                section: jig_client_view::address::ProjectSection::Goals,
            }),
        );
        w.person(
            c,
            vec![
                Step::See { find: Find::named(Role::TextArea, b"Goal specification"), within: WITHIN },
                Step::Type {
                    find: Find::named(Role::TextArea, b"Goal specification"),
                    words: Box::from(*b"Resolve incident"),
                },
                Step::Press { find: Find::named(Role::Button, b"Connector 2, procedure 7") },
                Step::Type { find: Find::named(Role::TextField, b"Goal budget"), words: Box::from(*b"2") },
                Step::Press { find: Find::named(Role::Button, b"Set goal") },
                Step::See { find: Find::text(b"Connector 2, procedure 7"), within: WITHIN },
                Step::See { find: Find::text(b"proposed"), within: WITHIN },
                Step::Press { find: Find::named(Role::Button, b"Confirm") },
                Step::See { find: Find::text(b"Proposed by"), within: WITHIN },
            ],
        );
        let result = w.run(Until::Done);
        assert_eq!(result.verdict, skein_world::domain::Verdict::Passed);
        let commits = w.front().engine.commits();
        let requests: Vec<_> = commits.iter().filter_map(|c| c.ask.as_ref()).collect();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0],
            &jig_client_domain::contract::Ask::Propose {
                project: 1,
                reason: stories::text(b""),
                action: jig_client_domain::contract::ProposeAction::Batch {
                    tasks: jig_client_world::test::list([jig_client_domain::contract::NewTask {
                        llm: None,
                        deadline: jig_client_domain::contract::NewDeadline::Named,
                        executor: jig_client_domain::contract::OwnerExecutor::Procedure { connector: 2, code: 7 },
                        spec: jig_client_domain::contract::Specification {
                            asks: false,
                            words: stories::text(b"Resolve incident"),
                            parameters: jig_client_world::test::list([]),
                            inputs: jig_client_world::test::list([])
                        },
                        contract: jig_client_domain::contract::NewContract::Report { words: 32 },
                        authority: jig_client_domain::contract::OwnerAuthority {
                            models: None,
                            tools: 0,
                            grants: jig_client_world::test::list([]),
                            delegation: jig_client_domain::contract::OwnerDelegation {
                                kinds: jig_client_world::test::list([]),
                                tasks: 1,
                                depth: 1
                            },
                            spend: 2,
                            deadline: None,
                            notes: 0,
                            note_resources: jig_client_world::test::list([])
                        },
                        budget: jig_client_domain::contract::NewBudget::Named,
                        wake: None,
                        dependencies: jig_client_world::test::list([]),
                        named: jig_client_world::test::list([]),
                        tracked: Some(0),
                        label: 2,
                        flags: jig_client_domain::contract::TaskFlags {
                            ended_by: jig_client_domain::contract::EndedBy::Result,
                            conversation: false,
                            listed: true,
                            retention: jig_client_domain::contract::Retention::Kept
                        },
                    }])
                },
            }
        );
        assert!(matches!(
            commits.iter().find(|c| c.key.is_some()).expect("the procedure goal has one keyed commit").answer,
            Some(jig_client_domain::contract::Answer::Done(jig_client_domain::contract::Outcome::Proposed { .. }))
        ));
    }
}

#[test]
fn starting_a_chat_at_the_text_shell() {
    let mut w = stories::chat_world(1);
    let c = w.client(7, stories::conversation());
    w.at_text_shell(c);
    w.person(
        c,
        vec![
            Step::See { find: Find::named(Role::TextArea, b"First words"), within: WITHIN },
            Step::Type { find: Find::named(Role::TextArea, b"First words"), words: Box::from(*b"Hello") },
            Step::Press { find: Find::named(Role::Button, b"Start a conversation") },
            Step::See { find: Find::text(b"Chat nine"), within: WITHIN },
        ],
    );
    let outcome = w.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert_eq!(w.front().engine.commits().iter().filter(|commit| commit.key.is_some()).count(), 1);
}
#[test]
fn a_held_chat_is_released_at_the_text_shell() {
    let mut w = stories::held_chat(11);
    let c = w.client(7, stories::chat());
    w.at_text_shell(c);
    w.person(
        c,
        vec![
            Step::See { find: Find::named(Role::Button, b"Release"), within: WITHIN },
            Step::Press { find: Find::named(Role::Button, b"Release") },
            Step::See { find: Find::text(b"Being decided"), within: WITHIN },
            Step::Press { find: Find::named(Role::Button, b"Confirm") },
            Step::Press { find: Find::named(Role::Tab, b"Overview") },
            Step::See { find: Find::text(b"Starting"), within: WITHIN },
        ],
    );
    let outcome = w.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert_eq!(w.front().engine.commits().iter().filter(|commit| commit.key.is_some()).count(), 1);
}

fn plan() -> Target {
    Target::Role { role: Role::Region, name: Box::from(*b"Plan") }
}
#[test]
fn a_proposal_accepted_in_an_arranged_plan_leaves_while_its_chat_shows() {
    let mut w = stories::arranged_proposal_world(15);
    let c = w.client_with(7, stories::chat(), test::limits(), test::arranged);
    w.run(Until::Time(w.now().saturating_add(Duration::from_millis(80))));
    w.person(
        c,
        vec![
            Step::See { find: Find::named(Role::Log, b"Conversation"), within: WITHIN },
            Step::Press { find: Find::named(Role::Button, b"Accept").within(Box::new([plan()])) },
            Step::Press { find: Find::named(Role::Button, b"Confirm") },
            Step::Gone { find: Find::named(Role::Button, b"Accept").within(Box::new([plan()])), within: WITHIN },
            Step::See { find: Find::named(Role::Log, b"Conversation"), within: WITHIN },
        ],
    );
    w.run(Until::Done);
    assert_eq!(w.front().engine.commits().iter().filter(|c| c.key.is_some()).count(), 1);
}

#[test]
fn a_family_page_shows_its_tasks_plan_and_not_a_second_one() {
    let mut w = stories::two_plans_world(16);
    let c = w.client(7, stories::incident());
    w.person(
        c,
        vec![
            Step::See { find: Find::named(Role::Region, b"Plan"), within: WITHIN },
            Step::See { find: Find::text(b"Cannot show this part"), within: WITHIN },
        ],
    );
    w.run(Until::Done);
}

const QUERY_PROJECT: u32 = 1;
const QUERY_PERSON: u64 = 7;
#[test]
fn a_project_s_tasks_section_watches_one_query_by_last_change() {
    use jig_client_domain::contract::{QueryOrder, Subject, TaskQuery};
    use jig_client_view::address::{Address, ProjectSection};
    let mut world = stories::chat_ready(18);
    world
        .front()
        .engine
        .act(
            skein_lib::Time::ZERO,
            skein_lib::Wall::EPOCH,
            jig_client_scripted_engine::Change::Project(Box::new(stories::project())),
        )
        .expect("query story fixture and delivery");
    let person = world.client(
        QUERY_PERSON,
        jig_client_test_domain::Address::Jig(Address::Project {
            project: QUERY_PROJECT,
            section: ProjectSection::Tasks,
        }),
    );
    world.person(person, vec![Step::See { find: Find::text(b"Chat nine"), within: WITHIN }]);
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    let subjects: Vec<_> =
        world.front().opened.iter().filter(|s| matches!(s, Subject::Tasks { .. })).cloned().collect();
    assert_eq!(
        subjects,
        vec![Subject::Tasks {
            query: TaskQuery {
                project: Some(QUERY_PROJECT),
                requester: None,
                root: None,
                parent: None,
                label: None,
                phase: None,
                executor: None,
                tracked: None,
                order: QueryOrder::Changed
            },
            window: test::limits().jig.rows
        }]
    );
}

#[test]
fn the_test_application_s_own_proposal_view_is_held_to_jig_s_check() {
    let mut world = stories::own_proposal::world(31);
    let client = world.client(stories::own_proposal::PERSON, stories::own_proposal::address());
    let mut person = stories::own_proposal::person();
    person.insert(3, Step::See { find: Find::text(b"After"), within: WITHIN });
    world.person(client, person);
    assert_eq!(world.run(Until::Done).verdict, skein_world::domain::Verdict::Passed);
    assert_eq!(world.front().engine.commits().iter().filter(|commit| commit.key.is_some()).count(), 1);
}
#[test]
#[should_panic(expected = "jig's check: a deciding act did not show its facts")]
fn an_own_proposal_view_hiding_its_effect_fails_jig_s_check() {
    let mut world = stories::own_proposal::world(31);
    let client = world.client_with(
        stories::own_proposal::PERSON,
        stories::own_proposal::address(),
        test::limits(),
        stories::own_proposal::hidden,
    );
    world.person(client, stories::own_proposal::person());
    world.run(Until::Done);
}

const OLDER_CHAT: u64 = 41;
const NEWER_CHAT: u64 = 42;
const EARLIER: skein_lib::Wall = skein_lib::Wall::from_nanos(1);
const LATER: skein_lib::Wall = skein_lib::Wall::from_nanos(2);
const READER: u64 = 7;
#[test]
fn the_conversations_section_lists_live_chats_by_last_activity() {
    let tasks = [
        stories::conversations::chat(OLDER_CHAT, b"Earlier chat", EARLIER),
        stories::conversations::chat(NEWER_CHAT, b"Later chat", LATER),
    ];
    let mut world = stories::conversations::list_world(511, &tasks);
    let client = world.client(READER, stories::conversation());
    world.person(client, vec![Step::See { find: Find::text(b"Later chat"), within: WITHIN }]);
    assert_eq!(world.run(Until::Done).verdict, skein_world::domain::Verdict::Passed);
    let keys: Vec<_> = world
        .tree(client)
        .nodes()
        .iter()
        .filter_map(|node| node.key.filter(|key| key.kind == 20).map(|key| key.a))
        .collect();
    assert_eq!(keys, vec![NEWER_CHAT, OLDER_CHAT]);
}
#[test]
fn the_frame_shows_no_conversations_entry_without_a_chat_template() {
    let mut world = stories::conversations::without_chats(512);
    let client = world.client(READER, stories::inbox());
    world.person(client, vec![Step::See { find: Find::text(b"Signed in as Pat"), within: WITHIN }]);
    assert_eq!(world.run(Until::Done).verdict, skein_world::domain::Verdict::Passed);
    assert!(
        !world
            .tree(client)
            .nodes()
            .iter()
            .any(|node| node.role == Role::Link && node.name.as_deref() == Some(b"Conversations"))
    );
}

#[path = "stories/projects.rs"]
mod projects;
