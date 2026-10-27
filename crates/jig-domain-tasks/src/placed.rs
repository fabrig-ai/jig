//! Placed holds, their owners, subtree stops and lifting (domain/tasks.md, 5.9).
use crate::compound::Staged;
use crate::domain::{fact, publish, record, task_mut};
use crate::{
    Capacity, CapacityAction, CapacityCause, Domain, Fact, Hold, Key, Limits, Party, Phase, Problem, Refusal, Request,
    Stored,
};
use alloc::boxed::Box;
use skein_lib::{Env, List, Map, Queue, ReplyTo};

/// Who placed a hold, and so who lifts it (domain/tasks.md, 5.9).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Owner {
    Party { person: u64 },
    Task { task: u64 },
}
/// Stop cancels each live run in scope at once; drain lets them finish their turn (4b).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Mode {
    Stop,
    Drain,
}
/// A task's subtree, those made later included, or a whole project (4b).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Scope {
    Subtree { task: u64 },
    Project,
}
/// One placed hold, numbered by the root in the deployment, never reused.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct PlacedHold {
    pub number: u64,
    pub project: u32,
    pub owner: Owner,
    pub mode: Mode,
    pub scope: Scope,
    /// Its reason in words, at most `Limits::message_bytes`; a party's Stop gives none.
    pub reason: Box<[u8]>,
    /// When it was placed, the root's wall clock in milliseconds.
    pub placed_at: u64,
    /// Moves when the hold changes; a Lift and a proposal name it.
    pub revision: u64,
    /// Tasks released from it while it stands, at most `Limits::exemptions`.
    pub exempt: Box<[u64]>,
    /// Lifted: kept, ended, while a task it held is still held by it.
    pub lifted: bool,
}

impl Domain {
    /// Borrow a placed hold for root authorization and reads.
    #[must_use]
    pub fn placed_hold(&self, number: u64) -> Option<&PlacedHold> {
        self.placed.get(&number)
    }
    /// Borrow the next retained hold by number, without a snapshot allocation.
    #[must_use]
    pub fn next_placed_hold(&self, after: u64) -> Option<&PlacedHold> {
        let mut next = None;
        for (number, _) in &self.placed {
            if *number > after {
                next = Some(*number);
                break;
            }
        }
        self.placed.get(&next?)
    }
    /// Borrow the placed hold currently holding this task, even under a cause of 5.5.
    #[must_use]
    pub fn placed_on(&self, task: u64) -> Option<&PlacedHold> {
        self.placed_hold(record(self, task)?.placed?)
    }
    /// Root supplies project-role precedence; tasks may only lift task-owned holds in their tree.
    #[must_use]
    pub fn may_lift(&self, hold: u64, by: Party, outranks: bool) -> bool {
        let Some(hold) = self.placed_hold(hold) else { return false };
        match hold.owner {
            Owner::Party { person } => match by {
                Party::Person(by) => by != 0 && (person == by || outranks),
                Party::Task(_) | Party::Deployment { .. } => false,
            },
            Owner::Task { task } => match by {
                Party::Task(by) => Staged::new(0).descendant(self, task, by, self.names.capacity()),
                Party::Person(person) => person != 0 && (outranks || person_tree(self, task, person)),
                Party::Deployment { .. } => false,
            },
        }
    }
}

fn person_tree(domain: &Domain, task: u64, person: u64) -> bool {
    let mut at = Some(task);
    for _ in 0..domain.names.capacity() {
        let Some(number) = at else { return false };
        let Some(row) = record(domain, number) else { return false };
        at = match row.requester {
            Party::Person(by) => return person == by,
            Party::Task(parent) => Some(parent),
            Party::Deployment { .. } => None,
        };
    }
    false
}

pub(crate) const fn task_caller(by: Party) -> bool {
    match by {
        Party::Task(_) => true,
        Party::Person(_) | Party::Deployment { .. } => false,
    }
}
const fn party_owner(owner: Owner) -> bool {
    match owner {
        Owner::Party { .. } => true,
        Owner::Task { .. } => false,
    }
}
pub(crate) const fn reason(phase: &Phase) -> Option<Hold> {
    match phase {
        Phase::Held { why, .. } => Some(*why),
        Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Ended(_) => None,
    }
}
pub(crate) const fn placed_reason(phase: &Phase) -> Option<u64> {
    match reason(phase) {
        Some(Hold::Placed { hold }) => Some(hold),
        Some(
            Hold::Exceeded(_)
            | Hold::Failures(_)
            | Hold::Stalled
            | Hold::Dependency(_)
            | Hold::Stopped
            | Hold::Drift
            | Hold::Effects
            | Hold::EffectFailed
            | Hold::Uncertain { .. }
            | Hold::Budget
            | Hold::Deadline
            | Hold::HoldsWaited
            | Hold::ResourceUnavailable
            | Hold::ResourceAccess
            | Hold::PoolLost { .. }
            | Hold::Procedure
            | Hold::Circle,
        )
        | None => None,
    }
}
const fn ending(phase: &Phase) -> bool {
    match phase {
        Phase::Closing(_) => true,
        Phase::Waiting | Phase::Active(_) | Phase::Held { .. } | Phase::Ended(_) => false,
    }
}
fn subtree(scope: Scope) -> Option<u64> {
    match scope {
        Scope::Subtree { task } => Some(task),
        Scope::Project => None,
    }
}

/// A cause may go away without releasing the independent placed hold.
pub(crate) fn enforce(row: &mut crate::TaskRecord) {
    let Some(hold) = row.placed else { return };
    let wrap = match &row.phase {
        Phase::Waiting | Phase::Active(_) => true,
        Phase::Closing(closing) => match closing.ending {
            crate::Ending::Cancelled { .. } => false,
            crate::Ending::Done(_) | crate::Ending::Failed { .. } => true,
        },
        Phase::Held { .. } | Phase::Ended(_) => false,
    };
    if wrap {
        row.phase = Phase::Held { was: crate::run::was(row.phase.clone()), why: Hold::Placed { hold }, source: None };
    }
}

pub(crate) fn shaped(hold: &PlacedHold, limits: &Limits) -> bool {
    hold.number != 0
        && hold.project != 0
        && hold.revision != 0
        && hold.revision != u64::MAX
        && hold.reason.len() <= usize::try_from(limits.message_bytes).expect("bounded bytes")
        && hold.exempt.len() <= usize::try_from(limits.exemptions).expect("bounded exemptions")
        && match hold.owner {
            Owner::Party { person } => person != 0,
            Owner::Task { task } => task != 0,
        }
        && match hold.scope {
            Scope::Subtree { task } => task != 0,
            Scope::Project => false,
        }
        && hold.mode == Mode::Stop
}

pub(crate) fn covers(domain: &Domain, staged: &Staged, hold: &PlacedHold, number: u64) -> bool {
    match hold.scope {
        Scope::Subtree { task } => staged.descendant(domain, number, task, domain.names.capacity()),
        Scope::Project => false,
    }
}

/// One preflighted whole-subtree placement; no task rows are copied.
pub(crate) struct Placement {
    pub hold: PlacedHold,
    pub tasks: List<u64>,
}

impl Staged {
    pub(crate) fn placed_hold<'a>(&'a self, domain: &'a Domain, number: u64) -> Option<&'a PlacedHold> {
        for plan in self.plans.iter().rev() {
            if let Some(changes) = plan.placed_change()
                && changes.hold.number == number
            {
                return Some(&changes.hold);
            }
            if let Some(plan) = plan.placement()
                && plan.hold.number == number
            {
                return Some(&plan.hold);
            }
        }
        domain.placed_hold(number)
    }
}

fn capacity(hold: &PlacedHold, cause: CapacityCause) -> Problem {
    let by = match hold.owner {
        Owner::Party { person } => Party::Person(person),
        Owner::Task { task } => Party::Task(task),
    };
    Problem::capacity(
        None,
        Refusal::Busy,
        Capacity { project: Some(hold.project), by, action: CapacityAction::Place { hold: hold.number }, cause },
    )
}

pub(crate) fn check_place(
    domain: &Domain,
    staged: &Staged,
    limits: &Limits,
    hold: PlacedHold,
) -> Result<Placement, Box<Problem>> {
    if !domain.ready() {
        return Err(Box::new(Problem::new(None, Refusal::NotReady)));
    }
    if !shaped(&hold, limits) || hold.revision != 1 || hold.lifted || !hold.exempt.is_empty() {
        return Err(Box::new(Problem::new(None, Refusal::AuthorityShape)));
    }
    if staged.placed_hold(domain, hold.number).is_some() {
        return Err(Box::new(Problem::new(None, Refusal::Duplicate)));
    }
    let task = subtree(hold.scope).expect("offered subtree");
    let Some(root) = staged.record(domain, task) else {
        return Err(Box::new(Problem::new(Some(task), Refusal::Unknown)));
    };
    if match root.phase {
        Phase::Ended(_) => true,
        Phase::Waiting | Phase::Active(_) | Phase::Held { .. } | Phase::Closing(_) => false,
    } || crate::compound::has(&staged.closing, task)
    {
        return Err(Box::new(Problem::new(Some(task), Refusal::State)));
    }
    if root.project != hold.project {
        return Err(Box::new(Problem::new(Some(task), Refusal::Project)));
    }
    let mut tasks = List::with_capacity(limits.tasks);
    for (number, _) in &domain.names {
        if covers(domain, staged, &hold, *number) && !crate::compound::has(&staged.closing, *number) {
            staged.placed_change_once(*number)?;
            tasks.push(*number).expect("subtree bounded by live tasks");
        }
    }
    if let Some(cause) = room(domain, staged, limits, &hold) {
        return Err(Box::new(capacity(&hold, cause)));
    }
    Ok(Placement { hold, tasks })
}

pub(crate) fn place(domain: &mut Domain, env: &Env<Limits>, to: ReplyTo, hold: PlacedHold, out: &mut Queue<Request>) {
    match check_place(domain, &Staged::new(0), &env.limits, hold) {
        Ok(plan) => {
            apply_place(domain, env, plan, out);
            out.push(Request::Done { reply_to: to });
        }
        Err(problem) => crate::capacity::refused(domain, env, to, *problem, out),
    }
}

pub(crate) fn apply_place(domain: &mut Domain, env: &Env<Limits>, plan: Placement, out: &mut Queue<Request>) {
    let hold = plan.hold.number;
    domain.placed.insert(hold, plan.hold).expect("placed table preflighted");
    save(domain, hold, out);
    for number in plan.tasks.into_boxed() {
        let row = record(domain, number).expect("selected live task");
        if let Some(attempt) = crate::run::run_attempt(&row.phase) {
            out.push(Request::Stop { task: number, attempt });
        }
        if row.placed.is_some() {
            continue;
        }
        task_mut(domain, number).expect("selected live task").record.placed = Some(hold);
        let row = task_mut(domain, number).expect("selected live task");
        if reason(&row.record.phase).is_none() {
            row.record.phase = Phase::Held {
                was: crate::run::was(row.record.phase.clone()),
                why: Hold::Placed { hold },
                source: None,
            };
            fact(domain, Fact::Held { task: number, why: Hold::Placed { hold } });
        }
        publish(domain, env, number, out);
    }
}

pub(crate) fn save(domain: &Domain, number: u64, out: &mut Queue<Request>) {
    out.push(Request::Save {
        record: Stored::PlacedHold(Box::new(domain.placed_hold(number).expect("retained hold").clone())),
    });
}

/// A placed-hold change and its preflighted per-task replacement holds.
pub(crate) struct ReleaseChanges {
    pub hold: PlacedHold,
    pub tasks: List<(u64, Option<u64>)>,
}

pub(crate) fn covering(domain: &Domain, staged: &Staged, number: u64, except: Option<u64>) -> Option<u64> {
    let mut first: Option<u64> = None;
    for (key, _) in &domain.placed {
        let hold = staged.placed_hold(domain, *key).expect("retained hold");
        if Some(*key) != except
            && !hold.lifted
            && !hold.exempt.contains(&number)
            && covers(domain, staged, hold, number)
        {
            first = Some(match first {
                Some(old) => old.min(*key),
                None => *key,
            });
        }
    }
    for plan in &staged.plans {
        if let Some(plan) = plan.placement() {
            let hold = staged.placed_hold(domain, plan.hold.number).expect("staged hold");
            if Some(hold.number) != except
                && !hold.lifted
                && !hold.exempt.contains(&number)
                && covers(domain, staged, hold, number)
            {
                first = Some(match first {
                    Some(old) => old.min(hold.number),
                    None => hold.number,
                });
            }
        }
    }
    first
}

pub(crate) fn check_release(
    domain: &Domain,
    staged: &Staged,
    limits: &Limits,
    number: u64,
    by: Party,
) -> Result<Option<Box<ReleaseChanges>>, Refusal> {
    let row = staged.record(domain, number).ok_or(Refusal::Unknown)?;
    let Some(identity) = row.placed else { return Ok(None) };
    if let Err(problem) = staged.placed_change_once(number) {
        return Err(problem.why);
    }
    if task_caller(by) && !domain.may_lift(identity, by, false) {
        return Err(Refusal::HoldOwner { hold: identity });
    }
    let mut hold = staged.placed_hold(domain, identity).ok_or(Refusal::State)?.clone();
    let whole = !hold.lifted && hold.scope == (Scope::Subtree { task: number });
    if whole {
        if hold.revision >= u64::MAX.saturating_sub(1) {
            return Err(Refusal::State);
        }
        hold.lifted = true;
        hold.revision = hold.revision.checked_add(1).ok_or(Refusal::State)?;
    } else if !hold.lifted {
        if hold.exempt.len() >= usize::try_from(limits.exemptions).expect("bounded exemptions") {
            return Err(Refusal::Busy);
        }
        let mut exempt = List::with_capacity(limits.exemptions);
        for task in &hold.exempt {
            exempt.push(*task).expect("admitted exemptions");
        }
        exempt.push(number).expect("exemption room preflighted");
        hold.exempt = exempt.into_boxed();
    }
    let tasks = replacements(domain, staged, limits, identity, if whole { None } else { Some(number) })?;
    Ok(Some(Box::new(ReleaseChanges { hold, tasks })))
}

fn replacements(
    domain: &Domain,
    staged: &Staged,
    limits: &Limits,
    hold: u64,
    only: Option<u64>,
) -> Result<List<(u64, Option<u64>)>, Refusal> {
    let mut tasks = List::with_capacity(limits.tasks);
    for (number, _) in &domain.names {
        let row = staged.record(domain, *number).expect("indexed row");
        if row.placed == Some(hold) && (only.is_none() || only == Some(*number)) {
            if let Err(problem) = staged.placed_change_once(*number) {
                return Err(problem.why);
            }
            let next = covering(domain, staged, *number, Some(hold));
            if placed_reason(&row.phase).is_some()
                && (row.revision == u64::MAX || (next.is_some() && row.revisions.hold == u64::MAX))
            {
                return Err(Refusal::State);
            }
            tasks.push((*number, next)).expect("bounded replacements");
        }
    }
    Ok(tasks)
}

pub(crate) fn apply_changes(
    domain: &mut Domain,
    env: &Env<Limits>,
    changes: ReleaseChanges,
    explicit: Option<u64>,
    by: Party,
    out: &mut Queue<Request>,
) -> u32 {
    let held = changes.hold.number;
    let previous = domain.placed_hold(held).expect("changed hold").revision;
    domain.placed.insert(held, changes.hold).expect("existing hold");
    save(domain, held, out);
    if domain.placed_hold(held).expect("changed hold").revision != previous {
        withdraw(domain, env, held, out);
    }
    let mut released = 0_u32;
    for (number, next) in changes.tasks.into_boxed() {
        let row = task_mut(domain, number).expect("selected task");
        row.record.placed = next;
        if Some(number) == explicit {
            continue;
        }
        if placed_reason(&row.record.phase).is_some() {
            if let Some(hold) = next {
                row.record.phase = Phase::Held {
                    was: crate::run::was(row.record.phase.clone()),
                    why: Hold::Placed { hold },
                    source: None,
                };
                publish(domain, env, number, out);
            } else {
                let plan = crate::control::check_release(domain, &Staged::new(0), env, number, by, None, None)
                    .expect("placed reason needs no effect choice or message");
                crate::control::apply_release(domain, env, by, plan, out);
                released = released.checked_add(1).expect("bounded tasks");
            }
        } else {
            publish(domain, env, number, out);
        }
    }
    prune(domain, held, out);
    released
}

pub(crate) struct Lift {
    pub by: Party,
    pub number: u64,
    pub revision: u64,
    pub with_tasks: bool,
}
pub(crate) fn lift(domain: &mut Domain, env: &Env<Limits>, to: ReplyTo, lift: Lift, out: &mut Queue<Request>) {
    let Lift { by, number, revision, with_tasks } = lift;
    if !domain.ready() {
        return crate::domain::refused(to, None, Refusal::NotReady, out);
    }
    let Some(hold) = domain.placed_hold(number) else { return crate::domain::refused(to, None, Refusal::Unknown, out) };
    if revision != hold.revision {
        return crate::domain::refused(to, None, Refusal::HoldRevision { revision: hold.revision }, out);
    }
    if hold.lifted || hold.revision >= u64::MAX.saturating_sub(1) {
        return crate::domain::refused(to, None, Refusal::State, out);
    }
    if task_caller(by) && !domain.may_lift(number, by, false) {
        return crate::domain::refused(to, None, Refusal::HoldOwner { hold: number }, out);
    }
    let mut hold = hold.clone();
    hold.lifted = true;
    hold.revision = hold.revision.checked_add(1).expect("hold revision admitted");
    let tasks = if with_tasks {
        match replacements(domain, &Staged::new(0), &env.limits, number, None) {
            Ok(tasks) => tasks,
            Err(why) => return crate::domain::refused(to, None, why, out),
        }
    } else {
        List::with_capacity(0)
    };
    let released = apply_changes(domain, env, ReleaseChanges { hold, tasks }, None, by, out);
    out.push(Request::Lifted { reply_to: to, released });
}

pub(crate) fn prune(domain: &mut Domain, hold: u64, out: &mut Queue<Request>) {
    let Some(row) = domain.placed_hold(hold) else { return };
    if !row.lifted {
        return;
    }
    for (number, _) in &domain.names {
        if record(domain, *number).expect("live row").placed == Some(hold) {
            return;
        }
    }
    domain.placed.remove(&hold);
    out.push(Request::Erase { key: Key::PlacedHold(hold) });
}

fn withdraw(domain: &mut Domain, env: &Env<Limits>, hold: u64, out: &mut Queue<Request>) {
    let numbers = crate::domain::snapshot(domain, env.limits.tasks);
    for number in numbers.into_boxed() {
        let row = task_mut(domain, number).expect("live proposer");
        let names = match &row.record.proposal {
            Some(proposal) => match proposal_hold(&proposal.action) {
                Some((named, _)) => named == hold,
                None => false,
            },
            None => false,
        };
        if names {
            let mut proposal = *row.record.proposal.take().expect("named proposal");
            proposal.state = crate::ProposalState::Withdrawn;
            crate::positions::history(
                domain,
                env,
                proposal,
                Party::Task(number),
                crate::Change::ProposalWithdrawn,
                out,
            );
            domain.proposal_alarms.cancel(number);
            publish(domain, env, number, out);
        }
    }
}

pub(crate) fn ended(domain: &mut Domain, env: &Env<Limits>, number: u64, out: &mut Queue<Request>) {
    let mut holds = List::with_capacity(env.limits.placed_holds);
    for (key, hold) in &domain.placed {
        if hold.scope == (Scope::Subtree { task: number }) {
            holds.push(*key).expect("bounded holds");
        }
    }
    for held in holds.into_boxed() {
        let mut hold = domain.placed_hold(held).expect("scope ended").clone();
        hold.lifted = true;
        hold.revision = hold.revision.checked_add(1).expect("hold revision admitted");
        let tasks = replacements(domain, &Staged::new(0), &env.limits, held, None)
            .expect("ending tasks need no placed release history");
        apply_changes(domain, env, ReleaseChanges { hold, tasks }, None, Party::Task(number), out);
        withdraw(domain, env, held, out);
        if domain.placed.remove(&held).is_some() {
            out.push(Request::Erase { key: Key::PlacedHold(held) });
        }
    }
}

pub(crate) fn worst_case(limits: &Limits) -> Option<u64> {
    Map::<u64, PlacedHold>::worst_case(limits.placed_holds)?
        .checked_add(
            u64::from(limits.placed_holds).checked_mul(
                u64::from(limits.message_bytes).checked_add(u64::from(limits.exemptions).checked_mul(8)?)?,
            )?,
        )?
        // Placement selection, replacement walk, and hold erasure snapshot.
        .checked_add(List::<u64>::worst_case(limits.tasks)?)?
        .checked_add(List::<(u64, Option<u64>)>::worst_case(limits.tasks)?)?
        .checked_add(List::<u64>::worst_case(limits.placed_holds)?)
}

/// Preserve the live attempt fence when resuming; preparation and backoff start due again.
pub(crate) fn resumed(was: &crate::Was) -> Phase {
    match was {
        crate::Was::Waiting => Phase::Waiting,
        crate::Was::Closing(closing) => Phase::Closing(closing.clone()),
        crate::Was::Active(crate::Active::Claimed { .. } | crate::Active::Running { .. }) => match was {
            crate::Was::Active(active) => Phase::Active(*active),
            crate::Was::Waiting | crate::Was::Closing(_) => unreachable!("active"),
        },
        crate::Was::Active(_) => Phase::Active(crate::Active::Due),
    }
}

pub(crate) fn check(domain: &Domain, limits: &Limits) -> bool {
    let staged = Staged::new(0);
    for (key, hold) in &domain.placed {
        let Some(task) = subtree(hold.scope) else { return false };
        let Some(root) = record(domain, task) else { return false };
        if root.project != hold.project || !shaped(hold, limits) {
            return false;
        }
        let mut project = 0_u32;
        let mut party = 0_u32;
        let mut named = false;
        for (_, other) in &domain.placed {
            if other.project == hold.project {
                project = project.checked_add(1).expect("bounded holds");
            }
            if other.owner == hold.owner {
                party = party.checked_add(1).expect("bounded holds");
            }
        }
        if project > limits.project_placed_holds || (party_owner(hold.owner) && party > limits.party_placed_holds) {
            return false;
        }
        for (at, task) in hold.exempt.iter().enumerate() {
            if *task == 0 {
                return false;
            }
            for earlier in hold.exempt.iter().take(at) {
                if earlier == task {
                    return false;
                }
            }
        }
        for (number, _) in &domain.names {
            let row = record(domain, *number).expect("live task");
            named |= row.placed == Some(*key);
            if !hold.lifted
                && covers(domain, &staged, hold, *number)
                && !hold.exempt.contains(number)
                && row.placed.is_none()
                && !ending(&row.phase)
            {
                return false;
            }
        }
        if hold.lifted && !named {
            return false;
        }
    }
    for (number, _) in &domain.names {
        let row = record(domain, *number).expect("live task");
        if let Some(key) = row.placed {
            let Some(hold) = domain.placed_hold(key) else { return false };
            if row.project != hold.project
                || hold.exempt.contains(number)
                || (!hold.lifted && !covers(domain, &staged, hold, *number))
                || (reason(&row.phase).is_none() && !ending(&row.phase))
            {
                return false;
            }
        }
        if let Some(hold) = placed_reason(&row.phase)
            && row.placed != Some(hold)
        {
            return false;
        }
        if let Some(proposal) = &row.proposal
            && let Some((hold, revision)) = proposal_hold(&proposal.action)
        {
            let Some(hold) = domain.placed_hold(hold) else { return false };
            if hold.lifted || hold.revision != revision {
                return false;
            }
        }
    }
    true
}

/// A new delegate is not covered by its creator's personal exemption.
pub(crate) fn delegation_hold(domain: &Domain, creator: u64) -> Option<u64> {
    let staged = Staged::new(0);
    let mut first: Option<u64> = None;
    for (number, hold) in &domain.placed {
        if !hold.lifted && covers(domain, &staged, hold, creator) {
            first = Some(match first {
                Some(old) => old.min(*number),
                None => *number,
            });
        }
    }
    first
}

pub(crate) fn proposal_hold(action: &crate::ProposalAction) -> Option<(u64, u64)> {
    match action {
        crate::ProposalAction::ReleasePlaced { hold, revision, .. } => Some((*hold, *revision)),
        crate::ProposalAction::Batch(_)
        | crate::ProposalAction::Effect { .. }
        | crate::ProposalAction::ProcedureEffect { .. }
        | crate::ProposalAction::Amend { .. }
        | crate::ProposalAction::Widen { .. }
        | crate::ProposalAction::Release { .. } => None,
    }
}

pub(crate) const fn is_placed(why: Hold) -> bool {
    match why {
        Hold::Placed { .. } => true,
        Hold::Exceeded(_)
        | Hold::Failures(_)
        | Hold::Stalled
        | Hold::Dependency(_)
        | Hold::Stopped
        | Hold::Drift
        | Hold::Effects
        | Hold::EffectFailed
        | Hold::Uncertain { .. }
        | Hold::Budget
        | Hold::Deadline
        | Hold::HoldsWaited
        | Hold::ResourceUnavailable
        | Hold::ResourceAccess
        | Hold::PoolLost { .. }
        | Hold::Procedure
        | Hold::Circle => false,
    }
}

fn room(domain: &Domain, staged: &Staged, limits: &Limits, hold: &PlacedHold) -> Option<CapacityCause> {
    let mut total = 0_u32;
    let mut project = 0_u32;
    let mut party = 0_u32;
    for (key, _) in &domain.placed {
        let old = staged.placed_hold(domain, *key).expect("retained hold");
        if !retained(domain, staged, old) {
            continue;
        }
        total = total.checked_add(1).expect("bounded holds");
        if old.project == hold.project {
            project = project.checked_add(1).expect("bounded holds");
        }
        if old.owner == hold.owner {
            party = party.checked_add(1).expect("bounded holds");
        }
    }
    for plan in &staged.plans {
        if let Some(plan) = plan.placement() {
            let old = staged.placed_hold(domain, plan.hold.number).expect("staged hold");
            if !retained(domain, staged, old) {
                continue;
            }
            total = total.checked_add(1).expect("bounded holds");
            if plan.hold.project == hold.project {
                project = project.checked_add(1).expect("bounded holds");
            }
            if plan.hold.owner == hold.owner {
                party = party.checked_add(1).expect("bounded holds");
            }
        }
    }
    if total >= limits.placed_holds {
        Some(CapacityCause::PlacedHolds { limit: limits.placed_holds })
    } else if project >= limits.project_placed_holds {
        Some(CapacityCause::ProjectPlacedHolds { limit: limits.project_placed_holds })
    } else if party_owner(hold.owner) && party >= limits.party_placed_holds {
        Some(CapacityCause::PartyPlacedHolds { limit: limits.party_placed_holds })
    } else {
        None
    }
}

fn retained(domain: &Domain, staged: &Staged, hold: &PlacedHold) -> bool {
    if !hold.lifted {
        return true;
    }
    for (number, _) in &domain.names {
        if staged.record(domain, *number).expect("projected live row").placed == Some(hold.number) {
            return true;
        }
    }
    false
}

pub(crate) fn compound_bytes(limits: &Limits) -> Option<u64> {
    let selection = List::<u64>::worst_case(limits.tasks)?.checked_add(u64::try_from(size_of::<Placement>()).ok()?)?;
    let release = List::<(u64, Option<u64>)>::worst_case(limits.tasks)?
        .checked_add(u64::try_from(size_of::<ReleaseChanges>()).ok()?)?
        .checked_add(u64::from(limits.message_bytes))?
        .checked_add(u64::from(limits.exemptions).checked_mul(8)?)?;
    u64::from(limits.tasks).checked_mul(selection.max(release))
}

/// Moving a whole scope preserves its hold; detaching a held delegate must first release it.
pub(crate) fn check_detach(domain: &Domain, staged: &Staged, ancestor: u64, number: u64) -> Result<(), Refusal> {
    let row = staged.record(domain, number).expect("moving live task");
    if let Some(identity) = row.placed {
        let hold = staged.placed_hold(domain, identity).expect("task names its placed hold");
        if !hold.lifted
            && let Some(root) = subtree(hold.scope)
            && !staged.descendant(domain, root, ancestor, domain.names.capacity())
        {
            return Err(Refusal::Placed { hold: identity });
        }
    }
    Ok(())
}
