//! The selected source grant is smith's replay identity (accounts.md, 4.3).
use skein_lib::{Duration, Env, Queue, Time, Token, Wall};
use smith_domain::{
    session::{self, llm, record},
    tools,
};

fn opening() -> record::Opening {
    let affinity = llm::Affinity { key: [7; 16], thread: 1 };
    let transcript = record::Transcript {
        version: record::VERSION,
        endpoint: Some(llm::Endpoint(0)),
        dialect: 1,
        turns: Box::new([record::Turn {
            version: record::VERSION,
            kind: record::Kind::First(Box::new(record::Opener {
                affinity,
                account: Some(11),
                backend: record::Backend::Direct,
            })),
            endpoint: Some(llm::Endpoint(0)),
            dialect: 1,
            model: b"fake".as_slice().into(),
            effort: None,
            sequence: 1,
            usage: llm::Usage::ZERO,
            spent: 0,
            jobs: 0,
            messages: Box::new([
                llm::Message {
                    role: llm::Role::User,
                    content: Box::new([llm::Block::Text { text: b"original request".as_slice().into(), replay: None }]),
                },
                llm::Message {
                    role: llm::Role::Assistant,
                    content: Box::new([
                        llm::Block::Opaque { bytes: b"old source opaque data".as_slice().into() },
                        llm::Block::Text {
                            text: b"committed plain answer".as_slice().into(),
                            replay: Some(llm::Replay { bytes: b"old source replay".as_slice().into() }),
                        },
                    ]),
                },
            ]),
        }]),
    };
    record::Opening {
        window_llm: None,
        spec: session::Spec {
            backend: record::Backend::Direct,
            // Both grants may belong to one billing account; smith sees source identities.
            account: Some(13),
            final_turn: None,
            endpoint: llm::Endpoint(0),
            affinity,
            protocol_allowance: 0,
            progress: session::Progress { head: Duration::from_secs(30), idle: Duration::from_secs(30) },
            model: b"fake".as_slice().into(),
            effort: None,
            system: b"instructions".as_slice().into(),
            authority: tools::Authority {
                cwd: Box::new([]),
                repos: Box::new([]),
                grants: tools::Grants { inspect: false, modify: false, shell: false },
            },
            delegated: Box::new([]),
            prompt: b"continue".as_slice().into(),
            window: 4096,
            line: 3968,
            summary: b"Summarise the work so far.".as_slice().into(),
            preamble: b"Earlier summary:".as_slice().into(),
            output: 128,
            budget: session::Budget {
                turns: 4,
                input: 100_000,
                output: 100_000,
                cache_read: 100_000,
                cache_write: 100_000,
                time: Duration::from_secs(60),
            },
        },
        dialect: 1,
        prices: record::Prices { input: 1, cached: 1, cache_write: None, output: 1, unit: 1_000_000 },
        budget: 100,
        answered: Box::new([]),
        transcript: Some(transcript),
    }
}

#[test]
fn a_source_grant_change_carries_plain_history_and_discards_replay() {
    let limits = smith_agent_process_world::limits().domain.session;
    let mut domain = session::Domain::new(&limits, 181);
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let mut out = Queue::with_capacity(session::max_out(&limits));
    session::step(
        &mut domain,
        &env,
        session::Event::Open { opener: Token::new(1), opening: Box::new(opening()) },
        &mut out,
    );
    let mut prompt = None;
    let mut opened = None;
    while let Some(request) = out.pop() {
        match request {
            session::Request::Opened { session, .. } => opened = Some(session),
            session::Request::Complete { prompt: found, .. } => prompt = Some(found),
            other @ (session::Request::JobsCancelled { .. }
            | session::Request::Turn { .. }
            | session::Request::Priced { .. }
            | session::Request::Yield { .. }
            | session::Request::Yielded { .. }
            | session::Request::Used { .. }
            | session::Request::Ended { .. }
            | session::Request::Cancel { .. }
            | session::Request::Io { .. }
            | session::Request::CancelIo { .. }
            | session::Request::Job { .. }
            | session::Request::Delegate { .. }
            | session::Request::Withdraw { .. }) => panic!("unexpected opening output: {other:?}"),
        }
    }
    let prompt = prompt.expect("source change starts a carry-over compaction");
    assert!(matches!(prompt.purpose, llm::Purpose::Compaction { .. }));
    assert_eq!(prompt.choice, llm::ToolChoice::None);
    let mut plain = Vec::new();
    for message in &prompt.messages {
        for block in &message.content {
            match block {
                llm::Block::Text { text, replay } => {
                    assert!(replay.is_none());
                    plain.push(text.as_ref());
                }
                llm::Block::Opaque { .. } => panic!("old source opaque data crossed the source change"),
                other @ (llm::Block::Refusal { .. } | llm::Block::ToolCall { .. } | llm::Block::ToolResult { .. }) => {
                    panic!("unexpected carried block: {other:?}")
                }
            }
        }
    }
    assert!(plain.contains(&b"committed plain answer".as_slice()));
    session::step(&mut domain, &env, session::Event::Close { session: opened.expect("opened session") }, &mut out);
    let Some(session::Request::Cancel { owner }) = out.pop() else { panic!("cancel carry-over completion") };
    session::step(&mut domain, &env, session::Event::Cancelled { owner }, &mut out);
    domain.reclaim();
    assert_eq!(domain.sessions(), 0);
}
