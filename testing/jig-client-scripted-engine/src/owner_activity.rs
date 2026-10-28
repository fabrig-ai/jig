//! Whole activity admission (client/domain/contract.md, 5.3; engine.md, 7.2).
//! Every reserved field row and opaque byte is priced; no clipped value is returned.
use crate::{Bounds, payload::Payload};
use core::mem::size_of;
use jig_client_domain::contract::{
    OwnerActivity, OwnerActivityLine, OwnerMessageKind, OwnerWord, OwnerWordResult, ResultField, Text,
};
use skein_lib::List;

fn message_words_heap(kind: &OwnerMessageKind, words: &Text, bounds: &Bounds) -> Option<u64> {
    if let OwnerMessageKind::Result(OwnerWordResult::Approved { words_present: false, .. }) = kind
        && !words.bytes().is_empty()
    {
        return None;
    }
    kind.heap(bounds)?.checked_add(words.heap(bounds)?)
}
impl Payload for OwnerWordResult {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        match self {
            Self::Approved { state, .. } => state.heap(bounds),
            Self::Verdict { fields, .. } => fields.heap(bounds),
            Self::Report | Self::Change { .. } | Self::Failed | Self::Cancelled => Some(0),
        }
    }
}
impl Payload for OwnerMessageKind {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        match self {
            Self::Result(result) => result.heap(bounds),
            Self::Escalation { .. }
            | Self::Proposal { .. }
            | Self::PersonStalled { .. }
            | Self::SettingsDecision { .. }
            | Self::ProposalDecision { .. }
            | Self::Words
            | Self::EffectNotMade { .. }
            | Self::Met { .. }
            | Self::QuestionAnswered { .. }
            | Self::QuestionOwed
            | Self::QuestionEnded { .. }
            | Self::Amendment { .. }
            | Self::Question
            | Self::Answer { .. }
            | Self::Notice { .. }
            | Self::Timer { .. }
            | Self::News { .. } => Some(0),
        }
    }
}
impl Payload for OwnerWord {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        message_words_heap(&self.kind, &self.words, bounds)
    }
}
impl Payload for OwnerActivity {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        match self {
            Self::Words { kind, words, .. } => message_words_heap(kind, words, bounds),
            Self::Heard(word) | Self::Notified { word, .. } => word.heap(bounds),
            Self::Kept { pending, .. } => pending.heap(bounds),
            Self::RunBegan { .. }
            | Self::RunEnded { .. }
            | Self::Turn { .. }
            | Self::Decided { .. }
            | Self::Milestone { .. } => Some(0),
        }
    }
}
impl Payload for OwnerActivityLine {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        self.line.heap(bounds)
    }
}
/// Exact whole inline activity plus all nested allocations, without trimming.
#[must_use]
pub fn bytes(value: &OwnerActivityLine, bounds: &Bounds) -> Option<u64> {
    u64::try_from(size_of::<OwnerActivityLine>()).ok()?.checked_add(value.heap(bounds)?)
}
#[must_use]
pub fn fits(value: &OwnerActivityLine, bounds: &Bounds) -> bool {
    crate::payload::fits(value, bounds)
}
#[must_use]
pub fn word_bytes(value: &OwnerWord, bounds: &Bounds) -> Option<u64> {
    u64::try_from(size_of::<OwnerWord>()).ok()?.checked_add(value.heap(bounds)?)
}
#[must_use]
pub fn word_fits(value: &OwnerWord, bounds: &Bounds) -> bool {
    crate::payload::fits(value, bounds)
}
fn word_heap(bounds: &Bounds) -> Option<u64> {
    let text = u64::from(bounds.text);
    let fields = List::<ResultField>::worst_case(bounds.rows)?
        .checked_add(u64::from(bounds.rows).checked_mul(text)?.checked_mul(2)?)?;
    text.checked_add(text.max(fields))
}
/// Configured whole activity envelope; exactly one message may occur in a line.
#[must_use]
pub fn worst_case(bounds: &Bounds) -> Option<u64> {
    u64::try_from(size_of::<OwnerActivityLine>())
        .ok()?
        .checked_add(word_heap(bounds)?.max(List::<u64>::worst_case(bounds.rows)?))
}
#[must_use]
pub fn word_worst_case(bounds: &Bounds) -> Option<u64> {
    u64::try_from(size_of::<OwnerWord>()).ok()?.checked_add(word_heap(bounds)?)
}
#[cfg(test)]
mod tests;
