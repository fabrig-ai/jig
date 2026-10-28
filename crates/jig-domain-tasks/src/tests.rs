use crate::{Retry, failures::backoff};
use skein_lib::{Duration, Rng};

#[test]
fn exponential_equal_jitter_saturates_without_overflow_and_replays() {
    let retry = Retry { retries: 4, base: Duration::from_millis(10), max: Duration::from_secs(1) };
    let mut first = Rng::new(42);
    let mut second = Rng::new(42);
    for times in [1, 2, 3, 4, 7, 32, 64, u32::MAX] {
        let factor = 1_u64.checked_shl(times.saturating_sub(1)).unwrap_or(u64::MAX);
        let ceiling = retry.base.saturating_mul(factor).min(retry.max);
        let pause = backoff(times, retry, &mut first);
        assert!(
            pause.as_nanos() >= ceiling.as_nanos().div_euclid(2) && pause <= ceiling,
            "pause stays in equal-jitter half-to-ceiling range"
        );
        assert_eq!(pause, backoff(times, retry, &mut second), "same seed replays backoff");
    }
}

/// One configured charter, number 1, with the one configured model, which declares no effort.
pub(crate) fn charters() -> Box<[crate::CharterModels]> {
    Box::new([crate::CharterModels { number: 1, revision: 1, models: Some(Box::new([0])) }])
}

mod decision_reads;
mod hold_origin;
mod kept;
mod keys;
mod refusals;
mod revisions;
mod waiting;

mod periods;

mod last_batch;

mod changed;

mod address;
mod instructions;

mod deadlines;

mod index;

mod payer;

mod account;

mod waits;

mod settings_proposals;

mod startup;

#[test]
fn valid_refuses_each_unrepresentable_limit() {
    let limits = revisions::LIMITS;
    for (name, limits, accepted) in [
        ("fixture", limits, true),
        ("placed_holds zero", crate::Limits { placed_holds: 0, ..limits }, false),
        ("project_placed_holds zero", crate::Limits { project_placed_holds: 0, ..limits }, false),
        ("party_placed_holds zero", crate::Limits { party_placed_holds: 0, ..limits }, false),
        ("exemptions zero", crate::Limits { exemptions: 0, ..limits }, false),
        ("circle zero", crate::Limits { circle: 0, ..limits }, false),
        ("tasks zero", crate::Limits { tasks: 0, ..limits }, false),
        ("batch zero", crate::Limits { batch: 0, ..limits }, false),
        ("tree_tasks zero", crate::Limits { tree_tasks: 0, ..limits }, false),
        ("run_inbox zero", crate::Limits { run_inbox: 0, ..limits }, false),
        ("inbox_messages zero", crate::Limits { inbox_messages: 0, ..limits }, false),
        ("inbox_bytes zero", crate::Limits { inbox_bytes: 0, ..limits }, false),
        ("message_bytes zero", crate::Limits { message_bytes: 0, ..limits }, false),
        ("exemption range", crate::Limits { exemptions: limits.tasks + 1, ..limits }, false),
        ("run inbox range", crate::Limits { run_inbox: limits.inbox_messages + 1, ..limits }, false),
        ("message range", crate::Limits { message_bytes: limits.inbox_bytes + 1, ..limits }, false),
        ("grace", crate::Limits { grace: Duration::ZERO, ..limits }, false),
        ("hold_wait", crate::Limits { hold_wait: Duration::ZERO, ..limits }, false),
        ("person_stall", crate::Limits { person_stall: Duration::ZERO, ..limits }, false),
        ("proposal_stall", crate::Limits { proposal_stall: Duration::ZERO, ..limits }, false),
        ("escalation_stall", crate::Limits { escalation_stall: Duration::ZERO, ..limits }, false),
        ("output count", crate::Limits { batch: u32::MAX, ..limits }, false),
        ("stub capacity", crate::Limits { dependencies: u32::MAX, ..limits }, false),
        ("wait resources", crate::Limits { tasks: 65_536, holdings: 1, subscriptions: 0, ..limits }, false),
        ("charter doubling", crate::Limits { charters: u32::MAX, ..limits }, false),
        ("charter addition", crate::Limits { charters: 1, versions: u32::MAX, ..limits }, false),
    ] {
        assert_eq!(crate::valid(&limits), accepted, "{name}");
    }
}

#[test]
fn validity_checks_each_retry_class() {
    for class in [
        crate::Class::Transient,
        crate::Class::Permanent,
        crate::Class::Run,
        crate::Class::Agent,
        crate::Class::Lost,
        crate::Class::Invalid,
    ] {
        for retry in [
            Retry { retries: u32::MAX, base: Duration::from_nanos(1), max: Duration::from_nanos(2) },
            Retry { retries: 1, base: Duration::ZERO, max: Duration::from_nanos(2) },
            Retry { retries: 1, base: Duration::from_nanos(2), max: Duration::from_nanos(1) },
        ] {
            let mut limits = revisions::LIMITS;
            match class {
                crate::Class::Transient => limits.retries.transient = retry,
                crate::Class::Permanent => limits.retries.permanent = retry,
                crate::Class::Run => limits.retries.run = retry,
                crate::Class::Agent => limits.retries.agent = retry,
                crate::Class::Lost => limits.retries.lost = retry,
                crate::Class::Invalid => limits.retries.invalid = retry,
            }
            assert!(!crate::valid(&limits), "{class:?}: {retry:?}");
        }
    }
}

mod wait_validity {
    #[test]
    fn valid_refuses_each_unrepresentable_limit() {
        let limits = super::revisions::LIMITS;
        for (name, limits, accepted) in [
            ("fixture", limits, true),
            ("squared tasks", crate::Limits { tasks: 65_536, holdings: 0, ..limits }, false),
            ("resource product", crate::Limits { tasks: 256, holdings: 65_536, ..limits }, false),
            ("dependencies", crate::Limits { tasks: 2, dependencies: u32::MAX, ..limits }, false),
        ] {
            assert_eq!(crate::waits::valid(&limits), accepted, "{name}");
        }
    }
}
