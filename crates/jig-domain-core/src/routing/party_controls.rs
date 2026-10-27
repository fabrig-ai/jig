//! Party task controls, amendments, messages, answers and moves routed to tasks
//! under current authority (`domain/people.md`, section 5.1).

use super::refusals::pool_refused;
use crate::{Core, Event, Limits, Now, PersonTaskRoute, Request};
use alloc::boxed::Box;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{Env, Queue, ReplyTo, Token};

#[expect(clippy::too_many_arguments, reason = "a keyed amendment carries its person, project and task")]
pub(super) fn amend_route(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    out: &mut Queue<Request>,
    request: Token,
    person: u64,
    role: Option<people::Role>,
    project: u32,
    task: u64,
    revision: u64,
    amendment: people::Amendment,
    release: Option<u64>,
    evidence: super::amendment_reads::Evidence,
) {
    if let Some(next) = super::amendment_reads::missing(
        core,
        &env.limits.tasks,
        tasks::Party::Person(person),
        task,
        amendment.dependencies.as_deref(),
        &evidence,
    ) {
        super::result_reads::wait(
            core,
            work,
            out,
            next,
            super::amendment_reads::AmendmentRead {
                replay: super::amendment_reads::Replay::Person {
                    request,
                    person,
                    project,
                    task,
                    revision,
                    amendment,
                    release,
                },
                evidence,
            },
        );
        return;
    }
    let Some(mut amendment) = crate::amendments::translate(amendment, core.kept_models(task), &env.limits.tasks) else {
        pool_refused(work, request, people::Refusal::Limit { which: people::LimitKind::Words });
        return;
    };
    let stop_run = match crate::acts::bounds::amendment_admit_with_places(
        core,
        person,
        role,
        project,
        task,
        &amendment,
        env.limits.tasks.depth,
        &env.limits.tasks,
        evidence.places.as_slice(),
    ) {
        Ok(admitted) => admitted,
        Err(why) => {
            pool_refused(work, request, why);
            return;
        }
    };
    if let Err(why) = core.party_amend_revision(task, revision, &amendment) {
        pool_refused(work, request, why);
        return;
    }
    if let Some(revision) = release {
        if let Err(why) = core.hold_revision(task, revision) {
            pool_refused(work, request, why);
            return;
        }
        match core.tasks.task(task).expect("amend admission found task").phase {
            tasks::Phase::Held { why: tasks::Hold::Uncertain { .. }, .. } => {
                pool_refused(
                    work,
                    request,
                    people::Refusal::Authority { proposable: None, lacking: people::Lack::Again { required: true } },
                );
                return;
            }
            tasks::Phase::Held { .. }
            | tasks::Phase::Waiting
            | tasks::Phase::Active(_)
            | tasks::Phase::Closing(_)
            | tasks::Phase::Ended(_) => {}
        }
    }
    amendment.release = release;
    let Some(message) = crate::fresh(&mut core.counters, crate::Family::Message) else {
        pool_refused(work, request, people::Refusal::Limit { which: people::LimitKind::Arithmetic });
        return;
    };
    let released = match core.tasks.task(task).expect("amend admission found task").phase {
        tasks::Phase::Held { .. } => release.is_some(),
        tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Closing(_) | tasks::Phase::Ended(_) => false,
    };
    assert!(
        core.person_tasks.insert(request, PersonTaskRoute::Amended { task, released }) == Ok(None),
        "one person amendment"
    );
    work.push(Event::Tasks(tasks::Event::Amend {
        places: evidence.places.into_boxed(),
        funder: None,
        reply_to: ReplyTo::new(request),
        by: tasks::Party::Person(person),
        task,
        message,
        stop_run,
        amendment,
    }));
}

#[expect(clippy::too_many_arguments, reason = "one authenticated person-task route carries its output queue")]
pub(super) fn person_task_request(
    core: &mut Core,
    work: &mut Queue<Event>,
    out: &mut Queue<Request>,
    request: Token,
    person: u64,
    project: u32,
    role: Option<people::Role>,
    ask: people::Ask,
) {
    let task = crate::person_task::number(&ask);
    if core.tasks.task(task).is_none() {
        out.push(Request::Now(Box::new(Now::HistoricalPersonTask { request, person, project, task })));
        return;
    }
    match core.person_task(request, person, project, role, ask) {
        Ok(event) => work.push(Event::Tasks(event)),
        Err(why) => {
            work.push(Event::People(people::Event::Decided { request, outcome: people::Outcome::Refused(why) }));
        }
    }
}

#[expect(clippy::too_many_arguments, reason = "the route retains the admitted party and request fields")]
pub(super) fn stop_request(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    request: Token,
    person: u64,
    project: u32,
    role: Option<people::Role>,
    ask: people::Ask,
) {
    match core.control_admit(&ask, person, project, role, &env.limits.tasks) {
        Ok(task) => {
            let Some(number) = crate::fresh(&mut core.counters, crate::Family::Message) else {
                work.push(Event::People(people::Event::Decided {
                    request,
                    outcome: people::Outcome::Refused(people::Refusal::Limit { which: people::LimitKind::Arithmetic }),
                }));
                return;
            };
            assert!(
                core.person_tasks.insert(request, PersonTaskRoute::Stop(task)) == Ok(None),
                "one stop continuation"
            );
            work.push(Event::Tasks(tasks::Event::Place {
                reply_to: ReplyTo::new(request),
                hold: tasks::PlacedHold {
                    number,
                    project,
                    owner: tasks::Owner::Party { person },
                    mode: tasks::Mode::Stop,
                    scope: tasks::Scope::Subtree { task },
                    reason: Box::new([]),
                    placed_at: env.wall.as_nanos() / 1_000_000,
                    revision: 1,
                    exempt: Box::new([]),
                    lifted: false,
                },
            }));
        }
        Err(why) => {
            work.push(Event::People(people::Event::Decided { request, outcome: people::Outcome::Refused(why) }));
        }
    }
}

#[expect(clippy::too_many_arguments, reason = "the route retains the admitted party and request fields")]
pub(super) fn cancel_request(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    request: Token,
    person: u64,
    project: u32,
    role: Option<people::Role>,
    ask: people::Ask,
) {
    let admitted = core.control_admit(&ask, person, project, role, &env.limits.tasks);
    let (reason, grace) = match ask {
        people::Ask::Cancel { reason, grace, .. } => (reason, grace),
        people::Ask::Compound { .. }
        | people::Ask::MarkSeen { .. }
        | people::Ask::Application { .. }
        | people::Ask::EditNote { .. }
        | people::Ask::MakeService { .. }
        | people::Ask::Adopt { .. }
        | people::Ask::Create { .. }
        | people::Ask::Propose { .. }
        | people::Ask::Withdraw { .. }
        | people::Ask::Stop { .. }
        | people::Ask::Release { .. }
        | people::Ask::TakePerson { .. }
        | people::Ask::HandBackPerson { .. }
        | people::Ask::AnswerPerson { .. }
        | people::Ask::DecideProposal { .. }
        | people::Ask::Amend { .. }
        | people::Ask::SetRoles { .. }
        | people::Ask::Rename { .. }
        | people::Ask::MakeProject { .. }
        | people::Ask::ChangeSettings { .. }
        | people::Ask::MoveToNewest { .. }
        | people::Ask::Restore { .. }
        | people::Ask::ChangePolicy { .. }
        | people::Ask::SetPool { .. }
        | people::Ask::DecideEscalation { .. }
        | people::Ask::Say { .. }
        | people::Ask::AnswerQuestion { .. }
        | people::Ask::Move { .. }
        | people::Ask::AccountSignIn { .. }
        | people::Ask::AccountSignOut { .. }
        | people::Ask::AccountChange { .. } => unreachable!("cancel route retains its ask"),
    };
    match admitted {
        Ok(task) => {
            let message = if grace.is_some() {
                let Some(message) = crate::fresh(&mut core.counters, crate::Family::Message) else {
                    pool_refused(work, request, people::Refusal::Limit { which: people::LimitKind::Arithmetic });
                    return;
                };
                Some(message)
            } else {
                None
            };
            assert!(core.person_tasks.insert(request, PersonTaskRoute::Cancel(task)) == Ok(None), "one cancel route");
            work.push(Event::Tasks(tasks::Event::Control {
                reply_to: ReplyTo::new(request),
                by: tasks::Party::Person(person),
                task,
                control: tasks::Control::Cancel { reason, grace, message },
            }));
        }
        Err(why) => {
            work.push(Event::People(people::Event::Decided { request, outcome: people::Outcome::Refused(why) }));
        }
    }
}

#[expect(clippy::too_many_arguments, reason = "the route retains the admitted party and request fields")]
pub(super) fn release_request(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    request: Token,
    person: u64,
    project: u32,
    role: Option<people::Role>,
    ask: people::Ask,
) {
    match core.control_admit(&ask, person, project, role, &env.limits.tasks) {
        Ok(task) => {
            let (revision, again) = match ask {
                people::Ask::Release { revision, again, .. } => (revision, again),
                people::Ask::Compound { .. }
                | people::Ask::MarkSeen { .. }
                | people::Ask::Application { .. }
                | people::Ask::EditNote { .. }
                | people::Ask::MakeService { .. }
                | people::Ask::Adopt { .. }
                | people::Ask::Create { .. }
                | people::Ask::Propose { .. }
                | people::Ask::Withdraw { .. }
                | people::Ask::Stop { .. }
                | people::Ask::Cancel { .. }
                | people::Ask::TakePerson { .. }
                | people::Ask::HandBackPerson { .. }
                | people::Ask::AnswerPerson { .. }
                | people::Ask::Move { .. }
                | people::Ask::DecideProposal { .. }
                | people::Ask::Say { .. }
                | people::Ask::AnswerQuestion { .. }
                | people::Ask::Amend { .. }
                | people::Ask::SetRoles { .. }
                | people::Ask::Rename { .. }
                | people::Ask::MakeProject { .. }
                | people::Ask::ChangeSettings { .. }
                | people::Ask::MoveToNewest { .. }
                | people::Ask::Restore { .. }
                | people::Ask::ChangePolicy { .. }
                | people::Ask::SetPool { .. }
                | people::Ask::DecideEscalation { .. }
                | people::Ask::AccountSignIn { .. }
                | people::Ask::AccountSignOut { .. }
                | people::Ask::AccountChange { .. } => unreachable!("release route owns its ask"),
            };
            let message = match core.release_admit(task, revision, again) {
                Ok(message) => message,
                Err(why) => {
                    pool_refused(work, request, why);
                    return;
                }
            };
            assert!(core.person_tasks.insert(request, PersonTaskRoute::Release(task)) == Ok(None), "one release route");
            work.push(Event::Tasks(tasks::Event::Control {
                reply_to: ReplyTo::new(request),
                by: tasks::Party::Person(person),
                task,
                control: tasks::Control::Release { again, message },
            }));
        }
        Err(why) => {
            work.push(Event::People(people::Event::Decided { request, outcome: people::Outcome::Refused(why) }));
        }
    }
}

#[expect(clippy::too_many_arguments, reason = "the route retains the admitted party and request fields")]
pub(super) fn say_request(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    request: Token,
    person: u64,
    project: u32,
    task: u64,
    words: Box<[u8]>,
) {
    match core.person_message(
        &env.limits,
        crate::PersonMessage { request, person, project, task, question: None, words, at: env.wall },
    ) {
        Ok(event) => work.push(Event::Tasks(event)),
        Err(why) => {
            work.push(Event::People(people::Event::Decided { request, outcome: people::Outcome::Refused(why) }));
        }
    }
}

#[expect(clippy::too_many_arguments, reason = "the route retains the admitted party and request fields")]
pub(super) fn answer_question_request(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    request: Token,
    person: u64,
    project: u32,
    task: u64,
    question: u64,
    words: Box<[u8]>,
) {
    match core.person_message(
        &env.limits,
        crate::PersonMessage { request, person, project, task, question: Some(question), words, at: env.wall },
    ) {
        Ok(event) => work.push(Event::Tasks(event)),
        Err(why) => {
            work.push(Event::People(people::Event::Decided { request, outcome: people::Outcome::Refused(why) }));
        }
    }
}

#[expect(clippy::too_many_arguments, reason = "the route retains the admitted party and request fields")]
pub(super) fn move_request(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    request: Token,
    person: u64,
    project: u32,
    role: Option<people::Role>,
    task: u64,
    reason: Box<[u8]>,
) {
    match core.move_admit(person, role, project, task, env.limits.tasks.tree_tasks, env.limits.tasks.depth) {
        Ok(()) => {
            assert!(core.moving.insert(request, task) == Ok(None), "one move flight per keyed request");
            work.push(Event::Tasks(tasks::Event::Move {
                reply_to: ReplyTo::new(request),
                task,
                person,
                period: core.period(project),
                pool_budget: core.pool_budget(project, person),
                period_budget: core.period_budget(project),
                reason,
            }));
        }
        Err(why) => {
            work.push(Event::People(people::Event::Decided { request, outcome: people::Outcome::Refused(why) }));
        }
    }
}
