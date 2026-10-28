//! Offered topic calls are reads before they enter the task hub (authority.md, 8.5).
use crate::{CallKey, CallPart, Core, Event, Family, Held, Limits, Request, Requests, fresh};
use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_tasks as tasks;
use skein_lib::{Env, Queue, ReplyTo};

#[expect(clippy::too_many_arguments, reason = "one decoded topic call carries its read descriptions")]
pub(super) fn subscribe(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    to: ReplyTo,
    key: CallKey,
    connector: u16,
    topic: u64,
    resources: &[authority::Effect],
) -> Requests {
    let mut out = Queue::with_capacity(3);
    if !core.current_proof(key.task, key.attempt) {
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    if let Some(record) = core.call_record(key) {
        out.push(Request::Held(Box::new(Held::CallAnswer { to, key, part: record.part })));
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    if core.pending_calls.contains_key(&key)
        || core.call_parts.len().saturating_add(core.pending_calls.len()) >= env.limits.call_records
        || core.counters.deployment().calls == u64::MAX
    {
        out.push(Request::Now(Box::new(crate::Now::EffectAnswer { to, key, part: CallPart::Unavailable })));
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    let mut findings = Queue::uncapped();
    let answer = core.topic_admit(key, resources, &mut findings);
    if answer != authority::Answer::Allow {
        let part = CallPart::ToolDenied {
            answer,
            findings: {
                let mut found = skein_lib::List::with_capacity(findings.len());
                for _ in 0..findings.len() {
                    found.push(findings.pop().expect("finding count")).expect("finding room");
                }
                found.into_boxed()
            },
        };
        super::runs::call_decision(core, &mut out, to, key, part);
    } else if let Some(subscription) = fresh(&mut core.counters, Family::Message) {
        let token = to.into_token();
        assert!(core.reserve_connector_subscription(token, key, subscription), "topic call room preflighted");
        work.push(Event::Tasks(tasks::Event::SubscribeTopic {
            reply_to: ReplyTo::new(token),
            task: key.task,
            subscription: tasks::Subscription {
                number: subscription,
                kind: tasks::SubscriptionKind::Topic { connector, topic },
            },
        }));
    } else {
        super::runs::call_decision(core, &mut out, to, key, CallPart::Unavailable);
    }
    out.push(Request::Decided);
    Requests::Out(out)
}
