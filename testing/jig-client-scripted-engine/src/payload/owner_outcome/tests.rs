use super::{admit_owner_outcome, owner_outcome_bytes};
use crate::{Bounds, Invalid};
use jig_client_domain::contract::{
    OwnerEscalationChoice, OwnerOutcome, OwnerProposalChoice, ProposalKey, Proposer, Revision, SeenThrough,
};
use skein_lib::Wall;
use std::mem::size_of;

fn maximum_outcomes() -> [OwnerOutcome; 26] {
    [
        OwnerOutcome::MarkedSeen { seen: SeenThrough::Results { through: u64::MAX } },
        OwnerOutcome::MarkedSeen { seen: SeenThrough::Conversation { task: u64::MAX, through: u64::MAX } },
        OwnerOutcome::NoteEdited { name: u64::MAX },
        OwnerOutcome::ServiceMade { person: u64::MAX },
        OwnerOutcome::Adopted { project: u32::MAX },
        OwnerOutcome::Stopped { task: u64::MAX },
        OwnerOutcome::Cancelled { task: u64::MAX },
        OwnerOutcome::Released { task: u64::MAX },
        OwnerOutcome::PersonTaken { task: u64::MAX },
        OwnerOutcome::PersonHandedBack { task: u64::MAX },
        OwnerOutcome::PersonAnswered { task: u64::MAX },
        OwnerOutcome::Moved { task: u64::MAX },
        OwnerOutcome::ProposalDecided {
            proposal: ProposalKey { proposer: Proposer::Party(u64::MAX), number: u64::MAX },
            by: u64::MAX,
            at: Wall::from_nanos(u64::MAX),
            choice: OwnerProposalChoice::Accepted,
        },
        OwnerOutcome::Said { task: u64::MAX, message: u64::MAX },
        OwnerOutcome::QuestionAnswered { task: u64::MAX, question: u64::MAX, message: u64::MAX },
        OwnerOutcome::Amended { task: u64::MAX },
        OwnerOutcome::Proposed { proposal: u64::MAX, to: u32::MAX },
        OwnerOutcome::Withdrawn { proposal: u64::MAX },
        OwnerOutcome::RolesSet { project: u32::MAX },
        OwnerOutcome::PolicyChanged { project: u32::MAX },
        OwnerOutcome::Renamed { project: u32::MAX },
        OwnerOutcome::ProjectMade { project: u32::MAX },
        OwnerOutcome::PoolSet { project: u32::MAX, person: u64::MAX },
        OwnerOutcome::AccountSignedOut { confirmed: false, failed_hosts: Box::new([]) },
        OwnerOutcome::AccountSignedOut { confirmed: true, failed_hosts: Box::new([]) },
        OwnerOutcome::EscalationDecided {
            task: u64::MAX,
            revision: Revision { value: u64::MAX },
            by: u64::MAX,
            at: Wall::from_nanos(u64::MAX),
            choice: OwnerEscalationChoice::Released,
        },
    ]
}

#[test]
fn every_committed_owner_outcome_is_admitted_only_at_its_whole_inline_bound() {
    let bytes = u64::try_from(size_of::<OwnerOutcome>()).expect("inline size fits");
    let exact = Bounds { text: 0, rows: 0, document_bytes: bytes, ..Bounds::default() };
    let short = Bounds { document_bytes: bytes.checked_sub(1).expect("outcome holds facts"), ..exact };
    for value in maximum_outcomes() {
        assert_eq!(owner_outcome_bytes(&value, &exact), Some(bytes));
        assert_eq!(admit_owner_outcome(&value, &exact), Ok(()));
        assert_eq!(admit_owner_outcome(&value, &short), Err(Invalid::Payload));
    }
}

#[test]
fn closed_decision_choices_and_release_flags_need_no_explanation_payload() {
    let bytes = u64::try_from(size_of::<OwnerOutcome>()).expect("inline size fits");
    let bounds = Bounds { text: 0, rows: 0, document_bytes: bytes, ..Bounds::default() };
    for proposer in [Proposer::Task(u64::MAX), Proposer::Party(u64::MAX)] {
        for choice in [
            OwnerProposalChoice::Accepted,
            OwnerProposalChoice::Rejected,
            OwnerProposalChoice::Passed,
            OwnerProposalChoice::Withdrawn,
            OwnerProposalChoice::Stale,
        ] {
            let value = OwnerOutcome::ProposalDecided {
                proposal: ProposalKey { proposer, number: u64::MAX },
                by: 23,
                at: Wall::from_nanos(31),
                choice,
            };
            assert_eq!(owner_outcome_bytes(&value, &bounds), Some(bytes));
            assert_eq!(admit_owner_outcome(&value, &bounds), Ok(()));
        }
    }
    for choice in [OwnerEscalationChoice::Released, OwnerEscalationChoice::Rejected, OwnerEscalationChoice::Passed] {
        let value = OwnerOutcome::EscalationDecided {
            task: 7,
            revision: Revision { value: 13 },
            by: 19,
            at: Wall::from_nanos(29),
            choice,
        };
        assert_eq!(owner_outcome_bytes(&value, &bounds), Some(bytes));
        assert_eq!(admit_owner_outcome(&value, &bounds), Ok(()));
    }
    {
        for value in [
            OwnerOutcome::Said { task: 7, message: 11 },
            OwnerOutcome::Amended { task: 13 },
            OwnerOutcome::Proposed { proposal: 23, to: 0 },
        ] {
            assert_eq!(owner_outcome_bytes(&value, &bounds), Some(bytes));
            assert_eq!(admit_owner_outcome(&value, &bounds), Ok(()));
        }
    }
}

#[test]
fn durable_owner_answers_are_admitted_only_with_the_complete_answer_envelope() {
    use crate::payload::{Payload, fits};
    use jig_client_domain::contract::{Answer, Outcome};
    let bytes = u64::try_from(size_of::<Answer>()).unwrap();
    let exact = Bounds { text: 0, rows: 0, document_bytes: bytes, ..Bounds::default() };
    let short = Bounds { document_bytes: bytes - 1, ..exact };
    for value in maximum_outcomes() {
        let answer = Answer::Done(Outcome::Owner(value));
        assert_eq!(answer.heap(&exact), Some(0));
        assert!(fits(&answer, &exact));
        assert!(!fits(&answer, &short));
    }
}

#[test]
fn an_account_sign_in_address_is_admitted_only_at_its_complete_bound() {
    let inline = u64::try_from(size_of::<OwnerOutcome>()).unwrap();
    for length in [0_u64, 1, 2048] {
        let value = OwnerOutcome::AccountSignInStarted {
            source: u32::MAX,
            address: vec![b'a'; usize::try_from(length).unwrap()].into(),
        };
        let exact = Bounds { text: 0, document_bytes: inline + length, ..Bounds::default() };
        assert_eq!(owner_outcome_bytes(&value, &exact), Some(inline + length));
        assert_eq!(admit_owner_outcome(&value, &exact), Ok(()));
        let short = Bounds { document_bytes: inline + length - 1, ..exact };
        assert_eq!(admit_owner_outcome(&value, &short), Err(Invalid::Payload));
    }
    let value = OwnerOutcome::AccountSignInStarted { source: 7, address: vec![b'a'; 2049].into() };
    assert_eq!(admit_owner_outcome(&value, &Bounds::default()), Err(Invalid::Payload));
}
