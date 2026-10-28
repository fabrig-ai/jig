//! A task's committed conversation, newest first, in one sliding window.
//! Keeps opaque read cursors and page-owned call folds; knows no transcript
//! encoding, application values or engine internals. `moved` reads a new tail
//! only while the window reaches it; `older` and `newer` follow its cursors.
//! `accept` consumes one requested page and may ask for another new-tail page.
//! The owner routes reads and keeps returned object names live in its working set.
//! See client/domain/client.md, 7 and 10, and objects.md, 10.
//!
//! | State | Event | Next | Emits |
//! |---|---|---|---|
//! | idle, at newest | activity moved | latest read | activity read |
//! | idle | scroll to a cursor | edge read | activity read |
//! | latest read | page before known tail | latest read or idle | continuation or names |
//! | edge read | page | idle | names |
//! | any read | failed | idle | nothing |
use self::window::{Edge, Edges, End, TurnWindow};
use crate::contract::{
    Activity, ActivityLine, ActivityPage, ArchivedTurn, Block, CallIdentity, Cursor, LiveCall, ObjectKey, Part,
    ProvisionalTurnName, Read, RunState, RunText, Text, TurnName, TurnRead,
};
use skein_lib::{List, Map, Wall, Writer};
mod model;
mod shown;
pub mod window;
pub mod words;
pub use model::Line;
pub use words::{KeptWords, Sent, Words};

/// Bounds on retained activity, provisional text and submitted words.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub lines: u32,
    pub bytes: u64,
    pub text: u32,
}

/// The page's presentation of one call, independent of committed turn facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Fold {
    /// One summary line.
    Folded,
    /// Input and outcome details are visible.
    Open,
}
/// A call is named within a turn, so calls in different attempts stay distinct.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CallName {
    pub attempt: u64,
    pub turn: u32,
    pub call: u64,
}
/// The one activity read currently requested by this conversation.
#[derive(Debug)]
pub enum Reading {
    /// The newest pages, stopping at the already held committed position.
    Latest { known: u64 },
    /// Older activity at the end of the displayed window.
    Older,
    /// Activity nearer the present, after scrolling into history.
    Newer,
}
/// One turn's text is provisional until its committed turn is read.
#[derive(Clone, Debug)]
pub struct LiveTurn {
    pub name: ProvisionalTurnName,
    pub began: Option<Wall>,
    pub text: Text,
    pub offset: u32,
    pub cut: bool,
    pub call: Option<LiveCall>,
}
/// Committed lines and page state for one task conversation.
#[derive(Debug)]
pub struct Thread {
    pub live: List<LiveTurn>,
    pub limits: Limits,
    pub attempt: Option<u64>,
    pub left: u32,
    /// Loss reported by retained activity pages, independent of turn and text loss.
    pub page_cut: bool,
    pub task: u64,
    pub lines: TurnWindow,
    /// The newest position announced by the task document.
    pub committed: u64,
    /// The newest position this conversation has read.
    pub loaded: u64,
    pub reading: Option<Reading>,
    /// Whole new-tail pages waiting to meet the held committed position.
    pub fresh: TurnWindow,
    /// The displayed calls whose details the person opened.
    pub opened: Map<CallName, ()>,
    /// Opaque current-call folds are equality-only and independent of transcript numbers.
    pub current_opened: List<CallIdentity>,
    /// The latest committed turn read, fencing stale stream deliveries.
    pub committed_turn: Option<TurnName>,
    /// The task ended its run; unread committed activity is taken before cleanup.
    pub run_finished: bool,
    /// Pending snapshots and durable words retained until the agent read them.
    pub words: KeptWords,
    /// One live chips source for tasks named by calls in this window.
    pub chips_watch: Option<crate::Handle>,
    /// Task membership shared by all displayed engine calls.
    pub chips: List<u64>,
    /// Membership is waiting for the old watch terminal or available watch room.
    pub chips_pending: bool,
}
impl Thread {
    /// Allocates the activity window and equally bounded staging and fold state.
    #[must_use]
    pub fn new(task: u64, limits: &Limits) -> Self {
        let rows = limits.lines;
        Self {
            live: List::with_capacity(rows),
            limits: *limits,
            attempt: None,
            left: 0,
            page_cut: false,
            task,
            lines: TurnWindow::new(rows),
            committed: 0,
            loaded: 0,
            reading: None,
            fresh: TurnWindow::new(rows),
            opened: Map::with_capacity(rows),
            current_opened: List::with_capacity(rows),
            committed_turn: None,
            run_finished: false,
            words: KeptWords::new(rows),
            chips_watch: None,
            chips: List::with_capacity(rows),
            chips_pending: false,
        }
    }
    /// Reads a newer committed tail once, leaving a historical window in place.
    pub fn moved(&mut self, committed: u64) -> Option<Read> {
        self.committed = self.committed.max(committed);
        if self.reading.is_some() || !self.lines.edges.reaches(End::Before) || self.committed <= self.loaded {
            return None;
        }
        self.fresh.rows.clear();
        self.fresh.edges = Edges { before: Edge::End, after: Edge::End };
        self.reading = Some(Reading::Latest { known: self.loaded });
        Some(Read::Activity { task: self.task, before: None })
    }
    /// Reads the preceding activity page from the window's opaque older cursor.
    pub fn older(&mut self) -> Option<Read> {
        if self.reading.is_some() {
            return None;
        }
        let cursor = match &self.lines.edges.after {
            Edge::End => return None,
            Edge::More(cursor) => cursor.clone(),
        };
        self.reading = Some(Reading::Older);
        Some(Read::Activity { task: self.task, before: Some(cursor) })
    }
    /// Re-reads rows dropped at the present end while scrolling into history.
    pub fn newer(&mut self) -> Option<Read> {
        if self.reading.is_some() {
            return None;
        }
        let cursor = match &self.lines.edges.before {
            Edge::End => return self.moved(self.committed),
            Edge::More(cursor) => cursor.clone(),
        };
        self.reading = Some(Reading::Newer);
        Some(Read::Activity { task: self.task, before: Some(cursor) })
    }
    /// Takes a requested page; repeated rows never duplicate a committed line.
    /// The owner has validated its row and payload bounds before this call.
    pub fn accept(&mut self, page: ActivityPage) -> Option<Read> {
        let reading = self.reading.take()?;
        self.page_cut |= page.cut;
        self.words.observe(&page);
        let edges = page_edges(page.after, page.before);
        match reading {
            Reading::Latest { known } => {
                let available = self.fresh.rows.capacity().saturating_sub(self.fresh.rows.len());
                let mut rows = List::with_capacity(self.lines.rows.capacity());
                let mut reached = false;
                for row in page.rows.into_boxed() {
                    if row.position <= known {
                        reached = true;
                    } else if rows.push(row).is_err() {
                        break;
                    }
                }
                order(&mut rows);
                let fits = rows.len() <= available;
                if fits {
                    let dropped = self.fresh.slide(rows, edges, End::After);
                    assert!(dropped == 0, "whole refresh pages fit staging room");
                } else {
                    // Keep the previous page's cursor: a cursor after this page
                    // would skip its rows if only part of it fitted the window.
                    reached = false;
                }
                if known > 0 && fits && !reached && self.fresh.rows.len() < self.fresh.rows.capacity() {
                    match &self.fresh.edges.after {
                        Edge::More(cursor) => {
                            let cursor = cursor.clone();
                            self.reading = Some(Reading::Latest { known });
                            return Some(Read::Activity { task: self.task, before: Some(cursor) });
                        }
                        Edge::End => {}
                    }
                }
                if let Some(row) = self.fresh.rows.get(0) {
                    self.loaded = self.loaded.max(row.position);
                }
                let rows = core::mem::replace(&mut self.fresh.rows, List::with_capacity(self.lines.rows.capacity()));
                let edges = core::mem::replace(&mut self.fresh.edges, Edges { before: Edge::End, after: Edge::End });
                if !reached {
                    self.lines.rows.clear();
                    self.lines.edges = Edges { before: Edge::End, after: Edge::End };
                }
                let dropped = self.lines.slide(rows, edges, End::Before);
                self.left = self.left.saturating_add(dropped);
                if dropped > 0 {
                    assert!(self.lines.rows.len() == self.lines.rows.capacity(), "a dropping slide fills its window");
                }
            }
            Reading::Older | Reading::Newer => {
                let end = match reading {
                    Reading::Older => End::After,
                    Reading::Newer => End::Before,
                    Reading::Latest { .. } => unreachable!("latest handled above"),
                };
                let mut rows = List::with_capacity(self.lines.rows.capacity());
                for row in page.rows.into_boxed() {
                    let mut held = false;
                    for old in &self.lines.rows {
                        if old.position == row.position {
                            held = true;
                            break;
                        }
                    }
                    if !held && rows.push(row).is_err() {
                        break;
                    }
                }
                order(&mut rows);
                let dropped = self.lines.slide(rows, edges, end);
                self.left = self.left.saturating_add(dropped);
                if dropped > 0 {
                    assert!(self.lines.rows.len() == self.lines.rows.capacity(), "a dropping slide fills its window");
                }
            }
        }
        // Staged pages fence live text only once their rows enter the window.
        for at in 0..self.lines.rows.len() {
            let row = self.lines.rows.get(at).expect("bounded window row exists");
            if let Some(name) = row.event.committed_turn() {
                self.committed(name);
            }
        }
        self.prune_live();
        self.moved(self.committed)
    }
    /// Allows a failed or refused read to be retried without inventing a page.
    pub fn failed(&mut self) {
        self.reading = None;
        self.fresh.rows.clear();
    }
    /// Ends the provisional presentation after its committed activity has been read.
    pub fn end_run(&mut self) {
        self.live.clear();
        self.prune_folds(&[], None);
    }
    fn committed(&mut self, name: TurnName) {
        match self.committed_turn {
            Some(old) => {
                if later(name, old) {
                    self.committed_turn = Some(name);
                }
            }
            None => self.committed_turn = Some(name),
        }
    }
    /// Connector-owned parts carried by calls, results or news, requested by the root.
    #[must_use]
    pub fn parts(&self, capacity: u32, live: &[LiveTurn]) -> List<Part> {
        let mut parts = List::with_capacity(capacity);
        for line in &self.lines.rows {
            match &line.event {
                Activity::Turn { turn: TurnRead::Decoded(turn), .. }
                | Activity::Owner { decoded: Some(ArchivedTurn { turn: TurnRead::Decoded(turn), .. }), .. } => {
                    for message in &turn.messages {
                        for block in &message.blocks {
                            match block {
                                Block::Call(call) => {
                                    for part in &call.parts {
                                        add_part(&mut parts, part);
                                    }
                                }
                                Block::Text(_) | Block::Result { .. } | Block::Opaque { .. } => {}
                            }
                        }
                    }
                }
                Activity::News { part, .. } => {
                    if let Some(part) = part {
                        add_part(&mut parts, part);
                    }
                }
                Activity::Result { result } => {
                    if let Some(value) = &result.value {
                        match value {
                            crate::contract::ResultValue::Connector { part, .. } => {
                                add_part(&mut parts, part);
                            }
                            crate::contract::ResultValue::Report(_)
                            | crate::contract::ResultValue::Verdict { .. }
                            | crate::contract::ResultValue::OwnerVerdict { .. }
                            | crate::contract::ResultValue::Change { .. }
                            | crate::contract::ResultValue::Choice { .. }
                            | crate::contract::ResultValue::Approved { .. }
                            | crate::contract::ResultValue::Failure(_)
                            | crate::contract::ResultValue::Words(_) => {}
                        }
                    }
                }
                Activity::Turn { turn: TurnRead::Undecodable { .. }, .. }
                | Activity::Owner { decoded: Some(ArchivedTurn { turn: TurnRead::Undecodable { .. }, .. }), .. }
                | Activity::Owner { decoded: None, .. }
                | Activity::Words { .. }
                | Activity::RunBegan { .. }
                | Activity::RunEnded { .. }
                | Activity::Decision { .. } => {}
            }
        }
        for live in live {
            if let Some(call) = &live.call {
                match call {
                    LiveCall::Decoded(call) => {
                        for part in &call.parts {
                            add_part(&mut parts, part);
                        }
                    }
                    LiveCall::Raw(_) => {}
                }
            }
        }
        parts
    }
    /// Default folds are implicit; only opened calls consume page fold room.
    #[must_use]
    pub fn fold(&self, name: CallName) -> Fold {
        if self.opened.contains_key(&name) { Fold::Open } else { Fold::Folded }
    }
    fn displayed_call(&self, name: CallName, live: &[LiveTurn]) -> bool {
        if has_call(&self.lines.rows, name) {
            return true;
        }
        for live in live {
            if live.name.attempt == name.attempt
                && live.name.number == name.turn
                && let Some(LiveCall::Decoded(call)) = &live.call
                && call.number == name.call
            {
                return true;
            }
        }
        false
    }
    /// Opens or closes one displayed call, refusing unknown calls or full fold room.
    pub fn set_fold(&mut self, name: CallName, fold: Fold, live: &[LiveTurn]) -> bool {
        if !self.displayed_call(name, live) {
            return false;
        }
        match fold {
            Fold::Folded => {
                self.opened.remove(&name);
                true
            }
            Fold::Open => {
                if !self.opened.contains_key(&name)
                    && match self.opened.len().checked_add(self.current_opened.len()) {
                        Some(total) => total >= self.opened.capacity(),
                        None => true,
                    }
                {
                    return false;
                }
                self.opened.insert(name, ()).is_ok()
            }
        }
    }
    /// Presentation state for a raw call is keyed by its supplied opaque triple.
    #[must_use]
    pub fn current_fold(&self, identity: &CallIdentity) -> Fold {
        if self.current_opened.as_slice().contains(identity) { Fold::Open } else { Fold::Folded }
    }
    fn displayed_current_call(identity: &CallIdentity, live: &[LiveTurn], current_run: Option<&RunState>) -> bool {
        if let Some(run) = current_run {
            return match &run.call {
                Some(call) => &call.identity == identity,
                None => false,
            };
        }
        for live in live {
            if let Some(LiveCall::Raw(call)) = &live.call
                && &call.identity == identity
            {
                return true;
            }
        }
        false
    }
    /// Opening an unknown identity or exceeding the bounded fold table is refused.
    pub fn set_current_fold(
        &mut self,
        identity: CallIdentity,
        fold: Fold,
        live: &[LiveTurn],
        current_run: Option<&RunState>,
    ) -> bool {
        if !Self::displayed_current_call(&identity, live, current_run) {
            return false;
        }
        match fold {
            Fold::Open => {
                if self.current_opened.as_slice().contains(&identity) {
                    return true;
                }
                if match self.opened.len().checked_add(self.current_opened.len()) {
                    Some(total) => total >= self.current_opened.capacity(),
                    None => true,
                } {
                    return false;
                }
                self.current_opened.push(identity).is_ok()
            }
            Fold::Folded => {
                let mut kept = List::with_capacity(self.current_opened.capacity());
                for name in &self.current_opened {
                    if name != &identity {
                        kept.push(name.clone()).expect("folds remain bounded");
                    }
                }
                self.current_opened = kept;
                true
            }
        }
    }
    pub fn prune_folds(&mut self, live: &[LiveTurn], current_run: Option<&RunState>) {
        let mut gone = List::with_capacity(self.opened.capacity());
        for (name, ()) in &self.opened {
            if !self.displayed_call(*name, live) {
                gone.push(*name).expect("removed folds fit fold capacity");
            }
        }
        for name in gone.into_boxed() {
            self.opened.remove(&name);
        }
        let mut kept = List::with_capacity(self.current_opened.capacity());
        for identity in &self.current_opened {
            if Self::displayed_current_call(identity, live, current_run) {
                kept.push(identity.clone()).expect("remaining current folds fit");
            }
        }
        self.current_opened = kept;
    }
    /// Names cards produced by calls and heard results, deduplicated by object key.
    #[must_use]
    pub fn names(&self, capacity: u32, live: &[LiveTurn]) -> List<ObjectKey> {
        let mut names = List::with_capacity(capacity);
        for line in &self.lines.rows {
            match &line.event {
                Activity::Turn { turn: TurnRead::Decoded(turn), .. }
                | Activity::Owner { decoded: Some(ArchivedTurn { turn: TurnRead::Decoded(turn), .. }), .. } => {
                    for message in &turn.messages {
                        for block in &message.blocks {
                            match block {
                                Block::Call(call) => {
                                    for key in &call.objects {
                                        add_name(&mut names, *key);
                                    }
                                }
                                Block::Text(_) | Block::Result { .. } | Block::Opaque { .. } => {}
                            }
                        }
                    }
                }
                Activity::Result { result } => add_name(&mut names, ObjectKey::Result(result.task.task)),
                Activity::Decision { object, .. } => add_name(&mut names, *object),
                Activity::Turn { turn: TurnRead::Undecodable { .. }, .. }
                | Activity::Owner { decoded: Some(ArchivedTurn { turn: TurnRead::Undecodable { .. }, .. }), .. }
                | Activity::Owner { decoded: None, .. }
                | Activity::Words { .. }
                | Activity::RunBegan { .. }
                | Activity::RunEnded { .. }
                | Activity::News { .. } => {}
            }
        }
        for live in live {
            if let Some(call) = &live.call {
                match call {
                    LiveCall::Decoded(call) => {
                        for key in &call.objects {
                            add_name(&mut names, *key);
                        }
                    }
                    LiveCall::Raw(_) => {}
                }
            }
        }
        names
    }
}
fn add_name(names: &mut List<ObjectKey>, key: ObjectKey) {
    if !names.as_slice().contains(&key) {
        match names.push(key) {
            Ok(()) | Err(_) => {}
        }
    }
}
fn page_edges(newer: Option<Cursor>, older: Option<Cursor>) -> Edges {
    Edges {
        before: match newer {
            Some(cursor) => Edge::More(cursor),
            None => Edge::End,
        },
        after: match older {
            Some(cursor) => Edge::More(cursor),
            None => Edge::End,
        },
    }
}
fn order(rows: &mut List<ActivityLine>) {
    for first in 0..rows.len() {
        for second in first.saturating_add(1)..rows.len() {
            let a = rows.get(first).expect("bounded row exists");
            let b = rows.get(second).expect("bounded row exists");
            if b.position > a.position {
                let b = b.clone();
                let a = core::mem::replace(rows.get_mut(first).expect("bounded row exists"), b);
                *rows.get_mut(second).expect("bounded row exists") = a;
            }
        }
    }
}
fn has_call(rows: &List<ActivityLine>, name: CallName) -> bool {
    for row in rows {
        if let Some((turn_name, turn)) = row.event.decoded_turn()
            && turn_name.attempt == name.attempt
            && turn_name.number == name.turn
        {
            for message in &turn.messages {
                for block in &message.blocks {
                    match block {
                        Block::Call(call) => {
                            if call.number == name.call {
                                return true;
                            }
                        }
                        Block::Text(_) | Block::Result { .. } | Block::Opaque { .. } => {}
                    }
                }
            }
        }
    }
    false
}

#[must_use]
pub fn live_turn(value: RunText, limit: u32) -> LiveTurn {
    let cut = value.cut || value.from > 0 || !value.text.fits(limit);
    LiveTurn {
        name: value.turn,
        began: value.began,
        text: tail(&[], value.text.bytes(), limit),
        offset: value.through,
        cut,
        call: value.call,
    }
}

#[must_use]
pub fn later(name: TurnName, other: TurnName) -> bool {
    name.attempt > other.attempt || (name.attempt == other.attempt && name.number > other.number)
}
fn tail(old: &[u8], incoming: &[u8], limit: u32) -> Text {
    let length = old.len().checked_add(incoming.len()).expect("validated text lengths fit usize");
    let kept = length.min(usize::try_from(limit).expect("text limit fits usize"));
    let dropped = length.saturating_sub(kept);
    let old_from = dropped.min(old.len());
    let new_from = dropped.saturating_sub(old.len());
    let mut writer = Writer::new(kept);
    writer.put(old.get(old_from..).expect("old tail starts within text")).expect("old tail fits allocation");
    writer.put(incoming.get(new_from..).expect("new tail starts within text")).expect("new tail fits allocation");
    Text::new(writer.finish(), limit).expect("tail respects its byte bound")
}
fn add_part(parts: &mut List<Part>, part: &Part) {
    if !parts.as_slice().contains(part) {
        match parts.push(part.clone()) {
            Ok(()) | Err(_) => {}
        }
    }
}
