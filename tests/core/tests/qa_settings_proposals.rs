//! Adversarial proposals of deployment settings (`domain/engine.md`, 17.6).
use jig_core_world::{
    effects::{Cut, World},
    fixtures, peers, reads,
};
use jig_domain_core::{self as core, settings as s};
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{ReplyTo, Token};

#[global_allocator]
static HEAP: skein_world::domain::heap::Counting = skein_world::domain::heap::Counting;

const SEED: u64 = 6001;
const OWNER: u64 = 1;
const DEPLOYMENT: u32 = 0;
// The small profile's settings bound; one further party exercises refusal.
const PROPOSAL_LIMIT: u32 = 4;
const PARTIES: u32 = PROPOSAL_LIMIT + 1;
const MEMBER: u64 = 2;
const CHARTER: u32 = 1;

fn configuration() -> (root::Config, root::Limits) {
    let (mut config, mut limits) = peers::fixture(SEED, 0);
    config.core.owners = Box::new([
        config.core.owners[0].clone(),
        people::InitialOwner { project: DEPLOYMENT, identity: config.core.owners[0].identity.clone() },
    ]);
    limits.core.people.initial_owners = u32::try_from(config.core.owners.len()).expect("two configured roles");
    limits.core.people.people = PARTIES;
    limits.core.people.sign_ins = PARTIES;
    limits.core.people.holdings = PARTIES;
    limits.core.settings.proposals = PROPOSAL_LIMIT;
    peers::journal(&mut limits);
    (config, limits)
}

fn world() -> World {
    let (config, limits) = configuration();
    let mut world = World::configured(SEED, false, config, limits);
    world.reads = Some(reads::Parties::new(world.store.applied));
    world
}

fn charter_item(words: &[u8]) -> s::Item {
    let mut charter = configuration().0.core.defaults.charters[0].clone();
    charter.policy.instructions = words.into();
    s::Item { setting: s::Setting::Charter(CHARTER), edit: s::Edit::Set(s::Value::Charter(Box::new(charter))) }
}

fn proposal_event(sign_in: u64, key: u8, against: u32, items: Box<[s::Item]>) -> root::Event {
    root::Event::Core(core::Event::ProposeSettings {
        reply_to: ReplyTo::new(Token::new(30_000 + u64::from(key))),
        sign_in,
        key: [key; 16],
        digest: [key; 32],
        change: Box::new(s::Proposed { against, items, reason: b"QA proposal".as_slice().into() }),
    })
}

fn propose(world: &mut World, sign_in: u64, key: u8, against: u32, items: Box<[s::Item]>) -> u64 {
    world.send(proposal_event(sign_in, key, against, items));
    match world.people_answers.last() {
        Some(people::Reply::Outcome(people::Outcome::Proposed { proposal, .. })) => *proposal,
        reply => panic!("settings proposal key {key}: {reply:?}"),
    }
}

fn decide(world: &mut World, sign_in: u64, key: u8, proposer: u64, proposal: u64, decision: people::ProposalDecision) {
    fixtures::ask_as(
        world,
        sign_in,
        key,
        people::Ask::DecideProposal {
            project: DEPLOYMENT,
            proposer: people::Proposer::Party(proposer),
            proposal,
            running: match decision {
                people::ProposalDecision::Accept => Some(people::Running::Stays),
                people::ProposalDecision::Reject { .. } | people::ProposalDecision::Pass => None,
            },
            decision,
        },
    );
}

fn proposal_document(world: &mut World, sign_in: u64, proposer: u64, proposal: u64) -> core::documents::Proposal {
    world.sign_in = Some(sign_in);
    reads::ask(world, 1, core::reads::Read::Proposal { proposer: people::Proposer::Party(proposer), proposal });
    let core::reads::Answer::Document(doc) = reads::latest(world) else {
        panic!("proposal read: {:?}", reads::latest(world))
    };
    let core::documents::Document::Proposal(doc) = &**doc else { panic!("proposal document: {doc:?}") };
    *doc.clone()
}

#[test]
fn member_proposal_has_an_owner_review() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    let member = fixtures::second_party(&mut world, b"member", people::Role::Member);
    let number = propose(&mut world, member, 10, 1, Box::new([charter_item(b"proposed charter")]));
    let own_review = proposal_document(&mut world, member, MEMBER, number);
    assert_eq!(own_review.project, DEPLOYMENT, "the proposing member can read its pending proposal");
    let doc = proposal_document(&mut world, owner, MEMBER, number);
    assert_eq!(doc.project, DEPLOYMENT);
    let core::documents::ProposalAction::Settings { against, items, review, made } = doc.action else {
        panic!("settings action")
    };
    assert_eq!(against, 1);
    assert_eq!(items.len(), 1);
    assert_eq!(made, None);
    let review = review.expect("pending review");
    assert_eq!(review.newest, 1);
    assert_eq!(review.items.len(), 1);
    assert_eq!(review.items.iter().next().expect("one review item").origin.map(|(_, revision)| revision), Some(1));
    assert_eq!(
        review.items.iter().next().expect("one review item").now,
        Some(s::Value::Charter(Box::new(configuration().0.core.defaults.charters[0].clone())))
    );
    assert_eq!(
        review.items.iter().next().expect("one review item").would_be,
        match charter_item(b"proposed charter").edit {
            s::Edit::Set(value) => Some(value),
            s::Edit::Removed | s::Edit::Reset => unreachable!("set fixture"),
        }
    );
}

#[test]
fn proposal_survives_an_unchanged_cold_restart() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    let number = propose(&mut world, owner, 11, 1, Box::new([charter_item(b"kept over restart")]));
    let before = proposal_document(&mut world, owner, OWNER, number);
    world.restart_with(configuration().0);
    let after = proposal_document(&mut world, owner, OWNER, number);
    assert_eq!(after, before);
    decide(&mut world, owner, 12, OWNER, number, people::ProposalDecision::Accept);
    assert_eq!(settings_document(&mut world, owner).newest, 2, "acceptance: {:?}", world.people_answers.last());
}

fn change(world: &mut World, sign_in: u64, key: u8, against: u32, items: Box<[s::Item]>) {
    world.send(root::Event::Core(core::Event::ChangeSettings {
        reply_to: ReplyTo::new(Token::new(31_000 + u64::from(key))),
        sign_in,
        key: [key; 16],
        digest: [key; 32],
        change: Box::new(s::Change { against, items, running: people::Running::Stays }),
    }));
}

fn settings_document(world: &mut World, sign_in: u64) -> core::documents::Settings {
    world.sign_in = Some(sign_in);
    reads::ask(world, 2, core::reads::Read::Settings);
    let core::reads::Answer::Document(doc) = reads::latest(world) else {
        panic!("settings read: {:?}", reads::latest(world))
    };
    let core::documents::Document::Settings(doc) = &**doc else { panic!("settings document: {doc:?}") };
    *doc.clone()
}

#[test]
fn the_last_proposal_slot_is_usable() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    fill_proposals(&mut world, owner);
    assert_eq!(settings_document(&mut world, owner).pending.len(), PROPOSAL_LIMIT);
}

#[test]
fn one_past_the_proposal_limit_is_refused() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    fill_proposals(&mut world, owner);
    let fifth = fixtures::second_party(&mut world, b"fifth", people::Role::Member);
    world.send(proposal_event(fifth, 24, 1, Box::new([charter_item(b"one too many")])));
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Limit {
            which: people::LimitKind::SettingsProposals
        })))
    );
    assert_eq!(settings_document(&mut world, owner).pending.len(), PROPOSAL_LIMIT);
}

fn proposal_state(world: &World, number: u64) -> tasks::PersonProposalState {
    world
        .store
        .rows
        .values()
        .find_map(|row| match row {
            root::Record::Core(core::Record::Tasks(
                tasks::Stored::PersonProposal(row) | tasks::Stored::DecidedProposal(row),
            )) if row.number == number => Some(row.state.clone()),
            root::Record::Core(_) | root::Record::Connector { .. } => None,
        })
        .expect("durable party proposal")
}

fn withdraw(world: &mut World, sign_in: u64, key: u8, proposal: u64) {
    fixtures::ask_as(world, sign_in, key, people::Ask::Withdraw { project: DEPLOYMENT, proposal });
}

#[test]
fn a_direct_change_withdraws_every_older_proposal() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    let member = fixtures::second_party(&mut world, b"member", people::Role::Member);
    let first = propose(&mut world, owner, 30, 1, Box::new([charter_item(b"first proposed")]));
    let second = propose(&mut world, member, 31, 1, Box::new([charter_item(b"second proposed")]));
    change(&mut world, owner, 32, 1, Box::new([charter_item(b"direct change")]));
    assert!(matches!(proposal_state(&world, first), tasks::PersonProposalState::Withdrawn { .. }));
    assert!(matches!(proposal_state(&world, second), tasks::PersonProposalState::Withdrawn { .. }));
    let doc = settings_document(&mut world, owner);
    assert_eq!(doc.newest, 2);
    assert!(doc.pending.is_empty());
}

#[test]
fn acceptance_withdraws_siblings_and_records_its_origin() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    let chosen = propose(&mut world, owner, 40, 1, Box::new([charter_item(b"chosen")]));
    let member = fixtures::second_party(&mut world, b"member", people::Role::Member);
    let sibling = propose(&mut world, member, 41, 1, Box::new([charter_item(b"sibling")]));
    decide(&mut world, owner, 42, OWNER, chosen, people::ProposalDecision::Accept);
    assert!(matches!(proposal_state(&world, chosen), tasks::PersonProposalState::SettingsAccepted { .. }));
    assert!(matches!(proposal_state(&world, sibling), tasks::PersonProposalState::Withdrawn { .. }));
    assert_eq!(settings_document(&mut world, owner).newest, 2);
    let revision = settings_revision(&world, 2);
    assert_eq!(revision.head.by, s::Decider::Party(OWNER));
    let origin = revision.head.proposal.expect("proposal origin");
    assert_eq!(origin.0, people::Proposer::Party(OWNER));
    assert_eq!(u64::from(origin.1), chosen);
    assert!(!revision.head.moved);
}

#[test]
fn withdrawal_wins_over_a_later_acceptance() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    let number = propose(&mut world, owner, 50, 1, Box::new([charter_item(b"withdraw me")]));
    withdraw(&mut world, owner, 51, number);
    decide(&mut world, owner, 52, OWNER, number, people::ProposalDecision::Accept);
    assert!(matches!(proposal_state(&world, number), tasks::PersonProposalState::Withdrawn { .. }));
    assert_eq!(settings_document(&mut world, owner).newest, 1);
}

#[test]
fn rejection_wins_over_a_later_acceptance() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    let number = propose(&mut world, owner, 60, 1, Box::new([charter_item(b"reject me")]));
    decide(
        &mut world,
        owner,
        61,
        OWNER,
        number,
        people::ProposalDecision::Reject { reason: b"too broad".as_slice().into() },
    );
    decide(&mut world, owner, 62, OWNER, number, people::ProposalDecision::Accept);
    let tasks::PersonProposalState::Rejected { reason, .. } = proposal_state(&world, number) else {
        panic!("rejection retained")
    };
    assert_eq!(reason.as_ref(), b"too broad");
    assert_eq!(settings_document(&mut world, owner).newest, 1);
}

#[test]
fn only_the_proposer_can_withdraw_a_pending_proposal() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    let member = fixtures::second_party(&mut world, b"member", people::Role::Member);
    let number = propose(&mut world, member, 70, 1, Box::new([charter_item(b"member proposal")]));
    withdraw(&mut world, owner, 71, number);
    assert!(matches!(proposal_state(&world, number), tasks::PersonProposalState::Pending { .. }));
    assert!(matches!(world.people_answers.last(), Some(people::Reply::Outcome(people::Outcome::Refused(_)))));
    withdraw(&mut world, member, 72, number);
    assert!(matches!(proposal_state(&world, number), tasks::PersonProposalState::Withdrawn { .. }));
}

#[test]
fn a_project_owner_cannot_accept_deployment_settings() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    let project_owner = fixtures::second_party(&mut world, b"project owner", people::Role::Owner);
    let number = propose(&mut world, project_owner, 80, 1, Box::new([charter_item(b"project owner proposal")]));
    decide(&mut world, project_owner, 81, MEMBER, number, people::ProposalDecision::Accept);
    assert!(matches!(proposal_state(&world, number), tasks::PersonProposalState::Pending { .. }));
    assert!(matches!(world.people_answers.last(), Some(people::Reply::Outcome(people::Outcome::Refused(_)))));
    decide(&mut world, owner, 82, MEMBER, number, people::ProposalDecision::Accept);
    assert_eq!(settings_document(&mut world, owner).newest, 2);
}

#[test]
fn withdrawal_returns_a_full_tables_proposal_slot() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    let numbers = fill_proposals(&mut world, owner);
    withdraw(&mut world, owner, 94, numbers[0]);
    propose(&mut world, owner, 95, 1, Box::new([charter_item(b"replacement")]));
    assert_eq!(settings_document(&mut world, owner).pending.len(), PROPOSAL_LIMIT);
}

#[test]
fn replaying_a_proposal_key_never_consumes_another_slot() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    let first = propose(&mut world, owner, 100, 1, Box::new([charter_item(b"same request")]));
    let committed = world.store.applied;
    let replay = propose(&mut world, owner, 100, 1, Box::new([charter_item(b"same request")]));
    assert_eq!(replay, first);
    assert_eq!(world.store.applied, committed);
    assert_eq!(settings_document(&mut world, owner).pending.len(), 1);
}

fn fill_proposals(world: &mut World, owner: u64) -> Vec<u64> {
    let mut numbers = vec![propose(world, owner, 20, 1, Box::new([charter_item(b"bounded proposal")]))];
    for (key, subject) in [(21, b"second".as_slice()), (22, b"third".as_slice()), (23, b"fourth".as_slice())] {
        let sign_in = fixtures::second_party(world, subject, people::Role::Member);
        numbers.push(propose(world, sign_in, key, 1, Box::new([charter_item(b"bounded proposal")])));
    }
    numbers
}

#[test]
fn an_uncommitted_proposal_disappears_after_a_crash() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    let replies = world.people_answers.len();
    let commit = world.store.applied + 1;
    world.cut = Some(Cut::Submitted(commit));
    world.send(proposal_event(owner, 110, 1, Box::new([charter_item(b"not durable yet")])));
    assert_eq!(world.reached_cut, Some(Cut::Submitted(commit)));
    assert_eq!(world.people_answers.len(), replies, "no answer before the commit");
    world.restart_with(configuration().0);
    assert!(settings_document(&mut world, owner).pending.is_empty());
    propose(&mut world, owner, 110, 1, Box::new([charter_item(b"not durable yet")]));
    assert_eq!(settings_document(&mut world, owner).pending.len(), 1);
}

#[test]
fn a_durable_unanswered_proposal_replays_after_a_crash() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    let replies = world.people_answers.len();
    let commit = world.store.applied + 1;
    world.cut = Some(Cut::Durable(commit));
    world.send(proposal_event(owner, 111, 1, Box::new([charter_item(b"durable unanswered")])));
    assert_eq!(world.reached_cut, Some(Cut::Durable(commit)));
    assert_eq!(world.people_answers.len(), replies);
    world.restart_with(configuration().0);
    let committed = world.store.applied;
    propose(&mut world, owner, 111, 1, Box::new([charter_item(b"durable unanswered")]));
    assert_eq!(world.store.applied, committed, "the restored key only replays");
    assert_eq!(settings_document(&mut world, owner).pending.len(), 1);
}

#[test]
fn an_uncommitted_acceptance_restores_the_pending_review() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    let number = propose(&mut world, owner, 112, 1, Box::new([charter_item(b"atomic acceptance")]));
    let before = proposal_document(&mut world, owner, OWNER, number);
    let replies = world.people_answers.len();
    let commit = world.store.applied + 1;
    world.cut = Some(Cut::Submitted(commit));
    decide(&mut world, owner, 113, OWNER, number, people::ProposalDecision::Accept);
    assert_eq!(world.reached_cut, Some(Cut::Submitted(commit)));
    assert_eq!(world.people_answers.len(), replies);
    world.restart_with(configuration().0);
    assert_eq!(proposal_document(&mut world, owner, OWNER, number), before);
    assert_eq!(settings_document(&mut world, owner).newest, 1);
    decide(&mut world, owner, 113, OWNER, number, people::ProposalDecision::Accept);
    assert_eq!(settings_document(&mut world, owner).newest, 2);
}

#[test]
fn durable_acceptance_and_sibling_withdrawal_restart_together() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    let member = fixtures::second_party(&mut world, b"member", people::Role::Member);
    let chosen = propose(&mut world, owner, 114, 1, Box::new([charter_item(b"durable acceptance")]));
    let sibling = propose(&mut world, member, 115, 1, Box::new([charter_item(b"stale sibling")]));
    let replies = world.people_answers.len();
    let commit = world.store.applied + 1;
    world.cut = Some(Cut::Durable(commit));
    decide(&mut world, owner, 116, OWNER, chosen, people::ProposalDecision::Accept);
    assert_eq!(world.reached_cut, Some(Cut::Durable(commit)));
    assert_eq!(world.people_answers.len(), replies);
    world.restart_with(configuration().0);
    assert!(matches!(proposal_state(&world, chosen), tasks::PersonProposalState::SettingsAccepted { revision: 2, .. }));
    assert!(matches!(proposal_state(&world, sibling), tasks::PersonProposalState::Withdrawn { .. }));
    let committed = world.store.applied;
    decide(&mut world, owner, 116, OWNER, chosen, people::ProposalDecision::Accept);
    assert_eq!(world.store.applied, committed, "acceptance replay makes no revision");
    let settings = settings_document(&mut world, owner);
    assert_eq!(settings.newest, 2);
    assert!(settings.pending.is_empty());
}

#[test]
fn a_failed_acceptance_commit_exposes_no_success() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    let number = propose(&mut world, owner, 117, 1, Box::new([charter_item(b"failed commit")]));
    let replies = world.people_answers.len();
    world.store.fault(jig_core_world::Fault::Fail { commit: world.store.applied + 1 });
    decide(&mut world, owner, 118, OWNER, number, people::ProposalDecision::Accept);
    assert!(world.stopped);
    assert_eq!(world.people_answers.len(), replies, "a failed decision has no success reply");
    world.restart_with(configuration().0);
    assert!(matches!(proposal_state(&world, number), tasks::PersonProposalState::Pending { .. }));
    assert_eq!(settings_document(&mut world, owner).newest, 1);
}

#[test]
fn a_release_revision_withdraws_restored_proposals() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    let member = fixtures::second_party(&mut world, b"member", people::Role::Member);
    let number = propose(&mut world, member, 120, 1, Box::new([charter_item(b"before release")]));
    let mut release = configuration().0;
    release.core.defaults.charters[0].policy.instructions = b"release default".as_slice().into();
    world.restart_with(release);
    assert!(matches!(proposal_state(&world, number), tasks::PersonProposalState::Withdrawn { .. }));
    let settings = settings_document(&mut world, owner);
    assert_eq!(settings.newest, 2);
    assert!(settings.pending.is_empty());
    assert!(world.store.rows.values().any(|row| matches!(row,
        root::Record::Core(core::Record::Tasks(tasks::Stored::Update(update))) if update.party == MEMBER && update.task == 0
    )), "the proposer has its durable notice");
}

#[test]
fn a_masked_release_keeps_a_pending_proposal() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    change(&mut world, owner, 121, 1, Box::new([charter_item(b"stored charter")]));
    let number = propose(&mut world, owner, 122, 2, Box::new([charter_item(b"after stored change")]));
    let before = proposal_document(&mut world, owner, OWNER, number);
    let mut release = configuration().0;
    release.core.defaults.charters[0].policy.instructions = b"masked release default".as_slice().into();
    world.restart_with(release);
    assert_eq!(settings_document(&mut world, owner).newest, 2);
    assert_eq!(proposal_document(&mut world, owner, OWNER, number), before);
}

#[test]
fn losing_ownership_refuses_acceptance_and_keeps_the_proposal() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    let member = fixtures::second_party(&mut world, b"member", people::Role::Member);
    let number = propose(&mut world, member, 123, 1, Box::new([charter_item(b"waiting for owner")]));
    world.send(root::Event::Core(core::Event::People(people::Event::Roles {
        project: DEPLOYMENT,
        holdings: Box::new([]),
    })));
    decide(&mut world, owner, 124, MEMBER, number, people::ProposalDecision::Accept);
    assert!(matches!(world.people_answers.last(), Some(people::Reply::Outcome(people::Outcome::Refused(_)))));
    assert!(matches!(proposal_state(&world, number), tasks::PersonProposalState::Pending { .. }));
    world.send(root::Event::Core(core::Event::People(people::Event::Roles {
        project: DEPLOYMENT,
        holdings: Box::new([people::Holding { person: OWNER, role: people::Role::Owner }]),
    })));
    decide(&mut world, owner, 125, MEMBER, number, people::ProposalDecision::Accept);
    assert_eq!(settings_document(&mut world, owner).newest, 2);
}

#[test]
fn a_proposal_without_an_owner_is_refused_at_admission() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    world.send(root::Event::Core(core::Event::People(people::Event::Roles {
        project: DEPLOYMENT,
        holdings: Box::new([]),
    })));
    world.send(proposal_event(owner, 126, 1, Box::new([charter_item(b"nobody can accept")])));
    assert!(matches!(
        world.people_answers.last(),
        Some(people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Authority {
            proposable: Some(people::Accepter::Nobody),
            ..
        })))
    ));
    assert!(
        !world
            .store
            .rows
            .values()
            .any(|row| matches!(row, root::Record::Core(core::Record::Tasks(tasks::Stored::PersonProposal(_)))))
    );
}

#[test]
fn an_observers_role_is_enough_to_propose_settings() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    let observer = fixtures::second_party(&mut world, b"observer", people::Role::Observer);
    let number = propose(&mut world, observer, 127, 1, Box::new([charter_item(b"observer suggestion")]));
    let review = proposal_document(&mut world, owner, MEMBER, number);
    assert_eq!(review.project, DEPLOYMENT);
    assert!(matches!(proposal_state(&world, number), tasks::PersonProposalState::Pending { .. }));
}

#[test]
fn review_resolves_reset_removal_and_addition_against_its_revision() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    change(&mut world, owner, 130, 1, Box::new([charter_item(b"stored override")]));
    let mut added = configuration().0.core.defaults.templates[0].clone();
    added.number = 3;
    added.name = b"added template".as_slice().into();
    let added_value = s::Value::Template(Box::new(added));
    let number = propose(
        &mut world,
        owner,
        131,
        2,
        Box::new([
            s::Item { setting: s::Setting::Charter(CHARTER), edit: s::Edit::Reset },
            s::Item { setting: s::Setting::Template(2), edit: s::Edit::Removed },
            s::Item { setting: s::Setting::Template(3), edit: s::Edit::Set(added_value.clone()) },
        ]),
    );
    let doc = proposal_document(&mut world, owner, OWNER, number);
    let core::documents::ProposalAction::Settings { against, review: Some(review), .. } = doc.action else {
        panic!("pending review")
    };
    assert_eq!(against, 2);
    assert_eq!(review.newest, 2);
    assert_eq!(review.items.len(), 3);
    let reset = review.items.iter().find(|item| item.setting == s::Setting::Charter(CHARTER)).expect("reset item");
    let s::Edit::Set(stored) = charter_item(b"stored override").edit else { unreachable!("set fixture") };
    assert_eq!(reset.now, Some(stored));
    assert_eq!(reset.origin.map(|(_, revision)| revision), Some(2));
    assert_eq!(reset.would_be, Some(s::Value::Charter(Box::new(configuration().0.core.defaults.charters[0].clone()))));
    assert_eq!(reset.edit, core::reads::RevisionEdit::Reset);
    let removed = review.items.iter().find(|item| item.setting == s::Setting::Template(2)).expect("removal item");
    assert_eq!(removed.now, Some(s::Value::Template(Box::new(configuration().0.core.defaults.templates[1].clone()))));
    assert_eq!(removed.origin.map(|(_, revision)| revision), Some(1));
    assert_eq!(removed.would_be, None);
    assert_eq!(removed.edit, core::reads::RevisionEdit::Removed);
    let addition = review.items.iter().find(|item| item.setting == s::Setting::Template(3)).expect("addition item");
    assert_eq!(addition.now, None);
    assert_eq!(addition.origin, None);
    assert_eq!(addition.would_be, Some(added_value));
}

#[test]
fn a_stale_proposal_reports_the_newest_revision() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    change(&mut world, owner, 132, 1, Box::new([charter_item(b"revision two")]));
    world.send(proposal_event(owner, 133, 1, Box::new([charter_item(b"obsolete suggestion")])));
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Stale { newest: 2 })))
    );
    assert!(settings_document(&mut world, owner).pending.is_empty());
}

#[test]
#[ignore = "F (mission 3): a task records and moves to its settings revision, engine.md 17.3-17.4; E-QA-1"]
fn acceptance_can_move_tasks_without_changing_a_live_run() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    let assigned = world.assigned.as_ref().expect("live run");
    let (task, attempt) = (assigned.task, assigned.attempt);
    let assignments = world.assignments.len();
    let number = propose(&mut world, owner, 134, 1, Box::new([charter_item(b"the next run")]));
    fixtures::ask_as(
        &mut world,
        owner,
        135,
        people::Ask::DecideProposal {
            project: DEPLOYMENT,
            proposer: people::Proposer::Party(OWNER),
            proposal: number,
            decision: people::ProposalDecision::Accept,
            running: Some(people::Running::Moves),
        },
    );
    assert_eq!(settings_document(&mut world, owner).newest, 2);
    assert!(settings_revision(&world, 2).head.moved, "acceptance records the owner's Moves choice");
    assert_eq!(fixtures::live_row(&world, task).revision, 2);
    let assigned = world.assigned.as_ref().expect("same live run");
    assert_eq!((assigned.task, assigned.attempt), (task, attempt));
    assert_eq!(world.assignments.len(), assignments);
    assert!(world.cancellations.is_empty(), "a move takes effect at the next run");
}

#[test]
fn a_party_without_any_role_cannot_propose_settings() {
    let mut world = world();
    let owner = world.sign_in.expect("owner sign-in");
    let outsider = fixtures::second_party(&mut world, b"outsider", people::Role::Observer);
    world.send(root::Event::Core(core::Event::People(people::Event::Roles {
        project: 1,
        holdings: Box::new([people::Holding { person: OWNER, role: people::Role::Owner }]),
    })));
    world.send(proposal_event(outsider, 136, 1, Box::new([charter_item(b"unauthorized suggestion")])));
    assert!(matches!(
        world.people_answers.last(),
        Some(people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Role { held: None })))
    ));
    assert!(settings_document(&mut world, owner).pending.is_empty());
}

#[test]
#[ignore = "F (mission 3): a task records and moves to its settings revision, engine.md 17.3-17.4; E-QA-1"]
fn acceptance_moves_work_created_after_a_real_first_start() {
    let (config, limits) = configuration();
    let mut world = World::first_start_with_accounts(config, &limits);
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 1000, next: None }));
    world.send(root::Event::Core(core::Event::SignIn {
        held: None,
        reply_to: ReplyTo::new(Token::new(800)),
        identity: people::Identity {
            key: people::IdentityKey { provider: 0, subject: 7_u64.to_be_bytes().into() },
            login: b"owner".as_slice().into(),
            name: b"Owner".as_slice().into(),
        },
    }));
    let owner = world.sign_in.expect("owner sign-in");
    world.reads = Some(reads::Parties::new(world.store.applied));
    let task = world
        .store
        .rows
        .values()
        .find_map(|row| match row {
            root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) => Some(row.number),
            root::Record::Core(_) | root::Record::Connector { .. } => None,
        })
        .expect("work created by the signed-in owner");
    let number = propose(&mut world, owner, 137, 1, Box::new([charter_item(b"next revision")]));
    fixtures::ask_as(
        &mut world,
        owner,
        138,
        people::Ask::DecideProposal {
            project: DEPLOYMENT,
            proposer: people::Proposer::Party(OWNER),
            proposal: number,
            decision: people::ProposalDecision::Accept,
            running: Some(people::Running::Moves),
        },
    );
    assert_eq!(settings_document(&mut world, owner).newest, 2);
    assert!(settings_revision(&world, 2).head.moved, "acceptance records the owner's Moves choice");
    assert_eq!(fixtures::live_row(&world, task).revision, 2);
}

#[test]
fn settings_proposal_profiles_keep_the_advertised_bounds() {
    assert_eq!(jig_protocol_config::profile(jig_protocol_config::Profile::Small).core.settings.proposals, 4);
    assert_eq!(jig_protocol_config::profile(jig_protocol_config::Profile::Standard).core.settings.proposals, 8);
}

fn settings_revision(world: &World, number: u32) -> &s::Revision {
    world
        .store
        .rows
        .values()
        .find_map(|row| match row {
            root::Record::Core(core::Record::Core(core::CoreRecord::Settings(s::Record::Revision {
                revision,
                ..
            }))) if revision.head.number == number => Some(revision),
            root::Record::Core(_) | root::Record::Connector { .. } => None,
        })
        .expect("durable settings revision")
}

// engine.md, 17.1, 17.2 and 17.6: acceptance rechecks the whole set;
// removing an unused model changes no setting and makes no release revision.
fn model_proposal_configuration() -> (root::Config, root::Limits, core::Model) {
    let (mut config, limits) = configuration();
    let mut model = config.core.settings.models[0].clone();
    model.name = b"pending-only".as_slice().into();
    config.core.settings.models = Box::new([config.core.settings.models[0].clone(), model.clone()]);
    config.core.settings.llm_providers.providers[0].models =
        Box::new([b"scripted".as_slice().into(), model.name.clone()]);
    (config, limits, model)
}

fn model_proposal_items(model: core::Model) -> Box<[s::Item]> {
    let mut charter = configuration().0.core.defaults.charters[0].clone();
    charter.policy.model = model;
    charter.policy.instructions = b"proposed model and instructions".as_slice().into();
    let mut template = fixtures::templates()[0].clone();
    template.name = b"proposed chat name".as_slice().into();
    Box::new([
        s::Item {
            setting: s::Setting::Template(template.number),
            edit: s::Edit::Set(s::Value::Template(Box::new(template))),
        },
        s::Item { setting: s::Setting::Charter(CHARTER), edit: s::Edit::Set(s::Value::Charter(Box::new(charter))) },
    ])
}

fn durable_settings(world: &World) -> Vec<root::Record> {
    world
        .store
        .rows
        .values()
        .filter(|row| matches!(row, root::Record::Core(core::Record::Core(core::CoreRecord::Settings(_)))))
        .cloned()
        .collect()
}

fn refused_model_proposal() -> (World, u64, u64) {
    let (config, limits, model) = model_proposal_configuration();
    let mut world = World::configured(SEED, false, config, limits);
    world.reads = Some(reads::Parties::new(world.store.applied));
    let owner = world.sign_in.expect("owner sign-in");
    let number = propose(&mut world, owner, 140, 1, model_proposal_items(model));
    let proposal_before = proposal_document(&mut world, owner, OWNER, number);
    world.restart_with(configuration().0);
    let settings_before = settings_document(&mut world, owner);
    let rows_before = durable_settings(&world);
    let task = world.assigned.as_ref().expect("fixture's live agent task").task;
    let task_before = fixtures::live_row(&world, task).clone();
    let assignments_before = world.assignments.clone();
    let cancellations_before = world.cancellations.clone();
    fixtures::ask_as(
        &mut world,
        owner,
        141,
        people::Ask::DecideProposal {
            project: DEPLOYMENT,
            proposer: people::Proposer::Party(OWNER),
            proposal: number,
            decision: people::ProposalDecision::Accept,
            running: Some(people::Running::Moves),
        },
    );
    let reply = world.people_answers.last().expect("one acceptance answer").clone();
    let proposal_after = proposal_document(&mut world, owner, OWNER, number);
    let settings_after = settings_document(&mut world, owner);
    assert_eq!(proposal_after, proposal_before, "shape refusal preserves the pending review; {reply:?}");
    assert_eq!(settings_after, settings_before, "no item of a refused set becomes visible; {reply:?}");
    assert!(matches!(proposal_state(&world, number), tasks::PersonProposalState::Pending { .. }));
    assert!(
        matches!(
            reply,
            people::Reply::Outcome(people::Outcome::Refused(people::Refusal::NotWellFormed {
                why: people::Malformed::Settings { .. },
                ..
            }))
        ),
        "shape refusal: {reply:?}"
    );
    assert_eq!(durable_settings(&world), rows_before, "no settings row saved or erased on refusal");
    assert_eq!(fixtures::live_row(&world, task), &task_before, "refused Moves does not move the task");
    assert_eq!(world.assignments, assignments_before, "refusal starts no work");
    assert_eq!(world.cancellations, cancellations_before, "refusal cancels no run");
    world.restart_with(configuration().0);
    assert_eq!(proposal_document(&mut world, owner, OWNER, number), proposal_before);
    assert_eq!(settings_document(&mut world, owner), settings_before);
    (world, owner, number)
}

#[test]
fn shape_refusal_keeps_the_pending_set_across_restart() {
    let (world, _, number) = refused_model_proposal();
    assert!(matches!(proposal_state(&world, number), tasks::PersonProposalState::Pending { .. }));
}

#[test]
#[ignore = "F: retained-version capacity remains pending; E-QA-2 retargeted newest-only compatibility and no-holder retry are in ordinary settings_proposals tests"]
fn acceptance_refuses_a_model_removed_since_proposal() {
    let (mut world, owner, number) = refused_model_proposal();
    // Retained failed expectation: restoring the model alone did not make
    // capacity available. The QA report distinguishes this assumption from
    // the shape refusal, whose preservation checks above pass.
    world.restart_with(model_proposal_configuration().0);
    decide(&mut world, owner, 142, OWNER, number, people::ProposalDecision::Accept);
    assert_eq!(settings_document(&mut world, owner).newest, 2, "retry: {:?}", world.people_answers.last());
    assert!(matches!(proposal_state(&world, number), tasks::PersonProposalState::SettingsAccepted { revision: 2, .. }));
}

#[test]
#[ignore = "F: retained-version capacity remains pending; E-QA-2 retargeted newest-only compatibility and no-holder retry are in ordinary settings_proposals tests"]
fn full_acceptance_preserves_the_pending_set_and_holders() {
    // engine.md, 17.2, 17.6 and 17.8: a live run holds its revision
    // even when its task moves. Retained failed capacity assumption: the
    // fixture's single charter slot does not establish a version bound.
    for running in [people::Running::Stays, people::Running::Moves] {
        let mut world = world();
        let owner = world.sign_in.expect("owner sign-in");
        world.restart_with(configuration().0);
        let model = configuration().0.core.settings.models[0].clone();
        let number = propose(&mut world, owner, 150, 1, model_proposal_items(model));
        let proposal_before = proposal_document(&mut world, owner, OWNER, number);
        let settings_before = settings_document(&mut world, owner);
        let rows_before = durable_settings(&world);
        let task = world.assigned.as_ref().expect("live holder").task;
        let task_before = fixtures::live_row(&world, task).clone();
        fixtures::ask_as(
            &mut world,
            owner,
            151,
            people::Ask::DecideProposal {
                project: DEPLOYMENT,
                proposer: people::Proposer::Party(OWNER),
                proposal: number,
                decision: people::ProposalDecision::Accept,
                running: Some(running),
            },
        );
        assert_eq!(
            world.people_answers.last(),
            Some(&people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Full {
                revision: 1,
                tasks: Box::new([task]),
                more: 0,
            }))),
            "full acceptance with {running:?}"
        );
        assert_eq!(proposal_document(&mut world, owner, OWNER, number), proposal_before);
        assert_eq!(settings_document(&mut world, owner), settings_before);
        assert_eq!(durable_settings(&world), rows_before, "full commits no item of the set");
        assert_eq!(fixtures::live_row(&world, task), &task_before, "full does not move a holder");
        assert!(matches!(proposal_state(&world, number), tasks::PersonProposalState::Pending { .. }));
        world.restart_with(configuration().0);
        assert_eq!(proposal_document(&mut world, owner, OWNER, number), proposal_before);
        assert_eq!(settings_document(&mut world, owner), settings_before);
    }
}

#[test]
fn full_model_change_keeps_the_pending_set_across_restart() {
    // engine.md, 17.2, 17.6 and 17.8. Reuse the exact model-changing
    // path observed to return Full, without fabricating a connector reply.
    let (mut world, owner, number) = refused_model_proposal();
    world.restart_with(model_proposal_configuration().0);
    let proposal_before = proposal_document(&mut world, owner, OWNER, number);
    let settings_before = settings_document(&mut world, owner);
    let rows_before = durable_settings(&world);
    let task = world.assigned.as_ref().expect("live holder").task;
    let task_before = fixtures::live_row(&world, task).clone();
    decide(&mut world, owner, 152, OWNER, number, people::ProposalDecision::Accept);
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Full {
            revision: 1,
            tasks: Box::new([task]),
            more: 0,
        })))
    );
    assert_eq!(proposal_document(&mut world, owner, OWNER, number), proposal_before);
    assert_eq!(settings_document(&mut world, owner), settings_before);
    assert_eq!(durable_settings(&world), rows_before, "full commits no item of the set");
    assert_eq!(fixtures::live_row(&world, task), &task_before, "full does not move a holder");
    assert!(matches!(proposal_state(&world, number), tasks::PersonProposalState::Pending { .. }));
    world.restart_with(model_proposal_configuration().0);
    assert_eq!(proposal_document(&mut world, owner, OWNER, number), proposal_before);
    assert_eq!(settings_document(&mut world, owner), settings_before);
}
