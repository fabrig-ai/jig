//! Facts read from delivered documents (client/domain/client.md, 19;
//! contract.md, 5.3–5.4).
use crate::contract::{
    ActKind, Admission, Chip, ChipPhase, Effect, HeldRole, HoldCategory, Me, Offered, OwnerChipDetails, OwnerChipPhase,
    OwnerChipWork, RetryNotice, RunStage, WaitedFor, WaitsOn, Whom,
};
use alloc::boxed::Box;
use skein_lib::Wall;

/// A chat's status, in the precedence of client/domain/client.md, 19.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatStatus {
    Starting,
    HostRefused(crate::contract::TaskWaitRefused),
    Writing,
    Tool { tool: Box<[u8]>, since: Wall },
    Working { since: Wall },
    WaitingForYou { kind: WaitedFor, entries: u32 },
    WaitingFor(Whom),
    Parked,
    Retrying { until: Wall, notice: RetryNotice },
    Held(HoldCategory),
    Closing,
    Ended { at: Wall },
}

/// Read the same facts for a task query's line and a task's page.
#[must_use]
pub fn chat_status(
    chip: &Chip,
    waits_on: &WaitsOn,
    run_wait: Option<&crate::contract::TaskRunWait>,
    project: u32,
    me: &Me,
) -> ChatStatus {
    match chip.details.as_ref() {
        ChipPhase::Owner { phase, details } => match phase {
            OwnerChipPhase::Held { reason, .. } => ChatStatus::Held(*reason),
            OwnerChipPhase::Closing(_) => ChatStatus::Closing,
            OwnerChipPhase::Ended => ChatStatus::Ended { at: details.ended_at.expect("ended chip has its end time") },
            OwnerChipPhase::Active(stage) => {
                if let Some(waiting) = waiting(waits_on, project, me) {
                    return waiting;
                }
                let unplaced = match stage {
                    RunStage::Due | RunStage::Preparing => true,
                    RunStage::Idle
                    | RunStage::Claimed { .. }
                    | RunStage::Running { .. }
                    | RunStage::BackingOff { .. } => false,
                };
                if unplaced && let Some(crate::contract::TaskRunWait::Refused(why)) = run_wait {
                    return ChatStatus::HostRefused(*why);
                }
                active(*stage, details)
            }
            OwnerChipPhase::Waiting { .. } | OwnerChipPhase::Person { .. } => ChatStatus::Starting,
        },
        ChipPhase::Active(_)
        | ChipPhase::Waiting { .. }
        | ChipPhase::Idle { .. }
        | ChipPhase::Starting
        | ChipPhase::Running { .. }
        | ChipPhase::WaitingForWords { .. }
        | ChipPhase::BackingOff { .. }
        | ChipPhase::Procedure { .. }
        | ChipPhase::Person { .. }
        | ChipPhase::Closing(_)
        | ChipPhase::Held { .. }
        | ChipPhase::Ended { .. } => unreachable!("delivered chips carry their whole owner phase"),
    }
}
fn active(stage: RunStage, details: &OwnerChipDetails) -> ChatStatus {
    match stage {
        RunStage::BackingOff { until } => {
            ChatStatus::Retrying { until, notice: details.retry.expect("backing off records its retry decision") }
        }
        RunStage::Idle => ChatStatus::Parked,
        RunStage::Due | RunStage::Preparing => ChatStatus::Starting,
        RunStage::Claimed { .. } | RunStage::Running { .. } => match &details.observation {
            Some(observation) => match &observation.work {
                OwnerChipWork::Preparing | OwnerChipWork::Starting => ChatStatus::Starting,
                OwnerChipWork::Writing => ChatStatus::Writing,
                OwnerChipWork::Tool { tool, .. } => ChatStatus::Tool { tool: tool.clone(), since: observation.since },
                OwnerChipWork::Waiting => ChatStatus::Parked,
                OwnerChipWork::Active | OwnerChipWork::Ending => ChatStatus::Working { since: observation.since },
            },
            None => match stage {
                RunStage::Claimed { .. } => ChatStatus::Starting,
                RunStage::Running { .. } => ChatStatus::Working { since: details.phase_since },
                RunStage::Idle | RunStage::Due | RunStage::Preparing | RunStage::BackingOff { .. } => {
                    unreachable!("only placed stages read observations")
                }
            },
        },
    }
}
fn waiting(waits_on: &WaitsOn, project: u32, me: &Me) -> Option<ChatStatus> {
    match waits_on {
        WaitsOn::Party { whom, kind, entries, .. } => {
            if reader(*whom, project, me) {
                Some(ChatStatus::WaitingForYou { kind: *kind, entries: *entries })
            } else {
                Some(ChatStatus::WaitingFor(*whom))
            }
        }
        WaitsOn::Dependencies { .. } => Some(ChatStatus::Parked),
        WaitsOn::Delegates | WaitsOn::News | WaitsOn::Nothing => None,
    }
}
fn reader(whom: Whom, project: u32, me: &Me) -> bool {
    match whom {
        Whom::Person(person) => person == me.party.number,
        Whom::Role { project: held_project, role } => {
            if held_project != project {
                return false;
            }
            for held in &me.projects {
                let number = match held.role {
                    HeldRole::Owner => 0,
                    HeldRole::Maintainer => 1,
                    HeldRole::Member => 2,
                    HeldRole::Observer => 3,
                    HeldRole::Policy { role } => role,
                };
                if held.project == project && number == role {
                    return true;
                }
            }
            false
        }
    }
}
/// The consequence supplied with this act, if it was offered with one.
#[must_use]
pub fn effect(offers: &[Offered], act: ActKind) -> Option<&Effect> {
    for offered in offers {
        if offered.act == act {
            return offered.effect.as_ref();
        }
    }
    None
}
/// The engine's admission, without inferring any permission.
#[must_use]
pub fn offer(offers: &[Offered], act: ActKind) -> Option<&Admission> {
    for offered in offers {
        if offered.act == act {
            return Some(&offered.admission);
        }
    }
    None
}
/// Dangerous acts, which a presentation that confirms marks so (client/domain/client.md, 19).
#[must_use]
pub const fn dangerous(act: ActKind) -> bool {
    match act {
        ActKind::Cancel | ActKind::Reject | ActKind::DeleteNote => true,
        ActKind::Message
        | ActKind::Answer
        | ActKind::Take
        | ActKind::HandBack
        | ActKind::Accept
        | ActKind::PassUp
        | ActKind::Release
        | ActKind::LeaveHeld
        | ActKind::Amend
        | ActKind::Stop
        | ActKind::Move
        | ActKind::Reorder
        | ActKind::Seen
        | ActKind::EditNote
        | ActKind::Adopt
        | ActKind::Rename
        | ActKind::ChangePolicy
        | ActKind::SetRoles
        | ActKind::MakeService
        | ActKind::SetPool
        | ActKind::Withdraw => false,
    }
}

/// Facts a deciding act must show, read only from its delivered document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionFacts {
    pub dangerous: bool,
    pub effect: Option<Effect>,
    pub funding: skein_lib::List<crate::contract::OwnerFundingOffer>,
    pub funds: Option<Funds>,
}
/// The proposal document's supplied budget and checked remainder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Funds {
    pub needed: u64,
    pub left: u64,
    pub after: u64,
}
/// Read decision facts; a missing document still carries the act's danger.
#[must_use]
pub fn decision(document: Option<&crate::contract::ReadValue>, act: ActKind) -> DecisionFacts {
    use crate::contract::ReadValue;
    let mut value = DecisionFacts {
        dangerous: dangerous(act),
        effect: None,
        funding: skein_lib::List::with_capacity(0),
        funds: None,
    };
    if let Some(document) = document {
        if let Some(offers) = decision_offers(document) {
            value.effect = effect(offers.as_slice(), act).copied();
        }
        if act == ActKind::Accept {
            match document {
                ReadValue::OwnerProposal(proposal) => value.funding = proposal.funding.clone(),
                ReadValue::Proposal(proposal) => {
                    if let Some(after) = proposal.funding.left.checked_sub(proposal.needs.budget) {
                        value.funds = Some(Funds { needed: proposal.needs.budget, left: proposal.funding.left, after });
                    }
                }
                ReadValue::Task(_)
                | ReadValue::Escalation(_)
                | ReadValue::OwnerEscalation(_)
                | ReadValue::PersonTask(_)
                | ReadValue::OwnerPersonTask(_)
                | ReadValue::Me(_)
                | ReadValue::Tasks(_)
                | ReadValue::Historical(_)
                | ReadValue::Question(_)
                | ReadValue::Result(_)
                | ReadValue::Ended(_)
                | ReadValue::History(_)
                | ReadValue::Activity(_)
                | ReadValue::Turn(_)
                | ReadValue::Inbox(_)
                | ReadValue::OwnerInbox(_)
                | ReadValue::Notes(_)
                | ReadValue::Holds(_)
                | ReadValue::Spend(_)
                | ReadValue::OwnerEnded(_)
                | ReadValue::OwnerSpend(_)
                | ReadValue::OwnerProject(_)
                | ReadValue::Project(_)
                | ReadValue::Deployment(_)
                | ReadValue::Settings(_)
                | ReadValue::Setting(_)
                | ReadValue::Revision(_)
                | ReadValue::Revisions(_)
                | ReadValue::OwnerDeployment(_) => {}
            }
        }
    }
    value
}
fn decision_offers(value: &crate::contract::ReadValue) -> Option<&skein_lib::List<Offered>> {
    use crate::contract::ReadValue;
    match value {
        ReadValue::Task(value) => Some(&value.acts),
        ReadValue::Escalation(value) => Some(&value.acts),
        ReadValue::OwnerEscalation(value) => Some(&value.acts),
        ReadValue::PersonTask(value) => Some(&value.acts),
        ReadValue::OwnerPersonTask(value) => Some(&value.acts),
        ReadValue::Proposal(value) => Some(&value.acts),
        ReadValue::OwnerProposal(value) => Some(&value.acts),
        ReadValue::Me(_)
        | ReadValue::Tasks(_)
        | ReadValue::Historical(_)
        | ReadValue::Question(_)
        | ReadValue::Result(_)
        | ReadValue::Ended(_)
        | ReadValue::History(_)
        | ReadValue::Activity(_)
        | ReadValue::Turn(_)
        | ReadValue::Inbox(_)
        | ReadValue::OwnerInbox(_)
        | ReadValue::Notes(_)
        | ReadValue::Holds(_)
        | ReadValue::Spend(_)
        | ReadValue::OwnerEnded(_)
        | ReadValue::OwnerSpend(_)
        | ReadValue::OwnerProject(_)
        | ReadValue::Project(_)
        | ReadValue::Deployment(_)
        | ReadValue::Settings(_)
        | ReadValue::Setting(_)
        | ReadValue::Revision(_)
        | ReadValue::Revisions(_)
        | ReadValue::OwnerDeployment(_) => None,
    }
}
