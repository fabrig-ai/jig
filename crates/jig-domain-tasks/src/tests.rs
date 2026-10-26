use crate::{Retry, failures::backoff};
use skein_lib::{Duration, Rng};

#[test]
fn exponential_equal_jitter_saturates_without_overflow_and_replays() {
    let retry = Retry { retries: 4, base: Duration::from_millis(10), max: Duration::from_secs(1) };
    let mut first = Rng::new(42);
    let mut second = Rng::new(42);
    for times in [1, 2, 3, 4, 7, 32, 64, u32::MAX] {
        let factor = 1_u64.checked_shl(times.saturating_sub(1)).unwrap_or(u64::MAX);
        let ceiling = retry.base.saturating_mul(factor).min(retry.max);
        let pause = backoff(times, retry, &mut first);
        assert!(
            pause.as_nanos() >= ceiling.as_nanos().div_euclid(2) && pause <= ceiling,
            "pause stays in equal-jitter half-to-ceiling range"
        );
        assert_eq!(pause, backoff(times, retry, &mut second), "same seed replays backoff");
    }
}

/// One configured charter, number 1, with the one configured model, which declares no effort.
pub(crate) fn charters() -> Box<[crate::CharterModels]> {
    Box::new([crate::CharterModels { number: 1, models: Box::new([0]) }])
}

mod decision_reads;
mod hold_origin;
mod kept;
mod keys;
mod refusals;
mod revisions;
mod waiting;

mod periods;

mod last_batch;

mod changed;

mod address;
mod instructions;

mod deadlines;

mod index;

mod payer;

mod account;

mod waits;

mod settings_proposals;

#[test]
fn projection_news_uses_reserved_or_refusable_room_and_stays_quiet() {
    use self::revisions::{Harness, task, to};
    use crate::{Event, MessageKind, NewsClass, Party, ProjectionNewsResult, Request, Word};
    use skein_lib::{Queue, Wall};
    for tracked in [None, Some(1)] {
        let mut test = Harness::new();
        let mut row = task(1, &[]);
        row.tracked = tracked;
        test.send(Event::Make { reply_to: to(), creator: Party::Person(9), batch: Box::new([row]) });
        let before = test.domain.task(1).expect("live goal").phase.clone();
        // A tracked goal keeps a full message credit; ordinary words cannot take it.
        for number in 1..=8_u64 {
            test.send(Event::Message {
                reply_to: to(),
                project: 1,
                task: 1,
                word: Word {
                    number,
                    from: Party::Person(9),
                    kind: MessageKind::Words,
                    words: Box::from(b"12345678901234567890123456789012".as_slice()),
                    at: Wall::EPOCH,
                    hits: 1,
                    eligible: false,
                },
            });
        }
        let word = Word {
            number: 20,
            from: Party::Task(1),
            kind: MessageKind::News { subscription: 0, class: NewsClass::Kept },
            words: Box::from(b"Projection 1 failed".as_slice()),
            at: Wall::from_nanos(7),
            hits: 1,
            eligible: false,
        };
        let room = test.domain.projection_news_room(&test.env.limits, 1, 19);
        assert_eq!(room, tracked.is_some(), "only the tracked reservation remains");
        let snapshot = test.domain.task(1).expect("live task").clone();
        let mut out = Queue::with_capacity(crate::max_out(&test.env.limits));
        let result = crate::projection_news(&mut test.domain, &test.env, 1, word, &mut out);
        if tracked.is_some() {
            assert_eq!(result, ProjectionNewsResult::Delivered);
            assert_eq!(test.domain.task(1).expect("goal").phase, before, "quiet news changes no execution phase");
            let mut heard = false;
            for _ in 0..out.len() {
                if let Request::Save { record: crate::Stored::Activity(line) } = out.pop().expect("news output")
                    && let crate::Line::Heard(word) = line.line
                {
                    heard = word.at == Wall::from_nanos(7);
                }
            }
            assert!(heard, "activity keeps receipt time");
        } else {
            assert_eq!(result, ProjectionNewsResult::Full);
            assert!(out.is_empty(), "full has no publication");
            assert_eq!(test.domain.task(1), Some(&snapshot));
        }
    }
}

#[test]
fn projection_news_checks_bytes_and_all_outstanding_credits_before_mutation() {
    use self::revisions::{Harness, task, to};
    use crate::{Event, MessageKind, NewsClass, Party, ProjectionNewsResult, Word};
    use skein_lib::{Queue, Wall};
    let mut test = Harness::new();
    test.send(Event::Make { reply_to: to(), creator: Party::Person(9), batch: Box::new([task(1, &[])]) });
    // The requested payload fits exactly; a byte less leaves the row untouched.
    test.env.limits.inbox_bytes = 19;
    let word = Word {
        number: 1,
        from: Party::Task(1),
        kind: MessageKind::News { subscription: 0, class: NewsClass::Kept },
        words: b"Projection 1 failed".as_slice().into(),
        at: Wall::EPOCH,
        hits: 1,
        eligible: false,
    };
    assert!(test.domain.projection_news_room(&test.env.limits, 1, 19));
    test.env.limits.inbox_bytes = 18;
    let before = test.domain.task(1).expect("live").clone();
    let mut out = Queue::with_capacity(crate::max_out(&test.env.limits));
    assert_eq!(
        crate::projection_news(&mut test.domain, &test.env, 1, word.clone(), &mut out),
        ProjectionNewsResult::Full
    );
    assert_eq!(test.domain.task(1), Some(&before));
    test.env.limits.inbox_bytes = 19;
    assert_eq!(crate::projection_news(&mut test.domain, &test.env, 1, word, &mut out), ProjectionNewsResult::Delivered);
    assert!(out.len() <= crate::max_out(&test.env.limits), "publication fits declared max_out");
}

#[test]
fn archived_projection_news_delivers_without_changing_the_ending() {
    use self::revisions::{Harness, task, to};
    use crate::{Event, MessageKind, NewsClass, Party, ProjectionNewsResult, Word};
    use skein_lib::{Queue, Wall};
    let mut test = Harness::new();
    test.send(Event::Make { reply_to: to(), creator: Party::Person(9), batch: Box::new([task(1, &[])]) });
    let mut row = test.domain.task(1).unwrap().clone();
    row.phase =
        crate::Phase::Ended(crate::Ending::Done(crate::TaskResult::Report { words: b"done".as_slice().into() }));
    row.ended_at = Some(Wall::from_nanos(5));
    row.result_position = 7;
    let before = row.clone();
    let word = Word {
        number: row.last_message + 1,
        from: Party::Task(1),
        kind: MessageKind::News { subscription: 0, class: NewsClass::Kept },
        words: b"Projection 1 failed".as_slice().into(),
        at: Wall::from_nanos(17),
        hits: 1,
        eligible: false,
    };
    let mut out = Queue::with_capacity(1);
    assert_eq!(
        crate::archived_projection_news(&mut row, &test.env.limits, word.clone(), &mut out),
        ProjectionNewsResult::Delivered
    );
    let Some(crate::Request::Save { record: crate::Stored::Activity(line) }) = out.pop() else {
        panic!("archived activity")
    };
    assert_eq!(line.at, word.at);
    assert_eq!(line.position, before.activity + 1);
    assert_eq!(line.line, crate::Line::Heard(word.clone()));
    assert_eq!(row.last_message, word.number);
    assert_eq!(row.activity, before.activity + 1);
    row.last_message = before.last_message;
    row.activity = before.activity;
    assert_eq!(row, before, "only message and Activity positions advance");
}

#[test]
fn archived_projection_news_refuses_full_missing_and_invalid_without_mutation() {
    use self::revisions::{Harness, task, to};
    use crate::{Event, MessageKind, NewsClass, Party, ProjectionNewsResult, Word};
    use skein_lib::{Queue, Wall};
    let mut test = Harness::new();
    test.send(Event::Make { reply_to: to(), creator: Party::Person(9), batch: Box::new([task(1, &[])]) });
    let live = test.domain.task(1).unwrap().clone();
    for case in 0_u8..10 {
        let mut row = live.clone();
        row.phase =
            crate::Phase::Ended(crate::Ending::Done(crate::TaskResult::Report { words: b"done".as_slice().into() }));
        let mut word = Word {
            number: row.last_message + 1,
            from: Party::Task(1),
            kind: MessageKind::News { subscription: 0, class: NewsClass::Kept },
            words: b"Projection 1 failed".as_slice().into(),
            at: Wall::EPOCH,
            hits: 1,
            eligible: false,
        };
        let expected = match case {
            0 => {
                row.activity = u64::MAX;
                ProjectionNewsResult::Full
            }
            1 => {
                row.last_message = u64::MAX;
                ProjectionNewsResult::Full
            }
            2 => {
                row.phase = live.phase.clone();
                ProjectionNewsResult::Missing
            }
            3 => {
                word.kind = MessageKind::Words;
                ProjectionNewsResult::Invalid
            }
            4 => {
                word.from = Party::Person(9);
                ProjectionNewsResult::Invalid
            }
            5 => {
                word.number = 0;
                ProjectionNewsResult::Invalid
            }
            6 => {
                word.words = Box::new([]);
                ProjectionNewsResult::Invalid
            }
            7 => {
                word.words = skein_lib::bytes::zeroed(usize::try_from(test.env.limits.message_bytes).unwrap() + 1);
                ProjectionNewsResult::Invalid
            }
            8 => {
                row.last_message = word.number;
                ProjectionNewsResult::Invalid
            }
            _ => {
                word.kind = MessageKind::News { subscription: 0, class: NewsClass::Wakes };
                ProjectionNewsResult::Invalid
            }
        };
        let before = row.clone();
        let mut out = Queue::with_capacity(1);
        assert_eq!(crate::archived_projection_news(&mut row, &test.env.limits, word, &mut out), expected);
        assert_eq!(row, before);
        assert!(out.is_empty());
    }
}
