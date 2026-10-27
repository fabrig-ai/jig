#![expect(clippy::disallowed_methods, reason = "ordinary tests inspect public trees and checked language data")]
use crate::owner_chip_words::{self as data, ChipPhrase, OwnerChipWords};
use crate::tests::fixtures::Fixture;
use crate::words::Words;
use crate::{JigBindings, owner_chip as view};
use skein_ui::Limits;
use skein_ui::phrases::{Invalid, PhraseLimits, Segment};
use skein_ui::{Builder, Tree};

use jig_client::contract::*;
use skein_lib::{Duration, List, Token, Wall, bytes::copy_of};
fn doc(bytes: &[u8], cut: bool) -> DocumentText {
    DocumentText { bytes: Text::new(copy_of(bytes), 256).unwrap(), cut }
}
fn part() -> OwnerPart {
    let mut path = List::with_capacity(2);
    path.push(copy_of(b"a/b")).unwrap();
    path.push(copy_of(&[255, 0])).unwrap();
    OwnerPart { connector: u16::MAX, kind: OwnerPartKind::Resource, key: OwnerPartKey::Path(path) }
}
pub(super) fn details() -> OwnerChipDetails {
    OwnerChipDetails {
        dependencies: List::with_capacity(0),
        project: u32::MAX,
        executor: OwnerExecutor::Agent { charter: u32::MAX },
        phase_since: Wall::from_nanos(10),
        run_since: Some(Wall::from_nanos(20)),
        attempt: Some(u64::MAX),
        retry: Some(RetryNotice::Refused { count: u32::MAX }),
        wake: OwnerWakePolicy {
            words: WakeRule::Never,
            notices: WakeRule::Immediate,
            news: WakeRule::Batch { count: u32::MAX, age: Duration::from_nanos(30) },
            results: ResultsWake::LastOrFailure,
            questions: true,
            answers: false,
            timers: true,
        },
        person_wait: None,
        hold: None,
        budget: OwnerBudget {
            numbers: OwnerNumbers { budget: 100, spent: 2, spent_below: 3, reserved: 5 },
            left: 7,
            funder: Funder::Task(u64::MAX),
            deadline: None,
        },
        activity: u64::MAX,
        resources: List::with_capacity(0),
        resources_cut: false,
        resources_observed: false,
        procedure: None,
        live_delegates: u32::MAX,
        closing_connectors: None,
        ending: None,
        ended_at: None,
        observation: None,
    }
}
fn hold() -> OwnerChipHold {
    OwnerChipHold {
        words: None,
        placed: None,
        circle: None,
        account: None,
        source: None,
        reason: HoldCategory::Budget,
        revision: u64::MAX,
        before: OwnerPriorPhase::Waiting,
        escalation: None,
        tries: OwnerFailureTries { transient: 1, permanent: 2, run: 3, agent: 4, lost: 5, invalid: u32::MAX },
        hold_wait_since: Some(Wall::from_nanos(40)),
    }
}
fn client_limits() -> crate::Limits {
    crate::Limits { rows: 4, text: 256, objects: 4, ..super::cards::CLIENT }
}
fn phrase_limits() -> PhraseLimits {
    PhraseLimits {
        languages: 2,
        phrases: crate::owner_words::phrase_count(),
        holes: 4,
        segments: super::english_phrase_segments(2),
        text_bytes: 100_000,
        hole_bytes: 256,
    }
}
fn words() -> &'static Words {
    static WORDS: std::sync::OnceLock<Words> = std::sync::OnceLock::new();
    WORDS.get_or_init(|| {
        let mut language = crate::phrases::english::language();
        language.wall = skein_ui::phrases::WallFormat::Nanos;
        language.span_unit = skein_ui::phrases::SpanUnit::Nanos;
        Words::new(
            skein_ui::phrases::Phrasebook::load(
                crate::phrases::jig::shapes(256),
                Box::new([language]),
                &phrase_limits(),
            )
            .unwrap(),
            0,
            0,
        )
    })
}
fn build(value: &OwnerChipDetails, words: &Words) -> (Tree, JigBindings, view::ChipFragment) {
    let client = Fixture::new(&client_limits());
    let f = view::required(value, &client_limits(), words).unwrap();
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
    view::render(&mut b, value, &client.view(), words, &mut bindings);
    let tree = b.finish();
    assert!(tree.len() <= f.nodes);
    assert!(tree.nodes().iter().all(|node| crate::tests::text_bytes(node) <= f.text_per_node));
    (tree, bindings, f)
}
fn has(tree: &Tree, bytes: &[u8]) -> bool {
    tree.nodes().iter().any(|node| node.text.iter().any(|run| run.text.as_ref() == bytes))
}
fn contains(tree: &Tree, bytes: &[u8]) -> bool {
    tree.nodes().iter().any(|node| node.text.iter().any(|run| run.text.windows(bytes.len()).any(|part| part == bytes)))
}
#[test]
fn retained_chip_preserves_concrete_executor_project_and_full_width_owner_scalars() {
    let mut value = details();
    for (executor, label) in [
        (OwnerExecutor::Agent { charter: u32::MAX }, b"Agent charter 4294967295".as_slice()),
        (
            OwnerExecutor::Procedure { connector: u16::MAX, code: u32::MAX },
            b"Procedure connector 65535, code 4294967295",
        ),
        (OwnerExecutor::Person(PersonAddress::Party(u64::MAX)), b"Person 18446744073709551615"),
        (OwnerExecutor::Person(PersonAddress::Role(u32::MAX)), b"Project role 4294967295"),
    ] {
        value.executor = executor;
        let (tree, _, _) = build(&value, words());
        assert_eq!(tree.nodes()[0].role, skein_ui::Role::Region);
        assert_eq!(tree.nodes()[0].name.as_deref(), Some(b"Project: 4294967295".as_slice()));
        assert_eq!(value.clone().executor, executor);
        assert!(has(&tree, label), "concrete executor identity is never inferred from phase");
        for label in [
            b"Project: 4294967295".as_slice(),
            b"Attempt: 18446744073709551615",
            b"Activity through: 18446744073709551615",
            b"Live delegates: 4294967295",
        ] {
            assert!(has(&tree, label));
        }
    }
}
#[test]
fn placement_and_attempt_do_not_attest_unobserved_work_or_actual_start() {
    let mut value = details();
    let (tree, _, _) = build(&value, words());
    assert!(has(&tree, b"Placed or adopted: 20"));
    assert!(has(&tree, b"Run work unobserved"));
    assert!(!contains(&tree, b"Actual run start:"));
    assert!(!contains(&tree, b"Reported work"));
    let observation = OwnerChipObservation {
        channel: Token::new(u64::MAX),
        work: OwnerChipWork::Tool { call: copy_of(&[255, 0]), tool: copy_of(b"opaque/tool") },
        since: Wall::from_nanos(50),
        reported_at: Wall::from_nanos(60),
        started: None,
    };
    value.observation = Some(observation.clone());
    let (tree, _, _) = build(&value, words());
    assert_eq!(value.observation, Some(observation));
    for label in [
        b"\\xFF\\x00".as_slice(),
        b"opaque/tool",
        b"Current work since: 50",
        b"Work reported at: 60",
        b"Actual run start unavailable",
    ] {
        assert!(has(&tree, label));
    }
    value.observation.as_mut().unwrap().started = Some(Wall::from_nanos(70));
    assert!(has(&build(&value, words()).0, b"Actual run start: 70"));
    for (work, label) in [
        (OwnerChipWork::Preparing, b"Reported work preparing".as_slice()),
        (OwnerChipWork::Starting, b"Reported work starting"),
        (OwnerChipWork::Active, b"Reported work active"),
        (OwnerChipWork::Writing, b"Reported work writing"),
        (OwnerChipWork::Waiting, b"Reported work waiting"),
        (OwnerChipWork::Ending, b"Reported work ending"),
    ] {
        value.observation.as_mut().unwrap().work = work;
        assert!(has(&build(&value, words()).0, label));
    }
}
#[test]
fn closing_connector_absence_empty_and_terminal_clock_are_independent_owner_facts() {
    let mut value = details();
    assert!(has(&build(&value, words()).0, b"Closing connectors unavailable"));
    value.closing_connectors = Some(List::with_capacity(2));
    assert!(has(&build(&value, words()).0, b"No closing connectors"));
    value.closing_connectors.as_mut().unwrap().push(u16::MAX).unwrap();
    let (tree, _, _) = build(&value, words());
    assert!(has(&tree, b"Closing connector: 65535"));
    assert!(!contains(&tree, b"effects: "));
    value.ending = Some(OwnerChipEnding::Failed(doc(b"failed", false)));
    assert!(!contains(&build(&value, words()).0, b"Ended at:"));
    value.ended_at = Some(Wall::from_nanos(80));
    value.ending = None;
    let tree = build(&value, words()).0;
    assert!(has(&tree, b"Ended at: 80"));
    assert!(!has(&tree, b"Failed with reason"));
}
#[test]
fn resource_rows_keep_raw_segmented_parts_and_independent_holder_writer_loss_observation() {
    let mut value = details();
    let reference = part();
    value.resources = List::with_capacity(2);
    value
        .resources
        .push(OwnerResourceLine {
            part: reference.clone(),
            kind: u16::MAX,
            taken: false,
            holder: None,
            writer: Some(OwnerResourceWriter::Effect { entry: u64::MAX }),
            writer_lost: true,
        })
        .unwrap();
    value.resources_cut = true;
    let (tree, bindings, _) = build(&value, words());
    for label in [
        b"a/b".as_slice(),
        b"\\xFF\\x00",
        b"Resource kind: 65535",
        b"Resource not taken",
        b"Writer effect: 18446744073709551615",
        b"Writer lost",
        b"Resource list incomplete",
        b"Resource holder and writer facts unavailable",
    ] {
        assert!(has(&tree, label));
    }
    assert!(!contains(&tree, b"Holder task:"));
    assert!(!contains(&tree, b"Resource is free"));
    for node in tree.nodes() {
        if let Some(slot) = node.slot {
            assert_eq!(bindings.owner_part(slot.token), Some(&reference));
        }
    }
    let row = value.resources.get_mut(0).unwrap();
    row.holder = Some(u64::MAX);
    row.writer = Some(OwnerResourceWriter::Run { task: u64::MAX, attempt: u64::MAX });
    row.taken = true;
    row.writer_lost = false;
    value.resources_observed = true;
    value.resources_cut = false;
    let tree = build(&value, words()).0;
    for label in [
        b"Holder task: 18446744073709551615".as_slice(),
        b"Writer task: 18446744073709551615",
        b"Writer attempt: 18446744073709551615",
        b"Resource taken",
        b"Writer not lost",
        b"Resource holder and writer facts observed",
    ] {
        assert!(has(&tree, label));
    }
    assert!(!has(&tree, b"Resource list incomplete"));
}
#[test]
fn all_hold_categories_and_six_failure_classes_retain_only_supplied_payloads_and_counters() {
    let mut value = details();
    value.hold = Some(hold());
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
    for (reason, label) in reasons {
        value.hold.as_mut().unwrap().reason = reason;
        let tree = build(&value, words()).0;
        assert!(has(&tree, label));
        for label in [
            b"Hold revision: 18446744073709551615".as_slice(),
            b"Hold wait began: 40",
            b"Transient tries: 1",
            b"Permanent tries: 2",
            b"Run tries: 3",
            b"Agent tries: 4",
            b"Lost tries: 5",
            b"Invalid tries: 4294967295",
        ] {
            assert!(has(&tree, label));
        }
        assert!(!contains(&tree, b"Previous attempt"));
        assert!(!contains(&tree, b"Escalation waiting since:"));
    }
    for (class, label) in [
        (HoldFailure::Transient, b"Transient failure".as_slice()),
        (HoldFailure::Permanent, b"Permanent failure"),
        (HoldFailure::Run, b"Run failure"),
        (HoldFailure::Agent, b"Agent failure"),
        (HoldFailure::Lost, b"Lost failure"),
        (HoldFailure::Invalid, b"Invalid failure"),
    ] {
        value.hold.as_mut().unwrap().reason = HoldCategory::Failures { class };
        let tree = build(&value, words()).0;
        assert!(has(&tree, label));
        assert!(has(&tree, b"Failure tries exhausted"));
    }
}
#[test]
fn held_prior_stages_have_no_invented_clocks_and_escalations_keep_holder_scope_and_local_loss() {
    let mut value = details();
    value.hold = Some(hold());
    value.run_since = None;
    value.attempt = None;
    let priors = [
        (OwnerPriorPhase::Waiting, b"Prior stage waiting".as_slice()),
        (OwnerPriorPhase::Active(RunStage::Idle), b"Prior stage idle"),
        (OwnerPriorPhase::Active(RunStage::Due), b"Prior stage due"),
        (OwnerPriorPhase::Active(RunStage::Preparing), b"Prior stage preparing"),
        (
            OwnerPriorPhase::Active(RunStage::Claimed { attempt: u64::MAX }),
            b"Prior stage claimed attempt: 18446744073709551615",
        ),
        (
            OwnerPriorPhase::Active(RunStage::Running { attempt: u64::MAX }),
            b"Prior stage running attempt: 18446744073709551615",
        ),
        (OwnerPriorPhase::Active(RunStage::BackingOff { until: Wall::from_nanos(90) }), b"Prior retry at: 90"),
        (
            OwnerPriorPhase::Closing(ClosingStage::Run { attempt: u64::MAX }),
            b"Prior closing run attempt: 18446744073709551615",
        ),
        (OwnerPriorPhase::Closing(ClosingStage::Delegates), b"Prior closing delegates"),
        (OwnerPriorPhase::Closing(ClosingStage::Effects), b"Prior closing effects"),
        (OwnerPriorPhase::Closing(ClosingStage::Releases), b"Prior closing releases"),
        (OwnerPriorPhase::Closing(ClosingStage::Settled), b"Prior closing settled"),
    ];
    for (prior, label) in priors {
        value.hold.as_mut().unwrap().before = prior;
        let tree = build(&value, words()).0;
        assert!(has(&tree, label));
        assert!(!contains(&tree, b"Placed or adopted:"));
        assert!(!contains(&tree, b"Actual run start:"));
    }
    value.hold.as_mut().unwrap().escalation = Some(OwnerHoldEscalation::Routing);
    assert!(has(&build(&value, words()).0, b"Escalation routing"));
    for (holder, label) in [
        (OwnerEscalationHolder::Task(u64::MAX), b"Escalation holder task: 18446744073709551615".as_slice()),
        (OwnerEscalationHolder::Person(u64::MAX), b"Escalation holder person: 18446744073709551615"),
        (OwnerEscalationHolder::Role { project: u32::MAX, role: u32::MAX }, b"Escalation holder role: 4294967295"),
    ] {
        value.hold.as_mut().unwrap().escalation =
            Some(OwnerHoldEscalation::Waiting { holder, entry: u64::MAX, since: Wall::from_nanos(100) });
        let tree = build(&value, words()).0;
        assert!(has(&tree, label));
        assert!(has(&tree, b"Escalation entry: 18446744073709551615"));
        assert!(has(&tree, b"Escalation waiting since: 100"));
        match holder {
            OwnerEscalationHolder::Role { .. } => assert!(has(&tree, b"Escalation holder project: 4294967295")),
            OwnerEscalationHolder::Task(_) | OwnerEscalationHolder::Person(_) => {}
        }
    }
    value.hold.as_mut().unwrap().escalation =
        Some(OwnerHoldEscalation::Rejected { by: u64::MAX, reason: doc(UTF8_TEXTS[0], true) });
    let tree = build(&value, words()).0;
    assert!(has(&tree, b"Left held by: 18446744073709551615"));
    assert!(has(&tree, UTF8_TEXTS[0]));
    assert!(has(&tree, b"Supplied words incomplete"));
}
#[test]
fn person_wait_notices_keep_full_message_activity_and_independent_owner_times() {
    let mut value = details();
    value.person_wait = Some(OwnerPersonWait {
        since: Wall::from_nanos(110),
        stall: Duration::from_nanos(120),
        deadline: Wall::from_nanos(130),
        notice: Some(OwnerPersonStallNotice { message: u64::MAX, at: Wall::from_nanos(140), activity: u64::MAX }),
    });
    let tree = build(&value, words()).0;
    for label in [
        b"Person wait began: 110".as_slice(),
        b"Person stall span: 120",
        b"Person stall deadline: 130",
        b"Stall notice message: 18446744073709551615",
        b"Stall notice at: 140",
        b"Stall notice activity: 18446744073709551615",
    ] {
        assert!(has(&tree, label));
    }
}
#[test]
fn complete_chip_endings_keep_result_requester_and_each_independent_reason_loss() {
    let mut value = details();
    value.ending = Some(OwnerChipEnding::Done(ResultValue::Report(doc(b"whole result", true))));
    let tree = build(&value, words()).0;
    assert!(has(&tree, b"whole result"));
    assert!(has(&tree, b"Completed with result"));
    assert!(has(&tree, b"Only part of this text was supplied"));
    for (by, label) in [
        (OwnerRequester::Task(u64::MAX), b"Cancellation requester task: 18446744073709551615".as_slice()),
        (OwnerRequester::Person(u64::MAX), b"Cancellation requester person: 18446744073709551615"),
        (OwnerRequester::Deployment { project: u32::MAX }, b"Cancellation requester deployment project: 4294967295"),
    ] {
        value.ending = Some(OwnerChipEnding::Cancelled {
            reason: doc(b"reason", true),
            result: Some(ResultValue::Words(doc(b"prior result", false))),
            by,
            at: Wall::from_nanos(150),
        });
        let tree = build(&value, words()).0;
        assert!(has(&tree, label));
        assert!(has(&tree, b"Cancelled at: 150"));
        assert!(has(&tree, b"prior result"));
        assert!(has(&tree, b"Supplied words incomplete"));
        assert!(!contains(&tree, b"Ended at:"));
    }
    value.ending = Some(OwnerChipEnding::Failed(doc(b"terminal failure", false)));
    let tree = build(&value, words()).0;
    assert!(has(&tree, b"terminal failure"));
    assert!(!has(&tree, b"Supplied words incomplete"));
    value.ending = Some(OwnerChipEnding::Failed(doc(b"terminal failure", true)));
    assert!(has(&build(&value, words()).0, b"Supplied words incomplete"));
    value.ending = Some(OwnerChipEnding::Cancelled {
        reason: doc(b"reason", false),
        result: None,
        by: OwnerRequester::Task(7),
        at: Wall::from_nanos(150),
    });
    let tree = build(&value, words()).0;
    assert!(!has(&tree, b"prior result"));
    assert!(!has(&tree, b"Supplied words incomplete"));
    let mut absent_nodes = None;
    for explanation in [None, Some(doc(b"", false)), Some(doc(b"approval explanation", true))] {
        let supplied = explanation.is_some();
        let lost = explanation.as_ref().is_some_and(|value| value.cut);
        value.ending =
            Some(OwnerChipEnding::Done(ResultValue::Approved { state: copy_of(&[255, 0, 254]), words: explanation }));
        let tree = build(&value, words()).0;
        assert!(has(&tree, b"\\xFF\\x00\\xFE"));
        if supplied {
            assert_eq!(tree.nodes().len(), absent_nodes.unwrap() + if lost { 2 } else { 1 });
        } else {
            absent_nodes = Some(tree.nodes().len());
        }
    }
}

#[test]
fn exact_owner_verdict_fits_chip_result_fragment_at_full_field_capacity_and_rejects_whole_overflow() {
    let mut fields = List::with_capacity(client_limits().rows);
    for _ in 0..client_limits().rows {
        fields.push(ResultField { name: copy_of(&[255; 256]), value: copy_of(&[b'x'; 256]) }).unwrap();
    }
    let mut value = details();
    value.ending = Some(OwnerChipEnding::Done(ResultValue::OwnerVerdict {
        verdict: u32::MAX,
        words: doc(&[b'w'; 256], true),
        fields,
    }));
    let before = value.clone();
    let (tree, _, envelope) = build(&value, words());
    assert!(has(&tree, b"Verdict 4294967295"));
    assert!(has(&tree, b"Only part of this text was supplied"));
    assert!(has(&tree, &[b'w'; 256]));
    assert!(has(&tree, &[b'x'; 256]));
    assert!(u32::try_from(tree.nodes().len()).unwrap() <= envelope.nodes);
    assert!(
        tree.nodes().iter().map(|node| u64::from(crate::tests::text_bytes(node))).sum::<u64>()
            <= u64::from(envelope.text_bytes)
    );
    assert!(view::required(&value, &crate::Limits { rows: 3, ..client_limits() }, words()).is_none());
    assert!(view::required(&value, &crate::Limits { text: 255, ..client_limits() }, words()).is_none());
    assert_eq!(value, before);
}
#[test]
fn maximum_chip_fragment_checks_selected_language_storage_raw_slots_loss_and_arithmetic_bounds() {
    let mut value = details();
    value.hold = Some(hold());
    value.hold.as_mut().unwrap().source = Some(part());
    value.procedure = Some(part());
    value.resources = List::with_capacity(4);
    for _ in 0_u32..4 {
        value
            .resources
            .push(OwnerResourceLine {
                part: part(),
                kind: u16::MAX,
                taken: true,
                holder: Some(u64::MAX),
                writer: Some(OwnerResourceWriter::Run { task: u64::MAX, attempt: u64::MAX }),
                writer_lost: true,
            })
            .unwrap();
    }
    value.resources_cut = true;
    value.closing_connectors = Some(List::with_capacity(4));
    for _ in 0_u32..4 {
        value.closing_connectors.as_mut().unwrap().push(u16::MAX).unwrap();
    }
    value.hold.as_mut().unwrap().escalation =
        Some(OwnerHoldEscalation::Rejected { by: u64::MAX, reason: doc(UTF8_TEXTS[1], true) });
    value.observation = Some(OwnerChipObservation {
        channel: Token::new(u64::MAX),
        work: OwnerChipWork::Tool { call: copy_of(&[255; 256]), tool: copy_of(&[254; 256]) },
        since: Wall::from_nanos(160),
        reported_at: Wall::from_nanos(170),
        started: Some(Wall::from_nanos(180)),
    });
    value.ending = Some(OwnerChipEnding::Cancelled {
        reason: doc(UTF8_TEXTS[2], true),
        result: Some(ResultValue::Change { kind: u16::MAX, part: part(), words: doc(b"change", true) }),
        by: OwnerRequester::Deployment { project: u32::MAX },
        at: Wall::from_nanos(190),
    });
    value.ended_at = Some(Wall::from_nanos(200));
    let (tree, bindings, envelope) = build(&value, words());
    assert!(
        tree.nodes().iter().map(|node| u64::from(crate::tests::text_bytes(node))).sum::<u64>()
            <= u64::from(envelope.text_bytes)
    );
    assert_eq!(tree.nodes().iter().filter(|node| node.slot.is_some()).count(), 7);
    for node in tree.nodes() {
        if let Some(slot) = node.slot {
            assert!(bindings.owner_part(slot.token).is_some());
        }
    }
    assert!(view::required(&value, &crate::Limits { text: u32::MAX, ..client_limits() }, words()).is_none());
    assert!(view::required(&value, &crate::Limits { rows: u32::MAX, ..client_limits() }, words()).is_none());
    let smaller = crate::Limits { text: 1, ..client_limits() };
    let mut terminal = details();
    terminal.ending = Some(OwnerChipEnding::Done(ResultValue::Approved { state: copy_of(&[255, 0]), words: None }));
    assert!(view::required(&terminal, &smaller, words()).is_none());
    terminal.ending =
        Some(OwnerChipEnding::Done(ResultValue::Change { kind: 1, part: part(), words: doc(b"", false) }));
    assert!(view::required(&terminal, &smaller, words()).is_none());
    terminal.ending = Some(OwnerChipEnding::Done(ResultValue::Report(doc(b"too large", false))));
    assert!(view::required(&terminal, &smaller, words()).is_none());
    let mut translated = crate::phrases::english::language();
    for phrase in &mut translated.phrases {
        for form in &mut phrase.forms {
            for segment in &mut form.segments {
                if let Segment::Text(bytes) = segment {
                    *bytes = copy_of(b"selected ");
                }
            }
        }
    }
    let selected =
        OwnerChipWords::load(Box::new([crate::phrases::english::language(), translated]), &phrase_limits(), 1, 0)
            .unwrap();
    let selected_tree = build(&details(), selected.words()).0;
    assert_eq!(selected_tree.nodes()[0].role, skein_ui::Role::Region);
    assert_eq!(selected_tree.nodes()[0].name.as_deref(), Some(b"selected 4294967295".as_slice()));
    assert!(has(&selected_tree, b"selected 4294967295"));
    assert!(!has(&selected_tree, b"Run work unobserved"));
    let mut language = data::language();
    language.phrases[0].id = ChipPhrase::PhaseSince.id();
    assert_eq!(
        skein_ui::phrases::Phrasebook::load(data::shapes(), Box::new([language]), &phrase_limits()).err(),
        Some(Invalid::Duplicate { id: ChipPhrase::PhaseSince.id() })
    );
    assert_eq!(
        OwnerChipWords::load(Box::new([crate::phrases::english::language()]), &phrase_limits(), 1, 0).err(),
        Some(Invalid::Language)
    );
}

fn retry_activation_tree(retry: Option<RetryNotice>, words: &Words) -> Tree {
    let mut value = details();
    value.retry = retry;
    let f = view::required(&value, &client_limits(), words).unwrap();
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
    let client = Fixture::new(&client_limits());
    let chip = Chip {
        task: u64::MAX,
        title: doc(b"retry", false),
        executor: Executor::Agent { charter: u32::MAX },
        phase: Phase::Running,
        details: Box::new(ChipPhase::Active(ActiveRun {
            stage: RunStage::Claimed { attempt: u64::MAX },
            phase_since: Wall::from_nanos(10),
            attempt: Some(u64::MAX),
            placement: Some(Wall::from_nanos(20)),
            retry,
            observation: None,
        })),
    };
    let mut b = Builder::new(&capacity);
    let mut bindings = JigBindings::new(f.bindings, 256);
    crate::cards::chip::chip(&mut b, &client.view(), &chip, words, &mut bindings);
    let tree = b.finish();
    assert!(tree.len() <= f.nodes);
    assert!(tree.nodes().iter().all(|node| crate::tests::text_bytes(node) <= f.text_per_node));
    assert!(
        tree.nodes().iter().map(|node| u64::from(crate::tests::text_bytes(node))).sum::<u64>()
            <= u64::from(f.text_bytes)
    );
    tree
}

#[test]
fn owner_retry_keeps_all_six_classes_and_independent_zero_max_counts_in_chip_and_activation() {
    let classes = [
        (HoldFailure::Transient, b"Transient failure".as_slice()),
        (HoldFailure::Permanent, b"Permanent failure"),
        (HoldFailure::Run, b"Run failure"),
        (HoldFailure::Agent, b"Agent failure"),
        (HoldFailure::Lost, b"Lost failure"),
        (HoldFailure::Invalid, b"Invalid failure"),
    ];
    for (class, label) in classes {
        for (count, allowance, caption) in [
            (0, 0, b"Failure count 0 of allowance 0".as_slice()),
            (0, u32::MAX, b"Failure count 0 of allowance 4294967295"),
            (u32::MAX, 0, b"Failure count 4294967295 of allowance 0"),
            (u32::MAX, u32::MAX, b"Failure count 4294967295 of allowance 4294967295"),
        ] {
            let retry = Some(RetryNotice::OwnerFailed { class, count, allowance });
            let mut value = details();
            value.retry = retry;
            assert_eq!(value.clone().retry, retry);
            let tree = build(&value, words()).0;
            assert!(has(&tree, label));
            assert_eq!(classes.iter().filter(|(_, other)| has(&tree, other)).count(), 1);
            assert!(has(&tree, caption));
            assert!(has(&tree, b"Run work unobserved"));
            assert!(!contains(&tree, b"Actual run start:"));
            let tree = retry_activation_tree(retry, words());
            assert!(has(&tree, label));
            assert_eq!(classes.iter().filter(|(_, other)| has(&tree, other)).count(), 1);
            assert!(has(&tree, caption));
            assert!(contains(&tree, b"Run observation not supplied"));
            assert!(!contains(&tree, b"Start observed at"));
            assert_eq!(value.retry, retry, "rendering cannot mutate owner class or independent counts");
        }
    }
}

#[test]
fn owner_retry_absence_refusal_and_all_supplied_failure_classes_remain_independent() {
    let mut value = details();
    value.retry = None;
    for tree in [build(&value, words()).0, retry_activation_tree(None, words())] {
        assert!(!contains(&tree, b"Failure count"));
        assert!(!contains(&tree, b"Activation refusal count"));
    }
    for (count, caption) in
        [(0, b"Activation refusal count 0".as_slice()), (u32::MAX, b"Activation refusal count 4294967295")]
    {
        value.retry = Some(RetryNotice::Refused { count });
        for tree in [build(&value, words()).0, retry_activation_tree(value.retry, words())] {
            assert!(has(&tree, caption));
            assert!(!contains(&tree, b"Failure count"));
        }
    }
    for (class, label) in [
        (FailureClass::HostLost, b"Host lost".as_slice()),
        (FailureClass::Temporary, b"Temporary failure"),
        (FailureClass::Permanent, b"Permanent failure"),
        (FailureClass::Stall, b"Stalled"),
    ] {
        value.retry = Some(RetryNotice::Failed { class, count: 7, allowance: 9 });
        for tree in [build(&value, words()).0, retry_activation_tree(value.retry, words())] {
            assert!(has(&tree, label));
            assert!(has(&tree, b"Failure count 7 of allowance 9"));
            assert!(!contains(&tree, b"Activation refusal count"));
        }
    }
}

#[test]
fn owner_retry_borrows_selected_checked_six_class_words_and_both_count_holes() {
    let cases = [
        (HoldFailure::Transient, ChipPhrase::Transient, b"chosen transient".as_slice()),
        (HoldFailure::Permanent, ChipPhrase::Permanent, b"chosen permanent"),
        (HoldFailure::Run, ChipPhrase::RunFailure, b"chosen run"),
        (HoldFailure::Agent, ChipPhrase::AgentFailure, b"chosen agent"),
        (HoldFailure::Lost, ChipPhrase::Lost, b"chosen lost"),
        (HoldFailure::Invalid, ChipPhrase::InvalidFailure, b"chosen invalid"),
    ];
    let mut selected = crate::phrases::english::language();
    for phrase in &mut selected.phrases {
        for (_, class_phrase, label) in cases {
            if phrase.id == class_phrase.id() {
                phrase.forms[0].segments = Box::new([Segment::Text(copy_of(label))]);
            }
        }
        if phrase.id == crate::phrases::jig::JigPhrase::RunRetryFailed.id() {
            phrase.forms[0].segments = Box::new([
                Segment::Text(copy_of(b"chosen count ")),
                Segment::Hole { index: 0, kind: skein_ui::phrases::HoleKind::Number },
                Segment::Text(copy_of(b" versus ")),
                Segment::Hole { index: 1, kind: skein_ui::phrases::HoleKind::Number },
            ]);
        }
    }
    let checked =
        OwnerChipWords::load(Box::new([crate::phrases::english::language(), selected]), &phrase_limits(), 1, 0)
            .unwrap();
    for (class, _, label) in cases {
        let mut value = details();
        value.retry = Some(RetryNotice::OwnerFailed { class, count: 0, allowance: u32::MAX });
        for tree in [build(&value, checked.words()).0, retry_activation_tree(value.retry, checked.words())] {
            assert!(has(&tree, label));
            assert!(has(&tree, b"chosen count 0 versus 4294967295"));
            assert!(!contains(&tree, b"Failure count"));
        }
    }
}

#[test]
fn supplied_hold_account_and_origin_keep_distinct_public_slots_and_absence() {
    let account = OwnerPart { connector: 7, kind: OwnerPartKind::Hold, key: OwnerPartKey::Number(u64::MAX) };
    let origin = part();
    for supplied in [None, Some(account.clone())] {
        for source in [None, Some(origin.clone())] {
            let mut held = hold();
            held.account = supplied.clone();
            held.source = source.clone();
            held.reason = HoldCategory::Drift;
            let mut value = details();
            value.hold = Some(held);
            let (tree, bindings, envelope) = build(&value, words());
            let parts: Box<[_]> = tree
                .nodes()
                .iter()
                .filter_map(|node| {
                    node.slot.map(|slot| bindings.owner_part(slot.token).expect("exact owner part").clone())
                })
                .collect();
            let expected: Box<[_]> = supplied.iter().chain(source.iter()).cloned().collect();
            assert_eq!(parts, expected);
            assert_eq!(contains(&tree, b"Hold account reference"), supplied.is_some());
            assert_eq!(contains(&tree, b"\\xFF\\x00"), source.is_some());
            assert!(tree.len() <= envelope.nodes);
            assert_eq!(value.hold.as_ref().unwrap().account, supplied);
            assert_eq!(value.hold.as_ref().unwrap().source, source);
        }
    }
}

const UTF8_TEXTS: [&[u8]; 3] = [b"~", &[b'~'; 256], &[b'?'; 256]];

#[test]
fn a_long_resource_path_shows_its_tail_and_keeps_the_whole_slot() {
    let path = crate::tests::fixtures::long_resource_path::<256>();
    let reference = OwnerPart { connector: u16::MAX, kind: OwnerPartKind::Resource, key: OwnerPartKey::Path(path) };
    let mut value = details();
    value.procedure = Some(reference.clone());
    let (tree, bindings, _) = build(&value, words());
    assert!(has(&tree, "…".as_bytes()));
    assert!(contains(&tree, b"\\xF3"));
    assert!(!contains(&tree, b"\\xF0"));
    let slot = tree.nodes().iter().find_map(|node| node.slot).expect("whole resource slot");
    assert_eq!(bindings.owner_part(slot.token), Some(&reference));
}

#[test]
fn a_circle_hold_names_the_waits_in_order_and_lifts_on_its_own() {
    let mut value = details();
    let mut held = hold();
    held.reason = HoldCategory::Circle;
    let mut tasks = List::with_capacity(3);
    tasks.push(7).unwrap();
    tasks.push(5).unwrap();
    tasks.push(9).unwrap();
    held.circle = Some(Circle { tasks, cut: true });
    value.hold = Some(held);
    let (tree, _, _) = build(&value, words());
    assert!(has(&tree, b"Held for a circle of waits"));
    assert!(has(&tree, b"This hold lifts on its own once the circle breaks"));
    let expected = [b"waits for T7".as_slice(), b"waits for T5", b"waits for T9"];
    let mut shown = 0;
    for node in tree.nodes() {
        for run in &node.text {
            if run.text.starts_with(b"waits for T") {
                assert_eq!(run.text.as_ref(), *expected.get(shown).unwrap());
                shown += 1;
            }
        }
    }
    assert_eq!(shown, expected.len());
}

#[test]
fn an_effect_or_procedure_hold_card_shows_its_retained_account() {
    let mut value = details();
    let mut held = hold();
    held.words = Some(doc(b"restart on production/checkout: permission refused; amend authority and release", false));
    value.hold = Some(held);
    let (tree, _, _) = build(&value, words());
    assert!(contains(&tree, b"restart on production/checkout: permission refused; amend authority and release"));
}
