//! Indexed dependency, delegate and threshold queue waits (domain/tasks.md, 6.2, 6.6 and 6.7).
#![expect(clippy::result_large_err, reason = "bounded owner refusal carries exact capacity diagnostics inline")]

use crate::compound::Staged;
use crate::domain::{Domain, publish, record, task_mut};
use crate::{
    Circle, Hold, HoldKind, HoldSource, Holding, Limits, Name, New, Party, Phase, Problem, Refusal, Request, Was,
};
use alloc::boxed::Box;
use skein_lib::{Env, List, Map, Queue, Set};

/// Compare every wait lost at publication, including phase and delegate changes (domain/tasks.md, 6.7).
#[derive(Debug)]
pub(crate) struct Indexed {
    holdings: Box<[Holding]>,
    pending: Box<[u64]>,
    taken: bool,
    queued: bool,
    phase: WaitPhase,
    children: Box<[u64]>,
}

/// Ordered resource ranges and reversed dependency pairs, bounded by the live edge limits.
/// The names belong to the index, independently of the durable records (domain/tasks.md, 6.7).
#[derive(Debug)]
pub(crate) struct Index {
    pub holders: Map<(Name, u64), ()>,
    pub waiters: Map<(Name, u64), ()>,
    pub dependents: Map<(u64, u64), ()>,
    pub rows: Map<u64, Indexed>,
}

impl Index {
    pub fn new(limits: &Limits) -> Self {
        let resources = resource_capacity(limits).expect("validated resource edges");
        Self {
            holders: Map::with_capacity(resources),
            waiters: Map::with_capacity(resources),
            dependents: Map::with_capacity(limits.tasks.checked_mul(limits.dependencies).expect("dependency edges")),
            rows: Map::with_capacity(limits.tasks),
        }
    }
}

/// A root may retain the full tree's holdings after children end (6.5); every
/// live row may therefore own up to tasks * holdings, including on restart.
fn resource_capacity(limits: &Limits) -> Option<u32> {
    limits.tasks.checked_mul(limits.tasks)?.checked_mul(limits.holdings)
}

/// Index and traversal heap, including owned names and the simultaneous batch/restore scratch.
pub(crate) fn worst_case(limits: &Limits) -> Option<u64> {
    let resources = resource_capacity(limits)?;
    let dependencies = limits.tasks.checked_mul(limits.dependencies)?;
    let name = u64::from(limits.hold_bytes).checked_add(List::<Box<[u8]>>::worst_case(limits.hold_segments)?)?;
    Map::<(Name, u64), ()>::worst_case(resources)?
        .checked_mul(2)?
        .checked_add(Map::<(u64, u64), ()>::worst_case(dependencies)?)?
        .checked_add(Map::<u64, Indexed>::worst_case(limits.tasks)?)?
        .checked_add(u64::from(resources).checked_mul(
            name.checked_mul(4)?.checked_add(u64::try_from(size_of::<Holding>()).ok()?.checked_mul(2)?)?,
        )?)?
        .checked_add(u64::from(dependencies).checked_mul(16)?)?
        // Old/new delegate comparison rows, including a refresh's replacement.
        .checked_add(u64::from(limits.tasks).checked_mul(u64::from(limits.delegates))?.checked_mul(16)?)?
        // Changes being drained, changes made by lifts, and the knot's visited set.
        .checked_add(Set::<u64>::worst_case(limits.tasks)?.checked_mul(3)?)?
        // The judgement and a restore remainder; the knot's chosen wait per task.
        .checked_add(Map::<u64, u32>::worst_case(limits.tasks)?.checked_mul(2)?)?
        .checked_add(Map::<u64, u64>::worst_case(limits.tasks)?)?
        .checked_add(Map::<Name, u32>::worst_case(resources)?.checked_mul(2)?)?
        .checked_add(u64::from(resources).checked_mul(name)?.checked_mul(2)?)?
        .checked_add(List::<u64>::worst_case(limits.tasks)?.checked_mul(3)?)?
        .checked_add(Map::<(u64, u64), ()>::worst_case(dependencies)?)?
        .checked_add(Map::<(Name, u64), Gate>::worst_case(resources)?)?
        .checked_add(u64::from(resources).checked_mul(name)?)?
        .checked_add(List::<u64>::worst_case(limits.circle)?)?
        .checked_add(List::<(u64, u64)>::worst_case(dependencies)?)?
        .checked_add(List::<(Name, u64)>::worst_case(resources)?)?
        .checked_add(u64::from(resources).checked_mul(name)?)
}

pub(crate) fn refresh(domain: &mut Domain, number: u64) {
    let row = record(domain, number).expect("published live row");
    let phase = WaitPhase::for_phase(&row.phase);
    let queued = crate::holds::queued(row);
    if let Some(old) = domain.waits.rows.get(&number)
        && old.holdings == row.holdings
        && old.pending == row.waiting_on
        && old.taken == row.holds_taken
        && old.queued == queued
        && old.phase == phase
        && old.children == row.delegates
    {
        return;
    }
    let next = Indexed {
        holdings: row.holdings.clone(),
        pending: row.waiting_on.clone(),
        taken: row.holds_taken,
        queued,
        phase,
        children: row.delegates.clone(),
    };
    let old = domain.waits.rows.remove(&number);
    if let Some(old) = &old {
        let mut freed = old.phase.loses(next.phase) || (old.queued && !next.queued);
        for child in &old.children {
            freed |= !crate::batch::contains(&next.children, *child);
        }
        for dependency in &old.pending {
            if !crate::batch::contains(&next.pending, *dependency) {
                domain.waits.dependents.remove(&(*dependency, number));
                freed = true;
            }
        }
        for holding in &old.holdings {
            let key = (crate::holds::name(holding).clone(), number);
            if old.taken && (!next.taken || !has(&next.holdings, holding)) {
                domain.waits.holders.remove(&key);
                freed = true;
            }
            if old.queued && (!next.queued || !has(&next.holdings, holding)) {
                domain.waits.waiters.remove(&key);
                freed = true;
            }
        }
        if freed {
            domain.circle_changes.insert(number);
        }
    }
    for dependency in &next.pending {
        domain.waits.dependents.insert((*dependency, number), ()).expect("bounded dependency edges");
    }
    for holding in &next.holdings {
        let key = (crate::holds::name(holding).clone(), number);
        if next.taken {
            domain.waits.holders.insert(key.clone(), ()).expect("bounded held edges");
        }
        if next.queued {
            domain.waits.waiters.insert(key, ()).expect("bounded queued edges");
        }
    }
    domain.waits.rows.insert(number, next).expect("one indexed row per live task");
}

fn has(holdings: &[Holding], holding: &Holding) -> bool {
    for known in holdings {
        if crate::holds::same(known, holding) {
            return true;
        }
    }
    false
}

pub(crate) fn remove(domain: &mut Domain, number: u64) {
    if let Some(old) = domain.waits.rows.remove(&number) {
        for holding in &old.holdings {
            let key = (crate::holds::name(holding).clone(), number);
            domain.waits.holders.remove(&key);
            domain.waits.waiters.remove(&key);
        }
        for dependency in &old.pending {
            domain.waits.dependents.remove(&(*dependency, number));
        }
    }
    // The dependent publications that follow the end remove their own comparison entries.
    for _ in 0..domain.names.capacity() {
        let pair = match domain.waits.dependents.range((number, 0)..=(number, u64::MAX)).next() {
            Some((pair, ())) => *pair,
            None => break,
        };
        domain.waits.dependents.remove(&pair);
    }
    domain.circle_changes.insert(number);
}

fn threshold(domain: &Domain, holding: &Holding) -> Option<u32> {
    match crate::holds::rule(domain, holding) {
        Some(HoldKind::Exclusive { .. }) => Some(1),
        Some(HoldKind::Pooled { .. }) => Some(match domain.pools.get(crate::holds::name(holding)) {
            Some(pool) => pool.slots.max(1),
            None => 1,
        }),
        Some(HoldKind::Shared) | None => None,
    }
}

/// Changes may outnumber live rows during a cascade; saturation rejudges all holds (domain/tasks.md, 6.7).
#[derive(Debug)]
pub(crate) struct CircleChanges {
    tasks: Set<u64>,
    all: bool,
}

impl CircleChanges {
    pub(crate) fn new(limits: &Limits) -> Self {
        Self { tasks: Set::with_capacity(limits.tasks), all: false }
    }

    fn insert(&mut self, task: u64) {
        if self.tasks.insert(task).is_err() {
            self.all = true;
        }
    }
}

/// Only the joining task differs from the staged rows (domain/tasks.md, 6.7).
#[derive(Clone, Copy)]
struct Entry {
    task: u64,
    phase: WaitPhase,
    waiting: bool,
    creator: Party,
}

struct WaitState {
    requester: Party,
    phase: WaitPhase,
    queued: bool,
    taken: bool,
}

/// The rows one judgement reads, borrowed for that call and never kept, as
/// `compound::CompoundTask` is. Only the joining task's phase and lists differ
/// from its row (domain/tasks.md, 6.7).
struct View<'a> {
    domain: &'a Domain,
    staged: &'a Staged,
    entry: Entry,
    pending: &'a [u64],
    added: &'a [u64],
    batch: &'a [New],
}

/// One task's waits as a judgement sees them.
struct Row<'a> {
    waiting_on: &'a [u64],
    added: &'a [u64],
    delegates: &'a [u64],
    holdings: &'a [Holding],
    state: WaitState,
}

impl Row<'_> {
    fn waits_for(&self, dependency: u64) -> bool {
        self.state.phase.dependencies()
            && (crate::batch::contains(self.waiting_on, dependency) || crate::batch::contains(self.added, dependency))
    }
}

fn rows<'a>(view: &'a View<'_>, number: u64) -> Row<'a> {
    let View { domain, staged, entry, pending, added, batch } = view;
    if let Some(row) = staged.record(domain, number) {
        let (phase, waiting, pending, added) = if number == entry.task {
            (entry.phase, entry.waiting, *pending, *added)
        } else {
            (WaitPhase::for_phase(&row.phase), row.phase == Phase::Waiting, row.waiting_on, &[][..])
        };
        Row {
            waiting_on: pending,
            added,
            delegates: &row.base.delegates,
            holdings: &row.base.holdings,
            state: WaitState {
                requester: row.requester,
                phase,
                // A staged last dependency met joins the queue and gains its wait
                // start in this commit; a circling amendment is refused, not held
                // (domain/tasks.md, 6.2 and 6.7).
                queued: waiting && pending.is_empty() && added.is_empty() && !row.holds_taken,
                taken: row.holds_taken,
            },
        }
    } else {
        let row = member(batch, number);
        Row {
            waiting_on: &row.dependencies,
            added: &[],
            delegates: &[],
            holdings: &row.holdings,
            state: WaitState {
                requester: entry.creator,
                phase: WaitPhase::Tasks,
                queued: row.dependencies.is_empty(),
                taken: false,
            },
        }
    }
}

fn row_state(view: &View<'_>, number: u64) -> WaitState {
    rows(view, number).state
}

fn is_child(view: &View<'_>, parent: u64, number: u64) -> bool {
    let Row { delegates, state, .. } = rows(view, parent);
    state.phase.delegates()
        && (crate::batch::contains(delegates, number)
            || (view.entry.creator == Party::Task(parent) && member_named(view.batch, number)))
}

fn transferred(view: &View<'_>, number: u64, holding: &Holding) -> bool {
    view.entry.creator == Party::Task(number) && handed(view.batch, holding)
}

fn is_holder(view: &View<'_>, number: u64, holding: &Holding) -> bool {
    row_state(view, number).taken && !transferred(view, number, holding)
}

fn gate(view: &View<'_>, waiter: u64, holding: &Holding) -> bool {
    // A first member receiving the creator's hold takes it without a queue (6.4).
    for prior in view.batch {
        if prior.number == waiter {
            break;
        }
        if has(&prior.holdings, holding) {
            return true;
        }
    }
    match view.entry.creator {
        Party::Task(parent) => !row_state(view, parent).taken || !has(rows(view, parent).holdings, holding),
        Party::Person(_) | Party::Deployment { .. } => true,
    }
}

/// Mark reachability first, then eliminate within it at the resource thresholds (domain/tasks.md, 6.7).
struct Judge {
    pending: Map<u64, u32>,
    counts: Map<Name, u32>,
    stack: List<u64>,
}

impl Judge {
    fn new(limits: &Limits) -> Self {
        Self {
            pending: Map::with_capacity(limits.tasks),
            counts: Map::with_capacity(resource_capacity(limits).expect("resource count")),
            stack: List::with_capacity(limits.tasks),
        }
    }

    fn mark(&mut self, task: u64) {
        if self.pending.contains_key(&task) {
            return;
        }
        self.pending.insert(task, 0).expect("one mark per live task");
        self.stack.push(task).expect("one walk entry per live task");
    }

    fn met(&mut self, number: u64) {
        if let Some(count) = self.pending.get_mut(&number) {
            *count = count.checked_sub(1).expect("each wait removed once");
            if *count == 0 {
                self.stack.push(number).expect("one ready entry per task");
            }
        }
    }

    fn dependent(&mut self, view: &View<'_>, dependent: u64, dependency: u64) {
        if rows(view, dependent).waits_for(dependency) {
            self.mark(dependent);
        }
    }

    fn waiter(&mut self, view: &View<'_>, waiter: u64) {
        if row_state(view, waiter).queued {
            self.mark(waiter);
        }
    }

    fn walk(&mut self, view: &View<'_>, links: &Links, limits: &Limits) {
        self.mark(view.entry.task);
        // Batch members can also form a circle without their creator. Judge them whole.
        for member in view.batch {
            self.mark(member.number);
        }
        for at in 0..limits.tasks {
            let Some(&number) = self.stack.get(at) else { break };
            for ((_, dependent), ()) in view.domain.waits.dependents.range((number, 0)..=(number, u64::MAX)) {
                self.dependent(view, *dependent, number);
            }
            for (dependency, dependent) in links.added.as_slice() {
                if *dependency == number {
                    self.dependent(view, *dependent, number);
                }
            }
            let Row { holdings, state, .. } = rows(view, number);
            if let Party::Task(parent) = state.requester
                && is_child(view, parent, number)
            {
                self.mark(parent);
            }
            if !state.taken {
                continue;
            }
            for holding in holdings {
                if transferred(view, number, holding) || threshold(view.domain, holding).is_none() {
                    continue;
                }
                let name = crate::holds::name(holding);
                let count = self.counts.get(name).copied().unwrap_or(0).saturating_add(1);
                self.counts.insert(name.clone(), count).expect("bounded resource counts");
                if count != 1 {
                    continue;
                }
                for ((_, waiter), ()) in view.domain.waits.waiters.range((name.clone(), 0)..=(name.clone(), u64::MAX)) {
                    self.waiter(view, *waiter);
                }
                for (resource, waiter) in links.queued.as_slice() {
                    if resource == name {
                        self.waiter(view, *waiter);
                    }
                }
            }
        }
    }

    fn waits(&self, view: &View<'_>, number: u64, gates: &Map<(Name, u64), Gate>) -> u32 {
        let Row { waiting_on, added, delegates, holdings, state } = rows(view, number);
        let mut pending = 0_u32;
        if state.phase.dependencies() {
            // Count the union, as Row::waits_for does when an edge is removed.
            for (at, list) in [waiting_on, added].into_iter().enumerate() {
                for dependency in list {
                    if at == 1 && crate::batch::contains(waiting_on, *dependency) {
                        continue;
                    }
                    if self.pending.contains_key(dependency) {
                        pending = pending.checked_add(1).expect("bounded waits");
                    }
                }
            }
        }
        if state.phase.delegates() {
            for child in delegates {
                if self.pending.contains_key(child) && row_state(view, *child).requester == Party::Task(number) {
                    pending = pending.checked_add(1).expect("bounded waits");
                }
            }
            if view.entry.creator == Party::Task(number) {
                pending =
                    pending.checked_add(u32::try_from(view.batch.len()).expect("batch bound")).expect("bounded waits");
            }
        }
        if state.queued {
            for holding in holdings {
                if self.stuck(view, number, holding, gates) {
                    pending = pending.checked_add(1).expect("bounded waits");
                }
            }
        }
        pending
    }

    fn stuck(&self, view: &View<'_>, number: u64, holding: &Holding, gates: &Map<(Name, u64), Gate>) -> bool {
        let Some(threshold) = threshold(view.domain, holding) else { return false };
        let name = crate::holds::name(holding);
        if member_named(view.batch, number) {
            match gates.get(&(name.clone(), number)) {
                Some(gate) => gate.count >= gate.threshold,
                None => false,
            }
        } else {
            self.counts.get(name).copied().unwrap_or(0) >= threshold
        }
    }

    fn eliminate(&mut self, view: &View<'_>, links: &Links, gates: &mut Map<(Name, u64), Gate>, limits: &Limits) {
        // Reuse the marking queue to initialize counts without changing membership.
        for at in 0..self.stack.len() {
            let number = *self.stack.get(at).expect("marked task");
            let count = self.waits(view, number, gates);
            *self.pending.get_mut(&number).expect("marked task") = count;
        }
        self.stack.clear();
        for (number, count) in &self.pending {
            if *count == 0 {
                self.stack.push(*number).expect("one ready entry per task");
            }
        }
        for at in 0..limits.tasks {
            let Some(&number) = self.stack.get(at) else { break };
            self.pending.remove(&number);
            for ((_, dependent), ()) in view.domain.waits.dependents.range((number, 0)..=(number, u64::MAX)) {
                if rows(view, *dependent).waits_for(number) {
                    self.met(*dependent);
                }
            }
            for (dependency, dependent) in links.added.as_slice() {
                if *dependency == number && rows(view, *dependent).waits_for(number) {
                    self.met(*dependent);
                }
            }
            let Row { holdings, state, .. } = rows(view, number);
            if let Party::Task(parent) = state.requester
                && is_child(view, parent, number)
            {
                self.met(parent);
            }
            if state.taken {
                for holding in holdings {
                    if !transferred(view, number, holding) {
                        self.free_resource(view, links, gates, holding);
                    }
                }
            }
            if member_named(view.batch, number) {
                let mut after = false;
                for later in view.batch {
                    if later.number == number {
                        after = true;
                        continue;
                    }
                    if after {
                        for holding in holdings {
                            if let Some(gate) = gates.get_mut(&(crate::holds::name(holding).clone(), later.number)) {
                                let before = gate.count;
                                gate.count = gate.count.checked_sub(1).expect("earlier member counted");
                                if before == gate.threshold {
                                    self.met(later.number);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn free_resource(&mut self, view: &View<'_>, links: &Links, gates: &mut Map<(Name, u64), Gate>, holding: &Holding) {
        let Some(threshold) = threshold(view.domain, holding) else { return };
        let name = crate::holds::name(holding);
        let count = self.counts.get_mut(name).expect("marked resource holder");
        let before = *count;
        *count = count.checked_sub(1).expect("holder counted once");
        if before == threshold {
            for ((_, waiter), ()) in view.domain.waits.waiters.range((name.clone(), 0)..=(name.clone(), u64::MAX)) {
                if row_state(view, *waiter).queued {
                    self.met(*waiter);
                }
            }
            for (resource, waiter) in links.queued.as_slice() {
                if resource == name && !member_named(view.batch, *waiter) && row_state(view, *waiter).queued {
                    self.met(*waiter);
                }
            }
        }
        for member in view.batch {
            if let Some(gate) = gates.get_mut(&(name.clone(), member.number)) {
                let before = gate.count;
                gate.count = gate.count.checked_sub(1).expect("live holder counted");
                if before == gate.threshold {
                    self.met(member.number);
                }
            }
        }
    }

    /// Choose the first stuck wait's ordinal: dependencies, delegates, then resources (6.7).
    fn choice(&self, view: &View<'_>, gates: &Map<(Name, u64), Gate>, number: u64) -> u64 {
        let Row { waiting_on, added, delegates, holdings, state } = rows(view, number);
        let mut at = 0_u64;
        for list in [waiting_on, added] {
            for dependency in list {
                if state.phase.dependencies() && self.pending.contains_key(dependency) {
                    return at;
                }
                at = at.checked_add(1).expect("bounded waits");
            }
        }
        for child in delegates {
            if is_child(view, number, *child)
                && self.pending.contains_key(child)
                && row_state(view, *child).requester == Party::Task(number)
            {
                return at;
            }
            at = at.checked_add(1).expect("bounded waits");
        }
        if view.entry.creator == Party::Task(number) {
            for member in view.batch {
                if state.phase.delegates() && self.pending.contains_key(&member.number) {
                    return at;
                }
                at = at.checked_add(1).expect("bounded waits");
            }
        }
        for holding in holdings {
            if state.queued && self.stuck(view, number, holding, gates) {
                return at;
            }
            at = at.checked_add(1).expect("bounded waits");
        }
        unreachable!("each remaining task has a stuck wait")
    }

    fn problem(&self, view: &View<'_>, gates: &Map<(Name, u64), Gate>, limits: &Limits, task: u64) -> Problem {
        let mut choices = Map::with_capacity(limits.tasks);
        for (number, _) in &self.pending {
            let choice = self.choice(view, gates, *number);
            choices.insert(*number, self.first(view, *number, choice)).expect("one chosen witness per remaining task");
        }
        let mut knot =
            Knot { seen: Set::with_capacity(limits.tasks), tasks: List::with_capacity(limits.circle), cut: false };
        let mut root = task;
        if !view.batch.is_empty() {
            // A whole batch can leave a creator or member upstream of its knot.
            // Following the first witnesses removes that prefix before breadth-first naming.
            for _ in 0..=limits.tasks {
                if !knot.seen.insert(root).expect("bounded remaining tasks") {
                    break;
                }
                root = *choices.get(&root).expect("remaining witness");
            }
            knot.seen = Set::with_capacity(limits.tasks);
        }
        knot.visit(root);
        for at in 0..limits.circle {
            let Some(&number) = knot.tasks.get(at) else { break };
            self.name_wait(view, number, self.choice(view, gates, number), &mut knot);
            if knot.cut {
                break;
            }
        }
        Problem::naming(Some(root), Refusal::Circle { cut: knot.cut }, knot.tasks.as_slice())
    }

    fn first(&self, view: &View<'_>, number: u64, choice: u64) -> u64 {
        let at = match witness(view, number, choice) {
            Witness::Task(task) => return task,
            Witness::Resource(at) => at,
        };
        let holding =
            rows(view, number).holdings.get(usize::try_from(at).expect("resource ordinal")).expect("chosen resource");
        let name = crate::holds::name(holding);
        if view.batch.is_empty() {
            for ((_, holder), ()) in view.domain.waits.holders.range((name.clone(), 0)..=(name.clone(), u64::MAX)) {
                if self.pending.contains_key(holder) {
                    return *holder;
                }
            }
        } else {
            for (holder, _) in &self.pending {
                if witnesses_resource(view, number, *holder, holding) {
                    return *holder;
                }
            }
        }
        unreachable!("stuck resource has a remaining holder")
    }

    fn name_wait(&self, view: &View<'_>, number: u64, choice: u64, knot: &mut Knot) {
        let at = match witness(view, number, choice) {
            Witness::Task(task) => {
                knot.visit(task);
                return;
            }
            Witness::Resource(at) => at,
        };
        let holdings = rows(view, number).holdings;
        let holding = holdings.get(usize::try_from(at).expect("bounded resource ordinal")).expect("chosen resource");
        let mut left = threshold(view.domain, holding).expect("stuck resource");
        let name = crate::holds::name(holding);
        if view.batch.is_empty() {
            for ((_, holder), ()) in view.domain.waits.holders.range((name.clone(), 0)..=(name.clone(), u64::MAX)) {
                if self.pending.contains_key(holder) && is_holder(view, *holder, holding) {
                    knot.visit(*holder);
                    left = left.checked_sub(1).expect("one resource witness");
                    if left == 0 || knot.cut {
                        return;
                    }
                }
            }
        } else {
            // Live holders and earlier member reservations share number order.
            for (holder, _) in &self.pending {
                if witnesses_resource(view, number, *holder, holding) {
                    knot.visit(*holder);
                    left = left.checked_sub(1).expect("one resource witness");
                    if left == 0 || knot.cut {
                        return;
                    }
                }
            }
        }
        unreachable!("stuck resource has its threshold of holders left")
    }
}

fn witness(view: &View<'_>, number: u64, mut choice: u64) -> Witness {
    let Row { waiting_on, added, delegates, .. } = rows(view, number);
    for list in [waiting_on, added, delegates] {
        for witness in list {
            if choice == 0 {
                return Witness::Task(*witness);
            }
            choice = choice.checked_sub(1).expect("chosen later wait");
        }
    }
    if view.entry.creator == Party::Task(number) {
        for member in view.batch {
            if choice == 0 {
                return Witness::Task(member.number);
            }
            choice = choice.checked_sub(1).expect("chosen later wait");
        }
    }
    Witness::Resource(u32::try_from(choice).expect("bounded resource ordinal"))
}

enum Witness {
    Task(u64),
    Resource(u32),
}

fn witnesses_resource(view: &View<'_>, waiter: u64, number: u64, holding: &Holding) -> bool {
    if is_holder(view, number, holding) && has(rows(view, number).holdings, holding) {
        return true;
    }
    if member_named(view.batch, waiter) {
        for prior in view.batch {
            if prior.number == waiter {
                break;
            }
            if prior.number == number && has(&prior.holdings, holding) {
                return true;
            }
        }
    }
    false
}

/// Breadth-first names stop at the configured bound; revisits never spend a name (domain/tasks.md, 6.7).
struct Knot {
    seen: Set<u64>,
    tasks: List<u64>,
    cut: bool,
}

impl Knot {
    fn visit(&mut self, number: u64) {
        if self.seen.insert(number).expect("bounded remaining tasks") && self.tasks.push(number).is_err() {
            self.cut = true;
        }
    }
}

/// Missing staged reverse edges are built once, then shared by marking and elimination (domain/tasks.md, 6.7).
struct Links {
    added: List<(u64, u64)>,
    queued: List<(Name, u64)>,
}

impl Links {
    fn new(view: &View<'_>, limits: &Limits) -> Self {
        let mut links = Self {
            added: List::with_capacity(limits.tasks.checked_mul(limits.dependencies).expect("dependency edges")),
            queued: List::with_capacity(resource_capacity(limits).expect("resource edges")),
        };
        for (number, _) in &view.domain.names {
            links.row(view, *number);
        }
        for member in view.batch {
            links.row(view, member.number);
        }
        links
    }

    fn row(&mut self, view: &View<'_>, number: u64) {
        let Row { waiting_on, added, holdings, state, .. } = rows(view, number);
        for (at, list) in [waiting_on, added].into_iter().enumerate() {
            for dependency in list {
                if at == 1 && crate::batch::contains(waiting_on, *dependency) {
                    continue;
                }
                // Indexed pairs are visited there, never a second time through added.
                if !view.domain.waits.dependents.contains_key(&(*dependency, number)) {
                    self.added.push((*dependency, number)).expect("bounded projected dependencies");
                }
            }
        }
        if state.queued {
            for holding in holdings {
                let pair = (crate::holds::name(holding).clone(), number);
                if !view.domain.waits.waiters.contains_key(&pair) {
                    self.queued.push(pair).expect("bounded projected queues");
                }
            }
        }
    }
}

fn handed(batch: &[New], holding: &Holding) -> bool {
    for member in batch {
        if has(&member.holdings, holding) {
            return true;
        }
    }
    false
}

fn batch_gates(view: &View<'_>, judge: &Judge, limits: &Limits) -> Map<(Name, u64), Gate> {
    let members = u32::try_from(view.batch.len()).expect("admitted batch bound");
    let mut gates = Map::with_capacity(members.checked_mul(limits.holdings).expect("batch gates"));
    for member in view.batch {
        if !row_state(view, member.number).queued {
            continue;
        }
        for holding in &member.holdings {
            let Some(threshold) = threshold(view.domain, holding) else { continue };
            if !gate(view, member.number, holding) {
                continue;
            }
            let name = crate::holds::name(holding);
            let mut count = judge.counts.get(name).copied().unwrap_or(0);
            for prior in view.batch {
                if prior.number == member.number {
                    break;
                }
                if has(&prior.holdings, holding) {
                    count = count.saturating_add(1);
                }
            }
            gates.insert((name.clone(), member.number), Gate { count, threshold }).expect("bounded batch gates");
        }
    }
    gates
}

fn judge(view: &View<'_>, limits: &Limits) -> Result<(), Problem> {
    let links = Links::new(view, limits);
    let mut judge = Judge::new(limits);
    judge.walk(view, &links, limits);
    let mut gates = batch_gates(view, &judge, limits);
    // Phase B can end an already-ready source immediately. A batch also judges
    // its members, which can circle without their creator (domain/tasks.md, 6.7).
    if view.batch.is_empty() && judge.waits(view, view.entry.task, &gates) == 0 {
        return Ok(());
    }
    judge.eliminate(view, &links, &mut gates, limits);
    if judge.pending.contains_key(&view.entry.task) {
        return Err(judge.problem(view, &gates, limits, view.entry.task));
    }
    // A batch can contain an internal circle while its creator is held or absent.
    for member in view.batch {
        if judge.pending.contains_key(&member.number) {
            return Err(judge.problem(view, &gates, limits, member.number));
        }
    }
    Ok(())
}

/// Judge new dependencies with every wait they join (domain/tasks.md, 6.7).
pub(crate) fn check_added(
    domain: &Domain,
    staged: &Staged,
    limits: &Limits,
    task: u64,
    added: &[u64],
) -> Result<(), Problem> {
    let row = staged.record(domain, task).expect("amended task");
    if added.is_empty() || !dependencies_wait(&row.phase) {
        return Ok(());
    }
    judge(
        &View {
            domain,
            staged,
            entry: Entry {
                task,
                phase: WaitPhase::for_phase(&row.phase),
                waiting: row.phase == Phase::Waiting,
                creator: Party::Person(0),
            },
            pending: &[],
            added,
            batch: &[],
        },
        limits,
    )
}

/// Judge all queues a waiting or released task would join together (domain/tasks.md, 6.7).
pub(crate) fn check_queue(domain: &Domain, staged: &Staged, limits: &Limits, task: u64) -> Result<(), Problem> {
    check_waits(domain, staged, limits, task, &Phase::Waiting, &[])
}

/// Leave a circling queue at its saved place, publishing the ordinary held notice (5.5 and 6.7).
pub(crate) fn hold_circle(
    domain: &mut Domain,
    env: &Env<Limits>,
    number: u64,
    problem: Problem,
    out: &mut Queue<Request>,
) {
    let cut = circle_cut(&problem.why);
    let tasks = problem.blocked_by.expect("circle names its tasks");
    let row = &mut task_mut(domain, number).expect("circling live task").record;
    let was = match &row.phase {
        Phase::Held { was, .. } => was.clone(),
        Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Ended(_) => crate::run::was(row.phase.clone()),
    };
    row.phase =
        Phase::Held { was, why: Hold::Circle, source: Some(Box::new(HoldSource::Circle(Circle { tasks, cut }))) };
    crate::holds::leave(domain, number);
    publish(domain, env, number, out);
}

/// Rejudge affected circle holds outside publication; a cut circle is always reconsidered.
pub(crate) fn lift(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) -> bool {
    if !domain.circle_changes.all && domain.circle_changes.tasks.is_empty() {
        return false;
    }
    let changes = core::mem::replace(&mut domain.circle_changes, CircleChanges::new(&env.limits));
    let numbers = crate::domain::snapshot(domain, env.limits.tasks);
    let mut lifted = false;
    for number in numbers.into_boxed() {
        let Some(row) = record(domain, number) else { continue };
        let Some(circle) = circle_held(&row.phase) else { continue };
        let mut affected = circle.cut || changes.all;
        for task in &circle.tasks {
            affected |= changes.tasks.contains(task);
        }
        if !affected {
            continue;
        }
        let resume = match &row.phase {
            Phase::Held { was: Was::Waiting, .. } => Phase::Waiting,
            Phase::Held { was: Was::Active(active), .. } => Phase::Active(*active),
            Phase::Held { was: Was::Closing(closing), .. } => Phase::Closing(closing.clone()),
            Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Ended(_) => unreachable!("circle hold"),
        };
        let resume = crate::run::ends::for_record(row).amended(row.executor, resume);
        match check_waits(domain, &Staged::new(0), &env.limits, number, &resume, &row.waiting_on) {
            Ok(()) => {
                task_mut(domain, number).expect("circle held task").record.phase = resume;
                if crate::holds::queued(record(domain, number).expect("lifted task")) {
                    crate::holds::back(domain, env, number, out);
                } else {
                    publish(domain, env, number, out);
                    if record(domain, number).expect("lifted task").phase == Phase::Active(crate::Active::Due) {
                        crate::domain::activate(domain, number, out);
                    }
                }
                lifted = true;
            }
            Err(problem) => {
                let cut = circle_cut(&problem.why);
                if circle.cut != cut || problem.blocked_by.as_deref() != Some(circle.tasks.as_ref()) {
                    hold_circle(domain, env, number, problem, out);
                }
            }
        }
    }
    lifted
}

#[derive(Debug)]
struct Gate {
    count: u32,
    threshold: u32,
}

pub(crate) fn pool_changed(domain: &mut Domain, _limits: &Limits) {
    domain.circle_changes.all = true;
}

/// Judge the restored set whole; hold its earliest circling queue waiter (domain/tasks.md, 6.7).
pub(crate) fn restore(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) -> bool {
    for _ in 0..=env.limits.tasks {
        let remaining = eliminate(domain, &env.limits);
        if remaining.is_empty() {
            return true;
        }
        let mut earliest = None;
        for (number, _) in &remaining {
            let row = record(domain, *number).expect("remaining restored task");
            if !crate::holds::queued(row) {
                continue;
            }
            if let Err(problem) = check_queue(domain, &Staged::new(0), &env.limits, *number) {
                match &earliest {
                    Some((old, _)) if crate::holds::earlier(domain, *old, *number, &env.limits) => {}
                    Some(_) | None => earliest = Some((*number, problem)),
                }
            }
        }
        let Some((number, problem)) = earliest else { return false };
        hold_circle(domain, env, number, problem, out);
    }
    false
}

fn eliminate(domain: &Domain, limits: &Limits) -> Map<u64, u32> {
    let Some((&task, _)) = domain.names.first() else { return Map::with_capacity(limits.tasks) };
    let staged = Staged::new(0);
    let row = record(domain, task).expect("live task");
    let view = View {
        domain,
        staged: &staged,
        entry: Entry {
            task,
            phase: WaitPhase::for_phase(&row.phase),
            waiting: row.phase == Phase::Waiting,
            creator: Party::Person(0),
        },
        pending: row.waiting_on.as_ref(),
        added: &[],
        batch: &[],
    };
    let links = Links::new(&view, limits);
    let mut judge = Judge::new(limits);
    for (number, _) in &domain.names {
        judge.mark(*number);
    }
    for ((name, _), ()) in &domain.waits.holders {
        let count = judge.counts.get(name).copied().unwrap_or(0).saturating_add(1);
        judge.counts.insert(name.clone(), count).expect("bounded resources");
    }
    judge.eliminate(&view, &links, &mut Map::with_capacity(0), limits);
    judge.pending
}

/// Judge members, their ordered reservations and their creator's new wait together (domain/tasks.md, 6.7).
pub(crate) fn check_batch(domain: &Domain, limits: &Limits, creator: Party, batch: &[New]) -> Result<(), Problem> {
    let staged = Staged::new(0);
    let task = match creator {
        Party::Task(parent) => parent,
        Party::Person(_) | Party::Deployment { .. } => batch.first().expect("nonempty admitted batch").number,
    };
    let (phase, pending) = match record(domain, task) {
        Some(row) => (&row.phase, row.waiting_on.as_ref()),
        None => (&Phase::Waiting, &[][..]),
    };
    judge(
        &View {
            domain,
            staged: &staged,
            entry: Entry { task, phase: WaitPhase::for_phase(phase), waiting: *phase == Phase::Waiting, creator },
            pending,
            added: &[],
            batch,
        },
        limits,
    )
}

fn member_named(batch: &[New], number: u64) -> bool {
    for member in batch {
        if member.number == number {
            return true;
        }
    }
    false
}

fn member(batch: &[New], number: u64) -> &New {
    for member in batch {
        if member.number == number {
            return member;
        }
    }
    unreachable!("admitted batch member")
}

fn dependencies_wait(phase: &Phase) -> bool {
    match phase {
        Phase::Waiting | Phase::Active(_) => true,
        Phase::Closing(_) | Phase::Held { .. } | Phase::Ended(_) => false,
    }
}

/// Tasks wait on dependencies and delegates, delegates alone, or no task (domain/tasks.md, 6.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WaitPhase {
    Tasks,
    Delegates,
    None,
}

impl WaitPhase {
    fn for_phase(phase: &Phase) -> Self {
        match phase {
            Phase::Waiting | Phase::Active(_) => Self::Tasks,
            Phase::Closing(_) => Self::Delegates,
            Phase::Held { .. } | Phase::Ended(_) => Self::None,
        }
    }

    fn dependencies(self) -> bool {
        match self {
            Self::Tasks => true,
            Self::Delegates | Self::None => false,
        }
    }

    fn delegates(self) -> bool {
        match self {
            Self::Tasks | Self::Delegates => true,
            Self::None => false,
        }
    }

    fn loses(self, next: Self) -> bool {
        (self.dependencies() && !next.dependencies()) || (self.delegates() && !next.delegates())
    }
}

fn circle_held(phase: &Phase) -> Option<&Circle> {
    match phase {
        Phase::Held { why: Hold::Circle, source: Some(source), .. } => match source.as_ref() {
            HoldSource::Circle(circle) => Some(circle),
            HoldSource::Resource(_) | HoldSource::Effect { .. } => None,
        },
        Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Held { .. } | Phase::Ended(_) => None,
    }
}

fn circle_cut(why: &Refusal) -> bool {
    match why {
        Refusal::Circle { cut } => *cut,
        Refusal::Account { .. }
        | Refusal::Grace
        | Refusal::Goals
        | Refusal::Unproposable
        | Refusal::PersonRevision { .. }
        | Refusal::ResourceUnavailable
        | Refusal::NotReady
        | Refusal::Unknown
        | Refusal::Busy
        | Refusal::Duplicate
        | Refusal::Empty
        | Refusal::Batch
        | Refusal::Live
        | Refusal::Project
        | Refusal::Tree
        | Refusal::Depth
        | Refusal::Delegates
        | Refusal::Reference
        | Refusal::Subscription
        | Refusal::Dependencies
        | Refusal::DependencyEnded { .. }
        | Refusal::KeptQuestion { .. }
        | Refusal::PersonAnswers { .. }
        | Refusal::Kept
        | Refusal::Executor
        | Refusal::Spec
        | Refusal::Contract
        | Refusal::AuthorityShape
        | Refusal::Inputs
        | Refusal::State
        | Refusal::Attempt
        | Refusal::LiveDelegates
        | Refusal::Restore
        | Refusal::Read
        | Refusal::Turn
        | Refusal::Funding
        | Refusal::HoldKind
        | Refusal::HoldTaken
        | Refusal::Holds
        | Refusal::Placed { .. }
        | Refusal::HoldOwner { .. }
        | Refusal::HoldRevision { .. }
        | Refusal::Llm { .. } => unreachable!("circle judge"),
    }
}

/// A release also restores dependency and delegation waits before it may go on (6.7).
pub(crate) fn check_return(domain: &Domain, staged: &Staged, limits: &Limits, task: u64) -> Result<(), Problem> {
    let row = staged.record(domain, task).expect("released live row");
    check_waits(domain, staged, limits, task, &row.phase, row.waiting_on)
}

pub(crate) fn check_waits(
    domain: &Domain,
    staged: &Staged,
    limits: &Limits,
    task: u64,
    phase: &Phase,
    pending: &[u64],
) -> Result<(), Problem> {
    judge(
        &View {
            domain,
            staged,
            entry: Entry {
                task,
                phase: WaitPhase::for_phase(phase),
                waiting: *phase == Phase::Waiting,
                creator: Party::Person(0),
            },
            pending,
            added: &[],
            batch: &[],
        },
        limits,
    )
}

#[cfg(test)]
mod tests {
    use super::CircleChanges;
    use skein_lib::Set;

    // No world event is known to exceed the live bound between progress drains.
    // Exercise saturation directly: duplicate changes at capacity and one beyond (6.7).
    #[test]
    fn circle_changes_saturate_without_losing_the_need_to_rejudge() {
        for capacity in [0, 1, 2] {
            let mut changes = CircleChanges { tasks: Set::with_capacity(capacity), all: false };
            for number in 0..capacity {
                changes.insert(u64::from(number));
                changes.insert(u64::from(number));
            }
            assert!(!changes.all, "at capacity {capacity}, including duplicates");
            changes.insert(u64::from(capacity));
            assert!(changes.all, "one beyond capacity {capacity}");
            assert_eq!(changes.tasks.len(), capacity);
            changes.insert(u64::MAX);
            assert!(changes.all);
            assert_eq!(changes.tasks.len(), capacity);
        }
    }
}
