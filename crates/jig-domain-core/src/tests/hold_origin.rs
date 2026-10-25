//! Connector notifications and documents retain only their opaque owner references.

use super::*;

#[test]
fn held_connector_parts_keep_exact_origin_and_refuse_shortened_historical_keys() {
    let (mut core, mut limits) = make_tree();
    let env = Env { now: Time::ZERO, wall: Wall::from_nanos(9), limits };
    let source = tasks::Name { connector: 2, path: Box::new([Box::from(&b"whole"[..])]) };
    let _out =
        step(&mut core, &env, Event::EffectConnector(connector::Event::Drift { task: 2, resource: source.clone() }));
    let row = core.tasks.task(2).unwrap();
    let tasks::Phase::Held { source: Some(origin), .. } = &row.phase else { panic!("retained source") };
    assert_eq!(**origin, tasks::HoldSource::Resource(source));
    let chip = chip_from_row(&limits, row).unwrap();
    let part = chip.details.hold.unwrap().source.unwrap();
    assert_eq!(part.connector, 2);
    assert_eq!(part.kind, PartKind::Resource);
    let PartKey::Path(path) = part.key else { panic!("whole path") };
    assert_eq!(path.get(0).unwrap().as_ref(), b"whole");
    let milestone = tasks::Milestone { task: 2, position: row.milestone, phase: row.phase.clone(), at: env.wall };
    let line = milestone_from_row(&limits, &milestone).unwrap();
    let MilestonePhase::Held { source: Some(_), .. } = line.phase else { panic!("historical source") };
    limits.documents.text_bytes = 4;
    assert_eq!(chip_from_row(&limits, row), Err(Missing::TooLarge));
    assert_eq!(milestone_from_row(&limits, &milestone), Err(Missing::TooLarge));
}

#[test]
fn failed_and_uncertain_outbox_parts_keep_the_actual_connector_and_entry() {
    for outcome in [connector::OutboxOutcome::Failed, connector::OutboxOutcome::Held { entry: 8 }] {
        let (mut core, limits) = make_tree();
        let _out = step(
            &mut core,
            &Env { now: Time::ZERO, wall: Wall::from_nanos(9), limits },
            Event::EffectConnector(connector::Event::Outbox { connector: 2, task: 2, entry: 8, outcome }),
        );
        let chip = chip_from_row(&limits, core.tasks.task(2).unwrap()).unwrap();
        assert_eq!(
            chip.details.hold.unwrap().source,
            Some(Part { connector: 2, kind: PartKind::Outbox, key: PartKey::Number(8) })
        );
    }
}

#[test]
fn drift_task_document_keeps_the_hold_account_separate_from_the_whole_resource_origin() {
    let (mut core, limits) = make_tree();
    let resource = tasks::Name { connector: 2, path: Box::new([Box::from(&b"whole"[..]), Box::from(&[255, 0][..])]) };
    let _out = step(
        &mut core,
        &Env { now: Time::ZERO, wall: Wall::from_nanos(9), limits },
        Event::EffectConnector(connector::Event::Drift { task: 2, resource: resource.clone() }),
    );
    let row = core.tasks.task(2).unwrap();
    let tasks::Phase::Held { why, source: Some(origin), .. } = &row.phase else { panic!("held drift") };
    assert_eq!(*why, tasks::Hold::Drift);
    assert_eq!(**origin, tasks::HoldSource::Resource(resource.clone()));
    let Document::Task(task) = make(&core, &limits, 1, &Subject::Task { task: 2 }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!("held task document")
    };
    let hold = task.hold.unwrap();
    assert_eq!(hold.account, Some(Part { connector: 2, kind: PartKind::Hold, key: PartKey::Number(2) }));
    let source = hold.source.unwrap();
    assert_eq!(source.connector, resource.connector);
    assert_eq!(source.kind, PartKind::Resource);
    let PartKey::Path(segments) = source.key else { panic!("resource origin") };
    assert_eq!(segments.as_slice(), resource.path.as_ref());
    assert_eq!(hold.reason, tasks::Hold::Drift);
    assert_eq!(hold.revision, row.revisions.hold);
    let Document::Escalation(escalation) =
        make(&core, &limits, 1, &Subject::Escalation { task: 2 }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!("escalation")
    };
    assert_eq!(escalation.task.hold.unwrap().account, hold.account);
}

#[test]
fn failed_effect_task_document_names_the_held_task_account_and_preserves_the_actual_outbox_origin() {
    let (mut core, limits) = make_tree();
    let _out = step(
        &mut core,
        &Env { now: Time::ZERO, wall: Wall::from_nanos(9), limits },
        Event::EffectConnector(connector::Event::Outbox {
            connector: 2,
            task: 2,
            entry: 8,
            outcome: connector::OutboxOutcome::Failed,
        }),
    );
    let row = core.tasks.task(2).unwrap();
    let tasks::Phase::Held { why, source: Some(origin), .. } = &row.phase else { panic!("held failed effect") };
    assert_eq!(*why, tasks::Hold::EffectFailed);
    assert_eq!(**origin, tasks::HoldSource::Effect { connector: 2, entry: 8 });
    let Document::Task(task) = make(&core, &limits, 1, &Subject::Task { task: 2 }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!("held task document")
    };
    let hold = task.hold.unwrap();
    assert_eq!(hold.account, Some(Part { connector: 2, kind: PartKind::Hold, key: PartKey::Number(2) }));
    assert_eq!(hold.source, Some(Part { connector: 2, kind: PartKind::Outbox, key: PartKey::Number(8) }));
    assert_eq!(hold.reason, tasks::Hold::EffectFailed);
    assert_eq!(hold.revision, row.revisions.hold);
    let Document::Escalation(escalation) =
        make(&core, &limits, 1, &Subject::Escalation { task: 2 }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!("escalation")
    };
    assert_eq!(escalation.task.hold.unwrap().account, hold.account);
}

struct ProjectionWriterFixture {
    core: Core,
    limits: Limits,
    resources: [tasks::Name; 2],
    entries: [u64; 2],
}

fn projection_writers(procedure: bool) -> ProjectionWriterFixture {
    let (mut core, limits) = tree_core();
    let resources = [b"first".as_slice(), b"other".as_slice()]
        .map(|path| tasks::Name { connector: 2, path: Box::new([Box::from(path)]) });
    let hold = tasks::HoldKind::Exclusive { taken: tasks::Taken::Waits };
    send_task(
        &mut core,
        &limits,
        tasks::Event::Kinds { connector: 2, kinds: Box::new([tasks::Kind { connector: 2, kind: 1, hold }]) },
    );
    for resource in &resources {
        send_task(&mut core, &limits, tasks::Event::Resource { name: resource.clone(), hold });
        send_task(
            &mut core,
            &limits,
            tasks::Event::ResourceAccess {
                project: 1,
                name: resource.clone(),
                access: tasks::ResourceAccess::Writable,
            },
        );
    }
    let mut holder = new_row(&core, 1, tasks::Funder::Period { project: 1, period: 1 }, None);
    holder.holdings = Box::new([
        tasks::Holding::Write { resource: resources[0].clone(), kind: 1 },
        tasks::Holding::Write { resource: resources[1].clone(), kind: 1 },
    ]);
    if procedure {
        holder.executor = tasks::Executor::Procedure { connector: 2, code: 1 };
    }
    send_task(
        &mut core,
        &limits,
        tasks::Event::Make {
            reply_to: ReplyTo::new(Token::new(1)),
            creator: tasks::Party::Person(1),
            batch: Box::new([holder]),
        },
    );
    assert_eq!(core.tasks.holder(&resources[0]), Some(1));
    if procedure {
        send_task(
            &mut core,
            &limits,
            tasks::Event::Procedure {
                reply_to: ReplyTo::new(Token::new(2)),
                task: 1,
                step: 1,
                read: None,
                decision: tasks::ProcedureDecision::new(tasks::ProcedureEnd::Wait),
            },
        );
        assert_eq!(core.tasks.procedure_due(1), None);
    }
    let entries = [
        fresh(&mut core.counters, Family::ConnectorRow).unwrap(),
        fresh(&mut core.counters, Family::ConnectorRow).unwrap(),
    ];
    for (resource, entry) in resources.iter().zip(entries) {
        send_task(
            &mut core,
            &limits,
            tasks::Event::EffectInFlight {
                reply_to: ReplyTo::new(Token::new(3)),
                task: 1,
                resource: resource.clone(),
                entry,
            },
        );
        assert_eq!(core.tasks.writer(resource).unwrap().writer, tasks::Writer::Effect { entry });
    }
    ProjectionWriterFixture { core, limits, resources, entries }
}

#[test]
fn projection_outcomes_settle_only_their_writers_without_holding_or_waking_a_procedure() {
    use connector::OutboxOutcome;
    for (outcome, terminal) in [
        (OutboxOutcome::Made, true),
        (OutboxOutcome::Failed, true),
        (OutboxOutcome::Withdrawn, true),
        (OutboxOutcome::NotMade, true),
        (OutboxOutcome::Uncertain, false),
        (OutboxOutcome::Held { entry: 1 }, false),
    ] {
        let ProjectionWriterFixture { mut core, limits, resources, entries } = projection_writers(true);
        let before = core.tasks.task(1).unwrap().clone();
        let key = CallKey { task: 1, attempt: 1, completion: 1, position: 0 };
        let part = CallPart::Effect { connector: 2, entry: entries[1], deadline: Wall::EPOCH, outcome: None };
        core.call_parts.insert(key, part.clone()).unwrap();
        let env = Env { now: Time::ZERO, wall: Wall::from_nanos(9), limits };
        for _ in 0_u8..2 {
            let Requests::Out(mut out) = step(
                &mut core,
                &env,
                Event::EffectConnector(connector::Event::ProjectionOutbox { connector: 2, entry: entries[0], outcome }),
            );
            while let Some(request) = out.pop() {
                if let Request::Held(held) = &request
                    && let Held::CallAnswer { .. } = **held
                {
                    panic!("projection answered a call");
                }
                if let Request::Ask { ask: Ask::StartProcedure { .. }, .. } = request {
                    panic!("projection started a procedure");
                }
            }
            assert_eq!(core.tasks.writer(&resources[0]).is_none(), terminal, "{outcome:?}");
            assert_eq!(core.tasks.writer(&resources[1]).unwrap().writer, tasks::Writer::Effect { entry: entries[1] });
            assert_eq!(core.tasks.task(1), Some(&before));
            assert_eq!(core.tasks.procedure_due(1), None);
            assert_eq!(core.call_parts.get(&key), Some(&part));
        }
        if !terminal {
            send_task(
                &mut core,
                &limits,
                tasks::Event::EffectInFlight {
                    reply_to: ReplyTo::new(Token::new(4)),
                    task: 1,
                    resource: resources[0].clone(),
                    entry: entries[0],
                },
            );
            assert_eq!(core.tasks.writer(&resources[0]).unwrap().writer, tasks::Writer::Effect { entry: entries[0] });
        }
    }
}

#[test]
fn projection_outcomes_ignore_unconfigured_connectors_and_unallocated_entries() {
    let ProjectionWriterFixture { mut core, limits, resources, entries } = projection_writers(false);
    for (connector, entry) in [(3, entries[0]), (2, 0), (2, entries[1] + 1)] {
        let _out = step(
            &mut core,
            &Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            Event::EffectConnector(connector::Event::ProjectionOutbox {
                connector,
                entry,
                outcome: connector::OutboxOutcome::Made,
            }),
        );
        assert!(core.tasks.writer(&resources[0]).is_some());
        assert!(core.tasks.writer(&resources[1]).is_some());
    }
}

#[test]
fn ordinary_writer_outcomes_still_hold_agents_and_wake_procedures() {
    use connector::OutboxOutcome;
    for (procedure, outcome) in [
        (false, OutboxOutcome::Failed),
        (false, OutboxOutcome::Held { entry: 1 }),
        (true, OutboxOutcome::Failed),
        (true, OutboxOutcome::Held { entry: 1 }),
    ] {
        let ProjectionWriterFixture { mut core, limits, resources, entries } = projection_writers(procedure);
        let _out = step(
            &mut core,
            &Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            Event::EffectConnector(connector::Event::Outbox { connector: 2, task: 1, entry: entries[0], outcome }),
        );
        if procedure && outcome == OutboxOutcome::Failed {
            assert!(core.tasks.procedure_due(1).is_some());
        } else {
            let tasks::Phase::Held { source, .. } = &core.tasks.task(1).unwrap().phase else { panic!("ordinary hold") };
            assert_eq!(source.as_deref(), Some(&tasks::HoldSource::Effect { connector: 2, entry: entries[0] }));
        }
        assert_eq!(core.tasks.writer(&resources[0]).is_none(), outcome == OutboxOutcome::Failed);
    }
}

#[test]
fn reported_projection_recovery_fences_stale_none_callbacks_and_releases_only_the_covered_writer() {
    let ProjectionWriterFixture { mut core, limits, resources, entries } = projection_writers(true);
    let mut goal = core.tasks.task(1).unwrap().clone();
    goal.tracked = Some(1);
    let mut saved = Queue::with_capacity(core.room_max.writes);
    projections::save(&mut core, &limits, &goal, &mut saved);
    assert!(projection_failures::restore(
        &mut core,
        ProjectionFailure {
            goal: 1,
            connector: 2,
            after: 0,
            state: ProjectionFailureState::Reported(ProjectionFailureSource { source: entries[0], at: Wall::EPOCH })
        }
    ));
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let owner = Token::new(901);
    drop(step(
        &mut core,
        &env,
        Event::EffectStart {
            owner,
            connector: 2,
            origin: EffectOrigin::Projection { goal: 1, source: entries[0], entry: None },
        },
    ));
    assert_eq!(core.tasks.writer(&resources[0]).unwrap().writer, tasks::Writer::Effect { entry: entries[0] });
    assert_eq!(core.tasks.writer(&resources[1]).unwrap().writer, tasks::Writer::Effect { entry: entries[1] });
    let phase = core.tasks.task(1).unwrap().phase.clone();
    let Requests::Out(mut recovered) =
        step(&mut core, &env, Event::ProjectionRecover { goal: 1, connector: 2, owner: Token::new(902) });
    let mut reset = false;
    while let Some(request) = recovered.pop() {
        if let Request::Ask { connector: 2, ask: Ask::CommitProjectionRecovery { goal: 1, after, .. } } = request {
            assert_eq!(after, entries[1]);
            reset = true;
            // This is the reset's existing writer-only callback. The paired
            // root world proves its writes share the actual fence commit.
            drop(step(
                &mut core,
                &env,
                Event::EffectConnector(connector::Event::ProjectionOutbox {
                    connector: 2,
                    entry: entries[0],
                    outcome: connector::OutboxOutcome::NotMade,
                }),
            ));
        }
    }
    assert!(reset);
    assert!(core.tasks.writer(&resources[0]).is_none());
    assert_eq!(core.tasks.writer(&resources[1]).unwrap().writer, tasks::Writer::Effect { entry: entries[1] });
    assert_eq!(core.tasks.task(1).unwrap().phase, phase, "writer-only reset does not hold or wake its task");
    drop(step(&mut core, &env, Event::EffectConnector(connector::Event::DescribeRefused { owner })));
    assert_eq!(
        *core.projection_failures.get(&(1, 2)).unwrap(),
        ProjectionFailure { goal: 1, connector: 2, after: entries[1], state: ProjectionFailureState::Armed }
    );
    let fresh = begin_projection(&mut core, &limits, 1, 2, 903).unwrap();
    assert!(fresh > entries[1]);
    drop(step(&mut core, &env, Event::ProjectionFailed { goal: 1, connector: 2, source: fresh }));
    let ProjectionFailureState::Pending(source) = core.projection_failures.get(&(1, 2)).unwrap().state else {
        panic!("new source opens a pending episode")
    };
    assert_eq!(source.source, fresh);
}
