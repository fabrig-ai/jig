use super::*;
use jig_domain_people as p;
pub const PROJECT: u32 = 3;
pub const CHARTER: u32 = 7;
pub const ROLE: u32 = 2;
pub const FIRST_TASK: u64 = 7;
pub const LAST_TASK: u64 = 8;
pub fn create_ask() -> (w::Ask, p::Ask) {
    let (wire, people) = authority_ask();
    let w::Ask::ChangePolicy(wire) = wire else { panic!("policy fixture") };
    let w::PolicyEdit::Role(wire) = wire.into_parts().edit else { panic!("role fixture") };
    let authority = wire.into_parts().value.into_parts().authority;
    let p::Ask::ChangePolicy { change: p::PolicyChange::Role(expected_role), .. } = people else {
        panic!("authority fixture")
    };
    let mut tasks = List::with_capacity(2);
    let mut batch = List::with_capacity(2);
    for (role, deadline) in [(false, None), (true, Some(DEADLINE))] {
        let mut wire = authority.clone().into_parts();
        wire.deadline = None;
        if let Some(wall) = deadline {
            wire.deadline = Some(wall.as_nanos());
        }
        let mut expected = expected_role.authority.clone();
        expected.deadline = deadline;
        let (task, member) = member(role, w::OwnerAuthority::new(&super::wire(), wire).unwrap(), expected);
        tasks.push(task).unwrap();
        batch.push(member).unwrap();
    }
    let mut encoded = List::with_capacity(tasks.len());
    for task in &tasks {
        let mut out = Writer::new(usize::try_from(task.measure()).unwrap());
        task.encode(&mut out).unwrap();
        encoded.push(out.finish()).unwrap();
    }
    let bytes = pieces(encoded.as_slice());

    (
        w::Ask::Create(w::Create::new(&super::wire(), w::CreateParts { project: PROJECT, tasks: bytes }).unwrap()),
        p::Ask::Create { project: PROJECT, batch: batch.into_boxed() },
    )
}
fn member(role: bool, authority: w::OwnerAuthority, expected: p::Authority) -> (w::NewTask, p::Member) {
    let l = wire();
    let dependencies = list([
        w::TaskRef::Member(w::TaskRefMember::new(&l, w::TaskRefMemberParts { value: 1 }).unwrap()),
        w::TaskRef::Task(w::TaskRefTask::new(&l, w::TaskRefTaskParts { value: u64::MAX }).unwrap()),
    ]);
    let spec = w::Specification::new(
        &l,
        w::SpecificationParts {
            words: b"new words".as_slice().into(),
            parameters: list([w::Parameter::Number(
                w::ParameterNumber::new(&l, w::ParameterNumberParts { name: 9, value: 12 }).unwrap(),
            )]),
            inputs: list([77]),
        },
    )
    .unwrap();
    let field = w::FieldRule::new(&l, w::FieldRuleParts { name: b"field".as_slice().into(), max: 13 }).unwrap();
    let choice =
        w::VerdictChoice::new(&l, w::VerdictChoiceParts { code: 5, words: 16, followups: 2, fields: list([field]) })
            .unwrap();
    let executor = if role {
        w::OwnerExecutor::Person(
            w::OwnerExecutorPerson::new(
                &l,
                w::OwnerExecutorPersonParts {
                    value: w::PersonAddress::Role(
                        w::PersonAddressRole::new(&l, w::PersonAddressRoleParts { value: ROLE }).unwrap(),
                    ),
                },
            )
            .unwrap(),
        )
    } else {
        w::OwnerExecutor::Agent(
            w::OwnerExecutorAgent::new(&l, w::OwnerExecutorAgentParts { charter: CHARTER }).unwrap(),
        )
    };
    let task = w::NewTask::new(
        &l,
        w::NewTaskParts {
            llm: if role { None } else { Some(careful_and_quick(&l)) },
            deadline: w::NewDeadline::Named,
            executor,
            spec,
            contract: w::NewContract::Verdict(
                w::ContractVerdict::new(&l, w::ContractVerdictParts { choices: list([choice]) }).unwrap(),
            ),
            authority: authority.clone(),
            budget: w::NewBudget::AsAllowed(
                w::BudgetAsAllowed::new(&l, w::BudgetAsAllowedParts { cap: Some(42) }).unwrap(),
            ),
            wake: Some(
                w::OwnerWakePolicy::new(
                    &l,
                    w::OwnerWakePolicyParts {
                        markers: u32::MAX,
                        words: w::WakeRule::Immediate,
                        notices: w::WakeRule::Immediate,
                        news: w::WakeRule::Immediate,
                        results: w::ResultsWake::LastOrFailure,
                        questions: true,
                        answers: true,
                        timers: true,
                    },
                )
                .unwrap(),
            ),
            dependencies,
            named: authority.grants().into(),
            tracked: Some(14),
            label: 15,
            flags: w::TaskFlags::new(
                &l,
                w::TaskFlagsParts {
                    ended_by: w::EndedBy::Result,
                    conversation: false,
                    listed: true,
                    retention: w::Retention::ForgottenOnceRead,
                },
            )
            .unwrap(),
        },
    )
    .unwrap();
    (task, expected_member(role, expected))
}
fn expected_member(role: bool, expected: p::Authority) -> p::Member {
    p::Member {
        deadline: jig_domain_people::Deadline::Named,
        executor: if role {
            p::TaskExecutor::Person(p::PersonAddress::Role(ROLE))
        } else {
            p::TaskExecutor::Agent { charter: CHARTER }
        },
        words: b"new words".as_slice().into(),
        parameters: Box::new([p::Parameter::Number { name: 9, value: 12 }]),
        inputs: Box::new([77]),
        contract: p::Contract::Verdict {
            choices: Box::new([p::Verdict {
                code: 5,
                words: 16,
                followups: 2,
                fields: Box::new([p::FieldRule { name: b"field".as_slice().into(), max: 13 }]),
            }]),
        },
        authority: expected.clone(),
        budget: p::Budget::AsAllowed { cap: Some(42) },
        wake: p::WakePolicy {
            markers: u32::MAX,
            words: p::WakeRule::Immediate,
            notices: p::WakeRule::Immediate,
            news: p::WakeRule::Immediate,
            results: p::ResultsWake::LastOrFailure,
            questions: true,
            answers: true,
            timers: true,
        },
        dependencies: Box::new([p::Dependency::Batch(1), p::Dependency::Existing(u64::MAX)]),
        named: expected.grants,
        tracked: Some(14),
        label: 15,
        flags: p::Flags {
            ended_by: p::EndedBy::Result,
            conversation: false,
            listed: true,
            retention: p::Retention::ForgottenOnceRead,
        },
        llm: if role {
            None
        } else {
            Some(p::LlmChoice {
                account: None,
                main: p::ModelEffort { model: 0, effort: Some(1) },
                subagents: Box::new([p::ModelEffort { model: 1, effort: None }]),
            })
        },
    }
}
/// The fixture's careful model at its high effort, its sub-agents the quick one.
pub fn careful_and_quick(l: &w::Limits) -> w::LlmChoice {
    llm_choice(l, b"careful", Some(b"high"), b"quick")
}
/// A main model and effort, and one sub-agent model at its default effort, by name.
#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub fn llm_choice(l: &w::Limits, main: &[u8], effort: Option<&[u8]>, subagent: &[u8]) -> w::LlmChoice {
    let main = w::ModelEffort::new(
        l,
        w::ModelEffortParts {
            model: main.into(),
            effort: match effort {
                Some(effort) => Some(effort.into()),
                None => None,
            },
        },
    )
    .unwrap();
    let subagent = w::ModelEffort::new(l, w::ModelEffortParts { model: subagent.into(), effort: None }).unwrap();
    w::LlmChoice::new(l, w::LlmChoiceParts { account: None, main, subagents: list([subagent]) }).unwrap()
}

pub fn create_refusals() -> [(&'static str, p::Refusal, w::Refusal); 5] {
    let l = wire();
    [
        (
            "two budgets",
            p::Refusal::NotWellFormed { at: 1, why: p::Malformed::AsAllowedTwice },
            w::Refusal::Malformed(
                w::RefusalMalformed::new(&l, w::RefusalMalformedParts { at: 1, why: w::Malformed::TwoAsAllowed })
                    .unwrap(),
            ),
        ),
        (
            "person conversation",
            p::Refusal::NotWellFormed { at: 2, why: p::Malformed::Conversation },
            w::Refusal::Malformed(
                w::RefusalMalformed::new(
                    &l,
                    w::RefusalMalformedParts { at: 2, why: w::Malformed::ConversationNotAgent },
                )
                .unwrap(),
            ),
        ),
        (
            "cycle",
            p::Refusal::NotWellFormed { at: 3, why: p::Malformed::Cycle },
            w::Refusal::Malformed(
                w::RefusalMalformed::new(&l, w::RefusalMalformedParts { at: 3, why: w::Malformed::Cycle }).unwrap(),
            ),
        ),
        (
            "proposed AsAllowed",
            p::Refusal::NotWellFormed { at: 4, why: p::Malformed::ProposedAsAllowed },
            w::Refusal::Malformed(
                w::RefusalMalformed::new(&l, w::RefusalMalformedParts { at: 4, why: w::Malformed::ProposedAsAllowed })
                    .unwrap(),
            ),
        ),
        (
            "goals",
            p::Refusal::Limit { which: p::LimitKind::Goals },
            w::Refusal::Limit(
                w::RefusalLimit::new(
                    &l,
                    w::RefusalLimitParts {
                        which: w::Limit::Admission(
                            w::LimitAdmission::new(&l, w::LimitAdmissionParts { which: w::AdmissionLimit::Goals })
                                .unwrap(),
                        ),
                    },
                )
                .unwrap(),
            ),
        ),
    ]
}
pub fn created_outcome() -> w::Outcome {
    let l = wire();
    w::Outcome::Owner(
        w::OutcomeOwner::new(
            &l,
            w::OutcomeOwnerParts {
                value: w::OwnerOutcome::Created(
                    w::OwnerOutcomeCreated::new(
                        &l,
                        w::OwnerOutcomeCreatedParts {
                            first: FIRST_TASK,
                            last: LAST_TASK,
                            given: Some(
                                w::CreatedGiven::new(&l, w::CreatedGivenParts { task: LAST_TASK, amount: 42 }).unwrap(),
                            ),
                        },
                    )
                    .unwrap(),
                ),
            },
        )
        .unwrap(),
    )
}

// A finite wall distinct from policy fixture timestamps.
const DEADLINE: Wall = Wall::from_nanos(101);
