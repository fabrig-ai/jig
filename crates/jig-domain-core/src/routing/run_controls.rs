//! Task-origin controls fill the finish message before entering the tasks child.
use crate::{CallKey, CallPart, Core, Event, Request, Requests, RoutedCall};
use jig_domain_tasks as tasks;
use skein_lib::{Queue, ReplyTo};

pub(super) fn control(
    core: &mut Core,
    work: &mut Queue<Event>,
    to: ReplyTo,
    key: CallKey,
    target: u64,
    mut control: tasks::Control,
) -> Requests {
    let mut out = Queue::with_capacity(3);
    if let tasks::Control::Cancel { grace: Some(_), message, .. } = &mut control {
        *message = crate::fresh(&mut core.counters, crate::Family::Message);
        if message.is_none() {
            super::runs::call_decision(
                core,
                &mut out,
                to,
                key,
                CallPart::ControlRefused(tasks::Problem {
                    limit: None,
                    task: Some(target),
                    why: tasks::Refusal::Busy,
                    blocked_by: None,
                    capacity: None,
                }),
            );
            out.push(Request::Decided);
            return Requests::Out(out);
        }
    }
    let token = to.into_token();
    assert!(core.pending_calls.insert(key, true).is_ok(), "call record room reserved");
    assert!(
        core.routing_calls.insert(token, RoutedCall::Control { key, target, revised: false }) == Ok(None),
        "one live routed call"
    );
    work.push(Event::Tasks(tasks::Event::Control {
        reply_to: ReplyTo::new(token),
        by: tasks::Party::Task(key.task),
        task: target,
        control,
    }));
    out.push(Request::Decided);
    Requests::Out(out)
}
