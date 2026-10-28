//! Admission categories and exact numeric facts before a keyed flight is made.
use super::{Ask, LimitKind, Limits, Refusal};

pub(crate) fn invalid_ask(limits: &Limits, ask: &Ask) -> Refusal {
    let which = match kind(limits, ask) {
        Ok(which) => which,
        Err(why) => return why,
    };
    Refusal::Limit { which, bound: bound(limits, ask, which) }
}
fn kind(limits: &Limits, ask: &Ask) -> Result<LimitKind, Refusal> {
    Ok(match ask {
        Ask::Compound { operations, .. } => {
            return Err(crate::compound::refusal(limits, operations).expect("invalid compound"));
        }
        Ask::EditNote { name, .. } => {
            if *name == 0 {
                return Err(Refusal::Unknown);
            }
            LimitKind::Notes
        }
        Ask::MakeService { name, .. } => {
            if name.is_empty() {
                LimitKind::Empty
            } else if name.len() > usize::try_from(limits.words).expect("word ceiling fits usize") {
                LimitKind::Words
            } else {
                LimitKind::Identity
            }
        }
        Ask::Move { task, .. } | Ask::Cancel { task, .. } | Ask::AnswerPerson { task, .. } => {
            if *task == 0 {
                return Err(Refusal::Unknown);
            }
            LimitKind::Words
        }
        Ask::MarkSeen { .. }
        | Ask::Application { .. }
        | Ask::TakePerson { .. }
        | Ask::HandBackPerson { .. }
        | Ask::Stop { .. }
        | Ask::Lift { .. }
        | Ask::Release { .. }
        | Ask::SetPool { .. }
        | Ask::Withdraw { .. }
        | Ask::AccountSignIn { .. }
        | Ask::AccountSignOut { .. } => return Err(Refusal::Unknown),
        Ask::Adopt { .. } => LimitKind::Resource,
        Ask::SetRoles { .. } => LimitKind::Roles,
        Ask::ChangePolicy { .. } => LimitKind::Policy,
        Ask::Create { batch, .. } => crate::batch::invalid(limits, batch),
        Ask::Propose { action, reason, .. } => proposed(limits, action, reason),
        Ask::Rename { .. } | Ask::MakeProject { .. } => LimitKind::Name,
        Ask::AccountChange { change, .. } => {
            if let crate::AccountChange::SetUses { uses: Some(uses), .. } = change
                && uses.len() > usize::try_from(crate::USES).expect("uses ceiling fits usize")
            {
                LimitKind::Resource
            } else {
                LimitKind::Words
            }
        }
        Ask::ChangeSettings { .. } | Ask::MoveToNewest { .. } => LimitKind::Words,
        Ask::Say { task, words, .. } => {
            if *task == 0 {
                return Err(Refusal::Unknown);
            }
            if words.is_empty() { LimitKind::Empty } else { LimitKind::Words }
        }
        Ask::AnswerQuestion { task, question, words, .. } => {
            if *task == 0 || *question == 0 {
                return Err(Refusal::Unknown);
            }
            if words.is_empty() { LimitKind::Empty } else { LimitKind::Words }
        }
        Ask::Amend { task, amendment, .. } => {
            if *task == 0 {
                return Err(Refusal::Unknown);
            }
            if amendment.reason.len() > usize::try_from(limits.words).expect("word bound fits usize") {
                LimitKind::Words
            } else {
                LimitKind::Amendment
            }
        }
        Ask::DecideEscalation { task, revision, .. } => {
            if *task == 0 {
                return Err(Refusal::Unknown);
            }
            if *revision == 0 { LimitKind::Revision } else { LimitKind::Words }
        }
        Ask::DecideProposal { proposer, proposal, decision, running, .. } => {
            if running.is_some() && *decision != crate::ProposalDecision::Accept {
                return Err(Refusal::NotWellFormed { at: 0, why: crate::Malformed::Running });
            }
            if proposer.number() == 0 || *proposal == 0 {
                return Err(Refusal::Unknown);
            }
            LimitKind::Words
        }
    })
}
fn proposed(limits: &Limits, action: &crate::Action, reason: &[u8]) -> LimitKind {
    if reason.len() > usize::try_from(limits.words).expect("word ceiling fits usize") {
        return LimitKind::Words;
    }
    match action {
        crate::Action::Batch(batch) => crate::batch::invalid(limits, batch),
        crate::Action::Amend { amendment, .. } => {
            if amendment.reason.len() > usize::try_from(limits.words).expect("word ceiling fits usize") {
                LimitKind::Words
            } else {
                LimitKind::Amendment
            }
        }
        crate::Action::Settings { .. } => LimitKind::Words,
    }
}
fn bound(limits: &Limits, ask: &Ask, which: LimitKind) -> Option<crate::LimitBound> {
    if which != LimitKind::Words && semantic_shape(ask) {
        return None;
    }
    if let Ask::AccountChange { change: crate::AccountChange::DeclareProgram { .. }, .. } = ask {
        return None;
    }
    let words_past = if let Ask::Create { batch, .. } = ask {
        crate::batch::words_past(limits, batch)
    } else if let Ask::Propose { action: crate::Action::Batch(batch), reason, .. } = ask {
        reason.len() > usize::try_from(limits.words).expect("word ceiling fits usize")
            || crate::batch::words_past(limits, batch)
    } else {
        true
    };
    if (which == LimitKind::Words && !words_past) || !crate::words::valid(ask) {
        None
    } else if let Ask::AccountChange { change: crate::AccountChange::SetUses { uses: Some(uses), .. }, .. } = ask
        && uses.len() > usize::try_from(crate::USES).expect("uses ceiling fits usize")
    {
        Some(crate::LimitBound::fixed(u64::from(crate::USES), crate::LimitUnit::Items))
    } else {
        use crate::{LimitBound as B, LimitKey as K, LimitUnit as U};
        match which {
            LimitKind::Words => Some(B::configured(u64::from(limits.words), K::PeopleWords, U::Bytes)),
            LimitKind::Amendment | LimitKind::Policy | LimitKind::Resource | LimitKind::Notes => {
                Some(B::configured(u64::from(limits.amendment_bytes), K::PeopleAmendmentBytes, U::Bytes))
            }
            LimitKind::Name => Some(B::fixed(u64::try_from(crate::NAME_BYTES).expect("name bound fits u64"), U::Bytes)),
            LimitKind::Identity => {
                Some(B::configured(u64::from(limits.identity_bytes), K::PeopleIdentityBytes, U::Bytes))
            }
            LimitKind::Roles => Some(B::configured(u64::from(limits.holdings), K::PeopleHoldings, U::Items)),
            LimitKind::Batch => Some(B::configured(u64::from(limits.batch), K::PeopleBatch, U::Items)),
            LimitKind::Empty | LimitKind::Revision => None,
            LimitKind::SettingsProposals
            | LimitKind::Projects
            | LimitKind::Operations
            | LimitKind::Key
            | LimitKind::Arithmetic
            | LimitKind::Duplicate
            | LimitKind::ProjectLiveTasks
            | LimitKind::LiveTasks
            | LimitKind::Tree
            | LimitKind::Depth
            | LimitKind::Delegates
            | LimitKind::Subscription
            | LimitKind::Dependencies
            | LimitKind::Cycle
            | LimitKind::Spec
            | LimitKind::Contract
            | LimitKind::Authority
            | LimitKind::Inputs
            | LimitKind::Attempts
            | LimitKind::LiveDelegates
            | LimitKind::Restore
            | LimitKind::Read
            | LimitKind::Turn
            | LimitKind::HoldKind
            | LimitKind::HoldTaken
            | LimitKind::Holds
            | LimitKind::InboxFull
            | LimitKind::Question
            | LimitKind::Priority
            | LimitKind::Grace
            | LimitKind::Goals
            | LimitKind::Service
            | LimitKind::Accounts
            | LimitKind::HolderAccounts
            | LimitKind::Sources
            | LimitKind::SignIns
            | LimitKind::PlacedHolds
            | LimitKind::ProjectPlacedHolds
            | LimitKind::PartyPlacedHolds
            | LimitKind::Exemptions => unreachable!("invalid ask names only its own admission checks"),
        }
    }
}

fn semantic_shape(ask: &Ask) -> bool {
    if let Ask::Amend { amendment, .. } = ask {
        return !crate::llm::valid(amendment.llm.as_ref());
    }
    if let Ask::Propose { action: crate::Action::Amend { amendment, .. }, .. } = ask {
        return !crate::llm::valid(amendment.llm.as_ref());
    }
    if let Ask::EditNote { name, scope, change, .. } = ask {
        return super::note_bytes(*name, scope, change).is_none();
    }
    if let Ask::Adopt { adoption, .. } = ask {
        return adoption.resource.path.is_empty();
    }
    false
}
