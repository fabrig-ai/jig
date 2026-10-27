//! Client-referee facts projected from externally observed whole applied store images.
//! No submitted request, client callback or live Domain state supplies durable truth
//! (client/domain/testing.md, sections 5–6).
use crate::harness::{CardKey, Observed};
use jig_client_domain::contract::{ObjectKey, ProposalKey, Proposer};
use jig_core_world::store_peer::DurableSnapshot;
use jig_domain_core as core;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use std::collections::{BTreeMap, BTreeSet};

/// Previous complete applied image, used to distinguish changes from redelivery.
#[derive(Default)]
pub struct Truth {
    rows: BTreeMap<root::Key, root::Record>,
    waiting: BTreeSet<CardKey>,
    words: BTreeMap<u64, BTreeMap<u64, Vec<u8>>>,
    ended: BTreeSet<u64>,
}
impl Truth {
    /// Seed the already-applied bootstrap image without reporting a new commit.
    #[must_use]
    pub fn after(rows: BTreeMap<root::Key, root::Record>) -> Self {
        let (waiting, words, ended) = facts(&rows);
        Self { rows, waiting, words, ended }
    }

    /// Consume one actual applied image; its durable fact precedes every consequence.
    pub fn applied(&mut self, snapshot: DurableSnapshot) -> Vec<Observed> {
        let commit = snapshot.commit;
        let mut observed = vec![Observed::Durable { commit, at: snapshot.at }];
        for (key, record) in &snapshot.rows {
            if self.rows.get(key) != Some(record)
                && let root::Record::Core(core::Record::People(jig_domain_people::Stored::Answer { key, .. })) = record
            {
                observed.push(Observed::Made { party: key.person, key: key.key, commit });
            }
        }
        let (waiting, words, ended) = facts(&snapshot.rows);
        for task in ended.difference(&self.ended) {
            observed.push(Observed::Ended { task: *task, commit });
        }
        for card in self.waiting.difference(&waiting) {
            observed.push(Observed::Stopped { card: card.clone(), commit });
        }
        let tasks: BTreeSet<_> = self.words.keys().chain(words.keys()).copied().collect();
        for task in tasks {
            if let Some(messages) = words.get(&task) {
                for (message, bytes) in messages {
                    if self.words.get(&task).and_then(|old| old.get(message)) != Some(bytes) {
                        observed.push(Observed::Kept { task, message: *message, words: bytes.clone(), commit });
                    }
                }
            }
            observed.push(Observed::Retained {
                task,
                messages: words.get(&task).map_or_else(Vec::new, |messages| messages.keys().copied().collect()),
                commit,
            });
        }
        self.rows = snapshot.rows;
        self.waiting = waiting;
        self.words = words;
        self.ended = ended;
        observed
    }
}

type WordImages = BTreeMap<u64, BTreeMap<u64, Vec<u8>>>;
fn facts(rows: &BTreeMap<root::Key, root::Record>) -> (BTreeSet<CardKey>, WordImages, BTreeSet<u64>) {
    let mut waiting = BTreeSet::new();
    let mut words = BTreeMap::new();
    let mut ended = BTreeSet::new();
    for record in rows.values() {
        if let root::Record::Core(core::Record::Tasks(record)) = record {
            match record {
                tasks::Stored::Live(task) => {
                    if matches!(task.phase, tasks::Phase::Ended(_)) {
                        ended.insert(task.number);
                    } else {
                        waiting.insert(crate::observe::key(ObjectKey::Task(task.number)));
                        if let Some(proposal) = &task.proposal
                            && matches!(proposal.state, tasks::ProposalState::Pending { .. })
                        {
                            waiting.insert(crate::observe::key(ObjectKey::Proposal(ProposalKey {
                                proposer: Proposer::Task(proposal.proposer),
                                number: proposal.number,
                            })));
                        }
                        if matches!(task.escalation, tasks::Escalation::Waiting { .. }) {
                            waiting.insert(crate::observe::key(ObjectKey::Escalation(task.number)));
                        }
                        for question in &task.questions {
                            waiting.insert(crate::observe::key(ObjectKey::Question {
                                task: task.number,
                                message: question.number,
                            }));
                        }
                        if matches!(task.executor, tasks::Executor::Person(_)) && task.person_answer.is_none() {
                            waiting.insert(crate::observe::key(ObjectKey::PersonTask(task.number)));
                        }
                    }
                }
                tasks::Stored::Ended(task) => {
                    ended.insert(task.number);
                }
                tasks::Stored::PersonProposal(proposal) | tasks::Stored::DecidedProposal(proposal) => {
                    if matches!(proposal.state, tasks::PersonProposalState::Pending { .. }) {
                        waiting.insert(crate::observe::key(ObjectKey::Proposal(ProposalKey {
                            proposer: Proposer::Party(proposal.proposer),
                            number: proposal.number,
                        })));
                    }
                }
                tasks::Stored::Activity(line) => {
                    if let Some((message, bytes)) = retained_words(&line.line) {
                        let prior =
                            words.entry(line.task).or_insert_with(BTreeMap::new).insert(message, bytes.to_vec());
                        assert!(
                            prior.as_deref().is_none_or(|prior| prior == bytes),
                            "one immutable message has whole words"
                        );
                    }
                }
                tasks::Stored::Milestone(_)
                | tasks::Stored::History(_)
                | tasks::Stored::PlacedHold(_)
                | tasks::Stored::Stub(_)
                | tasks::Stored::Ledger(_)
                | tasks::Stored::Writer(_)
                | tasks::Stored::Result(_)
                | tasks::Stored::EndedByParty(_)
                | tasks::Stored::EndedByProject(_)
                | tasks::Stored::EndedDelegate(_)
                | tasks::Stored::Delegate(_)
                | tasks::Stored::Update(_)
                | tasks::Stored::Pool(_)
                | tasks::Stored::ClosedLedger(_) => {}
            }
        }
    }
    // The connector owns its raw proposal identity and its authentic project.
    // Resource-less agent proposals do not require a synthetic Task record.
    for record in rows.values() {
        if let root::Record::Connector {
            record: jig_test_connector::Record::Proposal { number: proposal, project, .. },
            ..
        } = record
        {
            let subject = jig_client_test_domain::family::Subject {
                scope: jig_client_test_domain::family::Scope::Project(*project),
                kind: jig_client_test_domain::family::Kind::OwnerEffect(*proposal),
            };
            waiting.insert(CardKey::of(jig_client_test_view::family_key(&subject)));
        }
    }
    (waiting, words, ended)
}
fn retained_words(line: &tasks::Line) -> Option<(u64, &[u8])> {
    match line {
        tasks::Line::Words { message, kind: tasks::MessageKind::Words, words, .. } => Some((*message, words)),
        tasks::Line::Heard(word) if matches!(word.kind, tasks::MessageKind::Words) => Some((word.number, &word.words)),
        tasks::Line::Words { .. }
        | tasks::Line::Heard(_)
        | tasks::Line::Notified { .. }
        | tasks::Line::RunBegan { .. }
        | tasks::Line::RunEnded { .. }
        | tasks::Line::Turn { .. }
        | tasks::Line::Decided { .. }
        | tasks::Line::Kept { .. }
        | tasks::Line::Milestone { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skein_lib::{Time, Wall};

    fn image(commit: u64, rows: Vec<root::Record>) -> DurableSnapshot {
        DurableSnapshot {
            commit,
            at: Time::from_nanos(17),
            rows: rows.into_iter().map(|row| (row.key(), row)).collect(),
        }
    }
    fn answer(party: u64) -> root::Record {
        root::Record::Core(core::Record::People(jig_domain_people::Stored::Answer {
            models: Box::new([]),
            key: jig_domain_people::RequestKey { person: party, key: [9; 16] },
            ask: Box::new(jig_domain_people::Ask::Say {
                project: 1,
                task: 7,
                words: b"whole".to_vec().into_boxed_slice(),
            }),
            outcome: jig_domain_people::Outcome::Said { task: 7, message: 4 },
            at: Wall::from_nanos(0),
        }))
    }
    fn words(message: u64, bytes: &[u8]) -> root::Record {
        root::Record::Core(core::Record::Tasks(tasks::Stored::Activity(tasks::ActivityLine {
            task: 7,
            position: message,
            at: Wall::from_nanos(0),
            line: tasks::Line::Words {
                message,
                by: tasks::Party::Person(1),
                kind: tasks::MessageKind::Words,
                words: bytes.to_vec().into_boxed_slice(),
            },
        })))
    }
    #[test]
    fn applied_whole_images_report_durability_before_party_scoped_answers_and_never_remake_a_redelivery() {
        let mut truth = Truth::default();
        let first = truth.applied(image(1, vec![answer(1), answer(2)]));
        assert_eq!(
            first,
            vec![
                Observed::Durable { commit: 1, at: Time::from_nanos(17) },
                Observed::Made { party: 1, key: [9; 16], commit: 1 },
                Observed::Made { party: 2, key: [9; 16], commit: 1 }
            ]
        );
        assert_eq!(
            truth.applied(image(2, vec![answer(1), answer(2)])),
            vec![Observed::Durable { commit: 2, at: Time::from_nanos(17) }]
        );
        truth.applied(image(3, vec![]));
        assert!(truth.applied(image(4, vec![answer(1)])).contains(&Observed::Made {
            party: 1,
            key: [9; 16],
            commit: 4
        }));
    }
    #[test]
    fn complete_applied_activity_images_keep_whole_words_and_report_actual_retention_loss() {
        let mut truth = Truth::after(image(0, vec![words(3, b"bootstrap")]).rows);
        let kept = truth.applied(image(1, vec![words(3, b"bootstrap"), words(4, b"literal/\xff")]));
        assert_eq!(
            kept,
            vec![
                Observed::Durable { commit: 1, at: Time::from_nanos(17) },
                Observed::Kept { task: 7, message: 4, words: b"literal/\xff".to_vec(), commit: 1 },
                Observed::Retained { task: 7, messages: vec![3, 4], commit: 1 }
            ]
        );
        assert_eq!(
            truth.applied(image(2, vec![])),
            vec![
                Observed::Durable { commit: 2, at: Time::from_nanos(17) },
                Observed::Retained { task: 7, messages: vec![], commit: 2 }
            ]
        );
    }
    #[test]
    fn connector_departure_truth_joins_only_its_supplied_project_and_raw_owner_reference() {
        let subject = jig_client_test_domain::family::Subject {
            scope: jig_client_test_domain::family::Scope::Project(u32::MAX),
            kind: jig_client_test_domain::family::Kind::OwnerEffect(u64::MAX),
        };
        let expected = CardKey::of(jig_client_test_view::family_key(&subject));
        let proposal = root::Record::Connector {
            number: jig_client_test_domain::FIRST,
            record: jig_test_connector::Record::Proposal {
                number: u64::MAX,
                task: 7,
                project: u32::MAX,
                effect: jig_test_connector::Effect {
                    kind: u16::MAX,
                    resources: Box::new([]),
                    purpose: u64::MAX,
                    condition: None,
                    target: 0,
                    state: 0,
                },
            },
        };
        let task = root::Record::Connector {
            number: jig_client_test_domain::FIRST,
            record: jig_test_connector::Record::Task { task: 7, project: u32::MAX, resources: Box::new([]) },
        };
        let mut truth = Truth::after(image(0, vec![proposal.clone(), task.clone()]).rows);
        assert_eq!(truth.waiting, BTreeSet::from([expected.clone()]));
        assert_ne!(
            expected,
            crate::observe::key(ObjectKey::Proposal(ProposalKey { proposer: Proposer::Task(7), number: u64::MAX }))
        );
        assert_ne!(
            expected,
            CardKey::of(jig_client_test_view::family_key(&jig_client_test_domain::family::Subject {
                scope: jig_client_test_domain::family::Scope::Deployment,
                kind: jig_client_test_domain::family::Kind::OwnerEffect(u64::MAX),
            }))
        );
        assert_eq!(
            truth.applied(image(1, vec![task])),
            vec![
                Observed::Durable { commit: 1, at: Time::from_nanos(17) },
                Observed::Stopped { card: expected.clone(), commit: 1 },
            ]
        );
        let no_task = Truth::after(image(0, vec![proposal]).rows);
        assert_eq!(no_task.waiting, BTreeSet::from([expected]), "the proposal itself supplies the actual project");
    }
}
