//! The subjects named by the current page's sections (client/domain/client.md, 5.1).
use crate::{
    Ui,
    address::{Address, ProjectSection, TaskSection},
    objects::Body,
    page::Section,
};
impl Ui {
    /// Whether this page includes the section.
    #[must_use]
    pub fn shows(&self, section: Section, lent: &jig_client::Client) -> bool {
        if !self.sections().contains(&section) {
            return false;
        }
        match section {
            Section::Task { task, section } => {
                if let Some(id) = self.objects.find(jig_client::contract::ObjectKey::Task(task)) {
                    match &self.borrow(lent).objects().get(id).expect("named task exists").body {
                        Some(Body::Task(task)) => return super::task::offered(&super::task::offers(task), section),
                        Some(
                            Body::Historical(_)
                            | Body::Chip(_)
                            | Body::Proposal(_)
                            | Body::OwnerProposal(_)
                            | Body::Escalation(_)
                            | Body::OwnerEscalation(_)
                            | Body::Question(_)
                            | Body::PersonTask(_)
                            | Body::OwnerPersonTask(_)
                            | Body::Result(_)
                            | Body::Note { .. },
                        )
                        | None => {}
                    }
                }
            }
            Section::Inbox { .. } | Section::Conversations { .. } | Section::Project { .. } | Section::Deployment => {}
        }
        true
    }
    /// The task named by this page, if any.
    #[must_use]
    pub fn shown_task(&self) -> Option<u64> {
        for section in self.sections() {
            match section {
                Section::Task { task, .. } => return Some(*task),
                Section::Inbox { .. }
                | Section::Conversations { .. }
                | Section::Project { .. }
                | Section::Deployment => {}
            }
        }
        None
    }
    /// The project named by this page, if any.
    #[must_use]
    pub fn shown_project(&self) -> Option<u32> {
        for section in self.sections() {
            match section {
                Section::Project { project, .. } => return Some(*project),
                Section::Inbox { .. } | Section::Conversations { .. } | Section::Task { .. } | Section::Deployment => {}
            }
        }
        None
    }
    /// The inbox filter named by this page, including a project's overview.
    #[must_use]
    pub fn shown_inbox(&self) -> Option<Option<u32>> {
        for section in self.sections() {
            match section {
                Section::Inbox { project, .. } => return Some(*project),
                Section::Project { project, section: ProjectSection::Overview } => return Some(Some(*project)),
                Section::Conversations { .. }
                | Section::Task { .. }
                | Section::Project { .. }
                | Section::Deployment => {}
            }
        }
        None
    }
    #[expect(clippy::option_option, reason = "a section may name every project or one project")]
    pub(crate) fn shown_conversations(&self) -> Option<Option<u32>> {
        for section in self.sections() {
            match section {
                Section::Conversations { project } => return Some(*project),
                Section::Inbox { .. } | Section::Task { .. } | Section::Project { .. } | Section::Deployment => {}
            }
        }
        None
    }
}

/// Builds each address's union in the sections' display order, with Settings whole.
pub(crate) fn arrange(domain: &mut Ui) {
    match domain.address {
        Address::Task { task, section: own } => {
            domain.sections.clear();
            for section in [
                TaskSection::Overview,
                TaskSection::Activity,
                TaskSection::Plan,
                TaskSection::History,
                TaskSection::Budget,
                TaskSection::Panels,
            ] {
                if section == own || domain.arrangement.task.as_slice().contains(&section) {
                    domain.sections.push(Section::Task { task, section }).expect("task sections fit");
                }
            }
        }
        Address::Project { project, section: own } => {
            domain.sections.clear();
            let settings = own == ProjectSection::Settings
                || domain.arrangement.project.as_slice().contains(&ProjectSection::Settings);
            for section in [
                ProjectSection::Overview,
                ProjectSection::Goals,
                ProjectSection::Tasks,
                ProjectSection::Notes,
                ProjectSection::Settings,
                ProjectSection::Resources,
                ProjectSection::Policy,
                ProjectSection::Roles,
                ProjectSection::Spend,
                ProjectSection::Recurring,
            ] {
                let covered = match section {
                    ProjectSection::Resources
                    | ProjectSection::Policy
                    | ProjectSection::Roles
                    | ProjectSection::Spend
                    | ProjectSection::Recurring => settings,
                    ProjectSection::Overview
                    | ProjectSection::Goals
                    | ProjectSection::Tasks
                    | ProjectSection::Notes
                    | ProjectSection::Settings => false,
                };
                if !covered && (section == own || domain.arrangement.project.as_slice().contains(&section)) {
                    domain.sections.push(Section::Project { project, section }).expect("project sections fit");
                }
            }
        }
        Address::Inbox { .. } | Address::Conversations { .. } | Address::Deployment => {}
    }
}

pub(crate) fn owner(domain: &Ui) -> crate::handles::Owner {
    match domain.page {
        super::Page::Elsewhere => crate::handles::Owner::Sections,
        super::Page::Starting
        | super::Page::Work { .. }
        | super::Page::SigningIn
        | super::Page::Missing
        | super::Page::OutOfDate
        | super::Page::SomeoneElse => crate::handles::Owner::Page,
    }
}

pub(crate) fn show(
    domain: &mut Ui,
    env: &skein_lib::Env<crate::Limits>,
    sections: skein_lib::List<Section>,
    out: &mut skein_lib::Queue<crate::Request>,
    lent: &jig_client::Client,
) {
    if domain.page != super::Page::Elsewhere || domain.confirmed.is_none() || domain.link != jig_client::Link::Live {
        return;
    }
    let overlong = sections.len() > super::MOST_SECTIONS;
    let sections = sections
        .as_slice()
        .get(..usize::try_from(sections.len().min(super::MOST_SECTIONS)).expect("bounded section count"))
        .expect("bounded prefix");
    let previous = domain.sections.clone();
    domain.sections.clear();
    for section in sections {
        let fits = match *section {
            Section::Task { task, .. } => domain.shown_task().is_none() || domain.shown_task() == Some(task),
            Section::Project { project, section } => {
                let covered = match section {
                    ProjectSection::Resources
                    | ProjectSection::Policy
                    | ProjectSection::Roles
                    | ProjectSection::Spend
                    | ProjectSection::Recurring => {
                        sections.contains(&Section::Project { project, section: ProjectSection::Settings })
                    }
                    ProjectSection::Overview
                    | ProjectSection::Goals
                    | ProjectSection::Tasks
                    | ProjectSection::Notes
                    | ProjectSection::Settings => false,
                };
                !covered
                    && (domain.shown_project().is_none() || domain.shown_project() == Some(project))
                    && (section != ProjectSection::Overview
                        || (domain.shown_inbox().is_none() || domain.shown_inbox() == Some(Some(project))))
            }
            Section::Inbox { project, .. } => domain.shown_inbox().is_none() || domain.shown_inbox() == Some(project),
            Section::Conversations { project } => {
                domain.shown_conversations().is_none() || domain.shown_conversations() == Some(project)
            }
            Section::Deployment => true,
        };
        if fits && !domain.sections.as_slice().contains(section) {
            domain.sections.push(*section).expect("section count checked");
        }
    }

    let previous_statuses =
        core::mem::replace(&mut domain.section_statuses, skein_lib::List::with_capacity(super::MOST_SECTIONS));
    for section in &domain.sections {
        let mut status = SectionStatus { behind: false, excess: 0, refused: None, missing: false };
        for (index, old) in previous.as_slice().iter().enumerate() {
            if old == section {
                status = *previous_statuses.as_slice().get(index).expect("section status exists");
            }
        }
        domain.section_statuses.push(status).expect("section statuses fit");
    }
    if overlong {
        domain.section_context = u32::MAX;
        super::routing::refuse(domain, jig_client::contract::Limit::Objects);
    }
    let owner = owner(domain);
    let mut leaving = skein_lib::List::with_capacity(domain.limits.watches);
    for (handle, watch) in &domain.handles.watches {
        if watch.owner == owner && !needed(domain, &watch.subject) {
            leaving.push(*handle).expect("held handles fit");
        }
    }
    for handle in leaving.into_boxed() {
        domain.release(handle, out);
    }
    for index in 0..domain.sections.len() {
        let section = *domain.sections.get(index).expect("section index exists");
        if !previous.as_slice().contains(&section) {
            context_section(domain, section);
            super::routing::open_section(domain, env, section, owner, out, lent);
            domain.section_context = u32::MAX;
        }
    }
    if previous != domain.sections {
        if let Some(task) = domain.shown_task() {
            context_subject(domain, &jig_client::contract::Subject::Task { task });
        }
        super::task::document(domain, env, out, lent);
        domain.section_context = u32::MAX;
        domain.changed();
    }
}

pub(crate) fn needed(domain: &Ui, subject: &jig_client::contract::Subject) -> bool {
    for section in domain.sections() {
        if uses(*section, subject) {
            return true;
        }
    }
    false
}
pub(crate) fn uses(section: Section, subject: &jig_client::contract::Subject) -> bool {
    use jig_client::contract::Subject;
    match subject {
        Subject::Me | Subject::Accounts | Subject::Settings => false,
        Subject::Task { task } => match section {
            Section::Task { task: named, .. } => named == *task,
            Section::Inbox { .. } | Section::Conversations { .. } | Section::Project { .. } | Section::Deployment => {
                false
            }
        },
        Subject::Tree { task, .. } => section == Section::Task { task: *task, section: TaskSection::Plan },
        Subject::Run { task, .. } => section == Section::Task { task: *task, section: TaskSection::Activity },
        Subject::Chips { .. } => match section {
            Section::Task { section: TaskSection::Activity, .. } => true,
            Section::Task { .. }
            | Section::Inbox { .. }
            | Section::Conversations { .. }
            | Section::Project { .. }
            | Section::Deployment => false,
        },
        Subject::Inbox { .. } => match section {
            Section::Inbox { .. } | Section::Project { section: ProjectSection::Overview, .. } => true,
            Section::Conversations { .. } | Section::Task { .. } | Section::Project { .. } | Section::Deployment => {
                false
            }
        },
        Subject::Tasks { query, .. } => match section {
            Section::Conversations { project } => {
                query.project == project && query.requester.is_some() && query.label.is_some()
            }
            Section::Project { project, section: ProjectSection::Tasks } => query.project == Some(project),
            Section::Project { project, section: ProjectSection::Overview | ProjectSection::Goals } => {
                *query == crate::queries::goals(project)
            }
            Section::Inbox { .. } | Section::Task { .. } | Section::Project { .. } | Section::Deployment => false,
        },
        Subject::Deployment => section == Section::Deployment,
    }
}

/// What a section says about its sources on an application's page (client.md, 5.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SectionStatus {
    pub behind: bool,
    pub excess: u32,
    pub refused: Option<jig_client::contract::Limit>,
    pub missing: bool,
}
impl Ui {
    #[must_use]
    pub fn section_status(&self, section: Section, lent: &jig_client::Client) -> Option<SectionStatus> {
        if self.page != super::Page::Elsewhere || !self.shows(section, lent) {
            return None;
        }
        let mut found = None;
        for (index, value) in self.sections().iter().enumerate() {
            if *value == section {
                found = Some(index);
            }
        }
        let index = found?;
        let status = *self.section_statuses.as_slice().get(index)?;
        // The current client states are read when the section is built.
        Some(status)
    }
}
pub(crate) fn context_subject(domain: &mut Ui, subject: &jig_client::contract::Subject) {
    domain.section_context = 0;
    for (index, section) in domain.sections.as_slice().iter().enumerate() {
        if uses(*section, subject) {
            domain.section_context |=
                1_u32.checked_shl(u32::try_from(index).expect("bounded index")).expect("nineteen sections fit");
        }
    }
}
fn context_section(domain: &mut Ui, section: Section) {
    domain.section_context = 0;
    for (index, value) in domain.sections.as_slice().iter().enumerate() {
        if *value == section {
            domain.section_context |=
                1_u32.checked_shl(u32::try_from(index).expect("bounded index")).expect("nineteen sections fit");
        }
    }
}
pub(crate) fn status(
    domain: &mut Ui,
    refused: Option<jig_client::contract::Limit>,
    excess: Option<u32>,
    missing: bool,
) {
    for index in 0..domain.section_statuses.len() {
        let value = domain.section_statuses.get_mut(index).expect("section status exists");
        if domain.section_context & 1_u32.checked_shl(index).expect("nineteen sections fit") != 0 {
            if let Some(which) = refused {
                value.refused = Some(which);
            }
            if let Some(count) = excess {
                value.excess = count;
            }
            value.missing |= missing;
        }
    }
}

pub(crate) fn context_read(domain: &mut Ui, what: &jig_client::contract::Read) {
    use jig_client::contract::{Read, Subject};
    let section = match what {
        Read::Task { task } => {
            context_subject(domain, &Subject::Task { task: *task });
            return;
        }
        Read::History { task, .. } => Some(Section::Task { task: *task, section: TaskSection::History }),
        Read::Activity { task, .. } | Read::PersonTask { task } => {
            Some(Section::Task { task: *task, section: TaskSection::Activity })
        }
        Read::Ended { task, .. } => Some(Section::Task { task: *task, section: TaskSection::Plan }),
        Read::Inbox { group, project, .. } => Some(Section::Inbox { group: *group, project: *project }),
        Read::Spend { project, .. } => Some(Section::Project { project: *project, section: ProjectSection::Spend }),
        Read::Notes { .. } => {
            let Some(project) = domain.shown_project() else {
                domain.section_context = 0;
                return;
            };
            Some(Section::Project { project, section: ProjectSection::Notes })
        }
        Read::Project { .. } => {
            domain.section_context = 0;
            for index in 0..domain.sections.len() {
                match *domain.sections.get(index).expect("section exists") {
                    Section::Project { .. } => {
                        domain.section_context |= 1_u32.checked_shl(index).expect("nineteen sections fit");
                    }
                    Section::Inbox { .. }
                    | Section::Conversations { .. }
                    | Section::Task { .. }
                    | Section::Deployment => {}
                }
            }
            return;
        }
        Read::Deployment => Some(Section::Deployment),
        Read::Tasks { query, .. } => match query.project {
            Some(project) if query.requester.is_none() => {
                Some(Section::Project { project, section: ProjectSection::Goals })
            }
            _ => Some(Section::Conversations { project: query.project }),
        },
        Read::Holds { .. }
        | Read::Settings
        | Read::Setting(_)
        | Read::Revision(_)
        | Read::Revisions(_)
        | Read::Turn { .. }
        | Read::Me
        | Read::Proposal { .. }
        | Read::Escalation { .. }
        | Read::Question { .. }
        | Read::Result { .. } => None,
    };
    domain.section_context = 0;
    if let Some(section) = section {
        context_section(domain, section);
    }
}
