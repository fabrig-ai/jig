use super::*;
use skein_lib::List;
pub fn read_wire_proposal_action_batch() -> w::ProposalActionBatch {
    let lim = w::CEILINGS;
    w::ProposalActionBatch::new(&lim, w::ProposalActionBatchParts { tasks: Box::from([]), own: false }).unwrap()
}

pub fn read_wire_authority() -> w::Authority {
    let lim = w::CEILINGS;
    w::Authority::new(
        &lim,
        w::AuthorityParts {
            models: None,
            budget: 7,
            grants: Box::from([]),
            task_kinds: List::with_capacity(0),
            depth: 7,
            tasks: 7,
            deadline: None,
            tools: List::with_capacity(0),
            note_scopes: Box::from([]),
        },
    )
    .unwrap()
}

pub fn read_wire_budget() -> w::Budget {
    let lim = w::CEILINGS;
    w::Budget::new(&lim, w::BudgetParts { total: 7, own: 7, ended: 7, reserved: 7, left: 7, funder: read_wire_actor() })
        .unwrap()
}

pub fn read_wire_actor() -> w::Actor {
    w::Actor::Party(read_wire_actor_party())
}

pub fn read_wire_actor_party() -> w::ActorParty {
    let lim = w::CEILINGS;
    w::ActorParty::new(&lim, w::ActorPartyParts { value: 7 }).unwrap()
}

pub fn read_wire_recipient() -> w::Recipient {
    w::Recipient::Task(read_wire_recipient_task())
}

pub fn read_wire_recipient_task() -> w::RecipientTask {
    let lim = w::CEILINGS;
    w::RecipientTask::new(&lim, w::RecipientTaskParts { value: 7 }).unwrap()
}

pub fn read_wire_owner_proposal() -> w::OwnerProposal {
    jig_client_test_records::read_wire_owner_proposal()
}

pub fn read_wire_escalation() -> w::Escalation {
    let lim = w::CEILINGS;
    w::Escalation::new(
        &lim,
        w::EscalationParts {
            ending: None,
            task: chip_record(),
            hold: read_wire_hold(),
            steering: read_wire_steering(),
            impact: read_wire_impact(),
            waiting: read_wire_recipient(),
            trail: Box::from([]),
            acts: Box::from([]),
        },
    )
    .unwrap()
}

pub fn read_wire_hold() -> w::Hold {
    let lim = w::CEILINGS;
    w::Hold::new(
        &lim,
        w::HoldParts {
            placed: None,
            revision: read_wire_revision(),
            reason: read_hold_reason_record(),
            before: read_wire_phase(),
            since: 7,
            result: None,
            spend: List::with_capacity(0),
        },
    )
    .unwrap()
}
