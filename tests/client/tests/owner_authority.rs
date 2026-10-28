use jig_client_domain::contract::{
    AuthorityModels, OwnerAuthority, OwnerAuthorityDocument, OwnerAuthorityExecutor, OwnerDelegation, OwnerGrant,
    OwnerPattern, OwnerPatternEnd, OwnerResourceScope,
};
use jig_client_scripted_engine::{Bounds, owner_authority as price};
use skein_lib::{List, Wall};
use skein_world::domain::heap::{Counting, Meter};
#[global_allocator]
static HEAP: Counting = Counting;
fn list<T: core::fmt::Debug>(items: impl IntoIterator<Item = T>) -> List<T> {
    let items: Vec<T> = items.into_iter().collect();
    let mut result = List::with_capacity(u32::try_from(items.len()).expect("fixture size"));
    for item in items {
        result.push(item).expect("fixture capacity");
    }
    result
}
fn pattern(open: bool) -> OwnerPattern {
    OwnerPattern {
        segments: list([Box::from(*b"a/b\0"), Box::from([255, 0, 128, 47]), Box::from(*b"last")]),
        last: if open {
            OwnerPatternEnd::Open(Box::from(*b"/end"))
        } else {
            OwnerPatternEnd::Exact(Box::from(*b"/end"))
        },
    }
}
fn authority() -> OwnerAuthority {
    OwnerAuthority {
        models: Some(AuthorityModels::Named(list((0..64).map(|n| Box::from([b'm', b'o', b'd', n]))))),
        tools: u64::MAX,
        grants: list((0..3).map(|n| OwnerGrant { connector: u16::MAX, kind: n, pattern: pattern(n % 2 == 0) })),
        delegation: OwnerDelegation {
            kinds: list([
                OwnerAuthorityExecutor::Charter(u32::MAX),
                OwnerAuthorityExecutor::Procedure(u32::MAX),
                OwnerAuthorityExecutor::Role(u32::MAX),
            ]),
            tasks: u32::MAX,
            depth: u32::MAX,
        },
        spend: u64::MAX,
        deadline: Some(Wall::from_nanos(u64::MAX)),
        notes: u8::MAX,
        note_resources: list((0..3).map(|n| OwnerResourceScope { connector: n, pattern: pattern(n % 2 != 0) })),
    }
}
fn bounds() -> Bounds {
    Bounds { rows: 3, authority: 3, segments: 3, text: 4, document_bytes: u64::MAX, ..Bounds::default() }
}
#[test]
fn saturated_authority_prices_every_capacity_and_byte_exactly() {
    let original = OwnerAuthorityDocument { authority: authority(), cut: true };
    let meter = Meter::new();
    let value = original.clone();
    let held = meter.held();
    let total =
        held.checked_add(u64::try_from(core::mem::size_of::<OwnerAuthorityDocument>()).expect("size")).expect("sum");
    assert_eq!(price::document_bytes(&value, &bounds()), Some(total));
    assert!(total <= price::worst_case(&bounds()).expect("valid bounds"), "{total}");
    assert_eq!(meter.held(), held, "pricing borrows without allocation");
    let mut exact = bounds();
    exact.document_bytes = total;
    assert!(price::document_fits(&value, &exact));
    exact.document_bytes -= 1;
    assert!(!price::document_fits(&value, &exact));
    let payload = price::bytes(&value.authority, &bounds()).expect("whole payload price");
    exact.document_bytes = payload;
    assert!(price::fits(&value.authority, &exact));
    exact.document_bytes -= 1;
    assert!(!price::fits(&value.authority, &exact));
    assert_eq!(value, original, "no refusal shortens or rewrites the original");
}
#[test]
fn nested_bounds_check_reserved_empty_lists_terminal_bytes_and_overflow() {
    let mut value = authority();
    value.grants = List::with_capacity(4);
    assert_eq!(value.grants.len(), 0);
    assert_eq!(price::bytes(&value, &bounds()), None, "empty reserved capacity still owned");
    value = authority();
    value.grants.get_mut(0).expect("grant").pattern.segments = List::with_capacity(4);
    assert_eq!(price::bytes(&value, &bounds()), None, "nested capacity checked");
    for end in [OwnerPatternEnd::Exact(Box::from(*b"large")), OwnerPatternEnd::Open(Box::from(*b"large"))] {
        value = authority();
        value.grants.get_mut(0).expect("grant").pattern.last = end;
        assert_eq!(price::bytes(&value, &bounds()), None, "both terminal variants bounded");
    }
    value = authority();
    value
        .note_resources
        .get_mut(0)
        .expect("scope")
        .pattern
        .segments
        .get_mut(0)
        .expect("segment")
        .clone_from(&Box::from(*b"large"));
    assert_eq!(price::bytes(&value, &bounds()), None, "resource scope bytes bounded");
    assert_eq!(price::worst_case(&Bounds { rows: u32::MAX, authority: u32::MAX, text: u32::MAX, ..bounds() }), None);
}
#[test]
fn authority_document_cut_is_separate_from_whole_payload_and_keeps_exact_codes() {
    let payload = authority();
    let cut = OwnerAuthorityDocument { authority: payload.clone(), cut: true };
    let complete = OwnerAuthorityDocument { authority: payload.clone(), cut: false };
    assert_ne!(cut, complete);
    assert_eq!(cut.authority, complete.authority);
    assert_eq!(payload.tools, u64::MAX);
    assert_eq!(payload.notes, u8::MAX);
    assert_eq!(*payload.delegation.kinds.get(2).expect("executor"), OwnerAuthorityExecutor::Role(u32::MAX));
    assert_eq!(payload.grants.get(1).expect("grant").pattern.last, OwnerPatternEnd::Exact(Box::from(*b"/end")));
    assert_eq!(payload.grants.get(0).expect("grant").pattern.segments.get(0).expect("segment").as_ref(), b"a/b\0");
    assert_eq!(
        payload.grants.get(0).expect("grant").pattern.segments.get(1).expect("segment").as_ref(),
        &[255, 0, 128, 47]
    );
}

#[test]
fn saturated_owner_authority_renderer_fits_its_fragment_envelope() {
    use jig_client_view::owner_authority as view;
    use jig_client_view::owner_authority_words::{self as words, OwnerAuthorityWords};
    use skein_ui::Builder;
    use skein_ui::Limits;
    use skein_ui::phrases::PhraseLimits;

    let language_limits =
        PhraseLimits { languages: 1, phrases: 23, holes: 1, segments: 40, text_bytes: 4096, hole_bytes: 0 };
    let words = OwnerAuthorityWords::load(Box::new([words::english()]), &language_limits, 0, 0)
        .expect("closed English authority words");

    let value = OwnerAuthorityDocument { authority: authority(), cut: true };
    let envelope = view::worst_case(3, 3, 4, &words).expect("full view envelope");
    let required = view::document_required(&value, &words, 3).expect("value capacity");
    assert_eq!(required.nodes, 63, "full three-by-three patterns and executor rows");
    assert_eq!(required.nodes, envelope.nodes);
    assert!(required.text_bytes <= envelope.text_bytes);
    let limits = Limits {
        nodes: envelope.nodes,
        depth: envelope.depth,
        text_per_node: envelope.text_per_node,
        text_bytes: envelope.text_bytes,
        drawing_bytes: 0,
        styles_per_node: 0,
        patches: 0,
        bindings: 0,
        slots: 0,
        markdown_depth: 0,
        scene_marks: 0,
        scene_bytes: 0,
    };

    let mut builder = Builder::new(&limits);
    view::document(&mut builder, &value, &words, 4, 3);
    let tree = builder.finish();
    assert_eq!(tree.len(), 63);

    assert_eq!(
        tree.nodes().iter().flat_map(|n| &n.text).filter(|r| r.text.as_ref() == b"a/b\0").count(),
        6,
        "all six literal pattern segment boundaries retained"
    );
    let empty = OwnerAuthorityDocument {
        authority: OwnerAuthority {
            models: None,
            grants: List::with_capacity(0),
            delegation: OwnerDelegation { kinds: List::with_capacity(0), tasks: 0, depth: 0 },
            note_resources: List::with_capacity(0),
            tools: 0,
            spend: 0,
            notes: 0,
            deadline: None,
        },
        cut: false,
    };
    assert_eq!(view::document_required(&empty, &words, 3).expect("empty facts").nodes, 10);
    assert!(price::document_fits(&empty, &Bounds { rows: 0, authority: 0, text: 0, ..bounds() }));
}

#[test]
fn a_whole_authority_is_admitted_independently_of_document_rows() {
    const DISPLAY_ROWS: u32 = 2;
    let value = authority();
    let cap = Bounds { rows: DISPLAY_ROWS, ..bounds() };
    assert!(price::fits(&value, &cap));
    assert_eq!(
        price::worst_case(&cap),
        price::document_bytes(&OwnerAuthorityDocument { authority: value, cut: false }, &cap)
    );
}
