//! The oracle's one declaration of transient document observations.
//! Everything not named here remains durable, including derived inbox counts.
use jig_domain_core::{
    self as core,
    documents::{self as d, Document},
};
use jig_domain_fleet as fleet;
use jig_domain_tasks as tasks;
use skein_lib::{List, Wall};
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct DurableView(Document);

pub(super) struct Transients {
    engine_runs: BTreeMap<u64, u64>,
}
impl Transients {
    pub(super) fn from_core(core: &core::Core, workers_in_process: bool) -> Self {
        Self {
            engine_runs: core
                .proofs
                .iter()
                .filter_map(|(task, proof)| {
                    // engine.md 6: an in-process worker dies with the engine too.
                    if proof.host != fleet::HostKind::Engine && !workers_in_process {
                        return None;
                    }
                    Some((*task, core.tasks.task(*task)?.run_reserved))
                })
                .collect(),
        }
    }
    pub(super) fn durable(&self, document: &Document) -> DurableView {
        let mut document = document.clone();
        match &mut document {
            Document::Accounts(accounts) => {
                // accounts.md 5: the last sign-in terminal is an observation, not a stored account row.
                accounts.last_end = None;
            }
            Document::Task(task) => {
                let runtime = self.runtime(&task.chip);
                self.chip(&mut task.chip);
                self.chips(&mut task.lineage);
                for index in 0..task.delegates.len() {
                    self.chip(&mut task.delegates.get_mut(index).expect("delegate index").chip);
                }
                self.links(&mut task.inputs);
                self.links(&mut task.dependencies);
                self.links(&mut task.references);
                // engine.md 11: acts are current admission answers, recomputed from live host/capacity facts.
                task.acts = List::with_capacity(0);
                if let Some((reservation, open)) = runtime {
                    // engine.md 6 step 3: the local host dies; adoption releases its attempt and backs off.
                    task.run_wait = None;
                    task.activity = 0;
                    task.changed = Wall::EPOCH;
                    task.last_activity = Wall::EPOCH;
                    if let Some(settings) = &mut task.settings {
                        settings.run = None;
                    }
                    Self::budget(&mut task.budget, reservation, open);
                }
            }
            Document::Tree(tree) => {
                self.chip(&mut tree.root);
                for index in 0..tree.rows.len() {
                    let row = tree.rows.get_mut(index).expect("tree row index");
                    self.chip(&mut row.chip);
                    self.links(&mut row.dependencies);
                }
            }
            Document::Chips(chips) => self.chips(chips),
            Document::Project(project) => {
                // engine.md 6 step 3: project spend embeds the same local run
                // observations and own reservation as the task document.
                for index in 0..project.spend.len() {
                    let spend = project.spend.get_mut(index).expect("spend index");
                    if let Some((reservation, true)) = self.runtime(&spend.chip) {
                        spend.numbers.reserved = spend.numbers.reserved.checked_sub(reservation).expect("run held");
                    }
                    self.chip(&mut spend.chip);
                }
                for index in 0..project.recurring.len() {
                    self.chip(&mut project.recurring.get_mut(index).expect("recurring index").chip);
                }
            }
            Document::Inbox(inbox) => {
                for entries in [&mut inbox.needs, &mut inbox.updates] {
                    for index in 0..entries.len() {
                        let entry = entries.get_mut(index).expect("inbox entry index");
                        self.link(&mut entry.task);
                        if let Some(reply) = &mut entry.reply {
                            self.chip(&mut reply.chip);
                        }
                    }
                }
            }
            // Fields without an evidenced transient declaration stay in the comparison.
            Document::Frame(_)
            | Document::Run(_)
            | Document::Lines(_)
            | Document::Proposal(_)
            | Document::Escalation(_)
            | Document::Question(_)
            | Document::PersonTask(_)
            | Document::Result(_)
            | Document::Note(_)
            | Document::Deployment(_)
            | Document::Settings(_) => {}
        }
        DurableView(document)
    }
    fn chips(&self, chips: &mut List<d::Chip>) {
        for index in 0..chips.len() {
            self.chip(chips.get_mut(index).expect("chip index"));
        }
    }
    fn links(&self, links: &mut List<d::TaskLink>) {
        for index in 0..links.len() {
            self.link(links.get_mut(index).expect("link index"));
        }
    }
    fn link(&self, link: &mut d::TaskLink) {
        match link {
            d::TaskLink::Live(chip) | d::TaskLink::Retained { chip, .. } => self.chip(chip),
            d::TaskLink::Stored { .. } => {}
        }
    }
    fn runtime(&self, chip: &d::Chip) -> Option<(u64, bool)> {
        let reservation = *self.engine_runs.get(&chip.number)?;
        match chip.phase {
            d::PhaseLine::Active(tasks::Active::Claimed { .. } | tasks::Active::Running { .. }) => {
                Some((reservation, true))
            }
            d::PhaseLine::Active(tasks::Active::BackingOff { .. }) => Some((reservation, false)),
            d::PhaseLine::Waiting { .. }
            | d::PhaseLine::Active(_)
            | d::PhaseLine::Person { .. }
            | d::PhaseLine::Closing(_)
            | d::PhaseLine::Held { .. }
            | d::PhaseLine::Ended => None,
        }
    }
    fn chip(&self, chip: &mut d::Chip) {
        // engine.md 11 / hosts.md 6: receipt-time host observations are fenced volatile messages.
        chip.details.observation = None;
        if let Some((reservation, open)) = self.runtime(chip) {
            // engine.md 6 step 3: local claim loss changes runtime status and its release bookkeeping.
            chip.phase = d::PhaseLine::Active(tasks::Active::Idle);
            chip.details.phase_since = Wall::EPOCH;
            chip.details.run_since = None;
            chip.details.attempt = None;
            chip.details.retry = None;
            chip.details.activity = 0;
            Self::budget(&mut chip.details.budget, reservation, open);
        }
    }
    fn budget(budget: &mut d::BudgetLine, reservation: u64, open: bool) {
        // engine.md 6 step 3 / tasks run.rs: only the lost run's own reservation is released.
        // Delegates' durable reservations remain in the comparison.
        if open {
            budget.numbers.reserved = budget.numbers.reserved.checked_sub(reservation).expect("kept run reservation");
        }
        budget.left = budget
            .numbers
            .budget
            .saturating_sub(budget.numbers.spent)
            .saturating_sub(budget.numbers.spent_below)
            .saturating_sub(budget.numbers.reserved);
    }
}
