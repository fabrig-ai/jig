//! Owner loss, placement clocks, opaque identities and complete envelope admission.
use super::*;
use skein_lib::{Duration, List, Wall};

pub(crate) fn limits() -> Limits {
    Limits { rows: 8, text: 200, turn_text: 200, document_bytes: 100_000, ..crate::test::limits().client.domain }
}

fn rows<T>(capacity: u32, values: impl IntoIterator<Item = T>) -> List<T> {
    let mut result = List::with_capacity(capacity);
    for value in values {
        result.push(value).ok().expect("reserved fixture capacity");
    }
    result
}

fn source_run() -> documents::HostRun {
    documents::HostRun {
        task: documents::TaskLink::Stored { number: u64::MAX, status: None },
        attempt: 0,
        kept: u32::MAX,
        listed: false,
        state: fleet::AttemptState::Waiting,
        location: documents::RunLocation::Adrift { channel: Some(0), until: Some(Wall::EPOCH), remaining: None },
        grace_until: None,
    }
}

#[expect(clippy::too_many_lines, reason = "one complete deployment fixture exercises every document field")]
pub(crate) fn source() -> documents::Deployment {
    let mut run = source_run();
    run.state = fleet::AttemptState::Kept;
    let chip = chips::tests::source_chip();
    documents::Deployment {
        providers: skein_lib::List::with_capacity(0),
        providers_cut: false,
        key_bytes: 256,
        readiness: skein_lib::List::with_capacity(0),
        readiness_cut: false,
        store: documents::StoreFill { bytes: u64::MAX, mark: 0 },
        hosts: rows(
            2,
            [documents::Host {
                name: None,
                awaiting_logins: false,
                rule: jig_domain_core::HostRule::Open,
                programs: skein_lib::List::with_capacity(0),
                channel: u64::MAX,
                kind: fleet::HostKind::Engine,
                slots: u32::MAX,
                used: 7,
                free: 0,
                draining: true,
                connected: false,
                grace_until: Some(Wall::EPOCH),
                grace_remaining: Some(Duration::ZERO),
                runs: rows(3, [run.clone()]),
                cut: true,
            }],
        ),
        attempts: rows(4, [run]),
        accounts: rows(
            3,
            [documents::Account {
                provider_name: None,
                manage_usage: None,
                place: 0,
                uses: None,
                plan: false,
                provider: None,
                display: documents::Text { bytes: Box::new([]), cut: false },
                holder: jig_domain_accounts::Holder::Deployment,
                sources: List::with_capacity(0),
                number: u32::MAX,
                generation: Some(u64::MAX),
                state: documents::AccountState::SignInAgain,
                usable: false,
                expiring: true,
                attention: true,
                spent_until: Some(Wall::EPOCH),
                runs: rows(2, [chip.clone()]),
                waiting: rows(3, [chip]),
                cut: false,
            }],
        ),
        tasks: documents::Capacity { used: u32::MAX, limit: 0 },
        workers: documents::Capacity { used: 2, limit: 3 },
        attempts_capacity: documents::Capacity { used: 5, limit: 7 },
        waiting_attempts: u32::MAX,
        proposals: documents::Capacity { used: 11, limit: 13 },
        projects: rows(
            5,
            [documents::ProjectCapacity {
                project: u32::MAX,
                tasks: documents::Capacity { used: 17, limit: 19 },
                proposals: 23,
            }],
        ),
        grace_bound: Duration::from_nanos(u64::MAX),
        hello_refusals: rows(
            4,
            [documents::HelloRefusalLine {
                channel: 0,
                at: Wall::from_nanos(u64::MAX),
                slots: u32::MAX,
                stop_bound: Duration::ZERO,
                why: fleet::HelloRefusal::StopBound { grace: Duration::from_nanos(29) },
            }],
        ),
        hosts_forgotten: u64::MAX,
        hello_refusals_forgotten: 31,
        person_room_refusals: rows(
            6,
            [people::CapacityRefusal {
                key: people::RequestKey {
                    person: u64::MAX,
                    key: [255, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 254],
                },
                project: Some(0),
                action: people::CapacityAction::Question { task: u64::MAX, question: 0 },
                at: Wall::EPOCH,
                cause: people::CapacityCause::Waiters { limit: u32::MAX },
            }],
        ),
        person_room_refusals_forgotten: 37,
        person_answer_retention: Duration::ZERO,
        task_room_refusals: rows(
            7,
            [tasks::CapacityRefusal {
                request: tasks::Capacity {
                    project: None,
                    by: tasks::Party::Deployment { project: u32::MAX },
                    action: tasks::CapacityAction::Batch { first: None, members: u32::MAX },
                    cause: tasks::CapacityCause::Inbox { task: u64::MAX, messages: 0, bytes: u32::MAX },
                },
                at: Wall::from_nanos(u64::MAX),
            }],
        ),
        task_room_refusals_forgotten: 41,
        connectors: rows(8, [u16::MAX, 0, 2, 1]),
        connectors_cut: true,
        cut: false,
    }
}

#[test]
fn deployment_keeps_every_independent_capacity_history_loss_and_configured_connector_order() {
    let source = source();
    let value = document(&source, &limits()).expect("whole deployment");
    assert_eq!(value.tasks, client::OwnerCapacity { used: u32::MAX, limit: 0 });
    assert_eq!(value.workers, client::OwnerCapacity { used: 2, limit: 3 });
    assert_eq!(value.attempts_capacity, client::OwnerCapacity { used: 5, limit: 7 });
    assert_eq!(value.proposals, client::OwnerCapacity { used: 11, limit: 13 });
    assert_eq!(value.waiting_attempts, u32::MAX);
    assert_eq!(
        value.projects.as_slice(),
        &[client::OwnerProjectCapacity {
            project: u32::MAX,
            tasks: client::OwnerCapacity { used: 17, limit: 19 },
            proposals: 23,
        }]
    );
    assert_eq!(value.grace_bound, Duration::from_nanos(u64::MAX));
    assert_eq!((value.hosts_forgotten, value.hello_refusals_forgotten), (u64::MAX, 31));
    assert_eq!((value.person_room_refusals_forgotten, value.task_room_refusals_forgotten), (37, 41));
    assert_eq!(value.person_answer_retention, Duration::ZERO);
    assert_eq!(value.connectors.as_slice(), &[u16::MAX, 0, 2, 1]);
    assert!(value.connectors_cut);
    assert!(!value.cut);
    assert_eq!(
        [
            value.hosts.capacity(),
            value.attempts.capacity(),
            value.accounts.capacity(),
            value.projects.capacity(),
            value.hello_refusals.capacity(),
            value.person_room_refusals.capacity(),
            value.task_room_refusals.capacity(),
            value.connectors.capacity()
        ],
        [2, 4, 3, 5, 4, 6, 7, 8]
    );
    let hello = value.hello_refusals.get(0).expect("supplied fixture value");
    assert_eq!(
        hello,
        &client::OwnerHelloRefusalLine {
            channel: 0,
            at: Wall::from_nanos(u64::MAX),
            slots: u32::MAX,
            stop_bound: Duration::ZERO,
            why: client::OwnerHelloRefusal::StopBound { grace: Duration::from_nanos(29) },
        }
    );
}

#[test]
fn deployment_keeps_host_placement_and_secret_free_account_flags_without_deriving_free_or_usable() {
    let value = document(&source(), &limits()).expect("supplied fixture value");
    let host = value.hosts.get(0).expect("supplied fixture value");
    assert_eq!(
        (host.channel, host.kind, host.slots, host.used, host.free),
        (u64::MAX, client::OwnerHostKind::Engine, u32::MAX, 7, 0)
    );
    assert_eq!(host.rule, client::HostRule::Open);
    assert!(host.draining && !host.connected && host.cut);
    assert_eq!((host.grace_until, host.grace_remaining), (Some(Wall::EPOCH), Some(Duration::ZERO)));
    assert_eq!(host.runs.capacity(), 3);
    assert_eq!(host.runs.get(0), value.attempts.get(0));
    let run = value.attempts.get(0).expect("supplied fixture value");
    assert_eq!(run.task, client::OwnerTaskLink::Stored { number: u64::MAX, status: None });
    assert_eq!((run.attempt, run.kept, run.listed, run.state), (0, u32::MAX, false, client::OwnerAttemptState::Kept));
    assert_eq!(
        run.location,
        client::OwnerRunLocation::Adrift { channel: Some(0), until: Some(Wall::EPOCH), remaining: None }
    );
    assert_eq!(run.grace_until, None);
    let account = value.accounts.get(0).expect("supplied fixture value");
    assert_eq!(
        (account.number, account.generation, account.state),
        (u32::MAX, Some(u64::MAX), client::OwnerAccountState::SignInAgain)
    );
    assert!(!account.usable && account.expiring && account.attention && !account.cut);
    assert_eq!(account.spent_until, Some(Wall::EPOCH));
    assert_eq!((account.runs.capacity(), account.waiting.capacity()), (2, 3));
    let expected = chips::chip(&chips::tests::source_chip(), &limits()).expect("supplied fixture value");
    assert_eq!(account.runs.as_slice(), std::slice::from_ref(&expected));
    assert_eq!(account.waiting.as_slice(), &[expected]);
}

#[test]
fn deployment_refusals_keep_opaque_request_keys_original_callers_and_absent_project_or_first_member() {
    let value = document(&source(), &limits()).expect("supplied fixture value");
    assert_eq!(
        value.person_room_refusals.get(0),
        Some(&client::OwnerPersonCapacityRefusal {
            key: client::OwnerRequestKey {
                person: u64::MAX,
                key: [255, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 254]
            },
            project: Some(0),
            action: client::OwnerPersonCapacityAction::Question { task: u64::MAX, question: 0 },
            at: Wall::EPOCH,
            cause: client::OwnerPersonCapacityCause::Waiters { limit: u32::MAX },
        })
    );
    assert_eq!(
        value.task_room_refusals.get(0),
        Some(&client::OwnerTaskCapacityRefusal {
            request: client::OwnerTaskCapacityRequest {
                project: None,
                by: client::OwnerRequester::Deployment { project: u32::MAX },
                action: client::OwnerTaskCapacityAction::Batch { first: None, members: u32::MAX },
                cause: client::OwnerTaskCapacityCause::Inbox { task: u64::MAX, messages: 0, bytes: u32::MAX },
            },
            at: Wall::from_nanos(u64::MAX),
        })
    );
}

#[test]
fn deployment_run_locations_distinguish_all_absent_and_present_zero_clocks_and_channels() {
    for location in [
        documents::RunLocation::None,
        documents::RunLocation::Connected { channel: u64::MAX, kind: fleet::HostKind::Worker },
        documents::RunLocation::Connected { channel: 0, kind: fleet::HostKind::Engine },
        documents::RunLocation::Adrift { channel: None, until: None, remaining: None },
        documents::RunLocation::Adrift { channel: Some(0), until: Some(Wall::EPOCH), remaining: Some(Duration::ZERO) },
        documents::RunLocation::Adrift {
            channel: Some(u64::MAX),
            until: Some(Wall::from_nanos(u64::MAX)),
            remaining: Some(Duration::from_nanos(u64::MAX)),
        },
    ] {
        let mut source = source_run();
        source.location = location;
        let value = run(&source, &limits()).expect("supplied fixture value");
        match (location, value.location) {
            (documents::RunLocation::None, client::OwnerRunLocation::None) => {}
            (
                documents::RunLocation::Connected { channel: a, kind: ak },
                client::OwnerRunLocation::Connected { channel: b, kind: bk },
            ) => {
                assert_eq!(a, b);
                assert_eq!(host_kind(ak), bk);
            }
            (
                documents::RunLocation::Adrift { channel: a, until: au, remaining: ar },
                client::OwnerRunLocation::Adrift { channel: b, until: bu, remaining: br },
            ) => assert_eq!((a, au, ar), (b, bu, br)),
            pair => panic!("location changed: {pair:?}"),
        }
    }
}

#[test]
fn deployment_whole_read_and_watch_admit_their_own_complete_boxed_envelope_byte_boundary() {
    let source = source();
    let limits = limits();
    let original = source.clone();
    let read = super::super::documents::deployment_read(&source, &limits).expect("supplied fixture value");
    let delivery = super::super::documents::deployment_delivery(&source, &limits).expect("supplied fixture value");
    for watch in [false, true] {
        let accepted = |bytes| {
            let limits = Limits { document_bytes: bytes, turn_text: limits.turn_text, ..limits };
            if watch {
                super::super::documents::deployment_delivery(&source, &limits).is_ok()
            } else {
                super::super::documents::deployment_read(&source, &limits).is_ok()
            }
        };
        let mut lower = 0;
        let mut upper = limits.document_bytes;
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            if accepted(middle) {
                upper = middle;
            } else {
                lower = middle + 1;
            }
        }
        assert!(lower > 0 && accepted(lower) && !accepted(lower - 1));
        let exact = Limits { document_bytes: lower, turn_text: limits.turn_text, ..limits };
        if watch {
            assert_eq!(super::super::documents::deployment_delivery(&source, &exact), Ok(delivery.clone()));
        } else {
            assert_eq!(super::super::documents::deployment_read(&source, &exact), Ok(read.clone()));
        }
    }
    assert_eq!(source, original);
}

#[test]
fn deployment_refuses_reserved_empty_outer_nested_and_last_chip_allocations_without_trimming() {
    let limits = limits();
    let mut source = source();
    source.connectors = List::with_capacity(limits.rows + 1);
    assert_eq!(document(&source, &limits).err(), Some(owner::Invalid::TooLarge));
    source = self::source();
    source.hosts.get_mut(0).expect("supplied fixture value").runs = List::with_capacity(limits.rows + 1);
    assert_eq!(document(&source, &limits).err(), Some(owner::Invalid::TooLarge));
    source = self::source();
    source.accounts.get_mut(0).expect("supplied fixture value").waiting = List::with_capacity(limits.rows + 1);
    assert_eq!(document(&source, &limits).err(), Some(owner::Invalid::TooLarge));
    source = self::source();
    source
        .accounts
        .get_mut(0)
        .expect("supplied fixture value")
        .waiting
        .get_mut(0)
        .expect("supplied fixture value")
        .title
        .bytes = vec![255; 201].into_boxed_slice();
    assert_eq!(document(&source, &limits).err(), Some(owner::Invalid::TooLarge));
}

#[test]
fn deployment_attempt_and_account_lifecycles_preserve_every_supplied_state() {
    for (source, expected) in [
        (fleet::AttemptState::Waiting, client::OwnerAttemptState::Waiting),
        (fleet::AttemptState::Adopted, client::OwnerAttemptState::Adopted),
        (fleet::AttemptState::Claimed, client::OwnerAttemptState::Claimed),
        (fleet::AttemptState::Cancelled, client::OwnerAttemptState::Cancelled),
        (fleet::AttemptState::Handed, client::OwnerAttemptState::Handed),
        (fleet::AttemptState::Acknowledged, client::OwnerAttemptState::Acknowledged),
        (fleet::AttemptState::Stray, client::OwnerAttemptState::Stray),
        (fleet::AttemptState::Kept, client::OwnerAttemptState::Kept),
        (fleet::AttemptState::Fenced, client::OwnerAttemptState::Fenced),
        (fleet::AttemptState::Closed, client::OwnerAttemptState::Closed),
    ] {
        assert_eq!(attempt(source), expected);
    }
}

#[test]
fn deployment_accounts_preserve_every_supplied_state() {
    for (source, expected) in [
        (documents::AccountState::Granted, client::OwnerAccountState::Granted),
        (documents::AccountState::Asking, client::OwnerAccountState::Asking),
        (documents::AccountState::SignInAgain, client::OwnerAccountState::SignInAgain),
        (documents::AccountState::Misconfigured, client::OwnerAccountState::Misconfigured),
        (
            documents::AccountState::Later { until: Some(Wall::EPOCH) },
            client::OwnerAccountState::Later { until: Some(Wall::EPOCH) },
        ),
        (documents::AccountState::Later { until: None }, client::OwnerAccountState::Later { until: None }),
        (
            documents::AccountState::Unloaded { at: accounts::Place::File, why: accounts::Unread::Refused },
            client::OwnerAccountState::Unloaded {
                at: client::OwnerAccountPlace::File,
                why: client::OwnerAccountUnread::Refused,
            },
        ),
        (documents::AccountState::Cancelled, client::OwnerAccountState::Cancelled),
        (documents::AccountState::Closing, client::OwnerAccountState::Closing),
    ] {
        assert_eq!(account_state(source), expected);
    }
}

#[test]
fn deployment_hello_refusals_keep_each_owner_reason_and_its_full_width_bound() {
    for (source, expected) in [
        (fleet::HelloRefusal::Position { worker: u32::MAX }, client::OwnerHelloRefusal::Position { worker: u32::MAX }),
        (fleet::HelloRefusal::ReservedChannel, client::OwnerHelloRefusal::ReservedChannel),
        (fleet::HelloRefusal::Workers { limit: u32::MAX }, client::OwnerHelloRefusal::Workers { limit: u32::MAX }),
        (
            fleet::HelloRefusal::StopBound { grace: Duration::ZERO },
            client::OwnerHelloRefusal::StopBound { grace: Duration::ZERO },
        ),
    ] {
        assert_eq!(hello(source), expected);
    }
}

#[test]
fn deployment_person_capacity_actions_keep_every_original_kind_and_address() {
    for (source, expected) in [
        (people::CapacityAction::Create, client::OwnerPersonCapacityAction::Create),
        (people::CapacityAction::Compound, client::OwnerPersonCapacityAction::Compound),
        (
            people::CapacityAction::Application { connector: u16::MAX },
            client::OwnerPersonCapacityAction::Application { connector: u16::MAX },
        ),
        (people::CapacityAction::Note { name: u64::MAX }, client::OwnerPersonCapacityAction::Note { name: u64::MAX }),
        (people::CapacityAction::Service, client::OwnerPersonCapacityAction::Service),
        (people::CapacityAction::Adoption, client::OwnerPersonCapacityAction::Adoption),
        (people::CapacityAction::Roles, client::OwnerPersonCapacityAction::Roles),
        (people::CapacityAction::Policy, client::OwnerPersonCapacityAction::Policy),
        (
            people::CapacityAction::Pool { person: u64::MAX },
            client::OwnerPersonCapacityAction::Pool { person: u64::MAX },
        ),
        (
            people::CapacityAction::Escalation { task: u64::MAX },
            client::OwnerPersonCapacityAction::Escalation { task: u64::MAX },
        ),
        (people::CapacityAction::Words { task: u64::MAX }, client::OwnerPersonCapacityAction::Words { task: u64::MAX }),
        (
            people::CapacityAction::Question { task: 0, question: u64::MAX },
            client::OwnerPersonCapacityAction::Question { task: 0, question: u64::MAX },
        ),
        (people::CapacityAction::Amend { task: u64::MAX }, client::OwnerPersonCapacityAction::Amend { task: u64::MAX }),
        (people::CapacityAction::Move { task: u64::MAX }, client::OwnerPersonCapacityAction::Move { task: u64::MAX }),
        (
            people::CapacityAction::TakePerson { task: u64::MAX },
            client::OwnerPersonCapacityAction::TakePerson { task: u64::MAX },
        ),
        (
            people::CapacityAction::HandBackPerson { task: u64::MAX },
            client::OwnerPersonCapacityAction::HandBackPerson { task: u64::MAX },
        ),
        (
            people::CapacityAction::AnswerPerson { task: u64::MAX },
            client::OwnerPersonCapacityAction::AnswerPerson { task: u64::MAX },
        ),
        (people::CapacityAction::Stop { task: u64::MAX }, client::OwnerPersonCapacityAction::Stop { task: u64::MAX }),
        (
            people::CapacityAction::Cancel { task: u64::MAX },
            client::OwnerPersonCapacityAction::Cancel { task: u64::MAX },
        ),
        (
            people::CapacityAction::Release { task: u64::MAX },
            client::OwnerPersonCapacityAction::Release { task: u64::MAX },
        ),
        (
            people::CapacityAction::Proposal { proposer: people::Proposer::Task(u64::MAX), number: 0 },
            client::OwnerPersonCapacityAction::Proposal { proposer: client::Proposer::Task(u64::MAX), number: 0 },
        ),
        (
            people::CapacityAction::Proposal { proposer: people::Proposer::Party(u64::MAX), number: 0 },
            client::OwnerPersonCapacityAction::Proposal { proposer: client::Proposer::Party(u64::MAX), number: 0 },
        ),
        (
            people::CapacityAction::MarkSeen { seen: people::Seen::Results { through: u64::MAX } },
            client::OwnerPersonCapacityAction::MarkSeen { seen: client::SeenThrough::Results { through: u64::MAX } },
        ),
        (
            people::CapacityAction::MarkSeen { seen: people::Seen::Conversation { task: u64::MAX, through: 0 } },
            client::OwnerPersonCapacityAction::MarkSeen {
                seen: client::SeenThrough::Conversation { task: u64::MAX, through: 0 },
            },
        ),
    ] {
        assert_eq!(person_action(source), expected);
    }
}

#[test]
fn deployment_task_capacity_actions_keep_whole_batch_and_all_original_funder_kinds() {
    for (source, expected) in [
        (
            tasks::CapacityAction::Batch { first: None, members: u32::MAX },
            client::OwnerTaskCapacityAction::Batch { first: None, members: u32::MAX },
        ),
        (
            tasks::CapacityAction::Batch { first: Some(0), members: 0 },
            client::OwnerTaskCapacityAction::Batch { first: Some(0), members: 0 },
        ),
        (
            tasks::CapacityAction::Proposal { number: u64::MAX },
            client::OwnerTaskCapacityAction::Proposal { number: u64::MAX },
        ),
        (
            tasks::CapacityAction::PersonProposal { number: u64::MAX },
            client::OwnerTaskCapacityAction::PersonProposal { number: u64::MAX },
        ),
        (
            tasks::CapacityAction::Message { task: u64::MAX, number: 0 },
            client::OwnerTaskCapacityAction::Message { task: u64::MAX, number: 0 },
        ),
        (
            tasks::CapacityAction::Introduce { left: u64::MAX, right: 0 },
            client::OwnerTaskCapacityAction::Introduce { left: u64::MAX, right: 0 },
        ),
        (
            tasks::CapacityAction::Subscription { number: u64::MAX },
            client::OwnerTaskCapacityAction::Subscription { number: u64::MAX },
        ),
        (
            tasks::CapacityAction::Effect { entry: u64::MAX },
            client::OwnerTaskCapacityAction::Effect { entry: u64::MAX },
        ),
        (
            tasks::CapacityAction::Claim { attempt: u64::MAX },
            client::OwnerTaskCapacityAction::Claim { attempt: u64::MAX },
        ),
        (tasks::CapacityAction::Release, client::OwnerTaskCapacityAction::Release),
        (tasks::CapacityAction::Move, client::OwnerTaskCapacityAction::Move),
        (
            tasks::CapacityAction::Source { funder: tasks::Funder::Task(u64::MAX) },
            client::OwnerTaskCapacityAction::Source { funder: client::Funder::Task(u64::MAX) },
        ),
        (
            tasks::CapacityAction::Source {
                funder: tasks::Funder::Pool { project: u32::MAX, person: 0, period: u64::MAX },
            },
            client::OwnerTaskCapacityAction::Source {
                funder: client::Funder::Pool { project: u32::MAX, person: 0, period: u64::MAX },
            },
        ),
        (
            tasks::CapacityAction::Source {
                funder: tasks::Funder::Recurring { project: 0, task: u64::MAX, period: 0 },
            },
            client::OwnerTaskCapacityAction::Source {
                funder: client::Funder::Recurring { project: 0, task: u64::MAX, period: 0 },
            },
        ),
        (
            tasks::CapacityAction::Source { funder: tasks::Funder::Period { project: u32::MAX, period: u64::MAX } },
            client::OwnerTaskCapacityAction::Source {
                funder: client::Funder::Period { project: u32::MAX, period: u64::MAX },
            },
        ),
    ] {
        assert_eq!(task_action(source), expected);
    }
}

#[test]
fn deployment_task_capacity_causes_preserve_the_exact_failed_room_check() {
    for (source, expected) in [
        (tasks::CapacityCause::Tasks { limit: u32::MAX }, client::OwnerTaskCapacityCause::Tasks { limit: u32::MAX }),
        (
            tasks::CapacityCause::CreationFlights { limit: u32::MAX },
            client::OwnerTaskCapacityCause::CreationFlights { limit: u32::MAX },
        ),
        (
            tasks::CapacityCause::ResourceReports { limit: u32::MAX },
            client::OwnerTaskCapacityCause::ResourceReports { limit: u32::MAX },
        ),
        (
            tasks::CapacityCause::ProjectTasks { limit: u32::MAX },
            client::OwnerTaskCapacityCause::ProjectTasks { limit: u32::MAX },
        ),
        (tasks::CapacityCause::Tree { limit: u32::MAX }, client::OwnerTaskCapacityCause::Tree { limit: u32::MAX }),
        (
            tasks::CapacityCause::Delegates { limit: u32::MAX },
            client::OwnerTaskCapacityCause::Delegates { limit: u32::MAX },
        ),
        (
            tasks::CapacityCause::Inbox { task: u64::MAX, messages: 0, bytes: u32::MAX },
            client::OwnerTaskCapacityCause::Inbox { task: u64::MAX, messages: 0, bytes: u32::MAX },
        ),
        (
            tasks::CapacityCause::Holdings { limit: u32::MAX },
            client::OwnerTaskCapacityCause::Holdings { limit: u32::MAX },
        ),
        (
            tasks::CapacityCause::HoldWaiters { limit: u32::MAX },
            client::OwnerTaskCapacityCause::HoldWaiters { limit: u32::MAX },
        ),
        (
            tasks::CapacityCause::Proposals { limit: u32::MAX },
            client::OwnerTaskCapacityCause::Proposals { limit: u32::MAX },
        ),
        (
            tasks::CapacityCause::References { limit: u32::MAX },
            client::OwnerTaskCapacityCause::References { limit: u32::MAX },
        ),
        (
            tasks::CapacityCause::Subscriptions { limit: u32::MAX },
            client::OwnerTaskCapacityCause::Subscriptions { limit: u32::MAX },
        ),
        (
            tasks::CapacityCause::Funders { limit: u32::MAX },
            client::OwnerTaskCapacityCause::Funders { limit: u32::MAX },
        ),
        (
            tasks::CapacityCause::Writers { limit: u32::MAX },
            client::OwnerTaskCapacityCause::Writers { limit: u32::MAX },
        ),
    ] {
        assert_eq!(task_cause(source), expected);
    }
}

#[test]
fn deployment_person_room_causes_remain_independent_of_action_project_and_opaque_request_key() {
    let source = source().person_room_refusals.get(0).expect("person refusal").to_owned();
    for (cause, expected) in [
        (
            people::CapacityCause::Requests { limit: u32::MAX },
            client::OwnerPersonCapacityCause::Requests { limit: u32::MAX },
        ),
        (people::CapacityCause::Pending { limit: 0 }, client::OwnerPersonCapacityCause::Pending { limit: 0 }),
        (people::CapacityCause::Waiters { limit: 7 }, client::OwnerPersonCapacityCause::Waiters { limit: 7 }),
        (
            people::CapacityCause::Limit { which: people::LimitKind::Arithmetic },
            client::OwnerPersonCapacityCause::Limit { which: client::AdmissionLimit::Arithmetic },
        ),
    ] {
        let row = people::CapacityRefusal { cause, ..source };
        let actual = person_refusal(&row);
        assert_eq!(actual.cause, expected);
        assert_eq!(
            (actual.key.person, actual.key.key, actual.project, actual.at),
            (source.key.person, source.key.key, source.project, source.at)
        );
    }
}

#[test]
fn me_deployment_role_is_independent_of_project_rows() {
    for (source_role, expected) in [
        (None, None),
        (Some(people::Role::Owner), Some(client::HeldRole::Owner)),
        (Some(people::Role::Maintainer), Some(client::HeldRole::Maintainer)),
        (Some(people::Role::Member), Some(client::HeldRole::Member)),
        (Some(people::Role::Observer), Some(client::HeldRole::Observer)),
        (Some(people::Role::Policy { role: u32::MAX }), Some(client::HeldRole::Policy { role: u32::MAX })),
    ] {
        let mut source = documents::Frame {
            providers: skein_lib::List::with_capacity(0),
            providers_cut: false,
            key_bytes: 256,
            redirect_bytes: 4096,
            words_bytes: 65_536,
            amendment_bytes: 65_536,
            party: 17,
            kind: people::Kind::Person,
            login: documents::Text { bytes: Box::new([]), cut: false },
            name: documents::Text { bytes: Box::new([]), cut: true },
            deployment_role: source_role,
            projects: rows(
                1,
                [documents::FrameProjectRole {
                    project: 1,
                    name: jig_domain_core::documents::Text { bytes: Box::from(&b"Tally"[..]), cut: false },
                    role: people::Role::Owner,
                    pool: None,
                    creating: None,
                    models: jig_domain_authority::Models::NONE,
                    templates: List::with_capacity(0),
                }],
            ),
            needs: 0,
            need_counts: people::Needs { questions: 0, proposals: 0, escalations: 0, person_tasks: 0 },
            updates: documents::Count::Known(0),
            results_seen: 0,
            retention: Duration::ZERO,
            cut: true,
        };
        assert_eq!(
            owner::frame(&source, &limits(), &[], &jig_domain_accounts::Providers::none())
                .expect("supplied Me")
                .deployment_role,
            expected
        );
        source.projects = List::with_capacity(1);
        let value = owner::frame(&source, &limits(), &[], &jig_domain_accounts::Providers::none())
            .expect("clipped supplied Me");
        assert_eq!(value.deployment_role, expected);
        assert!(value.cut && value.projects.is_empty());
    }
}

#[test]
fn deployment_keeps_the_store_fill_and_the_mark() {
    let value = document(&source(), &limits()).expect("whole deployment");
    assert_eq!(value.store, client::StoreFill { bytes: u64::MAX, mark: 0 });
}
