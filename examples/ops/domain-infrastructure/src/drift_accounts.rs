//! Observed drift is this owner's account (`examples.md`, section 8;
//! `client/domain/root.md`, section 6), including explicit unknown attribution.
use crate::{Domain, Limits, Record, Request, Resource};
use alloc::boxed::Box;
use skein_lib::{Env, Queue, Wall};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DriftKind {
    Changed,
    Deleted,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct DriftAccount {
    pub resource: Resource,
    pub kind: DriftKind,
    /// Owner observation time unless an attributed system observation supplies it.
    pub at: Wall,
    pub by: Option<Box<[u8]>>,
}

pub(crate) fn capacity(limits: &Limits) -> Option<u32> {
    limits.tasks.checked_mul(limits.resources_per_task)?.checked_add(limits.made)
}

pub(crate) fn observed(domain: &mut Domain, env: &Env<Limits>, account: DriftAccount, out: &mut Queue<Request>) {
    if !crate::domain::resource_valid(&account.resource, &env.limits) {
        return;
    }
    let valid_actor = match &account.by {
        Some(actor) => !actor.is_empty() && u32::try_from(actor.len()).unwrap_or(u32::MAX) <= env.limits.name_bytes,
        None => true,
    };
    if !valid_actor
        || domain.drifts.len() >= capacity(&env.limits).expect("drift bound")
            && !domain.drifts.contains_key(&account.resource)
    {
        return;
    }
    if let Some(prior) = domain.drifts.get(&account.resource)
        && (prior == &account || prior.at > account.at)
    {
        return;
    }
    domain.drifts.insert(account.resource.clone(), account.clone()).expect("drift capacity checked");
    out.push(Request::Save { record: Record::Drift(account) });
}
