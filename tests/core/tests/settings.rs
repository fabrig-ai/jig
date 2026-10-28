//! Settings travel through the real restart, store and commit barrier (engine.md, 17.7).
use jig_core_world::{client_peer::ClientPeer, effects::World, fixtures, peers};
use jig_domain_core::{self as core, documents, reads, settings};
use jig_domain_fleet as fleet;
use jig_domain_people as people;
use jig_test_domain as root;
use skein_lib::{Duration, ReplyTo, Token};

#[path = "settings/support.rs"]
mod support;
use support::*;

#[test]
fn the_document_bridge_starts_once_and_preserves_templates_policies_rules_and_models() {
    let mut world = start(64);
    assert_eq!(revision(&world, 1, false).head.by, settings::Decider::FirstStart);
    assert!(revision(&world, 1, false).items.is_empty());
    let signed = sign_in(&mut world);
    let before = frame(&mut world, signed);
    assert_eq!(
        before.projects.get(0).expect("settings story fixture").templates.as_slice(),
        configuration().0.core.defaults.templates.as_ref()
    );
    let policy = configuration().0.core.defaults.policies.get(&1).expect("settings story fixture").policy.clone();
    let reply = ask(&mut world, signed, 1, people::Ask::MakeProject { name: Box::from("Made"), policy: 1 });
    assert!(matches!(reply, people::Reply::Outcome(people::Outcome::ProjectMade { .. })), "{reply:?}");
    let root::Record::Core(core::Record::People(people::Stored::Policy { value, .. })) =
        &world.store.rows[&root::Key::Core(core::Key::People(people::Key::Policy(2)))]
    else {
        panic!("copied project policy")
    };
    assert_eq!(value.ceiling.spend, policy.ceiling.budget.spend);
    assert_eq!(world.domain.settings().rules(), &configuration().0.core.defaults.rules);
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 100, next: None }));
    world.send(root::Event::Core(core::Event::Fleet(fleet::Event::Hello {
        worker: 0,
        channel: Token::new(7),
        hello: fleet::Hello {
            bounds: fixtures::bounds(),
            stop_bound: Duration::from_millis(1),
            slots: 1,
            workstreams: Box::new([]),
            hosting: Box::new([]),
        },
    })));
    let chat = fixtures::chat(&world, 1, Box::from(b"hello".as_slice()));
    let reply = ask(&mut world, signed, 2, chat);
    assert!(matches!(reply, people::Reply::Outcome(people::Outcome::Created { .. })), "{reply:?}");
    let assignment = world.assigned.as_ref().expect("document charter runs");
    assert_eq!(assignment.run.policy.model.name.as_ref(), b"scripted");
    assert_eq!(assignment.run.policy.instructions.as_ref(), b"Answer within the granted authority.");
    restart(&mut world, configuration().0);
    assert_eq!(world.domain.settings().newest(), 1);
    let signed = sign_in(&mut world);
    assert_eq!(
        frame(&mut world, signed).projects.get(0).expect("settings story fixture").templates,
        before.projects.get(0).expect("settings story fixture").templates
    );
}
#[test]
fn a_release_changes_the_template_once_and_archives_its_predecessor() {
    let mut world = start(64);
    let mut config = configuration().0;
    config.core.defaults.templates[0].grace = Some(Duration::from_millis(6000));
    restart(&mut world, config);
    let newest = revision(&world, 2, false);
    assert_eq!(newest.head.by, settings::Decider::Release);
    assert_eq!(newest.head.against, 1);
    assert!(
        matches!(&newest.items[..], [settings::Item { setting: settings::Setting::Template(1), edit: settings::Edit::Set(settings::Value::Template(value)) }] if value.grace == Some(Duration::from_millis(6000)))
    );
    assert_eq!(revision(&world, 1, true).head.by, settings::Decider::FirstStart);
    let signed = sign_in(&mut world);
    assert_eq!(
        frame(&mut world, signed)
            .projects
            .get(0)
            .expect("settings story fixture")
            .templates
            .get(0)
            .expect("settings story fixture")
            .grace,
        Some(Duration::from_millis(6000))
    );
    let mut config = configuration().0;
    config.core.defaults.templates[0].grace = Some(Duration::from_millis(6000));
    restart(&mut world, config);
    assert_eq!(world.domain.settings().newest(), 2);
}
#[test]
fn a_stored_change_wins_until_reset_and_a_dropped_default_is_no_longer_offered() {
    let mut world = start(64);
    let mut template = configuration().0.core.defaults.templates[0].clone();
    template.grace = Some(Duration::from_millis(7000));
    world.seed(Box::new([record(settings::Record::Changed {
        setting: settings::Setting::Template(1),
        changed: settings::Changed { revision: 1, value: Some(settings::Value::Template(Box::new(template))) },
    })]));
    let mut config = configuration().0;
    config.core.defaults.templates[0].grace = Some(Duration::from_millis(6000));
    restart(&mut world, config);
    assert_eq!(world.domain.settings().newest(), 1);
    assert_eq!(
        world.domain.settings().template(1).expect("settings story fixture").grace,
        Some(Duration::from_millis(7000))
    );
    world.erase(key(settings::Key::Changed(settings::Setting::Template(1))));
    let mut config = configuration().0;
    config.core.defaults.templates[0].grace = Some(Duration::from_millis(6000));
    restart(&mut world, config);
    assert_eq!(world.domain.settings().newest(), 1);
    assert_eq!(
        world.domain.settings().template(1).expect("settings story fixture").grace,
        Some(Duration::from_millis(6000))
    );
    let mut config = configuration().0;
    config.core.defaults.templates = Box::new([config.core.defaults.templates[0].clone()]);
    restart(&mut world, config);
    let signed = sign_in(&mut world);
    assert_eq!(frame(&mut world, signed).projects.get(0).expect("settings story fixture").templates.len(), 1);
}
#[test]
fn history_keeps_two_predecessors_after_three_releases() {
    let mut world = start(2);
    for grace in 6..=8 {
        let mut config = configuration().0;
        config.core.defaults.templates[0].grace = Some(Duration::from_secs(grace));
        restart(&mut world, config);
    }
    assert_eq!(world.domain.settings().newest(), 4);
    assert_eq!(world.domain.settings().oldest(), 2);
    assert!(!world.store.rows.contains_key(&key(settings::Key::History(1))));
    for number in 2..=3 {
        assert_eq!(revision(&world, number, true).head.number, number);
    }
    assert_eq!(revision(&world, 4, false).head.number, 4);
}
#[test]
fn a_stored_charter_with_an_unconfigured_model_refuses_the_start_naming_the_change() {
    let mut world = start(64);
    let mut charter = configuration().0.core.defaults.charters[0].clone();
    charter.policy.model.name = Box::from(b"no-longer-configured".as_slice());
    world.seed(Box::new([record(settings::Record::Changed {
        setting: settings::Setting::Charter(1),
        changed: settings::Changed { revision: 1, value: Some(settings::Value::Charter(Box::new(charter))) },
    })]));
    world.restart_with(configuration().0);
    assert_eq!(
        world.domain.restore_failure(),
        Some(core::RestartRequest::Settings {
            failure: settings::Failure { setting: settings::Setting::Charter(1), fault: settings::Fault::Reference },
            changed: true
        })
    );
    assert!(!world.domain.ready());
    assert!(world.stopped);
}

#[test]
fn a_start_keeps_a_dropped_charters_version_and_its_task_runs_on_it() {
    let mut world = start(64);
    let signed = sign_in(&mut world);
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 100, next: None }));
    let chat = fixtures::chat(&world, 1, Box::from(b"still live".as_slice()));
    let reply = ask(&mut world, signed, 3, chat);
    assert!(matches!(reply, people::Reply::Outcome(people::Outcome::Created { .. })), "{reply:?}");
    seed_restart_accounts(&mut world);
    let mut config = configuration().0;
    // T6 owns versioned fleet needs. Keep the model declared by a replacement
    // charter while this story restores work whose charter was dropped.
    config.core.defaults.charters[0].number = 2;
    config.core.defaults.templates = Box::new([]);
    config.core.defaults.policies = skein_lib::Map::with_capacity(0);
    config.core.defaults.rules.ceiling.delegation.kinds = Box::new([]);
    restart(&mut world, config);
    assert!(world.domain.restore_failure().is_none());
    assert_eq!(revision(&world, 2, false).head.by, settings::Decider::Release);
    assert_eq!(revision(&world, 1, false).head.number, 1);
    assert!(world.store.rows.contains_key(&key(settings::Key::Charter { charter: 1, revision: 1 })));
    resume_host_after_restart(&mut world);
    let run = world.assigned.as_ref().expect("the restored task runs on the dropped charter");
    assert_eq!(run.run.policy.instructions.as_ref(), b"Answer within the granted authority.");
    assert_eq!(fixtures::live_row(&world, run.task).settings.run, Some(1));
}

#[test]
fn changes_past_their_kinds_bound_refuse_before_the_overflow_value_is_retained() {
    let mut world = start(64);
    let config = configuration().0;
    let bound = configuration().1.core.tasks.charters;
    let extra = bound.checked_add(1).expect("one past the charter count");
    let mut rows = skein_lib::List::with_capacity(extra);
    for number in 1..=extra {
        let mut charter = config.core.defaults.charters[0].clone();
        charter.number = number;
        rows.push(record(settings::Record::Changed {
            setting: settings::Setting::Charter(number),
            changed: settings::Changed { revision: 1, value: Some(settings::Value::Charter(Box::new(charter))) },
        }))
        .expect("bounded fixture rows");
    }
    world.seed(rows.into_boxed());
    world.restart_with(config);
    assert!(!world.domain.ready());
    assert!(!world.domain.settings().changed(settings::Setting::Charter(extra)));
    assert_eq!(
        world.domain.restore_failure(),
        Some(core::RestartRequest::Settings {
            failure: settings::Failure { setting: settings::Setting::Charter(extra), fault: settings::Fault::Count },
            changed: true
        })
    );
    assert_eq!(revision(&world, 1, false).head.number, 1);
}

#[test]
fn an_owner_changes_a_template_once_me_serves_it_and_restart_restores_revision_two() {
    let mut world = start(64);
    let signed = sign_in(&mut world);
    let item = template_edit(&world, 7);
    changed(&change(&mut world, signed, 41, 1, Box::new([item.clone()])), 2);
    let me = frame(&mut world, signed);
    assert_eq!(
        me.projects.get(0).expect("settings fixture").templates.get(0).expect("settings fixture").grace,
        Some(Duration::from_secs(7))
    );
    let head = &revision(&world, 2, false).head;
    assert_eq!(head.by, settings::Decider::Party(OWNER));
    assert_eq!(head.against, 1);
    assert!(!head.moved);
    assert!(head.proposal.is_none());
    let before = world.store.rows.clone();
    changed(&change(&mut world, signed, 41, 1, Box::new([item])), 2);
    assert_eq!(world.store.rows, before, "a saved key never makes a second revision");
    restart(&mut world, configuration().0);
    assert_eq!(world.domain.settings().newest(), 2);
    let signed = sign_in(&mut world);
    assert_eq!(
        frame(&mut world, signed)
            .projects
            .get(0)
            .expect("settings fixture")
            .templates
            .get(0)
            .expect("settings fixture")
            .grace,
        Some(Duration::from_secs(7))
    );
}

#[test]
fn a_changed_future_policy_is_copied_only_by_projects_made_after_it() {
    let mut world = start(64);
    let signed = sign_in(&mut world);
    let old = world.domain.settings().policy(1).expect("settings fixture").clone();
    let mut next = old.clone();
    next.policy.period_spend -= 1;
    for role in &mut next.policy.roles {
        role.period_spend = role.period_spend.min(next.policy.period_spend);
    }
    changed(
        &change(
            &mut world,
            signed,
            41,
            1,
            Box::new([settings::Item {
                setting: settings::Setting::Policy(1),
                edit: settings::Edit::Set(settings::Value::Policy(Box::new(next.clone()))),
            }]),
        ),
        2,
    );
    assert!(matches!(
        ask(&mut world, signed, 42, people::Ask::MakeProject { name: "After".into(), policy: 1 }),
        people::Reply::Outcome(people::Outcome::ProjectMade { project: 2 })
    ));
    for (project, expected) in [(1, old.policy.period_spend), (2, next.policy.period_spend)] {
        let root::Record::Core(core::Record::People(people::Stored::Policy { value, .. })) =
            &world.store.rows[&root::Key::Core(core::Key::People(people::Key::Policy(project)))]
        else {
            panic!("policy record")
        };
        assert_eq!(value.period_spend, expected);
    }
}

#[test]
fn a_narrowed_run_rule_refuses_the_next_oversized_creation_and_keeps_the_old_project_policy() {
    let mut world = start(64);
    let signed = sign_in(&mut world);
    let policy = world.store.rows[&root::Key::Core(core::Key::People(people::Key::Policy(1)))].clone();
    let mut rules = world.domain.settings().rules().clone();
    rules.maximum_run_spend = 10;
    rules.ceiling.budget.spend = 10;
    let mut future = world.domain.settings().policy(1).expect("future policy").clone();
    future.policy.ceiling.budget.spend = 10;
    for role in &mut future.policy.roles {
        role.authority.budget.spend = role.authority.budget.spend.min(10);
    }
    // Run spend is a per-run cap. The authority ceiling
    // separately makes a new task's requested allotment of twenty inadmissible.
    changed(
        &change(
            &mut world,
            signed,
            41,
            1,
            Box::new([
                settings::Item {
                    setting: settings::Setting::Rules,
                    edit: settings::Edit::Set(settings::Value::Rules(Box::new(rules))),
                },
                settings::Item {
                    setting: settings::Setting::Policy(1),
                    edit: settings::Edit::Set(settings::Value::Policy(Box::new(future))),
                },
            ]),
        ),
        2,
    );
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 100, next: None }));
    let mut request = fixtures::chat(&world, 1, Box::from(b"over the rule".as_slice()));
    let people::Ask::Create { batch, .. } = &mut request else { panic!("chat create") };
    batch[0].budget = people::Budget::Named;
    batch[0].authority.spend = 20;
    let answer = ask(&mut world, signed, 42, request);
    assert!(
        matches!(answer, people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Authority { .. }))),
        "{answer:?}"
    );
    assert_eq!(world.store.rows[&root::Key::Core(core::Key::People(people::Key::Policy(1)))], policy);
    restart(&mut world, configuration().0);
    assert_eq!(world.domain.settings().rules().maximum_run_spend, 10);
    assert_eq!(world.store.rows[&root::Key::Core(core::Key::People(people::Key::Policy(1)))], policy);
}

#[test]
fn changed_charter_instructions_and_effort_prepare_the_next_new_task() {
    let (mut config, limits) = configuration();
    config.core.settings.models[0].efforts = Box::new([Box::from(b"low".as_slice()), Box::from(b"high".as_slice())]);
    config.core.settings.models[0].effort = Some(Box::from(b"low".as_slice()));
    config.core.defaults.charters[0].policy.model = config.core.settings.models[0].clone();
    let mut world = World::first_start_with_accounts(config, &limits);
    world.client_peer = Some(ClientPeer::default());
    let signed = sign_in(&mut world);
    let mut charter = world.domain.settings().charter(1).expect("settings fixture").clone();
    charter.policy.instructions = Box::from(b"Changed instructions.".as_slice());
    charter.policy.model.effort = Some(Box::from(b"high".as_slice()));
    // A selected effort is a charter default; the model's declared efforts stay configured.
    changed(
        &change(
            &mut world,
            signed,
            41,
            1,
            Box::new([settings::Item {
                setting: settings::Setting::Charter(1),
                edit: settings::Edit::Set(settings::Value::Charter(Box::new(charter))),
            }]),
        ),
        2,
    );
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 100, next: None }));
    world.send(root::Event::Core(core::Event::Fleet(fleet::Event::Hello {
        worker: 0,
        channel: Token::new(7),
        hello: fleet::Hello {
            bounds: fixtures::bounds(),
            stop_bound: Duration::from_millis(1),
            slots: 1,
            workstreams: Box::new([]),
            hosting: Box::new([]),
        },
    })));
    let chat = fixtures::chat(&world, 1, Box::from(b"new task".as_slice()));
    assert!(matches!(ask(&mut world, signed, 42, chat), people::Reply::Outcome(people::Outcome::Created { .. })));
    let assigned = world.assigned.as_ref().expect("next run");
    assert_eq!(assigned.run.policy.instructions.as_ref(), b"Changed instructions.");
    assert_eq!(assigned.run.policy.model.effort.as_deref(), Some(b"high".as_slice()));
}

#[test]
fn refused_changes_preserve_all_settings_rows_and_the_newest() {
    let mut world = start(64);
    let signed = sign_in(&mut world);
    let mut invalid = world.domain.settings().template(1).expect("settings fixture").clone();
    invalid.executor = people::TemplateExecutor::Charter(999);
    let cases: [(u32, Box<[settings::Item]>, people::Refusal); 3] = [
        (0, Box::new([template_edit(&world, 7)]), people::Refusal::Stale { newest: 1 }),
        (
            1,
            Box::<[settings::Item]>::default(),
            people::Refusal::NotWellFormed { at: 0, why: people::Malformed::SettingsUnchanged },
        ),
        (
            1,
            Box::new([settings::Item {
                setting: settings::Setting::Template(1),
                edit: settings::Edit::Set(settings::Value::Template(Box::new(invalid))),
            }]),
            people::Refusal::NotWellFormed {
                at: 0,
                why: people::Malformed::Settings {
                    setting: settings::Setting::Template(1),
                    fault: settings::Fault::Reference,
                },
            },
        ),
    ];
    for (at, (against, items, refusal)) in cases.into_iter().enumerate() {
        let before = settings_rows(&world);
        assert_eq!(
            change(&mut world, signed, u8::try_from(at + 41).expect("settings fixture"), against, items),
            people::Reply::Outcome(people::Outcome::Refused(refusal))
        );
        assert_eq!(world.domain.settings().newest(), 1);
        let after = settings_rows(&world);
        assert_eq!(before, after);
    }
}

#[test]
fn a_live_task_keeps_its_revision_when_the_newest_removes_its_charter() {
    let mut world = start(64);
    let signed = sign_in(&mut world);
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 100, next: None }));
    let chat = fixtures::chat(&world, 1, Box::from(b"live".as_slice()));
    let people::Reply::Outcome(people::Outcome::Created { first, .. }) = ask(&mut world, signed, 40, chat) else {
        panic!("live agent")
    };
    let mut rules = world.domain.settings().rules().clone();
    rules.ceiling.delegation.kinds = Box::new([]);
    let items = Box::new([
        settings::Item { setting: settings::Setting::Charter(1), edit: settings::Edit::Removed },
        settings::Item { setting: settings::Setting::Template(1), edit: settings::Edit::Removed },
        settings::Item { setting: settings::Setting::Policy(1), edit: settings::Edit::Removed },
        settings::Item {
            setting: settings::Setting::Rules,
            edit: settings::Edit::Set(settings::Value::Rules(Box::new(rules))),
        },
    ]);
    changed(&change(&mut world, signed, 41, 1, items), 2);
    assert_eq!(world.domain.settings().newest(), 2);
    assert!(world.store.rows.contains_key(&key(settings::Key::Held(1))));
    assert!(world.domain.settings().charter(1).is_none());
    assert_eq!(fixtures::live_row(&world, first).settings.task, 1);
    assert_eq!(
        ask(&mut world, signed, 42, people::Ask::MoveToNewest { revision: 1 }),
        people::Reply::Outcome(people::Outcome::MovedToNewest { newest: 2, moved: 0, stayed: 1 })
    );
}

#[test]
fn four_owner_changes_keep_two_history_records_beside_revision_five() {
    let mut world = start(2);
    let signed = sign_in(&mut world);
    for newest in 1..=4 {
        let item = template_edit(&world, u64::from(newest + 5));
        changed(
            &change(&mut world, signed, u8::try_from(newest + 40).expect("settings fixture"), newest, Box::new([item])),
            newest + 1,
        );
    }
    assert_eq!(world.domain.settings().newest(), 5);
    assert_eq!(world.domain.settings().oldest(), 3);
    for number in 1..=2 {
        assert!(!world.store.rows.contains_key(&key(settings::Key::History(number))));
    }
    for number in 3..=4 {
        assert_eq!(revision(&world, number, true).head.number, number);
    }
    assert_eq!(revision(&world, 5, false).head.number, 5);
}

#[test]
fn a_member_cannot_change_settings_and_the_key_keeps_that_refusal() {
    let mut world = start(64);
    let signed = sign_in(&mut world);
    world.send(root::Event::Core(core::Event::People(people::Event::Roles {
        project: people::DEPLOYMENT,
        holdings: Box::new([people::Holding { person: OWNER, role: people::Role::Member }]),
    })));
    let item = template_edit(&world, 7);
    let answer = change(&mut world, signed, 41, 1, Box::new([item.clone()]));
    assert!(matches!(
        answer,
        people::Reply::Refused(people::Refusal::Role { held: Some(people::Role::Member) })
            | people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Role {
                held: Some(people::Role::Member)
            }))
    ));
    assert_eq!(world.domain.settings().newest(), 1);
    assert!(!world.store.rows.contains_key(&key(settings::Key::Changed(settings::Setting::Template(1)))));
    assert_eq!(change(&mut world, signed, 41, 1, Box::new([item])), answer);
}

#[test]
fn a_settings_answer_and_the_new_me_watch_wait_together_for_durability() {
    let mut world = start(64);
    let signed = sign_in(&mut world);
    let watcher = Token::new(45);
    world.send(root::Event::Core(core::Event::Watch {
        watcher,
        sign_in: signed,
        subject: documents::Watched::Frame,
        window: documents::Window { rows: 8, after: None },
    }));
    let peer = world.client_peer.as_mut().expect("client");
    assert!(matches!(peer.delivered.pop(), Some((_, root::Delivery::Core(core::Held::Delivery { .. })))));
    world.send(root::Event::Core(core::Event::Delivered { watcher }));
    world.store.fault(jig_core_world::Fault::Hold { commits: 1 });
    let item = template_edit(&world, 7);
    world.send(root::Event::Core(core::Event::ChangeSettings {
        reply_to: ReplyTo::new(Token::new(SEED)),
        sign_in: signed,
        key: [41; 16],
        digest: [41; 32],
        change: Box::new(settings::Change { against: 1, items: Box::new([item]), running: settings::Running::Moves }),
    }));
    assert!(world.client_peer.as_ref().expect("client").delivered.is_empty());
    world.release_commits();
    let peer = world.client_peer.as_ref().expect("client");
    assert_eq!(peer.delivered.len(), 2);
    for (commit, delivery) in &peer.delivered {
        assert!(*commit <= world.store.applied);
        if let root::Delivery::Core(core::Held::PeopleReply { reply, .. }) = delivery {
            changed(reply, 2);
        } else if let root::Delivery::Core(core::Held::Delivery { document, .. }) = delivery {
            let documents::Document::Frame(frame) = document.as_ref() else { panic!("me watch") };
            assert_eq!(
                frame.projects.get(0).expect("project").templates.get(0).expect("template").grace,
                Some(Duration::from_secs(7))
            );
        } else {
            panic!("settings publication {delivery:?}");
        }
    }
    assert!(revision(&world, 2, false).head.moved);
}

#[test]
fn live_tasks_with_llm_choices_the_newest_disallows_stay_and_are_all_counted() {
    let (mut config, mut limits) = configuration();
    let mut other = config.core.settings.models[0].clone();
    other.name = Box::from(b"other".as_slice());
    config.core.settings.models = Box::new([config.core.settings.models[0].clone(), other.clone()]);
    config.core.defaults.charters[0].policy.alternatives = Box::new([other.clone()]);
    limits.core.tasks.circle = 1;
    peers::journal(&mut limits);
    let mut world = World::first_start_with_accounts(config, &limits);
    world.client_peer = Some(ClientPeer::default());
    let signed = sign_in(&mut world);
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 100, next: None }));
    let mut made = Vec::new();
    for key in [39, 40] {
        let mut chat = fixtures::chat(&world, 1, Box::from(b"model held".as_slice()));
        let people::Ask::Create { batch, .. } = &mut chat else { panic!("agent batch") };
        batch[0].budget = people::Budget::Named;
        batch[0].authority.spend = 20;
        let people::Reply::Outcome(people::Outcome::Created { first, .. }) = ask(&mut world, signed, key, chat) else {
            panic!("agent task")
        };
        made.push(first);
    }
    let mut charter = world.domain.settings().charter(1).expect("charter").clone();
    charter.policy.model = other;
    charter.policy.alternatives = Box::new([]);
    let item = settings::Item {
        setting: settings::Setting::Charter(1),
        edit: settings::Edit::Set(settings::Value::Charter(Box::new(charter))),
    };
    changed(&change(&mut world, signed, 41, 1, Box::new([item])), 2);
    assert_eq!(
        ask(&mut world, signed, 42, people::Ask::MoveToNewest { revision: 1 }),
        people::Reply::Outcome(people::Outcome::MovedToNewest { newest: 2, moved: 0, stayed: 2 })
    );
    for task in made {
        assert_eq!(fixtures::live_row(&world, task).settings.task, 1);
    }
    assert!(world.store.rows.contains_key(&key(settings::Key::Held(1))));
}

#[test]
fn a_changed_run_cap_applies_to_the_next_run_without_capping_its_tasks_total_allotment() {
    let mut world = start(64);
    let signed = sign_in(&mut world);
    let mut rules = world.domain.settings().rules().clone();
    rules.maximum_run_spend = 20;
    changed(
        &change(
            &mut world,
            signed,
            41,
            1,
            Box::new([settings::Item {
                setting: settings::Setting::Rules,
                edit: settings::Edit::Set(settings::Value::Rules(Box::new(rules))),
            }]),
        ),
        2,
    );
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 100, next: None }));
    world.send(root::Event::Core(core::Event::Fleet(fleet::Event::Hello {
        worker: 0,
        channel: Token::new(7),
        hello: fleet::Hello {
            bounds: fixtures::bounds(),
            stop_bound: Duration::from_millis(1),
            slots: 1,
            workstreams: Box::new([]),
            hosting: Box::new([]),
        },
    })));
    let mut chat = fixtures::chat(&world, 1, Box::from(b"forty across runs".as_slice()));
    let people::Ask::Create { batch, .. } = &mut chat else { panic!("chat") };
    batch[0].budget = people::Budget::Named;
    batch[0].authority.spend = 40;
    assert!(matches!(ask(&mut world, signed, 42, chat), people::Reply::Outcome(people::Outcome::Created { .. })));
    assert_eq!(world.assigned.as_ref().expect("new run").run.budget, 20);
}

fn client_read(world: &mut World, signed: u64, read: reads::Read) -> reads::Answer {
    world.send(root::Event::Core(core::Event::Read { reply: ReplyTo::new(Token::new(SEED)), sign_in: signed, read }));
    let (_, root::Delivery::Core(core::Held::Read { answer, .. })) =
        world.client_peer.as_mut().expect("settings story fixture").delivered.pop().expect("read terminal")
    else {
        panic!("settings read answer")
    };
    answer
}
fn inventory(world: &mut World, signed: u64) -> documents::Settings {
    let reads::Answer::Document(document) = client_read(world, signed, reads::Read::Settings) else {
        panic!("settings inventory")
    };
    let documents::Document::Settings(document) = *document else { panic!("settings document") };
    *document
}
fn released_history() -> World {
    let (config, mut limits) = configuration();
    limits.core.settings.history = 2;
    limits.core.documents.rows = 2;
    peers::journal(&mut limits);
    let mut world = World::first_start_with_accounts(config, &limits);
    world.client_peer = Some(ClientPeer::default());
    for grace in 6..=10 {
        let mut config = configuration().0;
        config.core.defaults.templates[0].grace = Some(Duration::from_secs(grace));
        restart(&mut world, config);
    }
    world
}
#[test]
fn settings_inventory_tracks_each_defaults_revision_across_a_release() {
    let mut world = start(2);
    let signed = sign_in(&mut world);
    let first = inventory(&mut world, signed);
    assert_eq!(first.newest, 1);
    assert!(first.settings.iter().all(|line| line.follows == documents::Follows::Default && line.last == 1));
    assert_eq!(first.held.get(0).expect("settings story fixture").holders, 0);
    let mut config = configuration().0;
    config.core.defaults.templates[0].grace = Some(Duration::from_secs(6));
    restart(&mut world, config);
    let signed = sign_in(&mut world);
    let second = inventory(&mut world, signed);
    assert_eq!(second.newest, 2);
    for line in &second.settings {
        assert_eq!(line.last, if line.setting == settings::Setting::Template(1) { 2 } else { 1 });
    }
}
#[test]
fn settings_inventory_is_unknown_without_a_deployment_role() {
    let mut world = jig_core_world::stories::reads::world(SEED);
    jig_core_world::reads::ask(&mut world, 1, reads::Read::Settings);
    assert_eq!(jig_core_world::reads::latest(&world), &reads::Answer::None(reads::Why::Unknown));
    jig_core_world::reads::ask(&mut world, 2, reads::Read::Deployment);
    assert_eq!(jig_core_world::reads::latest(&world), &reads::Answer::None(reads::Why::Unknown));
}
#[test]
fn a_setting_reads_whole_only_from_the_newest() {
    let mut world = released_history();
    let signed = sign_in(&mut world);
    let reads::Answer::Setting(value) = client_read(
        &mut world,
        signed,
        reads::Read::Setting { setting: settings::Setting::Template(1), revision: 6, proposal: None },
    ) else {
        panic!("whole template")
    };
    let settings::Value::Template(template) = value.value else { panic!("template value") };
    assert_eq!(template.grace, Some(Duration::from_secs(10)));
    assert_eq!(
        client_read(
            &mut world,
            signed,
            reads::Read::Setting { setting: settings::Setting::Template(1), revision: 5, proposal: None }
        ),
        reads::Answer::None(reads::Why::Unknown)
    );
    assert_eq!(
        client_read(
            &mut world,
            signed,
            reads::Read::Setting { setting: settings::Setting::Template(999), revision: 6, proposal: None }
        ),
        reads::Answer::None(reads::Why::Unknown)
    );
}
#[test]
fn revision_reads_load_held_and_history_records_and_name_the_forgotten_cutoff() {
    let mut world = released_history();
    let signed = sign_in(&mut world);
    for number in [6, 5, 4] {
        let reads::Answer::Revision(value) = client_read(&mut world, signed, reads::Read::Revision { number }) else {
            panic!("revision read")
        };
        let reads::RevisionAnswer::Revision { revision, .. } = *value else { panic!("kept revision") };
        assert_eq!(revision.head.number, number);
        assert_eq!(revision.head.by, settings::Decider::Release);
        assert_eq!(revision.items.len(), 1);
    }
    assert_eq!(
        client_read(&mut world, signed, reads::Read::Revision { number: 3 }),
        reads::Answer::Revision(Box::new(reads::RevisionAnswer::Forgotten { oldest: 4 }))
    );
    assert_eq!(
        client_read(&mut world, signed, reads::Read::Revision { number: 7 }),
        reads::Answer::Revision(Box::new(reads::RevisionAnswer::Unknown))
    );
    world.erase(key(settings::Key::History(5)));
    assert_eq!(
        client_read(&mut world, signed, reads::Read::Revision { number: 5 }),
        reads::Answer::Revision(Box::new(reads::RevisionAnswer::Forgotten { oldest: 4 }))
    );
}
#[test]
fn revisions_pages_run_newest_first_and_end_with_forgotten() {
    let mut world = released_history();
    let signed = sign_in(&mut world);
    let reads::Answer::Revisions(first) = client_read(&mut world, signed, reads::Read::Revisions { before: None })
    else {
        panic!("history page")
    };
    assert_eq!(first.rows.iter().map(|row| row.head.number).collect::<Vec<_>>(), [6, 5]);
    assert_eq!(first.next, Some(5));
    assert!(!first.forgotten);
    let reads::Answer::Revisions(last) = client_read(&mut world, signed, reads::Read::Revisions { before: first.next })
    else {
        panic!("last history page")
    };
    assert_eq!(last.rows.iter().map(|row| row.head.number).collect::<Vec<_>>(), [4]);
    assert_eq!(last.next, None);
    assert!(last.forgotten);
}
fn ready_host(world: &mut World) {
    world.send(root::Event::Core(core::Event::Fleet(fleet::Event::Hello {
        worker: 0,
        channel: Token::new(7),
        hello: fleet::Hello {
            bounds: fixtures::bounds(),
            stop_bound: Duration::from_millis(1),
            slots: 1,
            workstreams: Box::new([]),
            hosting: Box::new([]),
        },
    })));
}
fn watched_holders(world: &World) -> Vec<u32> {
    let mut holders = Vec::new();
    for (_, delivery) in &world.client_peer.as_ref().expect("settings story fixture").delivered {
        let root::Delivery::Core(core::Held::Delivery { document, .. }) = delivery else { continue };
        let documents::Document::Settings(value) = document.as_ref() else { continue };
        holders.push(value.held.get(0).expect("settings story fixture").holders);
    }
    holders
}
#[test]
fn settings_watch_marks_when_an_agent_task_is_added_and_ends() {
    const WATCHER: Token = Token::new(37_002);
    let mut world = start(2);
    let signed = sign_in(&mut world);
    world.send(root::Event::Core(core::Event::Watch {
        watcher: WATCHER,
        sign_in: signed,
        subject: documents::Watched::Settings,
        window: documents::Window { rows: 1, after: None },
    }));
    assert_eq!(watched_holders(&world), [0]);
    world.send(root::Event::Core(core::Event::Delivered { watcher: WATCHER }));
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 100, next: None }));
    ready_host(&mut world);
    let chat = fixtures::chat(&world, 1, Box::from(b"hello".as_slice()));
    world.send(root::Event::Core(core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(SEED)),
        sign_in: signed,
        key: [61; 16],
        ask: chat,
    })));
    assert_eq!(watched_holders(&world), [0, 1]);
    world.send(root::Event::Core(core::Event::Delivered { watcher: WATCHER }));
    let task = world.assigned.as_ref().expect("live agent task").task;
    world.send(root::Event::Core(core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(SEED)),
        sign_in: signed,
        key: [62; 16],
        ask: people::Ask::Cancel { project: 1, task, reason: Box::from(b"finished".as_slice()), grace: None },
    })));
    world.end(jig_domain_tasks::End::Parked, 0);
    assert_eq!(watched_holders(&world), [0, 1, 0]);
}

#[test]
fn a_removed_setting_keeps_its_inventory_line_and_reads_unknown() {
    let mut world = start(2);
    world.seed(Box::new([record(settings::Record::Changed {
        setting: settings::Setting::Template(2),
        changed: settings::Changed { value: None, revision: 1 },
    })]));
    restart(&mut world, configuration().0);
    let signed = sign_in(&mut world);
    let document = inventory(&mut world, signed);
    let line = document
        .settings
        .iter()
        .find(|line| line.setting == settings::Setting::Template(2))
        .expect("settings story fixture");
    assert_eq!(line.follows, documents::Follows::Change);
    assert_eq!(line.last, 1);
    assert_eq!(
        client_read(
            &mut world,
            signed,
            reads::Read::Setting { setting: settings::Setting::Template(2), revision: 1, proposal: None }
        ),
        reads::Answer::None(reads::Why::Unknown)
    );
}
#[test]
fn revision_item_previews_state_how_many_were_cut() {
    let (config, mut limits) = configuration();
    limits.core.documents.lines = 1;
    let mut world = World::first_start_with_accounts(config, &limits);
    world.client_peer = Some(ClientPeer::default());
    let mut config = configuration().0;
    for template in &mut config.core.defaults.templates {
        template.name = Box::from(b"new name".as_slice());
    }
    restart(&mut world, config);
    let signed = sign_in(&mut world);
    let reads::Answer::Revisions(page) = client_read(&mut world, signed, reads::Read::Revisions { before: None })
    else {
        panic!("revision page")
    };
    let row = page.rows.get(0).expect("settings story fixture");
    assert_eq!(row.head.number, 2);
    assert_eq!(row.items.len(), 1);
    assert_eq!(row.more, 1);
    assert_eq!(page.next, None);
    assert!(!page.forgotten);
}
#[test]
fn revision_loads_answer_busy_when_the_load_slab_is_full() {
    let mut world = start(2);
    let signed = sign_in(&mut world);
    world.sign_in = Some(signed);
    world.client_peer = None;
    let mut parties = jig_core_world::reads::Parties::new(world.store.applied);
    parties.hold_loads = true;
    world.reads = Some(parties);
    // The small profile has four load slots (configuration.md, section 6).
    for number in 1..=5 {
        jig_core_world::reads::ask(&mut world, number, reads::Read::Revision { number: 1 });
    }
    assert_eq!(jig_core_world::reads::latest(&world), &reads::Answer::None(reads::Why::Busy));
    jig_core_world::reads::complete(&mut world);
    assert_eq!(world.reads.as_ref().expect("settings story fixture").receipts.len(), 5);
}

#[test]
fn a_revision_item_past_the_single_read_budget_answers_its_encoded_size() {
    use jig_client_protocol_wire::v19 as w;
    use jig_protocol_engine_clients::translate::settings as wire;
    const SINGLE_READ_BYTES: u64 = 4096;
    const NAME_BYTES: usize = 4096;
    // v19 SettingValue tag + this fixture's Template: 46 fixed bytes + its 4096-byte name.
    const ENCODED_TEMPLATE_BYTES: u64 = 4142;
    let (config, mut limits) = configuration();
    limits.core.load_bytes = SINGLE_READ_BYTES;
    let mut world = World::first_start_with_accounts(config, &limits);
    world.client_peer = Some(ClientPeer::default());
    let mut config = configuration().0;
    config.core.defaults.templates[0].name = vec![b'x'; NAME_BYTES].into_boxed_slice();
    restart(&mut world, config);
    let signed = sign_in(&mut world);
    let reads::Answer::Revision(answer) = client_read(&mut world, signed, reads::Read::Revision { number: 2 }) else {
        panic!("revision read")
    };
    let reads::RevisionAnswer::Revision { value_bytes, .. } = answer.as_ref() else { panic!("revision") };
    assert_eq!(*value_bytes, SINGLE_READ_BYTES);
    let w::RevisionAnswer::Revision(document) = wire::revision(&answer, &[], &w::CEILINGS).expect("revision answer")
    else {
        panic!("revision record")
    };
    let mut pieces = skein_lib::Reader::new(document.items());
    let length = pieces.u32().expect("item length");
    let bytes = pieces.bytes(length).expect("item bytes");
    let item = w::RevisionItem::decode(&w::CEILINGS, &mut skein_lib::Reader::new(bytes)).expect("revision item");
    let w::RevisionEdit::TooLarge(size) = item.edit() else { panic!("item size") };
    assert_eq!(size.bytes(), ENCODED_TEMPLATE_BYTES);
}

fn held_goal(world: &mut World, signed: u64, request: u8) -> u64 {
    let mut goal = fixtures::chat(world, 1, b"Keep working on this goal".as_slice().into());
    let people::Ask::Create { batch, .. } = &mut goal else { panic!("goal creation") };
    batch[0].flags.conversation = false;
    batch[0].flags.listed = true;
    batch[0].budget = people::Budget::Named;
    batch[0].authority.spend = 40;
    let people::Reply::Outcome(people::Outcome::Created { first, .. }) = ask(world, signed, request, goal) else {
        panic!("goal admitted")
    };
    first
}
fn held_run() -> (World, u64, u64) {
    held_run_with_history(64)
}
fn held_run_with_history(history: u32) -> (World, u64, u64) {
    let (mut config, mut limits) = configuration();
    limits.core.settings.history = history;
    config.core.defaults.rules.maximum_run_spend = 20;
    let mut world = World::first_start_with_accounts(config, &limits);
    world.client_peer = Some(ClientPeer::default());
    let signed = sign_in(&mut world);
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 100, next: None }));
    ready_host(&mut world);
    let goal = held_goal(&mut world, signed, 101);
    assert_eq!(world.assigned.as_ref().expect("live goal").task, goal);
    (world, signed, goal)
}
fn instructions_edit(world: &World, instructions: &[u8]) -> settings::Item {
    let mut charter = world.domain.settings().charter(1).expect("offered charter").clone();
    charter.policy.instructions = instructions.into();
    settings::Item {
        setting: settings::Setting::Charter(1),
        edit: settings::Edit::Set(settings::Value::Charter(Box::new(charter))),
    }
}
fn change_instructions(world: &mut World, signed: u64) {
    let item = instructions_edit(world, b"Instructions from revision two.");
    changed(&change(world, signed, 102, 1, Box::new([item])), 2);
}
#[test]
fn a_charter_changed_while_its_goal_runs_keeps_the_run_and_the_goal_on_their_version() {
    let (mut world, signed, goal) = held_run();
    let old_policy = world.assigned.as_ref().expect("live goal").run.policy.clone();
    let assignments = world.assignments.clone();
    change_instructions(&mut world, signed);
    assert_eq!(world.assignments, assignments);
    assert_eq!(world.assigned.as_ref().expect("unchanged assignment").run.policy, old_policy);
    assert!(world.cancellations.is_empty());
    world.end(jig_domain_tasks::End::Parked, 0);
    let reply =
        ask(&mut world, signed, 103, people::Ask::Say { project: 1, task: goal, words: b"Continue".as_slice().into() });
    assert!(matches!(reply, people::Reply::Outcome(people::Outcome::Said { .. })), "{reply:?}");
    let next = world.assigned.as_ref().expect("goal's next run");
    assert_eq!(next.task, goal);
    assert!(next.attempt > assignments[0].1);
    assert_eq!(next.run.policy.instructions, old_policy.instructions);
    assert_eq!(fixtures::live_row(&world, goal).settings, jig_domain_tasks::SettingsRevision { task: 1, run: Some(1) });
    fixtures::finish_chat(&mut world, 0, b"Done");
    let new_task = held_goal(&mut world, signed, 104);
    assert_eq!(world.assigned.as_ref().expect("new task").task, new_task);
    assert_eq!(
        world.assigned.as_ref().expect("new task").run.policy.instructions.as_ref(),
        b"Instructions from revision two."
    );
    assert_eq!(fixtures::live_row(&world, new_task).settings.task, 2);
}
#[test]
fn a_delegate_is_made_on_the_newest_whatever_its_parents_revision() {
    let (mut world, signed, goal) = held_run();
    let attempt = world.assigned.as_ref().expect("goal run").attempt;
    change_instructions(&mut world, signed);
    let member = fixtures::member(&world, jig_domain_tasks::Executor::Agent { charter: 1 }, b"A fresh delegate");
    let delegate = fixtures::delegate(&mut world, (goal, attempt), 1, member);
    assert_eq!(fixtures::live_row(&world, delegate).settings.task, 2);
    assert_eq!(fixtures::live_row(&world, goal).settings.task, 1);
    world.end(jig_domain_tasks::End::Parked, 0);
    let assigned = world.assigned.as_ref().expect("delegate run");
    assert_eq!(assigned.task, delegate);
    assert_eq!(assigned.run.policy.instructions.as_ref(), b"Instructions from revision two.");
    assert_eq!(fixtures::live_row(&world, delegate).settings.run, Some(2));
}
#[test]
fn a_held_revisions_charter_reads_whole_and_the_inventory_counts_its_holders() {
    let (mut world, signed, _) = held_run();
    change_instructions(&mut world, signed);
    let reads::Answer::Setting(value) = client_read(
        &mut world,
        signed,
        reads::Read::Setting { setting: settings::Setting::Charter(1), revision: 1, proposal: None },
    ) else {
        panic!("held charter")
    };
    let settings::Value::Charter(charter) = value.value else { panic!("charter value") };
    assert_eq!(charter.policy.instructions.as_ref(), b"Answer within the granted authority.");
    let document = inventory(&mut world, signed);
    assert_eq!(document.held.iter().map(|row| (row.head.number, row.holders)).collect::<Vec<_>>(), [(1, 1), (2, 0)]);
    assert_eq!(
        client_read(
            &mut world,
            signed,
            reads::Read::Setting { setting: settings::Setting::Template(1), revision: 1, proposal: None }
        ),
        reads::Answer::None(reads::Why::Unknown)
    );
    let reads::Answer::Revision(value) = client_read(&mut world, signed, reads::Read::Revision { number: 1 }) else {
        panic!("held record")
    };
    assert!(matches!(*value, reads::RevisionAnswer::Revision { revision, .. } if revision.head.number == 1));
    let reads::Answer::Revisions(page) = client_read(&mut world, signed, reads::Read::Revisions { before: None })
    else {
        panic!("held history page")
    };
    assert_eq!(page.rows.iter().map(|row| row.head.number).collect::<Vec<_>>(), [2, 1]);
}
#[test]
fn a_revision_leaves_memory_as_its_last_holder_ends() {
    let (mut world, signed, goal) = held_run();
    change_instructions(&mut world, signed);
    assert!(world.store.rows.contains_key(&key(settings::Key::Held(1))));
    assert!(world.store.rows.contains_key(&key(settings::Key::Charter { charter: 1, revision: 1 })));
    fixtures::finish_chat(&mut world, 0, b"Done");
    let ended = root::Key::Core(core::Key::Tasks(jig_domain_tasks::Key::Ended(goal)));
    let decision = world
        .commits
        .iter()
        .find(|writes| writes.iter().any(|write| matches!(write, root::Write::Save(record) if record.key() == ended)))
        .expect("ending decision");
    assert!(decision.iter().any(|write| matches!(write, root::Write::CopySettingsRevision(1))));
    assert_eq!(
        decision
            .iter()
            .filter(|write| matches!(write, root::Write::Save(record) if record.key() == key(settings::Key::Extent)))
            .count(),
        1
    );
    assert!(
        decision
            .iter()
            .any(|write| matches!(write, root::Write::Erase(address) if *address == key(settings::Key::Held(1))))
    );
    assert!(decision.iter().any(|write| matches!(write, root::Write::Erase(address) if *address == key(settings::Key::Charter { charter: 1, revision: 1 }))));
    assert!(!world.store.rows.contains_key(&key(settings::Key::Held(1))));
    assert!(!world.store.rows.contains_key(&key(settings::Key::Charter { charter: 1, revision: 1 })));
    let reads::Answer::Historical(past) = client_read(&mut world, signed, reads::Read::Task { task: goal }) else {
        panic!("ended task")
    };
    let reads::Past::Task { document, .. } = past.value else { panic!("ended task document") };
    assert!(document.instructions.is_none(), "a released revision supplies no charter fields");
    assert_eq!(revision(&world, 1, true).head.number, 1);
    assert_eq!(inventory(&mut world, signed).held.iter().map(|row| row.head.number).collect::<Vec<_>>(), [2]);
    assert_eq!(
        client_read(
            &mut world,
            signed,
            reads::Read::Setting { setting: settings::Setting::Charter(1), revision: 1, proposal: None }
        ),
        reads::Answer::None(reads::Why::Unknown)
    );
    let reads::Answer::Revision(value) = client_read(&mut world, signed, reads::Read::Revision { number: 1 }) else {
        panic!("archived record")
    };
    assert!(matches!(*value, reads::RevisionAnswer::Revision { revision, .. } if revision.head.number == 1));
}
#[test]
fn held_revisions_past_their_limit_refuse_the_change_naming_the_oldest_and_its_tasks() {
    let (config, mut limits) = configuration();
    limits.core.settings.revisions = 1;
    peers::journal(&mut limits);
    let mut world = World::first_start_with_accounts(config, &limits);
    world.client_peer = Some(ClientPeer::default());
    let signed = sign_in(&mut world);
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 100, next: None }));
    ready_host(&mut world);
    let first = held_goal(&mut world, signed, 101);
    change_instructions(&mut world, signed);
    let _second = held_goal(&mut world, signed, 103);
    let before = settings_rows(&world);
    let item = instructions_edit(&world, b"One revision too many.");
    assert_eq!(
        change(&mut world, signed, 104, 2, Box::new([item])),
        people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Full {
            revision: 1,
            tasks: Box::new([first]),
            more: 0
        }))
    );
    assert_eq!(settings_rows(&world), before);
    assert_eq!(world.domain.settings().newest(), 2);
}

#[test]
fn a_held_record_below_the_history_cutoff_is_read_then_erased_without_a_copy() {
    let (mut world, signed, goal) = held_run_with_history(1);
    change_instructions(&mut world, signed);
    for (request, against) in [(103, 2), (104, 3)] {
        let item = template_edit(&world, u64::from(against) + 5);
        changed(&change(&mut world, signed, request, against, Box::new([item])), against + 1);
    }
    let reads::Answer::Revision(value) = client_read(&mut world, signed, reads::Read::Revision { number: 1 }) else {
        panic!("held record below cutoff")
    };
    assert!(matches!(*value, reads::RevisionAnswer::Revision { revision, .. } if revision.head.number == 1));
    let reads::Answer::Revisions(page) = client_read(&mut world, signed, reads::Read::Revisions { before: Some(3) })
    else {
        panic!("held page below cutoff")
    };
    assert_eq!(page.rows.iter().map(|row| row.head.number).collect::<Vec<_>>(), [1]);
    let before = world.commits.len();
    fixtures::finish_chat(&mut world, 0, b"Done");
    assert!(!world.store.rows.contains_key(&key(settings::Key::Held(1))));
    assert!(!world.store.rows.contains_key(&key(settings::Key::History(1))));
    assert!(
        !world.commits[before..].iter().flatten().any(|write| matches!(write, root::Write::CopySettingsRevision(1)))
    );
    assert!(!world.store.rows.contains_key(&root::Key::Core(core::Key::Tasks(jig_domain_tasks::Key::Live(goal)))));
}
#[test]
fn held_versions_past_their_limit_refuse_the_change_without_writing_settings() {
    let (config, mut limits) = configuration();
    limits.core.settings.versions = 1;
    peers::journal(&mut limits);
    let mut world = World::first_start_with_accounts(config, &limits);
    world.client_peer = Some(ClientPeer::default());
    let signed = sign_in(&mut world);
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 100, next: None }));
    ready_host(&mut world);
    let first = held_goal(&mut world, signed, 101);
    change_instructions(&mut world, signed);
    let _second = held_goal(&mut world, signed, 103);
    let before = settings_rows(&world);
    let item = instructions_edit(&world, b"One version too many.");
    assert_eq!(
        change(&mut world, signed, 104, 2, Box::new([item])),
        people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Full {
            revision: 1,
            tasks: Box::new([first]),
            more: 0
        }))
    );
    assert_eq!(settings_rows(&world), before);
}

#[test]
fn move_to_newest_of_the_newest_moves_nothing_and_an_unknown_revision_is_refused() {
    let mut world = start(64);
    let signed = sign_in(&mut world);
    let before = settings_rows(&world);
    let moved = people::Reply::Outcome(people::Outcome::MovedToNewest { newest: 1, moved: 0, stayed: 0 });
    assert_eq!(ask(&mut world, signed, 51, people::Ask::MoveToNewest { revision: 1 }), moved);
    assert_eq!(ask(&mut world, signed, 51, people::Ask::MoveToNewest { revision: 1 }), moved);
    assert_eq!(
        ask(&mut world, signed, 52, people::Ask::MoveToNewest { revision: 7 }),
        people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Unknown))
    );
    world.send(root::Event::Core(core::Event::People(people::Event::Roles {
        project: people::DEPLOYMENT,
        holdings: Box::new([people::Holding { person: OWNER, role: people::Role::Member }]),
    })));
    assert_eq!(
        ask(&mut world, signed, 53, people::Ask::MoveToNewest { revision: 1 }),
        people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Role { held: Some(people::Role::Member) }))
    );
    assert_eq!(settings_rows(&world), before);
    assert_eq!(world.domain.settings().newest(), 1);
}

fn moving_change(world: &mut World, signed: u64, request: u8, against: u32, item: settings::Item) {
    world.send(root::Event::Core(core::Event::ChangeSettings {
        reply_to: ReplyTo::new(Token::new(SEED)),
        sign_in: signed,
        key: [request; 16],
        digest: [request; 32],
        change: Box::new(settings::Change { against, items: Box::new([item]), running: settings::Running::Moves }),
    }));
    let (_, root::Delivery::Core(core::Held::PeopleReply { reply, .. })) =
        world.client_peer.as_mut().expect("client").delivered.pop().expect("settings reply")
    else {
        panic!("settings reply")
    };
    changed(&reply, against + 1);
}

#[test]
fn a_change_moving_running_work_moves_the_waiting_goal_and_keeps_the_live_run() {
    let (mut world, signed, parked) = held_run();
    world.end(jig_domain_tasks::End::Parked, 0);
    let live = held_goal(&mut world, signed, 103);
    let assignments = world.assignments.clone();
    let old_policy = world.assigned.as_ref().expect("live run").run.policy.clone();
    let item = instructions_edit(&world, b"The next run's instructions.");
    moving_change(&mut world, signed, 104, 1, item);
    assert_eq!(world.assignments, assignments);
    assert_eq!(world.assigned.as_ref().expect("live run").run.policy, old_policy);
    assert_eq!(fixtures::live_row(&world, parked).settings, jig_domain_tasks::SettingsRevision { task: 2, run: None });
    assert_eq!(fixtures::live_row(&world, live).settings, jig_domain_tasks::SettingsRevision { task: 2, run: Some(1) });
    assert_eq!(
        inventory(&mut world, signed).held.iter().map(|row| (row.head.number, row.holders)).collect::<Vec<_>>(),
        [(1, 1), (2, 2)]
    );
    let decision = world
        .commits
        .iter()
        .find(|writes| {
            writes
                .iter()
                .any(|write| matches!(write, root::Write::Save(record) if record.key() == key(settings::Key::Held(2))))
        })
        .expect("settings decision");
    for number in [parked, live] {
        assert!(decision.iter().any(|write| matches!(write, root::Write::Save(root::Record::Core(core::Record::Tasks(jig_domain_tasks::Stored::Live(row)))) if row.number == number && row.settings.task == 2)));
    }
    world.end(jig_domain_tasks::End::Parked, 0);
    assert!(!world.store.rows.contains_key(&key(settings::Key::Held(1))));
    assert_eq!(revision(&world, 1, true).head.number, 1);
    let reply = ask(
        &mut world,
        signed,
        105,
        people::Ask::Say { project: 1, task: parked, words: b"Continue".as_slice().into() },
    );
    assert!(matches!(reply, people::Reply::Outcome(people::Outcome::Said { .. })), "{reply:?}");
    let next = world.assigned.as_ref().expect("parked goal's next run");
    assert_eq!(next.task, parked);
    assert_eq!(next.run.policy.instructions.as_ref(), b"The next run's instructions.");
    assert_eq!(fixtures::live_row(&world, parked).settings.run, Some(2));
}

#[test]
fn move_to_newest_moves_a_held_revisions_tasks_and_releases_it() {
    let (mut world, signed, goal) = held_run();
    world.end(jig_domain_tasks::End::Parked, 0);
    change_instructions(&mut world, signed);
    let before = world.commits.len();
    assert_eq!(
        ask(&mut world, signed, 103, people::Ask::MoveToNewest { revision: 1 }),
        people::Reply::Outcome(people::Outcome::MovedToNewest { newest: 2, moved: 1, stayed: 0 })
    );
    assert_eq!(fixtures::live_row(&world, goal).settings, jig_domain_tasks::SettingsRevision { task: 2, run: None });
    let decision = world.commits[before..]
        .iter()
        .find(|writes| writes.iter().any(|write| matches!(write, root::Write::CopySettingsRevision(1))))
        .expect("move releases revision in its decision");
    assert!(
        decision
            .iter()
            .any(|write| matches!(write, root::Write::Erase(address) if *address == key(settings::Key::Held(1))))
    );
    assert!(decision.iter().any(|write| matches!(write, root::Write::Save(root::Record::Core(core::Record::Tasks(jig_domain_tasks::Stored::Live(row)))) if row.number == goal && row.settings.task == 2)));
    assert!(!world.store.rows.contains_key(&key(settings::Key::Held(1))));
    assert_eq!(revision(&world, 1, true).head.number, 1);
    let reply =
        ask(&mut world, signed, 104, people::Ask::Say { project: 1, task: goal, words: b"Continue".as_slice().into() });
    assert!(matches!(reply, people::Reply::Outcome(people::Outcome::Said { .. })), "{reply:?}");
    let next = world.assigned.as_ref().expect("goal's next run");
    assert_eq!(next.task, goal);
    assert_eq!(next.run.policy.instructions.as_ref(), b"Instructions from revision two.");
}

#[test]
fn a_move_uses_the_room_freed_by_its_tasks_for_heads_and_versions() {
    let (mut config, mut limits) = configuration();
    limits.core.settings.revisions = 1;
    limits.core.settings.versions = 1;
    config.core.defaults.rules.maximum_run_spend = 20;
    peers::journal(&mut limits);
    let mut world = World::first_start_with_accounts(config, &limits);
    world.client_peer = Some(ClientPeer::default());
    let signed = sign_in(&mut world);
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 100, next: None }));
    ready_host(&mut world);
    let first = held_goal(&mut world, signed, 101);
    world.end(jig_domain_tasks::End::Parked, 0);
    change_instructions(&mut world, signed);
    let second = held_goal(&mut world, signed, 103);
    let item = instructions_edit(&world, b"Revision three.");
    moving_change(&mut world, signed, 104, 2, item);
    for task in [first, second] {
        assert_eq!(fixtures::live_row(&world, task).settings.task, 3);
    }
    assert_eq!(fixtures::live_row(&world, second).settings.run, Some(2));
    assert!(!world.store.rows.contains_key(&key(settings::Key::Held(1))));
    assert!(!world.store.rows.contains_key(&key(settings::Key::Charter { charter: 1, revision: 1 })));
    assert!(world.store.rows.contains_key(&key(settings::Key::Held(2))));
    assert!(world.store.rows.contains_key(&key(settings::Key::Charter { charter: 1, revision: 2 })));
    assert_eq!(
        inventory(&mut world, signed).held.iter().map(|row| (row.head.number, row.holders)).collect::<Vec<_>>(),
        [(2, 1), (3, 2)]
    );
}

fn held_restart_configuration() -> root::Config {
    let mut config = configuration().0;
    config.core.defaults.rules.maximum_run_spend = 20;
    config
}

fn runs_after_held_restart(world: &mut World, goal: u64, newest_instructions: &[u8]) {
    let signed = sign_in(world);
    let document = inventory(world, signed);
    assert_eq!(document.held.iter().map(|row| (row.head.number, row.holders)).collect::<Vec<_>>(), [(1, 1), (2, 0)]);
    world.assigned = None;
    resume_host_after_restart(world);
    let reply =
        ask(world, signed, 103, people::Ask::Say { project: 1, task: goal, words: b"Continue".as_slice().into() });
    assert!(matches!(reply, people::Reply::Outcome(people::Outcome::Said { .. })), "{reply:?}");
    let next = world.assigned.as_ref().expect("restored goal's next run");
    assert_eq!(next.task, goal);
    assert_eq!(next.run.policy.instructions.as_ref(), b"Answer within the granted authority.");
    assert_eq!(fixtures::live_row(world, goal).settings.run, Some(1));
    fixtures::finish_chat(world, 0, b"Done");
    let fresh = held_goal(world, signed, 104);
    let next = world.assigned.as_ref().expect("new goal's run");
    assert_eq!(next.task, fresh);
    assert_eq!(next.run.policy.instructions.as_ref(), newest_instructions);
    assert_eq!(fixtures::live_row(world, fresh).settings.run, Some(2));
}

#[test]
fn a_restart_keeps_an_old_revision_held_and_its_goal_runs_on_its_version() {
    let (mut world, signed, goal) = held_run();
    seed_restart_accounts(&mut world);
    world.end(jig_domain_tasks::End::Parked, 0);
    change_instructions(&mut world, signed);
    restart(&mut world, held_restart_configuration());
    runs_after_held_restart(&mut world, goal, b"Instructions from revision two.");
}

#[test]
fn a_release_at_a_restart_keeps_the_old_default_for_the_revision_held() {
    let (mut world, _, goal) = held_run();
    seed_restart_accounts(&mut world);
    world.end(jig_domain_tasks::End::Parked, 0);
    let mut config = held_restart_configuration();
    config.core.defaults.charters[0].policy.instructions = b"Released default instructions".as_slice().into();
    restart(&mut world, config);
    assert_eq!(revision(&world, 2, false).head.by, settings::Decider::Release);
    let root::Record::Core(core::Record::Core(core::CoreRecord::Settings(settings::Record::Charter { value, .. }))) =
        &world.store.rows[&key(settings::Key::Charter { charter: 1, revision: 1 })]
    else {
        panic!("kept default version")
    };
    assert_eq!(
        value.charter.as_ref().expect("the old default is present").policy.instructions.as_ref(),
        b"Answer within the granted authority."
    );
    // The saved default stamp must keep the next start on the same release.
    let mut config = held_restart_configuration();
    config.core.defaults.charters[0].policy.instructions = b"Released default instructions".as_slice().into();
    restart(&mut world, config);
    assert_eq!(world.domain.settings().newest(), 2);
    runs_after_held_restart(&mut world, goal, b"Released default instructions");
}

#[test]
fn held_revisions_past_a_lowered_limit_refuse_the_start_naming_the_limit() {
    let (config, mut limits) = configuration();
    limits.core.settings.revisions = 2;
    peers::journal(&mut limits);
    let mut world = World::first_start_with_accounts(config, &limits);
    world.client_peer = Some(ClientPeer::default());
    let signed = sign_in(&mut world);
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 100, next: None }));
    ready_host(&mut world);
    for number in 1..=2 {
        held_goal(&mut world, signed, 110 + number);
        world.end(jig_domain_tasks::End::Parked, 0);
        let item = instructions_edit(&world, &[number]);
        changed(&change(&mut world, signed, 120 + number, u32::from(number), Box::new([item])), u32::from(number) + 1);
    }
    let before = world.store.rows.clone();
    let commits = world.store.applied;
    limits.core.settings.revisions = 1;
    peers::journal(&mut limits);
    world.restart_with_limits(configuration().0, limits);
    assert_eq!(
        world.domain.restore_failure(),
        Some(core::RestartRequest::HeldPastLimit { which: core::HeldLimit::Revisions, held: 2, limit: 1 })
    );
    assert!(world.stopped);
    assert!(!world.domain.ready());
    assert_eq!(world.store.rows, before);
    assert_eq!(world.store.applied, commits);
}

#[test]
fn an_unheld_task_or_run_revision_refuses_the_start_naming_the_task() {
    for run in [false, true] {
        let (mut world, _, goal) = held_run();
        world.end(jig_domain_tasks::End::Parked, 0);
        let mut row = fixtures::live_row(&world, goal).clone();
        if run {
            row.settings.run = Some(91);
        } else {
            row.settings.task = 91;
        }
        // This deliberately corrupt fixture violates the normal-world referee.
        // Record its observations while checking the engine's startup refusal.
        world.observer = jig_core_world::observations::Observer::recording(&held_restart_configuration());
        world.seed(Box::new([root::Record::Core(core::Record::Tasks(jig_domain_tasks::Stored::Live(Box::new(row))))]));
        let before = world.store.applied;
        world.restart_with(held_restart_configuration());
        assert_eq!(
            world.domain.restore_failure(),
            Some(core::RestartRequest::UnheldRevision { task: goal, revision: 91 })
        );
        assert!(world.stopped);
        assert_eq!(world.store.applied, before);
    }
}

fn resume_host_after_restart(world: &mut World) {
    ready_host(world);
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 2, budget: 100, next: None }));
}

fn seed_restart_accounts(world: &mut World) {
    // first_start_with_accounts preloads this fake's rows into the first core;
    // a cold start needs the same account fixture in the store.
    world.seed(fixtures::account_rows(&configuration().0.core.settings.llm_providers));
}

#[test]
fn a_release_past_either_held_limit_refuses_without_committing() {
    for which in [core::HeldLimit::Revisions, core::HeldLimit::Versions] {
        let (mut world, _, _) = held_run();
        seed_restart_accounts(&mut world);
        world.end(jig_domain_tasks::End::Parked, 0);
        let mut config = held_restart_configuration();
        config.core.defaults.charters[0].policy.instructions = b"A release that cannot fit".as_slice().into();
        let mut limits = configuration().1;
        match which {
            core::HeldLimit::Revisions => limits.core.settings.revisions = 0,
            core::HeldLimit::Versions => limits.core.settings.versions = 0,
        }
        peers::journal(&mut limits);
        let before = world.store.rows.clone();
        let commits = world.store.applied;
        world.restart_with_limits(config, limits);
        assert_eq!(
            world.domain.restore_failure(),
            Some(core::RestartRequest::HeldPastLimit { which, held: 1, limit: 0 })
        );
        assert!(world.stopped);
        assert!(!world.domain.ready());
        assert_eq!(world.store.applied, commits);
        assert_eq!(world.store.rows, before);
    }
}

#[test]
fn a_restored_run_must_have_a_charter_on_its_held_revision() {
    let (mut world, signed, goal) = held_run();
    change_instructions(&mut world, signed);
    let mut row = fixtures::live_row(&world, goal).clone();
    row.settings.task = 2;
    assert_eq!(row.settings.run, Some(1));
    world.seed(Box::new([
        root::Record::Core(core::Record::Tasks(jig_domain_tasks::Stored::Live(Box::new(row)))),
        record(settings::Record::Charter {
            charter: 1,
            revision: 1,
            value: settings::CharterVersion { charter: None },
        }),
    ]));
    let commits = world.store.applied;
    world.restart_with(held_restart_configuration());
    assert_eq!(
        world.domain.restore_failure(),
        Some(core::RestartRequest::Settings {
            failure: settings::Failure { setting: settings::Setting::Charter(1), fault: settings::Fault::Reference },
            changed: true,
        })
    );
    assert!(world.stopped);
    assert_eq!(world.store.applied, commits);
}

#[test]
fn a_goal_on_an_older_revision_runs_where_its_version_allows() {
    let (mut config, mut limits) = configuration();
    config.core.defaults.charters[0].hosts = fleet::Kinds::Engine;
    config.core.defaults.rules.maximum_run_spend = 20;
    limits.core.fleet.engine_slots = 1;
    peers::journal(&mut limits);
    let mut world = World::first_start_with_accounts(config, &limits);
    world.client_peer = Some(ClientPeer::default());
    let signed = sign_in(&mut world);
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 100, next: None }));
    world.send(root::Event::Core(core::Event::Fleet(fleet::Event::Hello {
        worker: 0,
        channel: Token::new(0),
        hello: fleet::Hello {
            bounds: fixtures::bounds(),
            stop_bound: Duration::from_millis(1),
            slots: 1,
            workstreams: Box::new([]),
            hosting: Box::new([]),
        },
    })));
    let goal = held_goal(&mut world, signed, 101);
    assert_eq!(world.assigned.as_ref().expect("first goal run").run.placement.kind, fleet::HostKind::Engine);
    let mut charter = world.domain.settings().charter(1).expect("offered charter").clone();
    charter.hosts = fleet::Kinds::Workers;
    changed(
        &change(
            &mut world,
            signed,
            102,
            1,
            Box::new([settings::Item {
                setting: settings::Setting::Charter(1),
                edit: settings::Edit::Set(settings::Value::Charter(Box::new(charter))),
            }]),
        ),
        2,
    );
    world.end(jig_domain_tasks::End::Parked, 0);
    let reply =
        ask(&mut world, signed, 103, people::Ask::Say { project: 1, task: goal, words: b"Continue".as_slice().into() });
    assert!(matches!(reply, people::Reply::Outcome(people::Outcome::Said { .. })), "{reply:?}");
    let next = world.assigned.as_ref().expect("old goal's next run");
    assert_eq!(next.task, goal);
    assert_eq!(
        next.run.placement,
        fleet::PlacementConstraint { channel: Token::new(0), kind: fleet::HostKind::Engine }
    );
    assert_eq!(fixtures::live_row(&world, goal).settings.run, Some(1));
    fixtures::finish_chat(&mut world, 0, b"Done");
    let count = world.assignments.len();
    let newest = held_goal(&mut world, signed, 104);
    assert_eq!(world.assignments.len(), count, "new work waits for a worker");
    assert_eq!(fixtures::live_row(&world, newest).settings.task, 2);
    ready_host(&mut world);
    let placed = world.assigned.as_ref().expect("new task runs on the arriving worker");
    assert_eq!(placed.task, newest);
    assert_eq!(placed.run.placement.kind, fleet::HostKind::Worker);
}

#[test]
fn a_template_only_change_places_a_run_by_the_original_charter_version() {
    let mut world = start(64);
    let signed = sign_in(&mut world);
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 100, next: None }));
    let item = template_edit(&world, 7);
    changed(&change(&mut world, signed, 101, 1, Box::new([item])), 2);
    ready_host(&mut world);
    let goal = held_goal(&mut world, signed, 102);
    assert_eq!(fixtures::live_row(&world, goal).settings.run, Some(2));
    let run = world.assigned.as_ref().expect("unchanged charter version (1, 1) is placed");
    assert_eq!(run.task, goal);
    assert_eq!(run.run.placement.kind, fleet::HostKind::Worker);
}

#[test]
fn the_bound_reached_its_named_tasks_moved_and_the_change_then_made() {
    let (mut config, mut limits) = configuration();
    limits.core.settings.revisions = 1;
    config.core.defaults.rules.maximum_run_spend = 20;
    peers::journal(&mut limits);
    let mut world = World::first_start_with_accounts(config, &limits);
    world.client_peer = Some(ClientPeer::default());
    let signed = sign_in(&mut world);
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 100, next: None }));
    ready_host(&mut world);
    let oldest = held_goal(&mut world, signed, 101);
    world.end(jig_domain_tasks::End::Parked, 0);
    change_instructions(&mut world, signed);
    let newest = held_goal(&mut world, signed, 103);
    let item = instructions_edit(&world, b"Revision three after the move.");
    let before = settings_rows(&world);
    assert_eq!(
        change(&mut world, signed, 104, 2, Box::new([item.clone()])),
        people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Full {
            revision: 1,
            tasks: Box::new([oldest]),
            more: 0,
        }))
    );
    assert_eq!(settings_rows(&world), before);
    assert_eq!(
        ask(&mut world, signed, 105, people::Ask::MoveToNewest { revision: 1 }),
        people::Reply::Outcome(people::Outcome::MovedToNewest { newest: 2, moved: 1, stayed: 0 })
    );
    assert_eq!(fixtures::live_row(&world, oldest).settings.task, 2);
    assert!(!world.store.rows.contains_key(&key(settings::Key::Held(1))));
    // A fresh request key retries the same edit after the named holders moved.
    changed(&change(&mut world, signed, 106, 2, Box::new([item])), 3);
    assert_eq!(world.domain.settings().newest(), 3);
    assert_eq!(fixtures::live_row(&world, newest).settings.run, Some(2));
}

#[test]
fn a_restart_adopts_a_live_run_on_its_old_version() {
    let (mut world, signed, goal) = held_run();
    let attempt = world.assigned.as_ref().expect("live run").attempt;
    let policy = world.assigned.as_ref().expect("live run").run.policy.clone();
    let claim = fixtures::live_row(&world, goal).phase.clone();
    change_instructions(&mut world, signed);
    seed_restart_accounts(&mut world);
    // The worker is a separate process, keeping its run through the engine restart.
    // Let its hello arrive while the cold store load is paused, before adoption.
    world.hold_restore_pages = true;
    let mut config = held_restart_configuration();
    config.core.workers_in_process = false;
    world.restart_with(config);
    world.send(root::Event::Core(core::Event::Fleet(fleet::Event::Hello {
        worker: 0,
        channel: Token::new(7),
        hello: fleet::Hello {
            bounds: fixtures::bounds(),
            stop_bound: Duration::from_millis(1),
            slots: 1,
            workstreams: Box::new([]),
            hosting: Box::new([fleet::Hosted {
                run: Token::new(goal),
                attempt: Token::new(attempt),
                phase: fleet::Phase::Active,
            }]),
        },
    })));
    world.finish_restore_pages();
    assert!(world.domain.ready());
    let signed = sign_in(&mut world);
    assert_eq!(fixtures::live_row(&world, goal).phase, claim);
    assert_eq!(fixtures::live_row(&world, goal).settings.run, Some(1));
    assert_eq!(world.assigned.as_ref().expect("adopted run").attempt, attempt);
    assert_eq!(world.assignments, [(goal, attempt)], "adoption makes no new assignment");
    assert_eq!(world.assigned.as_ref().expect("old policy").run.policy, policy);
    assert_eq!(
        inventory(&mut world, signed).held.iter().map(|row| (row.head.number, row.holders)).collect::<Vec<_>>(),
        [(1, 1), (2, 0)]
    );
    fixtures::finish_chat(&mut world, 0, b"Done after adoption");
    assert!(!world.store.rows.contains_key(&key(settings::Key::Held(1))));
}

fn two_story_charters() -> (root::Config, root::Limits) {
    let (mut config, limits) = configuration();
    config.core.defaults.rules.maximum_run_spend = 20;
    let mut second = config.core.defaults.charters[0].clone();
    second.number = 2;
    second.policy.instructions = b"The second charter's original instructions.".as_slice().into();
    config.core.defaults.charters = Box::new([config.core.defaults.charters[0].clone(), second]);
    let kinds: Box<[jig_domain_authority::Executor]> =
        Box::new([jig_domain_authority::Executor::Charter(1), jig_domain_authority::Executor::Charter(2)]);
    config.core.defaults.rules.ceiling.delegation.kinds.clone_from(&kinds);
    {
        let policy = config.core.defaults.policies.get_mut(&1).expect("future policy");
        policy.policy.ceiling.delegation.kinds.clone_from(&kinds);
        for role in &mut policy.policy.roles {
            role.authority.delegation.kinds.clone_from(&kinds);
        }
    }
    for project in &mut config.core.seed_projects {
        project.policy.policy.ceiling.delegation.kinds.clone_from(&kinds);
        for role in &mut project.policy.policy.roles {
            role.authority.delegation.kinds.clone_from(&kinds);
        }
    }
    (config, limits)
}

#[test]
fn a_delegation_to_a_removed_charter_is_refused_while_its_version_serves_its_task() {
    let (config, limits) = two_story_charters();
    let mut world = World::first_start_with_accounts(config, &limits);
    world.client_peer = Some(ClientPeer::default());
    let signed = sign_in(&mut world);
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 100, next: None }));
    ready_host(&mut world);
    let goal = held_goal(&mut world, signed, 101);
    let attempt = world.assigned.as_ref().expect("old goal run").attempt;
    let policy = world.assigned.as_ref().expect("old goal run").run.policy.clone();
    let item = instructions_edit(&world, b"Newest first charter instructions.");
    let items = remove_second_charter(&world, item);
    changed(&change(&mut world, signed, 102, 1, items), 2);
    let mut create = fixtures::chat(&world, 1, b"Removed charter".as_slice().into());
    let people::Ask::Create { batch, .. } = &mut create else { panic!("create") };
    batch[0].executor = people::TaskExecutor::Agent { charter: 2 };
    assert_eq!(
        ask(&mut world, signed, 103, create),
        people::Reply::Outcome(people::Outcome::Refused(people::Refusal::NotOffered))
    );
    let member = fixtures::member(&world, jig_domain_tasks::Executor::Agent { charter: 2 }, b"Removed charter");
    world.send(root::Event::Core(core::Event::DelegateValidated {
        to: ReplyTo::new(Token::new(9001)),
        key: core::CallKey { task: goal, attempt, completion: 1, position: 0 },
        batch: Box::new([member]),
        stubs: Box::new([]),
    }));
    assert!(world.domain.settings().charter(2).is_none());
    assert!(fixtures::live_row(&world, goal).delegates.is_empty());
    assert_eq!(fixtures::live_row(&world, goal).settings.run, Some(1));
    assert_eq!(world.assigned.as_ref().expect("goal still running").run.policy, policy);
    let answer = &world.answers.last().expect("delegation refused").2;
    assert!(
        matches!(
            answer,
            core::CallPart::DelegationRefused(jig_domain_tasks::Problem {
                why: jig_domain_tasks::Refusal::Executor,
                ..
            })
        ),
        "a removed charter is not offered by the newest revision: {answer:?}"
    );
}

fn remove_second_charter(world: &World, first: settings::Item) -> Box<[settings::Item]> {
    let mut rules = world.domain.settings().rules().clone();
    rules.ceiling.delegation.kinds = Box::new([jig_domain_authority::Executor::Charter(1)]);
    let mut policy = world.domain.settings().policy(1).expect("future policy").clone();
    policy.policy.ceiling.delegation.kinds.clone_from(&rules.ceiling.delegation.kinds);
    for role in &mut policy.policy.roles {
        role.authority.delegation.kinds.clone_from(&rules.ceiling.delegation.kinds);
    }
    Box::new([
        first,
        settings::Item { setting: settings::Setting::Charter(2), edit: settings::Edit::Removed },
        settings::Item {
            setting: settings::Setting::Rules,
            edit: settings::Edit::Set(settings::Value::Rules(Box::new(rules))),
        },
        settings::Item {
            setting: settings::Setting::Policy(1),
            edit: settings::Edit::Set(settings::Value::Policy(Box::new(policy))),
        },
    ])
}

#[test]
fn a_conversation_moves_at_its_next_turn_and_one_whose_charter_is_removed_stays() {
    let (config, limits) = two_story_charters();
    let mut world = World::first_start_with_accounts(config, &limits);
    world.client_peer = Some(ClientPeer::default());
    let signed = sign_in(&mut world);
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 100, next: None }));
    ready_host(&mut world);
    let mut conversations = Vec::new();
    for charter in 1..=2 {
        let mut chat = fixtures::chat(&world, 1, b"First turn".as_slice().into());
        let people::Ask::Create { batch, .. } = &mut chat else { panic!("chat") };
        batch[0].executor = people::TaskExecutor::Agent { charter };
        batch[0].flags.ended_by = people::EndedBy::Requester;
        batch[0].budget = people::Budget::Named;
        batch[0].authority.spend = 40;
        let reply = ask(&mut world, signed, 100 + u8::try_from(charter).expect("charter"), chat);
        let people::Reply::Outcome(people::Outcome::Created { first, .. }) = reply else {
            panic!("conversation created: {reply:?}")
        };
        assert_eq!(world.assigned.as_ref().expect("first turn").task, first);
        assert_eq!(fixtures::live_row(&world, first).settings.run, Some(1));
        conversations.push(first);
        conversation_answer(&mut world, b"First answer");
    }
    let item = instructions_edit(&world, b"The first conversation's new instructions.");
    let items = remove_second_charter(&world, item);
    changed(&change(&mut world, signed, 103, 1, items), 2);
    for (index, (revision, instructions)) in [
        (2, b"The first conversation's new instructions.".as_slice()),
        (1, b"The second charter's original instructions.".as_slice()),
    ]
    .into_iter()
    .enumerate()
    {
        let task = conversations[index];
        let previous_attempt = fixtures::live_row(&world, task).attempt;
        let reply = ask(
            &mut world,
            signed,
            104 + u8::try_from(index).expect("index"),
            people::Ask::Say { project: 1, task, words: b"Next turn".as_slice().into() },
        );
        assert!(matches!(reply, people::Reply::Outcome(people::Outcome::Said { .. })), "{reply:?}");
        let assignment = world.assigned.as_ref().expect("next turn");
        assert_eq!(assignment.task, task);
        assert!(assignment.attempt > previous_attempt);
        assert_eq!(assignment.run.policy.instructions.as_ref(), instructions);
        assert_eq!(
            fixtures::live_row(&world, task).settings,
            jig_domain_tasks::SettingsRevision { task: revision, run: Some(revision) }
        );
        conversation_answer(&mut world, b"Second answer");
    }
}

fn conversation_answer(world: &mut World, words: &[u8]) {
    let read = world.assigned.as_ref().expect("conversation run").messages.last().map(|word| word.number);
    world.turn(1, 0, read, words);
    fixtures::finish_chat(world, 0, words);
}

fn settings_task_document(world: &mut World, signed: u64, task: u64) -> documents::Task {
    let reads::Answer::Document(document) = client_read(world, signed, reads::Read::Task { task }) else {
        panic!("live task document")
    };
    let documents::Document::Task(document) = *document else { panic!("task document") };
    *document
}

#[test]
fn a_held_versions_documents_show_its_charter_and_nothing_once_released() {
    let (mut world, signed, goal) = held_run();
    let old_instructions = world.assigned.as_ref().expect("goal run").run.policy.instructions.clone();
    let llm = fixtures::live_row(&world, goal).llm.clone();
    let mut charter = world.domain.settings().charter(1).expect("charter").clone();
    assert_eq!(charter.hosts, fleet::Kinds::Workers);
    charter.hosts = fleet::Kinds::Engine;
    charter.policy.instructions = b"The newest instructions are different.".as_slice().into();
    changed(
        &change(
            &mut world,
            signed,
            102,
            1,
            Box::new([settings::Item {
                setting: settings::Setting::Charter(1),
                edit: settings::Edit::Set(settings::Value::Charter(Box::new(charter))),
            }]),
        ),
        2,
    );
    let document = settings_task_document(&mut world, signed, goal);
    assert_eq!(document.instructions.as_ref().expect("held instructions").bytes, old_instructions);
    assert_eq!(document.llm, llm);
    assert_eq!(document.settings, Some(documents::TaskSettings { task: 1, run: None, newest: 2 }));
    let review = goal_description_review(&mut world, signed, goal);
    let documents::ProposalAction::Amend { before, after, .. } = review.action else { panic!("amendment review") };
    assert_eq!(before.instructions.as_ref().expect("current fallback").bytes, old_instructions);
    assert_eq!(after.instructions.as_ref().expect("proposed fallback").bytes, old_instructions);
    world.end(jig_domain_tasks::End::Parked, 0);
    world.send(root::Event::Core(core::Event::Fleet(fleet::Event::Lost { channel: Token::new(7) })));
    world.send(root::Event::Core(core::Event::Fleet(fleet::Event::Hello {
        worker: 0,
        channel: Token::new(8),
        hello: fleet::Hello {
            bounds: fixtures::bounds(),
            stop_bound: Duration::from_secs(3600),
            slots: 1,
            workstreams: Box::new([]),
            hosting: Box::new([]),
        },
    })));
    let reply = ask(
        &mut world,
        signed,
        104,
        people::Ask::Say { project: 1, task: goal, words: b"Continue when a host is ready".as_slice().into() },
    );
    assert!(matches!(reply, people::Reply::Outcome(people::Outcome::Said { .. })), "{reply:?}");
    let document = settings_task_document(&mut world, signed, goal);
    assert!(
        matches!(
            document.run_wait,
            Some(documents::TaskRunWait::Refused(documents::TaskWaitRefused::Hello {
                kind: fleet::HostKind::Worker,
                why: fleet::HelloRefusal::StopBound { .. },
                ..
            }))
        ),
        "the held charter explains the worker refusal: {:?}",
        document.run_wait
    );
    assert_eq!(world.domain.settings().charter(1).expect("newest charter").hosts, fleet::Kinds::Engine);
    let reply = ask(
        &mut world,
        signed,
        105,
        people::Ask::Cancel {
            project: 1,
            task: goal,
            reason: b"The goal is no longer needed".as_slice().into(),
            grace: None,
        },
    );
    assert_eq!(reply, people::Reply::Outcome(people::Outcome::Cancelled { task: goal }));
    assert!(!world.store.rows.contains_key(&key(settings::Key::Held(1))));
    assert!(!world.store.rows.contains_key(&key(settings::Key::Charter { charter: 1, revision: 1 })));
    let reads::Answer::Historical(past) = client_read(&mut world, signed, reads::Read::Task { task: goal }) else {
        panic!("ended task")
    };
    let reads::Past::Task { document, .. } = past.value else { panic!("ended task document") };
    assert_eq!(document.executor, jig_domain_tasks::Executor::Agent { charter: 1 });
    assert_eq!(document.settings, Some(documents::TaskSettings { task: 1, run: None, newest: 2 }));
    assert!(document.instructions.is_none());
    assert!(document.run_wait.is_none());
    assert_eq!(document.llm, llm, "the LLM choice is retained task data");
}

fn goal_description_review(world: &mut World, signed: u64, goal: u64) -> documents::Proposal {
    let revision = fixtures::live_row(world, goal).revisions.spec;
    let mut amendment = fixtures::person_unchanged(b"Review the goal's description");
    amendment.spec = Some(people::Spec {
        asks: false,
        words: b"Proposed goal description".as_slice().into(),
        parameters: Box::new([]),
        inputs: Box::new([]),
    });
    let reply = ask(
        world,
        signed,
        103,
        people::Ask::Propose {
            project: 1,
            action: people::Action::Amend { task: goal, revision, amendment: Box::new(amendment) },
            reason: b"Review without replacing instructions".as_slice().into(),
        },
    );
    let proposal = proposed(&reply);
    proposal_document(world, signed, OWNER, proposal)
}
