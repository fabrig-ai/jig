//! A party's account request between people and the accounts child (domain/people.md, 5.1;
//! domain/accounts.md, 3.3 and section 5): people admitted it from its holder, the child decides.

use alloc::boxed::Box;
use jig_domain_accounts as accounts;
use jig_domain_people as people;

/// The account's holder: the asking person's own, or the deployment's.
pub(super) const fn holder(holder: people::AccountHolder, person: u64) -> accounts::Holder {
    match holder {
        people::AccountHolder::Own => accounts::Holder::Party(person),
        people::AccountHolder::Deployment => accounts::Holder::Deployment,
    }
}

/// The child's answer as the party's outcome. A sign-in's subject is checked at its end, a fact,
/// so the subject's refusals (another holder's, or not well formed) are never an answer here.
pub(super) fn outcome(core: &crate::Core, answer: accounts::Answer) -> people::Outcome {
    let refusal = match answer {
        accounts::Answer::MovingOut { account, number, source } => {
            return people::Outcome::AccountMovingOut { account, number, source };
        }
        accounts::Answer::MovedIn { account, renewed } => {
            return people::Outcome::AccountMovedIn { account, renewed };
        }
        accounts::Answer::Finishing { source } => return people::Outcome::AccountFinishing { source },
        accounts::Answer::Done => return people::Outcome::AccountChanged,
        accounts::Answer::Started { source, address } => {
            return people::Outcome::AccountSignInStarted { source, address };
        }
        accounts::Answer::ProgramDeclared { source, name } => {
            return people::Outcome::AccountProgramDeclared { source, name };
        }
        accounts::Answer::SignedOut { confirmed, failed_hosts } => {
            return people::Outcome::AccountSignedOut { confirmed, failed_hosts: hosts(core, failed_hosts) };
        }
        accounts::Answer::Refused { refusal } => refusal,
        accounts::Answer::KeyKept { source } => return people::Outcome::AccountKeyKept { source },
        accounts::Answer::Removed { confirmed, failed_hosts } => {
            return people::Outcome::AccountRemoved { confirmed, failed_hosts: hosts(core, failed_hosts) };
        }
    };
    let refusal = match refusal {
        // Account sign-in ended before its visit, not wanted yet, or closing: the key may ask again.
        accounts::Refusal::Closing
        | accounts::Refusal::NotStarted
        | accounts::Refusal::NotKept { why: accounts::KeyFailure::NotStored } => {
            return people::Outcome::Again(people::Again::NotReady);
        }
        accounts::Refusal::Limit { bound, number } => {
            people::Refusal::Limit { which: limit(bound), bound: numeric_bound(number) }
        }
        accounts::Refusal::Numbers => core.limit_refusal(people::LimitKind::Arithmetic),
        accounts::Refusal::SignInRedirect { why } => people::Refusal::SignInRedirect {
            why: match why {
                accounts::SignInRedirectWhy::NotWaiting => people::SignInRedirectWhy::NotWaiting,
                accounts::SignInRedirectWhy::Address => people::SignInRedirectWhy::Address,
                accounts::SignInRedirectWhy::State => people::SignInRedirectWhy::State,
                accounts::SignInRedirectWhy::TooMany => people::SignInRedirectWhy::TooMany,
            },
        },
        accounts::Refusal::NotSignedIn => people::Refusal::AccountMove { why: people::AccountMoveWhy::NotSignedIn },
        accounts::Refusal::NotOurs => people::Refusal::AccountMove { why: people::AccountMoveWhy::NotOurs },
        accounts::Refusal::Cancelled => people::Refusal::AccountMove { why: people::AccountMoveWhy::Cancelled },
        accounts::Refusal::Moving => people::Refusal::AccountMove { why: people::AccountMoveWhy::Moving },
        accounts::Refusal::Held { .. } => people::Refusal::AccountMove { why: people::AccountMoveWhy::Held },
        accounts::Refusal::Provider { .. }
        | accounts::Refusal::Kind { .. }
        | accounts::Refusal::NotLimited
        | accounts::Refusal::Uses => people::Refusal::NotOffered,
        accounts::Refusal::NotKept { why: accounts::KeyFailure::TooLong } => core.limit_refusal(people::LimitKind::Key),
        accounts::Refusal::Unknown | accounts::Refusal::Subject => people::Refusal::Unknown,
    };
    people::Outcome::Refused(refusal)
}

const fn limit(bound: accounts::Bound) -> people::LimitKind {
    match bound {
        accounts::Bound::Accounts => people::LimitKind::Accounts,
        accounts::Bound::HolderAccounts => people::LimitKind::HolderAccounts,
        accounts::Bound::Sources => people::LimitKind::Sources,
        accounts::Bound::SignIns => people::LimitKind::SignIns,
    }
}

/// Convert the holder's account change and check the provider's uses rule. LLM key
/// changes need the carried envelope and are dispatched by `key_routes` instead.
pub(crate) fn change(
    core: &crate::Core,
    holder: accounts::Holder,
    change: people::AccountChange,
) -> Result<accounts::Change, people::Refusal> {
    Ok(match change {
        people::AccountChange::DeclareProgram { provider, join, name } => {
            if let Some(number) = join {
                let row = core.accounts.account(number).ok_or(people::Refusal::Unknown)?;
                if row.holder() != holder || row.provider() != provider {
                    return Err(people::Refusal::Unknown);
                }
                if let Some(uses) = row.uses() {
                    for use_ in uses {
                        match use_ {
                            accounts::Use::Party(person) => payer(core, holder, true, *person)?,
                            accounts::Use::Project(project) => {
                                let members = core.people.project_members(*project).ok_or(people::Refusal::Unknown)?;
                                for member in members {
                                    payer(core, holder, true, member.person)?;
                                }
                            }
                        }
                    }
                }
            }
            accounts::Change::DeclareProgram { provider, join, name }
        }
        people::AccountChange::CheckProgram { source } => accounts::Change::CheckProgram { source },
        people::AccountChange::AddKey { .. }
        | people::AccountChange::ReplaceKey { .. }
        | people::AccountChange::FinishSignIn { .. }
        | people::AccountChange::MoveIn { .. } => {
            return Err(people::Refusal::NotOffered);
        }
        people::AccountChange::MoveOut { account } => accounts::Change::MoveOut { account },
        people::AccountChange::Written { number } => accounts::Change::Written { number },
        people::AccountChange::RemoveKey { source } => accounts::Change::RemoveKey { source },
        people::AccountChange::Resume { source } => accounts::Change::Resume { source },
        people::AccountChange::Remove { account } => accounts::Change::Remove { account },
        people::AccountChange::MoveAccount { account, place } => accounts::Change::MoveAccount { account, place },
        people::AccountChange::MoveSource { account, source, place } => {
            accounts::Change::MoveSource { account, source, place }
        }
        people::AccountChange::SetUses { account, uses } => {
            let row = core.accounts.account(account).ok_or(people::Refusal::Unknown)?;
            if row.holder() != holder {
                return Err(people::Refusal::Unknown);
            }
            let uses = match uses {
                None => None,
                Some(uses) => {
                    let mut converted =
                        skein_lib::List::with_capacity(u32::try_from(uses.len()).expect("admitted account uses"));
                    for value in &*uses {
                        let value = match *value {
                            people::Use::Party(party) => {
                                payer(core, holder, row.plan(), party)?;
                                accounts::Use::Party(party)
                            }
                            people::Use::Project(project) => {
                                let members = core.people.project_members(project).ok_or(people::Refusal::Unknown)?;
                                for member in members {
                                    payer(core, holder, row.plan(), member.person)?;
                                }
                                accounts::Use::Project(project)
                            }
                        };
                        converted.push(value).expect("one converted account use");
                    }
                    Some(converted.into_boxed())
                }
            };
            accounts::Change::SetUses { account, uses }
        }
    })
}
fn payer(core: &crate::Core, holder: accounts::Holder, plan: bool, party: u64) -> Result<(), people::Refusal> {
    let kind = core.people.kind(party).ok_or(people::Refusal::Unknown)?;
    if plan && holder != accounts::Holder::Party(party) && kind != people::Kind::Service {
        return Err(people::Refusal::Provider);
    }
    Ok(())
}

fn hosts(core: &crate::Core, hosts: Box<[accounts::ProgramHost]>) -> Box<[people::AccountHostFailure]> {
    let count = u32::try_from(hosts.len()).expect("admitted configured membership");
    let mut rows = skein_lib::List::with_capacity(count);
    for host in hosts {
        let name = core.fleet.configured_name(host.worker);
        let why = match host.result {
            accounts::ProgramFailure::NotInContact => people::AccountHostWhy::NotInContact,
            accounts::ProgramFailure::Lost => people::AccountHostWhy::Lost,
            accounts::ProgramFailure::Failed => people::AccountHostWhy::Failed,
        };
        rows.push(people::AccountHostFailure { name: skein_lib::bytes::copy_of(name), why })
            .expect("one output per admitted host failure");
    }
    rows.into_boxed()
}

fn numeric_bound(number: Option<accounts::NumericLimit>) -> Option<people::LimitBound> {
    let number = number?;
    Some(people::LimitBound {
        value: u64::from(number.value),
        unit: people::LimitUnit::Items,
        key: numeric_key(number.key),
    })
}
fn numeric_key(key: Option<accounts::LimitKey>) -> Option<people::LimitKey> {
    let key = key?;
    Some(match key {
        accounts::LimitKey::Accounts => people::LimitKey::AccountsAccounts,
        accounts::LimitKey::HolderAccounts => people::LimitKey::AccountsHolderAccounts,
        accounts::LimitKey::Sources => people::LimitKey::AccountsSources,
        accounts::LimitKey::SignIns => people::LimitKey::AccountsSignIns,
        accounts::LimitKey::ProgramNames => people::LimitKey::AccountsProgramNames,
        accounts::LimitKey::ProgramHosts => people::LimitKey::AccountsProgramHosts,
        accounts::LimitKey::Credentials => people::LimitKey::AccountsCredentials,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn account_room_answers_preserve_their_permanent_limit() {
        for (bound, which) in [
            (accounts::Bound::Accounts, people::LimitKind::Accounts),
            (accounts::Bound::HolderAccounts, people::LimitKind::HolderAccounts),
            (accounts::Bound::Sources, people::LimitKind::Sources),
            (accounts::Bound::SignIns, people::LimitKind::SignIns),
        ] {
            assert_eq!(limit(bound), which);
        }
    }
}
