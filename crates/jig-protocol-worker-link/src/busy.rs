//! Busy turns name the hub's retained copy (protocol/channel.md, 5.2).
use crate::Limits;
use skein_lib::{Deadlines, Duration, Map, Rng, Time, Token};
type Key = (Token, Token, u32);
#[derive(Debug)]
pub(crate) struct Busy {
    deadlines: Deadlines<Key>,
    keys: Map<Key, ()>,
    span: Duration,
    rng: Rng,
}
impl Busy {
    pub(crate) fn new(limits: &Limits, seed: u64) -> Self {
        let capacity = limits.sizes.slots.checked_mul(limits.sizes.turns).expect("checked capacity");
        Self {
            deadlines: Deadlines::with_capacity(capacity),
            keys: Map::with_capacity(capacity),
            span: limits.busy,
            rng: Rng::new(seed),
        }
    }
    pub(crate) fn add(&mut self, now: Time, run: Token, attempt: Token, turn: u32) -> bool {
        let key = (run, attempt, turn);
        let high = self.span.as_nanos();
        let span = Duration::from_nanos(self.rng.between(high.div_euclid(2), high));
        if self.deadlines.arm(key, now.saturating_add(span)).is_err() {
            return false;
        }
        self.keys.insert(key, ()).expect("same capacity as deadlines");
        true
    }
    pub(crate) fn kept(&mut self, run: Token, attempt: Token, turn: u32) {
        let key = (run, attempt, turn);
        self.deadlines.cancel(key);
        self.keys.remove(&key);
    }
    pub(crate) fn acknowledged(&mut self, run: Token, attempt: Token) {
        for _ in 0..self.keys.len() {
            let Some((key, ())) = self.keys.range((run, attempt, 0)..=(run, attempt, u32::MAX)).next() else { break };
            let (run, attempt, turn) = *key;
            self.kept(run, attempt, turn);
        }
    }
    pub(crate) fn clear(&mut self) {
        for _ in 0..self.keys.len() {
            let Some((&(run, attempt, turn), ())) = self.keys.first() else { break };
            self.kept(run, attempt, turn);
        }
    }
    pub(crate) fn next(&self) -> Option<Time> {
        self.deadlines.next()
    }
    pub(crate) fn expire(&mut self, now: Time) -> Option<Key> {
        let key = self.deadlines.expire(now)?;
        self.keys.remove(&key);
        Some(key)
    }
}
