//! Deployment facts and connector part references, only for its admitted page.
//! Hosts, accounts and capacity are whole owner documents. No connector value is
//! kept. See client/domain/pages.md, 7, and client/domain/root.md, 6.
use crate::{Request, Ui};
use alloc::boxed::Box;
use jig_client::contract::{Deployment, OwnerDeployment, Part};
use skein_lib::{Env, List, Queue};
/// Latest deployment document for the admitted page.
#[derive(Debug)]
pub struct DeploymentPage {
    pub document: Option<Box<Deployment>>,
    /// The latest deployment owner facts, independent of the synthetic parts path.
    pub owner: Option<Box<OwnerDeployment>>,
}
impl DeploymentPage {
    pub(crate) const fn new() -> Self {
        Self { document: None, owner: None }
    }
}
pub(crate) fn document(
    domain: &mut Ui,
    env: &Env<crate::Limits>,
    doc: Box<Deployment>,
    out: &mut Queue<Request>,
    lent: &jig_client::Client,
) {
    if !domain.shows(crate::page::Section::Deployment, lent) {
        return;
    }
    let mut parts = List::<Part>::with_capacity(env.limits.parts.saturating_add(1));
    for part in &doc.parts {
        if parts.room() == 0 {
            break;
        }
        parts.push(part.clone()).expect("part fits");
    }
    domain.deployment_page.owner = None;
    domain.deployment_page.document = Some(doc);
    crate::page::project::show(domain, env, parts, out);
    domain.changed();
}

/// Keep the whole owner value only for the admitted current deployment page.
pub(crate) fn owner_document(
    domain: &mut Ui,
    env: &Env<crate::Limits>,
    doc: Box<OwnerDeployment>,
    out: &mut Queue<Request>,
    lent: &jig_client::Client,
) {
    if !domain.shows(crate::page::Section::Deployment, lent) {
        return;
    }
    let mut parts = List::with_capacity(env.limits.parts.saturating_add(1));
    for connector in &doc.connectors {
        if parts.room() == 0 {
            break;
        }
        parts.push(Part::Deployment { connector: *connector }).expect("configured reference fits");
    }
    domain.deployment_page.document = None;
    domain.deployment_page.owner = Some(doc);
    crate::page::project::show(domain, env, parts, out);
    domain.changed();
}

/// Show the watched deployment's references without retaining its document.
pub(crate) fn watched(
    domain: &mut Ui,
    env: &Env<crate::Limits>,
    out: &mut Queue<Request>,
    client: &jig_client::Client,
) {
    let projected = domain.borrow(client).deployment_page();
    let mut parts = List::with_capacity(env.limits.parts.saturating_add(1));
    if let Some(owner) = projected.owner {
        for connector in &owner.connectors {
            if parts.push(Part::Deployment { connector: *connector }).is_err() {
                break;
            }
        }
    } else if let Some(doc) = projected.document {
        for part in &doc.parts {
            if parts.push(part.clone()).is_err() {
                break;
            }
        }
    }
    crate::page::project::show(domain, env, parts, out);
    domain.changed();
}
