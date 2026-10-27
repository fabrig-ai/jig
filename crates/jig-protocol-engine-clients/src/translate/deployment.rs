#![expect(clippy::manual_map, reason = "step code matches optional owner facts without closures")]
//! Whole live document facts (protocol/client.md, 3 and 7).
use super::{chips, owner, pieces, refusals, task, w};
use alloc::boxed::Box;
use jig_domain_accounts as accounts;
use jig_domain_core::documents as d;
use jig_domain_fleet as fleet;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{List, Writer};
pub(super) fn host_kind(value: fleet::HostKind) -> w::OwnerHostKind {
    match value {
        fleet::HostKind::Worker => w::OwnerHostKind::Worker,
        fleet::HostKind::Engine => w::OwnerHostKind::Engine,
    }
}
fn attempt(value: fleet::AttemptState) -> w::OwnerAttemptState {
    match value {
        fleet::AttemptState::Waiting => w::OwnerAttemptState::Waiting,
        fleet::AttemptState::Adopted => w::OwnerAttemptState::Adopted,
        fleet::AttemptState::Claimed => w::OwnerAttemptState::Claimed,
        fleet::AttemptState::Cancelled => w::OwnerAttemptState::Cancelled,
        fleet::AttemptState::Handed => w::OwnerAttemptState::Handed,
        fleet::AttemptState::Acknowledged => w::OwnerAttemptState::Acknowledged,
        fleet::AttemptState::Stray => w::OwnerAttemptState::Stray,
        fleet::AttemptState::Kept => w::OwnerAttemptState::Kept,
        fleet::AttemptState::Fenced => w::OwnerAttemptState::Fenced,
        fleet::AttemptState::Closed => w::OwnerAttemptState::Closed,
    }
}
// The current client wire reports these dimensions without their charter numbers.
fn shortfall(value: fleet::AgentShortfall, limits: &w::Limits) -> Result<w::OwnerAgentShortfall, w::Problem> {
    Ok(match value {
        fleet::AgentShortfall::Model { charter, model } => w::OwnerAgentShortfall::Model(
            w::OwnerAgentShortfallModel::new(limits, w::OwnerAgentShortfallModelParts { charter, model })?,
        ),
        fleet::AgentShortfall::Window { charter, model } => w::OwnerAgentShortfall::Window(
            w::OwnerAgentShortfallWindow::new(limits, w::OwnerAgentShortfallWindowParts { charter, model })?,
        ),
        fleet::AgentShortfall::Output { charter, model } => w::OwnerAgentShortfall::Output(
            w::OwnerAgentShortfallOutput::new(limits, w::OwnerAgentShortfallOutputParts { charter, model })?,
        ),
        fleet::AgentShortfall::Turns { charter: _ } => w::OwnerAgentShortfall::Turns,
        fleet::AgentShortfall::Spend => w::OwnerAgentShortfall::Spend,
        fleet::AgentShortfall::Time { charter: _ } => w::OwnerAgentShortfall::Time,
        fleet::AgentShortfall::Waiting { charter: _ } => w::OwnerAgentShortfall::Waiting,
        fleet::AgentShortfall::Checks { charter: _ } => w::OwnerAgentShortfall::Checks,
        fleet::AgentShortfall::Message => w::OwnerAgentShortfall::Message,
        fleet::AgentShortfall::Inbox => w::OwnerAgentShortfall::Inbox,
        fleet::AgentShortfall::Unacknowledged => w::OwnerAgentShortfall::Unacknowledged,
    })
}
pub(super) fn hello(value: fleet::HelloRefusal, limits: &w::Limits) -> Result<w::OwnerHelloRefusal, w::Problem> {
    Ok(match value {
        fleet::HelloRefusal::Agent { why } => w::OwnerHelloRefusal::Agent(w::OwnerHelloRefusalAgent::new(
            limits,
            w::OwnerHelloRefusalAgentParts { why: shortfall(why, limits)? },
        )?),
        fleet::HelloRefusal::ReservedChannel => w::OwnerHelloRefusal::ReservedChannel,
        fleet::HelloRefusal::Workers { limit } => w::OwnerHelloRefusal::Workers(w::OwnerHelloRefusalWorkers::new(
            limits,
            w::OwnerHelloRefusalWorkersParts { limit },
        )?),
        fleet::HelloRefusal::Position { worker } => w::OwnerHelloRefusal::Position(w::OwnerHelloRefusalPosition::new(
            limits,
            w::OwnerHelloRefusalPositionParts { worker },
        )?),
        fleet::HelloRefusal::StopBound { grace } => w::OwnerHelloRefusal::StopBound(
            w::OwnerHelloRefusalStopBound::new(limits, w::OwnerHelloRefusalStopBoundParts { grace })?,
        ),
    })
}
pub(crate) fn account_place(value: accounts::Place) -> w::OwnerAccountPlace {
    match value {
        accounts::Place::Directory => w::OwnerAccountPlace::Directory,
        accounts::Place::File => w::OwnerAccountPlace::File,
    }
}
pub(crate) fn account_unread(value: accounts::Unread) -> w::OwnerAccountUnread {
    match value {
        accounts::Unread::Unreadable => w::OwnerAccountUnread::Unreadable,
        accounts::Unread::Refused => w::OwnerAccountUnread::Refused,
        accounts::Unread::Failed => w::OwnerAccountUnread::Failed,
        accounts::Unread::Stalled => w::OwnerAccountUnread::Stalled,
        accounts::Unread::TooLarge => w::OwnerAccountUnread::TooLarge,
    }
}
pub(crate) fn account_state(value: d::AccountState, limits: &w::Limits) -> Result<w::OwnerAccountState, w::Problem> {
    Ok(match value {
        d::AccountState::Granted => w::OwnerAccountState::Granted,
        d::AccountState::Asking => w::OwnerAccountState::Asking,
        d::AccountState::SignInAgain => w::OwnerAccountState::SignInAgain,
        d::AccountState::Misconfigured => w::OwnerAccountState::Misconfigured,
        d::AccountState::Cancelled => w::OwnerAccountState::Cancelled,
        d::AccountState::Moving => w::OwnerAccountState::Moving,
        d::AccountState::Closing => w::OwnerAccountState::Closing,
        d::AccountState::Later { until } => w::OwnerAccountState::Later(w::OwnerAccountStateLater::new(
            limits,
            w::OwnerAccountStateLaterParts {
                until: match until {
                    Some(item) => Some(item.as_nanos()),
                    None => None,
                },
            },
        )?),
        d::AccountState::Unloaded { at, why } => w::OwnerAccountState::Unloaded(w::OwnerAccountStateUnloaded::new(
            limits,
            w::OwnerAccountStateUnloadedParts { at: account_place(at), why: account_unread(why) },
        )?),
    })
}
fn person_action(
    value: people::CapacityAction,
    limits: &w::Limits,
) -> Result<w::OwnerPersonCapacityAction, w::Problem> {
    Ok(match value {
        people::CapacityAction::AccountSignIn => w::OwnerPersonCapacityAction::AccountSignIn,
        people::CapacityAction::AccountSignOut => w::OwnerPersonCapacityAction::AccountSignOut,
        people::CapacityAction::AccountChange => w::OwnerPersonCapacityAction::AccountChange,
        people::CapacityAction::Create => w::OwnerPersonCapacityAction::Create,
        people::CapacityAction::Compound => w::OwnerPersonCapacityAction::Compound,
        people::CapacityAction::Service => w::OwnerPersonCapacityAction::Service,
        people::CapacityAction::Adoption => w::OwnerPersonCapacityAction::Adoption,
        people::CapacityAction::Roles => w::OwnerPersonCapacityAction::Roles,
        people::CapacityAction::Policy => w::OwnerPersonCapacityAction::Policy,
        people::CapacityAction::Application { connector } => {
            w::OwnerPersonCapacityAction::Application(w::OwnerPersonCapacityActionApplication::new(
                limits,
                w::OwnerPersonCapacityActionApplicationParts { connector },
            )?)
        }
        people::CapacityAction::Note { name } => w::OwnerPersonCapacityAction::Note(
            w::OwnerPersonCapacityActionNote::new(limits, w::OwnerPersonCapacityActionNoteParts { name })?,
        ),
        people::CapacityAction::Pool { person } => w::OwnerPersonCapacityAction::Pool(
            w::OwnerPersonCapacityActionPool::new(limits, w::OwnerPersonCapacityActionPoolParts { person })?,
        ),
        people::CapacityAction::Escalation { task } => w::OwnerPersonCapacityAction::Escalation(
            w::OwnerPersonCapacityActionEscalation::new(limits, w::OwnerPersonCapacityActionEscalationParts { task })?,
        ),
        people::CapacityAction::Words { task } => w::OwnerPersonCapacityAction::Words(
            w::OwnerPersonCapacityActionWords::new(limits, w::OwnerPersonCapacityActionWordsParts { task })?,
        ),
        people::CapacityAction::Question { task, question } => {
            w::OwnerPersonCapacityAction::Question(w::OwnerPersonCapacityActionQuestion::new(
                limits,
                w::OwnerPersonCapacityActionQuestionParts { task, question },
            )?)
        }
        people::CapacityAction::Amend { task } => w::OwnerPersonCapacityAction::Amend(
            w::OwnerPersonCapacityActionAmend::new(limits, w::OwnerPersonCapacityActionAmendParts { task })?,
        ),
        people::CapacityAction::Move { task } => w::OwnerPersonCapacityAction::MoveTask(
            w::OwnerPersonCapacityActionMove::new(limits, w::OwnerPersonCapacityActionMoveParts { task })?,
        ),
        people::CapacityAction::TakePerson { task } => w::OwnerPersonCapacityAction::TakePerson(
            w::OwnerPersonCapacityActionTakePerson::new(limits, w::OwnerPersonCapacityActionTakePersonParts { task })?,
        ),
        people::CapacityAction::HandBackPerson { task } => {
            w::OwnerPersonCapacityAction::HandBackPerson(w::OwnerPersonCapacityActionHandBackPerson::new(
                limits,
                w::OwnerPersonCapacityActionHandBackPersonParts { task },
            )?)
        }
        people::CapacityAction::AnswerPerson { task } => {
            w::OwnerPersonCapacityAction::AnswerPerson(w::OwnerPersonCapacityActionAnswerPerson::new(
                limits,
                w::OwnerPersonCapacityActionAnswerPersonParts { task },
            )?)
        }
        people::CapacityAction::Stop { task } => w::OwnerPersonCapacityAction::Stop(
            w::OwnerPersonCapacityActionStop::new(limits, w::OwnerPersonCapacityActionStopParts { task })?,
        ),
        people::CapacityAction::Cancel { task } => w::OwnerPersonCapacityAction::Cancel(
            w::OwnerPersonCapacityActionCancel::new(limits, w::OwnerPersonCapacityActionCancelParts { task })?,
        ),
        people::CapacityAction::Lift { hold } => w::OwnerPersonCapacityAction::Lift(
            w::OwnerPersonCapacityActionLift::new(limits, w::OwnerPersonCapacityActionLiftParts { hold })?,
        ),
        people::CapacityAction::Release { task } => w::OwnerPersonCapacityAction::Release(
            w::OwnerPersonCapacityActionRelease::new(limits, w::OwnerPersonCapacityActionReleaseParts { task })?,
        ),
        people::CapacityAction::Proposal { proposer, number } => {
            w::OwnerPersonCapacityAction::Proposal(w::OwnerPersonCapacityActionProposal::new(
                limits,
                w::OwnerPersonCapacityActionProposalParts {
                    proposer: task::proposal(&d::ProposalRef { proposer, number }, limits)?.proposer().clone(),
                    number,
                },
            )?)
        }
        people::CapacityAction::MarkSeen { seen } => {
            w::OwnerPersonCapacityAction::MarkSeen(w::OwnerPersonCapacityActionMarkSeen::new(
                limits,
                w::OwnerPersonCapacityActionMarkSeenParts { seen: seen_record(seen, limits)? },
            )?)
        }
    })
}
fn seen_record(value: people::Seen, limits: &w::Limits) -> Result<w::SeenThrough, w::Problem> {
    Ok(match value {
        people::Seen::Results { through } => {
            w::SeenThrough::Results(w::SeenResults::new(limits, w::SeenResultsParts { through })?)
        }
        people::Seen::Conversation { task, through } => {
            w::SeenThrough::Conversation(w::SeenConversation::new(limits, w::SeenConversationParts { task, through })?)
        }
    })
}
fn person_cause(value: people::CapacityCause, limits: &w::Limits) -> Result<w::OwnerPersonCapacityCause, w::Problem> {
    Ok(match value {
        people::CapacityCause::Requests { limit } => w::OwnerPersonCapacityCause::Requests(
            w::OwnerPersonCapacityCauseRequests::new(limits, w::OwnerPersonCapacityCauseRequestsParts { limit })?,
        ),
        people::CapacityCause::Pending { limit } => w::OwnerPersonCapacityCause::Pending(
            w::OwnerPersonCapacityCausePending::new(limits, w::OwnerPersonCapacityCausePendingParts { limit })?,
        ),
        people::CapacityCause::Waiters { limit } => w::OwnerPersonCapacityCause::Waiters(
            w::OwnerPersonCapacityCauseWaiters::new(limits, w::OwnerPersonCapacityCauseWaitersParts { limit })?,
        ),
        people::CapacityCause::Limit { which } => {
            w::OwnerPersonCapacityCause::Limit(w::OwnerPersonCapacityCauseLimit::new(
                limits,
                w::OwnerPersonCapacityCauseLimitParts { which: refusals::limit(which) },
            )?)
        }
    })
}
fn task_action(value: tasks::CapacityAction, limits: &w::Limits) -> Result<w::OwnerTaskCapacityAction, w::Problem> {
    Ok(match value {
        tasks::CapacityAction::Place { hold } => w::OwnerTaskCapacityAction::Place(
            w::OwnerTaskCapacityActionPlace::new(limits, w::OwnerTaskCapacityActionPlaceParts { hold })?,
        ),
        tasks::CapacityAction::Release => w::OwnerTaskCapacityAction::Release,
        tasks::CapacityAction::Move => w::OwnerTaskCapacityAction::MoveTask,
        tasks::CapacityAction::Batch { first, members } => w::OwnerTaskCapacityAction::Batch(
            w::OwnerTaskCapacityActionBatch::new(limits, w::OwnerTaskCapacityActionBatchParts { first, members })?,
        ),
        tasks::CapacityAction::Proposal { number } => w::OwnerTaskCapacityAction::Proposal(
            w::OwnerTaskCapacityActionProposal::new(limits, w::OwnerTaskCapacityActionProposalParts { number })?,
        ),
        tasks::CapacityAction::PersonProposal { number } => {
            w::OwnerTaskCapacityAction::PersonProposal(w::OwnerTaskCapacityActionPersonProposal::new(
                limits,
                w::OwnerTaskCapacityActionPersonProposalParts { number },
            )?)
        }
        tasks::CapacityAction::Message { task, number } => w::OwnerTaskCapacityAction::Message(
            w::OwnerTaskCapacityActionMessage::new(limits, w::OwnerTaskCapacityActionMessageParts { task, number })?,
        ),
        tasks::CapacityAction::Introduce { left, right } => w::OwnerTaskCapacityAction::Introduce(
            w::OwnerTaskCapacityActionIntroduce::new(limits, w::OwnerTaskCapacityActionIntroduceParts { left, right })?,
        ),
        tasks::CapacityAction::Subscription { number } => {
            w::OwnerTaskCapacityAction::Subscription(w::OwnerTaskCapacityActionSubscription::new(
                limits,
                w::OwnerTaskCapacityActionSubscriptionParts { number },
            )?)
        }
        tasks::CapacityAction::Effect { entry } => w::OwnerTaskCapacityAction::Effect(
            w::OwnerTaskCapacityActionEffect::new(limits, w::OwnerTaskCapacityActionEffectParts { entry })?,
        ),
        tasks::CapacityAction::Claim { attempt } => w::OwnerTaskCapacityAction::Claim(
            w::OwnerTaskCapacityActionClaim::new(limits, w::OwnerTaskCapacityActionClaimParts { attempt })?,
        ),
        tasks::CapacityAction::Source { funder } => {
            w::OwnerTaskCapacityAction::Source(w::OwnerTaskCapacityActionSource::new(
                limits,
                w::OwnerTaskCapacityActionSourceParts { funder: owner::funder(funder, limits)? },
            )?)
        }
    })
}
fn task_cause(value: tasks::CapacityCause, limits: &w::Limits) -> Result<w::OwnerTaskCapacityCause, w::Problem> {
    Ok(match value {
        tasks::CapacityCause::PlacedHolds { limit } => w::OwnerTaskCapacityCause::PlacedHolds(
            w::OwnerTaskCapacityCausePlacedHolds::new(limits, w::OwnerTaskCapacityCausePlacedHoldsParts { limit })?,
        ),
        tasks::CapacityCause::ProjectPlacedHolds { limit } => {
            w::OwnerTaskCapacityCause::ProjectPlacedHolds(w::OwnerTaskCapacityCauseProjectPlacedHolds::new(
                limits,
                w::OwnerTaskCapacityCauseProjectPlacedHoldsParts { limit },
            )?)
        }
        tasks::CapacityCause::PartyPlacedHolds { limit } => {
            w::OwnerTaskCapacityCause::PartyPlacedHolds(w::OwnerTaskCapacityCausePartyPlacedHolds::new(
                limits,
                w::OwnerTaskCapacityCausePartyPlacedHoldsParts { limit },
            )?)
        }
        tasks::CapacityCause::Exemptions { limit } => w::OwnerTaskCapacityCause::Exemptions(
            w::OwnerTaskCapacityCauseExemptions::new(limits, w::OwnerTaskCapacityCauseExemptionsParts { limit })?,
        ),
        tasks::CapacityCause::Tasks { limit } => w::OwnerTaskCapacityCause::Tasks(w::OwnerTaskCapacityCauseTasks::new(
            limits,
            w::OwnerTaskCapacityCauseTasksParts { limit },
        )?),
        tasks::CapacityCause::CreationFlights { limit } => {
            w::OwnerTaskCapacityCause::CreationFlights(w::OwnerTaskCapacityCauseCreationFlights::new(
                limits,
                w::OwnerTaskCapacityCauseCreationFlightsParts { limit },
            )?)
        }
        tasks::CapacityCause::ResourceReports { limit } => {
            w::OwnerTaskCapacityCause::ResourceReports(w::OwnerTaskCapacityCauseResourceReports::new(
                limits,
                w::OwnerTaskCapacityCauseResourceReportsParts { limit },
            )?)
        }
        tasks::CapacityCause::ProjectTasks { limit } => w::OwnerTaskCapacityCause::ProjectTasks(
            w::OwnerTaskCapacityCauseProjectTasks::new(limits, w::OwnerTaskCapacityCauseProjectTasksParts { limit })?,
        ),
        tasks::CapacityCause::Tree { limit } => w::OwnerTaskCapacityCause::Tree(w::OwnerTaskCapacityCauseTree::new(
            limits,
            w::OwnerTaskCapacityCauseTreeParts { limit },
        )?),
        tasks::CapacityCause::Delegates { limit } => w::OwnerTaskCapacityCause::Delegates(
            w::OwnerTaskCapacityCauseDelegates::new(limits, w::OwnerTaskCapacityCauseDelegatesParts { limit })?,
        ),
        tasks::CapacityCause::Holdings { limit } => w::OwnerTaskCapacityCause::Holdings(
            w::OwnerTaskCapacityCauseHoldings::new(limits, w::OwnerTaskCapacityCauseHoldingsParts { limit })?,
        ),
        tasks::CapacityCause::HoldWaiters { limit } => w::OwnerTaskCapacityCause::HoldWaiters(
            w::OwnerTaskCapacityCauseHoldWaiters::new(limits, w::OwnerTaskCapacityCauseHoldWaitersParts { limit })?,
        ),
        tasks::CapacityCause::Proposals { limit } => w::OwnerTaskCapacityCause::Proposals(
            w::OwnerTaskCapacityCauseProposals::new(limits, w::OwnerTaskCapacityCauseProposalsParts { limit })?,
        ),
        tasks::CapacityCause::References { limit } => w::OwnerTaskCapacityCause::References(
            w::OwnerTaskCapacityCauseReferences::new(limits, w::OwnerTaskCapacityCauseReferencesParts { limit })?,
        ),
        tasks::CapacityCause::Subscriptions { limit } => w::OwnerTaskCapacityCause::Subscriptions(
            w::OwnerTaskCapacityCauseSubscriptions::new(limits, w::OwnerTaskCapacityCauseSubscriptionsParts { limit })?,
        ),
        tasks::CapacityCause::Funders { limit } => w::OwnerTaskCapacityCause::Funders(
            w::OwnerTaskCapacityCauseFunders::new(limits, w::OwnerTaskCapacityCauseFundersParts { limit })?,
        ),
        tasks::CapacityCause::Writers { limit } => w::OwnerTaskCapacityCause::Writers(
            w::OwnerTaskCapacityCauseWriters::new(limits, w::OwnerTaskCapacityCauseWritersParts { limit })?,
        ),
        tasks::CapacityCause::Inbox { task, messages, bytes } => w::OwnerTaskCapacityCause::Inbox(
            w::OwnerTaskCapacityCauseInbox::new(limits, w::OwnerTaskCapacityCauseInboxParts { task, messages, bytes })?,
        ),
    })
}
pub fn capacity(value: d::Capacity, limits: &w::Limits) -> Result<w::OwnerCapacity, w::Problem> {
    w::OwnerCapacity::new(limits, w::OwnerCapacityParts { used: value.used, limit: value.limit })
}
fn location(value: d::RunLocation, limits: &w::Limits) -> Result<w::OwnerRunLocation, w::Problem> {
    Ok(match value {
        d::RunLocation::None => w::OwnerRunLocation::None,
        d::RunLocation::Connected { channel, kind } => {
            w::OwnerRunLocation::Connected(w::OwnerRunLocationConnected::new(
                limits,
                w::OwnerRunLocationConnectedParts { channel, kind: host_kind(kind) },
            )?)
        }
        d::RunLocation::Adrift { channel, until, remaining } => {
            w::OwnerRunLocation::Adrift(w::OwnerRunLocationAdrift::new(
                limits,
                w::OwnerRunLocationAdriftParts {
                    channel,
                    until: match until {
                        Some(item) => Some(item.as_nanos()),
                        None => None,
                    },
                    remaining,
                },
            )?)
        }
    })
}
pub fn run(value: &d::HostRun, limits: &w::Limits) -> Result<w::OwnerHostRun, w::Problem> {
    w::OwnerHostRun::new(
        limits,
        w::OwnerHostRunParts {
            task: task::link(&value.task, limits)?,
            attempt: value.attempt,
            kept: value.kept,
            listed: value.listed,
            state: attempt(value.state),
            location: location(value.location, limits)?,
            grace_until: match value.grace_until {
                Some(item) => Some(item.as_nanos()),
                None => None,
            },
        },
    )
}
fn host_rule(value: jig_domain_core::HostRule, limits: &w::Limits) -> Result<w::OwnerHostRule, w::Problem> {
    use jig_domain_core::HostRule;
    Ok(match value {
        HostRule::Open => w::OwnerHostRule::Open,
        HostRule::Person(party) => {
            w::OwnerHostRule::Person(w::OwnerHostPerson::new(limits, w::OwnerHostPersonParts { party })?)
        }
        HostRule::Several => w::OwnerHostRule::Several,
        HostRule::Unknown => w::OwnerHostRule::Unknown,
    })
}
pub fn host(value: &d::Host, limits: &w::Limits) -> Result<w::OwnerHost, w::Problem> {
    let mut runs = List::with_capacity(value.runs.len());
    for row in &value.runs {
        runs.push(run(row, limits)?).expect("supplied rows fit");
    }
    let runs = run_pieces(&runs, limits.owner_host_runs, w::Path::OwnerHostRuns)?;

    w::OwnerHost::new(
        limits,
        w::OwnerHostParts {
            skein_bools: [value.awaiting_logins, value.draining, value.connected, value.cut],
            name: value.name.clone(),
            rule: host_rule(value.rule, limits)?,
            programs: super::programs::logins(
                &value.programs,
                limits,
                limits.owner_host_programs,
                w::Path::OwnerHostPrograms,
            )?,
            channel: value.channel,
            kind: host_kind(value.kind),
            slots: value.slots,
            used: value.used,
            free: value.free,
            grace_until: match value.grace_until {
                Some(item) => Some(item.as_nanos()),
                None => None,
            },
            grace_remaining: value.grace_remaining,
            runs,
        },
    )
}
pub fn account(value: &d::Account, limits: &w::Limits) -> Result<w::OwnerAccount, w::Problem> {
    let mut runs = List::with_capacity(value.runs.len());
    for row in &value.runs {
        runs.push(chips::chip(row, limits)?).expect("supplied rows fit");
    }
    let runs = chip_pieces(&runs, limits.owner_account_runs, w::Path::OwnerAccountRuns)?;
    let mut waiting = List::with_capacity(value.waiting.len());
    for row in &value.waiting {
        waiting.push(chips::chip(row, limits)?).expect("supplied rows fit");
    }
    let waiting = chip_pieces(&waiting, limits.owner_account_waiting, w::Path::OwnerAccountWaiting)?;

    w::OwnerAccount::new(
        limits,
        w::OwnerAccountParts {
            number: value.number,
            place: value.place,
            uses: super::accounts::uses(value.uses.as_ref(), limits)?,
            provider: value.provider,
            provider_name: match &value.provider_name {
                Some(name) => Some(owner::text(name, limits)?),
                None => None,
            },
            manage_usage: value.manage_usage.clone(),
            display: owner::text(&value.display, limits)?,
            holder: super::accounts::holder(value.holder, limits)?,
            sources: super::accounts::source_pieces(
                &value.sources,
                limits,
                limits.owner_account_sources,
                w::Path::OwnerAccountSources,
            )?,
            generation: value.generation,
            state: account_state(value.state, limits)?,
            spent_until: match value.spent_until {
                Some(item) => Some(item.as_nanos()),
                None => None,
            },
            runs,
            waiting,
            skein_bools: [value.plan, value.usable, value.expiring, value.attention, value.cut],
        },
    )
}
pub fn person_refusal(
    value: &people::CapacityRefusal,
    limits: &w::Limits,
) -> Result<w::OwnerPersonCapacityRefusal, w::Problem> {
    w::OwnerPersonCapacityRefusal::new(
        limits,
        w::OwnerPersonCapacityRefusalParts {
            key: w::OwnerRequestKey::new(
                limits,
                w::OwnerRequestKeyParts { person: value.key.person, key: value.key.key },
            )?,
            project: value.project,
            action: person_action(value.action, limits)?,
            at: value.at.as_nanos(),
            cause: person_cause(value.cause, limits)?,
        },
    )
}
pub fn task_refusal(
    value: &tasks::CapacityRefusal,
    limits: &w::Limits,
) -> Result<w::OwnerTaskCapacityRefusal, w::Problem> {
    w::OwnerTaskCapacityRefusal::new(
        limits,
        w::OwnerTaskCapacityRefusalParts {
            request: w::OwnerTaskCapacityRequest::new(
                limits,
                w::OwnerTaskCapacityRequestParts {
                    project: value.request.project,
                    by: owner::requester(value.request.by, limits)?,
                    action: task_action(value.request.action, limits)?,
                    cause: task_cause(value.request.cause, limits)?,
                },
            )?,
            at: value.at.as_nanos(),
        },
    )
}
pub fn document(value: &d::Deployment, limits: &w::Limits) -> Result<w::OwnerDeployment, w::Problem> {
    let mut hosts = List::with_capacity(value.hosts.len());
    for row in &value.hosts {
        hosts.push(host(row, limits)?).expect("supplied rows fit");
    }
    let hosts = host_pieces(&hosts, limits.owner_deployment_hosts, w::Path::OwnerDeploymentHosts)?;
    let mut attempts = List::with_capacity(value.attempts.len());
    for row in &value.attempts {
        attempts.push(run(row, limits)?).expect("supplied rows fit");
    }
    let attempts = run_pieces(&attempts, limits.owner_deployment_attempts, w::Path::OwnerDeploymentAttempts)?;
    let mut accounts = List::with_capacity(value.accounts.len());
    for row in &value.accounts {
        accounts.push(account(row, limits)?).expect("supplied rows fit");
    }
    let accounts = account_pieces(&accounts, limits.owner_deployment_accounts, w::Path::OwnerDeploymentAccounts)?;
    let mut projects = List::with_capacity(value.projects.len());
    for row in &value.projects {
        projects
            .push(w::OwnerProjectCapacity::new(
                limits,
                w::OwnerProjectCapacityParts {
                    project: row.project,
                    tasks: capacity(row.tasks, limits)?,
                    proposals: row.proposals,
                },
            )?)
            .expect("supplied rows fit");
    }
    let mut hello_refusals = List::with_capacity(value.hello_refusals.len());
    for row in &value.hello_refusals {
        hello_refusals
            .push(w::OwnerHelloRefusalLine::new(
                limits,
                w::OwnerHelloRefusalLineParts {
                    channel: row.channel,
                    at: row.at.as_nanos(),
                    slots: row.slots,
                    stop_bound: row.stop_bound,
                    why: hello(row.why, limits)?,
                },
            )?)
            .expect("supplied rows fit");
    }
    let mut person_room_refusals = List::with_capacity(value.person_room_refusals.len());
    for row in &value.person_room_refusals {
        person_room_refusals.push(person_refusal(row, limits)?).expect("supplied rows fit");
    }
    let mut task_room_refusals = List::with_capacity(value.task_room_refusals.len());
    for row in &value.task_room_refusals {
        task_room_refusals.push(task_refusal(row, limits)?).expect("supplied rows fit");
    }
    let mut connectors = List::with_capacity(value.connectors.len());
    for row in &value.connectors {
        connectors.push(*row).expect("supplied rows fit");
    }

    w::OwnerDeployment::new(
        limits,
        w::OwnerDeploymentParts {
            skein_bools: [value.cut, value.connectors_cut, value.providers_cut, value.readiness_cut],
            providers: super::catalogue::offers(&value.providers, limits.owner_deployment_providers, limits)?,
            key_bytes: value.key_bytes,
            readiness: super::catalogue::readiness(&value.readiness, limits.owner_deployment_readiness, limits)?,
            store: w::StoreFill::new(limits, w::StoreFillParts { bytes: value.store.bytes, mark: value.store.mark })?,
            hosts,
            attempts,
            accounts,
            tasks: capacity(value.tasks, limits)?,
            workers: capacity(value.workers, limits)?,
            attempts_capacity: capacity(value.attempts_capacity, limits)?,
            waiting_attempts: value.waiting_attempts,
            proposals: capacity(value.proposals, limits)?,
            projects,
            grace_bound: value.grace_bound,
            hello_refusals,
            hosts_forgotten: value.hosts_forgotten,
            hello_refusals_forgotten: value.hello_refusals_forgotten,
            person_room_refusals,
            person_room_refusals_forgotten: value.person_room_refusals_forgotten,
            person_answer_retention: value.person_answer_retention,
            task_room_refusals,
            task_room_refusals_forgotten: value.task_room_refusals_forgotten,
            connectors,
        },
    )
}
fn chip_pieces(rows: &List<w::Chip>, bound: u32, path: w::Path) -> Result<Box<[u8]>, w::Problem> {
    let mut encoded = List::with_capacity(rows.len());
    for row in rows {
        let mut out = Writer::new(usize::try_from(row.measure()).expect("u32 measure fits usize"));
        match row.encode(&mut out) {
            Ok(()) => (),
            Err(_) => return Err(w::Problem { path, reason: skein_codec::Reason::Bound }),
        }
        encoded.push(out.finish()).expect("supplied rows fit");
    }
    pieces(&encoded, bound, path)
}
fn run_pieces(rows: &List<w::OwnerHostRun>, bound: u32, path: w::Path) -> Result<Box<[u8]>, w::Problem> {
    let mut encoded = List::with_capacity(rows.len());
    for row in rows {
        let mut out = Writer::new(usize::try_from(row.measure()).expect("u32 measure fits usize"));
        match row.encode(&mut out) {
            Ok(()) => (),
            Err(_) => return Err(w::Problem { path, reason: skein_codec::Reason::Bound }),
        }
        encoded.push(out.finish()).expect("supplied rows fit");
    }
    pieces(&encoded, bound, path)
}
fn host_pieces(rows: &List<w::OwnerHost>, bound: u32, path: w::Path) -> Result<Box<[u8]>, w::Problem> {
    let mut encoded = List::with_capacity(rows.len());
    for row in rows {
        let mut out = Writer::new(usize::try_from(row.measure()).expect("u32 measure fits usize"));
        match row.encode(&mut out) {
            Ok(()) => (),
            Err(_) => return Err(w::Problem { path, reason: skein_codec::Reason::Bound }),
        }
        encoded.push(out.finish()).expect("supplied rows fit");
    }
    pieces(&encoded, bound, path)
}
fn account_pieces(rows: &List<w::OwnerAccount>, bound: u32, path: w::Path) -> Result<Box<[u8]>, w::Problem> {
    let mut encoded = List::with_capacity(rows.len());
    for row in rows {
        let mut out = Writer::new(usize::try_from(row.measure()).expect("u32 measure fits usize"));
        match row.encode(&mut out) {
            Ok(()) => (),
            Err(_) => return Err(w::Problem { path, reason: skein_codec::Reason::Bound }),
        }
        encoded.push(out.finish()).expect("supplied rows fit");
    }
    pieces(&encoded, bound, path)
}
