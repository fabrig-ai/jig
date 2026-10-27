use super::*;
use crate::Bounds;
use crate::Decision as ApplicationDecision;
use fixtures::query;
use jig_client_domain::contract::Decision;
use jig_client_domain::contract::*;
use skein_lib::List;
mod frame_facts;
mod owner_project;
mod policy;
mod recurring;
fn text(bytes: &[u8]) -> Text {
    Text::new(bytes.into(), 4096).expect("fixture text fits")
}
fn list<T>(items: Vec<T>) -> List<T> {
    let mut out = List::with_capacity(u32::try_from(items.len()).expect("fixture list fits"));
    for item in items {
        assert!(out.push(item).is_ok());
    }
    out
}
fn party(number: u64) -> Party {
    Party { number, kind: PartyKind::Person, name: text(b"party") }
}
fn budget() -> Budget {
    Budget { total: 100, own: 0, ended: 0, reserved: 0, left: 100, funder: Actor::Deployment }
}
fn frame(number: u64) -> Me {
    Me {
        providers: skein_lib::List::with_capacity(0),
        providers_cut: false,
        key_bytes: 256,
        redirect_bytes: 4096,
        party: MeParty {
            number,
            kind: PartyKind::Person,
            login: DocumentText::complete(text(b"fixture-login")),
            name: DocumentText::complete(text(b"party")),
        },
        projects: list(vec![MeProject {
            models: skein_lib::List::with_capacity(0),
            templates: skein_lib::List::with_capacity(0),
            creating: None,
            project: 1,
            name: DocumentText::complete(text(b"project")),
            role: HeldRole::Policy { role: 1 },
            pool: Some(PoolLedger {
                source: Funder::Pool { project: 1, person: number, period: 1 },
                parent: None,
                numbers: OwnerNumbers { budget: 100, spent: 0, spent_below: 0, reserved: 0 },
                left: 100,
                closed: false,
                began: None,
                next: None,
            }),
        }]),
        needs: Needs { questions: 0, proposals: 0, escalations: 0, person_tasks: 0 },
        needs_total: 0,
        results_seen: 0,
        cut: false,
        updates: Count { value: 0, limited: false },

        retention: Duration::from_secs(1),
        deployment_role: Some(HeldRole::Policy { role: 1 }),
    }
}
pub(crate) fn owner_authority() -> OwnerAuthorityDocument {
    OwnerAuthorityDocument {
        authority: OwnerAuthority {
            models: None,
            tools: 0,
            grants: List::with_capacity(0),
            delegation: OwnerDelegation { kinds: List::with_capacity(0), tasks: 10, depth: 10 },
            spend: 100,
            deadline: None,
            notes: 0,
            note_resources: List::with_capacity(0),
        },
        cut: false,
    }
}
fn authority() -> Authority {
    Authority {
        models: None,
        budget: 100,
        grants: list(vec![]),
        task_kinds: list(vec![]),
        depth: 10,
        tasks: 10,
        deadline: None,
        tools: list(vec![]),
        note_scopes: list(vec![]),
    }
}
fn script() -> Script {
    let mut truth = Truth::default();
    truth.frames.insert(1, frame(1));
    truth.frames.insert(2, frame(2));
    for frame in truth.frames.values_mut() {
        fixtures::creating(frame);
    }
    truth.sign_ins.insert(11, 1);
    truth.sign_ins.insert(12, 1);
    truth.sign_ins.insert(22, 2);
    Script { truth, ..Script::default() }
}
fn send(engine: &mut Engine, now: u64, sign_in: u64, key: u8, token: u64, ask: Ask) {
    engine
        .send(Time::from_nanos(now), Wall::from_nanos(now), sign_in, [key; 16], Token::new(token), ask)
        .expect("typed request fits");
}
fn final_answer(up: &Up) -> &Answer {
    match up {
        Up::Answer { envelope: Envelope::Final, answer: Some(answer), .. } => answer,
        Up::ApplicationAnswer { .. }
        | Up::ApplicationRead { .. }
        | Up::App { .. }
        | Up::Read { .. }
        | Up::Watch { .. }
        | Up::Ended { .. }
        | Up::Answer { .. } => {
            panic!("expected a final answer: {up:?}")
        }
    }
}
fn proposal() -> Proposal {
    Proposal {
        key: ProposalKey { proposer: Proposer::Task(8), number: 1 },
        project: 1,
        proposer: Proposer::Task(8),
        why: text(b"wider"),
        action: ProposalAction::Batch { tasks: list(vec![]), own: false },
        needs: authority(),
        funding: budget(),
        waiting: Recipient::Role { project: 1, role: 1 },
        trail: list(vec![]),
        since: Wall::EPOCH,
        acts: list(vec![Offered { effect: None, act: ActKind::Accept, admission: Admission::Allowed }]),
        ending: None,
    }
}
#[test]
fn keyed_contract_durability_sign_ins_conflict_and_decided_first() {
    let mut setup = script();
    let p = proposal();
    setup.truth.objects.insert(ObjectKey::Proposal(p.key), ReadValue::Proposal(Box::new(p.clone())));
    let ask = Ask::Decide {
        project: 1,
        on: DecideOn::Proposal { proposal: p.key, decision: Decision::Accepted, running: None },
    };
    let mut engine = Engine::new(setup, 1).expect("valid script");
    send(&mut engine, 0, 11, 1, 1, ask.clone());
    send(&mut engine, 1, 12, 1, 2, ask.clone());
    send(&mut engine, 2, 22, 2, 3, ask.clone());
    assert_eq!(engine.next_due(), Some(Time::from_nanos(10_000_000)));
    assert!(engine.poll(Time::from_nanos(9_999_999)).is_empty());
    assert!(engine.commits().is_empty());
    let up = engine.poll(Time::from_nanos(10_000_002));
    assert_eq!(up.len(), 3);
    assert_eq!(final_answer(&up[0]), final_answer(&up[1]));
    assert_eq!(
        final_answer(&up[2]),
        &Answer::Done(Outcome::DecidedFirst { by: party(1), how: Decision::Accepted, at: Wall::EPOCH })
    );
    assert_eq!(engine.commits().len(), 2);
    assert_eq!(engine.commits()[0].durable, Some(Time::from_nanos(10_000_000)));
    send(
        &mut engine,
        20_000_000,
        11,
        1,
        4,
        Ask::Decide {
            project: 1,
            on: DecideOn::Proposal { proposal: p.key, decision: Decision::PassedUp, running: None },
        },
    );
    assert_eq!(
        final_answer(&engine.poll(Time::from_nanos(20_000_000))[0]),
        &Answer::Refused { refusal: Refusal::KeyConflict, at: None }
    );
    send(&mut engine, 20_000_000, 11, 1, 5, ask);
    assert_eq!(final_answer(&engine.poll(Time::from_nanos(20_000_000))[0]), final_answer(&up[0]));
    assert_eq!(engine.commits().len(), 2);
}
fn nested_chip_payload_bounds(chip: &Chip) {
    for details in [
        ChipPhase::Running {
            attempt: 1,
            since: Wall::EPOCH,
            tool: Some(Text::new(vec![0; 4097].into_boxed_slice(), 5000).unwrap()),
        },
        ChipPhase::Idle {
            wake: WakePolicy {
                words: true,
                results: true,
                topics: List::with_capacity(33),
                subscriptions: list(vec![]),
            },
        },
    ] {
        let mut invalid_setup = script();
        let mut invalid_chip = chip.clone();
        *invalid_chip.details = details;
        invalid_setup.truth.object_projects.insert(ObjectKey::Question { task: 8, message: 1 }, 1);
        invalid_setup.truth.objects.insert(
            ObjectKey::Question { task: 8, message: 1 },
            ReadValue::Question(Box::new(Question {
                task: invalid_chip,
                from: None,
                message: 1,
                words: text(b"question"),
                since: Wall::EPOCH,
                acts: list(vec![]),
                ending: None,
            })),
        );
        assert_eq!(
            Engine::new(invalid_setup, 1).err(),
            Some(Invalid::Payload),
            "nested chip phase facts are admitted whole"
        );
    }
}

#[test]
#[expect(clippy::too_many_lines, reason = "whole revision and retention lifecycle retains all independent assertions")]
fn revision_refusals_scripts_retention_and_whole_payload_admission() {
    bounded_offer_payloads_and_predictions();
    let mut setup = script();
    let chip = Chip {
        task: 8,
        title: DocumentText::complete(text(b"held")),
        executor: Executor::Agent { charter: 1 },
        phase: Phase::Held,
        details: Box::new(ChipPhase::Held {
            reason: HoldReason::Budget { spent: 100, limit: 100 },
            since: Wall::EPOCH,
            revision: Revision { value: 2 },
            was: None,
        }),
    };
    nested_chip_payload_bounds(&chip);
    let escalation = Escalation {
        ending: None,
        task: chip,
        hold: Hold {
            placed: None,
            revision: Revision { value: 2 },
            reason: HoldReason::Budget { spent: 100, limit: 100 },
            before: Phase::Running,
            since: Wall::EPOCH,
            result: None,
            spend: list(vec![SpendLine { task: 8, attempt: None, spent: 100 }]),
        },
        steering: Box::new(Steering {
            person_revision: None,
            taken_by: None,
            addressed: None,
            revision: Revision { value: 1 },
            spec: specification(text(b"held work")),
            instructions: Some(DocumentText::complete(text(b""))),
            wake: OwnerWakePolicy {
                words: WakeRule::Immediate,
                notices: WakeRule::Never,
                news: WakeRule::Never,
                results: ResultsWake::Each,
                questions: false,
                answers: false,
                timers: false,
            },
            dependencies: List::with_capacity(0),
            authority: owner_authority(),
            budget: Budget { total: 100, own: 100, ended: 0, reserved: 0, left: 0, funder: Actor::Party(1) },
        }),
        impact: Impact {
            tasks: PhaseCounts {
                ended: 0,
                waiting: 0,
                running: 0,
                held: 1,
                closing: 0,
                done: 0,
                failed: 0,
                cancelled: 0,
            },
            live_runs: 0,
            proposals: 0,
        },
        waiting: Recipient::Party(1),
        trail: list(vec![]),
        acts: list(vec![Offered { effect: None, act: ActKind::Release, admission: Admission::Allowed }]),
    };
    nested_escalation_payload_bounds(&escalation);
    setup.truth.object_projects.insert(ObjectKey::Escalation(8), 1);
    setup.truth.objects.insert(ObjectKey::Escalation(8), ReadValue::Escalation(Box::new(escalation)));
    let mut engine = Engine::new(setup, 2).expect("valid script");
    let ask = Ask::Decide {
        project: 1,
        on: DecideOn::Escalation {
            task: 8,
            revision: Revision { value: 1 },
            decision: Decision::Released { again: None },
        },
    };
    send(&mut engine, 0, 11, 1, 1, ask);
    assert_eq!(
        final_answer(&engine.poll(Time::from_nanos(10_000_000))[0]),
        &Answer::Refused {
            refusal: Refusal::Moved {
                now: Revision { value: 2 },
                what: Moved::Hold {
                    reason: Some(CurrentHold::Document(Box::new(HoldReason::Budget { spent: 100, limit: 100 })))
                }
            },
            at: None
        }
    );
    let mut setup = script();
    let ask = Ask::MakeService { project: 1, name: text(b"service"), role: HeldRole::Owner };
    setup.rules.push(Rule {
        party: None,
        ask: ask.clone(),
        answer: Answer::Refused {
            refusal: Refusal::Authority { lacking: Lacking::Budget { needed: 100, left: 5 }, proposable: None },
            at: None,
        },
        changes: vec![],
    });
    let mut engine = Engine::new(setup, 3).expect("valid script");
    send(&mut engine, 0, 11, 1, 1, ask.clone());
    assert!(matches!(
        final_answer(&engine.poll(Time::from_nanos(10_000_000))[0]),
        Answer::Refused { refusal: Refusal::Authority { .. }, at: None }
    ));
    send(&mut engine, 2_000_000_000, 11, 1, 2, ask);
    drop(engine.poll(Time::from_nanos(2_010_000_000)));
    assert_eq!(engine.commits().len(), 2, "retention expired before reuse");
    let mut setup = script();
    setup.bounds.document_bytes = 1;
    assert_eq!(Engine::new(setup, 1).err(), Some(Invalid::Payload));
    let mut setup = script();
    setup.truth.frames.get_mut(&1).expect("frame").projects = List::with_capacity(33);
    assert_eq!(Engine::new(setup, 1).err(), Some(Invalid::Payload), "unused list capacity is part of a whole payload");
    let mut engine = Engine::new(script(), 1).expect("valid script");
    let ask =
        fixtures::create(1, Text::new(vec![0; 4097].into_boxed_slice(), 5000).expect("larger producer permits text"));
    assert_eq!(engine.send(Time::ZERO, Wall::EPOCH, 11, [1; 16], Token::new(1), ask), Err(Invalid::Payload));
    assert!(engine.commits().is_empty());
}
fn counts() -> PhaseCounts {
    PhaseCounts { ended: 0, waiting: 0, running: 0, held: 0, closing: 0, done: 0, failed: 0, cancelled: 0 }
}
fn task() -> Task {
    Task {
        dependency_facts: skein_lib::List::with_capacity(0),
        kept_account: None,
        run_wait: None,
        settings: Some(TaskSettings { task: 1, run: None, newest: 1 }),
        llm: None,
        label: 0,
        flags: TaskFlags { ended_by: EndedBy::Result, conversation: false, listed: true, retention: Retention::Kept },
        last_activity: Wall::EPOCH,
        waits_on: WaitsOn::Nothing,
        chip: Chip {
            task: 8,
            title: DocumentText::complete(text(b"chat")),
            executor: Executor::Agent { charter: 1 },
            phase: Phase::Running,
            details: Box::new(ChipPhase::Running { attempt: 1, since: Wall::EPOCH, tool: None }),
        },
        goal: None,
        priority: None,
        lineage_chips: list(vec![]),
        inputs: list(vec![]),
        project: 1,
        requester: OwnerRequester::Person(1),
        lineage: list(vec![]),
        spec: specification(text(b"spec")),
        spec_revision: Revision { value: 1 },
        person_revision: None,
        taken_by: None,
        instructions: Some(DocumentText::complete(text(b"instructions"))),
        wake: OwnerWakePolicy {
            words: WakeRule::Immediate,
            notices: WakeRule::Never,
            news: WakeRule::Never,
            results: ResultsWake::Each,
            questions: false,
            answers: false,
            timers: false,
        },
        subscriptions: Subscriptions { entries: List::with_capacity(0) },
        result_contract: TaskContractDocument::Report { words: 32 },
        dependencies: list(vec![]),
        cut: false,
        references: list(vec![]),
        resources: list(vec![]),
        budget: OwnerBudget {
            numbers: OwnerNumbers { budget: 100, spent: 0, spent_below: 0, reserved: 0 },
            left: 100,
            funder: Funder::Period { project: 1, period: 3 },
            deadline: None,
        },
        authority: owner_authority(),
        hold: None,
        proposals: list(vec![]),
        activity: 0,
        changed: Wall::EPOCH,
        acts: list(vec![]),
        result: None,
        delegates: list(vec![]),

        context: Some(Box::new(jig_client_domain::contract::TaskContext {
            escalation: None,
            parts: list(vec![]),
            closing: None,
            spend: list(vec![]),
            runtime: Runtime::Running { attempt: 1, since: Wall::EPOCH, tool: None },
            impact: Impact { tasks: counts(), live_runs: 1, proposals: 0 },
            writer_slots: list(vec![]),
        })),
    }
}
fn live_script() -> Script {
    let mut setup = script();
    setup.truth.objects.insert(ObjectKey::Task(8), ReadValue::Task(Box::new(task())));
    setup.truth.trees.insert(8, Tree { task: 8, nodes: list(vec![]), folded: counts(), held_below: 0 });
    setup.truth.deployment = Some(Deployment {
        hosts: list(vec![]),
        accounts: list(vec![]),
        capacity: list(vec![]),
        proposals: 0,
        refusals: list(vec![]),
        parts: list(vec![]),
    });
    setup
}
fn at(nanos: u64) -> Time {
    Time::from_nanos(nanos)
}
fn scenario_pages_and_read_barriers() {
    let mut setup = live_script();
    setup.bounds.rows = 1;
    setup.truth.sources.push(Source {
        party: 1,
        read: Read::History { task: 8, subtree: false, after: None },
        rows: Rows::History(
            (0..3)
                .map(|position| {
                    HistoryRow::Line(Box::new(HistoryLine {
                        position,
                        task: 8,
                        by: Actor::Party(1),
                        at: Wall::EPOCH,
                        change: HistoryChange::Released,
                    }))
                })
                .collect(),
        ),
    });
    let mut engine = Engine::new(setup, 4).expect("source rows span pages");
    let mut changed = task();
    changed.chip.title = DocumentText::complete(text(b"latest"));
    engine
        .act(
            Time::ZERO,
            Wall::EPOCH,
            Change::Object { project: 1, key: ObjectKey::Task(8), value: ReadValue::Task(Box::new(changed.clone())) },
        )
        .expect("whole task");
    engine.read(Time::ZERO, 11, Token::new(1), Read::Task { task: 8 }).expect("read");
    assert!(engine.poll(at(9_999_999)).is_empty());
    assert_eq!(
        engine.poll(at(10_000_000)),
        vec![Up::Read { token: Token::new(1), terminal: ReadResult::Value(ReadValue::Task(Box::new(changed))) }]
    );
    assert_eq!(engine.read(at(10_000_000), 11, Token::new(1), Read::Me), Err(Invalid::Identity));
    let mut cursor = None;
    for position in 0..3 {
        engine
            .read(
                at(10_000_000),
                11,
                Token::new(10 + position),
                Read::History { task: 8, subtree: false, after: cursor },
            )
            .expect("page");
        let up = engine.poll(at(10_000_000));
        let Up::Read { terminal: ReadResult::Value(ReadValue::History(page)), .. } = &up[0] else {
            panic!("history page");
        };
        assert_eq!(page.rows.len(), 1);
        let HistoryRow::Line(row) = page.rows.iter().next().expect("row") else {
            panic!("supplied legacy history");
        };
        assert_eq!(row.position, position);
        assert_eq!(page.before.is_some(), position != 0);
        assert_eq!(page.after.is_some(), position != 2);
        cursor = page.after.clone();
    }
    engine
        .read(at(10_000_000), 11, Token::new(20), Read::History { task: 8, subtree: false, after: None })
        .expect("page");
    let up = engine.poll(at(10_000_000));
    let Up::Read { terminal: ReadResult::Value(ReadValue::History(page)), .. } = &up[0] else {
        panic!("history page");
    };
    let cursor = page.after.clone();
    engine
        .read(at(10_000_000), 22, Token::new(21), Read::History { task: 8, subtree: false, after: cursor })
        .expect("foreign continuation");
    assert_eq!(
        engine.poll(at(10_000_000)),
        vec![Up::Read { token: Token::new(21), terminal: ReadResult::None(ReadNone::Gone) }]
    );
    engine.read(at(10_000_000), 999, Token::new(22), Read::Me).expect("signed out");
    assert!(matches!(engine.poll(at(10_000_000))[0], Up::Read { terminal: ReadResult::None(ReadNone::SignedOut), .. }));
    let mut frame = frame(2);
    frame.projects = list(vec![]);
    engine.act(at(10_000_000), Wall::EPOCH, Change::Me(frame)).expect("roles removed");
    engine.read(at(10_000_000), 22, Token::new(23), Read::Task { task: 8 }).expect("hidden read");
    assert_eq!(
        engine.poll(at(20_000_000)),
        vec![Up::Read { token: Token::new(23), terminal: ReadResult::None(ReadNone::Unknown) }]
    );
}
fn scenario_watch_subjects_coalescing_and_lifecycle() {
    let mut engine = Engine::new(live_script(), 5).expect("live truth");
    let subjects = vec![
        Subject::Me,
        Subject::Task { task: 8 },
        Subject::Tree { task: 8, window: 1 },
        Subject::Chips { tasks: list(vec![8]) },
        Subject::Inbox { window: 1 },
        Subject::Tasks { query: query(1), window: 1 },
        Subject::Run { task: 8, turn: None, offset: 0 },
        Subject::Deployment,
    ];
    for (index, subject) in subjects.into_iter().enumerate() {
        engine.open(Time::ZERO, 11, Token::new(index as u64), subject).expect("subject");
    }
    let up = engine.poll(Time::ZERO);
    assert_eq!(up.iter().filter(|event| matches!(event, Up::Watch { event: WatchEvent::Opened, .. })).count(), 8);
    assert_eq!(
        up.iter().filter(|event| matches!(event, Up::Watch { event: WatchEvent::Delivered(_), .. })).count(),
        7,
        "idle run has no fabricated turn"
    );
    let mut changed = task();
    changed.chip.title = DocumentText::complete(text(b"one"));
    engine
        .act(
            Time::ZERO,
            Wall::EPOCH,
            Change::Object { project: 1, key: ObjectKey::Task(8), value: ReadValue::Task(Box::new(changed.clone())) },
        )
        .expect("one");
    changed.chip.title = DocumentText::complete(text(b"two"));
    engine
        .act(
            at(1),
            Wall::EPOCH,
            Change::Object { project: 1, key: ObjectKey::Task(8), value: ReadValue::Task(Box::new(changed.clone())) },
        )
        .expect("two");
    assert!(engine.poll(at(10_000_001)).is_empty(), "in-flight coalesces");
    engine.taken(Token::new(1));
    let up = engine.poll(at(10_000_001));
    assert_eq!(
        up,
        vec![Up::Watch { token: Token::new(1), event: WatchEvent::Delivered(Delivery::Task(Box::new(changed))) }]
    );
    engine.taken(Token::new(1));
    assert!(engine.poll(at(10_000_001)).is_empty());
    let up = engine.poll(at(1_010_000_001));
    assert_eq!(up.iter().filter(|event| matches!(event, Up::Watch { event: WatchEvent::Alive, .. })).count(), 8);
    engine.close(at(1_010_000_001), Token::new(1));
    engine.close(at(1_010_000_001), Token::new(1));
    assert_eq!(engine.poll(at(1_010_000_001)), vec![Up::Ended { token: Token::new(1), why: WatchEnd::Closed }]);
    let mut changed = task();
    changed.chip.phase = Phase::Done;
    *changed.chip.details = ChipPhase::Ended { how: Ending::Done, at: Wall::EPOCH };
    changed.context.as_mut().expect("supplied task context").runtime = Runtime::Ended;
    engine
        .act(
            at(1_010_000_001),
            Wall::EPOCH,
            Change::Object { project: 1, key: ObjectKey::Task(8), value: ReadValue::Task(Box::new(changed)) },
        )
        .expect("ended");
    let up = engine.poll(at(1_020_000_001));
    for id in [2, 3, 6] {
        assert!(up.contains(&Up::Ended { token: Token::new(id), why: WatchEnd::SubjectEnded }));
    }
    let mut setup = live_script();
    setup.bounds.watches = 1;
    let mut engine = Engine::new(setup, 1).expect("limited engine");
    engine.open(Time::ZERO, 11, Token::new(1), Subject::Me).expect("first");
    engine.open(Time::ZERO, 11, Token::new(2), Subject::Me).expect("over limit");
    assert!(engine.poll(Time::ZERO).contains(&Up::Ended { token: Token::new(2), why: WatchEnd::Busy }));
    engine.act(Time::ZERO, Wall::EPOCH, Change::SignedOut { sign_in: 11 }).expect("signed out");
    assert_eq!(engine.poll(at(10_000_000)), vec![Up::Ended { token: Token::new(1), why: WatchEnd::SignedOut }]);
}
fn scenario_run_offsets_cut_names_and_activity() {
    let mut setup = live_script();
    setup.bounds.text = 32;
    let mut engine = Engine::new(setup, 6).expect("run bound");
    let name = TurnName { attempt: 1, number: 0 };
    engine.open(Time::ZERO, 11, Token::new(1), Subject::Run { task: 8, turn: None, offset: 0 }).expect("run watch");
    drop(engine.poll(Time::ZERO));
    engine.write(Time::ZERO, 8, name, Wall::EPOCH, &text(b"abcdefghijklmnopqrst"), None).expect("first chunk");
    let up = engine.poll(Time::ZERO);
    let Up::Watch { event: WatchEvent::Delivered(Delivery::Run(run)), .. } = &up[0] else {
        panic!("run text");
    };
    assert_eq!((run.from, run.through, run.cut), (0, 20, false));
    assert_eq!(run.text.bytes(), b"abcdefghijklmnopqrst");
    engine.write(at(1), 8, name, Wall::EPOCH, &text(b"uvwxyz0123456789ABCD"), None).expect("second chunk");
    assert!(engine.poll(at(1)).is_empty());
    engine.continue_run(Token::new(1), Some(provisional(name)), 20).expect("same watch cursor");
    let up = engine.poll(at(1));
    let Up::Watch { event: WatchEvent::Delivered(Delivery::Run(run)), .. } = &up[0] else {
        panic!("run text");
    };
    assert_eq!((run.from, run.through, run.cut), (20, 40, false), "a valid acknowledged suffix needs no replacement");
    assert_eq!(run.text.bytes(), b"uvwxyz0123456789ABCD");
    engine.continue_run(Token::new(1), Some(provisional(name)), 0).expect("lagging offset");
    let up = engine.poll(at(1));
    let Up::Watch { event: WatchEvent::Delivered(Delivery::Run(run)), .. } = &up[0] else {
        panic!("cut text");
    };
    assert_eq!((run.from, run.through), (8, 40));
    assert!(run.cut, "the actual lagging origin still proves retained-prefix loss");
    let second = TurnName { attempt: 1, number: 1 };
    engine.write(at(2), 8, second, Wall::EPOCH, &text(b"second"), None).expect("new turn");
    engine.continue_run(Token::new(1), Some(provisional(name)), 40).expect("old turn position");
    let up = engine.poll(at(2));
    let Up::Watch { event: WatchEvent::Delivered(Delivery::Run(run)), .. } = &up[0] else {
        panic!("new turn text");
    };
    assert_eq!((run.turn, run.from, run.through), (provisional(second), 0, 6));
    engine
        .act(
            at(2),
            Wall::EPOCH,
            Change::Source(Source {
                party: 1,
                read: Read::Activity { task: 8, before: None },
                rows: Rows::Activity(vec![ActivityLine {
                    position: 1,
                    at: Wall::EPOCH,
                    event: Activity::Turn {
                        name: second,
                        turn: TurnRead::Decoded(Turn {
                            messages: list(vec![]),
                            used: Usage { input: 1, output: 1, cached: 0, cache_written: 0 },
                            spent: 1,
                            read_through: None,
                            cut: false,
                        }),
                    },
                }]),
            }),
        )
        .expect("turn committed through activity");
    assert_eq!(
        engine.truth().objects.get(&ObjectKey::Task(8)).and_then(|value| if let ReadValue::Task(task) = value {
            Some(task.activity)
        } else {
            None
        }),
        Some(1)
    );
    engine.read(at(2), 11, Token::new(2), Read::Activity { task: 8, before: None }).expect("activity");
    assert!(engine.poll(at(10_000_001)).is_empty());
    let up = engine.poll(at(10_000_002));
    assert!(
        matches!(&up[0],Up::Read { terminal:ReadResult::Value(ReadValue::Activity(page)),.. } if matches!(page.rows.iter().next().expect("turn").event,Activity::Turn { name,.. } if name == second))
    );
    assert!(!engine.runs.contains_key(&8), "durable matching activity replaces the provisional turn");
}
fn page_reply(position: u64) -> InboxEntry {
    let line = {
        let mut value = task();
        value.chip.title = DocumentText::complete(text(&[b'x'; 1100]));
        value.label = 1;
        value.flags = TaskFlags {
            ended_by: EndedBy::Result,
            conversation: true,
            listed: false,
            retention: Retention::ForgottenOnceRead,
        };
        crate::queries::line(
            &value,
            core::iter::empty(),
            Some(ConversationRead { seen: 0, unread: Count { value: 1, limited: false } }),
        )
    };
    InboxEntry {
        reply: Some(Box::new(line)),
        lineage: list(vec![]),
        goal: None,
        recipient: None,
        stall: None,
        object: ObjectKey::Task(8),
        project: 1,
        group: InboxGroup::Replies,
        position,
        at: Wall::EPOCH,
    }
}

fn scenario_byte_bound_pages_and_watch_windows() {
    let mut setup = live_script();
    setup.bounds.document_bytes = 3000;
    setup.truth.sources.push(Source {
        party: 1,
        read: Read::History { task: 8, subtree: false, after: None },
        rows: Rows::History(
            (0..2)
                .map(|position| {
                    HistoryRow::Line(Box::new(HistoryLine {
                        position,
                        task: 8,
                        by: Actor::Party(1),
                        at: Wall::EPOCH,
                        change: HistoryChange::Amended { before: text(&[b'a'; 1100]), after: text(&[b'b'; 1100]) },
                    }))
                })
                .collect(),
        ),
    });
    let mut engine = Engine::new(setup, 8).expect("each retained row fits");
    engine
        .read(Time::ZERO, 11, Token::new(1), Read::History { task: 8, subtree: false, after: None })
        .expect("byte bounded page");
    let up = engine.poll(Time::ZERO);
    let Up::Read { terminal: ReadResult::Value(value @ ReadValue::History(page)), .. } = &up[0] else {
        panic!("history page");
    };
    assert_eq!(page.rows.len(), 1, "byte bound shrinks below the row bound");
    assert!(page.after.is_some());
    assert!(payload::fits(value, &engine.bounds));
    let mut setup = live_script();
    setup.bounds.document_bytes = 3000;
    let entries = (0..2).map(page_reply).collect();
    setup.truth.sources.push(Source {
        party: 1,
        read: Read::Inbox {
            group: jig_client_domain::contract::InboxReadGroup::Updates,
            project: None,
            kind: None,
            goal: None,
            after: None,
        },
        rows: Rows::Inbox(entries),
    });
    let mut engine = Engine::new(setup, 9).expect("each whole reply row fits");
    engine.open(Time::ZERO, 11, Token::new(2), Subject::Inbox { window: 2 }).expect("inbox");
    let up = engine.poll(Time::ZERO);
    let Up::Watch { event: WatchEvent::Delivered(value @ Delivery::Inbox(inbox)), .. } = &up[1] else {
        panic!("inbox window");
    };
    assert_eq!(inbox.entries.len(), 1, "whole nested replies shrink the byte window");
    assert!(inbox.more);
    assert!(inbox.entries.iter().next().expect("entry").reply.is_some());
    assert!(payload::fits(value, &engine.bounds));
}
fn scenario_all_read_shapes_and_reverse_activity() {
    let mut setup = live_script();
    setup.bounds.rows = 1;
    setup.truth.sources.push(Source {
        party: 1,
        read: Read::Activity { task: 8, before: None },
        rows: Rows::Activity(
            (0..3)
                .map(|position| ActivityLine {
                    position,
                    at: Wall::EPOCH,
                    event: Activity::RunBegan { attempt: 1, how: RunBeginning::Resumed, why: text(b"run") },
                })
                .collect(),
        ),
    });
    let mut engine = Engine::new(setup, 7).expect("all reads");
    let mut cursor = None;
    for position in (0..3).rev() {
        engine
            .read(Time::ZERO, 11, Token::new(10 + position), Read::Activity { task: 8, before: cursor })
            .expect("reverse page");
        let up = engine.poll(Time::ZERO);
        let Up::Read { terminal: ReadResult::Value(ReadValue::Activity(page)), .. } = &up[0] else {
            panic!("activity");
        };
        assert_eq!(page.rows.iter().next().expect("row").position, position);
        assert_eq!(page.before.is_some(), position != 0);
        cursor = page.before.clone();
    }
    let reads = vec![
        Read::Me,
        Read::Task { task: 8 },
        Read::Proposal { proposal: ProposalKey { proposer: Proposer::Task(8), number: 1 } },
        Read::Escalation { task: 8 },
        Read::Question { task: 8, message: 1 },
        Read::PersonTask { task: 8 },
        Read::Result { task: 8 },
        Read::Ended { task: 8, after: None },
        Read::History { task: 8, subtree: true, after: None },
        Read::Inbox {
            group: jig_client_domain::contract::InboxReadGroup::Updates,
            kind: Some(InboxGroup::Replies),
            goal: None,
            project: None,
            after: None,
        },
        Read::Inbox {
            group: jig_client_domain::contract::InboxReadGroup::Needs,
            project: Some(1),
            kind: Some(InboxGroup::Questions),
            goal: None,
            after: None,
        },
        Read::Spend { project: 1, after: None },
        Read::Project { project: 1 },
        Read::Notes { scope: NoteScope::Project(1), description: Some(text(b"match")), after: None },
        Read::Deployment,
    ];
    for (index, read) in reads.into_iter().enumerate() {
        let token = Token::new(100 + index as u64);
        engine.read(Time::ZERO, 11, token, read).expect("contract read shape");
        let up = engine.poll(Time::ZERO);
        assert_eq!(up.len(), 1);
        assert!(matches!(up[0],Up::Read { token:id,.. } if id == token));
        assert!(engine.poll(Time::ZERO).is_empty());
    }
    scenario_reply_identity();
}
fn scenario_reply_identity() {
    let mut setup = live_script();
    let line = {
        let mut value = task();
        value.chip.title = DocumentText::complete(text(b"whole reply"));
        value.label = 1;
        value.flags = TaskFlags {
            ended_by: EndedBy::Result,
            conversation: true,
            listed: false,
            retention: Retention::ForgottenOnceRead,
        };
        crate::queries::line(
            &value,
            core::iter::empty(),
            Some(ConversationRead { seen: 0, unread: Count { value: 1, limited: false } }),
        )
    };
    let entry = InboxEntry {
        reply: Some(Box::new(line)),
        lineage: list(vec![]),
        goal: None,
        recipient: None,
        stall: None,
        object: ObjectKey::Task(9),
        project: 1,
        group: InboxGroup::Replies,
        position: 1,
        at: Wall::EPOCH,
    };
    setup.truth.sources.push(Source {
        party: 1,
        read: Read::Inbox {
            group: jig_client_domain::contract::InboxReadGroup::Updates,
            project: None,
            kind: None,
            goal: None,
            after: None,
        },
        rows: Rows::Inbox(vec![entry]),
    });
    assert_eq!(Engine::new(setup, 1).err(), Some(Invalid::Identity), "reply task identity is whole and authoritative");
}
#[test]
fn reads_watches_and_run_scenarios() {
    scenario_byte_bound_pages_and_watch_windows();
    scenario_all_read_shapes_and_reverse_activity();
    scenario_pages_and_read_barriers();
    scenario_watch_subjects_coalescing_and_lifecycle();
    scenario_run_offsets_cut_names_and_activity();
}

fn nested_escalation_payload_bounds(escalation: &Escalation) {
    for text_or_capacity in [true, false] {
        let mut invalid_setup = script();
        let mut row = escalation.clone();
        if text_or_capacity {
            row.steering.spec = specification(Text::new(vec![0; 4097].into_boxed_slice(), 5000).unwrap());
        } else {
            row.hold.spend = List::with_capacity(33);
        }
        invalid_setup.truth.object_projects.insert(ObjectKey::Escalation(8), 1);
        invalid_setup.truth.objects.insert(ObjectKey::Escalation(8), ReadValue::Escalation(Box::new(row)));
        assert_eq!(
            Engine::new(invalid_setup, 1).err(),
            Some(Invalid::Payload),
            "whole nested steering and hold spend are admitted"
        );
    }
}

mod application;
mod faults;
#[test]
fn restart_faults_and_application_hooks_keep_one_durable_truth() {
    faults::run();
    application::run();
}

fn overlong_resource_grant() -> OwnerGrant {
    OwnerGrant {
        connector: 1,
        kind: 1,
        pattern: OwnerPattern {
            segments: List::with_capacity(0),
            last: OwnerPatternEnd::Exact(vec![0; 4097].into_boxed_slice()),
        },
    }
}

fn bounded_offer_payloads_and_predictions() {
    use jig_client_domain::contract::Bounds as DraftBounds;
    closing_connector_payload_bounds();
    let mut direct = owner_authority();
    direct.authority.spend = 110;
    let mut ceiling = direct.clone();
    ceiling.authority.spend = 200;
    let within = DraftBounds::Amend {
        budget_left: 10,
        implications: List::with_capacity(0),
        grants: list(vec![]),
        authority: Box::new(direct),
        ceiling: Box::new(ceiling),
    };
    let offer = Admission::Bounded { within, beyond: Beyond::Proposed { to: Recipient::Party(2) } };
    for (budget, answer, valid) in [
        (110, Answer::Done(Outcome::Changed), true),
        (
            111,
            Answer::Done(Outcome::Proposed { proposal: ProposalKey { proposer: Proposer::Task(8), number: 1 } }),
            true,
        ),
        (
            110,
            Answer::Done(Outcome::Proposed { proposal: ProposalKey { proposer: Proposer::Task(8), number: 1 } }),
            false,
        ),
        (111, Answer::Done(Outcome::Changed), false),
    ] {
        let mut setup = live_script();
        let mut row = task();
        row.acts = list(vec![Offered { effect: None, act: ActKind::Amend, admission: offer.clone() }]);
        setup.truth.objects.insert(ObjectKey::Task(8), ReadValue::Task(Box::new(row)));
        let mut after = owner_authority().whole().unwrap();
        after.spend = budget;
        let ask = Ask::Amend {
            reason: text(b""),
            project: 1,
            task: 8,
            revision: Some(Revision { value: 1 }),
            change: Box::new(Amendment {
                llm: None,
                addressed: None,
                spec: None,
                wake: None,
                dependencies: None,
                authority: Some(after),
                instructions: None,
                tracked: None,
            }),
        };
        setup.rules.push(Rule { party: Some(1), ask: ask.clone(), answer: answer.clone(), changes: vec![] });
        let mut engine = Engine::new(setup, 1).unwrap();
        let result = engine.send(Time::ZERO, Wall::EPOCH, 11, [1; 16], Token::new(1), ask);
        if valid {
            result.unwrap();
            assert_eq!(final_answer(&engine.poll(at(10_000_000))[0]), &answer);
        } else {
            assert_eq!(result, Err(Invalid::Script));
            assert!(engine.commits().is_empty());
        }
    }
    oversized_bounded_offer_payloads();
}

fn oversized_bounded_offer_payloads() {
    use jig_client_domain::contract::Bounds as DraftBounds;
    for variant in 0..6 {
        let mut setup = live_script();
        let mut row = task();
        let within = if variant == 0 {
            DraftBounds::Create(jig_client_domain::contract::CreateBounds {
                authority: Box::new(owner_authority()),
                pool_left: 1,
                procedures: List::with_capacity(0),
                charters: List::with_capacity(33),
                latest: None,
                budget: 1,
                maximum_budget: 2,
                maximum_deadline: None,
            })
        } else {
            let mut direct = owner_authority();
            let mut ceiling = owner_authority();
            let mut grants = list(vec![]);
            if variant == 1 {
                grants = List::with_capacity(33);
            }
            if variant == 2 {
                direct.authority.delegation.kinds = List::with_capacity(33);
            }
            if variant == 3 {
                ceiling.authority.grants = list(vec![overlong_resource_grant()]);
            }
            if variant == 4 {
                ceiling.authority.note_resources = List::with_capacity(33);
            }
            DraftBounds::Amend {
                budget_left: 1,
                implications: List::with_capacity(if variant == 5 { 33 } else { 0 }),
                grants,
                authority: Box::new(direct),
                ceiling: Box::new(ceiling),
            }
        };
        row.acts = list(vec![Offered {
            effect: None,
            act: ActKind::Amend,
            admission: Admission::Bounded { within, beyond: Beyond::Proposed { to: Recipient::Party(2) } },
        }]);
        setup.truth.objects.insert(ObjectKey::Task(8), ReadValue::Task(Box::new(row)));
        assert_eq!(Engine::new(setup, 1).err(), Some(Invalid::Payload), "all nested bounded facts are priced");
    }
}

fn closing_connector_payload_bounds() {
    let mut setup = live_script();
    let mut row = task();
    row.chip.phase = Phase::Closing;
    row.chip.details = Box::new(ChipPhase::Closing(Closing {
        delegates: 1,
        connectors: Some(List::with_capacity(33)),
        stage: None,
        ending: None,
    }));
    setup.truth.objects.insert(ObjectKey::Task(8), ReadValue::Task(Box::new(row)));
    assert_eq!(Engine::new(setup, 1).err(), Some(Invalid::Payload));
}

#[test]
fn recurring_optional_boundaries_keep_whole_value_and_allocated_list_limits() {
    let mut bounds = Bounds { rows: 2, ..Bounds::default() };
    for next in [None, Some(Wall::from_nanos(u64::MAX))] {
        let mut chip = task().chip;
        chip.title = DocumentText::complete(text(b"job"));
        chip.details = Box::new(ChipPhase::Starting);
        let mut row = Recurring {
            chip,
            key: u32::MAX,
            batch: list(vec![]),
            budget: 20,
            overlap: RecurringOverlap::Wait,
            last_period: 8,
            pending_period: Some(9),
            last_batch: list(vec![7, 8]),
            cut: true,
            current_period: 9,
            began: Some(Wall::from_nanos(1)),
            next,
        };
        let bytes = u64::try_from(core::mem::size_of::<Recurring>()).unwrap()
            + u64::try_from(core::mem::size_of::<ChipPhase>()).unwrap()
            + List::<u64>::worst_case(2).unwrap()
            + 3;
        bounds.document_bytes = bytes;
        assert!(crate::payload::fits(&row, &bounds));
        bounds.document_bytes = bytes.checked_sub(1).unwrap();
        assert!(!crate::payload::fits(&row, &bounds));
        bounds.document_bytes = u64::MAX;
        row.last_batch = List::with_capacity(3);
        assert!(!crate::payload::fits(&row, &bounds));
    }
}

#[test]
fn collective_policy_recipients_accept_any_listed_role_and_price_the_whole_list() {
    let recipient = Recipient::Roles { project: 1, roles: list(vec![7, 1]) };
    let mut setup = script();
    setup.truth.frames.get_mut(&2).unwrap().projects.get_mut(0).unwrap().role = HeldRole::Policy { role: 99 };
    assert!(crate::requests::recipient(&setup.truth, 1, &recipient));
    assert!(!crate::requests::recipient(&setup.truth, 2, &recipient));
    let mut row = proposal();
    row.waiting = recipient.clone();
    setup.truth.objects.insert(ObjectKey::Proposal(row.key), ReadValue::Proposal(Box::new(row.clone())));
    let mut engine = Engine::new(setup, 1).unwrap();
    send(
        &mut engine,
        0,
        11,
        1,
        1,
        Ask::Decide {
            project: 1,
            on: DecideOn::Proposal { proposal: row.key, decision: Decision::Accepted, running: None },
        },
    );
    let up = engine.poll(Time::from_nanos(10_000_000));
    assert_eq!(
        final_answer(&up[0]),
        &Answer::Done(Outcome::Decided { by: Actor::Party(1), how: Decision::Accepted, at: Wall::EPOCH })
    );
    let mut bounds = Bounds { rows: 2, ..Bounds::default() };
    bounds.document_bytes =
        u64::try_from(core::mem::size_of::<Recipient>()).unwrap() + List::<u32>::worst_case(2).unwrap();
    assert!(crate::payload::fits(&recipient, &bounds));
    bounds.document_bytes = bounds.document_bytes.checked_sub(1).unwrap();
    assert!(!crate::payload::fits(&recipient, &bounds));
    for bounded in [false, true] {
        let mut setup = script();
        let mut row = row.clone();
        let recipient = Recipient::Roles { project: 1, roles: List::with_capacity(33) };
        row.acts.get_mut(0).unwrap().admission = if bounded {
            Admission::Bounded {
                within: jig_client_domain::contract::Bounds::Create(jig_client_domain::contract::CreateBounds {
                    authority: Box::new(owner_authority()),
                    pool_left: 10,
                    procedures: List::with_capacity(0),
                    charters: list(vec![1]),
                    latest: None,
                    budget: 0,
                    maximum_budget: 100,
                    maximum_deadline: None,
                }),
                beyond: Beyond::Proposed { to: recipient },
            }
        } else {
            Admission::Proposed { to: recipient }
        };
        setup.truth.objects.insert(ObjectKey::Proposal(row.key), ReadValue::Proposal(Box::new(row)));
        assert_eq!(Engine::new(setup, 1).err(), Some(Invalid::Payload));
    }
}

#[test]
fn amendment_release_checks_the_named_hold_separately_from_the_spec() {
    let change = Change::Request(Box::new(Ask::Seen { through: SeenThrough::Results { through: 1 } }));
    let mut exact = Bounds {
        document_bytes: u64::try_from(core::mem::size_of::<Box<Ask>>())
            .unwrap()
            .checked_add(u64::try_from(core::mem::size_of::<Ask>()).unwrap())
            .unwrap(),
        ..Bounds::default()
    };
    assert_eq!(change.validate(&exact), Ok(()));
    exact.document_bytes = exact.document_bytes.checked_sub(1).unwrap();
    assert_eq!(change.validate(&exact), Err(Invalid::Payload));
    for stale_hold in [false, true] {
        let mut setup = script();
        let mut row = task();
        row.hold = Some(TaskHold::Document(Box::new(Hold {
            placed: None,
            revision: Revision { value: 7 },
            reason: HoldReason::Stopped { by: party(1), at: Wall::EPOCH },
            before: Phase::Running,
            since: Wall::EPOCH,
            result: None,
            spend: list(vec![]),
        })));
        row.acts = list(vec![Offered { effect: None, act: ActKind::Amend, admission: Admission::Allowed }]);
        setup.truth.objects.insert(ObjectKey::Task(8), ReadValue::Task(Box::new(row)));
        let mut engine = Engine::new(setup, 1).unwrap();
        send(
            &mut engine,
            0,
            11,
            1,
            1,
            crate::tests::releasing(
                Ask::Amend {
                    reason: text(b""),
                    project: 1,
                    task: 8,
                    revision: Some(Revision { value: if stale_hold { 1 } else { 2 } }),

                    change: Box::new(Amendment {
                        llm: None,
                        addressed: None,
                        spec: Some(specification(text(b"new spec")).whole().unwrap()),
                        wake: None,
                        dependencies: None,
                        authority: None,
                        instructions: None,
                        tracked: None,
                    }),
                },
                Revision { value: if stale_hold { 6 } else { 7 } },
            ),
        );
        assert_eq!(
            final_answer(&engine.poll(Time::from_nanos(10_000_000))[0]),
            &Answer::Refused {
                refusal: Refusal::Moved {
                    now: Revision { value: if stale_hold { 7 } else { 1 } },
                    what: if stale_hold {
                        Moved::Hold {
                            reason: Some(CurrentHold::Document(Box::new(HoldReason::Stopped {
                                by: party(1),
                                at: Wall::EPOCH,
                            }))),
                        }
                    } else {
                        Moved::Spec
                    },
                },
                at: None
            }
        );
    }
}

#[test]
fn moved_hold_refusal_prices_the_complete_reason_allocation_and_each_nested_failure() {
    let failures = list(vec![
        AttemptFailure { attempt: 1, class: FailureClass::HostLost, reason: text(b"first"), at: Wall::EPOCH },
        AttemptFailure { attempt: 2, class: FailureClass::Temporary, reason: text(b"second"), at: Wall::EPOCH },
    ]);
    let answer = Answer::Refused {
        refusal: Refusal::Moved {
            now: Revision { value: 8 },
            what: Moved::Hold { reason: Some(CurrentHold::Document(Box::new(HoldReason::Tries { failures }))) },
        },
        at: None,
    };
    let mut bounds = Bounds {
        document_bytes: u64::try_from(core::mem::size_of::<Answer>())
            .unwrap()
            .checked_add(u64::try_from(core::mem::size_of::<HoldReason>()).unwrap())
            .unwrap()
            .checked_add(List::<AttemptFailure>::worst_case(2).unwrap())
            .unwrap()
            .checked_add(11)
            .unwrap(),
        ..Bounds::default()
    };
    assert!(crate::payload::fits(&answer, &bounds));
    bounds.document_bytes = bounds.document_bytes.checked_sub(1).unwrap();
    assert!(!crate::payload::fits(&answer, &bounds));
    bounds.document_bytes = u64::MAX;
    bounds.rows = 1;
    assert!(!crate::payload::fits(&answer, &bounds));
    bounds.rows = 2;
    bounds.text = 5;
    assert!(!crate::payload::fits(&answer, &bounds));
}

#[test]
fn verdict_documents_price_whole_machine_field_arrays_and_bytes() {
    let fields = list(vec![ResultField { name: b"state".as_slice().into(), value: Box::new([0, 255]) }]);
    let value = ResultValue::Verdict {
        verdict: u32::MAX,
        words: DocumentText::complete(text(b"why")),
        fields,
        tasks: list(vec![]),
        proposal: None,
    };
    let mut bounds = Bounds { text: 8, rows: 1, ..Bounds::default() };
    bounds.document_bytes = u64::try_from(core::mem::size_of::<ResultValue>()).unwrap()
        + List::<ResultField>::worst_case(1).unwrap()
        + List::<Chip>::worst_case(0).unwrap()
        + 10;
    assert!(crate::payload::fits(&value, &bounds));
    bounds.document_bytes -= 1;
    assert!(!crate::payload::fits(&value, &bounds));
    bounds.document_bytes += 1;
    bounds.text = 4;
    assert!(!crate::payload::fits(&value, &bounds), "field name is whole or refused");
    let mut names = List::with_capacity(2);
    names.push(FieldRule { name: b"state".as_slice().into(), max: u32::MAX }).unwrap();
    let option = OptionLine {
        number: u32::MAX,
        words: text(b"choice"),
        words_max: u32::MAX,
        followups: u32::MAX,
        fields: names,
    };
    bounds.text = 8;
    bounds.document_bytes = u64::MAX;
    assert!(!crate::payload::fits(&option, &bounds), "allocated rule capacity also consumes rows");
    bounds.rows = 2;
    assert!(crate::payload::fits(&option, &bounds));
}

#[test]
fn owner_verdict_payload_prices_reserved_field_rows_and_only_supplied_payload() {
    let mut fields = List::with_capacity(2);
    fields.push(ResultField { name: b"state".as_slice().into(), value: Box::new([0, 255]) }).unwrap();
    let value =
        ResultValue::OwnerVerdict { verdict: u32::MAX, words: DocumentText { bytes: text(b"why"), cut: true }, fields };
    let before = value.clone();
    let mut bounds = Bounds {
        text: 5,
        rows: 2,
        document_bytes: u64::try_from(core::mem::size_of::<ResultValue>()).unwrap()
            + List::<ResultField>::worst_case(2).unwrap()
            + 10,
        ..Bounds::default()
    };
    assert!(crate::payload::fits(&value, &bounds));
    bounds.document_bytes -= 1;
    assert!(!crate::payload::fits(&value, &bounds));
    bounds.document_bytes += 2;
    assert!(crate::payload::fits(&value, &bounds));
    bounds.rows = 1;
    assert!(!crate::payload::fits(&value, &bounds), "reserved empty field capacity stays priced");
    bounds.rows = 2;
    bounds.text = 4;
    assert!(!crate::payload::fits(&value, &bounds), "opaque name is whole or refused");
    assert_eq!(value, before);
}

#[test]
fn approved_documents_preserve_opaque_state_optional_words_and_exact_byte_bounds() {
    let mut bounds = Bounds { text: 3, document_bytes: u64::MAX, ..Bounds::default() };
    let absent = ResultValue::Approved { state: Box::new([0, 255, 1]), words: None };
    let empty = ResultValue::Approved { state: Box::new([0, 255, 1]), words: Some(DocumentText::complete(text(b""))) };
    assert_ne!(absent, empty);
    for value in [
        absent,
        empty,
        ResultValue::Approved { state: Box::new([0, 255, 1]), words: Some(DocumentText::complete(text(b"yes"))) },
    ] {
        let ResultValue::Approved { words, .. } = &value else {
            unreachable!("approval fixture");
        };
        let explanation = words.as_ref().map_or(0, |words| words.bytes().len());
        bounds.document_bytes = u64::try_from(core::mem::size_of::<ResultValue>() + 3 + explanation).unwrap();
        assert!(crate::payload::fits(&value, &bounds));
        bounds.document_bytes -= 1;
        assert!(!crate::payload::fits(&value, &bounds));
        bounds.document_bytes += 1;
        bounds.text = 2;
        assert!(!crate::payload::fits(&value, &bounds), "whole opaque state is refused rather than cut");
        bounds.text = 3;
    }
    let failure = ResultValue::Failure(DocumentText::complete(text(b"bad")));
    bounds.document_bytes = u64::try_from(core::mem::size_of::<ResultValue>() + 3).unwrap();
    assert!(crate::payload::fits(&failure, &bounds));
    bounds.document_bytes -= 1;
    assert!(!crate::payload::fits(&failure, &bounds));
}

fn specification(words: jig_client_domain::contract::Text) -> jig_client_domain::contract::SpecificationDocument {
    jig_client_domain::contract::SpecificationDocument::complete(jig_client_domain::contract::Specification {
        words,
        parameters: List::with_capacity(0),
        inputs: List::with_capacity(0),
    })
}

#[test]
fn whole_specification_prices_parameter_and_input_capacity_and_refuses_partial_values() {
    let mut parameters = List::with_capacity(4);
    parameters.push(Parameter::Number { name: u32::MAX, value: u64::MAX }).unwrap();
    parameters.push(Parameter::Bytes { name: 7, value: text(UTF8_TEXTS[0]) }).unwrap();
    parameters.push(Parameter::Resource { name: 8, connector: u16::MAX, resource: u64::MAX }).unwrap();
    let mut inputs = List::with_capacity(3);
    inputs.push(u64::MAX).unwrap();
    let value = {
        let mut document = SpecificationDocument::complete(Specification { words: text(b"whole"), parameters, inputs });
        document.cut = true;
        document
    };
    let allocated = u64::try_from(core::mem::size_of::<SpecificationDocument>() + 5 + 3).unwrap()
        + List::<ParameterDocument>::worst_case(4).unwrap()
        + List::<u64>::worst_case(3).unwrap();
    let mut bounds = Bounds { text: 5, rows: 4, document_bytes: allocated, ..Bounds::default() };
    assert!(crate::payload::fits(&value, &bounds));
    assert!(value.cut, "actual owner marker remains a fact");
    bounds.document_bytes -= 1;
    assert!(!crate::payload::fits(&value, &bounds));
    bounds.document_bytes = allocated;
    bounds.rows = 3;
    assert!(!crate::payload::fits(&value, &bounds), "allocated parameter slots are bounded");
    bounds.rows = 4;
    bounds.text = 4;
    assert!(!crate::payload::fits(&value, &bounds), "whole words are never clipped");
    bounds.text = 5;
    let mut too_long = value.clone();
    too_long
        .specification
        .parameters
        .get_mut(1)
        .unwrap()
        .clone_from(&ParameterDocument::Bytes { name: 7, value: DocumentText::complete(text(b"longer")) });
    bounds.document_bytes = u64::MAX;
    assert!(!crate::payload::fits(&too_long, &bounds), "whole opaque bytes parameter is never clipped");
}

fn cursor_inbox_entry(number: u64, kind: InboxGroup, at: u64, goal: Option<u64>) -> InboxEntry {
    InboxEntry {
        object: match kind {
            InboxGroup::Questions => ObjectKey::Question { task: number, message: 1 },
            InboxGroup::Results => ObjectKey::Result(number),
            InboxGroup::PersonTasks => ObjectKey::PersonTask(number),
            InboxGroup::Proposals | InboxGroup::Escalations | InboxGroup::Replies => unreachable!("fixture kinds"),
        },
        group: kind,
        project: 1,
        lineage: list(vec![]),
        goal,
        recipient: None,
        stall: None,
        reply: None,
        position: number,
        at: Wall::from_nanos(at),
    }
}

#[test]
fn inbox_group_cursors_keep_exact_filters_and_whole_query_order() {
    let mut setup = live_script();
    setup.bounds.rows = 1;
    for (group, rows) in [
        (
            InboxReadGroup::Needs,
            vec![
                cursor_inbox_entry(6, InboxGroup::PersonTasks, 1, Some(9)),
                cursor_inbox_entry(8, InboxGroup::Questions, 3, Some(9)),
                cursor_inbox_entry(7, InboxGroup::Questions, 2, Some(9)),
                cursor_inbox_entry(5, InboxGroup::Questions, 0, Some(8)),
            ],
        ),
        (
            InboxReadGroup::Updates,
            vec![
                cursor_inbox_entry(10, InboxGroup::Results, 4, Some(9)),
                cursor_inbox_entry(11, InboxGroup::Results, 5, Some(9)),
            ],
        ),
    ] {
        setup.truth.sources.push(Source {
            party: 1,
            read: Read::Inbox { group, project: None, kind: None, goal: None, after: None },
            rows: Rows::Inbox(rows),
        });
    }
    let mut engine = Engine::new(setup, 92).expect("whole owner collections fit");
    let needs = Read::Inbox {
        group: InboxReadGroup::Needs,
        project: Some(1),
        kind: Some(InboxGroup::Questions),
        goal: Some(9),
        after: None,
    };
    let updates = Read::Inbox {
        group: InboxReadGroup::Updates,
        project: Some(1),
        kind: Some(InboxGroup::Results),
        goal: Some(9),
        after: None,
    };
    engine.read(Time::ZERO, 11, Token::new(1), needs.clone()).unwrap();
    let up = engine.poll(Time::ZERO);
    let Up::Read { terminal: ReadResult::Value(ReadValue::Inbox(page)), .. } = &up[0] else {
        panic!("needs page");
    };
    assert_eq!(
        page.rows.get(0).unwrap().object,
        ObjectKey::Question { task: 7, message: 1 },
        "filter before bounded page slicing"
    );
    let needs_cursor = page.after.clone().unwrap();
    engine.read(Time::ZERO, 11, Token::new(2), updates.clone()).unwrap();
    let up = engine.poll(Time::ZERO);
    let Up::Read { terminal: ReadResult::Value(ReadValue::Inbox(page)), .. } = &up[0] else {
        panic!("updates page");
    };
    assert_eq!(page.rows.get(0).unwrap().object, ObjectKey::Result(11));
    let updates_cursor = page.after.clone().unwrap();
    for (token, mut query, cursor, wanted) in [
        (3, needs.clone(), needs_cursor.clone(), ObjectKey::Question { task: 8, message: 1 }),
        (4, updates.clone(), updates_cursor, ObjectKey::Result(10)),
    ] {
        if let Read::Inbox { after, .. } = &mut query {
            *after = Some(cursor);
        }
        engine.read(Time::ZERO, 11, Token::new(token), query).unwrap();
        let up = engine.poll(Time::ZERO);
        let Up::Read { terminal: ReadResult::Value(ReadValue::Inbox(page)), .. } = &up[0] else {
            panic!("independent continuation");
        };
        assert_eq!(page.rows.get(0).unwrap().object, wanted);
        assert!(page.after.is_none());
    }
    let mut wrong = updates;
    if let Read::Inbox { after, .. } = &mut wrong {
        *after = Some(needs_cursor);
    }
    engine.read(Time::ZERO, 11, Token::new(5), wrong).unwrap();
    assert!(matches!(engine.poll(Time::ZERO)[0], Up::Read { terminal: ReadResult::None(ReadNone::Gone), .. }));
    engine.open(Time::ZERO, 11, Token::new(6), Subject::Inbox { window: 1 }).unwrap();
    let up = engine.poll(Time::ZERO);
    let Up::Watch { event: WatchEvent::Delivered(Delivery::Inbox(value)), .. } = &up[1] else {
        panic!("shared live watch");
    };
    assert_eq!(
        value.entries.get(0).unwrap().object,
        ObjectKey::Question { task: 5, message: 1 },
        "live watch remains unfiltered"
    );
    assert!(value.more);
}

#[test]
fn procedure_goal_bounds_price_allocated_pairs_and_judge_the_exact_executor() {
    let exact = OwnerExecutor::Procedure { connector: u16::MAX, code: u32::MAX };
    let offer = Admission::Bounded {
        within: jig_client_domain::contract::Bounds::Create(jig_client_domain::contract::CreateBounds {
            authority: Box::new(owner_authority()),
            pool_left: 100,
            charters: list(vec![]),
            procedures: list(vec![
                CreateProcedure { connector: u16::MAX, code: u32::MAX },
                CreateProcedure { connector: 7, code: u32::MAX },
            ]),
            latest: None,
            budget: 3,
            maximum_budget: 10,
            maximum_deadline: None,
        }),
        beyond: Beyond::Proposed { to: Recipient::Party(2) },
    };
    let bytes = u64::try_from(core::mem::size_of::<Admission>()).unwrap()
        + u64::try_from(core::mem::size_of::<OwnerAuthorityDocument>()).unwrap()
        + List::<u32>::worst_case(0).unwrap()
        + List::<CreateProcedure>::worst_case(2).unwrap();
    let mut bounds = Bounds { rows: 2, document_bytes: bytes, ..Bounds::default() };
    assert!(crate::payload::fits(&offer, &bounds));
    bounds.document_bytes = bytes.checked_sub(1).unwrap();
    assert!(!crate::payload::fits(&offer, &bounds));
    bounds.document_bytes = u64::MAX;
    bounds.rows = 1;
    assert!(!crate::payload::fits(&offer, &bounds));
    let mut room = offer.clone();
    let Admission::Bounded {
        within:
            jig_client_domain::contract::Bounds::Create(jig_client_domain::contract::CreateBounds { procedures, .. }),
        ..
    } = &mut room
    else {
        panic!("goal bounds");
    };
    *procedures = List::with_capacity(3);
    bounds.rows = 2;
    assert!(!crate::payload::fits(&room, &bounds), "unused allocated room is bounded too");
    for (executor, budget, answer) in [
        (exact, 3, Answer::Done(Outcome::Changed)),
        (
            exact,
            4,
            Answer::Done(Outcome::Proposed { proposal: ProposalKey { proposer: Proposer::Task(1), number: 2 } }),
        ),
        (
            OwnerExecutor::Procedure { connector: 6, code: u32::MAX },
            3,
            Answer::Refused { refusal: Refusal::NotOffered, at: None },
        ),
        (
            OwnerExecutor::Procedure { connector: u16::MAX, code: 7 },
            3,
            Answer::Refused { refusal: Refusal::NotOffered, at: None },
        ),
        (OwnerExecutor::Agent { charter: 1 }, 3, Answer::Refused { refusal: Refusal::NotOffered, at: None }),
        (exact, 11, Answer::Refused { refusal: Refusal::NotOffered, at: None }),
    ] {
        let mut setup = live_script();
        let Admission::Bounded { within: jig_client_domain::contract::Bounds::Create(within), beyond } = &offer else {
            panic!("creating offer")
        };
        setup.truth.frames.get_mut(&1).unwrap().projects.get_mut(0).unwrap().creating =
            Some(Creating { within: within.clone(), beyond: beyond.clone() });
        let mut member = fixtures::new_task(text(b"exact procedure"));
        member.executor = executor;
        member.authority.spend = budget;
        member.tracked = Some(8);
        member.flags.conversation = false;
        let ask = Ask::Create { project: 1, tasks: list(vec![member]) };
        setup.rules.push(Rule { party: Some(1), ask: ask.clone(), answer: answer.clone(), changes: vec![] });
        let mut engine = Engine::new(setup, 1).unwrap();
        send(&mut engine, 0, 11, 1, 1, ask.clone());
        assert_eq!(final_answer(&engine.poll(at(10_000_000))[0]), &answer);
        assert_eq!(engine.commits()[0].ask, Some(ask));
    }
}

#[test]
fn document_text_loss_is_preserved_and_priced_with_whole_document_parameter_capacity() {
    for cut in [false, true] {
        let value = DocumentText { bytes: text(UTF8_TEXTS[0]), cut };
        let mut bounds = Bounds {
            text: 3,
            document_bytes: u64::try_from(core::mem::size_of::<DocumentText>() + 3).unwrap(),
            ..Bounds::default()
        };
        assert!(crate::payload::fits(&value, &bounds));
        assert_eq!(value.whole().is_some(), !cut);
        assert_eq!(value.bytes(), UTF8_TEXTS[0]);
        bounds.document_bytes -= 1;
        assert!(!crate::payload::fits(&value, &bounds));
        bounds.document_bytes = u64::MAX;
        bounds.text = 2;
        assert!(!crate::payload::fits(&value, &bounds));
    }
    let mut parameters = List::with_capacity(4);
    parameters.push(Parameter::Bytes { name: u32::MAX, value: text(UTF8_TEXTS[0]) }).unwrap();
    let value = SpecificationDocument::complete(Specification {
        words: text(b"w"),
        parameters,
        inputs: List::with_capacity(2),
    });
    let whole = value.whole().unwrap();
    assert_eq!(whole.parameters.capacity(), 4);
    assert_eq!(whole.inputs.capacity(), 2);
    let size = u64::try_from(core::mem::size_of::<SpecificationDocument>() + 1 + 3).unwrap()
        + List::<ParameterDocument>::worst_case(4).unwrap()
        + List::<u64>::worst_case(2).unwrap();
    let mut bounds = Bounds { text: 3, rows: 4, document_bytes: size, ..Bounds::default() };
    assert!(crate::payload::fits(&value, &bounds));
    bounds.document_bytes -= 1;
    assert!(!crate::payload::fits(&value, &bounds));
    bounds.document_bytes = size;
    bounds.rows = 3;
    assert!(!crate::payload::fits(&value, &bounds));
}

#[test]
fn activation_observations_and_prior_closing_are_admitted_whole_with_their_allocated_bounds() {
    let mut setup = script();
    let mut row = task();
    let mut active = ActiveRun {
        stage: RunStage::Running { attempt: u64::MAX },
        phase_since: Wall::EPOCH,
        attempt: Some(u64::MAX),
        placement: Some(Wall::EPOCH),
        retry: Some(RetryNotice::Refused { count: u32::MAX }),
        observation: Some(RunObservation {
            channel: Token::new(u64::MAX),
            work: RunWork::Tool { call: current_identity(b"call"), tool: text(b"tool") },
            since: Wall::EPOCH,
            reported_at: Wall::EPOCH,
            started: None,
        }),
    };
    *row.chip.details = ChipPhase::Active(active.clone());
    row.context.as_mut().expect("supplied task context").runtime = Runtime::Active(active.clone());
    setup.truth.objects.insert(ObjectKey::Task(8), ReadValue::Task(Box::new(row.clone())));
    assert!(Engine::new(setup.clone(), 1).is_ok());
    active.observation.as_mut().unwrap().work = RunWork::Tool {
        call: CallIdentity {
            id: Text::new(vec![b'x'; setup.bounds.text as usize + 1].into_boxed_slice(), setup.bounds.text + 1)
                .unwrap(),
            ..current_identity(b"call")
        },
        tool: text(b"tool"),
    };
    row.context.as_mut().expect("supplied task context").runtime = Runtime::Active(active);
    setup.truth.objects.insert(ObjectKey::Task(8), ReadValue::Task(Box::new(row.clone())));
    assert_eq!(Engine::new(setup.clone(), 1).err(), Some(Invalid::Payload));
    row.context.as_mut().expect("supplied task context").runtime = Runtime::Held {
        was: Some(HeldRuntime::Closing(Closing {
            delegates: 0,
            connectors: Some(List::with_capacity(setup.bounds.rows + 1)),
            stage: Some(ClosingStage::Run { attempt: u64::MAX }),
            ending: None,
        })),
    };
    setup.truth.objects.insert(ObjectKey::Task(8), ReadValue::Task(Box::new(row)));
    assert_eq!(Engine::new(setup, 1).err(), Some(Invalid::Payload));
}

#[test]
fn full_width_task_and_party_makers_remain_distinct_in_scripted_reads_decisions_and_keyed_replays() {
    let mut setup = script();
    let keys = [
        ProposalKey { proposer: Proposer::Task(u64::MAX), number: u64::MAX },
        ProposalKey { proposer: Proposer::Party(u64::MAX), number: u64::MAX },
    ];
    for (index, key) in keys.iter().enumerate() {
        let mut row = proposal();
        row.key = *key;
        row.proposer = key.proposer;
        row.why = text(if index == 0 { b"task maker" } else { b"party maker" });
        setup.truth.objects.insert(ObjectKey::Proposal(*key), ReadValue::Proposal(Box::new(row)));
    }
    let mut engine = Engine::new(setup, 197).unwrap();
    for (index, key) in keys.iter().enumerate() {
        engine.read(Time::ZERO, 11, Token::new(index as u64 + 1), Read::Proposal { proposal: *key }).unwrap();
        let rows = engine.poll(Time::ZERO);
        let Up::Read { terminal: ReadResult::Value(ReadValue::Proposal(row)), .. } = &rows[0] else {
            panic!("exact typed read")
        };
        assert_eq!(row.key, *key);
        assert_eq!(row.proposer, key.proposer);
        assert_eq!(row.why.bytes(), if index == 0 { b"task maker".as_slice() } else { b"party maker".as_slice() });
    }
    let asks = keys.map(|proposal| Ask::Decide {
        project: 1,
        on: DecideOn::Proposal { proposal, decision: Decision::Accepted, running: None },
    });
    send(&mut engine, 0, 11, 1, 10, asks[0].clone());
    send(&mut engine, 1, 11, 2, 11, asks[1].clone());
    let answers = engine.poll(Time::from_nanos(10_000_001));
    assert_eq!(answers.len(), 2);
    assert_eq!(engine.commits().len(), 2);
    assert_eq!(engine.commits()[0].ask, Some(asks[0].clone()));
    assert_eq!(engine.commits()[1].ask, Some(asks[1].clone()));
    send(&mut engine, 20_000_000, 11, 1, 12, asks[1].clone());
    assert_eq!(
        final_answer(&engine.poll(Time::from_nanos(20_000_000))[0]),
        &Answer::Refused { refusal: Refusal::KeyConflict, at: None }
    );
    send(&mut engine, 20_000_000, 11, 1, 13, asks[0].clone());
    assert_eq!(final_answer(&engine.poll(Time::from_nanos(20_000_000))[0]), final_answer(&answers[0]));
    send(&mut engine, 20_000_000, 11, 2, 14, asks[1].clone());
    assert_eq!(final_answer(&engine.poll(Time::from_nanos(20_000_000))[0]), final_answer(&answers[1]));
    assert_eq!(engine.commits().len(), 2);
}

#[test]
fn note_corrections_price_the_whole_scope_and_allocated_reference_capacity() {
    let mut references = List::with_capacity(4);
    references.push(u64::MAX).unwrap();
    let ask = Ask::EditNote {
        project: 1,
        note: 77,
        scope: NoteScope::Resource {
            project: 1,
            connector: u16::MAX,
            pattern: OwnerPattern {
                segments: List::with_capacity(0),
                last: OwnerPatternEnd::Exact(Box::from(*b"a/b")),
            },
        },
        revision: Revision { value: u64::MAX },
        description: text(b"new"),
        body: text(b"body"),
        references,
    };
    let mut bounds = Bounds { text: 4, rows: 4, document_bytes: u64::MAX, ..Bounds::default() };
    bounds.document_bytes =
        u64::try_from(core::mem::size_of::<Ask>() + 3 + 3 + 4).unwrap() + List::<u64>::worst_case(4).unwrap();
    assert!(crate::payload::fits(&ask, &bounds));
    bounds.document_bytes -= 1;
    assert!(!crate::payload::fits(&ask, &bounds));
    bounds.document_bytes += 1;
    bounds.rows = 3;
    assert!(!crate::payload::fits(&ask, &bounds), "allocated reference capacity is bounded as a whole");
    bounds.rows = 4;
    bounds.text = 2;
    assert!(!crate::payload::fits(&ask, &bounds), "the supplied scope and body are refused rather than cut");
}

#[test]
fn optional_instruction_sources_price_whole_task_and_steering_at_byte_and_text_boundaries() {
    use crate::payload::Payload;
    let mut bounds = Bounds { text: 33, document_bytes: u64::MAX, ..Bounds::default() };
    for source in [
        None,
        Some(DocumentText::complete(text(b""))),
        Some(DocumentText { bytes: text(b""), cut: true }),
        Some(DocumentText::complete(text(&[b'x'; 33]))),
        Some(DocumentText { bytes: text(&[b'x'; 33]), cut: true }),
    ] {
        let mut row = task();
        row.instructions = source.clone();
        let mut steering = Steering {
            person_revision: Some(Revision { value: 12 }),
            taken_by: None,
            addressed: None,
            revision: Revision { value: 6 },
            spec: row.spec.clone(),
            instructions: None,
            wake: row.wake,
            dependencies: List::with_capacity(0),
            authority: row.authority.clone(),
            budget: budget(),
        };
        let mut absent = row.clone();
        absent.instructions = None;
        let bytes = source.as_ref().map_or(0, |value| value.bytes().len()) as u64;
        assert_eq!(row.heap(&bounds).unwrap(), absent.heap(&bounds).unwrap() + bytes);
        let steering_base = steering.heap(&bounds).unwrap();
        steering.instructions = source.clone();
        assert_eq!(steering.heap(&bounds).unwrap(), steering_base + bytes);
        bounds.document_bytes = row.heap(&bounds).unwrap() + u64::try_from(core::mem::size_of::<Task>()).unwrap();
        assert!(crate::payload::fits(&row, &bounds));
        bounds.document_bytes -= 1;
        assert!(!crate::payload::fits(&row, &bounds));
        bounds.document_bytes += 2;
        assert!(crate::payload::fits(&row, &bounds));
        bounds.document_bytes =
            steering.heap(&bounds).unwrap() + u64::try_from(core::mem::size_of::<Steering>()).unwrap();
        assert!(crate::payload::fits(&steering, &bounds));
        bounds.document_bytes -= 1;
        assert!(!crate::payload::fits(&steering, &bounds));
        bounds.document_bytes += 2;
        assert!(crate::payload::fits(&steering, &bounds));
        bounds.document_bytes = u64::MAX;
        bounds.text = 32;
        assert_eq!(crate::payload::fits(&row, &bounds), bytes <= 32);
        assert_eq!(crate::payload::fits(&steering, &bounds), bytes <= 32);
        bounds.text = 34;
        assert!(crate::payload::fits(&row, &bounds));
        assert!(crate::payload::fits(&steering, &bounds));
        bounds.text = 33;
        assert_eq!(row.instructions, source);
        assert_eq!(steering.instructions, source);
    }
}

#[test]
fn addressed_changes_keep_named_person_revisions_and_whole_fixed_width_destinations_at_exact_bounds() {
    for address in [PersonAddress::Party(u64::MAX), PersonAddress::Role(u32::MAX)] {
        let change = AddressChange { revision: Revision { value: u64::MAX }, address };
        let exact = u64::try_from(core::mem::size_of::<AddressChange>()).unwrap();
        let mut bounds = Bounds { text: 0, rows: 0, document_bytes: exact, ..Bounds::default() };
        assert!(crate::payload::fits(&change, &bounds));
        bounds.document_bytes -= 1;
        assert!(!crate::payload::fits(&change, &bounds), "the complete named revision and target are charged together");
    }
}

#[test]
fn result_document_loss_and_raw_change_paths_are_admitted_whole_at_exact_allocated_bounds() {
    let mut path: List<Box<[u8]>> = List::with_capacity(4);
    path.push(b"a/b".as_slice().into()).unwrap();
    path.push(Box::new([255, 0])).unwrap();
    let value = ResultValue::Change {
        kind: u16::MAX,
        part: OwnerPart { connector: u16::MAX, kind: OwnerPartKind::Resource, key: OwnerPartKey::Path(path) },
        words: DocumentText { bytes: text(b"why"), cut: true },
    };
    let exact = u64::try_from(core::mem::size_of::<ResultValue>()).unwrap()
        + List::<Box<[u8]>>::worst_case(4).unwrap()
        + 3
        + 2
        + 3;
    let mut bounds = Bounds { rows: 2, segments: 4, text: 3, document_bytes: exact, ..Bounds::default() };
    assert!(crate::payload::fits(&value, &bounds));
    bounds.document_bytes -= 1;
    assert!(!crate::payload::fits(&value, &bounds));
    bounds.document_bytes = exact;
    bounds.segments = 3;
    assert!(!crate::payload::fits(&value, &bounds), "allocated path segments are charged even when unused");
    bounds.segments = 4;
    bounds.text = 2;
    assert!(
        !crate::payload::fits(&value, &bounds),
        "an incomplete source text does not permit further byte truncation"
    );
}

#[test]
fn result_metadata_boxes_price_unfilled_delegate_and_followup_capacity_without_inventing_a_wall() {
    use crate::payload::Payload;
    let mut bounds = Bounds { rows: 4, document_bytes: u64::MAX, ..Bounds::default() };
    let mut result = jig_client_domain::contract::Result {
        task: task().chip,
        ending: Ending::Done,
        value: None,
        at: None,
        details: None,
        acts: list(vec![]),
    };
    let base = u64::try_from(core::mem::size_of::<jig_client_domain::contract::Result>()).unwrap()
        + result.heap(&bounds).unwrap();
    result.details = Some(Box::new(ResultDetails {
        contract: TaskContractDocument::Report { words: u32::MAX },
        requester: OwnerRequester::Deployment { project: u32::MAX },
        executor: OwnerExecutor::Procedure { connector: u16::MAX, code: u32::MAX },
        chosen_by: None,
        delegates: List::with_capacity(3),
        followups: Some(List::with_capacity(4)),
        proposal: None,
        cut: true,
    }));
    let exact = base
        + u64::try_from(core::mem::size_of::<ResultDetails>()).unwrap()
        + List::<OwnerTaskLink>::worst_case(3).unwrap()
        + List::<OwnerTaskLink>::worst_case(4).unwrap();
    bounds.document_bytes = exact;
    assert!(crate::payload::fits(&result, &bounds));
    assert_eq!(result.at, None);
    bounds.document_bytes -= 1;
    assert!(!crate::payload::fits(&result, &bounds));
    bounds.document_bytes = exact;
    bounds.rows = 2;
    assert!(!crate::payload::fits(&result, &bounds), "whole supplied capacity is enforced for empty retained lists");
}

mod owner_chip;

#[test]
#[expect(clippy::too_many_lines, reason = "whole retained-run admission story includes every mandatory writing field")]
fn live_raw_calls_price_both_texts_whole_and_refuse_without_replacing_retained_run() {
    use crate::payload::Payload;
    let name = TurnName { attempt: u64::MAX, number: u32::MAX };
    let raw = CurrentCall {
        identity: current_identity(UTF8_TEXTS[1]),
        began: None,
        tool: text(UTF8_TEXTS[2]),
        input: text(UTF8_TEXTS[3]),
        cut: true,
    };
    let run = RunText {
        writing: None,
        attempt: name.attempt,
        began: None,
        turn: provisional(name),
        from: 0,
        through: 4,
        text: text(b"live"),
        cut: false,
        call: Some(LiveCall::Raw(raw.clone())),
    };
    let setup = live_script();
    let original_bounds = setup.bounds;
    let heap = run.heap(&original_bounds).unwrap();
    assert_eq!(heap, 22, "four streamed bytes plus four identity, nine tool and five input bytes");
    let exact = u64::try_from(core::mem::size_of::<RunText>()).unwrap() + heap;
    let mut bounds = original_bounds;
    bounds.document_bytes = exact;
    assert!(crate::payload::fits(&run, &bounds));
    bounds.document_bytes -= 1;
    assert!(!crate::payload::fits(&run, &bounds));
    bounds.document_bytes = exact;
    bounds.text = 8;
    assert!(!crate::payload::fits(&run, &bounds), "the whole raw tool must fit its own bound");
    let whole_identity = LiveCall::Raw(CurrentCall {
        identity: CallIdentity { id: text(UTF8_TEXTS[4]), ..raw.identity.clone() },
        tool: text(b"t"),
        input: text(b"i"),
        ..raw.clone()
    });
    bounds = original_bounds;
    let identity_heap = whole_identity.heap(&bounds).unwrap();
    assert_eq!(identity_heap, 21);
    bounds.document_bytes = u64::try_from(core::mem::size_of::<LiveCall>()).unwrap() + identity_heap;
    assert!(crate::payload::fits(&whole_identity, &bounds));
    bounds.document_bytes -= 1;
    assert!(!crate::payload::fits(&whole_identity, &bounds), "whole opaque id is priced with tool/input");
    bounds.document_bytes += 1;
    bounds.text = 18;
    assert!(!crate::payload::fits(&whole_identity, &bounds), "id admission never cuts or parses provider bytes");
    let large = CurrentCall {
        identity: current_identity(UTF8_TEXTS[1]),
        began: None,
        tool: text(UTF8_TEXTS[5]),
        input: text(UTF8_TEXTS[6]),
        cut: true,
    };
    let held = RunText { call: Some(LiveCall::Raw(large)), ..run.clone() };
    let delivery = Delivery::Run(Box::new(held.clone()));
    let whole = u64::try_from(core::mem::size_of::<Delivery>()).unwrap() + delivery.heap(&original_bounds).unwrap();
    let mut setup = setup;
    setup.bounds.document_bytes = whole;
    let mut engine = Engine::new(setup, 26).unwrap();
    let token = Token::new(1);
    engine.open(Time::ZERO, 11, token, Subject::Run { task: 8, turn: None, offset: 0 }).unwrap();
    drop(engine.poll(Time::ZERO));
    engine.write_live(Time::ZERO, 8, name, None, &held.text, held.call.clone()).unwrap();
    assert!(engine.poll(Time::ZERO).contains(&Up::Watch { token, event: WatchEvent::Delivered(delivery.clone()) }));
    let excess = core::mem::size_of::<Delivery>() + 1;
    assert_eq!(
        engine.write_live(Time::ZERO, 8, name, Some(Wall::EPOCH), &text(&vec![b'!'; excess]), held.call.clone()),
        Err(Invalid::Payload)
    );
    engine.continue_run(token, Some(provisional(name)), 0).unwrap();
    assert!(
        engine.poll(Time::ZERO).contains(&Up::Watch { token, event: WatchEvent::Delivered(delivery) }),
        "refusal preserved the whole previously delivered value"
    );
    engine.write_live(Time::ZERO, 8, name, Some(Wall::EPOCH), &text(b""), held.call.clone()).unwrap();
    engine.continue_run(token, Some(provisional(name)), 0).unwrap();
    let supplied = RunText { began: Some(Wall::EPOCH), ..held };
    assert!(
        engine
            .poll(Time::ZERO)
            .contains(&Up::Watch { token, event: WatchEvent::Delivered(Delivery::Run(Box::new(supplied))) })
    );
    let rich = LiveCall::Decoded(Call {
        number: u64::MAX,
        tool: text(b"rich"),
        input: text(b"input"),
        outcome: Some(text(b"result")),
        failed: true,
        objects: list(vec![ObjectKey::Task(8)]),
        parts: list(vec![]),
    });
    let heap = rich.heap(&original_bounds).unwrap();
    assert!(heap > 15, "rich allocated object-list capacity remains priced");
    bounds = original_bounds;
    bounds.document_bytes = u64::try_from(core::mem::size_of::<LiveCall>()).unwrap() + heap;
    assert!(crate::payload::fits(&rich, &bounds));
    bounds.document_bytes -= 1;
    assert!(!crate::payload::fits(&rich, &bounds));
}

fn current_identity(id: &[u8]) -> jig_client_domain::contract::CallIdentity {
    jig_client_domain::contract::CallIdentity {
        owner: skein_lib::Token::new(u64::MAX),
        sequence: u32::MAX,
        retry: u32::MAX,
        id: text(id),
    }
}

#[test]
fn whole_run_state_prices_independent_call_and_writing_and_preserves_refused_snapshot() {
    use crate::payload::Payload;
    let setup = live_script();
    let original = setup.bounds;
    let call = CurrentCall {
        identity: current_identity(UTF8_TEXTS[1]),
        tool: text(UTF8_TEXTS[7]),
        input: text(b"input/\x00"),
        cut: true,
        began: Some(Wall::from_nanos(73)),
    };
    let state = RunState {
        task: 8,
        attempt: Some(u64::MAX),
        committed_turn: u32::MAX,
        writing: None,
        call: Some(call.clone()),
    };
    let heap = state.heap(&original).unwrap();
    assert_eq!(heap, 17, "four identity, six tool and seven input bytes are priced whole");
    let mut bounds = original;
    bounds.document_bytes = u64::try_from(core::mem::size_of::<RunState>()).unwrap() + heap;
    assert!(crate::payload::fits(&state, &bounds));
    bounds.document_bytes -= 1;
    assert!(!crate::payload::fits(&state, &bounds));
    bounds = original;
    bounds.text = 3;
    assert!(!crate::payload::fits(&state, &bounds), "a whole opaque id never gets shortened to admit a document");
    let writing = RunText {
        writing: None,
        attempt: u64::MAX,
        began: None,
        turn: ProvisionalTurnName { retry: 0, attempt: u64::MAX, number: u32::MAX },
        from: 9,
        through: 13,
        text: text(b"tail"),
        cut: true,
        call: Some(LiveCall::Raw(call)),
    };
    let with_writing = RunState { writing: Some(writing.clone()), ..state.clone() };
    assert_eq!(
        with_writing.heap(&original).unwrap(),
        heap + writing.heap(&original).unwrap(),
        "the supplied independent call and writing are separately allocated and priced"
    );
    let large = CurrentCall {
        identity: CallIdentity { id: text(UTF8_TEXTS[8]), ..state.call.as_ref().unwrap().identity.clone() },
        tool: text(UTF8_TEXTS[5]),
        input: text(UTF8_TEXTS[6]),
        ..state.call.clone().unwrap()
    };
    let held = RunState { call: Some(large), ..state.clone() };
    let delivery = Delivery::RunState(Box::new(held.clone()));
    let exact = u64::try_from(core::mem::size_of::<Delivery>()).unwrap() + delivery.heap(&original).unwrap();
    let mut setup = setup;
    setup.bounds.document_bytes = exact;
    let mut engine = Engine::new(setup, 27).unwrap();
    let token = Token::new(1);
    engine.open(Time::ZERO, 11, token, Subject::Run { task: 8, turn: None, offset: 0 }).unwrap();
    drop(engine.poll(Time::ZERO));
    engine.write_run_state(Time::ZERO, held.clone()).unwrap();
    assert!(engine.poll(Time::ZERO).contains(&Up::Watch { token, event: WatchEvent::Delivered(delivery.clone()) }));
    engine.taken(token);
    assert!(engine.poll(Time::ZERO).is_empty(), "taking a normal document cannot create a stream delivery loop");
    let overflow = RunState { writing: Some(writing), ..held.clone() };
    assert_eq!(engine.write_run_state(Time::ZERO, overflow), Err(Invalid::Payload));
    engine.continue_run(token, None, 0).unwrap();
    assert!(
        engine.poll(Time::ZERO).contains(&Up::Watch { token, event: WatchEvent::Delivered(delivery) }),
        "refusal retained every independent current-call byte and the supplied clock"
    );
    engine.taken(token);
    let postcommit = RunState { committed_turn: u32::MAX, writing: None, ..state };
    engine.write_run_state(Time::ZERO, postcommit.clone()).unwrap();
    assert!(
        engine
            .poll(Time::ZERO)
            .contains(&Up::Watch { token, event: WatchEvent::Delivered(Delivery::RunState(Box::new(postcommit))) }),
        "a postcommit call is deliverable without a text turn or continuation cursor"
    );
}

mod chip_facts;

fn provisional(name: TurnName) -> ProvisionalTurnName {
    ProvisionalTurnName { attempt: name.attempt, number: name.number, retry: 0 }
}

#[test]
fn scripted_provisional_retry_reopens_exact_positions_and_replaces_on_empty_start() {
    let mut setup = live_script();
    setup.bounds.text = 32;
    let mut engine = Engine::new(setup, 28).unwrap();
    let old = ProvisionalTurnName { attempt: u64::MAX, number: u32::MAX, retry: u32::MAX - 1 };
    engine.write_provisional(Time::ZERO, 8, old, None, &text(b"abcdefghijklmnopqrst"), None).unwrap();
    engine.write_provisional(Time::ZERO, 8, old, None, &text(b"uvwxyz0123456789ABCD"), None).unwrap();
    let token = Token::new(1);
    engine.open(Time::ZERO, 11, token, Subject::Run { task: 8, turn: Some(old), offset: 20 }).unwrap();
    let up = engine.poll(Time::ZERO);
    let suffix = RunText {
        writing: None,
        attempt: old.attempt,
        began: None,
        turn: old,
        from: 20,
        through: 40,
        text: text(b"uvwxyz0123456789ABCD"),
        cut: false,
        call: None,
    };
    assert!(up.contains(&Up::Watch { token, event: WatchEvent::Delivered(Delivery::Run(Box::new(suffix))) }));
    engine.continue_run(token, Some(old), 99).unwrap();
    let up = engine.poll(Time::ZERO);
    let Up::Watch { event: WatchEvent::Delivered(Delivery::Run(value)), .. } = &up[0] else {
        panic!("bounded retained restart");
    };
    assert_eq!((value.turn, value.from, value.through, value.cut), (old, 8, 40, true));
    assert_eq!(value.text.bytes(), b"ijklmnopqrstuvwxyz0123456789ABCD");
    let captured = value.clone();
    let current = ProvisionalTurnName { retry: u32::MAX, ..old };
    engine.write_provisional(Time::ZERO, 8, current, None, &text(b""), None).unwrap();
    engine.continue_run(token, Some(old), 40).unwrap();
    let up = engine.poll(Time::ZERO);
    let empty = RunText {
        writing: None,
        attempt: current.attempt,
        began: None,
        turn: current,
        from: 0,
        through: 0,
        text: text(b""),
        cut: false,
        call: None,
    };
    assert!(up.contains(&Up::Watch { token, event: WatchEvent::Delivered(Delivery::Run(Box::new(empty))) }));
    assert_eq!(captured.turn, old);
    assert_eq!(
        captured.text.bytes(),
        b"ijklmnopqrstuvwxyz0123456789ABCD",
        "captured delivery is immutable across replacement"
    );
    engine.write_provisional(Time::ZERO, 8, old, Some(Wall::EPOCH), &text(b"stale"), None).unwrap();
    engine.continue_run(token, Some(current), 0).unwrap();
    let up = engine.poll(Time::ZERO);
    let Up::Watch { event: WatchEvent::Delivered(Delivery::Run(value)), .. } = &up[0] else {
        panic!("current retry");
    };
    assert_eq!((value.turn, value.from, value.through, value.cut), (current, 0, 0, false));
    assert_eq!(value.began, None);
    assert_eq!(value.text.bytes(), b"");
    engine.write_provisional(Time::ZERO, 8, current, Some(Wall::EPOCH), &text(b"retry"), None).unwrap();
    engine.close(Time::ZERO, token);
    drop(engine.poll(Time::ZERO));
    let reopened = Token::new(2);
    engine.open(Time::ZERO, 11, reopened, Subject::Run { task: 8, turn: Some(old), offset: 99 }).unwrap();
    let up = engine.poll(Time::ZERO);
    let Up::Watch { event: WatchEvent::Delivered(Delivery::Run(value)), .. } = &up[1] else {
        panic!("reopened current retry");
    };
    assert_eq!((value.turn, value.from, value.through, value.cut), (current, 0, 5, false));
    assert_eq!(value.text.bytes(), b"retry");
    assert_eq!(value.began, Some(Wall::EPOCH));
}

mod resource_facts;

mod note_facts;

mod task_links;

#[test]
fn task_subscription_capacity_is_priced_separately_from_whole_activation_replacements() {
    use crate::payload::Payload;
    let mut row = task();
    let original = row.clone();
    let bounds = Bounds::default();
    let baseline = row.heap(&bounds).unwrap();
    row.subscriptions.entries = List::with_capacity(4);
    for kind in [
        SubscriptionKind::Task { target: u64::MAX, held: true, result: false },
        SubscriptionKind::Topic { connector: u16::MAX, topic: u64::MAX },
        SubscriptionKind::Timer { at: Wall::EPOCH, period: None },
        SubscriptionKind::Timer {
            at: Wall::from_nanos(u64::MAX),
            period: Some(skein_lib::Duration::from_nanos(u64::MAX)),
        },
    ] {
        row.subscriptions.entries.push(Subscription { number: u64::MAX, kind }).unwrap();
    }
    assert_eq!(row.heap(&bounds).unwrap(), baseline + List::<Subscription>::worst_case(4).unwrap());
    let exact = u64::try_from(core::mem::size_of::<Task>()).unwrap() + row.heap(&bounds).unwrap();
    let mut limited = Bounds { document_bytes: exact, ..bounds };
    assert!(crate::payload::fits(&row, &limited));
    limited.document_bytes -= 1;
    assert!(!crate::payload::fits(&row, &limited));
    limited.document_bytes = u64::MAX;
    limited.rows = 3;
    assert!(!crate::payload::fits(&row, &limited));
    let mut reserved = original;
    reserved.subscriptions.entries = List::with_capacity(4);
    assert_eq!(reserved.heap(&bounds).unwrap(), row.heap(&bounds).unwrap());
    for results in [ResultsWake::Never, ResultsWake::Each, ResultsWake::LastOrFailure] {
        let mut change = Amendment {
            llm: None,
            addressed: None,
            spec: None,
            wake: Some(OwnerWakePolicy {
                words: WakeRule::Batch { count: u32::MAX, age: skein_lib::Duration::from_nanos(u64::MAX) },
                notices: WakeRule::Immediate,
                news: WakeRule::Never,
                results,
                questions: true,
                answers: false,
                timers: true,
            }),
            dependencies: None,
            authority: None,
            instructions: None,
            tracked: None,
        };
        let heap = change.heap(&bounds).unwrap();
        assert_eq!(heap, 0);
        let size = u64::try_from(core::mem::size_of::<Amendment>()).unwrap();
        assert!(crate::payload::fits(&change, &Bounds { document_bytes: size, ..bounds }));
        assert!(!crate::payload::fits(&change, &Bounds { document_bytes: size - 1, ..bounds }));
        change.wake = None;
        assert_eq!(change.heap(&bounds).unwrap(), heap);
        assert_eq!(row.subscriptions.entries.capacity(), 4);
    }
}

mod task_contract;

mod page_loss;
mod task_authority;

mod task_budget;

mod history_pages;

mod amendment_reason;
mod question_sender;
mod task_ending;

mod task_context;
mod task_hold;
mod task_resources;

mod offered;
mod owner_escalation;
mod owner_person_task;

mod historical_objects;
mod owner_deployment;

const UTF8_TEXTS: [&[u8]; 9] = [
    b"\0~\x01",
    b"id/\x7e",
    b"raw/tool\x7e",
    b"a/b\x00\x21",
    b"opaque/provider/id\x7e",
    &[b'~'; 4096],
    &[b'!'; 4096],
    b"tool/\x21",
    &[b'?'; 4096],
];

use jig_client_domain::contract::DecideOn;

mod compound;
use compound::releasing;

mod fixtures;
mod propose;

#[test]
fn span_acceptance_uses_the_supplied_decision_wall() {
    for (seconds, expected) in [
        (15, Answer::Done(Outcome::Changed)),
        (
            16,
            Answer::Refused {
                refusal: Refusal::Authority {
                    lacking: Lacking::Deadline {
                        asked: Some(Wall::from_nanos(21_000_000_000)),
                        latest: Some(Wall::from_nanos(20_000_000_000)),
                    },
                    proposable: Some(false),
                },
                at: None,
            },
        ),
    ] {
        let (setup, ask) = fixtures::span_proposal();
        let mut engine = Engine::new(setup, 1).expect("span fixture");
        send(&mut engine, seconds * 1_000_000_000, 11, 1, 1, ask);
        let answers = engine.poll(at(seconds * 1_000_000_000 + 10_000_000));
        assert_eq!(final_answer(&answers[0]), &expected, "{seconds}");
    }
}

#[test]
fn named_authority_models_price_their_full_allocated_list_and_bytes() {
    let mut value = owner_authority();
    let bounds = Bounds { document_bytes: u64::MAX, ..Bounds::default() };
    let before = crate::owner_authority::document_bytes(&value, &bounds).unwrap();
    let mut names = List::with_capacity(64);
    names.push(Box::from([b'x'; 4096].as_slice())).unwrap();
    let allocated = List::<Box<[u8]>>::worst_case(64).unwrap() + 4096;
    value.authority.models = Some(AuthorityModels::Named(names));
    assert_eq!(crate::owner_authority::document_bytes(&value, &bounds), Some(before + allocated));
    assert!(crate::owner_authority::worst_case(&bounds).unwrap() >= before + allocated);
    let mut over = List::with_capacity(65);
    over.push(Box::from(b"model".as_slice())).unwrap();
    value.authority.models = Some(AuthorityModels::Named(over));
    assert_eq!(crate::owner_authority::document_bytes(&value, &bounds), None);
}
mod projects;
