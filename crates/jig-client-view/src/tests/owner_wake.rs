#![expect(clippy::disallowed_methods, reason = "ordinary tests inspect complete semantic trees")]
use crate::owner_wake_words::WakePhrase;
use crate::{owner_wake, owner_wake_words};
use skein_ui::phrases::{
    Form, HoleKind, Language, Phrase, PhraseLimits, Phrasebook, PluralClass, PluralRule, Segment, SpanUnit, WallFormat,
};
use skein_ui::{Builder, Role, Tree};

use jig_client::contract::{OwnerWakePolicy, ResultsWake, Subscription, SubscriptionKind, Subscriptions, WakeRule};
use skein_lib::{Duration, List, Wall};

fn builder(nodes: u32, book: &Phrasebook) -> Builder {
    let bytes = owner_wake::phrase_bytes(book).unwrap();
    Builder::new(&skein_ui::Limits {
        nodes: nodes.checked_add(1).unwrap(),
        depth: 4,
        text_per_node: bytes,
        text_bytes: nodes.checked_add(1).unwrap().checked_mul(bytes).unwrap(),
        drawing_bytes: 0,
        styles_per_node: 0,
        patches: 0,
        bindings: 0,
        slots: 0,
        markdown_depth: 0,
        scene_marks: 0,
        scene_bytes: 0,
    })
}
fn contains(tree: &Tree, needle: &[u8]) -> bool {
    tree.nodes().iter().any(|n| n.text.iter().any(|r| r.text.windows(needle.len()).any(|w| w == needle)))
}
#[test]
fn owner_wake_renderer_keeps_every_rule_result_and_independent_switch() {
    let book = owner_wake_words::english().unwrap();
    for (rule, label) in [
        (WakeRule::Never, b"Does not wake".as_slice()),
        (WakeRule::Immediate, b"Wakes immediately".as_slice()),
        (
            WakeRule::Batch { count: u32::MAX, age: Duration::from_nanos(u64::MAX) },
            b"Wakes after 4294967295 messages or 18446744073709551615 ns of oldest message age".as_slice(),
        ),
    ] {
        for (results, label_results) in [
            (ResultsWake::Never, b"Delegate results do not wake".as_slice()),
            (ResultsWake::Each, b"Each delegate result wakes".as_slice()),
            (ResultsWake::LastOrFailure, b"The last delegate result or a failure wakes".as_slice()),
        ] {
            for switches in 0_u8..8 {
                let value = OwnerWakePolicy {
                    markers: 0,
                    words: rule,
                    notices: rule,
                    news: rule,
                    results,
                    questions: switches & 1 != 0,
                    answers: switches & 2 != 0,
                    timers: switches & 4 != 0,
                };
                let mut b = builder(owner_wake::policy_nodes(), &book);
                b.open(Role::Page);
                owner_wake::policy(&mut b, &value, &book, 0);
                b.close();
                let tree = b.finish();
                assert_eq!(tree.len(), owner_wake::policy_nodes() + 1);
                for channel in [b"Words".as_slice(), b"Notices", b"News"] {
                    assert!(contains(&tree, channel));
                }
                assert!(contains(&tree, label));
                assert!(contains(&tree, label_results));
                assert!(contains(&tree, if value.questions { b"Questions wake" } else { b"Questions do not wake" }));
                assert!(contains(&tree, if value.answers { b"Answers wake" } else { b"Answers do not wake" }));
                assert!(contains(&tree, if value.timers { b"Timers wake" } else { b"Timers do not wake" }));
            }
        }
    }
}
#[test]
fn owner_subscriptions_renderer_retains_numbers_task_flags_timer_absence_and_topic_pair() {
    let book = owner_wake_words::english().unwrap();
    let mut entries = List::with_capacity(7);
    for kind in [
        SubscriptionKind::Task { target: u64::MAX, held: false, result: false },
        SubscriptionKind::Task { target: 2, held: true, result: false },
        SubscriptionKind::Task { target: 3, held: false, result: true },
        SubscriptionKind::Task { target: 4, held: true, result: true },
        SubscriptionKind::Timer { at: Wall::from_nanos(u64::MAX), period: None },
        SubscriptionKind::Timer { at: Wall::EPOCH, period: Some(Duration::from_nanos(u64::MAX)) },
        SubscriptionKind::Topic { connector: u16::MAX, topic: u64::MAX },
    ] {
        entries.push(Subscription { number: u64::MAX, kind }).unwrap();
    }
    let nodes = owner_wake::subscription_nodes(7).unwrap();
    let mut b = builder(nodes, &book);
    b.open(Role::Page);
    owner_wake::subscriptions(&mut b, &Subscriptions { entries }, &book, 0);
    b.close();
    let tree = b.finish();
    assert!(tree.len() <= nodes + 1);
    for expected in [
        b"Subscription 18446744073709551615".as_slice(),
        b"Task 18446744073709551615",
        b"Held state is subscribed",
        b"Held state is not subscribed",
        b"Result is subscribed",
        b"Result is not subscribed",
        b"Timer at 18446744073709551615 ns since epoch",
        b"Repeats every 18446744073709551615 ns",
        b"Fires once",
        b"Connector 65535, topic 18446744073709551615",
    ] {
        assert!(contains(&tree, expected));
    }
    assert!(owner_wake::subscription_nodes(u32::MAX).is_none());
    let mut b = builder(owner_wake::subscription_nodes(0).unwrap(), &book);
    b.open(Role::Page);
    owner_wake::subscriptions(&mut b, &Subscriptions { entries: List::with_capacity(0) }, &book, 0);
    b.close();
    assert_eq!(b.finish().len(), 2);
}
#[test]
fn owner_wake_renderer_uses_checked_selected_language_with_reordered_holes() {
    let english = owner_wake_words::language();
    let mut second = owner_wake_words::language();
    for phrase in &mut second.phrases {
        if phrase.id == WakePhrase::Batch.id() {
            phrase.forms = Box::new([Form {
                class: PluralClass::Other,
                segments: Box::new([
                    Segment::Text(b"Age ".as_slice().into()),
                    Segment::Hole { index: 1, kind: HoleKind::Number },
                    Segment::Text(b"; count ".as_slice().into()),
                    Segment::Hole { index: 0, kind: HoleKind::Number },
                ]),
            }]);
        }
    }
    let book = Phrasebook::load(
        owner_wake_words::shapes(),
        Box::new([english, second]),
        &PhraseLimits { languages: 2, phrases: 27, holes: 2, segments: 200, text_bytes: 4000, hole_bytes: 0 },
    )
    .unwrap();
    let value = OwnerWakePolicy {
        markers: 0,
        words: WakeRule::Never,
        notices: WakeRule::Immediate,
        news: WakeRule::Batch { count: 7, age: Duration::from_nanos(13) },
        results: ResultsWake::Each,
        questions: true,
        answers: false,
        timers: true,
    };
    let mut b = builder(owner_wake::policy_nodes(), &book);
    b.open(Role::Page);
    owner_wake::policy(&mut b, &value, &book, 1);
    b.close();
    let tree = b.finish();
    assert!(contains(&tree, b"Age 13; count 7"));
    assert!(!contains(&tree, b"Wakes after"));
    // Missing phrases fail before they reach the renderer.
    let absent = Language {
        phrases: Box::<[Phrase]>::default(),
        plural: PluralRule::English,
        span_unit: SpanUnit::Nanos,
        wall: WallFormat::Nanos,
    };
    assert_eq!(
        Phrasebook::load(
            owner_wake_words::shapes(),
            Box::new([absent]),
            &PhraseLimits { languages: 1, phrases: 27, holes: 2, segments: 100, text_bytes: 2000, hole_bytes: 0 }
        )
        .unwrap_err(),
        skein_ui::phrases::Invalid::Missing { id: WakePhrase::Policy.id() }
    );
}
