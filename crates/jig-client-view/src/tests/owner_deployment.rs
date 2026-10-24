#![expect(
    clippy::disallowed_methods,
    clippy::disallowed_macros,
    clippy::too_many_lines,
    clippy::disallowed_types,
    clippy::wildcard_enum_match_arm,
    reason = "ordinary proofs inspect complete trees and select public requests"
)]
//! Native Deployment view proofs; no source translation or connector completion is inferred.
use super::cards::{CLIENT, contains, document, drive, shared_words, start};
use crate::owner_deployment_words::DeploymentPhrase as P;
use crate::tests::fixtures::{Fixture, Watched};
use crate::words::Words;
use crate::{JigBindings, owner_deployment as view};
use skein_ui::phrases::{Invalid, PhraseLimits, Phrasebook, Segment};
use skein_ui::{Builder, Role, Tree};

use crate::{
    Event, Request,
    address::{Address, Place},
};
use jig_client::contract::*;
use skein_lib::{Duration, List, Wall, bytes::copy_of};
fn list<T>(values: Vec<T>) -> List<T> {
    let mut list = List::with_capacity(u32::try_from(values.len()).unwrap());
    for value in values {
        list.push(value).ok().unwrap();
    }
    list
}
fn empty() -> OwnerDeployment {
    OwnerDeployment {
        providers: List::with_capacity(0),
        providers_cut: false,
        key_bytes: 256,
        readiness: List::with_capacity(0),
        readiness_cut: false,
        store: StoreFill { bytes: 0, mark: 0 },
        hosts: List::with_capacity(2),
        attempts: List::with_capacity(2),
        accounts: List::with_capacity(2),
        tasks: OwnerCapacity { used: 0, limit: u32::MAX },
        workers: OwnerCapacity { used: u32::MAX, limit: 1 },
        attempts_capacity: OwnerCapacity { used: 7, limit: 11 },
        waiting_attempts: 19,
        proposals: OwnerCapacity { used: 3, limit: 17 },
        projects: List::with_capacity(2),
        grace_bound: Duration::from_nanos(u64::MAX),
        hello_refusals: List::with_capacity(2),
        hosts_forgotten: u64::MAX,
        hello_refusals_forgotten: 1,
        person_room_refusals: List::with_capacity(2),
        person_room_refusals_forgotten: 2,
        person_answer_retention: Duration::from_nanos(5),
        task_room_refusals: List::with_capacity(2),
        task_room_refusals_forgotten: 3,
        cut: true,
        connectors: List::with_capacity(2),
        connectors_cut: false,
    }
}
fn run(task: OwnerTaskLink, state: OwnerAttemptState, location: OwnerRunLocation) -> OwnerHostRun {
    OwnerHostRun { task, attempt: u64::MAX, kept: u32::MAX, listed: false, state, location, grace_until: None }
}
fn person(action: OwnerPersonCapacityAction, cause: OwnerPersonCapacityCause) -> OwnerPersonCapacityRefusal {
    OwnerPersonCapacityRefusal {
        key: OwnerRequestKey {
            person: u64::MAX,
            key: [0, 255, 1, 254, 2, 253, 3, 252, 4, 251, 5, 250, 6, 249, 7, 248],
        },
        project: None,
        action,
        at: Wall::EPOCH,
        cause,
    }
}
fn task(action: OwnerTaskCapacityAction, cause: OwnerTaskCapacityCause) -> OwnerTaskCapacityRefusal {
    OwnerTaskCapacityRefusal {
        request: OwnerTaskCapacityRequest {
            project: Some(u32::MAX),
            by: OwnerRequester::Deployment { project: u32::MAX },
            action,
            cause,
        },
        at: Wall::from_nanos(u64::MAX),
    }
}
fn domain(value: OwnerDeployment) -> Fixture {
    let (mut client, frame) = start();
    let mut authoritative = document();
    authoritative.deployment_role = Some(HeldRole::Policy { role: 4 });
    authoritative.projects.clear();
    authoritative.cut = true;
    drive(&mut client, Watched { watch: frame, event: WatchEvent::Delivered(Delivery::Me(Box::new(authoritative))) });
    let out = drive(&mut client, Event::Went { place: Place::Jig(Address::Deployment) });
    let watch = out
        .iter()
        .find_map(|r| match r {
            Request::Client { asked: jig_client::Event::Watch { handle: watch, subject: jig_client::Watchable::Jig(Subject::Deployment) } } => Some(*watch),
            _ => None,
        })
        .unwrap();
    drive(&mut client, Watched { watch, event: WatchEvent::Delivered(Delivery::OwnerDeployment(Box::new(value))) });
    client
}
fn build(client: &Fixture, w: &Words, limits: skein_ui::Limits) -> Tree {
    let mut b = Builder::new(&limits);
    let mut bindings = JigBindings::new(limits.bindings, CLIENT.text);
    view::render(&mut b, &client.view(), client.deployment_page().owner.unwrap(), w, &mut bindings);
    b.finish()
}
fn round_trip(value: OwnerDeployment) -> Tree {
    let untouched = value.clone();
    let client = domain(value);
    assert_eq!(client.deployment_page().owner, Some(&untouched));
    let w = shared_words();
    let bounds = view::required(client.deployment_page().owner.unwrap(), &CLIENT, w).unwrap();
    let tree = build(&client, w, bounds);
    assert!(tree.len() <= bounds.nodes);
    assert!(
        tree.nodes().iter().filter(|n| n.role == Role::Region).all(|n| n.name.as_ref().is_some_and(|s| !s.is_empty()))
    );
    tree
}
fn waiting_account(until: Option<Wall>) -> OwnerDeployment {
    let mut value = empty();
    value
        .accounts
        .push(OwnerAccount {
            provider_name: None,
            manage_usage: None,
            place: 0,
            uses: None,
            plan: false,
            provider: None,
            display: DocumentText { bytes: Text::new(Box::from([]), 32).expect("empty display"), cut: false },
            holder: OwnerAccountHolder::Deployment,
            sources: List::with_capacity(0),
            number: 1,
            generation: None,
            state: OwnerAccountState::Later { until },
            usable: false,
            expiring: false,
            attention: false,
            spent_until: None,
            runs: List::with_capacity(0),
            waiting: List::with_capacity(0),
            cut: false,
        })
        .unwrap();
    value
}
#[test]
fn an_account_waiting_to_ask_again_shows_the_wall_it_asks_at() {
    assert!(contains(&round_trip(waiting_account(Some(Wall::from_nanos(46_920_000_000_000)))), b"Asks again at 14:02"));
}
#[test]
fn an_account_waiting_to_ask_again_without_a_clock_shows_later() {
    assert!(contains(&round_trip(waiting_account(None)), b"Asks again later"));
}
#[test]
fn deployment_view_all_attempt_account_hello_states_clocks_keys_and_independent_counts_are_visible() {
    let states = [
        (OwnerAttemptState::Waiting, b"Attempt State Waiting".as_slice()),
        (OwnerAttemptState::Adopted, b"Attempt State Adopted"),
        (OwnerAttemptState::Claimed, b"Attempt State Claimed"),
        (OwnerAttemptState::Cancelled, b"Attempt State Cancelled"),
        (OwnerAttemptState::Handed, b"Attempt State Handed"),
        (OwnerAttemptState::Acknowledged, b"Attempt State Acknowledged"),
        (OwnerAttemptState::Stray, b"Attempt State Stray"),
        (OwnerAttemptState::Kept, b"Attempt State Kept"),
        (OwnerAttemptState::Fenced, b"Attempt State Fenced"),
        (OwnerAttemptState::Closed, b"Attempt State Closed"),
    ];
    for (state, label) in states {
        let mut value = empty();
        value
            .attempts
            .push(run(
                OwnerTaskLink::Stored { number: u64::MAX, status: None },
                state,
                OwnerRunLocation::Adrift { channel: None, until: None, remaining: None },
            ))
            .unwrap();
        let tree = round_trip(value);
        assert!(contains(&tree, label));
        assert!(contains(&tree, b"Clock not supplied"));
        assert!(contains(&tree, b"18446744073709551615"));
        assert!(tree.nodes().iter().any(|n| n.key.is_some_and(|k| k.kind == 96 && k.a == u64::MAX && k.b == u64::MAX)));
    }
    for (state, label) in [
        (OwnerAccountState::Granted, b"Account State Granted".as_slice()),
        (OwnerAccountState::Asking, b"Account State Asking"),
        (OwnerAccountState::SignInAgain, b"Account State Sign in again"),
        (OwnerAccountState::Misconfigured, b"Account State Misconfigured"),
        (
            OwnerAccountState::Unloaded { at: OwnerAccountPlace::File, why: OwnerAccountUnread::Refused },
            b"Account State Unloaded",
        ),
        (OwnerAccountState::Closing, b"Account State Closing"),
        (OwnerAccountState::Moving, b"moving"),
    ] {
        let mut value = empty();
        value
            .accounts
            .push(OwnerAccount {
                provider_name: None,
                manage_usage: None,
                place: 0,
                uses: None,
                plan: false,
                provider: None,
                display: DocumentText { bytes: Text::new(Box::from([]), 32).expect("empty display"), cut: false },
                holder: OwnerAccountHolder::Deployment,
                sources: List::with_capacity(0),
                number: u32::MAX,
                generation: Some(u64::MAX),
                state,
                usable: false,
                expiring: true,
                attention: true,
                spent_until: Some(Wall::EPOCH),
                runs: List::with_capacity(2),
                waiting: List::with_capacity(2),
                cut: false,
            })
            .unwrap();
        let tree = round_trip(value);
        assert!(contains(&tree, label));
        if label == b"Account State Unloaded" {
            assert!(contains(&tree, b"Record file"));
            assert!(contains(&tree, b"Record refused"));
        }
        assert!(contains(&tree, b"Spent until:"));
        assert!(
            tree.nodes()
                .iter()
                .any(|n| n.key.is_some_and(|k| k.kind == 95 && k.a == u64::from(u32::MAX) && k.b == u64::MAX))
        );
    }
    for why in [
        OwnerHelloRefusal::Position { worker: u32::MAX },
        OwnerHelloRefusal::ReservedChannel,
        OwnerHelloRefusal::Workers { limit: u32::MAX },
        OwnerHelloRefusal::StopBound { grace: Duration::from_secs(17) },
    ] {
        let mut value = empty();
        value
            .hello_refusals
            .push(OwnerHelloRefusalLine {
                channel: u64::MAX,
                at: Wall::EPOCH,
                slots: u32::MAX,
                stop_bound: Duration::from_secs(19),
                why,
            })
            .unwrap();
        let tree = round_trip(value);
        assert!(contains(&tree, b"Hello stop bound:"));
    }
}
#[test]
fn deployment_view_opaque_history_keys_source_order_and_full_width_requester_proposer_seen_and_funders_do_not_collapse()
{
    let mut value = empty();
    value.person_room_refusals = list(vec![
        person(
            OwnerPersonCapacityAction::Proposal { proposer: Proposer::Task(u64::MAX), number: u64::MAX },
            OwnerPersonCapacityCause::Requests { limit: u32::MAX },
        ),
        person(
            OwnerPersonCapacityAction::Proposal { proposer: Proposer::Party(u64::MAX), number: 0 },
            OwnerPersonCapacityCause::Waiters { limit: 0 },
        ),
    ]);
    let tree = round_trip(value);
    assert!(contains(&tree, b"Task maker: 18446744073709551615"));
    assert!(contains(&tree, b"Party maker: 18446744073709551615"));
    let histories: Vec<_> = tree.nodes().iter().filter_map(|n| n.key.filter(|k| k.kind == 99)).collect();
    assert_eq!(histories.iter().map(|k| k.a).collect::<Vec<_>>(), vec![0, 1]);
    assert!(histories.iter().all(|k| k.b == 0));
    for funder in [
        Funder::Task(u64::MAX),
        Funder::Pool { project: u32::MAX, person: u64::MAX, period: u64::MAX },
        Funder::Recurring { project: u32::MAX, task: u64::MAX, period: u64::MAX },
        Funder::Period { project: u32::MAX, period: u64::MAX },
    ] {
        let mut value = empty();
        value
            .task_room_refusals
            .push(task(
                OwnerTaskCapacityAction::Source { funder },
                OwnerTaskCapacityCause::Inbox { task: u64::MAX, messages: u32::MAX, bytes: u32::MAX },
            ))
            .unwrap();
        let tree = round_trip(value);
        assert!(contains(&tree, b"18446744073709551615"));
        assert!(contains(&tree, b"4294967295"));
    }
}
#[test]
fn deployment_view_normal_words_select_second_language_and_reject_missing_or_wrong_checked_shapes() {
    const DEPLOYMENT_PHRASES: usize = 286; // 285 existing phrases and the pending settings proposals limit.
    let english = crate::phrases::english::language();
    let mut second = english.clone();
    for phrase in &mut second.phrases {
        for form in &mut phrase.forms {
            for segment in &mut form.segments {
                if let Segment::Text(bytes) = segment {
                    *bytes = copy_of(b"selected ");
                }
            }
        }
    }
    let phrase_limits = PhraseLimits {
        languages: 2,
        phrases: crate::owner_words::phrase_count(),
        holes: 4,
        segments: 5000,
        text_bytes: 100_000,
        hole_bytes: CLIENT.text,
    };
    let words = Words::new(
        Phrasebook::load(crate::phrases::jig::shapes(CLIENT.text), Box::new([english, second]), &phrase_limits)
            .unwrap(),
        1,
        0,
    );
    let client = domain(empty());
    let maximum = view::required(client.deployment_page().owner.unwrap(), &CLIENT, &words).unwrap();
    let tree = build(&client, &words, maximum);
    assert!(contains(&tree, b"selected "));
    assert!(!contains(&tree, b"Deployment diagnostics"));
    assert_eq!(crate::owner_deployment_words::ALL.len(), DEPLOYMENT_PHRASES);
    let mut wrong = crate::owner_deployment_words::language();
    let phrase = wrong.phrases.iter_mut().find(|p| p.id == P::Host.id()).unwrap();
    phrase.forms[0].segments = Box::new([Segment::Text(copy_of(b"host without number"))]);
    assert_eq!(
        Phrasebook::load(crate::owner_deployment_words::shapes(), Box::new([wrong]), &phrase_limits).err(),
        Some(Invalid::Holes { id: P::Host.id() })
    );
    let mut missing = crate::phrases::english::language();
    missing.phrases = missing.phrases.into_vec().into_iter().filter(|p| p.id != P::Hosts.id()).collect();
    assert_eq!(
        Phrasebook::load(crate::phrases::jig::shapes(CLIENT.text), Box::new([missing]), &phrase_limits).unwrap_err(),
        Invalid::Missing { id: P::Hosts.id() }
    );
}
#[test]
fn deployment_view_all_outer_and_nested_lists_fill_together_with_exact_and_one_under_storage() {
    let mut value = empty();
    for n in [0, u64::MAX] {
        let attempt = run(
            OwnerTaskLink::Retained { chip: genuine_owner_chip_fixture(n), at: Some(Wall::EPOCH) },
            OwnerAttemptState::Acknowledged,
            OwnerRunLocation::Adrift { channel: Some(n), until: Some(Wall::EPOCH), remaining: Some(Duration::ZERO) },
        );
        value
            .hosts
            .push(OwnerHost {
                name: None,
                awaiting_logins: false,
                rule: HostRule::Person(u64::MAX),
                programs: List::with_capacity(0),
                channel: n,
                kind: OwnerHostKind::Worker,
                slots: u32::MAX,
                used: u32::MAX,
                free: u32::MAX,
                draining: true,
                connected: false,
                grace_until: Some(Wall::EPOCH),
                grace_remaining: Some(Duration::ZERO),
                runs: list(vec![
                    attempt.clone(),
                    run(
                        OwnerTaskLink::Live(genuine_owner_chip_fixture(n.wrapping_add(1))),
                        OwnerAttemptState::Closed,
                        OwnerRunLocation::Connected { channel: n, kind: OwnerHostKind::Engine },
                    ),
                ]),
                cut: true,
            })
            .unwrap();
        value.attempts.push(attempt).unwrap();
        value
            .accounts
            .push(OwnerAccount {
                provider_name: None,
                manage_usage: None,
                place: 0,
                uses: None,
                plan: false,
                provider: None,
                display: DocumentText { bytes: Text::new(Box::from([]), 32).expect("empty display"), cut: false },
                holder: OwnerAccountHolder::Deployment,
                sources: List::with_capacity(0),
                number: if n == 0 { 0 } else { u32::MAX },
                generation: Some(n),
                state: OwnerAccountState::Closing,
                usable: true,
                expiring: true,
                attention: true,
                spent_until: Some(Wall::EPOCH),
                runs: list(vec![genuine_owner_chip_fixture(n), genuine_owner_chip_fixture(n.wrapping_add(1))]),
                waiting: list(vec![genuine_owner_chip_fixture(n), genuine_owner_chip_fixture(n.wrapping_add(1))]),
                cut: true,
            })
            .unwrap();
        value
            .projects
            .push(OwnerProjectCapacity {
                project: if n == 0 { 0 } else { u32::MAX },
                tasks: OwnerCapacity { used: u32::MAX, limit: u32::MAX },
                proposals: u32::MAX,
            })
            .unwrap();
        value
            .hello_refusals
            .push(OwnerHelloRefusalLine {
                channel: n,
                at: Wall::EPOCH,
                slots: u32::MAX,
                stop_bound: Duration::from_nanos(u64::MAX),
                why: OwnerHelloRefusal::StopBound { grace: Duration::from_nanos(u64::MAX) },
            })
            .unwrap();
        value
            .person_room_refusals
            .push(person(
                OwnerPersonCapacityAction::Question { task: n, question: n },
                OwnerPersonCapacityCause::Limit { which: AdmissionLimit::ProjectLiveTasks },
            ))
            .unwrap();
        value
            .task_room_refusals
            .push(task(
                OwnerTaskCapacityAction::Source { funder: Funder::Pool { project: u32::MAX, person: n, period: n } },
                OwnerTaskCapacityCause::Inbox { task: n, messages: u32::MAX, bytes: u32::MAX },
            ))
            .unwrap();
    }
    value.connectors = list(vec![u16::MAX, 0]);
    value.connectors_cut = true;
    let untouched = value.clone();
    let client = domain(value);
    let w = shared_words();
    let maximum = view::required(&untouched, &CLIENT, w).unwrap();
    let tree = build(&client, w, maximum);
    assert!(tree.len() <= maximum.nodes);
    assert!(crate::deployment_nodes(&CLIENT, 0).unwrap() >= maximum.nodes);
    let depth = u32::try_from(
        tree.nodes()
            .iter()
            .enumerate()
            .map(|(at, _)| {
                1 + tree
                    .nodes()
                    .iter()
                    .enumerate()
                    .take(at)
                    .filter(|(index, node)| index + usize::try_from(node.size).unwrap() > at)
                    .count()
            })
            .max()
            .unwrap(),
    )
    .unwrap();
    let exact = skein_ui::Limits {
        nodes: tree.len(),
        depth,
        text_per_node: tree.nodes().iter().map(crate::tests::text_bytes).max().unwrap(),
        text_bytes: tree.nodes().iter().map(crate::tests::text_bytes).sum(),
        ..maximum
    };
    assert_eq!(build(&client, w, exact).len(), tree.len());
    for below in [
        skein_ui::Limits { nodes: exact.nodes - 1, ..exact },
        skein_ui::Limits { depth: exact.depth - 1, ..exact },
        skein_ui::Limits { text_per_node: exact.text_per_node - 1, ..exact },
        skein_ui::Limits { text_bytes: exact.text_bytes - 1, ..exact },
    ] {
        std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| build(&client, w, below)))
            .expect_err("one-under storage refuses this entire tree");
    }
    let mut oversized = untouched.clone();
    oversized.hosts = List::with_capacity(CLIENT.rows + 1);
    assert!(view::required(&oversized, &CLIENT, w).is_none());
    assert!(view::max_nodes(&crate::Limits { rows: u32::MAX, ..CLIENT }).is_none());
    assert_eq!(client.deployment_page().owner, Some(&untouched));
}
fn genuine_owner_chip_fixture(number: u64) -> Chip {
    let part = OwnerPart {
        connector: u16::MAX,
        kind: OwnerPartKind::Resource,
        key: OwnerPartKey::Path(list(vec![Box::from([0, 255]), Box::from([1, 254, 2])])),
    };
    let resource = OwnerResourceLine {
        part: part.clone(),
        kind: u16::MAX,
        taken: true,
        holder: Some(u64::MAX),
        writer: Some(OwnerResourceWriter::Effect { entry: u64::MAX }),
        writer_lost: true,
    };
    let owner = OwnerChipDetails {
        dependencies: List::with_capacity(0),
        project: u32::MAX,
        executor: OwnerExecutor::Agent { charter: u32::MAX },
        phase_since: Wall::EPOCH,
        run_since: None,
        attempt: Some(u64::MAX),
        retry: None,
        wake: OwnerWakePolicy {
            words: WakeRule::Never,
            notices: WakeRule::Immediate,
            news: WakeRule::Batch { count: u32::MAX, age: Duration::ZERO },
            results: ResultsWake::LastOrFailure,
            questions: true,
            answers: false,
            timers: true,
        },
        person_wait: None,
        hold: None,
        budget: OwnerBudget {
            numbers: OwnerNumbers { budget: u64::MAX, spent: 1, spent_below: 2, reserved: 3 },
            left: 5,
            funder: Funder::Task(u64::MAX),
            deadline: Some(Wall::EPOCH),
        },
        activity: u64::MAX,
        resources: list(vec![resource.clone(), resource]),
        resources_cut: true,
        resources_observed: true,
        procedure: None,
        live_delegates: u32::MAX,
        closing_connectors: Some(list(vec![0, u16::MAX])),
        ending: Some(OwnerChipEnding::Done(ResultValue::Change {
            kind: u16::MAX,
            part,
            words: DocumentText::complete(Text::new(Box::from(*b"done"), 4).unwrap()),
        })),
        ended_at: None,
        observation: Some(OwnerChipObservation {
            channel: skein_lib::Token::new(u64::MAX),
            work: OwnerChipWork::Tool { call: Box::from(*b"raw"), tool: Box::from(*b"tool") },
            since: Wall::EPOCH,
            reported_at: Wall::from_nanos(u64::MAX),
            started: None,
        }),
    };
    Chip {
        task: number,
        title: DocumentText::complete(Text::new(Box::from(*b"task"), 4).unwrap()),
        executor: Executor::Agent { charter: u32::MAX },
        phase: Phase::Waiting,
        details: Box::new(ChipPhase::Owner {
            phase: OwnerChipPhase::Waiting { dependencies: List::with_capacity(2), more: true, holds: false },
            details: Box::new(owner),
        }),
    }
}

#[test]
fn deployment_view_person_actions_four_causes_all_structural_limits_and_opaque_request_keys_survive() {
    use AdmissionLimit as L;
    use OwnerPersonCapacityAction as A;
    let actions = [
        A::AccountSignIn,
        A::AccountSignOut,
        A::AccountChange,
        A::Create,
        A::Compound,
        A::Application { connector: u16::MAX },
        A::Note { name: u64::MAX },
        A::Service,
        A::Adoption,
        A::Roles,
        A::Policy,
        A::Pool { person: u64::MAX },
        A::Escalation { task: u64::MAX },
        A::Proposal { proposer: Proposer::Task(u64::MAX), number: u64::MAX },
        A::Proposal { proposer: Proposer::Party(u64::MAX), number: 0 },
        A::Words { task: u64::MAX },
        A::Question { task: u64::MAX, question: u64::MAX },
        A::Amend { task: u64::MAX },
        A::Move { task: u64::MAX },
        A::TakePerson { task: u64::MAX },
        A::HandBackPerson { task: u64::MAX },
        A::AnswerPerson { task: u64::MAX },
        A::Stop { task: u64::MAX },
        A::Cancel { task: u64::MAX },
        A::Release { task: u64::MAX },
        A::MarkSeen { seen: SeenThrough::Results { through: u64::MAX } },
        A::MarkSeen { seen: SeenThrough::Conversation { task: u64::MAX, through: u64::MAX } },
    ];
    for action in actions {
        let mut value = empty();
        value
            .person_room_refusals
            .push(person(action, OwnerPersonCapacityCause::Requests { limit: u32::MAX }))
            .unwrap();
        round_trip(value);
    }
    let limits = [
        L::Words,
        L::Identity,
        L::Roles,
        L::Arithmetic,
        L::Duplicate,
        L::Empty,
        L::Batch,
        L::ProjectLiveTasks,
        L::LiveTasks,
        L::Tree,
        L::Depth,
        L::Delegates,
        L::Subscription,
        L::Dependencies,
        L::Cycle,
        L::Spec,
        L::Contract,
        L::Authority,
        L::Inputs,
        L::Attempts,
        L::LiveDelegates,
        L::Restore,
        L::Read,
        L::Turn,
        L::HoldKind,
        L::HoldTaken,
        L::Holds,
        L::InboxFull,
        L::Notes,
        L::Revision,
        L::Question,
        L::Priority,
        L::Policy,
        L::Resource,
        L::Service,
    ];
    assert_eq!(limits.len(), 35);
    for which in limits {
        let mut value = empty();
        value
            .person_room_refusals
            .push(person(A::Application { connector: u16::MAX }, OwnerPersonCapacityCause::Limit { which }))
            .unwrap();
        round_trip(value);
    }
    for cause in
        [OwnerPersonCapacityCause::Pending { limit: u32::MAX }, OwnerPersonCapacityCause::Waiters { limit: u32::MAX }]
    {
        let mut value = empty();
        let mut refusal = person(A::Application { connector: u16::MAX }, cause);
        refusal.project = Some(0);
        value.person_room_refusals.push(refusal).unwrap();
        round_trip(value);
    }
}
#[test]
fn deployment_view_task_actions_fourteen_causes_three_requesters_and_four_funders_keep_full_width_values() {
    use OwnerTaskCapacityAction as A;
    use OwnerTaskCapacityCause as C;
    let actions = [
        A::Batch { first: None, members: u32::MAX },
        A::Batch { first: Some(0), members: 0 },
        A::Proposal { number: u64::MAX },
        A::PersonProposal { number: u64::MAX },
        A::Message { task: u64::MAX, number: u64::MAX },
        A::Source { funder: Funder::Task(u64::MAX) },
        A::Source { funder: Funder::Pool { project: u32::MAX, person: u64::MAX, period: u64::MAX } },
        A::Source { funder: Funder::Recurring { project: u32::MAX, task: u64::MAX, period: u64::MAX } },
        A::Source { funder: Funder::Period { project: u32::MAX, period: u64::MAX } },
        A::Introduce { left: u64::MAX, right: 0 },
        A::Subscription { number: u64::MAX },
        A::Effect { entry: u64::MAX },
        A::Claim { attempt: u64::MAX },
        A::Release,
        A::Move,
    ];
    for action in actions {
        let mut value = empty();
        value.task_room_refusals.push(task(action, C::Tasks { limit: u32::MAX })).unwrap();
        round_trip(value);
    }
    let causes = [
        C::Tasks { limit: u32::MAX },
        C::CreationFlights { limit: u32::MAX },
        C::ResourceReports { limit: u32::MAX },
        C::ProjectTasks { limit: u32::MAX },
        C::Tree { limit: u32::MAX },
        C::Delegates { limit: u32::MAX },
        C::Inbox { task: u64::MAX, messages: u32::MAX, bytes: u32::MAX },
        C::Holdings { limit: u32::MAX },
        C::HoldWaiters { limit: u32::MAX },
        C::Proposals { limit: u32::MAX },
        C::References { limit: u32::MAX },
        C::Subscriptions { limit: u32::MAX },
        C::Funders { limit: u32::MAX },
        C::Writers { limit: u32::MAX },
    ];
    assert_eq!(causes.len(), 14);
    for cause in causes {
        let mut value = empty();
        value.task_room_refusals.push(task(A::Move, cause)).unwrap();
        round_trip(value);
    }
    for by in [
        OwnerRequester::Task(u64::MAX),
        OwnerRequester::Person(u64::MAX),
        OwnerRequester::Deployment { project: u32::MAX },
    ] {
        let mut value = empty();
        let mut refusal = task(A::Release, C::Tasks { limit: 0 });
        refusal.request.by = by;
        refusal.request.project = None;
        value.task_room_refusals.push(refusal).unwrap();
        round_trip(value);
    }
}
#[test]
fn deployment_view_configured_empty_and_clipped_catalogues_are_distinct_and_slots_preserve_owner_order() {
    let tree = round_trip(empty());
    assert!(contains(&tree, b"No configured connectors"));
    let mut value = empty();
    value.connectors_cut = true;
    let tree = round_trip(value.clone());
    assert!(!contains(&tree, b"No configured connectors"));
    assert!(contains(&tree, b"Some configured connectors omitted"));
    value.connectors.push(u16::MAX).unwrap();
    value.connectors.push(0).unwrap();
    let tree = round_trip(value);
    let slots: Vec<_> = tree.nodes().iter().filter_map(|n| n.slot).collect();
    assert_eq!(
        slots.iter().map(|s| s.owner).collect::<Vec<_>>(),
        vec![
            (crate::Owner::Application { number: u16::MAX }).opaque(),
            (crate::Owner::Application { number: 0 }).opaque()
        ]
    );
    assert!(!contains(&tree, b"No configured connectors"));
}

fn store_words() -> Words {
    let limits = PhraseLimits {
        languages: 1,
        phrases: u32::try_from(crate::owner_deployment_words::ALL.len()).unwrap(),
        holes: 1,
        segments: 512,
        text_bytes: 16_384,
        hole_bytes: CLIENT.text,
    };
    Words::new(
        Phrasebook::load(
            crate::owner_deployment_words::shapes(),
            Box::new([crate::owner_deployment_words::language()]),
            &limits,
        )
        .unwrap(),
        0,
        0,
    )
}

#[test]
fn the_deployment_page_shows_the_store_fill_against_its_mark() {
    let mut value = empty();
    value.store = StoreFill { bytes: 6789, mark: 9012 };
    let client = domain(value);
    let tree = build(&client, &store_words(), super::cards::limits());
    assert!(contains(&tree, b"Store map bytes: 6789"));
    assert!(contains(&tree, b"Store high-water mark in bytes: 9012"));
}

#[test]
fn an_agent_shortfall_refusal_is_worded() {
    let mut value = empty();
    value
        .hello_refusals
        .push(OwnerHelloRefusalLine {
            channel: 9,
            at: Wall::EPOCH,
            slots: 1,
            stop_bound: Duration::ZERO,
            why: OwnerHelloRefusal::Agent { why: OwnerAgentShortfall::Window { charter: 42, model: 3 } },
        })
        .unwrap();
    let tree = round_trip(value);
    assert!(contains(&tree, b"Agent window too small for charter 42"));
    assert!(contains(&tree, b"Model position 3"));
}

fn identity_accounts() -> OwnerDeployment {
    let mut value = waiting_account(None);
    value.cut = false;
    let account = value.accounts.get_mut(0).unwrap();
    account.provider = Some(2);
    account.display = DocumentText { bytes: Text::new(copy_of(b"Personal"), CLIENT.text).unwrap(), cut: true };
    account.holder = OwnerAccountHolder::Party(23);
    account.plan = true;
    account.uses = Some(list(vec![OwnerAccountUse::Party(23), OwnerAccountUse::Project(29)]));
    account.sources = list(vec![
        OwnerAccountSource {
            name: None,
            place: 3,
            number: 11,
            kind: OwnerAccountKind::SignIn,
            state: OwnerAccountState::Granted,
            usable: true,
            expiring: true,
            attention: true,
            spent_until: Some(Wall::from_nanos(46_920_000_000_000)),
        },
        OwnerAccountSource {
            name: None,
            place: 5,
            number: 12,
            kind: OwnerAccountKind::Key,
            state: OwnerAccountState::Later { until: None },
            usable: false,
            expiring: false,
            attention: false,
            spent_until: None,
        },
    ]);
    value
}
#[test]
fn deployment_accounts_show_identity_sources_and_payers_in_order() {
    let tree = round_trip(identity_accounts());
    let text: Vec<_> =
        tree.nodes().iter().flat_map(|n| n.text.as_slice().iter().flat_map(|r| r.text.iter().copied())).collect();
    let ordered = [
        b"LLM provider 2".as_slice(),
        b"Personal",
        b"Some rows were omitted",
        b"Held by party 23",
        b"A plan's account",
        b"Source 11",
        b"Sign-in",
        b"Account State Granted",
        b"Usable",
        b"Expiring",
        b"Needs attention",
        b"14:02",
        b"Source 12",
        b"Key",
        b"Asks again later",
        b"Pays for party 23",
        b"Pays for project 29",
    ];
    let mut after = 0;
    for words in ordered {
        let offset = text[after..]
            .windows(words.len())
            .position(|w| w == words)
            .unwrap_or_else(|| panic!("missing ordered words {}", String::from_utf8_lossy(words)));
        after += offset + words.len();
    }
}
#[test]
fn deployment_accounts_show_the_default_uses_and_deployment_holder() {
    let tree = round_trip(waiting_account(None));
    assert!(contains(&tree, b"Default uses"));
    assert!(contains(&tree, b"Held by the deployment"));
}
#[test]
fn account_uses_keep_their_constant_receiving_bound() {
    // domain/accounts.md, 4.2; the independent client wire constant is 16.
    const USES: usize = 16;
    let mut value = waiting_account(None);
    value.accounts.get_mut(0).unwrap().uses = Some(list(vec![OwnerAccountUse::Party(23); USES]));
    let _tree = round_trip(value.clone());
    value.accounts.get_mut(0).unwrap().uses = Some(list(vec![OwnerAccountUse::Party(23); USES + 1]));
    assert!(view::required(&value, &CLIENT, shared_words()).is_none());
}

fn deployment_terms() -> OwnerDeployment {
    let mut value = waiting_account(None);
    let account = value.accounts.get_mut(0).unwrap();
    account.provider_name =
        Some(DocumentText { bytes: Text::new(copy_of(b"Arbitrary provider"), CLIENT.text).unwrap(), cut: true });
    account.manage_usage = Some(copy_of(b"https://usage.example/account"));
    value
}
#[test]
fn deployment_shows_the_supplied_provider_name() {
    assert!(contains(&round_trip(deployment_terms()), b"Arbitrary provider"));
}
#[test]
fn deployment_marks_a_cut_provider_name() {
    assert!(contains(&round_trip(deployment_terms()), b"Some rows were omitted"));
}
#[test]
fn deployment_links_the_whole_manage_address() {
    let tree = round_trip(deployment_terms());
    assert!(
        tree.nodes().iter().any(|node| node.role == Role::Link
            && node.external.as_deref() == Some(b"https://usage.example/account".as_slice()))
    );
}
#[test]
fn admitted_manage_targets_are_safe_for_rendering() {
    for address in [b"https://usage.example/".as_slice(), b"https://[::1]/", b"https://localhost:443/path"] {
        assert!(valid_manage_usage(address));
        assert!(skein_ui::markdown::safe_url(address));
    }
}
#[test]
fn deployment_manage_addresses_hold_at_the_whole_address_bound() {
    let mut value = waiting_account(None);
    let mut address = b"https://usage.example/".to_vec();
    address.resize(usize::try_from(ACCOUNT_MANAGE_USAGE).unwrap(), b'a');
    value.accounts.get_mut(0).unwrap().manage_usage = Some(address.into_boxed_slice());
    let client = domain(value.clone());
    let tree = super::page_capacity::render(&client, shared_words());
    assert!(tree.nodes().iter().any(|node| {
        node.external.as_ref().is_some_and(|address| address.len() == usize::try_from(ACCOUNT_MANAGE_USAGE).unwrap())
    }));
    round_trip(value);
}
#[test]
fn deployment_manage_addresses_one_past_the_bound_are_refused() {
    let mut value = waiting_account(None);
    let mut address = b"https://usage.example/".to_vec();
    address.resize(usize::try_from(ACCOUNT_MANAGE_USAGE).unwrap() + 1, b'a');
    value.accounts.get_mut(0).unwrap().manage_usage = Some(address.into_boxed_slice());
    assert!(view::required(&value, &CLIENT, shared_words()).is_none());
}

fn catalogue() -> OwnerDeployment {
    let mut value = empty();
    value.providers = list(vec![ProviderOffer {
        provider: 37,
        key_bytes: 123,
        name: DocumentText::complete(Text::new(b"Quasar Example".as_slice().into(), CLIENT.text).unwrap()),
        sign_in: true,
        key: true,
        programs: list(vec![Text::new(b"arbitrary".as_slice().into(), CLIENT.text).unwrap()]),
        programs_cut: true,
        none: false,
        signed_in_plan: true,
        manage_usage: Some(b"https://usage.example/other".as_slice().into()),
    }]);
    value.providers_cut = true;
    value.readiness = list(vec![PayerReadiness {
        provider: 37,
        payer: OwnerAccountUse::Project(91),
        readiness: Readiness::Unknown,
        settled: None,
    }]);
    value.readiness_cut = true;
    value.key_bytes = 123;
    value
}
#[test]
fn deployment_renders_the_public_provider_catalogue() {
    let tree = round_trip(catalogue());
    for words in [
        b"Quasar Example".as_slice(),
        b"arbitrary",
        b"Sign-in",
        b"Key",
        b"Sign-ins use plan terms",
        b"Maximum key bytes 123",
        b"More provider offers omitted",
    ] {
        assert!(contains(&tree, words), "{}", String::from_utf8_lossy(words));
    }
    assert!(
        tree.nodes().iter().any(|node| node.external.as_deref() == Some(b"https://usage.example/other".as_slice()))
    );
}
fn fragment(value: &OwnerDeployment, limits: skein_ui::Limits) -> Tree {
    let mut builder = Builder::new(&limits);
    builder.open(Role::Region);
    crate::account_catalogue::render(
        &mut builder,
        shared_words(),
        &value.providers,
        value.providers_cut,
        value.key_bytes,
        &value.readiness,
        value.readiness_cut,
    );
    builder.close();
    builder.finish()
}
#[test]
fn the_reusable_fragment_labels_payer_readiness() {
    for (ready, settled, words, settlement) in [
        (Readiness::Unknown, None, b"Readiness unknown".as_slice(), b"Program checks unknown".as_slice()),
        (Readiness::Usable, Some(true), b"Ready for this payer", b"Account requests settled"),
        (Readiness::Unavailable, Some(false), b"No usable account for this payer", b"Account request pending"),
    ] {
        let mut value = catalogue();
        value.readiness.get_mut(0).unwrap().readiness = ready;
        value.readiness.get_mut(0).unwrap().settled = settled;
        let tree = fragment(&value, view::limits(&CLIENT, shared_words()).unwrap());
        assert!(contains(&tree, words));
        assert!(contains(&tree, settlement));
        assert!(contains(&tree, b"Pays for project 91"));
        assert!(contains(&tree, b"More payer readiness omitted"));
    }
}
#[test]
fn catalogue_program_capacity_holds_at_its_bound() {
    let mut value = catalogue();
    value.providers.get_mut(0).unwrap().programs = List::with_capacity(CLIENT.rows);
    assert!(view::required(&value, &CLIENT, shared_words()).is_some());
    value.providers.get_mut(0).unwrap().programs = List::with_capacity(CLIENT.rows + 1);
    assert!(view::required(&value, &CLIENT, shared_words()).is_none());
}
#[test]
fn catalogue_fragment_storage_holds_at_its_exact_size() {
    let value = catalogue();
    let bounds = view::limits(&CLIENT, shared_words()).unwrap();
    let tree = fragment(&value, bounds);
    assert!(tree.len() <= crate::account_catalogue::max_nodes(&CLIENT).unwrap() + 1);
    let exact = skein_ui::Limits { nodes: tree.len(), ..bounds };
    assert_eq!(fragment(&value, exact).len(), tree.len());
    std::panic::catch_unwind(|| fragment(&value, skein_ui::Limits { nodes: exact.nodes - 1, ..exact }))
        .expect_err("one node short refuses");
}

#[test]
fn a_key_offer_has_no_plan_label() {
    let mut value = catalogue();
    let offer = value.providers.get_mut(0).unwrap();
    offer.sign_in = false;
    offer.signed_in_plan = false;
    offer.programs.clear();
    let tree = round_trip(value);
    assert!(contains(&tree, b"Key"));
    assert!(!contains(&tree, b"Sign-ins use plan terms"));
}
#[test]
fn a_none_offer_labels_its_absent_credential() {
    let mut value = catalogue();
    let offer = value.providers.get_mut(0).unwrap();
    offer.sign_in = false;
    offer.signed_in_plan = false;
    offer.key = false;
    offer.programs.clear();
    offer.none = true;
    assert!(contains(&round_trip(value), b"No credential"));
}

#[test]
fn catalogue_manage_targets_hold_at_the_address_bound() {
    let mut value = catalogue();
    let mut address = b"https://usage.example/".to_vec();
    address.resize(usize::try_from(ACCOUNT_MANAGE_USAGE).unwrap(), b'a');
    value.providers.get_mut(0).unwrap().manage_usage = Some(address.clone().into_boxed_slice());
    round_trip(value.clone());
    address.push(b'a');
    value.providers.get_mut(0).unwrap().manage_usage = Some(address.into_boxed_slice());
    assert!(view::required(&value, &CLIENT, shared_words()).is_none());
}

fn program_host() -> OwnerHost {
    OwnerHost {
        name: Some(b"configured-worker".as_slice().into()),
        awaiting_logins: false,
        rule: HostRule::Open,
        programs: List::with_capacity(2),
        channel: 17,
        kind: OwnerHostKind::Worker,
        slots: 1,
        used: 0,
        free: 1,
        draining: false,
        connected: true,
        grace_until: None,
        grace_remaining: None,
        runs: List::with_capacity(2),
        cut: false,
    }
}
#[test]
fn deployment_names_the_configured_worker() {
    let mut value = empty();
    value.hosts.push(program_host()).expect("host");
    assert!(contains(&round_trip(value), b"configured-worker"));
}
#[test]
fn deployment_renders_the_program_observation() {
    let mut host = program_host();
    host.programs
        .push(OwnerProgramLogin {
            name: b"personal".as_slice().into(),
            state: Some(OwnerProgramState::Usable),
            sign_out: Some(OwnerProgramSignOut::Failed {
                reason: OwnerProgramFailure::Deadline,
                detail: b"bounded failure".as_slice().into(),
            }),
            cleanup: true,
            cut: false,
        })
        .expect("program");
    let mut value = empty();
    value.hosts.push(host).expect("host");
    let tree = round_trip(value);
    for word in [
        b"personal".as_slice(),
        b"Login usable",
        b"Last sign-out failed",
        b"Program timed out",
        b"bounded failure",
        b"Keep signed out",
    ] {
        assert!(contains(&tree, word), "missing {word:?}");
    }
}

#[test]
fn configured_names_use_their_own_byte_bound() {
    let mut value = empty();
    let mut host = program_host();
    host.name = Some(vec![b'a'; 4096].into());
    value.hosts.push(host).unwrap();
    let limits = crate::Limits { text: 1, ..CLIENT };
    assert!(view::required(&value, &limits, shared_words()).is_some());
}

#[test]
fn a_refused_worker_position_is_shown() {
    let mut value = empty();
    value
        .hello_refusals
        .push(OwnerHelloRefusalLine {
            channel: 9,
            at: Wall::EPOCH,
            slots: 1,
            stop_bound: Duration::ZERO,
            why: OwnerHelloRefusal::Position { worker: u32::MAX },
        })
        .unwrap();
    let tree = round_trip(value);
    assert!(contains(&tree, b"Unconfigured or occupied worker position 4294967295"));
}

fn account_lists(accounts: u32, sources: u32) -> OwnerDeployment {
    let mut value = identity_accounts();
    let mut account = value.accounts.get(0).unwrap().clone();
    let source = account.sources.get(0).unwrap().clone();
    account.sources = List::with_capacity(sources);
    for number in 0..sources {
        let mut source = source.clone();
        source.number = number;
        account.sources.push(source).unwrap();
    }
    value.accounts = List::with_capacity(accounts);
    for number in 0..accounts {
        let mut account = account.clone();
        account.number = number;
        value.accounts.push(account).unwrap();
    }
    value
}
#[test]
fn whole_account_lists_above_rows_fill_the_view_counts() {
    let value = account_lists(CLIENT.accounts.accounts, CLIENT.accounts.sources);
    let client = domain(value.clone());
    let bounds = view::required(&value, &CLIENT, shared_words()).unwrap();
    let tree = build(&client, shared_words(), bounds);
    assert_eq!(
        tree.nodes().iter().filter(|node| node.key.is_some_and(|key| key.kind == 95)).count(),
        usize::try_from(CLIENT.accounts.accounts).unwrap()
    );
    assert!(tree.len() <= bounds.nodes);
    assert!(tree.nodes().iter().map(crate::tests::text_bytes).sum::<u32>() <= bounds.text_bytes);
}
#[test]
fn account_lists_one_past_the_view_counts_are_refused() {
    for value in [
        account_lists(CLIENT.accounts.accounts + 1, CLIENT.accounts.sources),
        account_lists(CLIENT.accounts.accounts, CLIENT.accounts.sources + 1),
    ] {
        assert!(view::required(&value, &CLIENT, shared_words()).is_none());
    }
}

#[test]
fn a_closed_host_says_why() {
    for (rule, expected) in [
        (HostRule::Person(u64::MAX), b"Only this person's runs: 18446744073709551615".as_slice()),
        (HostRule::Several, b"No run: several people's logins".as_slice()),
        (HostRule::Unknown, b"No run: a login jig cannot name".as_slice()),
    ] {
        let mut value = empty();
        let mut host = program_host();
        host.rule = rule;
        value.hosts.push(host).expect("host");
        assert!(contains(&round_trip(value), expected), "missing {expected:?}");
    }
}
