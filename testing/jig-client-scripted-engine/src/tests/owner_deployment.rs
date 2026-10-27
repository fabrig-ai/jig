//! Independent whole-carrier proofs through the scripted peer's public entries.
use super::*;
use crate::payload::Payload;
use core::mem::size_of;

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
fn chip(number: u64) -> Chip {
    Chip {
        task: number,
        title: DocumentText::complete(text(b"task")),
        executor: Executor::Agent { charter: 1 },
        phase: Phase::Waiting,
        details: Box::new(ChipPhase::Waiting { dependencies: List::with_capacity(2), hold: None }),
    }
}
fn run(task: OwnerTaskLink, state: OwnerAttemptState, location: OwnerRunLocation) -> OwnerHostRun {
    OwnerHostRun { task, attempt: u64::MAX, kept: u32::MAX, listed: false, state, location, grace_until: None }
}
fn round_trip(value: OwnerDeployment) {
    let mut setup = script();
    setup.truth.owner_deployment = Some(value.clone());
    let mut engine = Engine::new(setup, 1).unwrap();
    engine.read(Time::ZERO, 11, Token::new(1), Read::Deployment).unwrap();
    assert_eq!(
        engine.poll(Time::ZERO),
        vec![Up::Read {
            token: Token::new(1),
            terminal: ReadResult::Value(ReadValue::OwnerDeployment(Box::new(value.clone())))
        }]
    );
    engine.open(Time::ZERO, 11, Token::new(2), Subject::Deployment).unwrap();
    let events = engine.poll(Time::ZERO);
    let expected =
        Up::Watch { token: Token::new(2), event: WatchEvent::Delivered(Delivery::OwnerDeployment(Box::new(value))) };
    assert!(events.contains(&expected));
    assert!(engine.commits().is_empty());
}
#[test]
fn deployment_whole_twenty_one_fields_independent_capacities_loss_and_source_order_reach_read_and_watch() {
    let mut value = empty();
    value.projects = list(vec![
        OwnerProjectCapacity { project: u32::MAX, tasks: OwnerCapacity { used: 23, limit: 29 }, proposals: 31 },
        OwnerProjectCapacity { project: 0, tasks: OwnerCapacity { used: 37, limit: 41 }, proposals: 43 },
    ]);
    round_trip(value);
    let mut value = empty();
    value.cut = false;
    value.hosts_forgotten = 0;
    value.hello_refusals_forgotten = u64::MAX - 1;
    value.person_room_refusals_forgotten = u64::MAX - 2;
    value.task_room_refusals_forgotten = u64::MAX - 3;
    round_trip(value);
}
#[test]
fn deployment_hosts_ten_attempt_states_three_placements_task_links_and_absent_clocks_are_independent() {
    let states = [
        OwnerAttemptState::Waiting,
        OwnerAttemptState::Adopted,
        OwnerAttemptState::Claimed,
        OwnerAttemptState::Cancelled,
        OwnerAttemptState::Handed,
        OwnerAttemptState::Acknowledged,
        OwnerAttemptState::Stray,
        OwnerAttemptState::Kept,
        OwnerAttemptState::Fenced,
        OwnerAttemptState::Closed,
    ];
    let places = [
        OwnerRunLocation::None,
        OwnerRunLocation::Connected { channel: u64::MAX, kind: OwnerHostKind::Engine },
        OwnerRunLocation::Adrift { channel: None, until: None, remaining: None },
        OwnerRunLocation::Adrift { channel: Some(0), until: Some(Wall::EPOCH), remaining: Some(Duration::ZERO) },
    ];
    for (i, state) in states.into_iter().enumerate() {
        let mut value = empty();
        let attempt = run(OwnerTaskLink::Stored { number: u64::MAX, status: None }, state, places[i % places.len()]);
        value.attempts.push(attempt.clone()).unwrap();
        value
            .hosts
            .push(OwnerHost {
                name: None,
                awaiting_logins: false,
                rule: HostRule::Open,
                programs: List::with_capacity(0),
                channel: u64::MAX,
                kind: OwnerHostKind::Worker,
                slots: u32::MAX,
                used: 7,
                free: 13,
                draining: i % 2 == 0,
                connected: i % 2 != 0,
                grace_until: None,
                grace_remaining: Some(Duration::ZERO),
                runs: list(vec![attempt]),
                cut: false,
            })
            .unwrap();
        round_trip(value);
    }
    for task in [
        OwnerTaskLink::Live(chip(0)),
        OwnerTaskLink::Retained { chip: chip(u64::MAX), at: None },
        OwnerTaskLink::Retained { chip: chip(u64::MAX), at: Some(Wall::EPOCH) },
        OwnerTaskLink::Stored { number: 0, status: Some(OwnerTaskStatus::Cancelled) },
    ] {
        let mut value = empty();
        let mut attempt = run(task, OwnerAttemptState::Closed, places[0]);
        attempt.grace_until = Some(Wall::EPOCH);
        value.attempts.push(attempt).unwrap();
        round_trip(value);
    }
}
fn account_states() -> [OwnerAccountState; 8] {
    [
        OwnerAccountState::Granted,
        OwnerAccountState::Asking,
        OwnerAccountState::SignInAgain,
        OwnerAccountState::Misconfigured,
        OwnerAccountState::Later { until: Some(Wall::EPOCH) },
        OwnerAccountState::Unloaded { at: OwnerAccountPlace::File, why: OwnerAccountUnread::Refused },
        OwnerAccountState::Cancelled,
        OwnerAccountState::Closing,
    ]
}
#[test]
fn deployment_accounts_five_states_independent_flags_spend_overlay_and_generation_reach_both_carriers() {
    for state in account_states() {
        for flags in 0_u8..8 {
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
                    usable: flags & 1 != 0,
                    expiring: flags & 2 != 0,
                    attention: flags & 4 != 0,
                    spent_until: if flags & 1 == 0 { None } else { Some(Wall::EPOCH) },
                    runs: list(vec![chip(0)]),
                    waiting: list(vec![chip(u64::MAX)]),
                    cut: flags & 2 != 0,
                })
                .unwrap();
            round_trip(value);
        }
    }
}
#[test]
fn deployment_three_hello_refusals_keep_advertised_stop_bound_allowed_grace_and_original_clock() {
    for why in [
        OwnerHelloRefusal::Position { worker: u32::MAX },
        OwnerHelloRefusal::ReservedChannel,
        OwnerHelloRefusal::Workers { limit: u32::MAX },
        OwnerHelloRefusal::StopBound { grace: Duration::from_nanos(17) },
    ] {
        let mut value = empty();
        value
            .hello_refusals
            .push(OwnerHelloRefusalLine {
                channel: u64::MAX,
                at: Wall::EPOCH,
                slots: u32::MAX,
                stop_bound: Duration::from_nanos(19),
                why,
            })
            .unwrap();
        round_trip(value);
    }
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
#[test]
fn deployment_person_actions_four_causes_all_structural_limits_and_opaque_request_keys_survive() {
    use AdmissionLimit as L;
    use OwnerPersonCapacityAction as A;
    let actions = [
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
fn deployment_task_actions_fourteen_causes_three_requesters_and_four_funders_keep_full_width_values() {
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
fn deployment_heap_counts_every_reserved_list_box_chip_text_and_exact_carrier_threshold_before_mutation() {
    let mut value = empty();
    let capacity_bytes = 2
        * (size_of::<OwnerHost>()
            + size_of::<OwnerHostRun>()
            + size_of::<OwnerAccount>()
            + size_of::<OwnerProjectCapacity>()
            + size_of::<OwnerHelloRefusalLine>()
            + size_of::<OwnerPersonCapacityRefusal>()
            + size_of::<OwnerTaskCapacityRefusal>()
            + size_of::<u16>());
    let bounds = Bounds { rows: 2, text: 4, ..Bounds::default() };
    assert_eq!(value.heap(&bounds), Some(u64::try_from(capacity_bytes).unwrap()));
    let item = chip(7);
    let chip_heap = size_of::<ChipPhase>() + 4 + 2 * size_of::<u64>();
    value
        .hosts
        .push(OwnerHost {
            name: None,
            awaiting_logins: false,
            rule: HostRule::Open,
            programs: List::with_capacity(0),
            channel: 1,
            kind: OwnerHostKind::Engine,
            slots: 0,
            used: 0,
            free: 0,
            draining: false,
            connected: true,
            grace_until: Some(Wall::EPOCH),
            grace_remaining: None,
            runs: list(vec![run(
                OwnerTaskLink::Live(item.clone()),
                OwnerAttemptState::Claimed,
                OwnerRunLocation::None,
            )]),
            cut: true,
        })
        .unwrap();
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
            generation: Some(2),
            state: OwnerAccountState::Granted,
            usable: true,
            expiring: false,
            attention: false,
            spent_until: None,
            runs: list(vec![item.clone()]),
            waiting: list(vec![item]),
            cut: false,
        })
        .unwrap();
    let expected =
        u64::try_from(capacity_bytes + size_of::<OwnerHostRun>() + 2 * size_of::<Chip>() + 3 * chip_heap).unwrap();
    assert_eq!(value.heap(&bounds), Some(expected));
    let mut exact = bounds;
    exact.document_bytes = u64::try_from(size_of::<OwnerDeployment>()).unwrap() + expected;
    assert!(crate::owner_deployment::fits(&value, &exact));
    exact.document_bytes -= 1;
    assert!(!crate::owner_deployment::fits(&value, &exact));
    let read = ReadValue::OwnerDeployment(Box::new(value.clone()));
    exact.document_bytes = u64::try_from(size_of::<ReadValue>() + size_of::<OwnerDeployment>()).unwrap() + expected;
    assert!(crate::payload::fits(&read, &exact));
    exact.document_bytes -= 1;
    assert!(!crate::payload::fits(&read, &exact));
    let delivery = Delivery::OwnerDeployment(Box::new(value.clone()));
    exact.document_bytes = u64::try_from(size_of::<Delivery>() + size_of::<OwnerDeployment>()).unwrap() + expected;
    assert!(crate::payload::fits(&delivery, &exact));
    exact.document_bytes -= 1;
    assert!(!crate::payload::fits(&delivery, &exact));
    let mut setup = script();
    setup.bounds = Bounds { text: 32, ..bounds };
    let mut engine = Engine::new(setup, 1).unwrap();
    let mut oversized = value;
    oversized.accounts = List::with_capacity(3);
    assert_eq!(engine.act(Time::ZERO, Wall::EPOCH, Change::OwnerDeployment(oversized)), Err(Invalid::Payload));
    assert!(engine.commits().is_empty());
}
fn owner_chip_fixture(number: u64) -> Chip {
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
            markers: 0,
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
fn deployment_whole_owner_chip_resources_terminal_result_observation_and_nested_boxes_have_independent_heap_proof() {
    let bounds = Bounds { rows: 2, text: 8, ..Bounds::default() };
    let mut value = empty();
    let outer = value.heap(&bounds).unwrap();
    value
        .attempts
        .push(run(
            OwnerTaskLink::Live(owner_chip_fixture(u64::MAX)),
            OwnerAttemptState::Claimed,
            OwnerRunLocation::Connected { channel: u64::MAX, kind: OwnerHostKind::Engine },
        ))
        .unwrap();
    let path = 2 * size_of::<Box<[u8]>>() + 5;
    let chip = 4
        + size_of::<ChipPhase>()
        + 2 * size_of::<u64>()
        + size_of::<OwnerChipDetails>()
        + 2 * size_of::<OwnerResourceLine>()
        + 2 * path
        + 2 * size_of::<u16>()
        + path
        + 4
        + 7;
    assert_eq!(value.heap(&bounds), Some(outer + u64::try_from(chip).unwrap()));
    round_trip(value.clone());
    let mut oversized = value;
    let OwnerTaskLink::Live(chip) = &mut oversized.attempts.get_mut(0).unwrap().task else { unreachable!() };
    let ChipPhase::Owner { details, .. } = chip.details.as_mut() else { unreachable!() };
    details.resources = List::with_capacity(3);
    assert!(oversized.heap(&bounds).is_none());
}
#[test]
fn deployment_configured_connector_references_preserve_reserved_capacity_order_empty_and_independent_cut() {
    for connectors_cut in [false, true] {
        let mut value = empty();
        value.connectors_cut = connectors_cut;
        round_trip(value.clone());
        value.connectors.push(u16::MAX).unwrap();
        value.connectors.push(0).unwrap();
        round_trip(value.clone());
        value.connectors = List::with_capacity(3);
        assert!(value.heap(&Bounds { rows: 2, ..Bounds::default() }).is_none());
    }
}
