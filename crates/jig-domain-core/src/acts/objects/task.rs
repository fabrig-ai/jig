//! Task steering and answer eligibility, without a retained request.
use super::mutable;
use crate::acts::{Act, Admission, allowed, bounds, standing};
use crate::{Core, Limits};
use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_people as people;
use jig_domain_tasks as tasks;

#[expect(clippy::too_many_lines, reason = "one exhaustive object judgment preserves standing before task phase facts")]
pub(super) fn admit(core: &Core, limits: &Limits, party: u64, act: Act, number: u64) -> Admission {
    let Some(row) = core.tasks.task(number) else { return Admission::Refused(people::Refusal::Unknown) };
    let held = core.people.role(party, row.project);
    if let Err(why) = standing::role(core, row.project, held, act) {
        return Admission::Refused(why);
    }
    match act {
        Act::Reorder => allowed(core.reorder_allowed(row.project, held)),
        Act::Message => allowed(write(core, limits, party, number)),
        Act::Amend => {
            let admission = bounds::amend(core, limits, party, number, held);
            if let Admission::Refused(_) = admission {
                return admission;
            }
            if let Err(why) = mutable(row) {
                return Admission::Refused(why);
            }

            admission
        }
        Act::Stop | Act::Cancel | Act::Release => {
            let ask = match act {
                Act::Stop => people::Ask::Stop { project: row.project, task: number },
                Act::Cancel => {
                    people::Ask::Cancel { project: row.project, task: number, reason: Box::new([]), grace: None }
                }
                Act::Release => people::Ask::Release {
                    project: row.project,
                    task: number,
                    revision: row.revisions.hold,
                    again: None,
                },
                Act::Message
                | Act::Answer
                | Act::Take
                | Act::HandBack
                | Act::Withdraw
                | Act::Accept
                | Act::Reject
                | Act::PassUp
                | Act::LeaveHeld
                | Act::Amend
                | Act::Move
                | Act::Reorder
                | Act::Seen
                | Act::EditNote
                | Act::DeleteNote
                | Act::Adopt
                | Act::Rename
                | Act::ChangePolicy
                | Act::SetRoles
                | Act::MakeService
                | Act::SetPool => unreachable!("control act selected"),
            };
            if let Err(why) = core.control_admit(&ask, party, row.project, held, &limits.tasks) {
                return Admission::Refused(why);
            }
            if act == Act::Stop
                && let Err(why) = mutable(row)
            {
                return Admission::Refused(why);
            }
            if act == Act::Release
                && let tasks::Phase::Closing(closing) = &row.phase
            {
                return Admission::Refused(crate::routing::refusals::ended(&closing.ending));
            }
            if let tasks::Phase::Ended(ending) = &row.phase {
                return Admission::Refused(crate::routing::refusals::ended(ending));
            }
            if act == Act::Release {
                match row.phase {
                    tasks::Phase::Held { .. } => {}
                    tasks::Phase::Waiting | tasks::Phase::Active(_) if tasks::draining(&core.tasks, row) => {}
                    tasks::Phase::Waiting
                    | tasks::Phase::Active(_)
                    | tasks::Phase::Closing(_)
                    | tasks::Phase::Ended(_) => return Admission::Refused(people::Refusal::NotWaiting),
                }
            }
            Admission::Allowed
        }
        Act::Move => {
            allowed(core.move_admit(party, held, row.project, number, limits.tasks.tree_tasks, limits.tasks.depth))
        }
        Act::Seen => {
            if core.conversation_requester(row) == Some(party) && held.is_some() {
                Admission::Allowed
            } else {
                Admission::Refused(people::Refusal::Unknown)
            }
        }
        Act::Answer
        | Act::Take
        | Act::HandBack
        | Act::Withdraw
        | Act::Accept
        | Act::Reject
        | Act::PassUp
        | Act::LeaveHeld
        | Act::EditNote
        | Act::DeleteNote
        | Act::Adopt
        | Act::Rename
        | Act::ChangePolicy
        | Act::SetRoles
        | Act::MakeService
        | Act::SetPool => Admission::Refused(people::Refusal::NotOffered),
    }
}

pub(crate) fn write(core: &Core, limits: &Limits, party: u64, number: u64) -> Result<(), people::Refusal> {
    write_in(core, limits, party, number, &tasks::CompoundAdmission::new(&core.tasks, 0))
}
pub(crate) fn write_in(
    core: &Core,
    limits: &Limits,
    party: u64,
    number: u64,
    admission: &tasks::CompoundAdmission<'_>,
) -> Result<(), people::Refusal> {
    let row = admission.task(number).ok_or(people::Refusal::Unknown)?;
    let held = core.people.role(party, row.project);
    standing::role(core, row.project, held, Act::Message)?;
    let any = match held {
        Some(people::Role::Owner | people::Role::Maintainer) => true,
        Some(people::Role::Policy { role: number }) => match core.authority.role(row.project, number) {
            Some(role) => role.requests.allows(authority::RequestKind::Amend),
            None => false,
        },
        Some(people::Role::Member | people::Role::Observer) | None => false,
    };
    if !any && !Core::person_tree_in(party, number, limits.tasks.depth, admission) {
        return Err(people::Refusal::Unknown);
    }
    match row.executor {
        tasks::Executor::Agent { .. } => {}
        tasks::Executor::Procedure { .. } | tasks::Executor::Person(_) => return Err(people::Refusal::NotOffered),
    }
    super::mutable_phase(&row.phase)?;
    match row.phase {
        tasks::Phase::Active(_) | tasks::Phase::Held { was: tasks::Was::Active(_), .. } => Ok(()),
        tasks::Phase::Waiting
        | tasks::Phase::Closing(_)
        | tasks::Phase::Held { was: tasks::Was::Waiting | tasks::Was::Closing(_), .. }
        | tasks::Phase::Ended(_) => Err(people::Refusal::NotWaiting),
    }
}

pub(crate) fn person(
    core: &Core,
    party: u64,
    act: Act,
    number: u64,
    revision: Option<u64>,
) -> Result<(), people::Refusal> {
    let row = core.tasks.task(number).ok_or(people::Refusal::Unknown)?;
    let held = core.people.role(party, row.project);
    let addressed = match row.executor {
        tasks::Executor::Person(tasks::PersonAddress::Person(person)) => person == party && act == Act::Answer,
        tasks::Executor::Person(tasks::PersonAddress::Role(role)) => {
            act == Act::HandBack
                || match held {
                    Some(held) => held.number() == role,
                    None => false,
                }
        }
        tasks::Executor::Agent { .. } | tasks::Executor::Procedure { .. } => false,
    };
    if !addressed {
        return Err(people::Refusal::Unknown);
    }
    if let Some(answer) = row.person_answer {
        return Err(people::Refusal::Ended { how: people::Ending::Decided { by: answer.by, at: answer.at } });
    }
    if let Some(by) = row.taken_by
        && by != party
    {
        return Err(people::Refusal::Taken { by });
    }
    if let Some(revision) = revision {
        core.person_revision(number, revision)?;
    }
    mutable(row)?;
    match row.phase {
        tasks::Phase::Active(tasks::Active::Due | tasks::Active::Idle) => {}
        tasks::Phase::Active(
            tasks::Active::Preparing
            | tasks::Active::Claimed { .. }
            | tasks::Active::Running { .. }
            | tasks::Active::BackingOff { .. },
        )
        | tasks::Phase::Waiting
        | tasks::Phase::Closing(_)
        | tasks::Phase::Held { .. }
        | tasks::Phase::Ended(_) => return Err(people::Refusal::NotWaiting),
    }
    match act {
        Act::Take => {
            if row.taken_by.is_none() {
                Ok(())
            } else {
                Err(people::Refusal::NotWaiting)
            }
        }
        Act::HandBack => {
            if row.taken_by == Some(party) {
                Ok(())
            } else {
                Err(people::Refusal::NotWaiting)
            }
        }
        Act::Answer => {
            match row.executor {
                tasks::Executor::Person(tasks::PersonAddress::Role(_)) if row.taken_by != Some(party) => {
                    return Err(people::Refusal::NotWaiting);
                }
                tasks::Executor::Person(_) | tasks::Executor::Agent { .. } | tasks::Executor::Procedure { .. } => {}
            }
            if row.delegates.is_empty() {
                Ok(())
            } else {
                Err(people::Refusal::Limit { which: people::LimitKind::LiveDelegates })
            }
        }
        Act::Message
        | Act::Withdraw
        | Act::Accept
        | Act::Reject
        | Act::PassUp
        | Act::Release
        | Act::LeaveHeld
        | Act::Amend
        | Act::Stop
        | Act::Cancel
        | Act::Move
        | Act::Reorder
        | Act::Seen
        | Act::EditNote
        | Act::DeleteNote
        | Act::Adopt
        | Act::Rename
        | Act::ChangePolicy
        | Act::SetRoles
        | Act::MakeService
        | Act::SetPool => Err(people::Refusal::NotOffered),
    }
}

pub(crate) fn question(
    core: &Core,
    limits: &Limits,
    party: u64,
    act: Act,
    number: u64,
    message: u64,
) -> Result<(), people::Refusal> {
    if act != Act::Answer {
        return Err(people::Refusal::NotOffered);
    }
    let row = core.tasks.task(number).ok_or(people::Refusal::Unknown)?;
    standing::role(core, row.project, core.people.role(party, row.project), act)?;
    if row.requester != tasks::Party::Person(party) {
        return Err(people::Refusal::Unknown);
    }
    write(core, limits, party, number)?;
    let mut found = false;
    for line in &row.question_lines {
        if line.message == message && line.answer.is_none() {
            found = true;
        }
    }
    if found { Ok(()) } else { Err(people::Refusal::Authority { proposable: None, lacking: people::Lack::Reference }) }
}
