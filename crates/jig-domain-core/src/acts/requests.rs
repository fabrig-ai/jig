//! Current object judgments at the one authenticated keyed entrance.
use super::{Act, Admission, admit, standing};
use crate::{Core, Limits, documents};
use jig_domain_people as people;

#[expect(
    clippy::too_many_lines,
    reason = "one exhaustive keyed entrance uses concrete matches and preserves live dispatch precedence"
)]
pub(crate) fn request(
    core: &Core,
    limits: &Limits,
    person: u64,
    project: u32,
    role: Option<people::Role>,
    ask: &people::Ask,
    decision: skein_lib::Wall,
) -> Result<(), people::Refusal> {
    let (act, subject) = match ask {
        // An account request's check is its holder's, which people made (domain/authority.md, 8.4).
        people::Ask::Propose { action: people::Action::Settings { .. }, .. }
        | people::Ask::ChangeSettings { .. }
        | people::Ask::MoveToNewest { .. }
        | people::Ask::MakeProject { .. }
        | people::Ask::Application { .. }
        | people::Ask::Compound { .. }
        | people::Ask::Withdraw { .. }
        | people::Ask::AccountSignIn { .. }
        | people::Ask::AccountSignOut { .. }
        | people::Ask::AccountChange { .. } => return Ok(()),
        people::Ask::Create { .. }
        | people::Ask::Propose { action: people::Action::Batch(_) | people::Action::Amend { .. }, .. } => {
            return standing::role(core, project, role, Act::Amend);
        }
        people::Ask::Say { task, .. } => (Act::Message, documents::Subject::Task { task: *task }),
        people::Ask::AnswerQuestion { task, question, .. } => {
            (Act::Answer, documents::Subject::Question { task: *task, message: *question })
        }
        people::Ask::TakePerson { task, .. } => (Act::Take, documents::Subject::PersonTask { task: *task }),
        people::Ask::HandBackPerson { task, .. } => (Act::HandBack, documents::Subject::PersonTask { task: *task }),
        people::Ask::AnswerPerson { task, .. } => (Act::Answer, documents::Subject::PersonTask { task: *task }),
        people::Ask::DecideProposal { proposer, proposal, decision, running, .. } => {
            let settings = match proposer {
                people::Proposer::Party(person) => match core.tasks.person_proposal(*person, *proposal) {
                    Some(row) => Some(row.action.is_settings()),
                    None => None,
                },
                people::Proposer::Task(task) => match core.tasks.proposal(*task, *proposal) {
                    Some(row) => Some(match row.action {
                        jig_domain_tasks::ProposalAction::Settings { .. } => true,
                        jig_domain_tasks::ProposalAction::Batch(_)
                        | jig_domain_tasks::ProposalAction::Effect { .. }
                        | jig_domain_tasks::ProposalAction::ProcedureEffect { .. }
                        | jig_domain_tasks::ProposalAction::Amend { .. }
                        | jig_domain_tasks::ProposalAction::Widen { .. }
                        | jig_domain_tasks::ProposalAction::Release { .. }
                        | jig_domain_tasks::ProposalAction::ReleasePlaced { .. } => false,
                    }),
                    None => None,
                },
            };
            if let Some(settings) = settings
                && running.is_some() != (settings && *decision == people::ProposalDecision::Accept)
            {
                return Err(people::Refusal::NotWellFormed { at: 0, why: people::Malformed::Running });
            }

            let act = match decision {
                people::ProposalDecision::Accept => Act::Accept,
                people::ProposalDecision::Reject { .. } => Act::Reject,
                people::ProposalDecision::Pass => Act::PassUp,
            };
            (act, documents::Subject::Proposal { proposer: *proposer, number: *proposal })
        }
        people::Ask::DecideEscalation { task, decision, .. } => {
            let act = match decision {
                people::EscalationDecision::Release => Act::Release,
                people::EscalationDecision::Reject { .. } => Act::LeaveHeld,
                people::EscalationDecision::Pass => Act::PassUp,
            };
            (act, documents::Subject::Escalation { task: *task })
        }
        people::Ask::Release { task, .. } => (Act::Release, documents::Subject::Task { task: *task }),
        // Full draft judges are called by their request routes. Constructing
        // receiving-bound facts here would make a document's display limits
        // decide otherwise valid engine requests.
        people::Ask::Amend { task, .. } => {
            standing::role(core, project, role, Act::Amend)?;
            super::bounds::amend_preflight(core, limits, person, *task, role)?;
            let row = core.tasks.task(*task).expect("amend preflight found the task");
            return super::objects::mutable(row);
        }
        people::Ask::Stop { task, .. } => (Act::Stop, documents::Subject::Task { task: *task }),
        people::Ask::Cancel { task, .. } => (Act::Cancel, documents::Subject::Task { task: *task }),
        people::Ask::Move { task, .. } => (Act::Move, documents::Subject::Task { task: *task }),
        people::Ask::MarkSeen { seen } => match seen {
            people::Seen::Results { .. } => (Act::Seen, documents::Subject::Inbox { party: person }),
            people::Seen::Conversation { task, .. } => (Act::Seen, documents::Subject::Task { task: *task }),
        },
        people::Ask::EditNote { scope, change, .. } => {
            return super::note(
                core,
                person,
                project,
                scope,
                match &**change {
                    people::NoteChange::Correct { .. } => Act::EditNote,
                    people::NoteChange::Delete { .. } => Act::DeleteNote,
                },
            );
        }
        people::Ask::Adopt { .. } => (Act::Adopt, documents::Subject::Project { project }),
        people::Ask::Rename { .. } => (Act::Rename, documents::Subject::Project { project }),
        people::Ask::ChangePolicy { .. } => (Act::ChangePolicy, documents::Subject::Project { project }),
        people::Ask::SetRoles { .. } => (Act::SetRoles, documents::Subject::Project { project }),
        people::Ask::MakeService { .. } => (Act::MakeService, documents::Subject::Project { project }),
        people::Ask::SetPool { .. } => (Act::SetPool, documents::Subject::Project { project }),
    };
    match ask {
        // The person-task entrance compares the named revision before phase
        // eligibility, and preserves the historical ended-task lookup.
        people::Ask::AnswerPerson { .. } => return Ok(()),
        people::Ask::DecideProposal { proposer, proposal, .. } => {
            let present = match proposer {
                people::Proposer::Party(number) => core.tasks.person_proposal(*number, *proposal).is_some(),
                people::Proposer::Task(number) => core.tasks.proposal(*number, *proposal).is_some(),
            };
            if !present {
                return Ok(());
            }
        }
        people::Ask::DecideEscalation { task, revision, .. } => {
            let Some(context) = core.tasks.escalation(*task) else {
                return Ok(());
            };
            match context.escalation {
                jig_domain_tasks::Escalation::Waiting { revision: now, .. } if now == *revision => {}
                jig_domain_tasks::Escalation::Waiting { .. }
                | jig_domain_tasks::Escalation::Unheld { .. }
                | jig_domain_tasks::Escalation::Routing { .. }
                | jig_domain_tasks::Escalation::Rejected { .. } => return Ok(()),
            }
        }
        people::Ask::Compound { .. }
        | people::Ask::Application { .. }
        | people::Ask::Say { .. }
        | people::Ask::AnswerQuestion { .. }
        | people::Ask::TakePerson { .. }
        | people::Ask::HandBackPerson { .. }
        | people::Ask::Release { .. }
        | people::Ask::Amend { .. }
        | people::Ask::Stop { .. }
        | people::Ask::Cancel { .. }
        | people::Ask::Move { .. }
        | people::Ask::Create { .. }
        | people::Ask::Propose { .. }
        | people::Ask::Withdraw { .. }
        | people::Ask::MarkSeen { .. }
        | people::Ask::EditNote { .. }
        | people::Ask::Adopt { .. }
        | people::Ask::Rename { .. }
        | people::Ask::MakeProject { .. }
        | people::Ask::ChangeSettings { .. }
        | people::Ask::MoveToNewest { .. }
        | people::Ask::ChangePolicy { .. }
        | people::Ask::SetRoles { .. }
        | people::Ask::MakeService { .. }
        | people::Ask::SetPool { .. }
        | people::Ask::AccountSignIn { .. }
        | people::Ask::AccountSignOut { .. }
        | people::Ask::AccountChange { .. } => {}
    }
    match &subject {
        documents::Subject::PersonTask { task } => {
            if core.tasks.task(*task).is_none() {
                return Ok(());
            }
        }
        documents::Subject::Task { task } if act == Act::Seen && core.tasks.task(*task).is_none() => return Ok(()),
        documents::Subject::Task { .. }
        | documents::Subject::Application { .. }
        | documents::Subject::Frame { .. }
        | documents::Subject::Tree { .. }
        | documents::Subject::Chips { .. }
        | documents::Subject::Inbox { .. }
        | documents::Subject::Accounts { .. }
        | documents::Subject::Query { .. }
        | documents::Subject::Run { .. }
        | documents::Subject::Settings
        | documents::Subject::Deployment
        | documents::Subject::Proposal { .. }
        | documents::Subject::Escalation { .. }
        | documents::Subject::Question { .. }
        | documents::Subject::Result { .. }
        | documents::Subject::Project { .. }
        | documents::Subject::Note { .. } => {}
    }
    match admit(core, limits, person, act, &subject, decision) {
        Admission::Allowed | Admission::Proposed { .. } | Admission::Bounded { .. } => Ok(()),
        // The child emits the same refusal with its exact observed delegate
        // identities. Preserve that capacity observation at its owner.
        Admission::Refused(people::Refusal::Limit { which: people::LimitKind::LiveDelegates })
            if act == Act::Answer =>
        {
            Ok(())
        }
        Admission::Refused(why) => Err(why),
    }
}
