//! Complete retained connector rows, without prepared payloads or runtime recovery flags.
use super::Domain;
use crate::{Record, RecordKey};
impl Domain {
    /// Copy one row from its actual owner for cold-start readback.
    #[must_use]
    pub fn stored_record(&self, key: &RecordKey) -> Option<Record> {
        Some(match key {
            RecordKey::ApplicationObject(object) => {
                Record::ApplicationObject { object: *object, now: *self.application.objects.get(object)? }
            }
            RecordKey::ApplicationAnswer(token) => {
                let (until, outcome) = self.application.answers.get(token)?;
                Record::ApplicationAnswer { token: *token, until: *until, outcome: outcome.clone() }
            }
            RecordKey::Procedure(task) => Record::Procedure(self.procedures.get(task)?.clone()),
            RecordKey::ProcedureDelegateOutcome(task) => {
                let (step, child, revisions) = self.delegate_outcomes.get(task)?;
                Record::ProcedureDelegateOutcome {
                    task: *task,
                    step: *step,
                    child: *child,
                    revisions: revisions.clone(),
                }
            }
            RecordKey::Result(task) => Record::Result { task: *task, code: *self.results.get(task)? },
            RecordKey::Task(task) => return self.tasks.get(task).cloned(),
            RecordKey::Adoption { project, path } => Record::Adoption {
                project: *project,
                path: path.clone(),
                role: *self.adoptions.get(&(*project, path.clone()))?,
            },
            RecordKey::Subscription { topic, task } => {
                let (wake_at, keep_at) = self.subscriptions.get(&(*topic, *task))?;
                Record::Subscription { topic: *topic, task: *task, wake_at: *wake_at, keep_at: *keep_at }
            }
            RecordKey::Pool(path) => Record::Pool { path: path.clone(), slots: *self.pools.get(path)? },
            RecordKey::Proposal(number) => {
                let (project, task, effect) = self.outbox.proposals.get(number)?;
                Record::Proposal { number: *number, project: *project, task: *task, effect: effect.clone() }
            }
            RecordKey::Outbox(key) => {
                let mut found = None;
                for (_, row) in &self.outbox.entries {
                    if row.key == *key {
                        found = Some(row.clone());
                    }
                }
                Record::Outbox(found?)
            }
            RecordKey::Made(key) => {
                let row = self.outbox.made.get(key)?;
                Record::Made { key: *key, resources: row.resources.clone(), state: row.state }
            }
        })
    }
}
