//! Clock fixtures share the same party request path as the ordinary batch stories.
use super::ask_as;
use crate::effects::World;
use jig_domain_people as people;
use skein_lib::{Time, Wall};
const PARTY: u64 = 1;
const OWNER_ROLE: u32 = 0;
/// Five seconds makes the initial decision fit a twenty-second holder, then age beyond it.
pub const SPAN: skein_lib::Duration = skein_lib::Duration::from_secs(5);
pub fn clock(world: &mut World, seconds: u64) {
    let nanos = seconds * 1_000_000_000;
    world.set_clock(Time::from_nanos(nanos), Wall::from_nanos(nanos));
}
pub fn propose(world: &mut World, sign_in: u64, key: u8, batch: Box<[people::Member]>) -> u64 {
    ask_as(
        world,
        sign_in,
        key,
        people::Ask::Propose {
            project: PROJECT,
            action: people::Action::Batch(batch),
            reason: b"please fund this batch".as_slice().into(),
        },
    );
    let Some(people::Reply::Outcome(people::Outcome::Proposed { proposal, to: OWNER_ROLE })) =
        world.people_answers.last()
    else {
        panic!("batch proposed: {:?}", world.people_answers.last())
    };
    *proposal
}
pub fn decide(world: &mut World, sign_in: u64, key: u8, proposal: u64, decision: people::ProposalDecision) {
    ask_as(
        world,
        sign_in,
        key,
        people::Ask::DecideProposal {
            project: PROJECT,
            proposer: people::Proposer::Party(PARTY),
            proposal,
            decision,
            running: None,
        },
    );
}

/// The root fixture has one live chat; every story's first newly numbered member is two.
pub const CREATED: u64 = 2;
pub const PROJECT: u32 = 1;
#[must_use]
pub fn span_member() -> people::Member {
    let mut member = super::people_member(b"span work", 20);
    member.deadline = people::Deadline::Span(SPAN);
    member
}
pub fn create(world: &mut World, sign_in: u64, key: u8) {
    ask_as(world, sign_in, key, people::Ask::Create { project: PROJECT, batch: Box::new([span_member()]) });
}
#[must_use]
pub fn pending(seed: u64) -> (World, u64, u64, u64) {
    let mut world = crate::stories::offers::world_with(seed, Some(people::Role::Member), |limits| {
        limits.core.authority.batch = 2;
    });
    let member = world.sign_in.expect("member");
    let owner = super::second_party(&mut world, b"second-party", people::Role::Owner);
    clock(&mut world, 10);
    let proposal = propose(&mut world, member, 32, Box::new([span_member()]));
    (world, member, owner, proposal)
}
#[must_use]
pub fn person_proposal(world: &World, number: u64) -> &jig_domain_tasks::PersonProposal {
    let row = &world.store.rows
        [&jig_test_domain::Key::Core(jig_domain_core::Key::Tasks(jig_domain_tasks::Key::PersonProposal(number)))];
    let jig_test_domain::Record::Core(jig_domain_core::Record::Tasks(jig_domain_tasks::Stored::PersonProposal(
        proposal,
    ))) = row
    else {
        panic!("pending proposal: {row:?}")
    };
    proposal
}
/// Absolute role horizons make a five-second span age beyond its first holder.
pub fn finite_policy(
    limits: &mut jig_test_domain::Limits,
    _: &mut jig_domain_authority::Rules,
    policy: &mut jig_domain_authority::Policy,
) {
    // A one-second wait makes one explicit timer delivery sufficient for a stall.
    limits.core.tasks.proposal_stall = skein_lib::Duration::from_secs(1);
    policy.ceiling.budget.deadline = Some(Wall::from_nanos(100_000_000_000));
    for role in &mut policy.roles {
        role.authority.budget.deadline =
            Some(Wall::from_nanos(if role.number == people::Role::Member.number() { 20 } else { 100 } * 1_000_000_000));
        role.decides = jig_domain_authority::Proposals::ALL;
    }
}
pub fn finite_world(seed: u64) -> (World, u64, u64) {
    let mut world = crate::stories::offers::world_with_policy(seed, Some(people::Role::Member), finite_policy);
    let member = world.sign_in.expect("member");
    let owner = super::second_party(&mut world, b"second-party", people::Role::Owner);
    clock(&mut world, 10);
    world.sign_in = Some(member);
    (world, member, owner)
}
/// Reuse the allocated person-proposal batch for a task-origin holder story.
pub fn task_proposal(world: &mut World, member: u64) -> u64 {
    let number = propose(world, member, 32, Box::new([span_member()]));
    let batch = person_proposal(world, number).batch().into();
    world.send(jig_test_domain::Event::Core(jig_domain_core::Event::Tasks(jig_domain_tasks::Event::Propose {
        reply_to: skein_lib::ReplyTo::new(skein_lib::Token::new(8300)),
        proposal: jig_domain_tasks::Proposal {
            number,
            proposer: 1,
            project: PROJECT,
            action: jig_domain_tasks::ProposalAction::Batch(batch),
            reason: b"span work".as_slice().into(),
            as_holder: true,
            state: jig_domain_tasks::ProposalState::Pending {
                holder: jig_domain_tasks::ProposalHolder::Person(PARTY),
                since: world.clocks().1,
            },
        },
    })));
    number
}
pub fn task_decide(world: &mut World, sign_in: u64, key: u8, proposal: u64, decision: people::ProposalDecision) {
    ask_as(
        world,
        sign_in,
        key,
        people::Ask::DecideProposal {
            project: PROJECT,
            proposer: people::Proposer::Task(1),
            proposal,
            decision,
            running: None,
        },
    );
}
#[must_use]
pub fn holder(world: &World) -> jig_domain_tasks::ProposalHolder {
    let state = &super::live_row(world, 1).proposal.as_ref().expect("task proposal").state;
    let jig_domain_tasks::ProposalState::Pending { holder, .. } = state else { panic!("pending: {state:?}") };
    *holder
}
pub fn acceptance(world: &mut World, proposal: u64) -> jig_domain_core::acts::Admission {
    crate::reads::ask(
        world,
        9300,
        jig_domain_core::reads::Read::Proposal { proposer: people::Proposer::Task(1), proposal },
    );
    let jig_domain_core::reads::Answer::Document(doc) = crate::reads::latest(world) else {
        panic!("proposal document")
    };
    let jig_domain_core::documents::Document::Proposal(doc) = &**doc else { panic!("proposal document") };
    doc.acts
        .iter()
        .find(|offer| offer.act == jig_domain_core::acts::Act::Accept)
        .expect("acceptance offer")
        .admission
        .clone()
}

pub fn pass(world: &mut World, member: u64, proposal: u64, _: u64) {
    task_decide(world, member, 34, proposal, people::ProposalDecision::Pass);
}
pub fn reroute(world: &mut World, _: u64, _: u64, owner: u64) {
    ask_as(
        world,
        owner,
        35,
        people::Ask::SetRoles {
            project: PROJECT,
            holdings: Box::new([
                people::Holding { person: 1, role: people::Role::Member },
                people::Holding { person: 2, role: people::Role::Owner },
            ]),
        },
    );
}

pub fn stall(world: &mut World, _: u64, _: u64, _: u64) {
    world.send(jig_test_domain::Event::Timer(jig_domain_core::Timer::Tasks));
}
