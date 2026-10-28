//! Keep: bounded working sets, scoped read/watch lifetimes and owner routing.
//! Replace: the connector vocabulary, its families, words and revision-naming acts.
//! State is a bounded subject registry, page/part demand, scoped reads and watches.
//! New subjects begin Loading; Unknown never supplies an absent owner fact.
//! Entry points: step handles one event; fire retries dropped watches; reclaim frees terminals.
//! | Event | State change | Requests |
//! | Went/Show | acquire scoped demand | Read, Open |
//! | Read/Delivered | validate whole document, retain newest revision | `Named` |
//! | Left/PartsLeft | release demand, retain close obligation | Close |
//! | Dropped | schedule bounded retry | later Open |
//! | Closed/Unknown | retire watch; unknown hides supplied facts | replacement Read/Open |
//! | Confirmed/Link | restrict lifetimes to confirmed party and live link | reacquire or hide |
pub mod documents;
pub mod native;
use alloc::boxed::Box;
pub use documents::{Document, Kind, ReadResult, Scope, Subject, WatchEnd, WatchEvent};
use jig_client::Link;
use jig_client::contract::Part;
use skein_lib::{Env, Id, List, Map, Queue, Slab, Time, Token};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Address {
    Incidents { project: u32, open: bool },
    Incident { project: u32, incident: u64 },
}
impl Address {
    #[must_use]
    pub const fn subject(self) -> Subject {
        match self {
            Self::Incidents { project, open } => {
                Subject { scope: Scope::Project(project), kind: Kind::Incidents { open } }
            }
            Self::Incident { project, incident } => {
                Subject { scope: Scope::Project(project), kind: Kind::Incident(incident) }
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    Go { address: Address },
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
    ReadNotAllowed,
    WatchTerminal { end: WatchEnd },
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
    Out(Out),
    Named { objects: List<jig_client::contract::ObjectKey>, sections: List<jig_client_view::page::Section> },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub parts: u32,
    pub objects: u32,
    pub reads: u32,
    pub watches: u32,
    pub rows: u32,
    pub segments: u32,
    pub text: u32,
    pub retry: skein_lib::Duration,
    pub document_bytes: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Loading,
    Unknown,
    Known(Box<Document>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Card {
    Open,
}
#[derive(Debug)]
#[expect(clippy::partial_pub_fields, reason = "the view reads values; the side alone owns read and watch obligations")]
pub struct Object {
    pub subject: Subject,
    pub value: Value,
    pub card: Card,
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
pub struct Side {
    parts: List<Held>,
    objects: Slab<Object>,
    names: Map<Subject, Id<Object>>,
    reads: Slab<Reading>,
    watches: Slab<Watching>,
    watch_ids: Map<Id<Watching>, ()>,
    address: Option<Address>,
    generation: u32,
    link: Link,
    party: Option<u64>,
    me: bool,
    kept: Option<Record>,
    shown: u64,
}
impl Side {
    #[must_use]
    pub fn new(limits: &Limits, _seed: u64) -> Self {
        Self {
            parts: List::with_capacity(limits.parts),
            objects: Slab::with_capacity(limits.objects.checked_mul(2).expect("object retirement room fits")),
            names: Map::with_capacity(limits.objects),
            reads: Slab::with_capacity(limits.reads),
            watches: Slab::with_capacity(limits.watches),
            watch_ids: Map::with_capacity(limits.watches),
            address: None,
            generation: 0,
            link: Link::Starting,
            party: None,
            me: false,
            kept: None,
            shown: 0,
        }
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
        self.find(address.subject())
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
            let subject = documents::subject(&part, scope);
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
            hold(side, env, address.subject(), out);
            save(side, out);
        }
        Event::Left => {
            side.address = None;
            out.push(Request::Named { objects: List::with_capacity(0), sections: List::with_capacity(0) });
            prune(side, out);
            side.changed();
        }
        Event::Act { act } => match act {
            Act::Go { address } => out.push(Request::Show { address }),
        },
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
fn hold(side: &mut Side, env: &Env<Limits>, subject: Subject, out: &mut Queue<Request>) {
    if side.find(subject).is_none() {
        if side.names.len() == side.names.capacity() {
            full(out);
            return;
        }
        let object = Object { subject, value: Value::Loading, card: Card::Open, read: None, watch: None };
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
    match object.value {
        Value::Unknown => return,
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
        Value::Known(_) | Value::Unknown => false,
    }
}
fn received(side: &mut Side, env: &Env<Limits>, subject: Subject, document: Document, out: &mut Queue<Request>) {
    if document.subject() != subject || !document.fits(&env.limits) {
        out.push(Request::Out(Out::Fact { fact: Fact::InvalidDocument }));
        return;
    }
    let Some(id) = side.find(subject) else {
        return;
    };
    let object = side.objects.get_mut(id).expect("named object exists");
    match &object.value {
        Value::Known(current) => {
            if let Some(current) = current.revision()
                && let Some(incoming) = document.revision()
                && current.value > incoming.value
            {
                return;
            }
        }
        Value::Loading | Value::Unknown => {}
    }
    object.value = Value::Known(Box::new(document));
    if side.page() == Some(id) {
        let object = side.objects.get(id).expect("page object exists");
        match &object.value {
            Value::Known(document) => {
                out.push(Request::Named { objects: document.objects(env.limits.rows), sections: document.sections() });
            }
            Value::Loading | Value::Unknown => {}
        }
    }
    side.changed();
}
fn unknown(side: &mut Side, subject: Subject) {
    if let Some(id) = side.find(subject) {
        side.objects.get_mut(id).expect("named object exists").value = Value::Unknown;
        side.changed();
    }
}
fn named(side: &Side, subject: Subject) -> bool {
    if let Some(address) = side.address
        && subject == (address.subject())
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
        if !named(side, *subject) {
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
    let subject = side.watches.get(id).expect("retired watch exists").subject;
    if let Some(object) = side.find(subject) {
        let object = side.objects.get_mut(object).expect("named object exists");
        if object.watch == Some(id) {
            object.watch = None;
        }
    }
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
    let closing = watch.state == Following::Closing;
    if closing {
        retire_watch(side, id);
    } else {
        match end {
            WatchEnd::Dropped => {
                side.watches.get_mut(id).expect("watch exists").state =
                    Following::Dropped { again: env.now.saturating_add(env.limits.retry) }
            }
            WatchEnd::NotAllowed | WatchEnd::SubjectEnded | WatchEnd::Stopping => {
                retire_watch(side, id);
                unknown(side, subject);
                out.push(Request::Out(Out::Fact { fact: Fact::WatchTerminal { end } }));
            }
            WatchEnd::Busy => {
                side.watches.get_mut(id).expect("watch exists").state =
                    Following::Dropped { again: env.now.saturating_add(env.limits.retry) };
                out.push(Request::Out(Out::Fact { fact: Fact::WatchTerminal { end } }));
            }
            WatchEnd::Unknown => {
                retire_watch(side, id);
                unknown(side, subject);
            }
            WatchEnd::SignedOut => {
                retire_watch(side, id);
                out.push(Request::Out(Out::Fact { fact: Fact::SignedOut }));
            }
            WatchEnd::Closed => retire_watch(side, id),
        }
    }
    if closing || end != WatchEnd::Dropped && end != WatchEnd::Busy {
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
}
#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    if limits.retry == skein_lib::Duration::ZERO || limits.document_bytes == 0 {
        return None;
    }
    let path = List::<Box<[u8]>>::worst_case(limits.segments)?
        .checked_add(u64::from(limits.segments).checked_mul(u64::from(limits.text))?)?;
    let mut total = List::<Held>::worst_case(limits.parts)?
        .checked_mul(2)?
        .checked_add(u64::from(limits.parts).checked_mul(path)?.checked_mul(2)?)?;
    total = total.checked_add(Slab::<Object>::worst_case(limits.objects.checked_mul(2)?)?)?.checked_add(Map::<
        Subject,
        Id<Object>,
    >::worst_case(
        limits.objects,
    )?)?;
    let payload = limits.document_bytes;
    total = total.checked_add(u64::from(limits.objects.checked_mul(2)?).checked_mul(payload)?)?;
    total = total
        .checked_add(Slab::<Reading>::worst_case(limits.reads)?)?
        .checked_add(Slab::<Watching>::worst_case(limits.watches)?)?
        .checked_add(Map::<Id<Watching>, ()>::worst_case(limits.watches)?)?;
    total = total
        .checked_add(List::<Subject>::worst_case(limits.objects)?)?
        .checked_add(List::<Id<Watching>>::worst_case(limits.watches)?)?;
    total.checked_add(List::<jig_client::contract::ObjectKey>::worst_case(limits.rows.checked_add(1)?)?)
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
        ReadResult::Value(document) => received(side, env, subject, *document, out),
        ReadResult::NotAllowed => {
            unknown(side, subject);
            out.push(Request::Out(Out::Fact { fact: Fact::ReadNotAllowed }));
        }
        ReadResult::Unknown => unknown(side, subject),
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
    if watching.state == Following::Closing {
        return;
    }
    let subject = watching.subject;
    watching.state = Following::Live;
    match event {
        WatchEvent::Opened | WatchEvent::Alive => {}
        WatchEvent::Delivered(document) => received(side, env, subject, *document, out),
    }
}

fn part_fits(part: &Part, segments: u32, text: u32) -> bool {
    match part {
        Part::Resource { path, .. } => jig_client::contract::resource_path_fits(path, segments, text),
        Part::Connector { .. }
        | Part::Kind { .. }
        | Part::Panel { .. }
        | Part::Result { .. }
        | Part::OwnerEffect { .. }
        | Part::Effect { .. }
        | Part::Outbox { .. }
        | Part::Hold { .. }
        | Part::Project { .. }
        | Part::Deployment { .. } => true,
    }
}

/// Read this side's party from its own client handle.
pub(crate) fn me_told(
    side: &mut Side,
    env: &Env<Limits>,
    client: &jig_client::Client,
    told: &jig_client::Told,
    out: &mut Queue<Request>,
) -> bool {
    let Some(update) = crate::me::told(jig_ops_domain_connectors::Connector::Observability, &mut side.me, client, told)
    else {
        return false;
    };
    if let Some(asked) = update.asked {
        out.push(Request::Client { asked });
    }
    match update.change {
        crate::me::Change::Link(link) => {
            side.link = link;
            if link == Link::SignedOut || link == Link::OutOfDate {
                party(side, env, None, out);
            }
            sync(side, env, out);
            side.changed();
        }
        crate::me::Change::Party(value) => party(side, env, value, out),
        crate::me::Change::Unchanged => {}
    }
    true
}
fn party(side: &mut Side, env: &Env<Limits>, party: Option<u64>, out: &mut Queue<Request>) {
    if side.party != party {
        hide(side, out);
    }
    side.party = party;
    sync(side, env, out);
    save(side, out);
    side.changed();
}
