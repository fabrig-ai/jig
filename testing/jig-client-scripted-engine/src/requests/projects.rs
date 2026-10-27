//! The scripted project requests (contract.md, 3.2, 4 and 7.5).
use crate::{Application, Change, Engine, Invalid};
use jig_client_domain::contract::{
    AdmissionLimit, Answer, Ask, DocumentText, HeldRole, Limit, Malformed, MeProject, NameRule, Outcome, OwnerOutcome,
    Refusal, Text,
};
use skein_lib::List;

type ProjectDecision = Result<(Answer, Vec<Change>), Invalid>;

impl<A: Application> Engine<A> {
    pub(super) fn project_request(&mut self, party: u64, ask: &Ask) -> Option<ProjectDecision> {
        let (project, name, policy) = match ask {
            Ask::Rename { project, name } => (Some(*project), name, None),
            Ask::MakeProject { name, policy } => (None, name, Some(*policy)),
            Ask::ChangeSettings(_)
            | Ask::MoveToNewest(_)
            | Ask::Restore(_)
            | Ask::Create { .. }
            | Ask::Propose { .. }
            | Ask::Withdraw { .. }
            | Ask::Message { .. }
            | Ask::Compound { .. }
            | Ask::Answer { .. }
            | Ask::Take { .. }
            | Ask::HandBack { .. }
            | Ask::Decide { .. }
            | Ask::Release { .. }
            | Ask::Amend { .. }
            | Ask::Stop { .. }
            | Ask::Cancel { .. }
            | Ask::Move { .. }
            | Ask::Seen { .. }
            | Ask::EditNote { .. }
            | Ask::DeleteNote { .. }
            | Ask::ChangePolicy { .. }
            | Ask::SetRoles { .. }
            | Ask::MakeService { .. }
            | Ask::SetPool { .. }
            | Ask::SignIn { .. }
            | Ask::AccountChange { .. }
            | Ask::SignOut { .. } => return None,
        };
        Some(self.change_project(party, project, name, policy))
    }

    fn change_project(
        &mut self,
        party: u64,
        project: Option<u32>,
        name: &Text,
        policy: Option<u32>,
    ) -> ProjectDecision {
        let me = self.truth.frames.get(&party).expect("authenticated party has a frame");
        let held = match project {
            Some(project) => me.projects.iter().find(|p| p.project == project).map(|p| p.role),
            None => me.deployment_role,
        };
        if held != Some(HeldRole::Owner) {
            return Ok(refused(Refusal::Role { held }));
        }
        let name_str = core::str::from_utf8(name.bytes()).expect("Text is UTF-8");
        if let Some(rule) = name_rule(name_str) {
            return Ok(refused(Refusal::Malformed { at: 0, why: Malformed::Name(rule) }));
        }
        if policy.is_some_and(|number| !self.truth.policies.as_deref().unwrap_or(&[1]).contains(&number)) {
            return Ok(refused(Refusal::Unknown));
        }
        // The scripted engine compares ASCII case folding only; it is no
        // authority on Unicode simple case folding.
        let taken = self.truth.frames.values().any(|frame| {
            frame
                .projects
                .iter()
                .any(|p| Some(p.project) != project && p.name.bytes().eq_ignore_ascii_case(name.bytes()))
        }) || self
            .truth
            .projects
            .values()
            .any(|p| Some(p.number) != project && p.name.bytes().eq_ignore_ascii_case(name.bytes()))
            || self
                .truth
                .owner_projects
                .values()
                .any(|p| Some(p.number) != project && p.name.bytes().eq_ignore_ascii_case(name.bytes()));
        if taken {
            return Ok(refused(Refusal::NameTaken));
        }
        let (outcome, changes) = match project {
            Some(project) => (OwnerOutcome::Renamed { project }, self.rename_changes(project, name)),
            None => match self.make_change(party, name) {
                Ok((project, change)) => (OwnerOutcome::ProjectMade { project }, vec![change]),
                Err(refusal) => return Ok(refused(refusal)),
            },
        };
        let mut next = self.truth.clone();
        for change in &changes {
            change.validate(&self.bounds)?;
            next.apply(change)?;
        }
        self.truth = next;
        Ok((Answer::Done(Outcome::Owner(outcome)), changes))
    }

    fn rename_changes(&self, project: u32, name: &Text) -> Vec<Change> {
        let mut changes = Vec::new();
        for frame in self.truth.frames.values() {
            if frame.projects.iter().any(|p| p.project == project) {
                let mut frame = frame.clone();
                for at in 0..frame.projects.len() {
                    let membership = frame.projects.get_mut(at).expect("project index in frame");
                    if membership.project == project {
                        membership.name = DocumentText::complete(name.clone());
                    }
                }
                changes.push(Change::Me(frame));
            }
        }
        if let Some(value) = self.truth.projects.get(&project) {
            let mut value = value.clone();
            value.name = name.clone();
            changes.push(Change::Project(Box::new(value)));
        }
        if let Some(value) = self.truth.owner_projects.get(&project) {
            let mut value = value.clone();
            value.name = DocumentText::complete(name.clone());
            changes.push(Change::OwnerProject(Box::new(value)));
        }
        changes
    }
    fn make_change(&self, party: u64, name: &Text) -> Result<(u32, Change), Refusal> {
        let me = self.truth.frames.get(&party).expect("authenticated party has a frame");
        let highest = self
            .truth
            .projects
            .keys()
            .chain(self.truth.owner_projects.keys())
            .chain(self.truth.object_projects.values())
            .copied()
            .chain(self.truth.frames.values().flat_map(|frame| frame.projects.iter().map(|p| p.project)))
            .max()
            .unwrap_or(0);
        let Some(project) = highest.checked_add(1) else {
            return Err(Refusal::Limit { which: Limit::Admission { which: AdmissionLimit::Projects } });
        };
        if me.projects.len() >= self.bounds.rows {
            return Err(Refusal::Limit { which: Limit::Admission { which: AdmissionLimit::Projects } });
        }
        let mut frame = me.clone();
        let mut projects = List::with_capacity(frame.projects.len().checked_add(1).expect("bounded projects"));
        for membership in frame.projects.into_boxed() {
            projects.push(membership).expect("room for existing projects");
        }
        projects
            .push(MeProject {
                project,
                name: DocumentText::complete(name.clone()),
                role: HeldRole::Owner,
                pool: None,
                templates: List::with_capacity(0),
                creating: None,
                models: List::with_capacity(0),
            })
            .expect("room for made project");
        frame.projects = projects;
        Ok((project, Change::Me(frame)))
    }
}

fn refused(refusal: Refusal) -> (Answer, Vec<Change>) {
    (Answer::Refused { refusal, at: None }, Vec::new())
}

// Keep the name shape identical to the project's rule (core.md, 6.1).
fn name_rule(name: &str) -> Option<NameRule> {
    if name.is_empty() {
        return Some(NameRule::Empty);
    }
    if name.len() > 64 {
        return Some(NameRule::Long);
    }
    if name.chars().any(char::is_control) {
        return Some(NameRule::Control);
    }
    if name.chars().next().is_some_and(char::is_whitespace) || name.chars().next_back().is_some_and(char::is_whitespace)
    {
        return Some(NameRule::Space);
    }
    if name.chars().all(|c| c.is_ascii_digit()) {
        return Some(NameRule::Digits);
    }
    None
}
