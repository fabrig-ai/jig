//! Party and run settings proposals through the real store and commit barrier (engine.md, 17.6).
use jig_core_world::{client_peer::ClientPeer, effects::World, fixtures, peers};
use jig_domain_core::{self as core, documents, reads, settings};
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use std::fmt::Write as _;
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
    for running in [people::Running::Stays, people::Running::Moves] {
        let (config, limits) = full_version_configuration();
        let mut world = World::first_start_with_accounts(config, &limits);
        world.client_peer = Some(ClientPeer::default());
        let owner = sign_in(&mut world);
        world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 100, next: None }));
        let chat = fixtures::chat(&world, 1, b"live task".as_slice().into());
        let people::Reply::Outcome(people::Outcome::Created { first, .. }) = ask(&mut world, owner, 1, chat) else {
            panic!("live agent")
        };
        assert!(matches!(fixtures::live_row(&world, first).executor, tasks::Executor::Agent { charter: 1 }));
        let mut items = removing_charter(&world).into_vec();
        items.push(settings::Item { setting: settings::Setting::Charter(2), edit: settings::Edit::Removed });
        assert!(
            items
                .iter()
                .any(|item| item.setting == settings::Setting::Charter(1) && item.edit == settings::Edit::Removed)
        );
        let proposal = proposed(&propose(&mut world, owner, 2, 1, items.into_boxed_slice()));
        for request in [3, 4] {
            let before_rows = world.store.rows.clone();
            let before_task = fixtures::live_row(&world, first).clone();
            let before_settings = settings_rows(&world);
            let before_review = proposal_document(&mut world, owner, OWNER, proposal).action;
            let assignments = world.assignments.clone();
            let cancellations = world.cancellations.clone();
            assert_eq!(
                decide(&mut world, owner, request, OWNER, proposal, people::ProposalDecision::Accept, Some(running)),
                people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Full {
                    revision: 1,
                    tasks: Box::new([first]),
                    more: 0
                }))
            );
            for (key, row) in &before_rows {
                let mut expected = row.clone();
                if let root::Record::Core(core::Record::Core(core::CoreRecord::Deployment(header))) = &mut expected {
                    header.commits += 1; // The normal keyed refusal itself commits once.
                }
                assert_eq!(world.store.rows.get(key), Some(&expected), "refusal changed durable row {key:?}");
            }
            let added: Vec<_> = world.store.rows.keys().filter(|key| !before_rows.contains_key(*key)).collect();
            assert_eq!(added.len(), 1, "only the keyed refusal is added");
            assert!(matches!(added[0], root::Key::Core(core::Key::People(people::Key::Answer(_)))));
            assert_eq!(settings_rows(&world), before_settings);
            assert_eq!(fixtures::live_row(&world, first), &before_task);
            assert_eq!(world.assignments, assignments);
            assert_eq!(world.cancellations, cancellations);
            assert_eq!(proposal_document(&mut world, owner, OWNER, proposal).action, before_review);
            assert!(world.store.rows.contains_key(&proposal_set_key(OWNER, proposal)));
            assert_eq!(inventory(&mut world, owner).pending.len(), 1);
            restart(&mut world, full_version_configuration().0);
            assert_eq!(proposal_document(&mut world, owner, OWNER, proposal).action, before_review);
        }
    }
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

#[test]
fn a_shape_refused_proposal_can_retry_after_its_unused_model_returns_without_a_holder() {
    let (mut config, limits) = configuration();
    let mut model = config.core.settings.models[0].clone();
    model.name = b"pending-only".as_slice().into();
    config.core.settings.models = Box::new([config.core.settings.models[0].clone(), model.clone()]);
    config.core.settings.llm_providers.providers[0].models =
        Box::new([b"scripted".as_slice().into(), model.name.clone()]);
    let mut world = World::first_start_with_accounts(config, &limits);
    world.client_peer = Some(ClientPeer::default());
    let owner = sign_in(&mut world);
    assert!(world.assignments.is_empty(), "positive retry has no live holder");
    let mut charter = world.domain.settings().charter(1).expect("fixture charter").clone();
    charter.policy.model = model.clone();
    let items = Box::new([settings::Item {
        setting: settings::Setting::Charter(1),
        edit: settings::Edit::Set(settings::Value::Charter(Box::new(charter))),
    }]);
    let proposal = proposed(&propose(&mut world, owner, 1, 1, items));
    restart(&mut world, configuration().0);
    let before = proposal_document(&mut world, owner, OWNER, proposal).action;
    let rows = settings_rows(&world);
    assert!(matches!(
        decide(&mut world, owner, 2, OWNER, proposal, people::ProposalDecision::Accept, Some(people::Running::Stays)),
        people::Reply::Outcome(people::Outcome::Refused(people::Refusal::NotWellFormed {
            why: people::Malformed::Settings { .. },
            ..
        }))
    ));
    assert_eq!(settings_rows(&world), rows);
    assert_eq!(proposal_document(&mut world, owner, OWNER, proposal).action, before);
    let (mut restored, _) = configuration();
    restored.core.settings.models = Box::new([restored.core.settings.models[0].clone(), model.clone()]);
    restored.core.settings.llm_providers.providers[0].models =
        Box::new([b"scripted".as_slice().into(), model.name.clone()]);
    restart(&mut world, restored);
    assert!(matches!(
        decide(&mut world, owner, 3, OWNER, proposal, people::ProposalDecision::Accept, Some(people::Running::Stays)),
        people::Reply::Outcome(people::Outcome::ProposalDecided { choice: people::ProposalChoice::Accepted, .. })
    ));
    assert_eq!(world.domain.settings().newest(), 2);
    assert!(!world.store.rows.contains_key(&proposal_set_key(OWNER, proposal)));
}

fn full_version_configuration() -> (root::Config, root::Limits) {
    let (mut config, mut limits) = configuration();
    let mut second = config.core.defaults.charters[0].clone();
    second.number = 2;
    config.core.defaults.charters = Box::new([config.core.defaults.charters[0].clone(), second]);
    limits.core.settings.versions = 1;
    peers::journal(&mut limits);
    (config, limits)
}

fn run_propose(world: &mut World, against: u32, items: Box<[settings::Item]>) -> core::CallPart {
    let (config, limits) = run_configuration();
    let models = settings::model_names(&config.core.settings.models);
    let bytes = jig_protocol_store::codec::settings::encode(
        &settings::Record::Proposal {
            proposer: people::Proposer::Task(1),
            proposal: 1,
            value: settings::Proposal { against, items },
        },
        &jig_protocol_store::codec::settings::limits(&limits.core).expect("settings codec limits"),
        &models,
    )
    .expect("encoded whole settings set");
    let mut encoded = String::new();
    for byte in &bytes {
        write!(&mut encoded, "{byte:02x}").expect("hex string");
    }
    let input = format!("{{\"against\":{against},\"set\":\"{encoded}\",\"reason\":\"Please review these settings\"}}");
    world.tool_call(b"propose_settings", input.as_bytes(), true);
    world
        .answers
        .last()
        .unwrap_or_else(|| {
            panic!(
                "typed tool answer; input bytes {}; host answers {:?}; host calls {:?}; busy {:?}",
                input.len(),
                world.host_answers,
                world.host_calls,
                world.busy_calls
            )
        })
        .2
        .clone()
}
fn run_proposed(part: &core::CallPart) -> u64 {
    let core::CallPart::Proposed { proposal, holder } = part else { panic!("proposed: {part:?}") };
    assert_eq!(
        *holder,
        tasks::ProposalHolder::Policy { project: people::DEPLOYMENT, kind: tasks::ProposalKind::Settings }
    );
    *proposal
}
fn run_decide(
    world: &mut World,
    signed: u64,
    task: u64,
    proposal: u64,
    decision: people::ProposalDecision,
    running: Option<people::Running>,
) -> people::Reply {
    ask(
        world,
        signed,
        102,
        people::Ask::DecideProposal {
            project: people::DEPLOYMENT,
            proposer: people::Proposer::Task(task),
            proposal,
            decision,
            running,
        },
    )
}
fn task_set(task: u64, proposal: u64) -> root::Key {
    key(settings::Key::Proposal {
        proposer: people::Proposer::Task(task),
        proposal: u32::try_from(proposal).expect("proposal key"),
    })
}
fn run_outcome(world: &World, task: u64, proposal: u64) -> &tasks::Word {
    fixtures::live_row(world, task).inbox.iter().find(|word| matches!(word.kind, tasks::MessageKind::SettingsDecision { proposal: number, .. } if number == proposal)).expect("settings outcome in the proposer's inbox")
}
fn accepted(reply: &people::Reply) {
    assert!(
        matches!(
            reply,
            people::Reply::Outcome(people::Outcome::ProposalDecided { choice: people::ProposalChoice::Accepted, .. })
        ),
        "{reply:?}"
    );
}
#[test]
fn a_run_proposes_settings_and_an_owner_accepts_moving_its_work() {
    let (mut world, signed, task) = proposing_run(true);
    let item = run_charter(&world, b"Reviewed instructions");
    let proposal = run_proposed(&run_propose(&mut world, 1, Box::new([item])));
    assert!(world.store.rows.contains_key(&task_set(task, proposal)));
    let before_replay = settings_rows(&world);
    let key = world.answers.last().expect("typed answer").1;
    world.send(root::Event::Core(core::Event::NamedAction {
        to: skein_lib::ReplyTo::new(skein_lib::Token::new(39_001)),
        key,
        action: core::NamedAction::ProposeSettings {
            against: 1,
            items: Box::new([run_charter(&world, b"Replay cannot replace this set")]),
            reason: b"again".as_slice().into(),
        },
    }));
    assert_eq!(run_proposed(&world.answers.last().expect("typed answer").2), proposal);
    assert_eq!(settings_rows(&world), before_replay);
    let reads::Answer::Document(document) =
        read(&mut world, signed, reads::Read::Proposal { proposer: people::Proposer::Task(task), proposal })
    else {
        panic!("proposal document")
    };
    let documents::Document::Proposal(document) = *document else { panic!("proposal review") };
    let documents::ProposalAction::Settings { against, review: Some(review), .. } = document.action else {
        panic!("settings review")
    };
    assert_eq!((against, review.newest), (1, 1));
    let reviewed = review.items.get(0).expect("reviewed charter");
    assert_eq!(reviewed.origin, Some((false, 1)));
    let Some(settings::Value::Charter(now)) = &reviewed.now else { panic!("current value") };
    let Some(settings::Value::Charter(after)) = &reviewed.would_be else { panic!("proposed value") };
    assert_eq!(now.policy.instructions.as_ref(), b"Answer within the granted authority.");
    assert_eq!(after.policy.instructions.as_ref(), b"Reviewed instructions");
    let reads::Answer::Page(inbox) = read(
        &mut world,
        signed,
        reads::Read::Inbox {
            group: reads::Group::Proposals,
            filter: reads::Filter { project: None, goal: None },
            from: None,
        },
    ) else {
        panic!("owner inbox")
    };
    assert!(inbox.rows.iter().any(|row| matches!(row, reads::PageRow::Inbox(entry) if entry.object == documents::InboxObject::Proposal { proposer: people::Proposer::Task(task), number: proposal })));
    accepted(&run_decide(
        &mut world,
        signed,
        task,
        proposal,
        people::ProposalDecision::Accept,
        Some(people::Running::Moves),
    ));
    let head = &revision(&world, 2, false).head;
    assert_eq!(head.proposal, Some((people::Proposer::Task(task), u32::try_from(proposal).expect("proposal key"))));
    assert_eq!(head.by, settings::Decider::Party(OWNER));
    assert!(head.moved);
    assert_eq!(fixtures::live_row(&world, task).settings.task, 2);
    assert_eq!(
        run_outcome(&world, task, proposal).kind,
        tasks::MessageKind::SettingsDecision { proposal, outcome: tasks::SettingsOutcome::Accepted { revision: 2 } }
    );
    assert!(!world.store.rows.contains_key(&task_set(task, proposal)));
}
#[test]
fn a_runs_settings_proposal_rejected_reaches_its_inbox_with_the_reason() {
    let (mut world, signed, task) = proposing_run(true);
    let item = run_charter(&world, b"Later instructions");
    let proposal = run_proposed(&run_propose(&mut world, 1, Box::new([item])));
    let reason = b"Keep the current instructions";
    let reply = run_decide(
        &mut world,
        signed,
        task,
        proposal,
        people::ProposalDecision::Reject { reason: reason.as_slice().into() },
        None,
    );
    assert!(
        matches!(
            reply,
            people::Reply::Outcome(people::Outcome::ProposalDecided { choice: people::ProposalChoice::Rejected, .. })
        ),
        "{reply:?}"
    );
    let outcome = run_outcome(&world, task, proposal);
    assert_eq!(
        outcome.kind,
        tasks::MessageKind::SettingsDecision { proposal, outcome: tasks::SettingsOutcome::Rejected }
    );
    assert_eq!(outcome.words.as_ref(), reason);
    assert!(!world.store.rows.contains_key(&task_set(task, proposal)));
    assert_eq!(world.domain.settings().newest(), 1);
}
#[test]
fn a_runs_stale_settings_proposal_is_withdrawn_and_it_hears_the_newest() {
    let (mut world, signed, task) = proposing_run(true);
    let item = run_charter(&world, b"Proposed instructions");
    let proposal = run_proposed(&run_propose(&mut world, 1, Box::new([item])));
    let direct = run_charter(&world, b"Direct owner instructions");
    changed(&change(&mut world, signed, 103, 1, Box::new([direct])), 2);
    assert_eq!(
        run_outcome(&world, task, proposal).kind,
        tasks::MessageKind::SettingsDecision { proposal, outcome: tasks::SettingsOutcome::Stale { newest: 2 } }
    );
    assert!(!world.store.rows.contains_key(&task_set(task, proposal)));
    assert!(fixtures::live_row(&world, task).proposal.is_none());
    let item = run_charter(&world, b"Proposed again against newest");
    let next = run_proposed(&run_propose(&mut world, 2, Box::new([item])));
    assert_ne!(next, proposal);
    assert!(world.store.rows.contains_key(&task_set(task, next)));
}
#[test]
fn a_run_without_the_settings_family_is_denied_and_may_propose_nothing() {
    let (mut world, _, task) = proposing_run(false);
    let before = settings_rows(&world);
    let item = run_charter(&world, b"Unauthorized proposal");
    assert!(matches!(run_propose(&mut world, 1, Box::new([item])), core::CallPart::ToolDenied { .. }));
    assert_eq!(settings_rows(&world), before);
    assert!(fixtures::live_row(&world, task).proposal.is_none());
}
#[test]
fn a_runs_settings_proposal_refused_at_the_entrance_keeps_nothing() {
    let (mut world, signed, task) = proposing_run(true);
    let before = settings_rows(&world);
    let item = run_charter(&world, b"Stale basis");
    assert_eq!(
        run_propose(&mut world, 0, Box::new([item])),
        core::CallPart::SettingsRefused(people::Refusal::Stale { newest: 1 })
    );
    assert_eq!(settings_rows(&world), before);
    let mut item = run_charter(&world, b"Malformed proposal");
    let settings::Edit::Set(settings::Value::Charter(charter)) = &mut item.edit else { panic!("charter") };
    charter.number = 0;
    assert!(matches!(
        run_propose(&mut world, 1, Box::new([item])),
        core::CallPart::SettingsRefused(people::Refusal::NotWellFormed { .. })
    ));
    assert_eq!(settings_rows(&world), before);
    let first = run_charter(&world, b"First pending");
    proposed(&propose(&mut world, signed, 104, 1, Box::new([first])));
    let (_, member) = party(&mut world, b"another proposer", Some((1, people::Role::Member)));
    let second = run_charter(&world, b"Second pending");
    proposed(&propose(&mut world, member, 105, 1, Box::new([second])));
    let before = settings_rows(&world);
    let item = run_charter(&world, b"Past the limit");
    assert_eq!(
        run_propose(&mut world, 1, Box::new([item])),
        core::CallPart::SettingsRefused(people::Refusal::Limit { which: people::LimitKind::SettingsProposals })
    );
    assert_eq!(settings_rows(&world), before);
    assert!(fixtures::live_row(&world, task).proposal.is_none());
}
#[test]
fn a_runs_pending_settings_proposal_survives_a_restart_and_is_accepted() {
    let (mut world, signed, task) = proposing_run(true);
    let item = run_charter(&world, b"Durable proposed instructions");
    let proposal = run_proposed(&run_propose(&mut world, 1, Box::new([item])));
    restart(&mut world, run_configuration().0);
    assert!(world.store.rows.contains_key(&task_set(task, proposal)));
    accepted(&run_decide(
        &mut world,
        signed,
        task,
        proposal,
        people::ProposalDecision::Accept,
        Some(people::Running::Stays),
    ));
    assert_eq!(
        run_outcome(&world, task, proposal).kind,
        tasks::MessageKind::SettingsDecision { proposal, outcome: tasks::SettingsOutcome::Accepted { revision: 2 } }
    );
    assert!(!world.store.rows.contains_key(&task_set(task, proposal)));
    restart(&mut world, run_configuration().0);
    assert_eq!(
        revision(&world, 2, false).head.proposal,
        Some((people::Proposer::Task(task), u32::try_from(proposal).expect("proposal key")))
    );
    assert_eq!(fixtures::live_row(&world, task).inbox.iter().filter(|word| matches!(word.kind, tasks::MessageKind::SettingsDecision { proposal: number, .. } if number == proposal)).count(), 1);
}

#[test]
fn exhausted_stale_outcome_numbers_refuse_before_changing_settings() {
    let (mut world, signed, task) = proposing_run(true);
    let item = run_charter(&world, b"Pending instructions");
    let proposal = run_proposed(&run_propose(&mut world, 1, Box::new([item])));
    let header_key = root::Key::Core(core::Key::Core(core::CoreKey::Deployment));
    let mut header = world.store.rows[&header_key].clone();
    let root::Record::Core(core::Record::Core(core::CoreRecord::Deployment(numbers))) = &mut header else {
        panic!("deployment header")
    };
    numbers.messages = u64::MAX;
    let env = skein_lib::Env { now: skein_lib::Time::ZERO, wall: world.wall_time(), limits: run_configuration().1 };
    assert!(root::restore_record(&mut world.domain, &env, header));
    let before = settings_rows(&world);
    let direct = run_charter(&world, b"Direct change");
    assert_eq!(
        change(&mut world, signed, 106, 1, Box::new([direct])),
        people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Limit {
            which: people::LimitKind::Arithmetic
        }))
    );
    assert_eq!(settings_rows(&world), before);
    assert!(fixtures::live_row(&world, task).proposal.is_some());
    assert!(world.store.rows.contains_key(&task_set(task, proposal)));
    let (mut changed, _) = run_configuration();
    changed.core.defaults.charters[0].policy.instructions = b"Release default".as_slice().into();
    world.restart_with(changed);
    assert!(!world.domain.ready(), "startup also refuses an unnumberable stale outcome");
    assert!(world.stopped);
    assert_eq!(settings_rows(&world), before);
}
