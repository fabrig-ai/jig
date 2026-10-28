//! Calls retained across channels (protocol/channel.md, 10.1).
use crate::Limits;
use alloc::boxed::Box;
use skein_channel::{Frame, frame_writer};
use skein_lib::{List, Map, Token, bytes};
type Key = (Token, Token, Box<[u8]>);
#[derive(Debug)]
pub(crate) struct Relays {
    records: Map<Key, Frame>,
    order: List<Key>,
}
impl Relays {
    pub(crate) fn new(limits: &Limits) -> Self {
        let capacity = limits.sizes.slots.checked_mul(limits.sizes.calls).expect("checked capacity");
        Self { records: Map::with_capacity(capacity), order: List::with_capacity(capacity) }
    }
    pub(crate) fn add(&mut self, run: Token, attempt: Token, call: Box<[u8]>, frame: &Frame) -> Result<(), Box<[u8]>> {
        let key = (run, attempt, call);
        if self.records.contains_key(&key) {
            return Ok(());
        }
        if self.records.insert(key.clone(), copy(frame)).is_err() {
            return Err(key.2);
        }
        self.order.push(key).expect("same capacity as records");
        Ok(())
    }
    pub(crate) fn end(&mut self, run: Token, attempt: Token, call: &[u8]) -> bool {
        let key = (run, attempt, bytes::copy_of(call));
        if self.records.remove(&key).is_none() {
            return false;
        }
        let mut to = 0;
        for from in 0..self.order.len() {
            let item = self.order.get(from).expect("order index");
            if item != &key {
                let item = item.clone();
                *self.order.get_mut(to).expect("order index") = item;
                to = to.checked_add(1).expect("within order length");
            }
        }
        self.order.truncate(to);
        true
    }
    pub(crate) fn replay(&self, index: u32) -> Option<Frame> {
        let record = self.records.get(self.order.get(index)?)?;
        Some(copy(record))
    }
}
fn copy(record: &Frame) -> Frame {
    let mut copy = frame_writer(record.kind(), record.body_len()).expect("retained frame bounds");
    copy.put(record.bytes().get(8..).expect("retained header")).expect("retained body");
    copy.finish().expect("retained frame")
}
