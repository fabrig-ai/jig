//! Single-read positions remain bounded through lifecycle and restart.
use super::*;

#[test]
fn a_held_task_names_the_milestone_of_its_hold() {
    let mut h = Harness::new();
    h.make(vec![task(1, &[])]);
    assert_eq!(live(&h, 1).hold, 0);
    h.send(Event::Hold { task: 1, why: Hold::Stopped });
    assert_eq!(live(&h, 1).hold, 3);
    h.route(1, EscalationHolder::Person(9), 1);
    h.restart();
    assert_eq!(live(&h, 1).hold, 3);
    h.send(Event::Control {
        reply_to: to(),
        by: Party::Person(9),
        task: 1,
        control: Control::Release { again: None, message: None },
    });
    assert_eq!(live(&h, 1).hold, 3);
    h.send(Event::Hold { task: 1, why: Hold::Budget });
    assert_eq!(live(&h, 1).hold, 5);
    let Stored::Milestone(row) = &h.stored[&Key::Milestone { task: 1, position: 5 }] else { panic!() };
    let crate::Phase::Held { why, .. } = &row.phase else { panic!("held milestone") };
    assert_eq!(*why, Hold::Budget);
}

#[test]
fn unanswered_questions_survive_reads_and_refuse_past_the_position_bound() {
    let mut h = question_cohort(5);
    h.env.limits.inbox_messages = 2;
    h.env.limits.run_inbox = 2;
    h.send(Event::Prepare { reply_to: to(), task: 2 });
    h.send(Event::Claim {
        account: None,
        reply_to: to(),
        task: 2,
        attempt: 1,
        resumed: false,
        budget: 10,
        writes: Box::new([]),
    });
    h.send(Event::Started { task: 2, attempt: 1 });
    question_cycles(&mut h);
    assert_eq!(
        live(&h, 2).question_lines.as_ref(),
        [
            crate::QuestionLine { message: 3, line: 5, answer: None },
            crate::QuestionLine { message: 1, line: 3, answer: None },
        ]
    );
    h.env.limits = LIMITS;
    h.restart();
    assert_eq!(live(&h, 2).question_lines.len(), 2);
}

fn question_cycles(h: &mut Harness) {
    for (turn, source) in (3..=5).enumerate() {
        let message = u64::try_from(turn).unwrap().checked_mul(2).unwrap().checked_add(1).unwrap();
        let out = word(h, 2, Party::Task(source), message, MessageKind::Question);
        if source == 5 {
            assert!(
                out.iter().any(|request| match request {
                    Request::Refused { problem, .. } => problem.why == crate::Refusal::Busy,
                    _ => false,
                }),
                "{out:?}"
            );
            assert!(live(h, 5).questions.is_empty(), "refusal reserves no sender credit");
            continue;
        }
        assert!(
            out.iter().any(|request| match request {
                Request::Sent { .. } => true,
                _ => false,
            }),
            "{out:?}"
        );
        h.send(Event::Turn {
            refused: Box::new([]),
            reply_to: to(),
            task: 2,
            attempt: 1,
            turn: u32::try_from(turn).unwrap().checked_add(1).unwrap(),
            read: Some(message),
            offered: Some(message),
            outstanding: Box::new([]),
            cumulative: 0,
        });
    }
}

fn question_cohort(last: u64) -> Harness {
    let mut h = Harness::new();
    let mut parent = task(1, &[]);
    parent.numbers.budget = 1000;
    parent.authority.budget.spend = 1000;
    h.make(vec![parent]);
    let children = (2..=last)
        .map(|number| {
            let mut child = task(number, &[]);
            child.funder = Funder::Task(1);
            child
        })
        .collect::<Vec<_>>();
    let out = h.send(Event::Make { reply_to: to(), creator: Party::Task(1), batch: children.into_boxed_slice() });
    assert!(
        out.iter().any(|request| match request {
            Request::Made { .. } => true,
            _ => false,
        }),
        "{out:?}"
    );
    for right in 3..=last {
        let out = h.send(Event::Introduce { reply_to: to(), by: 1, left: 2, right });
        assert!(
            out.iter().any(|request| match request {
                Request::Done { .. } => true,
                _ => false,
            }),
            "{out:?}"
        );
    }
    h
}

fn live(h: &Harness, number: u64) -> &crate::TaskRecord {
    h.domain.task(number).expect("live task")
}

fn word(h: &mut Harness, task: u64, from: Party, number: u64, kind: MessageKind) -> Vec<Request> {
    h.send(Event::Message {
        reply_to: to(),
        project: 1,
        task,
        word: crate::Word { number, from, kind, words: Box::new([1]), at: Wall::EPOCH, hits: 1, eligible: true },
    })
}

#[test]
fn a_tasks_answer_names_the_line_in_the_delegates_activity() {
    let mut h = question_cohort(3);
    start(&mut h, 2);
    let _question = word(&mut h, 2, Party::Task(3), 1, MessageKind::Question);
    h.send(Event::Turn {
        refused: Box::new([]),
        reply_to: to(),
        task: 2,
        attempt: 1,
        turn: 1,
        read: Some(1),
        offered: Some(1),
        outstanding: Box::new([]),
        cumulative: 0,
    });
    let activity = live(&h, 2).activity;
    assert!(live(&h, 2).inbox.is_empty());
    let answer_position = live(&h, 3).activity + 1;
    let _answer = word(&mut h, 3, Party::Task(2), 2, MessageKind::Answer { question: 1 });
    let expected = Some(crate::AnswerLine { task: 3, position: answer_position });
    assert_eq!(live(&h, 2).question_lines[0].answer, expected);
    assert_eq!(live(&h, 2).activity, activity, "the answering task gains no line");
    let Stored::Live(saved) = &h.stored[&Key::Live(2)] else { panic!("answer pointer saved") };
    assert_eq!(saved.question_lines[0].answer, expected);
    let Stored::Activity(line) = &h.stored[&Key::Activity { task: 3, position: answer_position }] else {
        panic!("answer line")
    };
    let crate::Line::Words { kind, .. } = &line.line else { panic!("answer words") };
    assert_eq!(*kind, MessageKind::Answer { question: 1 });
    let original = live(&h, 2).clone();
    let _again = word(&mut h, 3, Party::Task(2), 3, MessageKind::Answer { question: 1 });
    assert_eq!(live(&h, 2), &original, "the first answer stays");
    h.restart();
    assert_eq!(live(&h, 2).question_lines[0].answer, expected);
}

#[test]
fn answered_positions_are_evicted_before_unanswered_ones() {
    let mut h = question_cohort(5);
    h.env.limits.inbox_messages = 2;
    h.env.limits.run_inbox = 2;
    start(&mut h, 2);
    question_cycles(&mut h);
    let out = word(&mut h, 3, Party::Task(2), 6, MessageKind::Answer { question: 1 });
    assert!(
        out.iter().any(|request| match request {
            Request::Sent { .. } => true,
            _ => false,
        }),
        "{out:?}"
    );
    assert!(live(&h, 2).question_lines[0].answer.is_none());
    assert!(live(&h, 2).question_lines[1].answer.is_some());
    let out = word(&mut h, 2, Party::Task(5), 7, MessageKind::Question);
    assert!(
        out.iter().any(|request| match request {
            Request::Sent { .. } => true,
            _ => false,
        }),
        "{out:?}"
    );
    assert_eq!(live(&h, 2).question_lines.iter().map(|line| line.message).collect::<Vec<_>>(), [7, 3]);
    h.env.limits = LIMITS;
    h.restart();
    assert_eq!(live(&h, 2).question_lines.len(), 2);
}

fn start(h: &mut Harness, task: u64) {
    h.send(Event::Prepare { reply_to: to(), task });
    h.send(Event::Claim {
        account: None,
        reply_to: to(),
        task,
        attempt: 1,
        resumed: false,
        budget: 10,
        writes: Box::new([]),
    });
    h.send(Event::Started { task, attempt: 1 });
}

#[test]
fn a_person_can_answer_after_the_run_reads_and_the_first_answer_closes_the_line() {
    let mut h = question_cohort(2);
    start(&mut h, 1);
    let out = word(&mut h, 1, Party::Task(2), 1, MessageKind::Question);
    assert!(
        out.iter().any(|request| match request {
            Request::Sent { .. } => true,
            _ => false,
        }),
        "{out:?}"
    );
    h.send(Event::Turn {
        refused: Box::new([]),
        reply_to: to(),
        task: 1,
        attempt: 1,
        turn: 1,
        read: Some(1),
        offered: Some(1),
        outstanding: Box::new([]),
        cumulative: 0,
    });
    assert!(live(&h, 1).inbox.is_empty());
    assert_eq!(live(&h, 1).question_lines[0].answer, None);
    let answer = crate::Word {
        number: 2,
        from: Party::Person(9),
        kind: MessageKind::Answer { question: 1 },
        words: b"yes".as_slice().into(),
        at: h.env.wall,
        hits: 1,
        eligible: false,
    };
    let out = h.send(Event::PersonQuestionAnswer {
        reply_to: to(),
        project: 1,
        asked: 1,
        person: 9,
        question: 1,
        word: answer,
        notice: 3,
    });
    assert!(
        out.iter().any(|request| match request {
            Request::Sent { .. } => true,
            _ => false,
        }),
        "{out:?}"
    );
    let first = live(&h, 1).question_lines[0].answer;
    assert!(first.is_some());
    assert!(live(&h, 2).questions.is_empty(), "the person's answer consumes the delegate's credit");
    h.send(Event::Turn {
        refused: Box::new([]),
        reply_to: to(),
        task: 1,
        attempt: 1,
        turn: 2,
        read: Some(3),
        offered: Some(3),
        outstanding: Box::new([]),
        cumulative: 0,
    });
    let out = word(&mut h, 1, Party::Person(9), 4, MessageKind::Answer { question: 1 });
    assert!(
        out.iter().any(|request| match request {
            Request::Refused { problem, .. } => problem.why == crate::Refusal::Reference,
            _ => false,
        }),
        "{out:?}"
    );
    h.restart();
    assert_eq!(live(&h, 1).question_lines[0].answer, first);
}

#[test]
fn questions_check_message_and_byte_bounds_at_both_inboxes() {
    for sender in [false, true] {
        for byte_bound in [false, true] {
            let mut h = question_cohort(3);
            h.env.limits.inbox_messages = if byte_bound { 8 } else { 2 };
            h.env.limits.run_inbox = h.env.limits.inbox_messages;
            h.env.limits.message_bytes = 1;
            h.env.limits.inbox_bytes = if byte_bound { 2 } else { 128 };
            let bounded = if sender { 3 } else { 2 };
            let out = word(&mut h, bounded, Party::Task(1), 1, MessageKind::Words);
            assert!(
                out.iter().any(|request| match request {
                    Request::Sent { .. } => true,
                    _ => false,
                }),
                "{out:?}"
            );
            let out = word(&mut h, 2, Party::Task(3), 2, MessageKind::Question);
            assert!(
                out.iter().any(|request| match request {
                    Request::Sent { .. } => true,
                    _ => false,
                }),
                "at the bound: {out:?}"
            );
            let before = live(&h, 2).clone();
            let source = live(&h, 3).clone();
            let out = word(&mut h, 2, Party::Task(3), 3, MessageKind::Question);
            assert!(
                out.iter().any(|request| match request {
                    Request::Refused { problem, .. } =>
                        problem.why == crate::Refusal::Busy && problem.task == Some(bounded),
                    _ => false,
                }),
                "past the bound: sender={sender} bytes={byte_bound}: {out:?}"
            );
            assert_eq!(live(&h, 2), &before);
            assert_eq!(live(&h, 3), &source);
        }
    }
}

#[test]
fn person_answers_to_the_received_task_are_refused_whole() {
    let mut h = question_cohort(2);
    start(&mut h, 1);
    let _question = word(&mut h, 1, Party::Task(2), 1, MessageKind::Question);
    let before = h.stored.clone();
    let operations = (2..=3)
        .map(|number| crate::Operation::Message {
            task: 1,
            word: crate::Word {
                number,
                from: Party::Person(9),
                kind: MessageKind::Answer { question: 1 },
                words: Box::new([1]),
                at: Wall::EPOCH,
                hits: 1,
                eligible: true,
            },
        })
        .collect::<Vec<_>>();
    let out = compound::compound(&mut h, operations, 1000);
    assert!(
        out.iter().any(|request| match request {
            Request::CompoundRefused { at: 0, problem, .. } => problem.why == crate::Refusal::Reference,
            _ => false,
        }),
        "{out:?}"
    );
    assert_eq!(h.stored, before);
}
