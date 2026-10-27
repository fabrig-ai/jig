#![expect(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros,
    reason = "ordinary fixtures inspect public semantic trees"
)]
use super::{
    cards::{CLIENT, contains, limits, words},
    work_pages::list,
};
use crate::{JigBindings, proposed_task};
use skein_ui::{Builder, Role, Tree};

use jig_client::contract::*;

fn builder() -> Builder {
    let mut limits = limits();
    limits.nodes = 512;
    limits.depth = 16;
    let mut builder = Builder::new(&limits);
    builder.open(Role::Page);
    builder
}
fn finish(mut builder: Builder) -> Tree {
    builder.close();
    builder.finish()
}

pub(super) fn full_template(number: u64) -> ProposedTask {
    let bytes: Box<[u8]> = vec![0xff; usize::try_from(CLIENT.text).unwrap()].into_boxed_slice();
    let pattern =
        OwnerPattern { segments: list([bytes.clone(), bytes.clone()]), last: OwnerPatternEnd::Open(bytes.clone()) };
    let grant = OwnerGrant { connector: u16::MAX, kind: u16::MAX, pattern: pattern.clone() };
    let note = OwnerResourceScope { connector: u16::MAX, pattern };
    let resource = OwnerPart {
        connector: u16::MAX,
        kind: OwnerPartKind::Resource,
        key: OwnerPartKey::Path(list([bytes.clone(), bytes.clone()])),
    };
    let field = FieldRule { name: bytes, max: u32::MAX };
    let choice =
        VerdictChoice { code: u32::MAX, words: u32::MAX, followups: u32::MAX, fields: list([field.clone(), field]) };
    let mut task = super::work_pages::recurring_task(number, &vec![b'x'; usize::try_from(CLIENT.text).unwrap()]);
    task.spec.specification.words.cut = true;
    task.spec.specification.parameters = list([
        ParameterDocument::Bytes { name: u32::MAX, value: task.spec.specification.words.clone() },
        ParameterDocument::Bytes { name: 2, value: task.spec.specification.words.clone() },
    ]);
    task.spec.specification.inputs = list([u64::MAX, 9]);
    task.spec.cut = true;
    task.executor = OwnerExecutor::Procedure { connector: u16::MAX, code: u32::MAX };
    task.contract = TaskContractDocument::Verdict {
        choices: list([choice.clone(), VerdictChoice { code: 2, ..choice }]),
        cut: true,
    };
    task.authority = OwnerAuthorityDocument {
        authority: OwnerAuthority {
            models: None,
            tools: u64::MAX,
            grants: list([grant.clone(), grant]),
            delegation: OwnerDelegation {
                kinds: list([OwnerAuthorityExecutor::Procedure(u32::MAX), OwnerAuthorityExecutor::Role(u32::MAX)]),
                tasks: u32::MAX,
                depth: u32::MAX,
            },
            spend: u64::MAX,
            deadline: Some(skein_lib::Wall::from_nanos(u64::MAX)),
            notes: u8::MAX,
            note_resources: list([note.clone(), note]),
        },
        cut: true,
    };
    task.budget = u64::MAX;
    task.funder = Funder::Pool { project: u32::MAX, person: u64::MAX, period: u64::MAX };
    task.dependencies = list([u64::MAX, 9]);
    task.resources = list([resource.clone(), resource]);
    task.wake = OwnerWakePolicy {
        markers: 0,
        words: WakeRule::Batch { count: u32::MAX, age: skein_lib::Duration::from_nanos(u64::MAX) },
        notices: WakeRule::Batch { count: u32::MAX, age: skein_lib::Duration::from_nanos(u64::MAX) },
        news: WakeRule::Batch { count: u32::MAX, age: skein_lib::Duration::from_nanos(u64::MAX) },
        results: ResultsWake::LastOrFailure,
        questions: true,
        answers: true,
        timers: true,
    };
    task.priority = Some(u32::MAX);
    task.cut = true;
    task
}

#[test]
fn exact_part_slots_keep_separate_literal_segments_all_owner_kinds_and_numeric_keys() {
    let words = words();
    for kind in [
        OwnerPartKind::Resource,
        OwnerPartKind::Pool,
        OwnerPartKind::Result,
        OwnerPartKind::Procedure,
        OwnerPartKind::Effect,
        OwnerPartKind::Outbox,
    ] {
        for key in [
            OwnerPartKey::Path(list([b"a/b".as_slice().into(), b"c\xff".as_slice().into()])),
            OwnerPartKey::Path(list([b"a".as_slice().into(), b"b/c".as_slice().into()])),
            OwnerPartKey::Number(u64::MAX),
        ] {
            let reference = OwnerPart { connector: u16::MAX, kind, key };
            let mut b = builder();
            let mut bindings = JigBindings::new(16, CLIENT.text);
            proposed_task::part(&mut b, &reference, words, &mut bindings);
            let tree = finish(b);
            let slot = tree.nodes().iter().find_map(|node| node.slot).unwrap();
            assert_eq!(slot.owner, (crate::Owner::Application { number: u16::MAX }).opaque());
            assert_eq!(bindings.owner_part(slot.token), Some(&reference));
            assert_eq!(bindings.part(slot.token), None);
            assert_eq!(crate::decode(&mut bindings, slot.token, skein_ui::Act::Press), None);
            match &reference.key {
                OwnerPartKey::Path(segments) => {
                    assert_eq!(segments.len(), 2);
                    if segments.get(1).unwrap().as_ref() == b"c\xff" {
                        assert!(contains(&tree, b"a/b"));
                        assert!(contains(&tree, b"\\x63\\xFF"));
                        assert!(!contains(&tree, b"a/b/c"));
                    } else {
                        assert!(contains(&tree, b"b/c"));
                    }
                }
                OwnerPartKey::Number(number) => {
                    assert_eq!(*number, u64::MAX);
                    assert!(contains(&tree, b"Part number 18446744073709551615"));
                }
            }
            bindings.begin();
            assert_eq!(bindings.owner_part(slot.token), None, "a departed tree's exact part token is retired");
            assert_eq!(crate::decode(&mut bindings, slot.token, skein_ui::Act::Press), None);
        }
    }
}

#[test]
fn proposed_executor_and_funder_keep_full_width_codes_and_original_period_identity() {
    let words = words();
    let mut b = builder();
    for executor in [
        OwnerExecutor::Agent { charter: u32::MAX },
        OwnerExecutor::Procedure { connector: u16::MAX, code: u32::MAX },
        OwnerExecutor::Person(PersonAddress::Party(u64::MAX)),
        OwnerExecutor::Person(PersonAddress::Role(u32::MAX)),
    ] {
        proposed_task::executor(&mut b, executor, words);
    }
    for funder in [
        Funder::Task(u64::MAX),
        Funder::Pool { project: u32::MAX, person: u64::MAX, period: 83 },
        Funder::Recurring { project: u32::MAX, task: u64::MAX, period: 71 },
        Funder::Period { project: u32::MAX, period: 59 },
    ] {
        proposed_task::funder(&mut b, funder, words);
    }
    let tree = finish(b);
    assert_eq!(tree.len(), 5 + proposed_task::funder_nodes() * 4);
    let actual = tree.nodes().iter().skip(5).flat_map(|node| &node.text).map(|run| run.text.len()).max().unwrap();
    assert!(usize::try_from(proposed_task::funder_phrase_bytes(words).unwrap()).unwrap() >= actual);
    assert!(contains(&tree, b"Agent charter 4294967295"));
    assert!(contains(&tree, b"Procedure connector 65535, code 4294967295"));
    assert!(contains(&tree, b"Person 18446744073709551615"));
    assert!(contains(&tree, b"Project role 4294967295"));
    assert!(contains(&tree, b"Funded by task 18446744073709551615"));
    assert!(contains(&tree, b"Funded by pool in project 4294967295, person 18446744073709551615, period 83"));
    assert!(contains(&tree, b"Funded by recurring task in project 4294967295, task 18446744073709551615, period 71"));
    assert!(contains(&tree, b"Funded by project period 4294967295, period 59"));
}

#[test]
fn exact_contracts_keep_approval_state_verdict_fields_and_each_result_bound() {
    let words = words();
    let mut b = builder();
    let mut bindings = JigBindings::new(16, CLIENT.text);
    for contract in [
        TaskContractDocument::Approval {
            state: b"opaque\xff".as_slice().into(),
            part: OwnerPart { connector: 9, kind: OwnerPartKind::Resource, key: OwnerPartKey::Number(83) },
        },
        TaskContractDocument::Report { words: u32::MAX },
        TaskContractDocument::Verdict {
            choices: list([
                VerdictChoice {
                    code: u32::MAX,
                    words: 83,
                    followups: 71,
                    fields: list([FieldRule { name: b"field/name".as_slice().into(), max: 59 }]),
                },
                VerdictChoice {
                    code: 31,
                    words: 29,
                    followups: 23,
                    fields: list([FieldRule { name: b"last\xff".as_slice().into(), max: 19 }]),
                },
            ]),
            cut: true,
        },
        TaskContractDocument::Change { connector: u16::MAX, kind: u16::MAX, words: u32::MAX },
    ] {
        proposed_task::contract(&mut b, &contract, words, &mut bindings);
    }
    let tree = finish(b);
    assert!(contains(&tree, b"Approval of exact state"));
    assert!(contains(&tree, b"\\x6F\\x70\\x61\\x71\\x75\\x65\\xFF"));
    assert!(contains(&tree, b"Report word byte limit 4294967295"));
    assert!(contains(&tree, b"Verdict choice 4294967295, word byte limit 83, follow-up limit 71"));
    assert!(contains(&tree, b"Verdict choice 31, word byte limit 29, follow-up limit 23"));
    assert!(contains(&tree, b"field/name"));
    assert!(contains(&tree, b"Field value byte limit 59"));
    assert!(contains(&tree, b"\\x6C\\x61\\x73\\x74\\xFF"));
    assert!(contains(&tree, b"Field value byte limit 19"));
    assert!(contains(&tree, b"Some contract choices were omitted"));
    assert!(contains(&tree, b"Change contract 65535, kind 65535, word byte limit 4294967295"));
}

#[test]
fn normal_words_adjust_extreme_public_offsets_by_saturating_the_wall_instant() {
    let mut words = words().clone();
    for offset in [i32::MAX, i32::MIN] {
        words.set_offset_minutes(offset);
        for instant in [skein_lib::Wall::EPOCH, skein_lib::Wall::from_nanos(u64::MAX)] {
            let expected = if offset > 0_i32 { u64::MAX } else { 0 };
            assert_eq!(words.adjusted_wall(instant).as_nanos(), expected);
        }
    }
    words.set_offset_minutes(60_i32);
    assert_eq!(words.adjusted_wall(skein_lib::Wall::EPOCH).as_nanos(), 3_600_000_000_000);
    words.set_offset_minutes(-60_i32);
    assert_eq!(words.adjusted_wall(skein_lib::Wall::from_nanos(7_200_000_000_000)).as_nanos(), 3_600_000_000_000);
}

fn selected_owner_language() -> skein_ui::phrases::Language {
    use crate::phrases::english;
    use skein_ui::phrases::Segment;
    let mut selected = english::language();
    for phrase in &mut selected.phrases {
        if phrase.id.number >= 3000
            || [
                jig_client::words::DecisionWord::FunderTask,
                jig_client::words::DecisionWord::FunderPool,
                jig_client::words::DecisionWord::FunderRecurring,
                jig_client::words::DecisionWord::FunderPeriod,
            ]
            .iter()
            .any(|word| jig_client::words::Word::Decision(*word).id() == phrase.id)
        {
            for form in &mut phrase.forms {
                for segment in &mut form.segments {
                    match segment {
                        Segment::Text(_) => {
                            *segment = Segment::Text(b"Selected owner fact: ".as_slice().into());
                            break;
                        }
                        Segment::Hole { .. } => {}
                    }
                }
            }
        }
    }
    let mut long = vec![b'L'; 1024];
    long.extend_from_slice(b": ");
    selected
        .phrases
        .iter_mut()
        .find(|phrase| phrase.id == crate::proposed_task_words::ProposedPhrase::ExecutorCharter.id())
        .unwrap()
        .forms[0]
        .segments[0] = Segment::Text(long.into_boxed_slice());
    selected
}
#[test]
fn normal_phrasebooks_require_and_render_exact_owner_words_in_the_selected_language() {
    use crate::phrases::english;
    use crate::phrases::jig::shapes;
    use skein_ui::phrases::{PhraseLimits, Phrasebook};
    let selected = selected_owner_language();
    let limits = PhraseLimits {
        languages: 2,
        phrases: crate::owner_words::phrase_count(),
        holes: 4,
        segments: 5000,
        text_bytes: 200_000,
        hole_bytes: CLIENT.text,
    };
    let mut incomplete = selected_owner_language();
    incomplete.phrases = incomplete
        .phrases
        .into_vec()
        .into_iter()
        .filter(|phrase| phrase.id != crate::owner_wake_words::WakePhrase::Policy.id())
        .collect::<Vec<_>>()
        .into_boxed_slice();
    Phrasebook::load(shapes(CLIENT.text), Box::new([english::language(), incomplete]), &limits).unwrap_err();
    let words = crate::words::Words::new(
        Phrasebook::load(shapes(CLIENT.text), Box::new([english::language(), selected]), &limits).unwrap(),
        1,
        60,
    );
    let limits = crate::limits::of_words(&CLIENT, &words).unwrap();
    assert!(limits.text_per_node >= 2048, "owner saying capacities contribute to normal tree limits");
    let mut b = Builder::new(&limits);
    b.open(Role::Page);
    let mut bindings = JigBindings::new(limits.bindings, CLIENT.text);
    let task = super::work_pages::recurring_task(u64::MAX, b"whole template words");
    crate::cards::proposal::proposed_task(&mut b, &task, &words, &mut bindings, CLIENT.rows);
    b.close();
    let tree = b.finish();
    assert!(contains(&tree, b"whole template words"));
    assert!(contains(&tree, b"Selected owner fact: "));
    assert!(contains(&tree, &[b'L'; 1024]));
    assert!(!contains(&tree, b"Wake policy"));
    assert!(!contains(&tree, b"Funded by recurring task"));
    assert!(!contains(&tree, b"Agent charter"));
    assert!(!contains(&tree, b"Tool bits: "));
}

#[test]
fn a_proposed_span_is_distinct_from_no_deadline() {
    let words = words();
    for (selector, expected) in
        [(NewDeadline::Named, false), (NewDeadline::Span(skein_lib::Duration::from_secs(120)), true)]
    {
        let mut task = super::work_pages::recurring_task(1, b"work");
        task.deadline = selector;
        let mut b = builder();
        proposed_task::authority(&mut b, &task, words, CLIENT.text, CLIENT.rows);
        let tree = finish(b);
        assert_eq!(contains(&tree, b"Deadline after acceptance (minutes): 2"), expected, "{selector:?}");
    }
}
