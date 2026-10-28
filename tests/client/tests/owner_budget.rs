use jig_client_domain::contract::{Funder, OwnerBudget, OwnerNumbers, PoolLedger};
use jig_client_scripted_engine::{Bounds, owner_budget as price};
use skein_lib::Wall;
use skein_world::domain::heap::{Counting, Meter};
#[global_allocator]
static HEAP: Counting = Counting;

fn sources() -> [Funder; 4] {
    [
        Funder::Task(u64::MAX),
        Funder::Pool { project: u32::MAX, person: u64::MAX - 1, period: u64::MAX - 2 },
        Funder::Recurring { project: u32::MAX - 1, task: u64::MAX - 3, period: u64::MAX - 4 },
        Funder::Period { project: u32::MAX - 2, period: u64::MAX - 5 },
    ]
}
fn numbers() -> OwnerNumbers {
    OwnerNumbers { budget: u64::MAX, spent: u64::MAX - 1, spent_below: u64::MAX - 2, reserved: u64::MAX - 3 }
}
fn bounds() -> Bounds {
    Bounds { rows: 0, text: 0, document_bytes: u64::MAX, ..Bounds::default() }
}
#[test]
fn every_exact_budget_funder_and_deadline_prices_as_one_inline_value() {
    for funder in sources() {
        for deadline in [None, Some(Wall::from_nanos(u64::MAX))] {
            let original = OwnerBudget { numbers: numbers(), left: u64::MAX - 4, funder, deadline };
            let meter = Meter::new();
            let value = original.clone();
            assert_eq!(meter.held(), 0, "every source and optional deadline is inline");
            let bytes = u64::try_from(core::mem::size_of::<OwnerBudget>()).expect("size");
            assert_eq!(price::bytes(&value, &bounds()), Some(bytes));
            assert_eq!(price::budget_worst_case(), Some(bytes));
            let mut exact = bounds();
            exact.document_bytes = bytes;
            assert!(price::fits(&value, &exact));
            exact.document_bytes -= 1;
            assert!(!price::fits(&value, &exact));
            assert_eq!(value, original, "whole refusal preserves owner facts and exact identity");
            assert_eq!(meter.held(), 0, "pricing and admission borrow without allocation");
        }
    }
}
#[test]
fn ledger_parent_state_and_period_options_survive_whole_value_pricing() {
    for source in sources() {
        for parent in [None, Some(sources()[0]), Some(sources()[1]), Some(sources()[2]), Some(sources()[3])] {
            for closed in [false, true] {
                for began in [None, Some(Wall::EPOCH)] {
                    for next in [None, Some(Wall::from_nanos(u64::MAX))] {
                        let original = PoolLedger { source, parent, numbers: numbers(), left: 17, closed, began, next };
                        let meter = Meter::new();
                        let value = original.clone();
                        assert_eq!(meter.held(), 0, "all ledger fields remain inline");
                        let bytes = u64::try_from(core::mem::size_of::<PoolLedger>()).expect("size");
                        assert_eq!(price::ledger_bytes(&value, &bounds()), Some(bytes));
                        assert_eq!(price::ledger_worst_case(), Some(bytes));
                        let mut exact = bounds();
                        exact.document_bytes = bytes;
                        assert!(price::ledger_fits(&value, &exact));
                        exact.document_bytes -= 1;
                        assert!(!price::ledger_fits(&value, &exact));
                        assert_eq!(value, original, "no actor substitution, inferred period or recalculated left");
                        assert_eq!(meter.held(), 0);
                    }
                }
            }
        }
    }
}
#[test]
fn maximal_budget_and_ledger_fragments_fit_the_tree_envelope() {
    use jig_client_view::owner_budget as view;
    use jig_client_view::owner_budget_words::{self as data, OwnerBudgetWords};
    use skein_ui::Builder;
    use skein_ui::Limits;
    use skein_ui::phrases::PhraseLimits;

    let language_limits = PhraseLimits {
        languages: 1,
        phrases: data::standalone_phrase_count(),
        holes: 4,
        segments: 200,
        text_bytes: 8192,
        hole_bytes: 256,
    };
    let words = OwnerBudgetWords::load(Box::new([data::standalone_language()]), &language_limits, 0, 0)
        .expect("whole checked budget and shared Funder sayings");

    let budget = OwnerBudget {
        numbers: numbers(),
        left: u64::MAX - 4,
        funder: sources()[1],
        deadline: Some(Wall::from_nanos(u64::MAX)),
    };
    let ledger = PoolLedger {
        source: sources()[2],
        parent: Some(sources()[1]),
        numbers: numbers(),
        left: u64::MAX - 4,
        closed: true,
        began: Some(Wall::from_nanos(u64::MAX - 1)),
        next: Some(Wall::from_nanos(u64::MAX)),
    };
    for is_ledger in [false, true] {
        let envelope = if is_ledger {
            view::ledger_worst_case(words.words()).expect("ledger envelope")
        } else {
            view::worst_case(words.words()).expect("budget envelope")
        };
        let required = if is_ledger {
            view::ledger_required(&ledger, words.words()).expect("ledger fits")
        } else {
            view::required(&budget, words.words()).expect("budget fits")
        };
        assert_eq!(required.nodes, if is_ledger { 13 } else { 9 });
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
        if is_ledger {
            view::ledger(&mut builder, &ledger, words.words());
        } else {
            view::render(&mut builder, &budget, words.words());
        }
        let tree = builder.finish();
        assert_eq!(tree.len(), required.nodes);
        for exact in [
            &b"Budget: 18446744073709551615"[..],
            &b"Spent directly: 18446744073709551614"[..],
            &b"Settled spend below: 18446744073709551613"[..],
            &b"Reserved for funded work: 18446744073709551612"[..],
            &b"Left: 18446744073709551611"[..],
        ] {
            assert!(
                tree.nodes().iter().any(|node| node.text.iter().any(|run| run.text.as_ref() == exact)),
                "all owner accounting values retain their full width in both fragments"
            );
        }
    }
}
