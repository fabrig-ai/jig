//! People-child outputs translated to durable replies and one exhaustive party
//! request dispatcher (`domain/people.md`, section 5.1). The former chain's
//! observable precedence remains in `party_decisions`: person-origin goals
//! before live task proposals before history; current escalation revisions
//! before history. Other families select disjoint variants.

use super::note_routes::edit_note_request;
use super::parties::{adopt_request, change_policy_request, make_service_request, pool_route, roles_route};
use super::party_controls::{
    amend_route, answer_question_request, cancel_request, move_request, person_task_request, release_request,
    say_request, stop_request,
};
use super::party_decisions::{escalation_request, proposal_request};
use crate::{Core, Event, Held, Key, Limits, Now, Record, Request, Requests, Write};
use alloc::boxed::Box;
use jig_domain_accounts as accounts;
use jig_domain_people as people;
use skein_lib::{Env, Queue, ReplyTo};

pub(super) fn route_people(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    event: people::Event,
) -> Requests {
    if event == people::Event::Restored {
        let stage = core.restart_step();
        if stage == Some(crate::RestartStep::LoadCore) || stage == Some(crate::RestartStep::LoadInbox) {
            core.counters.finish_restore();
            if core.counters.dirty() && !core.people.ready() {
                return super::first_start::seed(core, env, work);
            }
        }
        core.seed_owners = Box::new([]);
        core.seed_projects = Box::new([]);
    }
    if let people::Event::Forget { task, .. } = &event {
        drop(core.archived_chats.remove(task));
    }
    let mut unused = None;
    let event = match event {
        people::Event::Decided { request, outcome } => {
            if let Some(flight) = core.applications.get_mut(&request)
                && flight.part
            {
                flight.part = false;
                people::application_part_done(&mut core.people, request);
                return super::application_routes::answer(flight.connector, request, outcome);
            }
            if let Some(flight) = core.applications.remove(&request) {
                unused = flight.carried;
                if let people::Outcome::Applied { .. } = &outcome {
                    unused = None;
                }
            }
            let key_unused = super::key_routes::finished(core, request, &outcome);
            assert!(unused.is_none() || key_unused.is_none(), "one carried owner per people flight");
            unused = unused.or(key_unused);
            people::Event::Decided { request, outcome }
        }
        people::Event::SignOut { reply_to, sign_in } => {
            if let Some(person) = core.people.person(sign_in, env.now, env.wall) {
                super::result_reads::clear_person(core, person);
            }
            people::Event::SignOut { reply_to, sign_in }
        }
        other @ (people::Event::ChatState { .. }
        | people::Event::MoveChat { .. }
        | people::Event::ConversationSeen { .. }
        | people::Event::Forget { .. }
        | people::Event::MakeService { .. }
        | people::Event::Seed { .. }
        | people::Event::Waiting { .. }
        | people::Event::ApplyRoles { .. }
        | people::Event::SignedIn { .. }
        | people::Event::Roles { .. }
        | people::Event::Act { .. }
        | people::Event::Whose { .. }
        | people::Event::Ask { .. }
        | people::Event::Restore { .. }
        | people::Event::Restored) => other,
    };
    super::view_routes::touch_people_event(core, &env.limits, &event);
    let mut child = Queue::with_capacity(people::max_out(&env.limits.people));
    people::step(&mut core.people, &Env { now: env.now, wall: env.wall, limits: env.limits.people }, event, &mut child);
    super::application_routes::first(unused, tag_people(core, env, child, work))
}

pub(super) fn sign_in(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    reply_to: ReplyTo,
    identity: people::Identity,
    held: Option<u64>,
) -> Requests {
    let mut out = Queue::with_capacity(2);
    if !core.people.ready() {
        out.push(Request::Held(Box::new(Held::PeopleReply {
            to: reply_to,
            reply: people::Reply::Again(people::Again::NotReady),
        })));
    } else if core.fill >= env.limits.fill_mark
        || core.counters.deployment().people == u64::MAX
        || core.counters.deployment().sign_ins == u64::MAX
    {
        out.push(Request::Now(Box::new(Now::SignInRefused { to: reply_to })));
    } else {
        let person = crate::fresh(&mut core.counters, crate::Family::Person).expect("person counter available");
        let sign_in = crate::fresh(&mut core.counters, crate::Family::SignIn).expect("sign-in counter available");
        let kind = if identity.key.provider == core.settings.deployment_provider {
            people::Kind::Service
        } else {
            people::Kind::Person
        };
        work.push(Event::People(people::Event::SignedIn { reply_to, person, sign_in, identity, kind, held }));
    }
    out.push(Request::Decided);
    Requests::Out(out)
}
#[expect(clippy::too_many_lines, reason = "one exhaustive party vocabulary routes its bounded child decision")]
pub(super) fn tag_people(
    core: &mut Core,
    env: &Env<Limits>,
    mut child: Queue<people::Request>,
    work: &mut Queue<Event>,
) -> Requests {
    let room = people::max_out(&env.limits.people)
        .checked_mul(4)
        .expect("bounded party routes")
        .checked_add(env.limits.people.inbox_tasks)
        .expect("bounded party routes")
        .max(crate::settings::writes_worst_case(&env.limits).expect("settings output room"));
    let mut pending = Queue::with_capacity(room);
    for _ in 0..child.len() {
        pending.push(child.pop().expect("party output count"));
    }
    let mut out = Queue::with_capacity(room.checked_add(1).expect("party output room"));
    for _ in 0..room {
        let Some(next) = pending.pop() else { break };
        let request = match next {
            people::Request::Act { request, connector: _, act } => {
                super::deployment_acts::route(core, env, work, &mut out, request, *act);
                continue;
            }
            people::Request::Acted { to, outcome } => {
                if outcome == people::Outcome::Again(people::Again::Busy) {
                    core.touch_deployment();
                }
                Request::Held(Box::new(Held::Acted { to, outcome }))
            }
            people::Request::ServiceMade { request, outcome } => {
                work.push(Event::People(people::Event::Decided { request, outcome }));
                continue;
            }
            people::Request::Save { record } => {
                super::view_routes::touch_people_row(core, &record);
                Request::Write(Write::Save(Record::People(record)))
            }
            people::Request::Erase { key } => {
                match key {
                    people::Key::Answer(_) | people::Key::Acted(_) => core.touch_deployment(),
                    people::Key::Person(_)
                    | people::Key::ReadResults { .. }
                    | people::Key::EndedRead { .. }
                    | people::Key::ReadConversation { .. }
                    | people::Key::SignIn(_)
                    | people::Key::Roles(_)
                    | people::Key::Policy(_) => {}
                }
                Request::Write(Write::Erase(Key::People(key)))
            }
            people::Request::Reply { to, reply } => match reply {
                reply @ (people::Reply::Full { .. }
                | people::Reply::Again(people::Again::Busy)
                | people::Reply::Outcome(people::Outcome::Again(people::Again::Busy))) => {
                    core.touch_deployment();
                    Request::Held(Box::new(Held::PeopleReply { to, reply }))
                }
                people::Reply::Outcome(people::Outcome::Applied { connector, token }) => {
                    Request::Held(Box::new(Held::Applied { to, connector, token }))
                }
                reply @ (people::Reply::Signer(_)
                | people::Reply::SignedIn { .. }
                | people::Reply::SignedOut
                | people::Reply::Again(..)
                | people::Reply::Refused(..)
                | people::Reply::Outcome(
                    people::Outcome::Compound { .. }
                    | people::Outcome::CompoundRefused { .. }
                    | people::Outcome::Again(..)
                    | people::Outcome::SignedOut
                    | people::Outcome::MarkedSeen { .. }
                    | people::Outcome::NoteEdited { .. }
                    | people::Outcome::ServiceMade { .. }
                    | people::Outcome::Adopted { .. }
                    | people::Outcome::Created { .. }
                    | people::Outcome::Proposed { .. }
                    | people::Outcome::ReleaseProposed { .. }
                    | people::Outcome::Withdrawn { .. }
                    | people::Outcome::Stopped { .. }
                    | people::Outcome::Cancelled { .. }
                    | people::Outcome::Placed { .. }
                    | people::Outcome::Lifted { .. }
                    | people::Outcome::Released { .. }
                    | people::Outcome::PersonTaken { .. }
                    | people::Outcome::PersonHandedBack { .. }
                    | people::Outcome::PersonAnswered { .. }
                    | people::Outcome::Moved { .. }
                    | people::Outcome::ProposalDecided { .. }
                    | people::Outcome::Said { .. }
                    | people::Outcome::QuestionAnswered { .. }
                    | people::Outcome::Amended { .. }
                    | people::Outcome::RolesSet { .. }
                    | people::Outcome::PolicyChanged { .. }
                    | people::Outcome::Renamed { .. }
                    | people::Outcome::ProjectMade { .. }
                    | people::Outcome::SettingsChanged { .. }
                    | people::Outcome::MovedToNewest { .. }
                    | people::Outcome::PoolSet { .. }
                    | people::Outcome::EscalationDecided { .. }
                    | people::Outcome::AccountSignInStarted { .. }
                    | people::Outcome::AccountProgramDeclared { .. }
                    | people::Outcome::AccountKeyKept { .. }
                    | people::Outcome::AccountMovingOut { .. }
                    | people::Outcome::AccountFinishing { .. }
                    | people::Outcome::AccountMovedIn { .. }
                    | people::Outcome::AccountChanged
                    | people::Outcome::AccountRemoved { .. }
                    | people::Outcome::AccountSignedOut { .. }
                    | people::Outcome::Refused(..),
                )) => Request::Held(Box::new(Held::PeopleReply { to, reply })),
            },
            people::Request::RolesApplied { .. } => continue,
            people::Request::RolesRefused { .. } | people::Request::RestoreRefused { .. } => {
                Request::Now(Box::new(Now::RestoreRefused))
            }
            people::Request::Route { request, person, project, role, mut ask, models } => {
                if !crate::model_names::resolve(&mut ask, &models, &core.settings.models) {
                    work.push(Event::People(people::Event::Decided {
                        request,
                        outcome: people::Outcome::Refused(people::Refusal::Unknown),
                    }));
                    continue;
                }
                if let Err(why) = crate::acts::request(core, &env.limits, person, project, role, &ask, env.wall) {
                    work.push(Event::People(people::Event::Decided {
                        request,
                        outcome: people::Outcome::Refused(why),
                    }));
                    continue;
                }
                match *ask {
                    people::Ask::Compound { operations, .. } => {
                        crate::compound::route(
                            core,
                            env,
                            work,
                            crate::compound::Route { request, person, project, role },
                            operations,
                        );
                    }
                    people::Ask::Application { scope, connector, .. } => {
                        super::application_routes::admitted(
                            core, env, work, &mut out, request, connector, person, scope, role,
                        );
                    }
                    ask @ (people::Ask::TakePerson { .. }
                    | people::Ask::HandBackPerson { .. }
                    | people::Ask::AnswerPerson { .. }) => {
                        person_task_request(core, work, &mut out, request, person, project, role, ask);
                    }
                    ask @ people::Ask::Stop { .. } => {
                        stop_request(core, env, work, request, person, project, role, ask);
                    }
                    ask @ people::Ask::Cancel { .. } => {
                        cancel_request(core, env, work, request, person, project, role, ask);
                    }
                    people::Ask::Hold { mode, scope, reason, .. } => {
                        super::party_controls::hold_request(
                            core, env, work, request, person, project, role, mode, scope, reason,
                        );
                    }
                    people::Ask::Lift { hold, revision, with_tasks } => {
                        super::party_controls::lift_request(core, work, request, person, hold, revision, with_tasks);
                    }
                    ask @ people::Ask::Release { .. } => {
                        release_request(core, env, work, request, person, project, role, ask);
                    }
                    people::Ask::MakeService { role: service_role, .. } => {
                        make_service_request(core, env, request, person, project, &mut pending, service_role);
                    }
                    people::Ask::EditNote { name, scope, change, .. } => {
                        edit_note_request(core, work, request, person, project, role, name, scope, *change);
                    }
                    people::Ask::Adopt { adoption, .. } => {
                        adopt_request(core, env, work, request, person, project, &mut out, adoption);
                    }
                    people::Ask::SetPool { person: beneficiary, budget, .. } => {
                        pool_route(core, env, work, request, person, project, beneficiary, budget);
                    }
                    people::Ask::DecideProposal { proposer, proposal, decision, running, .. } => proposal_request(
                        core, env, work, request, person, project, role, &mut out, proposer, proposal, decision,
                        running,
                    ),
                    people::Ask::SetRoles { holdings, .. } => {
                        roles_route(core, env, work, &mut out, request, person, project, holdings);
                    }
                    people::Ask::Amend { task, revision, amendment, release, .. } => {
                        amend_route(
                            core,
                            env,
                            work,
                            &mut out,
                            request,
                            person,
                            role,
                            project,
                            task,
                            revision,
                            amendment,
                            release,
                            super::amendment_reads::Evidence::new(&env.limits.tasks),
                        );
                    }
                    people::Ask::Rename { name, .. } => {
                        super::projects::rename_request(core, env, work, &mut out, request, person, project, name);
                    }
                    people::Ask::MoveToNewest { revision } => {
                        let outcome = match super::settings::move_to_newest(core, env, &mut out, revision) {
                            Ok(moved) => people::Outcome::MovedToNewest {
                                newest: core.current.newest(),
                                moved: moved.moved,
                                stayed: moved.stayed,
                            },
                            Err(why) => people::Outcome::Refused(why),
                        };
                        work.push(Event::People(people::Event::Decided { request, outcome }));
                    }
                    people::Ask::ChangeSettings { against, .. } => {
                        super::settings::admitted(core, env, work, &mut out, request, person, against);
                    }
                    people::Ask::MakeProject { name, policy } => {
                        super::projects::make_project_request(core, env, work, &mut out, request, person, name, policy);
                    }
                    people::Ask::ChangePolicy { change, .. } => {
                        change_policy_request(core, env, work, request, person, project, &mut out, change);
                    }
                    people::Ask::DecideEscalation { task, revision, decision, .. } => escalation_request(
                        core, env, work, request, person, project, role, &mut out, task, revision, decision,
                    ),
                    people::Ask::Create { batch, .. } => super::create::create_request(
                        core,
                        env,
                        &mut out,
                        work,
                        super::create::Creation { request, person, project, role: role.expect("admitted role"), batch },
                    ),
                    people::Ask::Propose { action, reason, .. } => match action {
                        people::Action::ReleasePlaced { task, hold, revision } => super::party_releases::offer(
                            core,
                            env,
                            work,
                            super::party_releases::Offer {
                                request,
                                person,
                                project,
                                role,
                                task,
                                hold,
                                revision,
                                reason,
                            },
                        ),
                        people::Action::Settings { against, .. } => {
                            super::settings::propose_admitted(core, env, work, request, person, against, reason);
                        }
                        people::Action::Batch(batch) => super::propose::propose_request(
                            core,
                            env,
                            &mut out,
                            work,
                            super::create::Creation {
                                request,
                                person,
                                project,
                                role: role.expect("admitted role"),
                                batch,
                            },
                            reason,
                        ),
                        people::Action::Amend { task, revision, amendment } => super::propose::amend_request(
                            core,
                            env,
                            work,
                            super::propose::Edit {
                                request,
                                person,
                                project,
                                role,
                                task,
                                revision,
                                amendment: *amendment,
                                reason,
                            },
                        ),
                    },
                    people::Ask::AccountChange { holder, change } => {
                        let holder = super::account_requests::holder(holder, person);
                        match super::key_routes::admitted(core, request, holder, Some(person), change) {
                            Ok(change) => {
                                work.push(Event::Account(accounts::Event::Change { request, holder, change }));
                            }
                            Err(why) => work.push(Event::People(people::Event::Decided {
                                request,
                                outcome: people::Outcome::Refused(why),
                            })),
                        }
                    }
                    people::Ask::AccountSignIn { holder, provider } => {
                        let holder = super::account_requests::holder(holder, person);
                        work.push(Event::Account(accounts::Event::SignIn { request, holder, by: person, provider }));
                    }
                    people::Ask::AccountSignOut { holder, source } => {
                        let holder = super::account_requests::holder(holder, person);
                        work.push(Event::Account(accounts::Event::SignOut { request, holder, source }));
                    }
                    people::Ask::Withdraw { proposal, .. } => super::propose::withdraw_request(
                        core,
                        work,
                        &mut out,
                        super::propose::Withdrawal { request, person, project, proposal },
                    ),
                    people::Ask::MarkSeen { seen } => match seen {
                        people::Seen::Results { through } => {
                            let mut loading = false;
                            for result in core.people.cached_results(person).unwrap_or(&[]) {
                                loading |= result.position <= through
                                    && result.forgettable.is_some()
                                    && core.people.conversation_mark_loading(result.task, request);
                            }
                            if loading {
                                work.push(Event::People(people::Event::Decided {
                                    request,
                                    outcome: people::Outcome::Again(people::Again::Busy),
                                }));
                                continue;
                            }
                            work.push(Event::People(people::Event::Decided {
                                request,
                                outcome: people::Outcome::MarkedSeen { seen },
                            }));
                            for result in core.people.cached_results(person).unwrap_or(&[]) {
                                if result.position <= through
                                    && core.tasks.task(result.task).is_none()
                                    && let Some(owner) = crate::forget::Owner::of_result(person, result)
                                {
                                    let turns = if result.forgettable.expect("forgettable owner").turns_read {
                                        owner.last_reply
                                    } else {
                                        None
                                    };
                                    if crate::forget::forgets(
                                        &owner,
                                        crate::forget::Reading {
                                            results: through,
                                            turns,
                                            named: core.tasks.stub(owner.task).is_some(),
                                        },
                                    )
                                    .is_some()
                                    {
                                        work.push(Event::Forget { owner });
                                    }
                                }
                            }
                        }
                        people::Seen::Conversation { task, .. }
                            if core.people.conversation_mark_loading(task, request) =>
                        {
                            work.push(Event::People(people::Event::Decided {
                                request,
                                outcome: people::Outcome::Again(people::Again::Busy),
                            }));
                        }
                        people::Seen::Conversation { task, .. } => match core.tasks.task(task) {
                            Some(row) => {
                                if core.conversation_requester(row) == Some(person)
                                    && core.people.role(person, row.project).is_some()
                                {
                                    work.push(Event::People(people::Event::ConversationSeen {
                                        project: row.project,
                                        request,
                                        last_reply: row.last_reply,
                                    }));
                                } else {
                                    work.push(Event::People(people::Event::Decided {
                                        request,
                                        outcome: people::Outcome::Refused(people::Refusal::Role {
                                            held: core.people.role(person, row.project),
                                        }),
                                    }));
                                }
                            }
                            None => {
                                out.push(Request::Held(Box::new(Held::Load(Box::new(Now::SeenConversation {
                                    request,
                                    task,
                                })))));
                            }
                        },
                    },
                    people::Ask::Say { task, words, .. } => {
                        say_request(core, env, work, request, person, project, task, words);
                    }
                    people::Ask::AnswerQuestion { task, question, words, .. } => {
                        answer_question_request(core, env, work, request, person, project, task, question, words);
                    }
                    people::Ask::Move { task, reason, .. } => {
                        move_request(core, env, work, request, person, project, role, task, reason);
                    }
                }
                continue;
            }
        };
        out.push(request);
    }
    assert!(pending.is_empty(), "finite party routes fit their bound");
    out.push(Request::Decided);
    Requests::Out(out)
}
