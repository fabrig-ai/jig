#![expect(clippy::manual_map, reason = "step code translates optional facts without closures")]
//! Whole secret-free accounts documents (client/domain/contract.md, 5.1; domain/accounts.md, 3–5).
use super::{deployment, owner, pieces, w};
use alloc::boxed::Box;
use jig_domain_accounts as a;
use jig_domain_core::documents as d;
use skein_lib::{List, Writer};
pub fn payer(value: a::Use, limits: &w::Limits) -> Result<w::OwnerAccountUse, w::Problem> {
    Ok(match value {
        a::Use::Party(party) => {
            w::OwnerAccountUse::Party(w::OwnerAccountUseParty::new(limits, w::OwnerAccountUsePartyParts { party })?)
        }
        a::Use::Project(project) => w::OwnerAccountUse::Project(w::OwnerAccountUseProject::new(
            limits,
            w::OwnerAccountUseProjectParts { project },
        )?),
    })
}
pub fn holder(value: a::Holder, limits: &w::Limits) -> Result<w::OwnerAccountHolder, w::Problem> {
    Ok(match value {
        a::Holder::Deployment => w::OwnerAccountHolder::Deployment,
        a::Holder::Party(party) => w::OwnerAccountHolder::Party(w::OwnerAccountHolderParty::new(
            limits,
            w::OwnerAccountHolderPartyParts { party },
        )?),
    })
}
fn kind(value: a::Kind) -> w::OwnerAccountKind {
    match value {
        a::Kind::SignIn => w::OwnerAccountKind::SignIn,
        a::Kind::Key => w::OwnerAccountKind::Key,
        a::Kind::Program => w::OwnerAccountKind::Program,
        a::Kind::None => w::OwnerAccountKind::None,
    }
}
fn bound(value: a::Bound) -> w::OwnerAccountBound {
    match value {
        a::Bound::Accounts => w::OwnerAccountBound::Accounts,
        a::Bound::HolderAccounts => w::OwnerAccountBound::HolderAccounts,
        a::Bound::Sources => w::OwnerAccountBound::Sources,
        a::Bound::SignIns => w::OwnerAccountBound::SignIns,
    }
}
fn key_failure(value: a::KeyFailure) -> w::OwnerAccountKeyFailure {
    match value {
        a::KeyFailure::TooLong => w::OwnerAccountKeyFailure::TooLong,
        a::KeyFailure::NotStored => w::OwnerAccountKeyFailure::NotStored,
    }
}
pub fn refusal(value: a::Refusal, limits: &w::Limits) -> Result<w::OwnerAccountRefusal, w::Problem> {
    Ok(match value {
        // A finish refusal leaves the sign-in waiting, so it never ends an accounts watch's sign-in.
        a::Refusal::SignInRedirect { .. } => unreachable!("a redirect refusal is only a finish answer"),
        a::Refusal::NotSignedIn => w::OwnerAccountRefusal::NotSignedIn,
        a::Refusal::NotOurs => w::OwnerAccountRefusal::NotOurs,
        a::Refusal::Cancelled => w::OwnerAccountRefusal::Cancelled,
        a::Refusal::Moving => w::OwnerAccountRefusal::Moving,
        a::Refusal::Unknown => w::OwnerAccountRefusal::Unknown,
        a::Refusal::NotLimited => w::OwnerAccountRefusal::NotLimited,
        a::Refusal::Uses => w::OwnerAccountRefusal::Uses,
        a::Refusal::NotKept { why } => w::OwnerAccountRefusal::NotKept(w::OwnerAccountRefusalNotKept::new(
            limits,
            w::OwnerAccountRefusalNotKeptParts { why: key_failure(why) },
        )?),
        a::Refusal::Limit { bound, .. } => w::OwnerAccountRefusal::Limit(w::OwnerAccountRefusalLimit::new(
            limits,
            w::OwnerAccountRefusalLimitParts { bound: self::bound(bound) },
        )?),
        a::Refusal::Held { provider } => w::OwnerAccountRefusal::Held(w::OwnerAccountRefusalHeld::new(
            limits,
            w::OwnerAccountRefusalHeldParts { provider },
        )?),
        a::Refusal::Provider { provider } => w::OwnerAccountRefusal::Provider(w::OwnerAccountRefusalProvider::new(
            limits,
            w::OwnerAccountRefusalProviderParts { provider },
        )?),
        a::Refusal::Subject => w::OwnerAccountRefusal::Subject,
        a::Refusal::Numbers => w::OwnerAccountRefusal::Numbers,
        a::Refusal::Kind { kind } => w::OwnerAccountRefusal::Kind(w::OwnerAccountRefusalKind::new(
            limits,
            w::OwnerAccountRefusalKindParts { kind: self::kind(kind) },
        )?),
        a::Refusal::Closing => w::OwnerAccountRefusal::Closing,
        a::Refusal::NotStarted => w::OwnerAccountRefusal::NotStarted,
    })
}
pub fn failure(value: a::Failure, limits: &w::Limits) -> Result<w::OwnerAccountFailure, w::Problem> {
    Ok(match value {
        a::Failure::Later { after } => w::OwnerAccountFailure::Later(w::OwnerAccountFailureLater::new(
            limits,
            w::OwnerAccountFailureLaterParts { after },
        )?),
        a::Failure::SignInAgain => w::OwnerAccountFailure::SignInAgain,
        a::Failure::MissingScope { scope } => w::OwnerAccountFailure::MissingScope(
            w::OwnerAccountFailureMissingScope::new(limits, w::OwnerAccountFailureMissingScopeParts { scope })?,
        ),
        a::Failure::Misconfigured => w::OwnerAccountFailure::Misconfigured,
        a::Failure::Unloaded { at, why } => w::OwnerAccountFailure::Unloaded(w::OwnerAccountFailureUnloaded::new(
            limits,
            w::OwnerAccountFailureUnloadedParts {
                at: deployment::account_place(at),
                why: deployment::account_unread(why),
            },
        )?),
        a::Failure::Cancelled => w::OwnerAccountFailure::Cancelled,
    })
}
pub fn end(value: a::End, limits: &w::Limits) -> Result<w::OwnerAccountEnd, w::Problem> {
    Ok(match value {
        a::End::Made { account, declined } => w::OwnerAccountEnd::Made(w::OwnerAccountEndMade::new(
            limits,
            w::OwnerAccountEndMadeParts { account, declined },
        )?),
        a::End::Renewed { account, declined } => w::OwnerAccountEnd::Renewed(w::OwnerAccountEndRenewed::new(
            limits,
            w::OwnerAccountEndRenewedParts { account, declined },
        )?),
        a::End::Refused { refusal } => w::OwnerAccountEnd::Refused(w::OwnerAccountEndRefused::new(
            limits,
            w::OwnerAccountEndRefusedParts { refusal: self::refusal(refusal, limits)? },
        )?),
        a::End::Failed { failure } => w::OwnerAccountEnd::Failed(w::OwnerAccountEndFailed::new(
            limits,
            w::OwnerAccountEndFailedParts { failure: self::failure(failure, limits)? },
        )?),
    })
}
pub fn uses(value: Option<&List<a::Use>>, limits: &w::Limits) -> Result<Option<List<w::OwnerAccountUse>>, w::Problem> {
    let Some(value) = value else {
        return Ok(None);
    };
    let mut rows = List::with_capacity(value.len());
    for row in value {
        rows.push(payer(*row, limits)?).expect("supplied uses fit");
    }
    Ok(Some(rows))
}
pub fn source(value: &d::SourceLine, limits: &w::Limits) -> Result<w::OwnerAccountSource, w::Problem> {
    w::OwnerAccountSource::new(
        limits,
        w::OwnerAccountSourceParts {
            name: value.name.clone(),
            place: value.place,
            number: value.number,
            kind: kind(value.kind),
            state: deployment::account_state(value.state, limits)?,
            usable: value.usable,
            expiring: value.expiring,
            attention: value.attention,
            spent_until: match value.spent_until {
                Some(at) => Some(at.as_nanos()),
                None => None,
            },
        },
    )
}
pub fn source_pieces(
    value: &List<d::SourceLine>,
    limits: &w::Limits,
    bound: u32,
    path: w::Path,
) -> Result<Box<[u8]>, w::Problem> {
    let mut encoded = List::with_capacity(value.len());
    for row in value {
        let row = source(row, limits)?;
        let mut out = Writer::new(usize::try_from(row.measure()).expect("wire measure fits"));
        row.encode(&mut out).expect("measured account source");
        encoded.push(out.finish()).expect("supplied source pieces fit");
    }
    pieces(&encoded, bound, path)
}
pub fn line(value: &d::AccountLine, limits: &w::Limits) -> Result<w::OwnerAccountLine, w::Problem> {
    w::OwnerAccountLine::new(
        limits,
        w::OwnerAccountLineParts {
            number: value.number,
            place: value.place,
            uses: uses(value.uses.as_ref(), limits)?,
            plan: value.plan,
            provider: value.provider,
            provider_name: match &value.provider_name {
                Some(name) => Some(owner::text(name, limits)?),
                None => None,
            },
            manage_usage: value.manage_usage.clone(),
            display: owner::text(&value.display, limits)?,
            holder: holder(value.holder, limits)?,
            sources: source_pieces(
                &value.sources,
                limits,
                limits.owner_account_line_sources,
                w::Path::OwnerAccountLineSources,
            )?,
            cut: value.cut,
        },
    )
}
pub fn document(value: &d::Accounts, limits: &w::Limits) -> Result<w::OwnerAccounts, w::Problem> {
    let mut accounts = List::with_capacity(value.accounts.len());
    for row in &value.accounts {
        let row = line(row, limits)?;
        let mut out = Writer::new(usize::try_from(row.measure()).expect("wire measure fits"));
        row.encode(&mut out).expect("measured account line");
        accounts.push(out.finish()).expect("supplied account pieces fit");
    }
    let accounts = pieces(&accounts, limits.owner_accounts_accounts, w::Path::OwnerAccountsAccounts)?;
    let mut signing = List::with_capacity(value.signing.len());
    for row in &value.signing {
        let row = w::OwnerAccountSignIn::new(
            limits,
            w::OwnerAccountSignInParts {
                source: row.source,
                provider: row.provider,
                holder: holder(row.holder, limits)?,
            },
        )?;
        let mut out = Writer::new(usize::try_from(row.measure()).expect("wire measure fits"));
        row.encode(&mut out).expect("measured sign-in line");
        signing.push(out.finish()).expect("supplied sign-in pieces fit");
    }
    let signing = pieces(&signing, limits.owner_accounts_signing, w::Path::OwnerAccountsSigning)?;
    let last_end = match &value.last_end {
        Some(row) => Some(w::OwnerAccountLastEnd::new(
            limits,
            w::OwnerAccountLastEndParts { source: row.source, end: end(row.end, limits)? },
        )?),
        None => None,
    };
    w::OwnerAccounts::new(
        limits,
        w::OwnerAccountsParts {
            skein_bools: [value.usable, value.settled, value.cut, value.readiness_cut],
            readiness: super::catalogue::readiness(&value.readiness, limits.owner_accounts_readiness, limits)?,
            program_hosts: super::programs::hosts(&value.program_hosts, limits)?,
            accounts,
            signing,
            last_end,
        },
    )
}
