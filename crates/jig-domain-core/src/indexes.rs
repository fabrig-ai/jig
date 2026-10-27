//! Core-owned task indexes, saved in the owner's decision (store.md 7.4).
use crate::{Core, Record, Request, Write};
use jig_domain_tasks as tasks;
use skein_lib::Queue;

pub(crate) fn save(core: &Core, record: &tasks::Stored, at: skein_lib::Wall, out: &mut Queue<Request>) {
    let update = match record {
        tasks::Stored::Ended(task) => {
            match task.requester {
                tasks::Party::Person(party) => {
                    let row = tasks::ResultRow { party, position: task.result_position, task: task.number };
                    out.push(Request::Write(Write::Save(Record::Tasks(tasks::Stored::Result(row)))));
                }
                tasks::Party::Task(_) | tasks::Party::Deployment { .. } => {}
            }
            match task.requester {
                tasks::Party::Person(party) => Some(tasks::Update {
                    party,
                    at: task.ended_at.expect("ended task has a time"),
                    task: task.number,
                    object: tasks::UpdateObject::Result,
                }),
                tasks::Party::Task(_) | tasks::Party::Deployment { .. } => None,
            }
        }
        tasks::Stored::Activity(line) => {
            let addressed = match &line.line {
                tasks::Line::Turn { reply, .. } => match core.tasks.task(line.task) {
                    Some(task) => match task.requester {
                        tasks::Party::Person(party) => Some((party, *reply)),
                        tasks::Party::Task(_) | tasks::Party::Deployment { .. } => None,
                    },
                    None => None,
                },
                tasks::Line::Notified { recipient: tasks::Party::Person(party), word } => Some((*party, word.number)),
                tasks::Line::Notified { .. }
                | tasks::Line::Words { .. }
                | tasks::Line::Heard(_)
                | tasks::Line::RunBegan { .. }
                | tasks::Line::RunEnded { .. }
                | tasks::Line::Decided { .. }
                | tasks::Line::Kept { .. }
                | tasks::Line::Milestone { .. } => None,
            };
            let Some((party, message)) = addressed else { return };
            Some(tasks::Update {
                party,
                at: line.at,
                task: line.task,
                object: tasks::UpdateObject::Reply {
                    message: tasks::Newest::of(message),
                    activity: tasks::Newest::of(line.position),
                },
            })
        }
        tasks::Stored::DecidedProposal(row) => {
            let Some(decision) = crate::proposals::decision_record(record, at) else { return };
            Some(tasks::Update {
                party: row.proposer,
                at: decision.at,
                task: row.first(),
                object: tasks::UpdateObject::Proposal { proposal: tasks::Newest::of(row.number) },
            })
        }
        tasks::Stored::Result(_)
        | tasks::Stored::EndedByParty(_)
        | tasks::Stored::EndedByProject(_)
        | tasks::Stored::EndedDelegate(_)
        | tasks::Stored::Delegate(_)
        | tasks::Stored::Update(_)
        | tasks::Stored::Live(_)
        | tasks::Stored::History(_)
        | tasks::Stored::Milestone(_)
        | tasks::Stored::PersonProposal(_)
        | tasks::Stored::PlacedHold(_)
        | tasks::Stored::Stub(_)
        | tasks::Stored::Ledger(_)
        | tasks::Stored::Writer(_)
        | tasks::Stored::Pool(_)
        | tasks::Stored::ClosedLedger(_) => None,
    };
    if let Some(update) = update {
        out.push(Request::Write(Write::Save(Record::Tasks(tasks::Stored::Update(update)))));
    }
}
