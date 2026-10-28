//! Root fixtures decode the public transport without replacing the client.
use super::support_domain as d;
use d::{infrastructure as infra, observability as obs};
use jig_client::{Down, Up};
use skein_lib::{Env, Queue, Token};
#[derive(Debug, PartialEq, Eq)]
pub enum JigOut {
    Read { read: Token, what: jig_client::contract::Read },
    Open { watch: Token, subject: jig_client::contract::Subject },
    Continue { watch: Token, subject: jig_client::contract::Subject },
    Close { watch: Token },
}
#[derive(Debug, PartialEq, Eq)]
pub enum Request {
    Save { record: d::store::Record },
    Erase { key: d::store::Key },
    Send { attempt: Token, key: skein_lib::RequestKey, ask: d::Ask },
    Jig(JigOut),
    Infrastructure(infra::Out),
    Observability(obs::Out),
    Show { address: d::Address, replace: bool },
    SignIn { provider: u16 },
    SignOut,
    Fact { fact: d::Fact },
}
#[derive(Debug)]
pub struct Domain {
    root: d::Domain,
    later: Vec<Request>,
    watches: alloc::collections::BTreeMap<Token, u16>,
    me: Option<Token>,
    frame: Option<Box<jig_client::contract::Me>>,
}
impl core::ops::Deref for Domain {
    type Target = d::Domain;
    fn deref(&self) -> &Self::Target {
        &self.root
    }
}
impl core::ops::DerefMut for Domain {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.root
    }
}
pub fn new(limits: &d::Limits, seed: u64) -> Domain {
    Domain {
        root: d::unlinked(limits, seed),
        later: Vec::new(),
        watches: alloc::collections::BTreeMap::new(),
        me: None,
        frame: None,
    }
}
pub fn step(root: &mut Domain, env: &Env<d::Limits>, event: d::Event, out: &mut Queue<Request>) {
    if let d::Event::Client {
        event:
            jig_client::Event::Up {
                up:
                    Up::Watched {
                        event: jig_client::contract::WatchEvent::Delivered(jig_client::contract::Delivery::Me(frame)),
                        ..
                    },
            },
    } = &event
    {
        root.frame = Some(frame.clone());
    }
    let events = callbacks(event);
    for event in events {
        let mut raw = Queue::uncapped();
        d::step(&mut root.root, env, event, &mut raw);
        collect(root, &mut raw, out);
    }
}
pub fn take(root: &mut Domain, env: &Env<d::Limits>, out: &mut Queue<Request>) {
    let mut raw = Queue::uncapped();
    d::fire(&mut root.root, env, &mut raw);
    d::take(&mut root.root, env, &mut raw);
    collect(root, &mut raw, out);
    for row in core::mem::take(&mut root.later) {
        out.push(row);
    }
}
fn collect(root: &mut Domain, raw: &mut Queue<d::Request>, out: &mut Queue<Request>) {
    while let Some(row) = raw.pop() {
        if let d::Request::Client {
            request:
                jig_client::Request::Down {
                    down: Down::Open { watch, subject: jig_client::Watchable::Jig(jig_client::contract::Subject::Me) },
                },
        } = &row
        {
            root.me = Some(*watch);
        }
        let row = observed(row, &mut root.watches);
        match row {
            row @ (Request::Send { .. }
            | Request::Save { record: d::store::Record::Unlinked(_) }
            | Request::Erase { key: d::store::Key::Unlinked(_) }) => root.later.push(row),
            row => out.push(row),
        }
    }
}
fn observed(row: d::Request, watches: &mut alloc::collections::BTreeMap<Token, u16>) -> Request {
    match row {
        d::Request::Client { request: jig_client::Request::Down { down } } => match down {
            Down::Send { attempt, key, request, .. } => Request::Send {
                attempt,
                key,
                ask: match request {
                    jig_client::Ask::Jig(value) => d::Ask::Jig(value),
                    jig_client::Ask::Family { record: value, .. } => {
                        d::Ask::Infrastructure(d::wire::ask_value(&value.bytes).expect("whole family ask"))
                    }
                },
            },
            Down::Read { read, what: jig_client::Readable::Jig(what) } => Request::Jig(JigOut::Read { read, what }),
            Down::Open { watch, subject: jig_client::Watchable::Jig(subject) } => {
                Request::Jig(JigOut::Open { watch, subject })
            }
            Down::Continue { watch, subject } => Request::Jig(JigOut::Continue { watch, subject }),
            Down::Close { watch } => match watches.get(&watch) {
                Some(1) => Request::Observability(obs::Out::Close { watch }),
                Some(2) => Request::Infrastructure(infra::Out::Close { watch }),
                Some(_) | None => Request::Jig(JigOut::Close { watch }),
            },
            Down::Read { read, what: jig_client::Readable::Family { scope, record, .. } } => match record.family {
                1 => Request::Observability(obs::Out::Read {
                    read,
                    subject: d::wire::observability::subject_value(&record.bytes, scope).expect("incident subject"),
                }),
                2 => Request::Infrastructure(infra::Out::Read {
                    read,
                    subject: d::wire::infrastructure::subject_value(&record.bytes, scope).expect("environment subject"),
                }),
                _ => panic!("declared family"),
            },
            Down::Open { watch, subject: jig_client::Watchable::Family { scope, record, .. } } => {
                watches.insert(watch, record.family);
                match record.family {
                    1 => Request::Observability(obs::Out::Open {
                        watch,
                        subject: d::wire::observability::subject_value(&record.bytes, scope).expect("incident subject"),
                    }),
                    2 => Request::Infrastructure(infra::Out::Open {
                        watch,
                        subject: d::wire::infrastructure::subject_value(&record.bytes, scope)
                            .expect("environment subject"),
                    }),
                    _ => panic!("declared family"),
                }
            }
            Down::Save { .. } | Down::Erase { .. } => panic!("root wraps stores"),
        },
        d::Request::Save { record } => Request::Save { record },
        d::Request::Erase { key } => Request::Erase { key },
        d::Request::Infrastructure(value) => Request::Infrastructure(value),
        d::Request::Observability(value) => Request::Observability(value),
        d::Request::Show { address, replace } => Request::Show { address, replace },
        d::Request::SignIn { provider } => Request::SignIn { provider },
        d::Request::SignOut | d::Request::Client { request: jig_client::Request::SignOut } => Request::SignOut,
        d::Request::Fact { fact } => Request::Fact { fact },
        d::Request::Client { .. } => panic!("unlinked fixture"),
    }
}
pub fn answer(attempt: Token, envelope: skein_lib::RequestEnvelope, answer: Option<d::Answered>) -> d::Event {
    let answer = answer.map(|value| match value {
        d::Answered::Jig(value) => jig_client::Answered::Jig(value),
        d::Answered::Infrastructure(infra::Answer::EntranceRefused(why)) => {
            jig_client::Answered::Jig(jig_client::contract::Answer::Refused { refusal: why.into_refusal(), at: None })
        }
        d::Answered::Infrastructure(value) => jig_client::Answered::Family(jig_client::Family {
            family: 2,
            version: 1,
            bytes: d::wire::answer(&value).expect("whole family answer"),
        }),
    });
    up(Up::Answer { attempt, envelope, answer, why: None })
}
pub fn final_answer(attempt: Token, answer: d::Answered) -> d::Event {
    self::answer(attempt, skein_lib::RequestEnvelope::Final, Some(answer))
}

fn up(up: Up) -> d::Event {
    d::Event::Client { event: jig_client::Event::Up { up } }
}
fn callbacks(event: d::Event) -> Vec<d::Event> {
    match event {
        d::Event::Client { event: jig_client::Event::Up { up: Up::Served { served, versions } } } => Vec::from([
            up(Up::Served { served, versions }),
            up(Up::FamilyServed {
                family: 1,
                versions: Some(jig_client::contract::VersionRange { first: 1, last: 1 }),
            }),
            up(Up::FamilyServed {
                family: 2,
                versions: Some(jig_client::contract::VersionRange { first: 1, last: 1 }),
            }),
        ]),

        d::Event::Infrastructure(infra::Event::Read { read, result }) => {
            Vec::from([up(Up::ReadDone { read, result: infra_read(result) })])
        }
        d::Event::Observability(obs::Event::Read { read, result }) => {
            Vec::from([up(Up::ReadDone { read, result: obs_read(result) })])
        }
        d::Event::Infrastructure(infra::Event::Watch { watch, event: infra::WatchEvent::Delivered(value) }) => {
            Vec::from([up(Up::FamilyDelivered {
                watch,
                record: jig_client::Family {
                    family: 2,
                    version: 1,
                    bytes: d::wire::infrastructure::document(&value).expect("whole environment"),
                },
            })])
        }
        d::Event::Observability(obs::Event::Watch { watch, event: obs::WatchEvent::Delivered(value) }) => {
            Vec::from([up(Up::FamilyDelivered {
                watch,
                record: jig_client::Family {
                    family: 1,
                    version: 1,
                    bytes: d::wire::observability::document(&value).expect("whole incident"),
                },
            })])
        }
        event => Vec::from([event]),
    }
}
macro_rules! read {
    ($name:ident, $side:ident, $number:literal) => {
        fn $name(result: $side::ReadResult) -> jig_client::Readout {
            match result {
                $side::ReadResult::Value(value) => jig_client::Readout::Family {
                    family: $number,
                    record: Some(
                        d::wire::$side::read_answer(&$side::ReadResult::Value(value)).expect("whole family read"),
                    ),
                },
                $side::ReadResult::Unknown => jig_client::Readout::Family { family: $number, record: None },
                $side::ReadResult::Busy => jig_client::Readout::Limit,
                $side::ReadResult::SignedOut => jig_client::Readout::Jig(jig_client::contract::ReadResult::None(
                    jig_client::contract::ReadNone::SignedOut,
                )),
                $side::ReadResult::NotAllowed => jig_client::Readout::Family {
                    family: $number,
                    record: Some(
                        d::wire::$side::read_answer(&$side::ReadResult::NotAllowed).expect("whole read refusal"),
                    ),
                },
            }
        }
    };
}
use d::infrastructure;
use d::observability;
read!(infra_read, infrastructure, 2);
read!(obs_read, observability, 1);
pub fn client_limits() -> jig_client::Limits {
    const OWNERS: [jig_client::Spoken; 2] = [
        jig_client::Spoken {
            family: jig_ops_domain_connectors::number(jig_ops_domain_connectors::Connector::Observability),
            version: jig_ops_client_protocol_wire::VERSION,
        },
        jig_client::Spoken {
            family: jig_ops_domain_connectors::number(jig_ops_domain_connectors::Connector::Infrastructure),
            version: jig_ops_client_protocol_wire::VERSION,
        },
    ];

    jig_client::Limits {
        domain: jig_client::DomainLimits {
            owners: &OWNERS,
            requests: skein_lib::RequestLimits {
                requests: 4,
                bytes: 262_144,
                out: 16,
                first: skein_lib::Duration::from_nanos(10),
                most: skein_lib::Duration::from_nanos(80),
                margin: skein_lib::Duration::from_secs(1),
            },
            ask_bytes: 65536,
            handles: 128,
            watches: 64,
            reads: 64,
            turn_bytes: 65536,
            document_bytes: 65536,
            backoff: jig_client::Backoff {
                first: skein_lib::Duration::from_nanos(10),
                most: skein_lib::Duration::from_nanos(80),
            },
            rows: 2,
            segments: 4,
            text: 32,
            turn_text: 32,
            projects: 2,
            fields: 4,
            delegates: 2,
            records: 2,
            kept: 4,
            parts: 10,
            authority: 8,
            accounts: jig_client::AccountCounts { accounts: 8, sources: 8, sign_ins: 8 },
            heartbeat: skein_lib::Duration::from_millis(100),
        },
        exchanges: 8,
    }
}

pub fn offline(root: &Domain) -> d::Event {
    up(Up::WatchEnded { watch: root.me.expect("me open"), end: jig_client::contract::WatchEnd::Dropped })
}
pub fn reconnect(root: &mut Domain, env: &Env<d::Limits>, out: &mut Queue<Request>) {
    let now = env.now.saturating_add(env.limits.client.domain.backoff.most);
    let env = Env { now, wall: env.wall, limits: env.limits };
    take(root, &env, out);
    let watch = root.me.expect("me reopened");
    let frame = root.frame.clone().expect("confirmed frame");
    step(
        root,
        &env,
        up(Up::Watched {
            watch,
            event: jig_client::contract::WatchEvent::Delivered(jig_client::contract::Delivery::Me(frame)),
        }),
        out,
    );
}
