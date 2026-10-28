//! Existing single-read wire literals shared by both protocol sides.
use super::{List, chip_record, fact_bytes, task_record, w};

#[must_use]
pub fn read_ending_record() -> w::Ending {
    let lim = w::CEILINGS;
    let row = w::Fact::Ending(
        w::EndingRow::new(&lim, w::EndingRowParts { parent: None, value: w::EndingFields::Done })
            .expect("literal fixture fits"),
    );
    w::Ending::new(&lim, w::EndingParts { rows: fact_bytes(&row) }).expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_document_specification() -> w::DocumentSpecification {
    let lim = w::CEILINGS;
    w::DocumentSpecification::new(
        &lim,
        w::DocumentSpecificationParts {
            asks: false,
            words: read_wire_document_text(),
            parameters: List::with_capacity(0),
            inputs: List::with_capacity(0),
        },
    )
    .expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_document_text() -> w::DocumentText {
    let lim = w::CEILINGS;
    w::DocumentText::new(&lim, w::DocumentTextParts { bytes: Box::from([]), cut: false }).expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_object_key() -> w::ObjectKey {
    w::ObjectKey::Task(read_wire_object_key_task())
}

#[must_use]
pub fn read_wire_object_key_task() -> w::ObjectKeyTask {
    let lim = w::CEILINGS;
    w::ObjectKeyTask::new(&lim, w::ObjectKeyTaskParts { value: 7 }).expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_owner_escalation() -> w::OwnerEscalation {
    let lim = w::CEILINGS;
    w::OwnerEscalation::new(&lim, w::OwnerEscalationParts { acts: Box::from([]), task: task_record() })
        .expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_owner_historical() -> w::OwnerHistorical {
    let lim = w::CEILINGS;
    w::OwnerHistorical::new(
        &lim,
        w::OwnerHistoricalParts {
            key: read_wire_object_key(),
            value: read_wire_owner_historical_value(),
            at: None,
            by: None,
        },
    )
    .expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_owner_historical_value() -> w::OwnerHistoricalValue {
    w::OwnerHistoricalValue::Task(read_wire_owner_historical_value_task())
}

#[must_use]
pub fn read_wire_owner_historical_value_task() -> w::OwnerHistoricalValueTask {
    let lim = w::CEILINGS;
    w::OwnerHistoricalValueTask::new(
        &lim,
        w::OwnerHistoricalValueTaskParts { document: task_record(), result: read_wire_result() },
    )
    .expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_owner_history_holder() -> w::OwnerHistoryHolder {
    w::OwnerHistoryHolder::Task(read_wire_owner_history_holder_task())
}

#[must_use]
pub fn read_wire_owner_history_holder_task() -> w::OwnerHistoryHolderTask {
    let lim = w::CEILINGS;
    w::OwnerHistoryHolderTask::new(&lim, w::OwnerHistoryHolderTaskParts { value: 7 }).expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_owner_part() -> w::OwnerPart {
    let lim = w::CEILINGS;
    w::OwnerPart::new(
        &lim,
        w::OwnerPartParts { connector: 7, kind: read_wire_owner_part_kind(), key: read_wire_owner_part_key() },
    )
    .expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_owner_part_key() -> w::OwnerPartKey {
    w::OwnerPartKey::Path(read_wire_owner_part_key_path())
}

#[must_use]
pub fn read_wire_owner_part_key_path() -> w::OwnerPartKeyPath {
    let lim = w::CEILINGS;
    w::OwnerPartKeyPath::new(&lim, w::OwnerPartKeyPathParts { value: List::with_capacity(0) })
        .expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_owner_part_kind() -> w::OwnerPartKind {
    w::OwnerPartKind::Resource
}

#[must_use]
pub fn read_wire_owner_person_task() -> w::OwnerPersonTask {
    let lim = w::CEILINGS;
    w::OwnerPersonTask::new(
        &lim,
        w::OwnerPersonTaskParts {
            task: chip_record(),
            revision: read_wire_revision(),
            requester: read_wire_owner_requester(),
            spec: read_wire_specification_document(),
            address: read_wire_person_address(),
            taken_by: None,
            since: None,
            stall: None,
            deadline: None,
            contract: read_wire_task_contract_document(),
            results: Box::from([]),
            cut: false,
            acts: Box::from([]),
        },
    )
    .expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_owner_proposal() -> w::OwnerProposal {
    let lim = w::CEILINGS;
    w::OwnerProposal::new(
        &lim,
        w::OwnerProposalParts {
            key: read_wire_proposal_key(),
            project: 7,
            reason: read_wire_document_text(),
            action: read_wire_owner_proposal_action(),
            as_holder: false,
            waiting: read_wire_owner_proposal_waiting(),
            funding: List::with_capacity(0),
            acts: Box::from([]),
        },
    )
    .expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_owner_proposal_action() -> w::OwnerProposalAction {
    w::OwnerProposalAction::Batch(read_wire_owner_proposal_action_batch())
}

#[must_use]
pub fn read_wire_owner_proposal_action_batch() -> w::OwnerProposalActionBatch {
    let lim = w::CEILINGS;
    w::OwnerProposalActionBatch::new(&lim, w::OwnerProposalActionBatchParts { value: Box::from([]) })
        .expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_owner_proposal_waiting() -> w::OwnerProposalWaiting {
    let lim = w::CEILINGS;
    w::OwnerProposalWaiting::new(
        &lim,
        w::OwnerProposalWaitingParts { holder: read_wire_owner_history_holder(), since: 7 },
    )
    .expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_owner_requester() -> w::OwnerRequester {
    w::OwnerRequester::Task(read_wire_owner_requester_task())
}

#[must_use]
pub fn read_wire_owner_requester_task() -> w::OwnerRequesterTask {
    let lim = w::CEILINGS;
    w::OwnerRequesterTask::new(&lim, w::OwnerRequesterTaskParts { value: 7 }).expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_person_address() -> w::PersonAddress {
    w::PersonAddress::Party(read_wire_person_address_party())
}

#[must_use]
pub fn read_wire_person_address_party() -> w::PersonAddressParty {
    let lim = w::CEILINGS;
    w::PersonAddressParty::new(&lim, w::PersonAddressPartyParts { value: 7 }).expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_proposal_key() -> w::ProposalKey {
    let lim = w::CEILINGS;
    w::ProposalKey::new(&lim, w::ProposalKeyParts { proposer: read_wire_proposer(), number: 7 })
        .expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_proposer() -> w::Proposer {
    w::Proposer::Task(read_wire_proposer_task())
}

#[must_use]
pub fn read_wire_proposer_task() -> w::ProposerTask {
    let lim = w::CEILINGS;
    w::ProposerTask::new(&lim, w::ProposerTaskParts { value: 7 }).expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_question() -> w::Question {
    let lim = w::CEILINGS;
    w::Question::new(
        &lim,
        w::QuestionParts {
            task: chip_record(),
            from: None,
            message: 7,
            words: w::DocumentText::new(&lim, w::DocumentTextParts { bytes: Box::from([]), cut: false }).expect("text"),
            since: 7,
            acts: Box::from([]),
            ending: None,
        },
    )
    .expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_result() -> w::ResultDocument {
    let lim = w::CEILINGS;
    w::ResultDocument::new(
        &lim,
        w::ResultDocumentParts {
            task: chip_record(),
            ending: read_ending_record(),
            value: None,
            at: None,
            details: None,
            acts: Box::from([]),
        },
    )
    .expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_revision() -> w::Revision {
    let lim = w::CEILINGS;
    w::Revision::new(&lim, w::RevisionParts { value: 7 }).expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_specification_document() -> w::SpecificationDocument {
    let lim = w::CEILINGS;
    w::SpecificationDocument::new(
        &lim,
        w::SpecificationDocumentParts { specification: read_wire_document_specification(), cut: false },
    )
    .expect("literal fixture fits")
}

#[must_use]
pub fn read_wire_task_contract_document() -> w::TaskContractDocument {
    w::TaskContractDocument::Approval(read_wire_task_contract_document_approval())
}

#[must_use]
pub fn read_wire_task_contract_document_approval() -> w::TaskContractDocumentApproval {
    let lim = w::CEILINGS;
    w::TaskContractDocumentApproval::new(
        &lim,
        w::TaskContractDocumentApprovalParts { state: Box::from([]), part: read_wire_owner_part() },
    )
    .expect("literal fixture fits")
}
