//! Sparse entrance facts without invented names, words or bounds (contract.md, 4).
use jig_client_protocol_wire::v19 as w;
use jig_domain_people as p;
use skein_lib::Writer;
#[expect(clippy::too_many_lines, reason = "one total match preserves every people refusal")]
pub(crate) fn refusal(value: p::Refusal, limits: &w::Limits) -> Result<w::Refusal, w::Problem> {
    Ok(match value {
        p::Refusal::NameTaken => w::Refusal::NameTaken,
        p::Refusal::Role { held } => {
            let held = match held {
                Some(value) => Some(role(value, limits)?),
                None => None,
            };
            w::Refusal::Role(w::RefusalRole::new(limits, w::RefusalRoleParts { held })?)
        }
        p::Refusal::Authority { lacking, proposable } => w::Refusal::Authority(w::RefusalAuthority::new(
            limits,
            w::RefusalAuthorityParts {
                lacking: lack(lacking, limits)?,
                proposable: match proposable {
                    Some(p::Accepter::Role(_)) => Some(true),
                    Some(p::Accepter::Nobody) => Some(false),
                    None => None,
                },
            },
        )?),
        p::Refusal::Limit { which, bound } => {
            let which = w::Limit::Admission(w::LimitAdmission::new(
                limits,
                w::LimitAdmissionParts { which: limit(which), bound: limit_bound(bound, limits)? },
            )?);
            w::Refusal::Limit(w::RefusalLimit::new(limits, w::RefusalLimitParts { which })?)
        }
        p::Refusal::Moved { now } => moved(now, limits)?,
        p::Refusal::Ended { how } => {
            w::Refusal::Ended(w::RefusalEnded::new(limits, w::RefusalEndedParts { how: ending(how, limits)? })?)
        }
        p::Refusal::Taken { by } => w::Refusal::Taken(w::RefusalTaken::new(limits, w::RefusalTakenParts { by })?),
        p::Refusal::Stale { newest } => {
            w::Refusal::Stale(w::RefusalStale::new(limits, w::RefusalStaleParts { newest })?)
        }
        p::Refusal::Full { revision, tasks, more } => w::Refusal::Full(w::RefusalFull::new(
            limits,
            w::RefusalFullParts {
                revision,
                tasks: names(&tasks, limits.refusal_full_tasks, w::Path::RefusalFullTasks)?,
                more,
            },
        )?),
        p::Refusal::NotWellFormed { at, why } => w::Refusal::Malformed(w::RefusalMalformed::new(
            limits,
            w::RefusalMalformedParts {
                at,
                why: match why {
                    p::Malformed::Settings { setting, fault } => w::Malformed::Settings(w::MalformedSettings::new(
                        limits,
                        w::MalformedSettingsParts {
                            setting: encode_setting(setting, limits)?,
                            fault: encode_setting_fault(fault),
                        },
                    )?),
                    p::Malformed::SettingsUnchanged => w::Malformed::SettingsUnchanged,
                    p::Malformed::Running => w::Malformed::Running,
                    p::Malformed::Name(rule) => w::Malformed::Name(w::MalformedName::new(
                        limits,
                        w::MalformedNameParts { rule: name_rule(rule) },
                    )?),
                    p::Malformed::AsAllowedTwice => w::Malformed::TwoAsAllowed,
                    p::Malformed::Conversation => w::Malformed::ConversationNotAgent,

                    p::Malformed::EndedBy => w::Malformed::EndedBy,
                    p::Malformed::Cycle => w::Malformed::Cycle,
                    p::Malformed::ProposedAsAllowed => w::Malformed::ProposedAsAllowed,
                },
            },
        )?),
        p::Refusal::Circle { tasks, cut } => w::Refusal::Circle(w::RefusalCircle::new(
            limits,
            w::RefusalCircleParts {
                tasks: names(&tasks, limits.refusal_circle_tasks, w::Path::RefusalCircleTasks)?,
                cut,
            },
        )?),
        p::Refusal::Kept { pending } => w::Refusal::Kept(w::RefusalKept::new(
            limits,
            w::RefusalKeptParts { pending: names(&pending, limits.refusal_kept_pending, w::Path::RefusalKeptPending)? },
        )?),
        p::Refusal::Dependency { dependency, how } => w::Refusal::Dependency(w::RefusalDependency::new(
            limits,
            w::RefusalDependencyParts {
                dependency,
                how: match how {
                    p::DependencyStatus::Done => w::OwnerTaskStatus::Done,
                    p::DependencyStatus::Failed => w::OwnerTaskStatus::Failed,
                    p::DependencyStatus::Cancelled => w::OwnerTaskStatus::Cancelled,
                },
            },
        )?),
        p::Refusal::NotWaiting => w::Refusal::NotWaiting,
        p::Refusal::NoFurther => w::Refusal::NoFurther,
        p::Refusal::Unknown => w::Refusal::Unknown,
        p::Refusal::SignInRedirect { why } => w::Refusal::SignInRedirect(w::RefusalSignInRedirect::new(
            limits,
            w::RefusalSignInRedirectParts {
                why: match why {
                    p::SignInRedirectWhy::NotWaiting => w::SignInRedirectWhy::NotWaiting,
                    p::SignInRedirectWhy::Address => w::SignInRedirectWhy::Address,
                    p::SignInRedirectWhy::State => w::SignInRedirectWhy::State,
                    p::SignInRedirectWhy::TooMany => w::SignInRedirectWhy::TooMany,
                },
            },
        )?),
        p::Refusal::AccountMove { why } => w::Refusal::AccountMove(w::RefusalAccountMove::new(
            limits,
            w::RefusalAccountMoveParts {
                why: match why {
                    p::AccountMoveWhy::NotSignedIn => w::AccountMoveWhy::NotSignedIn,
                    p::AccountMoveWhy::NotOurs => w::AccountMoveWhy::NotOurs,
                    p::AccountMoveWhy::Held => w::AccountMoveWhy::Held,
                    p::AccountMoveWhy::Cancelled => w::AccountMoveWhy::Cancelled,
                    p::AccountMoveWhy::Moving => w::AccountMoveWhy::Moving,
                },
            },
        )?),
        p::Refusal::Account { account, why } => w::Refusal::Account(w::RefusalAccount::new(
            limits,
            w::RefusalAccountParts {
                account,
                why: match why {
                    p::AccountWhy::Unknown => w::AccountWhy::Unknown,
                    p::AccountWhy::Provider => w::AccountWhy::Provider,
                    p::AccountWhy::Uses => w::AccountWhy::Uses,
                    p::AccountWhy::Limited => w::AccountWhy::Limited,
                },
            },
        )?),
        p::Refusal::Provider => w::Refusal::Provider,
        p::Refusal::NotOffered => w::Refusal::NotOffered,
        p::Refusal::NoEscalationHolder => w::Refusal::NoEscalationHolder,
        p::Refusal::KeyConflict => w::Refusal::KeyConflict,
        // Only a made or amended task's refusal names a model, which the answer names by the
        // configured models (answers.rs); an offer's admission is people's own check.
        p::Refusal::Llm { .. } => {
            return Err(w::Problem { path: w::Path::RefusalLlmModel, reason: skein_codec::Reason::Bound });
        }
    })
}
pub(super) fn hold_owner(value: p::HoldOwner, limits: &w::Limits) -> Result<w::HoldOwner, w::Problem> {
    Ok(match value {
        p::HoldOwner::Party { person } => {
            w::HoldOwner::Party(w::HoldOwnerParty::new(limits, w::HoldOwnerPartyParts { person })?)
        }
        p::HoldOwner::Task { task } => {
            w::HoldOwner::Task(w::HoldOwnerTask::new(limits, w::HoldOwnerTaskParts { task })?)
        }
    })
}
fn role(value: p::Role, limits: &w::Limits) -> Result<w::HeldRole, w::Problem> {
    Ok(match value {
        p::Role::Owner => w::HeldRole::Owner,
        p::Role::Maintainer => w::HeldRole::Maintainer,
        p::Role::Member => w::HeldRole::Member,
        p::Role::Observer => w::HeldRole::Observer,
        p::Role::Policy { role } => {
            w::HeldRole::Policy(w::HeldRolePolicy::new(limits, w::HeldRolePolicyParts { role })?)
        }
    })
}
fn lack(value: p::Lack, limits: &w::Limits) -> Result<w::Lacking, w::Problem> {
    Ok(match value {
        p::Lack::Approver => w::Lacking::Approver,
        p::Lack::Funding => w::Lacking::Funding,
        p::Lack::Tool => w::Lacking::Tool,
        p::Lack::Scope => w::Lacking::Scope,
        p::Lack::Models => w::Lacking::Models,
        p::Lack::HoldOwner { hold, owner } => w::Lacking::HoldOwner(w::LackingHoldOwner::new(
            limits,
            w::LackingHoldOwnerParts { hold, owner: hold_owner(owner, limits)? },
        )?),
        p::Lack::Reference => w::Lacking::Reference,
        p::Lack::ResourceAccess => w::Lacking::ResourceAccess,
        p::Lack::ResourceUnavailable => w::Lacking::ResourceUnavailable,
        p::Lack::Shape => w::Lacking::Shape,
        p::Lack::Account => w::Lacking::Account,
        p::Lack::Writer => w::Lacking::Writer,
        p::Lack::Budget { needs, left } => {
            w::Lacking::Budget(w::LackingBudget::new(limits, w::LackingBudgetParts { needed: needs, left })?)
        }
        p::Lack::Grant { kind } => {
            w::Lacking::GrantKind(w::LackingGrantKind::new(limits, w::LackingGrantKindParts { kind })?)
        }
        p::Lack::TaskKind { .. } => {
            w::Lacking::TaskKind(w::LackingTaskKind::new(limits, w::LackingTaskKindParts { executor: None })?)
        }
        p::Lack::Depth { most } => {
            w::Lacking::Depth(w::LackingDepth::new(limits, w::LackingDepthParts { needed: None, allowed: most })?)
        }
        p::Lack::Deadline => w::Lacking::Deadline(w::LackingDeadline::new(
            limits,
            w::LackingDeadlineParts { asked: None, latest: None },
        )?),
        p::Lack::Tasks { left } => {
            w::Lacking::Tasks(w::LackingTasks::new(limits, w::LackingTasksParts { needed: None, left })?)
        }
        p::Lack::Again { required } => {
            w::Lacking::Again(w::LackingAgain::new(limits, w::LackingAgainParts { required })?)
        }
        p::Lack::Required { connector, kind } => {
            w::Lacking::Required(w::LackingRequired::new(limits, w::LackingRequiredParts { connector, kind })?)
        }
        p::Lack::Failed { connector, kind } => {
            w::Lacking::Failed(w::LackingFailed::new(limits, w::LackingFailedParts { connector, kind })?)
        }
        p::Lack::Unguarded { connector, kind } => {
            w::Lacking::Unguarded(w::LackingUnguarded::new(limits, w::LackingUnguardedParts { connector, kind })?)
        }
    })
}
pub(super) fn limit(value: p::LimitKind) -> w::AdmissionLimit {
    match value {
        p::LimitKind::Projects => w::AdmissionLimit::Projects,
        p::LimitKind::Words => w::AdmissionLimit::Words,
        p::LimitKind::Name => w::AdmissionLimit::Name,
        p::LimitKind::Key => w::AdmissionLimit::Key,
        p::LimitKind::Amendment => w::AdmissionLimit::Amendment,
        p::LimitKind::Identity => w::AdmissionLimit::Identity,
        p::LimitKind::Roles => w::AdmissionLimit::Roles,
        p::LimitKind::Arithmetic => w::AdmissionLimit::Arithmetic,
        p::LimitKind::Duplicate => w::AdmissionLimit::Duplicate,
        p::LimitKind::Empty => w::AdmissionLimit::Empty,
        p::LimitKind::Batch => w::AdmissionLimit::Batch,
        p::LimitKind::ProjectLiveTasks => w::AdmissionLimit::ProjectLiveTasks,
        p::LimitKind::Goals => w::AdmissionLimit::Goals,
        p::LimitKind::LiveTasks => w::AdmissionLimit::LiveTasks,
        p::LimitKind::Tree => w::AdmissionLimit::Tree,
        p::LimitKind::Depth => w::AdmissionLimit::Depth,
        p::LimitKind::Delegates => w::AdmissionLimit::Delegates,
        p::LimitKind::Subscription => w::AdmissionLimit::Subscription,
        p::LimitKind::Dependencies => w::AdmissionLimit::Dependencies,
        p::LimitKind::Cycle => w::AdmissionLimit::Cycle,
        p::LimitKind::Grace => w::AdmissionLimit::Grace,
        p::LimitKind::Operations => w::AdmissionLimit::Operations,
        p::LimitKind::Spec => w::AdmissionLimit::Spec,
        p::LimitKind::Contract => w::AdmissionLimit::Contract,
        p::LimitKind::Authority => w::AdmissionLimit::Authority,
        p::LimitKind::Inputs => w::AdmissionLimit::Inputs,
        p::LimitKind::Attempts => w::AdmissionLimit::Attempts,
        p::LimitKind::LiveDelegates => w::AdmissionLimit::LiveDelegates,
        p::LimitKind::Restore => w::AdmissionLimit::Restore,
        p::LimitKind::Read => w::AdmissionLimit::Read,
        p::LimitKind::Turn => w::AdmissionLimit::Turn,
        p::LimitKind::HoldKind => w::AdmissionLimit::HoldKind,
        p::LimitKind::HoldTaken => w::AdmissionLimit::HoldTaken,
        p::LimitKind::Holds => w::AdmissionLimit::Holds,
        p::LimitKind::InboxFull => w::AdmissionLimit::InboxFull,
        p::LimitKind::Notes => w::AdmissionLimit::Notes,
        p::LimitKind::Revision => w::AdmissionLimit::Revision,
        p::LimitKind::Question => w::AdmissionLimit::Question,
        p::LimitKind::Priority => w::AdmissionLimit::Priority,
        p::LimitKind::Policy => w::AdmissionLimit::Policy,
        p::LimitKind::Resource => w::AdmissionLimit::Resource,
        p::LimitKind::Service => w::AdmissionLimit::Service,
        p::LimitKind::PlacedHolds => w::AdmissionLimit::PlacedHolds,
        p::LimitKind::ProjectPlacedHolds => w::AdmissionLimit::ProjectPlacedHolds,
        p::LimitKind::PartyPlacedHolds => w::AdmissionLimit::PartyPlacedHolds,
        p::LimitKind::Exemptions => w::AdmissionLimit::Exemptions,
        p::LimitKind::Accounts => w::AdmissionLimit::Accounts,
        p::LimitKind::HolderAccounts => w::AdmissionLimit::HolderAccounts,
        p::LimitKind::Sources => w::AdmissionLimit::Sources,
        p::LimitKind::SignIns => w::AdmissionLimit::SignIns,
        p::LimitKind::SettingsProposals => w::AdmissionLimit::SettingsProposals,
    }
}
fn moved(value: p::Moved, limits: &w::Limits) -> Result<w::Refusal, w::Problem> {
    let (revision, what) = match value {
        p::Moved::Hold { revision, reason } => {
            let value = hold(reason, limits)?;
            let reason =
                Some(w::CurrentHold::Reason(w::CurrentHoldReason::new(limits, w::CurrentHoldReasonParts { value })?));
            (revision, w::Moved::Hold(w::MovedHold::new(limits, w::MovedHoldParts { reason })?))
        }
        p::Moved::PlacedHold { revision } => {
            (revision, w::Moved::PlacedHold(w::MovedPlacedHold::new(limits, w::MovedPlacedHoldParts { revision })?))
        }
        p::Moved::Spec { revision } => (revision, w::Moved::Spec),
        p::Moved::Person { revision } => (revision, w::Moved::PersonTask),
        p::Moved::Note { revision } => (u64::from(revision), w::Moved::Note),
    };
    let now = w::Revision::new(limits, w::RevisionParts { value: revision })?;
    Ok(w::Refusal::Moved(w::RefusalMoved::new(limits, w::RefusalMovedParts { now, what })?))
}
fn hold(value: p::HoldReason, limits: &w::Limits) -> Result<w::HoldCategory, w::Problem> {
    Ok(match value {
        p::HoldReason::Stalled => w::HoldCategory::Stalled,
        p::HoldReason::Stopped => w::HoldCategory::Stopped,
        p::HoldReason::Drift => w::HoldCategory::Drift,
        p::HoldReason::Effects => w::HoldCategory::Effects,
        p::HoldReason::EffectFailed => w::HoldCategory::EffectFailed,
        p::HoldReason::Budget => w::HoldCategory::Budget,
        p::HoldReason::Deadline => w::HoldCategory::Deadline,
        p::HoldReason::HoldsWaited => w::HoldCategory::HoldsWaited,
        p::HoldReason::ResourceUnavailable => w::HoldCategory::ResourceUnavailable,
        p::HoldReason::ResourceAccess => w::HoldCategory::ResourceAccess,
        p::HoldReason::Procedure => w::HoldCategory::Procedure,
        p::HoldReason::Circle => w::HoldCategory::Circle,
        p::HoldReason::Dependency { task } => w::HoldCategory::Dependency(w::HoldCategoryDependency::new(
            limits,
            w::HoldCategoryDependencyParts { task },
        )?),
        p::HoldReason::Placed { hold } => {
            w::HoldCategory::Placed(w::HoldCategoryPlaced::new(limits, w::HoldCategoryPlacedParts { hold })?)
        }
        p::HoldReason::Uncertain { entry } => {
            w::HoldCategory::Uncertain(w::HoldCategoryUncertain::new(limits, w::HoldCategoryUncertainParts { entry })?)
        }
        p::HoldReason::PoolLost { pool } => {
            w::HoldCategory::PoolLost(w::HoldCategoryPoolLost::new(limits, w::HoldCategoryPoolLostParts { pool })?)
        }
        p::HoldReason::Exceeded(value) => w::HoldCategory::Exceeded(w::Exceeded::new(
            limits,
            w::ExceededParts {
                bound: match value.bound {
                    p::Bound::Reasoning => w::Bound::Reasoning,
                    p::Bound::Metadata => w::Bound::Metadata,
                    p::Bound::Items => w::Bound::Items,
                    p::Bound::Answer => w::Bound::Answer,
                    p::Bound::Tools => w::Bound::Tools,
                    p::Bound::HistoryItems => w::Bound::HistoryItems,
                    p::Bound::History => w::Bound::History,
                    p::Bound::Window => w::Bound::Window,
                    p::Bound::Output => w::Bound::Output,
                },
                value: value.value,
            },
        )?),
        p::HoldReason::Failures { class } => {
            let class = match class {
                p::FailureClass::Transient => w::HoldFailure::Transient,
                p::FailureClass::Permanent => w::HoldFailure::Permanent,
                p::FailureClass::Run => w::HoldFailure::Run,
                p::FailureClass::Agent => w::HoldFailure::Agent,
                p::FailureClass::Lost => w::HoldFailure::Lost,
                p::FailureClass::Invalid => w::HoldFailure::Invalid,
            };
            w::HoldCategory::Failures(w::HoldCategoryFailures::new(limits, w::HoldCategoryFailuresParts { class })?)
        }
    })
}
fn actor(value: p::Party, limits: &w::Limits) -> Result<w::Actor, w::Problem> {
    Ok(match value {
        p::Party::Person(value) => w::Actor::Party(w::ActorParty::new(limits, w::ActorPartyParts { value })?),
        p::Party::Task(value) => w::Actor::Task(w::ActorTask::new(limits, w::ActorTaskParts { value })?),
        p::Party::Deployment { project } => w::Actor::ProjectDeployment(w::ActorProjectDeployment::new(
            limits,
            w::ActorProjectDeploymentParts { project },
        )?),
    })
}
fn ending(value: p::Ending, limits: &w::Limits) -> Result<w::Ending, w::Problem> {
    let value = match value {
        p::Ending::Done => w::EndingFields::Done,
        p::Ending::Failed => {
            w::EndingFields::Failed(w::EndingFailedFields::new(limits, w::EndingFailedFieldsParts { reason: None })?)
        }
        p::Ending::Cancelled { by, at } => w::EndingFields::Cancelled(w::EndingCancelledFields::new(
            limits,
            w::EndingCancelledFieldsParts { by: actor(by, limits)?, reason: None, at: Some(at.as_nanos()) },
        )?),
        p::Ending::Decided { by, at } => w::EndingFields::Decided(w::EndingDecidedFields::new(
            limits,
            w::EndingDecidedFieldsParts { by, party: None, how: None, at: at.as_nanos() },
        )?),
        p::Ending::Deleted => w::EndingFields::Deleted,
    };
    let row = w::Fact::Ending(w::EndingRow::new(limits, w::EndingRowParts { parent: None, value })?);
    let size = row
        .measure()
        .checked_add(4)
        .ok_or(w::Problem { path: w::Path::EndingRows, reason: skein_codec::Reason::Bound })?;
    if size > limits.ending_rows {
        return Err(w::Problem { path: w::Path::EndingRows, reason: skein_codec::Reason::Bound });
    }
    let mut rows = Writer::new(usize::try_from(size).expect("wire size fits"));
    rows.put(&row.measure().to_be_bytes()).expect("measured piece length");
    row.encode(&mut rows).expect("measured ending row");
    w::Ending::new(limits, w::EndingParts { rows: rows.finish() })
}

fn names(values: &[u64], bound: u32, path: w::Path) -> Result<skein_lib::List<u64>, w::Problem> {
    let mut rows = skein_lib::List::with_capacity(bound);
    for number in values {
        if rows.push(*number).is_err() {
            return Err(w::Problem { path, reason: skein_codec::Reason::Bound });
        }
    }
    Ok(rows)
}

fn name_rule(rule: p::NameRule) -> w::NameRule {
    match rule {
        p::NameRule::Empty => w::NameRule::Empty,
        p::NameRule::Long => w::NameRule::Long,
        p::NameRule::Control => w::NameRule::Control,
        p::NameRule::Space => w::NameRule::Space,
        p::NameRule::Digits => w::NameRule::Digits,
    }
}

fn encode_setting(value: p::Setting, limits: &w::Limits) -> Result<w::Setting, w::Problem> {
    Ok(match value {
        p::Setting::Charter(number) => {
            w::Setting::Charter(w::SettingNumber::new(limits, w::SettingNumberParts { number })?)
        }
        p::Setting::Template(number) => {
            w::Setting::Template(w::SettingNumber::new(limits, w::SettingNumberParts { number })?)
        }
        p::Setting::Policy(number) => {
            w::Setting::Policy(w::SettingNumber::new(limits, w::SettingNumberParts { number })?)
        }
        p::Setting::Rules => w::Setting::Rules,
    })
}
fn encode_setting_fault(value: p::SettingFault) -> w::SettingFault {
    match value {
        p::SettingFault::Shape => w::SettingFault::Shape,
        p::SettingFault::Count => w::SettingFault::Count,
        p::SettingFault::Reference => w::SettingFault::Reference,
        p::SettingFault::Rules => w::SettingFault::Rules,
        p::SettingFault::Budget => w::SettingFault::Budget,
        p::SettingFault::Grace => w::SettingFault::Grace,
        p::SettingFault::Owner => w::SettingFault::Owner,
    }
}

pub fn limit_bound(value: Option<p::LimitBound>, limits: &w::Limits) -> Result<Option<w::LimitBound>, w::Problem> {
    let Some(value) = value else {
        return Ok(None);
    };
    Ok(Some(w::LimitBound::new(
        limits,
        w::LimitBoundParts {
            value: value.value,
            key: encode_limit_key(value.key),
            unit: match value.unit {
                p::LimitUnit::Bytes => w::LimitUnit::Bytes,
                p::LimitUnit::Items => w::LimitUnit::Items,
                p::LimitUnit::Milliseconds => w::LimitUnit::Milliseconds,
            },
        },
    )?))
}

fn encode_limit_key(key: Option<p::LimitKey>) -> Option<w::LimitKey> {
    let key = key?;
    Some(match key {
        p::LimitKey::PeopleWords => w::LimitKey::PeopleWords,
        p::LimitKey::PeopleAmendmentBytes => w::LimitKey::PeopleAmendmentBytes,
        p::LimitKey::PeopleHoldings => w::LimitKey::PeopleHoldings,
        p::LimitKey::PeopleIdentityBytes => w::LimitKey::PeopleIdentityBytes,
        p::LimitKey::PeopleBatch => w::LimitKey::PeopleBatch,
        p::LimitKey::PeopleOperations => w::LimitKey::PeopleOperations,
        p::LimitKey::PeopleProjects => w::LimitKey::PeopleProjects,
        p::LimitKey::TasksTasks => w::LimitKey::TasksTasks,
        p::LimitKey::TasksProjectTasks => w::LimitKey::TasksProjectTasks,
        p::LimitKey::TasksProjectGoals => w::LimitKey::TasksProjectGoals,
        p::LimitKey::TasksTreeTasks => w::LimitKey::TasksTreeTasks,
        p::LimitKey::TasksDepth => w::LimitKey::TasksDepth,
        p::LimitKey::TasksDelegates => w::LimitKey::TasksDelegates,
        p::LimitKey::TasksSubscriptions => w::LimitKey::TasksSubscriptions,
        p::LimitKey::TasksDependencies => w::LimitKey::TasksDependencies,
        p::LimitKey::TasksSpecBytes => w::LimitKey::TasksSpecBytes,
        p::LimitKey::TasksParameters => w::LimitKey::TasksParameters,
        p::LimitKey::TasksContractChoices => w::LimitKey::TasksContractChoices,
        p::LimitKey::TasksResultBytes => w::LimitKey::TasksResultBytes,
        p::LimitKey::TasksInputs => w::LimitKey::TasksInputs,
        p::LimitKey::TasksHoldings => w::LimitKey::TasksHoldings,
        p::LimitKey::TasksHoldWaiters => w::LimitKey::TasksHoldWaiters,
        p::LimitKey::TasksInboxMessages => w::LimitKey::TasksInboxMessages,
        p::LimitKey::TasksInboxBytes => w::LimitKey::TasksInboxBytes,
        p::LimitKey::TasksMessageBytes => w::LimitKey::TasksMessageBytes,
        p::LimitKey::TasksGrace => w::LimitKey::TasksGrace,
        p::LimitKey::TasksPlacedHolds => w::LimitKey::TasksPlacedHolds,
        p::LimitKey::TasksProjectPlacedHolds => w::LimitKey::TasksProjectPlacedHolds,
        p::LimitKey::TasksPartyPlacedHolds => w::LimitKey::TasksPartyPlacedHolds,
        p::LimitKey::TasksExemptions => w::LimitKey::TasksExemptions,
        p::LimitKey::TasksAuthorityGrants => w::LimitKey::TasksAuthorityGrants,
        p::LimitKey::TasksAuthorityBytes => w::LimitKey::TasksAuthorityBytes,
        p::LimitKey::TasksAuthoritySegments => w::LimitKey::TasksAuthoritySegments,
        p::LimitKey::CoreTemplates => w::LimitKey::CoreTemplates,
        p::LimitKey::CorePolicyBytes => w::LimitKey::CorePolicyBytes,
        p::LimitKey::CoreSettingsProposals => w::LimitKey::CoreSettingsProposals,
        p::LimitKey::NotesScopes => w::LimitKey::NotesScopes,
        p::LimitKey::NotesEntriesPerScope => w::LimitKey::NotesEntriesPerScope,
        p::LimitKey::NotesPatternBytes => w::LimitKey::NotesPatternBytes,
        p::LimitKey::NotesDescriptionBytes => w::LimitKey::NotesDescriptionBytes,
        p::LimitKey::NotesBodyBytes => w::LimitKey::NotesBodyBytes,
        p::LimitKey::NotesReferences => w::LimitKey::NotesReferences,
        p::LimitKey::AccountsAccounts => w::LimitKey::AccountsAccounts,
        p::LimitKey::AccountsHolderAccounts => w::LimitKey::AccountsHolderAccounts,
        p::LimitKey::AccountsSources => w::LimitKey::AccountsSources,
        p::LimitKey::AccountsSignIns => w::LimitKey::AccountsSignIns,
        p::LimitKey::AccountsProgramHosts => w::LimitKey::AccountsProgramHosts,
        p::LimitKey::TasksBatch => w::LimitKey::TasksBatch,
        p::LimitKey::CoreEscalationReasonBytes => w::LimitKey::CoreEscalationReasonBytes,
        p::LimitKey::AccountsProgramNames => w::LimitKey::AccountsProgramNames,
        p::LimitKey::AccountsCredentials => w::LimitKey::AccountsCredentials,
        p::LimitKey::TasksReferences => w::LimitKey::TasksReferences,
        p::LimitKey::TasksHoldSegments => w::LimitKey::TasksHoldSegments,
        p::LimitKey::TasksHoldBytes => w::LimitKey::TasksHoldBytes,
        p::LimitKey::TasksFunders => w::LimitKey::TasksFunders,
        p::LimitKey::TasksExecutorKinds => w::LimitKey::TasksExecutorKinds,
        p::LimitKey::TasksResourceReports => w::LimitKey::TasksResourceReports,
        p::LimitKey::PeoplePending => w::LimitKey::PeoplePending,
        p::LimitKey::AuthorityProjects => w::LimitKey::AuthorityProjects,
        p::LimitKey::AuthorityGrants => w::LimitKey::AuthorityGrants,
        p::LimitKey::AuthoritySegments => w::LimitKey::AuthoritySegments,
        p::LimitKey::AuthoritySegmentBytes => w::LimitKey::AuthoritySegmentBytes,
    })
}
