//! Bounded per-task drafts, with persisted touch order and coalesced saves.
//! A confirmed party alone restores fields. Durable words clear only the exact
//! submitted draft; later typing survives its answer. See view.md, 18.5, and
//! client.md, 11.
//!
//! | Event | Draft | Effect |
//! |---|---|---|
//! | person edits | touched, kept | coalesced save |
//! | capacity full | oldest untouched erased | erase old record |
//! | page changes | retained | restore composer with a domain write |
//! | words done, same touch | removed | clear composer; erase record |
//! | words refused/unknown, or later edit | retained | none |
use crate::{
    Request, Ui,
    fields::Purpose,
    store::{Record, STORE_VERSION, StoreKey},
};
use alloc::boxed::Box;
use jig_client::contract::{Answer, Ask, Limit, Text};
use skein_lib::{List, Queue};
/// One draft's whole words and persistent ordering, owned by one party.
#[derive(Debug)]
pub struct Draft {
    pub task: u64,
    pub words: Text,
    pub touched: u64,
}
/// Active-party drafts; the records remain the restart boundary.
#[derive(Debug)]
pub struct Drafts {
    scope: Option<u64>,
    active: Option<u64>,
    rows: List<Draft>,
    clock: u64,
}
impl Drafts {
    /// Allocates the configured retained drafts, never a cache without a bound.
    #[must_use]
    pub fn new(capacity: u32) -> Self {
        Self { scope: None, active: None, rows: List::with_capacity(capacity), clock: 0 }
    }
    /// A draft in the current party's working set.
    #[must_use]
    pub fn get(&self, task: u64) -> Option<&Draft> {
        let mut found = None;
        for draft in &self.rows {
            if draft.task == task {
                found = Some(draft);
            }
        }
        found
    }
    /// The persisted edit included by a write request.
    #[must_use]
    pub fn touch(&self, task: u64) -> Option<u64> {
        Some(self.get(task)?.touched)
    }
    /// The number of drafts retained for the current party.
    #[must_use]
    pub fn len(&self) -> u32 {
        self.rows.len()
    }
    /// Whether the current party has no drafts.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    fn oldest(&self) -> Option<u64> {
        let mut oldest: Option<&Draft> = None;
        for draft in &self.rows {
            match oldest {
                Some(before) => {
                    if draft.touched < before.touched {
                        oldest = Some(draft);
                    }
                }
                None => oldest = Some(draft),
            }
        }
        Some(oldest?.task)
    }
    fn remove(&mut self, task: u64) {
        let capacity = self.rows.capacity();
        let old = core::mem::replace(&mut self.rows, List::with_capacity(capacity));
        for draft in old.into_boxed() {
            if draft.task != task {
                self.rows.push(draft).expect("filter preserves bounded draft capacity");
            }
        }
    }
}
fn erase(domain: &mut Ui, task: u64, out: &mut Queue<Request>) {
    domain.drafts.remove(task);
    if domain.drafts.active == Some(task) {
        let empty = Text::new(Box::from([]), 0).expect("empty text fits");
        let cleared = domain.write_field(Purpose::Composer { task }, empty);
        assert!(cleared.is_ok(), "active composer has a field");
    } else {
        domain.fields.remove(Purpose::Composer { task });
    }
    crate::store::erase(domain, StoreKey::Draft { task }, out);
}
/// Edits once, touches once, and keeps the record immediately for the save deadline.
pub(crate) fn edit(domain: &mut Ui, task: u64, words: Text, out: &mut Queue<Request>) -> Result<(), Limit> {
    let Some(party) = domain.confirmed else {
        return Ok(());
    };
    sync(domain, out);
    let touched = domain.drafts.clock.checked_add(1).ok_or(Limit::Records)?;
    if domain.drafts.rows.capacity() == 0 {
        return Err(Limit::Records);
    }
    if !words.bytes().is_empty() && domain.kept.len() == domain.kept.capacity() {
        let mut fits = false;
        let evicted = if domain.drafts.get(task).is_none() && domain.drafts.rows.len() == domain.drafts.rows.capacity()
        {
            domain.drafts.oldest()
        } else {
            None
        };
        for record in &domain.kept {
            let removes = match evicted {
                Some(task) => record.party() == party && record.key() == (StoreKey::Draft { task }),
                None => false,
            };
            if record.key() == (StoreKey::Draft { task }) || removes {
                fits = true;
            }
        }
        if !fits {
            return Err(Limit::Records);
        }
    }
    domain.fields.edit(Purpose::Composer { task }, words.clone())?;
    if words.bytes().is_empty() {
        erase(domain, task, out);
        domain.changed();
        return Ok(());
    }
    if domain.drafts.get(task).is_none() && domain.drafts.rows.len() == domain.drafts.rows.capacity() {
        let oldest = domain.drafts.oldest().expect("full nonempty drafts have an oldest");
        erase(domain, oldest, out);
    }
    domain.drafts.remove(task);
    domain.drafts.rows.push(Draft { task, words: words.clone(), touched }).expect("draft capacity checked");
    domain.drafts.clock = touched;
    if !crate::store::coalesce(domain, Record::Draft { version: STORE_VERSION, party, task, words, touched }, out) {
        domain.drafts.remove(task);
        return Err(Limit::Records);
    }
    domain.changed();
    Ok(())
}
fn restore(domain: &mut Ui, out: &mut Queue<Request>) {
    domain.fields.clear_composers();
    domain.drafts.rows.clear();
    domain.drafts.scope = domain.confirmed;
    domain.drafts.active = None;
    domain.drafts.clock = 0;
    let Some(party) = domain.confirmed else {
        return;
    };
    // Select most recently touched records regardless of the shell's record order.
    for record in &domain.kept {
        match record {
            Record::Draft { party: owner, task, words, touched, .. } => {
                if *owner != party || words.bytes().is_empty() || domain.drafts.rows.capacity() == 0 {
                    continue;
                }
                domain.drafts.clock = domain.drafts.clock.max(*touched);
                if domain.drafts.rows.len() == domain.drafts.rows.capacity() {
                    let oldest = domain.drafts.oldest().expect("full nonempty drafts have an oldest");
                    if domain.drafts.get(oldest).expect("oldest exists").touched >= *touched {
                        continue;
                    }
                    domain.drafts.remove(oldest);
                }
                domain
                    .drafts
                    .rows
                    .push(Draft { task: *task, words: words.clone(), touched: *touched })
                    .expect("draft capacity checked");
            }
            Record::Place { .. } => {}
        }
    }
    // Erase bounded-out records through the store owner; no stale save remains armed.
    let mut removed = List::with_capacity(domain.kept.capacity());
    for record in &domain.kept {
        match record {
            Record::Draft { party: owner, task, .. } => {
                if *owner == party && domain.drafts.get(*task).is_none() {
                    removed.push(*task).expect("removed keys fit the records bound");
                }
            }
            Record::Place { .. } => {}
        }
    }
    for task in removed.into_boxed() {
        crate::store::erase(domain, StoreKey::Draft { task }, out);
    }
}
/// Restores only when the party or the active task changes; typing is never overwritten.
pub(crate) fn sync(domain: &mut Ui, out: &mut Queue<Request>) {
    if domain.drafts.scope != domain.confirmed {
        restore(domain, out);
    }
    let active = if domain.confirmed.is_some() { domain.shown_task() } else { None };
    if domain.drafts.active != active {
        if let Some(previous) = domain.drafts.active
            && domain.drafts.get(previous).is_none()
        {
            domain.fields.remove(Purpose::Composer { task: previous });
        }
        domain.drafts.active = active;
        if let Some(task) = active {
            let words = match domain.drafts.get(task) {
                Some(draft) => draft.words.clone(),
                None => Text::new(Box::from([]), 0).expect("empty text fits"),
            };
            match domain.write_field(Purpose::Composer { task }, words) {
                Ok(()) => {}
                Err(which) => crate::notices::kept::show(domain, crate::notices::kept::Kind::Refused(which)),
            }
        }
    }
}
/// Clears only the original scope and touch after a durable message number exists.
pub(crate) fn sent(
    domain: &mut Ui,
    scope: u64,
    task: u64,
    touched: Option<u64>,
    ask: &Ask,
    answer: &Answer,
    out: &mut Queue<Request>,
) {
    let Some(words) = crate::activity::sending::words(ask, task) else {
        return;
    };
    let message_task = match answer {
        Answer::Done(jig_client::contract::Outcome::Owner(jig_client::contract::OwnerOutcome::Compound { .. })) => task,
        Answer::Done(outcome) => match crate::outcome::route(outcome) {
            crate::outcome::Route::Words { task, .. } => task,
            crate::outcome::Route::Created { .. } | crate::outcome::Route::Proposed | crate::outcome::Route::Other => {
                return;
            }
        },
        Answer::Refused { refusal: _, .. } => return,
    };
    if message_task != task || domain.confirmed != Some(scope) {
        return;
    }
    let Some(draft) = domain.drafts.get(task) else {
        return;
    };
    if Some(draft.touched) != touched || draft.words != *words {
        return;
    }
    let empty = Text::new(Box::from([]), 0).expect("empty text fits");
    if domain.write_field(Purpose::Composer { task }, empty).is_err() {
        return;
    }
    domain.drafts.remove(task);
    crate::store::erase(domain, StoreKey::Draft { task }, out);
}
