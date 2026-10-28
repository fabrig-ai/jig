//! Core-owned standing and policy decisions for a person's task amendment.

use jig_domain_authority as authority;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::List;

use crate::{Core, translate};

impl Core {
    /// The models a task keeps through an amendment of its authority, which names none; none for
    /// an unknown task, which the amendment's admission refuses (domain/authority.md, section 3).
    #[must_use]
    pub(crate) fn kept_models(&self, task: u64) -> tasks::Models {
        match self.tasks.task(task) {
            Some(row) => row.authority.models,
            None => tasks::Models::NONE,
        }
    }

    /// A task-origin amendment's authority keeps its target's models, as it names none
    /// (domain/authority.md, section 3).
    pub(crate) fn keep_models(&self, target: u64, amendment: &mut tasks::Amendment) {
        if let Some(authority) = &mut amendment.authority {
            authority.models = self.kept_models(target);
        }
    }

    /// A proposed action keeps the models its proposer names none of: a batch's members take the
    /// proposer's, an amendment or a widening keeps its task's, and an effect's needs name none
    /// (domain/authority.md, sections 3 and 5).
    pub(crate) fn keep_proposal_models(&self, proposer: u64, action: &mut tasks::ProposalAction) {
        match action {
            tasks::ProposalAction::Batch(batch) => {
                let models = self.kept_models(proposer);
                for member in batch.iter_mut() {
                    member.authority.models = models;
                }
            }
            tasks::ProposalAction::Amend { task, amendment, .. } => self.keep_models(*task, amendment),
            tasks::ProposalAction::Widen { task, authority } => authority.models = self.kept_models(*task),
            tasks::ProposalAction::Settings { .. }
            | tasks::ProposalAction::Effect { .. }
            | tasks::ProposalAction::ProcedureEffect { .. }
            | tasks::ProposalAction::ReleasePlaced { .. }
            | tasks::ProposalAction::Release { .. } => {}
        }
    }

    /// Admit a task-origin amendment, including ancestry and widening.
    pub fn task_amend_admit(
        &self,
        caller: u64,
        target: u64,
        amendment: &tasks::Amendment,
        depth: u32,
        limits: &tasks::Limits,
    ) -> Result<bool, TaskAmendDenied> {
        if amendment.release.is_some() {
            return Err(TaskAmendDenied::Refused { task: target, why: tasks::Refusal::Read });
        }
        let Some(holder) = self.tasks.delegation(caller) else {
            return Err(TaskAmendDenied::Refused { task: caller, why: tasks::Refusal::Unknown });
        };
        let Some(current) = self.tasks.delegation(target) else {
            return Err(TaskAmendDenied::Refused { task: target, why: tasks::Refusal::Unknown });
        };
        let mut next = target;
        let mut descendant = false;
        for _ in 0..=depth {
            if next == caller {
                descendant = true;
                break;
            }
            let Some(context) = self.tasks.delegation(next) else { break };
            match context.requester {
                tasks::Party::Task(parent) => next = parent,
                tasks::Party::Person(_) | tasks::Party::Deployment { .. } => break,
            }
        }
        if target == caller || current.project != holder.project || !descendant {
            return Err(TaskAmendDenied::Refused { task: target, why: tasks::Refusal::Reference });
        }
        let row = self.tasks.task(target).expect("delegation found target");
        if let Err(why) = tasks::address_admit(row, &amendment.addressed) {
            return Err(TaskAmendDenied::Refused { task: target, why });
        }
        if let Err(why) = tasks::instruction_admit(limits, row.executor, &amendment.instructions) {
            return Err(TaskAmendDenied::Refused { task: target, why });
        }
        if let Err(why) = self.amendment_account(target, amendment, None) {
            return Err(TaskAmendDenied::Refused { task: target, why });
        }
        let Some(after) = &amendment.authority else { return Ok(false) };
        let before = translate::authority_value(&current.authority);
        let after = translate::authority_value(after);
        let implies = &self.authority.rules().implies;
        let stop_run = !authority::at_most(&before, &after, implies);
        if !authority::at_most(&after, &before, implies) {
            let hard = match self.authority.policy(current.project) {
                Some(policy) => {
                    authority::at_most(&after, &policy.ceiling, implies)
                        && authority::at_most(&after, &self.authority.rules().ceiling, implies)
                }
                None => false,
            };
            let answer = if hard {
                if authority::at_most(&after, &translate::authority_value(&holder.authority), implies) {
                    authority::Answer::Allow
                } else {
                    authority::Answer::Propose
                }
            } else {
                authority::Answer::Refuse
            };
            if answer != authority::Answer::Allow {
                return Err(TaskAmendDenied::Denied(answer));
            }
        }
        Ok(stop_run)
    }

    /// A priority alone does not compare the task's specification revision.
    pub(crate) fn party_amend_revision(
        &self,
        task: u64,
        revision: u64,
        amendment: &tasks::Amendment,
    ) -> Result<(), people::Refusal> {
        Self::party_amend_revision_in(task, revision, amendment, &tasks::CompoundAdmission::new(&self.tasks, 0))
    }
    pub(crate) fn party_amend_revision_in(
        task: u64,
        revision: u64,
        amendment: &tasks::Amendment,
        admission: &tasks::CompoundAdmission<'_>,
    ) -> Result<(), people::Refusal> {
        let tracked = match admission.task(task) {
            Some(row) => row.tracked.is_some(),
            None => false,
        };
        let priority = match amendment.tracked {
            Some(tasks::Tracking::Tracked { .. }) => true,
            Some(tasks::Tracking::Untracked) | None => false,
        };
        if priority
            && tracked
            && amendment.addressed.is_none()
            && amendment.instructions.is_none()
            && amendment.spec.is_none()
            && amendment.wake.is_none()
            && amendment.dependencies.is_none()
            && amendment.authority.is_none()
            && amendment.llm.is_none()
        {
            Ok(())
        } else {
            Self::spec_revision_in(task, revision, admission)
        }
    }

    /// Return whether to stop a current run and whether widening must be
    /// offered to a policy holder as a proposal.
    #[expect(clippy::too_many_arguments, reason = "one amendment carries its authenticated party, task and limits")]
    pub fn amend_admit(
        &self,
        person: u64,
        role: Option<people::Role>,
        project: u32,
        task: u64,
        amendment: &tasks::Amendment,
        depth: u32,
        limits: &tasks::Limits,
    ) -> Result<bool, people::Refusal> {
        crate::acts::bounds::amendment_admit(self, person, role, project, task, amendment, depth, limits)
    }
}

/// One admission outcome translated by the application into its call answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskAmendDenied {
    /// Invalid task, reference or caller.
    Refused { task: u64, why: tasks::Refusal },
    /// Authority did not allow widening directly.
    Denied(authority::Answer),
}

fn wake_rule(value: people::WakeRule) -> tasks::WakeRule {
    match value {
        people::WakeRule::Never => tasks::WakeRule::Never,
        people::WakeRule::Immediate => tasks::WakeRule::Immediate,
        people::WakeRule::Batch { count, age } => tasks::WakeRule::Batch { count, age },
    }
}

pub(crate) fn wake(value: people::WakePolicy) -> tasks::WakePolicy {
    tasks::WakePolicy {
        words: wake_rule(value.words),
        notices: wake_rule(value.notices),
        news: wake_rule(value.news),
        results: match value.results {
            people::ResultsWake::Never => tasks::ResultsWake::Never,
            people::ResultsWake::Each => tasks::ResultsWake::Each,
            people::ResultsWake::LastOrFailure => tasks::ResultsWake::LastOrFailure,
        },
        questions: value.questions,
        answers: value.answers,
        timers: value.timers,
    }
}

pub(crate) fn spec(value: people::Spec, limits: &tasks::Limits) -> Result<tasks::Spec, people::Refusal> {
    if value.parameters.len() > usize::try_from(limits.parameters).expect("parameter ceiling fits usize") {
        return Err(numeric(people::LimitKind::Spec, people::LimitKey::TasksParameters, limits.parameters));
    }
    let mut parameters = List::with_capacity(limits.parameters);
    for parameter in value.parameters {
        let translated = match parameter {
            people::Parameter::Number { name, value } => tasks::Parameter::Number { name, value },
            people::Parameter::Bytes { name, value } => tasks::Parameter::Bytes { name, value },
            people::Parameter::Resource { name, connector, resource } => {
                tasks::Parameter::Resource { name, connector, resource }
            }
        };
        parameters.push(translated).expect("parameter count admitted");
    }
    Ok(tasks::Spec { words: value.words, parameters: parameters.into_boxed(), inputs: value.inputs })
}

/// A party's given authority in the task vocabulary, with the models the task takes: a party names
/// none, so a created task takes its creator's role's and an amended one keeps its own
/// (domain/authority.md, sections 3 and 6).
pub(crate) fn granted(
    value: people::Authority,
    models: tasks::Models,
    limits: &tasks::Limits,
) -> Result<tasks::Authority, people::Refusal> {
    authority_room(&value, limits)?;
    let mut grants = List::with_capacity(limits.authority_grants);
    for grant in value.grants {
        let mut segments = List::with_capacity(limits.authority_segments);
        for segment in grant.pattern.segments {
            segments.push(segment).expect("authority shape admitted");
        }
        let last = match grant.pattern.last {
            people::Last::Exact(word) => tasks::Last::Exact(word),
            people::Last::Open(word) => tasks::Last::Open(word),
        };
        grants
            .push(tasks::Grant {
                connector: grant.connector,
                kind: grant.kind,
                pattern: tasks::Pattern { segments: segments.into_boxed(), last },
            })
            .expect("authority shape admitted");
    }
    let mut note_resources = List::with_capacity(limits.authority_grants);
    for scope in value.note_resources {
        let mut segments = List::with_capacity(limits.authority_segments);
        for segment in scope.pattern.segments {
            segments.push(segment).expect("authority shape admitted");
        }
        let last = match scope.pattern.last {
            people::Last::Exact(word) => tasks::Last::Exact(word),
            people::Last::Open(word) => tasks::Last::Open(word),
        };
        note_resources
            .push(tasks::ResourceScope {
                connector: scope.connector,
                pattern: tasks::Pattern { segments: segments.into_boxed(), last },
            })
            .expect("authority shape admitted");
    }
    let mut kinds = List::with_capacity(limits.executor_kinds);
    for kind in value.delegation.kinds {
        kinds
            .push(match kind {
                people::Executor::Charter(number) => tasks::AuthorityExecutor::Charter(number),
                people::Executor::Procedure(number) => tasks::AuthorityExecutor::Procedure(number),
                people::Executor::Role(number) => tasks::AuthorityExecutor::Role(number),
            })
            .expect("authority shape admitted");
    }
    Ok(tasks::Authority {
        tools: tasks::Tools(value.tools),
        models: match value.models {
            Some(value) => tasks::Models::from_parts(value.bits(), value.is_all()),
            None => models,
        },
        grants: grants.into_boxed(),
        delegation: tasks::Delegation {
            kinds: kinds.into_boxed(),
            tasks: value.delegation.tasks,
            depth: value.delegation.depth,
        },
        budget: tasks::Budget { spend: value.spend, deadline: value.deadline },
        notes: tasks::Scopes(value.notes),
        note_resources: note_resources.into_boxed(),
    })
}

/// A party's amendment in the task vocabulary; a changed authority keeps the task's `models`.
#[expect(clippy::manual_map, reason = "the step subset spells out both option cases without a closure")]
pub(crate) fn translate(
    value: people::Amendment,
    models: tasks::Models,
    limits: &tasks::Limits,
) -> Result<tasks::Amendment, people::Refusal> {
    Ok(tasks::Amendment {
        tracked: match value.tracked {
            Some(people::Tracking::Tracked { priority }) => Some(tasks::Tracking::Tracked { priority }),
            Some(people::Tracking::Untracked) => Some(tasks::Tracking::Untracked),
            None => None,
        },
        addressed: match value.addressed {
            Some(change) => Some(tasks::AddressChange {
                revision: change.revision,
                address: match change.address {
                    people::PersonAddress::Person(person) => tasks::PersonAddress::Person(person),
                    people::PersonAddress::Role(role) => tasks::PersonAddress::Role(role),
                },
            }),
            None => None,
        },
        release: None,
        instructions: value.instructions,
        spec: match value.spec {
            Some(specification) => Some(spec(specification, limits)?),
            None => None,
        },
        wake: match value.wake {
            Some(policy) => Some(wake(policy)),
            None => None,
        },
        dependencies: value.dependencies,
        authority: match value.authority {
            Some(authority) => Some(granted(authority, models, limits)?),
            None => None,
        },
        llm: crate::llm::choice(value.llm.as_ref()),
        reason: value.reason,
    })
}

fn numeric(which: people::LimitKind, key: people::LimitKey, value: u32) -> people::Refusal {
    people::Refusal::configured(which, u64::from(value), key, people::LimitUnit::Items)
}
fn authority_room(value: &people::Authority, limits: &tasks::Limits) -> Result<(), people::Refusal> {
    if value.grants.len() > usize::try_from(limits.authority_grants).expect("grant ceiling fits")
        || value.note_resources.len() > usize::try_from(limits.authority_grants).expect("grant ceiling fits")
    {
        return Err(numeric(
            people::LimitKind::Authority,
            people::LimitKey::TasksAuthorityGrants,
            limits.authority_grants,
        ));
    }
    if value.delegation.kinds.len() > usize::try_from(limits.executor_kinds).expect("executor ceiling fits") {
        return Err(numeric(people::LimitKind::Authority, people::LimitKey::TasksExecutorKinds, limits.executor_kinds));
    }
    for grant in &value.grants {
        if grant.pattern.segments.len() > usize::try_from(limits.authority_segments).expect("segment ceiling fits") {
            return Err(numeric(
                people::LimitKind::Authority,
                people::LimitKey::TasksAuthoritySegments,
                limits.authority_segments,
            ));
        }
    }
    for scope in &value.note_resources {
        if scope.pattern.segments.len() > usize::try_from(limits.authority_segments).expect("segment ceiling fits") {
            return Err(numeric(
                people::LimitKind::Authority,
                people::LimitKey::TasksAuthoritySegments,
                limits.authority_segments,
            ));
        }
    }
    Ok(())
}
