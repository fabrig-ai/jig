//! Optional checks over deliveries and person acts (client/domain/testing.md, 6.2).
use jig_client::{
    Key,
    contract::{
        ActKind, Amendment, AnswerOn, Ask, DecideOn, Escalation, Note, NoteScope, Operation, OwnerEscalation,
        OwnerPersonTask, PersonTask, ProposeAction, ReadValue, Revision, Task,
    },
};
use skein_ui::{NodeId, Role, Tree};
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Object {
    PlacedHold(u64),
    Task(u64),
    Escalation(u64),
    PersonTask(u64),
    Note { scope: NoteScope, note: u64 },
}
#[derive(Clone, Copy, Debug)]
pub enum Delivered<'a> {
    Task(&'a Task),
    Escalation(&'a OwnerEscalation),
    PersonTask(&'a OwnerPersonTask),
    Note(&'a Note),
    PartyEscalation(&'a Escalation),
    PartyPersonTask(&'a PersonTask),
    Read(&'a ReadValue),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    Undelivered { key: Key, object: Object, revision: Revision },
    Unseen { key: Key, act: ActKind },
}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub objects: usize,
    pub revisions: usize,
}
/// The allowed regions' text at the moment of a person's act.
#[derive(Clone, Debug)]
pub struct Scene(Vec<Vec<u8>>);
#[derive(Debug)]
pub struct Check {
    limits: Limits,
    objects: VecDeque<(Object, VecDeque<Revision>)>,
}
impl Check {
    #[must_use]
    pub fn new(limits: &Limits) -> Self {
        assert!(limits.objects > 0 && limits.revisions > 0, "check limits are nonzero");
        Self { limits: *limits, objects: VecDeque::new() }
    }
    pub fn delivered(&mut self, document: Delivered<'_>) {
        match document {
            Delivered::Task(task) => {
                self.record(&Object::Task(task.chip.task), task.spec_revision);
                if let Some(hold) = &task.hold {
                    self.record(&Object::Escalation(task.chip.task), hold.revision());
                    if let Some(placed) = hold.placed() {
                        self.record(&Object::PlacedHold(placed.number), placed.revision);
                    }
                }
                if let Some(revision) = task.person_revision {
                    self.record(&Object::PersonTask(task.chip.task), revision);
                }
            }
            Delivered::Escalation(value) => self.delivered(Delivered::Task(&value.task)),
            Delivered::PersonTask(value) => self.record(&Object::PersonTask(value.task.task), value.revision),
            Delivered::Note(value) => {
                self.record(&Object::Note { scope: value.scope.clone(), note: value.number }, value.revision);
            }
            Delivered::PartyEscalation(value) => {
                self.record(&Object::Task(value.task.task), value.steering.revision);
                self.record(&Object::Escalation(value.task.task), value.hold.revision);
                if let Some(placed) = &value.hold.placed {
                    self.record(&Object::PlacedHold(placed.number), placed.revision);
                }
                if let Some(revision) = value.steering.person_revision {
                    self.record(&Object::PersonTask(value.task.task), revision);
                }
            }
            Delivered::PartyPersonTask(value) => self.record(&Object::PersonTask(value.task.task), value.revision),
            Delivered::Read(value) => match value {
                ReadValue::Task(value) => self.delivered(Delivered::Task(value)),
                ReadValue::OwnerEscalation(value) => self.delivered(Delivered::Escalation(value)),
                ReadValue::OwnerPersonTask(value) => self.delivered(Delivered::PersonTask(value)),
                ReadValue::Escalation(value) => self.delivered(Delivered::PartyEscalation(value)),
                ReadValue::PersonTask(value) => self.delivered(Delivered::PartyPersonTask(value)),
                ReadValue::Notes(value) => {
                    for note in &value.rows {
                        self.delivered(Delivered::Note(note));
                    }
                }
                ReadValue::Me(_)
                | ReadValue::Tasks(_)
                | ReadValue::Proposal(_)
                | ReadValue::Historical(_)
                | ReadValue::OwnerProposal(_)
                | ReadValue::Question(_)
                | ReadValue::Result(_)
                | ReadValue::Ended(_)
                | ReadValue::History(_)
                | ReadValue::Activity(_)
                | ReadValue::Turn(_)
                | ReadValue::Inbox(_)
                | ReadValue::OwnerInbox(_)
                | ReadValue::Spend(_)
                | ReadValue::OwnerEnded(_)
                | ReadValue::OwnerSpend(_)
                | ReadValue::OwnerProject(_)
                | ReadValue::Project(_)
                | ReadValue::Settings(_)
                | ReadValue::Setting(_)
                | ReadValue::Revision(_)
                | ReadValue::Revisions(_)
                | ReadValue::Deployment(_)
                | ReadValue::OwnerDeployment(_) => {}
            },
        }
    }
    fn record(&mut self, object: &Object, revision: Revision) {
        if !self.objects.iter().any(|(key, _)| key == object) {
            if self.objects.len() == self.limits.objects {
                self.objects.pop_front();
            }
            self.objects.push_back((object.clone(), VecDeque::new()));
        }
        let (_, revisions) = self.objects.iter_mut().find(|(key, _)| key == object).expect("object admitted");
        if !revisions.contains(&revision) {
            if revisions.len() == self.limits.revisions {
                revisions.pop_front();
            }
            revisions.push_back(revision);
        }
    }
    #[must_use]
    pub fn sent(&mut self, key: Key, ask: &Ask) -> Option<Fault> {
        for (object, revision) in named(ask) {
            if !self.objects.iter().any(|(value, revisions)| *value == object && revisions.contains(&revision)) {
                return Some(Fault::Undelivered { key, object, revision });
            }
        }
        None
    }
    #[must_use]
    pub fn acted(&mut self, key: Key, act: ActKind, tree: &Tree, node: NodeId, facts: &[&[u8]]) -> Option<Fault> {
        self.seen(key, act, &scene(tree, node), facts)
    }
    /// Check a scene retained while a shell dispatches the act's keyed request.
    #[must_use]
    pub fn seen(&mut self, key: Key, act: ActKind, scene: &Scene, facts: &[&[u8]]) -> Option<Fault> {
        if scene.0.iter().any(|text| facts.iter().all(|fact| contains_fact(text, fact))) {
            None
        } else {
            Some(Fault::Unseen { key, act })
        }
    }
}
/// Capture only an open dialog or the acted node's nearest named region.
#[must_use]
pub fn scene(tree: &Tree, node: NodeId) -> Scene {
    let Some(at) = tree.nodes().iter().position(|value| value.id == node) else {
        return Scene(Vec::new());
    };
    let ancestors: Vec<_> = tree
        .nodes()
        .iter()
        .enumerate()
        .take(at + 1)
        .filter(|(start, value)| start + usize::try_from(value.size).expect("subtree fits") > at)
        .collect();
    let nearest = ancestors.iter().rev().find(|(_, value)| value.role == Role::Region && value.name.is_some());
    let regions = tree
        .nodes()
        .iter()
        .enumerate()
        .filter(|(start, value)| value.role == Role::Dialog || nearest.is_some_and(|(region, _)| region == start));
    Scene(
        regions
            .map(|(start, value)| {
                tree.nodes()[start..start + usize::try_from(value.size).expect("subtree fits")]
                    .iter()
                    .flat_map(|value| {
                        value.text.iter().flat_map(|run| run.text.iter().copied()).chain(std::iter::once(b'\n'))
                    })
                    .collect()
            })
            .collect(),
    )
}
/// Every revision named by an operation, including each compound piece.
#[must_use]
pub fn named(ask: &Ask) -> Vec<(Object, Revision)> {
    match ask {
        Ask::Lift { hold, revision, .. } => vec![(Object::PlacedHold(*hold), *revision)],
        Ask::Release { task, revision, .. } | Ask::Decide { on: DecideOn::Escalation { task, revision, .. }, .. } => {
            vec![(Object::Escalation(*task), *revision)]
        }
        Ask::Amend { task, revision, change, .. } => amendment(*task, *revision, change),
        Ask::Propose { action: ProposeAction::Amendment { task, revision, change }, .. } => {
            amendment(*task, Some(*revision), change)
        }
        Ask::Answer { on: AnswerOn::Person { task, revision, .. }, .. } => vec![(Object::PersonTask(*task), *revision)],
        Ask::EditNote { scope, note, revision, .. } | Ask::DeleteNote { scope, note, revision, .. } => {
            vec![(Object::Note { scope: scope.clone(), note: *note }, *revision)]
        }
        Ask::Compound { operations, .. } => operations
            .iter()
            .flat_map(|operation| match operation {
                Operation::Release { task, revision, .. } => vec![(Object::Escalation(*task), *revision)],
                Operation::Amend { task, revision, change, .. } => amendment(*task, *revision, change),
                Operation::Message { .. }
                | Operation::Cancel { .. }
                | Operation::Stop { .. }
                | Operation::Move { .. } => Vec::new(),
            })
            .collect(),
        Ask::Answer { on: AnswerOn::Question { .. }, .. }
        | Ask::Decide { on: DecideOn::Proposal { .. }, .. }
        | Ask::Propose { action: ProposeAction::Settings(_), .. }
        | Ask::Propose { action: ProposeAction::Batch { .. }, .. }
        | Ask::Withdraw { .. }
        | Ask::Create { .. }
        | Ask::Message { .. }
        | Ask::Take { .. }
        | Ask::HandBack { .. }
        | Ask::Stop { .. }
        | Ask::Cancel { .. }
        | Ask::Move { .. }
        | Ask::Seen { .. }
        | Ask::ChangePolicy { .. }
        | Ask::SetRoles { .. }
        | Ask::MakeService { .. }
        | Ask::SignIn { .. }
        | Ask::AccountChange { .. }
        | Ask::SignOut { .. }
        | Ask::Rename { .. }
        | Ask::ChangeSettings(_)
        | Ask::MoveToNewest(_)
        | Ask::MakeProject { .. }
        | Ask::SetPool { .. } => Vec::new(),
    }
}
fn amendment(task: u64, revision: Option<Revision>, change: &Amendment) -> Vec<(Object, Revision)> {
    let mut values = Vec::new();
    if let Some(revision) = revision {
        values.push((Object::Task(task), revision));
    }
    if let Some(addressed) = &change.addressed {
        values.push((Object::PersonTask(task), addressed.revision));
    }
    values
}

fn contains_fact(text: &[u8], fact: &[u8]) -> bool {
    fact.is_empty()
        || text.windows(fact.len()).enumerate().any(|(at, part)| {
            part == fact
                && (at == 0 || !text[at - 1].is_ascii_alphanumeric() || !fact[0].is_ascii_alphanumeric())
                && (at + fact.len() == text.len()
                    || !text[at + fact.len()].is_ascii_alphanumeric()
                    || !fact[fact.len() - 1].is_ascii_alphanumeric())
        })
}
