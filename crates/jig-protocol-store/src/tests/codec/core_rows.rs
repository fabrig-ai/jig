//! Every core row and the transcript frame body (protocol/store.md, 5.1, 8.3).
use super::{call_limits, core_round_trip};
use crate::codec::core as codec;
use crate::tests::fixtures::core_rows as fixtures;
use jig_domain_core as core;
use jig_domain_tasks as tasks;

fn the_deployment_header_reads_back_with_its_id_and_projects_mark() {
    core_round_trip(core::CoreRecord::Deployment(fixtures::deployment()));
}
fn a_run_proof_with_its_terminal_reads_back() {
    core_round_trip(core::CoreRecord::RunProof(fixtures::run()));
}
fn a_closed_window_flag_reads_back_as_stored() {
    let mut run = fixtures::run();
    run.window.as_mut().expect("fixture window").closed = true;
    core_round_trip(core::CoreRecord::RunProof(run));
}
fn a_claim_before_its_first_turn_reads_back() {
    let mut run = fixtures::run();
    run.host = jig_domain_fleet::HostKind::Engine;
    run.credentials = Box::new([]);
    run.window = None;
    run.offered = None;
    run.outstanding = Box::new([]);
    run.turn = None;
    run.terminal = None;
    core_round_trip(core::CoreRecord::RunProof(run));
}
fn a_terminal_archive_reads_back_with_its_end() {
    core_round_trip(core::CoreRecord::Terminal(fixtures::terminal()));
}
fn a_goal_projection_reads_back_with_its_phase() {
    core_round_trip(core::CoreRecord::Projection(fixtures::goal()));
}
fn a_projection_task_reads_back_with_each_requester() {
    for requester in [tasks::Party::Task(11), tasks::Party::Person(12), tasks::Party::Deployment { project: 2 }] {
        core_round_trip(core::CoreRecord::Projection(fixtures::projection_task(requester)));
    }
}
fn a_milestone_reads_back_with_each_identity() {
    for identity in
        [core::MilestoneId::Lifecycle { task: 11, position: 3 }, core::MilestoneId::Revision { task: 11, revision: 4 }]
    {
        core_round_trip(core::CoreRecord::Projection(fixtures::milestone(identity)));
    }
}
fn a_milestone_without_carried_values_reads_back() {
    let core::ProjectionRecord::Milestone { goal, mut milestone } =
        fixtures::milestone(core::MilestoneId::Revision { task: 11, revision: 4 })
    else {
        panic!("milestone fixture")
    };
    milestone.phase = None;
    milestone.change = None;
    core_round_trip(core::CoreRecord::Projection(core::ProjectionRecord::Milestone { goal, milestone }));
}
fn a_turn_reads_back_with_its_bytes() {
    core_round_trip(core::CoreRecord::Turn(fixtures::turn()));
}
#[test]
fn a_turn_frame_body_reads_back_with_its_version() {
    let limits = call_limits();
    let turn = fixtures::turn();
    let bytes = codec::turn(&turn, &limits).expect("turn within limits");
    assert_eq!(codec::read_turn(&bytes, &limits), Ok(turn));
}
#[test]
fn a_turn_frame_of_another_version_is_refused() {
    let limits = call_limits();
    let mut bytes = codec::turn(&fixtures::turn(), &limits).expect("turn within limits");
    bytes[1] = 4;
    assert_eq!(
        codec::read_turn(&bytes, &limits).expect_err("another core version").reason,
        skein_codec::Reason::Version
    );
}
fn both_decision_records_read_back() {
    for row in [
        core::CoreRecord::ProposalDecision(fixtures::proposal()),
        core::CoreRecord::EscalationDecision(fixtures::escalation()),
    ] {
        core_round_trip(row);
    }
}
fn a_proposal_without_created_tasks_reads_back() {
    let mut proposal = fixtures::proposal();
    proposal.created = None;
    core_round_trip(core::CoreRecord::ProposalDecision(proposal));
}

#[test]
fn turn_limits_use_the_worker_channels_body_ceiling() {
    assert_eq!(call_limits().core.turn_record_transcript, 8_388_608);
}

#[test]
fn a_turn_exceeding_load_bytes_still_reads_back() {
    let mut domain = super::core_limits();
    domain.load_bytes = 1;
    let limits = codec::limits(&domain).expect("within schema ceilings");
    let turn = fixtures::turn();
    let bytes = codec::turn(&turn, &limits).expect("within the worker channel bound");
    assert_eq!(codec::read_turn(&bytes, &limits), Ok(turn));
}

fn run_proofs_read_back_with_their_optional_fields() {
    table([
        ("a_run_proof_with_its_terminal_reads_back", a_run_proof_with_its_terminal_reads_back),
        ("a_claim_before_its_first_turn_reads_back", a_claim_before_its_first_turn_reads_back),
        ("a_closed_window_flag_reads_back_as_stored", a_closed_window_flag_reads_back_as_stored),
    ]);
}

type Case = (&'static str, fn());

fn table<const N: usize>(cases: [Case; N]) {
    let mut failures = [None; N];
    let mut passed = true;
    for ((name, case), failure) in cases.into_iter().zip(&mut failures) {
        if std::panic::catch_unwind(case).is_err() {
            *failure = Some(name);
            passed = false;
        }
    }
    assert!(passed, "{failures:?}");
}

#[test]
fn core_rows_read_back_as_written() {
    table([
        (
            "the_deployment_header_reads_back_with_its_id_and_projects_mark",
            the_deployment_header_reads_back_with_its_id_and_projects_mark,
        ),
        ("a_terminal_archive_reads_back_with_its_end", a_terminal_archive_reads_back_with_its_end),
        ("a_goal_projection_reads_back_with_its_phase", a_goal_projection_reads_back_with_its_phase),
        ("a_projection_task_reads_back_with_each_requester", a_projection_task_reads_back_with_each_requester),
        ("a_milestone_reads_back_with_each_identity", a_milestone_reads_back_with_each_identity),
        ("a_milestone_without_carried_values_reads_back", a_milestone_without_carried_values_reads_back),
        ("a_turn_reads_back_with_its_bytes", a_turn_reads_back_with_its_bytes),
        ("both_decision_records_read_back", both_decision_records_read_back),
        ("a_proposal_without_created_tasks_reads_back", a_proposal_without_created_tasks_reads_back),
        ("run_proofs_read_back_with_their_optional_fields", run_proofs_read_back_with_their_optional_fields),
    ]);
}

#[test]
fn each_configured_bound_terminal_reads_back_with_its_value() {
    for bound in [
        tasks::Bound::Reasoning,
        tasks::Bound::Metadata,
        tasks::Bound::Items,
        tasks::Bound::Answer,
        tasks::Bound::Tools,
        tasks::Bound::HistoryItems,
        tasks::Bound::History,
        tasks::Bound::Window,
        tasks::Bound::Output,
    ] {
        let mut terminal = fixtures::terminal();
        terminal.end = tasks::End::Exceeded(tasks::Exceeded { bound, value: u64::MAX });
        core_round_trip(core::CoreRecord::Terminal(terminal));
    }
}

#[test]
fn outstanding_offers_use_the_run_inbox_bound_on_write_and_read() {
    let mut limits = call_limits();
    limits.core.run_proof_outstanding = 2;
    let mut proof = fixtures::run();
    proof.outstanding = Box::new([13, 14]);
    proof.offered = Some(14);
    let row = core::CoreRecord::RunProof(proof.clone());
    let bytes = codec::encode(&row, &limits).expect("exactly the run inbox bound");
    assert_eq!(codec::decode(&bytes, &limits), Ok(row));
    limits.core.run_proof_outstanding = 1;
    assert_eq!(codec::decode(&bytes, &limits).expect_err("oversized saved offers").reason, skein_codec::Reason::Bound);
    assert_eq!(
        codec::encode(&core::CoreRecord::RunProof(proof), &limits).expect_err("oversized offers").reason,
        skein_codec::Reason::Bound
    );
}

#[test]
fn projection_failure_states_and_final_generations_round_trip() {
    let source = core::ProjectionFailureSource { source: u64::MAX, at: skein_lib::Wall::from_nanos(u64::MAX) };
    for state in [
        core::ProjectionFailureState::Armed,
        core::ProjectionFailureState::Pending(source),
        core::ProjectionFailureState::Reported(source),
    ] {
        core_round_trip(core::CoreRecord::Projection(core::ProjectionRecord::Failure(core::ProjectionFailure {
            goal: 11,
            connector: u16::MAX,
            after: u64::MAX - 1,
            state,
        })));
    }
    for generation in [None, Some(1), Some(u64::MAX)] {
        let core::ProjectionRecord::Goal { goal, pending, changed, fed_at, .. } = fixtures::goal() else {
            panic!("goal fixture")
        };
        core_round_trip(core::CoreRecord::Projection(core::ProjectionRecord::Goal {
            goal,
            pending,
            changed,
            fed_at,
            closing_at: generation,
        }));
    }
}
