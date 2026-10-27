use jig_core_world::effects::World;
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{ReplyTo, Token};

fn ask(world: &mut World, request: u8, ask: people::Ask) {
    ask_as(world, world.sign_in.expect("signed-in person"), request, ask);
}

fn ask_as(world: &mut World, sign_in: u64, request: u8, ask: people::Ask) {
    world.send(root::Event::Core(core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(8000 + u64::from(request))),
        sign_in,
        key: [request; 16],
        ask,
    })));
}

fn hold_revision(world: &World, task: u64) -> u64 {
    match world.store.rows.get(&root::Key::Core(core::Key::Tasks(tasks::Key::Live(task)))) {
        Some(root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row)))) => row.revisions.hold,
        row => panic!("live task row missing: {row:?}"),
    }
}

fn second_person(world: &mut World, role: bool) -> u64 {
    world.send(root::Event::Core(core::Event::SignIn {
        held: None,
        reply_to: ReplyTo::new(Token::new(8090)),
        identity: people::Identity {
            key: people::IdentityKey { provider: 0, subject: Box::new([2]) },
            login: b"second".as_slice().into(),
            name: b"Second".as_slice().into(),
        },
    }));
    let Some(people::Reply::SignedIn { person, sign_in, .. }) = world.people_answers.last().cloned() else {
        panic!("second person signed in");
    };
    if role {
        world.send(root::Event::Core(core::Event::People(people::Event::Roles {
            project: 1,
            holdings: Box::new([
                people::Holding { person: 1, role: people::Role::Owner },
                people::Holding { person, role: people::Role::Owner },
            ]),
        })));
    }
    sign_in
}

#[test]
fn deciding_an_escalation_after_its_release_is_refused_as_not_waiting() {
    let mut world = World::new(152, false);
    let task = world.call_key(1).task;
    world.send(root::Event::Core(core::Event::Tasks(tasks::Event::Hold { task, why: tasks::Hold::Stalled })));
    world.end(tasks::End::Parked, 0);
    let revision = hold_revision(&world, task);
    ask(&mut world, 61, people::Ask::Release { project: 1, task, revision, again: None });
    ask(
        &mut world,
        62,
        people::Ask::DecideEscalation { project: 1, task, revision, decision: people::EscalationDecision::Pass },
    );
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Refused(people::Refusal::NotWaiting)))
    );
}

#[test]
fn deciding_an_escalation_another_party_decided_says_by_whom() {
    let mut world = World::new(153, false);
    let first = world.sign_in.expect("first person");
    let second = second_person(&mut world, true);
    let task = world.call_key(1).task;
    world.send(root::Event::Core(core::Event::Tasks(tasks::Event::Hold { task, why: tasks::Hold::Stalled })));
    world.end(tasks::End::Parked, 0);
    let revision = hold_revision(&world, task);
    ask_as(
        &mut world,
        first,
        61,
        people::Ask::DecideEscalation {
            project: 1,
            task,
            revision,
            decision: people::EscalationDecision::Reject { reason: b"leave held".as_slice().into() },
        },
    );
    ask_as(
        &mut world,
        second,
        62,
        people::Ask::DecideEscalation { project: 1, task, revision, decision: people::EscalationDecision::Pass },
    );
    assert!(matches!(
        world.people_answers.last(),
        Some(people::Reply::Outcome(people::Outcome::EscalationDecided {
            task: 1,
            revision: 1,
            by: 1,
            choice: people::EscalationChoice::Rejected,
            ..
        }))
    ));
}

#[test]
fn deciding_an_escalation_of_an_unseen_project_is_unknown() {
    let mut world = World::new(154, false);
    let task = world.call_key(1).task;
    world.send(root::Event::Core(core::Event::Tasks(tasks::Event::Hold { task, why: tasks::Hold::Stalled })));
    world.end(tasks::End::Parked, 0);
    let revision = hold_revision(&world, task);
    let other = second_person(&mut world, false);
    ask_as(
        &mut world,
        other,
        62,
        people::Ask::DecideEscalation { project: 1, task, revision, decision: people::EscalationDecision::Pass },
    );
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Unknown)))
    );
}

fn policy_without_effect_approvers() -> (root::Config, root::Limits) {
    use jig_domain_authority as authority;
    let (mut config, limits) = jig_core_world::effects::fixture(155, false);

    let rules = config.core.defaults.rules.clone();
    let mut policy = config.core.seed_projects[0].policy.policy.clone();
    for role in &mut policy.roles {
        role.decides = authority::Proposals(16);
        role.authority.grants = Box::new([]);
    }
    config.core.defaults.rules =
        (authority::Domain::new(rules, limits.core.authority).expect("authority")).rules().clone();
    config.core.seed_projects[0].policy.policy = policy;
    (config, limits)
}

fn proposal_rows(world: &World) -> Vec<(root::Key, root::Record)> {
    world
        .store
        .rows
        .iter()
        .filter(|(key, _)| {
            !matches!(key, root::Key::Core(core::Key::Core(core::CoreKey::Deployment | core::CoreKey::Call(_))))
        })
        .map(|(key, row)| (key.clone(), row.clone()))
        .collect()
}

#[test]
fn a_run_proposing_an_effect_nobody_may_accept_is_refused() {
    let (configuration, limits) = policy_without_effect_approvers();
    let mut world = World::configured(155, false, configuration, limits);
    let before = proposal_rows(&world);
    world.send(root::Event::EffectCall {
        to: ReplyTo::new(Token::new(8200)),
        key: world.call_key(1),
        number: 1,
        effect: World::effect(),
        deadline: skein_lib::Wall::from_nanos(world.wall_time().as_nanos() + 1_000_000_000),
        proposal: Some(b"ask the holder".as_slice().into()),
    });
    assert!(matches!(
        world.answers.last(),
        Some((_, _, core::CallPart::ProposalRefused(tasks::Problem { why: tasks::Refusal::Unproposable, .. })))
    ));
    assert_eq!(proposal_rows(&world), before, "only the durable call answer and its header may change");
}

#[test]
fn a_proposal_pending_when_the_policy_narrows_still_waits() {
    use jig_domain_authority as authority;
    let mut world = World::new(156, false);
    let proposal = world.propose(1);
    let task = world.call_key(1).task;
    let role = people::PolicyRole {
        number: 0,
        period_spend: 500,
        requests: authority::Requests::ALL.0,
        decides: 16,
        authority: people::Authority {
            tools: 1023,
            models: None,
            grants: Box::new([]),
            delegation: people::Delegation {
                kinds: Box::new([people::Executor::Charter(1), people::Executor::Procedure(1)]),
                tasks: 2,
                depth: 2,
            },
            spend: 500,
            deadline: None,
            notes: 0,
            note_resources: Box::new([]),
        },
    };
    ask(&mut world, 64, people::Ask::ChangePolicy { project: 1, change: people::PolicyChange::Role(role) });
    let key = root::Key::Core(core::Key::Tasks(tasks::Key::Live(task)));
    let Some(root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row)))) = world.store.rows.get(&key) else {
        panic!("pending proposal remains stored");
    };
    let row = row.proposal.as_ref().expect("pending proposal retained");
    assert_eq!(row.number, proposal);
    assert!(matches!(
        row.state,
        tasks::ProposalState::Pending {
            holder: tasks::ProposalHolder::Policy { project: 1, kind: tasks::ProposalKind::Effect },
            ..
        }
    ));
}
