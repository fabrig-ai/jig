//! The core's archived reads translated by the kit (protocol/store.md, 3).
use alloc::boxed::Box;
use jig_domain_core::{self as core, CoreKey, CoreRecord, Key, Now, Record};
use jig_domain_tasks as tasks;

#[must_use]
pub fn key(now: &Now) -> Option<Key> {
    match now {
        Now::EndedTask { task, .. } => Some(Key::Tasks(tasks::Key::Ended(*task))),
        Now::PartyInputs { tasks, .. } => Some(Key::Tasks(tasks::Key::Ended(*tasks.first()?))),
        Now::LoadEndedRead { task, party, .. } => {
            Some(Key::People(jig_domain_people::Key::EndedRead { task: *task, party: *party }))
        }
        Now::SeenConversation { task, .. } | Now::HistoricalPersonTask { task, .. } => {
            Some(Key::Tasks(tasks::Key::Ended(*task)))
        }
        Now::DecidedProposal { key, .. } => Some(Key::Tasks(tasks::Key::Live(key.task))),
        Now::HistoricalProposal { proposer, proposal, .. } => {
            Some(Key::Core(CoreKey::ProposalDecision { proposer: *proposer, proposal: *proposal }))
        }
        Now::HistoricalEscalation { task, revision, .. } => {
            Some(Key::Core(CoreKey::EscalationDecision { task: *task, revision: *revision }))
        }
        Now::RelayBusy { .. }
        | Now::CallBusy { .. }
        | Now::Relayed { .. }
        | Now::EffectAnswer { .. }
        | Now::SettingsAnswer { .. }
        | Now::SettledCallRefused { .. }
        | Now::Call { .. }
        | Now::DropCall { .. }
        | Now::Undelivered { .. }
        | Now::SignInRefused { .. }
        | Now::WatchRefused { .. }
        | Now::Account(_)
        | Now::RunText { .. }
        | Now::NotesIndexed { .. }
        | Now::NotesRecalled { .. }
        | Now::NotesRefused { .. }
        | Now::NoteBusy { .. }
        | Now::DropPayload { .. }
        | Now::DropAssignment { .. }
        | Now::TurnPayload { .. }
        | Now::AnswerPayload { .. }
        | Now::AcceptedTurn { .. }
        | Now::RefusedPayload { .. }
        | Now::Activate { .. }
        | Now::PrepareAgent { .. }
        | Now::StartPreparation { .. }
        | Now::CompleteBrief { .. }
        | Now::BriefCorePlanned { .. }
        | Now::WorkspaceRequest { .. }
        | Now::RunPreparationFailed { .. }
        | Now::RunPrepared { .. }
        | Now::RestoreRefused => None,
    }
}

#[must_use]
pub fn answer(now: Now, row: Option<Record>) -> Option<core::Event> {
    Some(match now {
        Now::EndedTask { request, .. } => core::Event::EndedTaskLoaded { request, row: ended(row), busy: false },
        Now::PartyInputs { request, .. } => core::Event::PartyInputsLoaded { request, row: ended(row), busy: false },
        Now::LoadEndedRead { request, task, party } => {
            let through = match row {
                Some(Record::People(jig_domain_people::Stored::EndedRead { task: number, party: owner, through }))
                    if number == task && owner == party =>
                {
                    through
                }
                Some(_) | None => 0,
            };
            core::Event::EndedReadLoaded { request, task, party, through, busy: false }
        }
        Now::SeenConversation { request, .. } => {
            core::Event::SeenConversationLoaded { request, row: ended(row), busy: false }
        }
        Now::HistoricalPersonTask { request, person, project, task } => {
            core::Event::HistoricalPersonTask { request, person, project, task, row: ended(row), busy: false }
        }
        Now::DecidedProposal { to, key, proposal } => {
            core::Event::DecidedProposal { to, key, proposal, row: task_proposal(row), busy: false }
        }
        Now::HistoricalProposal { request, person, project, proposer, proposal } => core::Event::HistoricalProposal {
            request,
            person,
            project,
            proposer,
            proposal,
            row: proposal_row(row),
            busy: false,
        },
        Now::HistoricalEscalation { request, person, project, task, revision } => core::Event::HistoricalEscalation {
            request,
            person,
            project,
            task,
            revision,
            row: escalation_row(row),
            busy: false,
        },
        Now::RelayBusy { .. }
        | Now::CallBusy { .. }
        | Now::Relayed { .. }
        | Now::EffectAnswer { .. }
        | Now::SettingsAnswer { .. }
        | Now::SettledCallRefused { .. }
        | Now::Call { .. }
        | Now::DropCall { .. }
        | Now::Undelivered { .. }
        | Now::SignInRefused { .. }
        | Now::WatchRefused { .. }
        | Now::Account(_)
        | Now::RunText { .. }
        | Now::NotesIndexed { .. }
        | Now::NotesRecalled { .. }
        | Now::NotesRefused { .. }
        | Now::NoteBusy { .. }
        | Now::DropPayload { .. }
        | Now::DropAssignment { .. }
        | Now::TurnPayload { .. }
        | Now::AnswerPayload { .. }
        | Now::AcceptedTurn { .. }
        | Now::RefusedPayload { .. }
        | Now::Activate { .. }
        | Now::PrepareAgent { .. }
        | Now::StartPreparation { .. }
        | Now::CompleteBrief { .. }
        | Now::BriefCorePlanned { .. }
        | Now::WorkspaceRequest { .. }
        | Now::RunPreparationFailed { .. }
        | Now::RunPrepared { .. }
        | Now::RestoreRefused => return None,
    })
}

fn ended(row: Option<Record>) -> Option<Box<tasks::TaskRecord>> {
    match row? {
        Record::Tasks(tasks::Stored::Ended(row)) => Some(row),
        Record::Tasks(
            tasks::Stored::Live(_)
            | tasks::Stored::PlacedHold(_)
            | tasks::Stored::Stub(_)
            | tasks::Stored::Ledger(_)
            | tasks::Stored::Writer(_)
            | tasks::Stored::Pool(_)
            | tasks::Stored::PersonProposal(_)
            | tasks::Stored::ClosedLedger(_)
            | tasks::Stored::DecidedProposal(_)
            | tasks::Stored::History(_)
            | tasks::Stored::Milestone(_)
            | tasks::Stored::Activity(_)
            | tasks::Stored::Update(_)
            | tasks::Stored::Result(_)
            | tasks::Stored::EndedByParty(_)
            | tasks::Stored::EndedByProject(_)
            | tasks::Stored::EndedDelegate(_)
            | tasks::Stored::Delegate(_),
        )
        | Record::Core(_)
        | Record::People(_)
        | Record::Notes(_)
        | Record::Accounts(_) => None,
    }
}

fn proposal_row(row: Option<Record>) -> Option<core::ProposalDecisionRecord> {
    match row? {
        Record::Core(CoreRecord::ProposalDecision(row)) => Some(row),
        Record::Core(
            CoreRecord::Settings(_)
            | CoreRecord::Deployment(_)
            | CoreRecord::Projection(_)
            | CoreRecord::RunProof(_)
            | CoreRecord::Call(_)
            | CoreRecord::Turn(_)
            | CoreRecord::Terminal(_)
            | CoreRecord::EscalationDecision(_),
        )
        | Record::Tasks(_)
        | Record::People(_)
        | Record::Notes(_)
        | Record::Accounts(_) => None,
    }
}

fn escalation_row(row: Option<Record>) -> Option<core::EscalationDecisionRecord> {
    match row? {
        Record::Core(CoreRecord::EscalationDecision(row)) => Some(row),
        Record::Core(
            CoreRecord::Settings(_)
            | CoreRecord::Deployment(_)
            | CoreRecord::Projection(_)
            | CoreRecord::RunProof(_)
            | CoreRecord::Call(_)
            | CoreRecord::Turn(_)
            | CoreRecord::Terminal(_)
            | CoreRecord::ProposalDecision(_),
        )
        | Record::Tasks(_)
        | Record::People(_)
        | Record::Notes(_)
        | Record::Accounts(_) => None,
    }
}

/// Follow a task owner's retained proposal position before answering the call.
#[must_use]
pub fn next(now: &Now, row: &Option<Record>) -> Option<Key> {
    if let Now::DecidedProposal { key, proposal, .. } = now
        && let Some(Record::Tasks(tasks::Stored::Live(owner))) = row
        && owner.number == key.task
    {
        return crate::reads::proposal_key(owner, *proposal);
    }
    None
}

fn task_proposal(row: Option<Record>) -> Option<tasks::Proposal> {
    let Some(Record::Tasks(tasks::Stored::History(row))) = row else { return None };
    Some(*row.proposal?)
}
