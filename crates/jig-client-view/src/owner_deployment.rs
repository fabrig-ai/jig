//! Deployment facts and bounded live diagnostics (pages.md, 7; view.md, 7).
//! Supplied clocks, loss counters and identities stay independent.
mod accounts;
mod hello;
pub(crate) use hello::refusal as hello_refusal;
mod programs;
use crate::owner_deployment_words::{AccountPhrase as A, DeploymentPhrase as P};
use crate::words::Words;
use crate::{JigBindings, Owner};
use skein_ui::phrases::Hole;
use skein_ui::{Builder, Key, Role, Run};

use crate::{Borrowed, Limits};
use hello::hello;
use jig_client::contract::{
    AdmissionLimit, Chip, ChipPhase, Funder, OwnerAccountPlace, OwnerAccountState, OwnerAccountUnread,
    OwnerAttemptState, OwnerCapacity, OwnerChipPhase, OwnerDeployment, OwnerHelloRefusal, OwnerHelloRefusalLine,
    OwnerHostKind, OwnerHostRun, OwnerPersonCapacityAction, OwnerPersonCapacityCause, OwnerRunLocation,
    OwnerTaskCapacityAction, OwnerTaskCapacityCause, OwnerTaskLink, Proposer, SeenThrough,
};
use skein_lib::{Duration, Wall};
pub(crate) fn text(b: &mut Builder, words: &Words, phrase: P, holes: &[Hole]) {
    let saying = words.book().say(words.language(), phrase.id(), holes).expect("deployment sayings checked");
    b.text(Run::Plain, &saying.text);
}
pub(crate) fn field_number(b: &mut Builder, words: &Words, phrase: P, value: u64) {
    text(b, words, phrase, &[Hole::Number(value)]);
}
fn region(b: &mut Builder, words: &Words, phrase: P, holes: &[Hole], key: Option<Key>) {
    match key {
        Some(key) => {
            b.open(Role::Item);
            b.key(key);
        }
        None => b.open(Role::Region),
    }
    let saying = words.book().say(words.language(), phrase.id(), holes).expect("deployment region name checked");
    b.name(&saying.text);
}
fn list(b: &mut Builder, words: &Words, phrase: P) {
    b.open(Role::List);
    let saying = words.book().say(words.language(), phrase.id(), &[]).expect("deployment list name checked");
    b.name(&saying.text);
}
fn wall(b: &mut Builder, words: &Words, phrase: P, absent: P, value: Option<Wall>) {
    match value {
        Some(value) => text(b, words, phrase, &[Hole::Wall(words.adjusted_wall(value))]),
        None => text(b, words, absent, &[]),
    }
}
fn duration(b: &mut Builder, words: &Words, phrase: P, value: Duration) {
    text(b, words, phrase, &[Hole::Span(value)]);
}
fn optional_duration(b: &mut Builder, words: &Words, phrase: P, value: Option<Duration>) {
    match value {
        Some(value) => duration(b, words, phrase, value),
        None => text(b, words, P::UnknownDuration, &[]),
    }
}
fn project(b: &mut Builder, words: &Words, value: Option<u32>) {
    match value {
        Some(value) => field_number(b, words, P::Project, u64::from(value)),
        None => text(b, words, P::UnknownProject, &[]),
    }
}
fn host_kind(b: &mut Builder, words: &Words, value: OwnerHostKind) {
    let phrase = match value {
        OwnerHostKind::Worker => P::HostKindWorker,
        OwnerHostKind::Engine => P::HostKindEngine,
    };
    text(b, words, phrase, &[]);
}
fn attempt_state(b: &mut Builder, words: &Words, value: OwnerAttemptState) {
    let phrase = match value {
        OwnerAttemptState::Waiting => P::AttemptStateWaiting,
        OwnerAttemptState::Adopted => P::AttemptStateAdopted,
        OwnerAttemptState::Claimed => P::AttemptStateClaimed,
        OwnerAttemptState::Cancelled => P::AttemptStateCancelled,
        OwnerAttemptState::Handed => P::AttemptStateHanded,
        OwnerAttemptState::Acknowledged => P::AttemptStateAcknowledged,
        OwnerAttemptState::Stray => P::AttemptStateStray,
        OwnerAttemptState::Kept => P::AttemptStateKept,
        OwnerAttemptState::Fenced => P::AttemptStateFenced,
        OwnerAttemptState::Closed => P::AttemptStateClosed,
    };
    text(b, words, phrase, &[]);
}
fn account_state(b: &mut Builder, words: &Words, value: OwnerAccountState) {
    let phrase = match value {
        OwnerAccountState::Granted => P::AccountStateGranted,
        OwnerAccountState::Asking => P::AccountStateAsking,
        OwnerAccountState::SignInAgain => P::AccountStateSignInAgain,
        OwnerAccountState::Misconfigured => P::AccountStateMisconfigured,
        OwnerAccountState::Later { until } => {
            match until {
                Some(until) => text(b, words, P::AccountDetail(A::Later), &[Hole::Wall(words.adjusted_wall(until))]),
                None => text(b, words, P::AccountDetail(A::LaterUnknown), &[]),
            }
            return;
        }
        OwnerAccountState::Unloaded { at, why } => {
            text(b, words, P::AccountDetail(A::Unloaded), &[]);
            let place = match at {
                OwnerAccountPlace::Directory => A::Directory,
                OwnerAccountPlace::File => A::File,
            };
            text(b, words, P::AccountDetail(place), &[]);
            let why = match why {
                OwnerAccountUnread::Unreadable => A::Unreadable,
                OwnerAccountUnread::Refused => A::Refused,
                OwnerAccountUnread::Failed => A::Failed,
                OwnerAccountUnread::Stalled => A::Stalled,
                OwnerAccountUnread::TooLarge => A::TooLarge,
            };
            text(b, words, P::AccountDetail(why), &[]);
            return;
        }
        OwnerAccountState::Cancelled => P::AccountDetail(A::Cancelled),
        OwnerAccountState::Closing => P::AccountStateClosing,
        OwnerAccountState::Moving => P::AccountStateMoving,
    };
    text(b, words, phrase, &[]);
}
fn placement(b: &mut Builder, words: &Words, value: OwnerRunLocation) {
    match value {
        OwnerRunLocation::None => text(b, words, P::RunLocationNone, &[]),
        OwnerRunLocation::Connected { channel, kind } => {
            text(b, words, P::RunLocationConnected, &[]);
            field_number(b, words, P::Channel, channel);
            host_kind(b, words, kind);
        }
        OwnerRunLocation::Adrift { channel, until, remaining } => {
            text(b, words, P::RunLocationAdrift, &[]);
            match channel {
                Some(channel) => field_number(b, words, P::Channel, channel),
                None => text(b, words, P::UnknownChannel, &[]),
            }
            wall(b, words, P::GraceUntil, P::UnknownClock, until);
            optional_duration(b, words, P::GraceRemaining, remaining);
        }
    }
}
fn run(b: &mut Builder, client: &Borrowed<'_>, words: &Words, value: &OwnerHostRun, bindings: &mut JigBindings) {
    region(
        b,
        words,
        P::Attempt,
        &[Hole::Number(value.attempt)],
        Some(Key { owner: Owner::Jig.opaque(), kind: 96, a: value.task.number(), b: value.attempt }),
    );
    field_number(b, words, P::Attempt, value.attempt);
    crate::owner_result::link(b, client, &value.task, words, bindings);
    field_number(b, words, P::Kept, u64::from(value.kept));
    text(b, words, if value.listed { P::Listed } else { P::NotListed }, &[]);
    attempt_state(b, words, value.state);
    placement(b, words, value.location);
    wall(b, words, P::GraceUntil, P::UnknownClock, value.grace_until);
    b.close();
}
fn capacity(b: &mut Builder, words: &Words, phrase: P, value: &OwnerCapacity) {
    region(b, words, phrase, &[], None);
    field_number(b, words, P::Used, u64::from(value.used));
    field_number(b, words, P::Limit, u64::from(value.limit));
    b.close();
}
fn show_proposer(b: &mut Builder, words: &Words, value: Proposer) {
    match value {
        Proposer::Task(value) => field_number(b, words, P::TaskProposer, value),
        Proposer::Party(value) => field_number(b, words, P::PartyProposer, value),
    }
}
fn show_seen(b: &mut Builder, words: &Words, value: SeenThrough) {
    match value {
        SeenThrough::Results { through } => field_number(b, words, P::Through, through),
        SeenThrough::Conversation { task: chat, through } => {
            field_number(b, words, P::Chat, chat);
            field_number(b, words, P::Through, through);
        }
    }
}
fn show_funder(b: &mut Builder, words: &Words, value: Funder) {
    match value {
        Funder::Task(task) => {
            text(b, words, P::FunderTask, &[]);
            field_number(b, words, P::Task, task);
        }
        Funder::Pool { project, person, period } => {
            text(b, words, P::FunderPool, &[]);
            field_number(b, words, P::Project, u64::from(project));
            field_number(b, words, P::Person, person);
            field_number(b, words, P::Period, period);
        }
        Funder::Recurring { project, task, period } => {
            text(b, words, P::FunderRecurring, &[]);
            field_number(b, words, P::Project, u64::from(project));
            field_number(b, words, P::Task, task);
            field_number(b, words, P::Period, period);
        }
        Funder::Period { project, period } => {
            text(b, words, P::FunderPeriod, &[]);
            field_number(b, words, P::Project, u64::from(project));
            field_number(b, words, P::Period, period);
        }
    }
}
fn person_action(b: &mut Builder, words: &Words, value: OwnerPersonCapacityAction) {
    match value {
        OwnerPersonCapacityAction::AccountSignIn => text(b, words, P::PersonCapacityActionAccountSignIn, &[]),
        OwnerPersonCapacityAction::AccountSignOut => text(b, words, P::PersonCapacityActionAccountSignOut, &[]),
        OwnerPersonCapacityAction::AccountChange => text(b, words, P::PersonCapacityActionAccountChange, &[]),
        OwnerPersonCapacityAction::Create => text(b, words, P::PersonCapacityActionCreate, &[]),
        OwnerPersonCapacityAction::Compound => text(b, words, P::PersonCapacityActionCompound, &[]),
        OwnerPersonCapacityAction::Application { connector } => {
            text(b, words, P::PersonCapacityActionApplication, &[]);
            field_number(b, words, P::Connector, u64::from(connector));
        }
        OwnerPersonCapacityAction::Note { name } => {
            text(b, words, P::PersonCapacityActionNote, &[]);
            field_number(b, words, P::Name, name);
        }
        OwnerPersonCapacityAction::Service => {
            text(b, words, P::PersonCapacityActionService, &[]);
        }
        OwnerPersonCapacityAction::Adoption => {
            text(b, words, P::PersonCapacityActionAdoption, &[]);
        }
        OwnerPersonCapacityAction::Roles => {
            text(b, words, P::PersonCapacityActionRoles, &[]);
        }
        OwnerPersonCapacityAction::Policy => {
            text(b, words, P::PersonCapacityActionPolicy, &[]);
        }
        OwnerPersonCapacityAction::Pool { person } => {
            text(b, words, P::PersonCapacityActionPool, &[]);
            field_number(b, words, P::Person, person);
        }
        OwnerPersonCapacityAction::Escalation { task } => {
            text(b, words, P::PersonCapacityActionEscalation, &[]);
            field_number(b, words, P::Task, task);
        }
        OwnerPersonCapacityAction::Proposal { proposer, number } => {
            text(b, words, P::PersonCapacityActionProposal, &[]);
            show_proposer(b, words, proposer);
            field_number(b, words, P::Number, number);
        }
        OwnerPersonCapacityAction::Words { task } => {
            text(b, words, P::PersonCapacityActionWords, &[]);
            field_number(b, words, P::Task, task);
        }
        OwnerPersonCapacityAction::Question { task, question } => {
            text(b, words, P::PersonCapacityActionQuestion, &[]);
            field_number(b, words, P::Task, task);
            field_number(b, words, P::Question, question);
        }
        OwnerPersonCapacityAction::Amend { task } => {
            text(b, words, P::PersonCapacityActionAmend, &[]);
            field_number(b, words, P::Task, task);
        }
        OwnerPersonCapacityAction::Move { task } => {
            text(b, words, P::PersonCapacityActionMove, &[]);
            field_number(b, words, P::Task, task);
        }
        OwnerPersonCapacityAction::TakePerson { task } => {
            text(b, words, P::PersonCapacityActionTakePerson, &[]);
            field_number(b, words, P::Task, task);
        }
        OwnerPersonCapacityAction::HandBackPerson { task } => {
            text(b, words, P::PersonCapacityActionHandBackPerson, &[]);
            field_number(b, words, P::Task, task);
        }
        OwnerPersonCapacityAction::AnswerPerson { task } => {
            text(b, words, P::PersonCapacityActionAnswerPerson, &[]);
            field_number(b, words, P::Task, task);
        }
        OwnerPersonCapacityAction::Stop { task } => {
            text(b, words, P::PersonCapacityActionStop, &[]);
            field_number(b, words, P::Task, task);
        }
        OwnerPersonCapacityAction::Cancel { task } => {
            text(b, words, P::PersonCapacityActionCancel, &[]);
            field_number(b, words, P::Task, task);
        }
        OwnerPersonCapacityAction::Release { task } => {
            text(b, words, P::PersonCapacityActionRelease, &[]);
            field_number(b, words, P::Task, task);
        }
        OwnerPersonCapacityAction::MarkSeen { seen } => {
            text(b, words, P::PersonCapacityActionMarkSeen, &[]);
            show_seen(b, words, seen);
        }
    }
}
fn person_cause(b: &mut Builder, words: &Words, value: OwnerPersonCapacityCause) {
    match value {
        OwnerPersonCapacityCause::Requests { limit } => {
            text(b, words, P::PersonCapacityCauseRequests, &[]);
            field_number(b, words, P::Limit, u64::from(limit));
        }
        OwnerPersonCapacityCause::Pending { limit } => {
            text(b, words, P::PersonCapacityCausePending, &[]);
            field_number(b, words, P::Limit, u64::from(limit));
        }
        OwnerPersonCapacityCause::Waiters { limit } => {
            text(b, words, P::PersonCapacityCauseWaiters, &[]);
            field_number(b, words, P::Limit, u64::from(limit));
        }
        OwnerPersonCapacityCause::Limit { which } => {
            text(b, words, P::PersonCapacityCauseLimit, &[]);
            admission(b, words, which);
        }
    }
}
fn task_action(b: &mut Builder, words: &Words, value: OwnerTaskCapacityAction) {
    match value {
        OwnerTaskCapacityAction::Batch { first, members } => {
            text(b, words, P::TaskCapacityActionBatch, &[]);
            match first {
                Some(value) => field_number(b, words, P::First, value),
                None => text(b, words, P::UnknownFirst, &[]),
            }
            field_number(b, words, P::Members, u64::from(members));
        }
        OwnerTaskCapacityAction::Proposal { number } => {
            text(b, words, P::TaskCapacityActionProposal, &[]);
            field_number(b, words, P::Number, number);
        }
        OwnerTaskCapacityAction::PersonProposal { number } => {
            text(b, words, P::TaskCapacityActionPersonProposal, &[]);
            field_number(b, words, P::Number, number);
        }
        OwnerTaskCapacityAction::Message { task, number } => {
            text(b, words, P::TaskCapacityActionMessage, &[]);
            field_number(b, words, P::Task, task);
            field_number(b, words, P::Number, number);
        }
        OwnerTaskCapacityAction::Source { funder } => {
            text(b, words, P::TaskCapacityActionSource, &[]);
            show_funder(b, words, funder);
        }
        OwnerTaskCapacityAction::Introduce { left, right } => {
            text(b, words, P::TaskCapacityActionIntroduce, &[]);
            field_number(b, words, P::Left, left);
            field_number(b, words, P::Right, right);
        }
        OwnerTaskCapacityAction::Subscription { number } => {
            text(b, words, P::TaskCapacityActionSubscription, &[]);
            field_number(b, words, P::Number, number);
        }
        OwnerTaskCapacityAction::Effect { entry } => {
            text(b, words, P::TaskCapacityActionEffect, &[]);
            field_number(b, words, P::Entry, entry);
        }
        OwnerTaskCapacityAction::Claim { attempt } => {
            text(b, words, P::TaskCapacityActionClaim, &[]);
            field_number(b, words, P::Attempt, attempt);
        }
        OwnerTaskCapacityAction::Release => {
            text(b, words, P::TaskCapacityActionRelease, &[]);
        }
        OwnerTaskCapacityAction::Move => {
            text(b, words, P::TaskCapacityActionMove, &[]);
        }
    }
}
fn task_cause(b: &mut Builder, words: &Words, value: OwnerTaskCapacityCause) {
    match value {
        OwnerTaskCapacityCause::Tasks { limit } => {
            text(b, words, P::TaskCapacityCauseTasks, &[]);
            field_number(b, words, P::Limit, u64::from(limit));
        }
        OwnerTaskCapacityCause::CreationFlights { limit } => {
            text(b, words, P::TaskCapacityCauseCreationFlights, &[]);
            field_number(b, words, P::Limit, u64::from(limit));
        }
        OwnerTaskCapacityCause::ResourceReports { limit } => {
            text(b, words, P::TaskCapacityCauseResourceReports, &[]);
            field_number(b, words, P::Limit, u64::from(limit));
        }
        OwnerTaskCapacityCause::ProjectTasks { limit } => {
            text(b, words, P::TaskCapacityCauseProjectTasks, &[]);
            field_number(b, words, P::Limit, u64::from(limit));
        }
        OwnerTaskCapacityCause::Tree { limit } => {
            text(b, words, P::TaskCapacityCauseTree, &[]);
            field_number(b, words, P::Limit, u64::from(limit));
        }
        OwnerTaskCapacityCause::Delegates { limit } => {
            text(b, words, P::TaskCapacityCauseDelegates, &[]);
            field_number(b, words, P::Limit, u64::from(limit));
        }
        OwnerTaskCapacityCause::Inbox { task, messages, bytes } => {
            text(b, words, P::TaskCapacityCauseInbox, &[]);
            field_number(b, words, P::Task, task);
            field_number(b, words, P::Messages, u64::from(messages));
            field_number(b, words, P::Bytes, u64::from(bytes));
        }
        OwnerTaskCapacityCause::Holdings { limit } => {
            text(b, words, P::TaskCapacityCauseHoldings, &[]);
            field_number(b, words, P::Limit, u64::from(limit));
        }
        OwnerTaskCapacityCause::HoldWaiters { limit } => {
            text(b, words, P::TaskCapacityCauseHoldWaiters, &[]);
            field_number(b, words, P::Limit, u64::from(limit));
        }
        OwnerTaskCapacityCause::Proposals { limit } => {
            text(b, words, P::TaskCapacityCauseProposals, &[]);
            field_number(b, words, P::Limit, u64::from(limit));
        }
        OwnerTaskCapacityCause::References { limit } => {
            text(b, words, P::TaskCapacityCauseReferences, &[]);
            field_number(b, words, P::Limit, u64::from(limit));
        }
        OwnerTaskCapacityCause::Subscriptions { limit } => {
            text(b, words, P::TaskCapacityCauseSubscriptions, &[]);
            field_number(b, words, P::Limit, u64::from(limit));
        }
        OwnerTaskCapacityCause::Funders { limit } => {
            text(b, words, P::TaskCapacityCauseFunders, &[]);
            field_number(b, words, P::Limit, u64::from(limit));
        }
        OwnerTaskCapacityCause::Writers { limit } => {
            text(b, words, P::TaskCapacityCauseWriters, &[]);
            field_number(b, words, P::Limit, u64::from(limit));
        }
    }
}
fn admission(b: &mut Builder, words: &Words, value: AdmissionLimit) {
    let phrase = match value {
        AdmissionLimit::Goals => P::AdmissionGoals,
        AdmissionLimit::Operations => P::AdmissionOperations,
        AdmissionLimit::Grace => P::AdmissionGrace,
        AdmissionLimit::Words => P::AdmissionWords,
        AdmissionLimit::Identity => P::AdmissionIdentity,
        AdmissionLimit::Roles => P::AdmissionRoles,
        AdmissionLimit::Arithmetic => P::AdmissionArithmetic,
        AdmissionLimit::Duplicate => P::AdmissionDuplicate,
        AdmissionLimit::Empty => P::AdmissionEmpty,
        AdmissionLimit::Batch => P::AdmissionBatch,
        AdmissionLimit::ProjectLiveTasks => P::AdmissionProjectLiveTasks,
        AdmissionLimit::LiveTasks => P::AdmissionLiveTasks,
        AdmissionLimit::Tree => P::AdmissionTree,
        AdmissionLimit::Depth => P::AdmissionDepth,
        AdmissionLimit::Delegates => P::AdmissionDelegates,
        AdmissionLimit::Subscription => P::AdmissionSubscription,
        AdmissionLimit::Dependencies => P::AdmissionDependencies,
        AdmissionLimit::Cycle => P::AdmissionCycle,
        AdmissionLimit::Spec => P::AdmissionSpec,
        AdmissionLimit::Contract => P::AdmissionContract,
        AdmissionLimit::Authority => P::AdmissionAuthority,
        AdmissionLimit::Inputs => P::AdmissionInputs,
        AdmissionLimit::Attempts => P::AdmissionAttempts,
        AdmissionLimit::LiveDelegates => P::AdmissionLiveDelegates,
        AdmissionLimit::Restore => P::AdmissionRestore,
        AdmissionLimit::Read => P::AdmissionRead,
        AdmissionLimit::Turn => P::AdmissionTurn,
        AdmissionLimit::HoldKind => P::AdmissionHoldKind,
        AdmissionLimit::HoldTaken => P::AdmissionHoldTaken,
        AdmissionLimit::Holds => P::AdmissionHolds,
        AdmissionLimit::InboxFull => P::AdmissionInboxFull,
        AdmissionLimit::Notes => P::AdmissionNotes,
        AdmissionLimit::Revision => P::AdmissionRevision,
        AdmissionLimit::Question => P::AdmissionQuestion,
        AdmissionLimit::Priority => P::AdmissionPriority,
        AdmissionLimit::Policy => P::AdmissionPolicy,
        AdmissionLimit::Resource => P::AdmissionResource,
        AdmissionLimit::Service => P::AdmissionService,
        AdmissionLimit::Accounts => P::AdmissionAccounts,
        AdmissionLimit::HolderAccounts => P::AdmissionHolderAccounts,
        AdmissionLimit::Sources => P::AdmissionSources,
        AdmissionLimit::Projects => P::AdmissionProjects,
        AdmissionLimit::SignIns => P::AdmissionSignIns,
        AdmissionLimit::SettingsProposals => P::AdmissionSettingsProposals,
    };
    text(b, words, phrase, &[]);
}
fn render_hosts(
    b: &mut Builder,
    client: &Borrowed<'_>,
    value: &OwnerDeployment,
    words: &Words,
    bindings: &mut JigBindings,
) {
    list(b, words, P::Hosts);
    for host in &value.hosts {
        region(
            b,
            words,
            P::Host,
            &[Hole::Number(host.channel)],
            Some(Key { owner: Owner::Jig.opaque(), kind: 94, a: host.channel, b: 0 }),
        );
        field_number(b, words, P::Host, host.channel);
        host_kind(b, words, host.kind);
        match host.kind {
            OwnerHostKind::Worker => {
                if let Some(name) = &host.name {
                    crate::literal_segment::render(b, name);
                }
            }
            OwnerHostKind::Engine => {}
        }
        programs::rule(b, words, host.rule);
        programs::render(b, words, &host.programs, host.awaiting_logins);
        field_number(b, words, P::Slots, u64::from(host.slots));
        field_number(b, words, P::Used, u64::from(host.used));
        field_number(b, words, P::Free, u64::from(host.free));
        text(b, words, if host.draining { P::Draining } else { P::NotDraining }, &[]);
        text(b, words, if host.connected { P::Connected } else { P::Disconnected }, &[]);
        wall(b, words, P::GraceUntil, P::UnknownClock, host.grace_until);
        optional_duration(b, words, P::GraceRemaining, host.grace_remaining);
        list(b, words, P::Runs);
        for value in &host.runs {
            run(b, client, words, value, bindings);
        }
        b.close();
        if host.cut {
            text(b, words, P::Cut, &[]);
        }
        b.close();
    }
    b.close();
    field_number(b, words, P::HostsForgotten, value.hosts_forgotten);
}
fn render_accounts(
    b: &mut Builder,
    client: &Borrowed<'_>,
    value: &OwnerDeployment,
    words: &Words,
    bindings: &mut JigBindings,
) {
    crate::account_catalogue::render(
        b,
        words,
        &value.providers,
        value.providers_cut,
        value.key_bytes,
        &value.readiness,
        value.readiness_cut,
    );
    list(b, words, P::Accounts);
    for account in &value.accounts {
        region(
            b,
            words,
            P::Account,
            &[Hole::Number(u64::from(account.number))],
            Some(Key {
                owner: Owner::Jig.opaque(),
                kind: 95,
                a: u64::from(account.number),
                b: account.generation.unwrap_or(0),
            }),
        );
        field_number(b, words, P::Account, u64::from(account.number));
        if let Some(generation) = account.generation {
            field_number(b, words, P::Generation, generation);
        }
        accounts::details(b, words, account);
        account_state(b, words, account.state);
        text(b, words, if account.usable { P::Usable } else { P::Unusable }, &[]);
        text(b, words, if account.expiring { P::Expiring } else { P::NotExpiring }, &[]);
        text(b, words, if account.attention { P::Attention } else { P::NoAttention }, &[]);
        wall(b, words, P::SpentUntil, P::UnknownClock, account.spent_until);
        region(b, words, P::Runs, &[], None);
        for chip in &account.runs {
            crate::cards::chip::chip(b, client, chip, words, bindings);
        }
        b.close();
        region(b, words, P::WaitingRuns, &[], None);
        for chip in &account.waiting {
            crate::cards::chip::chip(b, client, chip, words, bindings);
        }
        b.close();
        if account.cut {
            text(b, words, P::Cut, &[]);
        }
        b.close();
    }
    b.close();
}
/// Render every supplied deployment scalar and ordered row without decoding keys or deriving authority.
pub fn render(
    b: &mut Builder,
    client: &Borrowed<'_>,
    value: &OwnerDeployment,
    words: &Words,
    bindings: &mut JigBindings,
) {
    region(b, words, P::Deployment, &[], None);
    field_number(b, words, P::StoreBytes, value.store.bytes);
    field_number(b, words, P::StoreMark, value.store.mark);
    region(b, words, P::Hosts, &[], None);
    b.region(crate::regions::JigRegion::Hosts.region());
    render_hosts(b, client, value, words, bindings);
    b.close();
    list(b, words, P::Attempts);
    for value in &value.attempts {
        run(b, client, words, value, bindings);
    }
    b.close();
    region(b, words, P::Accounts, &[], None);
    b.region(crate::regions::JigRegion::Accounts.region());
    render_accounts(b, client, value, words, bindings);
    b.close();
    region(b, words, P::Capacity, &[], None);
    b.region(crate::regions::JigRegion::Capacity.region());
    capacity(b, words, P::TasksCapacity, &value.tasks);
    capacity(b, words, P::WorkersCapacity, &value.workers);
    capacity(b, words, P::AttemptsCapacity, &value.attempts_capacity);
    capacity(b, words, P::ProposalsCapacity, &value.proposals);
    field_number(b, words, P::WaitingAttempts, u64::from(value.waiting_attempts));
    duration(b, words, P::GraceBound, value.grace_bound);
    b.close();
    list(b, words, P::Projects);
    for project in &value.projects {
        region(
            b,
            words,
            P::Project,
            &[Hole::Number(u64::from(project.project))],
            Some(Key { owner: Owner::Jig.opaque(), kind: 98, a: u64::from(project.project), b: 0 }),
        );
        field_number(b, words, P::Project, u64::from(project.project));
        capacity(b, words, P::ProjectTasksCapacity, &project.tasks);
        field_number(b, words, P::Proposals, u64::from(project.proposals));
        b.close();
    }
    b.close();
    list(b, words, P::HelloRefusals);
    let mut position = 0_u64;
    for value in &value.hello_refusals {
        hello(b, words, value, position);
        position = position.checked_add(1).expect("bounded hello history");
    }
    b.close();
    field_number(b, words, P::HelloForgotten, value.hello_refusals_forgotten);
    list(b, words, P::PersonRefusals);
    let mut position = 0_u64;
    for value in &value.person_room_refusals {
        region(b, words, P::PersonRefusal, &[], Some(Key { owner: Owner::Jig.opaque(), kind: 99, a: position, b: 0 }));
        field_number(b, words, P::Person, value.key.person);
        text(b, words, P::RequestKey, &[]);
        crate::literal_segment::render(b, &value.key.key);
        project(b, words, value.project);
        text(b, words, P::At, &[Hole::Wall(words.adjusted_wall(value.at))]);
        person_action(b, words, value.action);
        person_cause(b, words, value.cause);
        b.close();
        position = position.checked_add(1).expect("bounded person history");
    }
    b.close();
    field_number(b, words, P::PersonForgotten, value.person_room_refusals_forgotten);
    duration(b, words, P::Retention, value.person_answer_retention);
    list(b, words, P::TaskRefusals);
    let mut position = 0_u64;
    for value in &value.task_room_refusals {
        region(b, words, P::TaskRefusal, &[], Some(Key { owner: Owner::Jig.opaque(), kind: 100, a: position, b: 0 }));
        project(b, words, value.request.project);
        crate::owner_result::requester(b, value.request.by, words);
        text(b, words, P::At, &[Hole::Wall(words.adjusted_wall(value.at))]);
        task_action(b, words, value.request.action);
        task_cause(b, words, value.request.cause);
        b.close();
        position = position.checked_add(1).expect("bounded task history");
    }
    b.close();
    field_number(b, words, P::TaskForgotten, value.task_room_refusals_forgotten);
    region(b, words, P::Connectors, &[], None);
    b.region(crate::regions::JigRegion::Connectors.region());
    for connector in &value.connectors {
        crate::cards::common::part(
            b,
            &jig_client::contract::Part::Deployment { connector: *connector },
            words,
            bindings,
        );
    }
    if value.connectors_cut {
        text(b, words, P::ConnectorsCut, &[]);
    } else if value.connectors.is_empty() {
        text(b, words, P::ConnectorsEmpty, &[]);
    }
    b.close();
    if value.cut {
        text(b, words, P::Cut, &[]);
    }
    b.close();
}
/// All nested lists can fill together, with each linked chip's unchanged renderer cost.
#[must_use]
pub fn max_nodes(l: &Limits) -> Option<u32> {
    let link = crate::chip_nodes(l)?.checked_add(3)?;
    let run = link.checked_add(14)?;
    let hosts = run.checked_mul(l.rows)?.checked_add(20)?.checked_add(l.rows.checked_mul(10)?)?.checked_mul(l.rows)?;
    let attempts = run.checked_mul(l.rows)?;
    let accounts = crate::chip_nodes(l)?
        .checked_mul(l.rows)?
        .checked_mul(2)?
        .checked_add(l.accounts.sources.checked_mul(13)?)?
        .checked_add(68)?
        .checked_mul(l.accounts.accounts)?;
    hosts
        .checked_add(attempts)?
        .checked_add(accounts)?
        .checked_add(crate::account_catalogue::max_nodes(l)?)?
        .checked_add(l.rows.checked_mul(80)?)?
        .checked_add(l.rows)?
        .checked_add(46)
}
/// Finite selected-language storage, including opaque fixed-width request keys.
#[must_use]
pub fn limits(l: &Limits, words: &Words) -> Option<skein_ui::Limits> {
    let nodes = max_nodes(l)?;
    let phrase = words.jig_capacity()?;
    let text = phrase.max(crate::literal_segment::max_bytes(l.text)?).max(64).checked_mul(2)?;
    // skein-ui still shares the external-target bound with text-run capacity.
    let text_per_node = text
        .max(jig_client::contract::ACCOUNT_MANAGE_USAGE.checked_add(phrase)?)
        .max(jig_client::contract::PROGRAM_HOST_NAME);
    Some(skein_ui::Limits {
        nodes,
        depth: l.rows.checked_add(32)?,
        text_per_node,
        text_bytes: nodes
            .checked_mul(text)?
            .checked_add(l.accounts.accounts.checked_mul(2)?.checked_mul(jig_client::contract::ACCOUNT_MANAGE_USAGE)?)?
            .checked_add(l.rows.checked_mul(jig_client::contract::PROGRAM_HOST_NAME)?)?,
        drawing_bytes: 0,
        styles_per_node: 2,
        patches: 0,
        bindings: nodes,
        slots: nodes,
        markdown_depth: 3,
        scene_marks: 0,
        scene_bytes: 0,
    })
}

fn chip_fits(value: &Chip, l: &Limits, words: &Words) -> Option<()> {
    if !value.title.bytes.fits(l.text) {
        return None;
    }
    match value.details.as_ref() {
        ChipPhase::Owner { phase, details } => {
            match phase {
                OwnerChipPhase::Waiting { dependencies, .. } => {
                    if dependencies.capacity() > l.rows {
                        return None;
                    }
                }
                OwnerChipPhase::Active(_)
                | OwnerChipPhase::Person { .. }
                | OwnerChipPhase::Closing(_)
                | OwnerChipPhase::Held { .. }
                | OwnerChipPhase::Ended => {}
            }
            crate::owner_chip::required(details, l, words)?;
        }
        ChipPhase::Waiting { dependencies, .. } => {
            if dependencies.capacity() > l.rows {
                return None;
            }
        }
        ChipPhase::Closing(value) => {
            if let Some(connectors) = &value.connectors
                && connectors.capacity() > l.rows
            {
                return None;
            }
        }
        ChipPhase::Active(_)
        | ChipPhase::Idle { .. }
        | ChipPhase::Starting
        | ChipPhase::Running { .. }
        | ChipPhase::WaitingForWords { .. }
        | ChipPhase::BackingOff { .. }
        | ChipPhase::Procedure { .. }
        | ChipPhase::Person { .. }
        | ChipPhase::Held { .. }
        | ChipPhase::Ended { .. } => {}
    }
    Some(())
}
fn run_fits(value: &OwnerHostRun, l: &Limits, words: &Words) -> Option<()> {
    match &value.task {
        OwnerTaskLink::Live(value) | OwnerTaskLink::Retained { chip: value, .. } => chip_fits(value, l, words),
        OwnerTaskLink::Stored { .. } => Some(()),
    }
}
/// Validate all independently allocated owner lists before using the finite fragment.
#[must_use]
pub fn required(value: &OwnerDeployment, l: &Limits, words: &Words) -> Option<skein_ui::Limits> {
    crate::account_catalogue::fits(&value.providers, &value.readiness, l)?;
    if value.connectors.capacity() > l.rows
        || value.hosts.capacity() > l.rows
        || value.attempts.capacity() > l.rows
        || value.accounts.capacity() > l.accounts.accounts
        || value.projects.capacity() > l.rows
        || value.hello_refusals.capacity() > l.rows
        || value.person_room_refusals.capacity() > l.rows
        || value.task_room_refusals.capacity() > l.rows
    {
        return None;
    }
    for host in &value.hosts {
        if let Some(name) = &host.name
            && crate::literal_segment::display_bytes(name)? > jig_client::contract::PROGRAM_HOST_NAME
        {
            return None;
        }
        programs::fits(&host.programs, l)?;
        if host.runs.capacity() > l.rows {
            return None;
        }
        for value in &host.runs {
            run_fits(value, l, words)?;
        }
    }
    for value in &value.attempts {
        run_fits(value, l, words)?;
    }
    for account in &value.accounts {
        if account.runs.capacity() > l.rows
            || account.waiting.capacity() > l.rows
            || account.sources.capacity() > l.accounts.sources
            || !account.display.bytes.fits(l.text)
        {
            return None;
        }
        if let Some(name) = &account.provider_name
            && !name.bytes.fits(l.text)
        {
            return None;
        }
        if let Some(address) = &account.manage_usage
            && (address.len() > usize::try_from(jig_client::contract::ACCOUNT_MANAGE_USAGE).ok()?
                || !skein_ui::markdown::safe_url(address))
        {
            return None;
        }
        if let Some(uses) = &account.uses
            && uses.capacity() > jig_client::contract::ACCOUNT_USES
        {
            return None;
        }
        for source in &account.sources {
            if let Some(name) = &source.name
                && !jig_client::contract::valid_program_name(name)
            {
                return None;
            }
        }
        for value in &account.runs {
            chip_fits(value, l, words)?;
        }
        for value in &account.waiting {
            chip_fits(value, l, words)?;
        }
    }
    limits(l, words)
}
