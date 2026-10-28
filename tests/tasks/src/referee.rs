//! Independent judgements from committed records and world inputs
//! (domain/tasks.md, 5.7, 6.7, sections 7 and 12).
use jig_domain_tasks::{
    Contract, Ending, HoldKind, Holding, Kind, Last, Limits, Name as ResourceName, Parameter, Party, Phase, PoolSlots,
    SettingsRevision, Status, TaskRecord, TaskResult, Was, WriterSlot,
};
use skein_lib::Duration;
use skein_world::domain::{Expectations, Judge};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Seen {
    Placed {
        live: Vec<TaskRecord>,
        holds: Vec<jig_domain_tasks::PlacedHold>,
        stops: BTreeSet<(u64, u64)>,
        parks: BTreeSet<(u64, u64)>,
        draining: BTreeSet<u64>,
        limits: Box<Limits>,
    },
    Batch {
        members: Vec<u64>,
        accepted: bool,
        made: Vec<u64>,
    },
    Made {
        task: u64,
        parent: Party,
        dependencies: Vec<u64>,
        depth: u32,
        settings: SettingsRevision,
        newest: u32,
    },
    Newest {
        revision: u32,
    },
    MovedToNewest {
        task: u64,
        revision: u32,
    },
    PreparationAsked {
        task: u64,
    },
    Prepared {
        task: u64,
        revision: u32,
    },
    Moved {
        task: u64,
        parent: Party,
    },
    Amended {
        task: u64,
        dependencies: Vec<u64>,
    },
    Durable {
        commit: u64,
    },
    Replied {
        call: u64,
        after: u64,
    },
    Assigned {
        task: u64,
        attempt: u64,
        after: u64,
        adopted: bool,
        settings: u32,
    },
    Terminal {
        task: u64,
        attempt: u64,
    },
    Closing {
        task: u64,
    },
    Settled {
        task: u64,
    },
    Ended {
        task: u64,
        status: Status,
        after: u64,
    },
    Cancelled {
        task: u64,
    },
    Limit {
        live: usize,
        cap: u32,
    },
    Stored {
        live: Vec<TaskRecord>,
        limits: Box<Limits>,
    },
    Resources {
        live: Vec<TaskRecord>,
        pools: Vec<PoolSlots>,
        writers: Vec<WriterSlot>,
        kinds: Vec<Kind>,
        reports: Vec<(ResourceName, HoldKind)>,
    },
    Finished,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Name {
    End(u64),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stimulus {
    Restart,
}

#[derive(Default, Debug)]
pub struct Tasks {
    durable: u64,
    newest: u32,
    settings: BTreeMap<u64, u32>,
    prepared: BTreeMap<u64, u32>,
    preparing: BTreeSet<u64>,
    replies: BTreeSet<u64>,
    parents: BTreeMap<u64, Party>,
    dependencies: BTreeMap<u64, Vec<u64>>,
    runs: BTreeMap<u64, u64>,
    attempts: BTreeMap<u64, u64>,
    closing: BTreeSet<u64>,
    settled: BTreeSet<u64>,
    ended: BTreeMap<u64, Status>,
    cancelled: BTreeSet<u64>,
    pool_counts: Vec<(ResourceName, usize)>,
}

impl Tasks {
    fn resources(
        &mut self,
        live: &[TaskRecord],
        pools: &[PoolSlots],
        writers: &[WriterSlot],
        rules: &Rules,
        judge: &mut Judge<Name, Stimulus>,
    ) {
        if circle_of_waits(live, pools, rules) {
            judge.fail("circle of waits");
        }
        for row in live {
            if row.holds_taken && matches!(row.phase, Phase::Waiting | Phase::Held { was: Was::Waiting, .. }) {
                judge.fail("task waits while holding a resource");
            }
            if !row.holds_taken {
                continue;
            }
            for holding in &row.holdings {
                if matches!(rules.rule(holding), Some(HoldKind::Exclusive { .. })) {
                    let resource = holding_name(holding);
                    let holders = holder_count(live, resource);
                    if holders != 1 {
                        judge.fail("resource has multiple holders");
                    }
                }
            }
        }
        let mut counts = Vec::new();
        for row in live.iter().filter(|row| row.holds_taken) {
            for holding in &row.holdings {
                if matches!(rules.rule(holding), Some(HoldKind::Pooled { .. })) {
                    let pool = holding_name(holding);
                    if counts.iter().any(|(name, _)| name == pool) {
                        continue;
                    }
                    let current = holder_count(live, pool);
                    let before = self.pool_counts.iter().find(|(name, _)| name == pool).map_or(0, |(_, count)| *count);
                    let slots = pools.iter().find(|row| row.pool == *pool).map_or(0, |row| row.slots);
                    if current > before && current > usize::try_from(slots).expect("u32 fits usize") {
                        judge.fail("pool admitted beyond known slots");
                    }
                    counts.push((pool.clone(), current));
                }
            }
        }
        self.pool_counts = counts;
        for (at, writer) in writers.iter().enumerate() {
            if writers.iter().skip(at + 1).any(|other| other.resource == writer.resource) {
                judge.fail("resource has multiple writers");
            }
        }
    }

    #[must_use]
    pub fn has_task(&self, task: u64) -> bool {
        self.parents.contains_key(&task)
    }

    #[must_use]
    pub fn descendants(&self, ancestor: u64) -> Vec<u64> {
        self.parents
            .keys()
            .filter(|task| !self.ended.contains_key(task))
            .filter(|task| {
                let mut at = Some(**task);
                for _ in 0..self.parents.len() {
                    let Some(number) = at else {
                        return false;
                    };
                    if number == ancestor {
                        return true;
                    }
                    at = match self.parents.get(&number) {
                        Some(Party::Task(parent)) => Some(*parent),
                        Some(Party::Person(_) | Party::Deployment { .. }) | None => None,
                    };
                }
                false
            })
            .copied()
            .collect()
    }

    fn end(&mut self, task: u64, status: Status, after: u64, judge: &mut Judge<Name, Stimulus>) {
        if after > self.durable {
            judge.fail("result before durability");
        }
        if !self.settled.contains(&task) {
            judge.fail("result before settlement");
        }
        if self.runs.contains_key(&task)
            || self
                .parents
                .iter()
                .any(|(child, parent)| *parent == Party::Task(task) && !self.ended.contains_key(child))
        {
            judge.fail("requester ended before descendant");
        }
        if self.ended.insert(task, status).is_some() {
            judge.fail("result delivered twice");
        }
        if self.cancelled.contains(&task) && status != Status::Cancelled {
            judge.fail("cancelled descendant returned different ending");
        }
        judge.meet(&Name::End(task));
    }
}

impl Expectations for Tasks {
    type Seen = Seen;

    type Name = Name;

    type Stimulus = Stimulus;

    #[expect(
        clippy::too_many_lines,
        reason = "one exhaustive observation seam checks the independent lifecycle promises"
    )]
    fn observe(&mut self, seen: Seen, judge: &mut Judge<Name, Stimulus>) {
        match seen {
            Seen::Batch { mut members, accepted, mut made } => {
                members.sort_unstable();
                made.sort_unstable();
                if (accepted && made != members) || (!accepted && !made.is_empty()) {
                    judge.fail("batch partially admitted");
                }
            }
            Seen::Newest { revision } => self.newest = revision,
            Seen::MovedToNewest { task, revision } => {
                if revision != self.newest || !self.settings.contains_key(&task) {
                    judge.fail("task moved to settings other than newest");
                }
                self.settings.insert(task, revision);
            }
            Seen::PreparationAsked { task } => {
                self.preparing.insert(task);
            }
            Seen::Prepared { task, revision } => {
                if !self.preparing.remove(&task) {
                    judge.fail("run settings changed without preparation");
                }
                if self.settings.get(&task) != Some(&revision) {
                    judge.fail("run prepared on another settings revision");
                }
                self.prepared.insert(task, revision);
            }
            Seen::Made { task, parent, dependencies, depth, settings, newest } => {
                if settings.task != newest {
                    judge.fail("task made on an older settings revision");
                }
                self.settings.insert(task, settings.task);
                if let Some(run) = settings.run {
                    if run != settings.task {
                        judge.fail("new task run prepared on another settings revision");
                    }
                    self.prepared.insert(task, run);
                }
                if self.parents.insert(task, parent).is_some() {
                    judge.fail("task number reused");
                }
                if dependencies.contains(&task) {
                    judge.fail("task waits on itself");
                }
                if let Party::Task(parent) = parent
                    && (!self.parents.contains_key(&parent) || self.ended.contains_key(&parent) || depth == 0)
                {
                    judge.fail("delegate has no live parent");
                }
                self.dependencies.insert(task, dependencies);
            }
            Seen::Moved { task, parent } => {
                if let Some(requester) = self.parents.get_mut(&task) {
                    *requester = parent;
                } else {
                    judge.fail("moved task was never made");
                }
            }
            Seen::Amended { task, dependencies } => {
                for dependency in &dependencies {
                    // Retaining a prerequisite that failed later is not an add (section 7).
                    let mut retained = false;
                    if let Some(before) = self.dependencies.get(&task) {
                        retained = before.contains(dependency);
                    }
                    if !retained && matches!(self.ended.get(dependency), Some(Status::Failed | Status::Cancelled)) {
                        judge.fail(format!("amendment of {task} adds failed or cancelled dependency {dependency}"));
                    }
                }
                self.dependencies.insert(task, dependencies);
            }
            Seen::Durable { commit } => self.durable = commit,
            Seen::Replied { call, after } => {
                if after > self.durable {
                    judge.fail("reply before durability");
                }
                if !self.replies.insert(call) {
                    judge.fail("call replied twice");
                }
            }
            Seen::Assigned { task, attempt, after, adopted, settings } => {
                if self.prepared.get(&task) != Some(&settings) {
                    judge.fail("assignment on another settings revision");
                }
                if after > self.durable {
                    judge.fail("run before claim durable");
                }
                if self.runs.get(&task).is_some_and(|old| *old != attempt || !adopted) {
                    judge.fail("two runs for task");
                }
                if !adopted && self.attempts.get(&task).is_some_and(|old| *old >= attempt) {
                    judge.fail("attempt did not grow");
                }
                if !adopted
                    && self.dependencies.get(&task).is_some_and(|dependencies| {
                        dependencies.iter().any(|number| self.ended.get(number) != Some(&Status::Done))
                    })
                {
                    judge.fail("run before dependencies done");
                }
                self.runs.insert(task, attempt);
                self.attempts.insert(task, attempt);
            }
            Seen::Terminal { task, attempt } => {
                if self.runs.get(&task) != Some(&attempt) {
                    judge.fail("terminal is not current run");
                }
                self.runs.remove(&task);
            }
            Seen::Closing { task } => {
                if self.runs.contains_key(&task) {
                    judge.fail("effects closed before own run ended");
                }
                if self
                    .parents
                    .iter()
                    .any(|(child, parent)| *parent == Party::Task(task) && !self.ended.contains_key(child))
                {
                    judge.fail("effects closed before delegates ended");
                }
                self.closing.insert(task);
            }
            Seen::Settled { task } => {
                if !self.closing.contains(&task) {
                    judge.fail("settled before closing");
                }
                self.settled.insert(task);
            }
            Seen::Ended { task, status, after } => {
                if status == Status::Done
                    && self.dependencies.get(&task).is_some_and(|dependencies| {
                        dependencies.iter().any(|dependency| self.ended.get(dependency) != Some(&Status::Done))
                    })
                {
                    judge.fail("kept task ended done");
                }
                self.end(task, status, after, judge);
            }
            Seen::Cancelled { task } => {
                for task in self.descendants(task) {
                    if self.cancelled.insert(task) {
                        judge.expect(Name::End(task), Duration::from_secs(10));
                    }
                }
            }
            Seen::Limit { live, cap } => {
                if live > usize::try_from(cap).expect("u32 fits usize") {
                    judge.fail("live task limit exceeded");
                }
            }
            Seen::Placed { live, holds, stops, parks, draining, limits } => placed(&live, &holds, &stops, &parks, &draining, &limits, judge),
            Seen::Stored { live, limits } => {
                stored(&live, &limits, judge);
                self.preparing.clear();
                for row in &live {
                    if self.settings.get(&row.number) != Some(&row.settings.task) {
                        judge.fail("stored task settings changed without moving to newest");
                    }
                    if let Some(run) = row.settings.run
                        && self.prepared.get(&row.number) != Some(&run)
                    {
                        judge.fail("stored run settings changed since preparation");
                    }
                    let proposal_holder = match &row.proposal {
                        Some(proposal) => match proposal.state {
                            jig_domain_tasks::ProposalState::Pending {
                                holder: jig_domain_tasks::ProposalHolder::Task(holder),
                                ..
                            } => Some(holder),
                            jig_domain_tasks::ProposalState::Pending { .. }
                            | jig_domain_tasks::ProposalState::Accepted { .. }
                            | jig_domain_tasks::ProposalState::Rejected { .. }
                            | jig_domain_tasks::ProposalState::Withdrawn => None,
                        },
                        None => None,
                    };
                    let escalation_holder = match row.escalation {
                        jig_domain_tasks::Escalation::Waiting {
                            holder: jig_domain_tasks::EscalationHolder::Task(holder),
                            ..
                        } => Some(holder),
                        jig_domain_tasks::Escalation::Unheld { .. }
                        | jig_domain_tasks::Escalation::Routing { .. }
                        | jig_domain_tasks::Escalation::Waiting { .. }
                        | jig_domain_tasks::Escalation::Rejected { .. } => None,
                    };
                    for holder in [proposal_holder, escalation_holder].into_iter().flatten() {
                        if live.iter().any(|task| {
                            task.number == holder
                                && matches!(task.phase, Phase::Active(_))
                                && !task.waiting_on.is_empty()
                        }) {
                            judge.fail("decision waits at kept task");
                        }
                    }
                }
            }
            Seen::Resources { live, pools, writers, kinds, reports } => {
                self.resources(&live, &pools, &writers, &Rules { kinds, reports }, judge);
            }
            Seen::Finished => {
                if self.cancelled.iter().any(|task| !self.ended.contains_key(task)) {
                    judge.fail("cancelled task did not end");
                }
            }
        }
    }
}

fn bytes(result: &TaskResult) -> usize {
    match result {
        TaskResult::Report { words } | TaskResult::Verdict { words, .. } | TaskResult::Change { words, .. } => {
            words.len()
        }
        TaskResult::Approved { state, words } => state.len() + words.as_ref().map_or(0, |words| words.len()),
        TaskResult::Failure { reason } => reason.len(),
    }
}

fn end_within(ending: &Ending, cap: usize) -> bool {
    match ending {
        Ending::Done(result) => bytes(result) <= cap,
        Ending::Failed { reason } => reason.len() <= cap,
        Ending::Cancelled { reason, result, .. } => {
            reason.len() <= cap && result.as_ref().is_none_or(|result| bytes(result) <= cap)
        }
    }
}

fn within(task: &TaskRecord, limits: &Limits) -> bool {
    let spec_bytes = task.spec.words.len()
        + task
            .spec
            .parameters
            .iter()
            .map(|parameter| match parameter {
                Parameter::Bytes { value, .. } => value.len(),
                Parameter::Number { .. } | Parameter::Resource { .. } => 0,
            })
            .sum::<usize>();
    let grant_bytes = task
        .authority
        .grants
        .iter()
        .map(|grant| {
            grant.pattern.segments.iter().map(|segment| segment.len()).sum::<usize>()
                + match &grant.pattern.last {
                    Last::Exact(bytes) | Last::Open(bytes) => bytes.len(),
                }
        })
        .sum::<usize>();
    let counts = [
        (task.delegates.len(), limits.delegates),
        (task.dependencies.len(), limits.dependencies),
        (task.spec.inputs.len(), limits.inputs),
        (task.spec.parameters.len(), limits.parameters),
        (spec_bytes, limits.spec_bytes),
        (task.authority.grants.len(), limits.authority_grants),
        (task.authority.delegation.kinds.len(), limits.executor_kinds),
        (grant_bytes, limits.authority_bytes),
    ];
    if task.depth > limits.depth
        || task.made > limits.tree_tasks
        || counts.into_iter().any(|(count, limit)| count > usize::try_from(limit).expect("u32 fits usize"))
        || task.authority.grants.iter().any(|grant| {
            grant.pattern.segments.len() > usize::try_from(limits.authority_segments).expect("u32 fits usize")
        })
    {
        return false;
    }
    let result_cap = usize::try_from(limits.result_bytes).expect("u32 fits usize");
    let contract = match &task.contract {
        Contract::Approval { state, .. } => state.len() <= result_cap,
        Contract::Report { words } | Contract::Change { words, .. } => *words <= limits.result_bytes,
        Contract::Verdict { choices } => {
            !choices.is_empty()
                && choices.len() <= usize::try_from(limits.contract_choices).expect("u32 fits usize")
                && choices.iter().all(|choice| choice.words <= limits.result_bytes)
        }
    };
    contract
        && match &task.phase {
            Phase::Closing(closing) | Phase::Held { was: Was::Closing(closing), .. } => {
                end_within(&closing.ending, result_cap)
            }
            Phase::Ended(ending) => end_within(ending, result_cap),
            Phase::Waiting | Phase::Active(_) | Phase::Held { was: Was::Waiting | Was::Active(_), .. } => true,
        }
}

fn stored(live: &[TaskRecord], limits: &Limits, judge: &mut Judge<Name, Stimulus>) {
    for task in live {
        if task.refused.iter().any(|number| !task.inbox.iter().any(|word| word.number == *number)) {
            judge.fail("committed message taken while refused");
        }
        if task.last_answer == Some(task.attempt) && !task.refused.is_empty() {
            judge.fail("message refusals kept past their attempt");
        }
    }
    if !live.iter().all(|task| within(task, limits))
        || live.iter().any(|task| {
            live.iter().filter(|other| other.project == task.project).count()
                > usize::try_from(limits.project_tasks).expect("u32 fits usize")
        })
    {
        judge.fail("stored shape exceeds configured limit");
    }
}

// Compute live waits independently of the tasks child's indexes and back walk.
fn circle_of_waits(live: &[TaskRecord], pools: &[PoolSlots], rules: &Rules) -> bool {
    let mut pending = BTreeMap::<u64, usize>::new();
    let mut dependents = BTreeMap::<u64, Vec<u64>>::new();
    let mut held = BTreeMap::<ResourceName, Vec<u64>>::new();
    let mut waiters = BTreeMap::<ResourceName, Vec<u64>>::new();
    let mut thresholds = BTreeMap::<ResourceName, usize>::new();
    for row in live.iter().filter(|row| row.holds_taken) {
        for holding in &row.holdings {
            let name = holding_name(holding);
            held.entry(name.clone()).or_default().push(row.number);
        }
    }
    for row in live {
        let mut waits = 0;
        if matches!(row.phase, Phase::Waiting | Phase::Active(_)) {
            for dependency in &row.waiting_on {
                if live.iter().any(|other| other.number == *dependency) {
                    dependents.entry(*dependency).or_default().push(row.number);
                    waits += 1;
                }
            }
        }
        if matches!(row.phase, Phase::Waiting | Phase::Active(_) | Phase::Closing(_)) {
            for delegate in &row.delegates {
                if live.iter().any(|other| other.number == *delegate) {
                    dependents.entry(*delegate).or_default().push(row.number);
                    waits += 1;
                }
            }
        }
        if row.phase == Phase::Waiting && row.hold_wait_since.is_some() && !row.holds_taken && row.waiting_on.is_empty()
        {
            for holding in &row.holdings {
                let name = holding_name(holding);
                let threshold = match rules.rule(holding) {
                    Some(HoldKind::Shared) | None => continue,
                    Some(HoldKind::Exclusive { .. }) => 1,
                    Some(HoldKind::Pooled { .. }) => pools
                        .iter()
                        .find(|pool| pool.pool == *name)
                        .map_or(1, |pool| usize::try_from(pool.slots.max(1)).expect("u32 fits usize")),
                };
                thresholds.insert(name.clone(), threshold);
                if held.get(name).map_or(0, Vec::len) >= threshold {
                    waiters.entry(name.clone()).or_default().push(row.number);
                    waits += 1;
                }
            }
        }
        pending.insert(row.number, waits);
    }
    let mut ready: Vec<_> = pending.iter().filter(|(_, waits)| **waits == 0).map(|(task, _)| *task).collect();
    while let Some(number) = ready.pop() {
        pending.remove(&number);
        for dependent in dependents.get(&number).into_iter().flatten() {
            remove_wait(&mut pending, &mut ready, *dependent);
        }
        let Some(row) = live.iter().find(|row| row.number == number) else { continue };
        if !row.holds_taken {
            continue;
        }
        for holding in &row.holdings {
            let name = holding_name(holding);
            let holders = held.get_mut(name).expect("counted holder");
            let before = holders.len();
            holders.retain(|holder| *holder != number);
            if thresholds.get(name) == Some(&before) {
                for waiter in waiters.get(name).into_iter().flatten() {
                    remove_wait(&mut pending, &mut ready, *waiter);
                }
            }
        }
    }
    !pending.is_empty()
}

fn remove_wait(pending: &mut BTreeMap<u64, usize>, ready: &mut Vec<u64>, task: u64) {
    if let Some(waits) = pending.get_mut(&task) {
        *waits = waits.checked_sub(1).expect("each observed wait removed once");
        if *waits == 0 {
            ready.push(task);
        }
    }
}

fn holding_name(holding: &Holding) -> &ResourceName {
    match holding {
        Holding::Write { resource, .. } => resource,
        Holding::Slot { pool, .. } => pool,
    }
}

// Only the rules sent by the world resolve a resource (domain/tasks.md, section 6).
struct Rules {
    kinds: Vec<Kind>,
    reports: Vec<(ResourceName, HoldKind)>,
}

impl Rules {
    fn rule(&self, holding: &Holding) -> Option<HoldKind> {
        let name = holding_name(holding);
        for (reported, rule) in &self.reports {
            if reported == name {
                return Some(*rule);
            }
        }
        let (Holding::Write { kind, .. } | Holding::Slot { kind, .. }) = holding;
        for configured in &self.kinds {
            if configured.connector == name.connector && configured.kind == *kind {
                return Some(configured.hold);
            }
        }
        None
    }
}

fn holder_count(live: &[TaskRecord], name: &ResourceName) -> usize {
    let mut count = 0;
    for row in live {
        if row.holds_taken {
            for holding in &row.holdings {
                if holding_name(holding) == name {
                    count += 1;
                }
            }
        }
    }
    count
}

fn in_scope(live: &[TaskRecord], number: u64, hold: &jig_domain_tasks::PlacedHold) -> bool {
    let root = match hold.scope {
        jig_domain_tasks::Scope::Subtree { task } => task,
        jig_domain_tasks::Scope::Project => {
            return live.iter().any(|row| {
                row.number == number
                    && row.project == hold.project
                    && !(row.flags.conversation && matches!(row.requester, Party::Person(_)))
            });
        }
    };
    let mut at = number;
    for _ in 0..live.len() {
        if at == root {
            return true;
        }
        match live.iter().find(|row| row.number == at).map(|row| row.requester) {
            Some(Party::Task(parent)) => at = parent,
            _ => return false,
        }
    }
    false
}

fn placed(
    live: &[TaskRecord],
    holds: &[jig_domain_tasks::PlacedHold],
    stops: &BTreeSet<(u64, u64)>,
    parks: &BTreeSet<(u64, u64)>,
    draining: &BTreeSet<u64>,
    limits: &Limits,
    judge: &mut Judge<Name, Stimulus>,
) {
    use jig_domain_tasks::{Escalation, Hold, Owner};
    if holds.len() > limits.placed_holds as usize {
        judge.fail("placed hold table exceeds bound");
    }
    for row in live {
        if let Some(number) = row.placed {
            match holds.iter().find(|hold| hold.number == number) {
                Some(hold)
                    if row.project == hold.project
                        && !hold.exempt.contains(&row.number)
                        && (hold.lifted || in_scope(live, row.number, hold)) => {}
                _ => judge.fail("task names missing or noncovering placed hold"),
            }
            if !matches!(row.phase, Phase::Held { .. } | Phase::Closing(_)) && !draining.contains(&row.number) {
                judge.fail("placed task is not held or ending");
            }
        }
        if let Phase::Held { why: Hold::Placed { hold }, .. } = row.phase {
            if row.placed != Some(hold) {
                judge.fail("placed reason differs from task hold");
            }
            if !matches!(row.escalation, Escalation::Unheld { .. }) {
                judge.fail("placed reason escalated");
            }
        }
    }
    for hold in holds {
        if hold.exempt.len() > limits.exemptions as usize {
            judge.fail("exemptions exceed bound");
        }
        if hold.lifted && !live.iter().any(|row| row.placed == Some(hold.number)) {
            judge.fail("unreferenced lifted hold retained");
        }
        if holds.iter().filter(|other| other.project == hold.project).count() > limits.project_placed_holds as usize {
            judge.fail("project placed holds exceed bound");
        }
        if matches!(hold.owner, Owner::Party { .. })
            && holds.iter().filter(|other| other.owner == hold.owner).count() > limits.party_placed_holds as usize
        {
            judge.fail("party placed holds exceed bound");
        }
        if hold.lifted {
            continue;
        }
        for row in live.iter().filter(|row| in_scope(live, row.number, hold) && !hold.exempt.contains(&row.number)) {
            if row.placed.is_none() && !matches!(row.phase, Phase::Closing(_)) {
                judge.fail("standing hold missed a task");
            }
            let attempt = match row.phase {
                Phase::Active(jig_domain_tasks::Active::Claimed { attempt } | jig_domain_tasks::Active::Running { attempt }) => Some(attempt),
                Phase::Held {
                    was:
                        Was::Active(
                            jig_domain_tasks::Active::Claimed { attempt }
                            | jig_domain_tasks::Active::Running { attempt },
                        ),
                    ..
                } => Some(attempt),
                Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Held { .. } | Phase::Ended(_) => None,
            };
            if let Some(attempt) = attempt
                && !(match hold.mode {
                    jig_domain_tasks::Mode::Stop => stops.contains(&(row.number, attempt)),
                    jig_domain_tasks::Mode::Drain => parks.contains(&(row.number, attempt)) || stops.contains(&(row.number, attempt)),
                })
            {
                judge.fail("standing stop left a run uncancelled");
            }
        }
    }
}
