//! Permanent people outcomes and their refusal facts (protocol/store.md, 5.1).
use super::{domain, wire};
use skein_lib::{Token, Wall};
mod facts;
use facts::{
    decode_again, decode_escalation_choice, decode_proposal_choice, encode_again, encode_escalation_choice,
    encode_proposal_choice,
};
#[expect(clippy::too_many_lines, reason = "one exhaustive permanent outcome translation table")]
pub(super) fn encode_outcome(value: &domain::Outcome, limits: &wire::Limits) -> Result<wire::Outcome, wire::Problem> {
    Ok(match value {
        domain::Outcome::MovedToNewest { newest, moved, stayed } => {
            wire::Outcome::MovedToNewest(wire::OutcomeMovedToNewest::new(
                limits,
                wire::OutcomeMovedToNewestParts { newest: *newest, moved: *moved, stayed: *stayed },
            )?)
        }
        domain::Outcome::SettingsChanged { revision } => wire::Outcome::SettingsChanged(
            wire::OutcomeSettingsChanged::new(limits, wire::OutcomeSettingsChangedParts { revision: *revision })?,
        ),
        domain::Outcome::Compound { project, operations, first_message } => {
            wire::Outcome::Compound(wire::OutcomeCompound::new(
                limits,
                wire::OutcomeCompoundParts {
                    project: *project,
                    operations: *operations,
                    first_message: *first_message,
                },
            )?)
        }
        domain::Outcome::CompoundRefused { at, why } => {
            wire::Outcome::CompoundRefused(wire::OutcomeCompoundRefused::new(
                limits,
                wire::OutcomeCompoundRefusedParts { at: *at, why: encode_refusal(why, limits)? },
            )?)
        }
        domain::Outcome::Created { first, last, given } => encode_created(*first, *last, *given, limits)?,
        domain::Outcome::MarkedSeen { seen } => wire::Outcome::MarkedSeen(marked_seen(seen, limits)?),
        domain::Outcome::Applied { connector, token } => wire::Outcome::Applied(wire::OutcomeApplied::new(
            limits,
            wire::OutcomeAppliedParts { connector: *connector, token: token.raw() },
        )?),
        domain::Outcome::Again(value) => wire::Outcome::Again(wire::OutcomeAgain::new(
            limits,
            wire::OutcomeAgainParts { value: encode_again(*value, limits) },
        )?),
        domain::Outcome::AccountKeyKept { source } => wire::Outcome::AccountKeyKept(wire::OutcomeAccountKeyKept::new(
            limits,
            wire::OutcomeAccountKeyKeptParts { source: *source },
        )?),
        domain::Outcome::AccountProgramDeclared { source, name } => {
            wire::Outcome::AccountProgramDeclared(wire::OutcomeAccountProgramDeclared::new(
                limits,
                wire::OutcomeAccountProgramDeclaredParts { source: *source, name: name.clone() },
            )?)
        }
        domain::Outcome::AccountMovingOut { account, number, source } => {
            wire::Outcome::AccountMovingOut(wire::OutcomeAccountMovingOut::new(
                limits,
                wire::OutcomeAccountMovingOutParts { account: *account, number: *number, source: *source },
            )?)
        }
        domain::Outcome::AccountFinishing { source } => wire::Outcome::AccountFinishing(
            wire::OutcomeAccountFinishing::new(limits, wire::OutcomeAccountFinishingParts { source: *source })?,
        ),
        domain::Outcome::AccountMovedIn { account, renewed } => {
            wire::Outcome::AccountMovedIn(wire::OutcomeAccountMovedIn::new(
                limits,
                wire::OutcomeAccountMovedInParts { account: *account, renewed: *renewed },
            )?)
        }
        domain::Outcome::AccountChanged => wire::Outcome::AccountChanged,
        domain::Outcome::AccountRemoved { confirmed, failed_hosts } => {
            wire::Outcome::AccountRemoved(wire::OutcomeAccountRemoved::new(
                limits,
                wire::OutcomeAccountRemovedParts {
                    confirmed: *confirmed,
                    failed_hosts: super::account_hosts::encode(failed_hosts, limits)?,
                },
            )?)
        }
        domain::Outcome::SignedOut => wire::Outcome::SignedOut,
        domain::Outcome::AccountSignInStarted { source, address } => account_sign_in_started(*source, address, limits)?,
        domain::Outcome::AccountSignedOut { confirmed, failed_hosts } => {
            wire::Outcome::AccountSignedOut(wire::OutcomeAccountSignedOut::new(
                limits,
                wire::OutcomeAccountSignedOutParts {
                    confirmed: *confirmed,
                    failed_hosts: super::account_hosts::encode(failed_hosts, limits)?,
                },
            )?)
        }
        domain::Outcome::NoteEdited { name } => wire::Outcome::NoteEdited(wire::OutcomeNoteEdited::new(
            limits,
            wire::OutcomeNoteEditedParts { name: *name },
        )?),
        domain::Outcome::ServiceMade { person } => wire::Outcome::ServiceMade(wire::OutcomeServiceMade::new(
            limits,
            wire::OutcomeServiceMadeParts { person: *person },
        )?),
        domain::Outcome::Adopted { project } => {
            wire::Outcome::Adopted(wire::OutcomeAdopted::new(limits, wire::OutcomeAdoptedParts { project: *project })?)
        }
        domain::Outcome::Proposed { proposal, to } => encode_proposed(*proposal, *to, limits)?,
        domain::Outcome::Withdrawn { proposal } => encode_withdrawn(*proposal, limits)?,
        domain::Outcome::Stopped { task } => {
            wire::Outcome::Stopped(wire::OutcomeStopped::new(limits, wire::OutcomeStoppedParts { task: *task })?)
        }
        domain::Outcome::Cancelled { task } => {
            wire::Outcome::Cancelled(wire::OutcomeCancelled::new(limits, wire::OutcomeCancelledParts { task: *task })?)
        }
        domain::Outcome::Placed { hold, revision } => wire::Outcome::Placed(wire::OutcomePlaced::new(
            limits,
            wire::OutcomePlacedParts { hold: *hold, revision: *revision },
        )?),
        domain::Outcome::Lifted { hold, released } => wire::Outcome::Lifted(wire::OutcomeLifted::new(
            limits,
            wire::OutcomeLiftedParts { hold: *hold, released: *released },
        )?),
        domain::Outcome::Released { task } => {
            wire::Outcome::Released(wire::OutcomeReleased::new(limits, wire::OutcomeReleasedParts { task: *task })?)
        }
        domain::Outcome::PersonTaken { task } => wire::Outcome::PersonTaken(wire::OutcomePersonTaken::new(
            limits,
            wire::OutcomePersonTakenParts { task: *task },
        )?),
        domain::Outcome::PersonHandedBack { task } => wire::Outcome::PersonHandedBack(
            wire::OutcomePersonHandedBack::new(limits, wire::OutcomePersonHandedBackParts { task: *task })?,
        ),
        domain::Outcome::PersonAnswered { task } => wire::Outcome::PersonAnswered(wire::OutcomePersonAnswered::new(
            limits,
            wire::OutcomePersonAnsweredParts { task: *task },
        )?),
        domain::Outcome::Moved { task } => {
            wire::Outcome::Moved(wire::OutcomeMoved::new(limits, wire::OutcomeMovedParts { task: *task })?)
        }
        domain::Outcome::ProposalDecided { proposer, proposal, by, at, choice, repeated } => {
            wire::Outcome::ProposalDecided(encode_proposal_decided(
                proposer, *proposal, *by, *at, *choice, *repeated, limits,
            )?)
        }
        domain::Outcome::Said { task, message } => wire::Outcome::Said(encode_said(*task, *message, limits)?),
        domain::Outcome::QuestionAnswered { task, question, message } => {
            wire::Outcome::QuestionAnswered(encode_question_answered(*task, *question, *message, limits)?)
        }
        domain::Outcome::Amended { task, released } => wire::Outcome::Amended(wire::OutcomeAmended::new(
            limits,
            wire::OutcomeAmendedParts { task: *task, released: *released },
        )?),

        domain::Outcome::RolesSet { project } => wire::Outcome::RolesSet(wire::OutcomeRolesSet::new(
            limits,
            wire::OutcomeRolesSetParts { project: *project },
        )?),
        domain::Outcome::Renamed { project } => {
            wire::Outcome::Renamed(wire::OutcomeRenamed::new(limits, wire::OutcomeRenamedParts { project: *project })?)
        }
        domain::Outcome::ProjectMade { project } => wire::Outcome::ProjectMade(wire::OutcomeProjectMade::new(
            limits,
            wire::OutcomeProjectMadeParts { project: *project },
        )?),
        domain::Outcome::PolicyChanged { project } => wire::Outcome::PolicyChanged(wire::OutcomePolicyChanged::new(
            limits,
            wire::OutcomePolicyChangedParts { project: *project },
        )?),
        domain::Outcome::PoolSet { project, person } => wire::Outcome::PoolSet(wire::OutcomePoolSet::new(
            limits,
            wire::OutcomePoolSetParts { project: *project, person: *person },
        )?),
        domain::Outcome::EscalationDecided { task, revision, by, at, choice } => {
            wire::Outcome::EscalationDecided(encode_escalation_decided(*task, *revision, *by, *at, *choice, limits)?)
        }
        domain::Outcome::Refused(value) => wire::Outcome::Refused(wire::OutcomeRefused::new(
            limits,
            wire::OutcomeRefusedParts { value: encode_refusal(value, limits)? },
        )?),
    })
}
fn account_sign_in_started(source: u32, address: &[u8], limits: &wire::Limits) -> Result<wire::Outcome, wire::Problem> {
    let parts = wire::OutcomeAccountSignInStartedParts { source, address: skein_lib::bytes::copy_of(address) };
    Ok(wire::Outcome::AccountSignInStarted(wire::OutcomeAccountSignInStarted::new(limits, parts)?))
}

#[expect(clippy::too_many_lines, reason = "one exhaustive permanent outcome translation table")]
pub(super) fn decode_outcome(value: wire::Outcome) -> domain::Outcome {
    match value {
        wire::Outcome::MovedToNewest(value) => {
            domain::Outcome::MovedToNewest { newest: value.newest(), moved: value.moved(), stayed: value.stayed() }
        }
        wire::Outcome::SettingsChanged(value) => domain::Outcome::SettingsChanged { revision: value.revision() },
        wire::Outcome::Compound(value) => {
            let wire::OutcomeCompoundParts { project, operations, first_message } = value.into_parts();
            domain::Outcome::Compound { project, operations, first_message }
        }
        wire::Outcome::CompoundRefused(value) => {
            let wire::OutcomeCompoundRefusedParts { at, why } = value.into_parts();
            domain::Outcome::CompoundRefused { at, why: decode_refusal(why) }
        }
        wire::Outcome::Created(value) => decode_created(value),
        wire::Outcome::MarkedSeen(value) => decode_marked_seen(value),
        wire::Outcome::Applied(value) => {
            let wire::OutcomeAppliedParts { connector, token } = value.into_parts();
            domain::Outcome::Applied { connector, token: Token::new(token) }
        }
        wire::Outcome::Again(value) => {
            let wire::OutcomeAgainParts { value } = value.into_parts();
            domain::Outcome::Again(decode_again(value))
        }
        wire::Outcome::AccountKeyKept(value) => domain::Outcome::AccountKeyKept { source: value.into_parts().source },
        wire::Outcome::AccountProgramDeclared(value) => domain::Outcome::AccountProgramDeclared {
            source: value.source(),
            name: skein_lib::bytes::copy_of(value.name()),
        },
        wire::Outcome::AccountMovingOut(value) => {
            let wire::OutcomeAccountMovingOutParts { account, number, source } = value.into_parts();
            domain::Outcome::AccountMovingOut { account, number, source }
        }
        wire::Outcome::AccountFinishing(row) => domain::Outcome::AccountFinishing { source: row.source() },
        wire::Outcome::AccountMovedIn(value) => {
            let wire::OutcomeAccountMovedInParts { account, renewed } = value.into_parts();
            domain::Outcome::AccountMovedIn { account, renewed }
        }
        wire::Outcome::AccountChanged => domain::Outcome::AccountChanged,
        wire::Outcome::AccountRemoved(value) => domain::Outcome::AccountRemoved {
            confirmed: value.confirmed(),
            failed_hosts: super::account_hosts::decode(value.failed_hosts()),
        },
        wire::Outcome::SignedOut => domain::Outcome::SignedOut,
        wire::Outcome::AccountSignInStarted(value) => {
            let wire::OutcomeAccountSignInStartedParts { source, address } = value.into_parts();
            domain::Outcome::AccountSignInStarted { source, address }
        }
        wire::Outcome::AccountSignedOut(value) => domain::Outcome::AccountSignedOut {
            confirmed: value.confirmed(),
            failed_hosts: super::account_hosts::decode(value.failed_hosts()),
        },
        wire::Outcome::NoteEdited(value) => {
            let wire::OutcomeNoteEditedParts { name } = value.into_parts();
            domain::Outcome::NoteEdited { name }
        }
        wire::Outcome::ServiceMade(value) => {
            let wire::OutcomeServiceMadeParts { person } = value.into_parts();
            domain::Outcome::ServiceMade { person }
        }
        wire::Outcome::Adopted(value) => {
            let wire::OutcomeAdoptedParts { project } = value.into_parts();
            domain::Outcome::Adopted { project }
        }
        wire::Outcome::Proposed(value) => {
            let wire::OutcomeProposedParts { proposal, to } = value.into_parts();
            domain::Outcome::Proposed { proposal, to }
        }
        wire::Outcome::Withdrawn(value) => domain::Outcome::Withdrawn { proposal: value.into_parts().proposal },
        wire::Outcome::Stopped(value) => {
            let wire::OutcomeStoppedParts { task } = value.into_parts();
            domain::Outcome::Stopped { task }
        }
        wire::Outcome::Cancelled(value) => {
            let wire::OutcomeCancelledParts { task } = value.into_parts();
            domain::Outcome::Cancelled { task }
        }
        wire::Outcome::Placed(value) => domain::Outcome::Placed { hold: value.hold(), revision: value.revision() },
        wire::Outcome::Lifted(value) => {
            let wire::OutcomeLiftedParts { hold, released } = value.into_parts();
            domain::Outcome::Lifted { hold, released }
        }
        wire::Outcome::Released(value) => {
            let wire::OutcomeReleasedParts { task } = value.into_parts();
            domain::Outcome::Released { task }
        }
        wire::Outcome::PersonTaken(value) => {
            let wire::OutcomePersonTakenParts { task } = value.into_parts();
            domain::Outcome::PersonTaken { task }
        }
        wire::Outcome::PersonHandedBack(value) => {
            let wire::OutcomePersonHandedBackParts { task } = value.into_parts();
            domain::Outcome::PersonHandedBack { task }
        }
        wire::Outcome::PersonAnswered(value) => {
            let wire::OutcomePersonAnsweredParts { task } = value.into_parts();
            domain::Outcome::PersonAnswered { task }
        }
        wire::Outcome::Moved(value) => {
            let wire::OutcomeMovedParts { task } = value.into_parts();
            domain::Outcome::Moved { task }
        }
        wire::Outcome::ProposalDecided(value) => decode_proposal_decided(value),
        wire::Outcome::Said(value) => {
            let wire::OutcomeSaidParts { task, message } = value.into_parts();
            domain::Outcome::Said { task, message }
        }
        wire::Outcome::QuestionAnswered(value) => decode_question_answered(value),
        wire::Outcome::Amended(value) => {
            let wire::OutcomeAmendedParts { task, released } = value.into_parts();
            domain::Outcome::Amended { task, released }
        }
        wire::Outcome::RolesSet(value) => {
            let wire::OutcomeRolesSetParts { project } = value.into_parts();
            domain::Outcome::RolesSet { project }
        }
        wire::Outcome::Renamed(value) => domain::Outcome::Renamed { project: value.project() },
        wire::Outcome::ProjectMade(value) => domain::Outcome::ProjectMade { project: value.project() },
        wire::Outcome::PolicyChanged(value) => {
            let wire::OutcomePolicyChangedParts { project } = value.into_parts();
            domain::Outcome::PolicyChanged { project }
        }
        wire::Outcome::PoolSet(value) => {
            let wire::OutcomePoolSetParts { project, person } = value.into_parts();
            domain::Outcome::PoolSet { project, person }
        }
        wire::Outcome::EscalationDecided(value) => decode_escalation_decided(value),
        wire::Outcome::Refused(value) => {
            let wire::OutcomeRefusedParts { value } = value.into_parts();
            domain::Outcome::Refused(decode_refusal(value))
        }
    }
}
fn encode_proposal_decided(
    proposer: &domain::Proposer,
    proposal: u64,
    by: u64,
    at: Wall,
    choice: domain::ProposalChoice,
    repeated: bool,
    limits: &wire::Limits,
) -> Result<wire::OutcomeProposalDecided, wire::Problem> {
    wire::OutcomeProposalDecided::new(
        limits,
        wire::OutcomeProposalDecidedParts {
            proposer: super::asks::encode_proposer(proposer, limits)?,
            proposal,
            by,
            at: at.as_nanos(),
            choice: encode_proposal_choice(choice, limits),
            repeated,
        },
    )
}
fn decode_proposal_decided(value: wire::OutcomeProposalDecided) -> domain::Outcome {
    let wire::OutcomeProposalDecidedParts { proposer, proposal, by, at, choice, repeated } = value.into_parts();
    domain::Outcome::ProposalDecided {
        proposer: super::asks::decode_proposer(proposer),
        proposal,
        by,
        at: Wall::from_nanos(at),
        choice: decode_proposal_choice(choice),
        repeated,
    }
}
fn encode_escalation_decided(
    task: u64,
    revision: u64,
    by: u64,
    at: Wall,
    choice: domain::EscalationChoice,
    limits: &wire::Limits,
) -> Result<wire::OutcomeEscalationDecided, wire::Problem> {
    wire::OutcomeEscalationDecided::new(
        limits,
        wire::OutcomeEscalationDecidedParts {
            task,
            revision,
            by,
            at: at.as_nanos(),
            choice: encode_escalation_choice(choice, limits),
        },
    )
}
fn decode_escalation_decided(value: wire::OutcomeEscalationDecided) -> domain::Outcome {
    let wire::OutcomeEscalationDecidedParts { task, revision, by, at, choice } = value.into_parts();
    domain::Outcome::EscalationDecided {
        task,
        revision,
        by,
        at: Wall::from_nanos(at),
        choice: decode_escalation_choice(choice),
    }
}
fn encode_question_answered(
    task: u64,
    question: u64,
    message: u64,
    limits: &wire::Limits,
) -> Result<wire::OutcomeQuestionAnswered, wire::Problem> {
    wire::OutcomeQuestionAnswered::new(limits, wire::OutcomeQuestionAnsweredParts { task, question, message })
}
fn decode_question_answered(value: wire::OutcomeQuestionAnswered) -> domain::Outcome {
    let wire::OutcomeQuestionAnsweredParts { task, question, message } = value.into_parts();
    domain::Outcome::QuestionAnswered { task, question, message }
}
fn encode_said(task: u64, message: u64, limits: &wire::Limits) -> Result<wire::OutcomeSaid, wire::Problem> {
    wire::OutcomeSaid::new(limits, wire::OutcomeSaidParts { task, message })
}

fn marked_seen(seen: &domain::Seen, limits: &wire::Limits) -> Result<wire::OutcomeMarkedSeen, wire::Problem> {
    wire::OutcomeMarkedSeen::new(limits, wire::OutcomeMarkedSeenParts { seen: super::asks::encode_seen(seen, limits)? })
}

fn encode_created(
    first: u64,
    last: u64,
    given: Option<domain::Given>,
    limits: &wire::Limits,
) -> Result<wire::Outcome, wire::Problem> {
    Ok(wire::Outcome::Created(wire::OutcomeCreated::new(
        limits,
        wire::OutcomeCreatedParts {
            first,
            last,
            given: match given {
                Some(domain::Given { task, amount }) => {
                    Some(wire::Given::new(limits, wire::GivenParts { task, amount })?)
                }
                None => None,
            },
        },
    )?))
}
fn decode_created(value: wire::OutcomeCreated) -> domain::Outcome {
    let wire::OutcomeCreatedParts { first, last, given } = value.into_parts();
    let mut amount = None;
    if let Some(value) = given {
        let wire::GivenParts { task, amount: value } = value.into_parts();
        amount = Some(domain::Given { task, amount: value });
    }
    domain::Outcome::Created { first, last, given: amount }
}

fn decode_marked_seen(value: wire::OutcomeMarkedSeen) -> domain::Outcome {
    let wire::OutcomeMarkedSeenParts { seen } = value.into_parts();
    domain::Outcome::MarkedSeen { seen: super::asks::decode_seen(seen) }
}

fn encode_proposed(proposal: u64, to: u32, limits: &wire::Limits) -> Result<wire::Outcome, wire::Problem> {
    Ok(wire::Outcome::Proposed(wire::OutcomeProposed::new(limits, wire::OutcomeProposedParts { proposal, to })?))
}
fn encode_withdrawn(proposal: u64, limits: &wire::Limits) -> Result<wire::Outcome, wire::Problem> {
    Ok(wire::Outcome::Withdrawn(wire::OutcomeWithdrawn::new(limits, wire::OutcomeWithdrawnParts { proposal })?))
}

pub(crate) use facts::{decode_refusal, encode_refusal};
