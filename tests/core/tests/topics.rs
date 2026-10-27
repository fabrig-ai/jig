use jig_core_world::effects::{World, fixture};
use jig_core_world::fixtures::{live_row, person_unchanged};
use jig_core_world::stories::reads::request;
use jig_domain_authority as authority;
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_connector as connector;
use jig_test_domain as root;
use skein_lib::{Queue, ReplyTo, Token};

fn topic_fixture(seed: u64, grant: bool) -> (root::Config, root::Limits) {
    let (mut configuration, limits) = fixture(seed, false);
    configuration.first.topics = Box::new([connector::TopicSpec {
        topic: 1,
        offered: true,
        resources: Box::new([jig_test_connector_world::path(1, 1)]),
    }]);
    let read_grant = authority::Grant {
        connector: 1,
        kind: 0,
        pattern: authority::Pattern { segments: Box::new([]), last: authority::Last::Open(Box::new([])) },
    };
    let mut rules = configuration.core.defaults.rules.clone();
    rules.ceiling.grants = Box::new([read_grant.clone()]);
    let mut policy = configuration.core.seed_projects[0].policy.policy.clone();
    policy.ceiling.grants = Box::new([read_grant.clone()]);
    policy.roles[0].authority.grants = if grant { Box::new([read_grant]) } else { Box::new([]) };
    let mut domain = authority::Domain::new(rules, limits.core.authority).expect("topic rules");
    let mut out = Queue::with_capacity(authority::POLICY_MAX_OUT);
    authority::step(&mut domain, authority::Event::Policy { project: 1, policy }, &mut out);
    assert_eq!(out.pop(), Some(authority::PolicyFact::Added { project: 1 }));
    configuration.core.seed_projects[0].policy.policy = domain.policy(1).expect("checked seed policy").clone();
    configuration.core.defaults.rules = (jig_domain_authority::Domain::new(domain.rules().clone(), *domain.limits())
        .expect("checked rules"))
    .rules()
    .clone();
    (configuration, limits)
}

fn topic_world(seed: u64, grant: bool) -> World {
    let (configuration, limits) = topic_fixture(seed, grant);
    World::configured(seed, false, configuration, limits)
}

fn subscribe(world: &mut World, topic: u16) {
    subscribe_at(world, topic, 1);
}

fn subscribe_at(world: &mut World, topic: u16, completion: u32) {
    world.send(root::Event::TopicCall {
        to: ReplyTo::new(Token::new(801)),
        key: world.call_key(completion),
        number: 1,
        topic,
    });
}

fn news(world: &mut World, importance: u16) {
    world.send(root::Event::Connector {
        number: 1,
        event: connector::Event::System(connector::SystemEvent::News {
            markers: 0,
            topic: 1,
            importance,
            origin: connector::Origin::Other,
        }),
    });
}

#[test]
fn a_topic_call_under_a_read_grant_subscribes_the_task() {
    let mut world = topic_world(181, true);
    subscribe(&mut world, 1);
    assert!(matches!(world.answers.last(), Some((_, _, core::CallPart::Subscribed { .. }))));
    let task = world.call_key(1).task;
    assert!(
        live_row(&world, task)
            .subscriptions
            .iter()
            .any(|subscription| { subscription.kind == tasks::SubscriptionKind::Topic { connector: 1, topic: 1 } })
    );
}

#[test]
fn a_topic_call_without_a_read_grant_is_denied_naming_the_grant() {
    let mut world = topic_world(182, false);
    subscribe(&mut world, 1);
    assert!(matches!(world.answers.last(), Some((_, _, core::CallPart::ToolDenied { findings, .. }))
        if findings.contains(&authority::Finding::Grant { source: authority::Source::Task })));
    assert!(live_row(&world, world.call_key(1).task).subscriptions.is_empty());
}

#[test]
fn a_topic_not_offered_to_agents_is_refused_to_the_run() {
    let mut world = topic_world(183, true);
    subscribe(&mut world, 2);
    assert!(matches!(
        world.answers.last(),
        Some((_, _, core::CallPart::SubscriptionRefused(tasks::Problem { why: tasks::Refusal::Subscription, .. })))
    ));
    assert!(live_row(&world, world.call_key(1).task).subscriptions.is_empty());
}

#[test]
fn news_on_a_subscribed_topic_reaches_the_tasks_inbox() {
    let mut world = topic_world(184, true);
    subscribe(&mut world, 1);
    let subscription = live_row(&world, world.call_key(1).task).subscriptions[0].number;
    news(&mut world, 2);
    let message = live_row(&world, world.call_key(1).task)
        .inbox
        .iter()
        .find(|word| word.kind == tasks::MessageKind::News { subscription, class: tasks::NewsClass::Wakes })
        .expect("classified news in task inbox");
    assert_eq!(message.words.as_ref(), b"News on topic 1");
    assert!(
        world
            .inbound
            .iter()
            .any(|word| word.kind == tasks::MessageKind::News { subscription, class: tasks::NewsClass::Wakes })
    );
}

#[test]
fn dropped_news_reaches_no_inbox() {
    let mut world = topic_world(185, true);
    subscribe(&mut world, 1);
    let before = live_row(&world, world.call_key(1).task).inbox.clone();
    news(&mut world, 0);
    assert_eq!(live_row(&world, world.call_key(1).task).inbox, before);
}

#[test]
fn unsubscribing_a_topic_ends_it_at_the_connector() {
    let mut world = topic_world(186, true);
    subscribe(&mut world, 1);
    let subscription = live_row(&world, world.call_key(1).task).subscriptions[0].number;
    world.send(root::Event::Core(core::Event::NamedAction {
        to: ReplyTo::new(Token::new(802)),
        key: world.call_key(2),
        action: core::NamedAction::Unsubscribe { subscription },
    }));
    assert!(matches!(world.answers.last(), Some((_, _, core::CallPart::Unsubscribed))));
    assert!(!world.store.rows.contains_key(&root::Key::Connector {
        number: 1,
        key: connector::RecordKey::Subscription { topic: 1, task: world.call_key(1).task }
    }));
}

#[test]
fn ending_a_task_ends_its_topic_at_the_connector() {
    let mut world = topic_world(187, true);
    subscribe(&mut world, 1);
    let task = world.call_key(1).task;
    world.end(
        tasks::End::Finished {
            result: tasks::TaskResult::Report { words: b"finished".as_slice().into() },
            cancel_delegates: false,
        },
        0,
    );
    assert!(
        !world.store.rows.contains_key(&root::Key::Connector {
            number: 1,
            key: connector::RecordKey::Subscription { topic: 1, task },
        })
    );
}

// Two application-numbered codes, including the highest bit admitted by tasks.md 8.3.
const FIRST_MARKER: u32 = 1;
const LAST_MARKER: u32 = 1 << 31;
const CONNECTOR: u16 = 1;
const TOPIC: u16 = 1;
const SECOND_TOPIC: u16 = 2;
// The amendment story refreshes both topics.
const MARKER_TOPICS: &[u16] = &[TOPIC, SECOND_TOPIC];
const AMEND: u8 = 11;

fn marker_policy(markers: u32, news: people::WakeRule) -> people::WakePolicy {
    people::WakePolicy {
        markers,
        words: people::WakeRule::Immediate,
        notices: people::WakeRule::Immediate,
        news,
        results: people::ResultsWake::LastOrFailure,
        questions: true,
        answers: true,
        timers: true,
    }
}

fn marker_world(seed: u64, markers: u32, news: people::WakeRule) -> World {
    marker_world_with_topics(seed, markers, news, &[TOPIC])
}

fn marker_world_with_topics(seed: u64, markers: u32, news: people::WakeRule, topics: &[u16]) -> World {
    let (mut configuration, mut limits) = topic_fixture(seed, true);
    let topic_count = u32::try_from(topics.len()).expect("fixture topics fit");
    limits.connector.topics = topic_count;
    // Each topic reserves result_bytes; the tracked goal also reserves message_bytes.
    limits.core.tasks.inbox_bytes = topic_count * limits.core.tasks.result_bytes + limits.core.tasks.message_bytes;
    configuration.first.topics = topics
        .iter()
        .map(|topic| connector::TopicSpec {
            topic: *topic,
            offered: true,
            resources: Box::new([jig_test_connector_world::path(1, 1)]),
        })
        .collect();
    let mut initial = jig_core_world::fixtures::create_chat(
        &jig_core_world::fixtures::Creating::new(&configuration.core, limits.core.tasks.tree_tasks),
        1,
        0,
        b"watch a topic".as_slice().into(),
    );
    let people::Ask::Create { batch, .. } = &mut initial else { unreachable!("chat creation") };
    batch[0].wake = marker_policy(markers, news);
    batch[0].tracked = Some(0);
    World::configured_with(seed, false, configuration, limits, initial)
}

fn amend_markers(world: &mut World, markers: u32, news: people::WakeRule) {
    let task = world.call_key(1).task;
    let revision = live_row(world, task).revisions.spec;
    request(
        world,
        AMEND,
        people::Ask::Amend {
            project: 1,
            task,
            revision,
            amendment: people::Amendment {
                wake: Some(marker_policy(markers, news)),
                ..person_unchanged(b"change wake policy")
            },
            release: None,
        },
    );
}

fn subscription_writes(world: &World) -> Vec<connector::Record> {
    world
        .commits
        .iter()
        .flatten()
        .filter_map(|write| match write {
            root::Write::Save(root::Record::Connector {
                number: CONNECTOR,
                record: record @ connector::Record::Subscription { .. },
            }) => Some(record.clone()),
            root::Write::CopySettingsRevision(_)
            | root::Write::Wires
            | root::Write::Save(_)
            | root::Write::Erase(_) => None,
        })
        .collect()
}

fn connector_subscription(world: &World, topic: u16, markers: u32) -> connector::Record {
    connector::Record::Subscription { task: world.call_key(1).task, topic, wake_at: 2, keep_at: 1, markers }
}

fn kept_subscription(world: &World) -> &connector::Record {
    let task = world.call_key(1).task;
    let key =
        root::Key::Connector { number: CONNECTOR, key: connector::RecordKey::Subscription { topic: TOPIC, task } };
    let root::Record::Connector { record, .. } = &world.store.rows[&key] else { unreachable!("connector row") };
    record
}

#[test]
fn a_goals_markers_cross_with_its_topic_subscription() {
    let mut world = marker_world(188, LAST_MARKER, people::WakeRule::Immediate);
    subscribe(&mut world, TOPIC);
    assert_eq!(kept_subscription(&world), &connector_subscription(&world, TOPIC, LAST_MARKER));
}

#[test]
fn amending_a_policys_markers_subscribes_again_with_the_new_set() {
    let mut world = marker_world_with_topics(189, FIRST_MARKER, people::WakeRule::Immediate, MARKER_TOPICS);
    subscribe(&mut world, TOPIC);
    subscribe_at(&mut world, SECOND_TOPIC, 2);
    world.commits.clear();
    amend_markers(&mut world, LAST_MARKER, people::WakeRule::Immediate);
    assert_eq!(
        subscription_writes(&world),
        [connector_subscription(&world, TOPIC, LAST_MARKER), connector_subscription(&world, SECOND_TOPIC, LAST_MARKER),]
    );
}

#[test]
fn an_amendment_that_keeps_the_markers_asks_nothing() {
    let mut world = marker_world(190, FIRST_MARKER, people::WakeRule::Immediate);
    subscribe(&mut world, TOPIC);
    world.commits.clear();
    amend_markers(&mut world, FIRST_MARKER, people::WakeRule::Never);
    assert_eq!(live_row(&world, world.call_key(1).task).wake.news, tasks::WakeRule::Never);
    assert!(subscription_writes(&world).is_empty());
}

fn marked_news(world: &mut World, markers: u32) {
    world.send(root::Event::Connector {
        number: CONNECTOR,
        event: connector::Event::System(connector::SystemEvent::News {
            topic: TOPIC,
            importance: 1,
            origin: connector::Origin::Other,
            markers,
        }),
    });
}

#[test]
fn news_carrying_a_marker_wakes_a_task_whose_policy_keeps_news_otherwise() {
    let mut world = marker_world(191, LAST_MARKER, people::WakeRule::Immediate);
    subscribe(&mut world, TOPIC);
    let attempt = world.assigned.as_ref().expect("initial run").attempt;
    world.end(tasks::End::Parked, 0);
    marked_news(&mut world, LAST_MARKER);
    assert!(world.assigned.as_ref().expect("marker wakes a run").attempt > attempt);
    assert!(
        live_row(&world, world.call_key(1).task)
            .inbox
            .iter()
            .any(|word| matches!(word.kind, tasks::MessageKind::News { class: tasks::NewsClass::Wakes, .. }))
    );
}

#[test]
fn the_news_rule_can_lower_marker_news_to_kept() {
    let mut world = marker_world(192, LAST_MARKER, people::WakeRule::Never);
    subscribe(&mut world, TOPIC);
    world.end(tasks::End::Parked, 0);
    marked_news(&mut world, LAST_MARKER);
    let task = world.call_key(1).task;
    assert_eq!(live_row(&world, task).phase, tasks::Phase::Active(tasks::Active::Idle));
    assert!(
        live_row(&world, task)
            .inbox
            .iter()
            .any(|word| matches!(word.kind, tasks::MessageKind::News { class: tasks::NewsClass::Wakes, .. })
                && !word.eligible)
    );
}

#[test]
fn a_marker_outside_the_policy_keeps_news_without_waking() {
    let mut world = marker_world(193, LAST_MARKER, people::WakeRule::Immediate);
    subscribe(&mut world, TOPIC);
    world.end(tasks::End::Parked, 0);
    marked_news(&mut world, FIRST_MARKER);
    assert_eq!(live_row(&world, world.call_key(1).task).phase, tasks::Phase::Active(tasks::Active::Idle));
}

#[test]
fn markers_survive_a_restart() {
    let mut world = marker_world(194, u32::MAX, people::WakeRule::Immediate);
    subscribe(&mut world, TOPIC);
    let (configuration, _) = topic_fixture(194, true);
    world.restart_with(configuration);
    assert_eq!(live_row(&world, world.call_key(1).task).wake.markers, u32::MAX);
    assert!(matches!(kept_subscription(&world), connector::Record::Subscription { markers: u32::MAX, .. }));
}
