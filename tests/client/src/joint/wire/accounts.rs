//! Independent whole accounts translation for the joint client world (contract.md, 5.1).
use super::{deployment, owner};
use jig_client::DomainLimits as Limits;
use jig_client_domain::contract as c;
use jig_domain_accounts as a;
use jig_domain_core::documents as d;
use skein_lib::List;
pub(super) fn payer(value: a::Use) -> c::OwnerAccountUse {
    match value {
        a::Use::Party(party) => c::OwnerAccountUse::Party(party),
        a::Use::Project(project) => c::OwnerAccountUse::Project(project),
    }
}
pub(super) fn holder(value: a::Holder) -> c::OwnerAccountHolder {
    match value {
        a::Holder::Deployment => c::OwnerAccountHolder::Deployment,
        a::Holder::Party(party) => c::OwnerAccountHolder::Party(party),
    }
}
pub(super) fn kind(value: a::Kind) -> c::OwnerAccountKind {
    match value {
        a::Kind::SignIn => c::OwnerAccountKind::SignIn,
        a::Kind::Key => c::OwnerAccountKind::Key,
        a::Kind::Program => c::OwnerAccountKind::Program,
        a::Kind::None => c::OwnerAccountKind::None,
    }
}
pub(super) fn bound(value: a::Bound) -> c::OwnerAccountBound {
    match value {
        a::Bound::Accounts => c::OwnerAccountBound::Accounts,
        a::Bound::HolderAccounts => c::OwnerAccountBound::HolderAccounts,
        a::Bound::Sources => c::OwnerAccountBound::Sources,
        a::Bound::SignIns => c::OwnerAccountBound::SignIns,
    }
}
pub(super) fn key_failure(value: a::KeyFailure) -> c::OwnerAccountKeyFailure {
    match value {
        a::KeyFailure::TooLong => c::OwnerAccountKeyFailure::TooLong,
        a::KeyFailure::NotStored => c::OwnerAccountKeyFailure::NotStored,
    }
}
pub(super) fn refusal(value: a::Refusal) -> c::OwnerAccountRefusal {
    match value {
        // A finish refusal leaves the sign-in waiting, so it never ends an accounts watch's sign-in.
        a::Refusal::SignInRedirect { .. } => unreachable!("a redirect refusal is only a finish answer"),
        a::Refusal::NotSignedIn => c::OwnerAccountRefusal::NotSignedIn,
        a::Refusal::NotOurs => c::OwnerAccountRefusal::NotOurs,
        a::Refusal::Cancelled => c::OwnerAccountRefusal::Cancelled,
        a::Refusal::Moving => c::OwnerAccountRefusal::Moving,
        a::Refusal::Unknown => c::OwnerAccountRefusal::Unknown,
        a::Refusal::NotLimited => c::OwnerAccountRefusal::NotLimited,
        a::Refusal::Uses => c::OwnerAccountRefusal::Uses,
        a::Refusal::NotKept { why } => c::OwnerAccountRefusal::NotKept { why: self::key_failure(why) },
        a::Refusal::Limit { bound, .. } => c::OwnerAccountRefusal::Limit { bound: self::bound(bound) },
        a::Refusal::Held { provider } => c::OwnerAccountRefusal::Held { provider },
        a::Refusal::Provider { provider } => c::OwnerAccountRefusal::Provider { provider },
        a::Refusal::Subject => c::OwnerAccountRefusal::Subject,
        a::Refusal::Numbers => c::OwnerAccountRefusal::Numbers,
        a::Refusal::Kind { kind } => c::OwnerAccountRefusal::Kind { kind: self::kind(kind) },
        a::Refusal::Closing => c::OwnerAccountRefusal::Closing,
        a::Refusal::NotStarted => c::OwnerAccountRefusal::NotStarted,
    }
}
pub(super) fn failure(value: a::Failure) -> c::OwnerAccountFailure {
    match value {
        a::Failure::Later { after } => c::OwnerAccountFailure::Later { after },
        a::Failure::SignInAgain => c::OwnerAccountFailure::SignInAgain,
        a::Failure::MissingScope { scope } => c::OwnerAccountFailure::MissingScope { scope },
        a::Failure::Misconfigured => c::OwnerAccountFailure::Misconfigured,
        a::Failure::Unloaded { at, why } => c::OwnerAccountFailure::Unloaded {
            at: match at {
                a::Place::Directory => c::OwnerAccountPlace::Directory,
                a::Place::File => c::OwnerAccountPlace::File,
            },
            why: match why {
                a::Unread::Unreadable => c::OwnerAccountUnread::Unreadable,
                a::Unread::Refused => c::OwnerAccountUnread::Refused,
                a::Unread::Failed => c::OwnerAccountUnread::Failed,
                a::Unread::Stalled => c::OwnerAccountUnread::Stalled,
                a::Unread::TooLarge => c::OwnerAccountUnread::TooLarge,
            },
        },
        a::Failure::Cancelled => c::OwnerAccountFailure::Cancelled,
    }
}
pub(super) fn end(value: a::End) -> c::OwnerAccountEnd {
    match value {
        a::End::Made { account, declined } => c::OwnerAccountEnd::Made { account, declined },
        a::End::Renewed { account, declined } => c::OwnerAccountEnd::Renewed { account, declined },
        a::End::Refused { refusal } => c::OwnerAccountEnd::Refused { refusal: self::refusal(refusal) },
        a::End::Failed { failure } => c::OwnerAccountEnd::Failed { failure: self::failure(failure) },
    }
}
pub(super) fn uses(value: Option<&List<a::Use>>) -> Option<List<c::OwnerAccountUse>> {
    value.map(|items| {
        let mut out = List::with_capacity(items.capacity());
        for item in items {
            out.push(payer(*item)).expect("supplied uses");
        }
        out
    })
}
/// # Errors
/// Refuses a source collection above the receiving row bound.
pub(super) fn sources(
    value: &List<d::SourceLine>,
    limits: &Limits,
) -> Result<List<c::OwnerAccountSource>, owner::Invalid> {
    owner::list(value, limits, |row| {
        Ok(c::OwnerAccountSource {
            name: row.name.as_deref().map(super::programs::program_name).transpose()?,
            place: row.place,
            number: row.number,
            kind: kind(row.kind),
            state: deployment::account_state(row.state),
            usable: row.usable,
            expiring: row.expiring,
            attention: row.attention,
            spent_until: row.spent_until,
        })
    })
}
/// # Errors
/// Refuses an account collection or display above the receiving bounds.
pub(super) fn document(value: &d::Accounts, limits: &Limits) -> Result<c::OwnerAccounts, owner::Invalid> {
    Ok(c::OwnerAccounts {
        readiness: super::catalogue::readiness(&value.readiness, limits)?,
        readiness_cut: value.readiness_cut,
        program_hosts: super::programs::hosts(&value.program_hosts, limits)?,
        accounts: owner::list(&value.accounts, limits, |row| {
            Ok(c::OwnerAccountLine {
                number: row.number,
                place: row.place,
                uses: uses(row.uses.as_ref()),
                plan: row.plan,
                provider: row.provider,
                provider_name: row.provider_name.as_ref().map(|name| owner::text(name, limits)).transpose()?,
                manage_usage: super::catalogue::manage_usage(row.manage_usage.as_deref())?,
                display: owner::text(&row.display, limits)?,
                holder: holder(row.holder),
                sources: sources(&row.sources, limits)?,
                cut: row.cut,
            })
        })?,
        signing: owner::list(&value.signing, limits, |row| {
            Ok(c::OwnerAccountSignIn { source: row.source, provider: row.provider, holder: holder(row.holder) })
        })?,
        last_end: value.last_end.as_ref().map(|row| c::OwnerAccountLastEnd { source: row.source, end: end(row.end) }),
        usable: value.usable,
        settled: value.settled,
        cut: value.cut,
    })
}
