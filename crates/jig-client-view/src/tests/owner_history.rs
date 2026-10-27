#![expect(clippy::disallowed_methods, reason = "ordinary proofs inspect public checked languages and trees")]
use crate::owner_history_words::{self as data, HistoryPhrase as P};
use crate::words::Words;
use crate::{JigBindings, owner_history as view};
use skein_ui::Limits;
use skein_ui::phrases::{Hole, Invalid, PhraseLimits, Phrasebook, Segment, SpanUnit, WallFormat};
use skein_ui::{Builder, Tree};

use jig_client::contract::*;
use skein_lib::{Duration, List, Wall, bytes::copy_of};
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
pub(super) fn state() -> OwnerAmendmentState {
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
                deadline: Some(Wall::from_nanos(u64::MAX)),
                notes: u8::MAX,
                note_resources: resources,
            },
            cut: false,
        },
        wake: OwnerWakePolicy {
            markers: 0,
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
pub(super) fn part() -> OwnerPart {
    let mut segments = List::with_capacity(3);
    for bytes in [b"a/b".as_slice(), &[255, 0], b""] {
        segments.push(copy_of(bytes)).unwrap();
    }
    OwnerPart { connector: u16::MAX, kind: OwnerPartKind::Effect, key: OwnerPartKey::Path(segments) }
}
pub(super) fn task() -> ProposedTask {
    let source = state();
    let mut resources = List::with_capacity(3);
    resources.push(part()).unwrap();
    ProposedTask {
        deadline: NewDeadline::Named,
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
fn actions() -> [OwnerRetainedAction; 5] {
    let mut batch = List::with_capacity(3);
    for _ in 0_u32..3 {
        batch.push(task()).unwrap();
    }
    [
        OwnerRetainedAction::Batch(batch),
        OwnerRetainedAction::Effect {
            part: part(),
            authority: state().authority,
            attempt: u64::MAX,
            completion: u32::MAX,
            position: u32::MAX,
            purpose: u64::MAX,
        },
        OwnerRetainedAction::Amend { task: u64::MAX, change: Box::new(delta()) },
        OwnerRetainedAction::Widen { task: u64::MAX, authority: state().authority },
        OwnerRetainedAction::Release { task: u64::MAX },
    ]
}
pub(super) fn proposal(action: OwnerRetainedAction) -> OwnerProposalHistory {
    OwnerProposalHistory {
        action,
        number: u64::MAX,
        proposer: u64::MAX,
        as_holder: true,
        state: OwnerProposalHistoryState::Pending {
            holder: OwnerHistoryHolder::Task(u64::MAX),
            since: Wall::from_nanos(7),
        },
    }
}
pub(super) fn line() -> OwnerHistoryLine {
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
pub(super) fn client_limits() -> crate::Limits {
    crate::Limits { rows: 3, text: 8, ..super::cards::CLIENT }
}
pub(super) fn words(selected: bool) -> Words {
    let mut english = crate::phrases::english::language();
    english.wall = WallFormat::Nanos;
    english.span_unit = SpanUnit::Nanos;
    let languages: Box<[skein_ui::phrases::Language]> = if selected {
        let mut translated = english.clone();
        for phrase in &mut translated.phrases {
            for form in &mut phrase.forms {
                for segment in &mut form.segments {
                    if let Segment::Text(bytes) = segment {
                        *bytes = copy_of(b"selected history: ");
                    }
                }
            }
        }
        Box::new([english, translated])
    } else {
        Box::new([english])
    };
    Words::new(
        Phrasebook::load(
            crate::phrases::jig::shapes(8),
            languages,
            &PhraseLimits {
                languages: 2,
                phrases: crate::owner_words::phrase_count(),
                holes: 4,
                segments: 9000,
                text_bytes: 200_000,
                hole_bytes: 8,
            },
        )
        .unwrap(),
        u16::from(selected),
        0,
    )
}
fn normal_words() -> &'static Words {
    static WORDS: std::sync::OnceLock<Words> = std::sync::OnceLock::new();
    WORDS.get_or_init(|| words(false))
}
fn build(value: &OwnerHistoryLine, words: &Words) -> (Tree, JigBindings) {
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
    assert!(crate::limits::jig_worst_case(&limits).is_some());
    let mut bindings = JigBindings::new(f.bindings, 8);
    let mut b = Builder::new(&limits);
    view::render(&mut b, value, words, &mut bindings, client_limits().rows);
    let tree = b.finish();
    assert!(tree.len() <= f.nodes);
    let mut total = 0_u64;
    let mut slots = 0_u32;
    for node in tree.nodes() {
        assert!(crate::tests::text_bytes(node) <= f.text_per_node);
        total = total.checked_add(u64::from(crate::tests::text_bytes(node))).unwrap();
        if let Some(name) = &node.name {
            assert!(name.len() <= usize::try_from(f.text_per_node).unwrap());
            total = total.checked_add(u64::try_from(name.len()).unwrap()).unwrap();
        }
        assert!(node.binding.is_none());
        if node.slot.is_some() {
            slots = slots.checked_add(1).unwrap();
        }
    }
    assert!(total <= u64::from(f.text_bytes));
    assert!(slots <= f.slots);
    assert!(slots <= f.bindings);
    (tree, bindings)
}
fn has(tree: &Tree, bytes: &[u8]) -> bool {
    tree.nodes().iter().any(|node| node.text.iter().any(|run| run.text.as_ref() == bytes))
}
fn count(tree: &Tree, bytes: &[u8]) -> usize {
    tree.nodes().iter().flat_map(|node| &node.text).filter(|run| run.text.as_ref() == bytes).count()
}
fn saying(tree: &Tree, words: &Words, phrase: P, holes: &[Hole]) {
    let expected = words.book().say(words.language(), phrase.id(), holes).unwrap();
    assert!(has(tree, &expected.text));
}
#[test]
fn owner_history_all_nine_changes_keep_category_collisions_full_width_identity_and_optional_clock() {
    let changes = [
        OwnerHistoryChange::Cancelled,
        OwnerHistoryChange::Released,
        OwnerHistoryChange::Amended,
        OwnerHistoryChange::Moved,
        OwnerHistoryChange::Proposed,
        OwnerHistoryChange::ProposalPassed,
        OwnerHistoryChange::ProposalAccepted,
        OwnerHistoryChange::ProposalRejected,
        OwnerHistoryChange::ProposalWithdrawn,
    ];
    let words = normal_words();
    for (index, change) in changes.into_iter().enumerate() {
        let value = OwnerHistoryLine {
            change,
            revision: u64::MAX,
            at: (index % 2 == 0).then_some(Wall::from_nanos(u64::MAX)),
            ..line()
        };
        let (tree, _) = build(&value, words);
        saying(&tree, words, P::Task, &[Hole::Number(u64::MAX)]);
        saying(&tree, words, P::Revision, &[Hole::Number(u64::MAX)]);
        assert_eq!(has(&tree, b"History time not supplied"), value.at.is_none());
        if let Some(at) = value.at {
            saying(&tree, words, P::At, &[Hole::Wall(at)]);
        }
        let expected = match change {
            OwnerHistoryChange::Cancelled => P::Cancelled,
            OwnerHistoryChange::Released => P::Released,
            OwnerHistoryChange::Amended => P::Amended,
            OwnerHistoryChange::Moved => P::Moved,
            OwnerHistoryChange::Proposed => P::Proposed,
            OwnerHistoryChange::ProposalPassed => P::Passed,
            OwnerHistoryChange::ProposalAccepted => P::Accepted,
            OwnerHistoryChange::ProposalRejected => P::Rejected,
            OwnerHistoryChange::ProposalWithdrawn => P::Withdrawn,
        };
        saying(&tree, words, expected, &[]);
        assert!(
            !tree.nodes().iter().any(|node| node.text.iter().any(|run| run.text.windows(9).any(|w| w == b"position:"))),
            "no history commit position invented"
        );
    }
    let mut trees: List<Tree> = List::with_capacity(6);
    for by in [
        OwnerRequester::Task(0),
        OwnerRequester::Person(0),
        OwnerRequester::Deployment { project: 0 },
        OwnerRequester::Task(u64::MAX),
        OwnerRequester::Person(u64::MAX),
        OwnerRequester::Deployment { project: u32::MAX },
    ] {
        let (tree, _) = build(&OwnerHistoryLine { by, task: 0, revision: 0, ..line() }, words);
        for other in &trees {
            assert_ne!(tree.nodes(), other.nodes());
        }
        trees.push(tree).unwrap();
    }
}
#[test]
fn owner_history_stored_snapshot_none_empty_bytes_and_loss_never_invent_historical_effective_text() {
    let words = normal_words();
    for (index, instructions) in
        [None, Some(doc(b"", false)), Some(doc(b"stored", false)), Some(doc(b"", true)), Some(doc(b"stored", true))]
            .into_iter()
            .enumerate()
    {
        let before = OwnerAmendmentState { instructions, ..state() };
        let after = OwnerAmendmentState { instructions: None, ..state() };
        let value = OwnerHistoryLine { amendment: Some((Box::new(before), Box::new(after))), ..line() };
        let (tree, _) = build(&value, words);
        saying(&tree, words, P::Stored, &[]);
        assert_eq!(count(&tree, b"Instructions were not supplied"), if index == 0 { 2 } else { 1 });
        assert_eq!(has(&tree, b"Instructions are empty"), index == 1);
        assert_eq!(has(&tree, b"stored"), index == 2 || index == 4);
        assert_eq!(has(&tree, b"Only part of this text was supplied"), index >= 3);
        assert!(!has(&tree, b"live"));
        assert!(!has(&tree, b"global"));
    }
    let (tree, _) = build(&line(), words);
    saying(&tree, words, P::SnapshotsAbsent, &[]);
    assert!(!has(&tree, b"stored"));
}
#[test]
fn owner_history_all_retained_actions_keep_whole_batches_raw_effect_coordinates_and_opaque_bindings() {
    let words = normal_words();
    for (index, action) in actions().into_iter().enumerate() {
        let original = action.clone();
        let value = OwnerHistoryLine { proposal: Some(proposal(action)), ..line() };
        let (tree, bindings) = build(&value, words);
        assert_eq!(value.proposal.as_ref().unwrap().action, original);
        let slots = tree.nodes().iter().filter_map(|n| n.slot).count();
        assert_eq!(
            slots,
            match index {
                0 => 6,
                1 => 1,
                _ => 0,
            }
        );
        for slot in tree.nodes().iter().filter_map(|n| n.slot) {
            assert_eq!(bindings.owner_part(slot.token), Some(&part()));
            assert!(bindings.part(slot.token).is_none());
        }
        if index < 2 {
            assert!(has(&tree, b"a/b"));
            assert!(has(&tree, b"\\xFF\\x00"));
        }
        if index == 0 {
            assert_eq!(count(&tree, b"Input task 9"), 3);
            assert_eq!(count(&tree, b"Input task 18446744073709551615"), 3);
        }
        if index == 1 {
            for (phrase, number) in [
                (P::Attempt, u64::MAX),
                (P::Completion, u64::from(u32::MAX)),
                (P::Position, u64::from(u32::MAX)),
                (P::Purpose, u64::MAX),
            ] {
                saying(&tree, words, phrase, &[Hole::Number(number)]);
            }
        }
    }
}
#[test]
fn owner_history_retained_optional_delta_keeps_revision_channels_empty_presence_order_and_independent_losses() {
    let words = normal_words();
    for flag in 0_u32..6 {
        let mut delta = delta();
        match flag {
            0 => delta.reason.cut = true,
            1 => delta.instructions.as_mut().unwrap().cut = true,
            2 => delta.spec.as_mut().unwrap().specification.words.cut = true,
            3 => delta.spec.as_mut().unwrap().cut = true,
            4 => delta.authority.as_mut().unwrap().cut = true,
            5 => delta.cut = true,
            _ => unreachable!(),
        }
        let value = OwnerHistoryLine {
            proposal: Some(proposal(OwnerRetainedAction::Amend { task: u64::MAX, change: Box::new(delta) })),
            ..line()
        };
        let (tree, _) = build(&value, words);
        saying(&tree, words, P::Revision, &[Hole::Number(6)]);
        saying(&tree, words, P::AddressRevision, &[Hole::Number(12)]);
        saying(&tree, words, P::HoldRevision, &[Hole::Number(4)]);
        assert_eq!(count(&tree, b"Only part of this text was supplied"), usize::from(flag < 3));
        assert_eq!(has(&tree, b"The specification is incomplete"), flag == 3);
        assert_eq!(has(&tree, b"More are held than shown"), flag == 4);
        assert_eq!(has(&tree, b"Retained dependency list incomplete"), flag == 5);
    }
    let absent = OwnerRetainedAmendment {
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
        ..absent.clone()
    };
    for (delta, is_empty) in [(absent, false), (empty, true)] {
        let (tree, _) = build(
            &OwnerHistoryLine {
                proposal: Some(proposal(OwnerRetainedAction::Amend { task: 0, change: Box::new(delta) })),
                ..line()
            },
            words,
        );
        assert_eq!(has(&tree, b"Instruction replacement not supplied"), !is_empty);
        assert_eq!(has(&tree, b"Instructions are empty"), is_empty);
        assert_eq!(has(&tree, b"Dependency replacement not supplied"), !is_empty);
        assert_eq!(has(&tree, b"Retained dependency replacement"), is_empty);
    }
}
#[test]
fn owner_history_all_states_holders_policy_kinds_raw_proposer_and_pending_clock_remain_independent() {
    let words = normal_words();
    let mut holders = List::with_capacity(7);
    holders.push(OwnerHistoryHolder::Task(0)).unwrap();
    holders.push(OwnerHistoryHolder::Person(0)).unwrap();
    for kind in [
        OwnerMessageProposalKind::Batch,
        OwnerMessageProposalKind::Effect,
        OwnerMessageProposalKind::Amend,
        OwnerMessageProposalKind::Widen,
        OwnerMessageProposalKind::Release,
    ] {
        holders.push(OwnerHistoryHolder::Policy { project: u32::MAX, kind }).unwrap();
    }
    for holder in &holders {
        for as_holder in [false, true] {
            let state = OwnerProposalHistoryState::Pending { holder: *holder, since: Wall::from_nanos(u64::MAX) };
            let value = OwnerHistoryLine {
                proposal: Some(OwnerProposalHistory {
                    state,
                    proposer: 0,
                    as_holder,
                    ..proposal(OwnerRetainedAction::Release { task: 0 })
                }),
                at: None,
                ..line()
            };
            let (tree, _) = build(&value, words);
            saying(&tree, words, P::Since, &[Hole::Wall(Wall::from_nanos(u64::MAX))]);
            saying(&tree, words, P::AtAbsent, &[]);
            saying(&tree, words, P::Proposer, &[Hole::Number(0)]);
            saying(&tree, words, if as_holder { P::AsHolder } else { P::NotAsHolder }, &[]);
        }
    }
    for state in [
        OwnerProposalHistoryState::Accepted { by: OwnerRequester::Task(0) },
        OwnerProposalHistoryState::Accepted { by: OwnerRequester::Person(0) },
        OwnerProposalHistoryState::Rejected {
            by: OwnerRequester::Deployment { project: u32::MAX },
            reason: doc(b"denied", true),
        },
        OwnerProposalHistoryState::Withdrawn,
    ] {
        let rejected = match &state {
            OwnerProposalHistoryState::Rejected { .. } => true,
            OwnerProposalHistoryState::Pending { .. }
            | OwnerProposalHistoryState::Accepted { .. }
            | OwnerProposalHistoryState::Withdrawn => false,
        };
        let value = OwnerHistoryLine {
            proposal: Some(OwnerProposalHistory { state, ..proposal(OwnerRetainedAction::Release { task: 0 }) }),
            ..line()
        };
        let (tree, _) = build(&value, words);
        assert_eq!(has(&tree, b"denied"), rejected);
        assert_eq!(has(&tree, b"Only part of this text was supplied"), rejected);
        assert!(!has(&tree, b"Proposal pending"));
    }
}
#[test]
fn owner_history_whole_view_admission_checks_nested_reserved_capacities_and_text_boundaries_without_clipping() {
    let words = normal_words();
    let source = OwnerHistoryLine {
        proposal: Some(proposal(actions().into_iter().next().unwrap())),
        amendment: Some((Box::new(state()), Box::new(state()))),
        ..line()
    };
    assert!(view::required(&source, &client_limits(), words).is_some());
    for limits in [crate::Limits { text: 7, ..client_limits() }, crate::Limits { rows: 2, ..client_limits() }] {
        let original = source.clone();
        assert!(view::required(&source, &limits, words).is_none());
        assert_eq!(source, original);
    }
    let reserved =
        OwnerHistoryLine { proposal: Some(proposal(OwnerRetainedAction::Batch(List::with_capacity(4)))), ..line() };
    assert!(view::required(&reserved, &client_limits(), words).is_none());
    assert_eq!(view::worst_case(&crate::Limits { rows: u32::MAX, text: u32::MAX, ..client_limits() }, words), None);
}
#[test]
fn owner_history_normal_selected_language_checks_all_holes_and_nested_tree_binding_envelope() {
    let words = words(true);
    for phrase in data::ALL {
        let shape = data::shapes().into_vec().into_iter().find(|s| s.id == phrase.id()).unwrap();
        let mut holes = List::with_capacity(1);
        for hole in &shape.holes {
            let value = match hole.kind {
                skein_ui::phrases::HoleKind::Number => Hole::Number(u64::MAX),
                skein_ui::phrases::HoleKind::Wall
                | skein_ui::phrases::HoleKind::Date
                | skein_ui::phrases::HoleKind::Moment => Hole::Wall(Wall::from_nanos(u64::MAX)),
                skein_ui::phrases::HoleKind::Amount
                | skein_ui::phrases::HoleKind::Span
                | skein_ui::phrases::HoleKind::Name
                | skein_ui::phrases::HoleKind::Reference => unreachable!(),
            };
            holes.push(value).unwrap();
        }
        assert!(words.book().say(1, phrase.id(), holes.as_slice()).unwrap().text.starts_with(b"selected history: "));
        assert_eq!(
            words.book().say(1, phrase.id(), &[Hole::Name(copy_of(b"wrong"))]).unwrap_err(),
            Invalid::Values { id: phrase.id() }
        );
    }
    for action in actions() {
        let value = OwnerHistoryLine {
            proposal: Some(proposal(action)),
            amendment: Some((Box::new(state()), Box::new(state()))),
            ..line()
        };
        let (tree, bindings) = build(&value, &words);
        assert!(tree.nodes().iter().any(|n| n.name.as_deref() == Some(b"selected history: ".as_slice())));
        for slot in tree.nodes().iter().filter_map(|n| n.slot) {
            assert_eq!(bindings.owner_part(slot.token), Some(&part()));
        }
    }
}

fn full_state() -> OwnerAmendmentState {
    let mut value = state();
    value.instructions = Some(doc(UTF8_TEXTS[0], true));
    value.spec.specification.words = doc(b"**x** y!", true);
    value.spec.cut = true;
    value.spec.specification.parameters.clear();
    value.spec.specification.inputs.clear();
    value.dependencies.clear();
    value.authority.authority.grants.clear();
    value.authority.authority.note_resources.clear();
    for n in 0_u32..3 {
        value
            .spec
            .specification
            .parameters
            .push(ParameterDocument::Bytes { name: n, value: doc(UTF8_TEXTS[1], true) })
            .unwrap();
        value.spec.specification.inputs.push(u64::from(n)).unwrap();
        value.dependencies.push(u64::from(n)).unwrap();
        let mut segments = List::with_capacity(3);
        for _ in 0_u32..3 {
            segments.push(copy_of(&[255; 8])).unwrap();
        }
        let pattern = OwnerPattern { segments, last: OwnerPatternEnd::Open(copy_of(&[253; 8])) };
        value
            .authority
            .authority
            .grants
            .push(OwnerGrant { connector: u16::MAX, kind: u16::MAX, pattern: pattern.clone() })
            .unwrap();
        value.authority.authority.note_resources.push(OwnerResourceScope { connector: u16::MAX, pattern }).unwrap();
    }
    value.authority.cut = true;
    value.cut = true;
    value
}
#[test]
fn owner_history_full_row_and_byte_nested_batch_hits_exact_raw_slot_capacity_with_selected_words() {
    let words = words(true);
    let source = full_state();
    let mut batch = List::with_capacity(3);
    for _ in 0_u32..3 {
        let mut value = task();
        value.spec = source.spec.clone();
        value.authority = source.authority.clone();
        value.dependencies = source.dependencies.clone();
        value.resources.clear();
        for _ in 0_u32..3 {
            value.resources.push(part()).unwrap();
        }
        batch.push(value).unwrap();
    }
    let value = OwnerHistoryLine {
        reason: doc(UTF8_TEXTS[0], true),
        proposal: Some(proposal(OwnerRetainedAction::Batch(batch))),
        amendment: Some((Box::new(source.clone()), Box::new(source))),
        ..line()
    };
    let source = value.clone();
    let f = view::required(&value, &client_limits(), &words).unwrap();
    let (tree, bindings) = build(&value, &words);
    let mut slots = 0_u32;
    for slot in tree.nodes().iter().filter_map(|n| n.slot) {
        slots = slots.checked_add(1).unwrap();
        assert_eq!(bindings.owner_part(slot.token), Some(&part()));
    }
    assert_eq!(slots, 12);
    assert_eq!(slots, f.bindings);
    assert_eq!(slots, f.slots);
    assert_eq!(value, source);
}

#[test]
fn owner_history_composed_normal_book_prices_every_entry_segment_literal_and_checked_hole_at_boundary() {
    use std::io::Write;
    let language = crate::phrases::english::language();
    let shapes = crate::phrases::jig::shapes(8);
    let entries = u32::try_from(language.phrases.len()).unwrap();
    let mut segments = 0_u32;
    let mut literals = 0_u32;
    let mut holes = 0_u16;
    for shape in &shapes {
        holes = holes.max(u16::try_from(shape.holes.len()).unwrap());
    }
    for phrase in &language.phrases {
        for form in &phrase.forms {
            for segment in &form.segments {
                segments = segments.checked_add(1).unwrap();
                match segment {
                    Segment::Text(bytes) => {
                        literals = literals.checked_add(u32::try_from(bytes.len()).unwrap()).unwrap();
                    }
                    Segment::Hole { .. } => {}
                }
            }
        }
    }
    std::io::stderr().write_fmt(format_args!("whole normal History book: entries={entries}, segments={segments}, literal_bytes={literals}, holes={holes}, languages=1, hole_bytes=8\n")).unwrap();
    assert_eq!(entries, crate::owner_words::phrase_count());
    assert_eq!(entries, u32::try_from(shapes.len()).unwrap());
    let limits = PhraseLimits { languages: 1, phrases: entries, holes, segments, text_bytes: literals, hole_bytes: 8 };
    let _exact_book = Phrasebook::load(shapes.clone(), Box::new([language.clone()]), &limits).unwrap();
    for below in [
        PhraseLimits { phrases: entries.checked_sub(1).unwrap(), ..limits },
        PhraseLimits { segments: segments.checked_sub(1).unwrap(), ..limits },
        PhraseLimits { text_bytes: literals.checked_sub(1).unwrap(), ..limits },
        PhraseLimits { holes: holes.checked_sub(1).unwrap(), ..limits },
        PhraseLimits { languages: 0, ..limits },
        PhraseLimits { hole_bytes: 7, ..limits },
    ] {
        assert_eq!(Phrasebook::load(shapes.clone(), Box::new([language.clone()]), &below).unwrap_err(), Invalid::Limit);
    }
    let above = PhraseLimits {
        phrases: entries.checked_add(1).unwrap(),
        segments: segments.checked_add(1).unwrap(),
        text_bytes: literals.checked_add(1).unwrap(),
        holes: holes.checked_add(1).unwrap(),
        languages: 2,
        hole_bytes: 9,
    };
    let _above_book = Phrasebook::load(shapes, Box::new([language]), &above).unwrap();
}

mod proposal_fragment;

const UTF8_TEXTS: [&[u8]; 2] = [&[b'~'; 8], &[b'!'; 8]];
