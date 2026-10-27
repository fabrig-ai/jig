use jig_client::contract::*;
use skein_lib::{List, Wall};

pub(super) fn details() -> OwnerChipDetails {
    OwnerChipDetails {
        dependencies: List::with_capacity(0),
        project: u32::MAX,
        executor: OwnerExecutor::Person(PersonAddress::Role(u32::MAX)),
        phase_since: Wall::from_nanos(10),
        run_since: None,
        attempt: None,
        retry: None,
        wake: OwnerWakePolicy {
            markers: 0,
            words: WakeRule::Never,
            notices: WakeRule::Never,
            news: WakeRule::Never,
            results: ResultsWake::Never,
            questions: false,
            answers: false,
            timers: false,
        },
        person_wait: None,
        hold: None,
        budget: OwnerBudget {
            numbers: OwnerNumbers { budget: 9, spent: 1, spent_below: 2, reserved: 3 },
            left: 4,
            funder: Funder::Period { project: u32::MAX, period: u64::MAX },
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
#[test]
fn folded_generic_endings_keep_unknown_outcome_separate_and_retain_whole_visible_chip() {
    let mut limits = super::spine::LIMITS;
    limits.rows = 1;
    let mut domain = crate::tests::fixtures::Fixture::new(&limits);
    let mut nodes = List::with_capacity(3);
    for (task, phase) in [(1, Phase::Active), (2, Phase::Ended), (3, Phase::Failed)] {
        let details = if phase == Phase::Failed {
            super::objects::phase_details(Phase::Failed)
        } else {
            ChipPhase::Owner {
                phase: if phase == Phase::Active {
                    OwnerChipPhase::Person {
                        address: PersonAddress::Role(u32::MAX),
                        taken_by: Some(u64::MAX),
                        since: None,
                    }
                } else {
                    OwnerChipPhase::Ended
                },
                details: Box::new(details()),
            }
        };
        nodes
            .push(TreeNode {
                chip: Chip {
                    task,
                    title: DocumentText { bytes: super::objects::text(b"partial"), cut: true },
                    executor: Executor::Role { project: u32::MAX, role: u32::MAX },
                    phase,
                    details: Box::new(details),
                },
                parent: None,
                dependencies: List::with_capacity(0),
                proposals: List::with_capacity(0),
                spend: 0,
                folded: PhaseCounts {
                    ended: 0,
                    waiting: 0,
                    running: 0,
                    held: 0,
                    closing: 0,
                    done: 0,
                    failed: 0,
                    cancelled: 0,
                },
                held_below: 0,
            })
            .unwrap();
    }
    let watch = domain.request_watch(Subject::Tree { task: 1, window: 1 }).unwrap();
    domain.drive(
        skein_lib::Time::ZERO,
        crate::tests::fixtures::Watched {
            watch,
            event: WatchEvent::Delivered(Delivery::Tree(Box::new(Tree {
                task: 1,
                nodes,
                folded: PhaseCounts {
                    ended: 0,
                    waiting: 0,
                    running: 0,
                    held: 0,
                    closing: 0,
                    done: 0,
                    failed: 0,
                    cancelled: 0,
                },
                held_below: 0,
            }))),
        },
    );
    let plan = &domain.task_page().plan;
    assert_eq!(plan.nodes.len(), 1);
    assert_eq!(plan.folded.ended, 1);
    assert_eq!(plan.folded.failed, 1);
    assert_eq!(plan.folded.done, 0);
    assert_eq!(plan.folded.cancelled, 0);
    let chip = &plan.nodes.first().unwrap().chip;
    assert_eq!(chip.title.bytes(), b"partial");
    assert_eq!(chip.title.whole(), None);
    let ChipPhase::Owner { phase, details } = chip.details.as_ref() else {
        panic!("the whole supplied owner chip is retained")
    };
    assert_eq!(
        phase,
        &OwnerChipPhase::Person { address: PersonAddress::Role(u32::MAX), taken_by: Some(u64::MAX), since: None }
    );
    assert_eq!(details.project, u32::MAX);
    assert_eq!(details.ending, None);
    assert_eq!(details.ended_at, None);
    assert_eq!(details.observation, None);
    assert_eq!(details.executor, OwnerExecutor::Person(PersonAddress::Role(u32::MAX)));
}
