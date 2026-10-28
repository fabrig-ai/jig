//! R12's delegate, unread person words, repeated releases and explicit LLM amendment.
use jig_core_world::{
    effects::World,
    fixtures::{ask_as, delegate, live_row, member, person_unchanged},
};
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{Duration, ReplyTo, Token};

const SEED: u64 = 1712;
const PROJECT: u32 = 1;
const OWNER: u64 = 1;
const CHANNEL: Token = Token::new(7);
const WORDS: &[u8] = b"please complete the task";
const MESSAGE: u8 = 40;
const FIRST_RELEASE: u8 = 41;
const SECOND_RELEASE: u8 = 42;
const AMENDMENT: u8 = 43;
const FAILED_RUNS: usize = 2; // The shared fixture permits one invalid retry.
const THIRD_HOLD: u64 = 3; // Initial hold, then one after each release.

fn delegated() -> (World, u64) {
    let (configuration, limits) = jig_core_world::faults::plan_fixture(SEED);
    let mut world = World::configured(SEED, false, configuration, limits);
    let parent = world.assignments[0];
    let child = member(&world, tasks::Executor::Agent { charter: 1 }, b"restart proof");
    let child = delegate(&mut world, parent, 1, child);
    (world, child)
}

fn hold(world: &mut World, task: u64) {
    for _ in 0..FAILED_RUNS {
        let attempt = live_row(world, task).attempt;
        world.send(root::Event::Answer {
            read: None,
            refused: Box::new([]),
            channel: CHANNEL,
            task,
            attempt,
            cumulative: 0,
            end: tasks::End::Failed(tasks::Class::Invalid),
        });
        world.advance(Duration::from_secs(2));
    }
}

fn ask(world: &mut World, key: u8, ask: people::Ask) {
    ask_as(world, world.sign_in.expect("owner session"), key, ask);
}

fn release(world: &mut World, task: u64, key: u8) {
    let revision = live_row(world, task).revisions.hold;
    ask(world, key, people::Ask::Release { project: PROJECT, task, revision, again: None });
}

fn amend(world: &mut World, task: u64) {
    let revision = live_row(world, task).revisions.spec;
    let mut amendment = person_unchanged(b"explicit model and effort");
    amendment.llm = Some(people::LlmChoice {
        account: None,
        main: people::ModelEffort { model: 0, effort: Some(1) }, // scripted's high effort.
        subagents: Box::new([]),
    });
    ask(world, AMENDMENT, people::Ask::Amend { project: PROJECT, task, revision, amendment, release: None });
}

fn twice_released(words: Option<&[u8]>) -> (World, u64) {
    let (mut world, task) = delegated();
    hold(&mut world, task);
    let tasks::Party::Task(parent) = live_row(&world, task).requester else { panic!("delegate requester") };
    hold(&mut world, parent);
    let retention = jig_core_world::faults::plan_fixture(SEED).1.core.people.request_retention;
    world.advance(retention.saturating_add(Duration::from_nanos(1)));
    let identity = world.domain.core().people.identity(OWNER).expect("owner").clone();
    world.send(root::Event::Core(core::Event::SignIn { held: None, reply_to: ReplyTo::new(Token::new(44)), identity }));
    if let Some(words) = words {
        ask(&mut world, MESSAGE, people::Ask::Say { project: PROJECT, task, words: words.into() });
    }
    release(&mut world, task, FIRST_RELEASE);
    hold(&mut world, task);
    release(&mut world, task, SECOND_RELEASE);
    hold(&mut world, task);
    (world, task)
}

fn cold(mut world: World) {
    world.restart_with(jig_core_world::faults::plan_fixture(SEED).0);
    assert!(world.domain.ready(), "cold restore refused: {:?}", world.domain.restore_failure());
}

#[test]
fn messaged_released_amended_delegate_survives_cold_restart() {
    let (mut world, task) = twice_released(Some(WORDS));
    amend(&mut world, task);
    cold(world);
}

#[test]
fn person_messaged_delegate_survives_cold_restart() {
    let (mut world, task) = delegated();
    ask(&mut world, MESSAGE, people::Ask::Say { project: PROJECT, task, words: WORDS.into() });
    cold(world);
}

#[test]
fn unmessaged_released_amended_delegate_survives_cold_restart() {
    let (mut world, task) = twice_released(None);
    amend(&mut world, task);
    cold(world);
}

#[test]
fn unmessaged_held_delegate_survives_cold_restart() {
    let (mut world, task) = delegated();
    hold(&mut world, task);
    cold(world);
}

#[test]
fn release_and_amend_fixture_commits_each_requested_operation() {
    let (mut world, task) = twice_released(Some(WORDS));
    amend(&mut world, task);
    let row = live_row(&world, task);
    assert_eq!(row.revisions.hold, THIRD_HOLD);
    assert!(matches!(row.phase, tasks::Phase::Held { why: tasks::Hold::Failures(tasks::Class::Invalid), .. }));
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Amended { task, released: false }))
    );
    assert!(
        row.inbox.iter().any(|word| word.kind == tasks::MessageKind::Words && word.from == tasks::Party::Person(OWNER))
    );
    assert!(world.store.rows.contains_key(&root::Key::Core(core::Key::People(people::Key::Person(OWNER)))));
}
