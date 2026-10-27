//! Whole pending proposal presentation (client/domain/objects.md, 2 and 4;
//! view.md, 7 and 10). Funding and proposed contexts are supplied facts.
use crate::JigBindings;
use crate::owner_proposal_words::ProposalPhrase as P;
use crate::words::Words;
use jig_client::words::DecisionWord as D;
use skein_ui::phrases::Hole;
use skein_ui::{Builder, Role, Run};

use crate::{Borrowed, Limits};
use jig_client::contract::{
    ChipPhase, DocumentText, OwnerChipHold, OwnerHoldEscalation, OwnerProposal, OwnerProposalAction, OwnerTaskLink,
};
/// Finite storage for the entire supplied proposal fragment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProposalFragment {
    pub nodes: u32,
    pub text_bytes: u32,
    pub text_per_node: u32,
    pub depth: u32,
    pub bindings: u32,
    pub slots: u32,
}
fn text_fits(value: &DocumentText, limits: &Limits) -> Option<()> {
    if value.bytes.fits(limits.text) { Some(()) } else { None }
}
fn link_fits(value: &OwnerTaskLink, limits: &Limits, words: &Words) -> Option<()> {
    let chip = match value {
        OwnerTaskLink::Live(chip) | OwnerTaskLink::Retained { chip, .. } => chip,
        OwnerTaskLink::Stored { .. } => return Some(()),
    };
    text_fits(&chip.title, limits)?;
    match chip.details.as_ref() {
        ChipPhase::Owner { details, .. } => {
            crate::owner_chip::required(details, limits, words)?;
        }
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
        | ChipPhase::Ended { .. } => {}
    }
    Some(())
}
fn hold_fits(value: &OwnerChipHold, limits: &Limits) -> Option<()> {
    if let Some(words) = &value.words {
        text_fits(words, limits)?;
    }
    if let Some(part) = &value.source {
        crate::owner_chip::part_fits(part, limits)?;
    }
    match &value.escalation {
        Some(OwnerHoldEscalation::Rejected { reason, .. }) => text_fits(reason, limits)?,
        Some(OwnerHoldEscalation::Routing | OwnerHoldEscalation::Waiting { .. }) | None => {}
    }
    Some(())
}
/// Validate every rendered nested field and reserved row capacity before reservation.
/// Whole protocol allocation admission remains the receiving kit's independent boundary.
#[must_use]
pub fn required(value: &OwnerProposal, limits: &Limits, words: &Words) -> Option<ProposalFragment> {
    text_fits(&value.reason, limits)?;
    if value.funding.capacity() > limits.rows || value.acts.capacity() > jig_client::contract::MAX_OFFERED_ACTS {
        return None;
    }
    match &value.action {
        OwnerProposalAction::Settings(_) => return None,
        OwnerProposalAction::Batch(tasks) => {
            if tasks.capacity() > limits.rows {
                return None;
            }
            for task in tasks {
                crate::owner_history::proposed_fits(task, limits)?;
            }
        }
        OwnerProposalAction::Effect { part, authority, .. }
        | OwnerProposalAction::ProcedureEffect { part, authority, .. } => {
            crate::owner_chip::part_fits(part, limits)?;
            crate::owner_amendment::authority_fits(authority, limits)?;
        }
        OwnerProposalAction::Amend { task, before, after, reason, .. } => {
            link_fits(task, limits, words)?;
            crate::owner_amendment::required(before, limits, words)?;
            crate::owner_amendment::required(after, limits, words)?;
            text_fits(reason, limits)?;
        }
        OwnerProposalAction::Widen { task, before, after } => {
            link_fits(task, limits, words)?;
            crate::owner_amendment::authority_fits(before, limits)?;
            crate::owner_amendment::authority_fits(after, limits)?;
        }
        OwnerProposalAction::Release { task, hold } => {
            link_fits(task, limits, words)?;
            if let Some(hold) = hold {
                hold_fits(hold, limits)?;
            }
        }
    }
    worst_case(limits, words)
}
/// Closed action alternatives, with brief linked chips rather than expanded task cards.
#[must_use]
pub fn max_nodes(limits: &Limits) -> Option<u32> {
    let markdown = skein_ui::markdown::max_nodes(limits.text)?;
    let authority = crate::owner_authority::document_nodes(limits.rows, limits.segments)?;
    let link = crate::chip_nodes(limits)?.max(crate::owner_chip::max_nodes(limits)?).checked_add(3)?;
    let part = crate::proposed_task::part_nodes(limits)?;
    let batch = crate::sizes::proposed_task_nodes(limits)?.checked_mul(limits.rows)?.checked_add(1)?;
    let effect = part.checked_add(authority)?.checked_add(5)?;
    let amend = crate::owner_amendment::max_nodes(limits)?
        .checked_mul(2)?
        .checked_add(link)?
        .checked_add(markdown)?
        .checked_add(12)?;
    let widen = authority.checked_mul(2)?.checked_add(link)?.checked_add(3)?;
    let release = part.checked_add(markdown)?.checked_add(link)?.checked_add(40)?;
    batch
        .max(effect)
        .max(amend)
        .max(widen)
        .max(release)
        .checked_add(markdown)?
        .checked_add(limits.rows.checked_mul(10)?)?
        .checked_add(20)
}
/// Whole checked saying/literal/Markdown envelope, including name and text storage.
#[must_use]
pub fn worst_case(limits: &Limits, words: &Words) -> Option<ProposalFragment> {
    let nodes = max_nodes(limits)?;
    let phrase = words.jig_capacity()?;
    let text_per_node = phrase.max(crate::literal_segment::max_bytes(limits.text)?).checked_mul(2)?;
    Some(ProposalFragment {
        nodes,
        text_bytes: nodes.checked_mul(text_per_node)?,
        text_per_node,
        depth: limits.rows.checked_add(32)?,
        bindings: nodes,
        slots: nodes,
    })
}
fn saying(b: &mut Builder, words: &Words, phrase: P, holes: &[Hole]) {
    let value = words
        .book()
        .say(words.language(), phrase.id(), holes)
        .expect("checked pending proposal sayings and typed holes");
    b.text(Run::Plain, &value.text);
}
/// Render the five exact supplied action kinds without historical joins or routing guesses.
pub fn action(
    b: &mut Builder,
    client: &Borrowed<'_>,
    value: &OwnerProposalAction,
    words: &Words,
    bindings: &mut JigBindings,
) {
    let text_limit = bindings.text_limit();
    match value {
        OwnerProposalAction::Settings(_) => {}
        OwnerProposalAction::Batch(tasks) => {
            saying(b, words, P::Batch, &[]);
            for task in tasks {
                crate::cards::proposal::proposed_task(b, task, words, bindings, client.limits.rows);
            }
        }
        OwnerProposalAction::Effect { part, authority, attempt, completion, position, purpose } => {
            saying(b, words, P::Effect, &[]);
            crate::proposed_task::part(b, part, words, bindings);
            crate::owner_authority::document_words(b, authority, words, text_limit, client.limits.rows);
            saying(b, words, P::Attempt, &[Hole::Number(*attempt)]);
            saying(b, words, P::Completion, &[Hole::Number(u64::from(*completion))]);
            saying(b, words, P::Position, &[Hole::Number(u64::from(*position))]);
            saying(b, words, P::Purpose, &[Hole::Number(*purpose)]);
        }
        OwnerProposalAction::ProcedureEffect { part, authority, step, purpose } => {
            saying(b, words, P::Effect, &[]);
            crate::proposed_task::part(b, part, words, bindings);
            crate::owner_authority::document_words(b, authority, words, text_limit, client.limits.rows);
            saying(b, words, P::ProcedureStep, &[Hole::Number(*step)]);
            saying(b, words, P::Purpose, &[Hole::Number(*purpose)]);
        }
        OwnerProposalAction::Amend { task, addressed, before, after, reason, release } => {
            saying(b, words, P::Amend, &[]);
            crate::owner_result::link(b, client, task, words, bindings);
            match addressed {
                Some(change) => crate::cards::proposal::address_change(b, change, words),
                None => saying(b, words, P::AddressAbsent, &[]),
            }
            saying(b, words, P::Before, &[]);
            crate::owner_amendment::render(b, before, words, text_limit, client.limits.rows);
            saying(b, words, P::After, &[]);
            crate::owner_amendment::render(b, after, words, text_limit, client.limits.rows);
            saying(b, words, P::AmendReason, &[]);
            crate::document_text::markdown(b, reason, words);
            match release {
                Some(revision) => saying(b, words, P::ReleaseRevision, &[Hole::Number(*revision)]),
                None => saying(b, words, P::ReleaseAbsent, &[]),
            }
        }
        OwnerProposalAction::Widen { task, before, after } => {
            saying(b, words, P::Widen, &[]);
            crate::owner_result::link(b, client, task, words, bindings);
            saying(b, words, P::Before, &[]);
            crate::owner_authority::document_words(b, before, words, text_limit, client.limits.rows);
            saying(b, words, P::After, &[]);
            crate::owner_authority::document_words(b, after, words, text_limit, client.limits.rows);
        }
        OwnerProposalAction::Release { task, hold } => {
            saying(b, words, P::Release, &[]);
            crate::owner_result::link(b, client, task, words, bindings);
            match hold {
                Some(hold) => crate::owner_chip::hold(b, hold, words, bindings),
                None => saying(b, words, P::HoldAbsent, &[]),
            }
        }
    }
}
/// Whole pending envelope: original maker/key, independent waiting and funding facts.
pub fn render(
    b: &mut Builder,
    client: &Borrowed<'_>,
    value: &OwnerProposal,
    words: &Words,
    bindings: &mut JigBindings,
) {
    b.open(Role::Region);
    let name = words.book().say(words.language(), P::Proposal.id(), &[]).expect("checked proposal region name");
    b.name(&name.text);
    saying(b, words, P::Number, &[Hole::Number(value.key.number)]);
    saying(b, words, P::Project, &[Hole::Number(u64::from(value.project))]);
    words.proposer(b, value.key.proposer);
    saying(b, words, if value.as_holder { P::AsHolder } else { P::NotHolder }, &[]);
    saying(b, words, P::Holder, &[]);
    crate::owner_history::holder(b, value.waiting.holder, words);
    saying(b, words, P::WaitingSince, &[Hole::Wall(words.adjusted_wall(value.waiting.since))]);
    saying(b, words, P::Reason, &[]);
    crate::document_text::markdown(b, &value.reason, words);
    action(b, client, &value.action, words, bindings);
    saying(b, words, P::Funding, &[]);
    for offer in &value.funding {
        crate::proposed_task::funder(b, offer.source, words);
        words.decision_fact(b, D::Needed, &[Hole::Number(offer.needed)]);
        match offer.left {
            Some(left) => words.decision_fact(b, D::Left, &[Hole::Number(left)]),
            None => words.decision_fact(b, D::LeftAbsent, &[]),
        }
        match offer.after {
            Some(after) => words.decision_fact(b, D::AfterFunding, &[Hole::Number(after)]),
            None => words.decision_fact(b, D::AfterAbsent, &[]),
        }
    }
    b.close();
}
