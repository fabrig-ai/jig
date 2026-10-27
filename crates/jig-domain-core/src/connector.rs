//! Connector vocabulary crossing the application's root in jig's terms.
//! The root translates numbered connectors' own types to these values;
//! connector-owned payloads remain with their owner (domain/connectors.md, 2).

use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{List, Queue, Token, Wall};

use crate::{Core, Family, fresh, translate};

/// Core-authorized task identity and funding for one connector repair.
#[derive(Debug)]
pub struct RepairSeed {
    pub(crate) number: u64,
    pub(crate) project: u32,
    pub(crate) period: u64,
    pub(crate) period_budget: u64,
    pub(crate) open_period: bool,
    pub(crate) authority: tasks::Authority,
    pub(crate) budget: u64,
}

impl RepairSeed {
    /// The allocated task number for root-owned connector holding translation.
    #[must_use]
    pub const fn number(&self) -> u64 {
        self.number
    }
}

/// What one adopted resource permits this project to do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ResourceRole {
    /// The deployment owns the resource.
    Owned,
    /// It writes as a participant of a shared resource.
    Participant,
    /// It may read, but may not write.
    Context,
    /// It is known but unavailable.
    Unavailable,
}

/// How a named resource is held while a task uses it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HoldKind {
    /// No task hold is needed.
    Shared,
    /// One task holds the resource at a time.
    Exclusive { wait: bool },
    /// At most the current number of slots may be held.
    Pooled { slots: u32, wait: bool },
}

/// What a connector's effect does to its resource.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EffectForm {
    Creation,
    Transition,
    Set,
}

/// Promise available after an effect's uncertain outcome.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Recovery {
    Keyed,
    Conditional,
    Idempotent,
    Unrecoverable,
}

/// The inspectable part of a connector-owned effect.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EffectDescription {
    /// Whether this kind waits while its task has pending dependencies.
    pub waits_for_dependencies: bool,
    /// Connector number assigned by the application root.
    pub connector: u16,
    /// Stable purpose within the asking task.
    pub purpose: u64,
    /// Authority's resource and state description.
    pub effect: authority::Effect,
    /// Effect's shape for outbox and recovery.
    pub form: EffectForm,
    /// Connector's declared recovery class.
    pub recovery: Recovery,
}

/// A connector's answer in jig's vocabulary, after root translation.
#[derive(Debug)]
pub enum Event {
    Resource {
        project: u32,
        name: tasks::Name,
        role: ResourceRole,
        hold: HoldKind,
    },
    PoolSlots {
        name: tasks::Name,
        slots: u32,
    },
    Described {
        owner: Token,
        description: Box<EffectDescription>,
    },
    DescribeRefused {
        owner: Token,
    },
    DescribeBusy {
        owner: Token,
    },
    Verdict {
        owner: Token,
        judge: authority::Judge,
        verdict: authority::Verdict,
        at: Wall,
        guarded: bool,
        state: [u8; 32],
    },
    Procedure {
        task: u64,
        step: u64,
        decision: Box<tasks::ProcedureDecision>,
    },
    Outbox {
        /// Number of the connector that owns this outbox entry.
        connector: u16,
        entry: u64,
        task: u64,
        outcome: OutboxOutcome,
    },
    /// A projection's outcome settles writers independently of any task or call.
    ProjectionOutbox {
        connector: u16,
        entry: u64,
        outcome: OutboxOutcome,
    },
    News {
        task: u64,
        subscription: u64,
        topic: u64,
        class: tasks::NewsClass,
        words: Box<[u8]>,
    },
    SectionReady {
        section: Token,
        size: Option<u32>,
    },
    WorkspaceReady {
        task: u64,
        size: u32,
    },
    Adopted {
        owner: Token,
        project: u32,
        role: people::ResourceRole,
    },
    Drift {
        task: u64,
        resource: tasks::Name,
    },
    /// This connector has settled every effect of a closing task.
    Closed {
        task: u64,
        connector: u16,
    },
    RestartDone {
        stage: RestartStage,
    },
}

/// An outbox terminal or uncertain write.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum OutboxOutcome {
    Made,
    Failed,
    Withdrawn,
    /// A person settled an uncertain effect as not made.
    NotMade,
    Uncertain,
    /// The connector cannot safely retry this entry; a person must resolve it.
    Held {
        entry: u64,
    },
}

/// The connector's place in the core-owned restart order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RestartStage {
    Restored,
    ReadAfresh,
    Settled,
}

/// One party the connector learned while adopting a resource.
#[derive(Debug)]
pub struct AdoptedParty {
    pub subject: Box<[u8]>,
    pub role: people::Role,
}

/// What the core asks a numbered connector to do.
#[derive(Debug)]
pub enum Ask {
    Names {
        task: u64,
        resources: Box<[tasks::Name]>,
    },
    Unname {
        task: u64,
    },
    Hold {
        task: u64,
        resource: tasks::Name,
        from: Option<u64>,
    },
    ReleaseHold {
        task: u64,
        resource: tasks::Name,
    },
    Writer {
        task: u64,
        attempt: u64,
        resources: Box<[tasks::Name]>,
    },
    Describe {
        owner: Token,
    },
    /// Stage the payload durably retained under this proposal's number.
    DescribeProposal {
        owner: Token,
        proposal: u64,
    },
    /// Retain an explicitly proposed payload without making an outbox entry.
    KeepProposal {
        owner: Token,
        proposal: u64,
        task: u64,
        project: u32,
    },
    /// Remove the connector payload of a terminal proposal.
    DropProposal {
        proposal: u64,
    },
    Keep {
        owner: Token,
        entry: u64,
        task: u64,
        key: crate::EffectKey,
    },
    Drop {
        owner: Token,
        answer: authority::Answer,
    },
    Make {
        entry: u64,
    },
    /// Settle a held entry as not made, or prepare a new attempt, within the release decision.
    ResolveUncertain {
        entry: u64,
        again: bool,
    },
    Judge {
        owner: Token,
        judge: authority::Judge,
        state: [u8; 32],
        resources: Box<[authority::Name]>,
    },
    ProcedureMade {
        task: u64,
    },
    ProcedureActivate {
        task: u64,
    },
    ProcedureMessage {
        task: u64,
        words: Box<[u8]>,
    },
    ProcedureClose {
        task: u64,
    },
    Project {
        goal: u64,
    },
    Release {
        task: u64,
    },
    Subscribe {
        task: u64,
        topic: u64,
    },
    Unsubscribe {
        task: u64,
        topic: u64,
    },
    Read {
        owner: Token,
    },
    Gather {
        section: Token,
        budget: u32,
    },
    CutTo {
        section: Token,
        size: u32,
    },
    Take {
        section: Token,
    },
    Prepare {
        task: u64,
        attempt: u64,
    },
    Left {
        task: u64,
        attempt: u64,
    },
    Adopt {
        owner: Token,
        project: u32,
        resource: tasks::Name,
    },
    Restore {
        owner: Token,
    },
    ReadAfresh,
    SettleOutbox,
}

impl Core {
    /// Whether a project or deployment effect requirement names this
    /// connector's landing gate on the resource.
    #[must_use]
    pub fn connector_gate_required(
        &self,
        project: u32,
        connector: u16,
        name: &authority::Name,
        parameters: u32,
        project_scope: bool,
    ) -> bool {
        let requirements = if project_scope {
            match self.authority.policy(project) {
                Some(policy) => &policy.requirements,
                None => return false,
            }
        } else {
            &self.authority.rules().requirements
        };
        for requirement in requirements.as_ref() {
            if requirement.connector == connector
                && requirement.kind == 4
                && requirement.judge.connector == connector
                && requirement.judge.requirement == 3
                && requirement.judge.parameters == parameters
                && authority::pattern_covers(&requirement.pattern, name)
            {
                return true;
            }
        }
        false
    }

    /// Bound a project-funded connector repair by current policy, period
    /// funding and the deployment's maximum run spend.
    #[must_use]
    pub fn connector_repair_authority(&self, project: u32) -> Option<authority::Authority> {
        let policy = self.authority.policy(project)?;
        let mut given = policy.ceiling.clone();
        let period = self.period(project);
        let available = match self.tasks.funding(tasks::Funder::Period { project, period }) {
            Some(funding) => funding
                .numbers
                .budget
                .saturating_sub(funding.numbers.spent)
                .saturating_sub(funding.numbers.spent_below)
                .saturating_sub(funding.numbers.reserved),
            None => self.period_budget(project),
        };
        given.budget.spend = given.budget.spend.min(available).min(self.authority.rules().maximum_run_spend);
        if given.budget.spend == 0 { None } else { Some(given) }
    }

    /// Allocate a policy-bounded connector repair before its root translates holdings.
    pub fn repair_seed(&mut self, project: u32) -> Option<RepairSeed> {
        let authority = self.connector_repair_authority(project)?;
        let number = fresh(&mut self.counters, Family::Task)?;
        let period = self.period(project);
        Some(RepairSeed {
            number,
            project,
            period,
            period_budget: self.period_budget(project),
            open_period: self.tasks.funding(tasks::Funder::Period { project, period }).is_none(),
            budget: authority.budget.spend,
            authority: translate::task_authority(&authority),
        })
    }

    /// Allocate a deployment-started procedure task that runs no agent
    /// (domain/authority.md, 6): the project's ceiling authority, a budget of zero,
    /// so a period reserved whole never stops it. None only when the project
    /// has no policy or no task number is left.
    pub fn procedure_seed(&mut self, project: u32) -> Option<RepairSeed> {
        let mut authority = self.authority.policy(project)?.ceiling.clone();
        authority.budget.spend = 0;
        let number = fresh(&mut self.counters, Family::Task)?;
        let period = self.period(project);
        Some(RepairSeed {
            number,
            project,
            period,
            period_budget: self.period_budget(project),
            open_period: self.tasks.funding(tasks::Funder::Period { project, period }).is_none(),
            budget: 0,
            authority: translate::task_authority(&authority),
        })
    }

    /// Deployment procedures with no task budget spend only at the effect,
    /// from the current period (domain/authority.md, 6–7). These durable facts
    /// distinguish them from procedures funded by a task or a person's pool.
    pub(crate) fn procedure_effect_funder(&self, task: u64) -> Option<tasks::Funder> {
        let row = self.tasks.task(task)?;
        let procedure = match row.executor {
            tasks::Executor::Procedure { .. } => true,
            tasks::Executor::Agent { .. } | tasks::Executor::Person(_) => false,
        };
        let funded = match row.funder {
            tasks::Funder::Period { project, .. } => project == row.project,
            tasks::Funder::Task(_) | tasks::Funder::Pool { .. } | tasks::Funder::Recurring { .. } => false,
        };
        if row.requester != (tasks::Party::Deployment { project: row.project })
            || !procedure
            || row.numbers.budget != 0
            || row.authority.budget.spend != 0
            || !funded
        {
            return None;
        }
        Some(tasks::Funder::Period { project: row.project, period: self.period(row.project) })
    }

    /// Whether a live task may address a resource adopted into this project.
    #[must_use]
    pub fn connector_project(&self, task: u64, project: u32) -> bool {
        match self.tasks.delegation(task) {
            Some(context) => context.project == project,
            None => false,
        }
    }

    /// Reserve identities and admit the connector's adopted parties as one
    /// atomic seed for the people child.
    pub fn connector_adopt_parties(
        &mut self,
        limits: &people::Limits,
        project: u32,
        provider: u16,
        parties: Box<[AdoptedParty]>,
    ) -> Result<Box<[people::Seed]>, people::Outcome> {
        let Ok(capacity) = u32::try_from(parties.len()) else {
            return Err(people::Outcome::Refused(people::Refusal::Limit { which: people::LimitKind::Roles }));
        };
        let mut seeds = List::with_capacity(capacity);
        let mut exhausted = false;
        for party in parties {
            match fresh(&mut self.counters, Family::Person) {
                Some(candidate) => {
                    seeds
                        .push(people::Seed {
                            identity: people::IdentityKey { provider, subject: party.subject },
                            candidate,
                            role: party.role,
                        })
                        .expect("bounded adopted parties");
                }
                None => exhausted = true,
            }
        }
        if exhausted || !self.people.can_seed(limits, project, seeds.as_slice()) {
            Err(people::Outcome::Again(people::Again::Busy))
        } else {
            Ok(seeds.into_boxed())
        }
    }

    /// Lower connector-classified news to the task hub, with a core-owned
    /// durable message number. Dropped news makes no task event.
    pub fn connector_news(
        &mut self,
        task: u64,
        subscription: u64,
        class: tasks::NewsClass,
        words: Box<[u8]>,
        at: Wall,
    ) -> Option<tasks::Event> {
        match class {
            tasks::NewsClass::Dropped => None,
            tasks::NewsClass::Wakes | tasks::NewsClass::Kept => {
                let number = fresh(&mut self.counters, Family::Message).expect("news number admitted");
                Some(tasks::Event::Notice {
                    task,
                    word: tasks::Word {
                        number,
                        from: tasks::Party::Task(task),
                        kind: tasks::MessageKind::News { subscription, class },
                        words,
                        at,
                        hits: 1,
                        eligible: false,
                    },
                })
            }
        }
    }

    /// Decide whether a connector's settled write holds or wakes its task.
    #[must_use]
    pub fn connector_outbox(
        &self,
        task: u64,
        connector: u16,
        entry: u64,
        outcome: OutboxOutcome,
        procedure: bool,
    ) -> Option<tasks::Event> {
        let cancelling = match self.tasks.task(task) {
            Some(row) => match &row.phase {
                tasks::Phase::Closing(closing) | tasks::Phase::Held { was: tasks::Was::Closing(closing), .. } => {
                    match &closing.ending {
                        tasks::Ending::Cancelled { .. } => true,
                        tasks::Ending::Done(_) | tasks::Ending::Failed { .. } => false,
                    }
                }
                tasks::Phase::Waiting
                | tasks::Phase::Active(_)
                | tasks::Phase::Held { .. }
                | tasks::Phase::Ended(_) => false,
            },
            None => false,
        };
        match outcome {
            OutboxOutcome::Held { entry } => Some(tasks::Event::HoldFrom {
                task,
                why: tasks::Hold::Uncertain { entry },
                source: Box::new(tasks::HoldSource::Effect { connector, entry }),
            }),
            OutboxOutcome::Failed if !procedure => Some(tasks::Event::HoldFrom {
                task,
                why: tasks::Hold::EffectFailed,
                source: Box::new(tasks::HoldSource::Effect { connector, entry }),
            }),
            OutboxOutcome::Withdrawn if !cancelling && !procedure => Some(tasks::Event::HoldFrom {
                task,
                why: tasks::Hold::EffectFailed,
                source: Box::new(tasks::HoldSource::Effect { connector, entry }),
            }),
            OutboxOutcome::Made | OutboxOutcome::Failed | OutboxOutcome::Withdrawn | OutboxOutcome::NotMade
                if procedure =>
            {
                Some(tasks::Event::WakeProcedure { task })
            }
            OutboxOutcome::Made
            | OutboxOutcome::Failed
            | OutboxOutcome::Withdrawn
            | OutboxOutcome::NotMade
            | OutboxOutcome::Uncertain => None,
        }
    }

    /// Decide a whole child batch from an agent or connector procedure.
    pub fn connector_batch_admit(
        &self,
        project: u32,
        creator: &tasks::Authority,
        numbers: tasks::Numbers,
        tasks_left: u32,
        members: Box<[authority::Delegate]>,
        findings: &mut Queue<authority::Finding>,
    ) -> authority::Checked {
        authority::check_batch(
            &self.authority,
            &authority::BatchAsk {
                project,
                creator: translate::authority_value(creator),
                numbers: translate::authority_numbers(numbers),
                tasks_left,
                tasks: members,
            },
            findings,
        )
    }

    /// Name the requirements whose connector facts must be supplied before
    /// this effect can be decided.
    #[must_use]
    pub fn connector_needed_judges(&self, project: u32, effect: &authority::Effect) -> Option<List<authority::Judge>> {
        authority::needed_judges(&self.authority, project, effect)
    }

    /// Check a connector-described projection against live project grants and
    /// requirements, independently of goal grants and funding (domain/authority.md, 6).
    pub fn connector_goal_effect_admit(
        &self,
        project: u32,
        description: &EffectDescription,
        now: Wall,
        given: &[authority::Given],
        findings: &mut Queue<authority::Finding>,
    ) -> authority::Answer {
        if description.connector != description.effect.connector {
            return authority::Answer::Refuse;
        }
        authority::check_projection(
            &self.authority,
            project,
            &crate::resources::effect(self, project, description.effect.clone()),
            now,
            given,
            findings,
        )
    }

    /// Check the write named by a connector for a run being prepared.
    pub fn connector_run_write_admit(
        &self,
        context: &tasks::RunContext,
        effect: authority::Effect,
        now: Wall,
        findings: &mut Queue<authority::Finding>,
    ) -> authority::Answer {
        let Some(current) = self.tasks.run_context(context.task) else { return authority::Answer::Wait };
        let numbers = translate::authority_numbers(current.numbers);
        let Ok(choice) = self.run_choice(context.task) else { return authority::Answer::Wait };
        let mut accounts = List::with_capacity(tasks::MODELS);
        for binding in &choice.accounts {
            accounts
                .push(authority::AccountCheck { binding: *binding, usable: true, host_eligible: true })
                .expect("checked providers");
        }
        authority::check_run(
            &self.authority,
            &authority::RunAsk {
                project: current.project,
                authority: translate::authority_value(&current.authority),
                numbers,
                budget: authority::left(numbers).min(self.authority.rules().maximum_run_spend),
                reserve: self.run_reserve(context.task),
                wall: now,
                main_provider: choice.accounts.first().expect("choice includes main provider").provider,
                accounts: accounts.into_boxed(),
                credentials: self.run_credentials_check(context.task),
                writes: Box::new([authority::Write {
                    effect: crate::resources::effect(self, current.project, effect),
                    held: authority::Writer::Task,
                }]),
            },
            findings,
        )
        .verdict()
    }

    /// Decide authority for a connector-described effect. The root gives
    /// verdicts only after asking the named judges through their connectors.
    pub fn connector_effect_admit(
        &self,
        task: u64,
        description: &EffectDescription,
        now: Wall,
        verdicts: &[authority::Given],
        findings: &mut Queue<authority::Finding>,
    ) -> authority::Answer {
        if description.connector != description.effect.connector {
            return authority::Answer::Refuse;
        }
        let Some(context) = self.tasks.delegation(task) else { return authority::Answer::Refuse };
        let period = self.procedure_effect_funder(task);
        let mut given = translate::authority_value(&context.authority);
        let mut numbers = translate::authority_numbers(context.numbers);
        if period.is_some() {
            // Check every permission and both ceilings at the described price;
            // this task has no allotment to widen. Actual funds are checked below.
            let maximum = description.effect.price.unwrap_or(0);
            given.budget.spend = maximum;
            numbers = authority::Numbers { budget: maximum, spent: 0, spent_below: 0, reserved: 0 };
        }
        let answer = authority::check_effect(
            &self.authority,
            &authority::EffectAsk {
                project: context.project,
                authority: given,
                numbers,
                effect: crate::resources::effect(self, context.project, description.effect.clone()),
                now,
            },
            verdicts,
            findings,
        );
        if answer == authority::Answer::Allow
            && let Some(funder) = period
        {
            let available = match self.tasks.funding(funder) {
                Some(row) => authority::left(translate::authority_numbers(row.numbers)),
                None => 0,
            };
            if description.effect.price.unwrap_or(0) > available {
                return authority::Answer::Wait;
            }
        }
        answer
    }

    /// Decide authority for a connector read described in jig's terms.
    pub fn connector_read_admit(
        &self,
        task: u64,
        effect: authority::Effect,
        findings: &mut Queue<authority::Finding>,
    ) -> authority::Answer {
        let Some(context) = self.tasks.delegation(task) else { return authority::Answer::Refuse };
        authority::check_call(
            &self.authority,
            &authority::CallAsk {
                project: context.project,
                authority: translate::authority_value(&context.authority),
                family: self.settings.tools.read,
                call: authority::Call::Read(effect),
            },
            findings,
        )
    }
}

impl Core {
    /// An offered topic needs subscribe and a read grant on every described resource.
    pub fn topic_admit(
        &self,
        key: crate::CallKey,
        resources: &[authority::Effect],
        findings: &mut Queue<authority::Finding>,
    ) -> authority::Answer {
        let Some(context) = self.tasks.delegation(key.task) else { return authority::Answer::Refuse };
        if resources.is_empty() {
            return authority::Answer::Refuse;
        }
        for resource in resources {
            let answer = authority::check_call(
                &self.authority,
                &authority::CallAsk {
                    project: context.project,
                    authority: translate::authority_value(&context.authority),
                    family: self.settings.tools.subscribe,
                    call: authority::Call::Read(resource.clone()),
                },
                findings,
            );
            if answer != authority::Answer::Allow {
                return answer;
            }
        }
        authority::Answer::Allow
    }
}
