//! Core call rows built from the store fixtures.
use alloc::boxed::Box;
use jig_domain_core as core;
use jig_domain_tasks as tasks;
use skein_lib::Wall;

pub fn call(part: core::CallPart) -> core::CoreRecord {
    core::CoreRecord::Call(core::CallRecord {
        key: core::CallKey { task: 1, attempt: 2, completion: 3, position: 4 },
        part,
        settled: None,
    })
}
pub fn problem() -> tasks::Problem {
    tasks::Problem {
        task: Some(1),
        why: tasks::Refusal::LiveDelegates,
        blocked_by: Some(Box::new([2, 3])),
        capacity: Some(tasks::Capacity {
            project: Some(1),
            by: tasks::Party::Task(1),
            action: tasks::CapacityAction::Batch { first: Some(2), members: 2 },
            cause: tasks::CapacityCause::Tasks { limit: 4 },
        }),
    }
}
pub fn call_parts() -> Box<[core::CallPart]> {
    use core::CallPart as C;
    use jig_domain_authority::{Answer, Finding, Lacks, Source};
    let findings = Box::new([Finding::Authority {
        source: Source::Task,
        lacks: Lacks {
            tools: true,
            grants: false,
            executors: true,
            tasks: false,
            depth: true,
            spend: false,
            deadline: true,
            notes: false,
            models: true,
        },
    }]);
    Box::new([
        C::Effect {
            connector: 1,
            entry: 2,
            deadline: Wall::from_nanos(17),
            outcome: Some(core::connector::OutboxOutcome::Held { entry: 2 }),
        },
        C::Connector { connector: 1 },
        C::EffectDenied { answer: Answer::Refuse, findings: findings.clone() },
        C::ToolDenied { answer: Answer::Wait, findings: findings.clone() },
        C::EscalationDecided { task: 1, revision: 2, outcome: tasks::EscalationOutcome::Passed { revision: 2 } },
        C::EscalationRefused(problem()),
        C::EffectRefused(tasks::Problem::naming(Some(1), tasks::Refusal::Kept, &[2, 3])),
        C::Proposed { proposal: 2, holder: jig_domain_tasks::ProposalHolder::Person(1) },
        C::ProposalDecided { proposal: 2, outcome: tasks::ProposalOutcome::Accepted },
        C::ProposalRefused(problem()),
        C::SettingsRefused(jig_domain_people::Refusal::Stale { newest: 3 }),
        C::SettingsRefused(jig_domain_people::Refusal::Limit {
            which: jig_domain_people::LimitKind::SettingsProposals,
        }),
        C::Controlled,
        C::ControlRefused(problem()),
        C::ControlDenied { answer: Answer::Propose },
        C::Sent { message: 3 },
        C::Introduced,
        C::MessageRefused(problem()),
        C::Subscribed { subscription: 4 },
        C::Unsubscribed,
        C::SubscriptionRefused(problem()),
        C::Delegated(Box::new([
            jig_domain_core::Made { task: 2, revision: 0, given: None },
            jig_domain_core::Made { task: 3, revision: 0, given: Some(40) },
        ])),
        C::DelegationDenied { answer: Answer::Refuse, findings },
        C::DelegationRefused(problem()),
        C::NoteWritten { name: 1, revision: 2 },
        C::NoteRecalled { entries: Box::new([super::note_entry()]), more: true },
        C::NoteRefused(jig_domain_notes::Refusal::Busy),
        C::Unavailable,
    ])
}
