//! Revision checks at the decision boundary (client/domain/contract.md,
//! section 5.2; domain/people.md, section 5.1). The tasks child keeps each
//! counter. The core compares the revision a party named before changing state,
//! and translates the current hold reason without exposing a child's types.
use crate::Core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;

impl Core {
    /// Shared release admission, in the party route's original order.
    pub(crate) fn release_admit(
        &mut self,
        task: u64,
        revision: u64,
        again: Option<bool>,
    ) -> Result<Option<u64>, people::Refusal> {
        self.hold_revision(task, revision)?;
        let required = match self.tasks.task(task).ok_or(people::Refusal::Unknown)?.phase {
            tasks::Phase::Held { why: tasks::Hold::Uncertain { .. }, .. } => true,
            tasks::Phase::Held { .. }
            | tasks::Phase::Waiting
            | tasks::Phase::Active(_)
            | tasks::Phase::Closing(_)
            | tasks::Phase::Ended(_) => false,
        };
        if required != again.is_some() {
            return Err(people::Refusal::Authority { proposable: None, lacking: people::Lack::Again { required } });
        }
        if again == Some(false) {
            match crate::fresh(&mut self.counters, crate::Family::Message) {
                Some(message) => Ok(Some(message)),
                None => Err(people::Refusal::Limit { which: people::LimitKind::Arithmetic }),
            }
        } else {
            Ok(None)
        }
    }

    /// Acceptance also records the proposal decision after the amendment and optional release.
    pub(crate) fn amendment_accept_revision(
        &self,
        proposer: u64,
        task: u64,
        release: Option<u64>,
    ) -> Result<(), people::Refusal> {
        let source = self.tasks.task(proposer).ok_or(people::Refusal::Unknown)?;
        let target = self.tasks.task(task).ok_or(people::Refusal::Unknown)?;
        let changes: u64 = match target.phase {
            tasks::Phase::Held { .. } if release == Some(target.revisions.hold) => 2,
            tasks::Phase::Held { .. }
            | tasks::Phase::Waiting
            | tasks::Phase::Active(_)
            | tasks::Phase::Closing(_)
            | tasks::Phase::Ended(_) => 1,
        };
        let room = if proposer == task {
            target.revision.checked_add(changes.checked_add(1).expect("at most two task changes")).is_some()
        } else {
            source.revision.checked_add(1).is_some() && target.revision.checked_add(changes).is_some()
        };
        if room { Ok(()) } else { Err(people::Refusal::Limit { which: people::LimitKind::Arithmetic }) }
    }

    /// Refuse an old hold revision with the current reason before release.
    pub(crate) fn hold_revision(&self, task: u64, revision: u64) -> Result<(), people::Refusal> {
        Self::hold_revision_in(task, revision, &tasks::CompoundAdmission::new(&self.tasks, 0))
    }
    pub(crate) fn hold_revision_in(
        task: u64,
        revision: u64,
        admission: &tasks::CompoundAdmission<'_>,
    ) -> Result<(), people::Refusal> {
        let row = admission.task(task).ok_or(people::Refusal::Unknown)?;
        match row.phase {
            tasks::Phase::Held { why, .. } if row.revisions.hold != revision => {
                Err(moved_hold(row.revisions.hold, why))
            }
            tasks::Phase::Held { .. }
            | tasks::Phase::Waiting
            | tasks::Phase::Active(_)
            | tasks::Phase::Closing(_)
            | tasks::Phase::Ended(_) => Ok(()),
        }
    }

    /// Compare only the task spec, independent of progress or history.
    pub(crate) fn spec_revision_in(
        task: u64,
        revision: u64,
        admission: &tasks::CompoundAdmission<'_>,
    ) -> Result<(), people::Refusal> {
        let row = admission.task(task).ok_or(people::Refusal::Unknown)?;
        if row.revisions.spec == revision {
            Ok(())
        } else {
            Err(people::Refusal::Moved { now: people::Moved::Spec { revision: row.revisions.spec } })
        }
    }

    /// Compare the question and displayed results before accepting an answer.
    pub(crate) fn person_revision(&self, task: u64, revision: u64) -> Result<(), people::Refusal> {
        let row = self.tasks.task(task).ok_or(people::Refusal::Unknown)?;
        if row.revisions.person == revision {
            Ok(())
        } else {
            Err(people::Refusal::Moved { now: people::Moved::Person { revision: row.revisions.person } })
        }
    }
}

pub(crate) fn moved_hold(revision: u64, why: tasks::Hold) -> people::Refusal {
    people::Refusal::Moved { now: people::Moved::Hold { revision, reason: hold_reason(why) } }
}

fn hold_reason(why: tasks::Hold) -> people::HoldReason {
    match why {
        tasks::Hold::Exceeded(value) => people::HoldReason::Exceeded(people::Exceeded {
            bound: match value.bound {
                tasks::Bound::Reasoning => people::Bound::Reasoning,
                tasks::Bound::Metadata => people::Bound::Metadata,
                tasks::Bound::Items => people::Bound::Items,
                tasks::Bound::Answer => people::Bound::Answer,
                tasks::Bound::Tools => people::Bound::Tools,
                tasks::Bound::HistoryItems => people::Bound::HistoryItems,
                tasks::Bound::History => people::Bound::History,
                tasks::Bound::Window => people::Bound::Window,
                tasks::Bound::Output => people::Bound::Output,
            },
            value: value.value,
        }),
        tasks::Hold::Failures(class) => people::HoldReason::Failures { class: failure_class(class) },
        tasks::Hold::Stalled => people::HoldReason::Stalled,
        tasks::Hold::Dependency(task) => people::HoldReason::Dependency { task },
        tasks::Hold::Stopped => people::HoldReason::Stopped,
        tasks::Hold::Placed { hold: party } => people::HoldReason::Placed { hold: party },
        tasks::Hold::Drift => people::HoldReason::Drift,
        tasks::Hold::Effects => people::HoldReason::Effects,
        tasks::Hold::EffectFailed => people::HoldReason::EffectFailed,
        tasks::Hold::Uncertain { entry } => people::HoldReason::Uncertain { entry },
        tasks::Hold::Budget => people::HoldReason::Budget,
        tasks::Hold::Deadline => people::HoldReason::Deadline,
        tasks::Hold::HoldsWaited => people::HoldReason::HoldsWaited,
        tasks::Hold::ResourceUnavailable => people::HoldReason::ResourceUnavailable,
        tasks::Hold::ResourceAccess => people::HoldReason::ResourceAccess,
        tasks::Hold::PoolLost { pool } => people::HoldReason::PoolLost { pool },
        tasks::Hold::Procedure => people::HoldReason::Procedure,
        tasks::Hold::Circle => people::HoldReason::Circle,
    }
}

fn failure_class(class: tasks::Class) -> people::FailureClass {
    match class {
        tasks::Class::Transient => people::FailureClass::Transient,
        tasks::Class::Permanent => people::FailureClass::Permanent,
        tasks::Class::Run => people::FailureClass::Run,
        tasks::Class::Agent => people::FailureClass::Agent,
        tasks::Class::Lost => people::FailureClass::Lost,
        tasks::Class::Invalid => people::FailureClass::Invalid,
    }
}
