//! Core-owned proposal action checks and nearest-holder routing
//! (domain/engine.md, 4.4; domain/tasks.md, 9).

use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{List, Queue, Wall};

use crate::{Core, Limits, ProposalDecisionRecord, translate};

fn kind(action: &tasks::ProposalAction) -> (tasks::ProposalKind, Option<authority::ProposalKind>) {
    match action {
        tasks::ProposalAction::Settings { .. } => (tasks::ProposalKind::Settings, None),
        tasks::ProposalAction::Batch(_) => (tasks::ProposalKind::Batch, Some(authority::ProposalKind::Batch)),
        tasks::ProposalAction::Effect { .. } | tasks::ProposalAction::ProcedureEffect { .. } => {
            (tasks::ProposalKind::Effect, Some(authority::ProposalKind::Effect))
        }
        tasks::ProposalAction::Amend { .. } => (tasks::ProposalKind::Amend, Some(authority::ProposalKind::Amend)),
        tasks::ProposalAction::Widen { .. } => (tasks::ProposalKind::Widen, Some(authority::ProposalKind::Widen)),
        tasks::ProposalAction::ReleasePlaced { .. } | tasks::ProposalAction::Release { .. } => {
            (tasks::ProposalKind::Release, Some(authority::ProposalKind::Escalation))
        }
    }
}

fn action_for_check(core: &Core, action: &tasks::ProposalAction, decision: Wall) -> Option<authority::Action> {
    match action {
        tasks::ProposalAction::Settings { .. } => None,
        tasks::ProposalAction::Batch(batch) => {
            let mut members = List::with_capacity(u32::try_from(batch.len()).expect("bounded proposed batch"));
            for member in batch {
                let executor = match member.executor {
                    tasks::Executor::Agent { charter } => authority::Executor::Charter(charter),
                    tasks::Executor::Procedure { code, .. } => authority::Executor::Procedure(code),
                    tasks::Executor::Person(_) => return None,
                };
                members
                    .push(authority::Delegate {
                        executor,
                        authority: crate::routing::deadlines::authority(&member.authority, member.deadline, decision),
                        symbolic: Box::new([]),
                    })
                    .expect("bounded proposed batch");
            }
            Some(authority::Action::Batch(members.into_boxed()))
        }
        // Tasks keeps a generic authority carrier; the connector supplies the
        // fresh pinned effect again when the holder accepts it.
        tasks::ProposalAction::Effect { authority, .. }
        | tasks::ProposalAction::ProcedureEffect { authority, .. }
        | tasks::ProposalAction::Widen { authority, .. } => {
            Some(authority::Action::Widen(translate::authority_value(authority)))
        }
        tasks::ProposalAction::Amend { task, amendment, .. } => Some(authority::Action::Amend(
            translate::authority_value(amendment.authority.as_ref().unwrap_or(&core.tasks.task(*task)?.authority)),
        )),
        tasks::ProposalAction::ReleasePlaced { .. } | tasks::ProposalAction::Release { .. } => {
            Some(authority::Action::Escalate { release: None })
        }
    }
}

fn covers_task(
    core: &Core,
    _limits: &Limits,
    needed: &authority::Authority,
    ancestor: u64,
    distance: u32,
    project: u32,
) -> bool {
    let Some(holder) = core.tasks.delegation(ancestor) else { return false };
    holder.project == project
        && holder.deciding
        && authority::covers(
            &core.authority,
            needed,
            &authority::Holder::Task {
                project,
                authority: translate::authority_value(&holder.authority),
                numbers: translate::authority_numbers(holder.numbers),
                tasks_left: holder.tasks_left,
            },
            distance,
        )
}

fn distance(core: &Core, limits: &Limits, proposer: u64, holder: u64) -> Option<u32> {
    let mut next = core.tasks.delegation(proposer)?.requester;
    for depth in 1..=limits.tasks.depth.saturating_add(1) {
        match next {
            tasks::Party::Task(task) if task == holder => return Some(depth),
            tasks::Party::Task(task) => next = core.tasks.delegation(task)?.requester,
            tasks::Party::Person(_) | tasks::Party::Deployment { .. } => return None,
        }
    }
    None
}

fn covers_person(
    core: &Core,
    limits: &Limits,
    needed: &authority::Authority,
    person: u64,
    project: u32,
    kind: authority::ProposalKind,
) -> bool {
    let Some(role) = core.people.role(person, project) else { return false };
    let funder = tasks::Funder::Pool { project, person, period: core.period(project) };
    let numbers = match core.tasks.funding(funder) {
        Some(pool) => pool.numbers,
        None => tasks::Numbers { budget: core.pool_budget(project, person), spent: 0, spent_below: 0, reserved: 0 },
    };
    authority::covers(
        &core.authority,
        needed,
        &authority::Holder::Person {
            project,
            role: role.number(),
            proposal: kind,
            pool: translate::authority_numbers(numbers),
            tasks_left: limits.tasks.tree_tasks,
        },
        0,
    )
}

/// Select the nearest live covering ancestor, then the root requester, then
/// the policy's eligible people. `after` skips holders through one pass/stall.
fn holder(
    core: &Core,
    limits: &Limits,
    proposer: u64,
    action: &tasks::ProposalAction,
    after: Option<tasks::ProposalHolder>,
    decision: Wall,
) -> Option<tasks::ProposalHolder> {
    if let tasks::ProposalAction::Settings { .. } = action {
        return if after.is_none() {
            Some(tasks::ProposalHolder::Policy { project: people::DEPLOYMENT, kind: tasks::ProposalKind::Settings })
        } else {
            None
        };
    }
    if let tasks::ProposalAction::ReleasePlaced { hold, .. } = action {
        if after.is_some() {
            return None;
        }
        return Some(match core.tasks.placed_hold(*hold)?.owner {
            tasks::Owner::Party { person } => tasks::ProposalHolder::Person(person),
            tasks::Owner::Task { task } => tasks::ProposalHolder::Task(task),
        });
    }
    let context = core.tasks.delegation(proposer)?;
    let needed = core.proposal_admit_action(context.project, action, decision).ok()?;
    let (kind, policy_kind) = kind(action);
    let policy_kind = policy_kind?;
    let mut next = context.requester;
    let mut distance = 1_u32;
    let mut passed = after.is_none();
    for _ in 0..limits.tasks.depth.saturating_add(1) {
        match next {
            tasks::Party::Task(task) => {
                let holder = tasks::ProposalHolder::Task(task);
                if passed && covers_task(core, limits, &needed, task, distance, context.project) {
                    return Some(holder);
                }
                if after == Some(holder) {
                    passed = true;
                }
                next = match core.tasks.delegation(task) {
                    Some(context) => context.requester,
                    None => return Some(tasks::ProposalHolder::Policy { project: context.project, kind }),
                };
                distance = distance.checked_add(1)?;
            }
            tasks::Party::Person(person) => {
                let holder = tasks::ProposalHolder::Person(person);
                if passed && covers_person(core, limits, &needed, person, context.project, policy_kind) {
                    return Some(holder);
                }
                if after == Some(holder) {
                    // The next recipient is the final policy group.
                }
                return Some(tasks::ProposalHolder::Policy { project: context.project, kind });
            }
            tasks::Party::Deployment { .. } => {
                return Some(tasks::ProposalHolder::Policy { project: context.project, kind });
            }
        }
    }
    None
}

impl Core {
    /// Check the accepting party's request and return the authority finding as facts.
    pub(crate) fn proposal_person_admit(
        &self,
        person: u64,
        project: u32,
        role: people::Role,
        action: authority::Action,
        tasks_left: u32,
    ) -> Result<(), people::Refusal> {
        let (_, pool) = self.person_pool(project, person);
        let ask = authority::PersonAsk {
            project,
            role: role.number(),
            pool: translate::authority_numbers(pool),
            tasks_left,
            request: authority::PersonRequest::Accept(action),
        };
        let mut findings = Queue::uncapped();
        let checked = authority::check_request(&self.authority, &ask, &mut findings);
        if checked.answer == authority::Answer::Allow {
            Ok(())
        } else {
            Err(self.request_refusal(&ask, checked.answer, &mut findings))
        }
    }

    /// Check a task's proposed action against both project and deployment ceilings.
    pub fn proposal_admit_action(
        &self,
        project: u32,
        action: &tasks::ProposalAction,
        decision: Wall,
    ) -> Result<authority::Authority, tasks::Refusal> {
        self.proposed_action_accounts(action)?;
        let Some(checked) = action_for_check(self, action, decision) else { return Err(tasks::Refusal::Executor) };
        let Some(needed) = authority::needs(&checked) else { return Err(tasks::Refusal::AuthorityShape) };
        let Some(policy) = self.authority.policy(project) else { return Err(tasks::Refusal::Project) };
        let implies = &self.authority.rules().implies;
        if !authority::at_most(&needed, &policy.ceiling, implies)
            || !authority::at_most(&needed, &self.authority.rules().ceiling, implies)
        {
            return Err(tasks::Refusal::AuthorityShape);
        }
        Ok(needed)
    }

    /// Check that a task holder can fund the action it accepted.
    pub fn proposal_accept_allowed(
        &self,
        limits: &Limits,
        proposer: u64,
        holder: u64,
        project: u32,
        action: &tasks::ProposalAction,
        decision: Wall,
    ) -> Result<(), tasks::Refusal> {
        if let tasks::ProposalAction::ReleasePlaced { hold, .. } = action {
            return if self.tasks.may_lift(*hold, tasks::Party::Task(holder), false) {
                Ok(())
            } else {
                Err(tasks::Refusal::HoldOwner { hold: *hold })
            };
        }
        let Some(checked) = action_for_check(self, action, decision) else { return Err(tasks::Refusal::Executor) };
        let Some(needed) = authority::needs(&checked) else { return Err(tasks::Refusal::AuthorityShape) };
        self.proposal_admit_action(project, action, decision)?;
        self.action_accounts(action, tasks::Funder::Task(holder))?;
        let Some(depth) = distance(self, limits, proposer, holder) else { return Err(tasks::Refusal::Reference) };
        if covers_task(self, limits, &needed, holder, depth, project) { Ok(()) } else { Err(tasks::Refusal::Funding) }
    }
    /// Revisit each pending holder after a project policy or role change.
    #[must_use]
    pub fn proposal_recheck_project(&self, limits: &Limits, project: u32, decision: Wall) -> Queue<tasks::Event> {
        let mut out = Queue::with_capacity(limits.tasks.tasks);
        for view in self.tasks.view_tasks() {
            if view.project != project {
                continue;
            }
            let Some(record) = self.tasks.task(view.number) else { continue };
            let Some(proposal) = &record.proposal else { continue };
            let current = match proposal.state {
                tasks::ProposalState::Pending { holder, .. } => holder,
                tasks::ProposalState::Accepted { .. }
                | tasks::ProposalState::Rejected { .. }
                | tasks::ProposalState::Withdrawn => continue,
            };
            let Some(next) = self.proposal_holder(limits, view.number, &proposal.action, None, decision) else {
                continue;
            };
            if current != next {
                out.push(tasks::Event::StalledProposal {
                    proposer: view.number,
                    proposal: proposal.number,
                    from: current,
                    revision: record.revision,
                    holder: next,
                });
            }
        }
        out
    }

    /// Whether a person's current role may decide this proposal kind.
    #[must_use]
    pub fn proposal_policy_standing(&self, person: u64, project: u32, kind: authority::ProposalKind) -> bool {
        let Some(role) = self.people.role(person, project) else { return false };
        let Some(policy) = self.authority.role(project, role.number()) else { return false };
        policy.decides.allows(kind)
    }

    /// The task and authority kinds of a proposal action.
    #[must_use]
    pub fn proposal_kind(action: &tasks::ProposalAction) -> (tasks::ProposalKind, Option<authority::ProposalKind>) {
        kind(action)
    }

    /// Translate a proposal action for authority admission.
    #[must_use]
    pub fn proposal_action_for_check(
        &self,
        action: &tasks::ProposalAction,
        decision: Wall,
    ) -> Option<authority::Action> {
        action_for_check(self, action, decision)
    }

    /// Whether one ancestor can decide this proposal.
    #[must_use]
    pub fn proposal_covers_task(
        &self,
        limits: &Limits,
        needed: &authority::Authority,
        ancestor: u64,
        distance: u32,
        project: u32,
    ) -> bool {
        covers_task(self, limits, needed, ancestor, distance, project)
    }

    /// Distance from a proposer to a named ancestor, if live.
    #[must_use]
    pub fn proposal_distance(&self, limits: &Limits, proposer: u64, holder: u64) -> Option<u32> {
        distance(self, limits, proposer, holder)
    }

    /// Whether one party can decide this proposal.
    #[must_use]
    pub fn proposal_covers_person(
        &self,
        limits: &Limits,
        needed: &authority::Authority,
        person: u64,
        project: u32,
        kind: authority::ProposalKind,
    ) -> bool {
        covers_person(self, limits, needed, person, project, kind)
    }

    /// Choose the next live proposal holder after any passed holder.
    #[must_use]
    pub fn proposal_holder(
        &self,
        limits: &Limits,
        proposer: u64,
        action: &tasks::ProposalAction,
        after: Option<tasks::ProposalHolder>,
        decision: Wall,
    ) -> Option<tasks::ProposalHolder> {
        holder(self, limits, proposer, action, after, decision)
    }
}

/// Extract one immutable person-facing final decision from the child's durable row.
pub(crate) fn decision_record(row: &tasks::Stored, at: Wall) -> Option<ProposalDecisionRecord> {
    match row {
        tasks::Stored::PersonProposal(row) | tasks::Stored::DecidedProposal(row) => {
            let (by, choice) = match &row.state {
                tasks::PersonProposalState::Accepted { by: tasks::Party::Person(by) }
                | tasks::PersonProposalState::SettingsAccepted { by: tasks::Party::Person(by), .. } => {
                    (*by, people::ProposalChoice::Accepted)
                }
                tasks::PersonProposalState::Rejected { by: tasks::Party::Person(by), .. } => {
                    (*by, people::ProposalChoice::Rejected)
                }
                tasks::PersonProposalState::Withdrawn { .. } => (row.proposer, people::ProposalChoice::Withdrawn),
                tasks::PersonProposalState::Pending { .. }
                | tasks::PersonProposalState::SettingsAccepted {
                    by: tasks::Party::Task(_) | tasks::Party::Deployment { .. },
                    ..
                }
                | tasks::PersonProposalState::Accepted {
                    by: tasks::Party::Task(_) | tasks::Party::Deployment { .. },
                }
                | tasks::PersonProposalState::Rejected {
                    by: tasks::Party::Task(_) | tasks::Party::Deployment { .. },
                    ..
                } => return None,
            };
            Some(ProposalDecisionRecord {
                project: row.project,
                proposer: people::Proposer::Party(row.proposer),
                proposal: row.number,
                kind: match row.action {
                    tasks::PersonAction::Settings { .. } => tasks::ProposalKind::Settings,
                    tasks::PersonAction::Batch(_) => tasks::ProposalKind::Batch,
                    tasks::PersonAction::Amend { .. } => tasks::ProposalKind::Amend,
                },
                by,
                at: match row.state {
                    tasks::PersonProposalState::Withdrawn { at } => at,
                    tasks::PersonProposalState::Pending { .. }
                    | tasks::PersonProposalState::SettingsAccepted { .. }
                    | tasks::PersonProposalState::Accepted { .. }
                    | tasks::PersonProposalState::Rejected { .. } => at,
                },
                choice,
                created: if choice == people::ProposalChoice::Accepted {
                    match (row.batch().first(), row.batch().last()) {
                        (Some(first), Some(last)) => Some((first.number, last.number)),
                        (None, _) | (_, None) => None,
                    }
                } else {
                    None
                },
            })
        }
        tasks::Stored::History(history) => {
            let proposal = history.proposal.as_ref()?;
            let (by, choice) = match &proposal.state {
                tasks::ProposalState::Accepted { by: tasks::Party::Person(by) } => {
                    (*by, people::ProposalChoice::Accepted)
                }
                tasks::ProposalState::Rejected { by: tasks::Party::Person(by), .. } => {
                    (*by, people::ProposalChoice::Rejected)
                }
                tasks::ProposalState::Pending { .. }
                | tasks::ProposalState::Withdrawn
                | tasks::ProposalState::Accepted { by: tasks::Party::Task(_) | tasks::Party::Deployment { .. } }
                | tasks::ProposalState::Rejected {
                    by: tasks::Party::Task(_) | tasks::Party::Deployment { .. }, ..
                } => return None,
            };
            Some(ProposalDecisionRecord {
                project: proposal.project,
                proposer: people::Proposer::Task(proposal.proposer),
                proposal: proposal.number,
                kind: kind(&proposal.action).0,
                by,
                at,
                choice,
                created: None,
            })
        }
        tasks::Stored::Live(_)
        | tasks::Stored::Ended(_)
        | tasks::Stored::Ledger(_)
        | tasks::Stored::Writer(_)
        | tasks::Stored::Result(_)
        | tasks::Stored::EndedByParty(_)
        | tasks::Stored::EndedByProject(_)
        | tasks::Stored::EndedDelegate(_)
        | tasks::Stored::Delegate(_)
        | tasks::Stored::Update(_)
        | tasks::Stored::Pool(_)
        | tasks::Stored::ClosedLedger(_)
        | tasks::Stored::PlacedHold(_)
        | tasks::Stored::Stub(_)
        | tasks::Stored::Activity(_)
        | tasks::Stored::Milestone(_) => None,
    }
}

impl Core {
    pub(crate) fn release_proposal_admit(
        &self,
        proposer: u64,
        task: u64,
        hold: u64,
        revision: u64,
        depth: u32,
    ) -> Result<(), tasks::Refusal> {
        let row = self.tasks.task(task).ok_or(tasks::Refusal::Unknown)?;
        let mut at = task;
        let mut above = false;
        for _ in 0..=depth {
            if at == proposer {
                above = true;
                break;
            }
            match self.tasks.task(at).ok_or(tasks::Refusal::Unknown)?.requester {
                tasks::Party::Task(parent) => at = parent,
                tasks::Party::Person(_) | tasks::Party::Deployment { .. } => break,
            }
        }
        if !above {
            return Err(tasks::Refusal::Reference);
        }
        if row.placed != Some(hold) {
            return Err(match self.tasks.placed_on(task) {
                Some(now) => tasks::Refusal::Placed { hold: now.number },
                None => tasks::Refusal::State,
            });
        }
        let hold = self.tasks.placed_hold(hold).ok_or(tasks::Refusal::Unknown)?;
        if hold.revision != revision || hold.lifted {
            return Err(tasks::Refusal::HoldRevision { revision: hold.revision });
        }
        Ok(())
    }
}

pub(crate) const fn placed_release(action: &tasks::ProposalAction) -> bool {
    match action {
        tasks::ProposalAction::ReleasePlaced { .. } => true,
        tasks::ProposalAction::Settings { .. }
        | tasks::ProposalAction::Batch(_)
        | tasks::ProposalAction::Effect { .. }
        | tasks::ProposalAction::ProcedureEffect { .. }
        | tasks::ProposalAction::Amend { .. }
        | tasks::ProposalAction::Widen { .. }
        | tasks::ProposalAction::Release { .. } => false,
    }
}
