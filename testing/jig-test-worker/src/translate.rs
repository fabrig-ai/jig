//! Stateless vocabulary translations; no payload codecs (hosts.md, 6.8).
use jig_domain_host as host;
use smith_host_domain as smith;

#[must_use]
pub fn grant(value: host::Grant) -> Option<smith::Grant> {
    let form = match value.form {
        host::Form::Credential(host::Scheme::Bearer) => smith::Form::Credential(smith::Scheme::Bearer),
        host::Form::Credential(host::Scheme::Key) => smith::Form::Credential(smith::Scheme::Key),
        host::Form::Program(name) => smith::Form::Program(smith::SourceName::new(name.as_bytes())?),
        host::Form::Application => return None,
    };
    Some(smith::Grant { account: value.account, generation: value.generation, valid: value.valid, form })
}

#[must_use]
pub const fn fault(value: smith::Fault) -> host::AgentFailure {
    match value {
        smith::Fault::Exited => host::AgentFailure::Exited,
        smith::Fault::Rules | smith::Fault::TooLarge => host::AgentFailure::Rules,
        smith::Fault::NoProgress => host::AgentFailure::NoProgress,
        smith::Fault::WallTime => host::AgentFailure::WallTime,
    }
}

#[must_use]
pub const fn refusal(value: smith::MessageRefusal) -> host::MessageRefusal {
    match value {
        smith::MessageRefusal::TooLarge => host::MessageRefusal::TooLarge,
        smith::MessageRefusal::Full => host::MessageRefusal::Full,
        smith::MessageRefusal::Ending => host::MessageRefusal::Ending,
        smith::MessageRefusal::NameInUse => host::MessageRefusal::NameInUse,
    }
}

#[must_use]
pub const fn operation(value: smith::Operation) -> host::Operation {
    match value {
        smith::Operation::Checks => host::Operation::Checks,
        smith::Operation::Completion => host::Operation::Completion,
        smith::Operation::Command => host::Operation::Command,
    }
}

#[must_use]
pub const fn failure(value: &smith::RunFailure) -> host::RunFailure {
    match value {
        smith::RunFailure::Model(smith::ModelFault::Limited { account, reset }) => {
            host::RunFailure::Limited { account: *account, reset: *reset }
        }
        smith::RunFailure::Model(smith::ModelFault::Refused { account, code: _ }) => {
            host::RunFailure::Refused { account: *account }
        }
        smith::RunFailure::Transcript(_)
        | smith::RunFailure::Model(
            smith::ModelFault::Completion { .. }
            | smith::ModelFault::Unauthorized { .. }
            | smith::ModelFault::Program { .. }
            | smith::ModelFault::Provider
            | smith::ModelFault::Declined
            | smith::ModelFault::Truncated
            | smith::ModelFault::Malformed,
        ) => host::RunFailure::Model,
        smith::RunFailure::Budget { exhausted: _, used: _, bound: _ } => host::RunFailure::Budget,
        smith::RunFailure::Policy(_) => host::RunFailure::Policy,
        smith::RunFailure::Cancelled => host::RunFailure::Cancelled,
        smith::RunFailure::Stale => host::RunFailure::Stale,
    }
}

/// A refusal before process admission is not an agent crash.
#[must_use]
pub const fn unstarted(end: smith::End) -> Option<&'static [u8]> {
    match end {
        smith::End::Busy => Some(b"busy"),
        smith::End::Invalid(invalid) => Some(match invalid {
            smith::Invalid::Messages => b"messages",
            smith::Invalid::Activation => b"activation",
            smith::Invalid::Charter => b"charter",
            smith::Invalid::Transcript => b"transcript",
            smith::Invalid::Answered => b"answered calls",
            smith::Invalid::Directories => b"directories",
            smith::Invalid::Grants => b"grants",
            smith::Invalid::Limits => b"limits",
        }),
        smith::End::Unspawned | smith::End::Exited { .. } | smith::End::Settled => None,
    }
}

/// Preserve agent start policy separately from its diagnostic bytes.
#[must_use]
pub const fn start_cause(why: smith::RunInvalid) -> host::StartCause {
    match why {
        smith::RunInvalid::Grants => host::StartCause::Grants,
        smith::RunInvalid::HostPart { bound } => host::StartCause::HostPart { bound },
        smith::RunInvalid::Room { .. }
        | smith::RunInvalid::Messages
        | smith::RunInvalid::CharterVersion
        | smith::RunInvalid::MalformedCharter
        | smith::RunInvalid::Activation
        | smith::RunInvalid::Window
        | smith::RunInvalid::Conventions
        | smith::RunInvalid::TooLarge
        | smith::RunInvalid::Workspace
        | smith::RunInvalid::Outcome
        | smith::RunInvalid::Budget(_)
        | smith::RunInvalid::Price(_)
        | smith::RunInvalid::Llm
        | smith::RunInvalid::Effort
        | smith::RunInvalid::Conversation
        | smith::RunInvalid::Endpoint => host::StartCause::Other,
    }
}
