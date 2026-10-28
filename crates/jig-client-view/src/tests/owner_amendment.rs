#![expect(
    clippy::disallowed_methods,
    reason = "ordinary proofs inspect public trees and selected checked language data"
)]
use crate::owner_amendment as view;
use crate::owner_amendment_words::{self as data, AmendmentPhrase as P};
use crate::words::Words;
use skein_ui::Limits;
use skein_ui::phrases::{Hole, Invalid, PhraseLimits, Phrasebook, Segment, SpanUnit, WallFormat};
use skein_ui::{Builder, Tree};

use jig_client::contract::*;
use skein_lib::{Duration, List, bytes::copy_of};
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
            specification: DocumentSpecification { words: doc(b"spec", false), parameters, inputs },
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
fn client_limits() -> crate::Limits {
    crate::Limits { rows: 3, segments: 3, text: 8, ..super::cards::CLIENT }
}
fn phrase_limits() -> PhraseLimits {
    PhraseLimits {
        languages: 2,
        phrases: crate::owner_words::phrase_count(),
        holes: 4,
        segments: super::english_phrase_segments(2),
        text_bytes: 100_000,
        hole_bytes: 8,
    }
}
fn normal_words() -> &'static Words {
    static WORDS: std::sync::OnceLock<Words> = std::sync::OnceLock::new();
    WORDS.get_or_init(|| words(0, false))
}
fn words(language: u16, selected: bool) -> Words {
    let mut english = crate::phrases::english::language();
    english.wall = WallFormat::Nanos;
    english.span_unit = SpanUnit::Nanos;
    let languages: Box<[skein_ui::phrases::Language]> = if selected {
        let mut translated = english.clone();
        for phrase in &mut translated.phrases {
            for form in &mut phrase.forms {
                for segment in &mut form.segments {
                    if let Segment::Text(bytes) = segment {
                        *bytes = copy_of(b"chosen language: ");
                    }
                }
            }
        }
        Box::new([english, translated])
    } else {
        Box::new([english])
    };
    Words::new(Phrasebook::load(crate::phrases::jig::shapes(8), languages, &phrase_limits()).unwrap(), language, 0)
}
fn build(value: &OwnerAmendmentState, words: &Words) -> Tree {
    let f = view::required(value, &client_limits(), words).unwrap();
    let limits = Limits {
        nodes: f.nodes,
        depth: f.depth,
        text_bytes: f.text_bytes,
        text_per_node: f.text_per_node,
        drawing_bytes: 0,
        styles_per_node: 2,
        patches: 0,
        bindings: f.bindings,
        slots: f.slots,
        markdown_depth: 2,
        scene_marks: 0,
        scene_bytes: 0,
    };

    let mut builder = Builder::new(&limits);
    view::render(&mut builder, value, words, client_limits().text, client_limits().rows);
    let tree = builder.finish();
    assert!(tree.len() <= f.nodes);
    assert!(tree.nodes().iter().all(|node| crate::tests::text_bytes(node) <= f.text_per_node));
    assert!(
        tree.nodes().iter().map(|node| u64::from(crate::tests::text_bytes(node))).sum::<u64>()
            <= u64::from(f.text_bytes)
    );
    assert!(tree.nodes().iter().all(|node| node.binding.is_none() && node.slot.is_none()));
    assert_eq!((f.bindings, f.slots), (0, 0));
    tree
}
fn has(tree: &Tree, bytes: &[u8]) -> bool {
    tree.nodes().iter().any(|node| node.text.iter().any(|run| run.text.as_ref() == bytes))
}
fn count(tree: &Tree, bytes: &[u8]) -> usize {
    tree.nodes().iter().flat_map(|node| &node.text).filter(|run| run.text.as_ref() == bytes).count()
}
#[test]
fn owner_amendment_instruction_absence_empty_and_each_loss_keep_context_without_default_substitution() {
    let variants =
        [None, Some(doc(b"", false)), Some(doc(b"live", false)), Some(doc(b"", true)), Some(doc(b"live", true))];
    let mut states = List::with_capacity(5);
    let mut trees: List<Tree> = List::with_capacity(5);
    for (index, instructions) in variants.into_iter().enumerate() {
        let state = OwnerAmendmentState { instructions, ..fixture() };
        let tree = build(&state, normal_words());
        assert_eq!(has(&tree, b"Instructions were not supplied"), index == 0);
        assert_eq!(has(&tree, b"Instructions are empty"), index == 1);
        assert_eq!(has(&tree, b"live"), index == 2 || index == 4);
        assert_eq!(has(&tree, b"Only part of this text was supplied"), index >= 3);
        for previous in &states {
            assert_ne!(&state, previous);
        }
        for previous in &trees {
            assert_ne!(tree.nodes(), previous.nodes(), "rendered None/empty/cut facts stay distinguishable");
        }
        states.push(state).unwrap();
        trees.push(tree).unwrap();
    }
    assert_eq!(
        states.get(0).unwrap().spec,
        states.get(2).unwrap().spec,
        "retained None and live effective share other facts"
    );
    assert_eq!(crate::instructions::whole(states.get(0).unwrap().instructions.as_ref()), None);
    assert_eq!(crate::instructions::whole(states.get(1).unwrap().instructions.as_ref()).unwrap().bytes(), b"");
    assert_eq!(crate::instructions::whole(states.get(3).unwrap().instructions.as_ref()), None);
    assert_eq!(states.len(), 5, "the consumer can retain every independent state");
}
#[test]
fn owner_amendment_each_text_spec_authority_and_dependency_loss_is_independent() {
    for flag in 0_u32..5 {
        let mut value = fixture();
        match flag {
            0 => value.spec.specification.words.cut = true,
            1 => {
                if let ParameterDocument::Bytes { value, .. } = value.spec.specification.parameters.get_mut(1).unwrap()
                {
                    value.cut = true;
                }
            }
            2 => value.spec.cut = true,
            3 => value.authority.cut = true,
            4 => value.cut = true,
            _ => unreachable!(),
        }
        let tree = build(&value, normal_words());
        assert_eq!(count(&tree, b"Only part of this text was supplied"), usize::from(flag < 2));
        assert_eq!(has(&tree, b"The specification is incomplete"), flag == 2);
        assert_eq!(has(&tree, b"More are held than shown"), flag == 3);
        assert_eq!(has(&tree, b"Dependency list incomplete"), flag == 4);
        assert_eq!(value.spec.whole().is_none(), flag <= 2);
        assert!(crate::instructions::whole(value.instructions.as_ref()).is_some());
        assert!(has(&tree, b"spec"));
        assert!(has(&tree, b"bytes"));
        assert_eq!(value.dependencies.get(0), Some(&u64::MAX));
        assert_eq!(value.spec.specification.parameters.capacity(), 3);
        assert_eq!(value.spec.specification.inputs.capacity(), 3);
    }
}
#[test]
fn owner_amendment_address_taker_spec_wake_and_opaque_authority_keep_full_independent_values() {
    let words = normal_words();
    let mut value = fixture();
    for (address, phrase, number) in [
        (PersonAddress::Party(u64::MAX), P::Party, u64::MAX),
        (PersonAddress::Role(u32::MAX), P::Role, u64::from(u32::MAX)),
    ] {
        value.addressed = Some(address);
        let tree = build(&value, words);
        let expected = words.book().say(words.language(), phrase.id(), &[Hole::Number(number)]).unwrap();
        assert!(has(&tree, &expected.text));
        assert!(has(&tree, b"Taken by party: 18446744073709551615"));
        for text in [
            b"a/b".as_slice(),
            b"\\xFF\\x00",
            b"tail",
            b"*",
            b"Exact ending",
            b"Open ending",
            b"Charter: 4294967295",
            b"Procedure: 4294967295",
            b"Role: 4294967295",
            b"Tool bits: 18446744073709551615",
            b"Note scope bits: 255",
            b"Dependency task: 18446744073709551615",
            b"Dependency task: 7",
        ] {
            assert!(has(&tree, text));
        }
        for text in [
            b"Parameter 4294967295 has value 18446744073709551615".as_slice(),
            b"Parameter 1",
            b"Parameter 2 names connector 65535 resource 18446744073709551615",
            b"Input task 9",
            b"Input task 18446744073709551615",
            b"Spend ceiling: 18446744073709551615",
            b"Delegate tasks: 4294967295",
            b"Delegate depth: 4294967295",
        ] {
            assert!(has(&tree, text));
        }
        let input_order: Box<[&[u8]]> = tree
            .nodes()
            .iter()
            .flat_map(|n| &n.text)
            .filter(|r| r.text.starts_with(b"Input task "))
            .map(|r| r.text.as_ref())
            .collect();
        assert_eq!(input_order.as_ref(), [b"Input task 9".as_slice(), b"Input task 18446744073709551615"].as_slice());
        for text in [
            b"Connector: 65535".as_slice(),
            b"Kind: 65535",
            b"Connector: 7",
            b"Deadline: 18446744073709551615",
            b"Does not wake",
            b"Wakes immediately",
            b"Wakes after 4294967295 messages or 18446744073709551615 ns of oldest message age",
        ] {
            assert!(has(&tree, text), "each nested authority and wake scalar is displayed whole");
        }
        assert_eq!(value.clone().addressed, Some(address));
        assert_eq!(
            value.authority.authority.grants.get(0).unwrap().pattern.segments.get(1).unwrap().as_ref(),
            &[255, 0]
        );
        assert_eq!(value.spec.specification.inputs.get(0), Some(&9));
        assert_eq!(value.spec.specification.inputs.get(1), Some(&u64::MAX));
        assert_eq!(
            value.spec.specification.parameters.get(0),
            Some(&ParameterDocument::Number { name: u32::MAX, value: u64::MAX })
        );
        assert_eq!(
            value.spec.specification.parameters.get(2),
            Some(&ParameterDocument::Resource { name: 2, connector: u16::MAX, resource: u64::MAX })
        );
    }
    value.addressed = None;
    value.taken_by = None;
    let tree = build(&value, words);
    assert!(has(&tree, b"Address not supplied"));
    assert!(has(&tree, b"Taker not supplied"));
    assert!(!has(&tree, b"Taken by party: 18446744073709551615"));
    assert_wake_fields(&mut value, words);
}

fn assert_wake_fields(value: &mut OwnerAmendmentState, words: &Words) {
    for results in [ResultsWake::Never, ResultsWake::Each, ResultsWake::LastOrFailure] {
        for switches in 0_u8..8 {
            value.wake.results = results;
            value.wake.questions = switches & 1 != 0;
            value.wake.answers = switches & 2 != 0;
            value.wake.timers = switches & 4 != 0;
            let tree = build(value, words);
            assert_eq!(tree.nodes().iter().filter(|n| n.role == skein_ui::Role::Region).count(), 2);
            let result_label: &[u8] = match results {
                ResultsWake::Never => b"Delegate results do not wake",
                ResultsWake::Each => b"Each delegate result wakes",
                ResultsWake::LastOrFailure => b"The last delegate result or a failure wakes",
            };
            assert!(has(&tree, result_label));
            assert!(has(&tree, if value.wake.questions { b"Questions wake" } else { b"Questions do not wake" }));
            assert!(has(&tree, if value.wake.answers { b"Answers wake" } else { b"Answers do not wake" }));
            assert!(has(&tree, if value.wake.timers { b"Timers wake" } else { b"Timers do not wake" }));
            assert!(!has(&tree, b"Subscriptions"));
        }
    }
}

#[test]
fn owner_amendment_selected_normal_words_preserve_checked_captions_numbers_and_nested_fragments() {
    let selected = words(1, true);
    let value = fixture();
    let tree = build(&value, &selected);
    assert!(
        tree.nodes().iter().any(|node| node.name.as_ref().is_some_and(|name| name.as_ref() == b"chosen language: "))
    );
    assert!(has(&tree, b"chosen language: 18446744073709551615"));
    assert!(has(&tree, b"chosen language: 4294967295"));
    assert!(!has(&tree, b"Instructions"));
    assert!(!has(&tree, b"Exact ending"));
    for bytes in [b"spec".as_slice(), b"bytes", b"live", b"a/b", b"\\xFF\\x00", b"tail", b"*"] {
        assert!(has(&tree, bytes));
    }
    let mut lost = value;
    lost.cut = true;
    lost.authority.cut = true;
    lost.spec.cut = true;
    lost.instructions.as_mut().unwrap().cut = true;
    let tree = build(&lost, &selected);
    assert!(count(&tree, b"chosen language: ") >= 6);
    for phrase in data::ALL {
        assert!(selected.book().capacity(phrase.id()).is_some());
    }
    let cloned = selected.clone();
    assert_eq!(
        build(&lost, &cloned).nodes(),
        tree.nodes(),
        "normal Words Clone retains the complete checked owner composition"
    );
}
type CapacityCase = (fn(&mut OwnerAmendmentState), bool);
const CAPACITY_CASES: [CapacityCase; 8] = [
    (|s| s.dependencies = List::with_capacity(4), false),
    (|s| s.spec.specification.parameters = List::with_capacity(4), false),
    (|s| s.spec.specification.inputs = List::with_capacity(4), false),
    (|s| s.authority.authority.grants = List::with_capacity(4), true),
    (|s| s.authority.authority.delegation.kinds = List::with_capacity(4), true),
    (|s| s.authority.authority.note_resources = List::with_capacity(4), true),
    (|s| s.authority.authority.grants.get_mut(0).unwrap().pattern.segments = List::with_capacity(4), false),
    (|s| s.authority.authority.note_resources.get_mut(0).unwrap().pattern.segments = List::with_capacity(4), false),
];
#[test]
fn owner_amendment_whole_tree_envelope_checks_full_nested_capacities_and_overflow_before_rendering() {
    let words = normal_words();
    let limits = client_limits();
    let value = fixture();

    for (mutate, fits) in CAPACITY_CASES {
        let mut larger = value.clone();
        mutate(&mut larger);
        assert_eq!(view::required(&larger, &limits, words).is_some(), fits);
        assert!(view::required(&larger, &crate::Limits { rows: 4, segments: 4, ..limits }, words).is_some());
    }
    assert!(view::required(&value, &crate::Limits { text: 4, ..limits }, words).is_none());
    assert!(view::worst_case(&crate::Limits { text: u32::MAX, ..limits }, words).is_none());
    assert!(view::worst_case(&crate::Limits { rows: u32::MAX, ..limits }, words).is_none());
    let mut full = value;
    full.instructions = Some(doc(b"12345678", true));
    full.spec.specification.words = doc(b"12345678", true);
    full.spec.specification.parameters = List::with_capacity(3);
    full.spec.specification.inputs = List::with_capacity(3);
    full.dependencies = List::with_capacity(3);
    full.authority.authority.grants = List::with_capacity(3);
    full.authority.authority.note_resources = List::with_capacity(3);
    for n in 0..3 {
        full.spec
            .specification
            .parameters
            .push(ParameterDocument::Bytes { name: n, value: doc(b"12345678", true) })
            .unwrap();
        full.spec.specification.inputs.push(u64::MAX).unwrap();
        full.dependencies.push(u64::MAX).unwrap();
        let mut segments = List::with_capacity(3);
        for _ in 0_u32..3 {
            segments.push(copy_of(&[255; 8])).unwrap();
        }
        full.authority
            .authority
            .grants
            .push(OwnerGrant {
                connector: u16::MAX,
                kind: u16::MAX,
                pattern: OwnerPattern { segments: segments.clone(), last: OwnerPatternEnd::Exact(copy_of(&[254; 8])) },
            })
            .unwrap();
        full.authority
            .authority
            .note_resources
            .push(OwnerResourceScope {
                connector: u16::MAX,
                pattern: OwnerPattern { segments, last: OwnerPatternEnd::Open(copy_of(&[253; 8])) },
            })
            .unwrap();
    }
    full.cut = true;
    full.authority.cut = true;
    full.spec.cut = true;
    let selected = words_for_full();
    let tree = build(&full, &selected);
    assert_eq!(count(&tree, b"\\xFF\\xFF\\xFF\\xFF\\xFF\\xFF\\xFF\\xFF"), 18);
    assert_eq!(count(&tree, b"12345678"), 5);
    assert!(tree.nodes().iter().filter(|n| n.role == skein_ui::Role::Region).all(|n| n.name.is_some()));
}
fn words_for_full() -> Words {
    words(1, true)
}
#[test]
fn owner_amendment_checked_book_refuses_missing_duplicate_and_wrong_number_holes() {
    let limits = phrase_limits();
    let mut language = crate::phrases::english::language();
    language.phrases = language.phrases.into_vec().into_iter().filter(|p| p.id != P::State.id()).collect();
    assert_eq!(
        Phrasebook::load(crate::phrases::jig::shapes(8), Box::new([language]), &limits).err(),
        Some(Invalid::Missing { id: P::State.id() })
    );
    let mut language = crate::phrases::english::language();
    language.phrases.iter_mut().find(|p| p.id == P::State.id()).unwrap().id = P::Party.id();
    assert_eq!(
        Phrasebook::load(crate::phrases::jig::shapes(8), Box::new([language]), &limits).err(),
        Some(Invalid::Duplicate { id: P::Party.id() })
    );
    let mut language = crate::phrases::english::language();
    language.phrases.iter_mut().find(|p| p.id == P::Party.id()).unwrap().forms[0].segments =
        Box::new([Segment::Text(copy_of(b"wrong hole"))]);
    assert_eq!(
        Phrasebook::load(crate::phrases::jig::shapes(8), Box::new([language]), &limits).err(),
        Some(Invalid::Holes { id: P::Party.id() })
    );
}
