//! Named people refusal and outcome fixtures (protocol/store.md, 5.1).
use jig_domain_people as domain;
use skein_lib::{Token, Wall};
pub fn again_values() -> Box<[domain::Again]> {
    Box::new([domain::Again::Busy, domain::Again::NotReady])
}
pub fn failure_class_values() -> Box<[domain::FailureClass]> {
    Box::new([
        domain::FailureClass::Transient,
        domain::FailureClass::Permanent,
        domain::FailureClass::Run,
        domain::FailureClass::Agent,
        domain::FailureClass::Lost,
        domain::FailureClass::Invalid,
    ])
}
pub fn hold_reason_values() -> Box<[domain::HoldReason]> {
    Box::new([
        domain::HoldReason::Exceeded(domain::Exceeded { bound: domain::Bound::Reasoning, value: u64::MAX }),
        domain::HoldReason::Exceeded(domain::Exceeded { bound: domain::Bound::Metadata, value: u64::MAX }),
        domain::HoldReason::Exceeded(domain::Exceeded { bound: domain::Bound::Items, value: u64::MAX }),
        domain::HoldReason::Exceeded(domain::Exceeded { bound: domain::Bound::Answer, value: u64::MAX }),
        domain::HoldReason::Exceeded(domain::Exceeded { bound: domain::Bound::Tools, value: u64::MAX }),
        domain::HoldReason::Exceeded(domain::Exceeded { bound: domain::Bound::HistoryItems, value: u64::MAX }),
        domain::HoldReason::Exceeded(domain::Exceeded { bound: domain::Bound::History, value: u64::MAX }),
        domain::HoldReason::Exceeded(domain::Exceeded { bound: domain::Bound::Window, value: u64::MAX }),
        domain::HoldReason::Exceeded(domain::Exceeded { bound: domain::Bound::Output, value: u64::MAX }),
        domain::HoldReason::Failures { class: domain::FailureClass::Lost },
        domain::HoldReason::Stalled,
        domain::HoldReason::Dependency { task: 2 },
        domain::HoldReason::Stopped,
        domain::HoldReason::Placed { hold: 2 },
        domain::HoldReason::Drift,
        domain::HoldReason::Effects,
        domain::HoldReason::EffectFailed,
        domain::HoldReason::Uncertain { entry: 2 },
        domain::HoldReason::Budget,
        domain::HoldReason::Deadline,
        domain::HoldReason::HoldsWaited,
        domain::HoldReason::ResourceUnavailable,
        domain::HoldReason::ResourceAccess,
        domain::HoldReason::PoolLost { pool: 2 },
        domain::HoldReason::Procedure,
    ])
}
pub fn moved_values() -> Box<[domain::Moved]> {
    Box::new([
        domain::Moved::Hold { revision: 2, reason: domain::HoldReason::Budget },
        domain::Moved::Spec { revision: 2 },
        domain::Moved::Person { revision: 2 },
        domain::Moved::Note { revision: 2 },
    ])
}
pub fn party_values() -> Box<[domain::Party]> {
    Box::new([domain::Party::Person(2), domain::Party::Task(2), domain::Party::Deployment { project: 2 }])
}
pub fn ending_values() -> Box<[domain::Ending]> {
    Box::new([
        domain::Ending::Done,
        domain::Ending::Failed,
        domain::Ending::Cancelled { by: domain::Party::Person(2), at: Wall::from_nanos(19) },
        domain::Ending::Decided { by: 2, at: Wall::from_nanos(19) },
        domain::Ending::Deleted,
    ])
}
pub fn lack_values() -> Box<[domain::Lack]> {
    Box::new([
        domain::Lack::Approver,
        domain::Lack::Budget { needs: 2, left: 3 },
        domain::Lack::Grant { kind: 2 },
        domain::Lack::Funding,
        domain::Lack::TaskKind { executor: None },
        domain::Lack::TaskKind { executor: Some(domain::TaskExecutor::Person(domain::PersonAddress::Person(2))) },
        domain::Lack::Depth { most: 2 },
        domain::Lack::Deadline,
        domain::Lack::Tasks { left: 2 },
        domain::Lack::Tool,
        domain::Lack::Scope,
        domain::Lack::Models,
        domain::Lack::Reference,
        domain::Lack::ResourceAccess,
        domain::Lack::ResourceUnavailable,
        domain::Lack::Ceiling(domain::Ceiling {
            holder: domain::CeilingHolder::Rules,
            role: None,
            dimension: domain::CeilingDimension::PeriodSpend,
            value: Some(449_696),
            edit: Some(449_697),
        }),
        domain::Lack::Shape,
        domain::Lack::Again { required: true },
        domain::Lack::Account,
        domain::Lack::Writer,
        domain::Lack::Required { connector: 2, kind: 3 },
        domain::Lack::Failed { connector: 2, kind: 3 },
        domain::Lack::Unguarded { connector: 2, kind: 3 },
    ])
}
pub fn limit_kind_values() -> Box<[domain::LimitKind]> {
    Box::new([
        domain::LimitKind::Operations,
        domain::LimitKind::Words,
        domain::LimitKind::Identity,
        domain::LimitKind::Roles,
        domain::LimitKind::Arithmetic,
        domain::LimitKind::Duplicate,
        domain::LimitKind::Empty,
        domain::LimitKind::Batch,
        domain::LimitKind::ProjectLiveTasks,
        domain::LimitKind::LiveTasks,
        domain::LimitKind::Tree,
        domain::LimitKind::Depth,
        domain::LimitKind::Delegates,
        domain::LimitKind::Subscription,
        domain::LimitKind::Dependencies,
        domain::LimitKind::Cycle,
        domain::LimitKind::Spec,
        domain::LimitKind::Contract,
        domain::LimitKind::Authority,
        domain::LimitKind::Inputs,
        domain::LimitKind::Attempts,
        domain::LimitKind::LiveDelegates,
        domain::LimitKind::Restore,
        domain::LimitKind::Read,
        domain::LimitKind::Turn,
        domain::LimitKind::HoldKind,
        domain::LimitKind::HoldTaken,
        domain::LimitKind::Holds,
        domain::LimitKind::InboxFull,
        domain::LimitKind::Notes,
        domain::LimitKind::Revision,
        domain::LimitKind::Question,
        domain::LimitKind::Priority,
        domain::LimitKind::Policy,
        domain::LimitKind::Resource,
        domain::LimitKind::Service,
        domain::LimitKind::Grace,
        domain::LimitKind::Goals,
        domain::LimitKind::Accounts,
        domain::LimitKind::HolderAccounts,
        domain::LimitKind::Sources,
        domain::LimitKind::SignIns,
        domain::LimitKind::SettingsProposals,
    ])
}
pub fn refusal_values() -> Box<[domain::Refusal]> {
    Box::new([
        domain::Refusal::NotWellFormed { at: 0, why: domain::Malformed::Cycle },
        domain::Refusal::NotWellFormed { at: 1, why: domain::Malformed::AsAllowedTwice },
        domain::Refusal::NotWellFormed { at: 0, why: domain::Malformed::Conversation },
        domain::Refusal::Authority { lacking: domain::Lack::Approver, proposable: Some(domain::Accepter::Nobody) },
        domain::Refusal::Role { held: Some(domain::Role::Policy { role: 7 }) },
        domain::Refusal::Authority {
            lacking: domain::Lack::Budget { needs: 17, left: 11 },
            proposable: Some(domain::Accepter::Role(0)),
        },
        domain::Refusal::Limit { which: domain::LimitKind::LiveTasks },
        domain::Refusal::Moved { now: domain::Moved::Hold { revision: 3, reason: domain::HoldReason::Budget } },
        domain::Refusal::Ended {
            how: domain::Ending::Cancelled { by: domain::Party::Person(2), at: Wall::from_nanos(19) },
        },
        domain::Refusal::Taken { by: 2 },
        domain::Refusal::NotWaiting,
        domain::Refusal::NoFurther,
        domain::Refusal::Unknown,
        domain::Refusal::NotOffered,
        domain::Refusal::KeyConflict,
        domain::Refusal::NoEscalationHolder,
        domain::Refusal::SignInRedirect { why: domain::SignInRedirectWhy::NotWaiting },
        domain::Refusal::SignInRedirect { why: domain::SignInRedirectWhy::Address },
        domain::Refusal::SignInRedirect { why: domain::SignInRedirectWhy::State },
        domain::Refusal::SignInRedirect { why: domain::SignInRedirectWhy::TooMany },
    ])
}
pub fn proposal_choice_values() -> Box<[domain::ProposalChoice]> {
    Box::new([
        domain::ProposalChoice::Accepted,
        domain::ProposalChoice::Rejected,
        domain::ProposalChoice::Passed,
        domain::ProposalChoice::Withdrawn,
        domain::ProposalChoice::Stale,
    ])
}
pub fn escalation_choice_values() -> Box<[domain::EscalationChoice]> {
    Box::new([domain::EscalationChoice::Released, domain::EscalationChoice::Rejected, domain::EscalationChoice::Passed])
}
pub fn outcome_values() -> Box<[domain::Outcome]> {
    Box::new([
        domain::Outcome::Compound { project: 1, operations: 6, first_message: 17 },
        domain::Outcome::CompoundRefused { at: 2, why: domain::Refusal::Unknown },
        domain::Outcome::Created { first: 2, last: 3, given: None },
        domain::Outcome::Created { first: 2, last: 3, given: Some(domain::Given { task: 3, amount: 70 }) },
        domain::Outcome::MarkedSeen { seen: domain::Seen::Conversation { task: 3, through: 7 } },
        domain::Outcome::Applied { connector: 2, token: Token::new(17) },
        domain::Outcome::Again(domain::Again::Busy),
        domain::Outcome::SignedOut,
        domain::Outcome::NoteEdited { name: 2 },
        domain::Outcome::ServiceMade { person: 2 },
        domain::Outcome::AccountSignInStarted { source: 2, address: b"https://issuer.test/visit".as_slice().into() },
        domain::Outcome::AccountFinishing { source: 2 },
        domain::Outcome::AccountSignedOut { confirmed: true, failed_hosts: Box::new([]) },
        domain::Outcome::Adopted { project: 2 },
        domain::Outcome::Proposed { proposal: 2, to: 0 },
        domain::Outcome::Withdrawn { proposal: 2 },
        domain::Outcome::Stopped { task: 2 },
        domain::Outcome::Cancelled { task: 2 },
        domain::Outcome::Released { task: 2 },
        domain::Outcome::PersonTaken { task: 2 },
        domain::Outcome::PersonHandedBack { task: 2 },
        domain::Outcome::PersonAnswered { task: 2 },
        domain::Outcome::Moved { task: 2 },
        domain::Outcome::ProposalDecided {
            proposer: domain::Proposer::Party(2),
            proposal: 3,
            by: 4,
            at: Wall::from_nanos(19),
            choice: domain::ProposalChoice::Rejected,
            repeated: false,
        },
        domain::Outcome::Said { task: 2, message: 3 },
        domain::Outcome::QuestionAnswered { task: 2, question: 3, message: 4 },
        domain::Outcome::Amended { task: 2, released: true },
        domain::Outcome::RolesSet { project: 2 },
        domain::Outcome::PolicyChanged { project: 2 },
        domain::Outcome::PoolSet { project: 2, person: 3 },
        domain::Outcome::EscalationDecided {
            task: 2,
            revision: 3,
            by: 4,
            at: Wall::from_nanos(19),
            choice: domain::EscalationChoice::Rejected,
        },
        domain::Outcome::Refused(domain::Refusal::Authority {
            lacking: domain::Lack::Budget { needs: 17, left: 11 },
            proposable: Some(domain::Accepter::Role(0)),
        }),
    ])
}
pub fn answered(outcome: domain::Outcome) -> domain::Stored {
    domain::Stored::Answer {
        models: Box::new([]),
        key: domain::RequestKey { person: 2, key: [7; 16] },
        ask: Box::new(domain::Ask::DecideProposal {
            project: 1,
            proposer: domain::Proposer::Party(2),
            proposal: 3,
            decision: domain::ProposalDecision::Reject { reason: Box::from(b"reject".as_slice()) },
            running: None,
        }),
        outcome,
        at: Wall::from_nanos(19),
    }
}
