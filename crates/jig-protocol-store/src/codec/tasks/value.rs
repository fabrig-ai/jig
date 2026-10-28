//! Funding and resource values shared by tasks rows (protocol/store.md, 5.1).
use crate::tasks_v9 as wire;
use jig_domain_tasks as domain;
use skein_lib::{List, Wall};

pub(super) fn funder(value: domain::Funder, limits: &wire::Limits) -> Result<wire::Funder, wire::Problem> {
    Ok(match value {
        domain::Funder::Task(task) => wire::Funder::Task(wire::Task::new(limits, wire::TaskParts { task })?),
        domain::Funder::Pool { project, person, period } => {
            wire::Funder::Pool(wire::Pool::new(limits, wire::PoolParts { project, person, period })?)
        }
        domain::Funder::Recurring { project, task, period } => {
            wire::Funder::Recurring(wire::Recurring::new(limits, wire::RecurringParts { project, task, period })?)
        }
        domain::Funder::Period { project, period } => {
            wire::Funder::Period(wire::Period::new(limits, wire::PeriodParts { project, period })?)
        }
    })
}
pub(super) fn read_funder(value: wire::Funder) -> domain::Funder {
    match value {
        wire::Funder::Task(value) => domain::Funder::Task(value.into_parts().task),
        wire::Funder::Pool(value) => {
            let wire::PoolParts { project, person, period } = value.into_parts();
            domain::Funder::Pool { project, person, period }
        }
        wire::Funder::Recurring(value) => {
            let wire::RecurringParts { project, task, period } = value.into_parts();
            domain::Funder::Recurring { project, task, period }
        }
        wire::Funder::Period(value) => {
            let wire::PeriodParts { project, period } = value.into_parts();
            domain::Funder::Period { project, period }
        }
    }
}
pub(super) fn funding(
    value: &domain::FundingRecord,
    limits: &wire::Limits,
) -> Result<wire::FundingRecord, wire::Problem> {
    let domain::FundingRecord { period, funder: source, parent, numbers, made, closed } = value;
    let period = match period {
        Some(domain::PeriodTimes { began, next }) => {
            Some(wire::PeriodTimes::new(limits, wire::PeriodTimesParts { began: began.as_nanos(), next: wall(*next) })?)
        }
        None => None,
    };
    let parent = match parent {
        Some(value) => Some(funder(*value, limits)?),
        None => None,
    };
    let domain::Numbers { budget, spent, spent_below, reserved } = numbers;
    wire::FundingRecord::new(
        limits,
        wire::FundingRecordParts {
            period,
            funder: funder(*source, limits)?,
            parent,
            numbers: wire::Numbers::new(
                limits,
                wire::NumbersParts { budget: *budget, spent: *spent, spent_below: *spent_below, reserved: *reserved },
            )?,
            made: *made,
            closed: *closed,
        },
    )
}
#[expect(clippy::manual_map, reason = "step code uses exhaustive matches, without closures or function pointers")]
pub(super) fn read_funding(value: wire::FundingRecord) -> domain::FundingRecord {
    let wire::FundingRecordParts { period, funder, parent, numbers, made, closed } = value.into_parts();
    let period = match period {
        Some(value) => {
            let wire::PeriodTimesParts { began, next } = value.into_parts();
            Some(domain::PeriodTimes { began: Wall::from_nanos(began), next: read_wall(next) })
        }
        None => None,
    };
    let parent = match parent {
        Some(value) => Some(read_funder(value)),
        None => None,
    };
    let wire::NumbersParts { budget, spent, spent_below, reserved } = numbers.into_parts();
    domain::FundingRecord {
        period,
        funder: read_funder(funder),
        parent,
        numbers: domain::Numbers { budget, spent, spent_below, reserved },
        made,
        closed,
    }
}
#[expect(clippy::manual_map, reason = "step code uses exhaustive matches, without closures or function pointers")]
pub(super) fn wall(value: Option<Wall>) -> Option<u64> {
    match value {
        Some(value) => Some(value.as_nanos()),
        None => None,
    }
}
#[expect(clippy::manual_map, reason = "step code uses exhaustive matches, without closures or function pointers")]
pub(super) fn read_wall(value: Option<u64>) -> Option<Wall> {
    match value {
        Some(value) => Some(Wall::from_nanos(value)),
        None => None,
    }
}
pub(super) fn name(value: &domain::Name, limits: &wire::Limits) -> Result<wire::Name, wire::Problem> {
    let domain::Name { connector, path } = value;
    let mut members = List::with_capacity(limits.name_path);
    let mut bytes = 0_usize;
    for segment in path {
        bytes = bytes.saturating_add(segment.len());
        let segment = wire::Segment::new(limits, wire::SegmentParts { bytes: segment.clone() })?;
        if members.push(segment).is_err() {
            return Err(name_problem());
        }
    }
    if bytes > usize::try_from(limits.segment_bytes).expect("limit fits usize") {
        return Err(name_problem());
    }
    wire::Name::new(limits, wire::NameParts { connector: *connector, path: members })
}
pub(super) fn read_name(value: wire::Name, limits: &wire::Limits) -> Result<domain::Name, wire::Problem> {
    let wire::NameParts { connector, path } = value.into_parts();
    let mut members = List::with_capacity(path.len());
    let mut bytes = 0_usize;
    for segment in path.into_boxed() {
        let wire::SegmentParts { bytes: segment } = segment.into_parts();
        bytes = bytes.saturating_add(segment.len());
        members.push(segment).expect("same number of segments");
    }
    if bytes > usize::try_from(limits.segment_bytes).expect("limit fits usize") {
        return Err(name_problem());
    }
    Ok(domain::Name { connector, path: members.into_boxed() })
}
fn name_problem() -> wire::Problem {
    wire::Problem { path: wire::Path::NamePath, reason: skein_codec::Reason::Bound }
}

/// Translate a tasks party.
pub fn party(value: &domain::Party, limits: &wire::Limits) -> Result<wire::Party, wire::Problem> {
    Ok(match value {
        domain::Party::Task(task) => wire::Party::Task(wire::Number::new(limits, wire::NumberParts { value: *task })?),
        domain::Party::Person(person) => {
            wire::Party::Person(wire::Person::new(limits, wire::PersonParts { person: *person })?)
        }
        domain::Party::Deployment { project } => {
            wire::Party::Deployment(wire::Project::new(limits, wire::ProjectParts { project: *project })?)
        }
    })
}
/// Read a tasks party.
#[must_use]
pub fn read_party(value: wire::Party) -> domain::Party {
    match value {
        wire::Party::Task(value) => domain::Party::Task(value.into_parts().value),
        wire::Party::Person(value) => domain::Party::Person(value.into_parts().person),
        wire::Party::Deployment(value) => domain::Party::Deployment { project: value.into_parts().project },
    }
}

/// Translate a person destination.
pub fn person_address(
    value: &domain::PersonAddress,
    limits: &wire::Limits,
) -> Result<wire::PersonAddress, wire::Problem> {
    Ok(match value {
        domain::PersonAddress::Person(person) => {
            wire::PersonAddress::Person(wire::Person::new(limits, wire::PersonParts { person: *person })?)
        }
        domain::PersonAddress::Role(role) => {
            wire::PersonAddress::Role(wire::Role::new(limits, wire::RoleParts { role: *role })?)
        }
    })
}
/// Read a person destination.
#[must_use]
pub fn read_person_address(value: wire::PersonAddress) -> domain::PersonAddress {
    match value {
        wire::PersonAddress::Person(value) => domain::PersonAddress::Person(value.into_parts().person),
        wire::PersonAddress::Role(value) => domain::PersonAddress::Role(value.into_parts().role),
    }
}

/// Translate a permitted executor.
pub fn authority_executor(
    value: &domain::AuthorityExecutor,
    limits: &wire::Limits,
) -> Result<wire::AuthorityExecutor, wire::Problem> {
    Ok(match value {
        domain::AuthorityExecutor::Charter(charter) => {
            wire::AuthorityExecutor::Charter(wire::Charter::new(limits, wire::CharterParts { charter: *charter })?)
        }
        domain::AuthorityExecutor::Procedure(code) => {
            wire::AuthorityExecutor::Procedure(wire::Code::new(limits, wire::CodeParts { code: *code })?)
        }
        domain::AuthorityExecutor::Role(role) => {
            wire::AuthorityExecutor::Role(wire::Role::new(limits, wire::RoleParts { role: *role })?)
        }
    })
}
/// Read a permitted executor.
#[must_use]
pub fn read_authority_executor(value: wire::AuthorityExecutor) -> domain::AuthorityExecutor {
    match value {
        wire::AuthorityExecutor::Charter(value) => domain::AuthorityExecutor::Charter(value.into_parts().charter),
        wire::AuthorityExecutor::Procedure(value) => domain::AuthorityExecutor::Procedure(value.into_parts().code),
        wire::AuthorityExecutor::Role(value) => domain::AuthorityExecutor::Role(value.into_parts().role),
    }
}

/// Translate a tasks class.
#[must_use]
pub fn class(value: domain::Class) -> wire::Class {
    match value {
        domain::Class::Transient => wire::Class::Transient,
        domain::Class::Permanent => wire::Class::Permanent,
        domain::Class::Run => wire::Class::Run,
        domain::Class::Agent => wire::Class::Agent,
        domain::Class::Lost => wire::Class::Lost,
        domain::Class::Invalid => wire::Class::Invalid,
    }
}

/// Translate a tasks class.
#[must_use]
pub fn read_class(value: wire::Class) -> domain::Class {
    match value {
        wire::Class::Transient => domain::Class::Transient,
        wire::Class::Permanent => domain::Class::Permanent,
        wire::Class::Run => domain::Class::Run,
        wire::Class::Agent => domain::Class::Agent,
        wire::Class::Lost => domain::Class::Lost,
        wire::Class::Invalid => domain::Class::Invalid,
    }
}

/// Translate a connector part kind.
#[must_use]
pub fn part_kind(value: domain::PartKind) -> wire::PartKind {
    match value {
        domain::PartKind::Resource => wire::PartKind::Resource,
        domain::PartKind::Pool => wire::PartKind::Pool,
        domain::PartKind::Result => wire::PartKind::Result,
        domain::PartKind::Procedure => wire::PartKind::Procedure,
        domain::PartKind::Effect => wire::PartKind::Effect,
        domain::PartKind::Outbox => wire::PartKind::Outbox,
        domain::PartKind::Hold => wire::PartKind::Hold,
    }
}

/// Translate a connector part kind.
#[must_use]
pub fn read_part_kind(value: wire::PartKind) -> domain::PartKind {
    match value {
        wire::PartKind::Resource => domain::PartKind::Resource,
        wire::PartKind::Pool => domain::PartKind::Pool,
        wire::PartKind::Result => domain::PartKind::Result,
        wire::PartKind::Procedure => domain::PartKind::Procedure,
        wire::PartKind::Effect => domain::PartKind::Effect,
        wire::PartKind::Outbox => domain::PartKind::Outbox,
        wire::PartKind::Hold => domain::PartKind::Hold,
    }
}

/// Translate a tasks tries.
pub fn tries(value: &domain::Tries, limits: &wire::Limits) -> Result<wire::Tries, wire::Problem> {
    let domain::Tries { transient, permanent, run, agent, lost, invalid } = value;
    wire::Tries::new(
        limits,
        wire::TriesParts {
            transient: *transient,
            permanent: *permanent,
            run: *run,
            agent: *agent,
            lost: *lost,
            invalid: *invalid,
        },
    )
}
/// Read a tasks tries.
#[must_use]
pub fn read_tries(value: wire::Tries) -> domain::Tries {
    let wire::TriesParts { transient, permanent, run, agent, lost, invalid } = value.into_parts();
    domain::Tries { transient, permanent, run, agent, lost, invalid }
}

/// Translate the cause of a task activation.
pub fn why(value: domain::Why, limits: &wire::Limits) -> Result<wire::Why, wire::Problem> {
    Ok(match value {
        domain::Why::Made => wire::Why::Made,
        domain::Why::Ready => wire::Why::Ready,
        domain::Why::Message { message } => {
            wire::Why::Message(wire::Number::new(limits, wire::NumberParts { value: message })?)
        }
        domain::Why::Released => wire::Why::Released,
        domain::Why::Retry { tries: value } => wire::Why::Retry(tries(&value, limits)?),
        domain::Why::Narrowed => wire::Why::Narrowed,
        domain::Why::Changed => wire::Why::Changed,
    })
}
/// Read the cause of a task activation.
#[must_use]
pub fn read_why(value: wire::Why) -> domain::Why {
    match value {
        wire::Why::Made => domain::Why::Made,
        wire::Why::Ready => domain::Why::Ready,
        wire::Why::Message(value) => domain::Why::Message { message: value.into_parts().value },
        wire::Why::Released => domain::Why::Released,
        wire::Why::Retry(value) => domain::Why::Retry { tries: read_tries(value) },
        wire::Why::Narrowed => domain::Why::Narrowed,
        wire::Why::Changed => domain::Why::Changed,
    }
}
/// Translate the selected executor.
pub fn executor(value: domain::Executor, limits: &wire::Limits) -> Result<wire::Executor, wire::Problem> {
    Ok(match value {
        domain::Executor::Agent { charter } => {
            wire::Executor::Agent(wire::Agent::new(limits, wire::AgentParts { charter })?)
        }
        domain::Executor::Procedure { connector, code } => {
            wire::Executor::Procedure(wire::Procedure::new(limits, wire::ProcedureParts { connector, code })?)
        }
        domain::Executor::Person(address) => wire::Executor::Person(wire::PersonExecutor::new(
            limits,
            wire::PersonExecutorParts { address: person_address(&address, limits)? },
        )?),
    })
}
/// Read the selected executor.
#[must_use]
pub fn read_executor(value: wire::Executor) -> domain::Executor {
    match value {
        wire::Executor::Agent(value) => domain::Executor::Agent { charter: value.into_parts().charter },
        wire::Executor::Procedure(value) => {
            let wire::ProcedureParts { connector, code } = value.into_parts();
            domain::Executor::Procedure { connector, code }
        }
        wire::Executor::Person(value) => domain::Executor::Person(read_person_address(value.into_parts().address)),
    }
}
/// Translate one specification parameter.
pub fn parameter(value: &domain::Parameter, limits: &wire::Limits) -> Result<wire::Parameter, wire::Problem> {
    Ok(match value {
        domain::Parameter::Number { name, value } => wire::Parameter::Number(wire::NumericParameter::new(
            limits,
            wire::NumericParameterParts { name: *name, value: *value },
        )?),
        domain::Parameter::Bytes { name, value } => wire::Parameter::Bytes(wire::BytesParameter::new(
            limits,
            wire::BytesParameterParts { name: *name, value: value.clone() },
        )?),
        domain::Parameter::Resource { name, connector, resource } => {
            wire::Parameter::Resource(wire::ResourceParameter::new(
                limits,
                wire::ResourceParameterParts { name: *name, connector: *connector, resource: *resource },
            )?)
        }
    })
}
/// Read one specification parameter.
#[must_use]
pub fn read_parameter(value: wire::Parameter) -> domain::Parameter {
    match value {
        wire::Parameter::Number(value) => {
            let wire::NumericParameterParts { name, value } = value.into_parts();
            domain::Parameter::Number { name, value }
        }
        wire::Parameter::Bytes(value) => {
            let wire::BytesParameterParts { name, value } = value.into_parts();
            domain::Parameter::Bytes { name, value }
        }
        wire::Parameter::Resource(value) => {
            let wire::ResourceParameterParts { name, connector, resource } = value.into_parts();
            domain::Parameter::Resource { name, connector, resource }
        }
    }
}
/// Translate a specification, checking its aggregate byte bound.
pub fn spec(value: &domain::Spec, limits: &wire::Limits) -> Result<wire::Spec, wire::Problem> {
    let domain::Spec { asks, words, parameters, inputs } = value;
    let mut members = List::with_capacity(limits.spec_parameters);
    let mut bytes = words.len();
    for item in parameters {
        match item {
            domain::Parameter::Bytes { value, .. } => bytes = bytes.saturating_add(value.len()),
            domain::Parameter::Number { .. } | domain::Parameter::Resource { .. } => {}
        }
        let item = parameter(item, limits)?;
        if members.push(item).is_err() {
            return Err(problem(wire::Path::SpecParameters));
        }
    }
    if bytes > usize::try_from(limits.spec_words).expect("limit fits usize") {
        return Err(problem(wire::Path::SpecWords));
    }
    let mut references = List::with_capacity(limits.spec_inputs);
    for input in inputs {
        let input = wire::Number::new(limits, wire::NumberParts { value: *input })?;
        if references.push(input).is_err() {
            return Err(problem(wire::Path::SpecInputs));
        }
    }
    wire::Spec::new(
        limits,
        wire::SpecParts { asks: *asks, words: words.clone(), parameters: members, inputs: references },
    )
}
/// Read a specification, checking its aggregate byte bound.
pub fn read_spec(value: wire::Spec, limits: &wire::Limits) -> Result<domain::Spec, wire::Problem> {
    let wire::SpecParts { words, parameters, inputs, asks } = value.into_parts();
    let mut members = List::with_capacity(parameters.len());
    let mut bytes = words.len();
    for item in parameters.into_boxed() {
        let item = read_parameter(item);
        match &item {
            domain::Parameter::Bytes { value, .. } => bytes = bytes.saturating_add(value.len()),
            domain::Parameter::Number { .. } | domain::Parameter::Resource { .. } => {}
        }
        members.push(item).expect("same number of parameters");
    }
    if bytes > usize::try_from(limits.spec_words).expect("limit fits usize") {
        return Err(problem(wire::Path::SpecWords));
    }
    let mut references = List::with_capacity(inputs.len());
    for input in inputs.into_boxed() {
        references.push(input.into_parts().value).expect("same number of inputs");
    }
    Ok(domain::Spec { asks, words, parameters: members.into_boxed(), inputs: references.into_boxed() })
}
/// Translate a connector part and its opaque key.
pub fn part(value: &domain::Part, limits: &wire::Limits) -> Result<wire::Part, wire::Problem> {
    let domain::Part { connector, kind, key } = value;
    let key = match key {
        domain::PartKey::Number(value) => {
            wire::PartKey::Number(wire::Number::new(limits, wire::NumberParts { value: *value })?)
        }
        domain::PartKey::Path(path) => {
            let mut members = List::with_capacity(limits.part_path_segments);
            let mut bytes = 0_usize;
            for segment in path {
                bytes = bytes.saturating_add(segment.len());
                let segment = wire::PartSegment::new(limits, wire::PartSegmentParts { bytes: segment.clone() })?;
                if members.push(segment).is_err() {
                    return Err(problem(wire::Path::PartPathSegments));
                }
            }
            if bytes > usize::try_from(limits.part_segment_bytes).expect("limit fits usize") {
                return Err(problem(wire::Path::PartPathSegments));
            }
            wire::PartKey::Path(wire::PartPath::new(limits, wire::PartPathParts { segments: members })?)
        }
    };
    wire::Part::new(limits, wire::PartParts { connector: *connector, kind: part_kind(*kind), key })
}
/// Read a connector part and its opaque key.
pub fn read_part(value: wire::Part, limits: &wire::Limits) -> Result<domain::Part, wire::Problem> {
    let wire::PartParts { connector, kind, key } = value.into_parts();
    let key = match key {
        wire::PartKey::Number(value) => domain::PartKey::Number(value.into_parts().value),
        wire::PartKey::Path(value) => {
            let mut bytes = 0_usize;
            let segments = value.into_parts().segments;
            let mut members = List::with_capacity(segments.len());
            for segment in segments.into_boxed() {
                let segment = segment.into_parts().bytes;
                bytes = bytes.saturating_add(segment.len());
                members.push(segment).expect("same number of segments");
            }
            if bytes > usize::try_from(limits.part_segment_bytes).expect("limit fits usize") {
                return Err(problem(wire::Path::PartPathSegments));
            }
            domain::PartKey::Path(members)
        }
    };
    Ok(domain::Part { connector, kind: read_part_kind(kind), key })
}
fn problem(path: wire::Path) -> wire::Problem {
    wire::Problem { path, reason: skein_codec::Reason::Bound }
}

/// Translate a result contract.
pub fn contract(value: &domain::Contract, limits: &wire::Limits) -> Result<wire::Contract, wire::Problem> {
    check_contract(value, limits)?;
    Ok(match value {
        domain::Contract::Approval { state, part: value } => wire::Contract::Approval(wire::Approval::new(
            limits,
            wire::ApprovalParts { state: state.clone(), part: part(value, limits)? },
        )?),
        domain::Contract::Report { words } => {
            wire::Contract::Report(wire::Report::new(limits, wire::ReportParts { words: *words })?)
        }
        domain::Contract::Change { connector, kind, words } => wire::Contract::Change(wire::ChangeContract::new(
            limits,
            wire::ChangeContractParts { connector: *connector, kind: *kind, words: *words },
        )?),
        domain::Contract::Verdict { choices } => {
            let mut members = List::with_capacity(limits.choices_choices);
            for choice in choices {
                let domain::Verdict { code, words, followups, fields } = choice;
                let rules = super::pieces::rules(fields, limits)?;
                let choice = wire::Verdict::new(
                    limits,
                    wire::VerdictParts { code: *code, words: *words, followups: *followups, fields: rules },
                )?;
                if members.push(choice).is_err() {
                    return Err(problem(wire::Path::ChoicesChoices));
                }
            }
            wire::Contract::Verdict(wire::Choices::new(limits, wire::ChoicesParts { choices: members })?)
        }
    })
}
/// Read a result contract.
pub fn read_contract(value: wire::Contract, limits: &wire::Limits) -> Result<domain::Contract, wire::Problem> {
    let value = match value {
        wire::Contract::Approval(value) => {
            let wire::ApprovalParts { state, part: value } = value.into_parts();
            domain::Contract::Approval { state, part: read_part(value, limits)? }
        }
        wire::Contract::Report(value) => domain::Contract::Report { words: value.into_parts().words },
        wire::Contract::Change(value) => {
            let wire::ChangeContractParts { connector, kind, words } = value.into_parts();
            domain::Contract::Change { connector, kind, words }
        }
        wire::Contract::Verdict(value) => {
            let choices = value.into_parts().choices;
            let mut members = List::with_capacity(choices.len());
            for choice in choices.into_boxed() {
                let wire::VerdictParts { code, words, followups, fields } = choice.into_parts();
                let rules = super::pieces::read_rules(fields, limits)?;
                members
                    .push(domain::Verdict { code, words, followups, fields: rules })
                    .expect("same number of choices");
            }
            domain::Contract::Verdict { choices: members.into_boxed() }
        }
    };
    check_contract(&value, limits)?;
    Ok(value)
}
/// Translate a resource authority pattern.
pub fn pattern(value: &domain::Pattern, limits: &wire::Limits) -> Result<wire::Pattern, wire::Problem> {
    let domain::Pattern { segments, last } = value;
    let mut members = List::with_capacity(limits.pattern_segments);
    for segment in segments {
        let segment = wire::AuthoritySegment::new(limits, wire::AuthoritySegmentParts { bytes: segment.clone() })?;
        if members.push(segment).is_err() {
            return Err(problem(wire::Path::PatternSegments));
        }
    }
    let last = match last {
        domain::Last::Exact(bytes) => wire::Last::Exact(wire::AuthoritySegment::new(
            limits,
            wire::AuthoritySegmentParts { bytes: bytes.clone() },
        )?),
        domain::Last::Open(bytes) => {
            wire::Last::Open(wire::AuthoritySegment::new(limits, wire::AuthoritySegmentParts { bytes: bytes.clone() })?)
        }
    };
    wire::Pattern::new(limits, wire::PatternParts { segments: members, last })
}
/// Read a resource authority pattern.
#[must_use]
pub fn read_pattern(value: wire::Pattern) -> domain::Pattern {
    let wire::PatternParts { segments, last } = value.into_parts();
    let mut members = List::with_capacity(segments.len());
    for segment in segments.into_boxed() {
        members.push(segment.into_parts().bytes).expect("same number of segments");
    }
    let last = match last {
        wire::Last::Exact(value) => domain::Last::Exact(value.into_parts().bytes),
        wire::Last::Open(value) => domain::Last::Open(value.into_parts().bytes),
    };
    domain::Pattern { segments: members.into_boxed(), last }
}
/// Translate authority, checking bytes shared by grants and note resources.
pub fn authority(
    value: &domain::Authority,
    limits: &wire::Limits,
    moved: Option<&[u64; 64]>,
) -> Result<wire::Authority, wire::Problem> {
    check_authority(value, limits)?;
    let domain::Authority { tools, models, grants, delegation, budget, notes, note_resources } = value;
    let models = super::models::selector(*models, moved);
    let mut translated_grants = List::with_capacity(limits.authority_grants);
    for grant in grants {
        let domain::Grant { connector, kind, pattern: value } = grant;
        let grant = wire::Grant::new(
            limits,
            wire::GrantParts { connector: *connector, kind: *kind, pattern: pattern(value, limits)? },
        )?;
        if translated_grants.push(grant).is_err() {
            return Err(problem(wire::Path::AuthorityGrants));
        }
    }
    let mut resources = List::with_capacity(limits.authority_note_resources);
    for scope in note_resources {
        let domain::ResourceScope { connector, pattern: value } = scope;
        let scope = wire::ResourceScope::new(
            limits,
            wire::ResourceScopeParts { connector: *connector, pattern: pattern(value, limits)? },
        )?;
        if resources.push(scope).is_err() {
            return Err(problem(wire::Path::AuthorityNoteResources));
        }
    }
    let domain::Delegation { kinds, tasks, depth } = delegation;
    let mut executors = List::with_capacity(limits.delegation_kinds);
    for kind in kinds {
        let kind = authority_executor(kind, limits)?;
        if executors.push(kind).is_err() {
            return Err(problem(wire::Path::DelegationKinds));
        }
    }
    let domain::Budget { spend, deadline } = budget;
    wire::Authority::new(
        limits,
        wire::AuthorityParts {
            tools: wire::Tools::new(limits, wire::ToolsParts { bits: tools.0 })?,
            models: wire::Models::new(limits, wire::ModelsParts { bits: models.bits(), all: models.is_all() })?,
            grants: translated_grants,
            delegation: wire::Delegation::new(
                limits,
                wire::DelegationParts { kinds: executors, tasks: *tasks, depth: *depth },
            )?,
            budget: wire::Budget::new(limits, wire::BudgetParts { spend: *spend, deadline: wall(*deadline) })?,
            notes: wire::Scopes::new(limits, wire::ScopesParts { bits: notes.0 })?,
            note_resources: resources,
        },
    )
}
/// Read authority, checking bytes shared by grants and note resources.
pub fn read_authority(value: wire::Authority, limits: &wire::Limits) -> Result<domain::Authority, wire::Problem> {
    let wire::AuthorityParts { tools, models, grants, delegation, budget, notes, note_resources } = value.into_parts();
    let mut translated_grants = List::with_capacity(grants.len());
    for grant in grants.into_boxed() {
        let wire::GrantParts { connector, kind, pattern } = grant.into_parts();
        translated_grants
            .push(domain::Grant { connector, kind, pattern: read_pattern(pattern) })
            .expect("same number of grants");
    }
    let mut resources = List::with_capacity(note_resources.len());
    for scope in note_resources.into_boxed() {
        let wire::ResourceScopeParts { connector, pattern } = scope.into_parts();
        resources
            .push(domain::ResourceScope { connector, pattern: read_pattern(pattern) })
            .expect("same number of resources");
    }
    let wire::DelegationParts { kinds, tasks, depth } = delegation.into_parts();
    let mut executors = List::with_capacity(kinds.len());
    for kind in kinds.into_boxed() {
        executors.push(read_authority_executor(kind)).expect("same number of executors");
    }
    let wire::BudgetParts { spend, deadline } = budget.into_parts();
    let value = domain::Authority {
        tools: domain::Tools(tools.into_parts().bits),
        models: domain::Models::from_parts(models.bits(), models.all()),
        grants: translated_grants.into_boxed(),
        delegation: domain::Delegation { kinds: executors.into_boxed(), tasks, depth },
        budget: domain::Budget { spend, deadline: read_wall(deadline) },
        notes: domain::Scopes(notes.into_parts().bits),
        note_resources: resources.into_boxed(),
    };
    check_authority(&value, limits)?;
    Ok(value)
}
/// Translate an agent task's LLM choice, if any (domain/tasks.md, section 3).
pub(super) fn llm(
    source: Option<&domain::LlmChoice>,
    limits: &wire::Limits,
) -> Result<Option<wire::LlmChoice>, wire::Problem> {
    let Some(domain::LlmChoice { main, subagents, account }) = source else {
        return Ok(None);
    };
    let mut chosen = List::with_capacity(limits.llm_choice_subagents);
    for subagent in subagents {
        if chosen.push(model_effort(*subagent, limits)?).is_err() {
            return Err(problem(wire::Path::LlmChoiceSubagents));
        }
    }
    Ok(Some(wire::LlmChoice::new(
        limits,
        wire::LlmChoiceParts { main: model_effort(*main, limits)?, subagents: chosen, account: *account },
    )?))
}
fn model_effort(source: domain::ModelEffort, limits: &wire::Limits) -> Result<wire::ModelEffort, wire::Problem> {
    let domain::ModelEffort { model, effort } = source;
    wire::ModelEffort::new(limits, wire::ModelEffortParts { model, effort })
}
/// Read an agent task's LLM choice, if any; its decode bounds its sub-agents.
pub(super) fn read_llm(source: Option<wire::LlmChoice>) -> Option<domain::LlmChoice> {
    let wire::LlmChoiceParts { main, subagents, account } = source?.into_parts();
    let mut chosen = List::with_capacity(subagents.len());
    for subagent in subagents.into_boxed() {
        chosen.push(read_model_effort(subagent)).expect("same number of sub-agents");
    }
    Some(domain::LlmChoice { main: read_model_effort(main), subagents: chosen.into_boxed(), account })
}
fn read_model_effort(source: wire::ModelEffort) -> domain::ModelEffort {
    let wire::ModelEffortParts { model, effort } = source.into_parts();
    domain::ModelEffort { model, effort }
}
fn check_authority(value: &domain::Authority, limits: &wire::Limits) -> Result<(), wire::Problem> {
    let mut bytes = 0_usize;
    for grant in &value.grants {
        bytes = bytes.saturating_add(pattern_bytes(&grant.pattern));
    }
    for scope in &value.note_resources {
        bytes = bytes.saturating_add(pattern_bytes(&scope.pattern));
    }
    if bytes > usize::try_from(limits.authority_segment_bytes).expect("limit fits usize") {
        return Err(problem(wire::Path::AuthorityGrants));
    }
    Ok(())
}
fn pattern_bytes(value: &domain::Pattern) -> usize {
    let mut bytes = match &value.last {
        domain::Last::Exact(bytes) | domain::Last::Open(bytes) => bytes.len(),
    };
    for segment in &value.segments {
        bytes = bytes.saturating_add(segment.len());
    }
    bytes
}

fn check_contract(value: &domain::Contract, limits: &wire::Limits) -> Result<(), wire::Problem> {
    let bound = usize::try_from(limits.field_rule_name).expect("limit fits usize");
    match value {
        domain::Contract::Report { words } => {
            if usize::try_from(*words).expect("u32 fits usize") > bound {
                return Err(problem(wire::Path::ReportWords));
            }
        }
        domain::Contract::Change { words, .. } => {
            if usize::try_from(*words).expect("u32 fits usize") > bound {
                return Err(problem(wire::Path::ChangeContractWords));
            }
        }
        domain::Contract::Verdict { choices } => {
            for choice in choices {
                let mut bytes = usize::try_from(choice.words).expect("u32 fits usize");
                for field in &choice.fields {
                    bytes = bytes
                        .saturating_add(field.name.len())
                        .saturating_add(usize::try_from(field.max).expect("u32 fits usize"));
                }
                if bytes > bound {
                    return Err(problem(wire::Path::VerdictFields));
                }
            }
        }
        domain::Contract::Approval { .. } => {}
    }
    Ok(())
}
