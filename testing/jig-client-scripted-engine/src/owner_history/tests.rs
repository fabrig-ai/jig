use super::*;
use crate::Bounds;
use jig_client_domain::contract::*;
use skein_lib::{Duration, bytes::copy_of};
fn doc(bytes: &[u8], cut: bool) -> DocumentText {
    DocumentText { bytes: Text::new(copy_of(bytes), 8).unwrap(), cut }
}
fn pattern(last: OwnerPatternEnd, capacity: u32) -> OwnerPattern {
    let mut segments = List::with_capacity(capacity);
    if capacity != 0 {
        for bytes in [b"a/b".as_slice(), &[255, 0], b""] {
            segments.push(copy_of(bytes)).unwrap();
        }
    }
    OwnerPattern { segments, last }
}
fn state() -> OwnerAmendmentState {
    let mut parameters = List::with_capacity(3);
    parameters.push(ParameterDocument::Number { name: u32::MAX, value: u64::MAX }).unwrap();
    parameters.push(ParameterDocument::Bytes { name: 1, value: doc(b"bytes", false) }).unwrap();
    parameters.push(ParameterDocument::Resource { name: 2, connector: u16::MAX, resource: u64::MAX }).unwrap();
    let mut inputs = List::with_capacity(3);
    inputs.push(9).unwrap();
    inputs.push(u64::MAX).unwrap();
    let mut grants = List::with_capacity(3);
    grants
        .push(OwnerGrant {
            connector: u16::MAX,
            kind: u16::MAX,
            pattern: pattern(OwnerPatternEnd::Exact(copy_of(b"tail")), 3),
        })
        .unwrap();
    let mut kinds = List::with_capacity(3);
    for kind in [
        OwnerAuthorityExecutor::Charter(u32::MAX),
        OwnerAuthorityExecutor::Procedure(u32::MAX),
        OwnerAuthorityExecutor::Role(u32::MAX),
    ] {
        kinds.push(kind).unwrap();
    }
    let mut resources = List::with_capacity(3);
    resources
        .push(OwnerResourceScope { connector: 7, pattern: pattern(OwnerPatternEnd::Open(copy_of(b"*")), 0) })
        .unwrap();
    let mut dependencies = List::with_capacity(3);
    dependencies.push(u64::MAX).unwrap();
    dependencies.push(7).unwrap();
    OwnerAmendmentState {
        addressed: Some(PersonAddress::Party(u64::MAX)),
        taken_by: Some(u64::MAX),
        instructions: Some(doc(b"stored", false)),
        spec: SpecificationDocument {
            specification: DocumentSpecification { asks: false, words: doc(b"spec", false), parameters, inputs },
            cut: false,
        },
        authority: OwnerAuthorityDocument {
            authority: OwnerAuthority {
                models: None,
                tools: u64::MAX,
                grants,
                delegation: OwnerDelegation { kinds, tasks: u32::MAX, depth: u32::MAX },
                spend: u64::MAX,
                deadline: Some(skein_lib::Wall::from_nanos(u64::MAX)),
                notes: u8::MAX,
                note_resources: resources,
            },
            cut: false,
        },
        wake: OwnerWakePolicy {
            words: WakeRule::Never,
            notices: WakeRule::Immediate,
            news: WakeRule::Batch { count: u32::MAX, age: Duration::from_nanos(u64::MAX) },
            results: ResultsWake::LastOrFailure,
            questions: true,
            answers: false,
            timers: true,
        },
        dependencies,
        cut: false,
    }
}
fn delta() -> OwnerRetainedAmendment {
    let source = state();
    OwnerRetainedAmendment {
        addressed: Some(AddressChange { revision: Revision { value: 12 }, address: PersonAddress::Party(u64::MAX) }),
        instructions: Some(doc(b"", false)),
        release: Some(4),
        spec: Some(source.spec),
        authority: Some(source.authority),
        wake: Some(source.wake),
        dependencies: Some(source.dependencies),
        reason: doc(b"delta", false),
        cut: false,
    }
}
fn part() -> OwnerPart {
    let mut segments = List::with_capacity(3);
    for bytes in [b"a/b".as_slice(), &[255, 0], b""] {
        segments.push(copy_of(bytes)).unwrap();
    }
    OwnerPart { connector: u16::MAX, kind: OwnerPartKind::Effect, key: OwnerPartKey::Path(segments) }
}
fn task() -> ProposedTask {
    let source = state();
    let mut resources = List::with_capacity(3);
    resources.push(part()).unwrap();
    ProposedTask {
        deadline: jig_client_domain::contract::NewDeadline::Named,
        number: u64::MAX,
        spec: source.spec,
        executor: OwnerExecutor::Procedure { connector: u16::MAX, code: u32::MAX },
        contract: TaskContractDocument::Approval { state: copy_of(&[255; 8]), part: part() },
        authority: source.authority,
        budget: u64::MAX,
        funder: Funder::Pool { project: u32::MAX, person: u64::MAX, period: u64::MAX },
        dependencies: source.dependencies,
        resources,
        wake: source.wake,
        priority: Some(u32::MAX),
        cut: false,
    }
}
fn actions() -> Vec<OwnerRetainedAction> {
    let mut batch = List::with_capacity(3);
    for _ in 0..3 {
        batch.push(task()).unwrap();
    }
    vec![
        OwnerRetainedAction::Batch(batch),
        OwnerRetainedAction::Effect {
            part: part(),
            authority: state().authority,
            attempt: u64::MAX,
            completion: u32::MAX,
            position: u32::MAX,
            purpose: u64::MAX,
        },
        OwnerRetainedAction::ProcedureEffect {
            part: part(),
            authority: state().authority,
            step: u64::MAX,
            purpose: u64::MAX,
        },
        OwnerRetainedAction::Amend { task: u64::MAX, change: Box::new(delta()) },
        OwnerRetainedAction::Widen { task: u64::MAX, authority: state().authority },
        OwnerRetainedAction::Release { task: u64::MAX },
        OwnerRetainedAction::Settings(OwnerRetainedActionSettings {
            against: 6,
            items: List::with_capacity(3),
            made: Some(7),
        }),
    ]
}
fn proposal(action: OwnerRetainedAction) -> OwnerProposalHistory {
    OwnerProposalHistory {
        action,
        number: u64::MAX,
        proposer: u64::MAX,
        as_holder: true,
        state: OwnerProposalHistoryState::Pending {
            holder: OwnerHistoryHolder::Task(u64::MAX),
            since: skein_lib::Wall::from_nanos(7),
        },
    }
}
fn line() -> OwnerHistoryLine {
    OwnerHistoryLine {
        task: u64::MAX,
        revision: 6,
        by: OwnerRequester::Person(u64::MAX),
        change: OwnerHistoryChange::Amended,
        reason: doc(b"reason", false),
        proposal: None,
        amendment: None,
        at: None,
    }
}
fn bounds() -> Bounds {
    Bounds { text: 8, rows: 3, segments: 3, document_bytes: u64::MAX, ..Bounds::default() }
}
#[test]
fn owner_history_whole_bytes_charge_boxes_reserved_lists_and_nested_opaque_allocations_at_plus_minus_one() {
    for action in actions() {
        let value = OwnerHistoryLine {
            proposal: Some(proposal(action)),
            amendment: Some((Box::new(state()), Box::new(state()))),
            ..line()
        };
        let source = value.clone();
        let expected = u64::try_from(size_of::<OwnerHistoryLine>()).unwrap()
            + 6
            + expected_action(&value.proposal.as_ref().unwrap().action)
            + crate::owner_amendment::bytes(&state(), &bounds()).unwrap() * 2;
        assert_eq!(bytes(&value, &bounds()), Some(expected));
        for (limit, admitted) in [(expected - 1, false), (expected, true), (expected + 1, true)] {
            assert_eq!(fits(&value, &Bounds { document_bytes: limit, ..bounds() }), admitted);
            assert_eq!(value, source, "whole admission never shortens source facts");
        }
        assert!(expected <= worst_case(&bounds()).unwrap());
    }
    assert_eq!(worst_case(&Bounds { text: u32::MAX, rows: u32::MAX, ..bounds() }), None);
}
#[test]
fn owner_history_reserved_empty_batch_and_optional_delta_fields_keep_actual_presence_and_capacity() {
    let none = OwnerRetainedAmendment {
        addressed: None,
        instructions: None,
        release: None,
        spec: None,
        authority: None,
        wake: None,
        dependencies: None,
        reason: doc(b"", false),
        cut: false,
    };
    let empty = OwnerRetainedAmendment {
        instructions: Some(doc(b"", false)),
        dependencies: Some(List::with_capacity(3)),
        ..none.clone()
    };
    assert_ne!(none, empty);
    assert_eq!(empty.heap(&bounds()).unwrap() - none.heap(&bounds()).unwrap(), List::<u64>::worst_case(3).unwrap());
    let a = OwnerHistoryLine { proposal: Some(proposal(OwnerRetainedAction::Batch(List::with_capacity(0)))), ..line() };
    let b = OwnerHistoryLine { proposal: Some(proposal(OwnerRetainedAction::Batch(List::with_capacity(3)))), ..line() };
    assert_eq!(
        bytes(&b, &bounds()).unwrap() - bytes(&a, &bounds()).unwrap(),
        List::<ProposedTask>::worst_case(3).unwrap()
    );
    assert!(!fits(&b, &Bounds { rows: 2, segments: 2, ..bounds() }));
    assert_eq!(b.proposal.as_ref().unwrap().action, OwnerRetainedAction::Batch(List::with_capacity(3)));
}
#[test]
fn owner_history_whole_batch_and_every_nested_text_or_row_boundary_refuses_unchanged_source() {
    let source = OwnerHistoryLine {
        proposal: Some(proposal(actions().remove(0))),
        amendment: Some((Box::new(state()), Box::new(state()))),
        ..line()
    };
    assert!(fits(&source, &bounds()));
    for bound in [Bounds { text: 7, ..bounds() }, Bounds { rows: 2, segments: 2, ..bounds() }] {
        let original = source.clone();
        assert!(!fits(&source, &bound));
        assert_eq!(source, original);
    }
    let mut value = source.clone();
    if let OwnerRetainedAction::Batch(batch) = &mut value.proposal.as_mut().unwrap().action {
        let mut choices = List::with_capacity(3);
        let mut fields = List::with_capacity(3);
        fields.push(FieldRule { name: copy_of(&[255; 8]), max: u32::MAX }).unwrap();
        choices.push(VerdictChoice { code: u32::MAX, words: u32::MAX, followups: u32::MAX, fields }).unwrap();
        batch.get_mut(0).unwrap().contract = TaskContractDocument::Verdict { choices, cut: true };
    }
    assert!(fits(&value, &bounds()));
    assert!(!fits(&value, &Bounds { text: 7, ..bounds() }));
    assert!(bytes(&value, &bounds()).unwrap() <= worst_case(&bounds()).unwrap());
}
#[test]
fn owner_history_snapshot_optional_overrides_and_independent_retained_losses_are_not_live_defaults() {
    let variants =
        [None, Some(doc(b"", false)), Some(doc(b"stored", false)), Some(doc(b"", true)), Some(doc(b"stored", true))];
    let mut values = Vec::new();
    for instructions in variants {
        let before = OwnerAmendmentState { instructions, ..state() };
        let value = OwnerHistoryLine { amendment: Some((Box::new(before), Box::new(state()))), ..line() };
        assert!(fits(&value, &bounds()));
        for previous in &values {
            assert_ne!(&value, previous);
        }
        values.push(value);
    }
    let mut changed = delta();
    changed.cut = true;
    changed.reason.cut = true;
    changed.spec.as_mut().unwrap().cut = true;
    changed.authority.as_mut().unwrap().cut = true;
    assert_eq!(changed.addressed.unwrap().revision.value, 12);
    assert_eq!(changed.release, Some(4));
    assert_eq!(line().revision, 6);
    assert_eq!(changed.dependencies.as_ref().unwrap().as_slice(), &[u64::MAX, 7]);
    assert_eq!(changed.dependencies.as_ref().unwrap().capacity(), 3);
    assert!(
        changed.reason.cut
            && changed.cut
            && changed.spec.as_ref().unwrap().cut
            && changed.authority.as_ref().unwrap().cut
    );
    assert_eq!(values[0].amendment.as_ref().unwrap().0.instructions, None);
    assert_eq!(values[1].amendment.as_ref().unwrap().0.instructions.as_ref().unwrap().bytes(), b"");
}

fn expected_action(action: &OwnerRetainedAction) -> u64 {
    let authority = List::<OwnerGrant>::worst_case(3).unwrap()
        + List::<OwnerResourceScope>::worst_case(3).unwrap()
        + List::<OwnerAuthorityExecutor>::worst_case(3).unwrap()
        + List::<Box<[u8]>>::worst_case(3).unwrap()
        + 3
        + 2
        + 4
        + 1;
    let spec = List::<ParameterDocument>::worst_case(3).unwrap() + List::<u64>::worst_case(3).unwrap() + 4 + 5;
    let path = List::<Box<[u8]>>::worst_case(3).unwrap() + 3 + 2;
    match action {
        OwnerRetainedAction::Settings(_) => List::<RevisionItemHead>::worst_case(3).unwrap(),
        OwnerRetainedAction::Batch(_) => {
            List::<ProposedTask>::worst_case(3).unwrap()
                + 3 * (spec
                    + authority
                    + 8
                    + path
                    + List::<u64>::worst_case(3).unwrap()
                    + List::<OwnerPart>::worst_case(3).unwrap()
                    + path)
        }
        OwnerRetainedAction::Effect { .. } | OwnerRetainedAction::ProcedureEffect { .. } => path + authority,
        OwnerRetainedAction::Amend { .. } => {
            u64::try_from(size_of::<OwnerRetainedAmendment>()).unwrap()
                + spec
                + authority
                + List::<u64>::worst_case(3).unwrap()
                + 5
        }
        OwnerRetainedAction::Widen { .. } => authority,
        OwnerRetainedAction::Release { .. } => 0,
    }
}
