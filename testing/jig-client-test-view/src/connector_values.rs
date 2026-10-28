//! Whole read-only owner fragments (client/domain/root.md, 6–7; view.md, 7).
//! Original request scope and owner identities remain separate; no form or
//! permission is inferred from an Object or from a procedure's current state.
use crate::connector_values_words::{ALL, Word};
use jig_client_test_domain::{
    connector_values::{
        ApplicationObject, ApplicationOutcome, ClientDocument, ClientResource, ConnectorAnswer, ConnectorDocument,
        ConnectorLimits, Description, DiagnosticAvailability, Effect, EffectPhase, Form, Hold, HoldMode, OutboxEntry,
        Path, ProcedureState, Recovery, ResourceRole, application_core,
    },
    family::Scope,
};
use jig_client_view::words::Words;
use skein_ui::phrases::Hole;
use skein_ui::{Builder, Role, Run};

use skein_lib::{List, Wall};

fn text(b: &mut Builder, words: &Words, word: Word, holes: &[Hole]) {
    let saying = words.book().say(words.language(), word.id(), holes).expect("checked owner phrase and supplied holes");
    b.text(Run::Plain, &saying.text);
}
fn number(b: &mut Builder, words: &Words, word: Word, value: u64) {
    text(b, words, word, &[Hole::Number(value)]);
}
fn wall(b: &mut Builder, words: &Words, word: Word, value: Wall) {
    text(b, words, word, &[Hole::Wall(value)]);
}
fn region(b: &mut Builder, words: &Words, word: Word) {
    let saying = words.book().say(words.language(), word.id(), &[]).expect("checked owner region phrase");
    b.open(Role::Region);
    b.name(&saying.text);
}
fn path(b: &mut Builder, words: &Words, value: &Path) {
    region(b, words, Word::Path);
    for segment in value.segments() {
        b.open(Role::Item);
        jig_client_view::literal_segment::render(b, segment);
        b.close();
    }
    b.close();
}
fn paths(b: &mut Builder, words: &Words, value: &[Path]) {
    region(b, words, Word::Resources);
    for value in value {
        path(b, words, value);
    }
    b.close();
}
fn effect(b: &mut Builder, words: &Words, value: &Effect) {
    region(b, words, Word::Request);
    number(b, words, Word::Kind, u64::from(value.kind));
    number(b, words, Word::Purpose, value.purpose);
    match value.condition {
        Some(value) => number(b, words, Word::Condition, value),
        None => text(b, words, Word::ConditionNone, &[]),
    }
    number(b, words, Word::Target, value.target);
    number(b, words, Word::State, value.state);
    paths(b, words, &value.resources);
    b.close();
}
fn description(b: &mut Builder, words: &Words, value: &Description) {
    region(b, words, Word::Description);
    number(b, words, Word::Kind, u64::from(value.kind));
    number(b, words, Word::Purpose, value.purpose);
    text(b, words, if value.condition { Word::ConditionYes } else { Word::ConditionNo }, &[]);
    let form = match value.form {
        Form::Creation => Word::Creation,
        Form::Transition => Word::Transition,
        Form::Set => Word::Set,
    };
    text(b, words, form, &[]);
    let recovery = match value.recovery {
        Recovery::Keyed => Word::Keyed,
        Recovery::Conditional => Word::Conditional,
        Recovery::Idempotent => Word::Idempotent,
        Recovery::Unrecoverable => Word::Unrecoverable,
    };
    text(b, words, recovery, &[]);
    match value.price {
        Some(value) => number(b, words, Word::Price, value),
        None => text(b, words, Word::PriceNone, &[]),
    }
    number(b, words, Word::State, value.state);
    paths(b, words, &value.resources);
    b.close();
}
fn entry(b: &mut Builder, words: &Words, value: &OutboxEntry) {
    region(b, words, Word::Outbox);
    number(b, words, Word::Entry, value.number);
    number(b, words, Word::Task, value.task);
    region(b, words, Word::Key);
    number(b, words, Word::KeyAttempt, value.key.attempt);
    number(b, words, Word::Completion, u64::from(value.key.completion));
    number(b, words, Word::Position, u64::from(value.key.position));
    number(b, words, Word::KeyTask, value.key.task);
    number(b, words, Word::KeyPurpose, value.key.purpose);
    // Separate literal positions preserve the entire deployment identity.
    for byte in value.key.deployment {
        number(b, words, Word::DeploymentByte, u64::from(byte));
    }
    b.close();
    effect(b, words, &value.effect);
    match value.attempt {
        Some(value) => {
            region(b, words, Word::Attempt);
            number(b, words, Word::AttemptNumber, u64::from(value.number));
            wall(b, words, Word::Sent, value.sent);
            wall(b, words, Word::Deadline, value.deadline);
            b.close();
        }
        None => text(b, words, Word::AttemptNone, &[]),
    }
    let phase = match value.phase {
        EffectPhase::Kept => Word::Kept,
        EffectPhase::Sent => Word::SentPhase,
        EffectPhase::Uncertain => Word::Uncertain,
        EffectPhase::Retry => Word::Retry,
        EffectPhase::Repeat => Word::Repeat,
        EffectPhase::Held => Word::Held,
    };
    text(b, words, phase, &[]);
    b.close();
}
fn entries(b: &mut Builder, words: &Words, values: &List<OutboxEntry>) {
    region(b, words, Word::Effects);
    for value in values {
        entry(b, words, value);
    }
    b.close();
}
fn resources(b: &mut Builder, words: &Words, values: &List<ClientResource>) {
    region(b, words, Word::Resources);
    for value in values {
        b.open(Role::Item);
        path(b, words, &value.path);
        let role = match value.role {
            ResourceRole::Owned => Word::Owned,
            ResourceRole::Participating => Word::Participating,
            ResourceRole::Context => Word::Context,
            ResourceRole::Unavailable => Word::Unavailable,
        };
        text(b, words, role, &[]);
        let hold = match value.hold {
            Hold::Exclusive { mode: HoldMode::Refuse } => Word::ExclusiveRefuse,
            Hold::Exclusive { mode: HoldMode::Wait } => Word::ExclusiveWait,
            Hold::Pooled { mode: HoldMode::Refuse } => Word::PooledRefuse,
            Hold::Pooled { mode: HoldMode::Wait } => Word::PooledWait,
            Hold::None => Word::NoHold,
        };
        text(b, words, hold, &[]);
        match value.fact {
            Some(value) => {
                region(b, words, Word::Fact);
                match value.state {
                    Some(value) => number(b, words, Word::FactState, value),
                    None => text(b, words, Word::StateNone, &[]),
                }
                wall(b, words, Word::Observed, value.observed);
                text(b, words, if value.pending { Word::PendingYes } else { Word::PendingNo }, &[]);
                b.close();
            }
            None => text(b, words, Word::FactNone, &[]),
        }
        b.close();
    }
    b.close();
}
fn procedure(b: &mut Builder, words: &Words, value: &ProcedureState) {
    region(b, words, Word::Procedure);
    number(b, words, Word::Task, value.task);
    number(b, words, Word::Program, u64::from(value.number));
    number(b, words, Word::Index, u64::from(value.index));
    number(b, words, Word::Steps, u64::from(value.steps));
    text(b, words, if value.awaiting { Word::AwaitingYes } else { Word::AwaitingNo }, &[]);
    match value.deadline {
        Some(value) => wall(b, words, Word::Deadline, value),
        None => text(b, words, Word::DeadlineNone, &[]),
    }
    path(b, words, &value.resource);
    b.close();
}
fn object(b: &mut Builder, words: &Words, number_value: u64, now: ApplicationObject) {
    number(b, words, Word::ObjectNumber, number_value);
    number(b, words, Word::Revision, now.revision);
    number(b, words, Word::Value, now.value);
}
/// Render every supplied value without inventing permissions, forms or revisions.
pub fn document(b: &mut Builder, value: &ConnectorDocument, words: &Words) {
    let title = match value.value.as_ref() {
        ClientDocument::Object { .. } => Word::Object,
        ClientDocument::Panel { .. } => Word::Panel,
        ClientDocument::Result { .. } => Word::Result,
        ClientDocument::Effect { .. } => Word::Effect,
        ClientDocument::Outbox { .. } => Word::Outbox,
        ClientDocument::Hold { .. } => Word::Hold,
        ClientDocument::Project { .. } => Word::Project,
        ClientDocument::Deployment { .. } => Word::Deployment,
    };
    b.open(Role::List);
    b.open(Role::Item);
    b.key(crate::family_key(&value.subject));
    region(b, words, title);
    match value.subject.scope {
        Scope::Project(project) => number(b, words, Word::ScopeProject, u64::from(project)),
        Scope::Deployment => text(b, words, Word::ScopeDeployment, &[]),
    }
    match value.value.as_ref() {
        ClientDocument::Object { object: number, now } => object(b, words, *number, *now),
        ClientDocument::Panel { task, procedure: state, result, resources: rows, effects } => {
            number(b, words, Word::Task, *task);
            match state {
                Some(value) => procedure(b, words, value),
                None => text(b, words, Word::ProcedureNone, &[]),
            }
            match result {
                Some(value) => number(b, words, Word::ResultCode, u64::from(*value)),
                None => text(b, words, Word::ResultNone, &[]),
            }
            resources(b, words, rows);
            entries(b, words, effects);
        }
        ClientDocument::Result { task, code } => {
            number(b, words, Word::Task, *task);
            number(b, words, Word::ResultCode, u64::from(*code));
        }
        ClientDocument::Effect { proposal, task, effect: request, description: facts } => {
            number(b, words, Word::Proposal, *proposal);
            number(b, words, Word::Task, *task);
            effect(b, words, request);
            description(b, words, facts);
        }
        ClientDocument::Outbox { entry: outbox, description: facts } => {
            entry(b, words, outbox);
            description(b, words, facts);
        }
        ClientDocument::Hold { task, resources: rows, effects } => {
            number(b, words, Word::Task, *task);
            resources(b, words, rows);
            entries(b, words, effects);
        }
        ClientDocument::Project { project, resources: rows } => {
            number(b, words, Word::ProjectNumber, u64::from(*project));
            resources(b, words, rows);
        }
        ClientDocument::Deployment { outbox, subject_limit, staged_limit, request_budget, system_last_response } => {
            number(b, words, Word::SubjectLimit, u64::from(*subject_limit));
            number(b, words, Word::StagedLimit, u64::from(*staged_limit));
            match request_budget {
                DiagnosticAvailability::NotKept => text(b, words, Word::SystemRequestBudgetNotKept, &[]),
            }
            match system_last_response {
                DiagnosticAvailability::NotKept => text(b, words, Word::LastSystemResponseNotKept, &[]),
            }
            entries(b, words, outbox);
        }
    }
    b.close();
    b.close();
    b.close();
}
/// Render the whole closed terminal; the exact owner refusal stays in its carrier.
pub fn answer(b: &mut Builder, value: &ConnectorAnswer, words: &Words, text_limit: u32) {
    region(b, words, Word::Answer);
    match value.value {
        ApplicationOutcome::Changed { object: number, now } => {
            text(b, words, Word::Changed, &[]);
            object(b, words, number, now);
        }
        ApplicationOutcome::Moved { object: number, now } => {
            text(b, words, Word::Moved, &[]);
            object(b, words, number, now);
        }
        ApplicationOutcome::Role { held } => number(b, words, Word::Role, u64::from(held)),
        ApplicationOutcome::Unknown => text(b, words, Word::Unknown, &[]),
        ApplicationOutcome::Exhausted => text(b, words, Word::Exhausted, &[]),
        ApplicationOutcome::CoreRefused { why } => {
            text(b, words, Word::CoreRefused, &[]);
            match why {
                application_core::Refusal::Role { held } => match held {
                    Some(held) => number(b, words, Word::Role, u64::from(held)),
                    None => words.text(b, jig_client_view::phrases::jig::JigPhrase::NoHeldRole, &[]),
                },
                application_core::Refusal::Authority { .. }
                | application_core::Refusal::Limit { .. }
                | application_core::Refusal::Moved { .. }
                | application_core::Refusal::Ended { .. }
                | application_core::Refusal::Taken { .. }
                | application_core::Refusal::NotWaiting
                | application_core::Refusal::NoFurther
                | application_core::Refusal::Unknown
                | application_core::Refusal::NotOffered
                | application_core::Refusal::KeyConflict => {
                    if let Some(why) = super::connector_values_refusal::refusal(why) {
                        words.refusal(b, &why, text_limit);
                    }
                }
            }
            if let application_core::Refusal::Authority { proposable, .. } = why {
                let phrase = match proposable {
                    Some(true) => Word::ProposableYes,
                    Some(false) => Word::ProposableNo,
                    None => Word::ProposableUnknown,
                };
                text(b, words, phrase, &[]);
            }
        }
    }
    b.close();
}
/// Region plus the largest closed refusal, its actual ending/hold facts and eligibility.
#[must_use]
pub const fn answer_nodes() -> u32 {
    16
}
/// Full structural document envelope; list capacities and all path segments count.
#[must_use]
pub fn document_nodes(limits: &ConnectorLimits) -> Option<u32> {
    let path = limits.segments.checked_mul(2)?.checked_add(1)?;
    let paths = limits.paths.checked_mul(path)?;
    let panel =
        limits.rows.checked_mul(44_u32.checked_add(path)?.checked_add(paths)?)?.checked_add(path)?.checked_add(15)?;
    let outbox = paths.checked_mul(2)?.checked_add(50)?;
    Some(panel.max(outbox).max(7))
}
/// Whole tree requirements, including one caller's enclosing Page and every loaded language.
#[must_use]
pub fn requirements(limits: &ConnectorLimits, words: &Words) -> Option<skein_ui::Limits> {
    let nodes = document_nodes(limits)?.max(answer_nodes()).checked_add(1)?;
    let mut phrase = jig_client_view::literal_segment::max_bytes(limits.segment_bytes)?;
    for word in ALL {
        phrase = phrase.max(words.book().capacity(word.id())?);
    }
    for word in jig_client_view::phrases::jig::ALL {
        phrase = phrase.max(words.book().capacity(word.id())?);
    }
    Some(skein_ui::Limits {
        nodes,
        depth: 12,
        text_per_node: phrase,
        text_bytes: nodes.checked_mul(phrase)?,
        drawing_bytes: 0,
        styles_per_node: 0,
        patches: nodes.checked_mul(5)?,
        bindings: 0,
        slots: 0,
        markdown_depth: 0,
        scene_marks: 0,
        scene_bytes: 0,
    })
}
