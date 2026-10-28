//! Durable proposals owned by live proposers (domain/tasks.md, section 9).
//! The root decides authority, funding and holder coverage. This child keeps
//! one pending proposal per live task, its action and current recipient; it
//! never knows role membership or a worker call identity. Terminal proposals
//! leave live memory as historical store rows.
use crate::domain::{Domain, publish, record, refused, task_mut};
use crate::{Amendment, Authority, Change, Limits, New, Party, Phase, Refusal, Request, Stored, Was};
use alloc::boxed::Box;
use skein_lib::{Env, Queue, ReplyTo, Time, Wall};

fn kind(action: &ProposalAction) -> ProposalKind {
    match action {
        ProposalAction::Settings { .. } => ProposalKind::Settings,
        ProposalAction::Batch(_) => ProposalKind::Batch,
        ProposalAction::Effect { .. } | ProposalAction::ProcedureEffect { .. } => ProposalKind::Effect,
        ProposalAction::Amend { .. } => ProposalKind::Amend,
        ProposalAction::Widen { .. } => ProposalKind::Widen,
        ProposalAction::Release { .. } | ProposalAction::ReleasePlaced { .. } => ProposalKind::Release,
    }
}

fn word(proposal: &Proposal, at: Wall) -> crate::Word {
    crate::Word {
        number: proposal.number,
        from: Party::Task(proposal.proposer),
        kind: crate::MessageKind::Proposal {
            proposer: proposal.proposer,
            proposal: proposal.number,
            kind: kind(&proposal.action),
        },
        words: if proposal.reason.is_empty() { Box::from(&b"Proposal"[..]) } else { proposal.reason.clone() },
        at,
        hits: 1,
        eligible: true,
    }
}

/// The holder's decision entry is projected from its proposer's durable row.
/// It never occupies or consumes an inbox reservation.
pub(crate) fn waiting_for(domain: &Domain, holder: u64) -> Box<[crate::Word]> {
    let mut waiting = skein_lib::List::with_capacity(
        domain.names.len().checked_add(domain.person_proposals.len()).expect("bounded proposal owners"),
    );
    for (number, _) in &domain.names {
        let task = record(domain, *number).expect("indexed proposal owner");
        if let Some(proposal) = &task.proposal {
            match proposal.state {
                ProposalState::Pending { holder: ProposalHolder::Task(target), since } if target == holder => {
                    waiting.push(word(proposal, since)).expect("one proposal per live task");
                }
                ProposalState::Pending { .. }
                | ProposalState::Accepted { .. }
                | ProposalState::Rejected { .. }
                | ProposalState::Withdrawn => {}
            }
        }
    }
    for (_, proposal) in &domain.person_proposals {
        if domain.person_proposal_owner(proposal) == Some(crate::Owner::Task { task: holder })
            && let Some(since) = proposal.state.pending_since()
        {
            waiting.push(person_word(proposal, since)).expect("bounded party and task proposals");
        }
    }
    waiting.into_boxed()
}

fn wake_holder(domain: &mut Domain, env: &Env<Limits>, proposal: &Proposal, out: &mut Queue<Request>) {
    let (task, since) = match proposal.state {
        ProposalState::Pending { holder: ProposalHolder::Task(task), since } => (task, since),
        ProposalState::Pending { .. }
        | ProposalState::Accepted { .. }
        | ProposalState::Rejected { .. }
        | ProposalState::Withdrawn => return,
    };
    if record(domain, task).is_none() {
        out.push(Request::ProposalStalled {
            proposer: proposal.proposer,
            proposal: proposal.number,
            holder: ProposalHolder::Task(task),
        });
        return;
    }
    crate::wake::after_message(domain, env, task, None, word(proposal, since), out);
}

pub(crate) fn person_word(proposal: &PersonProposal, at: Wall) -> crate::Word {
    crate::Word {
        number: proposal.number,
        from: Party::Person(proposal.proposer),
        kind: crate::MessageKind::Proposal {
            proposer: proposal.proposer,
            proposal: proposal.number,
            kind: ProposalKind::Release,
        },
        words: proposal.reason.clone(),
        at,
        hits: 1,
        eligible: true,
    }
}

/// A task action waiting for a holder's authority and funding.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[expect(
    clippy::large_enum_variant,
    reason = "a proposed amendment stays inline, as `Event::Amend` carries it, and is priced by size_of"
)]
pub enum ProposalAction {
    /// Settings values stay in the core (engine.md, 17.6).
    Settings { against: u32, items: Box<[SettingHead]> },
    /// Whole batch with IDs allocated on proposal admission, before reservation.
    Batch(Box<[New]>),
    /// Connector-owned payload retained under this proposal's number. Only
    /// the needed authority and original call purpose cross the child boundary.
    Effect { connector: u16, authority: Authority, attempt: u64, completion: u32, position: u32, purpose: u64 },
    /// A procedure-origin effect keeps its genuine stable purpose and step.
    ProcedureEffect { connector: u16, authority: Authority, step: u64, purpose: u64 },
    /// Change an existing live task after its holder accepts the needed authority.
    Amend { task: u64, revision: u64, amendment: Amendment },
    /// Widen a task's current authority.
    Widen { task: u64, authority: Authority },
    /// Release a held task whose decision needs the holder.
    Release { task: u64 },
    /// Release from a placed hold, naming what the proposer saw.
    ReleasePlaced { task: u64, hold: u64, revision: u64 },
}

/// Current decision recipient, selected by root from the nearest covering holder.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ProposalHolder {
    /// Ancestor task.
    Task(u64),
    /// Person requester above the root.
    Person(u64),
    /// People whose project role permits this kind of decision; there is no further holder.
    Policy { project: u32, kind: ProposalKind },
}

/// Decision family used for the final policy recipient.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ProposalKind {
    Settings,
    Batch,
    Effect,
    Amend,
    Widen,
    Release,
}

/// One proposal's current or historical decision state.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ProposalState {
    /// No holder has decided yet.
    Pending { holder: ProposalHolder, since: Wall },
    /// Accepted by a covered holder in the same commit as its action.
    Accepted { by: Party },
    /// Rejected with words delivered to the proposer.
    Rejected { by: Party, reason: Box<[u8]> },
    /// Proposer withdrew it or its live basis closed.
    Withdrawn,
}

/// Root-numbered proposal whose action and reason remain durable.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Proposal {
    pub number: u64,
    pub proposer: u64,
    pub project: u32,
    pub action: ProposalAction,
    pub reason: Box<[u8]>,
    pub as_holder: bool,
    pub state: ProposalState,
}

/// A party's explicit action request, kept separately from task-origin proposals.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct PersonProposal {
    pub number: u64,
    pub proposer: u64,
    pub project: u32,
    pub action: PersonAction,
    pub reason: Box<[u8]>,
    pub state: PersonProposalState,
}

/// A party settings proposal has no task for its inbox link or goal ancestry.
pub const PROPOSAL_WITHOUT_TASK: u64 = 0;

impl PersonProposal {
    /// Proposed roots in their durable order; empty for amendments, releases and settings.
    #[must_use]
    pub fn batch(&self) -> &[New] {
        match &self.action {
            PersonAction::Batch(batch) => batch,
            PersonAction::Amend { .. } | PersonAction::Settings { .. } | PersonAction::ReleasePlaced { .. } => &[],
        }
    }
    /// The amended or released task, or first root, for the inbox link; zero when there is no task.
    #[must_use]
    pub fn first(&self) -> u64 {
        match &self.action {
            PersonAction::Batch(batch) => match batch.first() {
                Some(member) => member.number,
                None => PROPOSAL_WITHOUT_TASK,
            },
            PersonAction::Amend { task, .. } | PersonAction::ReleasePlaced { task, .. } => *task,
            PersonAction::Settings { .. } => PROPOSAL_WITHOUT_TASK,
        }
    }
}

/// A party's requested action. Task actions use the accepter's funding;
/// deployment settings carry no funding (tasks.md, section 9).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum PersonAction {
    ReleasePlaced { task: u64, hold: u64, revision: u64 },
    Settings { against: u32, items: Box<[SettingHead]> },
    Batch(Box<[New]>),
    Amend { task: u64, revision: u64, amendment: Box<Amendment> },
}

/// The tasks child retains only settings item heads (engine.md, 17.6).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Setting {
    Charter(u32),
    Template(u32),
    Policy(u32),
    Rules,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SettingEdit {
    Set,
    Removed,
    Reset,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SettingHead {
    pub setting: Setting,
    pub edit: SettingEdit,
}
pub const SETTINGS_ITEMS: u32 = 128;

impl PersonAction {
    #[must_use]
    pub const fn is_settings(&self) -> bool {
        match self {
            Self::Settings { .. } => true,
            Self::Batch(_) | Self::Amend { .. } | Self::ReleasePlaced { .. } => false,
        }
    }
}
/// Durable decision on a person-origin proposal; terminal rows remain in the store.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum PersonProposalState {
    SettingsAccepted { by: Party, revision: u32 },
    Pending { since: Wall },
    Accepted { by: Party },
    Rejected { by: Party, reason: Box<[u8]>, message: u64, at: Wall },
    Withdrawn { at: Wall },
}

impl PersonProposalState {
    #[must_use]
    pub const fn pending_since(&self) -> Option<Wall> {
        match self {
            Self::Pending { since } => Some(*since),
            Self::SettingsAccepted { .. } | Self::Accepted { .. } | Self::Rejected { .. } | Self::Withdrawn { .. } => {
                None
            }
        }
    }
}
/// A restored or newly proposed action has its bounded durable shape.
pub(crate) fn valid_person_proposal(proposal: &PersonProposal, limits: &Limits) -> bool {
    if proposal.number == 0
        || proposal.proposer == 0
        || proposal.reason.len() > usize::try_from(limits.message_bytes).expect("u32 fits usize")
        || !crate::person_proposals::valid_action(proposal, limits)
    {
        return false;
    }
    let release = match proposal.action {
        PersonAction::ReleasePlaced { .. } => true,
        PersonAction::Settings { .. } | PersonAction::Batch(_) | PersonAction::Amend { .. } => false,
    };
    match &proposal.state {
        PersonProposalState::Pending { .. } | PersonProposalState::Withdrawn { .. } => true,
        PersonProposalState::SettingsAccepted { by, revision } => {
            let against = match proposal.action {
                PersonAction::Settings { against, .. } => against,
                PersonAction::Batch(_) | PersonAction::Amend { .. } | PersonAction::ReleasePlaced { .. } => {
                    return false;
                }
            };
            *revision > against
                && match by {
                    Party::Person(person) => *person != 0,
                    Party::Task(_) | Party::Deployment { .. } => false,
                }
        }
        PersonProposalState::Accepted { by } => match by {
            Party::Person(person) => !proposal.action.is_settings() && *person != 0,
            Party::Task(task) => *task != 0 && release,
            Party::Deployment { .. } => false,
        },
        PersonProposalState::Rejected { by, reason, message, .. } => match by {
            Party::Person(person) => {
                *person != 0
                    && *message != 0
                    && reason.len() <= usize::try_from(limits.message_bytes).expect("u32 fits usize")
            }
            Party::Task(task) => {
                *task != 0
                    && *message != 0
                    && reason.len() <= usize::try_from(limits.message_bytes).expect("message bound")
                    && release
            }
            Party::Deployment { .. } => false,
        },
    }
}

/// Admit one policy-bound party proposal without creating its roots.
pub(crate) fn propose_person(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    proposal: PersonProposal,
    out: &mut Queue<Request>,
) {
    if !domain.ready() {
        return refused(to, None, Refusal::NotReady, out);
    }
    if !valid_person_proposal(&proposal, &env.limits)
        || match proposal.state {
            PersonProposalState::Pending { .. } => false,
            PersonProposalState::Accepted { .. }
            | PersonProposalState::SettingsAccepted { .. }
            | PersonProposalState::Rejected { .. }
            | PersonProposalState::Withdrawn { .. } => true,
        }
    {
        return refused(to, None, Refusal::Read, out);
    }
    if let Err(problem) = crate::person_proposals::check(domain, env, &proposal) {
        return out.push(Request::Refused { reply_to: to, problem: *problem });
    }
    if domain.person_proposals.len() >= env.limits.tasks || domain.person_proposals.contains_key(&proposal.number) {
        if domain.person_proposals.len() >= env.limits.tasks && !domain.person_proposals.contains_key(&proposal.number)
        {
            return crate::capacity::refuse(
                domain,
                env,
                to,
                None,
                Refusal::Busy,
                crate::Capacity {
                    project: Some(proposal.project),
                    by: Party::Person(proposal.proposer),
                    action: crate::CapacityAction::PersonProposal { number: proposal.number },
                    cause: crate::CapacityCause::Proposals { limit: env.limits.tasks },
                },
                out,
            );
        }
        return refused(to, None, Refusal::Busy, out);
    }
    for (_, old) in &domain.person_proposals {
        if old.proposer == proposal.proposer {
            return refused(to, None, Refusal::Busy, out);
        }
    }
    let number = proposal.number;
    let saved = domain.person_proposals.insert(number, proposal.clone());
    assert!(saved == Ok(None), "admitted person proposal has room");
    if let Some(crate::Owner::Task { task }) = domain.person_proposal_owner(&proposal) {
        crate::wake::after_message(domain, env, task, None, person_word(&proposal, env.wall), out);
    }
    out.push(Request::Save { record: Stored::PersonProposal(Box::new(proposal)) });
    out.push(Request::PersonProposed { reply_to: to, proposal: number });
}

/// Finish one exact policy decision after root has made the accepted batch.
#[expect(clippy::too_many_arguments, reason = "one exact person proposal decision and its output")]
pub(crate) fn decide_person(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    proposer: u64,
    number: u64,
    by: Party,
    message: Option<u64>,
    decision: ProposalDecision,
    revision: Option<u32>,
    out: &mut Queue<Request>,
) {
    if !domain.ready() {
        return refused(to, None, Refusal::NotReady, out);
    }
    let Some(old) = domain.person_proposals.get(&number) else {
        return out.push(Request::PersonProposalDecided {
            reply_to: Some(to),
            proposer,
            number,
            outcome: ProposalOutcome::Stale,
        });
    };
    if old.proposer != proposer {
        return refused(to, None, Refusal::Unknown, out);
    }
    if match old.state {
        PersonProposalState::Pending { .. } => false,
        PersonProposalState::Accepted { .. }
        | PersonProposalState::SettingsAccepted { .. }
        | PersonProposalState::Rejected { .. }
        | PersonProposalState::Withdrawn { .. } => true,
    } {
        return out.push(Request::PersonProposalDecided {
            reply_to: Some(to),
            proposer,
            number,
            outcome: ProposalOutcome::Stale,
        });
    }
    if decision != ProposalDecision::Accept && revision.is_some() {
        return refused(to, None, Refusal::Read, out);
    }
    let outcome = match &decision {
        ProposalDecision::Accept => {
            if !crate::person_proposals::valid_revision(&old.action, revision)
                || !crate::person_proposals::made(domain, &old.action)
            {
                return refused(to, None, Refusal::State, out);
            }
            ProposalOutcome::Accepted
        }
        ProposalDecision::Reject { reason } => {
            if reason.len() > usize::try_from(env.limits.message_bytes).expect("u32 fits usize")
                || match message {
                    Some(number) => number == 0,
                    None => true,
                }
            {
                return refused(to, None, Refusal::Read, out);
            }
            ProposalOutcome::Rejected
        }
        ProposalDecision::Pass { .. } => return refused(to, None, Refusal::State, out),
    };
    let release = if decision == ProposalDecision::Accept {
        match crate::person_proposals::release_plan(domain, env, old, by) {
            Ok(plan) => plan,
            Err(why) => return refused(to, None, why, out),
        }
    } else {
        None
    };
    let mut proposal = domain.person_proposals.remove(&number).expect("checked pending proposal");
    if let Some(plan) = release {
        crate::control::apply_release(domain, env, by, plan, out);
    }
    let settings = match proposal.action {
        PersonAction::Settings { .. } => true,
        PersonAction::Batch(_) | PersonAction::Amend { .. } | PersonAction::ReleasePlaced { .. } => false,
    };
    proposal.state = match decision {
        ProposalDecision::Accept => match revision {
            Some(revision) => PersonProposalState::SettingsAccepted { by, revision },
            None => PersonProposalState::Accepted { by },
        },
        ProposalDecision::Reject { reason } => PersonProposalState::Rejected {
            by,
            reason,
            message: message.expect("checked rejection message"),
            at: env.wall,
        },
        ProposalDecision::Pass { .. } => unreachable!("person proposal has final policy holder"),
    };
    out.push(Request::Erase { key: crate::Key::PersonProposal(number) });
    out.push(Request::Save {
        record: Stored::Update(crate::Update {
            party: proposal.proposer,
            at: env.wall,
            task: proposal.first(),
            object: crate::UpdateObject::Proposal { proposal: crate::Newest::of(proposal.number) },
        }),
    });
    out.push(Request::Save { record: Stored::DecidedProposal(Box::new(proposal)) });
    if !settings {
        crate::closing::prune_stubs(domain, env, out);
    }
    out.push(Request::PersonProposalDecided { reply_to: Some(to), proposer, number, outcome });
}

/// Root-authenticated decision for the current holder.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ProposalDecision {
    Accept,
    Reject { reason: Box<[u8]> },
    Pass { holder: ProposalHolder },
}

/// Child terminal for an exact proposal decision.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ProposalOutcome {
    Accepted,
    Rejected,
    Passed,
    Withdrawn,
    Stale,
}

/// Borrowed current proposal clone for root's short-lived authority decision.
#[must_use]
pub fn context(domain: &Domain, proposer: u64, number: u64) -> Option<Proposal> {
    let task = record(domain, proposer)?;
    match &task.proposal {
        Some(proposal) if proposal.number == number => Some((**proposal).clone()),
        Some(_) | None => None,
    }
}

fn arm(domain: &mut Domain, env: &Env<Limits>, proposer: u64, since: Wall) {
    let until = since.as_nanos().saturating_add(env.limits.proposal_stall.as_nanos());
    let remaining = until.saturating_sub(env.wall.as_nanos());
    let due = Time::from_nanos(env.now.as_nanos().saturating_add(remaining));
    assert!(domain.proposal_alarms.arm(proposer, due).is_ok(), "one pending proposal per task");
}

/// Reproject durable pending waits from wall time after complete live restore.
pub(crate) fn rearm_all(domain: &mut Domain, env: &Env<Limits>) {
    let mut pending = skein_lib::List::with_capacity(env.limits.tasks);
    for (number, _) in &domain.names {
        let task = record(domain, *number).expect("live proposal owner");
        if let Some(proposal) = &task.proposal {
            match proposal.state {
                ProposalState::Pending { holder: ProposalHolder::Task(_) | ProposalHolder::Person(_), since } => {
                    pending.push((*number, since)).expect("pending tasks bounded");
                }
                ProposalState::Pending { holder: ProposalHolder::Policy { .. }, .. }
                | ProposalState::Accepted { .. }
                | ProposalState::Rejected { .. }
                | ProposalState::Withdrawn => {}
            }
        }
    }
    for &(number, since) in &pending {
        arm(domain, env, number, since);
    }
}

/// Once restoration has rearmed deadlines, make every durable task-holder
/// entry visible again to an idle or adopted run.
pub(crate) fn wake_restored(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    let mut waiting = skein_lib::List::with_capacity(env.limits.tasks);
    for (number, _) in &domain.names {
        let task = record(domain, *number).expect("restored name live");
        if let Some(proposal) = &task.proposal {
            match proposal.state {
                ProposalState::Pending { holder: ProposalHolder::Task(_), .. } => {
                    waiting.push((*number, proposal.number)).expect("one pending proposal per task");
                }
                ProposalState::Pending { .. }
                | ProposalState::Accepted { .. }
                | ProposalState::Rejected { .. }
                | ProposalState::Withdrawn => {}
            }
        }
    }
    for &(proposer, number) in &waiting {
        let proposal = context(domain, proposer, number).expect("restored pending proposal");
        wake_holder(domain, env, &proposal, out);
    }
}

/// A holder that becomes held or closing cannot decide its waiting entries.
pub(crate) fn holder_unavailable(domain: &Domain, holder: u64, out: &mut Queue<Request>) {
    for (proposer, _) in &domain.names {
        let task = record(domain, *proposer).expect("indexed proposal owner");
        if let Some(proposal) = &task.proposal {
            match proposal.state {
                ProposalState::Pending { holder: ProposalHolder::Task(target), .. } if target == holder => {
                    out.push(Request::ProposalStalled {
                        proposer: *proposer,
                        proposal: proposal.number,
                        holder: ProposalHolder::Task(holder),
                    });
                }
                ProposalState::Pending { .. }
                | ProposalState::Accepted { .. }
                | ProposalState::Rejected { .. }
                | ProposalState::Withdrawn => {}
            }
        }
    }
}

/// Expire one configured nonfinal wait; root computes fresh coverage before
/// returning `StalledProposal` in the same decision.
pub(crate) fn fire(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    let Some(proposer) = domain.proposal_alarms.expire(env.now) else { return };
    let Some(task) = record(domain, proposer) else { return };
    let Some(proposal) = &task.proposal else { return };
    match proposal.state {
        ProposalState::Pending { holder: holder @ (ProposalHolder::Task(_) | ProposalHolder::Person(_)), .. } => {
            out.push(Request::ProposalStalled { proposer, proposal: proposal.number, holder });
        }
        ProposalState::Pending { holder: ProposalHolder::Policy { .. }, .. }
        | ProposalState::Accepted { .. }
        | ProposalState::Rejected { .. }
        | ProposalState::Withdrawn => {}
    }
}

/// Apply root's fresh next-holder choice only if the exact old proposal still waits.
#[expect(clippy::too_many_arguments, reason = "one reroute carries its old holder, revision and replacement")]
pub(crate) fn stalled(
    domain: &mut Domain,
    env: &Env<Limits>,
    proposer: u64,
    number: u64,
    from: ProposalHolder,
    revision: u64,
    holder: ProposalHolder,
    out: &mut Queue<Request>,
) {
    let Some(mut proposal) = context(domain, proposer, number) else { return };
    let current = match proposal.state {
        ProposalState::Pending { holder, .. } => holder,
        ProposalState::Accepted { .. } | ProposalState::Rejected { .. } | ProposalState::Withdrawn => return,
    };
    let task_revision = record(domain, proposer).expect("live proposal").revision;
    if current != from || task_revision != revision || current == holder || task_revision == u64::MAX {
        return;
    }
    proposal.state = ProposalState::Pending { holder, since: env.wall };
    task_mut(domain, proposer).expect("live proposal").record.proposal = Some(Box::new(proposal.clone()));
    crate::positions::history(domain, env, proposal, Party::Task(proposer), Change::ProposalPassed, out);
    publish(domain, env, proposer, out);
    match holder {
        ProposalHolder::Task(_) | ProposalHolder::Person(_) => arm(domain, env, proposer, env.wall),
        ProposalHolder::Policy { .. } => domain.proposal_alarms.cancel(proposer),
    }
    let pending = context(domain, proposer, number).expect("rerouted proposal live");
    wake_holder(domain, env, &pending, out);
}

/// Admit a checked action once for a live proposer, with no source reservation.
pub(crate) fn propose(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    proposal: Proposal,
    out: &mut Queue<Request>,
) {
    if let Some(problem) = admission_problem(domain, &env.limits, &proposal) {
        return crate::capacity::refused(domain, env, to, problem, out);
    }
    install(domain, env, proposal, false, out);
    out.push(Request::Done { reply_to: to });
}

/// Shared admission for standalone proposals and compound procedure steps.
pub(crate) fn admission_problem(domain: &Domain, limits: &Limits, proposal: &Proposal) -> Option<crate::Problem> {
    if proposal.number == 0
        || proposal.proposer == 0
        || proposal.reason.len() > usize::try_from(limits.message_bytes).expect("u32 fits usize")
    {
        return Some(proposal_problem(proposal.proposer, Refusal::Read));
    }
    let Some(task) = record(domain, proposal.proposer) else {
        return Some(proposal_problem(proposal.proposer, Refusal::Unknown));
    };
    let same_scope = proposal_scope(task.project, proposal);
    if !same_scope || task.proposal.is_some() || task.revision == u64::MAX {
        if same_scope
            && task.revision != u64::MAX
            && let Some(prior) = &task.proposal
        {
            match prior.state {
                ProposalState::Pending { .. } => {
                    return Some(crate::Problem::capacity(
                        Some(proposal.proposer),
                        Refusal::Busy,
                        crate::Capacity {
                            project: Some(proposal.project),
                            by: Party::Task(proposal.proposer),
                            action: crate::CapacityAction::Proposal { number: proposal.number },
                            cause: crate::CapacityCause::Proposals { limit: 1 },
                        },
                    ));
                }
                ProposalState::Accepted { .. } | ProposalState::Rejected { .. } | ProposalState::Withdrawn => {}
            }
        }
        return Some(proposal_problem(proposal.proposer, Refusal::Busy));
    }
    if !crate::inbox::room(
        domain,
        limits,
        proposal.proposer,
        1,
        usize::try_from(limits.message_bytes).expect("u32 fits usize"),
    ) {
        return Some(crate::Problem::capacity(
            Some(proposal.proposer),
            Refusal::Busy,
            crate::Capacity {
                project: Some(proposal.project),
                by: Party::Task(proposal.proposer),
                action: crate::CapacityAction::Proposal { number: proposal.number },
                cause: crate::capacity::inbox(proposal.proposer, limits),
            },
        ));
    }
    match task.phase {
        Phase::Waiting | Phase::Active(_) | Phase::Held { was: Was::Waiting | Was::Active(_), .. } => {}
        Phase::Closing(_) | Phase::Held { was: Was::Closing(_), .. } | Phase::Ended(_) => {
            return Some(proposal_problem(proposal.proposer, Refusal::State));
        }
    }
    match proposal.state {
        ProposalState::Pending { .. } => {}
        ProposalState::Accepted { .. } | ProposalState::Rejected { .. } | ProposalState::Withdrawn => {
            return Some(proposal_problem(proposal.proposer, Refusal::State));
        }
    }
    match &proposal.action {
        ProposalAction::ReleasePlaced { task: target, hold, revision } => {
            let Some(placed) = domain.placed_hold(*hold) else {
                return Some(proposal_problem(proposal.proposer, Refusal::Unknown));
            };
            if placed.lifted || placed.revision != *revision || record(domain, *target).is_none() {
                return Some(proposal_problem(proposal.proposer, Refusal::HoldRevision { revision: placed.revision }));
            }
        }
        ProposalAction::Batch(_)
        | ProposalAction::Effect { .. }
        | ProposalAction::ProcedureEffect { .. }
        | ProposalAction::Amend { .. }
        | ProposalAction::Widen { .. }
        | ProposalAction::Release { .. }
        | ProposalAction::Settings { .. } => {}
    }
    None
}

fn proposal_problem(task: u64, why: Refusal) -> crate::Problem {
    crate::Problem { task: Some(task), why, blocked_by: None, capacity: None }
}

/// Check the one proposal a verdict creates before any result expense is posted.
pub(crate) fn check_result(domain: &Domain, env: &Env<Limits>, task: u64, proposal: &Proposal) -> bool {
    if proposal.number == 0
        || proposal.proposer != task
        || proposal.as_holder
        || proposal.reason.len() > usize::try_from(env.limits.message_bytes).expect("u32 fits usize")
    {
        return false;
    }
    let Some(row) = record(domain, task) else { return false };
    if row.project != proposal.project || row.proposal.is_some() || row.revision == u64::MAX {
        return false;
    }
    if !crate::inbox::room(
        domain,
        &env.limits,
        task,
        1,
        usize::try_from(env.limits.message_bytes).expect("u32 fits usize"),
    ) {
        return false;
    }
    match (&row.phase, &proposal.action, &proposal.state) {
        (Phase::Active(_), ProposalAction::Batch(batch), ProposalState::Pending { holder, .. }) => {
            let holder_valid = match holder {
                ProposalHolder::Task(number) => *number != 0 && *number != task,
                ProposalHolder::Person(number) => *number != 0,
                ProposalHolder::Policy { project, kind } => *project == row.project && *kind == ProposalKind::Batch,
            };
            if !holder_valid
                || batch.is_empty()
                || batch.len() > usize::try_from(env.limits.batch).expect("u32 fits usize")
            {
                return false;
            }
            for (at, member) in batch.iter().enumerate() {
                if member.number == 0
                    || member.project != row.project
                    || member.funder != crate::Funder::Task(task)
                    || !crate::batch::valid_spec(&env.limits, &member.spec)
                    || !member.spec.inputs.is_empty()
                    || !crate::batch::valid_contract(&env.limits, &member.contract)
                    || !crate::batch::valid_authority(&env.limits, &member.authority)
                    || !crate::wake::valid(&member.wake)
                    || member.dependencies.len() > usize::try_from(env.limits.dependencies).expect("u32 fits usize")
                {
                    return false;
                }
                for earlier in batch.iter().take(at) {
                    if earlier.number == member.number {
                        return false;
                    }
                }
            }
            true
        }
        (Phase::Waiting | Phase::Closing(_) | Phase::Held { .. } | Phase::Ended(_), _, _)
        | (
            Phase::Active(_),
            ProposalAction::Settings { .. }
            | ProposalAction::Effect { .. }
            | ProposalAction::ProcedureEffect { .. }
            | ProposalAction::Amend { .. }
            | ProposalAction::Widen { .. }
            | ProposalAction::Release { .. }
            | ProposalAction::ReleasePlaced { .. },
            _,
        )
        | (
            Phase::Active(_),
            _,
            ProposalState::Accepted { .. } | ProposalState::Rejected { .. } | ProposalState::Withdrawn,
        ) => false,
    }
}

/// Install a preflighted result proposal in the same decision that takes its verdict.
pub(crate) fn propose_result(domain: &mut Domain, env: &Env<Limits>, proposal: Proposal, out: &mut Queue<Request>) {
    install(domain, env, proposal, true, out);
}

fn install(domain: &mut Domain, env: &Env<Limits>, proposal: Proposal, result: bool, out: &mut Queue<Request>) {
    let number = proposal.proposer;
    let proposal_id = proposal.number;
    let row = &mut task_mut(domain, number).expect("proposer live").record;
    row.proposal = Some(Box::new(proposal.clone()));
    row.result_proposal = result;
    crate::positions::history(domain, env, proposal, Party::Task(number), Change::Proposed, out);
    publish(domain, env, number, out);
    let pending = context(domain, number, proposal_id).expect("proposal admitted");
    match pending.state {
        ProposalState::Pending { holder: ProposalHolder::Task(_) | ProposalHolder::Person(_), since } => {
            arm(domain, env, number, since);
        }
        ProposalState::Pending { holder: ProposalHolder::Policy { .. }, .. } => {}
        ProposalState::Accepted { .. } | ProposalState::Rejected { .. } | ProposalState::Withdrawn => {
            unreachable!("pending admission")
        }
    }
    wake_holder(domain, env, &pending, out);
}

/// Advance the checked holder or finish the decision. Root executes an accepted
/// action before calling this entry in the same synchronous decision.
#[expect(clippy::too_many_arguments, reason = "one exact proposal decision carries its caller and message")]
#[expect(clippy::too_many_lines, reason = "one proposal terminal preflights, persists, and notifies atomically")]
pub(crate) fn decide(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    proposer: u64,
    number: u64,
    message: Option<u64>,
    by: Party,
    decision: ProposalDecision,
    out: &mut Queue<Request>,
) {
    let Some(mut proposal) = context(domain, proposer, number) else {
        return out.push(Request::ProposalDecided { reply_to: to, proposer, number, outcome: ProposalOutcome::Stale });
    };
    if !holder_is(proposal.state.clone(), by) {
        return refused(to, Some(proposer), Refusal::Reference, out);
    }
    if record(domain, proposer).expect("pending proposer live").revision == u64::MAX {
        return refused(to, Some(proposer), Refusal::State, out);
    }
    let last_message = record(domain, proposer).expect("pending proposer live").last_message;
    match (&decision, message) {
        (ProposalDecision::Accept | ProposalDecision::Reject { .. }, Some(number)) if number > last_message => {}
        (ProposalDecision::Pass { .. }, None) => {}
        (ProposalDecision::Accept | ProposalDecision::Reject { .. } | ProposalDecision::Pass { .. }, _) => {
            return refused(to, Some(proposer), Refusal::Read, out);
        }
    }
    if decision == ProposalDecision::Accept {
        match release_plan(domain, env, &proposal, by) {
            Ok(Some(plan)) => {
                // Keep the accepted proposal in this step while the lift withdraws its siblings.
                drop(task_mut(domain, proposer).expect("pending proposer").record.proposal.take());
                crate::control::apply_release(domain, env, by, plan, out);
            }
            Ok(None) => {}
            Err(why) => return refused(to, Some(proposer), why, out),
        }
    }
    let words = match &decision {
        ProposalDecision::Reject { reason } => reason.clone(),
        ProposalDecision::Accept | ProposalDecision::Pass { .. } => Box::new([]),
    };
    let outcome = match decision {
        ProposalDecision::Accept => {
            proposal.state = ProposalState::Accepted { by };
            ProposalOutcome::Accepted
        }
        ProposalDecision::Reject { reason } => {
            if reason.len() > usize::try_from(env.limits.message_bytes).expect("u32 fits usize") {
                return refused(to, Some(proposer), Refusal::Read, out);
            }
            proposal.state = ProposalState::Rejected { by, reason };
            ProposalOutcome::Rejected
        }
        ProposalDecision::Pass { holder } => {
            proposal.state = ProposalState::Pending { holder, since: env.wall };
            ProposalOutcome::Passed
        }
    };
    let task = task_mut(domain, proposer).expect("pending proposer live");
    task.record.proposal = match outcome {
        ProposalOutcome::Passed => Some(Box::new(proposal.clone())),
        ProposalOutcome::Accepted | ProposalOutcome::Rejected | ProposalOutcome::Withdrawn | ProposalOutcome::Stale => {
            None
        }
    };
    if match outcome {
        ProposalOutcome::Accepted | ProposalOutcome::Rejected => true,
        ProposalOutcome::Passed | ProposalOutcome::Withdrawn | ProposalOutcome::Stale => false,
    } {
        task.record.result_proposal = false;
    }
    let change = match outcome {
        ProposalOutcome::Accepted => Change::ProposalAccepted,
        ProposalOutcome::Rejected => Change::ProposalRejected,
        ProposalOutcome::Passed => Change::ProposalPassed,
        ProposalOutcome::Withdrawn | ProposalOutcome::Stale => unreachable!("decision outcome"),
    };
    crate::positions::history(domain, env, proposal, by, change, out);
    publish(domain, env, proposer, out);
    match outcome {
        ProposalOutcome::Passed => {
            let pending = record(domain, proposer).expect("passed proposer live").proposal.as_ref().expect("pending");
            match pending.state {
                ProposalState::Pending { holder: ProposalHolder::Task(_) | ProposalHolder::Person(_), since } => {
                    arm(domain, env, proposer, since);
                }
                ProposalState::Pending { holder: ProposalHolder::Policy { .. }, .. } => {
                    domain.proposal_alarms.cancel(proposer);
                }
                ProposalState::Accepted { .. } | ProposalState::Rejected { .. } | ProposalState::Withdrawn => {
                    unreachable!("passed pending")
                }
            }
        }
        ProposalOutcome::Accepted | ProposalOutcome::Rejected | ProposalOutcome::Withdrawn | ProposalOutcome::Stale => {
            domain.proposal_alarms.cancel(proposer);
        }
    }
    match outcome {
        ProposalOutcome::Accepted | ProposalOutcome::Rejected => {
            let message = message.expect("terminal message preflighted");
            crate::inbox::proposal_decision(
                domain,
                env,
                proposer,
                crate::Word {
                    number: message,
                    from: by,
                    kind: crate::MessageKind::ProposalDecision {
                        proposal: number,
                        accepted: outcome == ProposalOutcome::Accepted,
                    },
                    words,
                    at: env.wall,
                    hits: 1,
                    eligible: true,
                },
                out,
            );
        }
        ProposalOutcome::Passed | ProposalOutcome::Withdrawn | ProposalOutcome::Stale => {}
    }
    if outcome == ProposalOutcome::Passed {
        let pending = context(domain, proposer, number).expect("passed proposal live");
        wake_holder(domain, env, &pending, out);
    }
    out.push(Request::ProposalDecided { reply_to: to, proposer, number, outcome });
}

fn holder_is(state: ProposalState, by: Party) -> bool {
    match state {
        ProposalState::Pending { holder: ProposalHolder::Task(task), .. } => by == Party::Task(task),
        ProposalState::Pending { holder: ProposalHolder::Person(person), .. } => by == Party::Person(person),
        ProposalState::Pending { holder: ProposalHolder::Policy { .. }, .. } => match by {
            Party::Person(_) => true,
            Party::Task(_) | Party::Deployment { .. } => false,
        },
        ProposalState::Accepted { .. } | ProposalState::Rejected { .. } | ProposalState::Withdrawn => false,
    }
}

/// Closing withdraws other pending proposals; a result proposal survives until decision.
pub(crate) fn withdraw_on_close(domain: &mut Domain, env: &Env<Limits>, number: u64, out: &mut Queue<Request>) {
    let Some(task) = record(domain, number) else { return };
    let closing = match &task.phase {
        Phase::Closing(closing) | Phase::Held { was: Was::Closing(closing), .. } => match &closing.ending {
            crate::Ending::Cancelled { .. } => true,
            crate::Ending::Done(_) | crate::Ending::Failed { .. } => !task.result_proposal,
        },
        Phase::Waiting
        | Phase::Active(_)
        | Phase::Held { was: Was::Waiting | Was::Active(_), .. }
        | Phase::Ended(_) => false,
    };
    if !closing {
        return;
    }
    let mut proposal = match task_mut(domain, number).expect("closing task live").record.proposal.take() {
        Some(proposal) => *proposal,
        None => return,
    };
    task_mut(domain, number).expect("closing task live").record.result_proposal = false;
    proposal.state = ProposalState::Withdrawn;
    crate::positions::history(domain, env, proposal, Party::Task(number), Change::ProposalWithdrawn, out);
    domain.proposal_alarms.cancel(number);
}

/// The proposer explicitly withdraws one live pending action.
pub(crate) fn withdraw(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    proposer: u64,
    number: u64,
    out: &mut Queue<Request>,
) {
    let Some(mut proposal) = context(domain, proposer, number) else {
        return out.push(Request::ProposalDecided { reply_to: to, proposer, number, outcome: ProposalOutcome::Stale });
    };
    if record(domain, proposer).expect("pending proposer live").result_proposal {
        return refused(to, Some(proposer), Refusal::State, out);
    }
    if record(domain, proposer).expect("pending proposer live").revision == u64::MAX {
        return refused(to, Some(proposer), Refusal::State, out);
    }
    task_mut(domain, proposer).expect("pending proposer live").record.proposal = None;
    task_mut(domain, proposer).expect("pending proposer live").record.result_proposal = false;
    proposal.state = ProposalState::Withdrawn;
    crate::positions::history(domain, env, proposal, Party::Task(proposer), Change::ProposalWithdrawn, out);
    domain.proposal_alarms.cancel(proposer);
    publish(domain, env, proposer, out);
    out.push(Request::ProposalDecided { reply_to: to, proposer, number, outcome: ProposalOutcome::Withdrawn });
}

/// Settings wait in deployment scope from the start; ordinary actions stay in their task's project.
pub(crate) fn proposal_scope(project: u32, proposal: &Proposal) -> bool {
    match &proposal.action {
        ProposalAction::Settings { against, items } => {
            let owners = match proposal.state {
                ProposalState::Pending { holder, .. } => {
                    holder == ProposalHolder::Policy { project: 0, kind: ProposalKind::Settings }
                }
                ProposalState::Accepted { .. } | ProposalState::Rejected { .. } | ProposalState::Withdrawn => false,
            };
            proposal.project == 0
                && !proposal.as_holder
                && *against > 0
                && crate::person_proposals::valid_settings(items)
                && owners
        }
        ProposalAction::Batch(_)
        | ProposalAction::Effect { .. }
        | ProposalAction::ProcedureEffect { .. }
        | ProposalAction::Amend { .. }
        | ProposalAction::Widen { .. }
        | ProposalAction::Release { .. }
        | ProposalAction::ReleasePlaced { .. } => proposal.project == project,
    }
}

/// Settings decisions name the made revision, or the revision that made them stale.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SettingsOutcome {
    Accepted { revision: u32 },
    Rejected,
    Stale { newest: u32 },
}
/// Root-authenticated settings terminal; its message was reserved at proposal admission.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SettingsDecision {
    pub proposer: u64,
    pub proposal: u64,
    pub message: u64,
    pub by: Party,
    pub outcome: SettingsOutcome,
    pub reason: Box<[u8]>,
}

pub(crate) fn decide_settings(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: Option<ReplyTo>,
    decision: SettingsDecision,
    out: &mut Queue<Request>,
) {
    let SettingsDecision { proposer, proposal: number, message, by, outcome, reason } = decision;
    let mut proposal = context(domain, proposer, number).expect("root checked settings proposal");
    let against = match proposal.action {
        ProposalAction::Settings { against, .. } => against,
        ProposalAction::Batch(_)
        | ProposalAction::Effect { .. }
        | ProposalAction::ProcedureEffect { .. }
        | ProposalAction::Amend { .. }
        | ProposalAction::Widen { .. }
        | ProposalAction::Release { .. }
        | ProposalAction::ReleasePlaced { .. } => unreachable!("settings terminal has settings action"),
    };
    let row = record(domain, proposer).expect("settings proposer live");
    assert!(
        message > row.last_message && reason.len() <= usize::try_from(env.limits.message_bytes).expect("word bound"),
        "checked settings outcome"
    );
    let (terminal, change) = match outcome {
        SettingsOutcome::Accepted { revision } => {
            assert!(revision > against && holder_is(proposal.state.clone(), by), "checked acceptance");
            proposal.state = ProposalState::Accepted { by };
            (ProposalOutcome::Accepted, Change::ProposalAccepted)
        }
        SettingsOutcome::Rejected => {
            assert!(holder_is(proposal.state.clone(), by), "checked rejection");
            proposal.state = ProposalState::Rejected { by, reason: reason.clone() };
            (ProposalOutcome::Rejected, Change::ProposalRejected)
        }
        SettingsOutcome::Stale { newest } => {
            assert!(newest > against, "stale settings basis");
            proposal.state = ProposalState::Withdrawn;
            (ProposalOutcome::Withdrawn, Change::ProposalWithdrawn)
        }
    };
    task_mut(domain, proposer).expect("settings proposer live").record.proposal = None;
    crate::positions::history(domain, env, proposal, by, change, out);
    publish(domain, env, proposer, out);
    domain.proposal_alarms.cancel(proposer);
    crate::inbox::proposal_decision(
        domain,
        env,
        proposer,
        crate::Word {
            number: message,
            from: by,
            kind: crate::MessageKind::SettingsDecision { proposal: number, outcome },
            words: reason,
            at: env.wall,
            hits: 1,
            eligible: true,
        },
        out,
    );
    if let Some(reply_to) = to {
        out.push(Request::ProposalDecided { reply_to, proposer, number, outcome: terminal });
    }
}

impl Domain {
    /// A parent retaining settings values preflights the same child admission before keeping them.
    #[must_use]
    pub fn proposal_problem(&self, limits: &Limits, proposal: &Proposal) -> Option<crate::Problem> {
        admission_problem(self, limits, proposal)
    }
}

/// The accepted action and its decision are preflighted before either changes (domain/tasks.md, 5.9).
fn release_plan(
    domain: &Domain,
    env: &Env<Limits>,
    proposal: &Proposal,
    by: Party,
) -> Result<Option<crate::control::ReleasePlan>, Refusal> {
    let ProposalAction::ReleasePlaced { task, hold, revision } = proposal.action else { return Ok(None) };
    let current = domain.placed_hold(hold).ok_or(Refusal::Unknown)?;
    if current.revision != revision || current.lifted {
        return Err(Refusal::HoldRevision { revision: current.revision });
    }
    let target = record(domain, task).ok_or(Refusal::Unknown)?;
    if target.placed != Some(hold) {
        return Err(match target.placed {
            Some(hold) => Refusal::Placed { hold },
            None => Refusal::State,
        });
    }
    if !crate::control::allowed(domain, task, by, env.limits.tasks) {
        return Err(Refusal::Reference);
    }
    let plan = crate::control::check_release(domain, &crate::compound::Staged::new(0), env, task, by, None, None)?;
    // The release may also write the proposer's release history before its accepted history.
    let proposer = record(domain, proposal.proposer).expect("pending proposer");
    let mut released = task == proposal.proposer;
    if let Some(changes) = &plan.placed {
        for (number, replacement) in &changes.tasks {
            if *number == proposal.proposer
                && replacement.is_none()
                && crate::placed::placed_reason(&proposer.phase).is_some()
            {
                released = true;
            }
        }
    }
    if released && proposer.revision >= u64::MAX.saturating_sub(1) {
        return Err(Refusal::State);
    }
    Ok(Some(plan))
}
