//! One engine watch per subject, held through its closing terminal.
use crate::{
    Domain, Latest, Limits, Request, State, Told, Up, Watchable, Why,
    contract::{WatchEnd, WatchEvent},
    domain::tell,
};
use alloc::boxed::Box;
use skein_lib::{Duration, Env, Queue, Time, Token};
#[derive(Debug)]
pub(crate) struct Watch {
    pub subject: Watchable,
    pub name: Option<Token>,
    pub latest: Option<Latest>,
    pub state: State,
    pub closing: bool,
    pub answered: bool,
    pub own: bool,
    pub until: Option<Time>,
    pub span: Option<Duration>,
}
impl Domain {
    pub(crate) fn up(&mut self, env: &Env<Limits>, up: Up, out: &mut Queue<Request>) {
        let name = match &up {
            Up::SignedIn
            | Up::SignedOut
            | Up::NotKept { .. }
            | Up::Served { .. }
            | Up::FamilyServed { .. }
            | Up::Answer { .. }
            | Up::ReadDone { .. } => {
                unreachable!("watch events only")
            }
            Up::Watched { watch, .. } | Up::FamilyDelivered { watch, .. } | Up::WatchEnded { watch, .. } => *watch,
        };
        let mut found = None;
        for (id, w) in &self.watches {
            if w.name == Some(name) {
                found = Some(*id);
            }
        }
        let Some(id) = found else {
            return;
        };
        match up {
            Up::SignedIn
            | Up::SignedOut
            | Up::NotKept { .. }
            | Up::Served { .. }
            | Up::FamilyServed { .. }
            | Up::Answer { .. }
            | Up::ReadDone { .. } => {
                unreachable!("watch events only")
            }
            Up::Watched { event, .. } => match event {
                WatchEvent::Delivered(value) => {
                    self.opened(id, name, out);
                    self.deliver(env, id, Latest::Jig(value), out);
                }
                WatchEvent::Opened => self.opened(id, name, out),
                WatchEvent::Alive | WatchEvent::Behind => {}
            },
            Up::FamilyDelivered { record, .. } => {
                self.opened(id, name, out);
                self.deliver(env, id, Latest::Family(record), out);
            }
            Up::WatchEnded { end, .. } => self.ended(env, id, end, out),
        }
    }
    fn opened(&mut self, id: u64, name: Token, out: &mut Queue<Request>) {
        let w = self.watches.get_mut(&id).expect("answered watch");
        if !w.answered {
            w.answered = true;
            if w.closing {
                crate::domain::down(out, crate::Down::Close { watch: name });
            }
        }
    }
    fn select_handles(&mut self, id: u64) {
        self.scratch.clear();
        for (handle, held) in &self.handles {
            if held.watch == Some(id) {
                self.scratch.push(*handle).expect("bounded handles");
            }
        }
    }
    fn deliver(&mut self, env: &Env<Limits>, id: u64, latest: Latest, out: &mut Queue<Request>) {
        if let Latest::Family(record) = &latest
            && u64::try_from(record.bytes.len()).expect("family bytes fit") > env.limits.document_bytes
        {
            self.refuse_watch(id, Why::Limit, out);
            return;
        }
        let w = self.watches.get_mut(&id).expect("delivery watch");
        if w.closing {
            return;
        }
        let (latest, streamed) = match latest {
            Latest::Jig(crate::contract::Delivery::Run(value)) => {
                let Some(text) = crate::text::merge(w, *value, env.limits.turn_bytes) else {
                    return;
                };
                (Latest::Jig(crate::contract::Delivery::Run(Box::new(text))), true)
            }
            Latest::Jig(crate::contract::Delivery::RunState(mut state)) => {
                if let Some(value) = state.writing.take() {
                    let Some(text) = crate::text::merge(w, value, env.limits.turn_bytes) else {
                        return;
                    };
                    state.writing = Some(text);
                }
                (Latest::Jig(crate::contract::Delivery::RunState(state)), true)
            }
            latest @ (Latest::Jig(_) | Latest::Family(_)) => (latest, false),
        };
        w.latest = Some(latest);
        w.state = State::Live;
        w.span = None;
        if w.own {
            self.set_link(crate::Link::Live, out);
        }
        self.select_handles(id);
        for index in 0..self.scratch.len() {
            let handle = *self.scratch.get(index).expect("selected handle");
            let held = self.handles.get_mut(&handle).expect("selected handle exists");
            held.state = State::Live;
            held.previous = None;
            tell(out, Told::Changed { handle });
            if streamed {
                tell(out, Told::Text { handle });
            }
        }
        self.scratch.clear();
    }
    pub(crate) fn refuse_watch(&mut self, id: u64, why: Why, out: &mut Queue<Request>) {
        if self.watches.get(&id).expect("refused watch").closing {
            return;
        }
        self.select_handles(id);
        for index in 0..self.scratch.len() {
            let handle = *self.scratch.get(index).expect("selected handle");
            let held = self.handles.get_mut(&handle).expect("selected handle exists");
            held.state = State::Ended;
            held.watch = None;
            held.previous = self.watches.get(&id).expect("refused watch").latest.clone().or(held.previous.take());
            tell(out, Told::Ended { handle, why });
        }
        self.scratch.clear();
        let watch = self.watches.get_mut(&id).expect("refused watch");
        watch.closing = true;
        watch.until = None;
        if let Some(name) = watch.name {
            if watch.answered {
                crate::domain::down(out, crate::Down::Close { watch: name });
            }
        } else {
            self.watches.remove(&id);
        }
    }
    fn ended(&mut self, env: &Env<Limits>, id: u64, end: WatchEnd, out: &mut Queue<Request>) {
        let w = self.watches.get_mut(&id).expect("terminal watch");
        w.name = None;
        w.answered = false;
        if w.closing {
            self.watches.remove(&id);
            return;
        }
        let why = match end {
            WatchEnd::Closed | WatchEnd::SubjectEnded | WatchEnd::Unknown | WatchEnd::NotAllowed if w.own => None,
            WatchEnd::Dropped | WatchEnd::Stopping | WatchEnd::Busy => None,
            WatchEnd::Closed | WatchEnd::SubjectEnded => Some(Why::SubjectEnded),
            WatchEnd::Unknown => Some(Why::Unknown),
            WatchEnd::TooLinked { joins } => Some(Why::TooLinked { joins }),
            WatchEnd::NotAllowed => Some(Why::NotAllowed),
            WatchEnd::NotServed => Some(Why::NotServed),
            WatchEnd::SignedOut => Some(Why::SignedOut),
        };
        match why {
            None => {
                let span = match w.span {
                    Some(s) => core::cmp::min(s.saturating_mul(2), env.limits.backoff.most),
                    None => env.limits.backoff.first,
                };
                w.span = Some(span);
                w.state = State::Behind;
                let nanos = span.as_nanos();
                w.until =
                    Some(env.now.saturating_add(Duration::from_nanos(self.rng.between(nanos.div_euclid(2), nanos))));
            }
            Some(_) => {
                w.state = State::Ended;
            }
        }
        if w.own && why.is_none() {
            self.set_link(crate::Link::Offline, out);
        }
        self.select_handles(id);
        for index in 0..self.scratch.len() {
            let handle = *self.scratch.get(index).expect("selected handle");
            let held = self.handles.get_mut(&handle).expect("selected handle exists");
            match why {
                None => {
                    held.state = State::Behind;
                    tell(out, Told::Changed { handle });
                }
                Some(why) => {
                    held.state = State::Ended;
                    held.watch = None;
                    held.previous =
                        self.watches.get(&id).expect("ending watch").latest.clone().or(held.previous.take());
                    tell(out, Told::Ended { handle, why });
                }
            }
        }
        self.scratch.clear();
        if why.is_some() {
            self.watches.remove(&id);
        }
    }
}
