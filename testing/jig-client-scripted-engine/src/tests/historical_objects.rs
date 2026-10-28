use crate::{Bounds, owner_historical};
use jig_client_domain::contract::*;
use skein_lib::{List, Wall};

fn goal() -> OwnerPersonProposalGoal {
    let mut parameters = List::with_capacity(3);
    parameters.push(Parameter::Number { name: u32::MAX, value: u64::MAX }).expect("number fits");
    parameters.push(Parameter::Bytes { name: 0, value: super::text(UTF8_TEXTS[0]) }).expect("bytes fit");
    parameters.push(Parameter::Resource { name: 7, connector: u16::MAX, resource: u64::MAX }).expect("resource fits");
    let mut inputs = List::with_capacity(3);
    inputs.push(u64::MAX).expect("input fits");
    let mut holdings = List::with_capacity(3);
    for write in [true, false] {
        let mut path = List::with_capacity(3);
        path.push(Bytes::new(Box::from(b"a/b".as_slice()), 4096).unwrap()).expect("literal segment fits");
        path.push(Bytes::new(Box::from([255, 0]), 4096).unwrap()).expect("opaque segment fits");
        let holding = if write {
            OwnerGoalHolding::Write { connector: u16::MAX, path, kind: u16::MAX }
        } else {
            OwnerGoalHolding::Slot { connector: u16::MAX, path, kind: 0 }
        };
        holdings.push(holding).expect("holding fits");
    }
    OwnerPersonProposalGoal {
        number: u64::MAX,
        project: u32::MAX,
        executor: OwnerExecutor::Agent { charter: u32::MAX },
        spec: Specification { asks: false, words: super::text(UTF8_TEXTS[0]), parameters, inputs },
        contract: TaskContractDocument::Report { words: u32::MAX },
        authority: super::owner_authority().authority,
        numbers: OwnerNumbers { budget: 1, spent: 7, spent_below: 11, reserved: u64::MAX },
        funder: Funder::Pool { project: u32::MAX, person: u64::MAX, period: 0 },
        dependencies: List::with_capacity(3),
        holdings,
        wake: super::task().wake,
        tracked: Some(u32::MAX),
    }
}
fn person(state: OwnerPersonProposalState) -> OwnerPersonProposal {
    OwnerPersonProposal { number: u64::MAX, proposer: u64::MAX, project: u32::MAX, goal: goal(), state }
}
fn record(choice: OwnerProposalChoice, created: Option<(u64, u64)>) -> OwnerProposalDecision {
    OwnerProposalDecision {
        project: u32::MAX,
        proposer: Proposer::Party(u64::MAX),
        proposal: u64::MAX,
        kind: OwnerMessageProposalKind::Batch,
        by: u64::MAX,
        at: Wall::EPOCH,
        choice,
        created,
    }
}
fn result() -> Result {
    Result {
        task: super::task().chip,
        ending: Ending::Done,
        value: Some(ResultValue::Report(DocumentText::complete(super::text(b"independent")))),
        at: None,
        details: None,
        acts: List::with_capacity(MAX_OFFERED_ACTS),
    }
}
fn question() -> Question {
    Question {
        task: super::task().chip,
        from: Some(OwnerRequester::Deployment { project: u32::MAX }),
        message: u64::MAX,
        words: DocumentText::complete(super::text(UTF8_TEXTS[0])),
        since: Wall::EPOCH,
        acts: List::with_capacity(0),
        ending: None,
    }
}
fn answer() -> OwnerActivityLine {
    OwnerActivityLine {
        task: 8,
        position: u64::MAX,
        at: Wall::from_nanos(u64::MAX),
        line: OwnerActivity::Words {
            message: 0,
            by: OwnerRequester::Task(u64::MAX),
            kind: OwnerMessageKind::Answer { question: u64::MAX },
            words: super::text(UTF8_TEXTS[1]),
        },
    }
}
fn envelope(key: ObjectKey, value: OwnerHistoricalValue) -> OwnerHistorical {
    OwnerHistorical { key, value, at: None, by: None }
}
fn person_value(state: OwnerPersonProposalState, decision: Option<OwnerProposalDecision>) -> OwnerHistorical {
    envelope(
        ObjectKey::Proposal(ProposalKey { proposer: Proposer::Party(u64::MAX), number: u64::MAX }),
        OwnerHistoricalValue::PersonProposal { proposal: Box::new(person(state)), decision },
    )
}
fn bounds() -> Bounds {
    Bounds { rows: 3, segments: 3, text: 32, document_bytes: 1_000_000, ..Bounds::default() }
}
#[test]
fn historical_person_goal_keeps_raw_project_all_four_numbers_parameters_and_distinct_literal_holdings() {
    // These full-width receiving facts are intentionally separate from the
    // authentic producer's initial spent/reserved/dependency admission rules.
    let value = person_value(OwnerPersonProposalState::Pending { since: Wall::EPOCH }, None);
    let OwnerHistoricalValue::PersonProposal { proposal, .. } = &value.value else { panic!("person source") };
    assert_eq!(proposal.goal.project, u32::MAX);
    assert_eq!(proposal.goal.number, u64::MAX);
    assert_eq!(proposal.goal.numbers, OwnerNumbers { budget: 1, spent: 7, spent_below: 11, reserved: u64::MAX });
    assert_eq!(proposal.goal.spec.words.bytes(), UTF8_TEXTS[0]);
    assert_eq!(proposal.goal.spec.parameters.capacity(), 3);
    assert_eq!(proposal.goal.spec.parameters.get(0), Some(&Parameter::Number { name: u32::MAX, value: u64::MAX }));
    assert_eq!(
        proposal.goal.spec.parameters.get(1),
        Some(&Parameter::Bytes { name: 0, value: super::text(UTF8_TEXTS[0]) })
    );
    assert_eq!(
        proposal.goal.spec.parameters.get(2),
        Some(&Parameter::Resource { name: 7, connector: u16::MAX, resource: u64::MAX })
    );
    assert_eq!(proposal.goal.spec.inputs.as_slice(), &[u64::MAX]);
    assert_eq!(proposal.goal.dependencies.capacity(), 3);
    assert!(proposal.goal.dependencies.is_empty());
    let OwnerGoalHolding::Write { connector, path, kind } = proposal.goal.holdings.get(0).expect("write") else {
        panic!("write kind preserved")
    };
    assert_eq!((*connector, *kind), (u16::MAX, u16::MAX));
    assert_eq!(path.capacity(), 3);
    assert_eq!(path.get(0).expect("literal").bytes(), b"a/b");
    assert_eq!(path.get(1).expect("opaque").bytes(), &[255, 0]);
    let OwnerGoalHolding::Slot { connector, path, kind } = proposal.goal.holdings.get(1).expect("slot") else {
        panic!("slot kind preserved")
    };
    assert_eq!((*connector, *kind), (u16::MAX, 0));
    assert_eq!(path.capacity(), 3);
    assert_eq!(path.get(0).expect("literal").bytes(), b"a/b");
    assert!(value.matches(value.key));
    assert!(!value.matches(ObjectKey::Proposal(ProposalKey { proposer: Proposer::Task(u64::MAX), number: u64::MAX })));
}
#[test]
fn historical_person_states_keep_independent_decisions_choices_created_pairs_and_outer_absence() {
    for choice in [
        OwnerProposalChoice::Accepted,
        OwnerProposalChoice::Rejected,
        OwnerProposalChoice::Passed,
        OwnerProposalChoice::Withdrawn,
        OwnerProposalChoice::Stale,
    ] {
        for created in [None, Some((0, 0)), Some((0, u64::MAX)), Some((u64::MAX, u64::MAX))] {
            let decision = record(choice, created);
            let states = [
                OwnerPersonProposalState::Pending { since: Wall::EPOCH },
                OwnerPersonProposalState::Accepted { by: OwnerRequester::Person(u64::MAX) },
                OwnerPersonProposalState::Rejected {
                    by: OwnerRequester::Person(1),
                    reason: super::text(UTF8_TEXTS[1]),
                    message: u64::MAX,
                    at: Wall::EPOCH,
                },
            ];
            for state in states {
                let pending = matches!(state, OwnerPersonProposalState::Pending { .. });
                let original = person_value(state, Some(decision));
                for at in [None, Some(Wall::EPOCH), Some(Wall::from_nanos(u64::MAX))] {
                    for by in [
                        None,
                        Some(OwnerRequester::Task(0)),
                        Some(OwnerRequester::Person(u64::MAX)),
                        Some(OwnerRequester::Deployment { project: u32::MAX }),
                    ] {
                        let value = OwnerHistorical { at, by, ..original.clone() };
                        assert!(value.matches(value.key));
                        assert_eq!(value.is_terminal(), !pending, "decision presence does not replace supplied state");
                        let OwnerHistoricalValue::PersonProposal { decision: stored, .. } = &value.value else {
                            panic!("person source")
                        };
                        assert_eq!(*stored, Some(decision));
                        assert_eq!((value.at, value.by), (at, by));
                    }
                }
            }
        }
    }
    let accepted = person_value(OwnerPersonProposalState::Accepted { by: OwnerRequester::Person(1) }, None);
    assert!(!accepted.matches(accepted.key), "authentic accepted source without decision returns Gone");
    let mut wrong = person_value(
        OwnerPersonProposalState::Pending { since: Wall::EPOCH },
        Some(record(OwnerProposalChoice::Passed, None)),
    );
    let OwnerHistoricalValue::PersonProposal { decision: Some(decision), .. } = &mut wrong.value else {
        panic!("record")
    };
    decision.proposer = Proposer::Task(u64::MAX);
    assert!(!wrong.matches(wrong.key));
}
#[test]
fn historical_all_six_source_cases_validate_nested_identity_before_terminal_classification() {
    let task = super::task();
    let proposal = OwnerProposalHistory {
        action: OwnerRetainedAction::Release { task: u64::MAX },
        number: u64::MAX,
        proposer: u64::MAX,
        as_holder: true,
        state: OwnerProposalHistoryState::Pending { holder: OwnerHistoryHolder::Task(0), since: Wall::EPOCH },
    };
    let mut person_task = crate::owner_person_task::tests::value();
    person_task.task = task.chip.clone();
    let cases = [
        (
            envelope(
                ObjectKey::Task(8),
                OwnerHistoricalValue::Task { document: Box::new(task.clone()), result: Box::new(result()) },
            ),
            true,
        ),
        (
            envelope(
                ObjectKey::Proposal(ProposalKey { proposer: Proposer::Task(u64::MAX), number: u64::MAX }),
                OwnerHistoricalValue::Proposal(proposal),
            ),
            false,
        ),
        (person_value(OwnerPersonProposalState::Pending { since: Wall::EPOCH }, None), false),
        (
            envelope(
                ObjectKey::Escalation(8),
                OwnerHistoricalValue::Escalation {
                    task: Box::new(task),
                    decision: OwnerEscalationDecisionRecord {
                        project: 1,
                        requester: u64::MAX,
                        task: 8,
                        revision: 0,
                        by: u64::MAX,
                        at: Wall::EPOCH,
                        decision: OwnerEscalationDecision::Pass,
                    },
                    hold: Some(OwnerMilestoneLine {
                        task: 8,
                        position: u64::MAX,
                        phase: OwnerMilestonePhase::Waiting,
                        at: None,
                    }),
                },
            ),
            true,
        ),
        (
            envelope(
                ObjectKey::Question { task: 8, message: u64::MAX },
                OwnerHistoricalValue::Question { question: question(), answer: None, ending: None },
            ),
            false,
        ),
        (
            envelope(
                ObjectKey::PersonTask(8),
                OwnerHistoricalValue::PersonTask { document: Box::new(person_task), result: Box::new(result()) },
            ),
            true,
        ),
    ];
    for (value, terminal) in cases {
        assert!(value.matches(value.key));
        assert_eq!(value.is_terminal(), terminal);
        assert!(!value.matches(ObjectKey::Note { project: 1, note: 8 }));
        let unchanged = value.clone();
        let n = owner_historical::bytes(&value, &bounds()).expect("whole source fits dimensions");
        assert!(owner_historical::fits(&value, &Bounds { document_bytes: n, ..bounds() }));
        assert!(!owner_historical::fits(&value, &Bounds { document_bytes: n - 1, ..bounds() }));
        assert_eq!(value, unchanged);
    }
    let mut mismatched = envelope(
        ObjectKey::Task(8),
        OwnerHistoricalValue::Task { document: Box::new(super::task()), result: Box::new(result()) },
    );
    let OwnerHistoricalValue::Task { result, .. } = &mut mismatched.value else { panic!("task") };
    result.task.task = 9;
    assert!(!mismatched.matches(mismatched.key));
    assert!(!owner_historical::fits(&mismatched, &bounds()));
}
#[test]
fn historical_question_answer_and_full_ending_presence_are_independent_from_outer_sender_and_clock() {
    for answer_present in [false, true] {
        for ending_present in [false, true] {
            let value = envelope(
                ObjectKey::Question { task: 8, message: u64::MAX },
                OwnerHistoricalValue::Question {
                    question: question(),
                    answer: answer_present.then(answer),
                    ending: ending_present.then(|| Box::new(result())),
                },
            );
            assert!(value.matches(value.key));
            assert_eq!(value.is_terminal(), answer_present || ending_present);
            assert_eq!((value.at, value.by), (None, None));
        }
    }
    let mut value = envelope(
        ObjectKey::Question { task: 8, message: u64::MAX },
        OwnerHistoricalValue::Question { question: question(), answer: Some(answer()), ending: None },
    );
    let OwnerHistoricalValue::Question { answer: Some(answer), .. } = &mut value.value else { panic!("answer") };
    answer.line = OwnerActivity::Words {
        message: 0,
        by: OwnerRequester::Person(1),
        kind: OwnerMessageKind::Answer { question: 7 },
        words: super::text(b""),
    };
    assert!(!value.matches(value.key));
}
#[test]
fn historical_recursive_admission_prices_reserved_raw_goal_paths_boxes_and_each_independent_source() {
    let b = bounds();
    let value = person_value(
        OwnerPersonProposalState::Rejected {
            by: OwnerRequester::Person(1),
            reason: super::text(UTF8_TEXTS[2]),
            message: u64::MAX,
            at: Wall::EPOCH,
        },
        None,
    );
    let original = value.clone();
    let whole = owner_historical::bytes(&value, &b).expect("whole goal fits");
    assert!(owner_historical::fits(&value, &Bounds { document_bytes: whole, ..b }));
    assert!(!owner_historical::fits(&value, &Bounds { document_bytes: whole - 1, ..b }));
    assert!(!owner_historical::fits(&value, &Bounds { text: 31, ..b }));
    let mut empty = value.clone();
    let OwnerHistoricalValue::PersonProposal { proposal, .. } = &mut empty.value else { panic!("goal") };
    proposal.goal.dependencies = List::with_capacity(0);
    assert_eq!(
        whole - owner_historical::bytes(&empty, &b).expect("empty source fits"),
        List::<u64>::worst_case(3).expect("reserved list")
    );
    let mut overflow = value.clone();
    let OwnerHistoricalValue::PersonProposal { proposal, .. } = &mut overflow.value else { panic!("goal") };
    let OwnerGoalHolding::Write { path, .. } = proposal.goal.holdings.get_mut(0).expect("write") else {
        panic!("write")
    };
    *path = List::with_capacity(4);
    assert!(!owner_historical::fits(&overflow, &b));
    assert_eq!(value, original);
    assert_eq!(owner_historical::worst_case(&b), Some(b.document_bytes));
}

#[test]
fn historical_task_proposal_states_preserve_raw_maker_and_do_not_use_outer_context_as_terminal_evidence() {
    let states = [
        OwnerProposalHistoryState::Pending { holder: OwnerHistoryHolder::Task(u64::MAX), since: Wall::EPOCH },
        OwnerProposalHistoryState::Accepted { by: OwnerRequester::Deployment { project: u32::MAX } },
        OwnerProposalHistoryState::Rejected {
            by: OwnerRequester::Task(0),
            reason: DocumentText { bytes: super::text(UTF8_TEXTS[1]), cut: true },
        },
        OwnerProposalHistoryState::Withdrawn,
    ];
    for state in states {
        let pending = matches!(state, OwnerProposalHistoryState::Pending { .. });
        let value = OwnerHistorical {
            key: ObjectKey::Proposal(ProposalKey { proposer: Proposer::Task(u64::MAX), number: 0 }),
            value: OwnerHistoricalValue::Proposal(OwnerProposalHistory {
                action: OwnerRetainedAction::Release { task: 0 },
                number: 0,
                proposer: u64::MAX,
                as_holder: false,
                state,
            }),
            at: Some(Wall::EPOCH),
            by: Some(OwnerRequester::Person(u64::MAX)),
        };
        assert!(value.matches(value.key));
        assert_eq!(value.is_terminal(), !pending);
        assert!(!value.matches(ObjectKey::Proposal(ProposalKey { proposer: Proposer::Party(u64::MAX), number: 0 })));
        assert!(owner_historical::fits(&value, &bounds()));
    }
}

#[test]
fn historical_escalation_decisions_keep_opaque_rejection_and_independent_optional_hold() {
    for decision in [
        OwnerEscalationDecision::Release,
        OwnerEscalationDecision::Pass,
        OwnerEscalationDecision::Reject { reason: super::text(b"") },
        OwnerEscalationDecision::Reject { reason: super::text(UTF8_TEXTS[0]) },
    ] {
        for hold in
            [None, Some(OwnerMilestoneLine { task: 8, position: 0, phase: OwnerMilestonePhase::Waiting, at: None })]
        {
            let record = OwnerEscalationDecisionRecord {
                project: 1,
                requester: u64::MAX,
                task: 8,
                revision: u64::MAX,
                by: 0,
                at: Wall::from_nanos(u64::MAX),
                decision: decision.clone(),
            };
            let value = OwnerHistorical {
                key: ObjectKey::Escalation(8),
                value: OwnerHistoricalValue::Escalation {
                    task: Box::new(super::task()),
                    decision: record.clone(),
                    hold: hold.clone(),
                },
                at: None,
                by: None,
            };
            assert!(value.matches(value.key));
            assert!(value.is_terminal());
            assert_eq!((value.at, value.by), (None, None));
            let OwnerHistoricalValue::Escalation { decision: stored, hold: stored_hold, .. } = &value.value else {
                panic!("escalation")
            };
            assert_eq!(stored, &record);
            assert_eq!(stored_hold, &hold);
            assert!(owner_historical::fits(&value, &bounds()));
            let mut wrong = value.clone();
            let OwnerHistoricalValue::Escalation { decision, .. } = &mut wrong.value else { panic!("decision") };
            decision.task = 9;
            assert!(!wrong.matches(wrong.key));
        }
    }
}

#[test]
fn historical_whole_task_and_result_boxes_charge_each_copy_with_independent_metadata() {
    let mut task = super::task();
    task.context = None;
    task.result = None;
    let mut final_result = result();
    final_result.at = Some(Wall::EPOCH);
    let source = OwnerHistorical {
        key: ObjectKey::Task(8),
        value: OwnerHistoricalValue::Task { document: Box::new(task.clone()), result: Box::new(final_result.clone()) },
        at: None,
        by: Some(OwnerRequester::Deployment { project: u32::MAX }),
    };
    let source_bytes = owner_historical::bytes(&source, &bounds()).expect("whole source bytes");
    let task_heap = crate::payload::Payload::heap(&task, &bounds()).expect("task heap");
    let result_heap = crate::payload::Payload::heap(&final_result, &bounds()).expect("result heap");
    let expected = u64::try_from(core::mem::size_of::<OwnerHistorical>()).expect("root fits")
        + u64::try_from(core::mem::size_of::<Task>()).expect("task box fits")
        + task_heap
        + u64::try_from(core::mem::size_of::<Result>()).expect("result box fits")
        + result_heap;
    assert_eq!(source_bytes, expected);
    let cloned = source.clone();
    assert_eq!(owner_historical::bytes(&cloned, &bounds()), Some(source_bytes));
    assert_eq!(source, cloned);
    assert_eq!(
        source_bytes.checked_add(source_bytes),
        Some(expected * 2),
        "retaining Body and Left pays for both sources before wrapper costs"
    );
    assert_eq!((source.at, source.by), (None, Some(OwnerRequester::Deployment { project: u32::MAX })));
    let OwnerHistoricalValue::Task { document, result } = &source.value else { panic!("task") };
    assert!(document.context.is_none());
    assert!(document.result.is_none());
    assert_eq!(result.at, Some(Wall::EPOCH));
    assert!(result.details.is_none());
}

const UTF8_TEXTS: [&[u8]; 3] = [b"~\0/", b"~\0", &[b'~'; 32]];
