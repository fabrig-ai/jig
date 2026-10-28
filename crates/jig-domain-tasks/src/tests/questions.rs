//! Both question rails at the tasks entrance (domain/tasks.md, 5.7; people.md, 6).
#![allow(
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    clippy::wildcard_enum_match_arm,
    clippy::match_like_matches_macro,
    reason = "unit-test scripts and assertions are ordinary Rust (programming-model.md, 10.2)"
)]
use super::revisions::{Harness, task, to};
use crate::*;
use alloc::boxed::Box;

const REQUESTER: u64 = 1;
const ASKER: u64 = 2;
const PERSON: u64 = 9;
const QUESTION: u64 = 1;
const ANSWER: u64 = 2;
const NOTICE: u64 = 3;
const ATTEMPT: u64 = 1;

fn pair() -> Harness {
    pair_with_asks(false)
}

fn pair_with_asks(asks: bool) -> Harness {
    let mut h = Harness::new();
    let mut parent = task(REQUESTER, &[]);
    parent.numbers.budget = 1000;
    parent.authority.budget.spend = 1000;
    h.make(vec![parent]);
    let mut child = task(ASKER, &[]);
    child.spec.asks = asks;
    child.funder = Funder::Task(REQUESTER);
    child.wake.answers = false;
    h.send(Event::Make { reply_to: to(), creator: Party::Task(REQUESTER), batch: Box::new([child]) });
    h.send(Event::Prepare { reply_to: to(), task: ASKER });
    h.send(Event::Claim {
        reply_to: to(),
        task: ASKER,
        attempt: ATTEMPT,
        resumed: false,
        budget: 10,
        account: None,
        writes: Box::new([]),
    });
    h.send(Event::Started { task: ASKER, attempt: ATTEMPT });
    h
}

fn word(h: &Harness, number: u64, from: Party, kind: MessageKind) -> Word {
    Word { number, from, kind, words: b"proceed?".as_slice().into(), at: h.env.wall, hits: 1, eligible: false }
}

fn asked() -> Harness {
    let mut h = pair();
    let word = word(&h, QUESTION, Party::Task(ASKER), MessageKind::Question);
    h.send(Event::Message { reply_to: to(), project: 1, task: REQUESTER, word });
    h
}

fn finish(h: &mut Harness, result: TaskResult) {
    h.send(Event::Activation {
        reply_to: to(),
        task: ASKER,
        attempt: ATTEMPT,
        end: End::Finished { result, cancel_delegates: false },
        saved: Some(Box::new([])),
        cause: Cause::Unpriced,
    });
}

fn park(h: &mut Harness) {
    finish(h, TaskResult::Report { words: b"ready".as_slice().into() });
}

fn answer(h: &mut Harness) {
    let word = word(h, ANSWER, Party::Person(PERSON), MessageKind::Answer { question: QUESTION });
    h.send(Event::PersonQuestionAnswer {
        reply_to: to(),
        project: 1,
        asked: REQUESTER,
        person: PERSON,
        question: QUESTION,
        word,
        notice: NOTICE,
    });
}

#[test]
fn a_task_with_its_own_question_unanswered_parks_on_finish() {
    let mut h = asked();
    park(&mut h);
    assert_eq!(h.domain.task(ASKER).unwrap().phase, Phase::Active(Active::Idle));
    assert_eq!(h.domain.task(ASKER).unwrap().tries, Tries::NONE);
    assert!(h.stored.values().any(|row| match row {
        Stored::Activity(ActivityLine {
            task: ASKER,
            line: Line::Kept { attempt: ATTEMPT, pending, question: Some(QUESTION) },
            ..
        }) => pending.is_empty(),
        _ => false,
    }));
}

#[test]
fn the_answer_wakes_a_task_kept_by_its_question() {
    let mut h = asked();
    park(&mut h);
    h.restart();
    answer(&mut h);
    let row = h.domain.task(ASKER).unwrap();
    assert_eq!(row.phase, Phase::Active(Active::Due));
    assert!(row.questions.is_empty());
    assert_eq!(row.inbox[0].from, Party::Task(REQUESTER));
}

#[test]
fn a_kept_question_does_not_prevent_failure() {
    let mut h = asked();
    finish(&mut h, TaskResult::Failure { reason: b"cannot proceed".as_slice().into() });
    assert!(match h.domain.task(ASKER).unwrap().phase {
        Phase::Closing(Closing { ending: Ending::Failed { .. }, .. }) => true,
        _ => false,
    });
}

#[test]
fn a_kept_question_does_not_prevent_cancellation() {
    let mut h = asked();
    park(&mut h);
    h.send(Event::Control {
        reply_to: to(),
        by: Party::Person(PERSON),
        task: ASKER,
        control: Control::Cancel { reason: Box::new([]), grace: None, message: None },
    });
    assert!(match h.domain.task(ASKER).unwrap().phase {
        Phase::Closing(Closing { ending: Ending::Cancelled { .. }, .. }) => true,
        _ => false,
    });
}

#[test]
fn a_task_answering_for_its_person_is_refused_by_name() {
    let mut h = asked();
    let word = word(&h, ANSWER, Party::Task(REQUESTER), MessageKind::Answer { question: QUESTION });
    let out = h.send(Event::Message { reply_to: to(), project: 1, task: ASKER, word });
    assert!(out.iter().any(|request| match request {
        Request::Refused { problem, .. } => problem.why == Refusal::PersonAnswers { question: QUESTION },
        _ => false,
    }));
    assert_eq!(h.domain.task(ASKER).unwrap().questions.len(), 1);
}

#[test]
fn the_persons_answer_reaches_the_asking_task() {
    let mut h = asked();
    answer(&mut h);
    assert!(
        h.domain
            .task(ASKER)
            .unwrap()
            .inbox
            .iter()
            .any(|word| word.from == Party::Task(REQUESTER) && word.kind == MessageKind::Answer { question: QUESTION })
    );
    assert_eq!(h.domain.task(REQUESTER).unwrap().question_lines[0].answer.unwrap().task, ASKER);
}

#[test]
fn the_asked_task_hears_the_question_answered_notice() {
    let mut h = asked();
    answer(&mut h);
    assert!(
        h.domain
            .task(REQUESTER)
            .unwrap()
            .inbox
            .iter()
            .any(|word| word.kind == MessageKind::QuestionAnswered { question: QUESTION })
    );
    assert!(
        !h.domain
            .task(REQUESTER)
            .unwrap()
            .inbox
            .iter()
            .any(|word| word.kind == MessageKind::Answer { question: QUESTION })
    );
}

#[test]
fn the_inbox_answer_keeps_the_chats_offered_question_readable() {
    let mut h = asked();
    h.send(Event::Prepare { reply_to: to(), task: REQUESTER });
    h.send(Event::Claim {
        reply_to: to(),
        task: REQUESTER,
        attempt: ANSWER,
        resumed: false,
        budget: 10,
        account: None,
        writes: Box::new([]),
    });
    answer(&mut h);
    let out = h.send(Event::Turn {
        reply_to: to(),
        task: REQUESTER,
        attempt: ANSWER,
        turn: 1,
        read: Some(QUESTION),
        refused: Box::new([]),
        offered: Some(QUESTION),
        outstanding: Box::new([]),
        cumulative: 0,
    });
    assert!(out.iter().any(|request| match request {
        Request::TurnAcknowledged { .. } => true,
        _ => false,
    }));
    assert!(
        h.domain
            .task(REQUESTER)
            .unwrap()
            .inbox
            .iter()
            .any(|word| word.kind == MessageKind::QuestionAnswered { question: QUESTION })
    );
}

#[test]
fn an_unread_question_reserves_its_asked_tasks_notice() {
    // One live delegate, the original question and its reserved notice fill three slots.
    const SLOTS: u32 = 3;
    let mut h = asked();
    h.env.limits.inbox_messages = SLOTS;
    h.env.limits.run_inbox = SLOTS;
    let word = word(&h, ANSWER, Party::Person(PERSON), MessageKind::Words);
    let out = h.send(Event::Message { reply_to: to(), project: 1, task: REQUESTER, word });
    assert!(out.iter().any(|request| match request {
        Request::Refused { problem, .. } => problem.why == Refusal::Busy,
        _ => false,
    }));
    answer(&mut h);
    assert!(
        h.domain
            .task(REQUESTER)
            .unwrap()
            .inbox
            .iter()
            .any(|word| word.kind == MessageKind::QuestionAnswered { question: QUESTION })
    );
}

#[test]
fn an_asked_tasks_new_person_answers_its_open_question() {
    const NEW_PERSON: u64 = 10;
    let mut h = asked();
    h.send(Event::Move {
        reply_to: to(),
        task: REQUESTER,
        person: NEW_PERSON,
        period: 0,
        pool_budget: 1000,
        period_budget: 10_000,
        reason: Box::new([]),
    });
    let word = word(&h, ANSWER, Party::Person(NEW_PERSON), MessageKind::Answer { question: QUESTION });
    h.send(Event::PersonQuestionAnswer {
        reply_to: to(),
        project: 1,
        asked: REQUESTER,
        person: NEW_PERSON,
        question: QUESTION,
        word,
        notice: NOTICE,
    });
    assert!(h.domain.task(ASKER).unwrap().questions.is_empty());
    assert_eq!(h.domain.task(ASKER).unwrap().inbox[0].from, Party::Task(REQUESTER));
}

#[test]
fn moving_a_question_recipient_refuses_without_notice_room() {
    // The original question fills this recipient's only slot.
    const SLOTS: u32 = 1;
    let mut h = pair();
    let word = word(&h, QUESTION, Party::Task(REQUESTER), MessageKind::Question);
    h.send(Event::Message { reply_to: to(), project: 1, task: ASKER, word });
    h.env.limits.inbox_messages = SLOTS;
    h.env.limits.run_inbox = SLOTS;
    let out = h.send(Event::Move {
        reply_to: to(),
        task: ASKER,
        person: PERSON,
        period: 0,
        pool_budget: 1000,
        period_budget: 10_000,
        reason: Box::new([]),
    });
    assert!(out.iter().any(|request| match request {
        Request::Refused { problem, .. } => problem.why == Refusal::Busy,
        _ => false,
    }));
    assert_eq!(h.domain.task(ASKER).unwrap().requester, Party::Task(REQUESTER));
}

#[test]
fn a_question_ending_unanswered_lifts_the_keep_with_a_notice() {
    let mut h = asked();
    park(&mut h);
    let out = h.send(Event::CancelQuestion { reply_to: to(), task: ASKER, question: QUESTION });
    for request in out {
        if let Request::QuestionEndedDue { task, questions } = request {
            let notices = questions
                .iter()
                .enumerate()
                .map(|(i, question)| QuestionNotice {
                    question: *question,
                    message: NOTICE + task + u64::try_from(i).unwrap(),
                })
                .collect();
            h.send(Event::QuestionEndedNotice { task, notices });
        }
    }
    let row = h.domain.task(ASKER).unwrap();
    assert_eq!(row.phase, Phase::Active(Active::Due));
    assert!(row.inbox.iter().any(|word| word.kind == MessageKind::QuestionEnded { question: QUESTION }));
    assert!(h.domain.task(REQUESTER).unwrap().question_lines.is_empty());
}

fn moved() -> Harness {
    let mut h = asked();
    park(&mut h);
    h.send(Event::Move {
        reply_to: to(),
        task: ASKER,
        person: PERSON,
        period: 0,
        pool_budget: 1000,
        period_budget: 10_000,
        reason: Box::new([]),
    });
    h.restart();
    h
}

#[test]
fn a_moved_asker_carries_its_open_question_to_its_new_requester() {
    let h = moved();
    let row = h.domain.task(ASKER).unwrap();
    assert_eq!(row.requester, Party::Person(PERSON));
    assert_eq!(row.phase, Phase::Active(Active::Idle));
    assert_eq!(row.questions[0].number, QUESTION);
    assert_eq!(row.questions[0].asked, Party::Person(PERSON));
    assert_eq!(row.questions[0].answerer, Party::Person(PERSON));
    assert_eq!(row.question_lines[0].message, QUESTION);
    assert_eq!(row.question_lines[0].line, row.questions[0].line);
    assert!(h.stored.values().any(|stored| match stored {
        Stored::Activity(line) if line.task == ASKER && line.position == row.questions[0].line => match &line.line {
            Line::Words { message: QUESTION, kind: MessageKind::Question, words, .. } => words.as_ref() == b"proceed?",
            _ => false,
        },
        _ => false,
    }));
    assert!(h.domain.task(REQUESTER).unwrap().question_lines.is_empty());
}

#[test]
fn a_question_through_a_person_task_keeps_nothing() {
    const PERSON_TASK: u64 = 3;
    let mut h = pair();
    let mut question = task(PERSON_TASK, &[]);
    question.executor = Executor::Person(PersonAddress::Person(PERSON));
    question.funder = Funder::Task(ASKER);
    question.numbers.budget = 20;
    question.authority.budget.spend = 20;
    h.send(Event::Make { reply_to: to(), creator: Party::Task(ASKER), batch: Box::new([question]) });
    let word = word(&h, QUESTION, Party::Task(ASKER), MessageKind::Words);
    h.send(Event::Message { reply_to: to(), project: 1, task: PERSON_TASK, word });
    assert!(h.domain.task(ASKER).unwrap().questions.is_empty());
    assert_eq!(h.domain.waiting_effect_problem(ASKER), None);
}

#[test]
fn an_asked_task_ending_unanswered_notifies_its_asker() {
    const PEER: u64 = 3;
    let mut h = pair();
    let mut peer = task(PEER, &[]);
    peer.funder = Funder::Task(REQUESTER);
    h.send(Event::Make { reply_to: to(), creator: Party::Task(REQUESTER), batch: Box::new([peer]) });
    h.send(Event::Introduce { reply_to: to(), by: REQUESTER, left: ASKER, right: PEER });
    let word = word(&h, QUESTION, Party::Task(ASKER), MessageKind::Question);
    h.send(Event::Message { reply_to: to(), project: 1, task: PEER, word });
    h.send(Event::Activation {
        reply_to: to(),
        task: ASKER,
        attempt: ATTEMPT,
        end: End::Parked,
        saved: None,
        cause: Cause::Unpriced,
    });
    h.send(Event::Prepare { reply_to: to(), task: PEER });
    h.send(Event::Claim {
        reply_to: to(),
        task: PEER,
        attempt: ANSWER,
        resumed: false,
        budget: 10,
        account: None,
        writes: Box::new([]),
    });
    h.send(Event::Activation {
        reply_to: to(),
        task: PEER,
        attempt: ANSWER,
        end: End::Finished { result: TaskResult::Report { words: b"done".as_slice().into() }, cancel_delegates: false },
        saved: None,
        cause: Cause::Unpriced,
    });
    h.send(Event::EffectsSettled { task: PEER });
    let out = h.send(Event::Settled { task: PEER });
    for request in out {
        if let Request::QuestionEndedDue { task, questions } = request {
            let notices = questions
                .iter()
                .enumerate()
                .map(|(i, question)| QuestionNotice {
                    question: *question,
                    message: NOTICE + task + u64::try_from(i).unwrap(),
                })
                .collect();
            h.send(Event::QuestionEndedNotice { task, notices });
        }
    }
    let row = h.domain.task(ASKER).unwrap();
    assert!(row.questions.is_empty());
    assert_eq!(row.phase, Phase::Active(Active::Due));
    assert!(row.inbox.iter().any(|word| word.kind == MessageKind::QuestionEnded { question: QUESTION }));
}

#[test]
fn cancelling_a_carried_question_removes_its_persons_inbox_entry() {
    let mut h = moved();
    h.send(Event::CancelQuestion { reply_to: to(), task: ASKER, question: QUESTION });
    h.send(Event::QuestionEndedNotice {
        task: ASKER,
        notices: Box::new([QuestionNotice { question: QUESTION, message: NOTICE }]),
    });
    let row = h.domain.task(ASKER).unwrap();
    assert!(row.question_lines.is_empty());
    assert_eq!(row.phase, Phase::Active(Active::Due));
}

#[test]
fn carrying_a_full_question_set_adds_at_most_one_publication_per_task() {
    const PEER: u64 = 3;
    let mut h = asked();
    h.env.limits.message_bytes = 16;
    let mut peer = task(PEER, &[]);
    peer.funder = Funder::Task(REQUESTER);
    h.send(Event::Make { reply_to: to(), creator: Party::Task(REQUESTER), batch: Box::new([peer]) });
    h.send(Event::Introduce { reply_to: to(), by: REQUESTER, left: ASKER, right: PEER });
    for number in 2..=u64::from(h.env.limits.inbox_messages) {
        let word = word(&h, number, Party::Task(ASKER), MessageKind::Question);
        h.send(Event::Message { reply_to: to(), project: 1, task: PEER, word });
    }
    let count = h.domain.task(ASKER).unwrap().questions.len();
    assert_eq!(count, usize::try_from(h.env.limits.inbox_messages).unwrap());
    let out = h.send(Event::Move {
        reply_to: to(),
        task: ASKER,
        person: PERSON,
        period: 0,
        pool_budget: 1000,
        period_budget: 10_000,
        reason: Box::new([]),
    });
    assert!(out.len() <= usize::try_from(max_out(&h.env.limits)).unwrap());
    for task in [REQUESTER, ASKER, PEER] {
        // Move publishes its changed topology and requester revisions; bookkeeping
        // adds at most one final publication, regardless of credit count.
        let saves = out
            .iter()
            .filter(|request| match request {
                Request::Save { record: Stored::Live(row) } => row.number == task,
                _ => false,
            })
            .count();
        assert!(saves <= 3, "{task}: {saves}");
    }
    assert!(!out.iter().any(|request| match request {
        Request::Save {
            record: Stored::Activity(ActivityLine { line: Line::Words { kind: MessageKind::Question, .. }, .. }),
        } => true,
        _ => false,
    }));
    let row = h.domain.task(ASKER).unwrap();
    assert_eq!(row.questions.len(), count);
    assert_eq!(row.question_lines.len(), count);
    assert!(row.questions.iter().all(|credit| credit.asked == Party::Person(PERSON)));
    h.restart();
    assert_eq!(h.domain.task(ASKER).unwrap().questions.len(), count);
}

#[test]
fn retiring_a_full_question_set_routes_one_numbered_batch_per_asker() {
    const PEER: u64 = 3;
    let mut h = pair();
    h.env.limits.message_bytes = 16;
    let mut peer = task(PEER, &[]);
    peer.funder = Funder::Task(REQUESTER);
    h.send(Event::Make { reply_to: to(), creator: Party::Task(REQUESTER), batch: Box::new([peer]) });
    h.send(Event::Introduce { reply_to: to(), by: REQUESTER, left: ASKER, right: PEER });
    for number in 1..=u64::from(h.env.limits.inbox_messages) {
        let word = word(&h, number, Party::Task(ASKER), MessageKind::Question);
        h.send(Event::Message { reply_to: to(), project: 1, task: PEER, word });
    }
    let count = h.domain.task(ASKER).unwrap().questions.len();
    assert_eq!(count, usize::try_from(h.env.limits.inbox_messages).unwrap());
    h.send(Event::Activation {
        reply_to: to(),
        task: ASKER,
        attempt: ATTEMPT,
        end: End::Parked,
        saved: None,
        cause: Cause::Unpriced,
    });
    h.send(Event::Control {
        reply_to: to(),
        by: Party::Task(REQUESTER),
        task: PEER,
        control: Control::Cancel { reason: Box::new([]), grace: None, message: None },
    });
    h.send(Event::EffectsSettled { task: PEER });
    let out = h.send(Event::Settled { task: PEER });
    assert!(out.len() <= usize::try_from(max_out(&h.env.limits)).unwrap());
    let batches: Box<[_]> = out
        .into_iter()
        .filter_map(|request| match request {
            Request::QuestionEndedDue { task, questions } => Some((task, questions)),
            _ => None,
        })
        .collect();
    assert_eq!(batches.len(), 1);
    let (task, questions) = batches.into_iter().next().unwrap();
    assert_eq!(task, ASKER);
    assert_eq!(questions.len(), count);
    let notices = questions
        .iter()
        .enumerate()
        .map(|(i, question)| QuestionNotice { question: *question, message: 100 + u64::try_from(i).unwrap() })
        .collect();
    let out = h.send(Event::QuestionEndedNotice { task, notices });
    assert!(out.len() <= usize::try_from(max_out(&h.env.limits)).unwrap());
    assert_eq!(
        out.iter()
            .filter(|request| match request {
                Request::Save { record: Stored::Live(row) } => row.number == ASKER,
                _ => false,
            })
            .count(),
        2 // One publication for the inbox batch and one for its wake.
    );
    let row = h.domain.task(ASKER).unwrap();
    assert_eq!(row.phase, Phase::Active(Active::Due));
    assert!(row.questions.is_empty());
    assert_eq!(row.inbox.len(), count);
    for question in questions {
        assert_eq!(row.inbox.iter().filter(|word| word.kind == MessageKind::QuestionEnded { question }).count(), 1);
    }
}

#[test]
fn a_task_owing_a_question_parks_on_finish_and_wakes_with_a_notice() {
    let mut h = pair_with_asks(true);
    park(&mut h);
    h.restart();
    let out = h.send(Event::QuestionOwedNotice { task: ASKER, message: NOTICE });
    let row = h.domain.task(ASKER).unwrap();
    assert_eq!(row.phase, Phase::Active(Active::Due));
    assert_eq!(row.tries.run, 1);
    assert!(row.inbox.iter().any(|word| word.kind == MessageKind::QuestionOwed));
    assert!(out.iter().any(|request| match request {
        Request::Activate { .. } => true,
        _ => false,
    }));
}

#[test]
fn once_asked_the_owing_task_is_kept_by_its_question() {
    let mut h = pair_with_asks(true);
    let word = word(&h, QUESTION, Party::Task(ASKER), MessageKind::Question);
    h.send(Event::Message { reply_to: to(), project: 1, task: REQUESTER, word });
    h.restart();
    park(&mut h);
    assert!(h.domain.task(ASKER).unwrap().asked_requester);
    assert_eq!(h.domain.task(ASKER).unwrap().phase, Phase::Active(Active::Idle));
    assert_eq!(h.domain.task(ASKER).unwrap().questions[0].number, QUESTION);
    assert!(!h.domain.task(ASKER).unwrap().inbox.iter().any(|word| word.kind == MessageKind::QuestionOwed));
}

#[test]
fn an_owing_task_that_never_asks_ends_by_its_tries() {
    let mut h = pair_with_asks(true);
    let attempts = u64::from(h.env.limits.retries.run.retries) + 1;
    for attempt in 1..=attempts {
        if attempt > ATTEMPT {
            h.send(Event::Prepare { reply_to: to(), task: ASKER });
            h.send(Event::Claim {
                reply_to: to(),
                task: ASKER,
                attempt,
                resumed: true,
                budget: 10,
                account: None,
                writes: Box::new([]),
            });
        }
        h.send(Event::Activation {
            reply_to: to(),
            task: ASKER,
            attempt,
            end: End::Finished {
                result: TaskResult::Report { words: b"plan".as_slice().into() },
                cancel_delegates: false,
            },
            saved: None,
            cause: Cause::Unpriced,
        });
        h.send(Event::QuestionOwedNotice { task: ASKER, message: NOTICE + attempt });
    }
    assert!(match h.domain.task(ASKER).unwrap().phase {
        Phase::Closing(Closing { ending: Ending::Failed { .. }, .. }) => true,
        _ => false,
    });
    assert_eq!(u64::from(h.domain.task(ASKER).unwrap().tries.run), attempts);
}

#[test]
fn asks_cannot_be_amended() {
    use super::revisions::amendment;
    for (old, replacement) in [(false, true), (true, false)] {
        let mut h = pair_with_asks(old);
        let mut change = amendment();
        let mut spec = h.domain.task(ASKER).unwrap().spec.clone();
        spec.asks = replacement;
        change.spec = Some(spec);
        let out = h.send(Event::Amend {
            reply_to: to(),
            by: Party::Task(REQUESTER),
            task: ASKER,
            places: Box::new([]),
            funder: None,
            stop_run: false,
            amendment: change,
            message: NOTICE,
        });
        assert!(out.iter().any(|request| match request {
            Request::Refused { problem, .. } => problem.why == Refusal::Spec,
            _ => false,
        }));
        assert_eq!(h.domain.task(ASKER).unwrap().spec.asks, old);
    }
}
