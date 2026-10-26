use super::*;
use crate::Bounds;
use jig_client_domain::contract::*;
use skein_lib::{Wall, bytes::copy_of};
fn text(bytes: &[u8]) -> Text {
    Text::new(copy_of(bytes), u32::try_from(bytes.len()).unwrap()).unwrap()
}
fn bounds() -> Bounds {
    Bounds { text: 8, rows: 3, document_bytes: u64::MAX, ..Bounds::default() }
}
fn fields() -> List<ResultField> {
    let mut fields = List::with_capacity(3);
    fields.push(ResultField { name: copy_of(&[255, 0]), value: copy_of(b"opaque") }).unwrap();
    fields.push(ResultField { name: copy_of(b"a/b"), value: copy_of(&[254, 1]) }).unwrap();
    fields
}
fn word(kind: OwnerMessageKind) -> OwnerWord {
    OwnerWord {
        number: u64::MAX,
        from: OwnerRequester::Task(u64::MAX),
        kind,
        words: text(UTF8_TEXTS[0]),
        at: Wall::from_nanos(u64::MAX),
        hits: u32::MAX,
        eligible: true,
    }
}
fn line(line: OwnerActivity) -> OwnerActivityLine {
    OwnerActivityLine { task: u64::MAX, position: u64::MAX, at: Wall::from_nanos(u64::MAX), line }
}
#[test]
fn owner_activity_whole_word_and_line_price_every_inline_reserved_field_and_opaque_byte_at_plus_minus_one() {
    let value = word(OwnerMessageKind::Result(OwnerWordResult::Verdict { code: u32::MAX, fields: fields() }));
    let saved = value.clone();
    let heap = List::<ResultField>::worst_case(3).unwrap() + 2 + 6 + 3 + 2 + 2;
    let expected = u64::try_from(size_of::<OwnerWord>()).unwrap() + heap;
    assert_eq!(word_bytes(&value, &bounds()), Some(expected));
    for (limit, admitted) in [(expected - 1, false), (expected, true), (expected + 1, true)] {
        assert_eq!(word_fits(&value, &Bounds { document_bytes: limit, ..bounds() }), admitted);
        assert_eq!(value, saved);
    }
    for activity in [
        OwnerActivity::Heard(value.clone()),
        OwnerActivity::Notified { recipient: OwnerRequester::Person(7), word: value.clone() },
        OwnerActivity::Words {
            message: u64::MAX,
            by: value.from,
            kind: value.kind.clone(),
            words: value.words.clone(),
        },
    ] {
        let value = line(activity);
        let saved = value.clone();
        let expected = u64::try_from(size_of::<OwnerActivityLine>()).unwrap() + heap;
        assert_eq!(bytes(&value, &bounds()), Some(expected));
        for (limit, admitted) in [(expected - 1, false), (expected, true), (expected + 1, true)] {
            assert_eq!(fits(&value, &Bounds { document_bytes: limit, ..bounds() }), admitted);
            assert_eq!(value, saved);
        }
    }
}
#[test]
fn owner_activity_approved_absent_empty_nonempty_and_invalid_presence_are_whole_and_independent() {
    let mut saved = Vec::new();
    for (present, bytes_value) in [(false, b"".as_slice()), (true, b""), (true, UTF8_TEXTS[0])] {
        let value = OwnerWord {
            words: text(bytes_value),
            ..word(OwnerMessageKind::Result(OwnerWordResult::Approved {
                state: text(UTF8_TEXTS[1]),
                words_present: present,
            }))
        };
        let expected = u64::try_from(size_of::<OwnerWord>()).unwrap() + 2 + u64::try_from(bytes_value.len()).unwrap();
        assert_eq!(word_bytes(&value, &bounds()), Some(expected));
        assert!(word_fits(&value, &Bounds { document_bytes: expected, ..bounds() }));
        assert!(!word_fits(&value, &Bounds { document_bytes: expected - 1, ..bounds() }));
        for previous in &saved {
            assert_ne!(&value, previous);
        }
        saved.push(value);
    }
    assert_eq!(saved.len(), 3);
    assert_eq!(word_bytes(&saved[0], &bounds()), word_bytes(&saved[1], &bounds()));
    let invalid =
        word(OwnerMessageKind::Result(OwnerWordResult::Approved { state: text(b"state"), words_present: false }));
    let saved = invalid.clone();
    assert_eq!(word_bytes(&invalid, &bounds()), None);
    assert!(!word_fits(&invalid, &bounds()));
    assert_eq!(invalid, saved);
    for activity in [
        OwnerActivity::Heard(invalid.clone()),
        OwnerActivity::Notified { recipient: invalid.from, word: invalid.clone() },
        OwnerActivity::Words { message: 1, by: invalid.from, kind: invalid.kind.clone(), words: invalid.words.clone() },
    ] {
        assert_eq!(bytes(&line(activity), &bounds()), None);
    }
}
#[test]
fn owner_activity_every_scalar_message_and_all_result_metadata_price_without_fabricating_text() {
    let scalar: Box<[OwnerMessageKind]> = Box::new([
        OwnerMessageKind::SettingsDecision {
            proposal: u64::MAX,
            outcome: jig_client_domain::contract::SettingsOutcome::Accepted { revision: u32::MAX },
        },
        OwnerMessageKind::SettingsDecision {
            proposal: u64::MAX,
            outcome: jig_client_domain::contract::SettingsOutcome::Rejected,
        },
        OwnerMessageKind::SettingsDecision {
            proposal: u64::MAX,
            outcome: jig_client_domain::contract::SettingsOutcome::Stale { newest: u32::MAX },
        },
        OwnerMessageKind::Escalation { task: u64::MAX, revision: u64::MAX },
        OwnerMessageKind::Proposal { proposer: u64::MAX, proposal: u64::MAX, kind: OwnerMessageProposalKind::Release },
        OwnerMessageKind::PersonStalled { task: u64::MAX },
        OwnerMessageKind::ProposalDecision { proposal: u64::MAX, accepted: false },
        OwnerMessageKind::Words,
        OwnerMessageKind::EffectNotMade { entry: u64::MAX },
        OwnerMessageKind::Amendment { revision: u64::MAX },
        OwnerMessageKind::Question,
        OwnerMessageKind::Answer { question: u64::MAX },
        OwnerMessageKind::Notice { subscription: u64::MAX, target: u64::MAX, state: OwnerNoticeState::Cancelled },
        OwnerMessageKind::Timer { subscription: u64::MAX },
        OwnerMessageKind::News { subscription: u64::MAX, class: OwnerNewsClass::Dropped },
        OwnerMessageKind::Result(OwnerWordResult::Report),
    ]);
    assert_eq!(scalar.len(), 16);
    for kind in scalar {
        let value = word(kind);
        assert_eq!(word_bytes(&value, &bounds()), Some(u64::try_from(size_of::<OwnerWord>()).unwrap() + 2));
    }
    for result in [
        OwnerWordResult::Report,
        OwnerWordResult::Change { connector: u16::MAX, kind: u16::MAX, resource: u64::MAX },
        OwnerWordResult::Failed,
        OwnerWordResult::Cancelled,
    ] {
        let value = word(OwnerMessageKind::Result(result));
        assert_eq!(word_bytes(&value, &bounds()), Some(u64::try_from(size_of::<OwnerWord>()).unwrap() + 2));
    }
    let tries = OwnerFailureTries {
        transient: u32::MAX,
        permanent: u32::MAX,
        run: u32::MAX,
        agent: u32::MAX,
        lost: u32::MAX,
        invalid: u32::MAX,
    };
    for activity in [
        OwnerActivity::RunBegan { attempt: u64::MAX, why: OwnerRunWhy::Retry { tries }, resumed: false },
        OwnerActivity::RunEnded {
            attempt: u64::MAX,
            end: OwnerRunEnd::Failed { class: HoldFailure::Permanent },
            spent: u64::MAX,
        },
        OwnerActivity::Turn { reply: u64::MAX, attempt: u64::MAX, turn: u32::MAX, read: None },
        OwnerActivity::Turn { reply: 0, attempt: 0, turn: 0, read: Some(0) },
        OwnerActivity::Decided { revision: u64::MAX },
        OwnerActivity::Milestone { position: u64::MAX },
    ] {
        let value = line(activity);
        let exact = u64::try_from(size_of::<OwnerActivityLine>()).unwrap();
        assert_eq!(bytes(&value, &bounds()), Some(exact));
        assert!(fits(&value, &Bounds { document_bytes: exact, ..bounds() }));
        assert!(!fits(&value, &Bounds { document_bytes: exact - 1, ..bounds() }));
    }
}
#[test]
fn owner_activity_reserved_empty_fields_and_each_nested_text_overflow_refuse_whole_without_mutation() {
    for capacity in [0_u32, 1, 3] {
        let value =
            word(OwnerMessageKind::Result(OwnerWordResult::Verdict { code: 0, fields: List::with_capacity(capacity) }));
        let expected =
            u64::try_from(size_of::<OwnerWord>()).unwrap() + List::<ResultField>::worst_case(capacity).unwrap() + 2;
        assert_eq!(word_bytes(&value, &bounds()), Some(expected));
        assert!(word_fits(&value, &Bounds { document_bytes: expected, ..bounds() }));
        assert!(!word_fits(&value, &Bounds { document_bytes: expected - 1, ..bounds() }));
    }
    let excess = word(OwnerMessageKind::Result(OwnerWordResult::Verdict { code: 0, fields: List::with_capacity(4) }));
    let saved = excess.clone();
    assert_eq!(word_bytes(&excess, &bounds()), None);
    assert_eq!(excess, saved);
    assert!(word_bytes(&excess, &Bounds { rows: 4, ..bounds() }).is_some());
    let mutations: [fn(&mut OwnerWord); 4] = [
        |s| s.words = text(b"123456789"),
        |s| {
            s.kind =
                OwnerMessageKind::Result(OwnerWordResult::Approved { state: text(b"123456789"), words_present: true });
        },
        |s| {
            if let OwnerMessageKind::Result(OwnerWordResult::Verdict { fields, .. }) = &mut s.kind {
                fields.get_mut(0).unwrap().name = copy_of(b"123456789");
            }
        },
        |s| {
            if let OwnerMessageKind::Result(OwnerWordResult::Verdict { fields, .. }) = &mut s.kind {
                fields.get_mut(0).unwrap().value = copy_of(b"123456789");
            }
        },
    ];
    for mutate in mutations {
        let mut value = word(OwnerMessageKind::Result(OwnerWordResult::Verdict { code: 7, fields: fields() }));
        mutate(&mut value);
        let saved = value.clone();
        assert_eq!(word_bytes(&value, &bounds()), None);
        assert!(!word_fits(&value, &bounds()));
        assert_eq!(value, saved);
        assert!(word_bytes(&value, &Bounds { text: 9, ..bounds() }).is_some());
        assert_eq!(bytes(&line(OwnerActivity::Heard(value.clone())), &bounds()), None);
        assert_eq!(value, saved);
    }
}
#[test]
fn owner_activity_full_fields_and_approved_state_reach_the_declared_whole_worst_case() {
    for rows in [0_u32, 2] {
        let bounds = Bounds { rows, text: 3, ..bounds() };
        let kind = if rows == 0 {
            OwnerMessageKind::Result(OwnerWordResult::Approved { state: text(b"xxx"), words_present: true })
        } else {
            let mut fields = List::with_capacity(rows);
            for _ in 0..rows {
                fields.push(ResultField { name: copy_of(b"xxx"), value: copy_of(b"xxx") }).unwrap();
            }
            OwnerMessageKind::Result(OwnerWordResult::Verdict { code: u32::MAX, fields })
        };
        let value = OwnerWord { words: text(b"xxx"), ..word(kind) };
        let maximum = word_worst_case(&bounds).unwrap();
        assert_eq!(word_bytes(&value, &bounds), Some(maximum));
        assert!(word_fits(&value, &Bounds { document_bytes: maximum, ..bounds }));
        assert!(!word_fits(&value, &Bounds { document_bytes: maximum - 1, ..bounds }));
        let value = line(OwnerActivity::Heard(value));
        let maximum = worst_case(&bounds).unwrap();
        assert_eq!(bytes(&value, &bounds), Some(maximum));
        assert!(fits(&value, &Bounds { document_bytes: maximum, ..bounds }));
        assert!(!fits(&value, &Bounds { document_bytes: maximum - 1, ..bounds }));
    }
    assert_eq!(worst_case(&Bounds { rows: u32::MAX, text: u32::MAX, ..bounds() }), None);
    assert_eq!(word_worst_case(&Bounds { rows: u32::MAX, text: u32::MAX, ..bounds() }), None);
}

const UTF8_TEXTS: [&[u8]; 2] = [b"~\0", b"!\0"];
