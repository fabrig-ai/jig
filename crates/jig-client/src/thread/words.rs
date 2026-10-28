//! Words stay beside activity until a committed turn says it read their message.
//! Pending words name the root-table intent; a durable answer supplies their
//! inbox number. Activity restores messages after reload without inventing a
//! request token. Read words may release room because activity keeps them whole.
//! See client/domain/client.md, 10, and domain/engine.md, 7.2.
//!
//! | State | Event | Next |
//! |---|---|---|
//! | absent | admitted write | being sent |
//! | being sent | durable words answer | sent, or read if the turn arrived first |
//! | being sent | refused or unknown | absent; the draft owner keeps the words |
//! | sent | committed read-through | read |
//! | absent | activity words | sent or read |
use crate::Key;
use crate::contract::{Activity, ActivityPage, MeParty, Party, Text};
use skein_lib::List;
/// Where a person's words are relative to the engine's committed turns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Words {
    /// The keyed request is still pending.
    BeingSent,
    /// The engine has kept this inbox message.
    Sent { message: u64 },
    /// A committed turn read through this message.
    Read { message: u64 },
}
/// Whole words, with a request only when this client sent them.
#[derive(Debug)]
pub struct Sent {
    pub request: Option<Key>,
    pub place: u32,
    pub by: Option<Party>,
    /// Name loss supplied by the local Me; historical Party has no such fact.
    pub by_name_cut: Option<bool>,
    pub words: Text,
    pub state: Words,
}
/// A bounded set of pending and sent words, and the latest committed read position.
#[derive(Debug)]
pub struct KeptWords {
    pub rows: List<Sent>,
    pub read_through: u64,
    /// Authentication scope of local request snapshots; activity has its own authors.
    pub scope: Option<u64>,
}
impl KeptWords {
    /// Allocates the configured admission room without evicting unread local words.
    #[must_use]
    pub fn new(capacity: u32) -> Self {
        Self { rows: List::with_capacity(capacity), read_through: 0, scope: None }
    }
    /// A different authenticated party never inherits local request snapshots.
    pub fn bind(&mut self, scope: Option<u64>) {
        if self.scope == scope {
            return;
        }
        let mut kept = List::with_capacity(self.rows.capacity());
        let rows = core::mem::replace(&mut self.rows, List::with_capacity(0));
        for row in rows.into_boxed() {
            if row.request.is_none() {
                kept.push(row).expect("removal keeps word room");
            }
        }
        self.rows = kept;
        self.scope = scope;
    }
    /// Whether the same pending request is already shown.
    #[must_use]
    pub fn pending(&self, request: Key) -> bool {
        for sent in &self.rows {
            if sent.request == Some(request) {
                return true;
            }
        }
        false
    }
    /// Frees only words a committed turn has read before admitting another write.
    pub fn admits(&mut self) -> bool {
        self.prune_read();
        self.rows.len() < self.rows.capacity()
    }
    /// Keeps the submitted snapshot once, including when a repeated press joins it.
    pub fn begin(&mut self, request: Key, by: Option<Party>, words: Text) -> bool {
        if self.pending(request) {
            return true;
        }
        if !self.admits() {
            return false;
        }
        self.rows
            .push(Sent { request: Some(request), place: 0, by, by_name_cut: None, words, state: Words::BeingSent })
            .expect("word room checked");
        true
    }
    /// Keeps the supplied local author label and its independent document loss.
    pub fn begin_frame(&mut self, request: Key, by: Option<MeParty>, words: Text, place: u32) -> bool {
        if self.pending(request) {
            return true;
        }
        let (by, cut) = match by {
            Some(party) => {
                (Some(Party { number: party.number, kind: party.kind, name: party.name.bytes }), Some(party.name.cut))
            }
            None => (None, None),
        };
        if !self.begin(request, by, words) {
            return false;
        }
        for at in 0..self.rows.len() {
            let row = self.rows.get_mut(at).expect("word row exists");
            if row.request == Some(request) {
                row.by_name_cut = cut;
                row.place = place;
                break;
            }
        }
        true
    }
    /// Applies a durable message number once; an activity copy is joined by number.
    pub fn sent(&mut self, request: Key, message: u64) -> bool {
        let mut present = None;
        for at in 0..self.rows.len() {
            if self.rows.get(at).expect("word row exists").request == Some(request) {
                present = Some(at);
                break;
            }
        }
        let Some(at) = present else {
            return false;
        };
        let state = self.state(message);
        self.rows.get_mut(at).expect("word row exists").state = state;
        let mut kept = List::with_capacity(self.rows.capacity());
        let rows = core::mem::replace(&mut self.rows, List::with_capacity(0));
        for row in rows.into_boxed() {
            let duplicate = row.request != Some(request) && number(row.state) == Some(message);
            if !duplicate {
                kept.push(row).expect("joining words does not grow their list");
            }
        }
        self.rows = kept;
        true
    }
    /// A refusal or unknown outcome restores the composer owner's display alone.
    pub fn refused(&mut self, request: Key) {
        let mut kept = List::with_capacity(self.rows.capacity());
        let rows = core::mem::replace(&mut self.rows, List::with_capacity(0));
        for row in rows.into_boxed() {
            if row.request != Some(request) {
                kept.push(row).expect("removal keeps word room");
            }
        }
        self.rows = kept;
    }
    /// The state of an activity message follows the durable read-through position.
    #[must_use]
    pub const fn state(&self, message: u64) -> Words {
        if message <= self.read_through { Words::Read { message } } else { Words::Sent { message } }
    }
    /// A committed turn marks every earlier inbox message read together.
    pub fn read(&mut self, through: u64) {
        self.read_through = self.read_through.max(through);
        for at in 0..self.rows.len() {
            let row = self.rows.get_mut(at).expect("word row exists");
            match row.state {
                Words::BeingSent => {}
                Words::Sent { message } | Words::Read { message } => {
                    if message <= self.read_through {
                        row.state = Words::Read { message };
                    }
                }
            }
        }
    }
    /// Restores durable activity words, even if their inbox copy was already taken.
    pub fn restore(&mut self, by: Party, message: u64, words: Text) -> bool {
        for row in &self.rows {
            if number(row.state) == Some(message) {
                return true;
            }
        }
        if !self.admits() {
            return false;
        }
        let state = self.state(message);
        self.rows
            .push(Sent { request: None, place: 0, by: Some(by), by_name_cut: None, words, state })
            .expect("word room checked");
        true
    }
    /// Restores a whole activity page; its read positions precede its words even
    /// though the page arrives newest first. Words beyond this cache stay whole
    /// in the activity window and obtain their state from `state`.
    pub fn observe(&mut self, page: &ActivityPage) {
        for row in &page.rows {
            if let Some(through) = row.event.read_position() {
                self.read(through);
            }
        }
        for row in &page.rows {
            match &row.event {
                Activity::Words { by, message, words } => match self.restore(by.clone(), *message, words.clone()) {
                    true | false => {}
                },
                Activity::Owner { .. }
                | Activity::Turn { .. }
                | Activity::RunBegan { .. }
                | Activity::RunEnded { .. }
                | Activity::Result { .. }
                | Activity::News { .. }
                | Activity::Decision { .. } => {}
            }
        }
    }
    fn prune_read(&mut self) {
        let mut kept = List::with_capacity(self.rows.capacity());
        let rows = core::mem::replace(&mut self.rows, List::with_capacity(0));
        for row in rows.into_boxed() {
            match row.state {
                Words::Read { .. } => {}
                Words::BeingSent | Words::Sent { .. } => {
                    kept.push(row).expect("removal keeps word room");
                }
            }
        }
        self.rows = kept;
    }
}
fn number(state: Words) -> Option<u64> {
    match state {
        Words::BeingSent => None,
        Words::Sent { message } | Words::Read { message } => Some(message),
    }
}
