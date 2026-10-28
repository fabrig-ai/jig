//! Compound and grace stories at durable task and party boundaries.
use super::{World, ask, check, parent};
use crate::Neighbours;
use jig_conformance::{
    referee::Violation,
    scenarios::{Action, Scenario},
};
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{Duration, List, ReplyTo, Token};

const KEY: u8 = 120;
const GRACE: Duration = Duration::from_secs(2);
const DELEGATE_REPLY: u64 = 8120;
const DELEGATE_COMPLETION: u32 = 1;
const ADOPTION_WAIT: u64 = 6_000_000_000;
const RETRY_WAIT: u64 = 1_000_000_000;
const REASON: &[u8] = b"finish within grace";
#[derive(Clone, Debug)]
pub(crate) struct State {
    finishes: bool,
    pub(super) ready: u64,
    until: u64,
    answer: Option<people::Outcome>,
}
impl State {
    pub(crate) fn new(seed: u64) -> Self {
        Self { finishes: seed.is_multiple_of(2), ready: 0, until: 0, answer: None }
    }
}
fn row(world: &World, task: u64) -> &tasks::TaskRecord {
    [tasks::Key::Live(task), tasks::Key::Ended(task)]
        .into_iter()
        .find_map(|key| match world.store.rows.get(&root::Key::Core(core::Key::Tasks(key))) {
            Some(root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row) | tasks::Stored::Ended(row)))) => {
                Some(row)
            }
            Some(root::Record::Core(_) | root::Record::Connector { .. }) | None => None,
        })
        .expect("story task retained")
}
fn delegate(world: &mut World) {
    let (task, attempt) = parent(world);
    let mut authority = row(world, task).authority.clone();
    authority.budget.spend = 10;
    authority.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
    let batch = [b"moved delegate".as_slice(), b"cancelled delegate".as_slice()].map(|words| core::Delegate {
        deadline: jig_domain_tasks::Deadline::Named,
        label: 0,
        flags: jig_domain_tasks::Flags::PLAIN,
        tracked: None,
        budget: jig_domain_people::Budget::Named,
        executor: tasks::Executor::Agent { charter: 1 },
        spec: tasks::Spec { asks: false, words: words.into(), parameters: Box::new([]), inputs: Box::new([]) },
        contract: tasks::Contract::Report { words: 128 },
        authority: authority.clone(),
        llm: None,
        symbolic_grants: Box::new([]),
        dependencies: Box::new([]),
        wake: tasks::WakePolicy::DEFAULT,
    });
    world.send(root::Event::Core(core::Event::DelegateValidated {
        to: ReplyTo::new(Token::new(DELEGATE_REPLY)),
        key: core::CallKey { task, attempt, completion: DELEGATE_COMPLETION, position: 0 },
        batch: Box::new(batch),
        stubs: Box::new([]),
    }));
}
fn compound(world: &mut World) {
    let mut operations = List::with_capacity(3);
    for operation in [
        people::Operation::Move { task: world.peers.children[0], reason: b"continue".as_slice().into() },
        people::Operation::Cancel { task: world.peers.children[1], reason: REASON.into(), grace: None },
        people::Operation::Cancel { task: parent(world).0, reason: REASON.into(), grace: Some(GRACE) },
    ] {
        operations.push(operation).expect("three compound operations");
    }
    world.peers.routes.push(jig_conformance::referee::Observed::CompoundAsked { tasks: world.peers.children.clone() });
    ask(world, KEY, people::Ask::Compound { project: 1, operations });
}
pub(super) fn prepare(world: &mut World, phase: &mut u8) -> Result<bool, Violation> {
    if *phase < 3 {
        delegate(world);
        *phase = 3;
        return Ok(true);
    }
    let core::CallPart::Delegated(children) =
        super::named_part(world, DELEGATE_COMPLETION).expect("delegation answered")
    else {
        panic!("compound delegates refused: {:?}", super::named_part(world, DELEGATE_COMPLETION))
    };
    world.peers.children = children.iter().map(|member| member.task).collect();
    let running = world.peers.children.iter().all(|&child| {
        matches!(row(world, child).phase, tasks::Phase::Active(tasks::Active::Running { attempt })
            if world.peers.assignments.contains(&(child, attempt)))
    });
    if !running && *phase < 6 {
        world.advance(if *phase == 3 { ADOPTION_WAIT } else { RETRY_WAIT });
        *phase += 1;
        return Ok(true);
    }
    check(
        running,
        &format!(
            "compound setup did not restore two live delegates: {:?}, assignments {:?}",
            world.peers.children.iter().map(|&task| &row(world, task).phase).collect::<Vec<_>>(),
            world.peers.assignments
        ),
    )?;
    Ok(false)
}
pub(super) fn advance(world: &mut World, action: Action, phase: &mut u8) -> Result<bool, Violation> {
    if matches!(action, Action::Compound | Action::CompoundAgain) {
        compound(world);
    } else if action == Action::CancelWithGrace {
        world.peers.lifecycle.until = world.clock().now + GRACE.as_nanos();
        ask(
            world,
            KEY,
            people::Ask::Cancel { project: 1, task: parent(world).0, reason: REASON.into(), grace: Some(GRACE) },
        );
    } else {
        if *phase == 0 {
            check(
                row(world, parent(world).0).grace.is_some() && world.clock().now < world.peers.lifecycle.until,
                "task did not retain its grace through both inside-grace restarts",
            )?;
        }
        if world.peers.lifecycle.finishes && *phase < 2 && !super::ended(world, parent(world).0) {
            super::message(world, parent(world).0);
            *phase += 1;
            return Ok(true);
        }
    }
    Ok(false)
}
pub(crate) fn delivered(peers: &mut Neighbours, delivery: &root::Delivery) {
    if let root::Delivery::Core(core::Held::PeopleReply { to, reply: people::Reply::Outcome(outcome) }) = delivery
        && *to == ReplyTo::new(Token::new(super::PARTY_REPLY_BASE + u64::from(KEY)))
    {
        if let Some(previous) = &peers.lifecycle.answer {
            assert_eq!(outcome, previous, "a compound retry must repeat its keyed answer");
        }
        peers.lifecycle.answer = Some(outcome.clone());
        if peers.scenario == Some(Scenario::Compound) {
            peers.routes.push(jig_conformance::referee::Observed::Compound {
                tasks: peers.children.clone(),
                made: matches!(outcome, people::Outcome::Compound { .. }),
            });
        }
    }
}
pub(super) fn check_ending(world: &World, scenario: Scenario) -> Result<(), Violation> {
    check(
        world.outcome().commits - world.peers.lifecycle.ready <= 8,
        &format!("lifecycle story exceeds eight commits after Ready: {:?}", world.outcome()),
    )?;
    let task = row(world, parent(world).0);
    if scenario == Scenario::Compound {
        check(row(world, world.peers.children[0]).requester == tasks::Party::Person(1), "compound move lost")?;
        check(
            matches!(row(world, world.peers.children[1]).phase, tasks::Phase::Ended(tasks::Ending::Cancelled { .. })),
            "compound delegate not cancelled",
        )?;
        check(
            matches!(task.phase, tasks::Phase::Ended(tasks::Ending::Cancelled { .. })),
            &format!("compound chat not cancelled: {:?}", task.phase),
        )?;
        check(
            matches!(world.peers.lifecycle.answer, Some(people::Outcome::Compound { .. })),
            "party has no made compound answer",
        )?;
        let key = people::RequestKey { person: 1, key: [KEY; 16] };
        check(
            world.store.rows.contains_key(&root::Key::Core(core::Key::People(people::Key::Answer(key)))),
            "party has no durable compound answer",
        )
    } else if world.peers.lifecycle.finishes {
        check(
            matches!(task.phase, tasks::Phase::Ended(tasks::Ending::Done { .. })),
            "task failed to finish inside grace",
        )
    } else {
        check(
            matches!(&task.phase, tasks::Phase::Ended(tasks::Ending::Cancelled { reason, .. }) if reason.as_ref() == REASON)
                && task.ended_at.is_some_and(|at| at.as_nanos() >= world.peers.lifecycle.until),
            "grace cancelled before its end or lost its reason",
        )
    }
}
