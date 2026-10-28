//! Client-owned handles over shared engine watches (client/domain/client.md, 8.1).
use crate::{Document, Down, Event, Handle, Latest, Limits, Request, State, Told, Watchable, Why, watches::Watch};
use skein_lib::{Env, List, Map, Queue, RequestTable, Rng, Time, Token};
#[derive(Debug)]
pub(crate) struct Owner {
    pub spoken: crate::Spoken,
    pub versions: Option<crate::contract::VersionRange>,
    pub served: bool,
}
#[derive(Debug)]
pub(crate) struct HandleState {
    pub watch: Option<u64>,
    pub state: State,
    pub previous: Option<Latest>,
}
#[derive(Debug)]
pub struct Domain {
    pub(crate) handles: Map<Handle, HandleState>,
    pub(crate) watches: Map<u64, Watch>,
    pub(crate) reads: Map<Handle, crate::reads::PendingRead>,
    pub(crate) next: u64,
    pub(crate) rng: Rng,
    pub(crate) scratch: List<Handle>,
    pub(crate) due: List<u64>,
    started: bool,
    pub(crate) link: crate::Link,
    pub(crate) stream_full: Option<crate::StreamFull>,
    pub(crate) table: RequestTable<crate::Key, crate::Held>,
    pub(crate) requests: Map<crate::Key, crate::requests::PendingRequest>,
    pub(crate) owners: List<Owner>,
    pub(crate) not_served: List<u16>,
    pub(crate) jig_versions: Option<crate::contract::VersionRange>,
    pub(crate) blocked: List<Token>,
    pub(crate) carried: Map<crate::Key, crate::carried::Carrying>,
    // Attempts of requests carrying a value, apart from the table's, which count from 1.
    pub(crate) next_carried: u64,
}
impl Domain {
    #[must_use]
    pub fn new(limits: &Limits, seed: u64) -> Self {
        crate::worst_case(limits).expect("valid client limits");
        let bound = u32::try_from(limits.owners.len()).expect("bounded owners");
        let mut owners = List::with_capacity(bound);
        for spoken in limits.owners {
            owners.push(Owner { spoken: *spoken, versions: None, served: true }).expect("declared owner");
        }
        Self {
            owners,
            not_served: List::with_capacity(bound),
            jig_versions: None,
            blocked: List::with_capacity(limits.requests.requests),
            handles: Map::with_capacity(limits.handles),
            watches: Map::with_capacity(limits.watches),
            reads: Map::with_capacity(limits.reads),
            next: 1,
            rng: Rng::new(seed),
            scratch: List::with_capacity(limits.handles),
            due: List::with_capacity(limits.watches),
            started: false,
            link: crate::Link::SignedOut,
            stream_full: None,
            table: RequestTable::new(&limits.requests, seed),
            requests: Map::with_capacity(limits.requests.requests),
            carried: Map::with_capacity(limits.requests.requests),
            next_carried: 1 << 63,
        }
    }
    #[must_use]
    pub fn document(&self, handle: Handle) -> Option<Document<'_>> {
        let held = self.handles.get(&handle)?;
        let latest = match held.watch {
            Some(id) => match self.watches.get(&id) {
                Some(w) => w.latest.as_ref().or(held.previous.as_ref()),
                None => held.previous.as_ref(),
            },
            None => held.previous.as_ref(),
        };
        Some(Document { state: held.state, latest })
    }
    pub(crate) fn token(&mut self) -> Token {
        let name = self.next;
        self.next = self.next.checked_add(1).expect("client names never wrap");
        Token::new(name)
    }
    pub(crate) fn declared(&self, record: &crate::Family) -> bool {
        for owner in &self.owners {
            if owner.spoken.family == record.family && owner.spoken.version == record.version {
                return true;
            }
        }
        false
    }
    pub(crate) fn family(&self, record: &crate::Family) -> &Owner {
        let mut declared = None;
        for owner in &self.owners {
            if owner.spoken.family == record.family {
                assert!(owner.spoken.version == record.version, "family version must be declared");
                declared = Some(owner);
            }
        }
        assert!(declared.is_some(), "family owner must be declared");
        declared.expect("declared owner")
    }
    #[must_use]
    pub fn not_served(&self) -> &[u16] {
        self.not_served.as_slice()
    }
    fn family_served(
        &mut self,
        family: u16,
        versions: Option<crate::contract::VersionRange>,
        out: &mut Queue<Request>,
    ) {
        let mut index = None;
        for i in 0..self.owners.len() {
            if self.owners.get(i).expect("owner index").spoken.family == family {
                index = Some(i);
            }
        }
        let Some(index) = index else { return };
        let owner = self.owners.get_mut(index).expect("declared owner");
        owner.versions = versions;
        owner.served = match versions {
            Some(v) => v.contains(owner.spoken.version),
            None => false,
        };
        let served = owner.served;
        self.not_served.clear();
        for owner in &self.owners {
            if !owner.served {
                self.not_served.push(owner.spoken.family).expect("declared owners");
            }
        }
        if !served {
            for _ in 0..self.watches.len() {
                let mut found = None;
                for (id, w) in &self.watches {
                    let same = match &w.subject {
                        Watchable::Family { record: f, .. } => f.family == family,
                        Watchable::Jig(_) => false,
                    };
                    if !w.closing && same {
                        found = Some(*id);
                    }
                }
                let Some(id) = found else { break };
                self.refuse_watch(id, Why::NotServed, out);
            }
        }
    }
    pub(crate) fn watch(&mut self, env: &Env<Limits>, handle: Handle, subject: Watchable, out: &mut Queue<Request>) {
        if let Watchable::Family { record, .. } = &subject {
            self.family(record);
        }
        let previous = match self.document(handle) {
            Some(d) => d.latest.cloned(),
            None => None,
        };
        if !self.handles.contains_key(&handle) && self.handles.len() == env.limits.handles {
            tell(out, Told::Ended { handle, why: Why::Limit });
            return;
        }
        let mut shared = None;
        let mut reserved = 1;
        for (id, w) in &self.watches {
            if w.own {
                reserved = 0;
            }
            if !w.closing && w.subject == subject {
                shared = Some(*id);
            }
        }
        if let Some(held) = self.handles.get(&handle)
            && held.watch == shared
            && shared.is_some()
        {
            return;
        }
        self.unwatch(handle, out);
        if let Watchable::Family { record, .. } = &subject
            && !self.family(record).served
        {
            self.handles
                .insert(handle, HandleState { watch: None, state: State::Ended, previous })
                .expect("handle admitted");
            tell(out, Told::Ended { handle, why: Why::NotServed });
            return;
        }
        let id = match shared {
            Some(id) => id,
            None => {
                if self.watches.len() >= env.limits.watches.checked_sub(reserved).expect("room for own watch") {
                    tell(out, Told::Ended { handle, why: Why::Limit });
                    return;
                }
                self.open(subject, false, out)
            }
        };
        let w = self.watches.get(&id).expect("watch exists");
        let state = if w.state == State::Live {
            State::Live
        } else if previous.is_some() || w.state == State::Behind {
            State::Behind
        } else {
            State::Opening
        };
        self.handles.insert(handle, HandleState { watch: Some(id), state, previous }).expect("handle admitted");
        if state == State::Behind || state == State::Live {
            tell(out, Told::Changed { handle });
        }
    }
    pub(crate) fn open(&mut self, subject: Watchable, own: bool, out: &mut Queue<Request>) -> u64 {
        let name = self.token();
        let id = name.raw();
        let name = if self.link == crate::Link::SignedOut {
            None
        } else {
            down(out, Down::Open { watch: name, subject: subject.clone() });
            Some(name)
        };
        self.watches
            .insert(
                id,
                Watch {
                    subject,
                    name,
                    latest: None,
                    state: State::Opening,
                    closing: false,
                    answered: false,
                    own,
                    until: None,
                    span: None,
                },
            )
            .expect("watch admitted");
        id
    }
    pub(crate) fn unwatch(&mut self, handle: Handle, out: &mut Queue<Request>) {
        self.reads.remove(&handle);
        let Some(held) = self.handles.remove(&handle) else {
            return;
        };
        let Some(id) = held.watch else {
            return;
        };
        for (_, other) in &self.handles {
            if other.watch == Some(id) {
                return;
            }
        }
        let w = self.watches.get_mut(&id).expect("held watch exists");
        if w.own {
            return;
        }
        w.closing = true;
        w.until = None;
        match w.name {
            Some(watch) => {
                if w.answered {
                    down(out, Down::Close { watch });
                }
            }
            None => {
                self.watches.remove(&id);
            }
        }
    }
}
pub fn step(domain: &mut Domain, env: &Env<Limits>, event: Event, out: &mut Queue<Request>) {
    match event {
        Event::Start { kept } => {
            assert!(!domain.started, "start once");
            domain.started = true;
            domain.restore_record(env, kept, out);
        }
        Event::Send { key, name, request } => domain.send(env, key, name, request, out),
        Event::Carry { key, name, request, value } => domain.carry(env, key, name, request, value, out),
        Event::Watch { handle, subject } => domain.watch(env, handle, subject, out),
        Event::Unwatch { handle } => domain.unwatch(handle, out),
        Event::Read { handle, read } => domain.read(env, handle, read, out),
        Event::Up { up } => match up {
            crate::Up::SignedIn => domain.signed_in(out),
            crate::Up::SignedOut => domain.signed_out(out),
            crate::Up::NotKept { key } => domain.not_kept(key, out),
            crate::Up::FamilyServed { family, versions } => domain.family_served(family, versions, out),
            crate::Up::Served { served, versions } => {
                domain.jig_versions = versions;
                if !served {
                    domain.set_link(crate::Link::OutOfDate, out);
                }
            }
            crate::Up::ReadDone { read, result } => domain.read_done(read, result, out),
            crate::Up::Answer { attempt, envelope, answer, why } => {
                domain.answer(env, attempt, envelope, answer, why, out);
            }
            crate::Up::Watched { .. } | crate::Up::FamilyDelivered { .. } | crate::Up::WatchEnded { .. } => {
                domain.up(env, up, out);
            }
        },
    }
    domain.flush(env, out);
}
pub fn fire(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    domain.due.clear();
    for (id, w) in &domain.watches {
        if let Some(t) = w.until
            && t <= env.now
        {
            domain.due.push(*id).expect("one deadline per watch");
        }
    }
    for index in 0..domain.due.len() {
        let id = *domain.due.get(index).expect("due index");
        let name = domain.token();
        let w = domain.watches.get_mut(&id).expect("due watch");
        w.name = Some(name);
        w.until = None;
        down(out, w.opening(name));
    }
    domain.due.clear();
    domain.flush(env, out);
}
#[must_use]
pub fn next_deadline(domain: &Domain) -> Option<Time> {
    let mut next = domain.table.next_deadline();
    for (_, w) in &domain.watches {
        if let Some(t) = w.until {
            next = Some(match next {
                Some(n) => core::cmp::min(n, t),
                None => t,
            });
        }
    }
    next
}
pub fn reclaim(domain: &mut Domain) {
    domain.table.reclaim();
}
pub(crate) fn down(out: &mut Queue<Request>, down: Down) {
    out.push(Request::Down { down });
}
pub(crate) fn tell(out: &mut Queue<Request>, told: Told) {
    out.push(Request::Told { told });
}
