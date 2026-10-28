use skein_lib::{Id, Map, Queue, Set, Slab, Token};

use crate::LinkUp;
use crate::boundary::{Delivery, Reason};
use crate::call::Call;
use crate::facts::Fact;
use crate::hosted::{Hosted, NamedEvent};
use crate::turns::{Name, Pending};

/// The host child domain's limits (section 7), handed by its parent to every
/// step read-only.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Limits {
    /// The most bytes of one piece of live content (hosts.md, 6.5).
    pub piece_bytes: u32,
    /// Local program sources; their command lifecycle does not consume run slots.
    pub program_sources: u32,
    /// Positive delay from a completed check to the next periodic check.
    pub source_interval: skein_lib::Duration,
    /// Positive deadline span for one source check or sign-out command.
    pub source_timeout: skein_lib::Duration,
    /// Runs hosted at once: the run slots. An assignment beyond them is
    /// refused as busy.
    pub slots: u32,
    /// Distinct grants retained for a hosted attempt.
    pub accounts: u32,
    /// The most bytes of a charter.
    pub charter_bytes: u64,
    /// Opaque transcript, including the committed call tail.
    pub transcript_bytes: u64,
    /// Maximum opaque evidence bytes in one settled delivery call.
    pub delivery_evidence_bytes: u64,
    /// One completed turn body.
    pub turn_bytes: u64,
    /// The most bytes of a run's declared outcome. A run that says more has
    /// broken the rules.
    pub outcome_bytes: u64,
    /// The most bytes of detail an answer carries for operators: the tail of
    /// what it is given.
    pub detail_bytes: u32,
    /// Inbound events a run holds until it is live. Beyond them, an event is
    /// bounced.
    pub held: u32,
    /// Messages a run's inbox takes: the most a start carries, and the most
    /// refusals a turn or an answer carries.
    pub inbox: u32,
    /// The most bytes of an inbound event.
    pub event_bytes: u64,
    /// Host calls a run may have in flight at once, its delivery among them. A
    /// call beyond them is answered as busy.
    pub run_calls: u32,
    /// Facts kept until the parent drains them. Beyond them, facts are
    /// dropped and counted.
    pub facts: u32,
    /// Activity and live pieces kept for the core while the link cannot take it.
    pub told: u32,
    /// Normal retained turns per attempt. One additional agent window of
    /// this size is reserved for turns sent before stopping.
    pub turns: u32,
    /// Normal retained body bytes per attempt, with the same stopping reserve.
    pub turn_queue_bytes: u64,
}

/// The host calls in flight at once, across runs, under `limits`, or `None` if
/// they do not fit a `u32`.
pub(crate) fn calls(limits: &Limits) -> Option<u32> {
    limits.slots.checked_mul(limits.run_calls)
}

/// The most memory the domain holds under `limits`, in bytes (6.3), or `None`
/// if it does not fit a `u64`.
///
/// It counts the containers, their bookkeeping included, and the payloads, not
/// allocator overhead. What the host passes on (a workspace, the bodies of
/// host calls and their answers, what a save did) is moved into a
/// request in the step it arrives in, and what it makes for a request (an
/// hello) is moved out in the step that makes it: either is its
/// receiver's to count. One delivery terminal is counted at the step peak,
/// alongside the runs' retained state, including their kept answers.
#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    if limits.turns > 0 && (limits.turn_bytes == 0 || limits.turn_queue_bytes < limits.turn_bytes) {
        return None;
    }
    cancellations_bound(limits)?;
    // The reserved output bound must be representable without saturation.
    // Cancelling each relay emits its reply and its cancellation request.
    let cancellations = limits.run_calls.checked_mul(2)?;
    if cancellations.checked_add(2).is_none() || limits.held.checked_add(limits.accounts)?.checked_add(2).is_none() {
        return None;
    }
    let sources = crate::sources::worst_case(limits)?;
    // Hosted includes its inline park intent, with no additional allocation.
    let hosted = Slab::<Hosted>::worst_case(limits.slots)?;
    let names = Map::<Token, Id<Hosted>>::worst_case(limits.slots)?;
    let answers = Map::<(Token, Token), Id<Hosted>>::worst_case(limits.slots)?;
    let ready = Map::<Id<Hosted>, Reason>::worst_case(limits.slots)?;
    let call_count = calls(limits)?;
    let calls =
        Slab::<Call>::worst_case(call_count)?.checked_add(u64::from(call_count).checked_mul(limits.event_bytes)?)?;
    let facts = Queue::<Fact>::worst_case(limits.facts)?;
    // Price every queue slot as a maximum piece, including Input's wire-bounded
    // call id (256 bytes) and tool (64 bytes), even though pieces use only half.
    let piece = u64::from(limits.piece_bytes).checked_add(256)?.checked_add(64)?;
    let told = Queue::<LinkUp>::worst_case(limits.told)?.checked_add(u64::from(limits.told).checked_mul(piece)?)?;
    let capacity = limits.slots.checked_mul(limits.turns)?.checked_mul(2)?;
    let refusal_bytes = u64::from(limits.inbox).checked_mul(u64::try_from(size_of::<crate::Refused>()).ok()?)?;
    let turns = Map::<Name, Pending>::worst_case(capacity)?
        .checked_add(u64::from(limits.slots).checked_mul(limits.turn_queue_bytes.checked_mul(2)?)?)?
        .checked_add(u64::from(capacity).checked_mul(refusal_bytes)?)?;
    // Until its agent starts, a run holds its charter, its conversation and the
    // inbound events that came meanwhile; from when it is told how the run
    // finishes, the outcome or the detail of the failure.
    let held = Queue::<NamedEvent>::worst_case(limits.held)?
        .checked_add(u64::from(limits.held).checked_mul(limits.event_bytes)?)?;
    let messages = u64::from(limits.inbox)
        .checked_mul(u64::try_from(size_of::<crate::StartMessage>()).ok()?.checked_add(limits.event_bytes)?)?;
    let starting =
        limits.charter_bytes.checked_add(limits.transcript_bytes)?.checked_add(held)?.checked_add(messages)?;
    let ending = limits.outcome_bytes.max(u64::from(limits.detail_bytes));
    // Throughout, the opaque workspace handle, grants, and relayed calls in
    // flight (a delivery is held inline).
    let run_calls = Set::<Id<Call>>::worst_case(limits.run_calls)?;
    let run = starting
        .max(ending)
        .checked_add(run_calls)?
        .checked_add(skein_lib::List::<crate::Refused>::worst_case(limits.inbox)?)?
        .checked_add(refusal_bytes)?
        .checked_add(u64::from(limits.accounts).checked_mul(u64::try_from(size_of::<crate::Grant>()).ok()?)?)?;
    let runs = u64::from(limits.slots).checked_mul(run)?;
    // One terminal arrives per step; its fixed payload is included.
    // A boxed start exists at the step peak before the parent takes it.
    let delivered = u64::try_from(size_of::<Delivery>()).ok()?;
    sources
        .checked_add(hosted)?
        .checked_add(names)?
        .checked_add(ready)?
        .checked_add(answers)?
        .checked_add(calls)?
        .checked_add(facts)?
        .checked_add(told)?
        .checked_add(turns)?
        .checked_add(runs)?
        .checked_add(ending)?
        .checked_add(u64::try_from(size_of::<crate::Start>()).ok()?)?
        .checked_add(delivered)
}

fn cancellations_bound(limits: &Limits) -> Option<u32> {
    limits
        .run_calls
        .checked_mul(2)?
        .max(limits.held.checked_add(limits.accounts)?)
        .checked_add(limits.program_sources)?
        .checked_add(4)?
        .checked_add(limits.turns.checked_mul(2)?)
}
