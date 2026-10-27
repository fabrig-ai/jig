use jig_core_world::fixtures::two_projects;
use jig_core_world::{effects::World, world};
use jig_domain_authority as authority;
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{ReplyTo, Token};

fn ask(world: &mut World, key: u8, ask: people::Ask) {
    world.send(root::Event::Core(core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(8000 + u64::from(key))),
        sign_in: world.sign_in.expect("signed in owner"),
        key: [key; 16],
        ask,
    })));
}

fn ledger(world: &World, funder: tasks::Funder) -> &tasks::FundingRecord {
    let root::Record::Core(core::Record::Tasks(tasks::Stored::Ledger(row))) =
        &world.store.rows[&root::Key::Core(core::Key::Tasks(tasks::Key::Ledger(funder)))]
    else {
        panic!("funding ledger")
    };
    row
}

#[test]
fn two_projects_with_different_spends_fund_their_own_periods() {
    let (config, limits) = two_projects(601);
    let mut world = World::configured(601, false, config, limits);
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 2, budget: 800, next: None }));
    world.send(root::Event::Core(core::Event::Period { project: 2, period: 9, budget: 600, next: None }));
    world.restart_with(two_projects(601).0);
    ask(&mut world, 1, people::Ask::SetPool { project: 1, person: 1, budget: 100 });
    ask(&mut world, 2, people::Ask::SetPool { project: 2, person: 1, budget: 200 });
    let first = tasks::Funder::Period { project: 1, period: 2 };
    let second = tasks::Funder::Period { project: 2, period: 9 };
    assert_eq!(ledger(&world, first).numbers.budget, 800);
    assert_eq!(ledger(&world, second).numbers.budget, 600);
    assert_eq!(ledger(&world, tasks::Funder::Pool { project: 1, person: 1, period: 2 }).parent, Some(first));
    assert_eq!(ledger(&world, tasks::Funder::Pool { project: 2, person: 1, period: 9 }).parent, Some(second));
}

#[test]
fn a_chat_takes_the_most_that_fits_under_its_persons_role() {
    let mut config = world::config(602);
    world::edit_role(&mut config.core, |role| {
        role.authority.tools = authority::Tools(513);
        role.authority.budget.spend = 80;
        role.period_spend = 90;
        role.authority.delegation.tasks = 3;
    });
    let mut limits = world::limits();
    limits.core.tasks.tree_tasks = 4;
    jig_core_world::peers::journal(&mut limits);
    let mut world = World::configured(602, false, config, limits);
    let remainder_chat = jig_core_world::fixtures::chat(&world, 1, b"use the remainder".as_slice().into());
    ask(&mut world, 6, remainder_chat);
    let task = 2;
    let root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) =
        &world.store.rows[&root::Key::Core(core::Key::Tasks(tasks::Key::Live(task)))]
    else {
        panic!("live chat")
    };
    assert_eq!(row.authority.tools, tasks::Tools(513));
    assert_eq!(row.authority.budget.spend, 10);
    assert_eq!(row.authority.delegation.depth, 1);
    assert_eq!(row.authority.delegation.tasks, 2);
    assert_eq!(row.numbers.budget, 10);
}

#[test]
fn a_whole_role_edit_resizes_the_open_pool_and_the_next() {
    let mut world = World::configured(603, false, world::config(603), world::limits());
    let initial = tasks::Funder::Pool { project: 1, person: 1, period: 1 };
    assert_eq!(ledger(&world, initial).numbers.budget, 500);
    ask(&mut world, 3, people::Ask::ChangePolicy { project: 1, change: people::PolicyChange::Role(changed_role()) });
    assert_eq!(ledger(&world, initial).numbers.budget, 175);
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 2, budget: 1000, next: None }));
    let funded_goal = jig_core_world::fixtures::goal(
        &world,
        1,
        b"next period".as_slice().into(),
        people::TaskExecutor::Agent { charter: 1 },
        jig_core_world::fixtures::Goal { budget: 20, priority: 1, ..Default::default() },
    );
    ask(&mut world, 4, funded_goal);
    assert_eq!(ledger(&world, tasks::Funder::Pool { project: 1, person: 1, period: 2 }).numbers.budget, 175);
    assert_eq!(ledger(&world, initial).numbers.budget, 175);
}

fn changed_role() -> people::PolicyRole {
    people::PolicyRole {
        number: 0,
        authority: people::Authority {
            tools: 1023,
            models: None,
            grants: Box::new([]),
            delegation: people::Delegation {
                kinds: Box::new([people::Executor::Charter(1), people::Executor::Procedure(1)]),
                tasks: 2,
                depth: 2,
            },
            spend: 100,
            deadline: None,
            notes: 0,
            note_resources: Box::new([]),
        },
        period_spend: 175,
        requests: authority::Requests::ALL.0,
        decides: authority::Proposals::ALL.0,
    }
}

#[test]
fn a_raised_project_funds_its_next_chat() {
    let mut config = world::config(604);
    config.core.seed_projects[0].policy.policy.period_spend = 500;
    let mut world = World::configured(604, false, config, world::limits());
    let period = tasks::Funder::Period { project: 1, period: 1 };
    let before = *ledger(&world, period);
    assert_eq!(before.numbers.reserved, 500);
    let owner = world.sign_in;
    let second = jig_core_world::fixtures::second_party(&mut world, b"second-party", people::Role::Owner);
    world.sign_in = Some(second);
    let chat = jig_core_world::fixtures::chat(&world, 1, b"second party chat".as_slice().into());
    ask(&mut world, 7, chat.clone());
    jig_core_world::fixtures::refusal(
        &world,
        people::Refusal::Authority { proposable: None, lacking: people::Lack::Funding },
    );
    world.sign_in = owner;
    ask(
        &mut world,
        8,
        people::Ask::ChangePolicy { project: 1, change: people::PolicyChange::ProjectSpend { period_spend: 1000 } },
    );
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::PolicyChanged { project: 1 }))
    );
    let mut raised = before;
    raised.numbers.budget = 1000;
    assert_eq!(*ledger(&world, period), raised);
    world.sign_in = Some(second);
    ask(&mut world, 9, chat);
    assert!(matches!(world.people_answers.last(), Some(people::Reply::Outcome(people::Outcome::Created { .. }))));
    let pool = tasks::Funder::Pool { project: 1, person: 2, period: 1 };
    assert_eq!(ledger(&world, pool).parent, Some(period));
    assert_eq!(ledger(&world, pool).numbers.budget, 500);
    assert_eq!(ledger(&world, pool).numbers.reserved, 100);
    let Some(people::Reply::Outcome(people::Outcome::Created { first, .. })) = world.people_answers.last() else {
        panic!("created chat answer")
    };
    let root::Record::Core(core::Record::Tasks(tasks::Stored::Live(chat))) =
        &world.store.rows[&root::Key::Core(core::Key::Tasks(tasks::Key::Live(*first)))]
    else {
        panic!("second party's live chat")
    };
    assert_eq!(chat.funder, pool);
    assert_eq!(ledger(&world, period).numbers.budget, 1000);
    assert_eq!(ledger(&world, period).numbers.reserved, 1000);
}

#[test]
fn a_cut_below_what_is_used_funds_nothing_more_and_takes_nothing_back() {
    let mut world = World::configured(605, false, world::config(605), world::limits());
    jig_core_world::fixtures::finish_chat(&mut world, 25, b"finished chat");
    jig_core_world::fixtures::second_party(&mut world, b"second-party", people::Role::Owner);
    ask(&mut world, 13, people::Ask::SetPool { project: 1, person: 2, budget: 250 });
    let period = tasks::Funder::Period { project: 1, period: 1 };
    let pool = tasks::Funder::Pool { project: 1, person: 1, period: 1 };
    let before = *ledger(&world, period);
    let pool_before = *ledger(&world, pool);
    assert_eq!(pool_before.numbers.spent_below, 25);
    let used = before.numbers.spent + before.numbers.spent_below + before.numbers.reserved;
    assert_eq!(used, 750);
    ask(
        &mut world,
        10,
        people::Ask::ChangePolicy { project: 1, change: people::PolicyChange::ProjectSpend { period_spend: 500 } },
    );
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::PolicyChanged { project: 1 }))
    );
    let mut cut = before;
    cut.numbers.budget = used;
    assert_eq!(*ledger(&world, period), cut);
    assert_eq!(*ledger(&world, pool), pool_before);
    let chat = jig_core_world::fixtures::chat(&world, 1, b"after cut".as_slice().into());
    ask(&mut world, 11, chat);
    jig_core_world::fixtures::refusal(
        &world,
        people::Refusal::Authority { proposable: None, lacking: people::Lack::Budget { needs: used, left: 500 } },
    );
    assert_eq!(*ledger(&world, period), cut);
    assert_eq!(*ledger(&world, pool), pool_before);
}

#[test]
fn a_raise_survives_a_restart() {
    let mut config = world::config(606);
    config.core.seed_projects[0].policy.policy.period_spend = 500;
    let mut world = World::configured(606, false, config, world::limits());
    let period = tasks::Funder::Period { project: 1, period: 1 };
    let mut expected = *ledger(&world, period);
    expected.numbers.budget = 1000;
    ask(
        &mut world,
        12,
        people::Ask::ChangePolicy { project: 1, change: people::PolicyChange::ProjectSpend { period_spend: 1000 } },
    );
    let mut config = world::config(606);
    config.core.seed_projects[0].policy.policy.period_spend = 500;
    world.restart_with(config);
    assert_eq!(*ledger(&world, period), expected);
    let root::Record::Core(core::Record::People(people::Stored::Policy { value, .. })) =
        &world.store.rows[&root::Key::Core(core::Key::People(people::Key::Policy(1)))]
    else {
        panic!("restored project policy")
    };
    assert_eq!(value.period_spend, 1000);
}

#[test]
fn a_roles_raise_resizes_its_parties_pools_one_set_by_hand_among_them() {
    let mut config = world::config(607);
    config.core.seed_projects[0].policy.policy.period_spend = 750;
    world::edit_role(&mut config.core, |role| role.period_spend = 400);
    let mut world = World::configured(607, false, config, world::limits());
    jig_core_world::fixtures::second_party(&mut world, b"second-party", people::Role::Owner);
    ask(&mut world, 14, people::Ask::SetPool { project: 1, person: 2, budget: 250 });
    ask(
        &mut world,
        15,
        people::Ask::ChangePolicy { project: 1, change: people::PolicyChange::ProjectSpend { period_spend: 1000 } },
    );
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::PolicyChanged { project: 1 }))
    );
    let pools = [
        tasks::Funder::Pool { project: 1, person: 1, period: 1 },
        tasks::Funder::Pool { project: 1, person: 2, period: 1 },
    ];
    let before = [*ledger(&world, pools[0]), *ledger(&world, pools[1])];
    assert_eq!(before[0].numbers.reserved, 100);
    assert_eq!(before[1].numbers.budget, 250);
    ask(
        &mut world,
        16,
        people::Ask::ChangePolicy {
            project: 1,
            change: people::PolicyChange::RoleSpend { role: 0, period_spend: 500 },
        },
    );
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::PolicyChanged { project: 1 }))
    );
    for (pool, mut expected) in pools.into_iter().zip(before) {
        expected.numbers.budget = 500;
        assert_eq!(*ledger(&world, pool), expected, "spend and reservations stay");
    }
    let period = ledger(&world, tasks::Funder::Period { project: 1, period: 1 });
    assert_eq!(period.numbers.budget, 1000);
    assert_eq!(period.numbers.reserved, 1000);
}

#[test]
fn a_roles_raise_past_the_periods_room_is_refused_whole_and_writes_nothing() {
    let mut config = world::config(608);
    world::edit_role(&mut config.core, |role| role.period_spend = 400);
    let mut world = World::configured(608, false, config, world::limits());
    let owner = world.sign_in;
    jig_core_world::fixtures::second_party(&mut world, b"second-party", people::Role::Owner);
    world.sign_in = owner;
    ask(&mut world, 17, people::Ask::SetPool { project: 1, person: 2, budget: 400 });
    let period = tasks::Funder::Period { project: 1, period: 1 };
    let pools = [
        tasks::Funder::Pool { project: 1, person: 1, period: 1 },
        tasks::Funder::Pool { project: 1, person: 2, period: 1 },
    ];
    let ledgers = [*ledger(&world, period), *ledger(&world, pools[0]), *ledger(&world, pools[1])];
    assert_eq!(ledgers[0].numbers.reserved, 800);
    let policy_key = root::Key::Core(core::Key::People(people::Key::Policy(1)));
    let policy = world.store.rows[&policy_key].clone();
    let before = world.store.rows.clone();
    ask(
        &mut world,
        18,
        people::Ask::ChangePolicy {
            project: 1,
            change: people::PolicyChange::RoleSpend { role: 0, period_spend: 600 },
        },
    );
    let refusal =
        people::Refusal::Authority { proposable: None, lacking: people::Lack::Budget { needs: 400, left: 200 } };
    jig_core_world::fixtures::refusal(&world, refusal.clone());
    for (key, row) in &before {
        let mut expected = row.clone();
        if let root::Record::Core(core::Record::Core(core::CoreRecord::Deployment(header))) = &mut expected {
            header.commits += 1;
        }
        assert_eq!(world.store.rows.get(key), Some(&expected), "refusal changed durable row {key:?}");
    }
    let added: Vec<_> = world.store.rows.iter().filter(|(key, _)| !before.contains_key(*key)).collect();
    assert_eq!(added.len(), 1, "only the keyed refusal is added");
    let answer_key = people::RequestKey { person: 1, key: [18; 16] };
    assert_eq!(*added[0].0, root::Key::Core(core::Key::People(people::Key::Answer(answer_key))));
    let root::Record::Core(core::Record::People(people::Stored::Answer { key, outcome, .. })) = added[0].1 else {
        panic!("retained refusal answer")
    };
    assert_eq!(*key, answer_key);
    assert_eq!(*outcome, people::Outcome::Refused(refusal));
    assert_eq!(world.store.rows[&policy_key], policy);
    let root::Record::Core(core::Record::People(people::Stored::Policy { value, .. })) = &world.store.rows[&policy_key]
    else {
        panic!("project policy")
    };
    assert_eq!(value.roles[0].period_spend, 400);
    for (funder, expected) in [period, pools[0], pools[1]].into_iter().zip(ledgers) {
        assert_eq!(*ledger(&world, funder), expected);
    }
}

#[test]
fn a_roles_cut_keeps_each_pool_at_what_it_used() {
    let mut world = World::configured(609, false, world::config(609), world::limits());
    jig_core_world::fixtures::finish_chat(&mut world, 25, b"finished chat");
    jig_core_world::fixtures::second_party(&mut world, b"second-party", people::Role::Owner);
    ask(&mut world, 19, people::Ask::SetPool { project: 1, person: 1, budget: 25 });
    ask(&mut world, 20, people::Ask::SetPool { project: 1, person: 2, budget: 500 });
    let period = tasks::Funder::Period { project: 1, period: 1 };
    let first = tasks::Funder::Pool { project: 1, person: 1, period: 1 };
    let second = tasks::Funder::Pool { project: 1, person: 2, period: 1 };
    let first_before = *ledger(&world, first);
    let mut second_expected = *ledger(&world, second);
    let mut period_expected = *ledger(&world, period);
    assert_eq!(first_before.numbers.spent_below, 25);
    assert_eq!(first_before.numbers.reserved, 0);
    assert_eq!(second_expected.numbers.spent_below, 0);
    assert_eq!(second_expected.numbers.reserved, 0);
    let tasks_before: Vec<_> = world
        .store
        .rows
        .iter()
        .filter(|(key, _)| matches!(key, root::Key::Core(core::Key::Tasks(tasks::Key::Live(_) | tasks::Key::Ended(_)))))
        .map(|(key, row)| (key.clone(), row.clone()))
        .collect();
    ask(
        &mut world,
        21,
        people::Ask::ChangePolicy { project: 1, change: people::PolicyChange::RoleSpend { role: 0, period_spend: 10 } },
    );
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::PolicyChanged { project: 1 }))
    );
    second_expected.numbers.budget = 10;
    period_expected.numbers.reserved -= 490;
    assert_eq!(*ledger(&world, first), first_before);
    assert_eq!(*ledger(&world, second), second_expected);
    assert_eq!(*ledger(&world, period), period_expected);
    for (key, row) in tasks_before {
        assert_eq!(world.store.rows.get(&key), Some(&row), "the cut changes no task");
    }
}

#[test]
fn a_roles_cut_funds_a_hand_set_pools_raise_from_another_pools_cut() {
    let mut config = world::config(610);
    config.core.seed_projects[0].policy.policy.period_spend = 750;
    let mut world = World::configured(610, false, config, world::limits());
    jig_core_world::fixtures::second_party(&mut world, b"second-party", people::Role::Owner);
    ask(&mut world, 22, people::Ask::SetPool { project: 1, person: 1, budget: 250 });
    ask(&mut world, 23, people::Ask::SetPool { project: 1, person: 2, budget: 500 });
    let period = tasks::Funder::Period { project: 1, period: 1 };
    let period_before = *ledger(&world, period);
    assert_eq!(period_before.numbers.reserved, period_before.numbers.budget);
    let pools = [
        tasks::Funder::Pool { project: 1, person: 1, period: 1 },
        tasks::Funder::Pool { project: 1, person: 2, period: 1 },
    ];
    let before = [*ledger(&world, pools[0]), *ledger(&world, pools[1])];
    ask(
        &mut world,
        24,
        people::Ask::ChangePolicy {
            project: 1,
            change: people::PolicyChange::RoleSpend { role: 0, period_spend: 375 },
        },
    );
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::PolicyChanged { project: 1 }))
    );
    for (pool, mut expected) in pools.into_iter().zip(before) {
        expected.numbers.budget = 375;
        assert_eq!(*ledger(&world, pool), expected);
    }
    assert_eq!(*ledger(&world, period), period_before);
}
