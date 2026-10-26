//! Ended rows gathered for preparation or amendment admission
//! (`domain/engine.md`, sections 5.3 and 9).

use super::amendment_reads::{self, AmendmentRead, Replay};
use alloc::boxed::Box;
use jig_domain_tasks as tasks;
use skein_lib::{Env, List, Queue, Token};

use crate::{Core, Event, Held, HistoricalResult, Limits, Now, Request, Requests};

#[derive(Debug)]
pub(crate) struct ResultRead {
    pub why: ReadFor,
    pub ids: Box<[u64]>,
    pub at: usize,
    pub results: List<HistoricalResult>,
}

#[derive(Debug)]
pub(crate) enum ReadFor {
    DeploymentClose { request: Token, project: u32 },
    ProjectionNews { goal: u64, connector: u16 },
    Preparation { task: u64, last_message: u64 },
    Amendment(Box<AmendmentRead>),
}

pub(crate) fn clear(core: &mut Core, task: u64) {
    // Several calls can wait for one run; remove all of its reads.
    for _ in 0..core.result_reads.capacity() {
        let mut request = None;
        for (&token, read) in &core.result_reads {
            let owner = match &read.why {
                ReadFor::DeploymentClose { .. } | ReadFor::ProjectionNews { .. } => None,
                ReadFor::Preparation { task, .. } => Some(*task),
                ReadFor::Amendment(read) => match &read.replay {
                    Replay::Agent { key, .. } => Some(key.task),
                    Replay::Person { .. } => None,
                },
            };
            if owner == Some(task) {
                request = Some(token);
                break;
            }
        }
        let Some(request) = request else { break };
        if let Some(read) = core.result_reads.remove(&request) {
            match read.why {
                ReadFor::ProjectionNews { .. } | ReadFor::DeploymentClose { .. } | ReadFor::Preparation { .. } => {}
                ReadFor::Amendment(amendment) => match amendment.replay {
                    Replay::Agent { key, .. } => {
                        let _pending = core.pending_calls.remove(&key);
                    }
                    Replay::Person { .. } => {}
                },
            }
        }
    }
}

pub(super) fn begin(core: &mut Core, work: &mut Queue<Event>, task: u64, ids: Box<[u64]>) -> Requests {
    let mut out = Queue::with_capacity(2);
    for (_, read) in &core.result_reads {
        match read.why {
            ReadFor::Preparation { task: owner, .. } if owner == task => {
                out.push(Request::Decided);
                return Requests::Out(out);
            }
            ReadFor::ProjectionNews { .. }
            | ReadFor::DeploymentClose { .. }
            | ReadFor::Preparation { .. }
            | ReadFor::Amendment(_) => {}
        }
    }
    if core.result_reads.len() == core.result_reads.capacity() {
        work.push(Event::Tasks(tasks::Event::PreparationFailed { task }));
    } else {
        let request = Token::new(core.next_result_request);
        core.next_result_request = core.next_result_request.checked_add(1).expect("transient result request space");
        let read = ResultRead {
            why: ReadFor::Preparation {
                task,
                last_message: core.contexts.get(&task).expect("preparing context").last_message,
            },
            results: List::with_capacity(u32::try_from(ids.len()).expect("bounded dependency results")),
            ids,
            at: 0,
        };
        ask(&mut out, request, &read);
        assert!(core.result_reads.insert(request, read).is_ok(), "admitted result read");
    }
    out.push(Request::Decided);
    Requests::Out(out)
}

// Held, so that it sees the commit that ended the dependency (`domain/engine.md`, 5.3).
fn ask(out: &mut Queue<Request>, request: Token, read: &ResultRead) {
    out.push(Request::Held(Box::new(Held::Load(Box::new(Now::EndedTask {
        request,
        task: *read.ids.get(read.at).expect("pending result ID"),
    })))));
}

pub(super) fn loaded(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    request: Token,
    row: Option<Box<tasks::TaskRecord>>,
    busy: bool,
) -> Requests {
    let mut out = Queue::with_capacity(core.room_max.writes);
    out.push(Request::Decided);
    let Some(mut read) = core.result_reads.remove(&request) else { return Requests::Out(out) };
    match read.why {
        ReadFor::ProjectionNews { goal, connector } => {
            crate::projection_failures::archived(core, env, goal, connector, row, busy, &mut out);
        }
        ReadFor::DeploymentClose { request, project } => {
            let outcome = closed_outcome(project, *read.ids.first().expect("one close subject"), row, busy);
            work.push(Event::People(jig_domain_people::Event::Decided { request, outcome }));
        }
        ReadFor::Amendment(mut amendment) => {
            if !amendment_reads::current(core, &amendment.replay) {
                return Requests::Out(out);
            }
            if busy {
                amendment_reads::busy(core, work, &mut out, amendment.replay);
                return Requests::Out(out);
            }
            let wanted = *read.ids.first().expect("one pending amendment place");
            amendment.evidence.loaded.push(wanted).expect("bounded loaded places");
            if let Some(row) = row {
                let status = match row.phase {
                    tasks::Phase::Ended(tasks::Ending::Done(_)) => Some(tasks::Status::Done),
                    tasks::Phase::Ended(tasks::Ending::Failed { .. }) => Some(tasks::Status::Failed),
                    tasks::Phase::Ended(tasks::Ending::Cancelled { .. }) => Some(tasks::Status::Cancelled),
                    tasks::Phase::Waiting
                    | tasks::Phase::Active(_)
                    | tasks::Phase::Closing(_)
                    | tasks::Phase::Held { .. } => None,
                };
                if row.number == wanted
                    && let Some(status) = status
                {
                    amendment
                        .evidence
                        .places
                        .push(tasks::DependencyPlace {
                            number: row.number,
                            project: row.project,
                            requester: row.requester,
                            root: row.root,
                            depth: row.depth,
                            status,
                        })
                        .expect("bounded loaded places");
                }
            }
            return amendment_reads::resume(core, env, work, *amendment);
        }
        ReadFor::Preparation { task, last_message } => {
            let current = match core.contexts.get(&task) {
                Some(context) if context.last_message == last_message => match core.tasks.task(task) {
                    Some(row) => {
                        row.phase == tasks::Phase::Active(tasks::Active::Preparing)
                            && core.tasks.run_message(task) == Some(last_message)
                    }
                    None => false,
                },
                Some(_) | None => false,
            };
            if !current {
                return Requests::Out(out);
            }
            let result = if busy {
                None
            } else {
                match row {
                    Some(row) => core.dependency_result(task, *read.ids.get(read.at).expect("pending result ID"), row),
                    None => None,
                }
            };
            let Some(result) = result else {
                work.push(Event::Tasks(tasks::Event::PreparationFailed { task }));
                return Requests::Out(out);
            };
            read.results.push(result).expect("one result per ID");
            read.at = read.at.checked_add(1).expect("bounded result position");
            if read.at < read.ids.len() {
                ask(&mut out, request, &read);
                assert!(core.result_reads.insert(request, read).is_ok(), "continuing result read");
            } else {
                assert!(
                    core.dependency_results.insert(task, read.results.into_boxed()).is_ok(),
                    "live task results room"
                );
                work.push(Event::StartBrief { task });
            }
        }
    }
    Requests::Out(out)
}

/// Holds only transient replay inputs; no call record or task change precedes the load.
pub(super) fn wait(
    core: &mut Core,
    work: &mut Queue<Event>,
    out: &mut Queue<Request>,
    next: u64,
    amendment: AmendmentRead,
) {
    if core.result_reads.len() == core.result_reads.capacity() {
        amendment_reads::busy(core, work, out, amendment.replay);
        return;
    }
    match &amendment.replay {
        Replay::Agent { key, .. } => {
            assert!(core.pending_calls.insert(*key, true).is_ok(), "waiting amendment call room");
        }
        Replay::Person { .. } => {}
    }
    let request = Token::new(core.next_result_request);
    core.next_result_request = core.next_result_request.checked_add(1).expect("transient result request space");
    let read = ResultRead {
        why: ReadFor::Amendment(Box::new(amendment)),
        ids: Box::new([next]),
        at: 0,
        results: List::with_capacity(0),
    };
    ask(out, request, &read);
    assert!(core.result_reads.insert(request, read).is_ok(), "admitted amendment read");
}

pub(super) fn clear_person(core: &mut Core, person: u64) {
    for _ in 0..core.result_reads.capacity() {
        let mut found = None;
        for (&request, read) in &core.result_reads {
            match &read.why {
                ReadFor::Amendment(read) => match &read.replay {
                    Replay::Person { person: owner, .. } if *owner == person => {
                        found = Some(request);
                        break;
                    }
                    Replay::Person { .. } | Replay::Agent { .. } => {}
                },
                ReadFor::ProjectionNews { .. } | ReadFor::DeploymentClose { .. } | ReadFor::Preparation { .. } => {}
            }
        }
        let Some(request) = found else { break };
        drop(core.result_reads.remove(&request));
    }
}

/// A close of an archived chat uses the existing durable ended-task point load.
pub(super) fn closed_subject(
    core: &mut Core,
    work: &mut Queue<Event>,
    out: &mut Queue<Request>,
    request: Token,
    project: u32,
    task: u64,
) {
    if core.result_reads.len() == core.result_reads.capacity() {
        work.push(Event::People(jig_domain_people::Event::Decided {
            request,
            outcome: jig_domain_people::Outcome::Again(jig_domain_people::Again::Busy),
        }));
        return;
    }
    let load = Token::new(core.next_result_request);
    core.next_result_request = core.next_result_request.checked_add(1).expect("transient result request space");
    let read = ResultRead {
        why: ReadFor::DeploymentClose { request, project },
        ids: Box::new([task]),
        at: 0,
        results: List::with_capacity(0),
    };
    ask(out, load, &read);
    assert!(core.result_reads.insert(load, read).is_ok(), "admitted close subject load");
}

fn closed_outcome(
    project: u32,
    wanted: u64,
    row: Option<Box<tasks::TaskRecord>>,
    busy: bool,
) -> jig_domain_people::Outcome {
    if busy {
        jig_domain_people::Outcome::Again(jig_domain_people::Again::Busy)
    } else {
        let why = match row {
            Some(row) if row.project == project && row.number == wanted => match &row.phase {
                tasks::Phase::Ended(ending) => super::refusals::ended(ending),
                tasks::Phase::Closing(closing) => super::refusals::ended(&closing.ending),
                tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Held { .. } => {
                    jig_domain_people::Refusal::Unknown
                }
            },
            Some(_) | None => jig_domain_people::Refusal::Unknown,
        };
        jig_domain_people::Outcome::Refused(why)
    }
}

/// News shares the preparation read budget, with at most one news load in flight.
pub(crate) fn news_room(core: &Core) -> bool {
    if core.result_reads.len() == core.result_reads.capacity() {
        return false;
    }
    for (_, read) in &core.result_reads {
        match read.why {
            ReadFor::ProjectionNews { .. } => return false,
            ReadFor::DeploymentClose { .. } | ReadFor::Preparation { .. } | ReadFor::Amendment(_) => {}
        }
    }
    true
}

pub(crate) fn news(core: &mut Core, goal: u64, connector: u16, out: &mut Queue<Request>) {
    let request = Token::new(core.next_result_request);
    core.next_result_request = core.next_result_request.checked_add(1).expect("transient result request space");
    let read = ResultRead {
        why: ReadFor::ProjectionNews { goal, connector },
        ids: Box::new([goal]),
        at: 0,
        results: List::with_capacity(0),
    };
    ask(out, request, &read);
    assert!(core.result_reads.insert(request, read).is_ok(), "admitted news read");
}

pub(crate) fn forget_news(core: &mut Core, goal: u64) {
    let mut found = None;
    for (&request, read) in &core.result_reads {
        match read.why {
            ReadFor::ProjectionNews { goal: owner, .. } if owner == goal => {
                found = Some(request);
                break;
            }
            ReadFor::ProjectionNews { .. }
            | ReadFor::DeploymentClose { .. }
            | ReadFor::Preparation { .. }
            | ReadFor::Amendment(_) => {}
        }
    }
    if let Some(request) = found {
        drop(core.result_reads.remove(&request));
    }
}
