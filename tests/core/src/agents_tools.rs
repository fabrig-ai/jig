//! Named tool stories share restart cuts around their own commits.
use crate::effects::{Cut, World};
use jig_domain_core as core;
use jig_domain_tasks as tasks;
const CHILD: u64 = 2;
const REQUESTER: u64 = 1;
const SUBSCRIPTION: u64 = 1;
const SUBSCRIBE_COMPLETION: u32 = 2;
const CHANNEL: u64 = 7;

pub struct Story {
    pub name: &'static str,
    pub run: fn(&mut World, Option<Cut>) -> u64,
}
pub const STORIES: &[Story] = &[
    Story { name: "amend done", run: amend_done },
    Story { name: "amend moved", run: amend_moved },
    Story { name: "release moved", run: release_moved },
    Story { name: "held notice", run: held_notice },
    Story { name: "delegate given", run: delegate_given },
    Story { name: "withdraw decided", run: withdraw_decided },
];
fn amend_done(world: &mut World, cut: Option<Cut>) -> u64 {
    amend(world, cut, false)
}
fn amend_moved(world: &mut World, cut: Option<Cut>) -> u64 {
    amend(world, cut, true)
}
fn amend(world: &mut World, cut: Option<Cut>, moved: bool) -> u64 {
    world.hold_procedures = true;
    world.delegate();
    let task = world.procedure.expect("delegated child");
    let amend = format!(
        r#"{{"target":{task},"revision":0,"amendment":{{"spec":{{"words":"check again"}},"reason":"new evidence"}}}}"#
    );
    if moved {
        world.tool_call(b"amend", amend.as_bytes(), true);
        world.turn(1, 0, None, b"delegate and amendment answered");
        world.turn(2, 0, None, b"record the answers");
    }
    let before = world.store.applied;
    world.cut = cut.map(|cut| match cut {
        Cut::Submitted(at) => Cut::Submitted(before + at),
        Cut::Durable(at) => Cut::Durable(before + at),
    });
    world.tool_call(b"amend", amend.as_bytes(), true);
    let key = world.call_key(if moved { 3 } else { 2 });
    if cut.is_some() {
        assert!(world.reached_cut.is_some(), "call reaches the selected commit");
        world.restart(190, false);
        reconnect(world, key);
    }
    replay(world, key, b"amend", amend.as_bytes());
    let body: &[u8] =
        if moved { b"Moved: task 2 is at spec revision 1 now" } else { b"Done: task 2 is at spec revision 1" };
    assert_eq!(
        &world.host_answers.last().expect("replayed answer").1.answer,
        &core::SettledAnswer::Host { error: moved, body: body.into() }
    );
    let commits = world.store.applied - before;
    if cut.is_some() {
        world.reached_cut = cut;
    }
    commits
}

fn reconnect(world: &mut World, key: core::CallKey) {
    use jig_domain_fleet as fleet;
    use skein_lib::{Duration, Token};
    world.send(jig_test_domain::Event::Core(core::Event::Fleet(fleet::Event::Hello {
        worker: 0,
        channel: Token::new(CHANNEL),
        hello: fleet::Hello {
            bounds: crate::fixtures::bounds(),
            stop_bound: Duration::from_millis(100),
            slots: crate::world::limits().core.fleet.slots,
            workstreams: Box::new([]),
            hosting: Box::new([fleet::Hosted {
                run: Token::new(key.task),
                attempt: Token::new(key.attempt),
                phase: fleet::Phase::Active,
            }]),
        },
    })));
}

fn replay(world: &mut World, key: core::CallKey, tool: &[u8], input: &[u8]) {
    let name = smith_channel::v5::CallName::new(
        &smith_channel::v5::CEILINGS,
        smith_channel::v5::CallNameParts {
            activation: key.attempt,
            completion: key.completion,
            position: key.position,
        },
    )
    .expect("call name");
    let mut bytes = skein_lib::Writer::new(name.measure() as usize);
    name.encode(&mut bytes).expect("measured call name");
    let answered = world.host_answers.len();
    world.relay_call(&bytes.finish(), tool, input, true);
    assert_eq!(world.host_answers.len(), answered + 1, "one answer to the repeated call");
}
fn release_moved(world: &mut World, cut: Option<Cut>) -> u64 {
    world.hold_procedures = true;
    world.delegate();
    assert_eq!(world.procedure, Some(CHILD));
    for why in [tasks::Hold::Stopped, tasks::Hold::Budget] {
        world.send(jig_test_domain::Event::Core(core::Event::Tasks(tasks::Event::Hold { task: CHILD, why })));
    }
    let input = format!(r#"{{"target":{CHILD},"revision":1}}"#);
    let before = world.store.applied;
    world.cut = cut.map(|cut| match cut {
        Cut::Submitted(at) => Cut::Submitted(before + at),
        Cut::Durable(at) => Cut::Durable(before + at),
    });
    world.tool_call(b"release", input.as_bytes(), true);
    let key = world.call_key(2);
    if cut.is_some() {
        assert!(world.reached_cut.is_some(), "call reaches the selected commit");
        world.restart(190, false);
        reconnect(world, key);
    }
    replay(world, key, b"release", input.as_bytes());
    assert_eq!(
        &world.host_answers.last().expect("release answer").1.answer,
        &core::SettledAnswer::Host {
            error: true,
            body: format!("Moved: task {CHILD} is held at hold revision 2 now, for its budget").as_bytes().into()
        }
    );
    assert_eq!(crate::fixtures::live_row(world, CHILD).revisions.hold, 2);
    let commits = world.store.applied - before;
    if cut.is_some() {
        world.reached_cut = cut;
    }
    commits
}

fn held_notice(world: &mut World, cut: Option<Cut>) -> u64 {
    *world = crate::fixtures::notices_world(190);
    world.hold_procedures = true;
    world.delegate();
    let input = format!(r#"{{"kind":"task","target":{CHILD},"held":true}}"#);
    world.tool_call(b"subscribe", input.as_bytes(), true);
    let key = world.call_key(SUBSCRIBE_COMPLETION);
    let before = world.store.applied;
    world.cut = cut.map(|cut| match cut {
        Cut::Submitted(at) => Cut::Submitted(before + at),
        Cut::Durable(at) => Cut::Durable(before + at),
    });
    world.send(jig_test_domain::Event::Core(core::Event::Tasks(tasks::Event::Hold {
        task: CHILD,
        why: tasks::Hold::Budget,
    })));
    if cut.is_some() {
        assert!(world.reached_cut.is_some(), "hold reaches its cut");
        world.restart(190, false);
        reconnect(world, key);
        world.send(jig_test_domain::Event::Core(core::Event::Tasks(tasks::Event::Hold {
            task: CHILD,
            why: tasks::Hold::Budget,
        })));
    }
    replay(world, key, b"subscribe", input.as_bytes());
    assert_eq!(
        &world.host_answers.last().expect("subscription answer").1.answer,
        &core::SettledAnswer::Host { error: false, body: b"Subscribed 1".as_slice().into() }
    );
    let requester = crate::fixtures::live_row(world, REQUESTER);
    assert_eq!(
        &requester.inbox.last().expect("held notice").kind,
        &tasks::MessageKind::Notice {
            subscription: SUBSCRIPTION,
            target: CHILD,
            state: tasks::NoticeState::Held { revision: 1 }
        }
    );
    let commits = world.store.applied - before;
    if cut.is_some() {
        world.reached_cut = cut;
    }
    commits
}

/// A capped `AsAllowed` delegate leaves room for the caller to keep running.
pub const DELEGATE_GIVEN: &[u8] = br#"{"batch":[{"executor":{"kind":"procedure","connector":1,"code":1},"spec":{"words":"check"},"contract":{"kind":"report","words":64},"authority":{"tools":0,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},"budget":{"spend":0},"notes":0},"budget":{"as_allowed":{"cap":40}}}]}"#;
pub fn delegate_given(world: &mut World, cut: Option<Cut>) -> u64 {
    world.hold_procedures = true;
    let before = world.store.applied;
    world.cut = cut.map(|cut| match cut {
        Cut::Submitted(at) => Cut::Submitted(before + at),
        Cut::Durable(at) => Cut::Durable(before + at),
    });
    world.tool_call(b"delegate", DELEGATE_GIVEN, true);
    let key = world.call_key(1);
    if cut.is_some() {
        assert!(world.reached_cut.is_some(), "delegate reaches its cut");
        world.restart(190, false);
        reconnect(world, key);
    }
    replay(world, key, b"delegate", DELEGATE_GIVEN);
    assert_eq!(
        &world.host_answers.last().expect("delegate answer").1.answer,
        &core::SettledAnswer::Host {
            error: false,
            body: b"Delegated tasks: 2 at spec revision 0, given 40".as_slice().into()
        }
    );
    assert_eq!(crate::fixtures::live_row(world, CHILD).numbers.budget, 40);
    let commits = world.store.applied - before;
    if cut.is_some() {
        world.reached_cut = cut;
    }
    commits
}

const PROPOSE_TRACK: &[u8] = br#"{"action":"amend","target":2,"revision":0,"amendment":{"tracked":{"priority":7},"reason":"track"},"reason":"please"}"#;
const EVIDENCE: &[u8] = br#"{"target":2,"form":"words","words":"evidence"}"#;
const WITHDRAW: &[u8] = br#"{"proposal":3}"#;
const ACCEPTED_PROPOSAL: u64 = 3;
const ACCEPT: u8 = 73;
const WITHDRAW_COMPLETION: u32 = 5;
fn accepted_proposal(world: &mut World) {
    world.hold_procedures = true;
    world.delegate();
    world.tool_call(b"message", EVIDENCE, true);
    world.turn(1, 0, None, b"record delegate");
    world.turn(2, 0, None, b"record message");
    world.tool_call(b"message", EVIDENCE, true);
    world.turn(3, 0, None, b"record message");
    world.tool_call(b"propose", PROPOSE_TRACK, true);
    assert_eq!(
        world.answers.last().expect("proposed").2,
        core::CallPart::Proposed { proposal: ACCEPTED_PROPOSAL, holder: tasks::ProposalHolder::Person(REQUESTER) }
    );
    world.turn(4, 0, None, b"record proposal");
    crate::fixtures::ask_as(
        world,
        world.sign_in.expect("signed in"),
        ACCEPT,
        jig_domain_people::Ask::DecideProposal {
            project: 1,
            proposer: jig_domain_people::Proposer::Task(REQUESTER),
            proposal: ACCEPTED_PROPOSAL,
            decision: jig_domain_people::ProposalDecision::Accept,
            running: None,
        },
    );
    let Some(jig_domain_people::Reply::Outcome(jig_domain_people::Outcome::ProposalDecided {
        choice: jig_domain_people::ProposalChoice::Accepted,
        ..
    })) = world.people_answers.last()
    else {
        panic!("acceptance: {:?}", world.people_answers.last())
    };
}
/// A withdrawal loads the accepted proposal from history and retains its answer.
pub fn withdraw_decided(world: &mut World, cut: Option<Cut>) -> u64 {
    accepted_proposal(world);
    let before = world.store.applied;
    world.cut = cut.map(|cut| match cut {
        Cut::Submitted(at) => Cut::Submitted(before + at),
        Cut::Durable(at) => Cut::Durable(before + at),
    });
    world.tool_call(b"withdraw", WITHDRAW, true);
    let key = world.call_key(WITHDRAW_COMPLETION);
    if cut.is_some() {
        assert!(world.reached_cut.is_some(), "withdraw reaches its cut");
        world.restart(190, false);
        reconnect(world, key);
    }
    replay(world, key, b"withdraw", WITHDRAW);
    assert_eq!(
        &world.host_answers.last().expect("withdraw answer").1.answer,
        &core::SettledAnswer::Host {
            error: false,
            body: b"Already decided: proposal 3 was accepted".as_slice().into()
        }
    );
    let commits = world.store.applied - before;
    if cut.is_some() {
        world.reached_cut = cut;
    }
    commits
}
