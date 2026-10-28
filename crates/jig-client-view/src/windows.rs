//! Concrete bounded windows sharing edges and one sliding rule.
//! Reading beyond a full window drops the opposite end and keeps the supplied
//! reverse cursor; live rows enter only at an end the window still reaches.
//! See client/domain/client.md, 7. Row owners supply cursors for re-reading drops.
use jig_client::thread::window::{Edge, Edges, End, edges, plan};

use jig_client::contract::{HistoryRow, ObjectKey, TaskLine};
use skein_lib::List;

/// Object names on a page.
#[derive(Debug)]
pub struct ObjectWindow {
    pub rows: List<ObjectKey>,
    pub edges: Edges,
}
impl ObjectWindow {
    /// Allocates a fixed row capacity and starts at both list ends.
    #[must_use]
    pub fn new(capacity: u32) -> Self {
        Self { rows: List::with_capacity(capacity), edges: Edges { before: Edge::End, after: Edge::End } }
    }
    /// Slides from one end, returning the number dropped from the other.
    pub fn slide(&mut self, incoming: List<ObjectKey>, incoming_edges: Edges, end: End) -> u32 {
        let capacity = self.rows.capacity();
        let empty = self.rows.is_empty();
        let slide = plan(self.rows.len(), incoming.len(), capacity, end);
        let mut rows = List::with_capacity(capacity);
        let old = core::mem::replace(&mut self.rows, List::with_capacity(0));
        match end {
            End::After => {
                move_objectwindow(old, slide.old_from, slide.old_count, &mut rows);
                move_objectwindow(incoming, slide.new_from, slide.new_count, &mut rows);
            }
            End::Before => {
                move_objectwindow(incoming, slide.new_from, slide.new_count, &mut rows);
                move_objectwindow(old, slide.old_from, slide.old_count, &mut rows);
            }
        }
        self.rows = rows;
        let current = core::mem::replace(&mut self.edges, Edges { before: Edge::End, after: Edge::End });
        self.edges = edges(current, incoming_edges, slide.dropped, empty, end);
        slide.dropped
    }
    /// Adds live rows only if this window reaches their end.
    pub fn live(&mut self, incoming: List<ObjectKey>, incoming_edges: Edges, end: End) -> bool {
        if !self.edges.reaches(end) {
            return false;
        }
        let dropped = self.slide(incoming, incoming_edges, end);
        if dropped > 0 {
            assert!(self.rows.len() == self.rows.capacity(), "a dropping slide fills its window");
        }
        true
    }
}
fn move_objectwindow(rows: List<ObjectKey>, start: u32, count: u32, out: &mut List<ObjectKey>) {
    let until = start.saturating_add(count);
    for (index, row) in rows.into_boxed().into_iter().enumerate() {
        let index = u32::try_from(index).expect("a bounded list index fits u32");
        if index >= start && index < until {
            out.push(row).expect("the slide plan fits the window");
        }
    }
}

/// Conversations in the party list.
#[derive(Clone, Debug)]
pub struct ConversationWindow {
    /// A retained read reported page loss; independent from its row edges.
    pub cut: bool,
    pub rows: List<TaskLine>,
    pub edges: Edges,
}
impl ConversationWindow {
    /// Allocates a fixed row capacity and starts at both list ends.
    #[must_use]
    pub fn new(capacity: u32) -> Self {
        Self { cut: false, rows: List::with_capacity(capacity), edges: Edges { before: Edge::End, after: Edge::End } }
    }
    /// Merges by task key before sliding; a retained row wins over a read overlap.
    pub fn slide(&mut self, incoming: List<TaskLine>, incoming_edges: Edges, end: End) -> u32 {
        let key_capacity = self.rows.len().checked_add(incoming.len()).expect("admitted row counts fit");
        let mut keys = List::with_capacity(key_capacity);
        let old_count = conversation_count(&self.rows);
        match end {
            End::After => conversation_after_keys(&self.rows, &incoming, &mut keys),
            End::Before => {
                conversation_keys(&self.rows, &mut keys);
                conversation_keys(&incoming, &mut keys);
            }
        }
        self.merge(incoming, incoming_edges, end, keys, old_count)
    }
    fn merge(
        &mut self,
        incoming: List<TaskLine>,
        incoming_edges: Edges,
        end: End,
        keys: List<u64>,
        old_count: u32,
    ) -> u32 {
        let capacity = self.rows.capacity();
        let empty = self.rows.is_empty();
        let new_count = keys.len().checked_sub(old_count).expect("keys only append");
        let slide = plan(old_count, new_count, capacity, end);
        let mut rows = List::with_capacity(capacity);
        let old = core::mem::replace(&mut self.rows, List::with_capacity(0));
        let new_from = old_count.checked_add(slide.new_from).expect("bounded key range fits");
        match end {
            End::After => {
                let count = keys.len().min(capacity);
                let start = keys.len().saturating_sub(count);
                move_conversationwindow(old, &keys, start, count, &mut rows);
                move_conversationwindow(incoming, &keys, start, count, &mut rows);
                order_conversationwindow(&mut rows, &keys, start);
            }
            End::Before => {
                move_conversationwindow(incoming, &keys, new_from, slide.new_count, &mut rows);
                move_conversationwindow(old, &keys, slide.old_from, slide.old_count, &mut rows);
            }
        }
        self.rows = rows;
        let current = core::mem::replace(&mut self.edges, Edges { before: Edge::End, after: Edge::End });
        self.edges = edges(current, incoming_edges, slide.dropped, empty, end);
        slide.dropped
    }
    /// Adds live rows only if this window reaches their end.
    pub fn live(&mut self, incoming: List<TaskLine>, incoming_edges: Edges, end: End) -> bool {
        if !self.edges.reaches(end) {
            return false;
        }
        let dropped = self.slide(incoming, incoming_edges, end);
        if dropped > 0 {
            assert!(self.rows.len() == self.rows.capacity(), "a dropping slide fills its window");
        }
        true
    }
}
fn conversation_keys(rows: &List<TaskLine>, keys: &mut List<u64>) {
    for row in rows {
        if !keys.as_slice().contains(&row.chip.task) {
            keys.push(row.chip.task).expect("one key per admitted row fits");
        }
    }
}
fn conversation_count(rows: &List<TaskLine>) -> u32 {
    let mut count = 0_u32;
    for index in 0..rows.len() {
        let key = rows.get(index).expect("row exists").chip.task;
        let mut present = false;
        for earlier in 0..index {
            if rows.get(earlier).expect("row exists").chip.task == key {
                present = true;
                break;
            }
        }
        if !present {
            count = count.checked_add(1).expect("a distinct key count fits the bounded rows");
        }
    }
    count
}
fn conversation_named(rows: &List<TaskLine>, key: u64) -> bool {
    for row in rows {
        if row.chip.task == key {
            return true;
        }
    }
    false
}
fn conversation_key(keys: &mut List<u64>, key: u64) {
    if !keys.as_slice().contains(&key) {
        keys.push(key).expect("the distinct union fits the admitted key capacity");
    }
}
/// A replay may introduce source rows before a retained overlap. The supplied
/// page's keys keep their order; current rows still supply the whole facts.
/// Retained gaps remain beside their old preceding source anchor, without an
/// inferred clock or numeric ordering. The bounded After slide selects its tail.
fn conversation_after_keys(old: &List<TaskLine>, incoming: &List<TaskLine>, keys: &mut List<u64>) {
    for row in old {
        if conversation_named(incoming, row.chip.task) {
            break;
        }
        conversation_key(keys, row.chip.task);
    }
    for row in incoming {
        conversation_key(keys, row.chip.task);
        let mut following = false;
        for held in old {
            if held.chip.task == row.chip.task {
                following = true;
            } else if following {
                if conversation_named(incoming, held.chip.task) {
                    break;
                }
                conversation_key(keys, held.chip.task);
            }
        }
    }
    conversation_keys(old, keys);
}
fn order_conversationwindow(rows: &mut List<TaskLine>, keys: &List<u64>, start: u32) {
    for first in 0..rows.len() {
        let key = keys.get(start.checked_add(first).expect("selected key range fits")).expect("key exists");
        for second in first..rows.len() {
            if rows.get(second).expect("row exists").chip.task == *key {
                if second != first {
                    let row = rows.get(second).expect("row exists").clone();
                    let row = core::mem::replace(rows.get_mut(first).expect("row exists"), row);
                    *rows.get_mut(second).expect("row exists") = row;
                }
                break;
            }
        }
    }
}
fn move_conversationwindow(rows: List<TaskLine>, keys: &List<u64>, start: u32, count: u32, out: &mut List<TaskLine>) {
    let until = start.saturating_add(count);
    for row in rows.into_boxed() {
        let mut selected = false;
        for index in start..until {
            if keys.get(index) == Some(&row.chip.task) {
                selected = true;
                break;
            }
        }
        let mut present = false;
        for held in &*out {
            if held.chip.task == row.chip.task {
                present = true;
                break;
            }
        }
        if selected && !present {
            out.push(row).expect("the distinct key slide fits the window");
        }
    }
}

/// Task or subtree history lines.
#[derive(Debug)]
pub struct HistoryWindow {
    /// A retained read reported page loss; independent from its row edges.
    pub cut: bool,
    pub rows: List<HistoryRow>,
    pub edges: Edges,
}
impl HistoryWindow {
    /// Allocates a fixed row capacity and starts at both list ends.
    #[must_use]
    pub fn new(capacity: u32) -> Self {
        Self { cut: false, rows: List::with_capacity(capacity), edges: Edges { before: Edge::End, after: Edge::End } }
    }
    /// Slides from one end, returning the number dropped from the other.
    pub fn slide(&mut self, incoming: List<HistoryRow>, incoming_edges: Edges, end: End) -> u32 {
        let capacity = self.rows.capacity();
        let empty = self.rows.is_empty();
        let slide = plan(self.rows.len(), incoming.len(), capacity, end);
        let mut rows = List::with_capacity(capacity);
        let old = core::mem::replace(&mut self.rows, List::with_capacity(0));
        match end {
            End::After => {
                move_historywindow(old, slide.old_from, slide.old_count, &mut rows);
                move_historywindow(incoming, slide.new_from, slide.new_count, &mut rows);
            }
            End::Before => {
                move_historywindow(incoming, slide.new_from, slide.new_count, &mut rows);
                move_historywindow(old, slide.old_from, slide.old_count, &mut rows);
            }
        }
        self.rows = rows;
        let current = core::mem::replace(&mut self.edges, Edges { before: Edge::End, after: Edge::End });
        self.edges = edges(current, incoming_edges, slide.dropped, empty, end);
        slide.dropped
    }
    /// Adds live rows only if this window reaches their end.
    pub fn live(&mut self, incoming: List<HistoryRow>, incoming_edges: Edges, end: End) -> bool {
        if !self.edges.reaches(end) {
            return false;
        }
        let dropped = self.slide(incoming, incoming_edges, end);
        if dropped > 0 {
            assert!(self.rows.len() == self.rows.capacity(), "a dropping slide fills its window");
        }
        true
    }
}
fn move_historywindow(rows: List<HistoryRow>, start: u32, count: u32, out: &mut List<HistoryRow>) {
    let until = start.saturating_add(count);
    for (index, row) in rows.into_boxed().into_iter().enumerate() {
        let index = u32::try_from(index).expect("a bounded list index fits u32");
        if index >= start && index < until {
            out.push(row).expect("the slide plan fits the window");
        }
    }
}
/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub(crate) fn valid(limits: &crate::Limits) -> bool {
    limits.rows.checked_mul(2).is_some()
}
