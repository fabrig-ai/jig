//! Authenticated person-task standing and task handoff
//! (domain/engine.md, section 4.4; domain/people.md, section 5).

use alloc::boxed::Box;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{ReplyTo, Token};

use crate::{Core, PersonTaskRoute};

pub(crate) fn number(ask: &people::Ask) -> u64 {
    match ask {
        people::Ask::TakePerson { task, .. }
        | people::Ask::HandBackPerson { task, .. }
        | people::Ask::AnswerPerson { task, .. } => *task,
        people::Ask::Compound { .. }
        | people::Ask::MarkSeen { .. }
        | people::Ask::Application { .. }
        | people::Ask::Move { .. }
        | people::Ask::DecideProposal { .. }
        | people::Ask::Say { .. }
        | people::Ask::AnswerQuestion { .. }
        | people::Ask::Amend { .. }
        | people::Ask::EditNote { .. }
        | people::Ask::MakeService { .. }
        | people::Ask::SetRoles { .. }
        | people::Ask::Adopt { .. }
        | people::Ask::Rename { .. }
        | people::Ask::MakeProject { .. }
        | people::Ask::ChangeSettings { .. }
        | people::Ask::MoveToNewest { .. }
        | people::Ask::ChangePolicy { .. }
        | people::Ask::SetPool { .. }
        | people::Ask::DecideEscalation { .. }
        | people::Ask::Stop { .. }
        | people::Ask::Cancel { .. }
        | people::Ask::Lift { .. }
        | people::Ask::Release { .. }
        | people::Ask::Create { .. }
        | people::Ask::Propose { .. }
        | people::Ask::Withdraw { .. }
        | people::Ask::AccountSignIn { .. }
        | people::Ask::AccountSignOut { .. }
        | people::Ask::AccountChange { .. } => unreachable!("person-task route owns its ask"),
    }
}

fn approval_matches(contract: &tasks::Contract, result: &people::PersonResult) -> bool {
    match result {
        people::PersonResult::Approved { state, .. } => match contract {
            tasks::Contract::Approval { state: expected, .. } => state == expected,
            tasks::Contract::Report { .. } | tasks::Contract::Verdict { .. } | tasks::Contract::Change { .. } => true,
        },
        people::PersonResult::Report { .. }
        | people::PersonResult::Verdict { .. }
        | people::PersonResult::Failure { .. } => true,
    }
}

fn person_result(result: people::PersonResult) -> tasks::TaskResult {
    match result {
        people::PersonResult::Approved { state, words } => tasks::TaskResult::Approved { state, words },
        people::PersonResult::Report { words } => tasks::TaskResult::Report { words },
        people::PersonResult::Verdict { code, words } => {
            tasks::TaskResult::Verdict { code, words, fields: Box::new([]) }
        }
        people::PersonResult::Failure { reason } => tasks::TaskResult::Failure { reason },
    }
}

impl Core {
    /// Recheck the stored owner and current address before answering an act on an ended task.
    pub(crate) fn person_task_ended(
        &self,
        person: u64,
        project: u32,
        task: u64,
        row: Option<Box<tasks::TaskRecord>>,
        busy: bool,
    ) -> people::Outcome {
        if busy {
            return people::Outcome::Again(people::Again::Busy);
        }
        let Some(row) = row else { return people::Outcome::Refused(people::Refusal::Unknown) };
        let role = self.people.role(person, project);
        let allowed = row.number == task
            && task != 0
            && task <= self.counters.deployment().tasks
            && row.project == project
            && role.is_some()
            && match row.executor {
                tasks::Executor::Person(tasks::PersonAddress::Person(address)) => address == person,
                tasks::Executor::Person(tasks::PersonAddress::Role(address)) => {
                    role.expect("membership").number() == address
                }
                tasks::Executor::Agent { .. } | tasks::Executor::Procedure { .. } => false,
            };
        if !allowed {
            return people::Outcome::Refused(people::Refusal::Unknown);
        }
        let ending = match &row.phase {
            tasks::Phase::Ended(ending) => ending,
            tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Closing(_) | tasks::Phase::Held { .. } => {
                return people::Outcome::Refused(people::Refusal::Unknown);
            }
        };
        let refusal = match row.person_answer {
            Some(answer) if answer.by != 0 && answer.by <= self.counters.deployment().people => {
                people::Refusal::Ended { how: people::Ending::Decided { by: answer.by, at: answer.at } }
            }
            Some(_) => people::Refusal::Unknown,
            None => crate::routing::refusals::ended(ending),
        };
        people::Outcome::Refused(refusal)
    }

    pub fn person_task(
        &mut self,
        request: Token,
        person: u64,
        project: u32,
        _role: Option<people::Role>,
        ask: people::Ask,
    ) -> Result<tasks::Event, people::Refusal> {
        let task = number(&ask);
        if self.tasks.task(task).ok_or(people::Refusal::Unknown)?.project != project {
            return Err(people::Refusal::Unknown);
        }
        let act = person_act(&ask);
        let revision = if let people::Ask::AnswerPerson { revision, .. } = &ask { Some(*revision) } else { None };
        match crate::acts::person(self, person, act, task, revision) {
            Ok(()) => {}
            // The task owner supplies its exact capacity observation.
            Err(people::Refusal::Limit { which: people::LimitKind::LiveDelegates })
                if act == crate::acts::Act::Answer => {}
            Err(why) => return Err(why),
        }
        let (route, event) = match ask {
            people::Ask::TakePerson { .. } => (
                PersonTaskRoute::Take(task),
                tasks::Event::TakePerson { reply_to: ReplyTo::new(request), task, person },
            ),
            people::Ask::HandBackPerson { .. } => (
                PersonTaskRoute::HandBack(task),
                tasks::Event::HandBackPerson { reply_to: ReplyTo::new(request), task, person },
            ),
            people::Ask::AnswerPerson { result, .. } => {
                let row = self.tasks.task(task).expect("person act admission checked");
                if !approval_matches(&row.contract, &result) {
                    return Err(people::Refusal::Moved {
                        now: people::Moved::Person { revision: row.revisions.person },
                    });
                }
                (
                    PersonTaskRoute::Answer(task),
                    tasks::Event::AnswerPerson {
                        reply_to: ReplyTo::new(request),
                        task,
                        person,
                        result: person_result(result),
                    },
                )
            }
            people::Ask::Compound { .. }
            | people::Ask::MarkSeen { .. }
            | people::Ask::Application { .. }
            | people::Ask::Move { .. }
            | people::Ask::DecideProposal { .. }
            | people::Ask::Say { .. }
            | people::Ask::AnswerQuestion { .. }
            | people::Ask::Amend { .. }
            | people::Ask::EditNote { .. }
            | people::Ask::MakeService { .. }
            | people::Ask::SetRoles { .. }
            | people::Ask::Adopt { .. }
            | people::Ask::Rename { .. }
            | people::Ask::MakeProject { .. }
            | people::Ask::ChangeSettings { .. }
            | people::Ask::MoveToNewest { .. }
            | people::Ask::ChangePolicy { .. }
            | people::Ask::SetPool { .. }
            | people::Ask::DecideEscalation { .. }
            | people::Ask::Stop { .. }
            | people::Ask::Cancel { .. }
            | people::Ask::Lift { .. }
            | people::Ask::Release { .. }
            | people::Ask::Create { .. }
            | people::Ask::Propose { .. }
            | people::Ask::Withdraw { .. }
            | people::Ask::AccountSignIn { .. }
            | people::Ask::AccountSignOut { .. }
            | people::Ask::AccountChange { .. } => unreachable!("person-task route owns its ask"),
        };
        assert!(self.person_tasks.insert(request, route) == Ok(None), "one routed person task per keyed flight");
        Ok(event)
    }
}

fn person_act(ask: &people::Ask) -> crate::acts::Act {
    match ask {
        people::Ask::TakePerson { .. } => crate::acts::Act::Take,
        people::Ask::HandBackPerson { .. } => crate::acts::Act::HandBack,
        people::Ask::AnswerPerson { .. } => crate::acts::Act::Answer,
        people::Ask::Compound { .. }
        | people::Ask::MarkSeen { .. }
        | people::Ask::Application { .. }
        | people::Ask::Move { .. }
        | people::Ask::DecideProposal { .. }
        | people::Ask::Say { .. }
        | people::Ask::AnswerQuestion { .. }
        | people::Ask::Amend { .. }
        | people::Ask::EditNote { .. }
        | people::Ask::MakeService { .. }
        | people::Ask::SetRoles { .. }
        | people::Ask::Adopt { .. }
        | people::Ask::Rename { .. }
        | people::Ask::MakeProject { .. }
        | people::Ask::ChangeSettings { .. }
        | people::Ask::MoveToNewest { .. }
        | people::Ask::ChangePolicy { .. }
        | people::Ask::SetPool { .. }
        | people::Ask::DecideEscalation { .. }
        | people::Ask::Stop { .. }
        | people::Ask::Cancel { .. }
        | people::Ask::Lift { .. }
        | people::Ask::Release { .. }
        | people::Ask::Create { .. }
        | people::Ask::Propose { .. }
        | people::Ask::Withdraw { .. }
        | people::Ask::AccountSignIn { .. }
        | people::Ask::AccountSignOut { .. }
        | people::Ask::AccountChange { .. } => unreachable!("person-task route owns its ask"),
    }
}
