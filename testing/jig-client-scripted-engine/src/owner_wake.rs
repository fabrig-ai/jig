//! Price complete wake facts at the scripted decoding boundary.
//! See client/domain/contract.md, 5.3; domain/tasks.md, 8.3–8.4.
use crate::{Bounds, payload::Payload};
use jig_client_domain::contract::{
    OwnerWakePolicy, ResultsWake, Subscription, SubscriptionKind, Subscriptions, WakeRule,
};

impl Payload for WakeRule {
    fn heap(&self, _bounds: &Bounds) -> Option<u64> {
        match self {
            Self::Never | Self::Immediate | Self::Batch { count: _, age: _ } => Some(0),
        }
    }
}
impl Payload for ResultsWake {
    fn heap(&self, _bounds: &Bounds) -> Option<u64> {
        match self {
            Self::Never | Self::Each | Self::LastOrFailure => Some(0),
        }
    }
}
impl Payload for OwnerWakePolicy {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        let Self { words, notices, news, results, questions, answers, timers, markers } = self;
        words
            .heap(bounds)?
            .checked_add(notices.heap(bounds)?)?
            .checked_add(news.heap(bounds)?)?
            .checked_add(results.heap(bounds)?)?
            .checked_add(questions.heap(bounds)?)?
            .checked_add(answers.heap(bounds)?)?
            .checked_add(timers.heap(bounds)?)?
            .checked_add(markers.heap(bounds)?)
    }
}
impl Payload for SubscriptionKind {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        match self {
            Self::Task { target, held, result } => {
                target.heap(bounds)?.checked_add(held.heap(bounds)?)?.checked_add(result.heap(bounds)?)
            }
            Self::Timer { at, period } => at.heap(bounds)?.checked_add(period.heap(bounds)?),
            Self::Topic { connector, topic } => connector.heap(bounds)?.checked_add(topic.heap(bounds)?),
        }
    }
}
impl Payload for Subscription {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        let Self { number, kind } = self;
        number.heap(bounds)?.checked_add(kind.heap(bounds)?)
    }
}
impl Payload for Subscriptions {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        let Self { entries } = self;
        entries.heap(bounds)
    }
}

/// Whole policy storage fits; semantic admission remains the engine owner's.
#[must_use]
pub fn policy_fits(value: &OwnerWakePolicy, bounds: &Bounds) -> bool {
    crate::payload::fits(value, bounds)
}

/// Price reserved list capacity, including entries not yet filled.
#[must_use]
pub fn subscriptions_fits(value: &Subscriptions, bounds: &Bounds) -> bool {
    crate::payload::fits(value, bounds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;
    use skein_lib::{Duration, List, Wall};

    #[test]
    fn owner_wake_prices_every_rule_result_and_switch_as_a_whole_value() {
        let bytes = u64::try_from(size_of::<OwnerWakePolicy>()).unwrap();
        for rule in [
            WakeRule::Never,
            WakeRule::Immediate,
            WakeRule::Batch { count: u32::MAX, age: Duration::from_nanos(u64::MAX) },
        ] {
            for results in [ResultsWake::Never, ResultsWake::Each, ResultsWake::LastOrFailure] {
                for switches in 0..8 {
                    let value = OwnerWakePolicy {
                        markers: 0,
                        words: rule,
                        notices: rule,
                        news: rule,
                        results,
                        questions: switches & 1 != 0,
                        answers: switches & 2 != 0,
                        timers: switches & 4 != 0,
                    };
                    assert!(policy_fits(&value, &Bounds { document_bytes: bytes, ..Bounds::default() }));
                    assert!(!policy_fits(&value, &Bounds { document_bytes: bytes - 1, ..Bounds::default() }));
                }
            }
        }
    }
    #[test]
    fn owner_subscriptions_price_reserved_capacity_and_every_exact_kind() {
        let mut entries = List::with_capacity(7);
        for kind in [
            SubscriptionKind::Task { target: u64::MAX, held: false, result: false },
            SubscriptionKind::Task { target: 2, held: true, result: false },
            SubscriptionKind::Task { target: 3, held: false, result: true },
            SubscriptionKind::Task { target: 4, held: true, result: true },
            SubscriptionKind::Timer { at: Wall::from_nanos(u64::MAX), period: None },
            SubscriptionKind::Timer { at: Wall::EPOCH, period: Some(Duration::from_nanos(u64::MAX)) },
            SubscriptionKind::Topic { connector: u16::MAX, topic: u64::MAX },
        ] {
            entries.push(Subscription { number: u64::MAX, kind }).unwrap();
        }
        let value = Subscriptions { entries };
        let bytes = u64::try_from(size_of::<Subscriptions>()).unwrap() + List::<Subscription>::worst_case(7).unwrap();
        assert!(subscriptions_fits(&value, &Bounds { rows: 7, document_bytes: bytes, ..Bounds::default() }));
        assert!(!subscriptions_fits(&value, &Bounds { rows: 7, document_bytes: bytes - 1, ..Bounds::default() }));
        assert!(!subscriptions_fits(&value, &Bounds { rows: 6, document_bytes: bytes, ..Bounds::default() }));
        let reserved = Subscriptions { entries: List::with_capacity(7) };
        assert!(subscriptions_fits(&reserved, &Bounds { rows: 7, document_bytes: bytes, ..Bounds::default() }));
        assert!(!subscriptions_fits(&reserved, &Bounds { rows: 7, document_bytes: bytes - 1, ..Bounds::default() }));
    }
    #[test]
    fn owner_timer_absence_and_whole_numeric_identities_survive_clone_and_hash() {
        use std::hash::{Hash, Hasher};
        let once = Subscription {
            number: u64::MAX,
            kind: SubscriptionKind::Timer { at: Wall::from_nanos(u64::MAX), period: None },
        };
        let periodic = Subscription {
            kind: SubscriptionKind::Timer { at: Wall::from_nanos(u64::MAX), period: Some(Duration::ZERO) },
            ..once
        };
        assert_ne!(once, periodic);
        let mut a = std::collections::hash_map::DefaultHasher::new();
        let mut b = std::collections::hash_map::DefaultHasher::new();
        once.hash(&mut a);
        periodic.hash(&mut b);
        assert_ne!(a.finish(), b.finish());
        let mut entries = List::with_capacity(2);
        entries.push(once).unwrap();
        entries.push(periodic).unwrap();
        let value = Subscriptions { entries };
        assert_eq!(value.clone(), value);
        assert_eq!(value.entries.capacity(), 2);
    }
}
