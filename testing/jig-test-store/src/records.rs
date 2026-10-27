//! Total translations of the test connector records (protocol/store.md, 12).
use crate::{Undecodable, v2 as w};
use alloc::boxed::Box;
use jig_test_connector as d;
use skein_lib::{List, Reader, Token, Wall, Writer};
mod facts;
use facts::{decode_refusal, encode_refusal};
/// Encode one record admitted under the connector's bounds.
#[must_use]
pub fn encode(record: &d::Record) -> Box<[u8]> {
    let row =
        w::Row::new(&w::CEILINGS, w::RowParts { value: encode_record(record) }).expect("admitted record fits schema");
    let mut writer = Writer::new(usize::try_from(row.measure()).expect("row length fits"));
    row.encode(&mut writer).expect("measured room");
    writer.finish()
}
/// Decode one record within this connector's configured bounds.
pub fn decode(bytes: &[u8], limits: &d::Limits) -> Result<d::Record, Undecodable> {
    let bounds = crate::bounds(limits).ok_or(Undecodable)?;
    let Ok(row) = w::Row::decode(&bounds, &mut Reader::new(bytes)) else {
        return Err(Undecodable);
    };
    decode_record(row.into_parts().value, limits)
}

fn encode_key(value: &d::Key) -> w::Key {
    let d::Key { attempt, completion, position, deployment, task, purpose } = value;
    w::Key::new(
        &w::CEILINGS,
        w::KeyParts {
            attempt: *attempt,
            completion: *completion,
            position: *position,
            deployment: *deployment,
            task: *task,
            purpose: *purpose,
        },
    )
    .expect("admitted fields fit schema")
}
fn decode_key(value: w::Key) -> d::Key {
    let w::KeyParts { attempt, completion, position, deployment, task, purpose } = value.into_parts();
    d::Key { attempt, completion, position, deployment, task, purpose }
}
fn encode_effect(value: &d::Effect) -> w::Effect {
    let d::Effect { kind, resources, purpose, condition, target, state } = value;
    w::Effect::new(
        &w::CEILINGS,
        w::EffectParts {
            kind: *kind,
            resources: encode_paths(resources),
            purpose: *purpose,
            condition: *condition,
            target: *target,
            state: *state,
        },
    )
    .expect("admitted fields fit schema")
}
fn decode_effect(value: w::Effect, limits: &d::Limits) -> Result<d::Effect, Undecodable> {
    let w::EffectParts { kind, resources, purpose, condition, target, state } = value.into_parts();
    Ok(d::Effect { kind, resources: decode_paths(resources, limits)?, purpose, condition, target, state })
}
fn encode_attempt(value: &d::Attempt) -> w::Attempt {
    let d::Attempt { number, sent, deadline } = value;
    w::Attempt::new(
        &w::CEILINGS,
        w::AttemptParts { number: *number, sent: sent.as_nanos(), deadline: deadline.as_nanos() },
    )
    .expect("admitted fields fit schema")
}
fn decode_attempt(value: w::Attempt) -> d::Attempt {
    let w::AttemptParts { number, sent, deadline } = value.into_parts();
    d::Attempt { number, sent: Wall::from_nanos(sent), deadline: Wall::from_nanos(deadline) }
}
#[expect(clippy::manual_map, reason = "step code translates optional fields without closures")]
fn encode_outbox_entry(value: &d::OutboxEntry) -> w::OutboxEntry {
    let d::OutboxEntry { number, task, key, effect, attempt, phase } = value;
    w::OutboxEntry::new(
        &w::CEILINGS,
        w::OutboxEntryParts {
            number: *number,
            task: *task,
            key: encode_key(key),
            effect: encode_effect(effect),
            attempt: match attempt {
                Some(value) => Some(encode_attempt(value)),
                None => None,
            },
            phase: encode_effect_phase(*phase),
        },
    )
    .expect("admitted fields fit schema")
}
#[expect(clippy::manual_map, reason = "step code translates optional fields without closures")]
fn decode_outbox_entry(value: w::OutboxEntry, limits: &d::Limits) -> Result<d::OutboxEntry, Undecodable> {
    let w::OutboxEntryParts { number, task, key, effect, attempt, phase } = value.into_parts();
    Ok(d::OutboxEntry {
        number,
        task,
        key: decode_key(key),
        effect: decode_effect(effect, limits)?,
        attempt: match attempt {
            Some(value) => Some(decode_attempt(value)),
            None => None,
        },
        phase: decode_effect_phase(phase),
    })
}
#[expect(clippy::manual_map, reason = "step code translates optional fields without closures")]
fn encode_procedure_state(value: &d::ProcedureState) -> w::ProcedureState {
    let d::ProcedureState { project, task, number, resource, index, steps, awaiting, deadline, delegate, revision } =
        value;
    w::ProcedureState::new(
        &w::CEILINGS,
        w::ProcedureStateParts {
            project: *project,
            task: *task,
            number: *number,
            resource: encode_path(resource),
            index: *index,
            steps: *steps,
            awaiting: *awaiting,
            delegate: *delegate,
            revision: *revision,
            deadline: match deadline {
                Some(value) => Some(value.as_nanos()),
                None => None,
            },
        },
    )
    .expect("admitted fields fit schema")
}
#[expect(clippy::manual_map, reason = "step code translates optional fields without closures")]
fn decode_procedure_state(value: w::ProcedureState, limits: &d::Limits) -> Result<d::ProcedureState, Undecodable> {
    let w::ProcedureStateParts {
        project,
        task,
        number,
        resource,
        index,
        steps,
        awaiting,
        deadline,
        delegate,
        revision,
    } = value.into_parts();
    Ok(d::ProcedureState {
        project,
        task,
        number,
        resource: decode_path(resource, limits)?,
        index,
        steps,
        awaiting,
        delegate,
        revision,
        deadline: match deadline {
            Some(value) => Some(Wall::from_nanos(value)),
            None => None,
        },
    })
}
fn encode_application_object(value: &d::ApplicationObject) -> w::ApplicationObject {
    let d::ApplicationObject { revision, value } = value;
    w::ApplicationObject::new(&w::CEILINGS, w::ApplicationObjectParts { revision: *revision, value: *value })
        .expect("admitted fields fit schema")
}
fn decode_application_object(value: w::ApplicationObject) -> d::ApplicationObject {
    let w::ApplicationObjectParts { revision, value } = value.into_parts();
    d::ApplicationObject { revision, value }
}
fn encode_effect_phase(value: d::EffectPhase) -> w::EffectPhase {
    match value {
        d::EffectPhase::Kept => w::EffectPhase::Kept,
        d::EffectPhase::Sent => w::EffectPhase::Sent,
        d::EffectPhase::Uncertain => w::EffectPhase::Uncertain,
        d::EffectPhase::Retry => w::EffectPhase::Retry,
        d::EffectPhase::Repeat => w::EffectPhase::Repeat,
        d::EffectPhase::Held => w::EffectPhase::Held,
    }
}
fn decode_effect_phase(value: w::EffectPhase) -> d::EffectPhase {
    match value {
        w::EffectPhase::Kept => d::EffectPhase::Kept,
        w::EffectPhase::Sent => d::EffectPhase::Sent,
        w::EffectPhase::Uncertain => d::EffectPhase::Uncertain,
        w::EffectPhase::Retry => d::EffectPhase::Retry,
        w::EffectPhase::Repeat => d::EffectPhase::Repeat,
        w::EffectPhase::Held => d::EffectPhase::Held,
    }
}
fn encode_resource_role(value: d::ResourceRole) -> w::ResourceRole {
    match value {
        d::ResourceRole::Owned => w::ResourceRole::Owned,
        d::ResourceRole::Participating => w::ResourceRole::Participating,
        d::ResourceRole::Context => w::ResourceRole::Context,
        d::ResourceRole::Unavailable => w::ResourceRole::Unavailable,
    }
}
fn decode_resource_role(value: w::ResourceRole) -> d::ResourceRole {
    match value {
        w::ResourceRole::Owned => d::ResourceRole::Owned,
        w::ResourceRole::Participating => d::ResourceRole::Participating,
        w::ResourceRole::Context => d::ResourceRole::Context,
        w::ResourceRole::Unavailable => d::ResourceRole::Unavailable,
    }
}
fn encode_application_outcome(value: &d::ApplicationOutcome) -> w::ApplicationOutcome {
    match value {
        d::ApplicationOutcome::Changed { object, now } => w::ApplicationOutcome::Changed(
            w::ApplicationOutcomeChanged::new(
                &w::CEILINGS,
                w::ApplicationOutcomeChangedParts { object: *object, now: encode_application_object(now) },
            )
            .expect("admitted fields fit schema"),
        ),
        d::ApplicationOutcome::Moved { object, now } => w::ApplicationOutcome::Moved(
            w::ApplicationOutcomeMoved::new(
                &w::CEILINGS,
                w::ApplicationOutcomeMovedParts { object: *object, now: encode_application_object(now) },
            )
            .expect("admitted fields fit schema"),
        ),
        d::ApplicationOutcome::Role { held } => w::ApplicationOutcome::Role(
            w::ApplicationOutcomeRole::new(&w::CEILINGS, w::ApplicationOutcomeRoleParts { held: *held })
                .expect("admitted fields fit schema"),
        ),
        d::ApplicationOutcome::Unknown => w::ApplicationOutcome::Unknown,
        d::ApplicationOutcome::Exhausted => w::ApplicationOutcome::Exhausted,
        d::ApplicationOutcome::CoreRefused { why } => w::ApplicationOutcome::CoreRefused(
            w::ApplicationOutcomeCoreRefused::new(
                &w::CEILINGS,
                w::ApplicationOutcomeCoreRefusedParts { why: encode_refusal(why) },
            )
            .expect("admitted fields fit schema"),
        ),
    }
}
fn decode_application_outcome(value: w::ApplicationOutcome) -> d::ApplicationOutcome {
    match value {
        w::ApplicationOutcome::Changed(value) => {
            let w::ApplicationOutcomeChangedParts { object, now } = value.into_parts();
            d::ApplicationOutcome::Changed { object, now: decode_application_object(now) }
        }
        w::ApplicationOutcome::Moved(value) => {
            let w::ApplicationOutcomeMovedParts { object, now } = value.into_parts();
            d::ApplicationOutcome::Moved { object, now: decode_application_object(now) }
        }
        w::ApplicationOutcome::Role(value) => {
            let w::ApplicationOutcomeRoleParts { held } = value.into_parts();
            d::ApplicationOutcome::Role { held }
        }
        w::ApplicationOutcome::Unknown => d::ApplicationOutcome::Unknown,
        w::ApplicationOutcome::Exhausted => d::ApplicationOutcome::Exhausted,
        w::ApplicationOutcome::CoreRefused(value) => {
            let w::ApplicationOutcomeCoreRefusedParts { why } = value.into_parts();
            d::ApplicationOutcome::CoreRefused { why: decode_refusal(why) }
        }
    }
}
fn encode_record(value: &d::Record) -> w::Value {
    match value {
        d::Record::ApplicationObject { object, now } => w::Value::ApplicationObject(
            w::RecordApplicationObject::new(
                &w::CEILINGS,
                w::RecordApplicationObjectParts { object: *object, now: encode_application_object(now) },
            )
            .expect("admitted fields fit schema"),
        ),
        d::Record::ApplicationAnswer { token, until, outcome } => w::Value::ApplicationAnswer(
            w::RecordApplicationAnswer::new(
                &w::CEILINGS,
                w::RecordApplicationAnswerParts {
                    token: token.raw(),
                    until: until.as_nanos(),
                    outcome: encode_application_outcome(outcome),
                },
            )
            .expect("admitted fields fit schema"),
        ),
        d::Record::Proposal { number, task, project, effect } => w::Value::Proposal(
            w::RecordProposal::new(
                &w::CEILINGS,
                w::RecordProposalParts {
                    number: *number,
                    task: *task,
                    project: *project,
                    effect: encode_effect(effect),
                },
            )
            .expect("admitted fields fit schema"),
        ),
        d::Record::Procedure(value) => w::Value::Procedure(
            w::RecordProcedure::new(&w::CEILINGS, w::RecordProcedureParts { value: encode_procedure_state(value) })
                .expect("admitted fields fit schema"),
        ),
        d::Record::ProcedureDelegateOutcome { task, step, child, revisions } => {
            encode_step(*task, *step, *child, revisions)
        }
        d::Record::Result { task, code } => w::Value::Result(
            w::RecordResult::new(&w::CEILINGS, w::RecordResultParts { task: *task, code: *code })
                .expect("admitted fields fit schema"),
        ),
        d::Record::Task { task, project, resources } => w::Value::Task(
            w::RecordTask::new(
                &w::CEILINGS,
                w::RecordTaskParts { task: *task, project: *project, resources: encode_paths(resources) },
            )
            .expect("admitted fields fit schema"),
        ),
        d::Record::Adoption { project, path, role } => w::Value::Adoption(
            w::RecordAdoption::new(
                &w::CEILINGS,
                w::RecordAdoptionParts {
                    project: *project,
                    path: encode_path(path),
                    role: encode_resource_role(*role),
                },
            )
            .expect("admitted fields fit schema"),
        ),
        d::Record::Subscription { topic, task, wake_at, keep_at, markers } => w::Value::Subscription(
            w::RecordSubscription::new(
                &w::CEILINGS,
                w::RecordSubscriptionParts {
                    topic: *topic,
                    task: *task,
                    wake_at: *wake_at,
                    keep_at: *keep_at,
                    markers: *markers,
                },
            )
            .expect("admitted fields fit schema"),
        ),
        d::Record::Pool { path, slots } => w::Value::Pool(
            w::RecordPool::new(&w::CEILINGS, w::RecordPoolParts { path: encode_path(path), slots: *slots })
                .expect("admitted fields fit schema"),
        ),
        d::Record::Outbox(value) => w::Value::Outbox(
            w::RecordOutbox::new(&w::CEILINGS, w::RecordOutboxParts { value: encode_outbox_entry(value) })
                .expect("admitted fields fit schema"),
        ),
        d::Record::Made { key, resources, state } => w::Value::Made(
            w::RecordMade::new(
                &w::CEILINGS,
                w::RecordMadeParts { key: encode_key(key), resources: encode_paths(resources), state: *state },
            )
            .expect("admitted fields fit schema"),
        ),
    }
}
fn decode_record(value: w::Value, limits: &d::Limits) -> Result<d::Record, Undecodable> {
    Ok(match value {
        w::Value::ApplicationObject(value) => {
            let w::RecordApplicationObjectParts { object, now } = value.into_parts();
            d::Record::ApplicationObject { object, now: decode_application_object(now) }
        }
        w::Value::ApplicationAnswer(value) => {
            let w::RecordApplicationAnswerParts { token, until, outcome } = value.into_parts();
            d::Record::ApplicationAnswer {
                token: Token::new(token),
                until: Wall::from_nanos(until),
                outcome: decode_application_outcome(outcome),
            }
        }
        w::Value::Proposal(value) => {
            let w::RecordProposalParts { number, task, project, effect } = value.into_parts();
            d::Record::Proposal { number, task, project, effect: decode_effect(effect, limits)? }
        }
        w::Value::Procedure(value) => {
            let w::RecordProcedureParts { value } = value.into_parts();
            d::Record::Procedure(decode_procedure_state(value, limits)?)
        }
        w::Value::ProcedureDelegateOutcome(value) => {
            let w::RecordProcedureDelegateOutcomeParts { task, step, child, revisions } = value.into_parts();
            if revisions.len() > limits.tasks {
                return Err(Undecodable);
            }
            let mut decoded = List::with_capacity(revisions.len());
            for value in revisions.into_boxed() {
                let w::SpecRevisionParts { task, revision } = value.into_parts();
                decoded.push((task, revision)).expect("one revision per entry");
            }
            let revisions = decoded.into_boxed();
            d::Record::ProcedureDelegateOutcome { task, step, child, revisions }
        }
        w::Value::Result(value) => {
            let w::RecordResultParts { task, code } = value.into_parts();
            d::Record::Result { task, code }
        }
        w::Value::Task(value) => {
            let w::RecordTaskParts { task, project, resources } = value.into_parts();
            d::Record::Task { task, project, resources: decode_paths(resources, limits)? }
        }
        w::Value::Adoption(value) => {
            let w::RecordAdoptionParts { project, path, role } = value.into_parts();
            d::Record::Adoption { project, path: decode_path(path, limits)?, role: decode_resource_role(role) }
        }
        w::Value::Subscription(value) => {
            let w::RecordSubscriptionParts { topic, task, wake_at, keep_at, markers } = value.into_parts();
            d::Record::Subscription { topic, task, wake_at, keep_at, markers }
        }
        w::Value::Pool(value) => {
            let w::RecordPoolParts { path, slots } = value.into_parts();
            d::Record::Pool { path: decode_path(path, limits)?, slots }
        }
        w::Value::Outbox(value) => {
            let w::RecordOutboxParts { value } = value.into_parts();
            d::Record::Outbox(decode_outbox_entry(value, limits)?)
        }
        w::Value::Made(value) => {
            let w::RecordMadeParts { key, resources, state } = value.into_parts();
            d::Record::Made { key: decode_key(key), resources: decode_paths(resources, limits)?, state }
        }
    })
}

fn encode_paths(paths: &[d::Path]) -> List<w::ResourcePath> {
    let mut result = List::with_capacity(u32::try_from(paths.len()).expect("bounded paths"));
    for path in paths {
        result.push(encode_path(path)).expect("same count");
    }
    result
}
fn decode_paths(paths: List<w::ResourcePath>, limits: &d::Limits) -> Result<Box<[d::Path]>, Undecodable> {
    let mut result = List::with_capacity(paths.len());
    for path in paths.into_boxed() {
        result.push(decode_path(path, limits)?).expect("same count");
    }
    Ok(result.into_boxed())
}
fn encode_path(path: &d::Path) -> w::ResourcePath {
    let mut segments = List::with_capacity(u32::try_from(path.segments().len()).expect("bounded path"));
    for bytes in path.segments() {
        segments
            .push(w::Segment::new(&w::CEILINGS, w::SegmentParts { bytes: bytes.clone() }).expect("bounded segment"))
            .expect("same count");
    }
    w::ResourcePath::new(&w::CEILINGS, w::ResourcePathParts { segments }).expect("bounded path")
}
fn decode_path(path: w::ResourcePath, limits: &d::Limits) -> Result<d::Path, Undecodable> {
    let mut segments = List::with_capacity(path.segments().len());
    for segment in path.into_parts().segments.into_boxed() {
        segments.push(segment.into_parts().bytes).expect("same count");
    }
    d::Path::new(segments, limits.segment_bytes).ok_or(Undecodable)
}

fn encode_step(task: u64, step: u64, child: Option<u64>, revisions: &[(u64, u64)]) -> w::Value {
    let mut encoded = List::with_capacity(u32::try_from(revisions.len()).expect("bounded revisions"));
    for (task, revision) in revisions {
        encoded
            .push(
                w::SpecRevision::new(&w::CEILINGS, w::SpecRevisionParts { task: *task, revision: *revision })
                    .expect("revision fits"),
            )
            .expect("one revision per entry");
    }
    w::Value::ProcedureDelegateOutcome(
        w::RecordProcedureDelegateOutcome::new(
            &w::CEILINGS,
            w::RecordProcedureDelegateOutcomeParts { task, step, child, revisions: encoded },
        )
        .expect("admitted fields fit schema"),
    )
}
