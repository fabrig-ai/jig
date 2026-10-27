//! Revisions explicitly named at peer boundaries and in durable procedure steps.
use super::{core, people, r, tasks};
use jig_test_connector as connector;
use std::collections::{BTreeMap, BTreeSet};

fn named(task: u64, revision: r::Revision) -> r::Observed {
    r::Observed::Named { task, revision }
}

pub(super) fn party(ask: &people::Ask) -> Vec<r::Observed> {
    if let people::Ask::Amend { task, revision, release, amendment, .. } = ask {
        let mut observed = spec(*task, *revision, party_priority(amendment));
        observed.extend(release.map(|r| named(*task, r::Revision::Hold(r))));
        return observed;
    }
    if let people::Ask::Release { task, revision, .. } | people::Ask::DecideEscalation { task, revision, .. } = ask {
        return vec![named(*task, r::Revision::Hold(*revision))];
    }
    if let people::Ask::Propose { action: people::Action::Amend { task, revision, amendment }, .. } = ask {
        return spec(*task, *revision, party_priority(amendment));
    }
    if let people::Ask::Compound { operations, .. } = ask {
        return operations.iter().flat_map(operation).collect();
    }
    Vec::new()
}

fn operation(operation: &people::Operation) -> Vec<r::Observed> {
    if let people::Operation::Amend { task, revision, release, amendment } = operation {
        let mut observed = spec(*task, *revision, party_priority(amendment));
        observed.extend(release.map(|r| named(*task, r::Revision::Hold(r))));
        return observed;
    }
    if let people::Operation::Release { task, revision, .. } = operation {
        return vec![named(*task, r::Revision::Hold(*revision))];
    }
    Vec::new()
}

pub(super) fn run(event: &core::Event) -> Vec<r::Observed> {
    if let core::Event::NamedAction { action, .. } = event {
        if let core::NamedAction::Amend { target, revision, amendment } = action {
            let mut observed = spec(*target, *revision, run_priority(amendment));
            observed.extend(amendment.release.map(|r| named(*target, r::Revision::Hold(r))));
            return observed;
        }
        if let core::NamedAction::Release { target, revision, .. }
        | core::NamedAction::DecideEscalation { task: target, revision, .. } = action
        {
            return vec![named(*target, r::Revision::Hold(*revision))];
        }
    }
    Vec::new()
}

// Priority alone names no specification; it cannot fund a later amendment.
fn spec(task: u64, revision: u64, priority: bool) -> Vec<r::Observed> {
    if priority { Vec::new() } else { vec![named(task, r::Revision::Spec(revision))] }
}

fn party_priority(amendment: &people::Amendment) -> bool {
    matches!(amendment.tracked, Some(people::Tracking::Tracked { .. }))
        && amendment.addressed.is_none()
        && amendment.instructions.is_none()
        && amendment.spec.is_none()
        && amendment.wake.is_none()
        && amendment.dependencies.is_none()
        && amendment.authority.is_none()
}

fn run_priority(amendment: &tasks::Amendment) -> bool {
    matches!(amendment.tracked, Some(tasks::Tracking::Tracked { .. }))
        && amendment.addressed.is_none()
        && amendment.instructions.is_none()
        && amendment.spec.is_none()
        && amendment.wake.is_none()
        && amendment.dependencies.is_none()
        && amendment.authority.is_none()
}

#[derive(Clone, Debug, Default)]
pub(super) struct Names {
    pending: BTreeMap<u64, Vec<(u16, connector::ProcedureState)>>,
    programs: BTreeMap<(u16, u16), Box<[connector::ProcedureAction]>>,
    seen: BTreeSet<(u16, u64, u16)>,
}

impl Names {
    pub(super) fn configured(configuration: &jig_test_domain::Config) -> Self {
        let mut names = Self::default();
        for (number, config) in [(1, &configuration.first), (2, &configuration.second)] {
            for spec in &config.procedures {
                names.programs.insert((number, spec.number), spec.actions.clone());
            }
        }
        names
    }

    pub(super) fn submitted(&mut self, store: &crate::Store) {
        if let Some(commit) = store.pending.back() {
            let states = commit
                .writes
                .iter()
                .filter_map(|write| {
                    if let jig_test_domain::Write::Save(jig_test_domain::Record::Connector {
                        number,
                        record: connector::Record::Procedure(state),
                    }) = write
                    {
                        return Some((*number, state.clone()));
                    }
                    None
                })
                .collect();
            self.pending.insert(commit.number, states);
        }
    }

    pub(super) fn durable(&mut self, applied: u64) -> Vec<r::Observed> {
        let mut observed = Vec::new();
        while let Some((&number, _)) = self.pending.first_key_value() {
            if number > applied {
                break;
            }
            for (connector, state) in self.pending.remove(&number).expect("pending commit") {
                observed.extend(self.procedure(connector, &state));
            }
        }
        observed
    }

    fn procedure(&mut self, number: u16, state: &connector::ProcedureState) -> Vec<r::Observed> {
        if state.awaiting
            && let Some(task) = state.delegate
            && let Some(actions) = self.programs.get(&(number, state.number))
            && let Some(connector::ProcedureAction::Amend { priority, .. }) = actions.get(usize::from(state.index))
            && self.seen.insert((number, state.task, state.steps))
        {
            return spec(task, state.revision, priority.is_some());
        }
        Vec::new()
    }
}

// A placed-hold revision names releases of the tasks that record currently holds.
// Derive these names from the peer's explicit ask and the previous durable rows.
pub(super) fn placed_party(store: &crate::Store, ask: &people::Ask) -> Vec<r::Observed> {
    if let people::Ask::Lift { hold, revision, with_tasks: true } = ask {
        return placed_names(store, *hold, *revision, None);
    }
    if let people::Ask::DecideProposal {
        proposer: people::Proposer::Task(proposer),
        proposal,
        decision: people::ProposalDecision::Accept,
        ..
    } = ask
    {
        return proposal_names(store, *proposer, *proposal);
    }
    Vec::new()
}
pub(super) fn placed_run(store: &crate::Store, event: &core::Event) -> Vec<r::Observed> {
    if let core::Event::NamedAction {
        action: core::NamedAction::DecideProposal { proposer, proposal, choice: core::ProposalChoice::Accept },
        ..
    } = event
    {
        proposal_names(store, *proposer, *proposal)
    } else {
        Vec::new()
    }
}
fn proposal_names(store: &crate::Store, proposer: u64, proposal: u64) -> Vec<r::Observed> {
    let key = jig_test_domain::Key::Core(core::Key::Tasks(tasks::Key::Live(proposer)));
    if let Some(jig_test_domain::Record::Core(core::Record::Tasks(tasks::Stored::Live(row)))) = store.rows.get(&key)
        && let Some(p) = &row.proposal
        && p.number == proposal
        && let tasks::ProposalAction::ReleasePlaced { task, hold, revision } = p.action
    {
        return placed_names(store, hold, revision, Some(task));
    }
    Vec::new()
}
fn placed_names(store: &crate::Store, number: u64, revision: u64, only: Option<u64>) -> Vec<r::Observed> {
    let key = jig_test_domain::Key::Core(core::Key::Tasks(tasks::Key::PlacedHold(number)));
    let Some(jig_test_domain::Record::Core(core::Record::Tasks(tasks::Stored::PlacedHold(hold)))) =
        store.rows.get(&key)
    else {
        return Vec::new();
    };
    if hold.revision != revision || hold.lifted {
        return Vec::new();
    }
    let whole = only.is_none() || matches!(hold.scope, tasks::Scope::Subtree { task } if Some(task) == only);
    store
        .rows
        .values()
        .filter_map(|row| {
            if let jig_test_domain::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) = row
                && row.placed == Some(number)
                && (whole || only == Some(row.number))
                && (only == Some(row.number)
                    || matches!(row.phase, tasks::Phase::Held { why: tasks::Hold::Placed { .. }, .. }))
            {
                Some(named(row.number, r::Revision::Hold(row.revisions.hold)))
            } else {
                None
            }
        })
        .collect()
}
