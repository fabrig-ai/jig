//! The first side's values on its owner's versioned bytes (root.md, 7).
use super::wire;
use crate::{Domain, Limits, Request, first, route};
use alloc::boxed::Box;
use jig_client::{Asker, Family, Handle, Key, Outcome, Progress, Readout, Told, Why};
use jig_client_test_protocol_wire::codec::family as codec;
use skein_lib::{Env, Queue, RequestProgress, Token};
fn owned(bytes: Box<[u8]>) -> Family {
    Family { family: crate::FIRST, version: jig_client_test_protocol_wire::VERSION, bytes }
}
fn handle(token: Token) -> Handle {
    Handle { asker: Asker(u32::from(crate::FIRST)), number: token.raw() }
}
pub(crate) fn send(root: &mut Domain, env: &Env<Limits>, token: Token, ask: &first::Ask, out: &mut Queue<Request>) {
    let (project, value) = wire::request(ask);
    route::client(
        root,
        env,
        jig_client::Event::Send {
            name: None,
            key: Key { asker: Asker(u32::from(crate::FIRST)), number: token.raw() ^ root.salt },
            request: jig_client::Ask::Family {
                scope: jig_client::contract::Scope::Project(project),
                record: owned(codec::request_record(project, value)),
            },
        },
        out,
    );
}
pub(crate) fn asked(root: &mut Domain, env: &Env<Limits>, request: first::Out, out: &mut Queue<Request>) {
    let event = match request {
        first::Out::Open { watch, subject } => jig_client::Event::Watch {
            handle: handle(watch),
            subject: jig_client::Watchable::Family {
                scope: match subject.scope {
                    super::Scope::Project(project) => jig_client::contract::Scope::Project(project),
                    super::Scope::Deployment => jig_client::contract::Scope::Deployment,
                },
                window: jig_client::contract::Window { rows: env.limits.first.rows, after: None },
                record: owned(codec::subject(wire::subject(subject).1)),
            },
        },
        // Slab tokens carry a 32-bit slot and a 32-bit generation.
        // Read slots follow watch slots, preserving the complete generation.
        first::Out::Read { read, subject } => jig_client::Event::Read {
            handle: Handle {
                asker: Asker(u32::from(crate::FIRST)),
                number: read
                    .raw()
                    .checked_add(u64::from(env.limits.first.watches) << 32)
                    .expect("checked read slots follow watch slots"),
            },
            read: jig_client::Readable::Family {
                scope: match subject.scope {
                    super::Scope::Project(project) => jig_client::contract::Scope::Project(project),
                    super::Scope::Deployment => jig_client::contract::Scope::Deployment,
                },
                window: jig_client::contract::Window { rows: env.limits.first.rows, after: None },
                record: owned(codec::read(wire::subject(subject).1)),
            },
        },
        first::Out::Close { watch } => jig_client::Event::Unwatch { handle: handle(watch) },
        value @ first::Out::Fact { .. } => {
            out.push(Request::First(value));
            return;
        }
    };
    route::client(root, env, event, out);
}
fn codec_limits(env: &Env<Limits>) -> jig_test_connector::Limits {
    let bounds = env.limits.first.connector;
    jig_test_connector::Limits {
        resources: bounds.paths,
        resources_per_task: bounds.paths,
        resources_per_effect: bounds.paths,
        entries: bounds.rows,
        path_segments: bounds.segments,
        segment_bytes: bounds.segment_bytes,
        // The codec reads only these receiving bounds; there is no live engine.
        tasks: 0,
        adoptions: 0,
        subscriptions: 0,
        pools: 0,
        topics: 0,
        kinds: 0,
        requirements: 0,
        procedures: 0,
        actions_per_procedure: 0,
        facts: 0,
        judges: 0,
        values: 0,
        value_bytes: 0,
        staged: 0,
        made: 0,
        max_attempts: 0,
        write_lifetime: skein_lib::Duration::ZERO,
        clock_margin: skein_lib::Duration::ZERO,
        subscribers_per_topic: 0,
        lost_per_pool: 0,
    }
}
fn document(
    bytes: &[u8],
    subject: Option<super::Subject>,
    read: bool,
    env: &Env<Limits>,
) -> Result<super::Document, wire::Invalid> {
    let bounds = codec_limits(env);
    let mut limits = env.limits.first.connector;
    limits.rows = limits.rows.max(env.limits.first.rows);
    if let Ok(value) = wire::decode_document(bytes, &bounds, &limits) {
        return Ok(value);
    }
    let subject = subject.ok_or(wire::Invalid::Identity)?;
    let value = if read {
        match jig_client_test_protocol_wire::codec::read_answer_value(bytes, &bounds) {
            Ok(Ok(value)) => value,
            Ok(Err(_)) => return Err(wire::Invalid::Identity),
            Err(_) => return Err(wire::Invalid::TooLarge),
        }
    } else {
        match jig_client_test_protocol_wire::codec::document_value(bytes, &bounds) {
            Ok(value) => value,
            Err(_) => return Err(wire::Invalid::TooLarge),
        }
    };
    match wire::document(subject, value, &env.limits.first.connector) {
        Ok(value) => Ok(super::Document::Connector(value)),
        Err(why) => Err(why),
    }
}
fn answer(bytes: &[u8], env: &Env<Limits>) -> Result<first::Answer, wire::Invalid> {
    let bounds = codec_limits(env);
    if let Ok(value) = wire::decode_answer(bytes, &bounds) {
        return Ok(value);
    }
    match jig_client_test_protocol_wire::codec::outcome_value(bytes, &bounds) {
        Ok(value) => Ok(first::Answer::Connector(wire::answer(value))),
        Err(_) => Err(wire::Invalid::TooLarge),
    }
}
fn read_token(handle: Handle, watches: u32) -> Token {
    Token::new(handle.number.checked_sub(u64::from(watches) << 32).expect("family read slot"))
}
fn read_page(bytes: &[u8], subject: Option<super::Subject>, env: &Env<Limits>) -> first::ReadResult {
    match document(bytes, subject, true, env) {
        Ok(value) => first::ReadResult::Value(value),
        Err(_) => match jig_client_test_protocol_wire::codec::read_answer_value(bytes, &codec_limits(env)) {
            Ok(Err(jig_test_connector::DocumentMissing::NotAllowed)) => first::ReadResult::NotAllowed,
            Ok(Ok(_) | Err(jig_test_connector::DocumentMissing::Unknown)) | Err(_) => first::ReadResult::Unknown,
        },
    }
}

pub(crate) fn told(root: &mut Domain, env: &Env<Limits>, told: Told, out: &mut Queue<Request>) {
    let event = match told {
        Told::Pending { key } => {
            let Some(pending) = root.client.pending(key) else { return };
            let progress = match pending.progress {
                Progress::InFlight => RequestProgress::InFlight,
                Progress::Retrying { .. } => RequestProgress::Retrying,
                Progress::NotServed | Progress::Parked => RequestProgress::Parked,
                Progress::Unknown => RequestProgress::Unknown,
            };
            first::Event::Progress { request: Token::new(key.number ^ root.salt), progress }
        }
        Told::Answered { key, outcome } => match outcome {
            Outcome::Answered(jig_client::Answered::Family(record)) => {
                let Ok(answer) = answer(&record.bytes, env) else { return };
                first::Event::Answered { request: Token::new(key.number ^ root.salt), answer }
            }
            Outcome::Answered(jig_client::Answered::Jig(jig_client::contract::Answer::Refused {
                refusal: why,
                ..
            })) => first::Event::Answered {
                request: Token::new(key.number ^ root.salt),
                answer: first::Answer::Refused(
                    jig_client::contract::EntranceRefusal::new(why).expect("entrance refusal"),
                ),
            },
            Outcome::Answered(jig_client::Answered::Jig(_))
            | Outcome::NotKept
            | Outcome::Limit
            | Outcome::Held
            | Outcome::Full { .. }
            | Outcome::Unsent
            | Outcome::Lost
            | Outcome::Unconfirmed
            | Outcome::Unknown => first::Event::Progress {
                request: Token::new(key.number ^ root.salt),
                progress: RequestProgress::Unknown,
            },
        },
        Told::Changed { handle } => {
            let Some(held) = root.client.document(handle) else { return };
            let Some(jig_client::Latest::Family(record)) = held.latest else { return };
            let Ok(value) = document(&record.bytes, root.first.watch_subject(Token::new(handle.number)), false, env)
            else {
                out.push(Request::First(first::Out::Fact { fact: first::Fact::InvalidDocument }));
                return;
            };
            first::Event::Watch { watch: Token::new(handle.number), event: first::WatchEvent::Delivered(value) }
        }
        Told::Ended { handle, why } => first::Event::Ended {
            watch: Token::new(handle.number),
            end: match why {
                Why::Unknown | Why::TooLinked { .. } => first::WatchEnd::Unknown,
                Why::NotAllowed => first::WatchEnd::NotAllowed,
                Why::SubjectEnded => first::WatchEnd::SubjectEnded,
                Why::SignedOut => first::WatchEnd::SignedOut,
                Why::NotServed => first::WatchEnd::Unsupported,
                Why::Limit => first::WatchEnd::Closed,
            },
        },
        Told::Page { handle, page } => {
            let result = match page {
                Readout::Family { record: Some(bytes), .. } => {
                    read_page(&bytes, root.first.read_subject(read_token(handle, env.limits.first.watches)), env)
                }
                Readout::Family { record: None, .. } => first::ReadResult::Unknown,
                Readout::Limit => first::ReadResult::Busy,
                Readout::Jig(jig_client::contract::ReadResult::None(why)) => match why {
                    jig_client::contract::ReadNone::Refused => first::ReadResult::NotAllowed,
                    jig_client::contract::ReadNone::SignedOut => first::ReadResult::SignedOut,
                    jig_client::contract::ReadNone::Busy | jig_client::contract::ReadNone::Unreachable => {
                        first::ReadResult::Busy
                    }
                    jig_client::contract::ReadNone::Unknown
                    | jig_client::contract::ReadNone::Gone
                    | jig_client::contract::ReadNone::TooLinked { .. }
                    | jig_client::contract::ReadNone::TooLarge { .. }
                    | jig_client::contract::ReadNone::Malformed => first::ReadResult::Unknown,
                },
                Readout::NotServed { .. } | Readout::Jig(jig_client::contract::ReadResult::Value(_)) => {
                    first::ReadResult::Unsupported
                }
            };
            first::Event::Read { read: read_token(handle, env.limits.first.watches), result }
        }
        Told::Link { .. } | Told::Kept { .. } | Told::Dropped { .. } | Told::Text { .. } => return,
    };
    route::first(root, env, event, out);
}
