//! Ordinary Rust parent scripts verify the core's immutable activity archives.
#![allow(
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::wildcard_enum_match_arm,
    clippy::match_like_matches_macro,
    reason = "unit-test parent scripts are ordinary Rust under programming-model.md section 10.2"
)]
use super::*;

/// Execute the ordinary root's immediate accepted-turn callback before committing
/// this decision; retain every archived row for identity/order assertions.
fn activity_decision(core: &mut Core, env: &Env<Limits>, event: Event) -> Vec<Record> {
    let turn_input = match &event {
        Event::TurnPayload { read, cumulative, .. } => Some((*read, *cumulative)),
        _ => None,
    };
    let Requests::Out(mut out) = step(core, env, event);
    let mut records = Vec::new();
    while let Some(request) = out.pop() {
        match request {
            Request::Write(Write::Save(record)) => records.push(record),
            Request::Now(now) => {
                if let Now::AcceptedTurn { task, attempt, turn, accepted, .. } = *now {
                    let (read, cumulative) = turn_input.expect("callback belongs to the offered turn");
                    records.extend(activity_decision(
                        core,
                        env,
                        Event::AcceptedTurn {
                            opens_window: false,
                            task,
                            attempt,
                            turn,
                            accepted,
                            cumulative,
                            read,
                            transcript: Box::from(&b"a committed turn"[..]),
                        },
                    ));
                }
            }
            Request::Write(Write::Erase(_) | Write::CopySettingsRevision(_))
            | Request::Ask { .. }
            | Request::Held(_)
            | Request::ProjectMade { .. }
            | Request::Decided => {}
        }
    }
    records
}

#[test]
fn activity_orders_claim_turns_person_words_and_a_delegate_result_with_their_archives() {
    let (mut core, limits) = tree_core();
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let to = || ReplyTo::new(Token::new(1));
    let root = new_row(&core, 1, tasks::Funder::Period { project: 1, period: 1 }, None);
    send_task(
        &mut core,
        &limits,
        tasks::Event::Make { reply_to: to(), creator: tasks::Party::Person(1), batch: Box::new([root]) },
    );
    let child = new_row(&core, 2, tasks::Funder::Task(1), None);
    send_task(
        &mut core,
        &limits,
        tasks::Event::Make { reply_to: to(), creator: tasks::Party::Task(1), batch: Box::new([child]) },
    );
    let first = core.tasks.task(1).unwrap().activity;
    let mut records =
        activity_decision(&mut core, &env, Event::Tasks(tasks::Event::Prepare { reply_to: to(), task: 1 }));
    core.proofs.insert(1, RunProof::claimed(1, 7, 7, None, jig_domain_fleet::HostKind::Worker)).unwrap();
    core.claiming.insert(1, 7).unwrap();
    records.extend(activity_decision(
        &mut core,
        &env,
        Event::ClaimPrepared { task: 1, attempt: 7, budget: 10, writes: Box::new([]) },
    ));
    records.extend(activity_decision(&mut core, &env, Event::Tasks(tasks::Event::Started { task: 1, attempt: 7 })));
    records.extend(activity_decision(
        &mut core,
        &env,
        Event::TurnPayload {
            refused: Box::new([]),
            run: Token::new(1),
            attempt: Token::new(7),
            turn: 1,
            body: Token::new(11),
            read: None,
            cumulative: 2,
        },
    ));
    let word = tasks::Word {
        number: 1,
        from: tasks::Party::Person(1),
        kind: tasks::MessageKind::Words,
        words: Box::from(&b"the person's complete words"[..]),
        at: env.wall,
        hits: 1,
        eligible: true,
    };
    let mut words_out = Queue::with_capacity(settings::task_room(&limits));
    tasks::step(
        &mut core.tasks,
        &Env { now: env.now, wall: env.wall, limits: limits.tasks },
        tasks::Event::Message { reply_to: to(), project: 1, task: 1, word: word.clone() },
        &mut words_out,
    );
    while let Some(request) = words_out.pop() {
        if let tasks::Request::Save { record } = request {
            records.push(Record::Tasks(record));
        }
    }
    core.proofs.get_mut(&1).unwrap().offered = Some(1);
    core.proofs.get_mut(&1).unwrap().outstanding = Box::new([1]);
    records.extend(activity_decision(
        &mut core,
        &env,
        Event::TurnPayload {
            refused: Box::new([]),
            run: Token::new(1),
            attempt: Token::new(7),
            turn: 2,
            body: Token::new(12),
            read: Some(1),
            cumulative: 5,
        },
    ));
    let result = tasks::Word {
        number: 2,
        from: tasks::Party::Task(2),
        kind: tasks::MessageKind::Result(tasks::ResultKind::Report),
        words: Box::from(&b"the delegate's complete result"[..]),
        at: env.wall,
        hits: 1,
        eligible: true,
    };
    records.extend(activity_decision(
        &mut core,
        &env,
        Event::Tasks(tasks::Event::DelegateResult { task: 1, word: result.clone() }),
    ));
    records.extend(activity_decision(
        &mut core,
        &env,
        Event::AnswerPayload {
            grants_refused: false,
            read: None,
            refused: Box::new([]),
            run: Token::new(1),
            attempt: Token::new(7),
            payload: Token::new(13),
            cumulative: 6,
            end: tasks::End::Parked,
            saved: None,
            invalid_saved: false,
        },
    ));
    let lines: Vec<_> = records
        .iter()
        .filter_map(|row| match row {
            Record::Tasks(tasks::Stored::Activity(row)) if row.task == 1 => Some(row),
            _ => None,
        })
        .collect();
    assert_eq!(lines.iter().map(|row| row.position).collect::<Vec<_>>(), (first + 1..=first + 6).collect::<Vec<_>>());
    assert_eq!(lines[0].line, tasks::Line::RunBegan { attempt: 7, why: tasks::Why::Made, resumed: false });
    assert_eq!(lines[1].line, tasks::Line::Turn { reply: 1, attempt: 7, turn: 1, read: None });
    assert_eq!(lines[2].line, tasks::Line::Words { message: 1, by: word.from, kind: word.kind, words: word.words });
    assert_eq!(lines[3].line, tasks::Line::Turn { reply: 2, attempt: 7, turn: 2, read: Some(1) });
    assert_eq!(lines[4].line, tasks::Line::Heard(result));
    assert_eq!(lines[5].line, tasks::Line::RunEnded { attempt: 7, end: tasks::EndKind::Parked, spent: 6 });
    for turn in [1, 2] {
        assert!(records.iter().any(|row| match row {
            Record::Core(CoreRecord::Turn(row)) =>
                row.task == 1 && row.attempt == 7 && row.turn == turn && row.transcript.as_ref() == b"a committed turn",
            _ => false,
        }));
    }
    assert!(records.iter().any(|row| match row {
        Record::Core(CoreRecord::Terminal(row)) =>
            row.task == 1 && row.attempt == 7 && row.cumulative == 6 && row.end == tasks::End::Parked,
        _ => false,
    }));
    assert!(core.tasks.task(1).unwrap().inbox.iter().all(|word| word.number != 1));
    let Document::Task(document) = make(&core, &limits, 1, &Subject::Task { task: 1 }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!()
    };
    assert_eq!(document.activity, first + 6);
    let before = document.activity;
    let repeated = activity_decision(
        &mut core,
        &env,
        Event::AnswerPayload {
            grants_refused: false,
            read: None,
            refused: Box::new([]),
            run: Token::new(1),
            attempt: Token::new(7),
            payload: Token::new(13),
            cumulative: 6,
            end: tasks::End::Parked,
            saved: None,
            invalid_saved: false,
        },
    );
    assert!(repeated.iter().all(|row| match row {
        Record::Tasks(tasks::Stored::Activity(_)) => false,
        _ => true,
    }));
    assert_eq!(core.tasks.task(1).unwrap().activity, before);
    // A subsequent offered transcript is distinguished from the fresh first claim.
    send_task(
        &mut core,
        &limits,
        tasks::Event::Message {
            reply_to: to(),
            project: 1,
            task: 1,
            word: tasks::Word {
                number: 3,
                from: tasks::Party::Person(1),
                kind: tasks::MessageKind::Words,
                words: Box::from(&b"continue"[..]),
                at: env.wall,
                hits: 1,
                eligible: true,
            },
        },
    );
    activity_decision(&mut core, &env, Event::Tasks(tasks::Event::Prepare { reply_to: to(), task: 1 }));
    core.proofs.insert(1, RunProof::claimed(1, 8, 7, Some(3), jig_domain_fleet::HostKind::Worker)).unwrap();
    core.claiming.insert(1, 8).unwrap();
    let resumed = activity_decision(
        &mut core,
        &env,
        Event::ClaimPrepared { task: 1, attempt: 8, budget: 10, writes: Box::new([]) },
    );
    assert!(resumed.iter().any(|row| match row {
        Record::Tasks(tasks::Stored::Activity(row)) =>
            row.line == (tasks::Line::RunBegan { attempt: 8, why: tasks::Why::Message { message: 3 }, resumed: true }),
        _ => false,
    }));
}

#[test]
fn a_proof_whose_window_passes_its_turn_is_refused_at_restore() {
    let (mut core, limits) = core();
    let mut proof = RunProof::claimed(1, 7, 7, None, jig_domain_fleet::HostKind::Worker);
    proof.turn = Some(TurnProof { turn: 1, cumulative: 0, read: None });
    proof.window = Some(CurrentWindow { attempt: 7, turn: 2, closed: false });
    core.restoring_proofs.insert(1, RestoringProof { attempt: 7, turn: 1, run_spent: 0, last_answer: None }).unwrap();
    assert!(!core.restore_proof(proof, limits.tasks.result_bytes));
}

fn cumulative_read_core() -> (Core, Env<Limits>) {
    let (mut core, limits) = tree_core();
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let to = || ReplyTo::new(Token::new(1));
    let row = new_row(&core, 1, tasks::Funder::Period { project: 1, period: 1 }, None);
    send_task(
        &mut core,
        &limits,
        tasks::Event::Make { reply_to: to(), creator: tasks::Party::Person(1), batch: Box::new([row]) },
    );
    activity_decision(&mut core, &env, Event::Tasks(tasks::Event::Prepare { reply_to: to(), task: 1 }));
    core.proofs.insert(1, RunProof::claimed(1, 7, 7, None, jig_domain_fleet::HostKind::Worker)).unwrap();
    core.claiming.insert(1, 7).unwrap();
    activity_decision(&mut core, &env, Event::ClaimPrepared { task: 1, attempt: 7, budget: 10, writes: Box::new([]) });
    activity_decision(&mut core, &env, Event::Tasks(tasks::Event::Started { task: 1, attempt: 7 }));
    for number in [1, 2] {
        send_task(
            &mut core,
            &limits,
            tasks::Event::Message {
                reply_to: to(),
                project: 1,
                task: 1,
                word: tasks::Word {
                    number,
                    from: tasks::Party::Person(1),
                    kind: tasks::MessageKind::Words,
                    words: b"word".as_slice().into(),
                    at: env.wall,
                    hits: 1,
                    eligible: true,
                },
            },
        );
    }
    core.proofs.get_mut(&1).unwrap().offered = Some(2);
    core.proofs.get_mut(&1).unwrap().outstanding = Box::new([1, 2]);
    activity_decision(
        &mut core,
        &env,
        Event::TurnPayload {
            refused: Box::new([1]),
            run: Token::new(1),
            attempt: Token::new(7),
            turn: 1,
            body: Token::new(11),
            read: Some(2),
            cumulative: 1,
        },
    );
    (core, env)
}

#[test]
fn cumulative_turn_reads_admit_the_last_fence_and_refuse_earlier_names() {
    for (label, read, accepted) in [("same committed fence", Some(2), true), ("earlier retained word", Some(1), false)]
    {
        let (mut core, env) = cumulative_read_core();
        let records = activity_decision(
            &mut core,
            &env,
            Event::TurnPayload {
                refused: Box::default(),
                run: Token::new(1),
                attempt: Token::new(7),
                turn: 2,
                body: Token::new(12),
                read,
                cumulative: 2,
            },
        );
        assert_eq!(core.tasks.task(1).unwrap().turn, if accepted { 2 } else { 1 }, "{label}");
        assert_eq!(
            core.tasks.task(1).unwrap().inbox.iter().map(|word| word.number).collect::<Vec<_>>(),
            [1],
            "{label}"
        );
        assert_eq!(core.proofs.get(&1).unwrap().turn.unwrap().read, Some(2), "{label}");
        assert_eq!(
            records.iter().any(|row| match row {
                Record::Core(CoreRecord::Turn(turn)) => turn.turn == 2 && turn.read == Some(2),
                _ => false,
            }),
            accepted,
            "{label}"
        );
    }
}

fn answer_refusal_core() -> (Core, Env<Limits>) {
    let (mut core, env) = cumulative_read_core();
    send_task(
        &mut core,
        &env.limits,
        tasks::Event::Message {
            reply_to: ReplyTo::new(Token::new(1)),
            project: 1,
            task: 1,
            word: tasks::Word {
                number: 3,
                from: tasks::Party::Person(1),
                kind: tasks::MessageKind::Words,
                words: b"new word".as_slice().into(),
                at: env.wall,
                hits: 1,
                eligible: true,
            },
        },
    );
    core.proofs.get_mut(&1).unwrap().offered = Some(3);
    core.proofs.get_mut(&1).unwrap().outstanding = Box::new([3]);
    (core, env)
}

#[test]
fn an_answers_refusals_are_committed_with_its_terminal() {
    let (mut core, env) = answer_refusal_core();
    let records = activity_decision(
        &mut core,
        &env,
        Event::AnswerPayload {
            grants_refused: false,
            read: Some(2),
            refused: Box::new([3]),
            run: Token::new(1),
            attempt: Token::new(7),
            payload: Token::new(13),
            cumulative: 1,
            end: tasks::End::Parked,
            saved: None,
            invalid_saved: false,
        },
    );
    assert!(records.iter().any(|row| match row {
        Record::Tasks(tasks::Stored::Live(task)) =>
            task.refused.as_ref() == [1, 3] && task.inbox.iter().any(|word| word.number == 3),
        _ => false,
    }));
    assert!(records.iter().any(|row| match row {
        Record::Core(CoreRecord::Terminal(answer)) =>
            answer.task == 1 && answer.attempt == 7 && answer.end == tasks::End::Parked,
        _ => false,
    }));
    assert_eq!(core.tasks.task(1).unwrap().turn, 1, "no invented answer turn");
    assert_eq!(core.tasks.task(1).unwrap().run_spent, 1, "no invented answer charge");
}
