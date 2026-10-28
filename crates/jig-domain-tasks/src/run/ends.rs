//! One rule for dependencies and ending authority (domain/tasks.md, 5.7, 5.8 and section 7).
use crate::domain::{Domain, record};
use crate::{Active, End, Executor, Phase, TaskResult, Was};

/// Whether an active task may go on, and why not (domain/tasks.md, 5.7).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Kept {
    /// A dependency an amendment added is pending.
    pub(crate) dependencies: bool,
    /// Its own unanswered question to its requester, oldest first.
    pub(crate) question: Option<u64>,
    /// Its requester ends it: no run's result may (5.8).
    pub(crate) open: bool,
    /// Its immutable spec requires a requester question it has not sent.
    pub(crate) owes: bool,
}

impl Domain {
    /// A waiting effect reads the task's current kept state (domain/tasks.md, 5.7).
    #[must_use]
    #[expect(clippy::manual_map, reason = "step code has no closures (programming-model.md, section 10)")]
    pub fn waiting_effect_problem(&self, task: u64) -> Option<crate::Problem> {
        let row = record(self, task)?;
        let kept = for_record(row);
        if kept.dependencies {
            Some(crate::Problem::naming(Some(task), crate::Refusal::Kept, &row.waiting_on))
        } else {
            match kept.question {
                Some(question) => Some(crate::Problem::new(Some(task), crate::Refusal::KeptQuestion { question })),
                None => None,
            }
        }
    }
}

pub(crate) fn kept(domain: &Domain, task: u64) -> Kept {
    match record(domain, task) {
        Some(row) => for_record(row),
        None => pending(&[], crate::EndedBy::Result),
    }
}

/// Publication borrows its row mutably; closing keeps cancellation's partial result.
pub(crate) fn for_record(row: &crate::TaskRecord) -> Kept {
    match row.phase {
        Phase::Active(_) | Phase::Held { was: Was::Active(_), .. } => pending(&row.waiting_on, row.flags.ended_by)
            .with_question(question(&row.questions, row.requester))
            .with_owing(row.spec.asks && !row.asked_requester),
        Phase::Waiting
        | Phase::Closing(_)
        | Phase::Held { was: Was::Waiting | Was::Closing(_), .. }
        | Phase::Ended(_) => pending(&[], crate::EndedBy::Result),
    }
}

/// The same facts for a compound request's projected dependency list.
pub(crate) fn pending(dependencies: &[u64], ended_by: crate::EndedBy) -> Kept {
    Kept {
        dependencies: !dependencies.is_empty(),
        question: None,
        open: ended_by == crate::EndedBy::Requester,
        owes: false,
    }
}

pub(crate) fn question(credits: &[crate::QuestionCredit], requester: crate::Party) -> Option<u64> {
    let mut first = None;
    for credit in credits {
        if credit.asked == requester {
            first = Some(match first {
                Some(old) => core::cmp::min(old, credit.number),
                None => credit.number,
            });
        }
    }
    first
}

/// Each caller asks its own question; a new reason to keep a task answers every one.
impl Kept {
    pub(crate) fn with_question(mut self, question: Option<u64>) -> Self {
        self.question = question;
        self
    }
    pub(crate) fn with_owing(mut self, owes: bool) -> Self {
        self.owes = owes;
        self
    }
    /// The task starts no run: an agent is never prepared, claimed or Due (5.7).
    pub(crate) fn blocks_claims(self) -> bool {
        self.dependencies || self.question.is_some()
    }

    /// A done end waits until every dependency is met: a person's answer is refused, a run's parks.
    pub(crate) fn refuses_done(self) -> bool {
        self.blocks_claims() || self.owes
    }

    /// No run of it may end it, so a finish grace without a live run cancels it at once.
    pub(crate) fn finishes_alone(self) -> bool {
        !self.refuses_done() && !self.open
    }

    /// Whether a wake may make this executor Due; a person or a procedure is never kept from its step.
    pub(crate) fn may_be_due(self, executor: Executor) -> bool {
        match executor {
            Executor::Agent { .. } => !self.blocks_claims(),
            Executor::Person(_) | Executor::Procedure { .. } => true,
        }
    }

    /// An amendment invalidates an agent's queued preparation; a committed claim stays live.
    pub(crate) fn amended(self, executor: Executor, phase: Phase) -> Phase {
        if self.may_be_due(executor) {
            return phase;
        }
        match phase {
            Phase::Active(Active::Due | Active::Preparing) => Phase::Active(Active::Idle),
            Phase::Waiting
            | Phase::Active(
                Active::Idle | Active::Claimed { .. } | Active::Running { .. } | Active::BackingOff { .. },
            )
            | Phase::Held { .. }
            | Phase::Closing(_)
            | Phase::Ended(_) => phase,
        }
    }

    /// Open discards a run's raw finish before judgment; its run parks (tasks.md, 5.8).
    pub(crate) fn discards(self, end: &End) -> bool {
        self.open
            && match end {
                End::Finished { .. } | End::FinishedWithFollowups { .. } => true,
                End::Exceeded(_) | End::Parked | End::Failed(_) | End::Refused | End::Recheck => false,
            }
    }

    /// A judged done end waits until every dependency is met; a failure still ends (5.7).
    pub(crate) fn parks(self, end: &End) -> bool {
        self.refuses_done()
            && match end {
                End::Finished { result, .. } | End::FinishedWithFollowups { result, .. } => done(result),
                End::Exceeded(_) | End::Parked | End::Failed(_) | End::Refused | End::Recheck => false,
            }
    }
}

fn done(result: &TaskResult) -> bool {
    match result {
        TaskResult::Failure { .. } => false,
        TaskResult::Approved { .. }
        | TaskResult::Report { .. }
        | TaskResult::Verdict { .. }
        | TaskResult::Change { .. } => true,
    }
}
