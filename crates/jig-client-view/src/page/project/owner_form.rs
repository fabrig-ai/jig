//! Whole supplied settings baselines and one keyed edit; pages.md, 6.5.
//! This form never computes policy. Only explicit offered requests are eligible.
use crate::{Request, Ui, action::Act, intents::Origin, page::IntentPurpose};
use alloc::boxed::Box;
use jig_client::contract::{ActKind, Admission, Ask, OwnerProject, PolicyEdit, Refusal};
use skein_lib::{Queue, Token};
/// One unsent review and the independent pending keyed request.
#[derive(Debug)]
pub struct Form {
    pub review: Option<Box<Ask>>,
    pub service_role: Option<jig_client::contract::HeldRole>,
    pub pool_period: Option<u64>,
    pub before: Option<Box<Ask>>,
    pub pending: Option<Token>,
    pub refused: Option<Refusal>,
}
/// The complete offered source baseline needed for one typed replacement.
#[must_use]
pub fn supported(doc: &OwnerProject, ask: &Ask) -> bool {
    let (project, kind, complete) = match ask {
        Ask::ChangePolicy { project, edit } => (*project, ActKind::ChangePolicy, policy_complete(doc, edit)),
        Ask::SetRoles { project, .. } => (*project, ActKind::SetRoles, !doc.cut),
        Ask::MakeService { project, .. } => (*project, ActKind::MakeService, true),
        // Only the separately supplied configured target establishes this baseline.
        Ask::SetPool { project, .. } => (*project, ActKind::SetPool, !doc.configured_pools_cut),
        Ask::Message { .. }
        | Ask::Propose { .. }
        | Ask::Withdraw { .. }
        | Ask::Create { .. }
        | Ask::Compound { .. }
        | Ask::Answer { .. }
        | Ask::Take { .. }
        | Ask::HandBack { .. }
        | Ask::Decide { .. }
        | Ask::Lift { .. }
        | Ask::Release { .. }
        | Ask::Amend { .. }
        | Ask::Stop { .. }
        | Ask::Cancel { .. }
        | Ask::Move { .. }
        | Ask::Seen { .. }
        | Ask::EditNote { .. }
        | Ask::SignIn { .. }
        | Ask::AccountChange { .. }
        | Ask::SignOut { .. }
        | Ask::Rename { .. }
        | Ask::ChangeSettings(_)
        | Ask::MoveToNewest(_)
        | Ask::MakeProject { .. }
        | Ask::DeleteNote { .. } => return false,
    };
    if project != doc.number || !complete {
        return false;
    }
    for offer in &doc.acts {
        if offer.act == kind {
            return match &offer.admission {
                Admission::Allowed | Admission::Proposed { .. } => true,
                Admission::Refused(_) | Admission::Bounded { .. } => false,
            };
        }
    }
    false
}
/// An explicit role choice has no implicit service name or request.
pub(crate) fn service_role(domain: &mut Ui, role: jig_client::contract::HeldRole) {
    if domain.blocked || domain.project_page.settings.pending.is_some() {
        return;
    }
    let Some(doc) = &domain.project_page.owner else {
        return;
    };
    if !service_offered(doc) {
        return;
    }
    match role {
        jig_client::contract::HeldRole::Policy { role } => {
            let mut supplied = false;
            for row in &doc.policy.roles {
                if row.number == role {
                    supplied = true;
                    break;
                }
            }
            if !supplied {
                return;
            }
        }
        jig_client::contract::HeldRole::Owner
        | jig_client::contract::HeldRole::Maintainer
        | jig_client::contract::HeldRole::Member
        | jig_client::contract::HeldRole::Observer => {}
    }
    domain.project_page.settings.service_role = Some(role);
    domain.project_page.settings.review = None;
    domain.project_page.settings.before = None;
    domain.project_page.settings.refused = None;
    domain.changed();
}
/// Source admission for a role/name service form, before either input is chosen.
#[must_use]
pub fn service_offered(doc: &OwnerProject) -> bool {
    for offer in &doc.acts {
        if offer.act == ActKind::MakeService {
            return match &offer.admission {
                Admission::Allowed | Admission::Proposed { .. } => true,
                Admission::Refused(_) | Admission::Bounded { .. } => false,
            };
        }
    }
    false
}
fn policy_complete(doc: &OwnerProject, edit: &PolicyEdit) -> bool {
    match edit {
        PolicyEdit::Ceiling { .. } => !doc.cut && !doc.policy.cut && !doc.policy.ceiling.cut,
        PolicyEdit::Templates { .. } => false,
        PolicyEdit::ProjectSpend { .. } => true,
        PolicyEdit::RoleSpend { role, .. } => {
            for before in &doc.policy.roles {
                if before.number == *role {
                    return true;
                }
            }
            false
        }
        PolicyEdit::Role(role) => {
            if doc.cut || doc.policy.cut {
                return false;
            }
            for before in &doc.policy.roles {
                if before.number == role.number {
                    return !before.authority.cut;
                }
            }
            false
        }
        PolicyEdit::Requirements { .. } | PolicyEdit::Projections { .. } => !doc.cut && !doc.policy.cut,
        PolicyEdit::Permissions { .. } => {
            !doc.cut && !doc.policy.cut && !doc.policy.permissions_cut && doc.policy.permissions.is_some()
        }
    }
}
/// The unchanged source fields shown before a supported replacement.
#[must_use]
pub fn baseline(doc: &OwnerProject, ask: &Ask) -> Option<Ask> {
    if !supported(doc, ask) {
        return None;
    }
    match ask {
        Ask::ChangePolicy { edit, .. } => {
            let edit = match edit {
                PolicyEdit::Ceiling { .. } => PolicyEdit::Ceiling { ceiling: doc.policy.ceiling.whole()? },
                PolicyEdit::Templates { .. } => return None,
                PolicyEdit::ProjectSpend { .. } => PolicyEdit::ProjectSpend { period_spend: doc.policy.period_spend },
                PolicyEdit::RoleSpend { role, .. } => {
                    let mut spend = None;
                    for row in &doc.policy.roles {
                        if row.number == *role {
                            spend = Some(row.period_spend);
                            break;
                        }
                    }
                    PolicyEdit::RoleSpend { role: *role, period_spend: spend? }
                }
                PolicyEdit::Role(role) => {
                    let mut source = None;
                    for row in &doc.policy.roles {
                        if row.number == role.number {
                            source = Some(row);
                            break;
                        }
                    }
                    let row = source?;
                    PolicyEdit::Role(jig_client::contract::PolicyEditRole {
                        number: row.number,
                        authority: row.authority.whole()?,
                        period_spend: row.period_spend,
                        requests: row.requests,
                        decides: row.decides,
                    })
                }
                PolicyEdit::Requirements { .. } => {
                    PolicyEdit::Requirements { requirements: doc.policy.requirements.clone() }
                }
                PolicyEdit::Projections { .. } => PolicyEdit::Projections { grants: doc.policy.projections.clone() },
                PolicyEdit::Permissions { .. } => {
                    PolicyEdit::Permissions { mappings: doc.policy.permissions.as_ref()?.clone() }
                }
            };
            Some(Ask::ChangePolicy { project: doc.number, edit })
        }
        Ask::SetRoles { .. } => {
            let mut holdings = skein_lib::List::with_capacity(doc.members.capacity());
            for row in &doc.members {
                holdings.push(jig_client::contract::Holding { party: row.party, role: row.role }).ok()?;
            }
            Some(Ask::SetRoles { project: doc.number, holdings })
        }
        Ask::SetPool { party, .. } => {
            for pool in &doc.configured_pools {
                match pool.source {
                    jig_client::contract::Funder::Pool { project, person, period }
                        if project == doc.number && person == *party && period == doc.configured_period =>
                    {
                        return Some(Ask::SetPool { project: doc.number, party: *party, budget: pool.numbers.budget });
                    }
                    jig_client::contract::Funder::Pool { .. }
                    | jig_client::contract::Funder::Task(_)
                    | jig_client::contract::Funder::Period { .. }
                    | jig_client::contract::Funder::Recurring { .. } => {}
                }
            }
            None
        }
        Ask::MakeService { .. }
        | Ask::Message { .. }
        | Ask::Propose { .. }
        | Ask::Withdraw { .. }
        | Ask::Create { .. }
        | Ask::Compound { .. }
        | Ask::Answer { .. }
        | Ask::Take { .. }
        | Ask::HandBack { .. }
        | Ask::Decide { .. }
        | Ask::Lift { .. }
        | Ask::Release { .. }
        | Ask::Amend { .. }
        | Ask::Stop { .. }
        | Ask::Cancel { .. }
        | Ask::Move { .. }
        | Ask::Seen { .. }
        | Ask::EditNote { .. }
        | Ask::SignIn { .. }
        | Ask::AccountChange { .. }
        | Ask::SignOut { .. }
        | Ask::Rename { .. }
        | Ask::ChangeSettings(_)
        | Ask::MoveToNewest(_)
        | Ask::MakeProject { .. }
        | Ask::DeleteNote { .. } => None,
    }
}
/// Keep previously edited channels when the rendered baseline is unchanged.
#[must_use]
pub fn draft(state: &Form, before: &Ask) -> Ask {
    if state.before.as_deref() == Some(before)
        && let Some(review) = &state.review
    {
        return review.as_ref().clone();
    }
    before.clone()
}
#[expect(clippy::manual_map, reason = "step code matches Option without closures")]
pub(crate) fn review(domain: &mut Ui, ask: Box<Ask>) {
    if domain.blocked || domain.project_page.settings.pending.is_some() {
        return;
    }
    let Some(doc) = &domain.project_page.owner else {
        return;
    };
    if supported(doc, &ask) {
        domain.project_page.settings.pool_period = match ask.as_ref() {
            Ask::SetPool { .. } => Some(doc.configured_period),
            Ask::MakeService { .. }
            | Ask::ChangePolicy { .. }
            | Ask::SetRoles { .. }
            | Ask::Message { .. }
            | Ask::Propose { .. }
            | Ask::Withdraw { .. }
            | Ask::Create { .. }
            | Ask::Compound { .. }
            | Ask::Answer { .. }
            | Ask::Take { .. }
            | Ask::HandBack { .. }
            | Ask::Decide { .. }
            | Ask::Lift { .. }
            | Ask::Release { .. }
            | Ask::Amend { .. }
            | Ask::Stop { .. }
            | Ask::Cancel { .. }
            | Ask::Move { .. }
            | Ask::Seen { .. }
            | Ask::EditNote { .. }
            | Ask::SignIn { .. }
            | Ask::AccountChange { .. }
            | Ask::SignOut { .. }
            | Ask::Rename { .. }
            | Ask::ChangeSettings(_)
            | Ask::MoveToNewest(_)
            | Ask::MakeProject { .. }
            | Ask::DeleteNote { .. } => None,
        };
        domain.project_page.settings.before = match baseline(doc, &ask) {
            Some(before) => Some(Box::new(before)),
            None => None,
        };
        domain.project_page.settings.review = Some(ask);
        domain.project_page.settings.refused = None;
    } else {
        domain.project_page.settings.review = None;
        domain.project_page.settings.refused = Some(Refusal::NotOffered);
    }
    domain.changed();
}
#[expect(clippy::manual_map, reason = "step code matches Option without closures")]
pub(crate) fn send(domain: &mut Ui, out: &mut Queue<Request>) {
    if domain.blocked || domain.project_page.settings.pending.is_some() {
        return;
    }
    let Some(scope) = domain.confirmed else {
        return;
    };
    let Some(doc) = &domain.project_page.owner else {
        return;
    };
    let Some(ask) = domain.project_page.settings.review.take() else {
        return;
    };
    let changed_period = match ask.as_ref() {
        Ask::SetPool { .. } => domain.project_page.settings.pool_period != Some(doc.configured_period),
        Ask::MakeService { .. }
        | Ask::ChangePolicy { .. }
        | Ask::SetRoles { .. }
        | Ask::Message { .. }
        | Ask::Propose { .. }
        | Ask::Withdraw { .. }
        | Ask::Create { .. }
        | Ask::Compound { .. }
        | Ask::Answer { .. }
        | Ask::Take { .. }
        | Ask::HandBack { .. }
        | Ask::Decide { .. }
        | Ask::Lift { .. }
        | Ask::Release { .. }
        | Ask::Amend { .. }
        | Ask::Stop { .. }
        | Ask::Cancel { .. }
        | Ask::Move { .. }
        | Ask::Seen { .. }
        | Ask::EditNote { .. }
        | Ask::SignIn { .. }
        | Ask::AccountChange { .. }
        | Ask::SignOut { .. }
        | Ask::Rename { .. }
        | Ask::ChangeSettings(_)
        | Ask::MoveToNewest(_)
        | Ask::MakeProject { .. }
        | Ask::DeleteNote { .. } => false,
    };
    if changed_period
        || !supported(doc, &ask)
        || baseline(doc, &ask).as_ref() != domain.project_page.settings.before.as_deref()
    {
        domain.project_page.settings.refused = Some(Refusal::NotOffered);
        domain.changed();
        return;
    }
    let origin = Origin::Page { handle: domain.page_handle(), purpose: IntentPurpose::ProjectSettings };
    domain.project_page.settings.pending = match domain.intents.ask(scope, origin, ask, out) {
        Some(id) => Some(id.token()),
        None => None,
    };
    domain.changed();
}
pub(crate) fn answered(domain: &mut Ui, request: Token, answer: jig_client::contract::Answer) {
    if domain.project_page.settings.pending == Some(request) {
        domain.project_page.settings.pending = None;
        crate::notices::kept::show(domain, crate::notices::kept::Kind::PageAnswer { answer });
        domain.changed();
    }
}
/// Retire only the matching pending settings request after retention becomes unknown.
pub(crate) fn unknown(domain: &mut Ui, request: Token) {
    if domain.project_page.settings.pending == Some(request) {
        domain.project_page.settings.pending = None;
    }
}
/// Typed settings values are supplied by the view, never parsed by the domain.
#[must_use]
pub fn action(ask: Ask) -> Act {
    Act::ProjectSettings { ask: Box::new(ask) }
}

impl Form {
    pub(crate) const fn new() -> Self {
        Self { review: None, service_role: None, pool_period: None, before: None, pending: None, refused: None }
    }
}
