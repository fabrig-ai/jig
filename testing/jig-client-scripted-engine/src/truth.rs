//! The story's starting documents and atomic changes; contract.md, 5.
//! Documents carry offers, revisions, terminal facts and parts in the client's vocabulary.
use crate::{Bounds, Invalid, Source, payload};
use jig_client_domain::contract::{Answer, Ask, Deployment, Me, Note, ObjectKey, Project, ReadValue, Tree};
use std::collections::BTreeMap;

/// Client documents indexed by their identities, with parties and sign-ins.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Truth {
    pub settings: Option<jig_client_domain::contract::SettingsDocument>,
    pub setting_values: Vec<jig_client_domain::contract::SettingDocument>,
    pub revisions: BTreeMap<u32, jig_client_domain::contract::RevisionAnswer>,
    pub revision_pages: BTreeMap<Option<u32>, jig_client_domain::contract::RevisionsPage>,
    /// Configured policy numbers for `MakeProject`; `None` supplies the default `[1]`.
    pub policies: Option<Vec<u32>>,
    pub frames: BTreeMap<u64, Me>,
    pub sign_ins: BTreeMap<u64, u64>,
    pub objects: BTreeMap<ObjectKey, ReadValue>,
    /// Project context for object documents whose client carrier names only a task.
    pub object_projects: BTreeMap<ObjectKey, u32>,
    pub projects: BTreeMap<u32, Project>,
    /// Whole authentic projects, independent of ordinary fixtures.
    pub owner_projects: BTreeMap<u32, jig_client_domain::contract::OwnerProject>,
    pub deployment: Option<Deployment>,
    /// Whole deployment diagnostics supplied by their owner.
    pub owner_deployment: Option<jig_client_domain::contract::OwnerDeployment>,
    pub notes: BTreeMap<u64, Note>,
    pub sources: Vec<Source>,
    pub results_seen: BTreeMap<u64, u64>,
    pub replies_seen: BTreeMap<u64, Vec<jig_client_domain::contract::ReadPosition>>,
    pub trees: BTreeMap<u64, Tree>,
}

/// One whole-document replacement made atomically with its story's decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Settings(jig_client_domain::contract::SettingsDocument),
    Setting(jig_client_domain::contract::SettingDocument),
    Revision {
        number: u32,
        value: jig_client_domain::contract::RevisionAnswer,
    },
    Revisions {
        before: Option<u32>,
        value: jig_client_domain::contract::RevisionsPage,
    },
    /// A jig request on an application requester's behalf; only application decisions may use this.
    Request(Box<Ask>),
    /// Replace a party's authoritative frame, including roles and retention.
    Me(Me),
    /// Confirm a numbered sign-in for a party.
    SignIn {
        sign_in: u64,
        party: u64,
    },
    /// End a numbered sign-in.
    SignedOut {
        sign_in: u64,
    },
    /// Replace an object, preserving its identity.
    Object {
        project: u32,
        key: ObjectKey,
        value: ReadValue,
    },
    /// Forget a live object; subsequent reads are unknown.
    Forget {
        key: ObjectKey,
    },
    /// Replace a project document.
    Project(Box<Project>),
    /// Replace a whole owner project after its name changes.
    OwnerProject(Box<jig_client_domain::contract::OwnerProject>),
    /// Replace a revisioned note.
    Note(Note),
    /// Delete a note.
    DeleteNote {
        note: u64,
    },
    /// Replace deployment hosts, accounts and capacities.
    Deployment(Deployment),
    /// Replace whole owner deployment facts and bounded diagnostics.
    OwnerDeployment(jig_client_domain::contract::OwnerDeployment),
    /// Replace the retained rows of one scoped read; rows may span many protocol pages.
    Source(Source),
    /// Replace the authoritative subtree, including folded counts.
    Tree(Tree),
    /// Replace committed inbox read positions for a party.
    Seen {
        party: u64,
        results: u64,
        replies: Vec<jig_client_domain::contract::ReadPosition>,
    },
}
impl Change {
    pub(crate) fn validate(&self, bounds: &Bounds) -> core::result::Result<(), Invalid> {
        let fits = match self {
            Self::Settings(v) => payload::fits(v, bounds),
            Self::Setting(v) => payload::fits(v, bounds),
            Self::Revision { value, .. } => payload::fits(value, bounds),
            Self::Revisions { value, .. } => payload::fits(value, bounds),
            Self::Request(ask) => payload::fits(ask, bounds),
            Self::Source(value) => {
                return value.validate(bounds);
            }
            Self::Seen { replies, .. } => replies.len() <= bounds.rows as usize,
            Self::Tree(value) => payload::fits(value, bounds),
            Self::Me(value) => payload::fits(value, bounds),
            Self::Object { project, key, value } => {
                if identity(value) != Some(*key) || document_project(value).is_some_and(|number| number != *project) {
                    return Err(Invalid::Identity);
                }
                payload::fits(value, bounds)
            }
            Self::Project(value) => payload::fits(value, bounds),
            Self::OwnerProject(value) => payload::fits(value, bounds),
            Self::Note(value) => payload::fits(value, bounds),
            Self::Deployment(value) => payload::fits(value, bounds),
            Self::OwnerDeployment(value) => payload::fits(value, bounds),
            Self::SignIn { .. } | Self::SignedOut { .. } | Self::Forget { .. } | Self::DeleteNote { .. } => true,
        };
        if fits { Ok(()) } else { Err(Invalid::Payload) }
    }
}
impl Truth {
    /// Validates fixture documents, including unused list capacity and nested payloads.
    pub fn validate(&self, bounds: &Bounds) -> core::result::Result<(), Invalid> {
        if let Some(value) = &self.settings {
            Change::Settings(value.clone()).validate(bounds)?;
        }
        for value in &self.setting_values {
            Change::Setting(value.clone()).validate(bounds)?;
        }
        for (number, value) in &self.revisions {
            Change::Revision { number: *number, value: value.clone() }.validate(bounds)?;
        }
        for (before, value) in &self.revision_pages {
            Change::Revisions { before: *before, value: value.clone() }.validate(bounds)?;
        }
        for (party, frame) in &self.frames {
            if *party != frame.party.number {
                return Err(Invalid::Identity);
            }
            Change::Me(frame.clone()).validate(bounds)?;
        }
        for party in self.sign_ins.values() {
            if !self.frames.contains_key(party) {
                return Err(Invalid::Identity);
            }
        }
        for (key, value) in &self.objects {
            let project = self.project(*key).ok_or(Invalid::Identity)?;
            Change::Object { project, key: *key, value: value.clone() }.validate(bounds)?;
        }
        for (number, project) in &self.projects {
            if *number != project.number {
                return Err(Invalid::Identity);
            }
            Change::Project(Box::new(project.clone())).validate(bounds)?;
        }
        for (number, project) in &self.owner_projects {
            if *number != project.number {
                return Err(Invalid::Identity);
            }
            if !crate::owner_project::fits(project, bounds) {
                return Err(Invalid::Payload);
            }
        }
        for (number, note) in &self.notes {
            if *number != note.number {
                return Err(Invalid::Identity);
            }
            Change::Note(note.clone()).validate(bounds)?;
        }
        if let Some(deployment) = &self.deployment {
            Change::Deployment(deployment.clone()).validate(bounds)?;
        }
        if let Some(deployment) = &self.owner_deployment {
            Change::OwnerDeployment(deployment.clone()).validate(bounds)?;
        }
        for source in &self.sources {
            source.validate(bounds)?;
        }
        for (task, tree) in &self.trees {
            if *task != tree.task {
                return Err(Invalid::Identity);
            }
            Change::Tree(tree.clone()).validate(bounds)?;
        }
        Ok(())
    }
    pub(crate) fn apply(&mut self, change: &Change) -> core::result::Result<(), Invalid> {
        match change {
            Change::Settings(value) => self.settings = Some(value.clone()),
            Change::Setting(value) => {
                self.setting_values.retain(|old| old.setting != value.setting || old.revision != value.revision);
                self.setting_values.push(value.clone());
            }
            Change::Revision { number, value } => {
                self.revisions.insert(*number, value.clone());
            }
            Change::Revisions { before, value } => {
                self.revision_pages.insert(*before, value.clone());
            }
            Change::Request(_) => return Err(Invalid::Script),
            Change::Source(source) => {
                self.sources.retain(|old| old.party != source.party || old.read != source.read);
                self.sources.push(source.clone());
                if let (jig_client_domain::contract::Read::Activity { task, .. }, crate::Rows::Activity(rows)) =
                    (&source.read, &source.rows)
                    && let Some(ReadValue::Task(doc)) = self.objects.get_mut(&ObjectKey::Task(*task))
                {
                    doc.activity = rows.last().map_or(0, |line| line.position);
                }
            }
            Change::Seen { party, results, replies } => {
                if !self.frames.contains_key(party) {
                    return Err(Invalid::Identity);
                }
                self.results_seen.insert(*party, *results);
                self.replies_seen.insert(*party, replies.clone());
            }
            Change::Tree(tree) => {
                self.trees.insert(tree.task, tree.clone());
            }
            Change::Me(frame) => {
                self.frames.insert(frame.party.number, frame.clone());
            }
            Change::SignIn { sign_in, party } => {
                if !self.frames.contains_key(party) {
                    return Err(Invalid::Identity);
                }
                self.sign_ins.insert(*sign_in, *party);
            }
            Change::SignedOut { sign_in } => {
                self.sign_ins.remove(sign_in);
            }
            Change::Object { project, key, value } => {
                self.object_projects.insert(*key, *project);
                self.objects.insert(*key, value.clone());
            }
            Change::Forget { key } => {
                self.objects.remove(key);
                self.object_projects.remove(key);
            }
            Change::OwnerProject(project) => {
                self.owner_projects.insert(project.number, (**project).clone());
            }
            Change::Project(project) => {
                self.projects.insert(project.number, (**project).clone());
            }
            Change::Note(note) => {
                self.notes.insert(note.number, note.clone());
            }
            Change::DeleteNote { note } => {
                self.notes.remove(note);
            }
            Change::OwnerDeployment(deployment) => {
                self.deployment = None;
                self.owner_deployment = Some(deployment.clone());
            }
            Change::Deployment(deployment) => {
                self.owner_deployment = None;
                self.deployment = Some(deployment.clone());
            }
        }
        Ok(())
    }
    pub(crate) fn project(&self, key: ObjectKey) -> Option<u32> {
        document_project(self.objects.get(&key)?).or_else(|| self.object_projects.get(&key).copied())
    }
    pub(crate) fn visible(&self, party: u64, project: u32) -> bool {
        self.frames.get(&party).is_some_and(|frame| frame.projects.iter().any(|role| role.project == project))
    }
}

/// The answer and truth changes scripted for one request, before key retention.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub party: Option<u64>,
    pub ask: Ask,
    pub answer: Answer,
    pub changes: Vec<Change>,
}
impl Rule {
    pub(crate) fn validate(&self, bounds: &Bounds) -> core::result::Result<(), Invalid> {
        if matches!(self.answer, Answer::Refused { refusal: _, .. }) && !self.changes.is_empty() {
            return Err(Invalid::Script);
        }
        if !payload::fits(&self.ask, bounds) || !payload::fits(&self.answer, bounds) {
            return Err(Invalid::Payload);
        }
        for change in &self.changes {
            if matches!(change, Change::Request(_)) {
                return Err(Invalid::Script);
            }
            change.validate(bounds)?;
        }
        Ok(())
    }
}
/// The starting truth, payload limits and request outcomes for a scenario.
#[derive(Clone, Debug)]
pub struct Script {
    /// Jig owner range stated by the scripted protocol stream hello.
    pub versions: jig_client_domain::contract::VersionRange,
    pub truth: Truth,
    pub bounds: Bounds,
    pub rules: Vec<Rule>,
}
impl Default for Script {
    fn default() -> Self {
        Self {
            versions: jig_client_domain::contract::VersionRange { first: 19, last: 19 },
            truth: Truth::default(),
            bounds: Bounds::default(),
            rules: Vec::new(),
        }
    }
}
impl Script {
    pub(crate) fn validate(&self) -> core::result::Result<(), Invalid> {
        if self.bounds.text < 8 || self.bounds.heartbeat == skein_lib::Duration::ZERO {
            return Err(Invalid::Script);
        }
        self.truth.validate(&self.bounds)?;
        for rule in &self.rules {
            rule.validate(&self.bounds)?;
        }
        Ok(())
    }
}
pub(crate) fn identity(value: &ReadValue) -> Option<ObjectKey> {
    match value {
        ReadValue::Task(v) => Some(ObjectKey::Task(v.chip.task)),
        ReadValue::Proposal(v) => Some(ObjectKey::Proposal(v.key)),
        ReadValue::Historical(v) => Some(v.key),
        ReadValue::OwnerProposal(v) => Some(ObjectKey::Proposal(v.key)),
        ReadValue::Escalation(v) => Some(ObjectKey::Escalation(v.task.task)),
        ReadValue::OwnerEscalation(v) => Some(ObjectKey::Escalation(v.task.chip.task)),
        ReadValue::Question(v) => Some(ObjectKey::Question { task: v.task.task, message: v.message }),
        ReadValue::PersonTask(v) => Some(ObjectKey::PersonTask(v.task.task)),
        ReadValue::OwnerPersonTask(v) => Some(ObjectKey::PersonTask(v.task.task)),
        ReadValue::Result(v) => Some(ObjectKey::Result(v.task.task)),
        ReadValue::Tasks(_)
        | ReadValue::Me(_)
        | ReadValue::Ended(_)
        | ReadValue::History(_)
        | ReadValue::Activity(_)
        | ReadValue::Turn(_)
        | ReadValue::Inbox(_)
        | ReadValue::OwnerInbox(_)
        | ReadValue::Holds(_)
        | ReadValue::Spend(_)
        | ReadValue::OwnerEnded(_)
        | ReadValue::OwnerSpend(_)
        | ReadValue::OwnerProject(_)
        | ReadValue::Project(_)
        | ReadValue::Notes(_)
        | ReadValue::Settings(_)
        | ReadValue::Setting(_)
        | ReadValue::Revision(_)
        | ReadValue::Revisions(_)
        | ReadValue::Deployment(_)
        | ReadValue::OwnerDeployment(_) => None,
    }
}

fn document_project(value: &ReadValue) -> Option<u32> {
    match value {
        ReadValue::Task(v) => Some(v.project),
        ReadValue::OwnerEscalation(v) => Some(v.task.project),
        ReadValue::Proposal(v) => Some(v.project),
        ReadValue::OwnerProposal(v) => Some(v.project),
        ReadValue::OwnerPersonTask(v) => v.project(),
        ReadValue::Historical(_)
        | ReadValue::Tasks(_)
        | ReadValue::Me(_)
        | ReadValue::Escalation(_)
        | ReadValue::Question(_)
        | ReadValue::PersonTask(_)
        | ReadValue::Result(_)
        | ReadValue::Ended(_)
        | ReadValue::History(_)
        | ReadValue::Activity(_)
        | ReadValue::Turn(_)
        | ReadValue::Inbox(_)
        | ReadValue::OwnerInbox(_)
        | ReadValue::Holds(_)
        | ReadValue::Spend(_)
        | ReadValue::OwnerEnded(_)
        | ReadValue::OwnerSpend(_)
        | ReadValue::OwnerProject(_)
        | ReadValue::Project(_)
        | ReadValue::Notes(_)
        | ReadValue::Settings(_)
        | ReadValue::Setting(_)
        | ReadValue::Revision(_)
        | ReadValue::Revisions(_)
        | ReadValue::Deployment(_)
        | ReadValue::OwnerDeployment(_) => None,
    }
}
