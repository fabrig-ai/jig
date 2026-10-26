//! Generic delegate executor and symbolic-grant decisions owned by the core
//! (domain/engine.md, 4.4 and 17.3).
#![expect(
    clippy::result_large_err,
    reason = "bounded owner refusal keeps exact revision and capacity facts inline instead of allocating on refusal"
)]

use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_tasks as tasks;
use skein_lib::{List, Queue, Wall};

use crate::{CallKey, CallPart, Core, translate};

/// A dependency named by a delegate before the core assigns batch task IDs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dependency {
    /// One member of the same batch, by zero-based position.
    Batch(u32),
    /// A live task already referenced by the creator.
    Existing(u64),
}

/// One proposed direct child before the core supplies its number and funder.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Delegate {
    pub executor: tasks::Executor,
    pub spec: tasks::Spec,
    pub contract: tasks::Contract,
    pub authority: tasks::Authority,
    /// When the delegate must end: its authority's wall, or a span after
    /// the decision that makes it (domain/authority.md, 7). Named with no
    /// wall ends with its creator's.
    pub deadline: tasks::Deadline,
    /// An agent child's LLM choice; none for the core to resolve its charter's default within
    /// the child's authority, as at Create (tasks.md, 3).
    pub llm: Option<tasks::LlmChoice>,
    /// Open connector grants narrowed with the allocated task number.
    pub symbolic_grants: Box<[tasks::Grant]>,
    pub dependencies: Box<[Dependency]>,
    pub wake: tasks::WakePolicy,
    pub tracked: Option<u32>,
    pub label: u32,
    pub flags: tasks::Flags,
    pub budget: jig_domain_people::Budget,
}

/// A connector-owned procedure's chosen task action before core admission.
#[derive(PartialEq, Eq, Debug)]
pub struct ProcedureStep {
    pub batch: Box<[Delegate]>,
    pub amendments: Box<[StepAmendment]>,
    pub cancels: Box<[StepCancel]>,
    pub messages: Box<[StepMessage]>,
    pub proposals: Box<[tasks::ProposalAction]>,
    pub end: StepEnd,
}

#[derive(PartialEq, Eq, Debug)]
pub struct StepAmendment {
    pub task: u64,
    pub revision: u64,
    pub amendment: tasks::Amendment,
}

#[derive(PartialEq, Eq, Debug)]
pub struct StepCancel {
    pub task: u64,
    pub reason: Box<[u8]>,
    pub grace: Option<skein_lib::Duration>,
}

#[derive(PartialEq, Eq, Debug)]
pub struct StepMessage {
    pub target: u64,
    pub kind: tasks::MessageKind,
    pub words: Box<[u8]>,
}

#[derive(PartialEq, Eq, Debug)]
pub enum StepEnd {
    Result(tasks::TaskResult),
    Hold(tasks::Hold),
    Wait,
}

impl ProcedureStep {
    #[must_use]
    pub fn new(end: StepEnd) -> Self {
        Self {
            batch: Box::new([]),
            amendments: Box::new([]),
            cancels: Box::new([]),
            messages: Box::new([]),
            proposals: Box::new([]),
            end,
        }
    }
}

impl Core {
    /// Admit one procedure's delegated batch against its current task authority.
    #[must_use]
    pub fn procedure_batch_admit(&self, limits: &tasks::Limits, task: u64, batch: &[Delegate], decision: Wall) -> bool {
        let Some(context) = self.tasks.delegation(task) else { return false };
        if batch.is_empty() || batch.len() > usize::try_from(limits.batch).expect("bounded batch") {
            return false;
        }
        let mut asked = List::with_capacity(limits.batch);
        for member in batch {
            if !member.spec.inputs.is_empty() {
                return false;
            }
            let Some(executor) = self.delegation_executor(context.project, member.executor) else { return false };
            let Some(symbolic) = symbolic_grants(&member.symbolic_grants, self.authority.limits().grants) else {
                return false;
            };
            asked
                .push(authority::Delegate {
                    executor,
                    authority: crate::routing::deadlines::authority(&member.authority, member.deadline, decision),
                    symbolic,
                })
                .expect("bounded procedure batch");
        }
        let mut findings = Queue::with_capacity(authority::max_out(self.authority.limits()).expect("finding bound"));
        self.connector_batch_admit(
            context.project,
            &context.authority,
            context.numbers,
            context.tasks_left,
            asked.into_boxed(),
            &mut findings,
        )
        .answer
            == authority::Answer::Allow
    }
    /// A delegate's models are its creator's, as a batch names none; a procedure's delegates take
    /// those it keeps, though it names none itself (domain/authority.md, sections 3 and 5).
    pub(crate) fn take_models(&self, task: u64, batch: &mut [Delegate]) {
        let models = self.kept_models(task);
        for member in batch {
            member.authority.models = models;
        }
    }
    /// A delegate naming no deadline ends with its creator's: its
    /// deadline is never later (domain/authority.md, 5 and 7).
    pub(crate) fn take_deadline(&self, task: u64, batch: &mut [Delegate]) {
        let Some(context) = self.tasks.delegation(task) else { return };
        for member in batch {
            if member.deadline == tasks::Deadline::Named && member.authority.budget.deadline.is_none() {
                member.authority.budget.deadline = context.authority.budget.deadline;
            }
        }
    }
    /// Check a named task delegation at `decision`, before any historical
    /// input load or connector handoff.
    pub fn delegate_preflight(
        &self,
        limits: &tasks::Limits,
        key: CallKey,
        batch: &[Delegate],
        decision: Wall,
    ) -> Result<(), CallPart> {
        if !self.current_proof(key.task, key.attempt) {
            return Err(delegate_refused(Some(key.task), tasks::Refusal::State));
        }
        let Some(context) = self.tasks.delegation(key.task) else {
            return Err(delegate_refused(Some(key.task), tasks::Refusal::Unknown));
        };
        if batch.is_empty() || batch.len() > usize::try_from(limits.batch).expect("u32 fits usize") {
            return Err(delegate_refused(None, tasks::Refusal::Batch));
        }
        let mut asked = List::with_capacity(limits.batch);
        for member in batch {
            if let tasks::Executor::Agent { charter } = member.executor
                && self.charter(charter).is_none()
            {
                return Err(delegate_refused(None, tasks::Refusal::Executor));
            }
            if let Err(why) = crate::account_preference::executor(member.executor, member.llm.as_ref()) {
                return Err(delegate_refused(None, why));
            }
            if let Some(choice) = &member.llm
                && choice.account.is_some()
            {
                let payer =
                    self.tasks.payer(key.task).ok_or(delegate_refused(Some(key.task), tasks::Refusal::Funding))?;
                if let Err(why) = self.account_preference(Some(choice), payer, None) {
                    return Err(delegate_refused(None, why));
                }
            }
            let Some(executor) = self.delegation_executor(context.project, member.executor) else {
                return Err(delegate_refused(None, tasks::Refusal::Executor));
            };
            let Some(symbolic) = symbolic_grants(&member.symbolic_grants, self.authority.limits().grants) else {
                return Err(delegate_refused(None, tasks::Refusal::AuthorityShape));
            };
            let mut authority = crate::routing::deadlines::authority(&member.authority, member.deadline, decision);
            if member.deadline == tasks::Deadline::Named && authority.budget.deadline.is_none() {
                authority.budget.deadline = context.authority.budget.deadline;
            }
            asked.push(authority::Delegate { executor, authority, symbolic }).expect("bounded delegation request");
        }
        let mut findings = Queue::with_capacity(authority::max_out(self.authority.limits()).expect("findings bound"));
        let checked = self.connector_batch_admit(
            context.project,
            &context.authority,
            context.numbers,
            context.tasks_left,
            asked.into_boxed(),
            &mut findings,
        );
        if checked.answer != authority::Answer::Allow {
            let mut found = List::with_capacity(authority::max_out(self.authority.limits()).expect("findings bound"));
            for _ in 0..findings.len() {
                found.push(findings.pop().expect("finding count")).expect("finding bound");
            }
            return Err(CallPart::DelegationDenied { answer: checked.answer, findings: found.into_boxed() });
        }
        Ok(())
    }

    /// Name each historical input once before the root starts its bounded result loads.
    pub fn delegate_inputs(
        &self,
        limits: &tasks::Limits,
        key: CallKey,
        batch: &[Delegate],
    ) -> Result<(u32, Box<[u64]>), CallPart> {
        let Some(context) = self.tasks.delegation(key.task) else {
            return Err(delegate_refused(Some(key.task), tasks::Refusal::Unknown));
        };
        let capacity = batch
            .len()
            .checked_mul(usize::try_from(limits.inputs).expect("u32 fits usize"))
            .expect("bounded batch input count");
        let mut ids = List::with_capacity(u32::try_from(capacity).expect("bounded input IDs"));
        for member in batch {
            if member.spec.inputs.len() > usize::try_from(limits.inputs).expect("u32 fits usize") {
                return Err(delegate_refused(None, tasks::Refusal::Inputs));
            }
            for &input in &member.spec.inputs {
                let mut known = false;
                for &id in &ids {
                    if id == input {
                        known = true;
                    }
                }
                if !known {
                    ids.push(input).expect("bounded input ID count");
                }
            }
        }
        Ok((context.project, ids.into_boxed()))
    }

    /// Resolve a proposed executor against the current party roles.
    #[must_use]
    pub fn delegation_executor(&self, project: u32, executor: tasks::Executor) -> Option<authority::Executor> {
        match executor {
            tasks::Executor::Agent { charter } => Some(authority::Executor::Charter(charter)),
            tasks::Executor::Procedure { code, .. } => Some(authority::Executor::Procedure(code)),
            tasks::Executor::Person(tasks::PersonAddress::Role(role)) => Some(authority::Executor::Role(role)),
            tasks::Executor::Person(tasks::PersonAddress::Person(person)) => {
                let holding = self.people.role(person, project)?;
                Some(authority::Executor::Role(holding.number()))
            }
        }
    }
}

fn delegate_refused(task: Option<u64>, why: tasks::Refusal) -> CallPart {
    CallPart::DelegationRefused(tasks::Problem { task, why, blocked_by: None, capacity: None })
}

/// Translate open grant patterns for a delegate whose number is not yet fixed.
#[must_use]
pub fn symbolic_grants(grants: &[tasks::Grant], limit: u32) -> Option<Box<[authority::Grant]>> {
    let mut result = List::with_capacity(limit);
    for grant in grants {
        let last = match &grant.pattern.last {
            tasks::Last::Open(prefix) => authority::Last::Open(prefix.clone()),
            tasks::Last::Exact(_) => return None,
        };
        result
            .push(authority::Grant {
                connector: grant.connector,
                kind: grant.kind,
                pattern: authority::Pattern { segments: grant.pattern.segments.clone(), last },
            })
            .ok()?;
    }
    Some(result.into_boxed())
}

/// Narrow a delegate's symbolic grants with its newly assigned task number.
#[must_use]
pub fn resolved_delegate_authority(
    base: &tasks::Authority,
    symbolic: &[tasks::Grant],
    task: u64,
    limits: &authority::Limits,
) -> Option<tasks::Authority> {
    let symbols = symbolic_grants(symbolic, limits.grants)?;
    let resolved = authority::resolve_task_grants(&symbols, task, limits)?;
    let mut authority = translate::authority_value(base);
    let mut grants = List::with_capacity(limits.grants);
    for grant in &authority.grants {
        grants.push(grant.clone()).ok()?;
    }
    for grant in resolved {
        grants.push(grant).ok()?;
    }
    authority.grants = grants.into_boxed();
    Some(translate::task_authority(&authority))
}
