//! Whole deployment owner facts (client/domain/contract.md, 5 and 7.1).
//! Clock options, independent loss, configured order and reserved collections
//! cross unchanged. Connector values remain their application owners' facts.
use super::{chips, owner, refusals, results};
use jig_client::DomainLimits as Limits;
use jig_client_domain::contract as client;
use jig_domain_accounts as accounts;
use jig_domain_core::documents;
use jig_domain_fleet as fleet;
use jig_domain_people as people;
use jig_domain_tasks as tasks;

/// # Errors
/// Refuses any whole reserved collection or nested owner value above bounds.
pub fn document(source: &documents::Deployment, limits: &Limits) -> Result<client::OwnerDeployment, owner::Invalid> {
    Ok(client::OwnerDeployment {
        providers: super::catalogue::offers(&source.providers, limits)?,
        providers_cut: source.providers_cut,
        key_bytes: source.key_bytes,
        readiness: super::catalogue::readiness(&source.readiness, limits)?,
        readiness_cut: source.readiness_cut,
        store: client::StoreFill { bytes: source.store.bytes, mark: source.store.mark },
        hosts: owner::list(&source.hosts, limits, |value| host(value, limits))?,
        attempts: owner::list(&source.attempts, limits, |value| run(value, limits))?,
        accounts: owner::list(&source.accounts, limits, |value| account(value, limits))?,
        tasks: capacity(source.tasks),
        workers: capacity(source.workers),
        attempts_capacity: capacity(source.attempts_capacity),
        waiting_attempts: source.waiting_attempts,
        proposals: capacity(source.proposals),
        projects: owner::list(&source.projects, limits, |value| {
            Ok(client::OwnerProjectCapacity {
                project: value.project,
                tasks: capacity(value.tasks),
                proposals: value.proposals,
            })
        })?,
        grace_bound: source.grace_bound,
        hello_refusals: owner::list(&source.hello_refusals, limits, |value| {
            Ok(client::OwnerHelloRefusalLine {
                channel: value.channel,
                at: value.at,
                slots: value.slots,
                stop_bound: value.stop_bound,
                why: hello(value.why),
            })
        })?,
        hosts_forgotten: source.hosts_forgotten,
        hello_refusals_forgotten: source.hello_refusals_forgotten,
        person_room_refusals: owner::list(&source.person_room_refusals, limits, |value| Ok(person_refusal(value)))?,
        person_room_refusals_forgotten: source.person_room_refusals_forgotten,
        person_answer_retention: source.person_answer_retention,
        task_room_refusals: owner::list(&source.task_room_refusals, limits, |value| Ok(task_refusal(value)))?,
        task_room_refusals_forgotten: source.task_room_refusals_forgotten,
        connectors: owner::list(&source.connectors, limits, |value| Ok(*value))?,
        connectors_cut: source.connectors_cut,
        cut: source.cut,
    })
}

fn capacity(value: documents::Capacity) -> client::OwnerCapacity {
    client::OwnerCapacity { used: value.used, limit: value.limit }
}

fn host(value: &documents::Host, limits: &Limits) -> Result<client::OwnerHost, owner::Invalid> {
    Ok(client::OwnerHost {
        name: value.name.as_deref().map(super::programs::configured_name).transpose()?,
        awaiting_logins: value.awaiting_logins,
        rule: match value.rule {
            jig_domain_core::HostRule::Open => client::HostRule::Open,
            jig_domain_core::HostRule::Person(person) => client::HostRule::Person(person),
            jig_domain_core::HostRule::Several => client::HostRule::Several,
            jig_domain_core::HostRule::Unknown => client::HostRule::Unknown,
        },
        programs: super::programs::logins(&value.programs, limits)?,
        channel: value.channel,
        kind: host_kind(value.kind),
        slots: value.slots,
        used: value.used,
        free: value.free,
        draining: value.draining,
        connected: value.connected,
        grace_until: value.grace_until,
        grace_remaining: value.grace_remaining,
        runs: owner::list(&value.runs, limits, |value| run(value, limits))?,
        cut: value.cut,
    })
}

fn run(value: &documents::HostRun, limits: &Limits) -> Result<client::OwnerHostRun, owner::Invalid> {
    Ok(client::OwnerHostRun {
        task: results::link(&value.task, limits)?,
        attempt: value.attempt,
        kept: value.kept,
        listed: value.listed,
        state: attempt(value.state),
        location: match value.location {
            documents::RunLocation::None => client::OwnerRunLocation::None,
            documents::RunLocation::Connected { channel, kind } => {
                client::OwnerRunLocation::Connected { channel, kind: host_kind(kind) }
            }
            documents::RunLocation::Adrift { channel, until, remaining } => {
                client::OwnerRunLocation::Adrift { channel, until, remaining }
            }
        },
        grace_until: value.grace_until,
    })
}

fn account(value: &documents::Account, limits: &Limits) -> Result<client::OwnerAccount, owner::Invalid> {
    Ok(client::OwnerAccount {
        number: value.number,
        place: value.place,
        uses: super::accounts::uses(value.uses.as_ref()),
        plan: value.plan,
        provider: value.provider,
        provider_name: value.provider_name.as_ref().map(|name| owner::text(name, limits)).transpose()?,
        manage_usage: super::catalogue::manage_usage(value.manage_usage.as_deref())?,
        display: owner::text(&value.display, limits)?,
        holder: super::accounts::holder(value.holder),
        sources: super::accounts::sources(&value.sources, limits)?,
        generation: value.generation,
        state: account_state(value.state),
        usable: value.usable,
        expiring: value.expiring,
        attention: value.attention,
        spent_until: value.spent_until,
        runs: owner::list(&value.runs, limits, |value| chips::chip(value, limits))?,
        waiting: owner::list(&value.waiting, limits, |value| chips::chip(value, limits))?,
        cut: value.cut,
    })
}

fn person_refusal(value: &people::CapacityRefusal) -> client::OwnerPersonCapacityRefusal {
    client::OwnerPersonCapacityRefusal {
        key: client::OwnerRequestKey { person: value.key.person, key: value.key.key },
        project: value.project,
        action: person_action(value.action),
        at: value.at,
        cause: match value.cause {
            people::CapacityCause::Requests { limit } => client::OwnerPersonCapacityCause::Requests { limit },
            people::CapacityCause::Pending { limit } => client::OwnerPersonCapacityCause::Pending { limit },
            people::CapacityCause::Waiters { limit } => client::OwnerPersonCapacityCause::Waiters { limit },
            people::CapacityCause::Limit { which } => {
                client::OwnerPersonCapacityCause::Limit { which: refusals::limit(which) }
            }
        },
    }
}

fn task_refusal(value: &tasks::CapacityRefusal) -> client::OwnerTaskCapacityRefusal {
    client::OwnerTaskCapacityRefusal {
        request: client::OwnerTaskCapacityRequest {
            project: value.request.project,
            by: results::requester(value.request.by),
            action: task_action(value.request.action),
            cause: task_cause(value.request.cause),
        },
        at: value.at,
    }
}

pub(super) fn host_kind(value: fleet::HostKind) -> client::OwnerHostKind {
    match value {
        fleet::HostKind::Worker => client::OwnerHostKind::Worker,
        fleet::HostKind::Engine => client::OwnerHostKind::Engine,
    }
}

fn attempt(value: fleet::AttemptState) -> client::OwnerAttemptState {
    match value {
        fleet::AttemptState::Waiting => client::OwnerAttemptState::Waiting,
        fleet::AttemptState::Adopted => client::OwnerAttemptState::Adopted,
        fleet::AttemptState::Claimed => client::OwnerAttemptState::Claimed,
        fleet::AttemptState::Cancelled => client::OwnerAttemptState::Cancelled,
        fleet::AttemptState::Handed => client::OwnerAttemptState::Handed,
        fleet::AttemptState::Acknowledged => client::OwnerAttemptState::Acknowledged,
        fleet::AttemptState::Stray => client::OwnerAttemptState::Stray,
        fleet::AttemptState::Kept => client::OwnerAttemptState::Kept,
        fleet::AttemptState::Fenced => client::OwnerAttemptState::Fenced,
        fleet::AttemptState::Closed => client::OwnerAttemptState::Closed,
    }
}

pub(super) fn account_state(value: documents::AccountState) -> client::OwnerAccountState {
    match value {
        documents::AccountState::Granted => client::OwnerAccountState::Granted,
        documents::AccountState::Asking => client::OwnerAccountState::Asking,
        documents::AccountState::SignInAgain => client::OwnerAccountState::SignInAgain,
        documents::AccountState::Misconfigured => client::OwnerAccountState::Misconfigured,
        documents::AccountState::Later { until } => client::OwnerAccountState::Later { until },
        documents::AccountState::Unloaded { at, why } => client::OwnerAccountState::Unloaded {
            at: match at {
                accounts::Place::Directory => client::OwnerAccountPlace::Directory,
                accounts::Place::File => client::OwnerAccountPlace::File,
            },
            why: match why {
                accounts::Unread::Unreadable => client::OwnerAccountUnread::Unreadable,
                accounts::Unread::Refused => client::OwnerAccountUnread::Refused,
                accounts::Unread::Failed => client::OwnerAccountUnread::Failed,
                accounts::Unread::Stalled => client::OwnerAccountUnread::Stalled,
                accounts::Unread::TooLarge => client::OwnerAccountUnread::TooLarge,
            },
        },
        documents::AccountState::Cancelled => client::OwnerAccountState::Cancelled,
        documents::AccountState::Moving => client::OwnerAccountState::Moving,
        documents::AccountState::Closing => client::OwnerAccountState::Closing,
    }
}

pub(super) fn hello(value: fleet::HelloRefusal) -> client::OwnerHelloRefusal {
    match value {
        fleet::HelloRefusal::Agent { why } => client::OwnerHelloRefusal::Agent { why: shortfall(why) },
        fleet::HelloRefusal::ReservedChannel => client::OwnerHelloRefusal::ReservedChannel,
        fleet::HelloRefusal::Workers { limit } => client::OwnerHelloRefusal::Workers { limit },
        fleet::HelloRefusal::Position { worker } => client::OwnerHelloRefusal::Position { worker },
        fleet::HelloRefusal::StopBound { grace } => client::OwnerHelloRefusal::StopBound { grace },
    }
}

fn person_action(value: people::CapacityAction) -> client::OwnerPersonCapacityAction {
    match value {
        people::CapacityAction::AccountSignIn => client::OwnerPersonCapacityAction::AccountSignIn,
        people::CapacityAction::AccountSignOut => client::OwnerPersonCapacityAction::AccountSignOut,
        people::CapacityAction::AccountChange => client::OwnerPersonCapacityAction::AccountChange,
        people::CapacityAction::Create => client::OwnerPersonCapacityAction::Create,
        people::CapacityAction::Compound => client::OwnerPersonCapacityAction::Compound,
        people::CapacityAction::Application { connector } => {
            client::OwnerPersonCapacityAction::Application { connector }
        }
        people::CapacityAction::Note { name } => client::OwnerPersonCapacityAction::Note { name },
        people::CapacityAction::Service => client::OwnerPersonCapacityAction::Service,
        people::CapacityAction::Adoption => client::OwnerPersonCapacityAction::Adoption,
        people::CapacityAction::Roles => client::OwnerPersonCapacityAction::Roles,
        people::CapacityAction::Policy => client::OwnerPersonCapacityAction::Policy,
        people::CapacityAction::Pool { person } => client::OwnerPersonCapacityAction::Pool { person },
        people::CapacityAction::Escalation { task } => client::OwnerPersonCapacityAction::Escalation { task },
        people::CapacityAction::Proposal { proposer, number } => client::OwnerPersonCapacityAction::Proposal {
            proposer: match proposer {
                people::Proposer::Task(value) => client::Proposer::Task(value),
                people::Proposer::Party(value) => client::Proposer::Party(value),
            },
            number,
        },
        people::CapacityAction::Words { task } => client::OwnerPersonCapacityAction::Words { task },
        people::CapacityAction::Question { task, question } => {
            client::OwnerPersonCapacityAction::Question { task, question }
        }
        people::CapacityAction::Amend { task } => client::OwnerPersonCapacityAction::Amend { task },
        people::CapacityAction::Move { task } => client::OwnerPersonCapacityAction::Move { task },
        people::CapacityAction::TakePerson { task } => client::OwnerPersonCapacityAction::TakePerson { task },
        people::CapacityAction::HandBackPerson { task } => client::OwnerPersonCapacityAction::HandBackPerson { task },
        people::CapacityAction::AnswerPerson { task } => client::OwnerPersonCapacityAction::AnswerPerson { task },
        people::CapacityAction::Stop { task } => client::OwnerPersonCapacityAction::Stop { task },
        people::CapacityAction::Cancel { task } => client::OwnerPersonCapacityAction::Cancel { task },
        people::CapacityAction::Release { task } => client::OwnerPersonCapacityAction::Release { task },
        people::CapacityAction::MarkSeen { seen } => client::OwnerPersonCapacityAction::MarkSeen {
            seen: match seen {
                people::Seen::Results { through } => client::SeenThrough::Results { through },
                people::Seen::Conversation { task, through } => client::SeenThrough::Conversation { task, through },
            },
        },
    }
}

fn task_action(value: tasks::CapacityAction) -> client::OwnerTaskCapacityAction {
    match value {
        tasks::CapacityAction::Batch { first, members } => client::OwnerTaskCapacityAction::Batch { first, members },
        tasks::CapacityAction::Proposal { number } => client::OwnerTaskCapacityAction::Proposal { number },
        tasks::CapacityAction::PersonProposal { number } => client::OwnerTaskCapacityAction::PersonProposal { number },
        tasks::CapacityAction::Message { task, number } => client::OwnerTaskCapacityAction::Message { task, number },
        tasks::CapacityAction::Source { funder } => {
            client::OwnerTaskCapacityAction::Source { funder: owner::funder(funder) }
        }
        tasks::CapacityAction::Introduce { left, right } => client::OwnerTaskCapacityAction::Introduce { left, right },
        tasks::CapacityAction::Subscription { number } => client::OwnerTaskCapacityAction::Subscription { number },
        tasks::CapacityAction::Effect { entry } => client::OwnerTaskCapacityAction::Effect { entry },
        tasks::CapacityAction::Claim { attempt } => client::OwnerTaskCapacityAction::Claim { attempt },
        tasks::CapacityAction::Place { hold } => client::OwnerTaskCapacityAction::Place { hold },
        tasks::CapacityAction::Release => client::OwnerTaskCapacityAction::Release,
        tasks::CapacityAction::Move => client::OwnerTaskCapacityAction::Move,
    }
}

fn task_cause(value: tasks::CapacityCause) -> client::OwnerTaskCapacityCause {
    match value {
        tasks::CapacityCause::PlacedHolds { limit } => client::OwnerTaskCapacityCause::PlacedHolds { limit },
        tasks::CapacityCause::ProjectPlacedHolds { limit } => {
            client::OwnerTaskCapacityCause::ProjectPlacedHolds { limit }
        }
        tasks::CapacityCause::PartyPlacedHolds { limit } => client::OwnerTaskCapacityCause::PartyPlacedHolds { limit },
        tasks::CapacityCause::Exemptions { limit } => client::OwnerTaskCapacityCause::Exemptions { limit },
        tasks::CapacityCause::Tasks { limit } => client::OwnerTaskCapacityCause::Tasks { limit },
        tasks::CapacityCause::CreationFlights { limit } => client::OwnerTaskCapacityCause::CreationFlights { limit },
        tasks::CapacityCause::ResourceReports { limit } => client::OwnerTaskCapacityCause::ResourceReports { limit },
        tasks::CapacityCause::ProjectTasks { limit } => client::OwnerTaskCapacityCause::ProjectTasks { limit },
        tasks::CapacityCause::Tree { limit } => client::OwnerTaskCapacityCause::Tree { limit },
        tasks::CapacityCause::Delegates { limit } => client::OwnerTaskCapacityCause::Delegates { limit },
        tasks::CapacityCause::Inbox { task, messages, bytes } => {
            client::OwnerTaskCapacityCause::Inbox { task, messages, bytes }
        }
        tasks::CapacityCause::Holdings { limit } => client::OwnerTaskCapacityCause::Holdings { limit },
        tasks::CapacityCause::HoldWaiters { limit } => client::OwnerTaskCapacityCause::HoldWaiters { limit },
        tasks::CapacityCause::Proposals { limit } => client::OwnerTaskCapacityCause::Proposals { limit },
        tasks::CapacityCause::References { limit } => client::OwnerTaskCapacityCause::References { limit },
        tasks::CapacityCause::Subscriptions { limit } => client::OwnerTaskCapacityCause::Subscriptions { limit },
        tasks::CapacityCause::Funders { limit } => client::OwnerTaskCapacityCause::Funders { limit },
        tasks::CapacityCause::Writers { limit } => client::OwnerTaskCapacityCause::Writers { limit },
    }
}

#[cfg(test)]
pub(crate) mod tests;

fn shortfall(value: fleet::AgentShortfall) -> client::OwnerAgentShortfall {
    match value {
        fleet::AgentShortfall::Model { charter, model } => client::OwnerAgentShortfall::Model { charter, model },
        fleet::AgentShortfall::Window { charter, model } => client::OwnerAgentShortfall::Window { charter, model },
        fleet::AgentShortfall::Output { charter, model } => client::OwnerAgentShortfall::Output { charter, model },
        fleet::AgentShortfall::Turns { charter: _ } => client::OwnerAgentShortfall::Turns,
        fleet::AgentShortfall::Spend => client::OwnerAgentShortfall::Spend,
        fleet::AgentShortfall::Time { charter: _ } => client::OwnerAgentShortfall::Time,
        fleet::AgentShortfall::Waiting { charter: _ } => client::OwnerAgentShortfall::Waiting,
        fleet::AgentShortfall::Checks { charter: _ } => client::OwnerAgentShortfall::Checks,
        fleet::AgentShortfall::Message => client::OwnerAgentShortfall::Message,
        fleet::AgentShortfall::Inbox => client::OwnerAgentShortfall::Inbox,
        fleet::AgentShortfall::Unacknowledged => client::OwnerAgentShortfall::Unacknowledged,
    }
}
