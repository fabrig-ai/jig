//! Independent scripted parties close a chat across disposal and restart boundaries.
use crate::{
    effects::World,
    peers::{Peers, fixture},
};
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_fake_parties::{self as parties, Act, Ask, DelegateChoice, Keep, Party, Role, Task};
use jig_fake_workers::{Script, Worker};
use jig_test_domain as root;
use skein_lib::{Duration, ReplyTo, Token};

fn acts(world: &mut World, key: u8, ask: Ask) {
    world.peers.as_mut().expect("close story fixture").parties[0]
        .extend(Box::new([Act::Request { key: [key; 16], ask }]));
    world.drain();
}

fn live(world: &World, number: u64) -> &tasks::TaskRecord {
    let Some(root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row)))) =
        world.store.rows.get(&root::Key::Core(core::Key::Tasks(tasks::Key::Live(number))))
    else {
        panic!("task {number} is live")
    };
    row
}

fn fixture_with_delegates() -> (World, root::Config, u64, [u64; 2]) {
    let (configuration, mut limits) = fixture(7074, 0);
    limits.core.fleet.slots = 3;
    limits.core.tasks.inbox_messages = 7;
    limits.core.tasks.inbox_bytes = 512;
    limits.core.tasks.contract_choices = 1;
    limits.core.people.inbox_recipients = 24;
    crate::peers::journal(&mut limits);
    let mut world = World::scripted(
        fixture(7074, 0).0,
        limits,
        Peers::new(
            vec![Worker::new(
                7,
                3,
                vec![Box::new([Script::Wait, Script::Wait]), Box::new([Script::Wait]), Box::new([Script::Wait])],
            )],
            vec![Party::new(
                1,
                Role::Owner,
                70_000,
                Box::new([Act::SignIn { provider: 0, subject: Box::from(&b"close"[..]) }]),
            )],
        ),
    );
    let person = world.peers.as_ref().expect("close story fixture").parties[0].person.expect("close story fixture");
    world.send(root::Event::Core(core::Event::People(people::Event::Roles {
        project: 1,
        holdings: Box::new([people::Holding { person, role: people::Role::Owner }]),
    })));
    acts(&mut world, 1, Ask::Chat { words: Box::from(&b"chat with delegates"[..]) });
    let run = world.assignments[0];
    let mut authority = world.assigned.as_ref().expect("close story fixture").run.authority.clone();
    authority.budget.spend = 10;
    authority.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
    let delegate = core::Delegate {
        deadline: jig_domain_tasks::Deadline::Named,
        label: 0,
        flags: jig_domain_tasks::Flags::PLAIN,
        tracked: None,
        budget: jig_domain_people::Budget::Named,
        executor: tasks::Executor::Agent { charter: 1 },
        spec: tasks::Spec {
            asks: false,
            words: Box::from(&b"delegate"[..]),
            parameters: Box::new([]),
            inputs: Box::new([]),
        },
        contract: tasks::Contract::Report { words: 128 },
        authority,
        llm: None,
        symbolic_grants: Box::new([]),
        dependencies: Box::new([]),
        wake: tasks::WakePolicy::DEFAULT,
    };
    let mut delegates = [0; 2];
    for (index, slot) in delegates.iter_mut().enumerate() {
        world.send(root::Event::Core(core::Event::DelegateValidated {
            to: ReplyTo::new(Token::new(70_500 + u64::try_from(index).expect("close story fixture"))),
            key: core::CallKey {
                task: run.0,
                attempt: run.1,
                completion: u32::try_from(index + 1).expect("close story fixture"),
                position: 0,
            },
            batch: Box::new([delegate.clone()]),
            stubs: Box::new([]),
        }));
        let core::CallPart::Delegated(numbers) = &world.answers.last().expect("close story fixture").2 else {
            panic!("delegates admitted: {:?}", world.answers)
        };
        *slot = numbers[0].task;
    }
    (world, configuration, run.0, delegates)
}

/// Three named obligations share one scenario setup without dropping any assertion.
pub fn disposals_and_finish_grace() {
    let (mut world, configuration, chat, delegates) = fixture_with_delegates();
    a_person_closes_a_chat_keeping_one_delegate_and_cancelling_another(&mut world, chat, delegates);
    a_chat_that_does_not_finish_within_its_grace_is_cancelled(world, configuration, chat, delegates[0]);
}

fn a_person_closes_a_chat_keeping_one_delegate_and_cancelling_another(
    world: &mut World,
    chat: u64,
    delegates: [u64; 2],
) {
    acts(
        world,
        3,
        Ask::Close {
            task: Task::Number(chat),
            disposals: Box::new([
                DelegateChoice { task: Task::Number(delegates[0]), keep: Keep::MakeMine },
                DelegateChoice { task: Task::Number(delegates[1]), keep: Keep::Cancel },
            ]),
        },
    );
    assert_eq!(
        world.peers.as_ref().expect("close story fixture").parties[0].replies.last().expect("close story fixture").1,
        parties::Reply::Closed { chat, cancelled: 1, kept: 1 }
    );
    let person = world.peers.as_ref().expect("close story fixture").parties[0].person.expect("close story fixture");
    assert_eq!(live(world, delegates[0]).requester, tasks::Party::Person(person));
    assert_eq!(live(world, delegates[0]).funder, tasks::Funder::Pool { project: 1, person, period: 1 });
    assert!(world.store.rows.contains_key(&root::Key::Core(core::Key::Tasks(tasks::Key::Ended(delegates[1])))));
    let grace = live(world, chat).grace.clone().expect("close story fixture");
    assert_eq!(grace.by, tasks::Party::Person(person));
    let Some(root::Record::Core(core::Record::People(people::Stored::Answer { at, .. }))) = world
        .store
        .rows
        .get(&root::Key::Core(core::Key::People(people::Key::Answer(people::RequestKey { person, key: [3; 16] }))))
    else {
        panic!("durable close answer")
    };
    assert_eq!(grace.until.as_nanos() - at.as_nanos(), Duration::from_secs(5).as_nanos());
    assert!(
        world.peers.as_ref().expect("close story fixture").workers[0]
            .messages
            .iter()
            .any(|(task, _, _, words)| *task == chat && words.as_ref() == b"Finish this chat.")
    );
    let decisions = world.peers.as_ref().expect("close story fixture").parties[0]
        .replies
        .iter()
        .filter(|(_, reply)| matches!(reply, parties::Reply::Closed { .. }))
        .count();
    world.peers.as_mut().expect("close story fixture").parties[0].extend(Box::new([Act::Twice]));
    world.drain();
    assert_eq!(
        world.peers.as_ref().expect("close story fixture").parties[0]
            .replies
            .iter()
            .filter(|(_, reply)| matches!(reply, parties::Reply::Closed { .. }))
            .count(),
        decisions + 1
    );
    assert_eq!(live(world, chat).grace, Some(grace), "key replay retained original deadline");
}

fn a_chat_that_does_not_finish_within_its_grace_is_cancelled(
    mut world: World,
    configuration: root::Config,
    chat: u64,
    kept: u64,
) {
    world.advance(Duration::from_secs(2));
    world.restart_with(configuration);
    world.advance(Duration::from_secs(1));
    assert!(live(&world, chat).grace.is_some());
    let until = live(&world, chat).grace.clone().expect("close story fixture").until;
    world.advance(Duration::from_secs(3));
    let Some(root::Record::Core(core::Record::Tasks(tasks::Stored::Ended(row)))) =
        world.store.rows.get(&root::Key::Core(core::Key::Tasks(tasks::Key::Ended(chat))))
    else {
        panic!("grace expired: {:?}", world.trace)
    };
    assert!(matches!(row.phase, tasks::Phase::Ended(tasks::Ending::Cancelled { at, .. }) if at >= until));
    assert!(live(&world, kept).grace.is_none(), "kept work survived old chat cancellation");
}
