//! Host handoffs and opaque settled answers (domain/engine.md, 7.2–7.3;
//! domain/hosts.md, 2). Protocol decoding and rendering remain with the root.

use crate::{CallKey, CallRecord, Core, CoreRecord, Held, Limits, Now, Record, Request, Requests, Write};
use alloc::boxed::Box;
use skein_lib::{Queue, ReplyTo};

/// The original host tool and answer; its name is its key's (store.md, 5.1).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct SettledCall {
    /// Core-issued order of this settled answer; the protocol supplies zero.
    pub serial: u64,
    pub tool: Box<[u8]>,
    pub answer: SettledAnswer,
}

/// A settled answer and its original key, carried together on replay (store.md, 5.1).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Settled {
    pub key: CallKey,
    pub call: SettledCall,
}

/// An answer is opaque conversation data, including replay evidence for delivery.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum SettledAnswer {
    Host { error: bool, body: Box<[u8]> },
    Delivery { outcome: DeliveryOutcome, evidence: Box<[u8]> },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DeliveryOutcome {
    Delivered,
    Nothing,
    Stale,
    Refused,
    Invalid,
    Failed,
}

impl SettledCall {
    #[must_use]
    pub fn owned_bytes(&self) -> Option<u64> {
        let answer = match &self.answer {
            SettledAnswer::Host { body, .. } => body.len(),
            SettledAnswer::Delivery { evidence, .. } => evidence.len(),
        };
        u64::try_from(self.tool.len()).ok()?.checked_add(u64::try_from(answer).ok()?)
    }

    #[must_use]
    pub fn valid(&self, limits: &Limits) -> bool {
        !self.tool.is_empty()
            && match self.owned_bytes() {
                Some(bytes) => bytes <= u64::from(limits.call_answer_bytes),
                None => false,
            }
    }
}

impl Core {
    /// The root wraps this whole core decision, including settled host evidence.
    #[must_use]
    pub fn call_record(&self, key: CallKey) -> Option<CallRecord> {
        Some(CallRecord {
            key,
            part: self.call_parts.get(&key)?.clone(),
            settled: self.call_settled.get(&key).cloned(),
        })
    }
    /// Settled answers after the last committed turn, in their commit order.
    #[must_use]
    pub fn settled_calls(&self, task: u64, attempt: u64) -> Box<[Settled]> {
        let mut rows = skein_lib::List::with_capacity(self.call_settled.len());
        let mut after = 0;
        for _ in 0..self.call_settled.len() {
            let mut next: Option<(CallKey, &SettledCall)> = None;
            for (key, call) in &self.call_settled {
                if key.task == task
                    && key.attempt < attempt
                    && call.serial > after
                    && match next {
                        Some((_, old)) => call.serial < old.serial,
                        None => true,
                    }
                {
                    next = Some((*key, call));
                }
            }
            let Some((key, call)) = next else { break };
            after = call.serial;
            rows.push(Settled { key, call: call.clone() }).expect("settled keys bound their answers");
        }
        rows.into_boxed()
    }
}

pub(crate) fn settle(
    core: &mut Core,
    limits: &Limits,
    to: ReplyTo,
    key: CallKey,
    name: Box<[u8]>,
    mut call: SettledCall,
) -> Requests {
    let mut out = Queue::with_capacity(3);
    if !call.valid(limits) || !core.call_parts.contains_key(&key) {
        out.push(Request::Now(Box::new(Now::SettledCallRefused { to, key, name, tool: call.tool })));
    } else {
        let settled = match core.call_settled.get(&key) {
            Some(old) => old.clone(),
            None => {
                let Some(serial) = crate::fresh(&mut core.counters, crate::Family::Call) else {
                    out.push(Request::Now(Box::new(Now::SettledCallRefused { to, key, name, tool: call.tool })));
                    out.push(Request::Decided);
                    return Requests::Out(out);
                };
                call.serial = serial;
                assert!(core.call_settled.insert(key, call.clone()).is_ok(), "settled calls fit their retained names");
                out.push(Request::Write(Write::Save(Record::Core(CoreRecord::Call(
                    core.call_record(key).expect("settled named call"),
                )))));
                call
            }
        };
        out.push(Request::Held(Box::new(Held::SettledCall { to, key, call: settled })));
    }
    out.push(Request::Decided);
    Requests::Out(out)
}
