//! The engine core composes the task hub, policy, parties, fleet, accounts,
//! briefs, notes and views. It keeps work in jig's vocabulary and asks its
//! application's root to route the store, hosts, parties and connectors.
//! See `domain/engine.md`, sections 3 to 5.
//!
//! The core never holds a connector's value or a store encoding.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

mod live;
pub use live::LiveAsk;

pub mod templates;
pub use templates::Template;
pub mod acts;
mod amendments;
mod as_allowed;
mod authority_facts;
mod brief_text;
mod compound;
pub use brief_text::{BriefPart, BriefTextLimits, note_index_text, read_brief_part};
pub mod connector;
mod delegation;
pub use amendments::TaskAmendDenied;
pub use delegation::{
    Delegate, Dependency, ProcedureStep, StepAmendment, StepCancel, StepEnd, StepMessage, resolved_delegate_authority,
    symbolic_grants,
};
pub mod documents;
mod effects;
mod escalation;
pub mod waits;
pub use effects::{EffectKey, EffectOrigin, EffectPurpose, effect_worst_case};

mod ended_marks;
pub mod forget;
mod funding;
mod goal_of;
mod goals;
pub use goals::CreateStart;
mod account_preference;
mod coverage;
mod historical;
mod inbox;
mod indexes;
mod limit_refusals;
mod llm;
mod memory;

pub use memory::{Problem, policy_owned_bytes, worst_case};
mod model_names;
mod numbers;
mod person_task;
mod policy;
mod projects;
pub use policy::{
    authority_to_authority, authority_to_people, grants_to_authority, pattern_to_people, requirement_to_authority,
    snapshot as policy_snapshot,
};
mod projections;
pub use projections::{
    MilestoneId, Projection, ProjectionFeed, ProjectionGoal, ProjectionKey, ProjectionMilestone, ProjectionRecord,
    ProjectionTask, finish_decision, projection_bytes, projection_record_bytes,
};
mod proposals;
/// Authenticated one-shot reads whose answers wait for durability.
pub mod reads;
pub use reads::{SettingsAnswer, SettingsRead};
mod relays;
mod resources;
mod restart;
mod stopping;
pub use restart::{Census, HeldLimit, ReadResults, RestartRequest, RestartStep, RestoreChild, RestoreRange};
mod host_rule;
mod revisions;
mod roles;
mod routing;
mod run;
mod run_choice;
mod transcript_text;
pub use host_rule::HostRule;
pub use run_choice::{RunAllowance, RunChoice, RunWait};
pub use transcript_text::{TranscriptRow, TranscriptText};
mod sibling_routes;
pub use numbers::{Counters, Deployment, Family, fresh};
pub use routing::{
    Ask, Carried, CoreBriefBudgets, EscalationChoice, Event, Held, Limits, NamedAction, Now, PayloadRefusal,
    ProposalChoice, Request, Requests, StepRefusal, Timer, ToolKind, WatchRefusal, Write, fire, resume_fleet, room,
    room_max, step, want_accounts,
};
pub use sibling_routes::{MadeRoute, PersonMessage, SentRoute};
mod conversation;
pub use conversation::{DeliveryOutcome, Settled, SettledAnswer, SettledCall, conversation_worst_case};
pub mod charters;
pub mod fleet_needs;
pub mod settings;
pub use charters::Charter;
mod stored;
mod translate;
use run::HistoricalResult;
pub use run::{
    EFFORT_BYTES, EFFORTS, GoalRoute, MODEL_DESCRIPTION_BYTES, MODEL_NAME_BYTES, Model, PendingRelay,
    PersonProposalRoute, PersonTaskRoute, RestoringProof, RoutedCall, RunAdmission, RunCharter, RunPolicy, Transcript,
    reserve_floor,
};
pub use stored::{
    CallKey, CallPart, CallRecord, CallReplay, CoreKey, CoreRecord, CurrentWindow, EscalationDecisionRecord, Key, Made,
    ProposalDecisionRecord, Record, Restored, RunProof, TerminalRecord, TurnProof, TurnRecord,
};
pub use translate::task_authority;

use alloc::boxed::Box;
use jig_domain_accounts as accounts;
use jig_domain_authority as authority;
use jig_domain_brief as brief;
use jig_domain_fleet as fleet;
use jig_domain_notes as notes;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_domain_views as views;
use skein_lib::{List, Map, Queue, Token, Wall};

/// Policy copied into a later project (configuration.md, 3; core.md, 6.1).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ConfiguredPolicy {
    pub policy: authority::Policy,
    pub permissions: Box<[people::PermissionRole]>,
    pub templates: Box<[people::TemplateOverride]>,
}

/// One first-start project seed (configuration.md, 3; core.md, 6.1).
#[derive(Debug)]
pub struct SeedProject {
    pub number: u32,
    pub name: Box<str>,
    pub policy: ConfiguredPolicy,
}

/// Application-supplied initial core state, in jig's vocabulary
/// (`engine.md`, 6; `configuration.md`, 3).
#[derive(Debug)]
pub struct Config {
    /// Configured worker names in authenticated protocol position order.
    pub worker_names: Box<[Box<[u8]>]>,
    /// Identity committed with the first decision.
    pub deployment: [u8; 16],
    /// Deterministic task scheduling seed.
    pub seed: u64,
    /// Workers share this engine's process and cannot outlive it.
    pub workers_in_process: bool,
    /// Normal retained turns per attempt in the deployment's hub.
    pub hub_turns: u32,
    /// Initial authenticated owners.
    pub owners: Box<[people::InitialOwner]>,
    /// Configured deployment acts for each connector (people.md, 5.3).
    pub grants: Box<[people::ActGrant]>,
    /// Configured authority and policy.
    pub defaults: settings::Defaults,
    /// Deployment and run settings retained by the core.
    pub settings: Settings,
    /// Projects used only by the first start (configuration.md, 3).
    pub seed_projects: Box<[SeedProject]>,
    /// Numbered connectors the application routes for task lifecycle handoffs.
    pub connectors: Box<[u16]>,
}

/// Immutable deployment and run settings used by core decisions.
#[derive(Debug)]
pub struct Settings {
    /// Provider number for the application's services.
    pub deployment_provider: u16,
    /// Configured models in configuration's order: model `i` is bit `i` of an authority's
    /// models and the `i` a task's LLM choice names (authority.md, 3; tasks.md, 3).
    pub models: Box<[Model]>,
    /// Maximum committed conversation bytes resumed whole.
    pub resume_bytes: u32,
    /// Maximum transcript text retained for a fresh run's brief.
    pub tail_bytes: u32,
    /// Jig's one-bit authority family for each engine tool.
    pub tools: ToolFamilies,
    /// Configured LLM providers, for the accounts child (accounts.md, 3.1).
    pub llm_providers: accounts::Providers,
    /// Actual client key ceiling, supplied by startup after protocol limits are checked.
    pub key_bytes: u32,
    /// The driver's redirect bound: the most bytes a pasted sign-in address may have.
    pub redirect_bytes: u32,
}

/// Jig's authority bits for the engine's tool families.
#[derive(Clone, Copy, Debug)]
pub struct ToolFamilies {
    pub delegate: authority::Tools,
    pub message: authority::Tools,
    pub control: authority::Tools,
    pub decide: authority::Tools,
    pub propose: authority::Tools,
    pub subscribe: authority::Tools,
    pub effect: authority::Tools,
    pub note: authority::Tools,
    pub recall: authority::Tools,
    pub read: authority::Tools,
    pub settings: u64,
}

/// Connector number reserved for the core's recurring procedure (tasks.md, 10).
pub const RECURRING: u16 = u16::MAX;

impl ToolFamilies {
    /// Every offered family has one distinct authority bit.
    #[must_use]
    pub fn valid(&self) -> bool {
        let mut seen = 0_u64;
        for family in [
            self.delegate,
            self.message,
            self.control,
            self.decide,
            self.propose,
            self.subscribe,
            self.effect,
            self.note,
            self.recall,
            self.read,
            authority::Tools(self.settings),
        ] {
            if family.0.count_ones() != 1 || seen & family.0 != 0 {
                return false;
            }
            seen |= family.0;
        }
        true
    }

    /// The eleven engine tool families, numbered alike in every deployment.
    pub const JIG: Self = Self {
        delegate: authority::Tools(1),
        message: authority::Tools(2),
        control: authority::Tools(4),
        decide: authority::Tools(8),
        propose: authority::Tools(16),
        subscribe: authority::Tools(32),
        effect: authority::Tools(64),
        note: authority::Tools(128),
        recall: authority::Tools(256),
        read: authority::Tools(512),
        settings: 1024,
    };
}

/// A watch's authenticated owner and concrete construction inputs.
/// `client/domain/contract.md`, sections 7.1 and 7.2.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct WatchBinding {
    /// The party whose document is constructed.
    pub(crate) party: u64,
    /// The sign-in that admitted this watch.
    pub(crate) sign_in: u64,
    /// The core-owned subject.
    pub(crate) subject: skein_lib::Id<WatchedSubject>,
    /// The requested bounded window.
    pub(crate) window: documents::Window,
}

/// A concrete subject shared by watches, with its opaque child name.
#[derive(Debug)]
pub(crate) struct WatchedSubject {
    pub(crate) subject: documents::Subject,
    pub(crate) watchers: u32,
}

/// One delegation waiting for the task child's durable creation reply.
pub type Delegating = (CallKey, Box<[tasks::Stub]>, Option<people::Given>);

/// The engine's children, routing and durable live state (domain/engine.md,
/// section 3). Configuration limits stay immutable (programming-model.md,
/// section 10.1); route room is cached only after validating those limits.
#[derive(Debug)]
#[expect(clippy::partial_pub_fields, reason = "the keyed escalation route remains private to the core")]
#[expect(
    clippy::struct_excessive_bools,
    reason = "pending generation, worker placement, stopping and role-table fullness are independent facts"
)]
pub struct Core {
    /// Immutable startup numbers for refusal facts; no policy or admission is recomputed.
    pub(crate) refusal_limits: Limits,
    room_max: skein_lib::JournalRoom,
    run_inbox: u32,
    pub(crate) run_now: skein_lib::Time,
    pub(crate) run_wall: Wall,
    /// Last reported store map bytes (protocol/store.md, 11.2).
    pub fill: u64,
    pub(crate) workers_in_process: bool,
    pub(crate) stopping: bool,
    pub(crate) draining: Option<bool>,
    pub(crate) question_activity: Map<(u64, u64), tasks::ActivityLine>,
    pub(crate) census_progress: Map<u64, inbox::Progress>,
    pub(crate) census_position: Option<(u64, u64, u64)>,
    pub(crate) ended_marks: Map<Token, ended_marks::Loaded>,
    pub(crate) archived_chats: Map<u64, inbox::ArchivedChat>,
    pub(crate) proposed_input: Option<Result<Box<settings::Proposal>, people::Malformed>>,
    pub(crate) settings_offering: Option<(Token, u64, u32, settings::Proposal)>,
    pub(crate) settings_proposals: Map<(people::Proposer, u32), settings::Proposal>,
    pub(crate) settings_input: Option<(Result<Box<settings::Change>, settings::Failure>, u32)>,
    pub(crate) key_input: Option<Carried>,
    pub(crate) keys: Map<Token, routing::key_routes::Flight>,
    pub(crate) application_input: Option<routing::application_routes::Input>,
    pub(crate) task_capacity_mark: u64,
    pub(crate) settings_holders: u32,
    pub(crate) applications: Map<Token, routing::application_routes::Flight>,
    pub(crate) restart: restart::Restart,
    pub(crate) restart_pending: bool,
    pub(crate) restart_attempts: u32,
    /// Application connectors addressed only by number.
    pub connectors: Box<[u16]>,
    /// Subjects owned by the parent; the child echoes each watch token.
    pub(crate) view_subjects: Map<Token, WatchBinding>,
    pub(crate) view_names: skein_lib::Slab<WatchedSubject>,
    pub(crate) changed_views: skein_lib::Set<skein_lib::Id<WatchedSubject>>,
    pub(crate) reads: skein_lib::Slab<reads::Pending>,
    /// Historical dependency results held for preparation.
    pub(crate) dependency_results: Map<u64, Box<[HistoricalResult]>>,
    /// Numbered task creation flights.
    pub(crate) made: Map<Token, routing::Made>,
    /// Goal proposal decisions in flight.
    pub goal_routes: Map<Token, GoalRoute>,
    /// Delegations awaiting input checks.
    pub delegating: Map<Token, Delegating>,
    /// Person proposal decisions in flight.
    pub routing_people_proposals: Map<Token, PersonProposalRoute>,
    /// Ended result positions waiting to be delivered.
    pub ending_positions: Map<u64, u64>,
    /// Person messages awaiting task admission.
    pub saying: Map<Token, (u64, Option<u64>)>,
    /// Person task moves awaiting their answer.
    pub moving: Map<Token, u64>,
    /// Person task request routes in flight.
    pub person_tasks: Map<Token, PersonTaskRoute>,
    /// Authenticated person escalation choices awaiting task semantics.
    pub(crate) person_escalations: Map<Token, routing::PersonEscalation>,
    /// Ended results waiting on the store for one core-owned read.
    pub(crate) result_reads: Map<Token, routing::result_reads::ResultRead>,
    pub(crate) next_result_request: u64,
    /// Person task creations waiting for their historical inputs.
    pub(crate) creating_inputs: Map<Token, routing::party_inputs::Inputs>,
    /// Person task creations waiting for connector-owned resource holdings.
    pub(crate) creating: Map<Token, routing::Creation>,
    pub(crate) resource_roles: Map<resources::Key, connector::ResourceRole>,
    pub(crate) resource_roles_full: bool,
    pub(crate) awaiting_resources: Map<Token, resources::Batch>,
    /// Named task delegation batches awaiting connector-owned holdings.
    pub(crate) creating_delegates: Map<Token, routing::DelegateCreation>,
    /// Procedure batches awaiting connector-owned holdings.
    pub(crate) creating_procedures: Map<(u64, u64), routing::ProcedureCreation>,
    /// At most one committed word per run awaiting host delivery.
    pub relaying: Map<u64, PendingRelay>,
    /// Configured project identifiers for bootstrap.
    pub projects: List<u32>,
    pub current: settings::Current,
    pub(crate) held: settings::Held,
    pub(crate) hub_turns: u32,
    pub(crate) settings_restore: settings::restart::Restore,
    pub(crate) restore_failure: Option<RestartRequest>,
    /// Names seeded or restored with the project policies (core.md, 6.1).
    pub(crate) names: Map<u32, Box<str>>,
    /// Mutable connector permission mappings approved by policy edits.
    pub permission_roles: Map<u32, Box<[people::PermissionRole]>>,
    /// Template overrides retained with each project policy.
    pub template_overrides: Map<u32, Box<[people::TemplateOverride]>>,
    pub(crate) seed_projects: Box<[SeedProject]>,
    pub(crate) seed_owners: Box<[people::InitialOwner]>,
    /// Immutable settings for core decisions.
    pub settings: Settings,
    /// Named calls awaiting their routed decision.
    pub pending_calls: Map<CallKey, bool>,
    /// Durable live named-call decisions, with connector payloads retained by
    /// their owner and represented here only by connector number.
    pub call_parts: Map<CallKey, CallPart>,
    /// Settled host answers and opaque delivery evidence under the same retained names.
    pub call_settled: Map<CallKey, SettledCall>,
    /// Calls routed among the core children.
    pub routing_calls: Map<Token, RoutedCall>,
    /// Synchronous connector descriptions and verdicts in this decision.
    pub(crate) effect_flights: Map<Token, Box<effects::Flight>>,
    /// Live reply rights waiting for a committed effect or its answer deadline.
    pub(crate) effect_replies: Map<u64, effects::Waiting>,
    /// Connectors whose outboxes must finish before a task closes.
    pub(crate) closing_connectors: Map<(u64, u16), ()>,
    pub(crate) projections: Map<u64, Box<Projection>>,
    /// Connector payloads awaiting the task hub's proposal admission.
    pub(crate) proposing_effects: Map<Token, (u16, Token)>,
    pub(crate) procedure_proposals: Map<Token, effects::ProcedureProposal>,
    /// Correlations while the notes child loads a page for a caller.
    pub(crate) note_routes: Map<Token, routing::NoteRoute>,
    /// Preparations waiting for notes, or deferring their next activation for load room.
    pub(crate) pending_note_briefs: Map<u64, routing::NoteBrief>,
    /// Transient owner names for notes routes; pending calls are restored from their records.
    pub(crate) next_note_owner: u64,
    /// Durable deployment numbers.
    pub counters: Counters,
    /// Live task preparation contexts.
    pub contexts: Map<u64, Box<tasks::RunContext>>,
    /// Briefs waiting for their application connector's workspace translation.
    pub(crate) workspace_pending: Map<u64, u64>,
    pub(crate) workspace_credentials: Map<u64, Box<[u32]>>,
    /// Recent transcript windows for runs being prepared.
    pub transcripts: Map<u64, Transcript>,
    /// Current durable claim evidence.
    pub proofs: Map<u64, RunProof>,
    /// Durable proof kept aside until a replacement claim commits.
    pub replaced: Map<u64, RunProof>,
    /// Correlation while current proofs are restored.
    pub restoring_proofs: Map<u64, RestoringProof>,
    /// Restored claims not yet reported by a host.
    pub unreported_restored: Map<u64, u64>,
    /// Claims awaiting completion of their route.
    pub claiming: Map<u64, u64>,
    /// Child work due to activation.
    pub due: Queue<Box<tasks::RunContext>>,
    /// Host adoption events held through startup.
    pub adopted: Queue<fleet::Event>,
    /// The task hub.
    pub tasks: tasks::Domain,
    /// Policy and authority.
    pub authority: authority::Domain,
    /// Parties and authenticated requests.
    pub people: people::Domain,
    /// Hosts and attempts.
    pub fleet: fleet::Domain,
    /// Secret-free account lifetimes.
    pub accounts: accounts::Domain,
    /// Run brief gathering.
    pub brief: brief::GatherDomain,
    /// Scoped notes.
    pub notes: notes::Domain,
    /// Live views.
    pub views: views::Domain,
}

fn assert_connector_registry(connectors: &[u16]) {
    assert!(!connectors.contains(&RECURRING), "the core reserves its recurring connector number");
    for connector in connectors {
        let mut count = 0_u32;
        for other in connectors {
            if connector == other {
                count = count.checked_add(1).expect("bounded connector registry");
            }
        }
        assert_eq!(count, 1, "connector numbers are unique");
    }
}

impl Core {
    /// Earliest absolute wall time among retained effect reply rights
    /// (`domain/engine.md`, 4.4). Stale rights remain visible until the
    /// existing effect-deadline event consumes them; this query changes nothing.
    #[must_use]
    pub fn effect_deadline(&self) -> Option<Wall> {
        effects::next_deadline(self)
    }

    /// Whether a live task still holds this exact fenced attempt.
    #[must_use]
    pub fn current_proof(&self, task: u64, attempt: u64) -> bool {
        match self.proofs.get(&task) {
            Some(proof) => proof.attempt == attempt,
            None => false,
        }
    }

    /// Retain the canonical terminal of an unpriced attempt before the task
    /// child makes its next lifecycle decision.
    pub fn remember_unpriced_terminal(&mut self, run: Token, attempt: Token, end: tasks::End) {
        if self.committed_terminal(run.raw(), attempt.raw()).is_some() {
            return;
        }
        let proof = self.proofs.get_mut(&run.raw()).expect("current fleet terminal has pre-reserved proof");
        assert!(proof.attempt == attempt.raw(), "fleet terminal belongs to current proof");
        let cumulative = match proof.turn {
            Some(turn) => turn.cumulative,
            None => 0,
        };
        proof.terminal = Some(TerminalRecord { task: run.raw(), attempt: attempt.raw(), cumulative, end });
    }

    pub(crate) fn committed_terminal(&self, task: u64, attempt: u64) -> Option<TerminalRecord> {
        if self.tasks.task(task)?.last_answer != Some(attempt) {
            return None;
        }
        let proof = self.proofs.get(&task)?;
        if proof.attempt != attempt {
            return None;
        }
        proof.terminal.clone()
    }

    /// Retain one newly accepted named call decision. The application saves
    /// its store row and holds the outward answer behind the same commit.
    pub fn record_call(&mut self, key: CallKey, part: CallPart) {
        assert!(self.call_parts.insert(key, part) == Ok(None), "named call room reserved");
    }

    /// Restore one live named call only after its current task proof was
    /// restored. The connector's separately owned answer is validated by the
    /// application before it asks the core to retain this part.
    pub fn restore_call(&mut self, key: CallKey, part: CallPart) -> bool {
        let deployment = self.counters.deployment();
        let valid = key.task != 0
            && key.attempt != 0
            && key.completion != 0
            && key.task <= deployment.tasks
            && key.attempt <= deployment.runs
            && match self.proofs.get(&key.task) {
                Some(proof) => proof.attempt >= key.attempt,
                None => false,
            };
        valid && self.call_parts.insert(key, part) == Ok(None)
    }

    /// Forget named calls whose answer entered a committed later turn. The
    /// application erases these keys in the same decision and drops any
    /// connector-owned answer for them.
    pub fn retire_calls(&mut self, task: u64, attempt: u64, turn: u32, room: u32) -> Box<[CallKey]> {
        let mut retired = List::with_capacity(room);
        for (&key, _) in &self.call_parts {
            if key.task == task && (key.attempt < attempt || (key.attempt == attempt && key.completion < turn)) {
                retired.push(key).expect("all retained call names fit their configured bound");
            }
        }
        for &key in &retired {
            self.call_settled.remove(&key);
            match self.call_parts.remove(&key) {
                Some(CallPart::Effect { entry, .. }) => {
                    self.effect_replies.remove(&entry);
                }
                Some(
                    CallPart::Connector { .. }
                    | CallPart::EffectRefused(_)
                    | CallPart::EffectDenied { .. }
                    | CallPart::ToolDenied { .. }
                    | CallPart::EscalationDecided { .. }
                    | CallPart::EscalationRefused(_)
                    | CallPart::Proposed { .. }
                    | CallPart::ProposalDecided { .. }
                    | CallPart::SettingsRefused(_)
                    | CallPart::ProposalRefused(_)
                    | CallPart::HoldOwner { .. }
                    | CallPart::Controlled
                    | CallPart::ControlRefused(_)
                    | CallPart::ControlDenied { .. }
                    | CallPart::Sent { .. }
                    | CallPart::Introduced
                    | CallPart::MessageRefused(_)
                    | CallPart::Subscribed { .. }
                    | CallPart::Unsubscribed
                    | CallPart::SubscriptionRefused(_)
                    | CallPart::Delegated(_)
                    | CallPart::DelegationDenied { .. }
                    | CallPart::DelegationRefused(_)
                    | CallPart::NoteWritten { .. }
                    | CallPart::NoteRecalled { .. }
                    | CallPart::NoteRefused(_)
                    | CallPart::Revised { .. }
                    | CallPart::Moved { .. }
                    | CallPart::Again { .. }
                    | CallPart::PassedUp { .. }
                    | CallPart::Unavailable,
                )
                | None => {}
            }
        }
        retired.into_boxed()
    }

    /// Allocate the eight children and the bounded live core tables.
    /// # Errors
    /// A configured worker count or name exceeds the priced membership bounds.
    #[expect(clippy::too_many_lines, reason = "construct all bounded children, bindings and subject marks together")]
    pub fn new(mut config: Config, limits: &Limits) -> Result<Core, Problem> {
        let worker_names = memory::problems(core::mem::replace(&mut config.worker_names, Box::new([])), limits)?;
        assert!(limits.people.words <= limits.tasks.message_bytes, "party words fit one run message");
        assert!(limits.tasks.message_bytes >= 24, "projection refusal news fits one run message");
        assert!(config.settings.tools.valid(), "each engine tool has one configured authority bit");

        assert!(
            config.connectors.len() <= usize::try_from(limits.connectors).expect("connector count fits usize"),
            "core prices every configured connector number"
        );
        assert_connector_registry(&config.connectors);
        for grant in &config.grants {
            assert!(config.connectors.contains(&grant.connector), "acts name configured connectors");
        }
        assert!(
            config.settings.resume_bytes > 0 && config.settings.resume_bytes <= limits.resume_bytes,
            "core prices its transcript retention"
        );
        assert!(
            config.settings.tail_bytes <= limits.brief_core_budgets.tail,
            "core prices its transcript text retention"
        );
        charters::assert_catalogue(&config.settings);
        assert!(
            config.seed_projects.len() <= usize::try_from(limits.authority.projects).expect("project bound"),
            "core prices seed projects"
        );
        assert!(config.defaults.policies.capacity() <= limits.authority.projects, "core prices configured policies");
        assert!(
            config.seed_projects.len() < usize::try_from(limits.people.projects).expect("people project bound"),
            "people prices seed rosters and the deployment's"
        );
        for (index, project) in config.seed_projects.iter().enumerate() {
            assert!(project.number != 0, "seed project numbers start at one");
            assert!(people::name_rule(&project.name).is_none(), "seed project names are in shape");
            for previous in config.seed_projects.iter().take(index) {
                assert!(project.number != previous.number, "seed project numbers are unique");
                assert!(!people::same_name(&project.name, &previous.name), "seed project names are unique");
            }
            let value =
                policy::snapshot(&project.policy.policy, &project.policy.permissions, &project.policy.templates)
                    .expect("seed policy snapshot");
            assert!(
                match people::policy_bytes(&value) {
                    Some(bytes) => bytes <= limits.policy_bytes,
                    None => false,
                },
                "core prices seed policies"
            );
        }
        let fleet_needs = fleet_needs::configured(&config, limits);
        let current = settings::Current::new(config.defaults, limits);
        for project in &config.seed_projects {
            assert!(
                project.policy.templates.is_empty()
                    || templates::valid(&current, &config.settings, &project.policy.templates),
                "merged template choices fit their effective executors"
            );
        }
        let authority = authority::Domain::new(current.rules().clone(), limits.authority).expect("configured rules");
        let room_max = room_max(limits).expect("validated immutable core limits");
        Ok(Core {
            refusal_limits: *limits,
            room_max,
            run_inbox: limits.tasks.run_inbox,
            fill: 0,
            workers_in_process: config.workers_in_process,
            stopping: false,
            draining: None,
            question_activity: Map::with_capacity(inbox::question_capacity(limits).expect("validated question bound")),
            census_progress: Map::with_capacity(limits.people.people),
            census_position: None,
            ended_marks: Map::with_capacity(limits.people.pending),
            archived_chats: Map::with_capacity(
                limits
                    .people
                    .people
                    .checked_mul(limits.people.inbox_entries)
                    .expect("validated archive cache product")
                    .checked_add(1)
                    .expect("validated archive cache bound"),
            ),
            proposed_input: None,
            settings_offering: None,
            settings_proposals: Map::with_capacity(limits.settings.proposals),
            settings_input: None,
            key_input: None,
            keys: Map::with_capacity(limits.people.pending),
            application_input: None,
            task_capacity_mark: 0,
            settings_holders: 0,
            applications: Map::with_capacity(limits.people.pending),
            restart: restart::Restart::Cold,
            restart_pending: false,
            restart_attempts: limits.fleet.attempts,
            counters: Counters::bootstrap(config.deployment),
            seed_owners: config.owners.clone(),
            seed_projects: config.seed_projects,
            view_subjects: Map::with_capacity(limits.views.watchers),
            view_names: skein_lib::Slab::with_capacity(limits.views.watchers),
            changed_views: skein_lib::Set::with_capacity(limits.views.watchers),
            reads: skein_lib::Slab::with_capacity(limits.load_slots),
            dependency_results: Map::with_capacity(limits.tasks.tasks),
            made: Map::with_capacity(limits.people.pending),
            goal_routes: Map::with_capacity(limits.people.pending),
            delegating: Map::with_capacity(limits.fleet.calls),
            routing_people_proposals: Map::with_capacity(limits.people.pending),
            ending_positions: Map::with_capacity(limits.tasks.tasks),
            saying: Map::with_capacity(limits.people.pending),
            moving: Map::with_capacity(limits.people.pending),
            person_tasks: Map::with_capacity(limits.people.pending),
            person_escalations: Map::with_capacity(limits.people.pending),
            result_reads: Map::with_capacity(limits.tasks.tasks),
            next_result_request: 1,
            creating_inputs: Map::with_capacity(limits.people.pending),
            creating: Map::with_capacity(limits.people.pending),
            resource_roles: Map::with_capacity(limits.tasks.resource_reports),
            resource_roles_full: false,
            awaiting_resources: Map::with_capacity(limits.tasks.tasks),
            creating_delegates: Map::with_capacity(limits.call_records),
            creating_procedures: Map::with_capacity(limits.tasks.tasks),
            relaying: Map::with_capacity(limits.tasks.tasks),
            projects: List::with_capacity(limits.authority.projects),

            names: Map::with_capacity(limits.authority.projects),
            permission_roles: Map::with_capacity(limits.authority.projects),
            template_overrides: Map::with_capacity(limits.authority.projects),
            connectors: config.connectors,
            pending_calls: Map::with_capacity(limits.call_records),
            call_parts: Map::with_capacity(limits.call_records),
            call_settled: Map::with_capacity(limits.call_records),
            routing_calls: Map::with_capacity(limits.fleet.calls),
            effect_flights: Map::with_capacity(
                limits
                    .call_records
                    .checked_add(limits.tasks.tasks)
                    .expect("call and procedure flights")
                    .checked_add(limits.people.pending)
                    .expect("effect flights"),
            ),
            effect_replies: Map::with_capacity(limits.call_records),
            closing_connectors: Map::with_capacity(
                limits.tasks.tasks.checked_mul(limits.connectors).expect("bounded closing connectors"),
            ),
            projections: Map::with_capacity(limits.tasks.tasks),
            proposing_effects: Map::with_capacity(limits.call_records),
            procedure_proposals: Map::with_capacity(limits.tasks.tasks),
            note_routes: Map::with_capacity(2),
            pending_note_briefs: Map::with_capacity(limits.brief.briefs),
            next_note_owner: 1,
            run_now: skein_lib::Time::ZERO,
            run_wall: Wall::EPOCH,
            contexts: Map::with_capacity(limits.tasks.tasks),
            workspace_pending: Map::with_capacity(limits.tasks.tasks),
            workspace_credentials: Map::with_capacity(limits.tasks.tasks),
            transcripts: Map::with_capacity(limits.tasks.tasks),
            proofs: Map::with_capacity(limits.tasks.tasks),
            replaced: Map::with_capacity(limits.tasks.tasks),
            restoring_proofs: Map::with_capacity(limits.tasks.tasks),
            unreported_restored: Map::with_capacity(limits.tasks.tasks),
            claiming: Map::with_capacity(limits.tasks.tasks),
            due: Queue::with_capacity(limits.tasks.tasks),
            adopted: Queue::with_capacity(limits.tasks.tasks.checked_mul(2).expect("adoption room representable")),
            tasks: tasks::Domain::new(
                &settings::task_limits(limits),
                config.seed,
                current.newest(),
                charters::models(&current, &config.settings, limits),
                charters::efforts(&config.settings),
            ),
            current,
            held: settings::Held::new(limits),
            hub_turns: config.hub_turns,
            settings_restore: settings::restart::Restore::new(limits),
            restore_failure: None,
            authority,
            people: people::Domain::new(
                &limits.people,
                config.owners,
                config.settings.deployment_provider,
                config.grants,
            ),
            fleet: {
                let mut fleet = fleet::Domain::new(&settings::fleet_limits(limits), fleet_needs, {
                    let mut names = List::with_capacity(tasks::MODELS);
                    for model in &config.settings.models {
                        names.push(model.name.clone()).expect("catalogue bound");
                    }
                    names.into_boxed()
                });
                fleet.configure_workers(worker_names);
                fleet
            },
            accounts: accounts::Domain::new(&limits.accounts, config.settings.llm_providers.clone()),
            brief: brief::GatherDomain::new(&limits.brief),
            notes: notes::Domain::new(&limits.notes),
            views: views::Domain::new(&limits.views),
            settings: config.settings,
        })
    }

    /// Reclaim retired child slots at the application's iteration boundary.
    pub fn reclaim(&mut self) {
        self.tasks.reclaim();
        self.people.reclaim();
        self.fleet.reclaim();
        self.views.reclaim();
        self.view_names.reclaim();
        self.reads.reclaim();
    }

    /// Discard expendable child observations without changing a decision.
    pub fn drain_facts(&mut self, limits: &Limits) {
        for _ in 0..limits.tasks.facts {
            let _fact = self.tasks.pop_fact();
        }
        for _ in 0..limits.people.facts {
            let _fact = self.people.pop_fact();
        }
        for _ in 0..limits.fleet.facts {
            let _fact = self.fleet.pop_fact();
        }
        for _ in 0..limits.accounts.facts {
            let _fact = self.accounts.pop_fact();
        }
        for _ in 0..limits.views.facts {
            let _fact = self.views.pop_fact();
        }
    }

    /// Pure, allocation-free finishing derivation (domain/root.md, section 3).
    /// Child-held waits and future alarms alone owe no finishing. Before
    /// omitting a closure the root must also check its current-clock timers.
    #[must_use]
    pub fn finish_owed(&self) -> bool {
        self.counters.dirty()
            || (self.draining == Some(true) && self.view_subjects.is_empty())
            || !self.changed_views.is_empty()
            || projections::owed(self)
            || routing::watch_marks::retention_owed(self)
            || routing::watch_marks::endings_owed(self)
    }

    /// Idle fence including journal, routing and finishing (domain/root.md,
    /// section 3). A host may still hold a live attempt. False can mean a child
    /// awaits input; use `finish_owed` for closure after checking due timers.
    #[must_use]
    pub fn quiescent(&self, journal_idle: bool) -> bool {
        self.counters.quiescent(journal_idle)
            && self.reads.is_empty()
            && self.dependency_results.is_empty()
            && self.result_reads.is_empty()
            && self.pending_calls.is_empty()
            && self.effect_flights.is_empty()
            && self.proposing_effects.is_empty()
            && self.procedure_proposals.is_empty()
            && self.routing_calls.is_empty()
            && self.note_routes.is_empty()
            && self.pending_note_briefs.is_empty()
            && self.routing_people_proposals.is_empty()
            && self.awaiting_resources.is_empty()
            && self.made.is_empty()
            && self.creating_inputs.is_empty()
            && self.goal_routes.is_empty()
            && self.delegating.is_empty()
            && self.ending_positions.is_empty()
            && self.saying.is_empty()
            && self.moving.is_empty()
            && self.person_tasks.is_empty()
            && self.relaying.is_empty()
            && self.adopted.is_empty()
            && self.due.is_empty()
            && self.claiming.is_empty()
            && self.replaced.is_empty()
            && self.contexts.is_empty()
            && self.workspace_credentials.is_empty()
            && self.transcripts.is_empty()
            && self.restoring_proofs.is_empty()
            && self.unreported_restored.is_empty()
            && self.brief.is_idle()
            && !self.fleet.is_ready()
            && self.fleet.turns() == 0
            && self.fleet.calls() == 0
            && self.fleet.next_deadline().is_none()
            && !self.accounts.waiting()
            && !self.finish_owed()
    }

    /// Restore one current claim only when it matches the live task's
    /// transient correlation. An archive row can never enter this live map.
    #[expect(clippy::manual_let_else, reason = "the step-code subset uses exhaustive matches")]
    pub fn restore_proof(&mut self, proof: RunProof, result_bytes: u32) -> bool {
        let expected = match self.restoring_proofs.remove(&proof.task) {
            Some(expected) => expected,
            None => return false,
        };
        let turn = match proof.turn {
            Some(turn) => turn.turn,
            None => 0,
        };
        if proof.accounts.is_empty() || proof.accounts.len() > usize::try_from(tasks::MODELS).expect("model bound") {
            return false;
        }
        let Some(channel) = proof.placement else { return false };
        if (proof.host == fleet::HostKind::Engine) != (channel == Token::new(0)) {
            return false;
        }
        if proof.account_terminal && proof.account_failure.is_none() {
            return false;
        }
        if let Some(source) = proof.account_failure {
            let mut recorded = false;
            for binding in &proof.accounts {
                if binding.source == source {
                    recorded = true;
                }
            }
            if !recorded {
                return false;
            }
        }
        for (at, binding) in proof.accounts.iter().enumerate() {
            for earlier in proof.accounts.iter().take(at) {
                if earlier.provider == binding.provider {
                    return false;
                }
            }
            if let Some(source) = self.accounts.source(binding.source)
                && source.account != binding.account
            {
                return false;
            }
            if let Some(account) = self.accounts.account(binding.account)
                && account.provider() != binding.provider
            {
                return false;
            }
        }
        if proof.credentials.len()
            > usize::try_from(self.accounts.credential_limit()).expect("u32 credential limit fits usize")
            || proof.attempt != expected.attempt
            || turn != expected.turn
            || proof.outstanding.len() > usize::try_from(self.run_inbox).expect("run inbox fits usize")
            || !valid_proof(&proof, &expected, result_bytes)
        {
            return false;
        }
        self.proofs.insert(proof.task, proof).is_ok()
    }
}

fn valid_proof(proof: &RunProof, expected: &RestoringProof, result_bytes: u32) -> bool {
    if proof.task == 0 || proof.attempt == 0 || proof.conversation == 0 || proof.conversation > proof.attempt {
        return false;
    }
    if proof.offered == Some(0) {
        return false;
    }
    let last_turn = match proof.turn {
        Some(turn) => turn.turn,
        None => 0,
    };
    if let Some(window) = proof.window
        && (window.turn == 0
            || (window.attempt, window.turn) < (proof.conversation, 1)
            || (window.attempt, window.turn) > (proof.attempt, last_turn))
    {
        return false;
    }
    let last_read = match proof.turn {
        Some(turn) => turn.read,
        None => None,
    };
    for (at, &number) in proof.outstanding.iter().enumerate() {
        if number == 0 || Some(number) > proof.offered || Some(number) <= last_read {
            return false;
        }
        for earlier in proof.outstanding.iter().take(at) {
            if *earlier == number {
                return false;
            }
        }
    }
    let spent = match proof.turn {
        Some(turn) => {
            let read_valid = match turn.read {
                Some(number) => match proof.offered {
                    Some(high) => number != 0 && number <= high,
                    None => false,
                },
                None => true,
            };
            if turn.turn == 0 || !read_valid || turn.cumulative > expected.run_spent {
                return false;
            }
            turn.cumulative
        }
        None => 0,
    };
    match &proof.terminal {
        Some(terminal) => {
            terminal.task == proof.task
                && terminal.attempt == proof.attempt
                && terminal.cumulative == expected.run_spent
                && expected.last_answer == Some(proof.attempt)
                && match tasks::terminal_bytes(&terminal.end) {
                    Some(bytes) => bytes <= u64::from(result_bytes).checked_mul(2).expect("result bound representable"),
                    None => false,
                }
        }
        None => spent == expected.run_spent && expected.last_answer != Some(proof.attempt),
    }
}

#[cfg(test)]
mod tests;
