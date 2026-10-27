use crate::{
    contract::{Bound, Exceeded, HoldCategory as H, HoldFailure, Phase},
    words::{self, Word},
};
use skein_ui::phrases::{Form, Hole, Language, PhraseLimits, Phrasebook, PluralClass, Segment};
const TEXT: u32 = 32;
const TASK: u64 = 7;
const HOLDS: &[(H, &[u8])] = &[
    (
        H::Exceeded(Exceeded { bound: Bound::Reasoning, value: u64::MAX }),
        b"Held: the model's reasoning went past its configured allowance of 18446744073709551615 bytes",
    ),
    (
        H::Exceeded(Exceeded { bound: Bound::Metadata, value: u64::MAX }),
        b"Held: the model's metadata went past its configured allowance of 18446744073709551615 bytes",
    ),
    (
        H::Exceeded(Exceeded { bound: Bound::Items, value: u64::MAX }),
        b"Held: the model's answer went past its configured allowance of 18446744073709551615 items",
    ),
    (
        H::Exceeded(Exceeded { bound: Bound::Answer, value: u64::MAX }),
        b"Held: the model's answer went past its configured allowance of 18446744073709551615 bytes",
    ),
    (
        H::Exceeded(Exceeded { bound: Bound::Tools, value: u64::MAX }),
        b"Held: the run's tools went past their configured allowance of 18446744073709551615 tools",
    ),
    (
        H::Exceeded(Exceeded { bound: Bound::HistoryItems, value: u64::MAX }),
        b"Held: the run's request went past its configured allowance of 18446744073709551615 messages and blocks",
    ),
    (
        H::Exceeded(Exceeded { bound: Bound::History, value: u64::MAX }),
        b"Held: the run's history no longer fits its configured allowance, even compacted",
    ),
    (
        H::Exceeded(Exceeded { bound: Bound::Window, value: u64::MAX }),
        b"Held: the run no longer fits the model's window of 18446744073709551615 tokens",
    ),
    (
        H::Exceeded(Exceeded { bound: Bound::Output, value: u64::MAX }),
        b"Held: the request went past the model's output ceiling of 18446744073709551615 tokens",
    ),
    (H::Circle, b"Held for a circle of waits"),
    (H::Failures { class: HoldFailure::Lost }, b"Held: out of tries"),
    (H::Stalled, b"The executor stopped progressing"),
    (H::Dependency { task: TASK }, b"Dependency task 7"),
    (H::Stopped, b"Task stopped"),
    (H::Placed { hold: TASK }, b"Held by a placed hold"),
    (H::Drift, b"Required resource drifted"),
    (H::Effects, b"Effects need a decision"),
    (H::EffectFailed, b"An effect failed permanently"),
    (H::Uncertain { entry: TASK }, b"Uncertain outbox entry 7"),
    (H::Budget, b"Insufficient activation budget"),
    (H::Deadline, b"Deadline passed"),
    (H::HoldsWaited, b"Resource holds waited too long"),
    (H::ResourceUnavailable, b"A required connector resource is unavailable"),
    (H::ResourceAccess, b"Current resource access forbids this action"),
    (H::PoolLost { pool: TASK }, b"Allocation lost from pool 7"),
    (H::Procedure, b"Procedure needs a decision"),
];
fn book(languages: Box<[Language]>) -> Phrasebook {
    let mut segments = 0_u32;
    for language in &languages {
        for phrase in &language.phrases {
            for form in &phrase.forms {
                segments = segments
                    .checked_add(u32::try_from(form.segments.len()).expect("bounded phrase segments"))
                    .expect("bounded language fixtures");
            }
        }
    }
    let limits =
        PhraseLimits { languages: 2, phrases: words::PHRASES, holes: 3, segments, text_bytes: 8192, hole_bytes: TEXT };
    Phrasebook::load(words::phrase_shapes(TEXT), languages, &limits).expect("bounded public words")
}
#[test]
fn each_hold_category_says_its_own_phrase() {
    let book = book(Box::new([words::english()]));
    for (hold, expected) in HOLDS {
        let holes: &[Hole] = match hold {
            H::Exceeded(Exceeded { bound: Bound::History, .. })
            | H::Circle
            | H::Failures { .. }
            | H::Stalled
            | H::Stopped
            | H::Placed { .. }
            | H::Drift
            | H::Effects
            | H::EffectFailed
            | H::Budget
            | H::Deadline
            | H::HoldsWaited
            | H::ResourceUnavailable
            | H::ResourceAccess
            | H::Procedure => &[],
            H::Exceeded(Exceeded { value, .. }) => &[Hole::Number(*value)],
            H::Dependency { .. } | H::Uncertain { .. } | H::PoolLost { .. } => &[Hole::Number(TASK)],
        };
        assert_eq!(book.say(0, Word::hold(hold).id(), holes).unwrap().text.as_ref(), *expected, "{hold:?}");
    }
}
#[test]
fn an_application_s_language_overrides_one_word_and_keeps_the_others() {
    let mut language = words::english();
    for phrase in &mut language.phrases {
        if phrase.id == Word::Phase(Phase::Held).id() {
            phrase.forms = Box::new([Form {
                class: PluralClass::Other,
                segments: Box::new([Segment::Text(Box::from(b"On hold".as_slice()))]),
            }]);
        }
    }
    let book = book(Box::new([words::english(), language]));
    assert_eq!(book.say(1, Word::Phase(Phase::Held).id(), &[]).unwrap().text.as_ref(), b"On hold");
    assert_eq!(book.say(1, Word::Phase(Phase::Running).id(), &[]).unwrap().text.as_ref(), b"Running");
}

const PHASES: &[(Phase, &[u8])] = &[
    (Phase::Waiting, b"Waiting"),
    (Phase::Idle, b"Waiting for news"),
    (Phase::Due, b"Due to run"),
    (Phase::Preparing, b"Preparing a host"),
    (Phase::Claimed, b"Claimed by a host"),
    (Phase::Running, b"Running"),
    (Phase::BackingOff, b"Retrying"),
    (Phase::Active, b"Executor active"),
    (Phase::Closing, b"Closing"),
    (Phase::Held, b"Held"),
    (Phase::Ended, b"Ended"),
    (Phase::Done, b"Completed"),
    (Phase::Failed, b"Failed"),
    (Phase::Cancelled, b"Cancelled"),
];
#[test]
fn each_phase_says_its_existing_english() {
    let book = book(Box::new([words::english()]));
    for (phase, expected) in PHASES {
        assert_eq!(book.say(0, Word::Phase(*phase).id(), &[]).unwrap().text.as_ref(), *expected, "{phase:?}");
    }
}

#[test]
fn a_party_hold_owner_refusal_names_the_hold_and_person() {
    let book = book(Box::new([words::english()]));
    let refusal = crate::contract::Refusal::Authority {
        lacking: crate::contract::Lacking::HoldOwner {
            hold: 7,
            owner: crate::contract::HoldOwner::Party { person: 9 },
        },
        proposable: None,
    };
    assert_eq!(
        book.say(0, Word::refusal(&refusal).id(), &[Hole::Number(7), Hole::Number(9)]).unwrap().text.as_ref(),
        b"Held by hold 7, placed by person 9: only they, or a role above theirs, may release it"
    );
}
#[test]
fn a_task_hold_owner_refusal_names_the_hold_and_task() {
    let book = book(Box::new([words::english()]));
    let refusal = crate::contract::Refusal::Authority {
        lacking: crate::contract::Lacking::HoldOwner { hold: 7, owner: crate::contract::HoldOwner::Task { task: 9 } },
        proposable: None,
    };
    assert_eq!(
        book.say(0, Word::refusal(&refusal).id(), &[Hole::Number(7), Hole::Number(9)]).unwrap().text.as_ref(),
        b"Held by hold 7, placed by task 9: only they, or a role above theirs, may release it"
    );
}
#[test]
fn a_lifted_outcome_names_the_hold_and_released_count() {
    let book = book(Box::new([words::english()]));
    assert_eq!(
        book.say(0, Word::Lifted.id(), &[Hole::Number(7), Hole::Number(2)]).unwrap().text.as_ref(),
        b"Hold 7 lifted, 2 released"
    );
}
