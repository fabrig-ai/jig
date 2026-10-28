//! Public offers and payer readiness (client/domain/contract.md, 5.1 and 7.1).
use super::{DocumentLimits, Missing, Text};
use crate::{Core, Limits};
use alloc::boxed::Box;
use jig_domain_accounts as accounts;
use jig_domain_fleet as fleet;
use jig_domain_tasks as tasks;
use skein_lib::List;

/// Public configuration, independent of private account identities.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[expect(clippy::struct_excessive_bools, reason = "independent configured source offers and cut facts")]
pub struct ProviderOffer {
    /// Stable configured LLM provider number.
    pub provider: u32,
    /// Actual startup key ceiling for this provider's key offer.
    pub key_bytes: u32,
    /// Configured display name, carrying its own text loss.
    pub name: Text,
    /// Whether a sign-in source is offered.
    pub sign_in: bool,
    /// Whether a key source is offered.
    pub key: bool,
    /// Whole configured program names within the list bound.
    pub programs: List<Box<[u8]>>,
    /// Whether additional program names were omitted.
    pub programs_cut: bool,
    /// Whether configuration offers a source needing no credential.
    pub none: bool,
    /// Whether configured sign-ins use plan terms.
    pub signed_in_plan: bool,
    /// Whole optional public HTTPS manage target, at most 2,048 bytes.
    pub manage_usage: Option<Box<[u8]>>,
}
/// Availability is an observation, never a run reservation.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Readiness {
    Usable,
    Unavailable,
    Unknown,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PayerReadiness {
    /// Stable configured LLM provider number.
    pub provider: u32,
    /// The authenticated party or a project it may read.
    pub payer: accounts::Use,
    /// Current observation from the engine selection and host checks.
    pub readiness: Readiness,
    /// None until program check facts can establish settlement.
    pub settled: Option<bool>,
}

pub(super) fn offers(core: &Core, limits: &DocumentLimits) -> Result<(List<ProviderOffer>, bool), Missing> {
    let mut result = List::with_capacity(limits.lines);
    let mut cut = false;
    for provider in &core.settings.llm_providers.providers {
        if result.room() == 0 {
            cut = true;
            continue;
        }
        let mut programs = List::with_capacity(limits.lines);
        let mut programs_cut = false;
        for name in &provider.offers.programs {
            if programs.room() == 0 {
                programs_cut = true;
                continue;
            }
            if name.len() > 32 {
                return Err(Missing::TooLarge);
            }
            programs.push(name.clone()).or(Err(Missing::TooLarge))?;
        }
        result
            .push(ProviderOffer {
                provider: provider.number,
                key_bytes: core.settings.key_bytes,
                name: super::small::text(&provider.name, limits.text_bytes),
                sign_in: provider.offers.sign_in,
                key: provider.offers.key,
                programs,
                programs_cut,
                none: provider.offers.none(),
                signed_in_plan: provider.signed_in_plan,
                manage_usage: provider.manage_usage.clone(),
            })
            .or(Err(Missing::TooLarge))?;
    }
    Ok((result, cut))
}

pub(super) fn rows(core: &Core, limits: &Limits, party: u64) -> Result<(List<PayerReadiness>, bool), Missing> {
    let mut rows = List::with_capacity(limits.documents.lines);
    let mut cut = false;
    let pending = core.accounts.pending();
    append(core, pending.as_slice(), tasks::Payer::Party(party), &mut rows, &mut cut)?;
    let roles = core.people.project_roles(party).ok_or(Missing::Unknown)?;
    for role in &roles {
        append(core, pending.as_slice(), tasks::Payer::Project(role.project), &mut rows, &mut cut)?;
    }
    Ok((rows, cut))
}
fn append(
    core: &Core,
    pending: &[accounts::Pending],
    payer: tasks::Payer,
    rows: &mut List<PayerReadiness>,
    cut: &mut bool,
) -> Result<(), Missing> {
    for provider in &core.settings.llm_providers.providers {
        if rows.room() == 0 {
            *cut = true;
            continue;
        }
        let (readiness, refreshing) = readiness(core, payer, provider.number);
        let mut settled = if provider.offers.programs.is_empty() { Some(true) } else { None };
        if refreshing {
            settled = Some(false);
        }
        for pending in pending {
            if payer == tasks::Payer::Party(pending.by) && pending.provider == provider.number {
                settled = Some(false);
            }
        }
        rows.push(PayerReadiness {
            provider: provider.number,
            payer: crate::coverage::payer(payer),
            readiness,
            settled,
        })
        .or(Err(Missing::TooLarge))?;
    }
    Ok(())
}

/// Full-state selection precedes any document truncation. No task pin is chosen here.
fn readiness(core: &Core, payer: tasks::Payer, provider: u32) -> (Readiness, bool) {
    let coverage = core.account_coverage(payer);
    let mut unknown = false;
    let mut refreshing = false;
    let hosts = core.fleet.host_views();
    for model in &core.settings.models {
        if core.provider(model) != Some(provider) {
            continue;
        }
        let candidates = core.accounts.candidates(
            crate::coverage::payer(payer),
            provider,
            model.program.as_deref(),
            None,
            None,
            coverage.as_slice(),
        );
        if candidates.is_empty() {
            continue;
        }
        let mut facts = List::with_capacity(core.host_fact_capacity());
        for host in &hosts {
            if !host.connected || host.draining || host.slots == 0 {
                continue;
            }
            if host.kind == fleet::HostKind::Worker && core.fleet.awaiting_logins(host.channel) {
                unknown = true;
                continue;
            }
            let placement = fleet::PlacementConstraint { channel: host.channel, kind: host.kind };
            if !core.fleet.supports(placement, &[crate::fleet_needs::model(model)]) {
                continue;
            }
            core.host_facts(placement, core.host_rule(placement), &mut facts);
            for account in &candidates {
                match core.accounts.select(
                    crate::coverage::payer(payer),
                    provider,
                    model.program.as_deref(),
                    Some(*account),
                    None,
                    coverage.as_slice(),
                    facts.as_slice(),
                    core.run_now,
                ) {
                    accounts::Selection::Chosen { .. } => return (Readiness::Usable, false),
                    accounts::Selection::Wait { why: accounts::WaitWhy::Host, .. } => unknown = true,
                    accounts::Selection::Wait { why: accounts::WaitWhy::Refreshing, .. } => refreshing = true,
                    accounts::Selection::Wait {
                        why: accounts::WaitWhy::Source(_) | accounts::WaitWhy::Empty, ..
                    }
                    | accounts::Selection::NoAccount => {}
                }
            }
        }
    }
    (if unknown || refreshing { Readiness::Unknown } else { Readiness::Unavailable }, refreshing)
}

pub(super) fn offers_worst_case(limits: &DocumentLimits) -> Option<u64> {
    let heap = List::<Box<[u8]>>::worst_case(limits.lines)?
        .checked_add(u64::from(limits.lines).checked_mul(32)?)?
        .checked_add(u64::from(limits.text_bytes))?
        .checked_add(u64::from(accounts::MANAGE_USAGE))?;
    List::<ProviderOffer>::worst_case(limits.lines)?.checked_add(u64::from(limits.lines).checked_mul(heap)?)
}
pub(super) fn rows_worst_case(limits: &DocumentLimits) -> Option<u64> {
    List::<PayerReadiness>::worst_case(limits.lines)
}
