//! Routing the engine's drain without deciding connector work (engine.md, 6.1).
use crate::{Delivery, Domain, Event, Payload, Request, Work};
use alloc::boxed::Box;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use skein_lib::{Queue, ReplyTo, Token};

pub(super) fn defer(domain: &mut Domain, event: Event) -> Result<Event, ()> {
    if !domain.stopping() {
        return Ok(event);
    }
    match event {
        event @ (Event::Connector { .. }
        | Event::ConnectorTimer { .. }
        | Event::WriterRead { .. }
        | Event::ProjectionEffect { .. }
        | Event::TopicCall { .. }
        | Event::EffectCall { .. }) => {
            domain.busy.push(Box::new(event));
            Err(())
        }
        event @ (Event::Link { .. }
        | Event::ResultRead { .. }
        | Event::Stop
        | Event::WatchApplication { .. }
        | Event::ReadApplication { .. }
        | Event::Application { .. }
        | Event::Switched
        | Event::RestartBegin { .. }
        | Event::RestartDone(_)
        | Event::Timer(_)
        | Event::Core(_)
        | Event::Credential { .. }
        | Event::CallAnswered { .. }
        | Event::Text { .. }
        | Event::Turn { .. }
        | Event::Answer { .. }
        | Event::TranscriptLoaded { .. }
        | Event::Committed { .. }
        | Event::Failed { .. }) => Ok(event),
    }
}
pub(super) fn release(domain: &mut Domain, out: &mut Queue<Request>) {
    for _ in 0..domain.busy.len() {
        out.push(Request::Busy { event: domain.busy.pop().expect("returned connector input") });
    }
}
pub(super) fn call(domain: &mut Domain, to: ReplyTo, work: &mut Queue<Work>) {
    let answer = crate::payload(domain, Payload::Busy);
    work.push(Work::Core(core::Event::Fleet(fleet::Event::Relayed { to, answer })));
}
pub(super) fn relayed(
    domain: &mut Domain,
    channel: Token,
    run: Token,
    attempt: Token,
    name: Box<[u8]>,
    answer: Token,
    writes: bool,
) -> Delivery {
    match domain.payloads.remove(&answer).expect("retained relay answer") {
        Payload::Busy => Delivery::CallBusy { channel, task: run.raw(), attempt: attempt.raw(), name },
        Payload::SettledCall(call) => {
            Delivery::CallAnswer { channel, task: run.raw(), attempt: attempt.raw(), name, call, writes }
        }
        Payload::Turn { .. } | Payload::Answer { .. } | Payload::InboxWord(_) => {
            unreachable!("relay answer family")
        }
    }
}

impl Domain {
    #[must_use]
    pub fn stopping(&self) -> bool {
        self.core.stopping()
    }
}
