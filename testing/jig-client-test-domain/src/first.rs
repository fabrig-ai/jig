//! First connector's client side. Parts and family objects have one working
//! set, owned reads/watches and revision-naming acts (root.md, sections 6–8).
//! Departures stay beside the supplied document and keyed answer. Busy,
//! stopping and dropped watches wait for the supplied retry deadline; signed
//! out watches park until confirmation (contract.md, sections 6–7).
pub mod me;
pub mod proposal;
use crate::family::{self, Document, Kind, Scope, Subject};
pub use crate::family::{Answer, Ask, ReadResult, WatchEnd, WatchEvent};
use jig_client::Link;
use jig_client::contract::{Part, Revision};
use skein_lib::{Env, Id, List, Map, Queue, RequestProgress, Slab, Time, Token};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Address {
    pub project: u32,
    pub item: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    Go { address: Address },
    Proposal(proposal::Act),
    Touch { object: Token, revision: Revision },
    Parameters { object: Token, revision: Revision, value: u64 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Place,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Record {
    Place { version: u16, party: u64, address: Address },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Out {
    Read { read: Token, subject: Subject },
    Open { watch: Token, subject: Subject },
    Close { watch: Token },
    Fact { fact: Fact },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fact {
    Full,
    InvalidDocument,
    SignedOut,
}
#[derive(Debug)]
pub enum Event {
    Show { part: Part, scope: Scope },
    PartsLeft,
    Went { address: Address },
    Left,
    Act { act: Act },
    Answered { request: Token, answer: Answer },
    Progress { request: Token, progress: RequestProgress },
    Read { read: Token, result: ReadResult },
    Watch { watch: Token, event: WatchEvent },
    Ended { watch: Token, end: WatchEnd },
    Restore { record: Record },
}
#[derive(Debug)]
pub enum Request {
    Client { asked: jig_client::Event },
    Show { address: Address },
    Save { record: Record },
    Send { request: Token, scope: u64, ask: Ask },
    Out(Out),
    Named { objects: List<jig_client::contract::ObjectKey>, sections: List<jig_client_view::page::Section> },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub parts: u32,
    pub objects: u32,
    pub reads: u32,
    pub watches: u32,
    pub requests: u32,
    pub rows: u32,
    pub segments: u32,
    pub text: u32,
    pub connector: crate::connector_values::ConnectorLimits,
    pub retry: skein_lib::Duration,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Loading,
    Unknown,
    Unsupported,
    Known(Document),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Card {
    Open,
    Deciding { request: Token, progress: RequestProgress },
    Refused { answer: Answer },
    Unknown,
    Done { answer: Answer },
}
/// A received departure, separate from a connector document or keyed answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FamilyTerminal {
    /// The client left the watch.
    Closed,
    /// Connection recovery waits for the retry deadline.
    Dropped,
    /// No visible subject has this name.
    Unknown,
    /// The owner cannot represent this subject.
    Unsupported,
    /// Work stays parked until confirmation.
    SignedOut,
    /// The party may not see the subject.
    NotAllowed,
    /// The subject ended and left the engine.
    SubjectEnded,
    /// The engine is stopping; recovery waits for the retry deadline.
    Stopping,
    /// Admission is busy; recovery waits for the retry deadline.
    Busy,
}
impl FamilyTerminal {
    /// Departures and parking prevent acquisition and mutation.
    #[must_use]
    pub const fn unavailable(self) -> bool {
        match self {
            Self::Unknown | Self::Unsupported | Self::NotAllowed | Self::SubjectEnded | Self::SignedOut => true,
            Self::Closed | Self::Dropped | Self::Stopping | Self::Busy => false,
        }
    }
}
#[derive(Debug)]
#[expect(clippy::partial_pub_fields, reason = "the view reads values; the side alone owns read and watch obligations")]
pub struct Object {
    pub subject: Subject,
    pub value: Value,
    pub card: Card,
    pub draft: Option<u64>,
    /// Last received departure; a fresh admitted document clears it.
    pub terminal: Option<FamilyTerminal>,
    read: Option<Id<Reading>>,
    watch: Option<Id<Watching>>,
}
#[derive(Debug)]
struct Held {
    part: Part,
    subject: Option<Subject>,
}
#[derive(Debug)]
struct Reading {
    live: bool,
    subject: Subject,
    party: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Following {
    Live,
    Waiting,
    Dropped { again: Time },
    Closing,
}
#[derive(Debug)]
struct Watching {
    subject: Subject,
    state: Following,
}
#[derive(Debug)]
struct Pending {
    live: bool,
    object: Id<Object>,
}
#[derive(Debug)]
pub struct Side {
    parts: List<Held>,
    objects: Slab<Object>,
    names: Map<Subject, Id<Object>>,
    reads: Slab<Reading>,
    watches: Slab<Watching>,
    watch_ids: Map<Id<Watching>, ()>,
    pending: Slab<Pending>,
    address: Option<Address>,
    generation: u32,
    link: Link,
    party: Option<u64>,
    me: bool,
    kept: Option<Record>,
    shown: u64,
    proposal: Option<proposal::Choice>,
    proposal_sequence: u64,
}
impl Side {
    #[must_use]
    pub fn new(limits: &Limits, seed: u64) -> Self {
        Self {
            parts: List::with_capacity(limits.parts),
            objects: Slab::with_capacity(limits.objects.checked_mul(2).expect("object retirement room fits")),
            names: Map::with_capacity(limits.objects),
            reads: Slab::with_capacity(limits.reads),
            watches: Slab::with_capacity(limits.watches),
            watch_ids: Map::with_capacity(limits.watches),
            pending: Slab::with_capacity(limits.requests),
            address: None,
            generation: 0,
            link: Link::Starting,
            party: None,
            me: false,
            kept: None,
            shown: 0,
            proposal: None,
            proposal_sequence: seed,
        }
    }
    pub(crate) fn read_subject(&self, read: Token) -> Option<Subject> {
        Some(self.reads.get(Id::from_token(read))?.subject)
    }
    pub(crate) fn watch_subject(&self, watch: Token) -> Option<Subject> {
        Some(self.watches.get(Id::from_token(watch))?.subject)
    }
    #[must_use]
    pub const fn address(&self) -> Option<Address> {
        self.address
    }
    #[must_use]
    pub const fn generation(&self) -> u32 {
        self.generation
    }
    #[must_use]
    pub const fn link(&self) -> Link {
        self.link
    }
    #[must_use]
    pub const fn party(&self) -> Option<u64> {
        self.party
    }
    #[must_use]
    pub fn find(&self, subject: Subject) -> Option<Id<Object>> {
        self.names.get(&subject).copied()
    }
    #[must_use]
    pub fn object(&self, id: Id<Object>) -> Option<&Object> {
        let object = self.objects.get(id)?;
        if self.names.get(&object.subject) == Some(&id) { Some(object) } else { None }
    }
    #[must_use]
    pub fn page(&self) -> Option<Id<Object>> {
        let address = self.address?;
        self.find(Subject { scope: Scope::Project(address.project), kind: Kind::Item(address.item) })
    }
    #[must_use]
    pub fn part(&self, part: &Part) -> Option<&Object> {
        for held in &self.parts {
            if &held.part == part {
                let subject = held.subject?;
                return self.object(self.find(subject)?);
            }
        }
        None
    }
    #[must_use]
    pub fn contains(&self, part: &Part) -> bool {
        for held in &self.parts {
            if &held.part == part {
                return true;
            }
        }
        false
    }
    fn changed(&mut self) {
        self.shown = self.shown.checked_add(1).expect("shown count fits");
    }
}
#[must_use]
pub const fn shown(side: &Side) -> u64 {
    side.shown
}
pub fn step(side: &mut Side, env: &Env<Limits>, event: Event, out: &mut Queue<Request>) {
    match event {
        Event::Show { part, scope } => {
            if !part_fits(&part, env.limits.segments, env.limits.text) {
                full(out);
                return;
            }
            if side.contains(&part) {
                return;
            }
            let subject = family::subject(&part, scope);
            if side.parts.push(Held { part, subject }).is_err() {
                full(out);
                return;
            }
            if let Some(subject) = subject {
                hold(side, env, subject, out);
            }
            side.changed();
        }
        Event::PartsLeft => {
            side.parts.clear();
            prune(side, out);
            side.changed();
        }
        Event::Went { address } => {
            side.address = Some(address);
            side.generation = side.generation.checked_add(1).expect("page generation fits");
            prune(side, out);
            side.changed();
            hold(side, env, Subject { scope: Scope::Project(address.project), kind: Kind::Item(address.item) }, out);
            save(side, out);
        }
        Event::Left => {
            side.address = None;
            out.push(Request::Named { objects: List::with_capacity(0), sections: List::with_capacity(0) });
            prune(side, out);
            side.changed();
        }
        Event::Act { act } => match act {
            Act::Proposal(act) => proposal::act(side, act, out),
            Act::Go { address } => out.push(Request::Show { address }),
            Act::Touch { object, revision } => touch(side, Id::from_token(object), revision, out),
            Act::Parameters { object, revision, value } => {
                let id = Id::from_token(object);
                if let Some(object) = side.objects.get_mut(id)
                    && !unavailable(object)
                    && parameters_revision(&object.value) == Some(revision)
                {
                    object.draft = Some(value);
                    side.changed();
                }
            }
        },
        Event::Answered { request, answer } => answered(side, env, request, answer, out),
        Event::Progress { request, progress } => progress_changed(side, env, request, progress, out),
        Event::Read { read, result } => read_result(side, env, read, result, out),
        Event::Watch { watch, event } => watched(side, env, watch, event, out),
        Event::Ended { watch, end } => ended(side, env, Id::from_token(watch), end, out),
        Event::Restore { record } => match record {
            Record::Place { version, party, address } => {
                if version == 1 {
                    side.kept = Some(Record::Place { version, party, address });
                }
            }
        },
    }
    prune(side, out);
}
fn save(side: &mut Side, out: &mut Queue<Request>) {
    if let Some(party) = side.party
        && let Some(address) = side.address
    {
        let record = Record::Place { version: 1, party, address };
        side.kept = Some(record.clone());
        out.push(Request::Save { record });
    }
}
#[expect(clippy::manual_map, reason = "step code matches optional values explicitly")]
fn parameters_revision(value: &Value) -> Option<Revision> {
    match value {
        Value::Known(Document::Part(part)) => match part.parameters {
            Some(parameters) => Some(parameters.revision),
            None => None,
        },
        Value::Known(Document::Connector(value)) => match value.value.as_ref() {
            crate::connector_values::ClientDocument::Object { now, .. } => Some(Revision { value: now.revision }),
            crate::connector_values::ClientDocument::Panel { .. }
            | crate::connector_values::ClientDocument::Result { .. }
            | crate::connector_values::ClientDocument::Effect { .. }
            | crate::connector_values::ClientDocument::Outbox { .. }
            | crate::connector_values::ClientDocument::Hold { .. }
            | crate::connector_values::ClientDocument::Project { .. }
            | crate::connector_values::ClientDocument::Deployment { .. } => None,
        },
        Value::Known(Document::Item(_)) | Value::Loading | Value::Unknown | Value::Unsupported => None,
    }
}
fn hold(side: &mut Side, env: &Env<Limits>, subject: Subject, out: &mut Queue<Request>) {
    if side.find(subject).is_none() {
        if side.names.len() == side.names.capacity() {
            full(out);
            return;
        }
        let object = Object {
            subject,
            value: Value::Loading,
            card: Card::Open,
            draft: None,
            terminal: None,
            read: None,
            watch: None,
        };
        let Ok(id) = side.objects.insert(object) else {
            full(out);
            return;
        };
        assert!(side.names.insert(subject, id).is_ok(), "names mirror object capacity");
    }
    acquire(side, env, subject, out);
}
fn acquire(side: &mut Side, _env: &Env<Limits>, subject: Subject, out: &mut Queue<Request>) {
    if side.party.is_none() || side.link != Link::Live {
        return;
    }
    let Some(id) = side.find(subject) else {
        return;
    };
    let object = side.objects.get_mut(id).expect("named object exists");
    if unavailable(object) {
        return;
    }
    match object.value {
        Value::Unknown | Value::Unsupported => return,
        Value::Known(_) | Value::Loading => {}
    }
    if object.read.is_none() && matches_loading(&object.value) {
        match side.reads.insert(Reading {
            live: true,
            subject,
            party: side.party.expect("only a confirmed party reads"),
        }) {
            Ok(read) => {
                object.read = Some(read);
                out.push(Request::Out(Out::Read { read: read.token(), subject }));
            }
            Err(_) => full(out),
        }
    }
    if object.watch.is_none() {
        match side.watches.insert(Watching { subject, state: Following::Waiting }) {
            Ok(watch) => {
                assert!(side.watch_ids.insert(watch, ()).is_ok(), "watch index mirrors slab");
                object.watch = Some(watch);
                out.push(Request::Out(Out::Open { watch: watch.token(), subject }));
            }
            Err(_) => full(out),
        }
    }
}
fn matches_loading(value: &Value) -> bool {
    match value {
        Value::Loading => true,
        Value::Known(_) | Value::Unknown | Value::Unsupported => false,
    }
}
fn received(side: &mut Side, env: &Env<Limits>, subject: Subject, document: Document, out: &mut Queue<Request>) {
    let fits = match &document {
        Document::Connector(value) => {
            value.matches_subject() && crate::connector_bounds::document_fits(value, &env.limits.connector)
        }
        Document::Item(_) | Document::Part(_) => document.fits(env.limits.rows),
    };
    if document.subject() != subject || !fits {
        out.push(Request::Out(Out::Fact { fact: Fact::InvalidDocument }));
        return;
    }
    let Some(id) = side.find(subject) else {
        return;
    };
    let object = side.objects.get_mut(id).expect("named object exists");
    match &object.value {
        Value::Known(current) => {
            if let Some(before) = current.revision()
                && let Some(now) = document.revision()
            {
                if before.value > now.value {
                    return;
                }
                if before != now {
                    object.draft = None;
                }
            } else {
                // An unversioned snapshot has no revision which can retain a draft.
                object.draft = None;
            }
        }
        Value::Loading | Value::Unknown | Value::Unsupported => {}
    }
    object.value = Value::Known(document);
    object.terminal = None;
    match object.card {
        Card::Unknown => object.card = Card::Open,
        Card::Open | Card::Deciding { .. } | Card::Refused { .. } | Card::Done { .. } => {}
    }
    if side.page() == Some(id) {
        let object = side.objects.get(id).expect("page object exists");
        match &object.value {
            Value::Known(document) => {
                out.push(Request::Named { objects: document.objects(env.limits.rows), sections: document.sections() });
            }
            Value::Loading | Value::Unknown | Value::Unsupported => {}
        }
    }
    side.changed();
}
fn unknown(side: &mut Side, subject: Subject) {
    departure(side, subject, FamilyTerminal::Unknown);
}
fn unsupported(side: &mut Side, subject: Subject) {
    departure(side, subject, FamilyTerminal::Unsupported);
}
fn departure(side: &mut Side, subject: Subject, terminal: FamilyTerminal) {
    if let Some(id) = side.find(subject) {
        let object = side.objects.get_mut(id).expect("named object exists");
        object.terminal = Some(terminal);
        match object.value {
            Value::Loading | Value::Unknown | Value::Unsupported => match terminal {
                FamilyTerminal::Unknown => object.value = Value::Unknown,
                FamilyTerminal::Unsupported => object.value = Value::Unsupported,
                FamilyTerminal::Closed
                | FamilyTerminal::Dropped
                | FamilyTerminal::SignedOut
                | FamilyTerminal::NotAllowed
                | FamilyTerminal::SubjectEnded
                | FamilyTerminal::Stopping
                | FamilyTerminal::Busy => {}
            },
            Value::Known(_) => {}
        }
        side.changed();
    }
}
/// Whether the received departure prevents acts on the retained value.
#[must_use]
pub fn unavailable(object: &Object) -> bool {
    match object.terminal {
        Some(terminal) => terminal.unavailable(),
        None => false,
    }
}
fn terminal(end: WatchEnd) -> FamilyTerminal {
    match end {
        WatchEnd::Closed => FamilyTerminal::Closed,
        WatchEnd::Dropped => FamilyTerminal::Dropped,
        WatchEnd::Unknown => FamilyTerminal::Unknown,
        WatchEnd::Unsupported => FamilyTerminal::Unsupported,
        WatchEnd::SignedOut => FamilyTerminal::SignedOut,
        WatchEnd::NotAllowed => FamilyTerminal::NotAllowed,
        WatchEnd::SubjectEnded => FamilyTerminal::SubjectEnded,
        WatchEnd::Stopping => FamilyTerminal::Stopping,
        WatchEnd::Busy => FamilyTerminal::Busy,
    }
}
fn named(side: &Side, subject: Subject) -> bool {
    if let Some(address) = side.address
        && subject == (Subject { scope: Scope::Project(address.project), kind: Kind::Item(address.item) })
    {
        return true;
    }
    for held in &side.parts {
        if held.subject == Some(subject) {
            return true;
        }
    }
    false
}
fn prune(side: &mut Side, out: &mut Queue<Request>) {
    let mut gone = List::with_capacity(side.names.capacity());
    for (subject, _) in &side.names {
        let object = side
            .objects
            .get(*side.names.get(subject).expect("registered subject exists"))
            .expect("named object exists");
        let pending = match object.card {
            Card::Deciding { .. } => true,
            Card::Open | Card::Refused { .. } | Card::Unknown | Card::Done { .. } => false,
        };
        if !named(side, *subject) && !pending {
            assert!(gone.push(*subject).is_ok(), "pruned keys fit names");
        }
    }
    for subject in gone.into_boxed() {
        let id = side.names.remove(&subject).expect("pruned subject exists");
        if let Some(object) = side.objects.get_mut(id) {
            if let Some(watch) = object.watch.take() {
                close(side, watch, out);
            }
            side.objects.retire(id);
        }
    }
}
fn close(side: &mut Side, id: Id<Watching>, out: &mut Queue<Request>) {
    if let Some(watch) = side.watches.get_mut(id) {
        match watch.state {
            Following::Closing => {}
            Following::Dropped { .. } => retire_watch(side, id),
            Following::Live | Following::Waiting => {
                watch.state = Following::Closing;
                out.push(Request::Out(Out::Close { watch: id.token() }));
            }
        }
    }
}
fn retire_watch(side: &mut Side, id: Id<Watching>) {
    let old = side.watch_ids.remove(&id);
    assert!(old.is_some(), "retired watch was indexed");
    side.watches.retire(id);
}
fn hide(side: &mut Side, out: &mut Queue<Request>) {
    let mut ids = List::with_capacity(side.watch_ids.capacity());
    for (id, ()) in &side.watch_ids {
        assert!(ids.push(*id).is_ok(), "watch ids fit");
    }
    for id in ids.into_boxed() {
        close(side, id, out);
    }
    for (_, id) in &side.names {
        let object = side.objects.get_mut(*id).expect("named object exists");
        object.value = Value::Loading;
    }
}
fn sync(side: &mut Side, env: &Env<Limits>, out: &mut Queue<Request>) {
    let mut subjects = List::with_capacity(side.names.capacity());
    for (subject, _) in &side.names {
        assert!(subjects.push(*subject).is_ok(), "subjects fit registry");
    }
    for subject in subjects.into_boxed() {
        acquire(side, env, subject, out);
    }
    if side.link != Link::Live {
        hide(side, out);
    }
}
fn ended(side: &mut Side, env: &Env<Limits>, id: Id<Watching>, end: WatchEnd, out: &mut Queue<Request>) {
    if side.watch_ids.get(&id).is_none() {
        return;
    }
    let Some(watch) = side.watches.get(id) else {
        return;
    };
    let subject = watch.subject;
    match watch.state {
        Following::Dropped { .. } => return,
        Following::Live | Following::Waiting | Following::Closing => {}
    }
    let closing = watch.state == Following::Closing;
    if closing {
        retire_watch(side, id);
    } else {
        departure(side, subject, terminal(end));
        match end {
            WatchEnd::Dropped | WatchEnd::Busy | WatchEnd::Stopping => {
                side.watches.get_mut(id).expect("watch exists").state =
                    Following::Dropped { again: env.now.saturating_add(env.limits.retry) }
            }
            WatchEnd::Unknown => {
                retire_watch(side, id);
                unknown(side, subject);
            }
            WatchEnd::Unsupported => {
                retire_watch(side, id);
                unsupported(side, subject);
            }
            WatchEnd::NotAllowed | WatchEnd::SubjectEnded | WatchEnd::Closed => retire_watch(side, id),
            WatchEnd::SignedOut => {
                retire_watch(side, id);
                out.push(Request::Out(Out::Fact { fact: Fact::SignedOut }));
            }
        }
    }
    let retrying = match end {
        WatchEnd::Dropped | WatchEnd::Busy | WatchEnd::Stopping => true,
        WatchEnd::Closed
        | WatchEnd::Unknown
        | WatchEnd::Unsupported
        | WatchEnd::SignedOut
        | WatchEnd::NotAllowed
        | WatchEnd::SubjectEnded => false,
    };
    if closing || !retrying {
        if let Some(object) = side.find(subject) {
            let object = side.objects.get_mut(object).expect("named object exists");
            if object.watch == Some(id) {
                object.watch = None;
            }
        }
        acquire(side, env, subject, out);
    }
    side.changed();
}
fn touch(side: &mut Side, id: Id<Object>, revision: Revision, out: &mut Queue<Request>) {
    let Some(party) = side.party else {
        return;
    };
    let Some(object) = side.object(id) else {
        return;
    };
    if unavailable(object) {
        return;
    }
    match object.card {
        Card::Deciding { .. } | Card::Unknown => return,
        Card::Open | Card::Refused { .. } | Card::Done { .. } => {}
    }
    let document = match &object.value {
        Value::Known(document) => document,
        Value::Loading | Value::Unknown | Value::Unsupported => return,
    };
    match document {
        Document::Connector(_) => {
            connector_touch(side, id, revision, party, out);
            return;
        }
        Document::Item(_) | Document::Part(_) => {}
    }
    let (project, item, current, allowed) = match document {
        Document::Item(item) => (item.project, item.key, item.revision, item.touchable),
        Document::Part(part) => {
            let Some(parameters) = part.parameters else {
                return;
            };
            let project = match part.subject.scope {
                Scope::Project(project) => project,
                Scope::Deployment => return,
            };
            (project, parameters.item, parameters.revision, parameters.amendable)
        }
        Document::Connector(_) => return,
    };
    if current != revision {
        side.objects.get_mut(id).expect("named object exists").card =
            Card::Refused { answer: Answer::Moved { item, revision: current } };
        side.changed();
        return;
    }
    if !allowed {
        return;
    }
    let parameters = object.draft;
    let Ok(request) = side.pending.insert(Pending { live: true, object: id }) else {
        full(out);
        return;
    };
    side.objects.get_mut(id).expect("named object exists").card =
        Card::Deciding { request: request.token(), progress: RequestProgress::Parked };
    out.push(Request::Send {
        request: request.token(),
        scope: party,
        ask: Ask::Touch { project, item, revision, parameters },
    });
    side.changed();
}
fn full(out: &mut Queue<Request>) {
    out.push(Request::Out(Out::Fact { fact: Fact::Full }));
}
pub fn fire(side: &mut Side, env: &Env<Limits>, out: &mut Queue<Request>) {
    for (id, ()) in &side.watch_ids {
        let watch = side.watches.get_mut(*id).expect("watch exists");
        match watch.state {
            Following::Dropped { again } => {
                if again <= env.now && side.link == Link::Live && side.party.is_some() {
                    watch.state = Following::Waiting;
                    out.push(Request::Out(Out::Open { watch: id.token(), subject: watch.subject }));
                }
            }
            Following::Waiting | Following::Live | Following::Closing => {}
        }
    }
}
#[must_use]
pub fn next_deadline(side: &Side) -> Option<Time> {
    let mut next: Option<Time> = None;
    for (id, ()) in &side.watch_ids {
        let watch = side.watches.get(*id).expect("watch exists");
        match watch.state {
            Following::Dropped { again } => {
                if side.link == Link::Live && side.party.is_some() {
                    next = Some(match next {
                        Some(old) => old.min(again),
                        None => again,
                    });
                }
            }
            Following::Waiting | Following::Live | Following::Closing => {}
        }
    }
    next
}
pub fn reclaim(side: &mut Side) {
    side.objects.reclaim();
    side.reads.reclaim();
    side.watches.reclaim();
    side.pending.reclaim();
}
/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    limits.retry != skein_lib::Duration::ZERO
        && limits.objects.checked_mul(2).is_some()
        && limits.rows.checked_add(1).is_some()
        && crate::connector_bounds::document_worst_case(&limits.connector).is_some()
}

fn read_result(side: &mut Side, env: &Env<Limits>, read: Token, result: ReadResult, out: &mut Queue<Request>) {
    let id = Id::from_token(read);
    let Some(reading) = side.reads.get_mut(id) else {
        return;
    };
    if !reading.live {
        return;
    }
    reading.live = false;
    let subject = reading.subject;
    let party = reading.party;
    side.reads.retire(id);
    let Some(object) = side.find(subject) else {
        return;
    };
    let object = side.objects.get_mut(object).expect("named object exists");
    if object.read != Some(id) {
        return;
    }
    object.read = None;
    if side.party != Some(party) {
        acquire(side, env, subject, out);
        return;
    }
    match result {
        ReadResult::Value(document) => received(side, env, subject, document, out),
        ReadResult::Unknown => unknown(side, subject),
        ReadResult::Unsupported => unsupported(side, subject),
        ReadResult::NotAllowed => departure(side, subject, FamilyTerminal::NotAllowed),
        ReadResult::Busy => full(out),
        ReadResult::SignedOut => out.push(Request::Out(Out::Fact { fact: Fact::SignedOut })),
    }
}

fn watched(side: &mut Side, env: &Env<Limits>, watch: Token, event: WatchEvent, out: &mut Queue<Request>) {
    let id = Id::from_token(watch);
    if side.watch_ids.get(&id).is_none() {
        return;
    }
    let Some(watching) = side.watches.get_mut(id) else {
        return;
    };
    match watching.state {
        Following::Closing | Following::Dropped { .. } => return,
        Following::Live | Following::Waiting => {}
    }
    let subject = watching.subject;
    watching.state = Following::Live;
    match event {
        WatchEvent::Opened | WatchEvent::Alive => {}
        WatchEvent::Delivered(document) => received(side, env, subject, document, out),
    }
}

fn answered(side: &mut Side, _env: &Env<Limits>, request: Token, answer: Answer, _out: &mut Queue<Request>) {
    let id = Id::from_token(request);
    if let Some(pending) = side.pending.get_mut(id) {
        if !pending.live {
            return;
        }
        let object = pending.object;
        pending.live = false;
        side.pending.retire(id);
        if let Some(object) = side.objects.get_mut(object) {
            object.card = match &answer {
                Answer::Touched { .. } => Card::Done { answer: answer.clone() },
                Answer::Moved { .. } | Answer::Gone | Answer::Refused(_) => Card::Refused { answer: answer.clone() },
                Answer::Connector(value) => match value.value {
                    crate::connector_values::ApplicationOutcome::Changed { .. } => Card::Done { answer },
                    crate::connector_values::ApplicationOutcome::Moved { .. }
                    | crate::connector_values::ApplicationOutcome::Role { .. }
                    | crate::connector_values::ApplicationOutcome::Unknown
                    | crate::connector_values::ApplicationOutcome::Exhausted
                    | crate::connector_values::ApplicationOutcome::CoreRefused { .. } => Card::Refused { answer },
                },
            };
            side.changed();
        }
    }
}

fn progress_changed(
    side: &mut Side,
    env: &Env<Limits>,
    request: Token,
    progress: RequestProgress,
    out: &mut Queue<Request>,
) {
    let id = Id::from_token(request);
    if let Some(pending) = side.pending.get_mut(id) {
        if !pending.live {
            return;
        }
        let object = pending.object;
        if let Some(object) = side.objects.get_mut(object) {
            object.card = match progress {
                RequestProgress::Unknown => Card::Unknown,
                RequestProgress::InFlight | RequestProgress::Retrying | RequestProgress::Parked => {
                    Card::Deciding { request, progress }
                }
            };
            side.changed();
        }
        if progress == RequestProgress::Unknown {
            side.pending.get_mut(id).expect("pending exists").live = false;
            side.pending.retire(id);
            if let Some(object) = side.objects.get_mut(object) {
                object.value = Value::Loading;
                let subject = object.subject;
                acquire(side, env, subject, out);
            }
        }
    }
}

fn part_fits(part: &Part, segments: u32, text: u32) -> bool {
    match part {
        Part::Resource { path, .. } => jig_client::contract::resource_path_fits(path, segments, text),
        Part::Connector { .. }
        | Part::Kind { .. }
        | Part::Panel { .. }
        | Part::Result { .. }
        | Part::Effect { .. }
        | Part::OwnerEffect { .. }
        | Part::Outbox { .. }
        | Part::Hold { .. }
        | Part::Project { .. }
        | Part::Deployment { .. } => true,
    }
}

fn connector_touch(side: &mut Side, id: Id<Object>, revision: Revision, party: u64, out: &mut Queue<Request>) {
    let Some(object) = side.object(id) else {
        return;
    };
    let document = match &object.value {
        Value::Known(Document::Connector(value)) => value,
        Value::Known(Document::Item(_) | Document::Part(_)) | Value::Loading | Value::Unknown | Value::Unsupported => {
            return;
        }
    };
    let (number, now) = match document.value.as_ref() {
        crate::connector_values::ClientDocument::Object { object, now } => (*object, *now),
        crate::connector_values::ClientDocument::Panel { .. }
        | crate::connector_values::ClientDocument::Result { .. }
        | crate::connector_values::ClientDocument::Effect { .. }
        | crate::connector_values::ClientDocument::Outbox { .. }
        | crate::connector_values::ClientDocument::Hold { .. }
        | crate::connector_values::ClientDocument::Project { .. }
        | crate::connector_values::ClientDocument::Deployment { .. } => return,
    };
    let project = match document.subject.scope {
        Scope::Project(project) => project,
        Scope::Deployment => return,
    };
    if now.revision != revision.value {
        side.objects.get_mut(id).expect("named object exists").card = Card::Refused {
            answer: Answer::Connector(crate::connector_values::ConnectorAnswer {
                value: crate::connector_values::ApplicationOutcome::Moved { object: number, now },
            }),
        };
        side.changed();
        return;
    }
    let value = object.draft.unwrap_or(now.value);
    let Ok(request) = side.pending.insert(Pending { live: true, object: id }) else {
        full(out);
        return;
    };
    side.objects.get_mut(id).expect("named object exists").card =
        Card::Deciding { request: request.token(), progress: RequestProgress::Parked };
    out.push(Request::Send {
        request: request.token(),
        scope: party,
        ask: Ask::ChangeObject { project, object: number, revision, value },
    });
    side.changed();
}
