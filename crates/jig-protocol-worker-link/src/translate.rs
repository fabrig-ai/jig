//! The hub's records across the channel (protocol/channel.md, 5 and 6).
//! Pieces cross whole; only the root judges their kinds and offsets (6.4).
use crate::{GrantValue, Request, Scheme};
use alloc::boxed::Box;
use jig_domain_host as host;
use jig_protocol_worker_wire::{self as wire, Part};
use skein_channel::{Frame, frame_writer};
use skein_lib::{List, Queue, Reader, Time, Token, Writer, bytes};

pub(crate) fn hello(hello: host::Hello, limits: &wire::v9::Limits) -> Option<Frame> {
    let bounds = hello.bounds;
    if !host::valid_program_endpoints(&bounds.program_endpoints) {
        return None;
    }
    let mut program_endpoints = List::with_capacity(limits.bounds_program_endpoints);
    for endpoint in bounds.program_endpoints {
        program_endpoints.push(endpoint).ok()?;
    }
    let mut models = List::with_capacity(limits.bounds_models);
    for model in bounds.models {
        let model = wire::ModelBound::new(
            limits,
            wire::ModelBoundParts { name: model.name, window: model.window, output: model.output },
        )
        .ok()?;
        models.push(model).ok()?;
    }
    let bounds = wire::Bounds::new(
        limits,
        wire::BoundsParts {
            program_endpoints,
            models,
            turns: bounds.turns,
            spend: bounds.spend,
            time: bounds.time,
            waiting: bounds.waiting,
            checks: bounds.checks,
            message_bytes: bounds.message_bytes,
            inbox: bounds.inbox,
            unacknowledged: bounds.unacknowledged,
        },
    )
    .ok()?;
    let mut workstreams = List::with_capacity(limits.hello_workstreams);
    for stream in hello.workstreams {
        workstreams.push(stream).ok()?;
    }
    let mut hosting = List::with_capacity(limits.hello_hosting);
    for run in hello.hosting {
        let phase = phase(run.phase);
        let run =
            wire::Hosting::new(limits, wire::HostingParts { task: run.run.raw(), attempt: run.attempt.raw(), phase })
                .ok()?;
        hosting.push(run).ok()?;
    }
    let record = wire::Hello::new(
        limits,
        wire::HelloParts { slots: hello.slots, stop_bound: hello.stop_bound, bounds, workstreams, hosting },
    )
    .ok()?;
    let mut body = Writer::new(usize::try_from(record.measure()).ok()?);
    record.encode(&mut body).ok()?;
    frame(0x0100, body.finish())
}

pub(crate) fn up(up: host::LinkUp, parts: &[Part], limits: &wire::v9::Limits) -> Option<Frame> {
    match up {
        host::LinkUp::Piece { run, attempt, piece } => {
            let value = wire::FactValue::Piece(piece_record(piece, limits)?);
            fact_frame(run, attempt, value, limits)
        }
        host::LinkUp::Logins { logins } => logins_frame(logins, limits),
        host::LinkUp::SignedOut { name, result } => signed_out_frame(name, result, limits),
        host::LinkUp::Activity { run, attempt, activity: value } => {
            fact_frame(run, attempt, activity(value, limits)?, limits)
        }
        host::LinkUp::Unauthorized { run, attempt, account, generation } => {
            let record = wire::Unauthorized::new(
                limits,
                wire::UnauthorizedParts { task: run.raw(), attempt: attempt.raw(), account, generation },
            )
            .ok()?;
            let mut body = Writer::new(usize::try_from(record.measure()).ok()?);
            record.encode(&mut body).ok()?;
            frame(0x0105, body.finish())
        }
        host::LinkUp::Limited { run, attempt, account, reset } => {
            let record = wire::Limited::new(
                limits,
                wire::LimitedParts { task: run.raw(), attempt: attempt.raw(), account, reset },
            )
            .ok()?;
            let mut body = Writer::new(usize::try_from(record.measure()).ok()?);
            record.encode(&mut body).ok()?;
            frame(0x0106, body.finish())
        }
        host::LinkUp::AccountRefused { run, attempt, account, code, reason } => {
            let record = wire::AccountRefused::new(
                limits,
                wire::AccountRefusedParts { task: run.raw(), attempt: attempt.raw(), account, code, reason },
            )
            .ok()?;
            let mut body = Writer::new(usize::try_from(record.measure()).ok()?);
            record.encode(&mut body).ok()?;
            frame(0x0107, body.finish())
        }
        host::LinkUp::Turn { run, attempt, turn } => {
            let record = wire::Turn::new(
                limits,
                wire::TurnParts {
                    task: run.raw(),
                    attempt: attempt.raw(),
                    number: turn.turn,
                    spent: turn.spent,
                    read: name(turn.read),
                    opens_window: turn.opens_window,
                    refused: refused(turn.refused, limits)?,
                    body: turn.body,
                },
            )
            .ok()?;
            let mut body = Writer::new(usize::try_from(record.measure()).ok()?);
            record.encode(&mut body).ok()?;
            frame(0x0101, body.finish())
        }
        host::LinkUp::Call { run, attempt, call, tool, writes, input, deadline } => {
            let record = wire::Call::new(
                limits,
                wire::CallParts { task: run.raw(), attempt: attempt.raw(), name: call, tool, writes, input, deadline },
            )
            .ok()?;
            let mut body = Writer::new(usize::try_from(record.measure()).ok()?);
            record.encode(&mut body).ok()?;
            frame(0x0102, body.finish())
        }
        host::LinkUp::Answer { run, attempt, answer } => {
            let record = wire::Answer::new(
                limits,
                wire::AnswerParts {
                    task: run.raw(),
                    attempt: attempt.raw(),
                    turns: answer.turns,
                    spent: answer.spent,
                    read: name(answer.read),
                    refused: refused(answer.refused, limits)?,
                    ending: ending(answer.ending, parts, limits)?,
                },
            )
            .ok()?;
            let mut body = Writer::new(usize::try_from(record.measure()).ok()?);
            record.encode(&mut body).ok()?;
            frame(0x0103, body.finish())
        }
    }
}
fn piece_record(piece: host::Piece, limits: &wire::v9::Limits) -> Option<wire::Piece> {
    let kind = match piece.kind {
        host::LiveKind::Text => wire::LiveKind::Text,
        host::LiveKind::Reasoning => wire::LiveKind::Reasoning,
        host::LiveKind::Input { call, tool } => {
            wire::LiveKind::Input(wire::InputCall::new(limits, wire::InputCallParts { id: call, tool }).ok()?)
        }
    };
    wire::Piece::new(
        limits,
        wire::PieceParts { turn: piece.turn, retry: piece.retry, kind, offset: piece.offset, bytes: piece.bytes },
    )
    .ok()
}
fn fact_frame(run: Token, attempt: Token, value: wire::FactValue, limits: &wire::v9::Limits) -> Option<Frame> {
    let record = wire::Fact::new(limits, wire::FactParts { task: run.raw(), attempt: attempt.raw(), value }).ok()?;
    let mut body = Writer::new(usize::try_from(record.measure()).ok()?);
    record.encode(&mut body).ok()?;
    frame(wire::Kind::Fact.number(), body.finish())
}
fn frame(kind: u16, body: Box<[u8]>) -> Option<Frame> {
    let mut frame = frame_writer(kind, u32::try_from(body.len()).ok()?).ok()?;
    frame.put(&body).ok()?;
    frame.finish().ok()
}
fn part(token: Option<Token>, parts: &[Part]) -> Result<Option<Box<[u8]>>, ()> {
    let Some(token) = token else { return Ok(None) };
    for part in parts {
        if part.token == token {
            return Ok(Some(bytes::copy_of(&part.bytes)));
        }
    }
    Err(())
}
fn name(token: Option<Token>) -> Option<u64> {
    Some(token?.raw())
}
fn work(work: host::Work, parts: &[Part], limits: &wire::v9::Limits) -> Option<wire::Work> {
    wire::Work::new(
        limits,
        wire::WorkParts { left: part(work.left, parts).ok()?, saved: part(work.saved, parts).ok()? },
    )
    .ok()
}
fn refused(items: Box<[host::Refused]>, limits: &wire::v9::Limits) -> Option<List<wire::Refused>> {
    let mut list = List::with_capacity(limits.turn_refused);
    for item in items {
        let reason = match item.reason {
            host::MessageRefusal::TooLarge => wire::MessageRefusal::TooLarge,
            host::MessageRefusal::Full => wire::MessageRefusal::Full,
            host::MessageRefusal::NameInUse => wire::MessageRefusal::NameInUse,
            host::MessageRefusal::Ending => wire::MessageRefusal::Ending,
        };
        let item = wire::Refused::new(limits, wire::RefusedParts { name: item.name.raw(), reason }).ok()?;
        list.push(item).ok()?;
    }
    Some(list)
}
fn ending(ending: host::Ending, parts: &[Part], limits: &wire::v9::Limits) -> Option<wire::Ending> {
    Some(match ending {
        host::Ending::Refused(refusal) => {
            let value = match refusal {
                host::Refusal::Busy => wire::Refusal::Busy,
                host::Refusal::Invalid(invalid) => {
                    let value = invalid_value(invalid, limits)?;
                    let value = wire::InvalidValue::new(limits, wire::InvalidValueParts { value }).ok()?;
                    wire::Refusal::Invalid(value)
                }
            };
            let value = wire::RefusalValue::new(limits, wire::RefusalValueParts { value }).ok()?;
            wire::Ending::Refused(value)
        }
        host::Ending::Ended { outcome, work: kept } => {
            let work = work(kept, parts, limits)?;
            wire::Ending::Ended(wire::Ended::new(limits, wire::EndedParts { outcome, work }).ok()?)
        }
        host::Ending::Parked { work: kept } => {
            let work = work(kept, parts, limits)?;
            wire::Ending::Parked(wire::Parked::new(limits, wire::ParkedParts { work }).ok()?)
        }
        host::Ending::Failed { failure: failed, detail, work: kept } => {
            let work = work(kept, parts, limits)?;
            let failure = failure(failed, parts, limits)?;
            wire::Ending::Failed(wire::Failed::new(limits, wire::FailedParts { failure, detail, work }).ok()?)
        }
    })
}
fn failure(failure: host::Failure, parts: &[Part], limits: &wire::v9::Limits) -> Option<wire::Failure> {
    Some(match failure {
        host::Failure::Unprepared(preparation) => {
            let value = match preparation {
                host::Preparation::Transient => wire::Preparation::Transient,
                host::Preparation::Permanent { resource } => {
                    let resource = part(resource, parts).ok()?;
                    wire::Preparation::Permanent(wire::Resource::new(limits, wire::ResourceParts { resource }).ok()?)
                }
            };
            wire::Failure::Unprepared(wire::PreparationValue::new(limits, wire::PreparationValueParts { value }).ok()?)
        }
        host::Failure::Run(failure) => {
            let value = run_failure(failure, limits)?;
            wire::Failure::Run(wire::RunFailureValue::new(limits, wire::RunFailureValueParts { value }).ok()?)
        }
        host::Failure::Agent(failure) => {
            let value = agent_failure(failure);
            wire::Failure::Agent(wire::AgentFailureValue::new(limits, wire::AgentFailureValueParts { value }).ok()?)
        }
        host::Failure::Cancelled(reason) => {
            let value = cancel_reason(reason);
            wire::Failure::Cancelled(wire::CancelReasonValue::new(limits, wire::CancelReasonValueParts { value }).ok()?)
        }
    })
}
fn activity(activity: host::Activity, limits: &wire::v9::Limits) -> Option<wire::FactValue> {
    Some(match activity {
        host::Activity::Phase(value) => {
            let value = phase(value);
            wire::FactValue::Phase(wire::PhaseValue::new(limits, wire::PhaseValueParts { value }).ok()?)
        }
        host::Activity::Long { operation: op, span } => {
            let operation = operation(op);
            wire::FactValue::Long(wire::Long::new(limits, wire::LongParts { operation, span }).ok()?)
        }
        host::Activity::LongDone => wire::FactValue::LongDone,
        host::Activity::Count { kind, elapsed, count } => {
            let kind = count_kind(kind);
            wire::FactValue::Count(wire::Count::new(limits, wire::CountParts { kind, elapsed, value: count }).ok()?)
        }
    })
}
fn phase(value: host::Phase) -> wire::Phase {
    match value {
        host::Phase::Answered => wire::Phase::Answered,
        host::Phase::Preparing => wire::Phase::Preparing,
        host::Phase::Starting => wire::Phase::Starting,
        host::Phase::Active => wire::Phase::Active,
        host::Phase::Waiting => wire::Phase::Waiting,
        host::Phase::Ending => wire::Phase::Ending,
    }
}
fn invalid_value(value: host::Invalid, limits: &wire::v9::Limits) -> Option<wire::Invalid> {
    Some(match value {
        host::Invalid::Start { why, cause } => wire::Invalid::Start(
            wire::InvalidStart::new(
                limits,
                wire::InvalidStartParts {
                    why,
                    cause: match cause {
                        host::StartCause::Other => wire::StartCause::Other,
                        host::StartCause::Grants => wire::StartCause::Grants,
                        host::StartCause::HostPart { bound } => {
                            wire::StartCause::HostPart(wire::HostPart::new(limits, wire::HostPartParts { bound }).ok()?)
                        }
                    },
                },
            )
            .ok()?,
        ),
        host::Invalid::Charter => wire::Invalid::Charter,
        host::Invalid::Transcript => wire::Invalid::Transcript,
        host::Invalid::Grants => wire::Invalid::Grants,
        host::Invalid::Messages => wire::Invalid::Messages,
    })
}
fn run_failure(value: host::RunFailure, limits: &wire::v9::Limits) -> Option<wire::RunFailure> {
    Some(match value {
        host::RunFailure::Exceeded(value) => wire::RunFailure::Exceeded(
            wire::Exceeded::new(
                limits,
                wire::ExceededParts {
                    bound: match value.bound {
                        host::Bound::Reasoning => wire::Bound::Reasoning,
                        host::Bound::Metadata => wire::Bound::Metadata,
                        host::Bound::Items => wire::Bound::Items,
                        host::Bound::Answer => wire::Bound::Answer,
                        host::Bound::Tools => wire::Bound::Tools,
                        host::Bound::HistoryItems => wire::Bound::HistoryItems,
                        host::Bound::History => wire::Bound::History,
                        host::Bound::Window => wire::Bound::Window,
                        host::Bound::Output => wire::Bound::Output,
                        host::Bound::HostPart => wire::Bound::HostPart,
                    },
                    value: value.value,
                },
            )
            .ok()?,
        ),
        host::RunFailure::Model => wire::RunFailure::Model,
        host::RunFailure::Budget => wire::RunFailure::Budget,
        host::RunFailure::Policy => wire::RunFailure::Policy,
        host::RunFailure::Cancelled => wire::RunFailure::Cancelled,
        host::RunFailure::Stale => wire::RunFailure::Stale,
        host::RunFailure::Limited { account, reset } => wire::RunFailure::Limited(
            wire::LimitedAccount::new(limits, wire::LimitedAccountParts { account, reset }).ok()?,
        ),
        host::RunFailure::Refused { account } => {
            wire::RunFailure::Refused(wire::RefusedAccount::new(limits, wire::RefusedAccountParts { account }).ok()?)
        }
    })
}
fn agent_failure(value: host::AgentFailure) -> wire::AgentFailure {
    match value {
        host::AgentFailure::Unstarted => wire::AgentFailure::Unstarted,
        host::AgentFailure::Exited => wire::AgentFailure::Exited,
        host::AgentFailure::Rules => wire::AgentFailure::Rules,
        host::AgentFailure::NoProgress => wire::AgentFailure::NoProgress,
        host::AgentFailure::WallTime => wire::AgentFailure::WallTime,
    }
}
fn cancel_reason(value: host::Reason) -> wire::CancelReason {
    match value {
        host::Reason::Engine => wire::CancelReason::Engine,
        host::Reason::Contact => wire::CancelReason::Contact,
        host::Reason::Shutdown => wire::CancelReason::Shutdown,
    }
}
fn operation(value: host::Operation) -> wire::Operation {
    match value {
        host::Operation::Checks => wire::Operation::Checks,
        host::Operation::Completion => wire::Operation::Completion,
        host::Operation::Command => wire::Operation::Command,
    }
}
fn count_kind(value: host::CountKind) -> wire::CountKind {
    match value {
        host::CountKind::Admitted => wire::CountKind::Admitted,
        host::CountKind::Ended => wire::CountKind::Ended,
        host::CountKind::LlmStarted => wire::CountKind::LlmStarted,
        host::CountKind::LlmRetried => wire::CountKind::LlmRetried,
        host::CountKind::LlmFinished => wire::CountKind::LlmFinished,
        host::CountKind::ToolStarted => wire::CountKind::ToolStarted,
        host::CountKind::ToolFinished => wire::CountKind::ToolFinished,
        host::CountKind::CheckStarted => wire::CountKind::CheckStarted,
        host::CountKind::CheckFinished => wire::CountKind::CheckFinished,
        host::CountKind::TextArrived => wire::CountKind::TextArrived,
    }
}

pub(crate) fn down(
    kind: u16,
    body: &[u8],
    limits: &wire::v9::Limits,
    now: Time,
    parts: &mut List<Part>,
    next: &mut u64,
    out: &mut Queue<Request>,
) -> Option<host::LinkDown> {
    let mut reader = Reader::new(body);
    let mut known = None;
    for rule in wire::KINDS {
        if rule.kind == kind {
            known = Some(rule.body);
        }
    }
    let down = match known? {
        wire::Kind::Sources => {
            let record = wire::Sources::decode(limits, &mut reader).ok()?.into_parts();
            let mut names = List::with_capacity(record.sources.len());
            for source in record.sources.into_boxed() {
                let name = host::SourceName::new(source.name())?;
                names.push(name).ok()?;
            }
            if !host::sources::unique(names.as_slice()) {
                return None;
            }
            host::LinkDown::Sources { sources: names.into_boxed() }
        }
        wire::Kind::Check => host::LinkDown::Check {
            name: host::SourceName::new(wire::Check::decode(limits, &mut reader).ok()?.name())?,
        },
        wire::Kind::SignOut => host::LinkDown::SignOut {
            name: host::SourceName::new(wire::SignOut::decode(limits, &mut reader).ok()?.name())?,
        },
        wire::Kind::Assignment => {
            let record = wire::Assignment::decode(limits, &mut reader).ok()?;
            host::LinkDown::Assign { assignment: assignment(record, now, parts, next, out)? }
        }
        wire::Kind::Message => {
            let record = wire::Inbound::decode(limits, &mut reader).ok()?.into_parts();
            host::LinkDown::Message {
                run: Token::new(record.task),
                attempt: Token::new(record.attempt),
                name: Token::new(record.name),
                sender: record.sender,
                words: record.words,
                finish_requested: record.finish_requested,
            }
        }
        wire::Kind::TurnKept => {
            let record = wire::TurnKept::decode(limits, &mut reader).ok()?.into_parts();
            host::LinkDown::TurnKept {
                run: Token::new(record.task),
                attempt: Token::new(record.attempt),
                turn: record.number,
            }
        }
        wire::Kind::Acknowledge => {
            let record = wire::Acknowledge::decode(limits, &mut reader).ok()?.into_parts();
            host::LinkDown::Acknowledge { run: Token::new(record.task), attempt: Token::new(record.attempt) }
        }
        wire::Kind::Cancel => {
            let record = wire::Cancel::decode(limits, &mut reader).ok()?.into_parts();
            host::LinkDown::Cancel { run: Token::new(record.task), attempt: Token::new(record.attempt) }
        }
        wire::Kind::Grant => {
            let record = wire::Granted::decode(limits, &mut reader).ok()?.into_parts();
            let grant = grant(record.grant, now, out)?;
            host::LinkDown::Grant { run: Token::new(record.task), attempt: Token::new(record.attempt), grant }
        }
        wire::Kind::Relayed => {
            let record = wire::Relayed::decode(limits, &mut reader).ok()?.into_parts();
            let answer = match record.answer {
                wire::RelayAnswer::Busy => host::Relayed::Busy,
                wire::RelayAnswer::Text(text) => {
                    let text = text.into_parts();
                    host::Relayed::Answer { error: text.error, text: text.text }
                }
            };
            host::LinkDown::Relayed {
                run: Token::new(record.task),
                attempt: Token::new(record.attempt),
                call: record.name,
                answer,
            }
        }
        // TurnBusy is decoded below the hub by the component; up kinds are invalid here.
        wire::Kind::Logins
        | wire::Kind::SignedOut
        | wire::Kind::TurnBusy
        | wire::Kind::Hello
        | wire::Kind::Turn
        | wire::Kind::Call
        | wire::Kind::Answer
        | wire::Kind::Fact
        | wire::Kind::Unauthorized
        | wire::Kind::Limited
        | wire::Kind::AccountRefused => return None,
    };
    Some(down)
}
/// A value's arm and scheme name the form in which it is lent.
fn grant_form(value: &wire::GrantValue) -> Option<host::Form> {
    Some(match value {
        wire::GrantValue::Program(value) => host::Form::Program(host::SourceName::new(value.name())?),
        wire::GrantValue::Credential(value) => host::Form::Credential(match value.scheme() {
            wire::Scheme::Bearer => host::Scheme::Bearer,
            wire::Scheme::Key => host::Scheme::Key,
        }),
        wire::GrantValue::Application(_) => host::Form::Application,
    })
}
fn grant(grant: wire::Grant, now: Time, out: &mut Queue<Request>) -> Option<host::Grant> {
    let grant = grant.into_parts();
    let form = grant_form(&grant.value)?;
    let value = match grant.value {
        wire::GrantValue::Program(_) => {
            return Some(host::Grant { form, account: grant.number, generation: grant.generation, valid: grant.valid });
        }
        wire::GrantValue::Credential(value) => {
            let value = value.into_parts();
            let scheme = match value.scheme {
                wire::Scheme::Bearer => Scheme::Bearer,
                wire::Scheme::Key => Scheme::Key,
            };
            GrantValue::Credential { scheme, token: value.bytes }
        }
        wire::GrantValue::Application(value) => GrantValue::Application { secret: value.into_parts().secret },
    };
    let expires = now.saturating_add(grant.valid);
    out.push(Request::Value { account: grant.number, generation: grant.generation, expires, value });
    Some(host::Grant { form, account: grant.number, generation: grant.generation, valid: grant.valid })
}
fn transcript(bytes: &[u8]) -> Box<[Box<[u8]>]> {
    let mut reader = Reader::new(bytes);
    let mut turns = List::with_capacity(u32::try_from(bytes.len().saturating_div(4)).expect("bounded transcript"));
    for _ in 0..bytes.len().saturating_div(4).saturating_add(1) {
        match skein_codec::piece::split(&mut reader) {
            Ok(Some(turn)) => turns.push(turn).expect("four-byte prefix per turn"),
            Ok(None) => return turns.into_boxed(),
            Err(_) => return Box::new([Box::new([])]),
        }
    }
    turns.into_boxed()
}

fn assignment(
    record: wire::Assignment,
    now: Time,
    parts: &mut List<Part>,
    next: &mut u64,
    out: &mut Queue<Request>,
) -> Option<host::Assignment> {
    let record = record.into_parts();
    // Decode every form before emitting values or parts. A second bounded pass
    // avoids staging credential copies or another output queue.
    for granted in &record.grants {
        grant_form(granted.value())?;
    }

    let workspace = match record.workspace {
        Some(workspace) => {
            let workspace = workspace.into_parts();
            let items = Token::new(*next);
            *next = next.checked_add(1).expect("part tokens never wrap");
            parts.push(Part { token: items, bytes: workspace.items }).expect("one workspace part per assignment");
            Some(host::Workspace { workstream: workspace.workstream, items })
        }
        None => None,
    };
    let mut messages = List::with_capacity(record.messages.len());
    for message in record.messages.into_boxed() {
        let message = message.into_parts();
        messages
            .push(host::StartMessage {
                name: Token::new(message.name),
                sender: message.sender,
                words: message.words,
                finish_requested: message.finish_requested,
            })
            .expect("one slot per message");
    }
    let mut answered = List::with_capacity(record.answered.len());
    for call in record.answered.into_boxed() {
        let call = call.into_parts();
        let answer = match call.answer {
            wire::SettledAnswer::Host(answer) => {
                let answer = answer.into_parts();
                host::SettledAnswer::Host { error: answer.error, body: answer.body }
            }
            wire::SettledAnswer::Delivery(answer) => {
                let answer = answer.into_parts();
                let outcome = match answer.outcome {
                    wire::DeliveryOutcome::Delivered => host::DeliveryOutcome::Delivered,
                    wire::DeliveryOutcome::Nothing => host::DeliveryOutcome::Nothing,
                    wire::DeliveryOutcome::Stale => host::DeliveryOutcome::Stale,
                    wire::DeliveryOutcome::Refused => host::DeliveryOutcome::Refused,
                    wire::DeliveryOutcome::Failed => host::DeliveryOutcome::Failed,
                };
                host::SettledAnswer::Delivery { outcome, evidence: answer.evidence }
            }
        };
        answered.push(host::AnsweredCall { name: call.name, tool: call.tool, answer }).expect("one slot per call");
    }
    let turns = transcript(&record.transcript);
    let mut grants = List::with_capacity(record.grants.len());
    for granted in record.grants.into_boxed() {
        let granted = grant(granted, now, out)?;
        grants.push(granted).expect("one slot per grant");
    }
    Some(host::Assignment {
        assignment: host::RunAssignment {
            identity: record.identity,
            run: Token::new(record.task),
            attempt: Token::new(record.attempt),
            workspace,
            save: record.save,
            live: host::LiveAsk {
                text: record.live.text(),
                reasoning: record.live.reasoning(),
                input: record.live.input(),
            },
            charter: record.charter,
            grants: grants.into_boxed(),
        },
        messages: messages.into_boxed(),
        answered: answered.into_boxed(),
        turns,
    })
}

fn login_state(state: host::LoginState, limits: &wire::v9::Limits) -> Option<wire::LoginState> {
    Some(match state {
        host::LoginState::Usable => wire::LoginState::Usable,
        host::LoginState::NotSignedIn => wire::LoginState::NotSignedIn,
        host::LoginState::SignedInOtherwise(method) => wire::LoginState::SignedInOtherwise(
            wire::LoginOtherwise::new(limits, wire::LoginOtherwiseParts { method: Box::from(method.as_bytes()) })
                .ok()?,
        ),
        host::LoginState::Failed { reason, detail } => wire::LoginState::Failed(source_failed(reason, detail, limits)?),
    })
}
fn source_failed(
    reason: host::SourceFailure,
    detail: Box<[u8]>,
    limits: &wire::v9::Limits,
) -> Option<wire::SourceFailed> {
    let reason = match reason {
        host::SourceFailure::Missing => wire::SourceFailure::Missing,
        host::SourceFailure::Deadline => wire::SourceFailure::Deadline,
        host::SourceFailure::Unreadable => wire::SourceFailure::Unreadable,
        host::SourceFailure::Exited => wire::SourceFailure::Exited,
        host::SourceFailure::Unconfigured => wire::SourceFailure::Unconfigured,
        host::SourceFailure::Unknown => wire::SourceFailure::Unknown,
        host::SourceFailure::Closed => wire::SourceFailure::Closed,
    };
    wire::SourceFailed::new(limits, wire::SourceFailedParts { reason, detail }).ok()
}

fn logins_frame(logins: Box<[host::Login]>, limits: &wire::v9::Limits) -> Option<Frame> {
    let mut rows = List::<wire::Login>::with_capacity(limits.logins_logins);
    for login in logins {
        for row in &rows {
            if row.name() == login.name.as_bytes() {
                return None;
            }
        }
        rows.push(
            wire::Login::new(
                limits,
                wire::LoginParts { name: Box::from(login.name.as_bytes()), state: login_state(login.state, limits)? },
            )
            .ok()?,
        )
        .ok()?;
    }
    let record = wire::Logins::new(limits, wire::LoginsParts { logins: rows }).ok()?;
    let mut body = Writer::new(usize::try_from(record.measure()).ok()?);
    record.encode(&mut body).ok()?;
    frame(wire::Kind::Logins.number(), body.finish())
}
fn signed_out_frame(name: host::SourceName, result: host::SignOutResult, limits: &wire::v9::Limits) -> Option<Frame> {
    let result = match result {
        host::SignOutResult::SignedOut => wire::SignOutResult::SignedOut,
        host::SignOutResult::Failed { reason, detail } => {
            wire::SignOutResult::Failed(source_failed(reason, detail, limits)?)
        }
    };
    let record =
        wire::SignedOut::new(limits, wire::SignedOutParts { name: Box::from(name.as_bytes()), result }).ok()?;
    let mut body = Writer::new(usize::try_from(record.measure()).ok()?);
    record.encode(&mut body).ok()?;
    frame(wire::Kind::SignedOut.number(), body.finish())
}
