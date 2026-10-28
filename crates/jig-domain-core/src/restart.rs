//! The core's cold-start script (domain/engine.md, 6; domain/root.md, 8).
//! The application performs each requested step and reports that exact step
//! done. Only the core chooses the next step or opens decisions.

mod census;
mod joins;
use crate::{Core, fleet};
pub use census::{Census, ReadResults};

/// Ordered live ranges of the cold start (protocol/store.md, 4.3 and 7.2).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RestoreRange {
    /// The deployment header, first.
    Deployment,
    /// The live settings, before the children restore.
    Settings,
    /// The accounts' counter, accounts and sources, before the child starts.
    Accounts,
    /// All people's live families.
    People,
    /// The tasks child's live families.
    Tasks,
    /// The core's remaining live families.
    Core,
    /// Inbox owners named by durable indexes after read positions are restored.
    Census(Census),
}

impl RestoreRange {
    /// The fixed order of `LoadCore`'s ranges.
    pub const LIVE: [Self; 6] =
        [Self::Deployment, Self::Settings, Self::Accounts, Self::People, Self::Tasks, Self::Core];

    /// Predicate checked against the store's intervals (protocol/store.md, 4.3).
    #[must_use]
    pub fn admits(&self, key: &crate::Key) -> bool {
        match self {
            Self::Census(census) => return census.admits(key),
            Self::Deployment | Self::Settings | Self::Accounts | Self::People | Self::Tasks | Self::Core => {}
        }
        match key {
            crate::Key::Core(crate::CoreKey::Settings(key)) => {
                *self == Self::Settings
                    && match key {
                        crate::settings::Key::History(_) => false,
                        crate::settings::Key::Default(_)
                        | crate::settings::Key::Changed(_)
                        | crate::settings::Key::Held(_)
                        | crate::settings::Key::Charter { .. }
                        | crate::settings::Key::Proposal { .. }
                        | crate::settings::Key::Extent => true,
                    }
            }
            crate::Key::Core(crate::CoreKey::Deployment) => *self == Self::Deployment,
            crate::Key::Core(key) => *self == Self::Core && key.live(),
            crate::Key::People(key) => *self == Self::People && key.live(),
            crate::Key::Tasks(key) => *self == Self::Tasks && key.live(),
            crate::Key::Accounts(_) => *self == Self::Accounts,
            crate::Key::Notes(_) => false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RestartStep {
    LoadCore,
    /// Rebuild derived inbox facts after people's read positions have restored.
    LoadInbox,
    RestoreConnector {
        connector: u16,
    },
    /// Drain bounded retention batches only after every child's links exist.
    Sweep,
    /// Make the store active, then commit the start’s wires (domain/engine.md, 6).
    Switch,
    AdoptRuns,
    ReadAfresh {
        connector: u16,
    },
    SettleOutbox {
        connector: u16,
    },
    Open,
}

/// The held settings bound that prevents a cold start (engine.md, 17.7–17.8).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HeldLimit {
    Revisions,
    Versions,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RestartRequest {
    AccountsPastLimit {
        which: jig_domain_accounts::RestoreLimit,
    },
    /// Held settings revisions or charter versions past a limit lowered since the stop
    /// (engine.md, 17.7, "past limits").
    HeldPastLimit {
        which: HeldLimit,
        held: u32,
        limit: u32,
    },
    Settings {
        failure: crate::settings::Failure,
        changed: bool,
    },
    /// A saved project policy exceeds the current byte budget or cannot be accounted.
    ProjectPolicy {
        project: u32,
    },
    Step(RestartStep),
    Idle,
    Refused {
        step: RestartStep,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Restart {
    Cold,
    Waiting(RestartStep),
    Open,
    Failed(RestartStep),
}

impl Core {
    /// At the sweep step, after the children's links were stepped:
    /// false refuses the start before any retention or switch.
    pub fn restart_checked(&mut self) -> bool {
        if self.restart != Restart::Waiting(RestartStep::Sweep) {
            let _request = self.restart_refuse();
            return false;
        }
        assert!(self.tasks.checked(), "tasks: link pass must finish before the switch");
        if let Some(which) = self.accounts.restore_limit() {
            self.restore_failure = Some(RestartRequest::AccountsPastLimit { which });
            let _request = self.restart_refuse();
            return false;
        }
        self.accounts.link_restored();
        self.link_tasks();
        self.settings_proposals_consistent();
        for ((party, task), _) in self.people.read_conversations() {
            assert!(
                self.tasks.task(*task).is_some(),
                "people: ReadConversation({party}, {task}) links to task {task}, which is absent"
            );
        }
        for (task, _) in &self.proofs {
            assert!(self.tasks.task(*task).is_some(), "core: RunProof({task}) links to task {task}, which is absent");
        }
        for (key, _) in &self.call_parts {
            assert!(
                self.proofs.contains_key(&key.task),
                "core: Call({key:?}) links to proof {}, which is absent",
                key.task
            );
        }
        true
    }

    /// One bounded post-link batch. Its continuation follows durability; no
    /// ordinary decision opens and no timer runs while records are loading.
    pub fn restart_sweep(&mut self, env: &skein_lib::Env<crate::Limits>) -> crate::Requests {
        assert!(self.restart == Restart::Waiting(RestartStep::Sweep), "core: sweep follows every restored link");
        if !self.restart_checked() {
            return crate::Requests::Out(skein_lib::Queue::with_capacity(0));
        }
        let mut expired = skein_lib::Queue::with_capacity(jig_domain_people::max_out(&env.limits.people));
        jig_domain_people::fire(
            &mut self.people,
            &skein_lib::Env { now: env.now, wall: env.wall, limits: env.limits.people },
            &mut expired,
        );
        let mut out = skein_lib::Queue::with_capacity(2);
        let mut changed = false;
        while let Some(request) = expired.pop() {
            let jig_domain_people::Request::Erase { key } = request else {
                unreachable!("people expiry only erases one retained row");
            };
            out.push(crate::Request::Write(crate::Write::Erase(crate::Key::People(key))));
            changed = true;
        }
        let next = if changed { RestartStep::Sweep } else { RestartStep::Switch };
        if !changed {
            let _request = self.restart_done(RestartStep::Sweep);
        }
        out.push(crate::Request::Held(alloc::boxed::Box::new(crate::Held::Restart(next))));
        crate::Requests::Out(out)
    }

    /// Check all loaded claims together before the fleet can settle any of
    /// them. Engine claims settle immediately and need no recovered slot.
    pub fn restart_admit_claims(&mut self) -> bool {
        match self.restart {
            Restart::Waiting(RestartStep::AdoptRuns) => {}
            Restart::Cold | Restart::Waiting(_) | Restart::Open | Restart::Failed(_) => {
                let _request = self.restart_refuse();
                return false;
            }
        }
        let mut workers = 0_u32;
        for (task, _) in &self.unreported_restored {
            match self.proofs.get(task).expect("restored claim has its proof").host {
                fleet::HostKind::Worker if !self.workers_in_process => {
                    workers = workers.checked_add(1).expect("bounded restored claims");
                }
                fleet::HostKind::Worker | fleet::HostKind::Engine => {}
            }
        }
        if workers > self.restart_attempts {
            let _request = self.restart_refuse();
            return false;
        }
        true
    }

    /// Begin once; a repeated start cannot repeat loads or open decisions.
    pub fn restart_begin(&mut self, pending: bool) -> RestartRequest {
        match self.restart {
            Restart::Cold => {
                self.restart_pending = pending;
                self.people.end_seeding();
                self.counters.begin_restore();
                self.restart_ask(RestartStep::LoadCore)
            }
            Restart::Waiting(_) | Restart::Open | Restart::Failed(_) => RestartRequest::Idle,
        }
    }

    /// A failed or oversized live range stops this start, identifying its step.
    pub fn restart_refuse(&mut self) -> RestartRequest {
        let step = match self.restart {
            Restart::Cold => RestartStep::LoadCore,
            Restart::Waiting(step) | Restart::Failed(step) => step,
            Restart::Open => RestartStep::Open,
        };
        self.restart = Restart::Failed(step);
        self.census_progress = skein_lib::Map::with_capacity(self.census_progress.capacity());
        RestartRequest::Refused { step }
    }

    #[must_use]
    pub fn restart_ready(&self) -> bool {
        match self.restart {
            Restart::Open => true,
            Restart::Cold | Restart::Waiting(_) | Restart::Failed(_) => false,
        }
    }

    #[must_use]
    pub fn restart_failure(&self) -> Option<RestartStep> {
        match self.restart {
            Restart::Failed(step) => Some(step),
            Restart::Cold | Restart::Waiting(_) | Restart::Open => None,
        }
    }

    /// The named restore refusal, retained for the shell's report.
    #[must_use]
    pub const fn restore_failure(&self) -> Option<RestartRequest> {
        self.restore_failure
    }

    /// Connector continuations and timers may finish the step currently asked.
    #[must_use]
    pub fn restart_step(&self) -> Option<RestartStep> {
        match self.restart {
            Restart::Waiting(step) => Some(step),
            Restart::Cold | Restart::Open | Restart::Failed(_) => None,
        }
    }

    /// Advance only from the matching completed step. Proofs must all be loaded
    /// and every restored claim must reach the fleet before the script proceeds.
    pub fn restart_done(&mut self, done: RestartStep) -> RestartRequest {
        if let Some(failure) = self.restore_failure {
            return failure;
        }
        match self.restart {
            Restart::Waiting(step) if step == done => {}
            Restart::Waiting(_) | Restart::Cold => return self.restart_refuse(),
            Restart::Open | Restart::Failed(_) => return RestartRequest::Idle,
        }
        let next = match done {
            RestartStep::LoadCore => {
                for (task, _) in &self.restoring_proofs {
                    assert!(
                        self.proofs.contains_key(task),
                        "core: Live({task}) links to run proof {task}, which is absent"
                    );
                }
                self.restoring_proofs = skein_lib::Map::with_capacity(self.restoring_proofs.capacity());
                self.counters.finish_restore();
                RestartStep::LoadInbox
            }
            RestartStep::LoadInbox => {
                if self.census_position.is_some() {
                    return self.restart_refuse();
                }
                self.census_progress = skein_lib::Map::with_capacity(self.census_progress.capacity());
                match self.connectors.first() {
                    Some(&connector) => RestartStep::RestoreConnector { connector },
                    None => RestartStep::Sweep,
                }
            }
            RestartStep::RestoreConnector { connector } => match self.restart_next_connector(connector) {
                Some(connector) => RestartStep::RestoreConnector { connector },
                None => RestartStep::Sweep,
            },
            RestartStep::Sweep => RestartStep::Switch,
            RestartStep::Switch => RestartStep::AdoptRuns,
            RestartStep::AdoptRuns => {
                if !self.adopted.is_empty() {
                    return self.restart_refuse();
                }
                match self.connectors.first() {
                    Some(&connector) => RestartStep::ReadAfresh { connector },
                    None => RestartStep::Open,
                }
            }
            RestartStep::ReadAfresh { connector } => match self.restart_next_connector(connector) {
                Some(connector) => RestartStep::ReadAfresh { connector },
                None => match self.connectors.first() {
                    Some(&connector) => RestartStep::SettleOutbox { connector },
                    None => RestartStep::Open,
                },
            },
            RestartStep::SettleOutbox { connector } => match self.restart_next_connector(connector) {
                Some(connector) => RestartStep::SettleOutbox { connector },
                None => RestartStep::Open,
            },
            RestartStep::Open => {
                self.restart = Restart::Open;
                return RestartRequest::Idle;
            }
        };
        self.restart_ask(next)
    }

    fn restart_ask(&mut self, step: RestartStep) -> RestartRequest {
        self.restart = Restart::Waiting(step);
        RestartRequest::Step(step)
    }

    fn restart_next_connector(&self, previous: u16) -> Option<u16> {
        let mut found = false;
        for &connector in &self.connectors {
            if found {
                return Some(connector);
            }
            found = connector == previous;
        }
        None
    }
}
