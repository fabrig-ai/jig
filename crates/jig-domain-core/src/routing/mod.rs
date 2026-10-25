//! Child entry points and their requests in the core's vocabulary
//! (`domain/engine.md`, sections 3 and 4). A root receives a whole child
//! request batch and routes it within the same decision.

#![expect(clippy::mod_module_files, reason = "the route split keeps dispatch in routing/mod.rs")]

pub(crate) mod amendment_reads;
pub(crate) mod application_routes;
pub(crate) mod create;
pub(crate) mod deadlines;
mod deployment_acts;
pub(crate) mod key_routes;
pub(crate) mod party_inputs;
mod propose;
pub(crate) mod result_reads;
pub(crate) mod settings;

use crate::{CallKey, CallPart, Core, RoutedCall};
use alloc::boxed::Box;
use children::{route_account, route_brief, route_fleet, route_projection_settled, tag_account, tag_brief, tag_fleet};
use historical::{route_historical_escalation, route_historical_proposal};
use jig_domain_accounts as accounts;
use jig_domain_authority as authority;
use jig_domain_brief as brief;
use jig_domain_fleet as fleet;
use jig_domain_notes as notes;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_domain_views as views;
use note_routes::route_notes;
use parties::route_holdings;
use people_routes::{route_people, sign_in};
use run_delegation::{
    delegate_holdings, delegate_input_failed, delegate_validated, procedure_holdings, procedure_step, route_period,
    route_start_recurring,
};
use run_payloads::{route_answer_payload, route_turn_payload};
use run_preparation::{
    brief_assembled, plan_brief, route_activate, route_claim_prepared, route_claim_refused, route_preparation_failed,
    route_prepared_agent, start_brief, workspace_prepared,
};
pub(crate) use runs::named_propose;
use runs::{named_action, route_named_answer, route_refused_payload};
use skein_lib::{Env, JournalRoom, List, Queue, ReplyTo, Token};
use task_routes::{
    route_connector_repair, route_person_escalation, route_person_proposal, route_task_escalation, route_tasks,
    tag_tasks,
};
use view_routes::{made_outputs, open, route_view};
pub use vocabulary::{
    Ask, Carried, CoreBriefBudgets, EscalationChoice, Event, Held, HoldingsNeed, Limits, NamedAction, Now,
    PayloadRefusal, ProposalChoice, Request, Requests, StepRefusal, Timer, ToolKind, WatchRefusal, Write,
};
pub(crate) use watch_marks::finish_watches;

pub(crate) mod account_requests;
mod children;
pub(crate) mod first_start;
mod historical;
#[path = "notes.rs"]
mod note_routes;
mod parties;
mod party_controls;
mod party_decisions;
#[path = "people.rs"]
mod people_routes;
pub(crate) mod procedure_steps;
mod projects;
mod proposal_revisions;
pub(crate) mod refusals;
mod run_controls;
mod run_delegation;
mod run_payloads;
mod run_preparation;
mod runs;
mod topics;
mod turns;
use turns::route_accepted_turn;
#[path = "tasks.rs"]
pub(crate) mod task_routes;
#[path = "views.rs"]
pub(crate) mod view_routes;
mod vocabulary;
pub(crate) mod watch_marks;

impl Core {
    /// Retain a numbered connector call until its translated answer arrives.
    #[must_use]
    pub fn reserve_connector_call(&mut self, key: CallKey) -> bool {
        self.pending_calls.insert(key, true) == Ok(None)
    }

    /// Retain a connector subscription's task-hub continuation with its named call.
    #[must_use]
    pub fn reserve_connector_subscription(&mut self, token: Token, key: CallKey, subscription: u64) -> bool {
        if !self.reserve_connector_call(key) {
            return false;
        }
        self.routing_calls.insert(token, RoutedCall::Subscribe { key, subscription }) == Ok(None)
    }
}

/// Person choice retained only while the task child decides its exact revision.
#[derive(PartialEq, Eq, Debug)]
pub(crate) struct PersonEscalation {
    request: Token,
    requester: u64,
    person: u64,
    project: u32,
    task: u64,
    revision: u64,
    decision: people::EscalationDecision,
}

#[derive(Debug)]
pub(super) enum CreationKind {
    Batch { person: u64, given: Option<people::Given>, stubs: Box<[tasks::Stub]> },
    Propose { person: u64, reason: Box<[u8]>, to: u32, stubs: Box<[tasks::Stub]> },
}

/// A party's batch waiting for the task child's make result.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Made {
    pub first: u64,
    pub given: Option<people::Given>,
    pub stubs: Box<[tasks::Stub]>,
}

/// One person task pending all numbered connector holdings in this decision.
#[derive(Debug)]
pub(crate) struct Creation {
    kind: CreationKind,
    members: Box<[tasks::New]>,
    at: usize,
    remaining: u32,
    holdings: List<tasks::Holding>,
    answered: List<u16>,
    failed: bool,
}

#[derive(Debug)]
pub(crate) struct DelegateCreation {
    key: CallKey,
    kind: BatchKind,
    members: Box<[tasks::New]>,
    remaining: u32,
    answered: List<u16>,
    failed: bool,
}

#[derive(Debug)]
enum BatchKind {
    Delegation { stubs: Box<[tasks::Stub]>, given: Option<people::Given> },
    Proposal { reason: Box<[u8]>, as_holder: bool },
}

#[derive(Debug)]
pub(crate) struct ProcedureCreation {
    decision: tasks::ProcedureDecision,
    members: Box<[tasks::New]>,
    remaining: u32,
    answered: List<u16>,
    failed: bool,
}

impl Core {
    /// Check a named run call against its live task and policy before routing it.
    #[must_use]
    pub fn authorize_tool(&self, key: CallKey, kind: ToolKind) -> Option<CallPart> {
        let Some(task) = self.tasks.task(key.task) else {
            return Some(CallPart::ToolDenied {
                answer: authority::Answer::Refuse,
                findings: Box::new([authority::Finding::UnknownProject]),
            });
        };
        let family = match &kind {
            ToolKind::Delegate => self.settings.tools.delegate,
            ToolKind::Message { .. } => self.settings.tools.message,
            ToolKind::Control => self.settings.tools.control,
            ToolKind::Decide => self.settings.tools.decide,
            ToolKind::Propose => self.settings.tools.propose,
            ToolKind::Subscribe => self.settings.tools.subscribe,
            ToolKind::Effect => self.settings.tools.effect,
            ToolKind::Note { .. } => self.settings.tools.note,
            ToolKind::Recall { .. } => self.settings.tools.recall,
            ToolKind::Read => self.settings.tools.read,
        };
        let call = match kind {
            ToolKind::Message { target } => authority::Call::Message {
                referenced: task.requester == tasks::Party::Task(target)
                    || task.delegates.contains(&target)
                    || task.references.contains(&target),
            },
            ToolKind::Note { scope } => {
                let scope = match scope {
                    notes::Scope::Deployment => authority::NoteScope::Deployment,
                    notes::Scope::Project { project } if project == task.project => authority::NoteScope::Project,
                    notes::Scope::Goal { project, goal }
                        if project == task.project && Some(goal) == self.goal_of(task) =>
                    {
                        authority::NoteScope::Goal
                    }
                    notes::Scope::Resources { project, connector, pattern } if project == task.project => {
                        let last = match pattern.last {
                            notes::Last::Exact(value) => authority::Last::Exact(value),
                            notes::Last::Open(value) => authority::Last::Open(value),
                        };
                        authority::NoteScope::Resources(authority::ResourceScope {
                            connector,
                            pattern: authority::Pattern { segments: pattern.segments, last },
                        })
                    }
                    notes::Scope::Project { .. } | notes::Scope::Goal { .. } | notes::Scope::Resources { .. } => {
                        return Some(CallPart::ToolDenied {
                            answer: authority::Answer::Refuse,
                            findings: Box::new([authority::Finding::Scope { source: authority::Source::Task }]),
                        });
                    }
                };
                authority::Call::Note(scope)
            }
            ToolKind::Recall { by } => {
                let readable = match by {
                    notes::Recall::Name { .. } => true,
                    notes::Recall::Search { scopes, .. } => {
                        let mut readable = true;
                        for scope in &scopes {
                            if !self.note_readable(key.task, scope) {
                                readable = false;
                            }
                        }
                        readable
                    }
                };
                if !readable {
                    return Some(CallPart::ToolDenied {
                        answer: authority::Answer::Refuse,
                        findings: Box::new([authority::Finding::Scope { source: authority::Source::Task }]),
                    });
                }
                authority::Call::Tool
            }
            ToolKind::Delegate
            | ToolKind::Control
            | ToolKind::Decide
            | ToolKind::Propose
            | ToolKind::Subscribe
            | ToolKind::Effect
            | ToolKind::Read => authority::Call::Tool,
        };
        let mut why = Queue::with_capacity(authority::max_out(self.authority.limits()).expect("bounded findings"));
        let answer = authority::check_call(
            &self.authority,
            &authority::CallAsk {
                project: task.project,
                authority: crate::translate::authority_value(&task.authority),
                family,
                call,
            },
            &mut why,
        );
        if answer == authority::Answer::Allow {
            return None;
        }
        let mut findings = List::with_capacity(why.capacity());
        for _ in 0..why.len() {
            findings.push(why.pop().expect("finding count")).expect("finding room");
        }
        Some(CallPart::ToolDenied { answer, findings: findings.into_boxed() })
    }

    /// A named recall may read only notes in this task's project and goal, or deployment notes.
    #[must_use]
    pub fn note_readable(&self, task: u64, scope: &notes::Scope) -> bool {
        let Some(row) = self.tasks.task(task) else { return false };
        match scope {
            notes::Scope::Deployment => true,
            notes::Scope::Project { project } | notes::Scope::Resources { project, .. } => *project == row.project,
            notes::Scope::Goal { project, goal } => *project == row.project && Some(*goal) == self.goal_of(row),
        }
    }
}

/// The parent caller held while the notes child reads its store pages.
#[derive(Debug)]
pub(crate) enum NoteRoute {
    Call { to: ReplyTo, key: CallKey },
    Person { request: Token },
    Brief { task: u64 },
}

impl Core {
    pub(crate) fn begin_note_route(&mut self, route: NoteRoute) -> Token {
        let owner = Token::new(self.next_note_owner);
        self.next_note_owner = self.next_note_owner.checked_add(1).expect("transient note owner space");
        let previous = self.note_routes.insert(owner, route).expect("notes keeps one pending route plus a refusal");
        assert!(previous.is_none(), "transient note owners are unique");
        owner
    }
}

fn route_room_checked(limits: &Limits) -> Option<u32> {
    accounts::worst_case(&limits.accounts)?;
    let tasks = tasks::max_out(&limits.tasks).checked_mul(16)?;
    let people = people::max_out(&limits.people).checked_mul(8)?;
    let fleet = fleet::max_out(&limits.fleet).checked_mul(8)?;
    let brief = brief::gather_max_out(&limits.brief).checked_mul(4)?;
    let bound =
        tasks
            .checked_add(people)?
            .checked_add(limits.people.inbox_tasks)?
            .checked_add(fleet)?
            .checked_add(brief)?
            .checked_add(accounts::max_out(&limits.accounts))?
            .checked_add(limits.people.pending.checked_mul(2)?)?
            .checked_add(limits.authority.projects.checked_mul(2)?)?
            .checked_add(limits.people.initial_owners)?
            .checked_add(limits.tasks.tasks.checked_mul(
                limits.tasks.tree_tasks.checked_add(limits.connectors.checked_mul(2)?)?.checked_add(4)?,
            )?)?
            .checked_add(limits.views.watchers.checked_mul(4)?)?
            .checked_add(8)?
            .checked_add(tasks::max_out(&limits.tasks).checked_mul(8)?)?
            .checked_add(people::max_out(&limits.people).checked_mul(4)?)?
            .checked_add(1)?;
    Some(bound.max(settings::room(limits)?.writes))
}

fn route_room(limits: &Limits) -> u32 {
    route_room_checked(limits).expect("validated core route room")
}

/// Largest core decision room, including a sibling callback reached after a
/// connector's own event. Roots use this when their input begins outside the
/// core but may synchronously hand work into it.
#[must_use]
pub fn room_max(limits: &Limits) -> Option<JournalRoom> {
    let bound = route_room_checked(limits)?;
    Some(JournalRoom { writes: bound, held: bound })
}

/// Pre-admission journal room for one core event, including all synchronous
/// sibling continuations and connector handoffs it can cause. The application
/// adds its connectors' writes and held outputs before taking the journal
/// decision (`domain/root.md`, section 10). The shared ceiling is deliberate:
/// a child callback may visit every other child before the decision ends.
#[must_use]
pub fn room(limits: &Limits, event: &Event) -> Option<JournalRoom> {
    let room = room_max(limits)?;
    match event {
        Event::ChangeSettings { .. } | Event::ProposeSettings { .. } => settings::room(limits),
        Event::SettingsCommit { room } => Some(JournalRoom { writes: *room, held: 0 }),
        Event::Fill { .. } => Some(JournalRoom { writes: 0, held: 0 }),
        Event::Read { .. } | Event::ReadLoaded { .. } => Some(JournalRoom { writes: 0, held: room.held }),
        Event::Stopping
        | Event::Marked { .. }
        | Event::Act { .. }
        | Event::Key { .. }
        | Event::Application { .. }
        | Event::ApplicationPart { .. }
        | Event::ApplicationFinished { .. }
        | Event::EffectStart { .. }
        | Event::SubscribeTopic { .. }
        | Event::EffectConnector(_)
        | Event::EffectDeadline
        | Event::SignIn { .. }
        | Event::Watch { .. }
        | Event::WatchRun { .. }
        | Event::Unwatch { .. }
        | Event::Delivered { .. }
        | Event::StreamDelivered { .. }
        | Event::RunInput { .. }
        | Event::RunText { .. }
        | Event::RunCall { .. }
        | Event::Tasks(_)
        | Event::StartRecurring { .. }
        | Event::StartBrief { .. }
        | Event::BriefNotes { .. }
        | Event::BriefAssembled { .. }
        | Event::WorkspacePrepared { .. }
        | Event::ClaimPrepared { .. }
        | Event::ClaimRefused { .. }
        | Event::ConnectorRepair { .. }
        | Event::Period { .. }
        | Event::PreparationFailed { .. }
        | Event::TranscriptUnread { .. }
        | Event::PersonProposal(_)
        | Event::PersonEscalation(_)
        | Event::TaskEscalation(_)
        | Event::People(_)
        | Event::Fleet(_)
        | Event::TurnPayload { .. }
        | Event::AnswerPayload { .. }
        | Event::AcceptedTurn { .. }
        | Event::RefusedPayload { .. }
        | Event::Activate { .. }
        | Event::PreparedAgent { .. }
        | Event::Forget { .. }
        | Event::EndedReadLoaded { .. }
        | Event::SeenConversationLoaded { .. }
        | Event::HistoricalProposal { .. }
        | Event::DecidedProposal { .. }
        | Event::HistoricalPersonTask { .. }
        | Event::PartyInputsLoaded { .. }
        | Event::EndedTaskLoaded { .. }
        | Event::HistoricalEscalation { .. }
        | Event::NamedAnswer { .. }
        | Event::NamedAction { .. }
        | Event::DelegateValidated { .. }
        | Event::DelegateInputFailed { .. }
        | Event::DelegateHoldings { .. }
        | Event::ProcedureHoldings { .. }
        | Event::ProcedureStep { .. }
        | Event::Brief(_)
        | Event::Holdings { .. }
        | Event::Account(_)
        | Event::View(_)
        | Event::Notes(_)
        | Event::SettledCall { .. }
        | Event::ProjectionSettled { .. }
        | Event::ProjectionBegin { .. }
        | Event::ProjectionFailed { .. }
        | Event::ProjectionRecover { .. } => Some(room),
    }
}

/// Route one child event and each sibling continuation within one core decision.
pub fn step(core: &mut Core, env: &Env<Limits>, event: Event) -> Requests {
    core.run_now = env.now;
    core.run_wall = env.wall;
    let room = route_room(&env.limits);
    let mut work = Queue::with_capacity(room);
    let mut out = Queue::with_capacity(room.checked_add(1).expect("core output mark"));
    work.push(event);
    for _ in 0..room {
        let Some(next) = work.pop() else { break };
        let Requests::Out(mut batch) = step_one(core, env, next, &mut work);
        for _ in 0..batch.len() {
            match batch.pop().expect("core route output count") {
                Request::Decided => {}
                request @ (Request::ProjectMade { .. }
                | Request::Write(_)
                | Request::Ask { .. }
                | Request::Held(_)
                | Request::Now(_)) => {
                    out.push(request);
                }
            }
        }
    }
    assert!(work.is_empty(), "finite sibling routes fit their bound");
    out.push(Request::Decided);
    Requests::Out(out)
}

/// Want every configured account once, after the restart has restored the core's own ranges.
pub fn want_accounts(core: &mut Core, env: &Env<Limits>) -> Requests {
    core.run_now = env.now;
    core.run_wall = env.wall;
    let mut work = Queue::with_capacity(0);
    let requests = route_account(core, env, &mut work, accounts::Event::Want);
    assert!(work.is_empty(), "wanting the accounts answers no party");
    requests
}

/// Resume the fleet's next held placement or relay through the core's sibling routes.
pub fn resume_fleet(core: &mut Core, env: &Env<Limits>) -> Requests {
    core.run_now = env.now;
    core.run_wall = env.wall;
    if core.stopping {
        return crate::stopping::empty();
    }
    let mut child = Queue::with_capacity(fleet::max_out(&env.limits.fleet));
    fleet::resume(&mut core.fleet, &Env { now: env.now, wall: env.wall, limits: env.limits.fleet }, &mut child);
    if !child.is_empty() {
        core.touch_deployment();
        core.touch_run_waits();
    }
    tag_fleet(core, env, &mut Queue::with_capacity(0), child, fleet::max_out(&env.limits.fleet))
}

#[expect(clippy::too_many_lines, reason = "exhaustive event dispatch delegates each concern")]
fn step_one(core: &mut Core, env: &Env<Limits>, event: Event, work: &mut Queue<Event>) -> Requests {
    let event = match crate::stopping::gate(core, event) {
        Ok(event) => event,
        Err(answer) => return answer,
    };
    match event {
        Event::ChangeSettings { reply_to, sign_in, key, digest, change } => {
            settings::start(core, env, work, reply_to, sign_in, key, digest, change)
        }
        Event::ProposeSettings { reply_to, sign_in, key, digest, change } => {
            settings::propose_start(core, env, work, reply_to, sign_in, key, digest, *change)
        }
        Event::SettingsCommit { room } => core.commit_restored_settings(env, room.saturating_sub(1)),
        Event::Fill { bytes } => {
            core.fill = bytes;
            crate::stopping::empty()
        }
        Event::Stopping => crate::stopping::start(core),
        Event::Marked { connector, subjects } => view_routes::marked(core, connector, subjects),
        Event::Act { reply_to, connector, key, act, carried } => {
            deployment_acts::start(core, env, work, reply_to, connector, key, act, carried)
        }
        Event::Key { reply_to, sign_in, key, ask, carried } => {
            key_routes::start(core, env, work, reply_to, sign_in, key, *ask, carried)
        }
        Event::Application { reply_to, sign_in, key, scope, connector, digest, owner, carried } => {
            let input = application_routes::Input { connector, owner, carried: application_routes::number(carried) };
            application_routes::start(core, env, work, reply_to, sign_in, key, scope, digest, input)
        }
        Event::ApplicationPart { request, ask } => application_routes::part(core, env, work, request, *ask),
        Event::ApplicationFinished { request, outcome } => {
            application_routes::finished(core, env, work, request, outcome)
        }
        Event::ProjectionSettled { goal, connector, generation } => {
            route_projection_settled(core, env, goal, connector, generation)
        }
        Event::ProjectionBegin { goal, connector, owner } => {
            crate::projection_failures::begin(core, goal, connector, owner)
        }
        Event::ProjectionFailed { goal, connector, source } => {
            let mut out = Queue::with_capacity(2);
            crate::projection_failures::failed(core, goal, connector, source, env.wall, &mut out);
            out.push(Request::Decided);
            Requests::Out(out)
        }
        Event::ProjectionRecover { goal, connector, owner } => {
            crate::projection_failures::recover(core, goal, connector, owner)
        }
        Event::SettledCall { to, key, call } => crate::conversation::settle(core, &env.limits, to, key, call),
        Event::EffectStart { owner, connector, origin } => crate::effects::start(core, env, owner, connector, origin),
        Event::EffectConnector(event) => crate::effects::connector(core, env, work, event),
        Event::SubscribeTopic { to, key, connector, topic, resources } => {
            topics::subscribe(core, env, work, to, key, connector, topic, &resources)
        }
        Event::EffectDeadline => crate::effects::deadline(core, env),
        Event::SignIn { reply_to, identity, held } => sign_in(core, env, work, reply_to, identity, held),
        Event::ReadLoaded { load, result } => crate::reads::loaded(core, env, load, result),
        Event::Read { reply, sign_in, read } => crate::reads::start(core, env, reply, sign_in, read),
        Event::Watch { watcher, sign_in, subject, window } => open(core, env, watcher, sign_in, subject, window),
        Event::WatchRun { watcher, sign_in, task, turn, offset } => {
            view_routes::open_run(core, env, watcher, sign_in, task, turn, offset)
        }
        Event::Unwatch { watcher } => route_view(core, env, views::Event::Unwatch { watcher }),
        Event::Delivered { watcher } => route_view(core, env, views::Event::Delivered { watcher }),
        Event::StreamDelivered { watcher } => view_routes::stream_delivered(core, env, watcher),
        Event::RunCall { run, attempt, call } => view_routes::run_call(core, env, run, attempt, call),
        Event::RunInput { run, attempt, turn, retry, offset, id, tool, input } => {
            view_routes::run_input(core, env, run, attempt, turn, retry, offset, id, tool, input)
        }
        Event::RunText { run, attempt, turn, retry, offset, text, call } => {
            view_routes::run_text(core, env, run, attempt, turn, retry, offset, text, call)
        }
        Event::Tasks(event) => route_tasks(core, env, work, event),
        Event::StartRecurring { project, authority, template } => {
            route_start_recurring(core, work, project, authority, template)
        }
        Event::StartBrief { task } => start_brief(core, env, work, task),
        Event::BriefNotes { task, lines, more } => plan_brief(core, env, work, task, lines, more),
        Event::BriefAssembled { task, ready } => brief_assembled(core, work, task, ready),
        Event::WorkspacePrepared { task, attempt, writes, credentials } => {
            workspace_prepared(core, env, work, task, attempt, writes, credentials)
        }
        Event::ClaimPrepared { task, attempt, budget, writes } => {
            route_claim_prepared(core, env, work, task, attempt, budget, writes)
        }
        Event::ClaimRefused { task } => route_claim_refused(core, work, task),
        Event::ConnectorRepair { seed, executor, spec, contract, holdings } => {
            route_connector_repair(work, seed, executor, spec, contract, holdings)
        }
        Event::Period { project, period, budget, next } => route_period(core, work, project, period, budget, next),
        Event::PreparationFailed { task } => route_preparation_failed(core, work, task),
        Event::TranscriptUnread { task } => run_preparation::transcript_unread(core, work, task),
        Event::PersonProposal(event) => route_person_proposal(core, env, event),
        Event::TaskEscalation(event) => route_task_escalation(core, env, event),
        Event::PersonEscalation(event) => route_person_escalation(core, env, event),
        Event::People(event) => route_people(core, env, work, event),
        Event::Holdings { request, connector, holdings } => {
            route_holdings(core, env, work, request, connector, holdings)
        }
        Event::Fleet(event) => route_fleet(core, env, work, event),
        Event::TurnPayload { run, attempt, turn, body, read, cumulative, refused } => {
            route_turn_payload(core, work, run, attempt, turn, body, read, refused, cumulative)
        }
        Event::AnswerPayload {
            run,
            attempt,
            payload,
            cumulative,
            end,
            saved,
            invalid_saved,
            grants_refused,
            read,
            refused,
        } => route_answer_payload(
            core,
            work,
            run,
            attempt,
            payload,
            cumulative,
            end,
            saved,
            invalid_saved,
            grants_refused,
            read,
            &refused,
        ),
        Event::AcceptedTurn { task, attempt, turn, accepted, cumulative, read, transcript, opens_window } => {
            route_accepted_turn(core, env, task, attempt, turn, accepted, cumulative, read, transcript, opens_window)
        }
        Event::RefusedPayload { request, problem, payload } => {
            route_refused_payload(core, work, request, problem, payload)
        }
        Event::Activate { context, ready } => route_activate(core, env, work, context, ready),
        Event::PreparedAgent { context, transcript_waiter, busy } => {
            route_prepared_agent(core, context, transcript_waiter, busy)
        }
        Event::Forget { owner } => crate::forget::forget(core, env, work, &owner),
        Event::EndedReadLoaded { request, task, party, through, busy } => crate::ended_marks::loaded(
            core,
            env,
            work,
            crate::ended_marks::Answer { request, task, party, through, busy },
        ),
        Event::SeenConversationLoaded { request, row, busy } => {
            crate::inbox::seen_conversation_loaded(core, env, work, request, row, busy)
        }
        Event::DecidedProposal { to, key, proposal, row, busy } => {
            historical::decided_proposal(core, to, key, proposal, row, busy)
        }
        Event::HistoricalProposal { request, person, project, proposer, proposal, row, busy } => {
            route_historical_proposal(core, work, request, person, project, proposer, proposal, row, busy)
        }
        Event::EndedTaskLoaded { request, row, busy } => result_reads::loaded(core, env, work, request, row, busy),
        Event::PartyInputsLoaded { request, row, busy } => {
            let mut out = Queue::with_capacity(route_room(&env.limits));
            party_inputs::loaded(core, env, &mut out, work, party_inputs::Loaded { request, row, busy });
            Requests::Out(out)
        }
        Event::HistoricalPersonTask { request, person, project, task, row, busy } => {
            let outcome = core.person_task_ended(person, project, task, row, busy);
            work.push(Event::People(people::Event::Decided { request, outcome }));
            let mut out = Queue::with_capacity(1);
            out.push(Request::Decided);
            Requests::Out(out)
        }
        Event::HistoricalEscalation { request, person, project, task, revision, row, busy } => {
            route_historical_escalation(core, env, work, request, person, project, task, revision, row, busy)
        }
        Event::NamedAnswer { to, key, part } => route_named_answer(core, to, key, part),
        Event::NamedAction { to, key, action } => named_action(core, env, work, to, key, action),
        Event::DelegateValidated { to, key, batch, stubs } => delegate_validated(core, env, to, key, batch, stubs),
        Event::DelegateInputFailed { to, key } => delegate_input_failed(work, to, key),
        Event::DelegateHoldings { request, connector, holdings } => {
            delegate_holdings(core, env, work, request, connector, holdings)
        }
        Event::ProcedureStep { task, step, connector, code, decision: action } => {
            procedure_step(core, env, work, task, step, connector, code, action)
        }
        Event::ProcedureHoldings { task, step, connector, holdings } => {
            procedure_holdings(core, env, work, task, step, connector, holdings)
        }
        Event::Brief(event) => route_brief(core, env, event),
        Event::Account(event) => route_account(core, env, work, event),
        Event::View(event) => route_view(core, env, event),
        Event::Notes(event) => route_notes(core, env, work, event),
    }
}

/// Fire one due child timer, returning its whole bounded request batch.
pub fn fire(core: &mut Core, env: &Env<Limits>, timer: Timer) -> Requests {
    core.run_now = env.now;
    core.run_wall = env.wall;
    if core.stopping && crate::stopping::waits(timer) {
        return crate::stopping::empty();
    }
    match timer {
        Timer::People => {
            let mut out = Queue::with_capacity(people::max_out(&env.limits.people));
            people::fire(&mut core.people, &Env { now: env.now, wall: env.wall, limits: env.limits.people }, &mut out);
            let mut work = Queue::with_capacity(route_room(&env.limits));
            let requests = people_routes::tag_people(core, env, out, &mut work);
            assert!(work.is_empty(), "people expirations produce no sibling continuations");
            requests
        }
        Timer::Tasks => {
            let mut out = Queue::with_capacity(tasks::max_out(&env.limits.tasks));
            tasks::fire(&mut core.tasks, &Env { now: env.now, wall: env.wall, limits: env.limits.tasks }, &mut out);
            tag_tasks(core, env, out, tasks::max_out(&env.limits.tasks), false, false, false)
        }
        Timer::Fleet => {
            let mut out = Queue::with_capacity(fleet::max_out(&env.limits.fleet));
            fleet::fire(&mut core.fleet, &Env { now: env.now, wall: env.wall, limits: env.limits.fleet }, &mut out);
            if !out.is_empty() {
                core.touch_deployment();
                core.touch_run_waits();
                core.touch_accounts(None);
            }
            tag_fleet(core, env, &mut Queue::with_capacity(0), out, fleet::max_out(&env.limits.fleet))
        }
        Timer::Brief => {
            let mut out = Queue::with_capacity(brief::gather_max_out(&env.limits.brief));
            brief::gather_fire(
                &mut core.brief,
                &Env { now: env.now, wall: env.wall, limits: env.limits.brief },
                &mut out,
            );
            tag_brief(core, env, out, brief::gather_max_out(&env.limits.brief))
        }
        Timer::Account => {
            let mut out = Queue::with_capacity(accounts::max_out(&env.limits.accounts));
            accounts::fire(
                &mut core.accounts,
                &Env { now: env.now, wall: env.wall, limits: env.limits.accounts },
                &mut out,
            );
            core.touch_deployment();
            core.touch_run_waits();
            core.touch_accounts(None);
            let mut work = Queue::with_capacity(0);
            let requests = tag_account(core, env, &mut work, out);
            assert!(work.is_empty(), "an account's alarm answers no party");
            requests
        }
        Timer::View => {
            let mut out = Queue::with_capacity(views::max_out(&env.limits.views));
            views::fire(&mut core.views, &Env { now: env.now, wall: env.wall, limits: env.limits.views }, &mut out);
            made_outputs(core, env, out)
        }
    }
}

pub(crate) fn tag_ended_mark(
    core: &mut Core,
    env: &Env<Limits>,
    child: Queue<people::Request>,
    work: &mut Queue<Event>,
) -> Requests {
    people_routes::tag_people(core, env, child, work)
}
