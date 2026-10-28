//! Connector sides route only their asker's client handles and keys (root.md, 7).
use crate::{Domain, Limits, Request, route, wire};
use alloc::boxed::Box;
use jig_client::{Asker, Family, Handle, Key, Outcome, Progress, Told};
use jig_ops_domain_connectors::{Connector, number};
use skein_lib::{Env, Queue, RequestProgress, Token};
fn asker(connector: Connector) -> Asker {
    jig_client_view::Owner::Application { number: number(connector) }.asker()
}
fn owned(connector: Connector, bytes: Box<[u8]>) -> Family {
    Family { family: number(connector), version: jig_ops_client_protocol_wire::VERSION, bytes }
}
fn progress(value: Progress) -> RequestProgress {
    match value {
        Progress::InFlight => RequestProgress::InFlight,
        Progress::Retrying { .. } => RequestProgress::Retrying,
        Progress::Parked | Progress::NotServed => RequestProgress::Parked,
        Progress::Unknown => RequestProgress::Unknown,
    }
}
pub(crate) fn send(
    root: &mut Domain,
    env: &Env<Limits>,
    token: Token,
    ask: &crate::infrastructure::Ask,
    out: &mut Queue<Request>,
) {
    let bytes = wire::ask(ask).expect("admitted infrastructure ask");
    route::client(
        root,
        env,
        jig_client::Event::Send {
            name: None,
            key: Key { asker: asker(Connector::Infrastructure), number: token.raw() ^ root.salt },
            request: jig_client::Ask::Family {
                scope: jig_client::contract::Scope::Project(match ask {
                    crate::infrastructure::Ask::MoveEnd { project, .. }
                    | crate::infrastructure::Ask::NativeMoveEnd { project, .. } => *project,
                }),
                record: owned(Connector::Infrastructure, bytes),
            },
        },
        out,
    );
}
fn answered(root: &mut Domain, env: &Env<Limits>, key: Key, outcome: Outcome, out: &mut Queue<Request>) {
    use crate::infrastructure as d;
    let request = Token::new(key.number ^ root.salt);
    let event = match outcome {
        Outcome::Answered(jig_client::Answered::Family(record)) => {
            let Ok(answer) = wire::answer_value(&record.bytes) else { return };
            d::Event::Answered { request, answer }
        }
        Outcome::Answered(jig_client::Answered::Jig(jig_client::contract::Answer::Refused {
            refusal: why, ..
        })) => d::Event::Answered {
            request,
            answer: d::Answer::EntranceRefused(
                jig_client::contract::EntranceRefusal::new(why).expect("entrance refusal"),
            ),
        },
        Outcome::Answered(jig_client::Answered::Jig(_))
        | Outcome::NotKept
        | Outcome::Limit
        | Outcome::Held
        | Outcome::Unsent
        | Outcome::Full { .. }
        | Outcome::Lost
        | Outcome::Unconfirmed
        | Outcome::Unknown => d::Event::Progress { request, progress: RequestProgress::Unknown },
    };
    route::infrastructure(root, env, event, out);
}
pub(crate) mod infrastructure {
    use super::{
        Box, Connector, Domain, Env, Handle, Limits, Queue, Request, Token, Told, answered, asker, owned, pending,
        route, wire,
    };
    use crate::infrastructure as d;
    fn handle(token: Token) -> Handle {
        Handle { asker: asker(Connector::Infrastructure), number: token.raw() }
    }
    pub(crate) fn asked(root: &mut Domain, env: &Env<Limits>, value: d::Out, out: &mut Queue<Request>) {
        let closing = match &value {
            d::Out::Close { watch } => Some(*watch),
            d::Out::Read { .. } | d::Out::Open { .. } | d::Out::Fact { .. } => None,
        };
        let event = match value {
            d::Out::Open { watch, subject } => jig_client::Event::Watch {
                handle: handle(watch),
                subject: jig_client::Watchable::Family {
                    scope: match subject.scope {
                        d::Scope::Project(project) => jig_client::contract::Scope::Project(project),
                        d::Scope::Deployment => jig_client::contract::Scope::Deployment,
                    },
                    window: jig_client::contract::Window { rows: env.limits.infrastructure.rows, after: None },
                    record: owned(
                        Connector::Infrastructure,
                        wire::infrastructure::subject(&subject).expect("admitted subject"),
                    ),
                },
            },
            d::Out::Read { read, subject } => jig_client::Event::Read {
                handle: Handle {
                    asker: asker(Connector::Infrastructure),
                    number: read
                        .raw()
                        .checked_add(u64::from(env.limits.infrastructure.watches) << 32)
                        .expect("read slots follow watch slots"),
                },
                read: jig_client::Readable::Family {
                    scope: match subject.scope {
                        d::Scope::Project(project) => jig_client::contract::Scope::Project(project),
                        d::Scope::Deployment => jig_client::contract::Scope::Deployment,
                    },
                    window: jig_client::contract::Window { rows: env.limits.infrastructure.rows, after: None },
                    record: owned(
                        Connector::Infrastructure,
                        wire::infrastructure::subject(&subject).expect("admitted subject"),
                    ),
                },
            },
            d::Out::Close { watch } => jig_client::Event::Unwatch { handle: handle(watch) },
            value @ d::Out::Fact { .. } => {
                out.push(Request::Infrastructure(value));
                return;
            }
        };
        route::client(root, env, event, out);
        if let Some(watch) = closing {
            route::infrastructure(root, env, d::Event::Ended { watch, end: d::WatchEnd::Closed }, out);
        }
    }
    pub(crate) fn told(root: &mut Domain, env: &Env<Limits>, value: Told, out: &mut Queue<Request>) {
        let event = match value {
            Told::Changed { handle } => {
                let Some(held) = root.client.document(handle) else { return };
                let Some(jig_client::Latest::Family(record)) = held.latest else { return };
                let Ok(value) = wire::infrastructure::document_value(&record.bytes) else {
                    out.push(Request::Infrastructure(d::Out::Fact { fact: d::Fact::InvalidDocument }));
                    return;
                };
                d::Event::Watch { watch: Token::new(handle.number), event: d::WatchEvent::Delivered(Box::new(value)) }
            }
            Told::Ended { handle, why } => d::Event::Ended {
                watch: Token::new(handle.number),
                end: match why {
                    jig_client::Why::Unknown | jig_client::Why::NotServed | jig_client::Why::TooLinked { .. } => {
                        d::WatchEnd::Unknown
                    }
                    jig_client::Why::NotAllowed => d::WatchEnd::NotAllowed,
                    jig_client::Why::SubjectEnded => d::WatchEnd::SubjectEnded,
                    jig_client::Why::SignedOut => d::WatchEnd::SignedOut,
                    jig_client::Why::Limit => d::WatchEnd::Closed,
                },
            },
            Told::Page { handle, page } => {
                let result = match page {
                    jig_client::Readout::Family { record: Some(bytes), .. } => {
                        wire::infrastructure::read_answer_value(&bytes)
                    }
                    jig_client::Readout::Family { record: None, .. } | jig_client::Readout::NotServed { .. } => {
                        d::ReadResult::Unknown
                    }
                    jig_client::Readout::Limit => d::ReadResult::Busy,
                    jig_client::Readout::Jig(jig_client::contract::ReadResult::None(why)) => match why {
                        jig_client::contract::ReadNone::SignedOut => d::ReadResult::SignedOut,
                        jig_client::contract::ReadNone::Refused => d::ReadResult::NotAllowed,
                        jig_client::contract::ReadNone::Busy | jig_client::contract::ReadNone::Unreachable => {
                            d::ReadResult::Busy
                        }
                        jig_client::contract::ReadNone::Unknown
                        | jig_client::contract::ReadNone::Gone
                        | jig_client::contract::ReadNone::TooLinked { .. }
                        | jig_client::contract::ReadNone::TooLarge { .. }
                        | jig_client::contract::ReadNone::Malformed => d::ReadResult::Unknown,
                    },
                    jig_client::Readout::Jig(jig_client::contract::ReadResult::Value(_)) => d::ReadResult::Unknown,
                };
                d::Event::Read {
                    read: Token::new(
                        handle
                            .number
                            .checked_sub(u64::from(env.limits.infrastructure.watches) << 32)
                            .expect("family read slot"),
                    ),
                    result,
                }
            }
            Told::Pending { key } => {
                pending(root, env, key, out);
                return;
            }
            Told::Answered { key, outcome } => {
                answered(root, env, key, outcome, out);
                return;
            }
            Told::Kept { .. } | Told::Link { .. } | Told::Dropped { .. } | Told::Text { .. } => return,
        };
        route::infrastructure(root, env, event, out);
    }
}
pub(crate) mod observability {
    use super::{
        Box, Connector, Domain, Env, Handle, Limits, Queue, Request, Token, Told, asker, no_answer, no_pending, owned,
        route, wire,
    };
    use crate::observability as d;
    fn handle(token: Token) -> Handle {
        Handle { asker: asker(Connector::Observability), number: token.raw() }
    }
    pub(crate) fn asked(root: &mut Domain, env: &Env<Limits>, value: d::Out, out: &mut Queue<Request>) {
        let closing = match &value {
            d::Out::Close { watch } => Some(*watch),
            d::Out::Read { .. } | d::Out::Open { .. } | d::Out::Fact { .. } => None,
        };
        let event = match value {
            d::Out::Open { watch, subject } => jig_client::Event::Watch {
                handle: handle(watch),
                subject: jig_client::Watchable::Family {
                    scope: match subject.scope {
                        d::Scope::Project(project) => jig_client::contract::Scope::Project(project),
                        d::Scope::Deployment => jig_client::contract::Scope::Deployment,
                    },
                    window: jig_client::contract::Window { rows: env.limits.observability.rows, after: None },
                    record: owned(
                        Connector::Observability,
                        wire::observability::subject(&subject).expect("admitted subject"),
                    ),
                },
            },
            d::Out::Read { read, subject } => jig_client::Event::Read {
                handle: Handle {
                    asker: asker(Connector::Observability),
                    number: read
                        .raw()
                        .checked_add(u64::from(env.limits.observability.watches) << 32)
                        .expect("read slots follow watch slots"),
                },
                read: jig_client::Readable::Family {
                    scope: match subject.scope {
                        d::Scope::Project(project) => jig_client::contract::Scope::Project(project),
                        d::Scope::Deployment => jig_client::contract::Scope::Deployment,
                    },
                    window: jig_client::contract::Window { rows: env.limits.observability.rows, after: None },
                    record: owned(
                        Connector::Observability,
                        wire::observability::subject(&subject).expect("admitted subject"),
                    ),
                },
            },
            d::Out::Close { watch } => jig_client::Event::Unwatch { handle: handle(watch) },
            value @ d::Out::Fact { .. } => {
                out.push(Request::Observability(value));
                return;
            }
        };
        route::client(root, env, event, out);
        if let Some(watch) = closing {
            route::observability(root, env, d::Event::Ended { watch, end: d::WatchEnd::Closed }, out);
        }
    }
    pub(crate) fn told(root: &mut Domain, env: &Env<Limits>, value: Told, out: &mut Queue<Request>) {
        let event = match value {
            Told::Changed { handle } => {
                let Some(held) = root.client.document(handle) else { return };
                let Some(jig_client::Latest::Family(record)) = held.latest else { return };
                let Ok(value) = wire::observability::document_value(&record.bytes) else {
                    out.push(Request::Observability(d::Out::Fact { fact: d::Fact::InvalidDocument }));
                    return;
                };
                d::Event::Watch { watch: Token::new(handle.number), event: d::WatchEvent::Delivered(Box::new(value)) }
            }
            Told::Ended { handle, why } => d::Event::Ended {
                watch: Token::new(handle.number),
                end: match why {
                    jig_client::Why::Unknown | jig_client::Why::NotServed | jig_client::Why::TooLinked { .. } => {
                        d::WatchEnd::Unknown
                    }
                    jig_client::Why::NotAllowed => d::WatchEnd::NotAllowed,
                    jig_client::Why::SubjectEnded => d::WatchEnd::SubjectEnded,
                    jig_client::Why::SignedOut => d::WatchEnd::SignedOut,
                    jig_client::Why::Limit => d::WatchEnd::Closed,
                },
            },
            Told::Page { handle, page } => {
                let result = match page {
                    jig_client::Readout::Family { record: Some(bytes), .. } => {
                        wire::observability::read_answer_value(&bytes)
                    }
                    jig_client::Readout::Family { record: None, .. } | jig_client::Readout::NotServed { .. } => {
                        d::ReadResult::Unknown
                    }
                    jig_client::Readout::Limit => d::ReadResult::Busy,
                    jig_client::Readout::Jig(jig_client::contract::ReadResult::None(why)) => match why {
                        jig_client::contract::ReadNone::SignedOut => d::ReadResult::SignedOut,
                        jig_client::contract::ReadNone::Refused => d::ReadResult::NotAllowed,
                        jig_client::contract::ReadNone::Busy | jig_client::contract::ReadNone::Unreachable => {
                            d::ReadResult::Busy
                        }
                        jig_client::contract::ReadNone::Unknown
                        | jig_client::contract::ReadNone::Gone
                        | jig_client::contract::ReadNone::TooLinked { .. }
                        | jig_client::contract::ReadNone::TooLarge { .. }
                        | jig_client::contract::ReadNone::Malformed => d::ReadResult::Unknown,
                    },
                    jig_client::Readout::Jig(jig_client::contract::ReadResult::Value(_)) => d::ReadResult::Unknown,
                };
                d::Event::Read {
                    read: Token::new(
                        handle
                            .number
                            .checked_sub(u64::from(env.limits.observability.watches) << 32)
                            .expect("family read slot"),
                    ),
                    result,
                }
            }
            Told::Pending { key } => {
                no_pending(root, env, key, out);
                return;
            }
            Told::Answered { key, outcome } => {
                no_answer(root, env, key, outcome, out);
                return;
            }
            Told::Kept { .. } | Told::Link { .. } | Told::Dropped { .. } | Told::Text { .. } => return,
        };
        route::observability(root, env, event, out);
    }
}
fn pending(root: &mut Domain, env: &Env<Limits>, key: Key, out: &mut Queue<Request>) {
    let Some(held) = root.client.pending(key) else { return };
    let event = crate::infrastructure::Event::Progress {
        request: Token::new(key.number ^ root.salt),
        progress: progress(held.progress),
    };
    route::infrastructure(root, env, event, out);
}
fn no_pending(_: &mut Domain, _: &Env<Limits>, _: Key, _: &mut Queue<Request>) {}
fn no_answer(_: &mut Domain, _: &Env<Limits>, _: Key, _: Outcome, _: &mut Queue<Request>) {}
