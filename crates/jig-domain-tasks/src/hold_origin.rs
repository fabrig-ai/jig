//! Bounded references to connector explanations (`client/domain/objects.md`, 5) and the circles
//! of waits that hold a task (domain/tasks.md, 6.7).
use crate::{Circle, Hold, HoldSource, Limits};
/// Maximum retained connector account, independently of inbox message bounds.
pub const ACCOUNT_BYTES: u32 = 512;
pub(crate) fn valid(limits: &Limits, why: Hold, source: Option<&HoldSource>) -> bool {
    match source {
        None => why != Hold::Circle,
        Some(HoldSource::Account { entry, words, .. }) => {
            if let Hold::Uncertain { entry: held } = why
                && *entry != Some(held)
            {
                return false;
            }
            why != Hold::Circle
                && *entry != Some(0)
                && !words.is_empty()
                && words.len() <= usize::try_from(ACCOUNT_BYTES).expect("bounded account")
        }
        Some(HoldSource::Circle(circle)) => why == Hold::Circle && valid_circle(limits, circle),
        Some(HoldSource::Resource(name)) => why == Hold::Drift && crate::holds::valid_name(limits, name),
        Some(HoldSource::Effect { entry, .. }) => {
            *entry != 0
                && match why {
                    Hold::EffectFailed | Hold::Effects => true,
                    Hold::Uncertain { entry: held } => held == *entry,
                    Hold::Exceeded(_)
                    | Hold::Failures(_)
                    | Hold::Stalled
                    | Hold::Dependency(_)
                    | Hold::Stopped
                    | Hold::Placed { .. }
                    | Hold::Drift
                    | Hold::Budget
                    | Hold::Deadline
                    | Hold::HoldsWaited
                    | Hold::ResourceUnavailable
                    | Hold::ResourceAccess
                    | Hold::PoolLost { .. }
                    | Hold::Procedure
                    | Hold::Circle => false,
                }
        }
    }
}

/// A circle names at least one task, none twice, and is cut only at its limit.
pub(crate) fn valid_circle(limits: &Limits, circle: &Circle) -> bool {
    let Ok(count) = u32::try_from(circle.tasks.len()) else { return false };
    if count == 0 || count > limits.circle || (circle.cut && count != limits.circle) {
        return false;
    }
    for (index, task) in circle.tasks.iter().enumerate() {
        let Some(before) = circle.tasks.get(..index) else { return false };
        if *task == 0 || before.contains(task) {
            return false;
        }
    }
    true
}
