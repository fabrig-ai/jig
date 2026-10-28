//! Stream state, confirmed through the own watch (client/domain/client.md, 8.2).
use crate::{Domain, Link, Request, Told, Watchable, contract, domain::tell};
use skein_lib::Queue;
impl Domain {
    /// The last refused stream opening, retained across retries until a hello or sign-in change.
    #[must_use]
    pub fn stream_full(&self) -> Option<crate::StreamFull> {
        self.stream_full
    }
    pub fn stream_refused(&mut self, full: Option<crate::StreamFull>, out: &mut Queue<Request>) {
        if self.stream_full != full {
            self.stream_full = full;
            tell(out, Told::Link { link: self.link });
        }
    }

    #[must_use]
    pub fn link(&self) -> Link {
        self.link
    }
    #[must_use]
    pub fn party(&self) -> Option<u64> {
        for (_, watch) in &self.watches {
            if watch.own
                && let Some(crate::Latest::Jig(contract::Delivery::Me(frame))) = &watch.latest
            {
                return Some(frame.party.number);
            }
        }
        None
    }
    pub(crate) fn set_link(&mut self, link: Link, out: &mut Queue<Request>) {
        if self.link != Link::OutOfDate && self.link != link {
            self.link = link;
            tell(out, Told::Link { link });
        }
    }
    pub(crate) fn signed_out(&mut self, out: &mut Queue<Request>) {
        self.stream_refused(None, out);
        self.scratch.clear();
        for (handle, held) in &self.handles {
            if held.state != crate::State::Ended {
                self.scratch.push(*handle).expect("bounded handles");
            }
        }
        for index in 0..self.scratch.len() {
            let handle = *self.scratch.get(index).expect("selected handle");
            let held = self.handles.get_mut(&handle).expect("selected handle exists");
            held.state = crate::State::Ended;
            if let Some(watch) = held.watch.take() {
                held.previous.clone_from(&self.watches.get(&watch).expect("handle's watch").latest);
            }
            tell(out, Told::Ended { handle, why: crate::Why::SignedOut });
        }
        self.scratch.clear();
        while let Some((key, _)) = (&self.watches).into_iter().next() {
            let key = *key;
            self.watches.remove(&key);
        }
        self.end_reads(out);
        self.table.link(false);
        self.table.confirmed(None);
        self.set_link(Link::SignedOut, out);
    }
    pub(crate) fn signed_in(&mut self, out: &mut Queue<Request>) {
        self.end_reads(out);
        if self.link != Link::SignedOut {
            self.signed_out(out);
        }
        self.set_link(Link::Starting, out);
        self.due.clear();
        let mut own = None;
        for (id, watch) in &self.watches {
            if watch.subject == Watchable::Jig(contract::Subject::Me) {
                own = Some(*id);
            }
            if watch.name.is_none() {
                self.due.push(*id).expect("one opening per watch");
            }
        }
        if let Some(id) = own {
            self.watches.get_mut(&id).expect("held frame watch").own = true;
        } else {
            self.open(Watchable::Jig(contract::Subject::Me), true, out);
        }
        for index in 0..self.due.len() {
            let id = *self.due.get(index).expect("held watch index");
            let name = self.token();
            let w = self.watches.get_mut(&id).expect("held watch");
            w.name = Some(name);
            w.until = None;
            crate::domain::down(out, w.opening(name));
        }
        self.due.clear();
    }
}
