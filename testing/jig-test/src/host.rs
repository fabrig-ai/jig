//! The shell's adapter over the reference composition (protocol/service.md, 7).
use jig_test_service::{End, Service};
use skein_io::kernel::{Complete, Exit, Submit};
use skein_lib::{Queue, Time, Wall};
use std::io::{self, Write};

pub struct Host {
    pub service: Service,
    output: Box<dyn Write>,
    output_failed: bool,
}
impl Host {
    #[must_use]
    pub fn new(service: Service) -> Self {
        Self { service, output: Box::new(io::stdout()), output_failed: false }
    }
}
impl skein_shell::Host for Host {
    fn iterate(&mut self, now: Time, wall: Wall) {
        jig_test_service::iterate(&mut self.service, now, wall);
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        self.service.completions()
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        self.service.submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        self.service.work_pending(now)
    }
    fn next_deadline(&self) -> Option<Time> {
        self.service.next_deadline()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.service.next_policy_deadline()
    }
    fn is_empty(&self) -> bool {
        self.service.is_empty()
    }
    fn operations(&self) -> u32 {
        self.service.operations()
    }
    fn exit(&self) -> Option<Exit> {
        if !self.service.is_empty() {
            return None;
        }
        Some(Exit::Code(u8::from(self.output_failed || self.service.end() != Some(&End::Drained))))
    }
    fn drain(&mut self) {
        while let Some(link) = self.service.links().pop() {
            if self
                .output
                .write_all(&link)
                .and_then(|()| self.output.write_all(b"\n"))
                .and_then(|()| self.output.flush())
                .is_err()
            {
                self.output_failed = true;
                self.service.shutdown();
            }
        }
    }
}
