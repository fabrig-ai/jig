use jig_core_world::effects::World;
use jig_domain_authority as authority;
use jig_domain_core as core;
use jig_domain_tasks as tasks;
use jig_test_connector as connector;
use jig_test_domain as root;

fn a_procedures_effect_waits_for_another_connectors_verdict_then_is_made() {
    let mut world = World::new(81, true);
    world.delegate();
    let task = world.procedure.expect("delegated procedure");
    assert!(world.systems[0].observed().is_empty(), "unknown observer facts authorize no effect");
    assert!(!world.pending_reads.is_empty(), "the second connector reads for its verdict");
    assert!(world.store.rows.values().any(|row| matches!(row, root::Record::Connector { number: 1, record: connector::Record::Procedure(state) } if state.task == task && state.steps == 1 && !state.awaiting)), "refused effect is counted, with no outstanding entry");
    world.hold_writes = true;
    world.observed(13);
    let (number, call) = world.pending_writes.pop().expect("procedure's committed write");
    world.restart(81, true);
    let event = world.systems[0].answer(call, jig_test_system::Fault::None);
    world.send(root::Event::Connector { number, event: connector::Event::System(event) });
    assert_eq!(world.systems[0].observed().iter().filter(|row| row.applied).count(), 1, "{:?}", world.trace);
    assert_eq!(world.systems[0].observed()[0].key.purpose, 19);
    assert!(
        world.store.rows.contains_key(&root::Key::Core(core::Key::Tasks(tasks::Key::Ended(task)))),
        "procedure finishes after its made outcome"
    );
}

fn a_tool_call_whose_answer_is_lost_with_the_channel_and_the_engine_is_asked_again_after_a_restart_and_answered_from_its_record()
 {
    let mut world = World::new(82, false);
    world.lose_answer = true;
    world.call(1, 202);
    let key = world.call_key(1);
    assert_eq!(world.lost_answers, 1);
    assert_eq!(world.spent(), 3, "the effect's maximum is charged in its deciding commit");
    let row = world
        .store
        .rows
        .get(&root::Key::Core(core::Key::Core(core::CoreKey::Call(key))))
        .expect("saved call answer")
        .clone();
    assert!(matches!(
        row,
        root::Record::Core(core::Record::Core(core::CoreRecord::Call(core::CallRecord {
            part: core::CallPart::Effect { outcome: Some(core::connector::OutboxOutcome::Made), .. },
            ..
        })))
    ));
    let commits = world.store.applied;
    world.restart(82, false);
    world.call(1, 203);
    assert_eq!(world.answers.len(), 1);
    assert_eq!(world.answers[0].0.raw(), 203);
    assert!(matches!(
        world.answers[0].2,
        core::CallPart::Effect { outcome: Some(core::connector::OutboxOutcome::Made), .. }
    ));
    assert_eq!(world.systems[0].observed().iter().filter(|row| row.applied).count(), 1);
    assert_eq!(world.store.applied, commits + 1, "only the start's mark commits; replay decides nothing again");
    assert_eq!(world.spent(), 3, "replay cannot charge the effect twice");
}

fn an_effect_call_wait_commits_nothing_and_the_same_name_can_be_asked_again() {
    let mut world = World::new(83, true);
    let commits = world.store.applied;
    world.call(1, 204);
    assert_eq!(world.store.applied, commits);
    assert_eq!(world.spent(), 0, "a waiting effect is not charged");
    assert!(matches!(
        world.answers.last(),
        Some((_, _, core::CallPart::EffectDenied { answer: authority::Answer::Wait, .. }))
    ));
    assert!(world.systems[0].observed().is_empty());
    world.observed(13);
    world.call(1, 205);
    assert!(matches!(
        world.answers.last(),
        Some((_, _, core::CallPart::Effect { outcome: Some(core::connector::OutboxOutcome::Made), .. }))
    ));
    assert_eq!(world.systems[0].observed().iter().filter(|row| row.applied).count(), 1);
}

fn an_effect_answers_committed_and_pending_only_past_its_absolute_deadline() {
    let mut world = World::new(84, false);
    world.hold_writes = true;
    world.call(1, 206);
    assert!(world.answers.is_empty(), "keeping an entry does not answer made");
    assert_eq!(world.pending_writes.len(), 1);
    let commits = world.store.applied;
    world.call(1, 207);
    assert!(world.answers.is_empty(), "a pending duplicate replaces its lost reply right");
    assert_eq!(world.store.applied, commits);
    world.deadline();
    assert_eq!(world.answers.len(), 1);
    assert_eq!(world.answers[0].0.raw(), 207);
    assert!(matches!(world.answers[0].2, core::CallPart::Effect { outcome: None, .. }));
    assert_eq!(world.store.applied, commits, "deadline reports the previously committed entry");
    world.release_writes();
    assert_eq!(world.answers.len(), 1, "the expired reply right is consumed once");
    world.call(1, 208);
    assert!(matches!(
        world.answers.last(),
        Some((_, _, core::CallPart::Effect { outcome: Some(core::connector::OutboxOutcome::Made), .. }))
    ));
    assert_eq!(world.systems[0].observed().iter().filter(|row| row.applied).count(), 1);
}

fn an_explicit_effect_proposal_is_kept_across_a_restart_and_checked_again_before_acceptance() {
    let mut world = World::new(85, true);
    let proposal = world.propose(1);
    assert!(world.systems[0].observed().is_empty());
    assert!(
        world
            .store
            .rows
            .contains_key(&root::Key::Connector { number: 1, key: connector::RecordKey::Proposal(proposal) })
    );
    world.restart(85, true);
    world.waiting_judge();
    let commits = world.store.applied;
    world.decide_proposal(proposal, true, 1);
    assert!(world.systems[0].observed().is_empty(), "acceptance cannot override an unknown judge");
    assert_eq!(world.store.applied, commits, "a waiting judge commits no accepted proposal");
    world.observed(13);
    world.decide_proposal(proposal, true, 2);
    assert_eq!(world.systems[0].observed().iter().filter(|row| row.applied).count(), 1, "{:?}", world.trace);
    assert!(
        !world
            .store
            .rows
            .contains_key(&root::Key::Connector { number: 1, key: connector::RecordKey::Proposal(proposal) })
    );
    assert_eq!(world.systems[0].observed()[0].key.attempt, world.call_key(1).attempt);
    assert_eq!(world.systems[0].observed()[0].key.completion, 1);
    assert_eq!(world.spent(), 0, "the proposal's accepter funds the effect");
    assert_eq!(world.holder_spent(), 3);
}

fn rejecting_an_effect_proposal_erases_its_payload_without_making_it() {
    let mut world = World::new(86, false);
    let proposal = world.propose(1);
    world.decide_proposal(proposal, false, 3);
    assert!(world.systems[0].observed().is_empty());
    assert!(
        !world
            .store
            .rows
            .contains_key(&root::Key::Connector { number: 1, key: connector::RecordKey::Proposal(proposal) })
    );
}

fn a_full_connector_outbox_is_busy_without_a_record_or_charge_and_can_retry_the_same_name() {
    let mut world = World::with_entries(87, false, 1);
    world.hold_writes = true;
    world.call(1, 210);
    let commits = world.store.applied;
    world.call(2, 211);
    assert!(matches!(world.answers.last(), Some((_, _, core::CallPart::Unavailable))));
    assert_eq!(world.store.applied, commits);
    assert_eq!(world.spent(), 3);
    assert!(!world.store.rows.contains_key(&root::Key::Core(core::Key::Core(core::CoreKey::Call(world.call_key(2))))));
    world.release_writes();
    world.call(2, 212);
    assert!(matches!(
        world.answers.last(),
        Some((_, _, core::CallPart::Effect { outcome: Some(core::connector::OutboxOutcome::Failed), .. }))
    ));
    assert_eq!(world.spent(), 6);
    assert_eq!(world.systems[0].observed().iter().filter(|row| row.applied).count(), 1);
}

// Each scenario keeps its own fresh world and all original assertions.
// A shared entrypoint avoids one process startup per short scenario.
#[test]
fn effects_scenarios() {
    a_procedures_effect_waits_for_another_connectors_verdict_then_is_made();
    a_tool_call_whose_answer_is_lost_with_the_channel_and_the_engine_is_asked_again_after_a_restart_and_answered_from_its_record();
    an_effect_call_wait_commits_nothing_and_the_same_name_can_be_asked_again();
    an_effect_answers_committed_and_pending_only_past_its_absolute_deadline();
    an_explicit_effect_proposal_is_kept_across_a_restart_and_checked_again_before_acceptance();
    rejecting_an_effect_proposal_erases_its_payload_without_making_it();
    a_full_connector_outbox_is_busy_without_a_record_or_charge_and_can_retry_the_same_name();
}

// Reuse an actual world's durable claim and public configuration, restoring
// through the core's public entry points rather than injecting waiting rights.
fn retained_effect_core(seed: u64) -> (core::Core, core::Limits, core::CallKey) {
    use skein_lib::{Env, Queue, Time, Wall};
    let world = World::new(seed, false);
    let key = world.call_key(1);
    let (configuration, limits) = jig_core_world::effects::fixture(seed, false);
    let mut domain = core::Core::new(configuration.core, &limits.core).expect("fixture core configuration");
    let deployment = world
        .store
        .rows
        .values()
        .find_map(|row| {
            if let root::Record::Core(core::Record::Core(record @ core::CoreRecord::Deployment(_))) = row {
                Some(record.clone())
            } else {
                None
            }
        })
        .expect("the actual world committed its deployment");
    assert!(matches!(domain.restore_core(deployment, &limits.core), core::Restored::Deployment { .. }));
    jig_core_world::fixtures::restore_project_policies(&mut domain, &limits.core, &world.store);
    let mut out = Queue::with_capacity(tasks::max_out(&limits.core.tasks));
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.core.tasks };
    for row in world.store.rows.values() {
        if let root::Record::Core(core::Record::Tasks(
            record @ (tasks::Stored::Live(_)
            | tasks::Stored::Ledger(_)
            | tasks::Stored::Result(_)
            | tasks::Stored::EndedByParty(_)
            | tasks::Stored::EndedByProject(_)
            | tasks::Stored::EndedDelegate(_)
            | tasks::Stored::Delegate(_)
            | tasks::Stored::Update(_)
            | tasks::Stored::Pool(_)
            | tasks::Stored::ClosedLedger(_)
            | tasks::Stored::DecidedProposal(_)
            | tasks::Stored::Writer(_)),
        )) = row
        {
            assert!(domain.restore_task_row(record));
            tasks::step(&mut domain.tasks, &env, tasks::Event::Restore { record: record.clone() }, &mut out);
            while out.pop().is_some() {}
        }
    }
    for row in world.store.rows.values() {
        if let root::Record::Core(core::Record::Core(record @ core::CoreRecord::RunProof(_))) = row {
            assert_eq!(domain.restore_core(record.clone(), &limits.core), core::Restored::Live);
        }
    }
    tasks::step(&mut domain.tasks, &env, tasks::Event::Restored, &mut out);
    assert!(domain.current_proof(key.task, key.attempt));
    assert!(matches!(domain.tasks.task(key.task).expect("restored caller").phase,
        tasks::Phase::Active(tasks::Active::Claimed { attempt } | tasks::Active::Running { attempt }) if attempt == key.attempt));
    (domain, limits.core, key)
}

fn effect_outputs(
    domain: &mut core::Core,
    limits: &core::Limits,
    wall: skein_lib::Wall,
    event: core::Event,
) -> Vec<core::Request> {
    let core::Requests::Out(mut out) =
        core::step(domain, &skein_lib::Env { now: skein_lib::Time::ZERO, wall, limits: *limits }, event);
    let mut requests = Vec::new();
    while let Some(request) = out.pop() {
        requests.push(request);
    }
    requests
}

fn kept_effect(
    domain: &mut core::Core,
    limits: &core::Limits,
    key: core::CallKey,
    deadline: skein_lib::Wall,
    right: u64,
) -> core::CallPart {
    use skein_lib::{ReplyTo, Token, Wall};
    let owner = Token::new(right.checked_add(1000).expect("fixture owner"));
    let started = effect_outputs(
        domain,
        limits,
        Wall::EPOCH,
        core::Event::EffectStart {
            owner,
            connector: 1,
            origin: core::EffectOrigin::Call { to: ReplyTo::new(Token::new(right)), key, deadline },
        },
    );
    assert!(started.iter().any(|row| matches!(row, core::Request::Ask { connector: 1, ask: core::Ask::Effect(core::connector::Ask::Describe { owner: actual }) } if *actual == owner)));
    let kept = effect_outputs(
        domain,
        limits,
        Wall::EPOCH,
        core::Event::EffectConnector(core::connector::Event::Described {
            owner,
            description: Box::new(core::connector::EffectDescription {
                waits_for_dependencies: false,
                connector: 1,
                purpose: 19,
                effect: authority::Effect {
                    connector: 1,
                    kind: 5,
                    name: authority::Name {
                        segments: jig_test_connector_world::path(1, 1).segments().to_vec().into_boxed_slice(),
                    },
                    state: [13; 32],
                    price: Some(1),
                    access: authority::EffectAccess::Owned,
                    additional: Box::new([]),
                    guards: Box::new([]),
                },
                form: core::connector::EffectForm::Set,
                recovery: core::connector::Recovery::Keyed,
            }),
        }),
    );
    assert!(
        !kept.iter().any(
            |row| matches!(row, core::Request::Held(held) if matches!(held.as_ref(), core::Held::CallAnswer { .. }))
        )
    );
    kept.iter()
        .find_map(|row| {
            if let core::Request::Write(core::Write::Save(core::Record::Core(core::CoreRecord::Call(record)))) = row {
                if record.key == key { Some(record.part.clone()) } else { None }
            } else {
                None
            }
        })
        .expect("the effect's actual committed evidence")
}

fn deadline_answers(outputs: Vec<core::Request>) -> Vec<(u64, core::CallKey, core::CallPart)> {
    outputs
        .into_iter()
        .filter_map(|row| {
            assert!(!matches!(row, core::Request::Write(_)), "a deadline makes no new commit");
            match row {
                core::Request::Held(held) => {
                    if let core::Held::CallAnswer { to, key, part } = *held {
                        Some((to.into_token().raw(), key, part))
                    } else {
                        panic!("the deadline emits only retained call answers")
                    }
                }
                core::Request::Decided => None,
                core::Request::ProjectMade { .. }
                | core::Request::Write(_)
                | core::Request::Ask { .. }
                | core::Request::Now(_) => {
                    panic!("the deadline makes no new effect or synchronous handoff")
                }
            }
        })
        .collect()
}

#[test]
fn retained_effect_deadline_is_a_pure_minimum_including_equal_walls_and_epoch() {
    use skein_lib::Wall;
    for (first, second) in [(20, 10), (10, 10), (30, 0)] {
        let (mut domain, limits, first_key) = retained_effect_core(104);
        assert_eq!(domain.effect_deadline(), None);
        assert_eq!(domain.effect_deadline(), None);
        let second_key = core::CallKey { completion: 2, ..first_key };
        let first_part = kept_effect(&mut domain, &limits, first_key, Wall::from_nanos(first), 301);
        assert_eq!(domain.effect_deadline(), Some(Wall::from_nanos(first)));
        let second_part = kept_effect(&mut domain, &limits, second_key, Wall::from_nanos(second), 302);
        let earliest = first.min(second);
        let caller = domain.tasks.task(first_key.task).expect("caller").clone();
        let quiescence = [domain.quiescent(false), domain.quiescent(true)];
        for _ in 0..3 {
            assert_eq!(domain.effect_deadline(), Some(Wall::from_nanos(earliest)));
            assert_eq!(domain.tasks.task(first_key.task), Some(&caller), "a query changes no owner fact");
            assert_eq!([domain.quiescent(false), domain.quiescent(true)], quiescence);
        }
        if earliest != 0 {
            assert!(
                deadline_answers(effect_outputs(
                    &mut domain,
                    &limits,
                    Wall::from_nanos(earliest - 1),
                    core::Event::EffectDeadline
                ))
                .is_empty()
            );
            assert_eq!(domain.effect_deadline(), Some(Wall::from_nanos(earliest)));
        }
        let answers = deadline_answers(effect_outputs(
            &mut domain,
            &limits,
            Wall::from_nanos(earliest),
            core::Event::EffectDeadline,
        ));
        let mut expected = Vec::new();
        if first == earliest {
            expected.push((301, first_key, first_part.clone()));
        }
        if second == earliest {
            expected.push((302, second_key, second_part.clone()));
        }
        assert_eq!(answers, expected, "exactly due rights retain their previous committed evidence");
        let remaining = if first == second { None } else { Some(Wall::from_nanos(first.max(second))) };
        assert_eq!(domain.effect_deadline(), remaining);
        assert!(
            deadline_answers(effect_outputs(
                &mut domain,
                &limits,
                Wall::from_nanos(earliest),
                core::Event::EffectDeadline
            ))
            .is_empty()
        );
        if let Some(last) = remaining {
            let final_answers =
                deadline_answers(effect_outputs(&mut domain, &limits, last, core::Event::EffectDeadline));
            assert_eq!(
                final_answers,
                if first > second { vec![(301, first_key, first_part)] } else { vec![(302, second_key, second_part)] }
            );
        }
        assert_eq!(domain.effect_deadline(), None);
    }
}

#[test]
fn a_duplicate_keeps_its_exact_deadline_and_latest_right_while_made_removes_only_its_own() {
    use skein_lib::{ReplyTo, Token, Wall};
    let (mut domain, limits, first_key) = retained_effect_core(105);
    let second_key = core::CallKey { completion: 2, ..first_key };
    let first = kept_effect(&mut domain, &limits, first_key, Wall::from_nanos(20), 311);
    let second = kept_effect(&mut domain, &limits, second_key, Wall::from_nanos(10), 312);
    let duplicate = effect_outputs(
        &mut domain,
        &limits,
        Wall::EPOCH,
        core::Event::EffectStart {
            owner: Token::new(1313),
            connector: 1,
            origin: core::EffectOrigin::Call {
                to: ReplyTo::new(Token::new(313)),
                key: first_key,
                deadline: Wall::from_nanos(999),
            },
        },
    );
    assert!(!duplicate.iter().any(|row| matches!(row, core::Request::Write(_) | core::Request::Held(_))));
    assert_eq!(domain.effect_deadline(), Some(Wall::from_nanos(10)));
    let core::CallPart::Effect { entry, .. } = second else { panic!("kept second effect") };
    let made = effect_outputs(
        &mut domain,
        &limits,
        Wall::from_nanos(1),
        core::Event::EffectConnector(core::connector::Event::Outbox {
            connector: 1,
            entry,
            task: second_key.task,
            outcome: core::connector::OutboxOutcome::Made,
        }),
    );
    let made_answers: Vec<_> = made
        .into_iter()
        .filter_map(|row| {
            if let core::Request::Held(held) = row {
                if let core::Held::CallAnswer { to, key, part } = *held {
                    Some((to.into_token().raw(), key, part))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        made_answers,
        vec![(
            312,
            second_key,
            core::CallPart::Effect {
                connector: 1,
                entry,
                deadline: Wall::from_nanos(10),
                outcome: Some(core::connector::OutboxOutcome::Made),
            }
        )]
    );
    assert_eq!(domain.effect_deadline(), Some(Wall::from_nanos(20)));
    assert!(
        deadline_answers(effect_outputs(&mut domain, &limits, Wall::from_nanos(19), core::Event::EffectDeadline))
            .is_empty()
    );
    assert_eq!(
        deadline_answers(effect_outputs(&mut domain, &limits, Wall::from_nanos(20), core::Event::EffectDeadline)),
        vec![(313, first_key, first)]
    );
    assert_eq!(domain.effect_deadline(), None);
}

#[test]
fn a_stale_claim_keeps_its_effect_deadline_until_the_due_right_is_consumed_without_answer() {
    use skein_lib::{ReplyTo, Token, Wall};
    let (mut domain, limits, key) = retained_effect_core(106);
    kept_effect(&mut domain, &limits, key, Wall::from_nanos(20), 321);
    effect_outputs(
        &mut domain,
        &limits,
        Wall::from_nanos(1),
        core::Event::Tasks(tasks::Event::Control {
            reply_to: ReplyTo::new(Token::new(322)),
            by: tasks::Party::Person(1),
            task: key.task,
            control: tasks::Control::Cancel { reason: Box::new([]), grace: None, message: None },
        }),
    );
    assert!(!matches!(
        domain.tasks.task(key.task).map(|row| &row.phase),
        Some(tasks::Phase::Active(tasks::Active::Claimed { .. } | tasks::Active::Running { .. }))
    ));
    assert_eq!(domain.effect_deadline(), Some(Wall::from_nanos(20)), "stale rights stay queryable for cleanup");
    assert!(
        deadline_answers(effect_outputs(&mut domain, &limits, Wall::from_nanos(19), core::Event::EffectDeadline))
            .is_empty()
    );
    assert_eq!(domain.effect_deadline(), Some(Wall::from_nanos(20)));
    assert!(
        deadline_answers(effect_outputs(&mut domain, &limits, Wall::from_nanos(20), core::Event::EffectDeadline))
            .is_empty()
    );
    assert_eq!(domain.effect_deadline(), None);
    assert!(
        deadline_answers(effect_outputs(&mut domain, &limits, Wall::from_nanos(20), core::Event::EffectDeadline))
            .is_empty()
    );
}

#[test]
fn an_unauthorized_grant_reaches_the_accounts_child() {
    use jig_domain_fleet as fleet;
    use skein_lib::Token;
    let (config, limits) = jig_core_world::world::two_charters(151);
    let mut rows = jig_core_world::fixtures::account_rows(&config.core.settings.llm_providers).into_vec();
    for row in &mut rows {
        match row {
            root::Record::Core(core::Record::Accounts(jig_domain_accounts::Stored::Account {
                number: 0,
                holder,
                ..
            })) => *holder = jig_domain_accounts::Holder::Party(1),
            root::Record::Core(core::Record::Accounts(jig_domain_accounts::Stored::Source {
                number: 1, kind, ..
            })) => *kind = jig_domain_accounts::Kind::SignIn,
            root::Record::Core(_) | root::Record::Connector { .. } => {}
        }
    }
    let mut world = jig_core_world::fixtures::configured_sources(151, config, limits, rows.into_boxed_slice());
    let before = assignment(&world).grants[0];
    assert_eq!(before.generation, 1);
    let current = assignment(&world);
    world.send(root::Event::Core(core::Event::Fleet(fleet::Event::Unauthorized {
        channel: Token::new(7),
        run: Token::new(current.task),
        attempt: Token::new(current.attempt),
        account: before.account,
        generation: before.generation,
    })));
    world.delegate_agent(1, b"use the refreshed account");
    assert_eq!(world.assignments.len(), 2, "the notice is observed through a fresh assignment");
    let grant = assignment(&world).grants[0];
    assert_eq!((grant.account, grant.generation), (before.account, 2));
}

#[test]
fn a_workers_limit_reaches_the_accounts_child_and_holds_its_charter_until_the_reset() {
    use jig_domain_fleet as fleet;
    use skein_lib::{Duration, Token, Wall};
    let (config, limits) = jig_core_world::world::two_charters(152);
    let mut world = World::configured(152, false, config, limits);
    let (run, attempt, account) = {
        let current = assignment(&world);
        (current.task, current.attempt, current.grants[0].account)
    };
    let reset = Duration::from_secs(5);
    world.send(root::Event::Core(core::Event::Fleet(fleet::Event::Limited {
        channel: Token::new(7),
        run: Token::new(run),
        attempt: Token::new(attempt),
        account,
        reset: Some(reset),
    })));
    let assigned = world.assignments.len();
    world.delegate_agent(1, b"wait out the limit");
    assert_eq!(world.assignments.len(), assigned, "the limited account starts nothing");
    let (now, wall) = world.clocks();
    let (now, wall) = (now.saturating_add(reset), Wall::from_nanos(wall.as_nanos().saturating_add(reset.as_nanos())));
    assert!(world.next_due().is_some_and(|due| due <= now), "the reset is armed");
    world.fire_due(now, wall);
    assert_eq!(world.assignments.len(), assigned + 1, "the limit ends at its reset");
    assert_eq!(assignment(&world).grants[0].account, account);
}

fn assignment(world: &World) -> &root::Assignment {
    world.assigned.as_ref().expect("live assignment")
}

#[test]
fn ordinary_failed_and_held_entries_keep_their_call_and_task_routes_across_restart() {
    for (seed, recovery, result, expected) in [
        (88, connector::Recovery::Keyed, connector::ApplyResult::Conflict, core::connector::OutboxOutcome::Failed),
        (
            89,
            connector::Recovery::Unrecoverable,
            connector::ApplyResult::Uncertain,
            core::connector::OutboxOutcome::Held { entry: 1 },
        ),
    ] {
        let (mut configuration, limits) = jig_core_world::effects::fixture(seed, false);
        configuration.first.kinds.iter_mut().find(|kind| kind.kind == 5).expect("configured effect kind").recovery =
            recovery;
        let mut world = World::configured(seed, false, configuration, limits);
        world.recovery = recovery;
        world.hold_writes = true;
        world.call(1, 220);
        let key = world.call_key(1);
        let entry = world
            .store
            .rows
            .values()
            .find_map(|row| match row {
                root::Record::Connector { record: connector::Record::Outbox(entry), .. } => Some(entry.clone()),
                root::Record::Core(_) | root::Record::Connector { .. } => None,
            })
            .expect("ordinary pending entry");
        world.restart(seed, false);
        world.send(root::Event::Connector {
            number: 1,
            event: connector::Event::System(connector::SystemEvent::Applied {
                entry: entry.number,
                attempt: entry.attempt.expect("sent attempt").number,
                result,
            }),
        });
        let expected = if result == connector::ApplyResult::Uncertain {
            core::connector::OutboxOutcome::Held { entry: entry.number }
        } else {
            expected
        };
        assert!(matches!(world.store.rows.get(&root::Key::Core(core::Key::Core(core::CoreKey::Call(key)))),
            Some(root::Record::Core(core::Record::Core(core::CoreRecord::Call(core::CallRecord {
                part: core::CallPart::Effect { outcome: Some(outcome), .. }, ..
            })))) if *outcome == expected));
        let row = jig_core_world::fixtures::live_row(&world, key.task);
        let tasks::Phase::Held { source, .. } = &row.phase else { panic!("ordinary outcome holds its task") };
        assert_eq!(source.as_deref(), Some(&tasks::HoldSource::Effect { connector: 1, entry: entry.number }));
        if recovery == connector::Recovery::Unrecoverable {
            world.send(root::Event::Connector {
                number: 1,
                event: connector::Event::Resolve { entry: entry.number, repeat: false },
            });
        }
        assert!(!world.store.rows.values().any(|row| matches!(row,
            root::Record::Connector { record: connector::Record::Outbox(row), .. } if row.number == entry.number)));
    }
}
