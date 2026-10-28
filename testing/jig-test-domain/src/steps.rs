//! Reference connector procedure decisions translated into the core's atomic step.
use crate::{Domain, Work};
use alloc::boxed::Box;
use jig_domain_core as core;
use jig_domain_tasks as tasks;
use jig_test_connector as connector;
use skein_lib::{Decimal, List, Queue, Writer};

pub(crate) fn translate(
    domain: &mut Domain,
    task: u64,
    step: u64,
    number: u16,
    decision: connector::StepDecision,
    work: &mut Queue<Work>,
) -> Option<core::ProcedureStep> {
    let end = match decision {
        connector::StepDecision::Finish { result: connector::ProcedureResult::Succeeded } => {
            core::StepEnd::Result(tasks::TaskResult::Report { words: Box::new([]) })
        }
        connector::StepDecision::Finish { result: connector::ProcedureResult::Local { code } } => {
            core::StepEnd::Result(tasks::TaskResult::Change {
                connector: number,
                kind: code,
                resource: 0,
                words: Box::new([]),
            })
        }
        connector::StepDecision::Wait { .. } | connector::StepDecision::Stall => core::StepEnd::Wait,
        connector::StepDecision::Hold { .. } => core::StepEnd::Hold(tasks::Hold::Effects),
        connector::StepDecision::Amend { task, revision, words, priority } => {
            return Some(amendment(task, revision, words, priority));
        }
        connector::StepDecision::Cancel { task, grace } => {
            return Some(core::ProcedureStep {
                cancels: Box::new([core::StepCancel {
                    task,
                    reason: b"procedure cancelled delegate".as_slice().into(),
                    grace: Some(grace),
                }]),
                ..core::ProcedureStep::new(core::StepEnd::Wait)
            });
        }
        connector::StepDecision::Message { task, words } => {
            return Some(core::ProcedureStep {
                messages: Box::new([core::StepMessage {
                    target: task,
                    kind: tasks::MessageKind::Words,
                    words: Decimal::of(words).as_bytes().into(),
                }]),
                ..core::ProcedureStep::new(core::StepEnd::Wait)
            });
        }
        connector::StepDecision::Propose { spend } => {
            let mut authority = domain.core.tasks.task(task).expect("procedure task live").authority.clone();
            authority.budget.spend = authority.budget.spend.saturating_add(spend);
            return Some(core::ProcedureStep {
                proposals: Box::new([tasks::ProposalAction::Widen { task, authority }]),
                ..core::ProcedureStep::new(core::StepEnd::Wait)
            });
        }
        connector::StepDecision::Delegate { kinds } => {
            let row = domain.core.tasks.task(task).expect("procedure task live");
            let capacity = u32::try_from(kinds.len()).expect("bounded program");
            let mut batch = List::with_capacity(capacity);
            let budget = row
                .numbers
                .budget
                .saturating_sub(row.numbers.spent)
                .saturating_sub(row.numbers.spent_below)
                .saturating_sub(row.numbers.reserved)
                .checked_div(u64::from(capacity))
                .unwrap_or(0);
            for kind in kinds {
                let mut authority = row.authority.clone();
                authority.budget.spend = budget;
                authority.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
                batch
                    .push(core::Delegate {
                        deadline: jig_domain_tasks::Deadline::Named,
                        label: 0,
                        flags: jig_domain_tasks::Flags::PLAIN,
                        tracked: None,
                        budget: jig_domain_people::Budget::Named,
                        executor: tasks::Executor::Agent { charter: u32::from(kind) },
                        spec: specification(words(task, step)),
                        contract: tasks::Contract::Report { words: 128 },
                        authority,
                        llm: None,
                        symbolic_grants: Box::new([]),
                        dependencies: Box::new([]),
                        wake: tasks::WakePolicy::DEFAULT,
                    })
                    .expect("one delegate per program kind");
            }
            return Some(core::ProcedureStep {
                batch: batch.into_boxed(),
                ..core::ProcedureStep::new(core::StepEnd::Wait)
            });
        }
        connector::StepDecision::Effect(effect) => {
            let owner = crate::effect_payload(domain, effect);
            domain.effect_procedures.insert(owner, task).expect("one procedure effect flight");
            work.push(Work::Core(core::Event::EffectStart {
                owner,
                connector: number,
                origin: core::EffectOrigin::Procedure { task, step, entry: None },
            }));
            return None;
        }
    };
    Some(core::ProcedureStep::new(end))
}

pub(crate) fn refused(number: u16, task: u64, step: u64, why: core::StepRefusal, work: &mut Queue<Work>) {
    let why = match why {
        core::StepRefusal::Moved { now: jig_domain_people::Moved::Spec { revision }, .. } => {
            connector::StepRefusal::Moved { revision }
        }
        core::StepRefusal::Refused(problem) if problem.why == tasks::Refusal::Kept => connector::StepRefusal::Kept {
            pending: match problem.blocked_by {
                Some(pending) => pending,
                None => Box::new([]),
            },
        },
        core::StepRefusal::Moved { .. } | core::StepRefusal::Refused(_) | core::StepRefusal::Denied { .. } => {
            connector::StepRefusal::Refused
        }
    };
    work.push(Work::Connector { number, event: connector::Event::StepRefused { task, step, why } });
}

fn words(task: u64, step: u64) -> Box<[u8]> {
    let task = Decimal::of(task);
    let step = Decimal::of(step);
    let mut words = Writer::new(
        16_usize
            .checked_add(task.as_bytes().len())
            .expect("bounded task label")
            .checked_add(step.as_bytes().len())
            .expect("bounded step label"),
    );
    words.put(b"procedure ").expect("bounded label");
    words.put(task.as_bytes()).expect("bounded task number");
    words.put(b" step ").expect("bounded label");
    words.put(step.as_bytes()).expect("bounded step number");
    words.finish()
}

fn amendment(task: u64, revision: u64, words: u64, priority: Option<u32>) -> core::ProcedureStep {
    let count = Decimal::of(words);
    let mut text = Writer::new(8_usize.checked_add(count.as_bytes().len()).expect("bounded spec"));
    text.put(b"amended ").expect("bounded spec");
    text.put(count.as_bytes()).expect("bounded word count");
    let mut tracked = None;
    if let Some(priority) = priority {
        tracked = Some(tasks::Tracking::Tracked { priority });
    }
    core::ProcedureStep {
        amendments: Box::new([core::StepAmendment {
            task,
            revision,
            amendment: tasks::Amendment {
                spec: if priority.is_some() { None } else { Some(specification(text.finish())) },
                tracked,
                addressed: None,
                instructions: None,
                release: None,
                wake: None,
                dependencies: None,
                authority: None,
                llm: None,
                reason: Box::new([]),
            },
        }]),
        ..core::ProcedureStep::new(core::StepEnd::Wait)
    }
}

/// Translate a committed procedure delivery into its connector's input.
#[must_use]
pub fn procedure_input(context: &tasks::RunContext, code: u16, resource: connector::Path) -> connector::Event {
    let mut added = List::with_capacity(u32::try_from(context.added.len()).expect("bounded dependencies"));
    for value in &context.added {
        let by = match value.by {
            tasks::Party::Person(person) => connector::application_core::Party::Person(person),
            tasks::Party::Task(task) => connector::application_core::Party::Task(task),
            tasks::Party::Deployment { project } => connector::application_core::Party::Deployment { project },
        };
        added.push(connector::Added { task: value.task, at: value.at, by }).expect("one row per dependency");
    }
    connector::Event::Procedure {
        project: context.project,
        task: context.task,
        number: code,
        resource,
        signal: connector::ProcedureSignal::Activate,
        pending: context.pending.clone(),
        added: added.into_boxed(),
    }
}

fn specification(words: Box<[u8]>) -> tasks::Spec {
    tasks::Spec { asks: false, words, parameters: Box::new([]), inputs: Box::new([]) }
}
