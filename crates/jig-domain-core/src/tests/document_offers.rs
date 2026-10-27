//! Delivered fields use the same owner judgments and whole receiving payloads.
use super::*;
use crate::acts::{Act, Admission};

fn find(offers: &List<crate::acts::Offered>, act: Act) -> Option<&Admission> {
    for offer in offers {
        if offer.act == act {
            return Some(&offer.admission);
        }
    }
    None
}

#[test]
fn document_offers_fit_the_object_and_preserve_actor_specific_refusals() {
    // Project subject actions, including Rename (people.md, 5.1).
    const PROJECT_ACTS: u32 = 7;
    let (mut core, mut limits) = make_tree();
    limits.authority.roles = 2;
    let mut policy = core.authority.policy(1).unwrap().clone();
    let mut observer = policy.roles[0].clone();
    observer.number = people::Role::Observer.number();
    observer.requests = authority::Requests(0);
    observer.decides = authority::Proposals(0);
    policy.roles = Box::new([policy.roles[0].clone(), observer]);
    core.authority = authority::Domain::new(core.authority.rules().clone(), limits.authority).unwrap();
    let mut facts = Queue::with_capacity(authority::POLICY_MAX_OUT);
    authority::step(&mut core.authority, authority::Event::Policy { project: 1, policy }, &mut facts);
    let Document::Task(value) = make(&core, &limits, 1, &Subject::Task { task: 2 }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!("task")
    };
    assert_eq!(find(&value.acts, Act::Message), Some(&Admission::Allowed));
    assert_eq!(find(&value.acts, Act::Stop), Some(&Admission::Allowed));
    assert_eq!(find(&value.acts, Act::Answer), None);
    assert_eq!(find(&value.acts, Act::Take), None);
    let mut out = Queue::with_capacity(people::max_out(&limits.people));
    people::step(
        &mut core.people,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people },
        people::Event::Roles {
            project: 1,
            holdings: Box::new([people::Holding { person: 1, role: people::Role::Observer }]),
        },
        &mut out,
    );
    let Document::Task(value) = make(&core, &limits, 1, &Subject::Task { task: 2 }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!("task")
    };
    assert_eq!(
        find(&value.acts, Act::Message),
        Some(&Admission::Refused(people::Refusal::Role { held: Some(people::Role::Observer) }))
    );
    let Document::Project(value) =
        make(&core, &limits, 1, &Subject::Project { project: 1 }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!("project")
    };
    assert_eq!(value.acts.len(), PROJECT_ACTS);
    assert!(find(&value.acts, Act::Rename).is_some());
    for offer in &value.acts {
        assert_eq!(offer.admission, Admission::Refused(people::Refusal::Role { held: Some(people::Role::Observer) }));
    }
}

#[test]
fn complete_policy_recipients_refuse_a_shortened_receiving_form() {
    let (mut core, mut limits) = make_tree();
    limits.authority.roles = 2;
    let mut policy = core.authority.policy(1).unwrap().clone();
    let mut second = policy.roles[0].clone();
    second.number = 4;
    policy.roles = Box::new([policy.roles[0].clone(), second]);
    core.authority = authority::Domain::new(core.authority.rules().clone(), limits.authority).unwrap();
    let mut out = Queue::with_capacity(authority::POLICY_MAX_OUT);
    authority::step(&mut core.authority, authority::Event::Policy { project: 1, policy }, &mut out);
    assert_eq!(out.pop(), Some(authority::PolicyFact::Added { project: 1 }));
    limits.documents.lines = 1;
    assert_eq!(make(&core, &limits, 1, &Subject::Task { task: 2 }, &window(), Wall::EPOCH), Err(Missing::TooLarge));
    let request = people::Ask::Amend {
        project: 1,
        task: 2,
        revision: core.tasks.task(2).unwrap().revisions.spec,
        amendment: fixtures::person_unchanged(b""),
        release: None,
    };
    assert_eq!(crate::acts::request(&core, &limits, 1, 1, Some(people::Role::Owner), &request, Wall::EPOCH), Ok(()));
}

#[test]
fn cancellation_during_closing_and_release_of_a_held_closing_task_keep_the_owner_admission() {
    let (mut core, limits) = make_tree();
    let mut funders = List::with_capacity(core.tasks.funding_records().len());
    for (_, row) in core.tasks.funding_records() {
        funders.push(*row).unwrap();
    }
    let mut rows = List::with_capacity(4);
    for number in 1..=4 {
        rows.push(core.tasks.task(number).unwrap().clone()).unwrap();
    }
    core.tasks = fixtures::fresh_tasks(&core, &limits);
    for row in &funders {
        send_task(&mut core, &limits, tasks::Event::Restore { record: tasks::Stored::Ledger(*row) });
    }
    for mut row in rows.into_boxed() {
        if row.number == 3 {
            row.escalation = tasks::Escalation::Unheld { revision: row.revisions.hold };
            row.phase = tasks::Phase::Closing(tasks::Closing {
                stage: tasks::Stage::Effects,
                ending: tasks::Ending::Done(tasks::TaskResult::Report { words: Box::new([]) }),
            });
        }
        send_task(&mut core, &limits, tasks::Event::Restore { record: tasks::Stored::Live(Box::new(row)) });
    }
    send_task(&mut core, &limits, tasks::Event::Check);
    send_task(&mut core, &limits, tasks::Event::Restored);
    let Document::Task(value) = make(&core, &limits, 1, &Subject::Task { task: 3 }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!("closing task")
    };
    assert_eq!(find(&value.acts, Act::Cancel), Some(&Admission::Allowed));
    assert_eq!(find(&value.acts, Act::Amend), None);
    assert_eq!(
        crate::acts::request(
            &core,
            &limits,
            1,
            1,
            Some(people::Role::Owner),
            &people::Ask::Cancel { project: 1, task: 3, reason: b"cancel closing".as_slice().into(), grace: None },
            Wall::EPOCH
        ),
        Ok(())
    );
    send_task(
        &mut core,
        &limits,
        tasks::Event::Control {
            reply_to: ReplyTo::new(Token::new(94)),
            by: tasks::Party::Person(1),
            task: 3,
            control: tasks::Control::Cancel { reason: b"cancel closing".as_slice().into(), grace: None, message: None },
        },
    );
    let tasks::Phase::Closing(closing) = &core.tasks.task(3).unwrap().phase else { panic!("closing cancellation") };
    if let tasks::Ending::Cancelled { by: tasks::Party::Person(1), .. } = closing.ending {
    } else {
        panic!("cancelled by owner");
    }
    send_task(&mut core, &limits, placed(3, 1, 1));
    let Document::Task(value) = make(&core, &limits, 1, &Subject::Task { task: 3 }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!("held closing task")
    };
    assert_eq!(find(&value.acts, Act::Release), Some(&Admission::Allowed));
    assert_eq!(find(&value.acts, Act::Amend), None);
    assert_eq!(
        crate::acts::request(
            &core,
            &limits,
            1,
            1,
            Some(people::Role::Owner),
            &people::Ask::Release {
                project: 1,
                task: 3,
                revision: core.tasks.task(3).unwrap().revisions.hold,
                again: None
            },
            Wall::EPOCH
        ),
        Ok(())
    );
    send_task(
        &mut core,
        &limits,
        tasks::Event::Control {
            reply_to: ReplyTo::new(Token::new(95)),
            by: tasks::Party::Person(1),
            task: 3,
            control: tasks::Control::Release { again: None, message: None },
        },
    );
    if let tasks::Phase::Closing(_) = core.tasks.task(3).unwrap().phase {
    } else {
        panic!("released closing");
    }
}

const PROJECT: u32 = 1;
const PERSON: u64 = 1;
const TASK: u64 = 2;
const CONNECTOR: u16 = 1;
const KINDS: [u16; 3] = [1, 2, 3];
const GRANTS: u32 = 3;
fn full_grants() -> (Core, Limits, tasks::TaskRecord) {
    let (mut core, mut limits) = make_tree();
    limits.authority.grants = GRANTS;
    limits.tasks.authority_grants = GRANTS;
    limits.documents.grants = GRANTS;
    limits.documents.lines = 2;
    let grants: Box<[_]> = KINDS
        .map(|kind| authority::Grant {
            connector: CONNECTOR,
            kind,
            pattern: authority::Pattern { segments: Box::new([]), last: authority::Last::Open(Box::new([])) },
        })
        .into();
    let mut rules = core.authority.rules().clone();
    rules.ceiling.grants = grants.clone();
    let mut policy = core.authority.policy(PROJECT).unwrap().clone();
    policy.ceiling.grants = grants.clone();
    policy.roles[0].authority.grants = grants;
    core.authority = authority::Domain::new(rules, limits.authority).unwrap();
    let mut out = Queue::with_capacity(authority::POLICY_MAX_OUT);
    authority::step(&mut core.authority, authority::Event::Policy { project: PROJECT, policy }, &mut out);
    assert_eq!(out.pop(), Some(authority::PolicyFact::Added { project: PROJECT }));
    let mut row = core.tasks.task(TASK).unwrap().clone();
    row.authority.grants = KINDS
        .map(|kind| tasks::Grant {
            connector: CONNECTOR,
            kind,
            pattern: tasks::Pattern { segments: Box::new([]), last: tasks::Last::Open(Box::new([])) },
        })
        .into();
    (core, limits, row)
}
#[test]
fn a_task_and_its_amend_offer_hold_the_roles_grants_whole() {
    let (core, limits, row) = full_grants();
    let value = task_from_row(&core, &limits, PERSON, &row, Wall::EPOCH).unwrap();
    let Some(Admission::Bounded { within: crate::acts::Bounds::Amend { authority, grants, .. }, .. }) =
        find(&value.acts, Act::Amend)
    else {
        panic!("whole bounded amendment")
    };
    assert_eq!(value.authority.grants.len(), limits.authority.grants);
    assert_eq!(authority.grants.len(), GRANTS);
    assert_eq!(grants.len(), GRANTS);
    for (index, kind) in KINDS.into_iter().enumerate() {
        let i = u32::try_from(index).unwrap();
        let expected = GrantLine {
            connector: CONNECTOR,
            kind,
            segments: List::with_capacity(0),
            last: PatternEnd::Open(Box::new([])),
        };
        assert_eq!(value.authority.grants.get(i), Some(&expected), "task grant {kind}");
        assert_eq!(authority.grants.get(i), Some(&expected), "role grant {kind}");
        assert_eq!(grants.get(i), Some(&expected), "listed grant {kind}");
    }
}
