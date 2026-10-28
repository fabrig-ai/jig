//! Offered topics, completed hub operations, and classified news (connectors.md, 5.3).
use crate::{Domain, Work, numbered};
use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_core as core;
use jig_domain_tasks as tasks;
use jig_test_connector as connector;
use skein_lib::{Decimal, List, Queue, ReplyTo, Token, Writer};

pub(super) fn start(
    domain: &mut Domain,
    to: ReplyTo,
    key: core::CallKey,
    number: u16,
    topic: u16,
    work: &mut Queue<Work>,
) {
    let token = Token::new(domain.next_payload);
    domain.next_payload = domain.next_payload.checked_add(1).expect("bounded topic calls");
    assert!(domain.topic_calls.insert(token, (to, key, topic)).is_ok(), "synchronous topic call room");
    work.push(Work::Connector { number, event: connector::Event::DescribeTopic { token, topic } });
}

pub(super) fn described(
    domain: &mut Domain,
    number: u16,
    token: Token,
    resources: Option<Box<[connector::Path]>>,
    work: &mut Queue<Work>,
) {
    let Some((to, key, topic)) = domain.topic_calls.remove(&token) else { return };
    if !domain.core.current_proof(key.task, key.attempt) {
        return;
    }
    match resources {
        Some(resources) => {
            let mut descriptions =
                List::with_capacity(u32::try_from(resources.len()).expect("bounded topic resources"));
            for resource in resources {
                descriptions
                    .push(authority::Effect {
                        connector: number,
                        kind: 0,
                        name: authority::Name { segments: resource.segments().into() },
                        state: [0; 32],
                        price: None,
                        access: authority::EffectAccess::Context,
                        additional: Box::new([]),
                        guards: Box::new([]),
                    })
                    .expect("one read per resource");
            }
            work.push(Work::Core(core::Event::SubscribeTopic {
                to,
                key,
                connector: number,
                topic: u64::from(topic),
                resources: descriptions.into_boxed(),
            }));
        }
        None => {
            work.push(Work::Core(core::Event::NamedAnswer {
                to,
                key,
                part: core::CallPart::SubscriptionRefused(tasks::Problem {
                    limit: None,
                    task: Some(key.task),
                    why: tasks::Refusal::Subscription,
                    blocked_by: None,
                    capacity: None,
                }),
            }));
        }
    }
}

pub(super) fn subscribed(
    domain: &mut Domain,
    number: u16,
    request: Token,
    key: core::CallKey,
    subscription: u64,
    work: &mut Queue<Work>,
) {
    if let Some(row) = domain.core.tasks.task(key.task) {
        for interest in &row.subscriptions {
            if interest.number == subscription {
                match interest.kind {
                    tasks::SubscriptionKind::Topic { connector, topic } if connector == number => {
                        work.push(Work::Connector {
                            number,
                            event: connector::Event::Subscribe {
                                task: key.task,
                                topic: u16::try_from(topic).expect("connector topic number"),
                                wake_at: 2,
                                keep_at: 1,
                            },
                        });
                    }
                    tasks::SubscriptionKind::Topic { .. }
                    | tasks::SubscriptionKind::Task { .. }
                    | tasks::SubscriptionKind::Timer { .. } => {}
                }
            }
        }
    }
    if number == 1 {
        work.push(Work::Core(core::Event::NamedAnswer {
            to: ReplyTo::new(request),
            key,
            part: core::CallPart::Subscribed { subscription },
        }));
    }
}

pub(super) fn sync(domain: &mut Domain, number: u16, task: u64, work: &mut Queue<Work>) {
    for topic in numbered(domain, number).subscribed_topics(task) {
        let mut live = false;
        if let Some(row) = domain.core.tasks.task(task) {
            for interest in &row.subscriptions {
                if interest.kind == (tasks::SubscriptionKind::Topic { connector: number, topic: u64::from(topic) }) {
                    live = true;
                }
            }
        }
        if !live {
            work.push(Work::Connector { number, event: connector::Event::Unsubscribe { task, topic } });
        }
    }
}

pub(super) fn unsubscribed(
    domain: &mut Domain,
    number: u16,
    request: Token,
    key: core::CallKey,
    work: &mut Queue<Work>,
) {
    sync(domain, number, key.task, work);
    if number == 1 {
        work.push(Work::Core(core::Event::NamedAnswer {
            to: ReplyTo::new(request),
            key,
            part: core::CallPart::Unsubscribed,
        }));
    }
}

pub(super) fn news(
    domain: &Domain,
    number: u16,
    topic: u16,
    subscribers: Box<[connector::Classed]>,
    work: &mut Queue<Work>,
) {
    for subscriber in subscribers {
        let Some(row) = domain.core.tasks.task(subscriber.task) else { continue };
        for interest in &row.subscriptions {
            if interest.kind == (tasks::SubscriptionKind::Topic { connector: number, topic: u64::from(topic) }) {
                let class = match subscriber.class {
                    connector::Class::Wake => tasks::NewsClass::Wakes,
                    connector::Class::Keep => tasks::NewsClass::Kept,
                };
                let digits = Decimal::of(u64::from(topic));
                let size = b"News on topic ".len().checked_add(digits.as_bytes().len()).expect("bounded topic words");
                let mut words = Writer::new(size);
                words.put(b"News on topic ").expect("topic label fits");
                words.put(digits.as_bytes()).expect("topic number fits");
                work.push(Work::Core(core::Event::EffectConnector(core::connector::Event::News {
                    task: subscriber.task,
                    subscription: interest.number,
                    topic: u64::from(topic),
                    class,
                    words: words.finish(),
                })));
            }
        }
    }
}
