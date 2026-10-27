use crate::tests::fixtures::Fixture;
use crate::words::Words;
use crate::{JigBindings, owner_milestone as view};
use skein_ui::Limits;
use skein_ui::phrases::{Hole, Invalid, PhraseLimits, Phrasebook, Segment, SpanUnit, WallFormat};
use skein_ui::{Builder, Tree};

use jig_client::contract::*;
use skein_lib::{
    List, Wall,
    bytes::{copy_of, find},
};
fn doc(bytes: &[u8], cut: bool) -> DocumentText {
    DocumentText { bytes: Text::new(copy_of(bytes), 8).unwrap(), cut }
}
fn part(capacity: u32) -> OwnerPart {
    let mut segments = List::with_capacity(capacity);
    if capacity >= 3 {
        for bytes in [b"a/b".as_slice(), &[255, 0], b""] {
            segments.push(copy_of(bytes)).unwrap();
        }
    }
    OwnerPart { connector: u16::MAX, kind: OwnerPartKind::Resource, key: OwnerPartKey::Path(segments) }
}
fn line(phase: OwnerMilestonePhase) -> OwnerMilestoneLine {
    OwnerMilestoneLine { task: u64::MAX, position: u64::MAX, phase, at: None }
}
fn limits() -> crate::Limits {
    crate::Limits { rows: 3, segments: 3, text: 8, ..super::cards::CLIENT }
}
fn words(selected: bool) -> Words {
    let mut english = crate::phrases::english::language();
    english.wall = WallFormat::Nanos;
    english.span_unit = SpanUnit::Nanos;
    let languages: Box<[skein_ui::phrases::Language]> = if selected {
        let mut second = english.clone();
        for phrase in &mut second.phrases {
            for form in &mut phrase.forms {
                for segment in &mut form.segments {
                    if let Segment::Text(bytes) = segment {
                        *bytes = copy_of(b"second ");
                    }
                }
            }
        }
        Box::new([english, second])
    } else {
        Box::new([english])
    };
    Words::new(
        Phrasebook::load(
            crate::phrases::jig::shapes(256),
            languages,
            &PhraseLimits {
                languages: 2,
                phrases: crate::owner_words::phrase_count(),
                holes: 4,
                segments: 5000,
                text_bytes: 100_000,
                hole_bytes: 256,
            },
        )
        .unwrap(),
        u16::from(selected),
        0,
    )
}
fn build(value: &OwnerMilestoneLine, words: &Words) -> (Tree, JigBindings, view::MilestoneFragment) {
    let f = view::required(value, &limits(), words).unwrap();
    let capacity = Limits {
        nodes: f.nodes,
        depth: f.depth,
        text_per_node: f.text_per_node,
        text_bytes: f.text_bytes,
        drawing_bytes: 0,
        styles_per_node: 2,
        patches: 0,
        bindings: f.bindings,
        slots: f.slots,
        markdown_depth: 2,
        scene_marks: 0,
        scene_bytes: 0,
    };
    let mut b = Builder::new(&capacity);
    let mut bindings = JigBindings::new(f.bindings, 256);
    let client = Fixture::new(&limits());
    view::render(&mut b, value, &client.view(), words, &mut bindings);
    let tree = b.finish();
    let region = tree.nodes().first().expect("standalone milestone region");
    assert_eq!(region.role, skein_ui::Role::Region);
    assert_eq!(
        region.name.as_deref(),
        Some(if words.language() == 0 { b"Lifecycle milestone".as_slice() } else { b"second ".as_slice() })
    );
    assert!(tree.len() <= f.nodes);
    let mut total = 0_u64;
    for node in tree.nodes() {
        assert!(crate::tests::text_bytes(node) <= f.text_per_node);
        total = total.checked_add(u64::from(crate::tests::text_bytes(node))).unwrap();
    }
    assert!(total <= u64::from(f.text_bytes));
    (tree, bindings, f)
}
fn has(tree: &Tree, bytes: &[u8]) -> bool {
    for node in tree.nodes() {
        for run in &node.text {
            if run.text.as_ref() == bytes {
                return true;
            }
        }
    }
    false
}
fn contains(tree: &Tree, bytes: &[u8]) -> bool {
    for node in tree.nodes() {
        for run in &node.text {
            if find(&run.text, bytes).is_some() {
                return true;
            }
        }
    }
    false
}
#[test]
fn owner_milestone_all_five_phases_six_activation_and_five_closing_stages_keep_owner_coordinates() {
    let w = words(false);
    let waiting = line(OwnerMilestonePhase::Waiting);
    assert!(has(&build(&waiting, &w).0, b"Waiting"));
    for (stage, label) in [
        (RunStage::Idle, b"Activation idle".as_slice()),
        (RunStage::Due, b"Activation due"),
        (RunStage::Preparing, b"Activation preparing"),
        (RunStage::Claimed { attempt: u64::MAX }, b"Claimed attempt: 18446744073709551615"),
        (RunStage::Running { attempt: u64::MAX }, b"Running attempt: 18446744073709551615"),
        (RunStage::BackingOff { until: Wall::from_nanos(u64::MAX) }, b"Backing off until: 18446744073709551615"),
    ] {
        let value = line(OwnerMilestonePhase::Active(stage));
        let tree = build(&value, &w).0;
        assert!(has(&tree, label));
        assert!(!contains(&tree, b"Started at"));
        assert!(!contains(&tree, b"Observed"));
        assert_eq!(value.phase, OwnerMilestonePhase::Active(stage));
    }
    for (stage, label) in [
        (ClosingStage::Run { attempt: u64::MAX }, b"Closing run attempt: 18446744073709551615".as_slice()),
        (ClosingStage::Delegates, b"Closing delegates"),
        (ClosingStage::Effects, b"Closing effects"),
        (ClosingStage::Releases, b"Closing releases"),
        (ClosingStage::Settled, b"Closing settled"),
    ] {
        let value =
            line(OwnerMilestonePhase::Closing { stage, ending: OwnerChipEnding::Failed(doc(b"closing", false)) });
        let tree = build(&value, &w).0;
        assert!(has(&tree, label));
        assert!(has(&tree, b"closing"));
    }
    assert!(has(
        &build(&line(OwnerMilestonePhase::Ended(OwnerChipEnding::Failed(doc(b"end", false)))), &w).0,
        b"Ended"
    ));
    assert!(has(
        &build(
            &line(OwnerMilestonePhase::Held {
                before: OwnerPriorPhase::Waiting,
                why: HoldCategory::Budget,
                source: None,
                ending: None
            }),
            &w
        )
        .0,
        b"Held"
    ));
}
#[test]
fn owner_milestone_all_sixteen_hold_categories_and_six_failures_preserve_prior_without_live_revision_or_clock() {
    let w = words(false);
    let reasons = [
        (HoldCategory::Stalled, b"Held stalled".as_slice()),
        (HoldCategory::Dependency { task: u64::MAX }, b"Held dependency task: 18446744073709551615"),
        (HoldCategory::Stopped, b"Held stopped"),
        (HoldCategory::Placed { hold: u64::MAX }, b"Held by a placed hold"),
        (HoldCategory::Drift, b"Held drift"),
        (HoldCategory::Effects, b"Held effects"),
        (HoldCategory::EffectFailed, b"Held effect failed"),
        (HoldCategory::Uncertain { entry: u64::MAX }, b"Held uncertain entry: 18446744073709551615"),
        (HoldCategory::Budget, b"Held budget"),
        (HoldCategory::Deadline, b"Held deadline"),
        (HoldCategory::HoldsWaited, b"Held waited holds"),
        (HoldCategory::ResourceUnavailable, b"Held resource unavailable"),
        (HoldCategory::ResourceAccess, b"Held resource access"),
        (HoldCategory::PoolLost { pool: u64::MAX }, b"Held lost pool: 18446744073709551615"),
        (HoldCategory::Procedure, b"Held procedure"),
    ];
    for (why, label) in reasons {
        let value =
            line(OwnerMilestonePhase::Held { before: OwnerPriorPhase::Waiting, why, source: None, ending: None });
        let tree = build(&value, &w).0;
        assert!(has(&tree, label));
        assert!(has(&tree, b"Hold source not supplied"));
        assert!(has(&tree, b"Held ending not supplied"));
        assert!(!contains(&tree, b"Hold revision"));
        assert!(!contains(&tree, b"Phase began"));
    }
    for (class, label) in [
        (HoldFailure::Transient, b"Transient failure".as_slice()),
        (HoldFailure::Permanent, b"Permanent failure"),
        (HoldFailure::Run, b"Run failure"),
        (HoldFailure::Agent, b"Agent failure"),
        (HoldFailure::Lost, b"Lost failure"),
        (HoldFailure::Invalid, b"Invalid failure"),
    ] {
        let value = line(OwnerMilestonePhase::Held {
            before: OwnerPriorPhase::Active(RunStage::Running { attempt: u64::MAX }),
            why: HoldCategory::Failures { class },
            source: None,
            ending: None,
        });
        let tree = build(&value, &w).0;
        assert!(has(&tree, label));
        assert!(has(&tree, b"Failure tries exhausted"));
        assert!(has(&tree, b"Prior stage running attempt: 18446744073709551615"));
    }
    let value = line(OwnerMilestonePhase::Held {
        before: OwnerPriorPhase::Closing(ClosingStage::Run { attempt: u64::MAX }),
        why: HoldCategory::Budget,
        source: Some(part(3)),
        ending: Some(OwnerChipEnding::Failed(doc(b"pending", true))),
    });
    let (tree, bindings, _) = build(&value, &w);
    assert!(has(&tree, b"Prior closing run attempt: 18446744073709551615"));
    assert!(has(&tree, b"pending"));
    let mut slots = 0_u32;
    for node in tree.nodes() {
        if let Some(slot) = node.slot {
            assert_eq!(bindings.owner_part(slot.token), Some(&part(3)));
            slots = slots.checked_add(1).unwrap();
        }
    }
    assert_eq!(slots, 1);
    assert!(has(&tree, b"a/b"));
    assert!(has(&tree, b"\\xFF\\x00"));
    assert!(!has(&tree, b"Hold source not supplied"));
}
#[test]
fn owner_milestone_task_and_position_are_full_width_and_optional_time_is_separate_from_cancellation_time() {
    let w = words(false);
    for task in [0, u64::MAX] {
        for position in [0, u64::MAX] {
            for at in [None, Some(Wall::from_nanos(0)), Some(Wall::from_nanos(u64::MAX))] {
                let value = OwnerMilestoneLine { task, position, at, phase: OwnerMilestonePhase::Waiting };
                let tree = build(&value, &w).0;
                assert_eq!(value.task, task);
                assert_eq!(value.position, position);
                assert_eq!(value.at, at);
                assert_eq!(has(&tree, b"Milestone time not supplied"), at.is_none());
                if let Some(at) = at {
                    assert!(has(
                        &tree,
                        if at.as_nanos() == 0 {
                            b"Milestone time: 0".as_slice()
                        } else {
                            b"Milestone time: 18446744073709551615"
                        }
                    ));
                }
                assert!(!contains(&tree, b"Revision"));
                if task == u64::MAX {
                    assert!(has(&tree, b"Task: 18446744073709551615"));
                } else {
                    assert!(has(&tree, b"Task: 0"));
                }
                if position == u64::MAX {
                    assert!(has(&tree, b"Activity position: 18446744073709551615"));
                } else {
                    assert!(has(&tree, b"Activity position: 0"));
                }
            }
        }
    }
    for (by, label) in [
        (OwnerRequester::Task(u64::MAX), b"Cancellation requester task: 18446744073709551615".as_slice()),
        (OwnerRequester::Person(u64::MAX), b"Cancellation requester person: 18446744073709551615"),
        (OwnerRequester::Deployment { project: u32::MAX }, b"Cancellation requester deployment project: 4294967295"),
    ] {
        let value = line(OwnerMilestonePhase::Ended(OwnerChipEnding::Cancelled {
            reason: doc(b"stop", true),
            result: None,
            by,
            at: Wall::from_nanos(12),
        }));
        let tree = build(&value, &w).0;
        assert!(has(&tree, label));
        assert!(has(&tree, b"Cancelled at: 12"));
        assert!(has(&tree, b"Milestone time not supplied"));
        assert!(has(&tree, b"Supplied words incomplete"));
        assert!(!has(&tree, b"Held ending not supplied"));
    }
}
#[test]
fn owner_milestone_whole_results_keep_all_nine_variants_and_independent_approval_presence_loss_and_raw_bytes() {
    let w = words(false);
    let mut fields = List::with_capacity(3);
    fields.push(ResultField { name: copy_of(b"name"), value: copy_of(b"value") }).unwrap();
    let values = [
        ResultValue::Report(doc(b"report", true)),
        ResultValue::OwnerVerdict { verdict: u32::MAX, words: doc(b"verdict", false), fields: fields.clone() },
        ResultValue::Verdict {
            verdict: u32::MAX,
            words: doc(b"followup", true),
            fields,
            tasks: List::with_capacity(3),
            proposal: None,
        },
        ResultValue::Connector { summary: doc(b"summary", false), part: Part::Connector { connector: u16::MAX } },
        ResultValue::Change { kind: u16::MAX, part: part(3), words: doc(b"change", true) },
        ResultValue::Choice {
            option: u32::MAX,
            by: Party { number: u64::MAX, kind: PartyKind::Person, name: Text::new(copy_of(b"person"), 8).unwrap() },
            reason: doc(b"choice", false),
        },
        ResultValue::Approved { state: copy_of(&[255, 0]), words: None },
        ResultValue::Failure(doc(b"failure", true)),
        ResultValue::Words(doc(b"words", false)),
    ];
    for value in values {
        let label: &[u8] = match &value {
            ResultValue::Report(_) => b"report",
            ResultValue::OwnerVerdict { .. } => b"verdict",
            ResultValue::Verdict { .. } => b"followup",
            ResultValue::Connector { .. } => b"summary",
            ResultValue::Change { .. } => b"change",
            ResultValue::Choice { .. } => b"choice",
            ResultValue::Approved { .. } => b"\\xFF\\x00",
            ResultValue::Failure(_) => b"failure",
            ResultValue::Words(_) => b"words",
        };
        let fields_supplied = match &value {
            ResultValue::OwnerVerdict { .. } | ResultValue::Verdict { .. } => true,
            ResultValue::Report(_)
            | ResultValue::Connector { .. }
            | ResultValue::Change { .. }
            | ResultValue::Choice { .. }
            | ResultValue::Approved { .. }
            | ResultValue::Failure(_)
            | ResultValue::Words(_) => false,
        };
        let source = line(OwnerMilestonePhase::Ended(OwnerChipEnding::Done(value)));
        let original = source.clone();
        let tree = build(&source, &w).0;
        assert!(has(&tree, b"Completed with result"));
        assert!(has(&tree, label));
        if fields_supplied {
            assert!(has(&tree, b"name"));
            assert!(has(&tree, b"value"));
            assert!(contains(&tree, b"4294967295"));
        }
        assert_eq!(source, original);
    }
    let mut absent: Option<u32> = None;
    for explanation in [None, Some(doc(b"", false)), Some(doc(b"explain", true))] {
        let supplied = explanation.is_some();
        let lost = match &explanation {
            Some(value) => value.cut,
            None => false,
        };
        let value = line(OwnerMilestonePhase::Ended(OwnerChipEnding::Done(ResultValue::Approved {
            state: copy_of(&[255, 0, 254]),
            words: explanation,
        })));
        let tree = build(&value, &w).0;
        assert!(has(&tree, b"\\xFF\\x00\\xFE"));
        if supplied {
            assert_eq!(tree.len(), absent.unwrap().checked_add(if lost { 2 } else { 1 }).unwrap());
        } else {
            absent = Some(tree.len());
        }
    }
    let value = line(OwnerMilestonePhase::Ended(OwnerChipEnding::Failed(doc(b"failed", true))));
    assert!(has(&build(&value, &w).0, b"Supplied words incomplete"));
}
#[test]
fn owner_milestone_whole_view_refuses_each_nested_reserved_list_text_and_opaque_boundary_without_mutation() {
    let w = words(false);
    let base = limits();
    let mut fields = List::with_capacity(3);
    fields.push(ResultField { name: copy_of(b"12345678"), value: copy_of(b"12345678") }).unwrap();
    let exact = line(OwnerMilestonePhase::Held {
        before: OwnerPriorPhase::Waiting,
        why: HoldCategory::Budget,
        source: Some(part(3)),
        ending: Some(OwnerChipEnding::Done(ResultValue::OwnerVerdict {
            verdict: 1,
            words: doc(b"12345678", true),
            fields,
        })),
    });
    let unchanged = exact.clone();
    assert!(view::required(&exact, &base, &w).is_some());
    assert!(view::required(&exact, &crate::Limits { rows: 2, ..base }, &w).is_none());
    assert!(view::required(&exact, &crate::Limits { text: 7, ..base }, &w).is_none());
    assert_eq!(exact, unchanged);
    for phase in [
        OwnerMilestonePhase::Ended(OwnerChipEnding::Done(ResultValue::Approved {
            state: copy_of(&[255; 8]),
            words: Some(doc(b"12345678", true)),
        })),
        OwnerMilestonePhase::Ended(OwnerChipEnding::Failed(doc(b"12345678", false))),
    ] {
        let value = line(phase);
        assert!(view::required(&value, &base, &w).is_some());
        assert!(view::required(&value, &crate::Limits { text: 7, ..base }, &w).is_none());
    }
    let empty = line(OwnerMilestonePhase::Held {
        before: OwnerPriorPhase::Waiting,
        why: HoldCategory::Budget,
        source: Some(part(4)),
        ending: None,
    });
    assert!(view::required(&empty, &base, &w).is_none());
    assert!(view::max_nodes(&crate::Limits { rows: u32::MAX, ..base }).is_none());
    assert!(view::worst_case(&crate::Limits { text: u32::MAX, ..base }, &w).is_none());
}
#[test]
fn owner_milestone_second_checked_language_and_full_fragment_preserve_raw_slots_and_reject_wrong_holes() {
    let w = words(true);
    let value = line(OwnerMilestonePhase::Held {
        before: OwnerPriorPhase::Closing(ClosingStage::Settled),
        why: HoldCategory::PoolLost { pool: u64::MAX },
        source: Some(part(3)),
        ending: Some(OwnerChipEnding::Cancelled {
            reason: doc(UTF8_TEXTS[0], true),
            result: Some(ResultValue::Change { kind: u16::MAX, part: part(3), words: doc(UTF8_TEXTS[1], true) }),
            by: OwnerRequester::Person(u64::MAX),
            at: Wall::from_nanos(u64::MAX),
        }),
    });
    let (tree, bindings, f) = build(&value, &w);
    assert_eq!(f.nodes, view::max_nodes(&limits()).unwrap());
    assert_eq!(f, view::worst_case(&limits(), &w).unwrap());
    assert!(has(&tree, b"second 18446744073709551615"));
    assert!(!contains(&tree, b"Milestone time"));
    let mut slots = 0_u32;
    for n in tree.nodes() {
        if let Some(slot) = n.slot {
            assert_eq!(bindings.owner_part(slot.token), Some(&part(3)));
            slots = slots.checked_add(1).unwrap();
        }
    }
    assert_eq!(slots, 2);
    let id = crate::owner_milestone_words::MilestonePhrase::Position.id();
    let saying = w.book().say(1, id, &[Hole::Number(u64::MAX)]).expect("checked numeric hole");
    assert_eq!(saying.text.as_ref(), b"second 18446744073709551615");
    assert_eq!(w.book().say(1, id, &[Hole::Wall(Wall::from_nanos(0))]).err(), Some(Invalid::Values { id }));
}

const UTF8_TEXTS: [&[u8]; 2] = [&[b'~'; 8], &[b'!'; 8]];
