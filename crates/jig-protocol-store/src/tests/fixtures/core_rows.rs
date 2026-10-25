//! Core archive and projection fixtures (protocol/store.md, 5.1).
use jig_domain_core as core;
use jig_domain_fleet::HostKind;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::Wall;

pub fn deployment() -> core::Deployment {
    core::Deployment {
        id: [7; 16],
        tasks: 11,
        people: 12,
        sign_ins: 13,
        messages: 14,
        runs: 15,
        calls: 16,
        connector_rows: 17,
        projects: 20,
        applications: 18,
        commits: 19,
    }
}
pub fn terminal() -> core::TerminalRecord {
    core::TerminalRecord {
        task: 11,
        attempt: 15,
        cumulative: 17,
        end: tasks::End::Finished {
            result: tasks::TaskResult::Report { words: Box::from(b"finished".as_slice()) },
            cancel_delegates: true,
        },
    }
}
pub fn run() -> core::RunProof {
    core::RunProof {
        accounts: Box::new([
            jig_domain_authority::AccountBinding { provider: 2, account: 5, source: 7 },
            jig_domain_authority::AccountBinding { provider: 3, account: 11, source: 13 },
        ]),
        placement: Some(skein_lib::Token::new(17)),
        account_failure: Some(7),
        account_terminal: true,
        task: 11,
        attempt: 15,
        conversation: 9,
        credentials: Box::new([2, 3]),
        window: Some(core::CurrentWindow { attempt: 14, turn: 2, closed: false }),
        host: HostKind::Worker,
        offered: Some(13),
        outstanding: Box::new([13]),
        turn: Some(core::TurnProof { turn: 3, cumulative: 17, read: Some(12) }),
        terminal: Some(terminal()),
    }
}
pub fn turn() -> core::TurnRecord {
    core::TurnRecord {
        opens_window: true,
        task: 11,
        attempt: 15,
        turn: 3,
        spent: 17,
        read: Some(12),
        at: Wall::from_nanos(19),
        transcript: Box::new([0, 1, 254, 255]),
    }
}
pub fn goal() -> core::ProjectionRecord {
    core::ProjectionRecord::Goal {
        goal: core::ProjectionGoal {
            number: 11,
            project: 2,
            priority: 7,
            words: Box::from(b"goal".as_slice()),
            phase: super::task_phase(),
        },
        pending: Box::new([2, 3]),
        changed: true,
        fed_at: Some(19),
        closing_at: None,
    }
}
pub fn projection_task(requester: tasks::Party) -> core::ProjectionRecord {
    core::ProjectionRecord::Task {
        goal: 11,
        task: core::ProjectionTask {
            number: 12,
            requester,
            words: Box::from(b"task".as_slice()),
            phase: super::task_phase(),
        },
    }
}
pub fn milestone(identity: core::MilestoneId) -> core::ProjectionRecord {
    core::ProjectionRecord::Milestone {
        goal: 11,
        milestone: core::ProjectionMilestone {
            identity,
            phase: Some(super::task_phase()),
            change: Some(jig_domain_tasks::Change::Amended),
            words: Box::from(b"history".as_slice()),
        },
    }
}
pub fn proposal() -> core::ProposalDecisionRecord {
    core::ProposalDecisionRecord {
        project: 2,
        proposer: people::Proposer::Party(12),
        proposal: 13,
        kind: tasks::ProposalKind::Batch,
        by: 14,
        at: Wall::from_nanos(19),
        choice: people::ProposalChoice::Accepted,
        created: Some((15, 17)),
    }
}
pub fn escalation() -> core::EscalationDecisionRecord {
    core::EscalationDecisionRecord {
        project: 2,
        requester: 12,
        task: 11,
        revision: 3,
        by: 14,
        at: Wall::from_nanos(19),
        decision: people::EscalationDecision::Reject { reason: Box::from(b"rejected".as_slice()) },
    }
}
