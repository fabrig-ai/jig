use jig_core_world::effects::World;
use jig_core_world::fixtures::ask_as;
use jig_core_world::stories::offers::no_holder_fixture;
use jig_domain_authority as authority;
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{ReplyTo, Time, Token, Wall};

#[test]
fn a_chats_flags_and_last_activity_survive_a_cold_restart() {
    let mut world = World::new(171, false);
    world.set_clock(Time::from_nanos(9_000_000_000), Wall::from_nanos(9_000_000_000));
    world.send(root::Event::Core(core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(8001)),
        sign_in: world.sign_in.expect("signed in"),
        key: [1; 16],
        ask: people::Ask::Say { project: 1, task: 1, words: b"heard at nine".as_slice().into() },
    })));
    world.restart(171, false);
    let root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) =
        &world.store.rows[&root::Key::Core(core::Key::Tasks(tasks::Key::Live(1)))]
    else {
        panic!("the chat is live after the restart")
    };
    let templates = jig_core_world::fixtures::templates();
    let label = templates.iter().find(|template| template.number == 1).expect("fixture template").label;
    assert_eq!(row.label, label);
    assert_eq!(
        row.flags,
        tasks::Flags {
            ended_by: tasks::EndedBy::Result,
            conversation: true,
            listed: false,
            retention: tasks::Retention::Kept
        }
    );
    assert_eq!(row.last_activity, Wall::from_nanos(9_000_000_000));
}

const PROJECT: u32 = 1;
const OWNER: u64 = 1;
const MEMBER: u64 = 2;
const MEMBER_CHAT: u64 = 2;
const MEMBER_CHAT_BUDGET: u64 = 100;
const SIGN_IN: u64 = 8100;
const ACTION: u64 = 8300;
const COMPLETION: u32 = 1;

fn member_world() -> (World, u64, u64) {
    let (configuration, limits) = no_holder_fixture(172);
    let mut world = World::configured(172, false, configuration, limits);
    let owner = world.sign_in.expect("signed-in owner");
    world.send(root::Event::Core(core::Event::SignIn {
        held: None,
        reply_to: ReplyTo::new(Token::new(SIGN_IN)),
        identity: people::Identity {
            key: people::IdentityKey { provider: 0, subject: b"member".as_slice().into() },
            login: b"member".as_slice().into(),
            name: b"Member".as_slice().into(),
        },
    }));
    let member = world.sign_in.expect("signed-in member");
    world.send(root::Event::Core(core::Event::People(people::Event::Roles {
        project: PROJECT,
        holdings: Box::new([
            people::Holding { person: OWNER, role: people::Role::Owner },
            people::Holding { person: MEMBER, role: people::Role::Member },
        ]),
    })));
    (world, owner, member)
}

fn member_decides(world: &mut World, owner: u64, key: u8, decides: authority::Proposals) {
    let role = people::PolicyRole {
        number: people::Role::Member.number(),
        authority: people::Authority {
            tools: 1023,
            models: None,
            grants: Box::new([]),
            delegation: people::Delegation {
                kinds: Box::new([people::Executor::Charter(1), people::Executor::Procedure(1)]),
                tasks: 2,
                depth: 2,
            },
            spend: 100,
            deadline: None,
            notes: 0,
            note_resources: Box::new([]),
        },
        period_spend: 500,
        requests: authority::Requests::ALL.0,
        decides: decides.0,
    };
    ask_as(world, owner, key, people::Ask::ChangePolicy { project: PROJECT, change: people::PolicyChange::Role(role) });
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::PolicyChanged { project: PROJECT }))
    );
}

fn start_member_chat(world: &mut World, member: u64, key: u8) {
    let request = jig_core_world::fixtures::chat_for(
        world,
        PROJECT,
        people::Role::Member.number(),
        b"member's root".as_slice().into(),
    );
    ask_as(world, member, key, request);
}

#[test]
fn a_party_refused_a_root_is_made_once_their_role_holds_it() {
    let (mut world, owner, member) = member_world();
    start_member_chat(&mut world, member, 11);
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Refused(people::Refusal::NoEscalationHolder)))
    );
    member_decides(&mut world, owner, 12, authority::Proposals(16));
    start_member_chat(&mut world, member, 13);
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Created {
            first: MEMBER_CHAT,
            last: MEMBER_CHAT,
            given: Some(people::Given { task: MEMBER_CHAT, amount: MEMBER_CHAT_BUDGET })
        }))
    );
}

fn propose_member_batch(world: &mut World) -> u64 {
    assert_eq!(world.call_key(COMPLETION).task, MEMBER_CHAT);
    let mut given = world.assigned.as_ref().expect("assigned member chat").run.authority.clone();
    given.budget.spend = 20;
    given.delegation.tasks = 0;
    given.delegation.depth = 0;
    let delegate = core::Delegate {
        deadline: jig_domain_tasks::Deadline::Named,
        label: 0,
        flags: jig_domain_tasks::Flags::PLAIN,
        tracked: None,
        budget: jig_domain_people::Budget::Named,
        executor: tasks::Executor::Agent { charter: 1 },
        spec: tasks::Spec {
            asks: false,
            words: b"proposed root".as_slice().into(),
            parameters: Box::new([]),
            inputs: Box::new([]),
        },
        contract: tasks::Contract::Report { words: 128 },
        authority: given,
        llm: None,
        symbolic_grants: Box::new([]),
        dependencies: Box::new([]),
        wake: tasks::WakePolicy::DEFAULT,
    };
    world.send(root::Event::Core(core::Event::NamedAction {
        to: ReplyTo::new(Token::new(ACTION)),
        key: world.call_key(COMPLETION),
        action: core::NamedAction::ProposeBatch {
            batch: Box::new([delegate]),
            reason: b"make this yours".as_slice().into(),
            as_holder: true,
        },
    }));
    let core::CallPart::Proposed { proposal, .. } = &world.answers.last().expect("proposal answer").2 else {
        panic!("batch proposed: {:?}", world.answers)
    };
    *proposal
}

#[test]
fn accepting_a_chats_batch_as_ones_own_is_refused_while_nobody_holds_it() {
    let (mut world, owner, member) = member_world();
    world.end(
        tasks::End::Finished {
            result: tasks::TaskResult::Report { words: b"owner finished".as_slice().into() },
            cancel_delegates: false,
        },
        0,
    );
    member_decides(&mut world, owner, 21, authority::Proposals(17));
    start_member_chat(&mut world, member, 22);
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Created {
            first: MEMBER_CHAT,
            last: MEMBER_CHAT,
            given: Some(people::Given { task: MEMBER_CHAT, amount: MEMBER_CHAT_BUDGET })
        }))
    );
    let proposal = propose_member_batch(&mut world);
    member_decides(&mut world, owner, 23, authority::Proposals(1));
    let tasks_before = live_task_count(&world);
    world.decide_proposal(proposal, true, 24);
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Refused(people::Refusal::NoEscalationHolder)))
    );
    assert_eq!(live_task_count(&world), tasks_before);
}

fn live_task_count(world: &World) -> usize {
    world.store.rows.keys().filter(|key| matches!(key, root::Key::Core(core::Key::Tasks(tasks::Key::Live(_))))).count()
}
