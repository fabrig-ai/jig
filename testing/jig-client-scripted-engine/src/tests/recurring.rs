use super::{list, specification, task, text};
use crate::{Bounds, payload::fits};
use jig_client_domain::contract::*;
use skein_lib::Wall;

fn template(number: u64, words: &[u8]) -> ProposedTask {
    ProposedTask {
        deadline: jig_client_domain::contract::NewDeadline::Named,
        number,
        spec: specification(text(words)),
        executor: OwnerExecutor::Agent { charter: 1 },
        budget: 20,
        contract: TaskContractDocument::Report { words: 32 },
        authority: OwnerAuthorityDocument {
            authority: OwnerAuthority {
                models: None,
                tools: 0,
                grants: list(vec![]),
                delegation: OwnerDelegation { kinds: list(vec![]), tasks: 2, depth: 1 },
                spend: 20,
                deadline: None,
                notes: 0,
                note_resources: list(vec![]),
            },
            cut: false,
        },
        funder: Funder::Recurring { project: 1, task: 7, period: 9 },
        dependencies: list(vec![]),
        resources: list(vec![]),
        wake: OwnerWakePolicy {
            markers: 0,
            words: WakeRule::Immediate,
            notices: WakeRule::Never,
            news: WakeRule::Never,
            results: ResultsWake::Each,
            questions: false,
            answers: false,
            timers: false,
        },
        priority: None,
        cut: false,
    }
}

fn recurring() -> Recurring {
    let mut chip = task().chip;
    chip.title = DocumentText::complete(text(b"job"));
    chip.details = Box::new(ChipPhase::Starting);
    Recurring {
        chip,
        key: u32::MAX,
        batch: list(vec![template(1, b"first"), template(2, b"second")]),
        budget: u64::MAX,
        overlap: RecurringOverlap::Wait,
        current_period: u64::MAX,
        began: Some(Wall::from_nanos(u64::MAX)),
        next: None,
        last_period: u64::MAX,
        pending_period: Some(u64::MAX),
        last_batch: list(vec![u64::MAX, 9]),
        cut: true,
    }
}

#[test]
fn recurring_prices_the_whole_template_batch_and_final_members_owned_bytes() {
    let row = recurring();
    let bytes = u64::try_from(core::mem::size_of::<Recurring>()).unwrap()
        + u64::try_from(core::mem::size_of::<ChipPhase>()).unwrap()
        + skein_lib::List::<ProposedTask>::worst_case(2).unwrap()
        + skein_lib::List::<u64>::worst_case(2).unwrap()
        + 3
        + 5
        + 6;
    let mut bounds = Bounds { rows: 2, segments: 2, document_bytes: bytes, ..Bounds::default() };
    assert!(fits(&row, &bounds));
    bounds.document_bytes = bytes - 1;
    assert!(!fits(&row, &bounds));
    bounds.document_bytes = bytes;
    let mut longer = row;
    longer.batch.get_mut(1).unwrap().spec.specification.words.bytes = text(b"seconds");
    assert!(!fits(&longer, &bounds), "the final member is priced, even when the first still fits");
}

#[test]
fn recurring_checks_allocated_batch_last_batch_and_nested_member_capacities() {
    let bounds = Bounds { rows: 2, authority: 2, segments: 2, document_bytes: u64::MAX, ..Bounds::default() };
    assert!(fits(&recurring(), &bounds));
    let mut row = recurring();
    row.batch = skein_lib::List::with_capacity(3);
    assert!(!fits(&row, &bounds), "allocated template capacity is checked even when empty");
    let mut row = recurring();
    row.last_batch = skein_lib::List::with_capacity(3);
    assert!(!fits(&row, &bounds));
    let mut row = recurring();
    row.batch.get_mut(1).unwrap().dependencies = skein_lib::List::with_capacity(3);
    assert!(!fits(&row, &bounds));
    let mut row = recurring();
    row.batch.get_mut(1).unwrap().resources = skein_lib::List::with_capacity(3);
    assert!(!fits(&row, &bounds), "the final member's allocated resource capacity is checked");
    let mut row = recurring();
    row.batch.get_mut(1).unwrap().resources = list(vec![OwnerPart {
        connector: 9,
        kind: OwnerPartKind::Resource,
        key: OwnerPartKey::Path(skein_lib::List::with_capacity(3)),
    }]);
    assert!(!fits(&row, &bounds), "an empty nested path still owns its allocated capacity");
    let mut row = recurring();
    row.batch.get_mut(1).unwrap().authority.authority.grants = skein_lib::List::with_capacity(3);
    assert!(!fits(&row, &bounds));
    let mut row = recurring();
    row.batch.get_mut(1).unwrap().authority.authority.delegation.kinds = skein_lib::List::with_capacity(3);
    assert!(!fits(&row, &bounds));
    let mut row = recurring();
    row.batch.get_mut(1).unwrap().authority.authority.note_resources = list(vec![OwnerResourceScope {
        connector: 9,
        pattern: OwnerPattern {
            segments: skein_lib::List::with_capacity(3),
            last: OwnerPatternEnd::Exact(b"last".as_slice().into()),
        },
    }]);
    assert!(!fits(&row, &bounds), "the final member's nested authority pattern is checked");
    let mut row = recurring();
    row.batch.get_mut(1).unwrap().contract = TaskContractDocument::Verdict {
        choices: list(vec![VerdictChoice {
            code: 1,
            words: 2,
            followups: 3,
            fields: skein_lib::List::with_capacity(3),
        }]),
        cut: false,
    };
    assert!(!fits(&row, &bounds), "the final member's verdict field capacity is checked");
    let mut row = recurring();
    row.batch.get_mut(1).unwrap().contract = TaskContractDocument::Approval {
        state: vec![0; 4097].into_boxed_slice(),
        part: OwnerPart { connector: 9, kind: OwnerPartKind::Resource, key: OwnerPartKey::Number(7) },
    };
    assert!(!fits(&row, &bounds), "the final member's opaque approval bytes are admitted whole");
    let mut row = recurring();
    *row.chip.details = ChipPhase::Waiting { dependencies: skein_lib::List::with_capacity(3), hold: None };
    assert!(!fits(&row, &bounds));
    let mut row = recurring();
    row.batch.get_mut(1).unwrap().spec.specification.words.bytes =
        Text::new(vec![0; 4097].into_boxed_slice(), 5000).unwrap();
    assert!(!fits(&row, &bounds), "the final template member's text is checked");
    let mut row = recurring();
    row.batch.get_mut(1).unwrap().spec.specification.parameters = skein_lib::List::with_capacity(3);
    assert!(!fits(&row, &bounds), "the final member's parameter allocation is checked");
    let mut row = recurring();
    row.batch.get_mut(1).unwrap().spec.specification.inputs = skein_lib::List::with_capacity(3);
    assert!(!fits(&row, &bounds), "the final member's input allocation is checked");
    let mut row = recurring();
    row.batch.get_mut(1).unwrap().spec.specification.parameters = list(vec![ParameterDocument::Bytes {
        name: 7,
        value: DocumentText::complete(Text::new(vec![0; 4097].into_boxed_slice(), 5000).unwrap()),
    }]);
    assert!(!fits(&row, &bounds), "the final member's opaque parameter bytes are checked");
}

#[test]
fn exact_parts_and_contracts_price_every_segment_choice_and_opaque_state_byte() {
    let mut bounds = Bounds { rows: 2, segments: 2, document_bytes: u64::MAX, ..Bounds::default() };
    let reference = OwnerPart {
        connector: u16::MAX,
        kind: OwnerPartKind::Outbox,
        key: OwnerPartKey::Path(list(vec![b"a/b".as_slice().into(), b"c\xff".as_slice().into()])),
    };
    let bytes = u64::try_from(core::mem::size_of::<OwnerPart>()).unwrap()
        + skein_lib::List::<Box<[u8]>>::worst_case(2).unwrap()
        + 3
        + 2;
    bounds.document_bytes = bytes;
    assert!(fits(&reference, &bounds));
    bounds.document_bytes -= 1;
    assert!(!fits(&reference, &bounds));
    bounds.document_bytes = u64::MAX;
    let mut oversized = reference.clone();
    oversized.key = OwnerPartKey::Path(skein_lib::List::with_capacity(3));
    assert!(!fits(&oversized, &bounds));

    let approval = TaskContractDocument::Approval { state: b"opaque\xff".as_slice().into(), part: reference };
    let bytes = u64::try_from(core::mem::size_of::<TaskContractDocument>()).unwrap()
        + skein_lib::List::<Box<[u8]>>::worst_case(2).unwrap()
        + 3
        + 2
        + 7;
    bounds.document_bytes = bytes;
    assert!(fits(&approval, &bounds));
    bounds.document_bytes -= 1;
    assert!(!fits(&approval, &bounds));

    let choice = VerdictChoice {
        code: u32::MAX,
        words: u32::MAX,
        followups: u32::MAX,
        fields: list(vec![
            FieldRule { name: b"a/b".as_slice().into(), max: 11 },
            FieldRule { name: b"c\xff".as_slice().into(), max: 17 },
        ]),
    };
    let verdict = TaskContractDocument::Verdict { choices: list(vec![choice]), cut: true };
    let bytes = u64::try_from(core::mem::size_of::<TaskContractDocument>()).unwrap()
        + skein_lib::List::<VerdictChoice>::worst_case(1).unwrap()
        + skein_lib::List::<FieldRule>::worst_case(2).unwrap()
        + 3
        + 2;
    bounds.document_bytes = bytes;
    assert!(fits(&verdict, &bounds));
    bounds.document_bytes -= 1;
    assert!(!fits(&verdict, &bounds));
    bounds.document_bytes = u64::MAX;
    assert!(!fits(&TaskContractDocument::Verdict { choices: skein_lib::List::with_capacity(3), cut: false }, &bounds));
    let oversized = VerdictChoice { code: 9, words: 8, followups: 7, fields: skein_lib::List::with_capacity(3) };
    assert!(!fits(&TaskContractDocument::Verdict { choices: list(vec![oversized]), cut: false }, &bounds));
}
