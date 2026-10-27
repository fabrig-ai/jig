//! Hosted run lifecycle (hosts.md, section 6). An accepted run prepares a
//! workspace, starts its agent, then serves calls and turns until it stops.
//! The host waits for agent exit and each in-flight delivery before saving,
//! releasing, and answering. The same tail follows cancellation or a fault.

mod start_refused;
pub(crate) use start_refused::start_refused;

use crate::LinkUp;
use alloc::boxed::Box;
use core::mem;

use skein_lib::bytes::copy_of;
use skein_lib::{Env, Id, List, Map, Queue, Set, Slab, Token};

use crate::assignment::{self, len};
use crate::boundary::{
    AgentFailure, AnsweredCall, Ask, Assignment, Delivery, Ending, Failure, Finish, Grant, GrantTable, Hosting,
    MessageRefusal, Phase, Preparation, Reason, Refusal, Refused, Reply, Request, RunAssignment, RunFailure,
    SettledAnswer, Work,
};
use crate::call::{self, Call};
use crate::domain::Domain;
use crate::facts::{Fact, Facts};
use crate::limits::Limits;

#[derive(Debug)]
pub(crate) struct Hosted {
    /// The engine's names for the run and for the attempt hosted.
    pub(crate) run: Token,
    pub(crate) attempt: Token,
    grants: Box<[Grant]>,
    refreshed: bool,
    refusals: crate::refusals::Refusals,
    pub(crate) answer: Option<crate::answers::Kept>,
    /// Whether the workspace saves unfinished work before release.
    save: bool,
    pub(crate) live: crate::LiveAsk,
    /// The application's token for the last delivery that changed the workspace.
    left: Option<Token>,
    /// Its relayed calls in flight, while it is live.
    relays: Set<Id<Call>>,
    /// Its delivery in flight, if it has one: a write, so one at a time, and
    /// waited for through the stop.
    delivery: Option<Id<Call>>,
    runtime: Runtime,
    state: State,
    /// Engine drain intent; retained turns must commit before settlement.
    parking: bool,
    observed: Option<Phase>,
}

#[derive(Debug)]
struct Runtime {
    start: Option<Conversation>,
    turns: u32,
    spent: u64,
    read: Option<Token>,
}

impl Hosted {
    pub(crate) const fn taken_turns(&self) -> u32 {
        self.runtime.turns
    }
}

type CallName = Box<[u8]>;

fn reply(agent: Token, call: CallName, reply: Reply, out: &mut Queue<Request>) {
    out.push(Request::Reply { agent, call, reply });
}

fn take_name(call: &mut Call) -> CallName {
    call.name.take().expect("one reply for each named call")
}

#[derive(Debug)]
struct Conversation {
    identity: [u8; 16],
    messages: Box<[crate::StartMessage]>,
    turns: Box<[Box<[u8]>]>,
    answered: Box<[AnsweredCall]>,
}

#[derive(Debug)]
pub(crate) struct NamedEvent {
    pub(crate) name: Token,
    event: Box<[u8]>,
    sender: Option<Box<[u8]>>,
    finish_requested: bool,
}

#[derive(Debug)]
enum State {
    /// Its workspace is being prepared. The charter and conversation wait for
    /// the agent, and inbound events in `held`.
    Preparing { charter: Box<[u8]>, held: Queue<NamedEvent> },
    /// Cancelled for `reason` as it was prepared: it answers once the prepare
    /// has settled.
    Cancelling { reason: Reason },
    /// Its agent is starting, with a workspace when it has items.
    Starting { workspace: Option<Token>, held: Queue<NamedEvent> },
    /// Cancelled for `reason` as its agent started: the agent is stopped once
    /// it has started.
    Unwanted { workspace: Option<Token>, reason: Reason },
    /// Its agent `agent` is at work, with a workspace when it has items.
    Active { workspace: Option<Token>, agent: Token },
    /// It yielded, and waits for its next inbound event.
    Waiting { workspace: Option<Token>, agent: Token },
    /// It ends with `ending` once its agent has gone (`gone`) and its delivery in
    /// flight has settled.
    Stopping { workspace: Option<Token>, agent: Token, ending: RunEnding, gone: bool },
    /// Its unfinished work is being saved; it ends with `ending` once it is.
    Saving { workspace: Token, ending: RunEnding },
    /// Agent and workspace ended; an unacknowledged answer retains the slot.
    Closed,
}

/// How a run ends.
#[derive(Debug)]
enum RunEnding {
    /// It ended with `outcome`, as it said.
    Ended { outcome: Box<[u8]> },
    /// It parked, as it said.
    Parked,
    /// The host selected a turn boundary; the run may still give its own ending.
    Parking,
    /// It failed: as it said, or its agent broke the rules saying it, or
    /// exited without a word.
    Failed { failure: Failure, detail: Box<[u8]> },
    /// The host stopped it, for `failure`: a cancel, or a fault of its
    /// agent. What the run says before its agent has gone still wins.
    Stopped { failure: Failure, detail: Box<[u8]> },
}

// Entry points, one per event: look the run up, take its state out, run the
// cell's handler, conclude from the run's new state.

pub(crate) fn assign(domain: &mut Domain, env: &Env<Limits>, next: Assignment, out: &mut Queue<Request>) {
    let Assignment { assignment, turns, answered, messages } = next;
    if !assignment::messages_fit(&messages, &env.limits) {
        refuse(&assignment, Refusal::Invalid(crate::Invalid::Messages), out);
        return;
    }
    if !conversation_fits(&turns, &answered, &env.limits) {
        let invalid = crate::Invalid::Transcript;
        refuse(&assignment, Refusal::Invalid(invalid), out);
        return;
    }
    let conversation = Conversation { identity: assignment.identity, messages, turns, answered };
    assign_runtime(domain, env, assignment, Runtime { start: Some(conversation), turns: 0, spent: 0, read: None }, out);
}

fn conversation_fits(turns: &[Box<[u8]>], answered: &[AnsweredCall], limits: &Limits) -> bool {
    let Ok(turn_count) = u64::try_from(turns.len()) else { return false };
    let Ok(turn_size) = u64::try_from(size_of::<Box<[u8]>>()) else { return false };
    let Some(mut bytes) = turn_count.checked_mul(turn_size) else {
        return false;
    };
    let Ok(answer_count) = u64::try_from(answered.len()) else { return false };
    let Ok(answer_size) = u64::try_from(size_of::<AnsweredCall>()) else { return false };
    let Some(rows) = answer_count.checked_mul(answer_size) else {
        return false;
    };
    let Some(sum) = bytes.checked_add(rows) else { return false };
    bytes = sum;
    for body in turns {
        if len(body) > limits.turn_bytes {
            return false;
        }
        let Some(sum) = bytes.checked_add(len(body)) else { return false };
        bytes = sum;
    }
    for call in answered {
        if len(&call.name) > limits.event_bytes || len(&call.tool) > limits.event_bytes {
            return false;
        }
        let body = match &call.answer {
            SettledAnswer::Host { error: _, body } => len(body),
            SettledAnswer::Delivery { outcome: _, evidence } => {
                if len(evidence) > limits.delivery_evidence_bytes {
                    return false;
                }
                len(evidence)
            }
        };
        let Some(sum) = bytes.checked_add(len(&call.name)) else { return false };
        let Some(sum) = sum.checked_add(len(&call.tool)) else { return false };
        let Some(sum) = sum.checked_add(body) else { return false };
        bytes = sum;
    }
    bytes <= limits.transcript_bytes
}

fn assign_runtime(
    domain: &mut Domain,
    env: &Env<Limits>,
    assignment: RunAssignment,
    runtime: Runtime,
    out: &mut Queue<Request>,
) {
    // The parent deduplicates wire retries before handing an assignment down.
    if fenced(&domain.names, &domain.hosted, assignment.run, assignment.attempt).is_some()
        || domain.answers.contains_key(&(assignment.run, assignment.attempt))
    {
        refused(&assignment, Refusal::Busy, &runtime, out);
        return;
    }
    let Domain { hosted, names, facts, shut, .. } = domain;
    // An assignment that can never fit is invalid, room or not: busy invites a
    // retry.
    if let Err(invalid) = assignment::check(&assignment, &env.limits) {
        refused(&assignment, Refusal::Invalid(invalid), &runtime, out);
        return;
    }
    // A slot whose run has answered is not free until the engine has the
    // answer.
    let taken = hosted.len();
    if *shut || taken >= env.limits.slots || names.contains_key(&assignment.run) {
        refused(&assignment, Refusal::Busy, &runtime, out);
        return;
    }
    let RunAssignment { run, attempt, workspace, save, live, charter, grants, identity: _ } = assignment;
    let held = Queue::with_capacity(env.limits.held);
    let entry = Hosted {
        run,
        attempt,
        grants,
        refreshed: false,
        answer: None,
        refusals: crate::refusals::Refusals::new(env.limits.inbox),
        save: save && workspace.is_some(),
        live,
        left: None,
        relays: Set::with_capacity(env.limits.run_calls),
        delivery: None,
        runtime,
        state: State::Preparing { charter, held },
        parking: false,
        observed: Some(Phase::Preparing),
    };
    let id = hosted.insert(entry).expect("checked for room above");
    let named = names.insert(run, id).expect("a name for every slot");
    assert!(named.is_none(), "checked the run is not hosted above");
    facts.push(Fact::Admitted { run, attempt });
    facts.activity(LinkUp::Activity { run, attempt, activity: crate::Activity::Phase(Phase::Preparing) });
    match workspace {
        Some(workspace) => out.push(Request::Prepare { owner: id.token(), workspace }),
        None => start_prepared(domain, id.token(), None, out),
    }
}

#[expect(clippy::too_many_arguments, reason = "one message's fields enter through the host boundary")]
pub(crate) fn inbound(
    domain: &mut Domain,
    env: &Env<Limits>,
    run: Token,
    attempt: Token,
    name: Token,
    sender: Box<[u8]>,
    words: Box<[u8]>,
    finish_requested: bool,
    out: &mut Queue<Request>,
) {
    inbound_named(
        domain,
        env,
        run,
        attempt,
        NamedEvent { name, event: words, sender: Some(sender), finish_requested },
        out,
    );
}

fn inbound_named(
    domain: &mut Domain,
    env: &Env<Limits>,
    run: Token,
    attempt: Token,
    message: NamedEvent,
    out: &mut Queue<Request>,
) {
    let Some(id) = fenced(&domain.names, &domain.hosted, run, attempt) else {
        return;
    };
    let sender_len = match &message.sender {
        Some(sender) => len(sender),
        None => 0,
    };
    let fits = match sender_len.checked_add(len(&message.event)) {
        Some(bytes) => bytes <= env.limits.event_bytes,
        None => false,
    };
    if !fits {
        let entry = domain.hosted.get_mut(id).expect("a named run is hosted");
        crate::refusals::keep(&mut entry.refusals, Refused { name: message.name, reason: MessageRefusal::TooLarge });
        conclude(domain, id, out);
        return;
    }
    let entry = domain.hosted.get_mut(id).expect("a named run is hosted");
    let state = mem::replace(&mut entry.state, State::Closed);
    entry.state = match state {
        State::Preparing { charter, mut held } => {
            crate::refusals::hold(&mut held, message, &mut entry.refusals);
            State::Preparing { charter, held }
        }
        State::Starting { workspace, mut held } => {
            crate::refusals::hold(&mut held, message, &mut entry.refusals);
            State::Starting { workspace, held }
        }
        State::Active { workspace, agent } => {
            deliver_message(agent, message, out);
            State::Active { workspace, agent }
        }
        State::Waiting { workspace, agent } => {
            domain.facts.push(Fact::Resumed { run, attempt });
            deliver_message(agent, message, out);
            State::Active { workspace, agent }
        }
        state @ (State::Cancelling { .. } | State::Unwanted { .. } | State::Stopping { .. } | State::Saving { .. }) => {
            crate::refusals::keep(&mut entry.refusals, Refused { name: message.name, reason: MessageRefusal::Ending });
            state
        }
        State::Closed => unreachable!("a closed run has left the names"),
    };
    drain_stopping(domain, env, id, out);
    conclude(domain, id, out);
}

pub(crate) fn cancel(domain: &mut Domain, env: &Env<Limits>, run: Token, attempt: Token, out: &mut Queue<Request>) {
    let Some(id) = fenced(&domain.names, &domain.hosted, run, attempt) else {
        return;
    };
    domain.hosted.get_mut(id).expect("fenced run").parking = false;
    stop(domain, env, id, Reason::Engine, out);
}

/// Remember the drain while a turn runs; other live states stop at once.
pub(crate) fn park(domain: &mut Domain, env: &Env<Limits>, run: Token, attempt: Token, out: &mut Queue<Request>) {
    let Some(id) = fenced(&domain.names, &domain.hosted, run, attempt) else { return };
    let entry = domain.hosted.get_mut(id).expect("fenced run");
    if entry.parking { return; }
    let active = match entry.state {
        State::Active { .. } => true,
        State::Preparing { .. } | State::Starting { .. } | State::Waiting { .. } => false,
        State::Cancelling { .. } | State::Unwanted { .. } | State::Stopping { .. }
        | State::Saving { .. } | State::Closed => return,
    };
    entry.parking = true;
    if !active { stop(domain, env, id, Reason::Engine, out); }
}

fn stopped(entry: &Hosted, reason: Reason) -> RunEnding {
    if entry.parking { RunEnding::Parking }
    else { RunEnding::Stopped { failure: Failure::Cancelled(reason), detail: Box::new([]) } }
}

pub(crate) fn is_hosting(domain: &Domain, run: Token, attempt: Token) -> bool {
    fenced(&domain.names, &domain.hosted, run, attempt).is_some() || domain.answers.contains_key(&(run, attempt))
}

pub(crate) fn find_relay(domain: &Domain, run: Token, attempt: Token, call: &[u8]) -> Option<Id<Call>> {
    let id = fenced(&domain.names, &domain.hosted, run, attempt)?;
    let hosted = domain.hosted.get(id).expect("fenced run");
    for call_id in &hosted.relays {
        let entry = domain.calls.get(*call_id).expect("the run owns every relay");
        if entry.name.as_deref() == Some(call) {
            return Some(*call_id);
        }
    }
    None
}

pub(crate) fn relayed(
    domain: &mut Domain,
    run: Token,
    attempt: Token,
    call: &[u8],
    answer: crate::Relayed,
    out: &mut Queue<Request>,
) {
    let Some(call_id) = find_relay(domain, run, attempt, call) else {
        return;
    };
    let entry = domain.calls.get_mut(call_id).expect("a relay lives until its terminal");
    match entry.state {
        call::State::Relayed { agent } => {
            let name = take_name(entry);
            reply(
                agent,
                name,
                match answer {
                    crate::Relayed::Answer { error, text } => Reply::Relayed { error, text },
                    crate::Relayed::Busy => Reply::Busy,
                },
                out,
            );
        }
        call::State::Settling => {}
        call::State::Delivering { .. } | call::State::Closed => unreachable!("only a live relay receives an answer"),
    }
    relay_ended(domain, call_id, out);
}

pub(crate) fn relay_cancelled(domain: &mut Domain, run: Token, attempt: Token, call: &[u8], out: &mut Queue<Request>) {
    let Some(call_id) = find_relay(domain, run, attempt, call) else {
        return;
    };
    let entry = domain.calls.get(call_id).expect("a cancelled relay lives until its terminal");
    match entry.state {
        call::State::Settling => {}
        call::State::Relayed { .. } | call::State::Delivering { .. } | call::State::Closed => {
            unreachable!("a relay is cancelled only after its cancel request")
        }
    }
    relay_ended(domain, call_id, out);
}

fn relay_ended(domain: &mut Domain, call_id: Id<Call>, out: &mut Queue<Request>) {
    let entry = domain.calls.get_mut(call_id).expect("a relay lives until its terminal");
    let id = entry.hosted;
    entry.state = call::State::Closed;
    entry.name = None;
    domain.calls.retire(call_id);
    let entry = domain.hosted.get_mut(id).expect("a run lives until every relay settles");
    let removed = entry.relays.remove(&call_id);
    assert!(removed, "a relay belongs to its run until it settles");
    conclude(domain, id, out);
}

pub(crate) fn cancel_all(domain: &mut Domain, reason: Reason) {
    let Domain { names, ready, shut, .. } = domain;
    match reason {
        // A host shutting down takes no more work.
        Reason::Shutdown => *shut = true,
        Reason::Engine | Reason::Contact => {}
    }
    for (_, id) in names.iter() {
        if !ready.contains_key(id) {
            let earlier = ready.insert(*id, reason).expect("room on the ready list for every run");
            assert!(earlier.is_none(), "checked the run is not ready above");
        }
    }
}

/// Cancels the first run on the ready list, if there is one.
pub(crate) fn resume(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    let Some((id, reason)) = domain.ready.first() else {
        return;
    };
    let (id, reason) = (*id, *reason);
    domain.ready.remove(&id);
    domain.hosted.get_mut(id).expect("ready run").parking = false;
    stop(domain, env, id, reason, out);
}

pub(crate) fn prepared(domain: &mut Domain, owner: Token, workspace: Token, out: &mut Queue<Request>) {
    start_prepared(domain, owner, Some(workspace), out);
}

fn start_prepared(domain: &mut Domain, owner: Token, workspace: Option<Token>, out: &mut Queue<Request>) {
    let Domain { hosted, facts, .. } = domain;
    let id = Id::<Hosted>::from_token(owner);
    let entry = hosted.get_mut(id).expect("a run lives until its prepare has settled");
    if workspace.is_some() {
        facts.push(Fact::Prepared { run: entry.run, attempt: entry.attempt });
    }
    let state = mem::replace(&mut entry.state, State::Closed);
    entry.state = match state {
        State::Preparing { charter, held } => {
            let mut grants = List::with_capacity(u32::try_from(entry.grants.len()).expect("validated grants"));
            for grant in &entry.grants {
                if agent_grant(grant.form) {
                    grants.push(*grant).expect("room for every account grant");
                }
            }
            let conversation = entry.runtime.start.take().expect("an agent is started once");
            out.push(Request::Start {
                owner,
                start: Box::new(crate::Start {
                    live: entry.live,
                    workspace,
                    charter,
                    activation: entry.attempt.raw(),
                    identity: conversation.identity,
                    turns: conversation.turns,
                    answered: conversation.answered,
                    messages: conversation.messages,
                    grants: grants.into_boxed(),
                }),
            });
            entry.refreshed = false;
            State::Starting { workspace, held }
        }
        State::Cancelling { reason } => {
            let ending = stopped(entry, reason);
            release(entry, facts, workspace, ending, None, out)
        }
        State::Starting { .. }
        | State::Unwanted { .. }
        | State::Active { .. }
        | State::Waiting { .. }
        | State::Stopping { .. }
        | State::Saving { .. }
        | State::Closed => unreachable!("a workspace is prepared once, while its run prepares"),
    };
    conclude(domain, id, out);
}

pub(crate) fn unprepared(
    domain: &mut Domain,
    env: &Env<Limits>,
    owner: Token,
    failure: Preparation,
    detail: Box<[u8]>,
    out: &mut Queue<Request>,
) {
    let Domain { hosted, facts, .. } = domain;
    let id = Id::<Hosted>::from_token(owner);
    let entry = hosted.get_mut(id).expect("a run lives until its prepare has settled");
    let state = mem::replace(&mut entry.state, State::Closed);
    entry.state = match state {
        // Nothing of the workspace is held: nothing to release.
        State::Preparing { held, .. } => {
            crate::refusals::ending(held, &mut entry.refusals);
            let detail = tail(detail, &env.limits);
            let ending = RunEnding::Failed { failure: Failure::Unprepared(failure), detail };
            answer(entry, facts, ending, None, out)
        }
        // Cancelled first: the prepare's failure is not the answer's.
        State::Cancelling { reason } => {
            let ending = stopped(entry, reason);
            answer(entry, facts, ending, None, out)
        }
        State::Starting { .. }
        | State::Unwanted { .. }
        | State::Active { .. }
        | State::Waiting { .. }
        | State::Stopping { .. }
        | State::Saving { .. }
        | State::Closed => unreachable!("a workspace is prepared once, while its run prepares"),
    };
    drain_stopping(domain, env, id, out);
    conclude(domain, id, out);
}

pub(crate) fn started(domain: &mut Domain, env: &Env<Limits>, owner: Token, agent: Token, out: &mut Queue<Request>) {
    let Domain { hosted, facts, .. } = domain;
    let id = Id::<Hosted>::from_token(owner);
    let entry = hosted.get_mut(id).expect("a run lives until its agent has gone");
    facts.push(Fact::Started { run: entry.run, attempt: entry.attempt });
    let state = mem::replace(&mut entry.state, State::Closed);
    entry.state = match state {
        State::Starting { workspace, mut held } => {
            if entry.refreshed {
                for grant in &entry.grants {
                    if agent_grant(grant.form) {
                        out.push(Request::Grant { agent, grant: *grant });
                    }
                }
                entry.refreshed = false;
            }
            // Bounded: the hold has room for the limit, and no more.
            for _ in 0..env.limits.held {
                let Some(event) = held.pop() else {
                    break;
                };
                deliver_message(agent, event, out);
            }
            State::Active { workspace, agent }
        }
        State::Unwanted { workspace, reason } => {
            out.push(Request::Stop { agent });
            let ending = stopped(entry, reason);
            State::Stopping { workspace, agent, ending, gone: false }
        }
        State::Preparing { .. }
        | State::Cancelling { .. }
        | State::Active { .. }
        | State::Waiting { .. }
        | State::Stopping { .. }
        | State::Saving { .. }
        | State::Closed => unreachable!("an agent starts once, after its start"),
    };
    drain_stopping(domain, env, id, out);
    conclude(domain, id, out);
}

pub(crate) fn called(
    domain: &mut Domain,
    env: &Env<Limits>,
    owner: Token,
    call: Box<[u8]>,
    ask: Ask,
    out: &mut Queue<Request>,
) {
    called_named(domain, env, owner, call, ask, out);
}

fn called_named(
    domain: &mut Domain,
    env: &Env<Limits>,
    owner: Token,
    call: CallName,
    ask: Ask,
    out: &mut Queue<Request>,
) {
    let Domain { hosted, calls, facts, .. } = domain;
    let id = Id::<Hosted>::from_token(owner);
    let entry = hosted.get_mut(id).expect("a run lives until its agent has gone");
    let state = mem::replace(&mut entry.state, State::Closed);
    entry.state = match state {
        // A run that calls is at work, whatever it said before.
        State::Active { workspace, agent } => {
            serve(entry, calls, id, workspace, agent, call, ask, &env.limits, out);
            State::Active { workspace, agent }
        }
        State::Waiting { workspace, agent } => {
            facts.push(Fact::Resumed { run: entry.run, attempt: entry.attempt });
            serve(entry, calls, id, workspace, agent, call, ask, &env.limits, out);
            State::Active { workspace, agent }
        }
        // Once a run is cancelled or ending, its calls are not served.
        State::Stopping { workspace, agent, ending, gone } => {
            reply(agent, call, Reply::Unavailable, out);
            State::Stopping { workspace, agent, ending, gone }
        }
        State::Preparing { .. }
        | State::Cancelling { .. }
        | State::Starting { .. }
        | State::Unwanted { .. }
        | State::Saving { .. }
        | State::Closed => unreachable!("only an agent that has started and not gone calls"),
    };
    drain_stopping(domain, env, id, out);
    conclude(domain, id, out);
}

pub(crate) fn withdrawn(domain: &mut Domain, owner: Token, call: Box<[u8]>, out: &mut Queue<Request>) {
    withdrawn_named(domain, owner, call, out);
}

fn withdrawn_named(domain: &mut Domain, owner: Token, call: CallName, out: &mut Queue<Request>) {
    let Domain { hosted, calls, .. } = domain;
    let id = Id::<Hosted>::from_token(owner);
    let entry = hosted.get_mut(id).expect("a run lives until its agent has gone");
    match &entry.state {
        State::Active { .. } | State::Waiting { .. } => withdraw(entry, calls, call, out),
        // Its relays were answered as it left live, and its delivery goes on.
        State::Stopping { .. } => {}
        State::Preparing { .. }
        | State::Cancelling { .. }
        | State::Starting { .. }
        | State::Unwanted { .. }
        | State::Saving { .. }
        | State::Closed => unreachable!("only an agent that has started and not gone withdraws a call"),
    }
    conclude(domain, id, out);
}

pub(crate) fn refused_message(
    domain: &mut Domain,
    owner: Token,
    name: Token,
    reason: MessageRefusal,
    out: &mut Queue<Request>,
) {
    let id = Id::<Hosted>::from_token(owner);
    let entry = domain.hosted.get_mut(id).expect("a run lives until its agent has gone");
    match &entry.state {
        State::Active { .. } | State::Waiting { .. } | State::Stopping { .. } => {
            crate::refusals::keep(&mut entry.refusals, Refused { name, reason });
        }
        State::Preparing { .. }
        | State::Cancelling { .. }
        | State::Starting { .. }
        | State::Unwanted { .. }
        | State::Saving { .. }
        | State::Closed => unreachable!("only an agent that has started and not gone bounces an event"),
    }
    conclude(domain, id, out);
}

pub(crate) fn yielded(domain: &mut Domain, env: &Env<Limits>, owner: Token, out: &mut Queue<Request>) {
    let id = Id::<Hosted>::from_token(owner);
    let entry = domain.hosted.get_mut(id).expect("a run lives until its agent has gone");
    let state = mem::replace(&mut entry.state, State::Closed);
    entry.state = match state {
        State::Active { workspace, agent } => {
            domain.facts.push(Fact::Waiting { run: entry.run, attempt: entry.attempt });
            State::Waiting { workspace, agent }
        }
        State::Waiting { workspace, agent } => State::Waiting { workspace, agent },
        state @ State::Stopping { .. } => state,
        State::Preparing { .. }
        | State::Cancelling { .. }
        | State::Starting { .. }
        | State::Unwanted { .. }
        | State::Saving { .. }
        | State::Closed => unreachable!("only an agent that has started and not gone yields"),
    };
    if entry.parking { stop(domain, env, id, Reason::Engine, out); }
    conclude(domain, id, out);
}

pub(crate) fn finish_run(
    domain: &mut Domain,
    env: &Env<Limits>,
    owner: Token,
    finish: Finish,
    out: &mut Queue<Request>,
) {
    let Domain { hosted, calls, .. } = domain;
    let id = Id::<Hosted>::from_token(owner);
    let entry = hosted.get_mut(id).expect("a run lives until its agent has gone");
    let state = mem::replace(&mut entry.state, State::Closed);
    entry.state = match state {
        State::Active { workspace, agent } | State::Waiting { workspace, agent } => {
            let ending = said(finish, Failure::Run(RunFailure::Cancelled), &env.limits);
            leave(entry.run, entry.attempt, &entry.relays, calls, out);
            out.push(Request::Stop { agent });
            State::Stopping { workspace, agent, ending, gone: false }
        }
        State::Stopping { workspace, agent, ending, gone } => {
            let ending = match ending {
                // Stopped by the host, the run says how it finishes as it
                // winds down: its own ending wins.
                RunEnding::Stopped { failure, detail: _ } => said(finish, failure, &env.limits),
                RunEnding::Parking => said(finish, Failure::Run(RunFailure::Cancelled), &env.limits),
                // It said how it finishes already, or its agent exited.
                ending @ (RunEnding::Ended { .. } | RunEnding::Parked | RunEnding::Failed { .. }) => ending,
            };
            State::Stopping { workspace, agent, ending, gone }
        }
        State::Preparing { .. }
        | State::Cancelling { .. }
        | State::Starting { .. }
        | State::Unwanted { .. }
        | State::Saving { .. }
        | State::Closed => unreachable!("only an agent that has started and not gone finishes"),
    };
    drain_stopping(domain, env, id, out);
    conclude(domain, id, out);
}

pub(crate) fn faulted(
    domain: &mut Domain,
    env: &Env<Limits>,
    owner: Token,
    fault: AgentFailure,
    out: &mut Queue<Request>,
) {
    let Domain { hosted, calls, .. } = domain;
    let id = Id::<Hosted>::from_token(owner);
    let entry = hosted.get_mut(id).expect("a run lives until its agent has gone");
    let state = mem::replace(&mut entry.state, State::Closed);
    entry.state = match state {
        State::Active { workspace, agent } | State::Waiting { workspace, agent } => {
            let ending = RunEnding::Stopped { failure: Failure::Agent(fault), detail: Box::new([]) };
            leave(entry.run, entry.attempt, &entry.relays, calls, out);
            out.push(Request::Stop { agent });
            State::Stopping { workspace, agent, ending, gone: false }
        }
        // How the run ends is decided already, or the host stopped it first.
        state @ State::Stopping { .. } => state,
        State::Preparing { .. }
        | State::Cancelling { .. }
        | State::Starting { .. }
        | State::Unwanted { .. }
        | State::Saving { .. }
        | State::Closed => unreachable!("only an agent that has started and not gone is faulted"),
    };
    drain_stopping(domain, env, id, out);
    conclude(domain, id, out);
}

pub(crate) fn gone(domain: &mut Domain, env: &Env<Limits>, owner: Token, detail: Box<[u8]>, out: &mut Queue<Request>) {
    let Domain { hosted, calls, facts, .. } = domain;
    let id = Id::<Hosted>::from_token(owner);
    let entry = hosted.get_mut(id).expect("a run lives until its agent has gone");
    let detail = tail(detail, &env.limits);
    let state = mem::replace(&mut entry.state, State::Closed);
    entry.state = match state {
        // Nothing ran: nothing to save.
        State::Starting { workspace, held } => {
            crate::refusals::ending(held, &mut entry.refusals);
            let ending = RunEnding::Failed { failure: Failure::Agent(AgentFailure::Unstarted), detail };
            release(entry, facts, workspace, ending, None, out)
        }
        State::Unwanted { workspace, reason } => {
            let ending = explained(stopped(entry, reason), detail);
            release(entry, facts, workspace, ending, None, out)
        }
        // It exited without saying how its run finishes.
        State::Active { workspace, agent } | State::Waiting { workspace, agent } => {
            let ending = RunEnding::Failed { failure: Failure::Agent(AgentFailure::Exited), detail };
            leave(entry.run, entry.attempt, &entry.relays, calls, out);
            State::Stopping { workspace, agent, ending, gone: true }
        }
        State::Stopping { workspace, agent, ending, gone } => {
            assert!(!gone, "an agent goes once");
            let ending = explained(ending, detail);
            State::Stopping { workspace, agent, ending, gone: true }
        }
        State::Preparing { .. } | State::Cancelling { .. } | State::Saving { .. } | State::Closed => {
            unreachable!("only an agent that was started goes")
        }
    };
    drain_stopping(domain, env, id, out);
    conclude(domain, id, out);
}

pub(crate) fn delivered(domain: &mut Domain, owner: Token, delivery: Delivery, out: &mut Queue<Request>) {
    let Domain { hosted, calls, .. } = domain;
    let call_id = Id::<Call>::from_token(owner);
    let call = calls.get_mut(call_id).expect("a delivery call lives until it settles");
    let id = call.hosted;
    let state = mem::replace(&mut call.state, call::State::Closed);
    let name = call.name.take();
    calls.retire(call_id);
    let entry = hosted.get_mut(id).expect("a run lives until its delivery settles");
    assert!(entry.delivery == Some(call_id), "one delivery is in flight");
    entry.delivery = None;
    if delivery.changed {
        entry.left = Some(delivery.left);
    }
    match state {
        call::State::Delivering { agent } => {
            let name = name.expect("a delivery retains its opaque name");
            reply(agent, name, Reply::Delivered(delivery), out);
        }
        call::State::Relayed { .. } | call::State::Settling | call::State::Closed => {
            unreachable!("only a delivery ends a delivery call")
        }
    }
    conclude(domain, id, out);
}

pub(crate) fn saved(domain: &mut Domain, owner: Token, at: Option<Token>, out: &mut Queue<Request>) {
    let Domain { hosted, facts, .. } = domain;
    let id = Id::<Hosted>::from_token(owner);
    let entry = hosted.get_mut(id).expect("a run lives until its save settles");
    let state = mem::replace(&mut entry.state, State::Closed);
    entry.state = match state {
        State::Saving { workspace, ending } => release(entry, facts, Some(workspace), ending, at, out),
        State::Preparing { .. }
        | State::Cancelling { .. }
        | State::Starting { .. }
        | State::Unwanted { .. }
        | State::Active { .. }
        | State::Waiting { .. }
        | State::Stopping { .. }
        | State::Closed => unreachable!("a save ends once, while its run saves"),
    };
    conclude(domain, id, out);
}

/// The engine's names for the hosted run `owner`, and where it is, unless it
/// has closed.
pub(crate) fn hosting(domain: &Domain, owner: Token) -> Option<Hosting> {
    let entry = domain.hosted.get(Id::<Hosted>::from_token(owner))?;
    match &entry.state {
        State::Closed => {
            entry.answer.is_some().then_some(Hosting { run: entry.run, attempt: entry.attempt, phase: Phase::Answered })
        }
        state @ (State::Preparing { .. }
        | State::Cancelling { .. }
        | State::Starting { .. }
        | State::Unwanted { .. }
        | State::Active { .. }
        | State::Waiting { .. }
        | State::Stopping { .. }
        | State::Saving { .. }) => Some(Hosting { run: entry.run, attempt: entry.attempt, phase: phase(state) }),
    }
}

/// The hosted run the engine names `run`, if it hosts that attempt at it.
fn fenced(names: &Map<Token, Id<Hosted>>, hosted: &Slab<Hosted>, run: Token, attempt: Token) -> Option<Id<Hosted>> {
    let id = *names.get(&run)?;
    let entry = hosted.get(id).expect("a named run is hosted");
    (entry.attempt == attempt).then_some(id)
}

/// Cancels the run `id` for `reason`: the cancel cells.
fn stop(domain: &mut Domain, env: &Env<Limits>, id: Id<Hosted>, reason: Reason, out: &mut Queue<Request>) {
    let Domain { hosted, calls, .. } = domain;
    let entry = hosted.get_mut(id).expect("a run on the names or the ready list is hosted");
    let state = mem::replace(&mut entry.state, State::Closed);
    entry.state = match state {
        // A prepare in flight is abandoned, and its end waited for.
        State::Preparing { held, .. } => {
            crate::refusals::ending(held, &mut entry.refusals);
            out.push(Request::Abort { owner: id.token() });
            State::Cancelling { reason }
        }
        State::Starting { workspace, held } => {
            crate::refusals::ending(held, &mut entry.refusals);
            State::Unwanted { workspace, reason }
        }
        State::Active { workspace, agent } | State::Waiting { workspace, agent } => {
            let ending = stopped(entry, reason);
            leave(entry.run, entry.attempt, &entry.relays, calls, out);
            out.push(Request::Stop { agent });
            State::Stopping { workspace, agent, ending, gone: false }
        }
        State::Stopping { workspace, agent, ending: RunEnding::Parking, gone } if !entry.parking => {
            State::Stopping { workspace, agent, ending: stopped(entry, reason), gone }
        }
        // How the run ends is decided already, or it is stopping.
        state @ (State::Cancelling { .. } | State::Unwanted { .. } | State::Stopping { .. } | State::Saving { .. }) => {
            state
        }
        State::Closed => unreachable!("a closed run has left the names and the ready list"),
    };
    drain_stopping(domain, env, id, out);
    conclude(domain, id, out);
}

fn drain_stopping(domain: &mut Domain, env: &Env<Limits>, id: Id<Hosted>, out: &mut Queue<Request>) {
    let entry = domain.hosted.get(id).expect("hosted run lives through its transition");
    let stopping = match entry.state {
        State::Stopping { .. } => true,
        State::Preparing { .. }
        | State::Cancelling { .. }
        | State::Starting { .. }
        | State::Unwanted { .. }
        | State::Active { .. }
        | State::Waiting { .. }
        | State::Saving { .. }
        | State::Closed => false,
    };
    if stopping {
        let mode = if entry.parking { crate::turns::Mode::Parking } else { crate::turns::Mode::Stopping };
        domain.turns.admit(entry.run, entry.attempt, mode, &env.limits, out);
    }
}

/// What a run's state implies, applied after every transition: a stopping run
/// that has settled saves, or releases its workspace and answers; and a
/// Closed run is retired, leaving the names and the ready list.
fn conclude(domain: &mut Domain, id: Id<Hosted>, out: &mut Queue<Request>) {
    let Domain { hosted, names, ready, facts, answers, refusals_lost, turns, .. } = domain;
    let entry = hosted.get_mut(id).expect("a run lives until it is retired");
    facts.phase(entry.run, entry.attempt, &mut entry.observed, phase(&entry.state));
    *refusals_lost = refusals_lost.saturating_add(entry.refusals.lost);
    entry.refusals.lost = 0;
    let settled = match &entry.state {
        State::Stopping { gone, .. } => *gone && entry.delivery.is_none() && entry.relays.is_empty()
            && (!entry.parking || !turns.has_run(entry.run, entry.attempt)),
        State::Preparing { .. }
        | State::Cancelling { .. }
        | State::Starting { .. }
        | State::Unwanted { .. }
        | State::Active { .. }
        | State::Waiting { .. }
        | State::Saving { .. }
        | State::Closed => false,
    };
    if settled {
        let state = mem::replace(&mut entry.state, State::Closed);
        entry.state = match state {
            State::Stopping { workspace, agent: _, ending, gone: _ } => {
                settle(entry, facts, id, workspace, ending, out)
            }
            State::Preparing { .. }
            | State::Cancelling { .. }
            | State::Starting { .. }
            | State::Unwanted { .. }
            | State::Active { .. }
            | State::Waiting { .. }
            | State::Saving { .. }
            | State::Closed => unreachable!("only a stopping run settles"),
        };
    }
    facts.phase(entry.run, entry.attempt, &mut entry.observed, phase(&entry.state));
    let closed = match &entry.state {
        State::Closed => true,
        State::Preparing { .. }
        | State::Cancelling { .. }
        | State::Starting { .. }
        | State::Unwanted { .. }
        | State::Active { .. }
        | State::Waiting { .. }
        | State::Stopping { .. }
        | State::Saving { .. } => false,
    };
    if closed {
        let run = entry.run;
        names.remove(&run);
        ready.remove(&id);
        if entry.answer.is_some() {
            answers.insert((run, entry.attempt), id).expect("one answer per occupied slot");
        } else {
            hosted.retire(id);
        }
    }
}

// Cell handlers and what they share.

/// Stopping, settled: nothing of the run is running and nothing it asked of
/// the workspace is in flight. Its work is saved if the assignment asks,
/// unless it ended with a landed change, which leaves nothing to save; it
/// releases its workspace and answers otherwise. A save of an unchanged tree
/// pushes nothing.
fn settle(
    entry: &mut Hosted,
    facts: &mut Facts,
    id: Id<Hosted>,
    workspace: Option<Token>,
    ending: RunEnding,
    out: &mut Queue<Request>,
) -> State {
    let delivered_end = match &ending {
        RunEnding::Ended { .. } => entry.left.is_some(),
        RunEnding::Parked | RunEnding::Parking | RunEnding::Failed { .. } | RunEnding::Stopped { .. } => false,
    };
    match workspace {
        Some(workspace) if entry.save && !delivered_end => {
            out.push(Request::Save { owner: id.token(), workspace });
            State::Saving { workspace, ending }
        }
        Some(_) | None => release(entry, facts, workspace, ending, None, out),
    }
}

/// Releases the run's workspace and answers.
fn release(
    entry: &mut Hosted,
    facts: &mut Facts,
    workspace: Option<Token>,
    ending: RunEnding,
    saved: Option<Token>,
    out: &mut Queue<Request>,
) -> State {
    if let Some(workspace) = workspace {
        out.push(Request::Release { workspace });
    }
    answer(entry, facts, ending, saved, out)
}

/// Answers the engine: the run's one answer.
fn answer(
    entry: &mut Hosted,
    facts: &mut Facts,
    ending: RunEnding,
    saved: Option<Token>,
    out: &mut Queue<Request>,
) -> State {
    let (run, attempt) = (entry.run, entry.attempt);
    let work = Work { left: entry.left, saved };
    let answer = match ending {
        RunEnding::Ended { outcome } => {
            facts.push(Fact::Ended { run, attempt });
            Ending::Ended { outcome, work }
        }
        RunEnding::Parked | RunEnding::Parking => {
            facts.push(Fact::Parked { run, attempt });
            Ending::Parked { work }
        }
        RunEnding::Failed { failure, detail } | RunEnding::Stopped { failure, detail } => {
            facts.push(Fact::Failed { run, attempt, failure });
            Ending::Failed { failure, detail, work }
        }
    };
    let answer = crate::Answer {
        turns: entry.runtime.turns,
        spent: entry.runtime.spent,
        read: entry.runtime.read,
        refused: entry.refusals.take(),
        ending: answer,
    };
    entry.answer = Some(crate::answers::Kept { answer: answer.clone() });
    out.push(Request::Up(LinkUp::Answer { run, attempt, answer }));
    State::Closed
}

fn deliver_message(agent: Token, message: NamedEvent, out: &mut Queue<Request>) {
    match message.sender {
        Some(sender) => out.push(Request::Message {
            agent,
            name: message.name,
            sender,
            words: message.event,
            finish_requested: message.finish_requested,
        }),
        None => unreachable!("every held message retains its sender label"),
    }
}

/// Serves the host call `call` of the live run `id`, through its workspace
/// or the engine, or answers it as busy.
#[expect(clippy::too_many_arguments, reason = "a cell handler takes the fields it touches")]
fn serve(
    entry: &mut Hosted,
    calls: &mut Slab<Call>,
    id: Id<Hosted>,
    workspace: Option<Token>,
    agent: Token,
    call: CallName,
    ask: Ask,
    limits: &Limits,
    out: &mut Queue<Request>,
) {
    let name_fits = len(&call) <= limits.event_bytes;
    let input_fits = match &ask {
        Ask::Relay { tool, input, .. } => match len(tool).checked_add(len(input)) {
            Some(bytes) => bytes <= limits.event_bytes,
            None => false,
        },
        Ask::Deliver { .. } => true,
    };
    if !name_fits || !input_fits {
        reply(agent, call, Reply::Unavailable, out);
        return;
    }
    let delivery = match &ask {
        Ask::Deliver { .. } => true,
        Ask::Relay { .. } => false,
    };
    if delivery && workspace.is_none() {
        reply(agent, call, Reply::Unavailable, out);
        return;
    }
    for pending in &entry.relays {
        let pending = calls.get(*pending).expect("the run owns every relay");
        if pending.name.as_ref() == Some(&call) {
            reply(agent, call, Reply::Busy, out);
            return;
        }
    }
    let in_flight = entry.relays.len().saturating_add(u32::from(entry.delivery.is_some()));
    // A delivery changes a workspace: one at a time.
    if in_flight >= limits.run_calls || (delivery && entry.delivery.is_some()) {
        reply(agent, call, Reply::Busy, out);
        return;
    }
    let name = Some(call.clone());
    let state = if delivery { call::State::Delivering { agent } } else { call::State::Relayed { agent } };
    // Calls answered in this iteration keep their slots until the reclaim
    // point: a call may find none free even within its run's limit.
    let Ok(call_id) = calls.insert(Call { hosted: id, state, name }) else {
        reply(agent, call, Reply::Busy, out);
        return;
    };
    match ask {
        Ask::Deliver { title, body } => {
            entry.delivery = Some(call_id);
            out.push(Request::Deliver {
                owner: call_id.token(),
                workspace: workspace.expect("a delivery has a workspace"),
                title,
                body,
            });
        }
        Ask::Relay { tool, writes, input, deadline } => {
            let added = entry.relays.insert(call_id).expect("checked the run's calls for room above");
            assert!(added, "a call is new to its run");

            out.push(Request::Up(LinkUp::Call {
                run: entry.run,
                attempt: entry.attempt,
                call,
                tool,
                writes,
                input,
                deadline,
            }));
        }
    }
}

/// The live run withdrew the call its agent names `call`: a relay in flight is
/// answered as withdrawn, and closes. A delivery goes on, and is answered once it
/// settles; a call answered already is not found, and nothing happens.
fn withdraw(entry: &mut Hosted, calls: &mut Slab<Call>, call: CallName, out: &mut Queue<Request>) {
    let mut found = None;
    for call_id in &entry.relays {
        let relayed = calls.get(*call_id).expect("a run's calls live until they close");
        let same = relayed.name.as_ref() == Some(&call);
        if same {
            found = Some(*call_id);
        }
    }
    let Some(call_id) = found else {
        return;
    };
    let relayed = calls.get_mut(call_id).expect("found above");
    match relayed.state {
        call::State::Relayed { agent } => {
            let name = relayed.name.as_ref().expect("a pending relay has its name").clone();
            reply(agent, name, Reply::Withdrawn, out);
            relayed.state = call::State::Settling;
            out.push(Request::CancelRelay { run: entry.run, attempt: entry.attempt, call });
        }
        call::State::Settling => {}
        call::State::Delivering { .. } | call::State::Closed => {
            unreachable!("found among the active relays")
        }
    }
}

/// Leaving live answers each relay's agent at once, then waits for the
/// cancellation of local delivery. Each binding ends with a terminal event.
fn leave(run: Token, attempt: Token, relays: &Set<Id<Call>>, calls: &mut Slab<Call>, out: &mut Queue<Request>) {
    for call_id in relays {
        let entry = calls.get_mut(*call_id).expect("a run's relays live until their terminals");
        match entry.state {
            call::State::Relayed { agent } => {
                let name = entry.name.as_ref().expect("a pending relay has its name").clone();
                reply(agent, name.clone(), Reply::Unavailable, out);
                entry.state = call::State::Settling;
                out.push(Request::CancelRelay { run, attempt, call: name });
            }
            call::State::Settling => {}
            call::State::Delivering { .. } | call::State::Closed => {
                unreachable!("the run holds only its pending relays")
            }
        }
    }
}

/// How the run ends, as it says it finishes; a cancel it reports is
/// `cancelled`. Saying more than the limits allow breaks the rules.
fn said(finish: Finish, cancelled: Failure, limits: &Limits) -> RunEnding {
    let rules = RunEnding::Failed { failure: Failure::Agent(AgentFailure::Rules), detail: Box::new([]) };
    match finish {
        Finish::Ended { outcome } if len(&outcome) > limits.outcome_bytes => rules,
        Finish::Ended { outcome } => RunEnding::Ended { outcome },
        Finish::Parked => RunEnding::Parked,
        Finish::Failed { failure: RunFailure::Cancelled } => {
            RunEnding::Failed { failure: cancelled, detail: Box::new([]) }
        }
        Finish::Failed { failure } => RunEnding::Failed { failure: Failure::Run(failure), detail: Box::new([]) },
    }
}

/// A failure's ending gains the detail its agent left as it went.
fn explained(ending: RunEnding, detail: Box<[u8]>) -> RunEnding {
    match ending {
        RunEnding::Failed { failure, detail: _ } => RunEnding::Failed { failure, detail },
        RunEnding::Stopped { failure, detail: _ } => RunEnding::Stopped { failure, detail },
        ending @ (RunEnding::Ended { .. } | RunEnding::Parked | RunEnding::Parking) => ending,
    }
}

/// The last bytes of `detail`, as many as the limits keep.
fn tail(detail: Box<[u8]>, limits: &Limits) -> Box<[u8]> {
    let keep = usize::try_from(limits.detail_bytes).expect("a u32 fits in a usize");
    match detail.len().checked_sub(keep) {
        Some(cut) if cut > 0 => copy_of(detail.get(cut..).expect("cut within the detail")),
        Some(_) | None => detail,
    }
}

fn phase(state: &State) -> Phase {
    match state {
        State::Preparing { .. } => Phase::Preparing,
        State::Starting { .. } => Phase::Starting,
        State::Active { .. } => Phase::Active,
        State::Waiting { .. } => Phase::Waiting,
        State::Cancelling { .. } | State::Unwanted { .. } | State::Stopping { .. } | State::Saving { .. } => {
            Phase::Ending
        }
        State::Closed => Phase::Answered,
    }
}

/// A refresh is fenced to the attempt and to one of its admitted accounts.
pub(crate) fn grant(domain: &mut Domain, run: Token, attempt: Token, grant: Grant, out: &mut Queue<Request>) {
    let Some(id) = fenced(&domain.names, &domain.hosted, run, attempt) else {
        return;
    };
    let entry = domain.hosted.get_mut(id).expect("a named run is hosted");
    let mut accepted = false;
    for current in &mut entry.grants {
        if current.form.table() == grant.form.table()
            && current.account == grant.account
            && current.generation < grant.generation
        {
            *current = grant;
            accepted = true;
        }
    }
    if !accepted || !agent_grant(grant.form) {
        return;
    }
    entry.refreshed = true;
    match entry.state {
        State::Active { agent, .. } | State::Waiting { agent, .. } => out.push(Request::Grant { agent, grant }),
        State::Preparing { .. }
        | State::Starting { .. }
        | State::Cancelling { .. }
        | State::Unwanted { .. }
        | State::Stopping { .. }
        | State::Saving { .. }
        | State::Closed => {}
    }
}

fn next_answer(turns: u32, spent: u64, answer: Ending) -> crate::Answer {
    crate::Answer { turns, spent, read: None, refused: Box::new([]), ending: answer }
}

fn refuse(assignment: &RunAssignment, refusal: Refusal, out: &mut Queue<Request>) {
    out.push(Request::Up(LinkUp::Answer {
        run: assignment.run,
        attempt: assignment.attempt,
        answer: next_answer(0, 0, Ending::Refused(refusal)),
    }));
}

fn refused(assignment: &RunAssignment, refusal: Refusal, _runtime: &Runtime, out: &mut Queue<Request>) {
    refuse(assignment, refusal, out);
}

pub(crate) fn turned(
    domain: &mut Domain,
    env: &Env<Limits>,
    owner: Token,
    mut turn: crate::Turn,
    out: &mut Queue<Request>,
) {
    let id = Id::<Hosted>::from_token(owner);
    let Some(entry) = domain.hosted.get_mut(id) else {
        return;
    };
    let agent = match entry.state {
        State::Active { agent, .. } | State::Waiting { agent, .. } | State::Stopping { agent, gone: false, .. } => {
            agent
        }
        State::Preparing { .. }
        | State::Cancelling { .. }
        | State::Starting { .. }
        | State::Unwanted { .. }
        | State::Stopping { gone: true, .. }
        | State::Saving { .. }
        | State::Closed => return,
    };
    let valid = if entry.runtime.turns.checked_add(1) != Some(turn.turn)
        || len(&turn.body) > env.limits.turn_bytes
        || !turn.refused.is_empty()
    {
        false
    } else {
        entry.runtime.turns = turn.turn;
        entry.runtime.read = turn.read;
        entry.runtime.spent = entry.runtime.spent.saturating_add(turn.spent);
        true
    };
    if valid {
        let (run, attempt) = (entry.run, entry.attempt);
        turn.refused = entry.refusals.take();
        domain.turns.retain(agent, run, attempt, turn);
        let parking = entry.parking;
        let stopping = matches!(entry.state, State::Stopping { .. });
        if parking && !stopping { stop(domain, env, id, Reason::Engine, out); }
        let mode = if parking { crate::turns::Mode::Parking }
            else if stopping { crate::turns::Mode::Stopping } else { crate::turns::Mode::Live };
        domain.turns.admit(run, attempt, mode, &env.limits, out);
    } else {
        faulted(domain, env, owner, AgentFailure::Rules, out);
    }
}

/// Release a committed turn and resume the agent reader when room returns.
pub(crate) fn acknowledge_turn(
    domain: &mut Domain,
    env: &Env<Limits>,
    run: Token,
    attempt: Token,
    turn: u32,
    out: &mut Queue<Request>,
) {
    domain.turns.acknowledge(run, attempt, turn);
    let id = fenced(&domain.names, &domain.hosted, run, attempt);
    let parking = match id { Some(id) => domain.hosted.get(id).expect("fenced run").parking, None => false };
    let mode = if parking { crate::turns::Mode::Parking } else { crate::turns::Mode::Live };
    domain.turns.admit(run, attempt, mode, &env.limits, out);
    if let Some(id) = id { conclude(domain, id, out); }
}

#[expect(clippy::too_many_arguments, reason = "one last word carries its fence and accounting")]
pub(crate) fn finished(
    domain: &mut Domain,
    env: &Env<Limits>,
    owner: Token,
    turns: u32,
    spent: u64,
    finish: Finish,
    read: Option<Token>,
    out: &mut Queue<Request>,
) {
    let Some(entry) = domain.hosted.get_mut(Id::<Hosted>::from_token(owner)) else {
        return;
    };
    let valid = turns == entry.runtime.turns;
    if valid {
        entry.runtime.spent = spent;
        entry.runtime.read = read;
    }
    if !valid {
        faulted(domain, env, owner, AgentFailure::Rules, out);
        return;
    }
    finish_run(domain, env, owner, finish, out);
}

#[cfg(test)]
impl Hosted {
    pub(crate) fn grants(&self) -> &[Grant] {
        &self.grants
    }
}

fn agent_grant(form: crate::Form) -> bool {
    match form.table() {
        GrantTable::Accounts | GrantTable::Name => true,
        GrantTable::Application => false,
    }
}
