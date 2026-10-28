//! Total translations of the worker records (protocol/channel.md, sections 3 and 6.4).
use alloc::boxed::Box;
use jig_domain_host as host;
use jig_protocol_worker_wire as wire;
use skein_lib::{List, Reader, Token};
pub(crate) fn hello(record: wire::Hello) -> Option<host::Hello> {
    let fields = record.into_parts();
    let bounds = fields.bounds.into_parts();
    if !host::valid_program_endpoints(bounds.program_endpoints.as_slice()) {
        return None;
    }
    let mut models = List::with_capacity(bounds.models.len());
    for model in bounds.models.into_boxed() {
        let model = model.into_parts();
        models
            .push(host::ModelBound { name: model.name, window: model.window, output: model.output })
            .expect("one slot per decoded model");
    }
    let mut hosting = List::with_capacity(fields.hosting.len());
    for run in fields.hosting.into_boxed() {
        let run = run.into_parts();
        hosting
            .push(host::Hosting {
                run: Token::new(run.task),
                attempt: Token::new(run.attempt),
                phase: phase(run.phase),
            })
            .expect("one slot per decoded run");
    }
    Some(host::Hello {
        slots: fields.slots,
        stop_bound: fields.stop_bound,
        workstreams: fields.workstreams.into_boxed(),
        hosting: hosting.into_boxed(),
        bounds: host::Bounds {
            program_endpoints: bounds.program_endpoints.into_boxed(),
            models: models.into_boxed(),
            turns: bounds.turns,
            spend: bounds.spend,
            time: bounds.time,
            waiting: bounds.waiting,
            checks: bounds.checks,
            message_bytes: bounds.message_bytes,
            inbox: bounds.inbox,
            unacknowledged: bounds.unacknowledged,
        },
    })
}
fn phase(value: wire::Phase) -> host::Phase {
    match value {
        wire::Phase::Preparing => host::Phase::Preparing,
        wire::Phase::Starting => host::Phase::Starting,
        wire::Phase::Active => host::Phase::Active,
        wire::Phase::Waiting => host::Phase::Waiting,
        wire::Phase::Ending => host::Phase::Ending,
        wire::Phase::Answered => host::Phase::Answered,
    }
}

#[derive(Debug)]
pub(crate) enum Up {
    Source(host::LinkUp),
    Turn(wire::Turn),
    Call(wire::Call),
    Answer(wire::Answer),
    Fact(wire::Fact),
    Unauthorized(wire::Unauthorized),
    Limited(wire::Limited),
    AccountRefused(wire::AccountRefused),
}
pub(crate) fn decode(kind: u16, body: &[u8], limits: &wire::v9::Limits) -> Option<Up> {
    let mut found = None;
    for rule in wire::KINDS {
        if rule.kind == kind {
            found = Some(rule.body);
            break;
        }
    }
    let kind = found?;
    let reader = &mut Reader::new(body);
    match kind {
        wire::Kind::Logins => {
            let record = wire::Logins::decode(limits, reader).ok()?.into_parts();
            let mut rows = List::<host::Login>::with_capacity(record.logins.len());
            for row in record.logins.into_boxed() {
                let row = row.into_parts();
                let name = host::SourceName::new(&row.name)?;
                for old in &rows {
                    if old.name == name {
                        return None;
                    }
                }
                rows.push(host::Login { name, state: login_state(row.state)? }).ok()?;
            }
            Some(Up::Source(host::LinkUp::Logins { logins: rows.into_boxed() }))
        }
        wire::Kind::SignedOut => {
            let record = wire::SignedOut::decode(limits, reader).ok()?.into_parts();
            let name = host::SourceName::new(&record.name)?;
            let result = match record.result {
                wire::SignOutResult::SignedOut => host::SignOutResult::SignedOut,
                wire::SignOutResult::Failed(failure) => {
                    let failure = failure.into_parts();
                    host::SignOutResult::Failed { reason: source_failure(failure.reason), detail: failure.detail }
                }
            };
            Some(Up::Source(host::LinkUp::SignedOut { name, result }))
        }
        wire::Kind::Turn => Some(Up::Turn(wire::Turn::decode(limits, reader).ok()?)),
        wire::Kind::Call => Some(Up::Call(wire::Call::decode(limits, reader).ok()?)),
        wire::Kind::Answer => Some(Up::Answer(wire::Answer::decode(limits, reader).ok()?)),
        wire::Kind::Fact => Some(Up::Fact(wire::Fact::decode(limits, reader).ok()?)),
        wire::Kind::Unauthorized => Some(Up::Unauthorized(wire::Unauthorized::decode(limits, reader).ok()?)),
        wire::Kind::Limited => Some(Up::Limited(wire::Limited::decode(limits, reader).ok()?)),
        wire::Kind::AccountRefused => Some(Up::AccountRefused(wire::AccountRefused::decode(limits, reader).ok()?)),
        wire::Kind::Sources
        | wire::Kind::Check
        | wire::Kind::SignOut
        | wire::Kind::Hello
        | wire::Kind::Assignment
        | wire::Kind::Message
        | wire::Kind::Relayed
        | wire::Kind::TurnKept
        | wire::Kind::TurnBusy
        | wire::Kind::Acknowledge
        | wire::Kind::Cancel
        | wire::Kind::Park
        | wire::Kind::Grant => None,
    }
}
pub(crate) fn up(record: Up, parts: &mut Parts) -> host::LinkUp {
    match record {
        Up::Source(up) => up,
        Up::Turn(record) => turn(record),
        Up::Call(record) => call(record),
        Up::Answer(record) => answer(record, parts),
        Up::Fact(record) => fact(record),
        Up::Unauthorized(record) => {
            let fields = record.into_parts();
            host::LinkUp::Unauthorized {
                run: Token::new(fields.task),
                attempt: Token::new(fields.attempt),
                account: fields.account,
                generation: fields.generation,
            }
        }
        Up::Limited(record) => {
            let fields = record.into_parts();
            host::LinkUp::Limited {
                run: Token::new(fields.task),
                attempt: Token::new(fields.attempt),
                account: fields.account,
                reset: fields.reset,
            }
        }
        Up::AccountRefused(record) => {
            let fields = record.into_parts();
            host::LinkUp::AccountRefused {
                run: Token::new(fields.task),
                attempt: Token::new(fields.attempt),
                account: fields.account,
                code: fields.code,
                reason: fields.reason,
            }
        }
    }
}
fn turn(record: wire::Turn) -> host::LinkUp {
    let fields = record.into_parts();
    let refused = refusals(fields.refused);
    host::LinkUp::Turn {
        run: Token::new(fields.task),
        attempt: Token::new(fields.attempt),
        turn: host::Turn {
            opens_window: fields.opens_window,
            refused,
            turn: fields.number,
            spent: fields.spent,
            read: optional_token(fields.read),
            body: fields.body,
        },
    }
}
fn refusals(records: List<wire::Refused>) -> Box<[host::Refused]> {
    let mut refused = List::with_capacity(records.len());
    for record in records.into_boxed() {
        let fields = record.into_parts();
        let reason = match fields.reason {
            wire::MessageRefusal::TooLarge => host::MessageRefusal::TooLarge,
            wire::MessageRefusal::Full => host::MessageRefusal::Full,
            wire::MessageRefusal::NameInUse => host::MessageRefusal::NameInUse,
            wire::MessageRefusal::Ending => host::MessageRefusal::Ending,
        };
        refused.push(host::Refused { name: Token::new(fields.name), reason }).expect("one slot per refusal");
    }
    refused.into_boxed()
}
fn optional_token(value: Option<u64>) -> Option<Token> {
    Some(Token::new(value?))
}
fn fact(record: wire::Fact) -> host::LinkUp {
    let fields = record.into_parts();
    let activity = match fields.value {
        wire::FactValue::Piece(piece) => {
            let piece = piece.into_parts();
            let kind = match piece.kind {
                wire::LiveKind::Text => host::LiveKind::Text,
                wire::LiveKind::Reasoning => host::LiveKind::Reasoning,
                wire::LiveKind::Input(call) => {
                    let call = call.into_parts();
                    host::LiveKind::Input { call: call.id, tool: call.tool }
                }
            };
            return host::LinkUp::Piece {
                run: Token::new(fields.task),
                attempt: Token::new(fields.attempt),
                piece: host::Piece {
                    turn: piece.turn,
                    retry: piece.retry,
                    kind,
                    offset: piece.offset,
                    bytes: piece.bytes,
                },
            };
        }
        wire::FactValue::Phase(value) => host::Activity::Phase(phase(value.into_parts().value)),
        wire::FactValue::Long(value) => {
            let fields = value.into_parts();
            let operation = match fields.operation {
                wire::Operation::Checks => host::Operation::Checks,
                wire::Operation::Completion => host::Operation::Completion,
                wire::Operation::Command => host::Operation::Command,
            };
            host::Activity::Long { operation, span: fields.span }
        }
        wire::FactValue::LongDone => host::Activity::LongDone,
        wire::FactValue::Count(value) => {
            let fields = value.into_parts();
            host::Activity::Count { kind: count_kind(fields.kind), elapsed: fields.elapsed, count: fields.value }
        }
    };
    host::LinkUp::Activity { run: Token::new(fields.task), attempt: Token::new(fields.attempt), activity }
}
fn count_kind(kind: wire::CountKind) -> host::CountKind {
    match kind {
        wire::CountKind::Admitted => host::CountKind::Admitted,
        wire::CountKind::Ended => host::CountKind::Ended,
        wire::CountKind::LlmStarted => host::CountKind::LlmStarted,
        wire::CountKind::LlmRetried => host::CountKind::LlmRetried,
        wire::CountKind::LlmFinished => host::CountKind::LlmFinished,
        wire::CountKind::ToolStarted => host::CountKind::ToolStarted,
        wire::CountKind::ToolFinished => host::CountKind::ToolFinished,
        wire::CountKind::CheckStarted => host::CountKind::CheckStarted,
        wire::CountKind::CheckFinished => host::CountKind::CheckFinished,
        wire::CountKind::TextArrived => host::CountKind::TextArrived,
    }
}

#[derive(Debug)]
pub(crate) struct Parts {
    next: u64,
    values: List<wire::Part>,
}
impl Parts {
    pub fn new(next: u64) -> Self {
        Self { next, values: List::with_capacity(3) }
    }
    pub fn optional(&mut self, bytes: Option<Box<[u8]>>) -> Option<Token> {
        let bytes = bytes?;
        let token = Token::new(self.next);
        self.next = self.next.checked_add(1).expect("part tokens never wrap in a process life");
        self.values.push(wire::Part { token, bytes }).expect("at most resource, left and saved work");
        Some(token)
    }
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
    pub fn next(&self) -> u64 {
        self.next
    }
    pub fn finish(self) -> Box<[wire::Part]> {
        self.values.into_boxed()
    }
}
pub(crate) fn answer(record: wire::Answer, parts: &mut Parts) -> host::LinkUp {
    let fields = record.into_parts();
    let ending = ending(fields.ending, parts);
    host::LinkUp::Answer {
        run: Token::new(fields.task),
        attempt: Token::new(fields.attempt),
        answer: host::Answer {
            read: optional_token(fields.read),
            refused: refusals(fields.refused),
            turns: fields.turns,
            spent: fields.spent,
            ending,
        },
    }
}
fn ending(value: wire::Ending, parts: &mut Parts) -> host::Ending {
    match value {
        wire::Ending::Refused(record) => {
            let refusal = match record.into_parts().value {
                wire::Refusal::Busy => host::Refusal::Busy,
                wire::Refusal::Invalid(record) => {
                    let invalid = match record.into_parts().value {
                        wire::Invalid::Charter => host::Invalid::Charter,
                        wire::Invalid::Transcript => host::Invalid::Transcript,
                        wire::Invalid::Grants => host::Invalid::Grants,
                        wire::Invalid::Messages => host::Invalid::Messages,
                        wire::Invalid::Start(record) => {
                            let record = record.into_parts();
                            host::Invalid::Start {
                                why: record.why,
                                cause: match record.cause {
                                    wire::StartCause::Other => host::StartCause::Other,
                                    wire::StartCause::Grants => host::StartCause::Grants,
                                },
                            }
                        }
                    };
                    host::Refusal::Invalid(invalid)
                }
            };
            host::Ending::Refused(refusal)
        }
        wire::Ending::Ended(record) => {
            let fields = record.into_parts();
            host::Ending::Ended { outcome: fields.outcome, work: work(fields.work, parts) }
        }
        wire::Ending::Parked(record) => host::Ending::Parked { work: work(record.into_parts().work, parts) },
        wire::Ending::Failed(record) => {
            let fields = record.into_parts();
            let failure = failure(fields.failure, parts);
            host::Ending::Failed { failure, detail: fields.detail, work: work(fields.work, parts) }
        }
    }
}
fn work(value: wire::Work, parts: &mut Parts) -> host::Work {
    let fields = value.into_parts();
    let left = parts.optional(fields.left);
    let saved = parts.optional(fields.saved);
    host::Work { left, saved }
}
fn failure(value: wire::Failure, parts: &mut Parts) -> host::Failure {
    match value {
        wire::Failure::Unprepared(record) => {
            let preparation = match record.into_parts().value {
                wire::Preparation::Transient => host::Preparation::Transient,
                wire::Preparation::Permanent(record) => {
                    host::Preparation::Permanent { resource: parts.optional(record.into_parts().resource) }
                }
            };
            host::Failure::Unprepared(preparation)
        }
        wire::Failure::Run(record) => host::Failure::Run(run_failure(record.into_parts().value)),
        wire::Failure::Agent(record) => host::Failure::Agent(agent_failure(record.into_parts().value)),
        wire::Failure::Cancelled(record) => {
            let reason = match record.into_parts().value {
                wire::CancelReason::Engine => host::Reason::Engine,
                wire::CancelReason::Contact => host::Reason::Contact,
                wire::CancelReason::Shutdown => host::Reason::Shutdown,
            };
            host::Failure::Cancelled(reason)
        }
    }
}
fn run_failure(value: wire::RunFailure) -> host::RunFailure {
    match value {
        wire::RunFailure::Exceeded(value) => host::RunFailure::Exceeded(host::Exceeded {
            bound: match value.bound() {
                wire::Bound::Reasoning => host::Bound::Reasoning,
                wire::Bound::Metadata => host::Bound::Metadata,
                wire::Bound::Items => host::Bound::Items,
                wire::Bound::Answer => host::Bound::Answer,
                wire::Bound::Tools => host::Bound::Tools,
                wire::Bound::HistoryItems => host::Bound::HistoryItems,
                wire::Bound::History => host::Bound::History,
                wire::Bound::Window => host::Bound::Window,
                wire::Bound::Output => host::Bound::Output,
            },
            value: value.value(),
        }),
        wire::RunFailure::Model => host::RunFailure::Model,
        wire::RunFailure::Budget => host::RunFailure::Budget,
        wire::RunFailure::Policy => host::RunFailure::Policy,
        wire::RunFailure::Cancelled => host::RunFailure::Cancelled,
        wire::RunFailure::Stale => host::RunFailure::Stale,
        wire::RunFailure::Limited(record) => {
            let fields = record.into_parts();
            host::RunFailure::Limited { account: fields.account, reset: fields.reset }
        }
        wire::RunFailure::Refused(record) => host::RunFailure::Refused { account: record.into_parts().account },
    }
}
fn agent_failure(value: wire::AgentFailure) -> host::AgentFailure {
    match value {
        wire::AgentFailure::Unstarted => host::AgentFailure::Unstarted,
        wire::AgentFailure::Exited => host::AgentFailure::Exited,
        wire::AgentFailure::Rules => host::AgentFailure::Rules,
        wire::AgentFailure::NoProgress => host::AgentFailure::NoProgress,
        wire::AgentFailure::WallTime => host::AgentFailure::WallTime,
    }
}

pub(crate) fn call(record: wire::Call) -> host::LinkUp {
    let fields = record.into_parts();
    host::LinkUp::Call {
        run: Token::new(fields.task),
        attempt: Token::new(fields.attempt),
        call: fields.name,
        tool: fields.tool,
        writes: fields.writes,
        input: fields.input,
        deadline: fields.deadline,
    }
}

/// These records are private to the translation; the wire crate exposes individual codecs.
#[derive(Debug)]
pub(crate) enum Down {
    Sources(wire::Sources),
    Check(wire::Check),
    SignOut(wire::SignOut),
    Assign(wire::Assignment),
    Message(wire::Inbound),
    Relayed(wire::Relayed),
    TurnKept(wire::TurnKept),
    TurnBusy(wire::TurnBusy),
    Acknowledge(wire::Acknowledge),
    Cancel(wire::Cancel),
    Park(wire::Park),
    Grant(wire::Granted),
}
impl Down {
    pub fn frame(self) -> skein_channel::Frame {
        let (kind, length) = match &self {
            Self::Sources(record) => (0x0188, record.measure()),
            Self::Check(record) => (0x0189, record.measure()),
            Self::SignOut(record) => (0x018a, record.measure()),
            Self::Assign(record) => (0x0180, record.measure()),
            Self::Message(record) => (0x0181, record.measure()),
            Self::Relayed(record) => (0x0182, record.measure()),
            Self::TurnKept(record) => (0x0183, record.measure()),
            Self::TurnBusy(record) => (0x0184, record.measure()),
            Self::Acknowledge(record) => (0x0185, record.measure()),
            Self::Cancel(record) => (0x0186, record.measure()),
            Self::Park(record) => (wire::Kind::Park.number(), record.measure()),
            Self::Grant(record) => (0x0187, record.measure()),
        };
        let mut writer = skein_channel::frame_writer(kind, length).expect("checked record fits a frame");
        let mut body = skein_lib::Writer::new(usize::try_from(length).expect("checked body length"));
        let encoded = match self {
            Self::Sources(record) => record.encode(&mut body),
            Self::Check(record) => record.encode(&mut body),
            Self::SignOut(record) => record.encode(&mut body),
            Self::Assign(record) => record.encode(&mut body),
            Self::Message(record) => record.encode(&mut body),
            Self::Relayed(record) => record.encode(&mut body),
            Self::TurnKept(record) => record.encode(&mut body),
            Self::TurnBusy(record) => record.encode(&mut body),
            Self::Acknowledge(record) => record.encode(&mut body),
            Self::Cancel(record) => record.encode(&mut body),
            Self::Park(record) => record.encode(&mut body),
            Self::Grant(record) => record.encode(&mut body),
        };
        encoded.expect("measured record");
        writer.put(&body.finish()).expect("measured body fits frame");
        writer.finish().expect("complete record")
    }
}
pub(crate) fn down(
    down: host::LinkDown,
    parts: &[wire::Part],
    values: Box<[Option<crate::GrantValue>]>,
    limits: &wire::v9::Limits,
) -> Option<Down> {
    match down {
        host::LinkDown::Sources { sources } => {
            if !host::sources::unique(&sources) {
                return None;
            }
            let mut names = List::with_capacity(limits.sources_sources);
            for name in sources {
                names
                    .push(
                        wire::ProgramSource::new(limits, wire::ProgramSourceParts { name: Box::from(name.as_bytes()) })
                            .ok()?,
                    )
                    .ok()?;
            }
            Some(Down::Sources(wire::Sources::new(limits, wire::SourcesParts { sources: names }).ok()?))
        }
        host::LinkDown::Check { name } => {
            Some(Down::Check(wire::Check::new(limits, wire::CheckParts { name: Box::from(name.as_bytes()) }).ok()?))
        }
        host::LinkDown::SignOut { name } => Some(Down::SignOut(
            wire::SignOut::new(limits, wire::SignOutParts { name: Box::from(name.as_bytes()) }).ok()?,
        )),
        host::LinkDown::Assign { assignment } => {
            Some(Down::Assign(assignment_record(assignment, parts, values, limits)?))
        }
        host::LinkDown::Message { run, attempt, name, sender, words, finish_requested } => Some(Down::Message(
            wire::Inbound::new(
                limits,
                wire::InboundParts {
                    task: run.raw(),
                    attempt: attempt.raw(),
                    name: name.raw(),
                    sender,
                    words,
                    finish_requested,
                },
            )
            .ok()?,
        )),
        host::LinkDown::Relayed { run, attempt, call, answer } => {
            let answer = match answer {
                host::Relayed::Busy => wire::RelayAnswer::Busy,
                host::Relayed::Answer { error, text } => {
                    wire::RelayAnswer::Text(wire::RelayText::new(limits, wire::RelayTextParts { error, text }).ok()?)
                }
            };
            Some(Down::Relayed(
                wire::Relayed::new(
                    limits,
                    wire::RelayedParts { task: run.raw(), attempt: attempt.raw(), name: call, answer },
                )
                .ok()?,
            ))
        }
        host::LinkDown::TurnKept { run, attempt, turn } => Some(Down::TurnKept(
            wire::TurnKept::new(limits, wire::TurnKeptParts { task: run.raw(), attempt: attempt.raw(), number: turn })
                .ok()?,
        )),
        host::LinkDown::Acknowledge { run, attempt } => Some(Down::Acknowledge(
            wire::Acknowledge::new(limits, wire::AcknowledgeParts { task: run.raw(), attempt: attempt.raw() }).ok()?,
        )),
        host::LinkDown::Cancel { run, attempt } => Some(Down::Cancel(
            wire::Cancel::new(limits, wire::CancelParts { task: run.raw(), attempt: attempt.raw() }).ok()?,
        )),
        host::LinkDown::Park { run, attempt } => Some(Down::Park(
            wire::Park::new(limits, wire::ParkParts { run: run.raw(), attempt: attempt.raw() }).ok()?,
        )),
        host::LinkDown::Grant { run, attempt, grant } => {
            let value = values.into_iter().next()??;
            let grant = grant_record(grant, value, limits)?;
            Some(Down::Grant(
                wire::Granted::new(limits, wire::GrantedParts { task: run.raw(), attempt: attempt.raw(), grant })
                    .ok()?,
            ))
        }
    }
}
fn assignment_record(
    assignment: host::Assignment,
    parts: &[wire::Part],
    values: Box<[Option<crate::GrantValue>]>,
    limits: &wire::v9::Limits,
) -> Option<wire::Assignment> {
    let run = assignment.assignment;
    let workspace = match run.workspace {
        Some(workspace) => Some(
            wire::Workspace::new(
                limits,
                wire::WorkspaceParts { workstream: workspace.workstream, items: part(workspace.items, parts)? },
            )
            .ok()?,
        ),
        None => None,
    };
    let mut messages = List::with_capacity(limits.assignment_messages);
    for message in assignment.messages {
        messages
            .push(
                wire::Message::new(
                    limits,
                    wire::MessageParts {
                        name: message.name.raw(),
                        sender: message.sender,
                        words: message.words,
                        finish_requested: message.finish_requested,
                    },
                )
                .ok()?,
            )
            .ok()?;
    }
    let mut answered = List::with_capacity(limits.assignment_answered);
    for call in assignment.answered {
        let answer = settled(call.answer, limits)?;
        answered
            .push(
                wire::AnsweredCall::new(limits, wire::AnsweredCallParts { name: call.name, tool: call.tool, answer })
                    .ok()?,
            )
            .ok()?;
    }
    let mut grants = List::with_capacity(limits.assignment_grants);
    for (grant, value) in run.grants.into_iter().zip(values) {
        if let Some(value) = value {
            grants.push(grant_record(grant, value, limits)?).ok()?;
        }
    }
    let transcript = transcript(&assignment.turns, limits.assignment_transcript)?;
    wire::Assignment::new(
        limits,
        wire::AssignmentParts {
            task: run.run.raw(),
            attempt: run.attempt.raw(),
            identity: run.identity,
            workspace,
            save: run.save,
            live: wire::LiveAsk::new(
                limits,
                wire::LiveAskParts { text: run.live.text, reasoning: run.live.reasoning, input: run.live.input },
            )
            .ok()?,
            charter: run.charter,
            transcript,
            answered,
            messages,
            grants,
        },
    )
    .ok()
}
fn part(token: Token, parts: &[wire::Part]) -> Option<Box<[u8]>> {
    for part in parts {
        if part.token == token {
            return Some(part.bytes.clone());
        }
    }
    None
}
fn transcript(turns: &[Box<[u8]>], bound: u32) -> Option<Box<[u8]>> {
    let mut length = 0_u32;
    for turn in turns {
        length = length.checked_add(u32::try_from(turn.len()).ok()?.checked_add(4)?)?;
        if length > bound {
            return None;
        }
    }
    let mut writer = skein_lib::Writer::new(usize::try_from(length).ok()?);
    for turn in turns {
        skein_codec::piece::write(&mut writer, turn).ok()?;
    }
    Some(writer.finish())
}
/// A grant's form and its value's arm agree, or the grant is not sent.
fn grant_record(grant: host::Grant, value: crate::GrantValue, limits: &wire::v9::Limits) -> Option<wire::Grant> {
    let value = match grant.form {
        host::Form::Program(form) => match value {
            crate::GrantValue::Program { name } => {
                if name != form {
                    return None;
                }
                wire::GrantValue::Program(
                    wire::ProgramValue::new(limits, wire::ProgramValueParts { name: name.as_bytes().into() }).ok()?,
                )
            }
            crate::GrantValue::Credential { .. } | crate::GrantValue::Application { .. } => return None,
        },
        host::Form::Credential(form) => match value {
            crate::GrantValue::Credential { scheme, token } => {
                if scheme != grant_scheme(form) {
                    return None;
                }
                let scheme = match form {
                    host::Scheme::Bearer => wire::Scheme::Bearer,
                    host::Scheme::Key => wire::Scheme::Key,
                };
                wire::GrantValue::Credential(
                    wire::CredentialValue::new(limits, wire::CredentialValueParts { scheme, bytes: token }).ok()?,
                )
            }
            crate::GrantValue::Application { .. } | crate::GrantValue::Program { .. } => return None,
        },
        host::Form::Application => match value {
            crate::GrantValue::Application { secret } => wire::GrantValue::Application(
                wire::ApplicationValue::new(limits, wire::ApplicationValueParts { secret }).ok()?,
            ),
            crate::GrantValue::Credential { .. } | crate::GrantValue::Program { .. } => return None,
        },
    };
    wire::Grant::new(
        limits,
        wire::GrantParts { number: grant.account, generation: grant.generation, valid: grant.valid, value },
    )
    .ok()
}
const fn grant_scheme(scheme: host::Scheme) -> crate::Scheme {
    match scheme {
        host::Scheme::Bearer => crate::Scheme::Bearer,
        host::Scheme::Key => crate::Scheme::Key,
    }
}
fn settled(answer: host::SettledAnswer, limits: &wire::v9::Limits) -> Option<wire::SettledAnswer> {
    match answer {
        host::SettledAnswer::Host { error, body } => {
            Some(wire::SettledAnswer::Host(wire::HostAnswer::new(limits, wire::HostAnswerParts { error, body }).ok()?))
        }
        host::SettledAnswer::Delivery { outcome, evidence } => {
            let outcome = match outcome {
                host::DeliveryOutcome::Delivered => wire::DeliveryOutcome::Delivered,
                host::DeliveryOutcome::Nothing => wire::DeliveryOutcome::Nothing,
                host::DeliveryOutcome::Stale => wire::DeliveryOutcome::Stale,
                host::DeliveryOutcome::Refused => wire::DeliveryOutcome::Refused,
                host::DeliveryOutcome::Failed => wire::DeliveryOutcome::Failed,
            };
            Some(wire::SettledAnswer::Delivery(
                wire::DeliveryAnswer::new(limits, wire::DeliveryAnswerParts { outcome, evidence }).ok()?,
            ))
        }
    }
}

pub(crate) fn turn_busy(run: Token, attempt: Token, turn: u32, limits: &wire::v9::Limits) -> Down {
    Down::TurnBusy(
        wire::TurnBusy::new(limits, wire::TurnBusyParts { task: run.raw(), attempt: attempt.raw(), number: turn })
            .expect("fixed turn busy fields fit"),
    )
}

fn source_failure(reason: wire::SourceFailure) -> host::SourceFailure {
    match reason {
        wire::SourceFailure::Missing => host::SourceFailure::Missing,
        wire::SourceFailure::Deadline => host::SourceFailure::Deadline,
        wire::SourceFailure::Unreadable => host::SourceFailure::Unreadable,
        wire::SourceFailure::Exited => host::SourceFailure::Exited,
        wire::SourceFailure::Unconfigured => host::SourceFailure::Unconfigured,
        wire::SourceFailure::Unknown => host::SourceFailure::Unknown,
        wire::SourceFailure::Closed => host::SourceFailure::Closed,
    }
}
fn login_state(state: wire::LoginState) -> Option<host::LoginState> {
    Some(match state {
        wire::LoginState::Usable => host::LoginState::Usable,
        wire::LoginState::NotSignedIn => host::LoginState::NotSignedIn,
        wire::LoginState::SignedInOtherwise(method) => {
            host::LoginState::SignedInOtherwise(host::LoginMethod::new(method.method())?)
        }
        wire::LoginState::Failed(failure) => {
            let failure = failure.into_parts();
            host::LoginState::Failed { reason: source_failure(failure.reason), detail: failure.detail }
        }
    })
}
