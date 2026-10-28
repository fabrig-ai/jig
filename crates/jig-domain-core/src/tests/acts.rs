//! Offers and the authenticated entrance share their current authority facts.
use super::*;
use crate::acts as offers;
use crate::acts::{Act, Admission, Beyond, Bounds};

fn policy_holder(numbers: &[u32]) -> offers::Recipient {
    let mut roles = List::with_capacity(u32::try_from(numbers.len()).unwrap());
    for number in numbers {
        roles.push(*number).unwrap();
    }
    offers::Recipient::Roles { project: 1, roles }
}

fn holding(core: &mut Core, limits: &Limits, role: people::Role) {
    let mut out = Queue::with_capacity(people::max_out(&limits.people));
    people::step(
        &mut core.people,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people },
        people::Event::Roles { project: 1, holdings: Box::new([people::Holding { person: 1, role }]) },
        &mut out,
    );
}

fn policy(core: &mut Core, accept: bool, period: u64) {
    let mut value = core.authority.policy(1).unwrap().clone();
    value.roles[0].decides = if accept { authority::Proposals::ALL } else { authority::Proposals(16) };
    value.roles[0].period_spend = period;
    let mut out = Queue::with_capacity(authority::POLICY_MAX_OUT);
    authority::step(&mut core.authority, authority::Event::Policy { project: 1, policy: value }, &mut out);
    assert_eq!(out.pop(), Some(authority::PolicyFact::Changed { project: 1 }));
}

#[test]
fn a_goal_within_its_offer_is_direct_and_a_supported_goal_beyond_it_is_proposed() {
    let (core, limits) = core();
    let Admission::Bounded { within: Bounds::Create { budget, pool_left, charters, .. }, beyond } =
        fixtures::creating(&core, &limits, 1, 1)
    else {
        panic!("bounded goal")
    };
    assert_eq!((budget, pool_left), (500, 500));
    assert_eq!(charters.as_slice(), &[1]);
    assert_eq!(beyond, Beyond::Proposed { to: policy_holder(&[0]) });
    let started = crate::acts::bounds::create_start(
        &core,
        1,
        1,
        people::Role::Owner,
        tasks::Executor::Agent { charter: 1 },
        budget,
        None,
        limits.tasks.tree_tasks,
    )
    .unwrap();
    assert_eq!(started.authority.budget.spend, budget);
    assert_eq!(
        crate::acts::bounds::create_start(
            &core,
            1,
            1,
            people::Role::Owner,
            tasks::Executor::Agent { charter: 1 },
            budget + 1,
            None,
            limits.tasks.tree_tasks
        )
        .unwrap_err(),
        people::Refusal::Authority {
            lacking: people::Lack::Budget { needs: 501, left: 500 },
            proposable: Some(people::Accepter::Role(0))
        }
    );
    assert_eq!(
        crate::acts::bounds::create_start(
            &core,
            1,
            1,
            people::Role::Owner,
            tasks::Executor::Agent { charter: 2 },
            budget,
            None,
            limits.tasks.tree_tasks
        )
        .unwrap_err(),
        people::Refusal::NotOffered
    );
}

#[test]
fn a_goal_without_an_accepting_role_reports_the_same_refusal_in_its_bound_and_admission() {
    let (mut core, limits) = core();
    policy(&mut core, false, 500);
    let Admission::Bounded { beyond: Beyond::Refused(why), .. } = fixtures::creating(&core, &limits, 1, 1) else {
        panic!("unproposable bound")
    };
    assert_eq!(
        why,
        people::Refusal::Authority { lacking: people::Lack::Approver, proposable: Some(people::Accepter::Nobody) }
    );
    assert_eq!(
        crate::acts::bounds::create_start(
            &core,
            1,
            1,
            people::Role::Owner,
            tasks::Executor::Agent { charter: 1 },
            501,
            None,
            limits.tasks.tree_tasks
        )
        .unwrap_err(),
        people::Refusal::Authority {
            lacking: people::Lack::Budget { needs: 501, left: 500 },
            proposable: Some(people::Accepter::Nobody)
        }
    );
    let started = crate::acts::bounds::create_start(
        &core,
        1,
        1,
        people::Role::Owner,
        tasks::Executor::Agent { charter: 1 },
        100,
        None,
        limits.tasks.tree_tasks,
    )
    .unwrap();
    assert_eq!(started.authority.budget.spend, 100);
}

#[test]
fn a_policy_proposal_names_every_eligible_role_without_choosing_one_holder() {
    let (mut core, mut limits) = core();
    limits.authority.roles = 3;
    let mut policy = core.authority.policy(1).unwrap().clone();
    let mut second = policy.roles[0].clone();
    second.number = 4;
    let mut third = second.clone();
    third.number = 5;
    third.decides = authority::Proposals(16);
    policy.roles = Box::new([policy.roles[0].clone(), second, third]);
    core.authority = authority::Domain::new(core.authority.rules().clone(), limits.authority).unwrap();
    let mut out = Queue::with_capacity(authority::POLICY_MAX_OUT);
    authority::step(&mut core.authority, authority::Event::Policy { project: 1, policy }, &mut out);
    assert_eq!(out.pop(), Some(authority::PolicyFact::Added { project: 1 }));
    let Admission::Bounded { beyond, .. } = fixtures::creating(&core, &limits, 1, 1) else { panic!("bounded goal") };
    assert_eq!(beyond, Beyond::Proposed { to: policy_holder(&[0, 4]) });
    assert_eq!(
        crate::acts::bounds::create_start(
            &core,
            1,
            1,
            people::Role::Owner,
            tasks::Executor::Agent { charter: 1 },
            501,
            None,
            limits.tasks.tree_tasks
        )
        .unwrap_err(),
        people::Refusal::Authority {
            lacking: people::Lack::Budget { needs: 501, left: 500 },
            proposable: Some(people::Accepter::Role(0))
        }
    );
}

#[test]
fn an_amendment_uses_live_pool_and_remaining_task_facts_and_preserves_narrowing() {
    let (mut core, limits) = core();
    policy(&mut core, true, 1000);
    make_task(&mut core, &limits, 1);
    send_task(
        &mut core,
        &limits,
        tasks::Event::CarvePool {
            reply_to: ReplyTo::new(Token::new(2)),
            project: 1,
            person: 1,
            period: 1,
            budget: 600,
        },
    );
    let Admission::Bounded { within: Bounds::Amend { budget_left, authority, ceiling, .. }, beyond } =
        offers::admit(&core, &limits, 1, Act::Amend, &Subject::Task { task: 1 }, Wall::EPOCH)
    else {
        panic!("bounded amendment")
    };
    assert_eq!(budget_left, 400);
    assert_eq!(authority.ceiling.spend, 500);
    assert_eq!(ceiling.ceiling.spend, 600);
    assert_eq!(authority.tasks, core.tasks.delegation(1).unwrap().tasks_left);
    assert_eq!(beyond, Beyond::Proposed { to: policy_holder(&[0]) });
    let mut after = core.tasks.task(1).unwrap().authority.clone();
    after.budget.spend = 550;
    let mut change = tasks::Amendment {
        tracked: None,
        addressed: None,
        instructions: None,
        spec: None,
        wake: None,
        dependencies: None,
        authority: Some(after),
        llm: None,
        release: None,
        reason: Box::new([]),
    };
    assert_eq!(
        core.amend_admit(1, Some(people::Role::Owner), 1, 1, &change, limits.tasks.depth, &limits.tasks),
        Err(people::Refusal::Authority {
            lacking: people::Lack::Budget { needs: 550, left: 500 },
            proposable: Some(people::Accepter::Role(0))
        })
    );
    policy(&mut core, false, 1000);
    let Admission::Bounded { beyond: Beyond::Refused(why), .. } =
        offers::admit(&core, &limits, 1, Act::Amend, &Subject::Task { task: 1 }, Wall::EPOCH)
    else {
        panic!("unproposable amendment")
    };

    assert_eq!(
        why,
        people::Refusal::Authority { lacking: people::Lack::Approver, proposable: Some(people::Accepter::Nobody) }
    );
    assert_eq!(
        core.amend_admit(1, Some(people::Role::Owner), 1, 1, &change, limits.tasks.depth, &limits.tasks),
        Err(people::Refusal::Authority {
            lacking: people::Lack::Budget { needs: 550, left: 500 },
            proposable: Some(people::Accepter::Nobody)
        })
    );
    change.authority.as_mut().unwrap().budget.spend = 90;
    assert_eq!(
        core.amend_admit(1, Some(people::Role::Owner), 1, 1, &change, limits.tasks.depth, &limits.tasks),
        Ok(true)
    );
}

#[test]
fn an_observers_core_role_refusals_match_their_offers() {
    let (mut core, limits) = core();
    holding(&mut core, &limits, people::Role::Observer);
    let project = Subject::Project { project: 1 };
    let expected = people::Refusal::Role { held: Some(people::Role::Observer) };
    for act in [Act::Reorder, Act::Adopt, Act::ChangePolicy, Act::SetRoles, Act::MakeService, Act::SetPool] {
        assert_eq!(offers::admit(&core, &limits, 1, act, &project, Wall::EPOCH), Admission::Refused(expected.clone()));
    }
    core.people = people::Domain::new(&limits.people, Box::new([]), 2, Box::new([]));
    let mut child = Queue::with_capacity(people::max_out(&limits.people));
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people };
    people::step(
        &mut core.people,
        &env,
        people::Event::Restore {
            record: people::Stored::Person {
                number: 1,
                identity: people::Identity {
                    key: people::IdentityKey { provider: 0, subject: Box::new([1]) },
                    login: b"owner".as_slice().into(),
                    name: b"A Person".as_slice().into(),
                },
            },
        },
        &mut child,
    );
    people::step(
        &mut core.people,
        &env,
        people::Event::Restore {
            record: people::Stored::Roles {
                project: 1,
                holdings: Box::new([people::Holding { person: 1, role: people::Role::Observer }]),
            },
        },
        &mut child,
    );
    people::step(
        &mut core.people,
        &env,
        people::Event::Restore {
            record: people::Stored::SignIn {
                number: 9,
                person: 1,
                expires: Wall::from_nanos(Duration::from_secs(60).as_nanos()),
            },
        },
        &mut child,
    );
    people::step(
        &mut core.people,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people },
        people::Event::Restored,
        &mut child,
    );
    for (index, ask) in [
        people::Ask::SetPool { project: 1, person: 1, budget: 1 },
        people::Ask::SetRoles { project: 1, holdings: Box::new([]) },
        people::Ask::MakeService { project: 1, name: b"service".as_slice().into(), role: people::Role::Member },
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            offers::request(&core, &limits, 1, 1, Some(people::Role::Observer), &ask, Wall::EPOCH),
            Err(expected.clone())
        );
        let key = [u8::try_from(index).unwrap(); 16];
        let outcome = people::Outcome::Refused(expected.clone());
        for replay in [false, true] {
            let Requests::Out(out) = step(
                &mut core,
                &Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
                Event::People(people::Event::Ask {
                    models: Box::new([]),
                    reply_to: ReplyTo::new(Token::new(77)),
                    sign_in: 9,
                    key,
                    ask: ask.clone(),
                }),
            );
            let mut replied = false;
            let mut saved = 0;
            for row in &out {
                match row {
                    Request::Held(held) => {
                        if let Held::PeopleReply { reply, .. } = &**held {
                            replied |= *reply == people::Reply::Outcome(outcome.clone());
                        }
                    }
                    Request::Write(Write::Save(Record::People(people::Stored::Answer { outcome: value, .. }))) => {
                        assert_eq!(*value, outcome);
                        saved += 1;
                    }
                    Request::Write(Write::Save(Record::Tasks(_))) => panic!("refused act wrote a task"),
                    Request::ProjectMade { .. }
                    | Request::Write(_)
                    | Request::Ask { .. }
                    | Request::Now(_)
                    | Request::Decided => {}
                }
            }
            assert!(replied, "index {index}, replay {replay}: {out:?}");
            assert_eq!(saved, usize::from(!replay));
            core.reclaim();
        }
    }
}

#[test]
fn a_maintainer_word_uses_the_trusted_entrance_and_keeps_its_actor() {
    let (mut core, limits) = make_tree();
    holding(&mut core, &limits, people::Role::Maintainer);
    assert_eq!(
        offers::admit(&core, &limits, 1, Act::Message, &Subject::Task { task: 2 }, Wall::EPOCH),
        Admission::Allowed
    );
    let event = core
        .person_message(
            &limits,
            PersonMessage {
                request: Token::new(77),
                person: 1,
                project: 1,
                task: 2,
                question: None,
                words: b"maintainer word".as_slice().into(),
                at: Wall::EPOCH,
            },
        )
        .unwrap();
    let mut out = Queue::with_capacity(settings::task_room(&limits));
    tasks::step(&mut core.tasks, &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.tasks }, event, &mut out);
    let mut sent = false;
    for row in &out {
        if let tasks::Request::Sent { .. } = row {
            sent = true;
        }
    }
    assert!(sent, "{out:?}");
    let word = core.tasks.task(2).unwrap().inbox.last().unwrap();
    assert_eq!(word.from, tasks::Party::Person(1));
    assert_eq!(word.words.as_ref(), b"maintainer word");
    assert_eq!(word.kind, tasks::MessageKind::Words);
}

#[test]
fn offered_person_handoffs_and_answers_reach_the_child_and_change_the_offer() {
    let (mut core, limits) = make_tree();
    let mut row = new_row(&core, 5, tasks::Funder::Period { project: 1, period: 1 }, None);
    row.executor = tasks::Executor::Person(tasks::PersonAddress::Role(0));
    send_task(
        &mut core,
        &limits,
        tasks::Event::Make {
            reply_to: ReplyTo::new(Token::new(80)),
            creator: tasks::Party::Person(1),
            batch: Box::new([row]),
        },
    );
    let subject = Subject::PersonTask { task: 5 };
    assert_eq!(offers::admit(&core, &limits, 1, Act::Take, &subject, Wall::EPOCH), Admission::Allowed);
    let event = core
        .person_task(Token::new(81), 1, 1, Some(people::Role::Owner), people::Ask::TakePerson { project: 1, task: 5 })
        .unwrap();
    send_task(&mut core, &limits, event);
    assert_eq!(core.person_tasks.remove(&Token::new(81)), Some(PersonTaskRoute::Take(5)));
    assert_eq!(core.tasks.task(5).unwrap().taken_by, Some(1));
    assert_eq!(
        offers::admit(&core, &limits, 1, Act::Take, &subject, Wall::EPOCH),
        Admission::Refused(people::Refusal::NotWaiting)
    );
    holding(&mut core, &limits, people::Role::Observer);
    assert_eq!(offers::admit(&core, &limits, 1, Act::HandBack, &subject, Wall::EPOCH), Admission::Allowed);
    let event = core
        .person_task(
            Token::new(82),
            1,
            1,
            Some(people::Role::Observer),
            people::Ask::HandBackPerson { project: 1, task: 5 },
        )
        .unwrap();
    send_task(&mut core, &limits, event);
    assert_eq!(core.person_tasks.remove(&Token::new(82)), Some(PersonTaskRoute::HandBack(5)));
    assert_eq!(core.tasks.task(5).unwrap().taken_by, None);
    holding(&mut core, &limits, people::Role::Owner);
    let why = people::Refusal::NotWaiting;
    assert_eq!(offers::admit(&core, &limits, 1, Act::Answer, &subject, Wall::EPOCH), Admission::Refused(why.clone()));
    let answer = people::Ask::AnswerPerson {
        project: 1,
        task: 5,
        revision: core.tasks.task(5).unwrap().revisions.person,
        result: people::PersonResult::Report { words: b"done".as_slice().into() },
    };
    assert_eq!(core.person_task(Token::new(83), 1, 1, Some(people::Role::Owner), answer.clone()), Err(why));
    let event = core
        .person_task(Token::new(84), 1, 1, Some(people::Role::Owner), people::Ask::TakePerson { project: 1, task: 5 })
        .unwrap();
    send_task(&mut core, &limits, event);
    assert_eq!(core.person_tasks.remove(&Token::new(84)), Some(PersonTaskRoute::Take(5)));
    assert_eq!(offers::admit(&core, &limits, 1, Act::Answer, &subject, Wall::EPOCH), Admission::Allowed);
    let event = core.person_task(Token::new(85), 1, 1, Some(people::Role::Owner), answer).unwrap();
    send_task(&mut core, &limits, event);
    assert_eq!(core.person_tasks.remove(&Token::new(85)), Some(PersonTaskRoute::Answer(5)));
    let actor = core.tasks.task(5).unwrap().person_answer.unwrap();
    assert_eq!(actor.by, 1);
    assert_eq!(
        offers::admit(&core, &limits, 1, Act::Answer, &subject, Wall::EPOCH),
        Admission::Refused(people::Refusal::Ended { how: people::Ending::Decided { by: 1, at: actor.at } })
    );
}

#[test]
fn a_named_person_can_answer_without_project_membership() {
    let (mut core, limits) = make_tree();
    let mut row = new_row(&core, 5, tasks::Funder::Period { project: 1, period: 1 }, None);
    row.executor = tasks::Executor::Person(tasks::PersonAddress::Person(1));
    send_task(
        &mut core,
        &limits,
        tasks::Event::Make {
            reply_to: ReplyTo::new(Token::new(86)),
            creator: tasks::Party::Person(1),
            batch: Box::new([row]),
        },
    );
    let mut out = Queue::with_capacity(people::max_out(&limits.people));
    people::step(
        &mut core.people,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people },
        people::Event::Roles { project: 1, holdings: Box::new([]) },
        &mut out,
    );
    assert_eq!(core.people.role(1, 1), None);
    assert_eq!(
        offers::admit(&core, &limits, 1, Act::Answer, &Subject::PersonTask { task: 5 }, Wall::EPOCH),
        Admission::Allowed
    );
    let answer = people::Ask::AnswerPerson {
        project: 1,
        task: 5,
        revision: core.tasks.task(5).unwrap().revisions.person,
        result: people::PersonResult::Report { words: b"done".as_slice().into() },
    };
    let event = core.person_task(Token::new(87), 1, 1, None, answer).unwrap();
    send_task(&mut core, &limits, event);
    assert_eq!(core.person_tasks.remove(&Token::new(87)), Some(PersonTaskRoute::Answer(5)));
    assert_eq!(core.tasks.task(5).unwrap().person_answer.unwrap().by, 1);
}

#[test]
fn task_controls_owner_changes_priorities_and_note_scopes_share_their_judges() {
    let (mut core, limits) = make_tree();
    let mut given = core.authority.role(1, 0).unwrap().authority.clone();
    given.delegation.tasks = 3;
    given.delegation.depth = 2;
    set_role_authority(&mut core.authority, given);
    for (act, ask, number) in [
        (Act::Stop, people::Ask::Stop { project: 1, task: 2 }, 2),
        (Act::Cancel, people::Ask::Cancel { project: 1, task: 2, reason: b"done".as_slice().into(), grace: None }, 2),
        (
            Act::Release,
            people::Ask::Release {
                project: 1,
                task: 3,
                revision: core.tasks.task(3).unwrap().revisions.hold,
                again: None,
            },
            3,
        ),
    ] {
        assert_eq!(
            offers::admit(&core, &limits, 1, act, &Subject::Task { task: number }, Wall::EPOCH),
            Admission::Allowed
        );
        assert_eq!(core.control_admit(&ask, 1, 1, Some(people::Role::Owner), &limits.tasks), Ok(number));
    }
    let why = people::Refusal::Authority {
        lacking: people::Lack::Tasks { left: 3 },
        proposable: Some(people::Accepter::Role(0)),
    };
    assert_eq!(
        offers::admit(&core, &limits, 1, Act::Move, &Subject::Task { task: 2 }, Wall::EPOCH),
        Admission::Refused(why.clone())
    );
    assert_eq!(
        core.move_admit(1, Some(people::Role::Owner), 1, 2, limits.tasks.tree_tasks, limits.tasks.depth),
        Err(why)
    );
    assert_eq!(
        offers::admit(&core, &limits, 1, Act::Reorder, &Subject::Project { project: 1 }, Wall::EPOCH),
        Admission::Allowed
    );
    for act in [Act::Adopt, Act::ChangePolicy, Act::SetRoles, Act::MakeService, Act::SetPool] {
        assert_eq!(
            offers::admit(&core, &limits, 1, act, &Subject::Project { project: 1 }, Wall::EPOCH),
            Admission::Allowed
        );
        assert_eq!(core.role_allowed(1, 1, limits.tasks.tree_tasks), Ok(()));
    }
    let why = people::Refusal::Authority { lacking: people::Lack::Shape, proposable: None };
    for act in [Act::EditNote, Act::DeleteNote] {
        assert_eq!(offers::note(&core, 1, 1, &people::NoteScope::Project, act), Err(why.clone()));
    }
    let mut rules = core.authority.rules().clone();
    rules.ceiling.notes = authority::Scopes(2);
    let mut policy = core.authority.policy(1).unwrap().clone();
    policy.ceiling.notes = authority::Scopes(2);
    policy.roles[0].authority.notes = authority::Scopes(2);
    core.authority = authority::Domain::new(rules, limits.authority).unwrap();
    let mut out = Queue::with_capacity(authority::POLICY_MAX_OUT);
    authority::step(&mut core.authority, authority::Event::Policy { project: 1, policy }, &mut out);
    assert_eq!(out.pop(), Some(authority::PolicyFact::Added { project: 1 }));
    for act in [Act::EditNote, Act::DeleteNote] {
        assert_eq!(offers::note(&core, 1, 1, &people::NoteScope::Project, act), Ok(()));
    }
    holding(&mut core, &limits, people::Role::Observer);
    let why = people::Refusal::Role { held: Some(people::Role::Observer) };
    for act in [Act::EditNote, Act::DeleteNote] {
        assert_eq!(offers::note(&core, 1, 1, &people::NoteScope::Project, act), Err(why.clone()));
    }
}

fn sign_in(core: &mut Core, limits: &Limits) {
    let identity = core.people.identity(1).unwrap().clone();
    let mut out = Queue::with_capacity(people::max_out(&limits.people));
    people::step(
        &mut core.people,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people },
        people::Event::SignedIn {
            held: None,
            reply_to: ReplyTo::new(Token::new(100)),
            person: 1,
            sign_in: 9,
            identity,
            kind: people::Kind::Person,
        },
        &mut out,
    );
    assert_eq!(core.people.person(9, Time::ZERO, Wall::EPOCH), Some(1));
}

fn ask_now(core: &mut Core, limits: &Limits, key: u8, ask: people::Ask) -> people::Outcome {
    let Requests::Out(out) = step(
        core,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: *limits },
        Event::People(people::Event::Ask {
            models: Box::new([]),
            reply_to: ReplyTo::new(Token::new(101)),
            sign_in: 9,
            key: [key; 16],
            ask,
        }),
    );
    for row in &out {
        if let Request::Held(held) = row
            && let Held::PeopleReply { reply: people::Reply::Outcome(outcome), .. } = &**held
        {
            return outcome.clone();
        }
    }
    panic!("one terminal keyed outcome: {out:?}");
}

#[test]
fn offered_proposal_decisions_are_made_at_the_named_holder_and_another_holder_is_refused() {
    for (act, choice, expected) in [
        (Act::Accept, people::ProposalDecision::Accept, people::ProposalChoice::Accepted),
        (
            Act::Reject,
            people::ProposalDecision::Reject { reason: b"no".as_slice().into() },
            people::ProposalChoice::Rejected,
        ),
        (Act::PassUp, people::ProposalDecision::Pass, people::ProposalChoice::Passed),
    ] {
        let (mut core, limits) = make_tree();
        sign_in(&mut core, &limits);
        let mut next = new_row(&core, 5, tasks::Funder::Task(1), None);
        next.authority.delegation.tasks = 1;
        next.authority.delegation.depth = 1;
        send_task(
            &mut core,
            &limits,
            tasks::Event::Propose {
                reply_to: ReplyTo::new(Token::new(102)),
                proposal: tasks::Proposal {
                    number: 10,
                    proposer: 2,
                    project: 1,
                    action: tasks::ProposalAction::Batch(Box::new([next])),
                    reason: b"room".as_slice().into(),
                    as_holder: true,
                    state: tasks::ProposalState::Pending {
                        holder: tasks::ProposalHolder::Person(1),
                        since: Wall::EPOCH,
                    },
                },
            },
        );
        let object = Subject::Proposal { proposer: Proposer::Task(2), number: 10 };
        assert_eq!(offers::admit(&core, &limits, 1, act, &object, Wall::EPOCH), Admission::Allowed);
        assert_eq!(
            offers::admit(&core, &limits, 2, act, &object, Wall::EPOCH),
            Admission::Refused(people::Refusal::NotWaiting)
        );
        assert_eq!(
            ask_now(
                &mut core,
                &limits,
                1,
                people::Ask::DecideProposal {
                    project: 1,
                    proposer: Proposer::Task(2),
                    proposal: 10,
                    decision: choice,
                    running: None
                }
            ),
            people::Outcome::ProposalDecided {
                proposer: Proposer::Task(2),
                proposal: 10,
                by: 1,
                at: Wall::EPOCH,
                choice: expected,
                repeated: false,
            }
        );
    }
}

mod conversations;

#[test]
fn a_move_offer_tracks_requester_changes_after_adoption() {
    let (mut core, limits) = make_tree();
    sign_in(&mut core, &limits);
    let mut row = new_row(&core, 5, tasks::Funder::Task(1), None);
    row.authority.delegation.tasks = 1;
    row.authority.delegation.depth = 1;
    send_task(
        &mut core,
        &limits,
        tasks::Event::Make {
            reply_to: ReplyTo::new(Token::new(120)),
            creator: tasks::Party::Task(1),
            batch: Box::new([row]),
        },
    );
    let object = Subject::Task { task: 5 };
    assert_eq!(offers::admit(&core, &limits, 1, Act::Move, &object, Wall::EPOCH), Admission::Allowed);
    assert_eq!(
        ask_now(
            &mut core,
            &limits,
            1,
            people::Ask::Move { project: 1, task: 5, reason: b"take responsibility".as_slice().into() }
        ),
        people::Outcome::Moved { task: 5 }
    );
    assert_eq!(core.tasks.task(5).unwrap().requester, tasks::Party::Person(1));
    assert!(!core.tasks.takeover_state(5, 1));
    let why = people::Refusal::NotWaiting;
    assert_eq!(offers::admit(&core, &limits, 1, Act::Move, &object, Wall::EPOCH), Admission::Refused(why.clone()));
    assert_eq!(
        core.move_admit(1, Some(people::Role::Owner), 1, 5, limits.tasks.tree_tasks, limits.tasks.depth),
        Err(why)
    );
}

#[test]
fn procedure_goal_bounds_and_admission_keep_exact_executor_and_authority_facts() {
    let (mut core, limits) = core();
    let executor = tasks::Executor::Procedure { connector: 2, code: 1 };
    let Admission::Bounded { within: Bounds::Create { procedures, .. }, .. } = fixtures::creating(&core, &limits, 1, 1)
    else {
        panic!("goal facts")
    };
    assert_eq!(
        procedures.as_slice(),
        &[offers::CreateProcedure { connector: 1, code: 1 }, offers::CreateProcedure { connector: 2, code: 1 },]
    );
    let admitted = crate::acts::bounds::create_start(
        &core,
        1,
        1,
        people::Role::Owner,
        executor,
        100,
        None,
        limits.tasks.tree_tasks,
    )
    .unwrap();
    assert_eq!(admitted.authority.budget.spend, 100);
    assert_eq!(admitted.authority.tools, authority::Tools(0));
    assert_eq!(admitted.authority.grants, core.authority.role(1, 0).unwrap().authority.grants);
    assert_eq!(
        admitted.authority.delegation,
        core.creating_authority(1, 1, people::Role::Owner, limits.tasks.tree_tasks).delegation
    );
    assert_eq!(
        crate::acts::bounds::create_start(
            &core,
            1,
            1,
            people::Role::Owner,
            executor,
            501,
            None,
            limits.tasks.tree_tasks
        )
        .unwrap_err(),
        people::Refusal::Authority {
            lacking: people::Lack::Budget { needs: 501, left: 500 },
            proposable: Some(people::Accepter::Role(0))
        }
    );
    assert_eq!(
        crate::acts::bounds::create_start(
            &core,
            1,
            1,
            people::Role::Owner,
            tasks::Executor::Procedure { connector: 3, code: 1 },
            100,
            None,
            limits.tasks.tree_tasks
        )
        .unwrap_err(),
        people::Refusal::NotOffered
    );
    crate::acts::bounds::create_start(
        &core,
        1,
        1,
        people::Role::Owner,
        tasks::Executor::Procedure { connector: 1, code: u32::MAX },
        100,
        None,
        limits.tasks.tree_tasks,
    )
    .unwrap_err();
    let mut changed = core.authority.policy(1).unwrap().clone();
    changed.roles[0].authority.delegation.kinds = Box::new([authority::Executor::Charter(1)]);
    let mut out = Queue::with_capacity(authority::POLICY_MAX_OUT);
    authority::step(&mut core.authority, authority::Event::Policy { project: 1, policy: changed }, &mut out);
    assert_eq!(out.pop(), Some(authority::PolicyFact::Changed { project: 1 }));
    crate::acts::bounds::create_start(&core, 1, 1, people::Role::Owner, executor, 100, None, limits.tasks.tree_tasks)
        .unwrap_err();
    let Admission::Bounded { within: Bounds::Create { procedures, .. }, .. } = fixtures::creating(&core, &limits, 1, 1)
    else {
        panic!("changed goal facts")
    };
    assert!(procedures.is_empty(), "current authority removes every direct procedure choice");
}

#[test]
fn procedure_goal_bounds_refuse_an_incomplete_executor_list() {
    let (core, mut limits) = core();
    limits.documents.lines = 1;
    assert_eq!(
        fixtures::creating(&core, &limits, 1, 1),
        Admission::Refused(people::Refusal::Limit { which: people::LimitKind::Authority, bound: None })
    );
    let executor = tasks::Executor::Procedure { connector: 2, code: 1 };
    assert!(
        crate::acts::bounds::create_start(
            &core,
            1,
            1,
            people::Role::Owner,
            executor,
            100,
            None,
            limits.tasks.tree_tasks
        )
        .is_ok(),
        "document receiving bound does not alter owner admission"
    );
}

fn keyed_proposal(core: &mut Core, limits: &Limits, key: u8, proposer: Proposer) -> Queue<Request> {
    let Requests::Out(out) = step(
        core,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: *limits },
        Event::People(people::Event::Ask {
            models: Box::new([]),
            reply_to: ReplyTo::new(Token::new(101)),
            sign_in: 9,
            key: [key; 16],
            ask: people::Ask::DecideProposal {
                project: 1,
                proposer,
                proposal: 10,
                decision: people::ProposalDecision::Reject { reason: b"no".as_slice().into() },
                running: None,
            },
        }),
    );
    out
}

fn proposal_answer(out: &Queue<Request>) -> people::Outcome {
    for row in out {
        if let Request::Held(held) = row
            && let Held::PeopleReply { reply: people::Reply::Outcome(outcome), .. } = &**held
        {
            return outcome.clone();
        }
    }
    panic!("one terminal keyed outcome: {out:?}");
}

#[test]
fn a_task_and_a_party_with_identical_proposal_numbers_are_decided_separately() {
    for party_first in [false, true] {
        let (mut core, limits) = make_tree();
        sign_in(&mut core, &limits);
        let next = new_row(&core, 5, tasks::Funder::Task(1), None);
        send_task(
            &mut core,
            &limits,
            tasks::Event::Propose {
                reply_to: ReplyTo::new(Token::new(102)),
                proposal: tasks::Proposal {
                    number: 10,
                    proposer: 1,
                    project: 1,
                    action: tasks::ProposalAction::Batch(Box::new([next])),
                    reason: b"task work".as_slice().into(),
                    as_holder: true,
                    state: tasks::ProposalState::Pending {
                        holder: tasks::ProposalHolder::Person(1),
                        since: Wall::EPOCH,
                    },
                },
            },
        );
        let goal = new_row(&core, 5, tasks::Funder::Pool { project: 1, person: 1, period: 1 }, Some(4));
        send_task(
            &mut core,
            &limits,
            tasks::Event::ProposePerson {
                reply_to: ReplyTo::new(Token::new(103)),
                proposal: tasks::PersonProposal {
                    number: 10,
                    proposer: 1,
                    project: 1,
                    action: tasks::PersonAction::Batch(Box::new([goal])),
                    reason: Box::from(b"proposed batch".as_slice()),
                    state: tasks::PersonProposalState::Pending { since: Wall::EPOCH },
                },
            },
        );
        assert!(core.tasks.proposal(1, 10).is_some(), "task proposal admitted");
        assert!(core.tasks.person_proposal(1, 10).is_some(), "party proposal admitted");
        let task = Proposer::Task(1);
        let party = Proposer::Party(1);
        let order = if party_first { [party, task] } else { [task, party] };
        let mut archives = alloc::collections::BTreeMap::new();
        for (position, proposer) in order.into_iter().enumerate() {
            let out = keyed_proposal(&mut core, &limits, u8::try_from(position + 1).unwrap(), proposer);
            assert_eq!(
                proposal_answer(&out),
                people::Outcome::ProposalDecided {
                    proposer,
                    proposal: 10,
                    by: 1,
                    at: Wall::EPOCH,
                    choice: people::ProposalChoice::Rejected,
                    repeated: false,
                }
            );
            for request in &out {
                if let Request::Write(Write::Save(Record::Core(CoreRecord::ProposalDecision(row)))) = request {
                    let key = CoreKey::ProposalDecision { proposer: row.proposer, proposal: row.proposal };
                    assert!(archives.insert(key, *row).is_none(), "makers retain separate immutable histories");
                }
            }
            if position == 0 {
                match proposer {
                    Proposer::Task(_) => assert_eq!(
                        core.tasks.person_proposal(1, 10).unwrap().state,
                        tasks::PersonProposalState::Pending { since: Wall::EPOCH }
                    ),
                    Proposer::Party(_) => assert_eq!(
                        core.tasks.proposal(1, 10).unwrap().state,
                        tasks::ProposalState::Pending { holder: tasks::ProposalHolder::Person(1), since: Wall::EPOCH }
                    ),
                }
            }
        }
        assert_eq!(archives.len(), 2);
        assert_eq!(
            archives[&CoreKey::ProposalDecision { proposer: Proposer::Task(1), proposal: 10 }].choice,
            people::ProposalChoice::Rejected
        );
        assert_eq!(
            archives[&CoreKey::ProposalDecision { proposer: Proposer::Party(1), proposal: 10 }].choice,
            people::ProposalChoice::Rejected
        );
    }
}

#[test]
fn a_historical_proposal_decision_requires_the_exact_maker() {
    for proposer in [Proposer::Task(1), Proposer::Party(1)] {
        for other_store in [false, true] {
            let (mut core, limits) = make_tree();
            sign_in(&mut core, &limits);
            // Allocate the archived decider and proposal through their real deployment families.
            assert_eq!(fresh(&mut core.counters, Family::Person), Some(1));
            for _ in 0_u8..10 {
                fresh(&mut core.counters, Family::Message).unwrap();
            }
            let out = keyed_proposal(&mut core, &limits, 1, proposer);
            let mut named_request = None;
            for output in &out {
                if let Request::Now(now) = output
                    && let Now::HistoricalProposal { request, person, project, proposer: named, proposal } = &**now
                {
                    assert_eq!((*person, *project, *named, *proposal), (1, 1, proposer, 10));
                    assert!(named_request.replace(*request).is_none(), "one exact historical load");
                }
            }
            let request = named_request.expect("only the named absent store falls back to history");
            let maker = match proposer {
                Proposer::Task(number) => {
                    if other_store {
                        Proposer::Party(number)
                    } else {
                        Proposer::Task(number)
                    }
                }
                Proposer::Party(number) => {
                    if other_store {
                        Proposer::Task(number)
                    } else {
                        Proposer::Party(number)
                    }
                }
            };
            let row = ProposalDecisionRecord {
                project: 1,
                proposer: maker,
                proposal: 10,
                kind: tasks::ProposalKind::Batch,
                by: 1,
                at: Wall::from_nanos(2_000_000_000),
                choice: people::ProposalChoice::Rejected,
                created: None,
            };
            let Requests::Out(out) = step(
                &mut core,
                &Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
                Event::HistoricalProposal {
                    request,
                    person: 1,
                    project: 1,
                    proposer,
                    proposal: 10,
                    row: Some(row),
                    busy: false,
                },
            );
            assert_eq!(
                proposal_answer(&out),
                if other_store {
                    people::Outcome::Refused(people::Refusal::Unknown)
                } else {
                    people::Outcome::ProposalDecided {
                        proposer,
                        proposal: 10,
                        by: 1,
                        at: row.at,
                        choice: row.choice,
                        repeated: true,
                    }
                }
            );
        }
    }
}

mod kind_order;

mod escalation;

mod proposals;

mod tracking;

mod decision_effects;

#[test]
fn a_procedure_repeat_is_refused_before_authority_and_its_uncertain_effect_keeps_its_choice() {
    let (mut core, limits) = tree_core();
    let connector = core.connectors[0];
    let mut new = new_row(&core, 1, tasks::Funder::Period { project: 1, period: 1 }, None);
    new.executor = tasks::Executor::Procedure { connector, code: u32::MAX };
    send_task(
        &mut core,
        &limits,
        tasks::Event::Make {
            reply_to: ReplyTo::new(Token::new(120)),
            creator: tasks::Party::Person(1),
            batch: Box::new([new]),
        },
    );
    send_task(&mut core, &limits, tasks::Event::Hold { task: 1, why: tasks::Hold::Procedure });
    let mut procedure = core.tasks.task(1).expect("procedure made").clone();
    let ask = people::Ask::Release { project: 1, task: 1, revision: procedure.revisions.hold, again: Some(true) };
    let refusal = people::Refusal::NotOffered;
    assert_eq!(core.control_admit(&ask, 1, 1, None, &limits.tasks), Err(refusal.clone()));
    assert_eq!(offers::request(&core, &limits, 1, 1, None, &ask, Wall::EPOCH), Err(refusal));
    assert_eq!(revisions::release_run(&procedure, None), Ok(()));
    let tasks::Phase::Held { why, .. } = &mut procedure.phase else { panic!("held fixture") };
    *why = tasks::Hold::Uncertain { entry: 1 };
    assert_eq!(revisions::release_run(&procedure, Some(true)), Ok(()));
    assert_eq!(revisions::release_run(&procedure, Some(false)), Ok(()));
    procedure.executor = tasks::Executor::Agent { charter: 1 };
    assert_eq!(revisions::release_run(&procedure, Some(true)), Ok(()));
}
