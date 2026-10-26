//! Task lifecycle and proposal rows in both directions (protocol/store.md, 5.1).
use super::{bound_problem, fixtures, saved_tasks, saved_wire, tasks_limits};

const WITHDRAWN_AT: skein_lib::Wall = skein_lib::Wall::from_nanos(73);

fn a_history_row_with_an_amendment_reads_back() {
    let row = jig_domain_tasks::Stored::History(fixtures::task_history());
    assert_eq!(saved_tasks(&row), Ok(row));
}
fn a_held_phase_reads_back_with_its_source() {
    let source = fixtures::task_phase();
    assert_eq!(saved_phase(&source), Ok(source));
}

fn a_pending_person_proposal_reads_back() {
    for action in [
        fixtures::task_person_proposal().action,
        jig_domain_tasks::PersonAction::Amend {
            task: 23,
            revision: 0,
            amendment: Box::new(fixtures::task_amendment()),
        },
    ] {
        let mut proposal = fixtures::task_person_proposal();
        proposal.action = action;
        let row = jig_domain_tasks::Stored::PersonProposal(Box::new(proposal));
        assert_eq!(saved_tasks(&row), Ok(row));
    }
}
fn a_recurring_template_reads_back_one_level_deep() {
    use crate::{codec::tasks::phase, tasks_v9::Creation};
    let mut source = fixtures::task_creation(59);
    source.recurring = Some(Box::new(fixtures::task_template()));
    let encoded = phase::new(&source, &tasks_limits(), None).expect("one level is supported");
    let encoded = saved_wire(encoded, Creation::measure, Creation::encode, Creation::decode);
    assert_eq!(phase::read_new(encoded, &tasks_limits()), Ok(source));
}
#[test]
fn a_template_member_holding_a_template_is_refused() {
    let mut source = fixtures::task_template();
    source.batch.get_mut(0).expect("first member").recurring = Some(Box::new(fixtures::task_template()));
    assert_eq!(
        crate::codec::tasks::phase::template(&source, &tasks_limits(), None),
        Err(bound_problem(crate::tasks_v9::Path::CreationRecurring))
    );
}
fn a_milestone_reads_back_with_its_complete_phase() {
    let row = jig_domain_tasks::Stored::Milestone(jig_domain_tasks::Milestone {
        task: 17,
        position: 7,
        phase: fixtures::task_phase(),
        at: skein_lib::Wall::from_nanos(61),
    });
    assert_eq!(saved_tasks(&row), Ok(row));
}
fn a_decided_person_proposal_reads_back() {
    use jig_domain_tasks::{Party, PersonProposalState};
    for state in [
        PersonProposalState::Accepted { by: Party::Person(31) },
        PersonProposalState::Withdrawn { at: WITHDRAWN_AT },
        PersonProposalState::Rejected {
            by: Party::Person(31),
            reason: Box::from(b"no".as_slice()),
            message: 67,
            at: skein_lib::Wall::from_nanos(71),
        },
    ] {
        let mut proposal = fixtures::task_person_proposal();
        proposal.state = state;
        let row = jig_domain_tasks::Stored::DecidedProposal(Box::new(proposal));
        assert_eq!(saved_tasks(&row), Ok(row));
    }
}

fn saved_phase(source: &jig_domain_tasks::Phase) -> Result<jig_domain_tasks::Phase, crate::tasks_v9::Problem> {
    use crate::{codec::tasks::phase, tasks_v9::Phase};
    let encoded = phase::phase(source, &tasks_limits())?;
    let encoded = saved_wire(encoded, Phase::measure, Phase::encode, Phase::decode);
    phase::read_phase(encoded, &tasks_limits())
}
fn each_lifecycle_phase_reads_back() {
    use jig_domain_tasks::{Active, Closing, Ending, Phase, Stage};
    for source in [
        Phase::Waiting,
        Phase::Active(Active::Claimed { attempt: 3 }),
        Phase::Closing(Closing {
            stage: Stage::Run { attempt: 3 },
            ending: Ending::Failed { reason: Box::from(b"failure".as_slice()) },
        }),
        Phase::Ended(Ending::Done(jig_domain_tasks::TaskResult::Report { words: Box::from(b"done".as_slice()) })),
    ] {
        assert_eq!(saved_phase(&source), Ok(source));
    }
}
fn each_prior_held_phase_reads_back() {
    use jig_domain_tasks::{Active, Closing, Ending, Hold, HoldSource, Party, Phase, Stage, Was};
    for was in [
        Was::Waiting,
        Was::Active(Active::BackingOff { until: skein_lib::Wall::from_nanos(u64::MAX) }),
        Was::Closing(Closing {
            stage: Stage::Settled,
            ending: Ending::Cancelled {
                by: Party::Person(31),
                at: skein_lib::Wall::from_nanos(47),
                reason: Box::from(b"stop".as_slice()),
                result: Some(jig_domain_tasks::TaskResult::Failure { reason: Box::from(b"failed".as_slice()) }),
            },
        }),
    ] {
        let source = Phase::Held {
            was,
            why: Hold::StoppedBy { party: 31 },
            source: Some(Box::new(HoldSource::Effect { connector: 2, entry: 53 })),
        };
        assert_eq!(saved_phase(&source), Ok(source));
    }
}
fn a_held_phase_without_a_source_reads_back() {
    let source = jig_domain_tasks::Phase::Held {
        was: jig_domain_tasks::Was::Waiting,
        why: jig_domain_tasks::Hold::Stalled,
        source: None,
    };
    assert_eq!(saved_phase(&source), Ok(source));
}
fn saved_result(
    source: &jig_domain_tasks::TaskResult,
) -> Result<jig_domain_tasks::TaskResult, crate::tasks_v9::Problem> {
    use crate::{codec::tasks::phase, tasks_v9::TaskResult};
    let encoded = phase::result(source, &tasks_limits())?;
    let encoded = saved_wire(encoded, TaskResult::measure, TaskResult::encode, TaskResult::decode);
    phase::read_result(encoded, &tasks_limits())
}
fn each_result_kind_reads_back() {
    use jig_domain_tasks::{ResultField, TaskResult};
    for source in [
        TaskResult::Approved { state: Box::from(b"state".as_slice()), words: Some(Box::from(b"yes".as_slice())) },
        TaskResult::Report { words: Box::from(b"report".as_slice()) },
        TaskResult::Verdict {
            code: 7,
            fields: Box::new([ResultField { name: Box::from(b"k".as_slice()), value: Box::from(b"value".as_slice()) }]),
            words: Box::from(b"v".as_slice()),
        },
        TaskResult::Change { connector: 2, kind: 3, resource: 31, words: Box::from(b"change".as_slice()) },
        TaskResult::Failure { reason: Box::from(b"failed".as_slice()) },
    ] {
        assert_eq!(saved_result(&source), Ok(source));
    }
}
fn result_with_state(bytes: usize) -> jig_domain_tasks::TaskResult {
    jig_domain_tasks::TaskResult::Approved { state: skein_lib::bytes::zeroed(bytes), words: Some(Box::new([1])) }
}
fn a_result_at_its_shared_byte_bound_reads_back() {
    let source = result_with_state(31);
    assert_eq!(saved_result(&source), Ok(source));
}
#[test]
fn a_result_past_its_shared_byte_bound_is_refused() {
    assert_eq!(
        crate::codec::tasks::phase::result(&result_with_state(32), &tasks_limits()),
        Err(bound_problem(crate::tasks_v9::Path::TaskResultTag))
    );
}
#[test]
fn a_decoded_result_past_its_shared_byte_bound_is_refused() {
    use crate::codec::tasks::phase;
    let mut wider = tasks_limits();
    wider.result_field_value = 33;
    let encoded = phase::result(&result_with_state(32), &wider).expect("within wider aggregate bound");
    assert_eq!(phase::read_result(encoded, &tasks_limits()), Err(bound_problem(crate::tasks_v9::Path::TaskResultTag)));
}

fn saved_proposal(source: &jig_domain_tasks::Proposal) -> Result<jig_domain_tasks::Proposal, crate::tasks_v9::Problem> {
    use crate::{codec::tasks::proposals, tasks_v9::Proposal};
    let encoded = proposals::proposal(source, &tasks_limits(), None)?;
    let encoded = saved_wire(encoded, Proposal::measure, Proposal::encode, Proposal::decode);
    proposals::read_proposal(encoded, &tasks_limits())
}
fn each_proposal_action_reads_back() {
    use jig_domain_tasks::ProposalAction;
    for action in [
        ProposalAction::Batch(Box::new([fixtures::task_creation(23)])),
        ProposalAction::Effect {
            connector: 2,
            authority: fixtures::task_authority(),
            attempt: 3,
            completion: 5,
            position: 7,
            purpose: 11,
        },
        ProposalAction::ProcedureEffect { connector: 2, authority: fixtures::task_authority(), step: 13, purpose: 17 },
        ProposalAction::Amend { task: 17, revision: 0, amendment: fixtures::task_amendment() },
        ProposalAction::Widen { task: 17, authority: fixtures::task_authority() },
        ProposalAction::Release { task: 17 },
        ProposalAction::Settings {
            against: 1,
            items: Box::new([jig_domain_tasks::SettingHead {
                setting: jig_domain_tasks::Setting::Charter(1),
                edit: jig_domain_tasks::SettingEdit::Set,
            }]),
        },
    ] {
        let mut source = fixtures::task_proposal();
        source.action = action;
        assert_eq!(saved_proposal(&source), Ok(source));
    }
}
fn each_proposal_decision_reads_back() {
    use jig_domain_tasks::{Party, ProposalHolder, ProposalKind, ProposalState};
    for state in [
        ProposalState::Pending { holder: ProposalHolder::Task(19), since: skein_lib::Wall::from_nanos(23) },
        ProposalState::Pending { holder: ProposalHolder::Person(31), since: skein_lib::Wall::from_nanos(37) },
        ProposalState::Pending {
            holder: ProposalHolder::Policy { project: 3, kind: ProposalKind::Widen },
            since: skein_lib::Wall::from_nanos(41),
        },
        ProposalState::Accepted { by: Party::Person(31) },
        ProposalState::Rejected { by: Party::Task(19), reason: Box::from(b"no".as_slice()) },
        ProposalState::Withdrawn,
    ] {
        let mut source = fixtures::task_proposal();
        source.state = state;
        assert_eq!(saved_proposal(&source), Ok(source));
    }
}
fn an_amendment_without_replacements_reads_back() {
    use crate::{codec::tasks::proposals, tasks_v9::Amendment};
    let source = jig_domain_tasks::Amendment {
        tracked: None,
        addressed: None,
        instructions: None,
        release: None,
        spec: None,
        wake: None,
        dependencies: None,
        authority: None,
        llm: None,
        reason: Box::new([]),
    };
    let encoded = proposals::amendment(&source, &tasks_limits(), None).expect("absent replacements fit");
    let encoded = saved_wire(encoded, Amendment::measure, Amendment::encode, Amendment::decode);
    assert_eq!(proposals::read_amendment(encoded, &tasks_limits()), Ok(source));
}
fn a_history_without_optional_details_reads_back() {
    let mut source = fixtures::task_history();
    source.proposal = None;
    source.amendment = None;
    let row = jig_domain_tasks::Stored::History(source);
    assert_eq!(saved_tasks(&row), Ok(row));
}
fn a_finish_grace_reads_back() {
    use crate::{codec::tasks::phase, tasks_v9::Grace};
    let source = jig_domain_tasks::Grace {
        by: jig_domain_tasks::Party::Person(31),
        until: skein_lib::Wall::from_nanos(u64::MAX),
        reason: b"finish".as_slice().into(),
        message: 1,
    };
    let encoded = phase::grace(&source, &tasks_limits()).expect("fixed-width fields fit");
    let encoded = saved_wire(encoded, Grace::measure, Grace::encode, Grace::decode);
    assert_eq!(phase::read_grace(encoded, &tasks_limits()), Ok(source));
}
fn a_revision_set_reads_back() {
    use crate::{codec::tasks::phase, tasks_v9::Revisions};
    let source = jig_domain_tasks::Revisions { hold: 3, spec: 5, person: 7 };
    let encoded = phase::revisions(&source, &tasks_limits()).expect("fixed-width fields fit");
    let encoded = saved_wire(encoded, Revisions::measure, Revisions::encode, Revisions::decode);
    assert_eq!(phase::read_revisions(encoded, &tasks_limits()), Ok(source));
}
fn each_retry_notice_reads_back() {
    use crate::{codec::tasks::phase, tasks_v9::RetryNotice};
    for source in [
        jig_domain_tasks::RetryNotice::Failed { class: jig_domain_tasks::Class::Lost, count: 3, allowance: 5 },
        jig_domain_tasks::RetryNotice::Refused { count: 7 },
    ] {
        let encoded = phase::retry_notice(&source, &tasks_limits()).expect("fixed-width fields fit");
        let encoded = saved_wire(encoded, RetryNotice::measure, RetryNotice::encode, RetryNotice::decode);
        assert_eq!(phase::read_retry_notice(encoded, &tasks_limits()), Ok(source));
    }
}

fn history_rows_read_back() {
    table([
        ("a_history_row_with_an_amendment_reads_back", a_history_row_with_an_amendment_reads_back),
        ("a_history_without_optional_details_reads_back", a_history_without_optional_details_reads_back),
    ]);
}

fn lifecycle_phases_read_back() {
    table([
        ("a_held_phase_reads_back_with_its_source", a_held_phase_reads_back_with_its_source),
        ("a_held_phase_without_a_source_reads_back", a_held_phase_without_a_source_reads_back),
        ("each_lifecycle_phase_reads_back", each_lifecycle_phase_reads_back),
        ("each_prior_held_phase_reads_back", each_prior_held_phase_reads_back),
    ]);
}

fn results_read_back() {
    table([
        ("each_result_kind_reads_back", each_result_kind_reads_back),
        ("a_result_at_its_shared_byte_bound_reads_back", a_result_at_its_shared_byte_bound_reads_back),
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
fn task_phases_and_proposals_read_back_as_written() {
    table([
        ("a_pending_person_proposal_reads_back", a_pending_person_proposal_reads_back),
        ("a_recurring_template_reads_back_one_level_deep", a_recurring_template_reads_back_one_level_deep),
        ("a_milestone_reads_back_with_its_complete_phase", a_milestone_reads_back_with_its_complete_phase),
        ("a_decided_person_proposal_reads_back", a_decided_person_proposal_reads_back),
        ("each_proposal_action_reads_back", each_proposal_action_reads_back),
        ("each_proposal_decision_reads_back", each_proposal_decision_reads_back),
        ("an_amendment_without_replacements_reads_back", an_amendment_without_replacements_reads_back),
        ("a_finish_grace_reads_back", a_finish_grace_reads_back),
        ("a_revision_set_reads_back", a_revision_set_reads_back),
        ("each_retry_notice_reads_back", each_retry_notice_reads_back),
        ("history_rows_read_back", history_rows_read_back),
        ("lifecycle_phases_read_back", lifecycle_phases_read_back),
        ("results_read_back", results_read_back),
    ]);
}

#[test]
fn tracking_amendments_read_back() {
    use crate::{codec::tasks::proposals, tasks_v9::Amendment};
    use jig_domain_tasks::Tracking;
    for tracked in [None, Some(Tracking::Untracked), Some(Tracking::Tracked { priority: 7 })] {
        let mut source = fixtures::task_amendment();
        source.tracked = tracked;
        let encoded = proposals::amendment(&source, &tasks_limits(), None).unwrap();
        let encoded = saved_wire(encoded, Amendment::measure, Amendment::encode, Amendment::decode);
        assert_eq!(proposals::read_amendment(encoded, &tasks_limits()), Ok(source), "{tracked:?}");
    }
}

#[test]
fn a_named_deadline_keeps_its_optional_wall_in_a_pending_batch() {
    for (name, selector, wall) in [
        ("none", jig_domain_tasks::Deadline::Named, None),
        ("finite", jig_domain_tasks::Deadline::Named, Some(fixtures::DEADLINE)),
        ("span", jig_domain_tasks::Deadline::Span(skein_lib::Duration::from_secs(5)), Some(fixtures::DEADLINE)),
    ] {
        let mut proposal = fixtures::task_person_proposal();
        let jig_domain_tasks::PersonAction::Batch(batch) = &mut proposal.action else { panic!("batch fixture") };
        for member in batch {
            member.authority.budget.deadline = wall;
            member.deadline = selector;
        }
        let row = jig_domain_tasks::Stored::PersonProposal(Box::new(proposal.clone()));
        assert_eq!(saved_tasks(&row), Ok(row), "{name}: pending");
        proposal.state = jig_domain_tasks::PersonProposalState::Accepted { by: jig_domain_tasks::Party::Person(2) };
        let row = jig_domain_tasks::Stored::DecidedProposal(Box::new(proposal));
        assert_eq!(saved_tasks(&row), Ok(row), "{name}: decided");
    }
}

#[test]
fn each_configured_bound_hold_reads_back_with_its_value() {
    use jig_domain_tasks::{Active, Bound, Exceeded, Hold, Phase, Was};
    for bound in [
        Bound::Reasoning,
        Bound::Metadata,
        Bound::Items,
        Bound::Answer,
        Bound::Tools,
        Bound::HistoryItems,
        Bound::History,
        Bound::Window,
        Bound::Output,
    ] {
        let source = Phase::Held {
            was: Was::Active(Active::Due),
            why: Hold::Exceeded(Exceeded { bound, value: u64::MAX }),
            source: None,
        };
        assert_eq!(saved_phase(&source), Ok(source));
    }
}
