//! Worker channels at the test root (domain/root.md, section 5).
//! Payloads stay owned here until the fleet asks for or drops them. The
//! protocol routing alone reads result bytes and encodes typed assignments.
use crate::{Delivery, Domain, Limits, Payload, Request};
use alloc::boxed::Box;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_host as host;
use jig_domain_tasks as tasks;
use jig_test_connector as connector;
use skein_lib::{Decimal, Env, List, Queue, ReplyTo, Token, Wall};

/// A scripted terminal is already read; a channel terminal is opaque.
#[derive(Debug)]
pub(super) enum Ending {
    Scripted(tasks::End),
    Link(host::Ending),
}

/// The three inputs of a worker channel, in arrival order.
#[derive(Debug)]
pub enum Event {
    Hello { worker: u32, hello: host::Hello },
    Up(host::LinkUp),
    Lost,
}

/// Translate a worker's complete admission report.
#[must_use]
pub fn hello(channel: Token, worker: u32, hello: host::Hello) -> core::Event {
    let mut models = List::with_capacity(u32::try_from(hello.bounds.models.len()).expect("bounded models"));
    for model in hello.bounds.models {
        models
            .push(fleet::ModelBound { name: model.name, window: model.window, output: model.output })
            .expect("one slot per model");
    }
    let mut hosting = List::with_capacity(u32::try_from(hello.hosting.len()).expect("bounded hosted runs"));
    for run in hello.hosting {
        hosting
            .push(fleet::Hosted { run: run.run, attempt: run.attempt, phase: phase(run.phase) })
            .expect("one slot per hosted run");
    }
    core::Event::Fleet(fleet::Event::Hello {
        channel,
        worker,
        hello: fleet::Hello {
            slots: hello.slots,
            stop_bound: hello.stop_bound,
            workstreams: hello.workstreams,
            hosting: hosting.into_boxed(),
            bounds: fleet::Bounds {
                program_endpoints: hello.bounds.program_endpoints,
                models: models.into_boxed(),
                turns: hello.bounds.turns,
                spend: hello.bounds.spend,
                time: hello.bounds.time,
                waiting: hello.bounds.waiting,
                checks: hello.bounds.checks,
                message_bytes: hello.bounds.message_bytes,
                inbox: hello.bounds.inbox,
                unacknowledged: hello.bounds.unacknowledged,
            },
        },
    })
}
fn phase(phase: host::Phase) -> fleet::Phase {
    match phase {
        host::Phase::Preparing => fleet::Phase::Preparing,
        host::Phase::Starting => fleet::Phase::Starting,
        host::Phase::Active => fleet::Phase::Active,
        host::Phase::Waiting => fleet::Phase::Waiting,
        host::Phase::Ending => fleet::Phase::Ending,
        host::Phase::Answered => fleet::Phase::Answered,
    }
}
/// A channel loss goes through the fleet's grace and fences.
#[must_use]
pub fn lost(channel: Token) -> core::Event {
    core::Event::Fleet(fleet::Event::Lost { channel })
}
/// Retain a worker's bytes and translate only their boundary metadata.
#[must_use]
pub fn up(domain: &mut Domain, channel: Token, up: host::LinkUp) -> Option<core::Event> {
    let event = match up {
        host::LinkUp::Piece { run, attempt, piece } => return live_piece(domain, run, attempt, piece),
        host::LinkUp::Logins { logins } => {
            let mut rows = List::with_capacity(u32::try_from(logins.len()).expect("bounded source rows"));
            for login in logins {
                rows.push(fleet::Login { name: source_name(login.name), state: source_login(login.state) })
                    .expect("one source row per input");
            }
            fleet::Event::Logins { channel, logins: rows.into_boxed() }
        }
        host::LinkUp::SignedOut { name, result } => fleet::Event::SignedOut {
            channel,
            name: source_name(name),
            result: match result {
                host::SignOutResult::SignedOut => fleet::SignOutResult::SignedOut,
                host::SignOutResult::Failed { reason, detail } => {
                    fleet::SignOutResult::Failed { reason: source_failure(reason), detail }
                }
            },
        },
        host::LinkUp::Turn { run, attempt, turn } => {
            let number = turn.turn;
            let body = crate::payload(
                domain,
                Payload::Turn {
                    task: run.raw(),
                    attempt: attempt.raw(),
                    turn: number,
                    cumulative: turn.spent,
                    read: read_name(turn.read),
                    transcript: turn.body,
                    opens_window: turn.opens_window,
                    refused: refusals(turn.refused),
                },
            );
            fleet::Event::Turn { channel, run, attempt, turn: number, body }
        }
        host::LinkUp::Answer { run, attempt, answer } => {
            let kind = match answer.ending {
                host::Ending::Ended { .. } => fleet::Answer::Ended,
                host::Ending::Parked { .. } => fleet::Answer::Parked,
                host::Ending::Failed { failure, .. } => fleet::Answer::Failed { turned: turned(failure) },
                host::Ending::Refused(host::Refusal::Busy) => fleet::Answer::Busy,
                host::Ending::Refused(host::Refusal::Invalid(_)) => fleet::Answer::Invalid,
            };
            let payload = crate::payload(
                domain,
                Payload::Answer {
                    task: run.raw(),
                    attempt: attempt.raw(),
                    cumulative: answer.spent,
                    read: read_name(answer.read),
                    refused: refusals(answer.refused),
                    end: Ending::Link(answer.ending),
                },
            );
            fleet::Event::Answer { channel, run, attempt, answer: kind, payload }
        }
        host::LinkUp::Call { run, attempt, call, tool, writes, input, deadline } => fleet::Event::Relay {
            channel,
            run,
            attempt,
            call: fleet::Call { name: call, tool, writes, input, deadline },
        },
        host::LinkUp::Unauthorized { run, attempt, account, generation } => {
            fleet::Event::Unauthorized { channel, run, attempt, account, generation }
        }
        host::LinkUp::Limited { run, attempt, account, reset } => {
            fleet::Event::Limited { channel, run, attempt, account, reset }
        }
        host::LinkUp::AccountRefused { run, attempt, account, code, reason } => {
            fleet::Event::AccountRefused { channel, run, attempt, account, code, reason }
        }
        host::LinkUp::Activity { run, attempt, activity } => {
            fleet::Event::Observe { channel, run, attempt, report: report(activity) }
        }
    };
    Some(core::Event::Fleet(event))
}
/// The account a failed run turned on, as the fleet carries it.
const fn turned(failure: host::Failure) -> Option<fleet::Turned> {
    match failure {
        host::Failure::Run(host::RunFailure::Limited { account, reset }) => {
            Some(fleet::Turned::Limited { account, reset })
        }
        host::Failure::Run(host::RunFailure::Refused { account }) => Some(fleet::Turned::Refused { account }),
        host::Failure::Run(
            host::RunFailure::Exceeded(_)
            | host::RunFailure::Model
            | host::RunFailure::Budget
            | host::RunFailure::Policy
            | host::RunFailure::Cancelled
            | host::RunFailure::Stale,
        )
        | host::Failure::Unprepared(_)
        | host::Failure::Agent(_)
        | host::Failure::Cancelled(_) => None,
    }
}
fn read_name(name: Option<Token>) -> Option<u64> {
    Some(name?.raw())
}
fn refusals(values: Box<[host::Refused]>) -> Box<[u64]> {
    let mut names = List::with_capacity(u32::try_from(values.len()).expect("bounded refusals"));
    for value in values {
        names.push(value.name.raw()).expect("one slot per refusal");
    }
    names.into_boxed()
}

pub(super) fn receive(domain: &mut Domain, channel: Token, link: Event, work: &mut Queue<Work>) {
    let event = match link {
        Event::Hello { worker, hello: value } => {
            // The port is routing state; the fleet owns admission and its refusal.
            let _inserted = domain.links.insert(channel);
            Some(hello(channel, worker, value))
        }
        Event::Up(value) => up(domain, channel, value),
        Event::Lost => {
            let _removed = domain.links.remove(&channel);
            Some(lost(channel))
        }
    };
    if let Some(event) = event {
        work.push(Work::Core(event));
    }
}

/// Translate complete channel deliveries; typed charters stay handed out.
/// The scripted worker port continues to receive its original vocabulary.
#[must_use]
pub fn down(domain: &mut Domain, delivery: Delivery) -> Delivery {
    let (channel, down) = match delivery {
        Delivery::Core(core::Held::Sources { channel, sources }) if domain.links.contains(&channel) => {
            let mut names = List::with_capacity(u32::try_from(sources.len()).expect("bounded source names"));
            for name in sources {
                names.push(host_source_name(name)).expect("one name per input");
            }
            (channel, host::LinkDown::Sources { sources: names.into_boxed() })
        }
        Delivery::Core(core::Held::CheckSource { channel, name }) if domain.links.contains(&channel) => {
            (channel, host::LinkDown::Check { name: host_source_name(name) })
        }
        Delivery::Core(core::Held::SignOutSource { channel, name }) if domain.links.contains(&channel) => {
            (channel, host::LinkDown::SignOut { name: host_source_name(name) })
        }
        Delivery::Message { channel, task, attempt, word } if domain.links.contains(&channel) => (
            channel,
            host::LinkDown::Message {
                run: Token::new(task),
                attempt: Token::new(attempt),
                name: Token::new(word.number),
                sender: sender(word.from),
                finish_requested: word.finish_requested(),
                words: jig_protocol_tools::words(&word),
            },
        ),
        Delivery::CallBusy { channel, task, attempt, name } if domain.links.contains(&channel) => (
            channel,
            host::LinkDown::Relayed {
                run: Token::new(task),
                attempt: Token::new(attempt),
                call: name,
                answer: host::Relayed::Busy,
            },
        ),
        Delivery::CallAnswer { channel, task, attempt, name, call, writes } if domain.links.contains(&channel) => {
            match call.answer {
                core::SettledAnswer::Host { error, body } => (
                    channel,
                    host::LinkDown::Relayed {
                        run: Token::new(task),
                        attempt: Token::new(attempt),
                        call: name,
                        answer: host::Relayed::Answer { error, text: body },
                    },
                ),
                core::SettledAnswer::Delivery { .. } => {
                    return Delivery::CallAnswer { channel, task, attempt, name, call, writes };
                }
            }
        }
        Delivery::Core(core::Held::Acknowledge { channel, run, attempt }) if domain.links.contains(&channel) => {
            (channel, host::LinkDown::Acknowledge { run, attempt })
        }
        Delivery::Core(core::Held::AcknowledgeTurn { channel, run, attempt, turn })
            if domain.links.contains(&channel) =>
        {
            (channel, host::LinkDown::TurnKept { run, attempt, turn })
        }
        Delivery::Core(core::Held::Cancel { channel, run, attempt }) if domain.links.contains(&channel) => {
            (channel, host::LinkDown::Cancel { run, attempt })
        }
        Delivery::Core(core::Held::Park { channel, run, attempt }) if domain.links.contains(&channel) => {
            (channel, host::LinkDown::Park { run, attempt })
        }
        Delivery::Core(core::Held::Grant { channel, run, attempt, grant }) if domain.links.contains(&channel) => (
            channel,
            host::LinkDown::Grant {
                run,
                attempt,
                grant: host::Grant {
                    form: host_form(grant.form),
                    account: grant.account,
                    generation: grant.generation,
                    valid: grant.valid,
                },
            },
        ),
        Delivery::Core(core::Held::Refuse { channel }) => {
            let _removed = domain.links.remove(&channel);
            return Delivery::Core(core::Held::Refuse { channel });
        }
        other @ (Delivery::Link { .. }
        | Delivery::Message { .. }
        | Delivery::CallBusy { .. }
        | Delivery::CallAnswer { .. }
        | Delivery::Core(_)
        | Delivery::Assigned { .. }
        | Delivery::Fleet(_)
        | Delivery::ApplicationDocument { .. }
        | Delivery::ApplicationRead { .. }
        | Delivery::Application { .. }
        | Delivery::ProjectMade { .. }
        | Delivery::Projection { .. }
        | Delivery::Restart(_)
        | Delivery::System { .. }
        | Delivery::LostRead { .. }
        | Delivery::Procedure { .. }) => return other,
    };
    Delivery::Link { channel, down }
}

pub(super) fn answer(domain: &mut Domain, run: Token, attempt: Token, payload: Token, work: &mut Queue<Work>) {
    let Some(Payload::Answer { task, attempt: retained, end, .. }) = domain.payloads.get(&payload) else {
        unreachable!("fleet retained the answer body");
    };
    assert!(*task == run.raw() && *retained == attempt.raw(), "fenced answer identity");
    let end = match end {
        Ending::Link(host::Ending::Ended { outcome, .. }) => {
            domain.now.push(Request::ReadResult { payload, outcome: outcome.clone() });
            return;
        }
        Ending::Link(
            other @ (host::Ending::Refused(_) | host::Ending::Parked { .. } | host::Ending::Failed { .. }),
        ) => ending(other),
        Ending::Scripted(end) => end.clone(),
    };
    work.push(Work::Core(result_event(domain, payload, end).expect("retained answer")));
}
pub(super) fn read(domain: &mut Domain, env: &Env<Limits>, payload: Token, end: tasks::End) {
    if let Some(event) = result_event(domain, payload, end) {
        crate::decide(domain, env, Work::Core(event));
    }
}
pub(super) fn result_event(domain: &Domain, payload: Token, end: tasks::End) -> Option<core::Event> {
    let Some(Payload::Answer { task, attempt, cumulative, read, refused, .. }) = domain.payloads.get(&payload) else {
        return None;
    };
    Some(core::Event::AnswerPayload {
        run: Token::new(*task),
        attempt: Token::new(*attempt),
        payload,
        cumulative: *cumulative,
        end,
        saved: None,
        invalid_saved: false,
        read: *read,
        refused: refused.clone(),
    })
}
fn ending(ending: &host::Ending) -> tasks::End {
    match ending {
        host::Ending::Refused(host::Refusal::Busy) => tasks::End::Refused,
        host::Ending::Refused(host::Refusal::Invalid(_)) => tasks::End::Failed(tasks::Class::Invalid),
        host::Ending::Parked { .. } => tasks::End::Parked,
        host::Ending::Failed { failure, .. } => match failure {
            host::Failure::Run(host::RunFailure::Exceeded(value)) => tasks::End::Exceeded(tasks::Exceeded {
                bound: match value.bound {
                    host::Bound::Reasoning => tasks::Bound::Reasoning,
                    host::Bound::Metadata => tasks::Bound::Metadata,
                    host::Bound::Items => tasks::Bound::Items,
                    host::Bound::Answer => tasks::Bound::Answer,
                    host::Bound::Tools => tasks::Bound::Tools,
                    host::Bound::HistoryItems => tasks::Bound::HistoryItems,
                    host::Bound::History => tasks::Bound::History,
                    host::Bound::Window => tasks::Bound::Window,
                    host::Bound::Output => tasks::Bound::Output,
                },
                value: value.value,
            }),
            host::Failure::Unprepared(host::Preparation::Transient) => tasks::End::Failed(tasks::Class::Transient),
            host::Failure::Unprepared(host::Preparation::Permanent { .. }) => {
                tasks::End::Failed(tasks::Class::Permanent)
            }
            host::Failure::Run(
                host::RunFailure::Model
                | host::RunFailure::Budget
                | host::RunFailure::Policy
                | host::RunFailure::Cancelled
                | host::RunFailure::Stale
                | host::RunFailure::Limited { .. }
                | host::RunFailure::Refused { .. },
            ) => tasks::End::Failed(tasks::Class::Run),
            host::Failure::Agent(_) | host::Failure::Cancelled(_) => tasks::End::Failed(tasks::Class::Agent),
        },
        host::Ending::Ended { .. } => unreachable!("ended payloads leave for the routing"),
    }
}

fn report(activity: host::Activity) -> fleet::Report {
    match activity {
        host::Activity::Phase(phase) => fleet::Report::Activity(match phase {
            host::Phase::Preparing => fleet::Activity::Preparing,
            host::Phase::Starting => fleet::Activity::Starting,
            host::Phase::Active => fleet::Activity::Active,
            host::Phase::Waiting => fleet::Activity::Waiting,
            host::Phase::Ending | host::Phase::Answered => fleet::Activity::Ending,
        }),
        host::Activity::Long { operation, span } => fleet::Report::Activity(fleet::Activity::Long {
            operation: match operation {
                host::Operation::Checks => fleet::Operation::Checks,
                host::Operation::Completion => fleet::Operation::Completion,
                host::Operation::Command => fleet::Operation::Command,
            },
            span,
        }),
        host::Activity::LongDone => fleet::Report::Activity(fleet::Activity::Active),
        host::Activity::Count { kind, elapsed, count } => fleet::Report::Count {
            kind: match kind {
                host::CountKind::Admitted => fleet::CountKind::Admitted,
                host::CountKind::Ended => fleet::CountKind::Ended,
                host::CountKind::LlmStarted => fleet::CountKind::LlmStarted,
                host::CountKind::LlmRetried => fleet::CountKind::LlmRetried,
                host::CountKind::LlmFinished => fleet::CountKind::LlmFinished,
                host::CountKind::ToolStarted => fleet::CountKind::ToolStarted,
                host::CountKind::ToolFinished => fleet::CountKind::ToolFinished,
                host::CountKind::CheckStarted => fleet::CountKind::CheckStarted,
                host::CountKind::CheckFinished => fleet::CountKind::CheckFinished,
                host::CountKind::TextArrived => fleet::CountKind::TextArrived,
            },
            elapsed,
            count,
        },
    }
}

/// Preserve the task-owned message kind when translating its opaque words.
fn sender(value: tasks::Party) -> Box<[u8]> {
    match value {
        tasks::Party::Task(number) | tasks::Party::Person(number) => Box::from(Decimal::of(number).as_bytes()),
        tasks::Party::Deployment { .. } => Box::from(&b"deployment"[..]),
    }
}

#[expect(clippy::large_enum_variant, reason = "the bounded route queue owns complete core events")]
pub(super) enum Work {
    Link {
        channel: Token,
        link: Event,
    },
    WatchApplication {
        watcher: Token,
        sign_in: u64,
        scope: jig_domain_people::ApplicationScope,
        number: u16,
        subject: connector::ClientSubject,
        window: core::documents::Window,
    },
    ReadApplication {
        reply: ReplyTo,
        sign_in: u64,
        scope: jig_domain_people::ApplicationScope,
        number: u16,
        subject: connector::ClientSubject,
        window: core::documents::Window,
    },
    Application {
        reply_to: ReplyTo,
        sign_in: u64,
        key: [u8; 16],
        scope: jig_domain_people::ApplicationScope,
        number: u16,
        digest: [u8; 32],
        value: connector::ApplicationRequest,
        carried: Option<core::Carried>,
    },
    ProjectionEffect {
        goal: u64,
        number: u16,
        effect: connector::Effect,
    },
    ConnectorFire {
        number: u16,
    },
    ConnectorResume {
        number: u16,
    },
    Switched,
    Restart(core::RestartRequest),
    CallAnswered {
        to: ReplyTo,
        call: core::SettledCall,
    },
    SwitchChecked,
    AdoptRestored,
    AdoptDone,
    WriterRead {
        connector: u16,
        resource: tasks::Name,
        event: connector::SystemEvent,
    },
    Timer(core::Timer),
    Core(core::Event),
    ResumeFleet,
    Finish,
    Connector {
        number: u16,
        event: connector::Event,
    },
    TopicCall {
        to: ReplyTo,
        key: core::CallKey,
        number: u16,
        topic: u16,
    },
    EffectCall {
        to: ReplyTo,
        key: core::CallKey,
        number: u16,
        effect: connector::Effect,
        deadline: Wall,
        proposal: Option<Box<[u8]>>,
    },
    Turn {
        channel: Token,
        task: u64,
        attempt: u64,
        turn: u32,
        cumulative: u64,
        read: Option<u64>,
        opens_window: bool,
        refused: Box<[u64]>,
        transcript: Box<[u8]>,
    },
    Answer {
        channel: Token,
        task: u64,
        attempt: u64,
        cumulative: u64,
        end: tasks::End,
        read: Option<u64>,
        refused: Box<[u64]>,
    },
}

fn host_form(form: fleet::Form) -> host::Form {
    match form {
        fleet::Form::Credential(scheme) => host::Form::Credential(match scheme {
            fleet::Scheme::Bearer => host::Scheme::Bearer,
            fleet::Scheme::Key => host::Scheme::Key,
        }),
        fleet::Form::Application => host::Form::Application,
        fleet::Form::Program(name) => {
            host::Form::Program(host::SourceName::new(name.as_bytes()).expect("source name follows the same rules"))
        }
    }
}

fn source_name(name: host::SourceName) -> fleet::SourceName {
    fleet::SourceName::new(name.as_bytes()).expect("same source name grammar")
}
fn host_source_name(name: fleet::SourceName) -> host::SourceName {
    host::SourceName::new(name.as_bytes()).expect("same source name grammar")
}
fn source_login(state: host::LoginState) -> fleet::LoginState {
    match state {
        host::LoginState::Usable => fleet::LoginState::Usable,
        host::LoginState::NotSignedIn => fleet::LoginState::NotSignedIn,
        host::LoginState::SignedInOtherwise(method) => fleet::LoginState::SignedInOtherwise(
            fleet::LoginMethod::new(method.as_bytes()).expect("same printable method grammar"),
        ),
        host::LoginState::Failed { reason, detail } => {
            fleet::LoginState::Failed { reason: source_failure(reason), detail }
        }
    }
}
const fn source_failure(reason: host::SourceFailure) -> fleet::SourceFailure {
    match reason {
        host::SourceFailure::Missing => fleet::SourceFailure::Missing,
        host::SourceFailure::Deadline => fleet::SourceFailure::Deadline,
        host::SourceFailure::Unreadable => fleet::SourceFailure::Unreadable,
        host::SourceFailure::Exited => fleet::SourceFailure::Exited,
        host::SourceFailure::Unconfigured => fleet::SourceFailure::Unconfigured,
        host::SourceFailure::Unknown => fleet::SourceFailure::Unknown,
        host::SourceFailure::Closed => fleet::SourceFailure::Closed,
    }
}

fn live_piece(domain: &mut Domain, run: Token, attempt: Token, piece: host::Piece) -> Option<core::Event> {
    if let Ok(offset) = u32::try_from(piece.offset) {
        match piece.kind {
            host::LiveKind::Text => {
                return Some(core::Event::RunText {
                    run,
                    attempt,
                    turn: piece.turn,
                    retry: piece.retry,
                    offset,
                    text: piece.bytes,
                    call: None,
                });
            }
            host::LiveKind::Input { call, tool } => {
                return Some(core::Event::RunInput {
                    run,
                    attempt,
                    turn: piece.turn,
                    retry: piece.retry,
                    offset,
                    id: call,
                    tool,
                    input: piece.bytes,
                });
            }
            host::LiveKind::Reasoning => {}
        }
    }
    domain.pieces_dropped = domain.pieces_dropped.saturating_add(1);
    None
}
