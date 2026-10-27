//! Exact sparse entrance facts (client/domain/contract.md, 4–5).
//! Missing explanations, actors' names and numerical bounds remain absent.
use jig_client_domain::contract as client;
use jig_domain_people as people;

/// # Errors
/// Refuses unsupported sparse facts; LLM refusals need the catalogue at the answer boundary.
pub fn refusal(value: people::Refusal) -> Result<client::Refusal, super::owner::Invalid> {
    Ok(match value {
        people::Refusal::Forgotten { oldest } => client::Refusal::Forgotten(client::RefusalForgotten { oldest }),
        people::Refusal::Stale { newest } => client::Refusal::Stale(client::RefusalStale { newest }),
        people::Refusal::Full { revision, tasks, more } => {
            client::Refusal::Full(client::RefusalFull { revision, tasks: names(&tasks), more })
        }
        people::Refusal::Role { held } => client::Refusal::Role { held: held.map(super::owner::held_role) },
        people::Refusal::Authority { lacking, proposable } => client::Refusal::Authority {
            lacking: lack(lacking),
            proposable: match proposable {
                Some(people::Accepter::Role(_)) => Some(true),
                Some(people::Accepter::Nobody) => Some(false),
                None => None,
            },
        },
        people::Refusal::Limit { which } => {
            client::Refusal::Limit { which: client::Limit::Admission { which: limit(which) } }
        }
        people::Refusal::Moved { now } => moved(now),
        people::Refusal::Ended { how } => client::Refusal::Ended { how: ending(how) },
        people::Refusal::Taken { by } => client::Refusal::Taken { by },
        people::Refusal::NotWellFormed { at, why } => client::Refusal::Malformed {
            at,
            why: match why {
                people::Malformed::Settings { setting, fault } => {
                    client::Malformed::Settings(client::MalformedSettings {
                        setting: settings_name(setting),
                        fault: settings_fault(fault),
                    })
                }
                people::Malformed::SettingsUnchanged => client::Malformed::SettingsUnchanged,
                people::Malformed::Running => client::Malformed::Running,
                people::Malformed::Name(rule) => client::Malformed::Name(name_rule(rule)),
                people::Malformed::AsAllowedTwice => client::Malformed::TwoAsAllowed,
                people::Malformed::EndedBy => client::Malformed::EndedBy,
                people::Malformed::Conversation => client::Malformed::ConversationNotAgent,
                people::Malformed::Cycle => client::Malformed::Cycle,
                people::Malformed::ProposedAsAllowed => client::Malformed::ProposedAsAllowed,
            },
        },
        people::Refusal::Circle { tasks, cut } => client::Refusal::Circle { tasks: names(&tasks), cut },
        people::Refusal::Kept { pending } => client::Refusal::Kept { pending: names(&pending) },
        people::Refusal::Dependency { dependency, how } => client::Refusal::Dependency {
            dependency,
            how: match how {
                people::DependencyStatus::Done => client::OwnerTaskStatus::Done,
                people::DependencyStatus::Failed => client::OwnerTaskStatus::Failed,
                people::DependencyStatus::Cancelled => client::OwnerTaskStatus::Cancelled,
            },
        },
        people::Refusal::NotWaiting => client::Refusal::NotWaiting,
        people::Refusal::NoFurther => client::Refusal::NoFurther,
        people::Refusal::Unknown => client::Refusal::Unknown,
        people::Refusal::SignInRedirect { why } => client::Refusal::SignInRedirect {
            why: match why {
                people::SignInRedirectWhy::NotWaiting => client::SignInRedirectWhy::NotWaiting,
                people::SignInRedirectWhy::Address => client::SignInRedirectWhy::Address,
                people::SignInRedirectWhy::State => client::SignInRedirectWhy::State,
                people::SignInRedirectWhy::TooMany => client::SignInRedirectWhy::TooMany,
            },
        },
        people::Refusal::AccountMove { why } => client::Refusal::AccountMove {
            why: match why {
                people::AccountMoveWhy::NotSignedIn => client::AccountMoveWhy::NotSignedIn,
                people::AccountMoveWhy::NotOurs => client::AccountMoveWhy::NotOurs,
                people::AccountMoveWhy::Held => client::AccountMoveWhy::Held,
                people::AccountMoveWhy::Cancelled => client::AccountMoveWhy::Cancelled,
                people::AccountMoveWhy::Moving => client::AccountMoveWhy::Moving,
            },
        },
        people::Refusal::Account { account, why } => client::Refusal::Account {
            account,
            why: match why {
                people::AccountWhy::Unknown => client::AccountWhy::Unknown,
                people::AccountWhy::Provider => client::AccountWhy::Provider,
                people::AccountWhy::Uses => client::AccountWhy::Uses,
                people::AccountWhy::Limited => client::AccountWhy::Limited,
            },
        },
        people::Refusal::Provider => client::Refusal::Provider,
        people::Refusal::NameTaken => client::Refusal::NameTaken,
        people::Refusal::NotOffered | people::Refusal::NoEscalationHolder => client::Refusal::NotOffered,
        people::Refusal::KeyConflict => client::Refusal::KeyConflict,
        people::Refusal::Llm { .. } => return Err(super::owner::Invalid::Llm),
    })
}

#[must_use]
pub fn lack(value: people::Lack) -> client::Lacking {
    match value {
        people::Lack::Approver => client::Lacking::Approver,
        people::Lack::Budget { needs, left } => client::Lacking::Budget { needed: needs, left },
        people::Lack::Grant { kind } => client::Lacking::GrantKind { kind },
        people::Lack::Funding => client::Lacking::Funding,
        people::Lack::TaskKind { .. } => client::Lacking::TaskKind { executor: None },
        people::Lack::Depth { most } => client::Lacking::Depth { needed: None, allowed: most },
        people::Lack::Deadline => client::Lacking::Deadline { asked: None, latest: None },
        people::Lack::Tasks { left } => client::Lacking::Tasks { needed: None, left },
        people::Lack::Tool => client::Lacking::Tool,
        people::Lack::Scope => client::Lacking::Scope,
        people::Lack::Models => client::Lacking::Models,
        people::Lack::Reference => client::Lacking::Reference,
        people::Lack::ResourceAccess => client::Lacking::ResourceAccess,
        people::Lack::ResourceUnavailable => client::Lacking::ResourceUnavailable,
        people::Lack::Shape => client::Lacking::Shape,
        people::Lack::Again { required } => client::Lacking::Again { required },
        people::Lack::Account => client::Lacking::Account,
        people::Lack::Writer => client::Lacking::Writer,
        people::Lack::Required { connector, kind } => client::Lacking::Required { connector, kind },
        people::Lack::Failed { connector, kind } => client::Lacking::Failed { connector, kind },
        people::Lack::Unguarded { connector, kind } => client::Lacking::Unguarded { connector, kind },
    }
}

#[must_use]
pub fn limit(value: people::LimitKind) -> client::AdmissionLimit {
    match value {
        people::LimitKind::Projects => client::AdmissionLimit::Projects,
        people::LimitKind::Words => client::AdmissionLimit::Words,
        people::LimitKind::Identity => client::AdmissionLimit::Identity,
        people::LimitKind::Roles => client::AdmissionLimit::Roles,
        people::LimitKind::Arithmetic => client::AdmissionLimit::Arithmetic,
        people::LimitKind::Duplicate => client::AdmissionLimit::Duplicate,
        people::LimitKind::Empty => client::AdmissionLimit::Empty,
        people::LimitKind::Batch => client::AdmissionLimit::Batch,
        people::LimitKind::ProjectLiveTasks | people::LimitKind::Goals => client::AdmissionLimit::ProjectLiveTasks,
        people::LimitKind::LiveTasks => client::AdmissionLimit::LiveTasks,
        people::LimitKind::Tree => client::AdmissionLimit::Tree,
        people::LimitKind::Depth => client::AdmissionLimit::Depth,
        people::LimitKind::Delegates => client::AdmissionLimit::Delegates,
        people::LimitKind::Subscription => client::AdmissionLimit::Subscription,
        people::LimitKind::Dependencies => client::AdmissionLimit::Dependencies,
        people::LimitKind::Cycle => client::AdmissionLimit::Cycle,
        people::LimitKind::Operations => client::AdmissionLimit::Operations,
        people::LimitKind::Grace => client::AdmissionLimit::Grace,
        people::LimitKind::Spec => client::AdmissionLimit::Spec,
        people::LimitKind::Contract => client::AdmissionLimit::Contract,
        people::LimitKind::Authority => client::AdmissionLimit::Authority,
        people::LimitKind::Inputs => client::AdmissionLimit::Inputs,
        people::LimitKind::Attempts => client::AdmissionLimit::Attempts,
        people::LimitKind::LiveDelegates => client::AdmissionLimit::LiveDelegates,
        people::LimitKind::Restore => client::AdmissionLimit::Restore,
        people::LimitKind::Read => client::AdmissionLimit::Read,
        people::LimitKind::Turn => client::AdmissionLimit::Turn,
        people::LimitKind::HoldKind => client::AdmissionLimit::HoldKind,
        people::LimitKind::HoldTaken => client::AdmissionLimit::HoldTaken,
        people::LimitKind::Holds => client::AdmissionLimit::Holds,
        people::LimitKind::InboxFull => client::AdmissionLimit::InboxFull,
        people::LimitKind::Notes => client::AdmissionLimit::Notes,
        people::LimitKind::Revision => client::AdmissionLimit::Revision,
        people::LimitKind::Question => client::AdmissionLimit::Question,
        people::LimitKind::Priority => client::AdmissionLimit::Priority,
        people::LimitKind::Policy => client::AdmissionLimit::Policy,
        people::LimitKind::Resource => client::AdmissionLimit::Resource,
        people::LimitKind::Service => client::AdmissionLimit::Service,
        people::LimitKind::PlacedHolds => client::AdmissionLimit::PlacedHolds,
        people::LimitKind::ProjectPlacedHolds => client::AdmissionLimit::ProjectPlacedHolds,
        people::LimitKind::PartyPlacedHolds => client::AdmissionLimit::PartyPlacedHolds,
        people::LimitKind::Exemptions => client::AdmissionLimit::Exemptions,
        people::LimitKind::Accounts => client::AdmissionLimit::Accounts,
        people::LimitKind::HolderAccounts => client::AdmissionLimit::HolderAccounts,
        people::LimitKind::Sources => client::AdmissionLimit::Sources,
        people::LimitKind::SignIns => client::AdmissionLimit::SignIns,
        people::LimitKind::SettingsProposals => client::AdmissionLimit::SettingsProposals,
    }
}

#[must_use]
pub fn moved(value: people::Moved) -> client::Refusal {
    let (revision, what) = match value {
        people::Moved::Hold { revision, reason } => {
            (revision, client::Moved::Hold { reason: Some(client::CurrentHold::Reason(hold(reason))) })
        }
        people::Moved::Spec { revision } => (revision, client::Moved::Spec),
        people::Moved::Person { revision } => (revision, client::Moved::PersonTask),
        people::Moved::Note { revision } => (u64::from(revision), client::Moved::Note),
    };
    client::Refusal::Moved { now: client::Revision { value: revision }, what }
}

#[must_use]
pub fn hold(value: people::HoldReason) -> client::HoldCategory {
    match value {
        people::HoldReason::Exceeded(value) => client::HoldCategory::Exceeded(client::Exceeded {
            bound: match value.bound {
                people::Bound::Reasoning => client::Bound::Reasoning,
                people::Bound::Metadata => client::Bound::Metadata,
                people::Bound::Items => client::Bound::Items,
                people::Bound::Answer => client::Bound::Answer,
                people::Bound::Tools => client::Bound::Tools,
                people::Bound::HistoryItems => client::Bound::HistoryItems,
                people::Bound::History => client::Bound::History,
                people::Bound::Window => client::Bound::Window,
                people::Bound::Output => client::Bound::Output,
            },
            value: value.value,
        }),
        people::HoldReason::Failures { class } => client::HoldCategory::Failures { class: failure(class) },
        people::HoldReason::Stalled => client::HoldCategory::Stalled,
        people::HoldReason::Dependency { task } => client::HoldCategory::Dependency { task },
        people::HoldReason::Stopped => client::HoldCategory::Stopped,
        people::HoldReason::Placed { hold } => client::HoldCategory::Placed { hold },
        people::HoldReason::Drift => client::HoldCategory::Drift,
        people::HoldReason::Effects => client::HoldCategory::Effects,
        people::HoldReason::EffectFailed => client::HoldCategory::EffectFailed,
        people::HoldReason::Uncertain { entry } => client::HoldCategory::Uncertain { entry },
        people::HoldReason::Budget => client::HoldCategory::Budget,
        people::HoldReason::Deadline => client::HoldCategory::Deadline,
        people::HoldReason::HoldsWaited => client::HoldCategory::HoldsWaited,
        people::HoldReason::ResourceUnavailable => client::HoldCategory::ResourceUnavailable,
        people::HoldReason::ResourceAccess => client::HoldCategory::ResourceAccess,
        people::HoldReason::PoolLost { pool } => client::HoldCategory::PoolLost { pool },
        people::HoldReason::Procedure => client::HoldCategory::Procedure,
        people::HoldReason::Circle => client::HoldCategory::Circle,
    }
}

fn failure(value: people::FailureClass) -> client::HoldFailure {
    match value {
        people::FailureClass::Transient => client::HoldFailure::Transient,
        people::FailureClass::Permanent => client::HoldFailure::Permanent,
        people::FailureClass::Run => client::HoldFailure::Run,
        people::FailureClass::Agent => client::HoldFailure::Agent,
        people::FailureClass::Lost => client::HoldFailure::Lost,
        people::FailureClass::Invalid => client::HoldFailure::Invalid,
    }
}

#[must_use]
pub fn ending(value: people::Ending) -> client::Ending {
    match value {
        people::Ending::Done => client::Ending::Done,
        people::Ending::Failed => client::Ending::Failed { reason: None },
        people::Ending::Cancelled { by, at } => client::Ending::Cancelled {
            by: match by {
                people::Party::Person(number) => client::Actor::Party(number),
                people::Party::Task(number) => client::Actor::Task(number),
                people::Party::Deployment { project } => client::Actor::ProjectDeployment { project },
            },
            reason: None,
            at: Some(at),
        },
        people::Ending::Decided { by, at } => client::Ending::Decided { by, party: None, how: None, at },
        people::Ending::Deleted => client::Ending::Deleted,
    }
}

fn names(values: &[u64]) -> skein_lib::List<u64> {
    let mut rows = skein_lib::List::with_capacity(u32::try_from(values.len()).expect("bounded numbers"));
    for number in values {
        rows.push(*number).expect("same number count");
    }
    rows
}

fn name_rule(rule: people::NameRule) -> client::NameRule {
    match rule {
        people::NameRule::Empty => client::NameRule::Empty,
        people::NameRule::Long => client::NameRule::Long,
        people::NameRule::Control => client::NameRule::Control,
        people::NameRule::Space => client::NameRule::Space,
        people::NameRule::Digits => client::NameRule::Digits,
    }
}

fn settings_name(value: people::Setting) -> client::Setting {
    match value {
        people::Setting::Charter(number) => client::Setting::Charter(client::SettingNumber { number }),
        people::Setting::Template(number) => client::Setting::Template(client::SettingNumber { number }),
        people::Setting::Policy(number) => client::Setting::Policy(client::SettingNumber { number }),
        people::Setting::Rules => client::Setting::Rules,
    }
}
fn settings_fault(value: people::SettingFault) -> client::SettingFault {
    match value {
        people::SettingFault::Shape => client::SettingFault::Shape,
        people::SettingFault::Count => client::SettingFault::Count,
        people::SettingFault::Reference => client::SettingFault::Reference,
        people::SettingFault::Rules => client::SettingFault::Rules,
        people::SettingFault::Budget => client::SettingFault::Budget,
        people::SettingFault::Grace => client::SettingFault::Grace,
        people::SettingFault::Owner => client::SettingFault::Owner,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skein_lib::Wall;

    #[test]
    fn malformed_refusals_keep_their_reason() {
        for (why, expected) in [
            (people::Malformed::AsAllowedTwice, client::Malformed::TwoAsAllowed),
            (people::Malformed::Conversation, client::Malformed::ConversationNotAgent),
            (people::Malformed::Cycle, client::Malformed::Cycle),
            (people::Malformed::ProposedAsAllowed, client::Malformed::ProposedAsAllowed),
        ] {
            assert_eq!(
                refusal(people::Refusal::NotWellFormed { at: 7, why }),
                Ok(client::Refusal::Malformed { at: 7, why: expected })
            );
        }
    }

    #[test]
    fn sparse_refusals_preserve_absent_facts_and_full_width_supplied_identities() {
        let cases = [
            (people::Refusal::Role { held: None }, client::Refusal::Role { held: None }),
            (
                people::Refusal::Role { held: Some(people::Role::Policy { role: u32::MAX }) },
                client::Refusal::Role { held: Some(client::HeldRole::Policy { role: u32::MAX }) },
            ),
            (people::Refusal::Taken { by: u64::MAX }, client::Refusal::Taken { by: u64::MAX }),
            (people::Refusal::NotWaiting, client::Refusal::NotWaiting),
            (people::Refusal::NoFurther, client::Refusal::NoFurther),
            (people::Refusal::Unknown, client::Refusal::Unknown),
            (people::Refusal::NotOffered, client::Refusal::NotOffered),
            (people::Refusal::KeyConflict, client::Refusal::KeyConflict),
            (people::Refusal::NoEscalationHolder, client::Refusal::NotOffered),
        ];
        for (source, expected) in cases {
            let actual = refusal(source).expect("representable owner value");
            assert_eq!(actual, expected);
            assert!(client::EntranceRefusal::new(actual).is_ok());
        }
        for (proposable, boolean) in
            [(None, None), (Some(people::Accepter::Nobody), Some(false)), (Some(people::Accepter::Role(0)), Some(true))]
        {
            let source = people::Refusal::Authority { lacking: people::Lack::TaskKind { executor: None }, proposable };
            assert_eq!(
                refusal(source).expect("representable owner value"),
                client::Refusal::Authority {
                    lacking: client::Lacking::TaskKind { executor: None },
                    proposable: boolean
                }
            );
        }
        assert_eq!(
            lack(people::Lack::Budget { needs: u64::MAX, left: 0 }),
            client::Lacking::Budget { needed: u64::MAX, left: 0 }
        );
        assert_eq!(
            lack(people::Lack::Depth { most: u32::MAX }),
            client::Lacking::Depth { needed: None, allowed: u32::MAX }
        );
        assert_eq!(lack(people::Lack::Deadline), client::Lacking::Deadline { asked: None, latest: None });
        assert_eq!(
            lack(people::Lack::Tasks { left: u32::MAX }),
            client::Lacking::Tasks { needed: None, left: u32::MAX }
        );
        assert_eq!(ending(people::Ending::Failed), client::Ending::Failed { reason: None });
        assert_eq!(
            ending(people::Ending::Decided { by: u64::MAX, at: Wall::EPOCH }),
            client::Ending::Decided { by: u64::MAX, party: None, how: None, at: Wall::EPOCH }
        );
        for (by, actor) in [
            (people::Party::Person(u64::MAX), client::Actor::Party(u64::MAX)),
            (people::Party::Task(u64::MAX), client::Actor::Task(u64::MAX)),
            (people::Party::Deployment { project: u32::MAX }, client::Actor::ProjectDeployment { project: u32::MAX }),
        ] {
            assert_eq!(
                ending(people::Ending::Cancelled { by, at: Wall::EPOCH }),
                client::Ending::Cancelled { by: actor, reason: None, at: Some(Wall::EPOCH) }
            );
        }
    }

    #[test]
    fn independent_moved_revisions_keep_sparse_hold_categories_and_numbered_references() {
        for (source, revision, what) in [
            (people::Moved::Spec { revision: u64::MAX }, u64::MAX, client::Moved::Spec),
            (people::Moved::Person { revision: 17 }, 17, client::Moved::PersonTask),
            (people::Moved::Note { revision: u32::MAX }, u64::from(u32::MAX), client::Moved::Note),
        ] {
            assert_eq!(moved(source), client::Refusal::Moved { now: client::Revision { value: revision }, what });
        }
        for (source, expected) in [
            (people::HoldReason::Stalled, client::HoldCategory::Stalled),
            (people::HoldReason::Dependency { task: u64::MAX }, client::HoldCategory::Dependency { task: u64::MAX }),
            (people::HoldReason::Stopped, client::HoldCategory::Stopped),
            (people::HoldReason::Placed { hold: u64::MAX }, client::HoldCategory::Placed { hold: u64::MAX }),
            (people::HoldReason::Drift, client::HoldCategory::Drift),
            (people::HoldReason::Effects, client::HoldCategory::Effects),
            (people::HoldReason::EffectFailed, client::HoldCategory::EffectFailed),
            (people::HoldReason::Uncertain { entry: u64::MAX }, client::HoldCategory::Uncertain { entry: u64::MAX }),
            (people::HoldReason::Budget, client::HoldCategory::Budget),
            (people::HoldReason::Deadline, client::HoldCategory::Deadline),
            (people::HoldReason::HoldsWaited, client::HoldCategory::HoldsWaited),
            (people::HoldReason::ResourceUnavailable, client::HoldCategory::ResourceUnavailable),
            (people::HoldReason::ResourceAccess, client::HoldCategory::ResourceAccess),
            (people::HoldReason::PoolLost { pool: u64::MAX }, client::HoldCategory::PoolLost { pool: u64::MAX }),
            (people::HoldReason::Procedure, client::HoldCategory::Procedure),
        ] {
            let actual = moved(people::Moved::Hold { revision: 31, reason: source });
            assert_eq!(
                actual,
                client::Refusal::Moved {
                    now: client::Revision { value: 31 },
                    what: client::Moved::Hold { reason: Some(client::CurrentHold::Reason(expected)) },
                }
            );
            assert!(client::EntranceRefusal::new(actual).is_ok());
        }
        for (source, expected) in [
            (people::FailureClass::Transient, client::HoldFailure::Transient),
            (people::FailureClass::Permanent, client::HoldFailure::Permanent),
            (people::FailureClass::Run, client::HoldFailure::Run),
            (people::FailureClass::Agent, client::HoldFailure::Agent),
            (people::FailureClass::Lost, client::HoldFailure::Lost),
            (people::FailureClass::Invalid, client::HoldFailure::Invalid),
        ] {
            assert_eq!(
                hold(people::HoldReason::Failures { class: source }),
                client::HoldCategory::Failures { class: expected }
            );
        }
    }

    #[test]
    fn every_entrance_admission_category_remains_a_category_without_a_guessed_bound() {
        use client::AdmissionLimit as To;
        use people::LimitKind as From;
        let cases = [
            (From::Words, To::Words),
            (From::Identity, To::Identity),
            (From::Roles, To::Roles),
            (From::Arithmetic, To::Arithmetic),
            (From::Duplicate, To::Duplicate),
            (From::Empty, To::Empty),
            (From::Batch, To::Batch),
            (From::ProjectLiveTasks, To::ProjectLiveTasks),
            (From::LiveTasks, To::LiveTasks),
            (From::Tree, To::Tree),
            (From::Depth, To::Depth),
            (From::Delegates, To::Delegates),
            (From::Subscription, To::Subscription),
            (From::Dependencies, To::Dependencies),
            (From::Cycle, To::Cycle),
            (From::Spec, To::Spec),
            (From::Contract, To::Contract),
            (From::Authority, To::Authority),
            (From::Inputs, To::Inputs),
            (From::Attempts, To::Attempts),
            (From::LiveDelegates, To::LiveDelegates),
            (From::Restore, To::Restore),
            (From::Read, To::Read),
            (From::Turn, To::Turn),
            (From::HoldKind, To::HoldKind),
            (From::HoldTaken, To::HoldTaken),
            (From::Holds, To::Holds),
            (From::InboxFull, To::InboxFull),
            (From::Notes, To::Notes),
            (From::Revision, To::Revision),
            (From::Question, To::Question),
            (From::Priority, To::Priority),
            (From::Policy, To::Policy),
            (From::Resource, To::Resource),
            (From::Service, To::Service),
        ];
        assert_eq!(cases.len(), 35);
        for (source, expected) in cases {
            let actual = refusal(people::Refusal::Limit { which: source }).expect("representable owner value");
            assert_eq!(actual, client::Refusal::Limit { which: client::Limit::Admission { which: expected } });
            assert!(client::EntranceRefusal::new(actual).is_ok());
        }
    }
}
