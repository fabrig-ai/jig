use super::*;
use skein_lib::{List, Wall};

pub(crate) fn limits() -> Limits {
    Limits { rows: 8, text: 200, turn_text: 200, document_bytes: 1_000_000, ..crate::test::limits().client.domain }
}
fn raw_goal() -> tasks::New {
    tasks::New {
        deadline: jig_domain_tasks::Deadline::Named,
        label: 0,
        flags: jig_domain_tasks::Flags::PLAIN,
        number: u64::MAX,
        project: u32::MAX,
        executor: tasks::Executor::Procedure { connector: u16::MAX, code: u32::MAX },
        llm: None,
        spec: tasks::Spec {
            asks: false,
            words: Box::from(UTF8_TEXTS[0]),
            parameters: Box::new([
                tasks::Parameter::Number { name: 0, value: u64::MAX },
                tasks::Parameter::Bytes { name: u32::MAX, value: Box::from(UTF8_TEXTS[1]) },
                tasks::Parameter::Resource { name: 1, connector: u16::MAX, resource: u64::MAX },
            ]),
            inputs: Box::new([0, u64::MAX]),
        },
        contract: tasks::Contract::Report { words: u32::MAX },
        authority: tasks::Authority {
            tools: tasks::Tools(u64::MAX),
            models: tasks::Models::ALL,
            grants: Box::new([tasks::Grant {
                connector: u16::MAX,
                kind: u16::MAX,
                pattern: tasks::Pattern {
                    segments: Box::new([Box::from(b"a/b".as_slice()), Box::from([255, 0])]),
                    last: tasks::Last::Open(Box::from([0, 255])),
                },
            }]),
            delegation: tasks::Delegation {
                kinds: Box::new([
                    tasks::AuthorityExecutor::Charter(0),
                    tasks::AuthorityExecutor::Procedure(u32::MAX),
                    tasks::AuthorityExecutor::Role(1),
                ]),
                tasks: u32::MAX,
                depth: 0,
            },
            budget: tasks::Budget { spend: u64::MAX, deadline: Some(Wall::EPOCH) },
            notes: tasks::Scopes(u8::MAX),
            note_resources: Box::new([tasks::ResourceScope {
                connector: 0,
                pattern: tasks::Pattern { segments: Box::new([]), last: tasks::Last::Exact(Box::new([])) },
            }]),
        },
        numbers: tasks::Numbers { budget: 2, spent: 3, spent_below: 5, reserved: 7 },
        funder: tasks::Funder::Pool { project: 0, person: u64::MAX, period: 0 },
        dependencies: Box::new([0, u64::MAX]),
        holdings: Box::new([
            tasks::Holding::Write {
                resource: tasks::Name {
                    connector: u16::MAX,
                    path: Box::new([Box::from(b"one/two".as_slice()), Box::from([255, 0])]),
                },
                kind: 0,
            },
            tasks::Holding::Slot { pool: tasks::Name { connector: 0, path: Box::new([Box::new([])]) }, kind: u16::MAX },
        ]),
        wake: task::tests::source().wake,
        recurring: None,
        tracked: Some(0),
    }
}
pub(crate) fn sources() -> Vec<(client::Read, reads::Historical)> {
    let task = task::tests::source();
    let result = results::tests::document();
    let proposal = history::tests::source().proposal.expect("supplied historical proposal");
    let question = documents::tests::question_source();
    let person = tasks::PersonProposal {
        number: u64::MAX,
        proposer: u64::MAX,
        project: u32::MAX,
        action: tasks::PersonAction::Batch(Box::new([raw_goal()])),
        reason: Box::from(b"proposed batch".as_slice()),
        state: tasks::PersonProposalState::Pending { since: Wall::EPOCH },
    };
    let values = vec![
        (
            client::Read::Task { task: u64::MAX },
            reads::Past::Task { document: Box::new(task.clone()), result: Box::new(result.clone()) },
        ),
        (
            client::Read::Proposal {
                proposal: client::ProposalKey { proposer: client::Proposer::Task(0), number: u64::MAX },
            },
            reads::Past::Proposal(proposal),
        ),
        (
            client::Read::Proposal {
                proposal: client::ProposalKey { proposer: client::Proposer::Party(u64::MAX), number: u64::MAX },
            },
            reads::Past::PersonProposal { proposal: Box::new(person), decision: None },
        ),
        (
            client::Read::Escalation { task: u64::MAX },
            reads::Past::Escalation {
                task: Box::new(task.clone()),
                decision: jig_domain_core::EscalationDecisionRecord {
                    project: task.project,
                    requester: 0,
                    task: u64::MAX,
                    revision: u64::MAX,
                    by: u64::MAX,
                    at: Wall::EPOCH,
                    decision: people::EscalationDecision::Reject { reason: Box::from(UTF8_TEXTS[0]) },
                },
                hold: None,
            },
        ),
        (
            client::Read::Question { task: u64::MAX, message: u64::MAX },
            reads::Past::Question { question, answer: None, ending: None },
        ),
        (
            client::Read::PersonTask { task: u64::MAX },
            reads::Past::PersonTask { document: Box::new(person_tasks::tests::source()), result: Box::new(result) },
        ),
    ];
    values
        .into_iter()
        .map(|(query, value)| {
            (query, reads::Historical { value, at: Some(Wall::EPOCH), by: Some(tasks::Party::Task(0)) })
        })
        .collect()
}
fn value(query: &client::Read, source: &reads::Historical) -> client::OwnerHistorical {
    let client::ReadValue::Historical(value) = read(query, source, &limits(), &[]).expect("whole source") else {
        panic!("whole historical carrier")
    };
    *value
}

#[test]
fn historical_six_whole_sources_keep_the_original_kind_outer_facts_and_independent_nested_allocations() {
    for (query, source) in sources() {
        let before = source.clone();
        let actual = value(&query, &source);
        assert_eq!(actual.at, Some(Wall::EPOCH));
        assert_eq!(actual.by, Some(client::OwnerRequester::Task(0)));
        assert!(actual.matches(actual.key));
        match actual.value {
            client::OwnerHistoricalValue::Task { document, result } => {
                assert!(document.cut);
                assert_eq!(document.inputs.capacity(), 4);
                assert_eq!(result.details.as_ref().expect("whole result metadata").delegates.capacity(), 8);
                assert_eq!(document.taken_by, Some(u64::MAX));
                assert_eq!(result.at, None);
            }
            client::OwnerHistoricalValue::Proposal(proposal) => {
                assert!(proposal.as_holder);
                assert_eq!(proposal.proposer, 0);
            }
            client::OwnerHistoricalValue::PersonProposal { proposal, decision } => {
                assert_eq!(proposal.goal.number, u64::MAX);
                assert_eq!(decision, None);
            }
            client::OwnerHistoricalValue::Escalation { task, decision, hold } => {
                assert_eq!(task.chip.task, u64::MAX);
                assert_eq!(decision.requester, 0);
                assert_eq!(hold, None);
            }
            client::OwnerHistoricalValue::Question { question, answer, ending } => {
                assert_eq!(question.words.bytes(), UTF8_TEXTS[0]);
                assert_eq!(answer, None);
                assert_eq!(ending, None);
            }
            client::OwnerHistoricalValue::PersonTask { document, result } => {
                assert!(document.cut);
                assert_eq!(document.results.capacity(), 4);
                assert_eq!(result.at, None);
            }
        }
        assert_eq!(source, before);
    }
}

#[test]
fn historical_raw_person_goal_keeps_all_four_counters_authority_parameters_and_literal_holding_segments() {
    let (query, source) = sources().remove(2);
    let client::OwnerHistoricalValue::PersonProposal { proposal, .. } = value(&query, &source).value else {
        panic!("person proposal")
    };
    let goal = proposal.goal;
    assert_eq!(goal.numbers, client::OwnerNumbers { budget: 2, spent: 3, spent_below: 5, reserved: 7 });
    assert_eq!(goal.spec.words.bytes(), UTF8_TEXTS[0]);
    assert_eq!(goal.spec.parameters.capacity(), 3);
    assert_eq!(goal.spec.inputs.as_slice(), [0, u64::MAX]);
    assert_eq!(goal.dependencies.as_slice(), [0, u64::MAX]);
    assert_eq!(goal.authority.tools, u64::MAX);
    assert_eq!(goal.authority.notes, u8::MAX);
    assert_eq!(goal.authority.spend, u64::MAX);
    assert_eq!(goal.authority.deadline, Some(Wall::EPOCH));
    assert_eq!(goal.authority.delegation.kinds.capacity(), 3);
    assert_eq!(goal.authority.delegation.tasks, u32::MAX);
    let grant = goal.authority.grants.get(0).expect("whole grant");
    assert_eq!(grant.pattern.segments.as_slice(), [Box::from(b"a/b".as_slice()), Box::from([255, 0])]);
    assert_eq!(grant.pattern.last, client::OwnerPatternEnd::Open(Box::from([0, 255])));
    let client::OwnerGoalHolding::Write { connector, path, kind } = goal.holdings.get(0).expect("write holding") else {
        panic!("write")
    };
    assert_eq!((*connector, *kind), (u16::MAX, 0));
    assert_eq!(path.iter().map(client::Bytes::bytes).collect::<Vec<_>>(), [b"one/two".as_slice(), [255, 0].as_slice()]);
    let client::OwnerGoalHolding::Slot { connector, path, kind } = goal.holdings.get(1).expect("slot holding") else {
        panic!("slot")
    };
    assert_eq!((*connector, *kind), (0, u16::MAX));
    assert_eq!(path.len(), 1);
    assert_eq!(path.get(0).expect("empty segment").bytes(), []);
    assert_eq!(goal.tracked, Some(0));
}

#[test]
fn historical_unadmitted_recurrence_is_refused_before_copying_any_raw_goal_field() {
    let (query, mut source) = sources().remove(2);
    let reads::Past::PersonProposal { proposal, .. } = &mut source.value else { panic!("person source") };
    let tasks::PersonAction::Batch(batch) = &mut proposal.action else { panic!("batch fixture") };
    batch.first_mut().expect("proposal member").recurring = Some(Box::new(tasks::RecurringTemplate {
        key: 0,
        batch: Box::new([]),
        overlap: tasks::RecurringOverlap::Wait,
    }));
    let l = Limits { text: 0, turn_text: 0, rows: 0, ..limits() };
    assert_eq!(read(&query, &source, &l, &[]), Err(Invalid::Recurrence));
}

#[test]
fn historical_original_query_and_all_nested_companion_identities_are_checked_before_route_admission() {
    for (query, source) in sources() {
        assert_eq!(read(&client::Read::Me, &source, &limits(), &[]), Err(Invalid::Query));
        assert_eq!(read(&client::Read::Task { task: 0 }, &source, &limits(), &[]), Err(Invalid::Identity));
        let mut wrong = source.clone();
        match &mut wrong.value {
            reads::Past::Task { result, .. } | reads::Past::PersonTask { result, .. } => result.task.number = 0,
            reads::Past::Proposal(proposal) => proposal.proposer = 1,
            reads::Past::PersonProposal { proposal, .. } => {
                let tasks::PersonAction::Batch(batch) = &mut proposal.action else { panic!("batch fixture") };
                batch.first_mut().expect("proposal member").project = 0;
            }
            reads::Past::Escalation { decision, .. } => decision.task = 0,
            reads::Past::Question { question, .. } => question.message = 0,
            reads::Past::Reply(_) | reads::Past::Turn(_) => unreachable!("six object fixtures"),
        }
        assert_eq!(read(&query, &wrong, &limits(), &[]), Err(Invalid::Identity));
    }
}

#[test]
fn historical_question_answer_and_ending_are_independent_of_each_other_and_outer_clocks() {
    let (query, base) = sources().remove(4);
    for mask in 0..4 {
        for outer in [None, Some(Wall::EPOCH)] {
            let mut source = base.clone();
            source.at = outer;
            source.by = None;
            let reads::Past::Question { answer, ending, .. } = &mut source.value else { panic!("question source") };
            if mask & 1 != 0 {
                *answer = Some(tasks::ActivityLine {
                    task: u64::MAX,
                    position: 1,
                    at: Wall::from_nanos(7),
                    line: tasks::Line::Words {
                        message: 0,
                        by: tasks::Party::Person(0),
                        kind: tasks::MessageKind::Answer { question: u64::MAX },
                        words: Box::from(UTF8_TEXTS[1]),
                    },
                });
            }
            if mask & 2 != 0 {
                *ending = Some(Box::new(results::tests::document()));
            }
            let actual = value(&query, &source);
            assert_eq!(actual.at, outer);
            assert_eq!(actual.by, None);
            let client::OwnerHistoricalValue::Question { question, answer, ending } = actual.value else {
                panic!("question facts")
            };
            assert_eq!(question.since, Wall::EPOCH);
            assert_eq!(question.message, u64::MAX);
            assert_eq!(answer.is_some(), mask & 1 != 0);
            assert_eq!(ending.is_some(), mask & 2 != 0);
            if let Some(answer) = answer {
                assert_eq!(answer.position, 1);
                assert_eq!(answer.at, Wall::from_nanos(7));
            }
        }
    }
}

#[test]
fn historical_task_all_eight_optional_source_families_survive_the_complete_envelope() {
    let (query, base) = sources().remove(0);
    for mask in 0_u16..256 {
        let mut source = base.clone();
        let reads::Past::Task { document, .. } = &mut source.value else { panic!("task source") };
        document.goal = (mask & 1 != 0).then_some(0);
        document.priority = (mask & 2 != 0).then_some(u32::MAX);
        document.person_revision = (mask & 4 != 0).then_some(0);
        document.taken_by = (mask & 8 != 0).then_some(0);
        document.instructions =
            (mask & 16 != 0).then_some(jig_domain_core::documents::Text { bytes: Box::new([]), cut: false });
        document.result =
            (mask & 32 != 0).then_some(jig_domain_core::documents::Ending::Failed(jig_domain_core::documents::Text {
                bytes: Box::from(UTF8_TEXTS[2]),
                cut: true,
            }));
        if mask & 64 == 0 {
            document.hold = None;
        }
        if let Some(hold) = &mut document.hold {
            hold.hold_wait_since = (mask & 128 != 0).then_some(Wall::EPOCH);
        }
        let client::OwnerHistoricalValue::Task { document, .. } = value(&query, &source).value else {
            panic!("task facts")
        };
        assert_eq!(document.goal.is_some(), mask & 1 != 0);
        assert_eq!(document.priority.is_some(), mask & 2 != 0);
        assert_eq!(document.person_revision.is_some(), mask & 4 != 0);
        assert_eq!(document.taken_by.is_some(), mask & 8 != 0);
        assert_eq!(document.instructions.is_some(), mask & 16 != 0);
        assert_eq!(document.result.is_some(), mask & 32 != 0);
        assert_eq!(document.hold.is_some(), mask & 64 != 0);
        if let Some(client::TaskHold::Owner(hold)) = document.hold {
            assert_eq!(hold.hold_wait_since.is_some(), mask & 128 != 0);
        }
    }
}

#[test]
fn historical_whole_six_boxed_envelopes_have_exact_receiving_byte_boundaries() {
    for (query, source) in sources() {
        let mut low = 0;
        let mut high = limits().document_bytes;
        while low < high {
            let at = low + (high - low) / 2;
            if read(&query, &source, &Limits { document_bytes: at, turn_text: limits().turn_text, ..limits() }, &[])
                .is_ok()
            {
                high = at;
            } else {
                low = at + 1;
            }
        }
        assert!(low > 0);
        assert_eq!(
            read(&query, &source, &Limits { document_bytes: low - 1, turn_text: limits().turn_text, ..limits() }, &[]),
            Err(Invalid::Payload)
        );
        assert!(
            read(&query, &source, &Limits { document_bytes: low, turn_text: limits().turn_text, ..limits() }, &[])
                .is_ok()
        );
        assert!(
            read(&query, &source, &Limits { document_bytes: low + 1, turn_text: limits().turn_text, ..limits() }, &[])
                .is_ok()
        );
    }
}

#[test]
fn historical_late_nested_allocations_are_refused_whole_without_trimming_empty_reserved_lists() {
    let (query, mut source) = sources().remove(0);
    let reads::Past::Task { result, .. } = &mut source.value else { panic!("task source") };
    result.delegates = List::with_capacity(9);
    assert_eq!(result.delegates.capacity(), 9);
    let before = source.clone();
    assert_eq!(read(&query, &source, &limits(), &[]), Err(Invalid::Owner(owner::Invalid::TooLarge)));
    assert_eq!(source, before);
}

#[test]
fn historical_person_proposal_states_keep_the_independent_whole_decision_and_created_pair() {
    let (query, base) = sources().remove(2);
    let states = [
        tasks::PersonProposalState::Pending { since: Wall::EPOCH },
        tasks::PersonProposalState::Accepted { by: tasks::Party::Person(0) },
        tasks::PersonProposalState::Rejected {
            by: tasks::Party::Deployment { project: 0 },
            reason: Box::from(UTF8_TEXTS[0]),
            message: u64::MAX,
            at: Wall::EPOCH,
        },
    ];
    for state in states {
        for choice in [
            people::ProposalChoice::Accepted,
            people::ProposalChoice::Rejected,
            people::ProposalChoice::Passed,
            people::ProposalChoice::Withdrawn,
            people::ProposalChoice::Stale,
        ] {
            let mut source = base.clone();
            let reads::Past::PersonProposal { proposal, decision } = &mut source.value else { panic!("person source") };
            proposal.state = state.clone();
            *decision = Some(jig_domain_core::ProposalDecisionRecord {
                project: u32::MAX,
                proposer: people::Proposer::Party(u64::MAX),
                proposal: u64::MAX,
                kind: tasks::ProposalKind::Batch,
                by: u64::MAX,
                at: Wall::from_nanos(7),
                choice,
                created: Some((0, u64::MAX)),
            });
            let client::OwnerHistoricalValue::PersonProposal { proposal, decision } = value(&query, &source).value
            else {
                panic!("person facts")
            };
            assert_eq!(proposal.goal.numbers.reserved, 7);
            match &state {
                tasks::PersonProposalState::Withdrawn { .. } | tasks::PersonProposalState::SettingsAccepted { .. } => {
                    unreachable!("ordinary proposal fixture")
                }
                tasks::PersonProposalState::Pending { since } => {
                    assert_eq!(proposal.state, client::OwnerPersonProposalState::Pending { since: *since });
                }
                tasks::PersonProposalState::Accepted { .. } => assert_eq!(
                    proposal.state,
                    client::OwnerPersonProposalState::Accepted { by: client::OwnerRequester::Person(0) }
                ),
                tasks::PersonProposalState::Rejected { .. } => {
                    let client::OwnerPersonProposalState::Rejected { by, reason, message, at } = proposal.state else {
                        panic!("rejection facts")
                    };
                    assert_eq!(by, client::OwnerRequester::Deployment { project: 0 });
                    assert_eq!(reason.bytes(), UTF8_TEXTS[0]);
                    assert_eq!(message, u64::MAX);
                    assert_eq!(at, Wall::EPOCH);
                }
            }
            let decision = decision.expect("independent decision");
            assert_eq!(decision.by, u64::MAX);
            assert_eq!(decision.at, Wall::from_nanos(7));
            assert_eq!(decision.created, Some((0, u64::MAX)));
            assert_eq!(
                decision.choice,
                match choice {
                    people::ProposalChoice::Accepted => client::OwnerProposalChoice::Accepted,
                    people::ProposalChoice::Rejected => client::OwnerProposalChoice::Rejected,
                    people::ProposalChoice::Passed => client::OwnerProposalChoice::Passed,
                    people::ProposalChoice::Withdrawn => client::OwnerProposalChoice::Withdrawn,
                    people::ProposalChoice::Stale => client::OwnerProposalChoice::Stale,
                }
            );
        }
    }
    let mut missing = base;
    let reads::Past::PersonProposal { proposal, .. } = &mut missing.value else { panic!("person source") };
    proposal.state = tasks::PersonProposalState::Accepted { by: tasks::Party::Person(0) };
    assert_eq!(read(&query, &missing, &limits(), &[]), Err(Invalid::Identity));
}

#[test]
fn historical_raw_goal_contracts_keep_opaque_state_closed_machine_fields_and_change_identity() {
    let contracts = [
        tasks::Contract::Approval {
            state: Box::from([255, 0]),
            part: tasks::Part {
                connector: u16::MAX,
                kind: tasks::PartKind::Effect,
                key: tasks::PartKey::Number(u64::MAX),
            },
        },
        tasks::Contract::Report { words: u32::MAX },
        tasks::Contract::Verdict {
            choices: Box::new([tasks::Verdict {
                code: u32::MAX,
                words: 0,
                followups: u32::MAX,
                fields: Box::new([tasks::FieldRule { name: Box::from([0, 255]), max: u32::MAX }]),
            }]),
        },
        tasks::Contract::Change { connector: u16::MAX, kind: 0, words: u32::MAX },
    ];
    for contract in contracts {
        let mut source = raw_goal();
        source.contract = contract;
        let actual = historical_goal::goal(&source, &limits(), &[jig_core_world::world::scripted_model()])
            .expect("whole raw goal");
        match actual.contract {
            client::TaskContractDocument::Approval { state, part } => {
                assert_eq!(*state, [255, 0]);
                assert_eq!(part.connector, u16::MAX);
                assert_eq!(part.key, client::OwnerPartKey::Number(u64::MAX));
            }
            client::TaskContractDocument::Report { words } => assert_eq!(words, u32::MAX),
            client::TaskContractDocument::Verdict { choices, cut } => {
                assert!(!cut);
                assert_eq!(choices.capacity(), 1);
                let choice = choices.get(0).expect("whole choice");
                assert_eq!((choice.code, choice.words, choice.followups), (u32::MAX, 0, u32::MAX));
                assert_eq!(choice.fields.capacity(), 1);
                let field = choice.fields.get(0).expect("whole machine field");
                assert_eq!(*field.name, [0, 255]);
                assert_eq!(field.max, u32::MAX);
            }
            client::TaskContractDocument::Change { connector, kind, words } => {
                assert_eq!((connector, kind, words), (u16::MAX, 0, u32::MAX));
            }
        }
    }
}

const UTF8_TEXTS: [&[u8]; 3] = [b"~\0", b"\0~", b"~"];
