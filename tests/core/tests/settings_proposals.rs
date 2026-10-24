//! Party settings proposals through the real store and commit barrier (engine.md, 17.6).
use jig_core_world::{client_peer::ClientPeer, effects::World, fixtures, peers};
use jig_domain_core::{self as core, documents, reads, settings};
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
#[path = "settings/support.rs"]
mod support;
use support::*;

#[test]
fn a_project_member_proposes_and_owners_review_current_and_proposed_values() {
    let mut world = start(64);
    let owner = sign_in(&mut world);
    let (member, signed) = party(&mut world, b"member", Some((1, people::Role::Member)));
    let items = charter_and_template(&world);
    let proposal = proposed(&propose(&mut world, signed, 1, 1, items));
    let document = proposal_document(&mut world, owner, member, proposal);
    let documents::ProposalAction::Settings { against, items, review: Some(review), made } = document.action else {
        panic!("reviewed settings")
    };
    assert_eq!((against, review.newest, made), (1, 1, None));
    assert_eq!(items.len(), 2);
    assert_eq!(review.items.len(), 2);
    let charter = review.items.get(0).expect("charter review");
    assert_eq!(charter.setting, settings::Setting::Charter(1));
    assert_eq!(charter.origin, Some((false, 1)));
    let Some(settings::Value::Charter(now)) = &charter.now else { panic!("current charter") };
    assert_eq!(now.policy.instructions.as_ref(), b"Answer within the granted authority.");
    let Some(settings::Value::Charter(after)) = &charter.would_be else { panic!("proposed charter") };
    assert_eq!(after.policy.instructions.as_ref(), b"Changed by review");
    let template = review.items.get(1).expect("template review");
    assert_eq!(template.edit, reads::RevisionEdit::Removed);
    assert!(template.now.is_some());
    assert!(template.would_be.is_none());
    let reads::Answer::Page(inbox) = read(
        &mut world,
        owner,
        reads::Read::Inbox {
            group: reads::Group::Proposals,
            filter: reads::Filter { project: None, goal: None },
            from: None,
        },
    ) else {
        panic!("owner inbox")
    };
    assert!(inbox.rows.iter().any(|row| matches!(row, reads::PageRow::Inbox(entry) if entry.object == documents::InboxObject::Proposal { proposer: people::Proposer::Party(member), number: proposal })));
    assert_eq!(frame(&mut world, owner).need_counts.proposals, 1);
    assert_eq!(proposal_document(&mut world, signed, member, proposal).number, proposal);
}
#[test]
fn accepting_settings_records_the_proposal_accepter_and_revision_atomically() {
    let mut world = start(64);
    let owner = sign_in(&mut world);
    let edit = template_edit(&world, 7);
    let proposal = proposed(&propose(&mut world, owner, 1, 1, Box::new([edit])));
    assert!(matches!(
        decide(&mut world, owner, 2, OWNER, proposal, people::ProposalDecision::Accept, Some(people::Running::Stays)),
        people::Reply::Outcome(people::Outcome::ProposalDecided { choice: people::ProposalChoice::Accepted, .. })
    ));
    let head = &revision(&world, 2, false).head;
    assert_eq!(
        head.proposal,
        Some((people::Proposer::Party(OWNER), u32::try_from(proposal).expect("proposal fits the settings key")))
    );
    assert_eq!(head.by, settings::Decider::Party(OWNER));
    assert!(!head.moved);
    assert!(!world.store.rows.contains_key(&proposal_set_key(OWNER, proposal)));
    assert_eq!(
        frame(&mut world, owner).projects.get(0).expect("project").templates.get(0).expect("chat template").grace,
        Some(skein_lib::Duration::from_secs(7))
    );
    let reads::Answer::Historical(history) =
        read(&mut world, owner, reads::Read::Proposal { proposer: people::Proposer::Party(OWNER), proposal })
    else {
        panic!("accepted history")
    };
    let reads::Past::PersonProposal { proposal: row, .. } = history.value else { panic!("party history") };
    assert_eq!(
        row.state,
        tasks::PersonProposalState::SettingsAccepted { by: tasks::Party::Person(OWNER), revision: 2 }
    );
}
#[test]
fn rejected_settings_keep_the_reason_and_notify_the_project_member() {
    let mut world = start(64);
    let owner = sign_in(&mut world);
    let (member, signed) = party(&mut world, b"member", Some((1, people::Role::Member)));
    let item = template_edit(&world, 7);
    let proposal = proposed(&propose(&mut world, signed, 1, 1, Box::new([item])));
    decide(
        &mut world,
        owner,
        1,
        member,
        proposal,
        people::ProposalDecision::Reject { reason: b"Keep the current grace".as_slice().into() },
        None,
    );
    let reads::Answer::Historical(history) =
        read(&mut world, signed, reads::Read::Proposal { proposer: people::Proposer::Party(member), proposal })
    else {
        panic!("rejection history")
    };
    let reads::Past::PersonProposal { proposal: row, .. } = history.value else { panic!("party history") };
    assert!(
        matches!(&row.state, tasks::PersonProposalState::Rejected { reason, .. } if reason.as_ref() == b"Keep the current grace")
    );
    assert!(updates(&mut world, signed).iter().any(|entry| entry.object
        == documents::InboxObject::Proposal { proposer: people::Proposer::Party(member), number: proposal }));
    assert!(!world.store.rows.contains_key(&proposal_set_key(member, proposal)));
    let item = template_edit(&world, 8);
    let withdrawn = proposed(&propose(&mut world, signed, 2, 1, Box::new([item])));
    assert!(matches!(
        ask(&mut world, signed, 3, people::Ask::Withdraw { project: 0, proposal: withdrawn }),
        people::Reply::Outcome(people::Outcome::Withdrawn { proposal }) if proposal == withdrawn
    ));
    assert!(!world.store.rows.contains_key(&proposal_set_key(member, withdrawn)));
    assert!(updates(&mut world, signed).iter().any(|entry| entry.object
        == documents::InboxObject::Proposal { proposer: people::Proposer::Party(member), number: withdrawn }));
}
#[test]
fn an_acceptance_refused_as_full_keeps_the_proposal_and_set_pending() {
    let mut world = start(64);
    let owner = sign_in(&mut world);
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 100, next: None }));
    let chat = fixtures::chat(&world, 1, b"live task".as_slice().into());
    let people::Reply::Outcome(people::Outcome::Created { first, .. }) = ask(&mut world, owner, 1, chat) else {
        panic!("live agent")
    };
    let items = removing_charter(&world);
    let proposal = proposed(&propose(&mut world, owner, 2, 1, items));
    assert_eq!(
        decide(&mut world, owner, 3, OWNER, proposal, people::ProposalDecision::Accept, Some(people::Running::Stays)),
        people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Full {
            revision: 1,
            tasks: Box::new([first]),
            more: 0
        }))
    );
    assert!(world.store.rows.contains_key(&proposal_set_key(OWNER, proposal)));
    assert_eq!(inventory(&mut world, owner).pending.len(), 1);
    assert!(matches!(
        proposal_document(&mut world, owner, OWNER, proposal).action,
        documents::ProposalAction::Settings { review: Some(_), made: None, .. }
    ));
}
#[test]
fn accepting_or_changing_settings_withdraws_stale_proposals_and_proposers_hear() {
    let mut world = start(64);
    let owner = sign_in(&mut world);
    let (member, signed) = party(&mut world, b"member", Some((1, people::Role::Member)));
    let first = template_edit(&world, 7);
    let second = template_edit(&world, 8);
    let accepted = proposed(&propose(&mut world, owner, 1, 1, Box::new([first])));
    let stale = proposed(&propose(&mut world, signed, 1, 1, Box::new([second])));
    decide(&mut world, owner, 2, OWNER, accepted, people::ProposalDecision::Accept, Some(people::Running::Stays));
    let entries = updates(&mut world, signed);
    assert!(entries.iter().any(|entry| entry.object
        == documents::InboxObject::Proposal { proposer: people::Proposer::Party(member), number: stale }));
    let reads::Answer::Historical(history) =
        read(&mut world, signed, reads::Read::Proposal { proposer: people::Proposer::Party(member), proposal: stale })
    else {
        panic!("withdrawn history")
    };
    let reads::Past::PersonProposal { proposal: row, decision: Some(decision) } = history.value else {
        panic!("withdrawn party proposal")
    };
    assert!(matches!(row.state, tasks::PersonProposalState::Withdrawn { .. }));
    assert_eq!(decision.choice, people::ProposalChoice::Withdrawn);
    let again = template_edit(&world, 9);
    let stale = proposed(&propose(&mut world, signed, 2, 2, Box::new([again])));
    let direct = template_edit(&world, 10);
    changed(&change(&mut world, owner, 3, 2, Box::new([direct])), 3);
    assert!(updates(&mut world, signed).iter().any(|entry| entry.object
        == documents::InboxObject::Proposal { proposer: people::Proposer::Party(member), number: stale }));
    assert!(inventory(&mut world, owner).pending.is_empty());
}
#[test]
fn pending_reviews_survive_restart_and_a_release_withdraws_them() {
    let mut world = start(64);
    let owner = sign_in(&mut world);
    let edit = template_edit(&world, 7);
    let proposal = proposed(&propose(&mut world, owner, 1, 1, Box::new([edit])));
    let before = proposal_document(&mut world, owner, OWNER, proposal).action;
    restart(&mut world, configuration().0);
    let owner = sign_in(&mut world);
    assert_eq!(proposal_document(&mut world, owner, OWNER, proposal).action, before);
    let mut config = configuration().0;
    config.core.defaults.charters[0].policy.instructions = b"A new release default".as_slice().into();
    restart(&mut world, config);
    let owner = sign_in(&mut world);
    assert_eq!(revision(&world, 2, false).head.by, settings::Decider::Release);
    assert!(!world.store.rows.contains_key(&proposal_set_key(OWNER, proposal)));
    assert!(inventory(&mut world, owner).pending.is_empty());
    assert!(updates(&mut world, owner).iter().any(|entry| entry.object
        == documents::InboxObject::Proposal { proposer: people::Proposer::Party(OWNER), number: proposal }));
    for erase_set in [true, false] {
        let mut inconsistent = start(64);
        let owner = sign_in(&mut inconsistent);
        let item = template_edit(&inconsistent, 7);
        let proposal = proposed(&propose(&mut inconsistent, owner, 1, 1, Box::new([item])));
        let missing = if erase_set {
            proposal_set_key(OWNER, proposal)
        } else {
            root::Key::Core(core::Key::Tasks(tasks::Key::PersonProposal(proposal)))
        };
        inconsistent.erase(missing).expect("one side of the durable pair");
        inconsistent.restart_with(configuration().0);
        assert!(!inconsistent.domain.ready(), "missing set: {erase_set}");
        assert!(inconsistent.stopped, "inconsistent pending settings must refuse startup");
    }
}
#[test]
fn the_pending_settings_bound_admits_two_and_names_the_thirds_refusal() {
    const PENDING: u32 = 2;
    let (config, mut limits) = configuration();
    limits.core.settings.proposals = PENDING;
    peers::journal(&mut limits);
    let mut world = World::first_start_with_accounts(config, &limits);
    world.client_peer = Some(ClientPeer::default());
    let owner = sign_in(&mut world);
    let (_, first) = party(&mut world, b"first", Some((1, people::Role::Member)));
    let (_, second) = party(&mut world, b"second", Some((1, people::Role::Member)));
    let item = template_edit(&world, 7);
    proposed(&propose(&mut world, owner, 1, 1, Box::new([item.clone()])));
    proposed(&propose(&mut world, first, 1, 1, Box::new([item.clone()])));
    assert_eq!(inventory(&mut world, owner).pending.len(), PENDING);
    assert_eq!(
        propose(&mut world, second, 1, 1, Box::new([item])),
        people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Limit {
            which: people::LimitKind::SettingsProposals
        }))
    );
}
#[test]
fn entrance_refusals_keep_nothing_and_only_deployment_owners_accept() {
    let mut world = start(64);
    let owner = sign_in(&mut world);
    let (project_owner, signed) = party(&mut world, b"project owner", Some((1, people::Role::Owner)));
    let (_, outsider) = party(&mut world, b"outsider", None);
    let before = settings_rows(&world);
    let item = template_edit(&world, 7);
    let cases: [(u8, u32, Box<[settings::Item]>, people::Refusal); 3] = [
        (1, 0, Box::new([item.clone()]), people::Refusal::Stale { newest: 1 }),
        (
            2,
            1,
            Box::new([settings::Item { setting: settings::Setting::Rules, edit: settings::Edit::Reset }]),
            people::Refusal::NotWellFormed { at: 0, why: people::Malformed::SettingsUnchanged },
        ),
        (
            3,
            1,
            Box::new([settings::Item { setting: settings::Setting::Rules, edit: settings::Edit::Removed }]),
            people::Refusal::NotWellFormed {
                at: 0,
                why: people::Malformed::Settings { setting: settings::Setting::Rules, fault: settings::Fault::Shape },
            },
        ),
    ];
    for (key, against, items, why) in cases {
        assert_eq!(
            propose(&mut world, owner, key, against, items),
            people::Reply::Outcome(people::Outcome::Refused(why))
        );
    }
    assert_eq!(
        propose(&mut world, outsider, 1, 1, Box::new([item.clone()])),
        people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Role { held: None }))
    );
    assert_eq!(settings_rows(&world), before);
    let proposal = proposed(&propose(&mut world, signed, 1, 1, Box::new([item])));
    assert_eq!(
        decide(
            &mut world,
            signed,
            2,
            project_owner,
            proposal,
            people::ProposalDecision::Accept,
            Some(people::Running::Stays)
        ),
        people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Role { held: None }))
    );
    assert_eq!(
        decide(&mut world, owner, 4, project_owner, proposal, people::ProposalDecision::Accept, None),
        people::Reply::Outcome(people::Outcome::Refused(people::Refusal::NotWellFormed {
            at: 0,
            why: people::Malformed::Running
        }))
    );
    assert_eq!(inventory(&mut world, owner).pending.len(), 1);
}

#[test]
fn settings_proposal_reads_hide_pending_and_retained_values_from_project_only_parties() {
    let mut world = start(64);
    let owner = sign_in(&mut world);
    let (member, signed) = party(&mut world, b"proposer", Some((1, people::Role::Member)));
    let (_, outsider) = party(&mut world, b"project-owner", Some((1, people::Role::Owner)));
    let item = template_edit(&world, 7);
    let proposal = proposed(&propose(&mut world, signed, 1, 1, Box::new([item])));
    let point = reads::Read::Proposal { proposer: people::Proposer::Party(member), proposal };
    assert_eq!(read(&mut world, outsider, point.clone()), reads::Answer::None(reads::Why::Unknown));
    decide(
        &mut world,
        owner,
        1,
        member,
        proposal,
        people::ProposalDecision::Reject { reason: b"Keep the default".as_slice().into() },
        None,
    );
    assert_eq!(read(&mut world, outsider, point.clone()), reads::Answer::None(reads::Why::Unknown));
    assert!(matches!(read(&mut world, owner, point.clone()), reads::Answer::Historical(_)));
    assert!(matches!(read(&mut world, signed, point.clone()), reads::Answer::Historical(_)));
    assert_eq!(
        decide(&mut world, owner, 2, member, proposal, people::ProposalDecision::Accept, None),
        people::Reply::Outcome(people::Outcome::Refused(people::Refusal::NotWellFormed {
            at: 0,
            why: people::Malformed::Running
        }))
    );
    restart(&mut world, configuration().0);
    let (_, outsider) = party(&mut world, b"project-owner", None);
    assert_eq!(read(&mut world, outsider, point), reads::Answer::None(reads::Why::Unknown));
}

#[test]
fn ordinary_party_proposal_history_stays_private_to_its_proposer() {
    let mut world = start(64);
    let owner = sign_in(&mut world);
    let (_, other) = party(&mut world, b"other-owner", Some((1, people::Role::Owner)));
    let people::Ask::Create { project, mut batch } = fixtures::chat(&world, 1, b"proposed chat".as_slice().into())
    else {
        panic!("chat batch")
    };
    batch[0].budget = people::Budget::Named;
    batch[0].authority.spend = 20;
    let people::Reply::Outcome(people::Outcome::Proposed { proposal, .. }) = ask(
        &mut world,
        owner,
        1,
        people::Ask::Propose { project, action: people::Action::Batch(batch), reason: b"Review".as_slice().into() },
    ) else {
        panic!("ordinary proposal")
    };
    ask(
        &mut world,
        owner,
        2,
        people::Ask::DecideProposal {
            project,
            proposer: people::Proposer::Party(OWNER),
            proposal,
            decision: people::ProposalDecision::Reject { reason: b"Keep it".as_slice().into() },
            running: None,
        },
    );
    let point = reads::Read::Proposal { proposer: people::Proposer::Party(OWNER), proposal };
    assert_eq!(read(&mut world, other, point.clone()), reads::Answer::None(reads::Why::Unknown));
    assert!(matches!(read(&mut world, owner, point), reads::Answer::Historical(_)));
    assert_eq!(
        ask(
            &mut world,
            owner,
            3,
            people::Ask::DecideProposal {
                project,
                proposer: people::Proposer::Party(OWNER),
                proposal,
                decision: people::ProposalDecision::Accept,
                running: Some(people::Running::Stays),
            }
        ),
        people::Reply::Outcome(people::Outcome::Refused(people::Refusal::NotWellFormed {
            at: 0,
            why: people::Malformed::Running
        }))
    );
}
