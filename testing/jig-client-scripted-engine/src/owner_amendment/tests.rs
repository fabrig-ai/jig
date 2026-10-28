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
fn fixture() -> OwnerAmendmentState {
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
        instructions: Some(doc(b"live", false)),
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
fn bounds() -> Bounds {
    Bounds { text: 8, rows: 3, authority: 3, segments: 3, document_bytes: u64::MAX, ..Bounds::default() }
}
#[test]
fn owner_amendment_whole_admission_counts_inline_nested_lists_and_every_opaque_byte_at_plus_minus_one() {
    let value = fixture();
    let original = value.clone();
    let expected = u64::try_from(size_of::<OwnerAmendmentState>()).unwrap()
        + List::<ParameterDocument>::worst_case(3).unwrap()
        + 2 * List::<u64>::worst_case(3).unwrap()
        + List::<OwnerGrant>::worst_case(3).unwrap()
        + List::<OwnerResourceScope>::worst_case(3).unwrap()
        + List::<OwnerAuthorityExecutor>::worst_case(3).unwrap()
        + List::<Box<[u8]>>::worst_case(3).unwrap()
        + List::<Box<[u8]>>::worst_case(0).unwrap()
        + 4
        + 4
        + 5
        + 3
        + 2
        + 4
        + 1;
    assert_eq!(bytes(&value, &bounds()), Some(expected));
    for (limit, admitted) in [(expected - 1, false), (expected, true), (expected + 1, true)] {
        assert_eq!(fits(&value, &Bounds { document_bytes: limit, ..bounds() }), admitted);
        assert_eq!(value, original, "admission never trims the source");
    }
    assert!(expected <= worst_case(&bounds()).unwrap());
    assert_eq!(worst_case(&Bounds { rows: u32::MAX, text: u32::MAX, ..bounds() }), None);
}
#[test]
fn owner_amendment_optional_instruction_contexts_and_losses_never_collapse_or_default() {
    let variants =
        [None, Some(doc(b"", false)), Some(doc(b"live", false)), Some(doc(b"", true)), Some(doc(b"live", true))];
    let mut states = Vec::new();
    for instructions in variants {
        let state = OwnerAmendmentState { instructions, ..fixture() };
        assert_eq!(state.clone(), state);
        for previous in &states {
            assert_ne!(&state, previous, "None, known empty and each loss remain distinct");
        }
        let size = bytes(&state, &bounds()).unwrap();
        assert!(fits(&state, &Bounds { document_bytes: size, ..bounds() }));
        assert!(!fits(&state, &Bounds { document_bytes: size - 1, ..bounds() }));
        states.push(state);
    }
    assert_eq!(
        bytes(&states[0], &bounds()),
        bytes(&states[1], &bounds()),
        "the inline Option already prices complete empty"
    );
    assert_eq!(
        bytes(&states[0], &bounds()),
        bytes(&states[3], &bounds()),
        "cut metadata is inline, never text fabrication"
    );
    assert_eq!(states[0].spec, states[2].spec);
    assert_eq!(states[0].instructions, None, "retained no-own-override does not become current effective text");
    assert_eq!(states[1].instructions.as_ref().unwrap().whole().unwrap().bytes(), b"");
    assert_eq!(states[3].instructions.as_ref().unwrap().whole(), None);
    assert_eq!(states.len(), 5, "a batch retains every state even with identical spec");
}
#[test]
fn owner_amendment_empty_reserved_capacities_and_each_nested_text_refuse_without_trimming() {
    let mutations: [fn(&mut OwnerAmendmentState); 8] = [
        |s| s.dependencies = List::with_capacity(4),
        |s| s.spec.specification.parameters = List::with_capacity(4),
        |s| s.spec.specification.inputs = List::with_capacity(4),
        |s| s.authority.authority.grants = List::with_capacity(4),
        |s| s.authority.authority.delegation.kinds = List::with_capacity(4),
        |s| s.authority.authority.note_resources = List::with_capacity(4),
        |s| s.authority.authority.grants.get_mut(0).unwrap().pattern.segments = List::with_capacity(4),
        |s| s.authority.authority.note_resources.get_mut(0).unwrap().pattern.segments = List::with_capacity(4),
    ];
    for mutate in mutations {
        let mut value = fixture();
        mutate(&mut value);
        let original = value.clone();
        assert_eq!(bytes(&value, &bounds()), None);
        assert!(!fits(&value, &bounds()));
        assert_eq!(value, original);
        let larger = Bounds { rows: 4, authority: 4, segments: 4, ..bounds() };
        let size = bytes(&value, &larger).unwrap();
        assert!(fits(&value, &Bounds { document_bytes: size, ..larger }));
        assert!(!fits(&value, &Bounds { document_bytes: size - 1, ..larger }));
    }
    let mutations: [fn(&mut OwnerAmendmentState); 6] = [
        |s| s.instructions = Some(DocumentText::complete(Text::new(copy_of(b"123456789"), 9).unwrap())),
        |s| s.spec.specification.words = DocumentText::complete(Text::new(copy_of(b"123456789"), 9).unwrap()),
        |s| {
            *s.spec.specification.parameters.get_mut(1).unwrap() = ParameterDocument::Bytes {
                name: 1,
                value: DocumentText::complete(Text::new(copy_of(b"123456789"), 9).unwrap()),
            };
        },
        |s| {
            *s.authority.authority.grants.get_mut(0).unwrap().pattern.segments.get_mut(0).unwrap() =
                copy_of(b"123456789");
        },
        |s| {
            s.authority.authority.grants.get_mut(0).unwrap().pattern.last =
                OwnerPatternEnd::Exact(copy_of(b"123456789"));
        },
        |s| {
            s.authority.authority.note_resources.get_mut(0).unwrap().pattern.last =
                OwnerPatternEnd::Open(copy_of(b"123456789"));
        },
    ];
    for mutate in mutations {
        let mut value = fixture();
        mutate(&mut value);
        let original = value.clone();
        assert_eq!(bytes(&value, &bounds()), None);
        assert_eq!(value, original);
        assert!(fits(&value, &Bounds { text: 9, ..bounds() }));
    }
}
#[test]
fn owner_amendment_full_rows_and_text_reach_the_declared_worst_case_without_omission() {
    let bounds = Bounds { rows: 2, authority: 2, segments: 2, text: 3, ..bounds() };
    let mut value = fixture();
    let doc = doc(b"xxx", true);
    value.instructions = Some(doc.clone());
    value.spec.specification.words = doc.clone();
    value.spec.specification.parameters = List::with_capacity(2);
    value.spec.specification.inputs = List::with_capacity(2);
    value.dependencies = List::with_capacity(2);
    value.authority.authority.grants = List::with_capacity(2);
    value.authority.authority.delegation.kinds = List::with_capacity(2);
    value.authority.authority.note_resources = List::with_capacity(2);
    for n in 0..2 {
        let mut segments = List::with_capacity(2);
        for _ in 0..2 {
            segments.push(copy_of(b"xxx")).unwrap();
        }
        value.spec.specification.parameters.push(ParameterDocument::Bytes { name: n, value: doc.clone() }).unwrap();
        value.spec.specification.inputs.push(u64::MAX).unwrap();
        value.dependencies.push(u64::MAX).unwrap();
        value.authority.authority.delegation.kinds.push(OwnerAuthorityExecutor::Role(u32::MAX)).unwrap();
        value
            .authority
            .authority
            .grants
            .push(OwnerGrant {
                connector: u16::MAX,
                kind: u16::MAX,
                pattern: OwnerPattern { segments: segments.clone(), last: OwnerPatternEnd::Exact(copy_of(b"xxx")) },
            })
            .unwrap();
        value
            .authority
            .authority
            .note_resources
            .push(OwnerResourceScope {
                connector: u16::MAX,
                pattern: OwnerPattern { segments, last: OwnerPatternEnd::Open(copy_of(b"xxx")) },
            })
            .unwrap();
    }
    let mut names = List::with_capacity(AUTHORITY_MODELS);
    for at in 0..AUTHORITY_MODELS {
        names
            .push(vec![b'0' + u8::try_from(at).unwrap(); usize::try_from(bounds.text).unwrap()].into_boxed_slice())
            .unwrap();
    }
    value.authority.authority.models = Some(AuthorityModels::Named(names));
    let maximum = worst_case(&bounds).unwrap();
    assert_eq!(bytes(&value, &bounds), Some(maximum));
    assert!(fits(&value, &Bounds { document_bytes: maximum, ..bounds }));
    assert!(!fits(&value, &Bounds { document_bytes: maximum - 1, ..bounds }));
}
