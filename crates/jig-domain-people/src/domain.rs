//! Bounded people state, task-derived inbox counts, request admission and restoration
//! (domain/people.md, sections 3–6). `step` handles root events; `fire` expires sign-ins.
//! The child keeps identities, roles, sign-ins and keyed outcomes, routes each
//! admitted flight once and waits for `Decided`. Its bounded volatile need references
//! count every live task projection independently of the entry cache. Root owns policy, task creation
//! and durability; this child never performs IO or retains credential secrets.

#[path = "ended.rs"]
mod ended;
#[path = "inbox.rs"]
mod inbox;
pub use ended::EndedMark;
pub(crate) mod acts;
mod parties;
use inbox::{conversation_seen_decision, mark_seen, prune_unheld_roles, replace_waiting};

use crate::{
    Again, Ask, Entry, EntryKind, Event, Fact, Holding, Identity, IdentityKey, InitialOwner, Key, Kind, LimitKind,
    Limits, Outcome, ProjectRole, Refusal, Reply, Request, RequestKey, ResultRef, Role, Seed, Seen, Stored, Update,
    Whom,
};
use alloc::boxed::Box;
use skein_lib::{Deadlines, Env, Id, List, Map, Queue, ReplyTo, Set, Slab, Time, Token, Wall};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct SignIn {
    person: u64,
    expires: Wall,
    due: Time,
}

#[derive(PartialEq, Eq, Hash, Debug)]
pub(crate) struct Answered {
    ask: Ask,
    models: Box<[Box<[u8]>]>,
    outcome: Outcome,
    at: Wall,
    due: Time,
}

#[derive(Debug)]
pub(crate) struct PartyPending {
    part: Option<Ask>,
    key: RequestKey,
    ask: Ask,
    models: Box<[Box<[u8]>]>,
    replies: List<ReplyTo>,
}

#[derive(Debug)]
#[expect(clippy::large_enum_variant, reason = "one shared slab prices the largest admitted flight inline")]
pub(crate) enum Pending {
    Party(PartyPending),
    Act(acts::PendingAct),
    /// The answered move keeps its slab slot and key index for Written's answer.
    MoveOut {
        holder: crate::AccountHolder,
        account: u32,
        number: u32,
        source: u32,
    },
}
impl Pending {
    pub(super) fn party(&self) -> Option<&PartyPending> {
        match self {
            Self::Party(party) => Some(party),
            Self::Act(_) | Self::MoveOut { .. } => None,
        }
    }
    pub(super) fn party_mut(&mut self) -> Option<&mut PartyPending> {
        match self {
            Self::Party(party) => Some(party),
            Self::Act(_) | Self::MoveOut { .. } => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Phase {
    Restoring,
    Ready,
    Failed,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct Need {
    project: u32,
    whom: Whom,
    kind: EntryKind,
    at: Wall,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct ChatTotal {
    party: u64,
    project: u32,
    last_reply: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct Recent {
    update: Update,
    gone: bool,
}

/// Bounded secret-free identities, sign-ins/deadlines, project holdings, completed keys and
/// volatile flights and task-derived inbox references; knows no task content and starts restoring.
/// (domain/people.md, sections 2–6).
#[derive(Debug)]
pub struct Domain {
    phase: Phase,
    deployment_provider: u16,
    people: Map<u64, Identity>,
    results_seen: Map<u64, u64>,
    conversations_seen: Map<(u64, u64), u64>,
    replies: Map<u64, Box<[Entry]>>,
    chat_totals: Map<u64, ChatTotal>,
    chat_updates: Map<(u64, u32), u128>,
    result_census: Map<(u64, u32), Box<[ResultRef]>>,
    recent: Map<u64, Box<[Recent]>>,
    unread: Map<u64, Box<[ResultRef]>>,
    waiting: Map<Whom, Box<[Entry]>>,
    needs: Map<u64, Box<[Need]>>,
    release_proposals: Map<u64, Entry>,
    identities: Map<IdentityKey, u64>,
    sign_ins: Map<u64, SignIn>,
    alarms: Deadlines<u64>,
    expiries: Set<(Wall, u64)>,
    roles: Map<u32, Box<[Holding]>>,
    /// The deployment's first owners, who decide its LLM accounts (accounts.md, 3.2) for as long
    /// as it runs; their first sign-ins seed their roles only while `seeding`.
    owners: Box<[InitialOwner]>,
    /// The deployment's first start, until the core begins a restore (protocol/configuration.md,
    /// 3: owners seed the first start only).
    seeding: bool,
    answers: Map<RequestKey, Answered>,
    acted: Map<crate::ActKey, acts::Acted>,
    act_alarms: Deadlines<crate::ActKey>,
    act_flights: Map<crate::ActKey, Id<Pending>>,
    grants: Box<[crate::ActGrant]>,
    answer_alarms: Deadlines<RequestKey>,
    pending: Slab<Pending>,
    flights: Map<RequestKey, Id<Pending>>,
    facts: Queue<Fact>,
    lost: u64,
    room_refusals: Queue<crate::CapacityRefusal>,
    room_refusals_lost: u64,
}

impl Domain {
    pub(super) fn party_pending(&self, id: Id<Pending>) -> Option<&PartyPending> {
        self.pending.get(id)?.party()
    }
    /// The admitted ask retained for one root route, including an asynchronous lookup.
    #[must_use]
    pub fn pending_ask(&self, request: Token) -> Option<&Ask> {
        let flight = self.party_pending(Id::from_token(request))?;
        Some(&flight.ask)
    }

    fn party_pending_mut(&mut self, id: Id<Pending>) -> Option<&mut PartyPending> {
        self.pending.get_mut(id)?.party_mut()
    }

    /// Release a cancelled move's volatile Written place (protocol/llm.md, 5.6).
    /// `None` cancels all moves when the accounts component closes.
    pub fn cancel_moves(&mut self, source: Option<u32>) {
        for _ in 0..self.flights.capacity() {
            let mut found = None;
            for (key, id) in &self.flights {
                if let Pending::MoveOut { source: held, .. } = self.pending.get(*id).expect("indexed pending")
                    && (source.is_none() || source == Some(*held))
                {
                    found = Some((*key, *id));
                    break;
                }
            }
            let Some((key, id)) = found else { break };
            assert!(self.flights.remove(&key) == Some(id), "one cancelled move index");
            self.pending.retire(id);
        }
    }

    /// Create restoring state with capacities from validated `limits` and at most
    /// `limits.initial_owners` owned bootstrap entries. Panics on unrepresentable/invalid limits,
    /// oversized owners or duplicate project/identity pairs; project roles must be initialized
    /// before sign-in.
    #[must_use]
    pub fn new(
        limits: &Limits,
        owners: Box<[InitialOwner]>,
        deployment_provider: u16,
        grants: Box<[crate::ActGrant]>,
    ) -> Domain {
        assert!(crate::worst_case(limits).is_some(), "people limits are valid");
        assert!(
            owners.len() <= usize::try_from(limits.initial_owners).expect("u32 fits usize"),
            "configured owners fit"
        );
        for (at, owner) in owners.iter().enumerate() {
            assert!(
                !owner.identity.subject.is_empty()
                    && owner.identity.subject.len() <= usize::try_from(limits.identity_bytes).expect("u32 fits usize")
                    && owner.identity.provider != deployment_provider,
                "configured first owners are bounded people"
            );
            for earlier in owners.iter().take(at) {
                assert!(
                    owner.project != earlier.project || owner.identity != earlier.identity,
                    "one initial owner identity per project"
                );
            }
        }
        assert!(
            grants.len() <= usize::try_from(limits.act_grants).expect("u32 fits usize"),
            "configured act grants fit"
        );
        for (at, grant) in grants.iter().enumerate() {
            assert!(grant.may.len() <= crate::ActKind::ALL.len(), "bounded configured act kinds");
            for earlier in grants.iter().take(at) {
                assert!(earlier.connector != grant.connector, "unique connector grants");
            }
            for (at, kind) in grant.may.iter().enumerate() {
                for earlier in grant.may.iter().take(at) {
                    assert!(earlier != kind, "unique configured act kinds");
                }
            }
        }
        Domain {
            phase: Phase::Restoring,
            deployment_provider,
            people: Map::with_capacity(limits.people),
            results_seen: Map::with_capacity(limits.people),
            conversations_seen: Map::with_capacity(
                limits.people.checked_mul(limits.inbox_tasks).expect("bounded conversation positions"),
            ),
            replies: Map::with_capacity(limits.inbox_tasks),
            chat_totals: Map::with_capacity(limits.inbox_tasks),
            chat_updates: Map::with_capacity(limits.people.checked_mul(limits.projects).expect("project reply census")),
            result_census: Map::with_capacity(
                limits.people.checked_mul(limits.projects).expect("project result census"),
            ),
            recent: Map::with_capacity(limits.people),
            unread: Map::with_capacity(limits.people),
            needs: Map::with_capacity(limits.inbox_tasks),
            release_proposals: Map::with_capacity(limits.inbox_tasks),
            waiting: Map::with_capacity(
                limits
                    .people
                    .checked_add(limits.projects.checked_mul(limits.holdings).expect("role cache room"))
                    .expect("inbox cache room"),
            ),
            identities: Map::with_capacity(limits.people),
            sign_ins: Map::with_capacity(limits.sign_ins),
            alarms: Deadlines::with_capacity(limits.sign_ins),
            expiries: Set::with_capacity(limits.sign_ins),
            roles: Map::with_capacity(limits.projects),
            owners,
            seeding: true,
            answers: Map::with_capacity(limits.requests),
            acted: Map::with_capacity(limits.requests),
            act_alarms: Deadlines::with_capacity(limits.requests),
            act_flights: Map::with_capacity(limits.pending),
            grants,
            answer_alarms: Deadlines::with_capacity(limits.requests),
            pending: Slab::with_capacity(limits.pending),
            flights: Map::with_capacity(limits.pending),
            facts: Queue::with_capacity(limits.facts),
            lost: 0,
            room_refusals: Queue::with_capacity(limits.room_refusals),
            room_refusals_lost: 0,
        }
    }

    /// Pure scheduler query of the earliest armed monotonic sign-in expiry, or `None`; no
    /// expiration or output occurs here.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Time> {
        let ordinary = match (self.alarms.next(), self.answer_alarms.next()) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (Some(left), None) => Some(left),
            (None, Some(right)) => Some(right),
            (None, None) => None,
        };
        match (ordinary, self.act_alarms.next()) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (Some(at), None) | (None, Some(at)) => Some(at),
            (None, None) => None,
        }
    }

    /// Earliest saved wall expiry; translate beside the original monotonic
    /// deadline at the caller's current injected clocks (people.md, section 3).
    #[must_use]
    pub fn next_wall_expiry(&self) -> Option<Wall> {
        let (expires, _) = self.expiries.first()?;
        Some(*expires)
    }

    /// Timeless table membership only; makes no claim of clock validity.
    /// Authentication must use `person` with the current clocks.
    #[must_use]
    pub fn sign_in_person(&self, sign_in: u64) -> Option<u64> {
        Some(self.sign_ins.get(&sign_in)?.person)
    }

    /// Pure scheduler query returning whether any armed expiry is at or before supplied `now`; no
    /// state change or output.
    #[must_use]
    pub fn is_due(&self, now: Time) -> bool {
        match self.next_deadline() {
            Some(at) => at <= now,
            None => false,
        }
    }

    /// Reclaim retired flight slots at the parent's iteration reclaim point after their outputs
    /// have been routed; no request or client reply is emitted.
    pub fn reclaim(&mut self) {
        self.pending.reclaim();
    }

    /// Remove one optional content-free observation from the bounded fact queue; keeping/dropping
    /// facts changes no decision or persistence output.
    pub fn pop_fact(&mut self) -> Option<Fact> {
        self.facts.pop()
    }

    /// Return the saturating count of observations dropped when `Limits::facts` was full;
    /// diagnostic only, not a lifecycle acknowledgement.
    #[must_use]
    pub const fn facts_lost(&self) -> u64 {
        self.lost
    }

    /// Recent permanent room refusals from retained keyed answers, together with
    /// bounded unsaved entrance pressure, newest wall time first. Reads copy no ask
    /// payloads and change no diagnostic, replay or expiry state.
    #[must_use]
    pub fn capacity_refusals(&self) -> List<crate::CapacityRefusal> {
        let capacity =
            self.answers.capacity().checked_add(self.room_refusals.capacity()).expect("accepted room limits");
        let mut ordered = Map::with_capacity(capacity);
        let mut ordinal = 0_u32;
        for (key, answer) in &self.answers {
            if let Some(cause) = crate::capacity::permanent(&answer.outcome) {
                let action = crate::capacity::action(&answer.ask, key.person);
                let value = crate::CapacityRefusal {
                    key: *key,
                    project: crate::capacity::project(&answer.ask),
                    action,
                    at: answer.at,
                    cause,
                };
                ordered.insert((value.at, ordinal), value).expect("bounded permanent refusal snapshot");
                ordinal = ordinal.checked_add(1).expect("bounded refusal ordinal");
            }
        }
        for value in &self.room_refusals {
            ordered.insert((value.at, ordinal), *value).expect("bounded entrance refusal snapshot");
            ordinal = ordinal.checked_add(1).expect("bounded refusal ordinal");
        }
        let mut result = List::with_capacity(capacity);
        for (_, value) in ordered.iter().rev() {
            result.push(*value).expect("bounded refusal view");
        }
        result
    }

    /// Unsaved entrance-pressure records evicted from the bounded live history.
    /// Permanent answers instead follow their configured request retention.
    #[must_use]
    pub const fn capacity_refusals_lost(&self) -> u64 {
        self.room_refusals_lost
    }

    /// Pure bounded membership lookup used by root escalation routing and authenticated named
    /// reads; caller separately authenticates identities. Available during restore,
    /// allocating/emitting/mutating nothing.
    #[must_use]
    pub fn role(&self, person: u64, project: u32) -> Option<Role> {
        role(self, person, project)
    }

    /// Borrow one retained authenticated party's display identity; no secret is known.
    #[must_use]
    pub fn identity(&self, party: u64) -> Option<&Identity> {
        if !self.ready() {
            return None;
        }
        self.people.get(&party)
    }

    /// Classify a retained party by its authenticated provider.
    #[must_use]
    pub fn kind(&self, party: u64) -> Option<Kind> {
        let identity = self.identity(party)?;
        Some(self.kind_of(identity))
    }

    fn kind_of(&self, identity: &Identity) -> Kind {
        if identity.key.provider == self.deployment_provider { Kind::Service } else { Kind::Person }
    }

    /// Ordinary project roles of a retained party, bounded by the role table; no admission changes.
    /// The deployment’s roster is read separately through `role` (people.md, 3).
    #[must_use]
    pub fn project_roles(&self, party: u64) -> Option<List<ProjectRole>> {
        self.identity(party)?;
        let mut result = List::with_capacity(self.roles.capacity());
        for (project, holdings) in &self.roles {
            for holding in holdings {
                if *project != crate::DEPLOYMENT && holding.person == party {
                    result.push(ProjectRole { project: *project, role: holding.role }).expect("one role per project");
                }
            }
        }
        Some(result)
    }

    /// Whether another project roster fits, including the deployment roster.
    #[must_use]
    pub fn project_room(&self) -> bool {
        self.roles.len() < self.roles.capacity()
    }

    /// Borrow the retained role holders of one project without changing admission.
    #[must_use]
    pub fn project_members(&self, project: u32) -> Option<&[Holding]> {
        if !self.ready() {
            return None;
        }
        Some(self.roles.get(&project)?)
    }

    /// Resolve a provider-authenticated collaborator to this project's role.
    #[must_use]
    pub fn role_for_identity(&self, key: IdentityKey, project: u32) -> Option<(u64, Role)> {
        let person = *self.identities.get(&key)?;
        Some((person, role(self, person, project)?))
    }

    /// Check that one whole adoption seed can commit without displacing current roles.
    #[must_use]
    pub fn can_seed(&self, limits: &Limits, project: u32, collaborators: &[Seed]) -> bool {
        if !self.ready() || collaborators.len() > usize::try_from(limits.holdings).expect("u32 fits usize") {
            return false;
        }
        let Some(holdings) = self.roles.get(&project) else { return false };
        let mut new_people = 0_usize;
        let mut new_roles = 0_usize;
        for (at, seed) in collaborators.iter().enumerate() {
            if seed.identity.subject.is_empty()
                || seed.identity.subject.len() > usize::try_from(limits.identity_bytes).expect("u32 fits usize")
                || seed.identity.provider == self.deployment_provider
                || seed.candidate == 0
                || self.people.contains_key(&seed.candidate)
            {
                return false;
            }
            for earlier in collaborators.iter().take(at) {
                if earlier.identity == seed.identity || earlier.candidate == seed.candidate {
                    return false;
                }
            }
            match self.identities.get(&seed.identity) {
                Some(person) => {
                    if !has_person(holdings, *person) {
                        new_roles = new_roles.saturating_add(1);
                    }
                }
                None => {
                    new_people = new_people.saturating_add(1);
                    new_roles = new_roles.saturating_add(1);
                }
            }
        }
        match (
            self.people.len().checked_add(u32::try_from(new_people).ok().unwrap_or(u32::MAX)),
            holdings.len().checked_add(new_roles),
        ) {
            (Some(people), Some(roles)) => {
                people <= limits.people && roles <= usize::try_from(limits.holdings).expect("u32 fits usize")
            }
            (None, _) | (_, None) => false,
        }
    }

    /// End the first owners' seeding when the core begins restore (protocol/configuration.md,
    /// 3: owners seed the first start only); they stay the deployment's first owners.
    pub fn end_seeding(&mut self) {
        self.seeding = false;
    }

    /// Pure lookup of whether the bounded role table contains `project`, including an empty
    /// holdings set. Available during restoration so the root preserves restored roles and seeds
    /// only missing bootstrap projects; it implies neither readiness nor person membership. The
    /// table is bounded by `Limits::projects`; this query allocates nothing, emits no output and
    /// grants no role.
    #[must_use]
    pub fn has_project(&self, project: u32) -> bool {
        self.roles.contains_key(&project)
    }

    /// Pure authenticated lookup for the root-issued `sign_in` at supplied `now` and `wall`.
    /// Returns `None` before readiness, for an absent sign-in, or when either its projected
    /// monotonic deadline is at or before `now` or its saved wall expiry is at or before `wall`. A
    /// backward wall correction cannot revive a monotonic-expired session, even before fire.
    /// Otherwise returns the stable requester person number. The sign-in table is bounded by
    /// `Limits::sign_ins`; this read allocates nothing, emits no output and changes no sign-in or
    /// role state. Root uses it for named task result reads.
    #[must_use]
    pub fn person(&self, sign_in: u64, now: Time, wall: Wall) -> Option<u64> {
        if !self.ready() {
            return None;
        }
        let session = self.sign_ins.get(&sign_in)?;
        if session.expires <= wall || session.due <= now {
            return None;
        }
        Some(session.person)
    }

    /// Pure query of completed, successful restoration; false during restoring and permanently
    /// after restore failure.
    #[must_use]
    pub fn ready(&self) -> bool {
        match self.phase {
            Phase::Ready => true,
            Phase::Restoring | Phase::Failed => false,
        }
    }
}

/// Bootstrap writes one roles record per configured owner match, alongside person/sign-in records
/// and reply, plus one expired sign-in's erase; completion answers every bounded waiter.
/// Required free Request slots for one step/fire under validated `limits`: maximum of bootstrap
/// owner matches plus four, waiters plus two, and sign-ins, holdings or people plus one.
/// `worst_case` validates those additions before use; caller counts
/// output payload bytes separately.
#[must_use]
pub fn max_out(limits: &Limits) -> u32 {
    limits
        .initial_owners
        .saturating_add(4)
        .max(limits.waiters.saturating_add(2))
        .max(limits.sign_ins.saturating_add(1))
        .max(limits.holdings.saturating_add(1))
        .max(limits.people.saturating_add(1))
}

/// Apply one root-issued `event` with iteration clocks and immutable configured bounds in `env`;
/// caller reserves at least `max_out(&env.limits)` free `out` slots. Emits
/// routing/persistence/replies, never IO; root completes each `Route` once and withholds replies
/// until required atomic writes are durable. Restore admission validates role-success shape;
/// `Restored` checks its project/person references without rechecking current membership or
/// requiring the historical roster to equal the current one. Refused role asks retain their invalid
/// targets for keyed replay.
pub fn step(domain: &mut Domain, env: &Env<Limits>, event: Event, out: &mut Queue<Request>) {
    match event {
        Event::Act { reply_to, connector, key, act } => {
            acts::admit(domain, env, reply_to, crate::ActKey { connector, key }, *act, out);
        }
        Event::MoveChat { task, project, from, to } => {
            inbox::move_chat(domain, &env.limits, task, project, from, to, out);
        }
        Event::ChatState { party, project, task, last_reply, live, seen, replies } => {
            inbox::chat_state_event(domain, env, party, project, task, last_reply, live, seen, replies, out);
        }
        Event::ConversationSeen { request, project, last_reply } => {
            conversation_seen_decision(domain, env, request, project, last_reply, out);
        }
        Event::Forget { task, project, party, last_reply } => {
            inbox::forget_event(domain, env, task, project, party, last_reply, out);
        }
        Event::MakeService { request, person } => make_service(domain, env, request, person, out),
        Event::Seed { project, collaborators } => seed(domain, &env.limits, project, &collaborators, out),
        Event::Waiting { task, entries } => replace_waiting(domain, &env.limits, task, &entries),
        Event::ApplyRoles { reply_to, request } => {
            let result = apply_roles(domain, env, request, out);
            out.push(Request::RolesApplied { reply_to, request, result });
        }
        Event::Restore { record } => restore(domain, env, record, out),
        Event::Restored => restored(domain, env, out),
        Event::SignedIn { reply_to, person, sign_in, identity, kind, held } => {
            signin(domain, env, reply_to, person, sign_in, identity, kind, held, out);
        }
        Event::Whose { reply_to, sign_in } => {
            if !domain.ready() {
                return again(reply_to, Again::NotReady, out);
            }
            let signer = match domain.person(sign_in, env.now, env.wall) {
                Some(person) => {
                    let identity = domain.people.get(&person).expect("live sign-in has a party");
                    Some(crate::Signer { identity: identity.clone(), kind: domain.kind_of(identity) })
                }
                None => None,
            };
            out.push(Request::Reply { to: reply_to, reply: Reply::Signer(signer) });
        }
        Event::SignOut { reply_to, sign_in } => {
            if !domain.ready() {
                return again(reply_to, Again::NotReady, out);
            }
            end_signin(domain, sign_in, out);
            out.push(Request::Reply { to: reply_to, reply: Reply::SignedOut });
        }
        Event::Roles { project, holdings } => {
            if domain.phase == Phase::Failed {
                return;
            }
            match valid_roles(domain, &env.limits, project, &holdings) {
                Ok(()) => {
                    out.push(Request::Save { record: Stored::Roles { project, holdings: holdings.clone() } });
                    let saved = domain.roles.insert(project, holdings);
                    assert!(saved.is_ok(), "roles admitted before mutation");
                    prune_unheld_roles(domain, project);
                }
                Err(outcome) => out.push(Request::RolesRefused { project, outcome }),
            }
        }
        Event::Ask { reply_to, sign_in, key, ask, models } => {
            admit_ask(domain, env, reply_to, sign_in, key, ask, models, out);
        }
        Event::Decided { request, outcome } => decided(domain, env, Id::<Pending>::from_token(request), outcome, out),
    }
}

/// One expiration per fire; other expirations stay due for later iterations. Expire at most one
/// sign-in due by either injected clock, emitting its `Erase`; no work before ready. Caller reserves
/// `max_out(&env.limits)` free slots and calls again in later iterations while due; parent owns
/// durability and secret cleanup.
pub fn fire(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    if !domain.ready() {
        return;
    }
    if let Some(key) = domain.act_alarms.expire(env.now) {
        assert!(domain.acted.remove(&key).is_some(), "acted alarm has its row");
        out.push(Request::Erase { key: Key::Acted(key) });
    } else if let Some(key) = domain.answer_alarms.expire(env.now) {
        let removed = domain.answers.remove(&key);
        assert!(removed.is_some(), "retained answer has its expiry alarm");
        out.push(Request::Erase { key: Key::Answer(key) });
    } else if let Some(number) = domain.alarms.expire(env.now) {
        end_signin(domain, number, out);
    } else if let Some(&(expires, number)) = domain.expiries.first()
        && expires <= env.wall
    {
        end_signin(domain, number, out);
    }
}

fn answer_expiry(at: Wall, limits: &Limits) -> Wall {
    Wall::from_nanos(at.as_nanos().saturating_add(limits.request_retention.as_nanos()))
}

fn fact(domain: &mut Domain, observation: Fact) {
    if domain.facts.try_push(observation).is_err() {
        domain.lost = domain.lost.saturating_add(1);
    }
}

fn again(to: ReplyTo, why: Again, out: &mut Queue<Request>) {
    out.push(Request::Reply { to, reply: Reply::Again(why) });
}

fn signed_out(to: ReplyTo, out: &mut Queue<Request>) {
    out.push(Request::Reply { to, reply: Reply::SignedOut });
}

fn outcome_reply(outcome: Outcome) -> Reply {
    match outcome {
        Outcome::Again(why) => Reply::Again(why),
        Outcome::SignedOut => Reply::SignedOut,
        Outcome::Compound { .. }
        | Outcome::CompoundRefused { .. }
        | Outcome::MarkedSeen { .. }
        | Outcome::Applied { .. }
        | Outcome::NoteEdited { .. }
        | Outcome::ServiceMade { .. }
        | Outcome::Adopted { .. }
        | Outcome::Created { .. }
        | Outcome::Proposed { .. }
        | Outcome::ReleaseProposed { .. }
        | Outcome::Withdrawn { .. }
        | Outcome::Stopped { .. }
        | Outcome::Cancelled { .. }
        | Outcome::Placed { .. }
        | Outcome::Lifted { .. }
        | Outcome::Released { .. }
        | Outcome::PersonTaken { .. }
        | Outcome::PersonHandedBack { .. }
        | Outcome::PersonAnswered { .. }
        | Outcome::Moved { .. }
        | Outcome::ProposalDecided { .. }
        | Outcome::Said { .. }
        | Outcome::QuestionAnswered { .. }
        | Outcome::Amended { .. }
        | Outcome::RolesSet { .. }
        | Outcome::PolicyChanged { .. }
        | Outcome::Renamed { .. }
        | Outcome::ProjectMade { .. }
        | Outcome::SettingsChanged { .. }
        | Outcome::MovedToNewest { .. }
        | Outcome::PoolSet { .. }
        | Outcome::EscalationDecided { .. }
        | Outcome::AccountSignInStarted { .. }
        | Outcome::AccountProgramDeclared { .. }
        | Outcome::AccountKeyKept { .. }
        | Outcome::AccountMovingOut { .. }
        | Outcome::AccountFinishing { .. }
        | Outcome::AccountMovedIn { .. }
        | Outcome::AccountChanged
        | Outcome::AccountRemoved { .. }
        | Outcome::AccountSignedOut { .. }
        | Outcome::Refused(_) => Reply::Outcome(outcome),
    }
}

fn refused(to: ReplyTo, refusal: Refusal, out: &mut Queue<Request>) {
    out.push(Request::Reply { to, reply: Reply::Refused(refusal) });
}

fn valid_identity(limits: &Limits, identity: &Identity) -> bool {
    let Some(first) = identity.key.subject.len().checked_add(identity.login.len()) else {
        return false;
    };
    match first.checked_add(identity.name.len()) {
        Some(bytes) => {
            !identity.key.subject.is_empty() && bytes <= usize::try_from(limits.identity_bytes).expect("u32 fits usize")
        }
        None => false,
    }
}

fn apply_roles(
    domain: &mut Domain,
    env: &Env<Limits>,
    request: Token,
    out: &mut Queue<Request>,
) -> Result<(), Outcome> {
    if !domain.ready() {
        return Err(Outcome::Again(Again::NotReady));
    }
    let flight = domain.party_pending(Id::from_token(request)).ok_or(Outcome::Refused(Refusal::Unknown))?;
    let project = match flight.part.as_ref().unwrap_or(&flight.ask) {
        Ask::SetRoles { project, .. } => *project,
        Ask::Compound { .. }
        | Ask::MarkSeen { .. }
        | Ask::Application { .. }
        | Ask::MakeService { .. }
        | Ask::EditNote { .. }
        | Ask::Adopt { .. }
        | Ask::ChangePolicy { .. }
        | Ask::Rename { .. }
        | Ask::MakeProject { .. }
        | Ask::ChangeSettings { .. }
        | Ask::MoveToNewest { .. }
        | Ask::SetPool { .. }
        | Ask::DecideEscalation { .. }
        | Ask::DecideProposal { .. }
        | Ask::Say { .. }
        | Ask::AnswerQuestion { .. }
        | Ask::Amend { .. }
        | Ask::Move { .. }
        | Ask::TakePerson { .. }
        | Ask::HandBackPerson { .. }
        | Ask::AnswerPerson { .. }
        | Ask::Stop { .. }
        | Ask::Cancel { .. }
        | Ask::Hold { .. }
        | Ask::Lift { .. }
        | Ask::Release { .. }
        | Ask::Create { .. }
        | Ask::Propose { .. }
        | Ask::Withdraw { .. }
        | Ask::AccountSignIn { .. }
        | Ask::AccountSignOut { .. }
        | Ask::AccountChange { .. } => {
            return Err(Outcome::Refused(Refusal::Unknown));
        }
    };
    if !domain.roles.contains_key(&project) {
        return Err(Outcome::Refused(Refusal::Unknown));
    }
    if role(domain, flight.key.person, project) != Some(Role::Owner) {
        return Err(Outcome::Refused(Refusal::Role { held: role(domain, flight.key.person, project) }));
    }
    let holdings = match flight.part.as_ref().unwrap_or(&flight.ask) {
        Ask::SetRoles { holdings, .. } => {
            valid_roles(domain, &env.limits, project, holdings)?;
            for holding in holdings {
                if holding.person == 0 {
                    return Err(Outcome::Refused(Refusal::Limit { which: LimitKind::Roles }));
                }
                if !domain.people.contains_key(&holding.person) {
                    return Err(Outcome::Refused(Refusal::Unknown));
                }
            }
            holdings.clone()
        }
        Ask::Compound { .. }
        | Ask::MarkSeen { .. }
        | Ask::Application { .. }
        | Ask::MakeService { .. }
        | Ask::EditNote { .. }
        | Ask::Adopt { .. }
        | Ask::ChangePolicy { .. }
        | Ask::Rename { .. }
        | Ask::MakeProject { .. }
        | Ask::ChangeSettings { .. }
        | Ask::MoveToNewest { .. }
        | Ask::SetPool { .. }
        | Ask::DecideEscalation { .. }
        | Ask::DecideProposal { .. }
        | Ask::Say { .. }
        | Ask::AnswerQuestion { .. }
        | Ask::Amend { .. }
        | Ask::Move { .. }
        | Ask::TakePerson { .. }
        | Ask::HandBackPerson { .. }
        | Ask::AnswerPerson { .. }
        | Ask::Stop { .. }
        | Ask::Cancel { .. }
        | Ask::Hold { .. }
        | Ask::Lift { .. }
        | Ask::Release { .. }
        | Ask::Create { .. }
        | Ask::Propose { .. }
        | Ask::Withdraw { .. }
        | Ask::AccountSignIn { .. }
        | Ask::AccountSignOut { .. }
        | Ask::AccountChange { .. } => {
            unreachable!("validated roster flight")
        }
    };
    out.push(Request::Save { record: Stored::Roles { project, holdings: holdings.clone() } });
    let saved = domain.roles.insert(project, holdings);
    assert!(saved.is_ok(), "existing role row replaces without consuming capacity");
    prune_unheld_roles(domain, project);
    Ok(())
}

fn valid_roles(domain: &Domain, limits: &Limits, project: u32, holdings: &[Holding]) -> Result<(), Outcome> {
    if holdings.len() > usize::try_from(limits.holdings).expect("u32 fits usize") {
        return Err(Outcome::Refused(Refusal::Limit { which: LimitKind::Roles }));
    }
    if !domain.roles.contains_key(&project) && domain.roles.len() >= limits.projects {
        return Err(Outcome::Again(Again::Busy));
    }
    for (at, holding) in holdings.iter().enumerate() {
        match holding.role {
            Role::Policy { role } if role <= 3 => {
                return Err(Outcome::Refused(Refusal::Limit { which: LimitKind::Roles }));
            }
            Role::Owner | Role::Maintainer | Role::Member | Role::Observer | Role::Policy { .. } => {}
        }
        for earlier in holdings.iter().take(at) {
            if earlier.person == holding.person {
                return Err(Outcome::Refused(Refusal::Limit { which: LimitKind::Roles }));
            }
        }
    }
    Ok(())
}

fn seed(domain: &mut Domain, limits: &Limits, project: u32, collaborators: &[Seed], out: &mut Queue<Request>) {
    assert!(domain.can_seed(limits, project, collaborators), "root preflights an adoption seed");
    let old = domain.roles.get(&project).expect("preflighted project");
    let mut holdings = List::with_capacity(limits.holdings);
    for holding in &**old {
        holdings.push(*holding).expect("preflighted roster room");
    }
    let mut changed = false;
    for collaborator in collaborators {
        let person = match domain.identities.get(&collaborator.identity) {
            Some(person) => *person,
            None => {
                let person = collaborator.candidate;
                let identity = Identity { key: collaborator.identity.clone(), login: Box::new([]), name: Box::new([]) };
                domain.identities.insert(collaborator.identity.clone(), person).expect("preflighted identity room");
                domain.people.insert(person, identity.clone()).expect("preflighted person room");
                out.push(Request::Save { record: Stored::Person { number: person, identity } });
                person
            }
        };
        if !has_person(holdings.as_slice(), person) {
            holdings.push(Holding { person, role: collaborator.role }).expect("preflighted roster room");
            changed = true;
        }
    }
    if changed {
        let holdings = holdings.into_boxed();
        domain.roles.insert(project, holdings.clone()).expect("preflighted project");
        out.push(Request::Save { record: Stored::Roles { project, holdings } });
    }
}

fn has_person(holdings: &[Holding], person: u64) -> bool {
    for holding in holdings {
        if holding.person == person {
            return true;
        }
    }
    false
}

fn make_service(domain: &mut Domain, env: &Env<Limits>, request: Token, person: u64, out: &mut Queue<Request>) {
    if !domain.ready() {
        out.push(Request::ServiceMade { request, outcome: Outcome::Again(Again::NotReady) });
        return;
    }
    let flight = domain.party_pending(Id::from_token(request)).expect("service creation names a live keyed flight");
    let (project, name, service_role) = match flight.part.as_ref().unwrap_or(&flight.ask) {
        Ask::MakeService { project, name, role } => (*project, name.clone(), *role),
        Ask::Compound { .. }
        | Ask::MarkSeen { .. }
        | Ask::Application { .. }
        | Ask::EditNote { .. }
        | Ask::Adopt { .. }
        | Ask::Create { .. }
        | Ask::Propose { .. }
        | Ask::Withdraw { .. }
        | Ask::Stop { .. }
        | Ask::Cancel { .. }
        | Ask::Hold { .. }
        | Ask::Lift { .. }
        | Ask::Release { .. }
        | Ask::TakePerson { .. }
        | Ask::HandBackPerson { .. }
        | Ask::AnswerPerson { .. }
        | Ask::Move { .. }
        | Ask::DecideProposal { .. }
        | Ask::Say { .. }
        | Ask::AnswerQuestion { .. }
        | Ask::Amend { .. }
        | Ask::SetRoles { .. }
        | Ask::ChangePolicy { .. }
        | Ask::Rename { .. }
        | Ask::MakeProject { .. }
        | Ask::ChangeSettings { .. }
        | Ask::MoveToNewest { .. }
        | Ask::SetPool { .. }
        | Ask::DecideEscalation { .. }
        | Ask::AccountSignIn { .. }
        | Ask::AccountSignOut { .. }
        | Ask::AccountChange { .. } => unreachable!("service creation is routed only from its ask"),
    };
    let identity = Identity {
        key: IdentityKey { provider: domain.deployment_provider, subject: person.to_be_bytes().into() },
        login: Box::new([]),
        name,
    };
    let holdings = domain.roles.get(&project);
    let outcome = if role(domain, flight.key.person, project) != Some(Role::Owner) {
        Outcome::Refused(Refusal::Role { held: role(domain, flight.key.person, project) })
    } else if person == 0 || domain.people.contains_key(&person) || domain.identities.contains_key(&identity.key) {
        Outcome::Refused(Refusal::Unknown)
    } else if !valid_identity(&env.limits, &identity) {
        Outcome::Refused(Refusal::Limit { which: LimitKind::Identity })
    } else if domain.people.len() >= env.limits.people
        || match holdings {
            Some(rows) => rows.len() >= usize::try_from(env.limits.holdings).expect("u32 fits usize"),
            None => true,
        }
    {
        Outcome::Again(Again::Busy)
    } else {
        let old = holdings.expect("project room checked");
        let mut next = List::with_capacity(env.limits.holdings);
        for holding in &**old {
            next.push(*holding).expect("preflighted role room");
        }
        next.push(Holding { person, role: service_role }).expect("preflighted role room");
        let next = next.into_boxed();
        let indexed = domain.identities.insert(identity.key.clone(), person);
        assert!(indexed == Ok(None), "service identity preflighted");
        let saved = domain.people.insert(person, identity.clone());
        assert!(saved == Ok(None), "service person preflighted");
        let saved = domain.roles.insert(project, next.clone());
        assert!(saved.is_ok(), "service project preflighted");
        out.push(Request::Save { record: Stored::Person { number: person, identity } });
        out.push(Request::Save { record: Stored::Roles { project, holdings: next } });
        Outcome::ServiceMade { person }
    };
    out.push(Request::ServiceMade { request, outcome });
}

/// The first owners whose first sign-ins seed their roles: none once the first start is over.
fn seeds(owners: &[InitialOwner], seeding: bool) -> &[InitialOwner] {
    if seeding { owners } else { &[] }
}

#[expect(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one sign-in decision checks replay and held ownership before capacity and writes"
)]
fn signin(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    candidate: u64,
    number: u64,
    identity: Identity,
    kind: Kind,
    held: Option<u64>,
    out: &mut Queue<Request>,
) {
    if !domain.ready() {
        return again(to, Again::NotReady, out);
    }
    if !valid_identity(&env.limits, &identity) {
        return refused(to, Refusal::Limit { which: LimitKind::Identity }, out);
    }
    match kind {
        Kind::Person if identity.key.provider == domain.deployment_provider => {
            return signed_out(to, out);
        }
        Kind::Service if identity.key.provider != domain.deployment_provider => {
            return signed_out(to, out);
        }
        Kind::Person | Kind::Service => {}
    }
    let known = domain.identities.get(&identity.key).copied();
    if kind == Kind::Service && known.is_none() {
        return signed_out(to, out);
    }
    let person = known.unwrap_or(candidate);
    if let Some(old) = domain.sign_ins.get(&number) {
        if old.person != person {
            return signed_out(to, out);
        }
        if old.expires <= env.wall || old.due <= env.now {
            return signed_out(to, out);
        }
        return out
            .push(Request::Reply { to, reply: Reply::SignedIn { person, sign_in: number, expires: old.expires } });
    }
    if let Some(held) = held
        && let Some(row) = domain.sign_ins.get(&held)
    {
        // A held sign-in is kept only for the same person as the identity.
        #[expect(clippy::collapsible_if, reason = "held ownership is an explicit comparison (people.md, 3)")]
        if row.person == person {
            if row.expires > env.wall && row.due > env.now {
                return out.push(Request::Reply {
                    to,
                    reply: Reply::SignedIn { person, sign_in: held, expires: row.expires },
                });
            }
        }
    }
    if domain.sign_ins.len() >= env.limits.sign_ins {
        if let Some(number) = domain.alarms.expire(env.now) {
            end_signin(domain, number, out);
        } else if let Some(&(expires, number)) = domain.expiries.first()
            && expires <= env.wall
        {
            end_signin(domain, number, out);
        } else {
            let &(earliest, _) = domain.expiries.first().expect("positive full sign-in pool has an expiry");
            assert!(earliest > env.wall, "full sign-ins are live by both clocks");
            return out.push(Request::Reply { to, reply: Reply::Full { sign_ins: env.limits.sign_ins, earliest } });
        }
    }
    if known.is_none() {
        if domain.people.len() >= env.limits.people {
            return again(to, Again::Busy, out);
        }
        assert!(!domain.people.contains_key(&person), "root gives a fresh candidate person number");
        // Reserve all bootstrap role slots before making either record.
        for owner in seeds(&domain.owners, domain.seeding) {
            if owner.identity == identity.key {
                let Some(holdings) = domain.roles.get(&owner.project) else {
                    return refused(to, Refusal::Unknown, out);
                };
                if holdings.len() >= usize::try_from(env.limits.holdings).expect("u32 fits usize")
                    && !has_person(holdings, person)
                {
                    return again(to, Again::Busy, out);
                }
            }
        }
    }
    let Some(expires_nanos) = env.wall.as_nanos().checked_add(env.limits.sign_in_lifetime.as_nanos()) else {
        return refused(to, Refusal::Limit { which: LimitKind::Arithmetic }, out);
    };
    let expires = Wall::from_nanos(expires_nanos);
    if known.is_none() {
        let indexed = domain.identities.insert(identity.key.clone(), person);
        assert!(indexed == Ok(None), "new identity has room");
        // Each project occurs at most once for this identity in configuration.
        for owner in seeds(&domain.owners, domain.seeding) {
            if owner.identity != identity.key {
                continue;
            }
            let old = domain.roles.get(&owner.project).expect("bootstrap project admitted");
            let mut amended = List::with_capacity(env.limits.holdings);
            for holding in &**old {
                if holding.person != person {
                    amended.push(*holding).expect("bounded list room checked");
                }
            }
            amended.push(Holding { person, role: Role::Owner }).expect("bounded list room checked");
            let holdings = amended.into_boxed();
            out.push(Request::Save { record: Stored::Roles { project: owner.project, holdings: holdings.clone() } });
            let saved = domain.roles.insert(owner.project, holdings);
            assert!(saved.is_ok(), "existing bootstrap project");
        }
    }
    let changed = kind == Kind::Person
        && match domain.people.get(&person) {
            Some(old) => old != &identity,
            None => true,
        };
    if changed {
        out.push(Request::Save { record: Stored::Person { number: person, identity: identity.clone() } });
        let saved = domain.people.insert(person, identity);
        assert!(saved.is_ok(), "person admitted before mutation");
    }
    let saved = domain.sign_ins.insert(number, SignIn { person, expires, due: deadline(env, expires) });
    assert!(saved == Ok(None), "sign-in admitted before mutation");
    arm(domain, env, number, expires);
    out.push(Request::Save { record: Stored::SignIn { number, person, expires } });
    out.push(Request::Reply { to, reply: Reply::SignedIn { person, sign_in: number, expires } });
    fact(domain, Fact::SignedIn { person, sign_in: number });
}

fn deadline(env: &Env<Limits>, expires: Wall) -> Time {
    let left = skein_lib::Duration::from_nanos(expires.as_nanos().saturating_sub(env.wall.as_nanos()));
    env.now.saturating_add(left)
}

fn arm(domain: &mut Domain, env: &Env<Limits>, number: u64, expires: Wall) {
    assert!(domain.expiries.insert((expires, number)).is_ok(), "one saved expiry per sign-in");
    let armed = domain.alarms.arm(number, deadline(env, expires));
    assert!(armed.is_ok(), "one alarm per admitted sign-in");
}

fn end_signin(domain: &mut Domain, number: u64, out: &mut Queue<Request>) {
    if let Some(signin) = domain.sign_ins.remove(&number) {
        domain.expiries.remove(&(signin.expires, number));
        domain.alarms.cancel(number);
        out.push(Request::Erase { key: Key::SignIn(number) });
        fact(domain, Fact::SignedOut { sign_in: number });
    }
}

fn project(ask: &Ask) -> u32 {
    match ask {
        Ask::Application { scope, .. } => match scope {
            crate::ApplicationScope::Project(project) => *project,
            crate::ApplicationScope::Own | crate::ApplicationScope::Deployment => 0,
        },
        // An account request names no project; its check is its holder (domain/authority.md, 8.4).
        Ask::Lift { .. }
        | Ask::MarkSeen { .. }
        | Ask::AccountSignIn { .. }
        | Ask::AccountSignOut { .. }
        | Ask::AccountChange { .. } => 0,
        Ask::MakeProject { .. } | Ask::ChangeSettings { .. } | Ask::MoveToNewest { .. } => crate::DEPLOYMENT,
        Ask::Hold { project, .. }
        | Ask::Rename { project, .. }
        | Ask::Compound { project, .. }
        | Ask::EditNote { project, .. }
        | Ask::MakeService { project, .. }
        | Ask::Adopt { project, .. }
        | Ask::SetRoles { project, .. }
        | Ask::ChangePolicy { project, .. }
        | Ask::SetPool { project, .. }
        | Ask::DecideEscalation { project, .. }
        | Ask::DecideProposal { project, .. }
        | Ask::Say { project, .. }
        | Ask::AnswerQuestion { project, .. }
        | Ask::Amend { project, .. }
        | Ask::Move { project, .. }
        | Ask::TakePerson { project, .. }
        | Ask::HandBackPerson { project, .. }
        | Ask::AnswerPerson { project, .. }
        | Ask::Stop { project, .. }
        | Ask::Cancel { project, .. }
        | Ask::Release { project, .. }
        | Ask::Create { project, .. }
        | Ask::Propose { project, .. }
        | Ask::Withdraw { project, .. } => *project,
    }
}

fn valid_adoption(limits: &Limits, adoption: &crate::Adoption) -> bool {
    let Some(items) = adoption.resource.path.len().checked_add(adoption.options.len()) else { return false };
    let Some(mut bytes) = items.checked_mul(size_of::<Box<[u8]>>()) else { return false };
    for segment in &adoption.resource.path {
        let Some(next) = bytes.checked_add(segment.len()) else { return false };
        bytes = next;
    }
    for option in &adoption.options {
        let Some(next) = bytes.checked_add(option.len()) else { return false };
        bytes = next;
    }
    !adoption.resource.path.is_empty() && bytes <= usize::try_from(limits.amendment_bytes).expect("u32 fits usize")
}

fn note_pattern_bytes(pattern: &crate::Pattern) -> Option<usize> {
    let mut bytes = pattern.segments.len().checked_mul(size_of::<Box<[u8]>>())?;
    for segment in &pattern.segments {
        bytes = bytes.checked_add(segment.len())?;
    }
    let last = match &pattern.last {
        crate::Last::Exact(bytes) | crate::Last::Open(bytes) => bytes.len(),
    };
    bytes.checked_add(last)
}

fn valid_note(limits: &Limits, name: u64, scope: &crate::NoteScope, change: &crate::NoteChange) -> bool {
    if name == 0 {
        return false;
    }
    let scope_bytes = match scope {
        crate::NoteScope::Deployment | crate::NoteScope::Project => 0,
        crate::NoteScope::Goal { goal } => {
            if *goal == 0 {
                return false;
            }
            0
        }
        crate::NoteScope::Resources { pattern, .. } => {
            let Some(bytes) = note_pattern_bytes(pattern) else { return false };
            bytes
        }
    };
    let change_bytes = match change {
        crate::NoteChange::Correct { description, body, references, recalled } => {
            if *recalled == 0 {
                return false;
            }
            for number in references {
                if *number == 0 {
                    return false;
                }
            }
            let Some(bytes) = description.len().checked_add(body.len()) else { return false };
            let Some(ref_bytes) = references.len().checked_mul(size_of::<u64>()) else { return false };
            let Some(total) = bytes.checked_add(ref_bytes) else { return false };
            total
        }
        crate::NoteChange::Delete { recalled } => {
            if *recalled == 0 {
                return false;
            }
            0
        }
    };
    match scope_bytes.checked_add(change_bytes) {
        Some(bytes) => bytes <= usize::try_from(limits.amendment_bytes).expect("u32 fits usize"),
        None => false,
    }
}

fn valid_person_result(limits: &Limits, result: &crate::PersonResult) -> bool {
    let bound = usize::try_from(limits.words).expect("u32 fits usize");
    match result {
        crate::PersonResult::Report { words } | crate::PersonResult::Verdict { words, .. } => words.len() <= bound,
        crate::PersonResult::Failure { reason } => reason.len() <= bound,
        crate::PersonResult::Approved { state, words } => {
            let words = match words {
                Some(words) => words.len(),
                None => 0,
            };
            match state.len().checked_add(words) {
                Some(bytes) => bytes <= bound,
                None => false,
            }
        }
    }
}

pub(crate) fn valid_ask(limits: &Limits, ask: &Ask) -> bool {
    if !crate::words::valid(ask) {
        return false;
    }
    match ask {
        Ask::Compound { operations, .. } => crate::compound::refusal(limits, operations).is_none(),
        Ask::MarkSeen { seen } => match seen {
            Seen::Results { .. } => true,
            Seen::Conversation { task, .. } => *task != 0,
        },
        Ask::AccountChange { change: crate::AccountChange::DeclareProgram { name, .. }, .. } => {
            crate::program_name(name)
        }
        Ask::AccountChange { change: crate::AccountChange::SetUses { uses: Some(uses), .. }, .. } => {
            uses.len() <= usize::try_from(crate::USES).expect("account uses fit usize")
                && match uses.len().checked_mul(size_of::<crate::Use>()) {
                    Some(bytes) => bytes <= usize::try_from(limits.words).expect("word bound fits usize"),
                    None => false,
                }
        }
        Ask::ChangeSettings { .. }
        | Ask::MoveToNewest { .. }
        | Ask::Application { .. }
        | Ask::AccountSignIn { .. }
        | Ask::AccountSignOut { .. }
        | Ask::AccountChange { .. } => true,
        Ask::EditNote { name, scope, change, .. } => valid_note(limits, *name, scope, change),
        Ask::MakeService { name, .. } => {
            let Some(bytes) = name.len().checked_add(size_of::<u64>()) else {
                return false;
            };
            !name.is_empty()
                && name.len() <= usize::try_from(limits.words).expect("u32 fits usize")
                && bytes <= usize::try_from(limits.identity_bytes).expect("u32 fits usize")
        }
        Ask::Move { task, reason, .. } | Ask::Cancel { task, reason, .. } => {
            *task != 0 && reason.len() <= usize::try_from(limits.words).expect("u32 fits usize")
        }
        Ask::Hold { scope, reason, .. } => valid_hold(limits, *scope, reason),
        Ask::Lift { hold, .. } => *hold != 0,
        Ask::TakePerson { task, .. }
        | Ask::HandBackPerson { task, .. }
        | Ask::Stop { task, .. }
        | Ask::Release { task, .. } => *task != 0,
        Ask::AnswerPerson { task, result, .. } => *task != 0 && valid_person_result(limits, result),
        Ask::Adopt { adoption, .. } => valid_adoption(limits, adoption),
        Ask::SetRoles { holdings, .. } => holdings.len() <= usize::try_from(limits.holdings).expect("u32 fits usize"),
        Ask::Rename { name, .. } | Ask::MakeProject { name, .. } => name.len() <= crate::NAME_BYTES,
        Ask::ChangePolicy { change, .. } => match crate::policy_change_bytes(change) {
            Some(bytes) => bytes <= u64::from(limits.amendment_bytes),
            None => false,
        },
        Ask::SetPool { person, .. } => *person != 0,
        Ask::Create { batch, .. } => crate::batch::valid(limits, batch),
        Ask::Propose { project, action, reason } => valid_propose(limits, *project, action, reason),
        Ask::Withdraw { proposal, .. } => *proposal != 0,
        Ask::Say { task, words, .. } => {
            *task != 0 && !words.is_empty() && words.len() <= usize::try_from(limits.words).expect("u32 fits usize")
        }
        Ask::AnswerQuestion { task, question, words, .. } => {
            *task != 0
                && *question != 0
                && !words.is_empty()
                && words.len() <= usize::try_from(limits.words).expect("u32 fits usize")
        }
        Ask::Amend { task, amendment, .. } => *task != 0 && crate::amendment::valid(limits, amendment),
        Ask::DecideEscalation { task, revision, decision, .. } => {
            *task != 0
                && *revision != 0
                && match decision {
                    crate::EscalationDecision::Release | crate::EscalationDecision::Pass => true,
                    crate::EscalationDecision::Reject { reason } => {
                        reason.len() <= usize::try_from(limits.words).expect("u32 fits usize")
                    }
                }
        }
        Ask::DecideProposal { proposer, proposal, decision, running, .. } => {
            (running.is_none() || *decision == crate::ProposalDecision::Accept)
                && proposer.number() != 0
                && *proposal != 0
                && match decision {
                    crate::ProposalDecision::Accept | crate::ProposalDecision::Pass => true,
                    crate::ProposalDecision::Reject { reason } => {
                        reason.len() <= usize::try_from(limits.words).expect("u32 fits usize")
                    }
                }
        }
    }
}

fn valid_propose(limits: &Limits, project: u32, action: &crate::Action, reason: &[u8]) -> bool {
    if reason.len() > usize::try_from(limits.words).expect("words bound") {
        return false;
    }
    match action {
        crate::Action::ReleasePlaced { task, hold, .. } => *task != 0 && *hold != 0 && !reason.is_empty(),
        crate::Action::Batch(batch) => crate::batch::valid(limits, batch),
        crate::Action::Amend { task, amendment, .. } => *task != 0 && crate::amendment::valid(limits, amendment),
        crate::Action::Settings { .. } => project == crate::DEPLOYMENT,
    }
}

fn valid_hold(limits: &Limits, scope: crate::HoldScope, reason: &[u8]) -> bool {
    !reason.is_empty()
        && reason.len() <= usize::try_from(limits.words).expect("words bound fits usize")
        && valid_hold_scope(scope)
}

fn valid_hold_scope(scope: crate::HoldScope) -> bool {
    match scope {
        crate::HoldScope::Subtree { task } => task != 0,
        crate::HoldScope::Project => true,
    }
}

// Admission identifies the bounded payload before any keyed flight is made.
pub(crate) fn invalid_ask(limits: &Limits, ask: &Ask) -> Refusal {
    let which = match ask {
        Ask::Hold { scope, reason, .. } => {
            if !valid_hold_scope(*scope) {
                return Refusal::Unknown;
            }
            if reason.is_empty() { LimitKind::Empty } else { LimitKind::Words }
        }
        Ask::Compound { operations, .. } => {
            return crate::compound::refusal(limits, operations).expect("invalid compound");
        }
        Ask::EditNote { name, .. } => {
            if *name == 0 {
                return Refusal::Unknown;
            }
            LimitKind::Notes
        }
        Ask::MakeService { name, .. } => {
            if name.is_empty() {
                LimitKind::Empty
            } else {
                LimitKind::Identity
            }
        }
        Ask::Move { task, .. } | Ask::Cancel { task, .. } | Ask::AnswerPerson { task, .. } => {
            if *task == 0 {
                return Refusal::Unknown;
            }
            LimitKind::Words
        }
        Ask::MarkSeen { .. }
        | Ask::Application { .. }
        | Ask::TakePerson { .. }
        | Ask::HandBackPerson { .. }
        | Ask::Stop { .. }
        | Ask::Lift { .. }
        | Ask::Release { .. }
        | Ask::SetPool { .. }
        | Ask::Withdraw { .. }
        | Ask::AccountSignIn { .. }
        | Ask::AccountSignOut { .. } => return Refusal::Unknown,
        Ask::Adopt { .. } => LimitKind::Resource,
        Ask::SetRoles { .. } => LimitKind::Roles,
        Ask::ChangePolicy { .. } => LimitKind::Policy,
        Ask::Propose { action: crate::Action::ReleasePlaced { task, hold, .. }, .. } if *task == 0 || *hold == 0 => {
            return Refusal::Unknown;
        }
        Ask::Propose { action: crate::Action::ReleasePlaced { .. }, reason, .. } if reason.is_empty() => {
            LimitKind::Empty
        }
        Ask::Create { batch, .. } | Ask::Propose { action: crate::Action::Batch(batch), .. } => {
            crate::batch::invalid(limits, batch)
        }
        Ask::Rename { .. }
        | Ask::MakeProject { .. }
        | Ask::ChangeSettings { .. }
        | Ask::MoveToNewest { .. }
        | Ask::AccountChange { .. }
        | Ask::Propose {
            action: crate::Action::Amend { .. } | crate::Action::Settings { .. } | crate::Action::ReleasePlaced { .. },
            ..
        } => LimitKind::Words,
        Ask::Say { task, words, .. } => {
            if *task == 0 {
                return Refusal::Unknown;
            }
            if words.is_empty() { LimitKind::Empty } else { LimitKind::Words }
        }
        Ask::AnswerQuestion { task, question, words, .. } => {
            if *task == 0 || *question == 0 {
                return Refusal::Unknown;
            }
            if words.is_empty() { LimitKind::Empty } else { LimitKind::Words }
        }
        Ask::Amend { task, .. } => {
            if *task == 0 {
                return Refusal::Unknown;
            }
            LimitKind::Spec
        }
        Ask::DecideEscalation { task, revision, .. } => {
            if *task == 0 {
                return Refusal::Unknown;
            }
            if *revision == 0 { LimitKind::Revision } else { LimitKind::Words }
        }
        Ask::DecideProposal { proposer, proposal, decision, running, .. } => {
            if running.is_some() && *decision != crate::ProposalDecision::Accept {
                return Refusal::NotWellFormed { at: 0, why: crate::Malformed::Running };
            }
            if proposer.number() == 0 || *proposal == 0 {
                return Refusal::Unknown;
            }
            LimitKind::Words
        }
    };
    Refusal::Limit { which }
}

#[expect(clippy::too_many_lines, reason = "one exhaustive keyed request-to-answer shape table")]
fn valid_answer_shape(ask: &Ask, outcome: Outcome) -> bool {
    #[derive(PartialEq, Eq)]
    enum Expected {
        Compound(u32, u32),
        MarkSeen(Seen),
        Application(u16),
        Note(u64),
        Service,
        Create,
        Propose,
        ReleasePropose,
        Withdraw(u64),
        Stop(u64),
        Cancel(u64),
        Release(u64),
        Hold,
        Lift(u64),
        Take(u64),
        HandBack(u64),
        AnswerPerson(u64),
        Move(u64),
        Proposal(crate::Proposer, u64),
        Say(u64),
        AnswerQuestion(u64, u64),
        Amend(u64, bool),
        Roles(u32, bool),
        Policy(u32),
        Rename(u32),
        MakeProject,
        ChangeSettings,
        MoveToNewest,
        Pool(u32, u64),
        Adoption(u32),
        Escalation,
        AccountSignIn,
        AccountSignOut,
        AccountKey,
        AccountProgram,
        AccountChanged,
        AccountRemoved,
        AccountMovingOut(u32),
        AccountMovedIn,
        AccountFinishing(u32),
    }

    let expected = match ask {
        Ask::Compound { project, operations } => Expected::Compound(*project, operations.len()),
        Ask::MarkSeen { seen } => Expected::MarkSeen(*seen),
        Ask::Application { connector, .. } => Expected::Application(*connector),
        Ask::EditNote { name, .. } => Expected::Note(*name),
        Ask::MakeService { .. } => Expected::Service,
        Ask::Adopt { project, .. } => Expected::Adoption(*project),
        Ask::Create { .. } => Expected::Create,
        Ask::Propose { action: crate::Action::ReleasePlaced { .. }, .. } => Expected::ReleasePropose,
        Ask::Propose { .. } => Expected::Propose,
        Ask::Withdraw { proposal, .. } => Expected::Withdraw(*proposal),
        Ask::Stop { task, .. } => Expected::Stop(*task),
        Ask::Cancel { task, .. } => Expected::Cancel(*task),
        Ask::Release { task, .. } => Expected::Release(*task),
        Ask::Hold { .. } => Expected::Hold,
        Ask::Lift { hold, .. } => Expected::Lift(*hold),
        Ask::TakePerson { task, .. } => Expected::Take(*task),
        Ask::HandBackPerson { task, .. } => Expected::HandBack(*task),
        Ask::AnswerPerson { task, .. } => Expected::AnswerPerson(*task),
        Ask::Move { task, .. } => Expected::Move(*task),
        Ask::DecideProposal { proposer, proposal, .. } => Expected::Proposal(*proposer, *proposal),
        Ask::Say { task, .. } => Expected::Say(*task),
        Ask::AnswerQuestion { task, question, .. } => Expected::AnswerQuestion(*task, *question),
        Ask::Amend { task, release, .. } => Expected::Amend(*task, release.is_some()),
        Ask::SetRoles { project, holdings } => {
            let mut valid = true;
            for (at, holding) in holdings.iter().enumerate() {
                if holding.person == 0 {
                    valid = false;
                }
                for earlier in holdings.get(..at).expect("enumerated holding position is in bounds") {
                    if earlier.person == holding.person {
                        valid = false;
                    }
                }
            }
            Expected::Roles(*project, valid)
        }
        Ask::ChangePolicy { project, .. } => Expected::Policy(*project),
        Ask::Rename { project, .. } => Expected::Rename(*project),
        Ask::MakeProject { .. } => Expected::MakeProject,
        Ask::ChangeSettings { .. } => Expected::ChangeSettings,
        Ask::MoveToNewest { .. } => Expected::MoveToNewest,
        Ask::SetPool { project, person, .. } => Expected::Pool(*project, *person),
        Ask::DecideEscalation { .. } => Expected::Escalation,
        Ask::AccountSignIn { .. } => Expected::AccountSignIn,
        Ask::AccountSignOut { .. } => Expected::AccountSignOut,
        Ask::AccountChange { change, .. } => match change {
            crate::AccountChange::AddKey { .. } | crate::AccountChange::ReplaceKey { .. } => Expected::AccountKey,
            crate::AccountChange::DeclareProgram { .. } => Expected::AccountProgram,
            crate::AccountChange::Remove { .. } => Expected::AccountRemoved,
            crate::AccountChange::MoveOut { account } => Expected::AccountMovingOut(*account),
            crate::AccountChange::FinishSignIn { source, .. } => Expected::AccountFinishing(*source),
            crate::AccountChange::MoveIn { .. } => Expected::AccountMovedIn,
            crate::AccountChange::Written { .. }
            | crate::AccountChange::CheckProgram { .. }
            | crate::AccountChange::RemoveKey { .. }
            | crate::AccountChange::Resume { .. }
            | crate::AccountChange::MoveAccount { .. }
            | crate::AccountChange::MoveSource { .. }
            | crate::AccountChange::SetUses { .. } => Expected::AccountChanged,
        },
    };

    match outcome {
        Outcome::Compound { project, operations, first_message } => {
            expected == Expected::Compound(project, operations) && first_message != 0
        }
        Outcome::CompoundRefused { at, .. } => {
            if let Ask::Compound { operations, .. } = ask {
                at < operations.len()
            } else {
                false
            }
        }
        Outcome::MarkedSeen { seen } => expected == Expected::MarkSeen(seen),
        Outcome::Applied { connector, token } => expected == Expected::Application(connector) && token.raw() != 0,
        Outcome::NoteEdited { name } => expected == Expected::Note(name),
        Outcome::ServiceMade { person } => expected == Expected::Service && person != 0,
        Outcome::Adopted { project } => expected == Expected::Adoption(project),
        Outcome::Created { first, last, .. } => expected == Expected::Create && first != 0 && last >= first,
        Outcome::Proposed { proposal, .. } => expected == Expected::Propose && proposal != 0,
        Outcome::ReleaseProposed { proposal } => expected == Expected::ReleasePropose && proposal != 0,
        Outcome::Withdrawn { proposal } => expected == Expected::Withdraw(proposal),
        Outcome::Stopped { task } => expected == Expected::Stop(task),
        Outcome::Cancelled { task } => expected == Expected::Cancel(task),
        Outcome::Released { task } => expected == Expected::Release(task),
        Outcome::Placed { hold, revision } => expected == Expected::Hold && hold != 0 && revision == 1,
        Outcome::Lifted { hold, .. } => expected == Expected::Lift(hold),
        Outcome::PersonTaken { task } => expected == Expected::Take(task),
        Outcome::PersonHandedBack { task } => expected == Expected::HandBack(task),
        Outcome::PersonAnswered { task } => expected == Expected::AnswerPerson(task),
        Outcome::Moved { task } => expected == Expected::Move(task),
        Outcome::ProposalDecided { proposer, proposal, .. } => expected == Expected::Proposal(proposer, proposal),
        Outcome::Said { task, message } => expected == Expected::Say(task) && message != 0,
        Outcome::QuestionAnswered { task, question, message } => {
            expected == Expected::AnswerQuestion(task, question) && message != 0
        }
        Outcome::Amended { task, released } => {
            expected == Expected::Amend(task, true) || !released && expected == Expected::Amend(task, false)
        }
        Outcome::RolesSet { project } => expected == Expected::Roles(project, true),
        Outcome::PolicyChanged { project } => expected == Expected::Policy(project),
        Outcome::Renamed { project } => expected == Expected::Rename(project),
        Outcome::MovedToNewest { newest, .. } => expected == Expected::MoveToNewest && newest != 0,
        Outcome::SettingsChanged { revision } => expected == Expected::ChangeSettings && revision != 0,
        Outcome::ProjectMade { project } => expected == Expected::MakeProject && project != crate::DEPLOYMENT,
        Outcome::PoolSet { project, person } => expected == Expected::Pool(project, person),
        Outcome::EscalationDecided { .. } => expected == Expected::Escalation,
        Outcome::AccountSignInStarted { .. } => expected == Expected::AccountSignIn,
        Outcome::AccountProgramDeclared { source, name } => {
            if expected != Expected::AccountProgram || source == u32::MAX || !crate::program_name(&name) {
                return false;
            }
            let Ask::AccountChange { change: crate::AccountChange::DeclareProgram { name: asked, .. }, .. } = ask
            else {
                return false;
            };
            *asked == name
        }
        Outcome::AccountKeyKept { source } => expected == Expected::AccountKey && source != u32::MAX,
        Outcome::AccountChanged => expected == Expected::AccountChanged,
        Outcome::AccountMovingOut { account, number, source } => {
            expected == Expected::AccountMovingOut(account) && number != u32::MAX && source != u32::MAX
        }
        Outcome::AccountFinishing { source } => expected == Expected::AccountFinishing(source),
        Outcome::AccountMovedIn { account, .. } => expected == Expected::AccountMovedIn && account != u32::MAX,
        Outcome::AccountRemoved { .. } => expected == Expected::AccountRemoved,
        Outcome::AccountSignedOut { .. } => expected == Expected::AccountSignOut,
        Outcome::Refused(_) => true,
        Outcome::Again(_) | Outcome::SignedOut => false,
    }
}

fn role(domain: &Domain, person: u64, project: u32) -> Option<Role> {
    for holding in &**domain.roles.get(&project)? {
        if holding.person == person {
            return Some(holding.role);
        }
    }
    None
}

#[expect(clippy::too_many_arguments, reason = "one keyed request includes its canonical original model context")]
fn admit_ask(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    sign_in: u64,
    bytes: [u8; 16],
    ask: Ask,
    models: Box<[Box<[u8]>]>,
    out: &mut Queue<Request>,
) {
    if !domain.ready() {
        return again(to, Again::NotReady, out);
    }
    let Some(session) = domain.sign_ins.get(&sign_in) else {
        return signed_out(to, out);
    };
    if session.expires <= env.wall || session.due <= env.now {
        return signed_out(to, out);
    }
    let key = RequestKey { person: session.person, key: bytes };
    if !valid_ask(&env.limits, &ask) || !crate::valid_model_context(&ask, &models) {
        return refused(to, invalid_ask(&env.limits, &ask), out);
    }
    if let Some(answer) = domain.answers.get(&key)
        && (answer_expiry(answer.at, &env.limits) <= env.wall || answer.due <= env.now)
    {
        let old = domain.answers.remove(&key);
        assert!(old.is_some(), "expired answer was retained");
        domain.answer_alarms.cancel(key);
        out.push(Request::Erase { key: Key::Answer(key) });
    }
    if let Some(answer) = domain.answers.get(&key) {
        if answer.ask != ask || answer.models != models {
            return refused(to, Refusal::KeyConflict, out);
        }
        return out.push(Request::Reply { to, reply: Reply::Outcome(answer.outcome.clone()) });
    }

    if let Some(id) = domain.flights.get(&key).copied() {
        if let Pending::MoveOut { holder, account, .. } = domain.pending.get(id).expect("indexed move") {
            let same = ask
                == Ask::AccountChange { holder: *holder, change: crate::AccountChange::MoveOut { account: *account } };
            return if same { again(to, Again::Busy, out) } else { refused(to, Refusal::KeyConflict, out) };
        }
        let flight = domain.party_pending_mut(id).expect("key index names live pending request");
        if flight.ask != ask || flight.models != models {
            return refused(to, Refusal::KeyConflict, out);
        }
        if flight.replies.len() >= env.limits.waiters {
            room_pressure(domain, env, key, &ask, crate::CapacityCause::Waiters { limit: env.limits.waiters });
            return again(to, Again::Busy, out);
        }
        flight.replies.push(to).expect("bounded list room checked");
        return;
    }
    let written = written_place(domain, key.person, &ask);
    // A move is admitted whole; Written transfers its already reserved place.
    // Seen still uses only pending room (protocol/llm.md, 5.6; people.md, 5.1).
    let occupied = domain.answers.len().saturating_add(domain.acted.len()).saturating_add(answer_reservations(domain));
    let credit = u32::from(written.is_some());
    if occupied.saturating_sub(credit).saturating_add(answer_places(&ask)) > env.limits.requests {
        room_pressure(domain, env, key, &ask, crate::CapacityCause::Requests { limit: env.limits.requests });
        return again(to, full_answers(domain, env, occupied), out);
    }
    let project = project(&ask);
    // An account request's check is its holder, never a role (domain/authority.md, 8.4).
    let role = if let Ask::Application { scope: crate::ApplicationScope::Own, .. }
    | Ask::Lift { .. }
    | Ask::AccountSignIn { .. }
    | Ask::AccountSignOut { .. }
    | Ask::AccountChange { .. } = &ask
    {
        None
    } else {
        role(domain, key.person, project)
    };
    let refusal = admission(domain, key.person, project, role, &ask);
    if let Some(refusal) = refusal {
        if let Some((previous, id)) = written {
            assert!(domain.flights.remove(&previous) == Some(id), "Written consumes its reserved place");
            domain.pending.retire(id);
        }
        let outcome = Outcome::Refused(refusal);
        save_answer(domain, env, key, ask, models, outcome.clone(), out);
        return out.push(Request::Reply { to, reply: outcome_reply(outcome) });
    }
    if written.is_none() && domain.pending.is_full() {
        room_pressure(domain, env, key, &ask, crate::CapacityCause::Pending { limit: env.limits.pending });
        return again(to, Again::Busy, out);
    }
    let mut replies = List::with_capacity(env.limits.waiters);
    replies.push(to).expect("bounded list room checked");
    let flight = Pending::Party(PartyPending { key, ask: ask.clone(), models: models.clone(), replies, part: None });
    let id = match written {
        Some((previous, id)) => {
            assert!(domain.flights.remove(&previous) == Some(id), "Written takes its move's index");
            *domain.pending.get_mut(id).expect("reserved Written slot") = flight;
            id
        }
        None => domain.pending.insert(flight).expect("pending request admitted"),
    };
    let indexed = domain.flights.insert(key, id);
    assert!(indexed == Ok(None), "one key per flight within pending capacity");
    out.push(Request::Route { request: id.token(), person: key.person, project, role, ask: Box::new(ask), models });
    fact(domain, Fact::Routed { person: key.person });
}

fn full_answers(domain: &Domain, env: &Env<Limits>, held: u32) -> Again {
    let mut oldest = None;
    for (_, answer) in &domain.answers {
        oldest = Some(oldest.unwrap_or(answer.at).min(answer.at));
    }
    for (_, answer) in &domain.acted {
        oldest = Some(oldest.unwrap_or(answer.at()).min(answer.at()));
    }
    // Reservations have no expiry yet. With no retained answer, report
    // now: keep the ordinary backoff and the caller's patience, rather
    // than inventing an expiry for a decision still in flight.
    let room_at = match oldest {
        Some(at) => answer_expiry(at, &env.limits),
        None => env.wall,
    };
    Again::Full { held, room_at }
}

fn room_pressure(domain: &mut Domain, env: &Env<Limits>, key: RequestKey, ask: &Ask, cause: crate::CapacityCause) {
    let action = crate::capacity::action(ask, key.person);
    if domain.room_refusals.capacity() == 0 {
        domain.room_refusals_lost = domain.room_refusals_lost.saturating_add(1);
        return;
    }
    if domain.room_refusals.room() == 0 {
        domain.room_refusals.pop();
        domain.room_refusals_lost = domain.room_refusals_lost.saturating_add(1);
    }
    domain.room_refusals.push(crate::CapacityRefusal {
        key,
        project: crate::capacity::project(ask),
        action,
        at: env.wall,
        cause,
    });
}

fn save_answer(
    domain: &mut Domain,
    env: &Env<Limits>,
    key: RequestKey,
    ask: Ask,
    models: Box<[Box<[u8]>]>,
    outcome: Outcome,
    out: &mut Queue<Request>,
) {
    if !reserves_answer(&ask) {
        return;
    }
    out.push(Request::Save {
        record: Stored::Answer {
            key,
            ask: Box::new(ask.clone()),
            models: models.clone(),
            outcome: outcome.clone(),
            at: env.wall,
        },
    });
    let due = deadline(env, answer_expiry(env.wall, &env.limits));
    let saved = domain.answers.insert(key, Answered { ask, models, outcome, at: env.wall, due });
    assert!(saved.is_ok(), "answer room reserved at request entrance");
    let armed = domain.answer_alarms.arm(key, due);
    assert!(armed.is_ok(), "one expiry per retained completed key");
    fact(domain, Fact::Answered { person: key.person });
}

fn answer_reservations(domain: &Domain) -> u32 {
    let mut reserved = domain.act_flights.len();
    for (_, id) in &domain.flights {
        match domain.pending.get(*id).expect("key index names live pending request") {
            Pending::Party(flight) => reserved = reserved.saturating_add(answer_places(&flight.ask)),
            Pending::MoveOut { .. } => reserved = reserved.saturating_add(1),
            Pending::Act(_) => unreachable!("acts have their own index"),
        }
    }
    reserved
}

fn answer_places(ask: &Ask) -> u32 {
    if let Ask::AccountChange { change: crate::AccountChange::MoveOut { .. }, .. } = ask {
        2
    } else {
        u32::from(reserves_answer(ask))
    }
}

fn written_place(domain: &Domain, person: u64, ask: &Ask) -> Option<(RequestKey, Id<Pending>)> {
    let Ask::AccountChange { holder, change: crate::AccountChange::Written { number } } = ask else { return None };
    for (key, id) in &domain.flights {
        if let Pending::MoveOut { holder: held, number: moving, .. } = domain.pending.get(*id).expect("indexed move")
            && key.person == person
            && holder == held
            && number == moving
        {
            return Some((*key, *id));
        }
    }
    None
}

fn reserves_answer(ask: &Ask) -> bool {
    match ask {
        Ask::MarkSeen { .. } => false,
        Ask::Application { .. }
        | Ask::AccountSignIn { .. }
        | Ask::AccountSignOut { .. }
        | Ask::AccountChange { .. }
        | Ask::Compound { .. }
        | Ask::EditNote { .. }
        | Ask::MakeService { .. }
        | Ask::Adopt { .. }
        | Ask::SetRoles { .. }
        | Ask::ChangePolicy { .. }
        | Ask::Rename { .. }
        | Ask::MakeProject { .. }
        | Ask::ChangeSettings { .. }
        | Ask::MoveToNewest { .. }
        | Ask::SetPool { .. }
        | Ask::Create { .. }
        | Ask::Propose { .. }
        | Ask::Withdraw { .. }
        | Ask::Say { .. }
        | Ask::AnswerQuestion { .. }
        | Ask::Amend { .. }
        | Ask::DecideEscalation { .. }
        | Ask::DecideProposal { .. }
        | Ask::Move { .. }
        | Ask::TakePerson { .. }
        | Ask::HandBackPerson { .. }
        | Ask::AnswerPerson { .. }
        | Ask::Stop { .. }
        | Ask::Cancel { .. }
        | Ask::Hold { .. }
        | Ask::Lift { .. }
        | Ask::Release { .. } => true,
    }
}

fn decided(domain: &mut Domain, env: &Env<Limits>, id: Id<Pending>, outcome: Outcome, out: &mut Queue<Request>) {
    if let Pending::Act(_) = domain.pending.get(id).expect("pending decision") {
        return acts::decided(domain, env, id, outcome, out);
    }
    let party = domain.party_pending(id).expect("pending decision").key.person;
    if let Outcome::MarkedSeen { seen } = outcome {
        mark_seen(domain, &env.limits, party, seen, out);
    }
    answer_decided(domain, env, id, outcome, out);
}

pub(super) fn answer_decided(
    domain: &mut Domain,
    env: &Env<Limits>,
    id: Id<Pending>,
    outcome: Outcome,
    out: &mut Queue<Request>,
) {
    let flight = domain.party_pending_mut(id).expect("root returns each routed request exactly once");
    let key = flight.key;
    let ask = flight.ask.clone();
    let models = flight.models.clone();
    let replies = core::mem::replace(&mut flight.replies, List::with_capacity(0));
    if let (
        Ask::AccountChange { holder, change: crate::AccountChange::MoveOut { .. } },
        Outcome::AccountMovingOut { account, number, source },
    ) = (&ask, &outcome)
    {
        *domain.pending.get_mut(id).expect("admitted move") =
            Pending::MoveOut { holder: *holder, account: *account, number: *number, source: *source };
    } else {
        domain.pending.retire(id);
        let removed = domain.flights.remove(&key);
        assert!(removed == Some(id), "flight has one key index");
    }

    match outcome {
        // A refused admission is retryable with the same key, including
        // pressure reported by tasks or another child through the root.
        Outcome::Again(_) | Outcome::SignedOut => {}
        Outcome::Compound { .. }
        | Outcome::CompoundRefused { .. }
        | Outcome::MarkedSeen { .. }
        | Outcome::Applied { .. }
        | Outcome::NoteEdited { .. }
        | Outcome::ServiceMade { .. }
        | Outcome::Adopted { .. }
        | Outcome::RolesSet { .. }
        | Outcome::PolicyChanged { .. }
        | Outcome::Renamed { .. }
        | Outcome::ProjectMade { .. }
        | Outcome::SettingsChanged { .. }
        | Outcome::MovedToNewest { .. }
        | Outcome::PoolSet { .. }
        | Outcome::Said { .. }
        | Outcome::QuestionAnswered { .. }
        | Outcome::Amended { .. }
        | Outcome::Moved { .. }
        | Outcome::Stopped { .. }
        | Outcome::Cancelled { .. }
        | Outcome::Placed { .. }
        | Outcome::Lifted { .. }
        | Outcome::Released { .. }
        | Outcome::Created { .. }
        | Outcome::Proposed { .. }
        | Outcome::ReleaseProposed { .. }
        | Outcome::Withdrawn { .. }
        | Outcome::PersonTaken { .. }
        | Outcome::PersonHandedBack { .. }
        | Outcome::PersonAnswered { .. }
        | Outcome::EscalationDecided { .. }
        | Outcome::ProposalDecided { .. }
        | Outcome::AccountSignInStarted { .. }
        | Outcome::AccountProgramDeclared { .. }
        | Outcome::AccountKeyKept { .. }
        | Outcome::AccountMovingOut { .. }
        | Outcome::AccountFinishing { .. }
        | Outcome::AccountMovedIn { .. }
        | Outcome::AccountChanged
        | Outcome::AccountRemoved { .. }
        | Outcome::AccountSignedOut { .. }
        | Outcome::Refused(_) => save_answer(domain, env, key, ask, models, outcome.clone(), out),
    }
    for to in replies.into_boxed() {
        out.push(Request::Reply { to, reply: outcome_reply(outcome.clone()) });
    }
}

fn restore(domain: &mut Domain, env: &Env<Limits>, record: Stored, out: &mut Queue<Request>) {
    match domain.phase {
        Phase::Ready => {
            return restore_failed(domain, record.key(), Refusal::Limit { which: LimitKind::Restore }, out);
        }
        Phase::Failed => return,
        Phase::Restoring => {}
    }
    let key = record.key();
    let valid = match &record {
        Stored::Acted { key, act, outcome, at } => {
            acts::valid(&env.limits, act).is_ok()
                && valid_program_outcome(&env.limits, outcome)
                && acts::valid_outcome(&env.limits, act, outcome)
                && !domain.acted.contains_key(key)
                && (answer_expiry(*at, &env.limits) <= env.wall
                    || domain.answers.len().saturating_add(domain.acted.len()) < env.limits.requests)
        }
        Stored::Person { number, identity } => {
            valid_identity(&env.limits, identity)
                && !domain.people.contains_key(number)
                && !domain.identities.contains_key(&identity.key)
                && domain.people.len() < env.limits.people
        }
        Stored::ReadResults { party: person, through: position } => {
            person != &0
                && *position != 0
                && !domain.results_seen.contains_key(person)
                && domain.results_seen.len() < env.limits.people
        }
        Stored::ReadConversation { party, task, through } => {
            *party != 0
                && *task != 0
                && *through != 0
                && !domain.conversations_seen.contains_key(&(*party, *task))
                && domain.conversations_seen.len() < domain.conversations_seen.capacity()
        }
        Stored::SignIn { number, .. } => {
            !domain.sign_ins.contains_key(number) && domain.sign_ins.len() < env.limits.sign_ins
        }
        Stored::Roles { project, holdings } => {
            valid_roles(domain, &env.limits, *project, holdings).is_ok() && !domain.roles.contains_key(project)
        }
        Stored::Policy { .. } | Stored::EndedRead { .. } => false,
        Stored::Answer { key, ask, models, outcome, at } => {
            valid_ask(&env.limits, ask)
                && crate::batch::restore_shaped(ask)
                && crate::valid_model_context(ask, models)
                && valid_program_outcome(&env.limits, outcome)
                && valid_answer_shape(ask, outcome.clone())
                && !domain.answers.contains_key(key)
                && (answer_expiry(*at, &env.limits) <= env.wall
                    || domain.answers.len().saturating_add(domain.acted.len()) < env.limits.requests)
        }
    };
    if !valid {
        return restore_failed(domain, key, Refusal::Limit { which: LimitKind::Restore }, out);
    }
    match record {
        Stored::Acted { key, act, outcome, at } => acts::restore(domain, env, key, act, outcome, at, out),
        Stored::Person { number, identity } => {
            let indexed = domain.identities.insert(identity.key.clone(), number);
            assert!(indexed == Ok(None), "restored identity admitted");
            let saved = domain.people.insert(number, identity);
            assert!(saved.is_ok(), "restored person admitted");
        }
        Stored::ReadResults { party: person, through: position } => {
            let saved = domain.results_seen.insert(person, position);
            assert!(saved == Ok(None), "restored read position admitted");
        }
        Stored::ReadConversation { party, task, through } => {
            assert!(
                domain.conversations_seen.insert((party, task), through) == Ok(None),
                "restored conversation position admitted"
            );
        }
        Stored::SignIn { number, person, expires } => {
            let saved = domain.sign_ins.insert(number, SignIn { person, expires, due: deadline(env, expires) });
            assert!(saved == Ok(None), "restored sign-in admitted");
            assert!(domain.expiries.insert((expires, number)).is_ok(), "restored expiry admitted");
        }
        Stored::Roles { project, holdings } => {
            let saved = domain.roles.insert(project, holdings);
            assert!(saved.is_ok(), "restored roles admitted");
        }
        Stored::Policy { .. } | Stored::EndedRead { .. } => {
            unreachable!("root does not restore on-demand records into people")
        }
        Stored::Answer { key, ask, models, outcome, at } => {
            if answer_expiry(at, &env.limits) <= env.wall {
                out.push(Request::Erase { key: Key::Answer(key) });
            } else {
                let due = deadline(env, answer_expiry(at, &env.limits));
                let saved = domain.answers.insert(key, Answered { ask: *ask, models, outcome, at, due });
                assert!(saved.is_ok(), "restored answer admitted");
                let armed = domain.answer_alarms.arm(key, due);
                assert!(armed.is_ok(), "one expiry per restored answer");
            }
        }
    }
}

fn restore_failed(domain: &mut Domain, key: Key, refusal: Refusal, out: &mut Queue<Request>) {
    domain.phase = Phase::Failed;
    out.push(Request::RestoreRefused { key, refusal });
}

#[expect(clippy::too_many_lines, reason = "all restored references are validated before requests open")]
fn restored(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    match domain.phase {
        Phase::Ready | Phase::Failed => return,
        Phase::Restoring => {}
    }
    // Restore order is immaterial; validate references once all rows arrived.
    for (number, signin) in &domain.sign_ins {
        if !domain.people.contains_key(&signin.person) {
            return restore_failed(domain, Key::SignIn(*number), Refusal::Unknown, out);
        }
    }
    for (project, holdings) in &domain.roles {
        for holding in &**holdings {
            if !domain.people.contains_key(&holding.person) {
                return restore_failed(domain, Key::Roles(*project), Refusal::Unknown, out);
            }
        }
    }
    for (key, answer) in &domain.answers {
        if !domain.people.contains_key(&key.person) {
            return restore_failed(domain, Key::Answer(*key), Refusal::Unknown, out);
        }
        match answer.outcome {
            Outcome::ServiceMade { person } => {
                let Some(identity) = domain.people.get(&person) else {
                    return restore_failed(domain, Key::Answer(*key), Refusal::Unknown, out);
                };
                if identity.key.provider != domain.deployment_provider {
                    return restore_failed(domain, Key::Answer(*key), Refusal::Unknown, out);
                }
            }
            Outcome::RolesSet { .. } => match &answer.ask {
                Ask::SetRoles { project, holdings } => {
                    if !domain.roles.contains_key(project) {
                        return restore_failed(domain, Key::Answer(*key), Refusal::Unknown, out);
                    }
                    for holding in holdings {
                        if !domain.people.contains_key(&holding.person) {
                            return restore_failed(domain, Key::Answer(*key), Refusal::Unknown, out);
                        }
                    }
                }
                Ask::Compound { .. }
                | Ask::MarkSeen { .. }
                | Ask::Application { .. }
                | Ask::EditNote { .. }
                | Ask::MakeService { .. }
                | Ask::Adopt { .. }
                | Ask::ChangePolicy { .. }
                | Ask::Rename { .. }
                | Ask::MakeProject { .. }
                | Ask::ChangeSettings { .. }
                | Ask::MoveToNewest { .. }
                | Ask::SetPool { .. }
                | Ask::DecideEscalation { .. }
                | Ask::DecideProposal { .. }
                | Ask::Say { .. }
                | Ask::AnswerQuestion { .. }
                | Ask::Amend { .. }
                | Ask::Move { .. }
                | Ask::TakePerson { .. }
                | Ask::HandBackPerson { .. }
                | Ask::AnswerPerson { .. }
                | Ask::Stop { .. }
                | Ask::Cancel { .. }
                | Ask::Hold { .. }
                | Ask::Lift { .. }
                | Ask::Release { .. }
                | Ask::Create { .. }
                | Ask::Propose { .. }
                | Ask::Withdraw { .. }
                | Ask::AccountSignIn { .. }
                | Ask::AccountSignOut { .. }
                | Ask::AccountChange { .. } => {
                    unreachable!("restored role success has a matching roster ask");
                }
            },
            Outcome::Compound { .. }
            | Outcome::CompoundRefused { .. }
            | Outcome::MarkedSeen { .. }
            | Outcome::Applied { .. }
            | Outcome::NoteEdited { .. }
            | Outcome::Adopted { .. }
            | Outcome::PolicyChanged { .. }
            | Outcome::Renamed { .. }
            | Outcome::ProjectMade { .. }
            | Outcome::SettingsChanged { .. }
            | Outcome::MovedToNewest { .. }
            | Outcome::PoolSet { .. }
            | Outcome::Said { .. }
            | Outcome::QuestionAnswered { .. }
            | Outcome::Amended { .. }
            | Outcome::Moved { .. }
            | Outcome::PersonTaken { .. }
            | Outcome::PersonHandedBack { .. }
            | Outcome::PersonAnswered { .. }
            | Outcome::Stopped { .. }
            | Outcome::Cancelled { .. }
            | Outcome::Placed { .. }
            | Outcome::Lifted { .. }
            | Outcome::Released { .. }
            | Outcome::Created { .. }
            | Outcome::Proposed { .. }
            | Outcome::ReleaseProposed { .. }
            | Outcome::Withdrawn { .. }
            | Outcome::EscalationDecided { .. }
            | Outcome::ProposalDecided { .. }
            | Outcome::AccountSignInStarted { .. }
            | Outcome::AccountProgramDeclared { .. }
            | Outcome::AccountKeyKept { .. }
            | Outcome::AccountMovingOut { .. }
            | Outcome::AccountFinishing { .. }
            | Outcome::AccountMovedIn { .. }
            | Outcome::AccountChanged
            | Outcome::AccountRemoved { .. }
            | Outcome::AccountSignedOut { .. }
            | Outcome::Refused(_) => {}
            Outcome::Again(_) | Outcome::SignedOut => {
                return restore_failed(domain, Key::Answer(*key), Refusal::Limit { which: LimitKind::Restore }, out);
            }
        }
    }
    for (person, _) in &domain.results_seen {
        if !domain.people.contains_key(person) {
            return restore_failed(domain, Key::ReadResults { party: *person }, Refusal::Unknown, out);
        }
    }
    for ((party, task), _) in &domain.conversations_seen {
        if !domain.people.contains_key(party) {
            return restore_failed(domain, Key::ReadConversation { party: *party, task: *task }, Refusal::Unknown, out);
        }
    }
    domain.phase = Phase::Ready;
    // A bounded snapshot lets cleanup and projection mutate the table.
    let mut recovered = List::with_capacity(env.limits.sign_ins);
    for (number, signin) in &domain.sign_ins {
        recovered.push((*number, *signin)).expect("one snapshot row per admitted sign-in");
    }
    for (number, mut signin) in recovered.into_boxed() {
        if signin.expires <= env.wall {
            end_signin(domain, number, out);
        } else {
            signin.due = deadline(env, signin.expires);
            let saved = domain.sign_ins.insert(number, signin);
            assert!(saved.is_ok(), "restored sign-in already exists");
            arm(domain, env, number, signin.expires);
        }
    }
}

/// Judge a connector's core part as its admitted party, without allocating another key.
/// The caller completes this part before the application flight (domain/people.md, 5.1).
/// Explicit model selectors refuse `Lack::Models`: parts carry no model-name context yet.
pub fn application_part(domain: &mut Domain, env: &Env<Limits>, request: Token, ask: Ask) -> Result<Request, Outcome> {
    let id = Id::from_token(request);
    let flight = domain.party_pending(id).ok_or(Outcome::Refused(Refusal::Unknown))?;
    let application_project = match &flight.ask {
        Ask::Application { scope, .. } => match scope {
            crate::ApplicationScope::Project(project) => *project,
            crate::ApplicationScope::Own | crate::ApplicationScope::Deployment => 0,
        },
        Ask::Compound { .. }
        | Ask::MarkSeen { .. }
        | Ask::MakeService { .. }
        | Ask::Adopt { .. }
        | Ask::SetRoles { .. }
        | Ask::ChangePolicy { .. }
        | Ask::Rename { .. }
        | Ask::MakeProject { .. }
        | Ask::ChangeSettings { .. }
        | Ask::MoveToNewest { .. }
        | Ask::SetPool { .. }
        | Ask::Create { .. }
        | Ask::Propose { .. }
        | Ask::Withdraw { .. }
        | Ask::Say { .. }
        | Ask::AnswerQuestion { .. }
        | Ask::Amend { .. }
        | Ask::DecideEscalation { .. }
        | Ask::DecideProposal { .. }
        | Ask::Move { .. }
        | Ask::TakePerson { .. }
        | Ask::HandBackPerson { .. }
        | Ask::AnswerPerson { .. }
        | Ask::Stop { .. }
        | Ask::Cancel { .. }
        | Ask::Hold { .. }
        | Ask::Lift { .. }
        | Ask::Release { .. }
        | Ask::EditNote { .. }
        | Ask::AccountSignIn { .. }
        | Ask::AccountSignOut { .. }
        | Ask::AccountChange { .. } => return Err(Outcome::Refused(Refusal::NotOffered)),
    };
    match &ask {
        // An account request is its holder's own and a sign-out waits on the accounts'
        // component, so neither is a synchronous part of an application's request.
        Ask::Application { .. }
        | Ask::AccountSignIn { .. }
        | Ask::AccountSignOut { .. }
        | Ask::AccountChange { .. } => {
            return Err(Outcome::Refused(Refusal::NotOffered));
        }
        Ask::Compound { .. }
        | Ask::MarkSeen { .. }
        | Ask::MakeService { .. }
        | Ask::Adopt { .. }
        | Ask::SetRoles { .. }
        | Ask::ChangePolicy { .. }
        | Ask::Rename { .. }
        | Ask::MakeProject { .. }
        | Ask::ChangeSettings { .. }
        | Ask::MoveToNewest { .. }
        | Ask::SetPool { .. }
        | Ask::Create { .. }
        | Ask::Propose { .. }
        | Ask::Withdraw { .. }
        | Ask::Say { .. }
        | Ask::AnswerQuestion { .. }
        | Ask::Amend { .. }
        | Ask::DecideEscalation { .. }
        | Ask::DecideProposal { .. }
        | Ask::Move { .. }
        | Ask::TakePerson { .. }
        | Ask::HandBackPerson { .. }
        | Ask::AnswerPerson { .. }
        | Ask::Stop { .. }
        | Ask::Cancel { .. }
        | Ask::Hold { .. }
        | Ask::Lift { .. }
        | Ask::Release { .. }
        | Ask::EditNote { .. } => {}
    }
    if project(&ask) != application_project {
        return Err(Outcome::Refused(Refusal::Unknown));
    }
    if !valid_ask(&env.limits, &ask) {
        return Err(Outcome::Refused(invalid_ask(&env.limits, &ask)));
    }
    if crate::models::has_model_selector(&ask) {
        return Err(Outcome::Refused(Refusal::Authority { lacking: crate::Lack::Models, proposable: None }));
    }
    let person = flight.key.person;
    let role = role(domain, person, application_project);
    if let Some(why) = admission(domain, person, application_project, role, &ask) {
        return Err(Outcome::Refused(why));
    }
    let flight = domain.party_pending_mut(id).expect("admitted application flight");
    assert!(flight.part.is_none(), "one core part at a time");
    flight.part = Some(ask.clone());
    Ok(Request::Route { request, person, project: application_project, role, ask: Box::new(ask), models: Box::new([]) })
}

fn admission(domain: &Domain, person: u64, _project: u32, role: Option<Role>, ask: &Ask) -> Option<Refusal> {
    match ask {
        Ask::Propose { project, action: crate::Action::Settings { .. }, .. } => {
            if *project != crate::DEPLOYMENT {
                return Some(Refusal::Unknown);
            }
            for (_, holdings) in &domain.roles {
                for holding in &**holdings {
                    if holding.person == person {
                        return None;
                    }
                }
            }
            Some(Refusal::Role { held: None })
        }
        Ask::ChangeSettings { .. } | Ask::MoveToNewest { .. } => {
            if role == Some(Role::Owner) {
                None
            } else {
                Some(Refusal::Role { held: role })
            }
        }
        Ask::Application { scope, .. } => {
            if *scope == crate::ApplicationScope::Own || role.is_some() {
                None
            } else {
                Some(Refusal::Role { held: None })
            }
        }
        Ask::AccountSignIn { holder, .. } | Ask::AccountSignOut { holder, .. } | Ask::AccountChange { holder, .. } => {
            if holds(domain, person, *holder) {
                None
            } else {
                Some(Refusal::Role { held: None })
            }
        }
        Ask::Compound { .. }
        | Ask::EditNote { .. }
        | Ask::MakeService { .. }
        | Ask::Adopt { .. }
        | Ask::SetRoles { .. }
        | Ask::ChangePolicy { .. }
        | Ask::Rename { .. }
        | Ask::MakeProject { .. }
        | Ask::SetPool { .. }
        | Ask::Create { .. }
        | Ask::Propose { .. }
        | Ask::Withdraw { .. }
        | Ask::Say { .. }
        | Ask::AnswerQuestion { .. }
        | Ask::Amend { .. }
        | Ask::MarkSeen { .. }
        | Ask::DecideEscalation { .. }
        | Ask::DecideProposal { .. }
        | Ask::Move { .. }
        | Ask::TakePerson { .. }
        | Ask::HandBackPerson { .. }
        | Ask::AnswerPerson { .. }
        | Ask::Stop { .. }
        | Ask::Cancel { .. }
        | Ask::Hold { .. }
        | Ask::Lift { .. }
        | Ask::Release { .. } => None,
    }
}

/// Whether this party is the holder an account request names: a person for their own, a first
/// owner for the deployment's (domain/accounts.md, 3.2; domain/people.md, section 3). A service
/// holds no account and decides none of the deployment's.
fn holds(domain: &Domain, person: u64, holder: crate::AccountHolder) -> bool {
    if domain.kind(person) != Some(Kind::Person) {
        return false;
    }
    match holder {
        crate::AccountHolder::Own => true,
        crate::AccountHolder::Deployment => {
            let Some(identity) = domain.identity(person) else { return false };
            for owner in &domain.owners {
                if owner.identity == identity.key {
                    return true;
                }
            }
            false
        }
    }
}

/// Forget the synchronous part while preserving its application key and digest.
pub fn application_part_done(domain: &mut Domain, request: Token) {
    if let Some(flight) = domain.party_pending_mut(Id::from_token(request)) {
        flight.part = None;
    }
}

fn valid_program_outcome(limits: &Limits, outcome: &Outcome) -> bool {
    if let Outcome::Refused(why) | Outcome::CompoundRefused { why, .. } = outcome {
        if let Refusal::Circle { tasks, .. } | Refusal::Full { tasks, .. } = why {
            return tasks.len() <= usize::try_from(crate::NAMED_TASKS).expect("named tasks bound");
        }
        if let Refusal::Kept { pending } = why {
            return pending.len() <= 64;
        }
    }

    let (confirmed, rows) = match outcome {
        Outcome::AccountSignedOut { confirmed, failed_hosts } | Outcome::AccountRemoved { confirmed, failed_hosts } => {
            (*confirmed, failed_hosts)
        }
        Outcome::AccountProgramDeclared { name, .. } => return crate::program_name(name),
        Outcome::Compound { .. }
        | Outcome::CompoundRefused { .. }
        | Outcome::MarkedSeen { .. }
        | Outcome::Applied { .. }
        | Outcome::NoteEdited { .. }
        | Outcome::ServiceMade { .. }
        | Outcome::Adopted { .. }
        | Outcome::Created { .. }
        | Outcome::Proposed { .. }
        | Outcome::ReleaseProposed { .. }
        | Outcome::Withdrawn { .. }
        | Outcome::Stopped { .. }
        | Outcome::Cancelled { .. }
        | Outcome::Placed { .. }
        | Outcome::Lifted { .. }
        | Outcome::Released { .. }
        | Outcome::PersonTaken { .. }
        | Outcome::PersonHandedBack { .. }
        | Outcome::PersonAnswered { .. }
        | Outcome::Moved { .. }
        | Outcome::ProposalDecided { .. }
        | Outcome::Said { .. }
        | Outcome::QuestionAnswered { .. }
        | Outcome::Amended { .. }
        | Outcome::RolesSet { .. }
        | Outcome::PolicyChanged { .. }
        | Outcome::Renamed { .. }
        | Outcome::ProjectMade { .. }
        | Outcome::SettingsChanged { .. }
        | Outcome::MovedToNewest { .. }
        | Outcome::PoolSet { .. }
        | Outcome::EscalationDecided { .. }
        | Outcome::AccountSignInStarted { .. }
        | Outcome::AccountKeyKept { .. }
        | Outcome::AccountMovingOut { .. }
        | Outcome::AccountFinishing { .. }
        | Outcome::AccountMovedIn { .. }
        | Outcome::AccountChanged
        | Outcome::SignedOut
        | Outcome::Again(_)
        | Outcome::Refused(_) => return true,
    };
    if rows.len() > usize::try_from(limits.program_hosts).unwrap_or(0) || confirmed && !rows.is_empty() {
        return false;
    }
    for at in 0..rows.len() {
        let Some(row) = rows.get(at) else { return false };
        if row.name.len() > usize::try_from(limits.program_host_name_bytes).unwrap_or(0) {
            return false;
        }
        let Some(prefix) = rows.get(..at) else { return false };
        for earlier in prefix {
            if earlier.name == row.name {
                return false;
            }
        }
    }
    true
}
