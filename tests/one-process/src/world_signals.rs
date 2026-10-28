//! Factories close unused startup signal descriptors before running their host.
use skein_io::kernel::{Complete, Fd, Op, Submit};
use skein_lib::{Queue, Time, Token, Wall};
use skein_shell::Host;
use std::ops::{Deref, DerefMut};
const CLOSE: Token = Token::new(u64::MAX);
pub(super) struct SignalHost<H> {
    host: H,
    signal: Option<Fd>,
    closing: bool,
}
impl<H> SignalHost<H> {
    pub fn new(host: H, signal: Option<Fd>) -> Self {
        Self { host, signal, closing: false }
    }
}
impl<H> Deref for SignalHost<H> {
    type Target = H;
    fn deref(&self) -> &H {
        &self.host
    }
}
impl<H> DerefMut for SignalHost<H> {
    fn deref_mut(&mut self) -> &mut H {
        &mut self.host
    }
}
impl<H: Host> Host for SignalHost<H> {
    fn drain(&mut self) {
        self.host.drain();
    }
    fn iterate(&mut self, now: Time, wall: Wall) {
        for _ in 0..self.host.completions().len() {
            let complete = self.host.completions().pop().expect("completion count");
            if complete.op == CLOSE {
                complete.result.expect("startup signal close");
                self.signal = None;
            } else {
                self.host.completions().push(complete);
            }
        }
        if !self.closing
            && self.host.submissions().room() > 0
            && let Some(fd) = self.signal
        {
            self.closing = true;
            self.host.submissions().push(Submit { op: CLOSE, kind: Op::Close { fd } });
        }
        self.host.iterate(now, wall);
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        self.host.completions()
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        self.host.submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        self.host.work_pending(now) || (self.signal.is_some() && !self.closing)
    }
    fn next_deadline(&self) -> Option<Time> {
        self.host.next_deadline()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.host.next_policy_deadline()
    }
    fn is_empty(&self) -> bool {
        self.signal.is_none() && self.host.is_empty()
    }
    fn operations(&self) -> u32 {
        self.host.operations() + 1
    }
}
