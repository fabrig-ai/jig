//! Source-faithful standalone chip metadata (client/domain/objects.md, 3;
//! view.md, 7 and 10). Borrowed normal Words supply every label; opaque
//! channel identity is retained by the contract and never turned into a number.
use crate::JigBindings;
use crate::owner_chip_words::ChipPhrase as P;
use crate::words::Words;
use skein_ui::phrases::Hole;
use skein_ui::{Builder, Role, Run};

use crate::{Borrowed, Limits};
use jig_client::contract::{
    ClosingStage, DocumentText, HoldCategory, HoldFailure, OwnerChipDetails, OwnerChipEnding, OwnerChipHold,
    OwnerChipObservation, OwnerChipWork, OwnerEscalationHolder, OwnerHoldEscalation, OwnerPart, OwnerPartKey,
    OwnerPersonWait, OwnerPriorPhase, OwnerRequester, OwnerResourceLine, OwnerResourceWriter, ResultValue, RetryNotice,
    RunStage,
};
use skein_lib::Wall;

/// Maximum bounded storage for the supplied metadata and its existing result fragments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChipFragment {
    pub nodes: u32,
    pub text_bytes: u32,
    pub text_per_node: u32,
    pub depth: u32,
    pub bindings: u32,
    pub slots: u32,
}
/// Whole bounded metadata, including one terminal result with brief follow-up chips.
#[must_use]
pub fn max_nodes(limits: &Limits) -> Option<u32> {
    120_u32
        .checked_add(crate::proposed_task::part_nodes(limits)?)?
        .checked_add(limits.segments.checked_add(20)?.checked_mul(limits.rows)?)?
        .checked_add(limits.segments.checked_add(6)?.checked_mul(2)?)?
        .checked_add(limits.rows.checked_mul(6)?)?
        .checked_add(crate::result_nodes(limits)?)
}
pub(crate) fn part_fits(part: &OwnerPart, limits: &Limits) -> Option<()> {
    match &part.key {
        OwnerPartKey::Number(_) => Some(()),
        OwnerPartKey::Path(segments) => {
            if segments.capacity() > limits.segments {
                return None;
            }
            for segment in segments {
                if u32::try_from(segment.len()).ok()? > limits.text {
                    return None;
                }
            }
            Some(())
        }
    }
}
fn text_fits(value: &DocumentText, limits: &Limits) -> Option<()> {
    if value.bytes.fits(limits.text) { Some(()) } else { None }
}
pub(crate) fn result_nodes(value: &ResultValue, limits: &Limits) -> Option<u32> {
    match value {
        ResultValue::OwnerVerdict { words, fields, .. } => {
            text_fits(words, limits)?;
            if fields.capacity() > limits.rows {
                return None;
            }
            for field in fields {
                if u32::try_from(field.name.len()).ok()? > limits.text
                    || u32::try_from(field.value.len()).ok()? > limits.text
                {
                    return None;
                }
            }
            crate::result_nodes(limits)
        }
        ResultValue::Verdict { words, fields, tasks, .. } => {
            text_fits(words, limits)?;
            if fields.capacity() > limits.rows || tasks.capacity() > limits.rows {
                return None;
            }
            for field in fields {
                if u32::try_from(field.name.len()).ok()? > limits.text
                    || u32::try_from(field.value.len()).ok()? > limits.text
                {
                    return None;
                }
            }
            for task in tasks {
                if !task.title.bytes.fits(limits.text) {
                    return None;
                }
            }
            crate::result_nodes(limits)
        }
        ResultValue::Connector { summary, part } => {
            text_fits(summary, limits)?;
            if let jig_client::contract::Part::Resource { path, .. } = part
                && !jig_client::contract::resource_path_fits(path, limits.segments, limits.text)
            {
                return None;
            }
            skein_ui::markdown::max_nodes(limits.text)?.checked_add(8)
        }
        ResultValue::Choice { by, reason, .. } => {
            text_fits(reason, limits)?;
            if !by.name.fits(limits.text) {
                return None;
            }
            skein_ui::markdown::max_nodes(limits.text)?.checked_add(8)
        }
        ResultValue::Change { part, words, .. } => {
            text_fits(words, limits)?;
            part_fits(part, limits)?;
            skein_ui::markdown::max_nodes(limits.text)?
                .checked_add(crate::proposed_task::part_nodes(limits)?)?
                .checked_add(4)
        }
        ResultValue::Report(words) | ResultValue::Failure(words) | ResultValue::Words(words) => {
            text_fits(words, limits)?;
            skein_ui::markdown::max_nodes(limits.text)?.checked_add(4)
        }
        ResultValue::Approved { state, words } => {
            if u32::try_from(state.len()).ok()? > limits.text {
                return None;
            }
            if let Some(words) = words {
                text_fits(words, limits)?;
            }
            skein_ui::markdown::max_nodes(limits.text)?.checked_add(4)
        }
    }
}
/// A bounded maximum, not a claim that absent facts generated text. All actual
/// raw parts and observations are validated before this envelope is returned.
/// Existing `ResultValue` cards retain their existing domain/row envelopes.
#[must_use]
pub fn required(value: &OwnerChipDetails, limits: &Limits, words: &Words) -> Option<ChipFragment> {
    if value.resources.capacity() > limits.rows || value.dependencies.capacity() > limits.rows {
        return None;
    }
    for resource in &value.resources {
        part_fits(&resource.part, limits)?;
    }
    if let Some(part) = &value.procedure {
        part_fits(part, limits)?;
    }
    if let Some(hold) = &value.hold {
        if let Some(circle) = &hold.circle
            && circle.tasks.capacity() > limits.rows
        {
            return None;
        }
        if let Some(account) = &hold.words
            && account.bytes.bytes().len() > usize::try_from(limits.text).ok()?
        {
            return None;
        }
        if let Some(part) = &hold.account {
            part_fits(part, limits)?;
        }
        if let Some(part) = &hold.source {
            part_fits(part, limits)?;
        }
        match &hold.escalation {
            Some(OwnerHoldEscalation::Rejected { reason, .. }) => text_fits(reason, limits)?,
            Some(OwnerHoldEscalation::Routing | OwnerHoldEscalation::Waiting { .. }) | None => {}
        }
    }
    if let Some(connectors) = &value.closing_connectors
        && connectors.capacity() > limits.rows
    {
        return None;
    }
    if let Some(observation) = &value.observation {
        match &observation.work {
            OwnerChipWork::Tool { call, tool } => {
                if u32::try_from(call.len()).ok()? > limits.text || u32::try_from(tool.len()).ok()? > limits.text {
                    return None;
                }
            }
            OwnerChipWork::Preparing
            | OwnerChipWork::Starting
            | OwnerChipWork::Active
            | OwnerChipWork::Writing
            | OwnerChipWork::Waiting
            | OwnerChipWork::Ending => {}
        }
    }
    let resource_nodes = limits.segments.checked_add(20)?.checked_mul(value.resources.len())?;
    let mut nodes = 120_u32
        .checked_add(resource_nodes)?
        .checked_add(limits.segments.checked_add(6)?.checked_mul(2)?)?
        .checked_add(limits.rows)?;
    if let Some(ending) = &value.ending {
        match ending {
            OwnerChipEnding::Done(result) => nodes = nodes.checked_add(result_nodes(result, limits)?)?,
            OwnerChipEnding::Failed(reason) => text_fits(reason, limits)?,
            OwnerChipEnding::Cancelled { reason, result, .. } => {
                text_fits(reason, limits)?;
                if let Some(result) = result {
                    nodes = nodes.checked_add(result_nodes(result, limits)?)?;
                }
            }
        }
    }
    let phrase_bytes = words.jig_capacity()?;
    // A card node can carry a title and text together; each literal byte can
    // need four escaped display bytes. Both costs are included, never narrowed.
    let text_per_node = crate::literal_segment::max_bytes(limits.text)?.max(phrase_bytes).checked_mul(2)?;
    Some(ChipFragment {
        nodes,
        text_bytes: nodes.checked_mul(text_per_node)?,
        text_per_node,
        depth: 20,
        bindings: nodes,
        slots: nodes,
    })
}
fn text(b: &mut Builder, words: &Words, phrase: P, holes: &[Hole]) {
    let saying =
        words.book().say(words.language(), phrase.id(), holes).expect("checked chip words and exact typed holes");
    b.text(Run::Plain, &saying.text);
}
fn number(b: &mut Builder, words: &Words, phrase: P, value: u64) {
    text(b, words, phrase, &[Hole::Number(value)]);
}
fn wall(b: &mut Builder, words: &Words, phrase: P, at: Wall) {
    text(b, words, phrase, &[Hole::Wall(words.adjusted_wall(at))]);
}
fn document(b: &mut Builder, value: &DocumentText, words: &Words) {
    crate::literal_segment::render(b, value.bytes());
    if value.cut {
        text(b, words, P::Cut, &[]);
    }
}
pub(crate) fn resource(b: &mut Builder, value: &OwnerResourceLine, words: &Words, bindings: &mut JigBindings) {
    b.open(Role::Region);
    let name = words
        .book()
        .say(words.language(), P::Kind.id(), &[Hole::Number(u64::from(value.kind))])
        .expect("checked resource kind and exact supplied number");
    b.name(&name.text);
    crate::proposed_task::part(b, &value.part, words, bindings);
    number(b, words, P::Kind, u64::from(value.kind));
    text(b, words, if value.taken { P::Taken } else { P::NotTaken }, &[]);
    if let Some(holder) = value.holder {
        number(b, words, P::Holder, holder);
    }
    match value.writer {
        Some(OwnerResourceWriter::Run { task, attempt }) => {
            number(b, words, P::WriterTask, task);
            number(b, words, P::WriterAttempt, attempt);
        }
        Some(OwnerResourceWriter::Effect { entry }) => number(b, words, P::WriterEffect, entry),
        None => {}
    }
    text(b, words, if value.writer_lost { P::WriterLost } else { P::WriterKept }, &[]);
    b.close();
}
fn person_wait(b: &mut Builder, value: &OwnerPersonWait, words: &Words) {
    wall(b, words, P::WaitSince, value.since);
    text(b, words, P::Stall, &[Hole::Span(value.stall)]);
    wall(b, words, P::Deadline, value.deadline);
    if let Some(notice) = &value.notice {
        number(b, words, P::NoticeMessage, notice.message);
        wall(b, words, P::NoticeAt, notice.at);
        number(b, words, P::NoticeActivity, notice.activity);
    }
}
pub(crate) fn hold_failure(b: &mut Builder, value: HoldFailure, words: &Words) {
    let phrase = match value {
        HoldFailure::Transient => P::Transient,
        HoldFailure::Permanent => P::Permanent,
        HoldFailure::Run => P::RunFailure,
        HoldFailure::Agent => P::AgentFailure,
        HoldFailure::Lost => P::Lost,
        HoldFailure::Invalid => P::InvalidFailure,
    };
    text(b, words, phrase, &[]);
}
pub(crate) fn reason(b: &mut Builder, value: HoldCategory, words: &Words) {
    match value {
        HoldCategory::Exceeded(_) => words.hold_category(b, value),
        HoldCategory::Failures { class } => {
            text(b, words, P::Failures, &[]);
            hold_failure(b, class, words);
        }
        HoldCategory::Stalled => text(b, words, P::Stalled, &[]),
        HoldCategory::Dependency { task } => number(b, words, P::Dependency, task),
        HoldCategory::Stopped => text(b, words, P::Stopped, &[]),
        HoldCategory::Placed { .. } => text(b, words, P::Placed, &[]),
        HoldCategory::Drift => text(b, words, P::Drift, &[]),
        HoldCategory::Effects => text(b, words, P::Effects, &[]),
        HoldCategory::EffectFailed => text(b, words, P::EffectFailed, &[]),
        HoldCategory::Uncertain { entry } => number(b, words, P::Uncertain, entry),
        HoldCategory::Budget => text(b, words, P::Budget, &[]),
        HoldCategory::Deadline => text(b, words, P::HoldDeadline, &[]),
        HoldCategory::HoldsWaited => text(b, words, P::HoldsWaited, &[]),
        HoldCategory::ResourceUnavailable => text(b, words, P::ResourceUnavailable, &[]),
        HoldCategory::ResourceAccess => text(b, words, P::ResourceAccess, &[]),
        HoldCategory::PoolLost { pool } => number(b, words, P::PoolLost, pool),
        HoldCategory::Circle => words.text(b, crate::phrases::jig::JigPhrase::CircleHold, &[]),
        HoldCategory::Procedure => text(b, words, P::HoldProcedure, &[]),
    }
}
pub(crate) fn prior(b: &mut Builder, value: OwnerPriorPhase, words: &Words) {
    match value {
        OwnerPriorPhase::Waiting => text(b, words, P::PriorWaiting, &[]),
        OwnerPriorPhase::Active(stage) => match stage {
            RunStage::Idle => text(b, words, P::PriorIdle, &[]),
            RunStage::Due => text(b, words, P::PriorDue, &[]),
            RunStage::Preparing => text(b, words, P::PriorPreparing, &[]),
            RunStage::Claimed { attempt } => number(b, words, P::PriorClaimed, attempt),
            RunStage::Running { attempt } => number(b, words, P::PriorRunning, attempt),
            RunStage::BackingOff { until } => wall(b, words, P::PriorBackingOff, until),
        },
        OwnerPriorPhase::Closing(stage) => match stage {
            ClosingStage::Run { attempt } => number(b, words, P::PriorClosingRun, attempt),
            ClosingStage::Delegates => text(b, words, P::PriorDelegates, &[]),
            ClosingStage::Effects => text(b, words, P::PriorEffects, &[]),
            ClosingStage::Releases => text(b, words, P::PriorReleases, &[]),
            ClosingStage::Settled => text(b, words, P::PriorSettled, &[]),
        },
    }
}
fn escalation(b: &mut Builder, value: &OwnerHoldEscalation, words: &Words) {
    match value {
        OwnerHoldEscalation::Routing => text(b, words, P::Routing, &[]),
        OwnerHoldEscalation::Waiting { holder, entry, since } => {
            match holder {
                OwnerEscalationHolder::Task(task) => number(b, words, P::EscalationTask, *task),
                OwnerEscalationHolder::Person(person) => number(b, words, P::EscalationPerson, *person),
                OwnerEscalationHolder::Role { project, role } => {
                    number(b, words, P::EscalationProject, u64::from(*project));
                    number(b, words, P::EscalationRole, u64::from(*role));
                }
            }
            number(b, words, P::Entry, *entry);
            wall(b, words, P::EscalationSince, *since);
        }
        OwnerHoldEscalation::Rejected { by, reason } => {
            number(b, words, P::RejectedBy, *by);
            document(b, reason, words);
        }
    }
}
pub(crate) fn hold(b: &mut Builder, value: &OwnerChipHold, words: &Words, bindings: &mut JigBindings) {
    if let Some(account) = &value.account {
        crate::proposed_task::part(b, account, words, bindings);
    }
    if let Some(source) = &value.source {
        crate::proposed_task::part(b, source, words, bindings);
    }
    reason(b, value.reason, words);
    if let Some(account) = &value.words {
        document(b, account, words);
    }
    if let Some(circle) = &value.circle {
        for task in &circle.tasks {
            words.text(b, crate::phrases::jig::JigPhrase::CircleWaitsFor, &[Hole::Number(*task)]);
        }
        words.text(b, crate::phrases::jig::JigPhrase::CircleLifts, &[]);
        if circle.cut {
            words.text(b, crate::phrases::jig::JigPhrase::TaskListsCut, &[]);
        }
    }
    number(b, words, P::Revision, value.revision);
    prior(b, value.before, words);
    if let Some(value) = &value.escalation {
        escalation(b, value, words);
    }
    for pair in [
        (P::TransientTries, value.tries.transient),
        (P::PermanentTries, value.tries.permanent),
        (P::RunTries, value.tries.run),
        (P::AgentTries, value.tries.agent),
        (P::LostTries, value.tries.lost),
        (P::InvalidTries, value.tries.invalid),
    ] {
        number(b, words, pair.0, u64::from(pair.1));
    }
    if let Some(at) = value.hold_wait_since {
        wall(b, words, P::HoldWaitSince, at);
    }
}
/// Maximum leaves for the supplied work report: three tool leaves and three
/// independent clock leaves. No other metadata or inferred work is included.
#[must_use]
pub const fn observation_nodes() -> u32 {
    6
}

/// Validate the independent work report and its selected checked sayings.
/// The byte envelope also permits a name and text on each leaf; the current
/// renderer emits only text. It carries no bindings, slots or nested cards.
#[must_use]
pub fn observation_required(value: &OwnerChipObservation, limits: &Limits, words: &Words) -> Option<ChipFragment> {
    match &value.work {
        OwnerChipWork::Tool { call, tool } => {
            if u32::try_from(call.len()).ok()? > limits.text || u32::try_from(tool.len()).ok()? > limits.text {
                return None;
            }
        }
        OwnerChipWork::Preparing
        | OwnerChipWork::Starting
        | OwnerChipWork::Active
        | OwnerChipWork::Writing
        | OwnerChipWork::Waiting
        | OwnerChipWork::Ending => {}
    }
    let mut phrase_bytes = 0;
    for phrase in [
        P::Preparing,
        P::Starting,
        P::Active,
        P::Writing,
        P::Tool,
        P::Waiting,
        P::Ending,
        P::ObservedSince,
        P::ReportedAt,
        P::Started,
        P::StartUnknown,
    ] {
        phrase_bytes = phrase_bytes.max(words.book().capacity(phrase.id())?);
    }
    let nodes = observation_nodes();
    let text_per_node = crate::literal_segment::max_bytes(limits.text)?.max(phrase_bytes).checked_mul(2)?;
    Some(ChipFragment {
        nodes,
        text_bytes: nodes.checked_mul(text_per_node)?,
        text_per_node,
        depth: 1,
        bindings: 0,
        slots: 0,
    })
}

/// Show only the supplied observation, preserving all seven work kinds and
/// each independent clock. Channel identity remains an opaque contract fact.
pub fn observation(b: &mut Builder, value: &OwnerChipObservation, words: &Words) {
    match &value.work {
        OwnerChipWork::Preparing => text(b, words, P::Preparing, &[]),
        OwnerChipWork::Starting => text(b, words, P::Starting, &[]),
        OwnerChipWork::Active => text(b, words, P::Active, &[]),
        OwnerChipWork::Writing => text(b, words, P::Writing, &[]),
        OwnerChipWork::Tool { call, tool } => {
            text(b, words, P::Tool, &[]);
            crate::literal_segment::render(b, call);
            crate::literal_segment::render(b, tool);
        }
        OwnerChipWork::Waiting => text(b, words, P::Waiting, &[]),
        OwnerChipWork::Ending => text(b, words, P::Ending, &[]),
    }
    wall(b, words, P::ObservedSince, value.since);
    wall(b, words, P::ReportedAt, value.reported_at);
    match value.started {
        Some(at) => wall(b, words, P::Started, at),
        None => text(b, words, P::StartUnknown, &[]),
    }
}
fn result_value(
    b: &mut Builder,
    value: &ResultValue,
    client: &Borrowed<'_>,
    words: &Words,
    bindings: &mut JigBindings,
) {
    match value {
        ResultValue::Approved { state, words: explanation } => {
            words.text(b, crate::phrases::jig::JigPhrase::ApprovalExpected, &[]);
            crate::literal_segment::render(b, state);
            if let Some(explanation) = explanation {
                document(b, explanation, words);
            }
        }
        ResultValue::Report(_)
        | ResultValue::Words(_)
        | ResultValue::Failure(_)
        | ResultValue::Verdict { .. }
        | ResultValue::OwnerVerdict { .. }
        | ResultValue::Connector { .. }
        | ResultValue::Change { .. }
        | ResultValue::Choice { .. } => crate::cards::result::value_nodes(b, client, value, words, bindings),
    }
}
/// The departure caption and its supplied reason, author and time.
/// The complete result payload is rendered by the document's content below this state.
pub(crate) fn ending_summary(b: &mut Builder, value: &OwnerChipEnding, words: &Words) {
    match value {
        OwnerChipEnding::Done(_) => text(b, words, P::Done, &[]),
        OwnerChipEnding::Failed(reason) => {
            text(b, words, P::Failed, &[]);
            document(b, reason, words);
        }
        OwnerChipEnding::Cancelled { reason, by, at, .. } => {
            text(b, words, P::Cancelled, &[]);
            document(b, reason, words);
            match by {
                OwnerRequester::Task(task) => number(b, words, P::RequesterTask, *task),
                OwnerRequester::Person(person) => number(b, words, P::RequesterPerson, *person),
                OwnerRequester::Deployment { project } => number(b, words, P::RequesterDeployment, u64::from(*project)),
            }
            wall(b, words, P::CancelledAt, *at);
        }
    }
}
pub(crate) fn ending(
    b: &mut Builder,
    value: &OwnerChipEnding,
    client: &Borrowed<'_>,
    words: &Words,
    bindings: &mut JigBindings,
) {
    ending_summary(b, value, words);
    match value {
        OwnerChipEnding::Done(result) => result_value(b, result, client, words, bindings),
        OwnerChipEnding::Failed(_) => {}
        OwnerChipEnding::Cancelled { result, .. } => {
            if let Some(result) = result {
                result_value(b, result, client, words, bindings);
            }
        }
    }
}
/// Render metadata without changing domain state, routing or issuing any read.
/// A borrowed `Ui` is used only by the existing supplied `ResultValue` fragment.
pub fn render(
    b: &mut Builder,
    value: &OwnerChipDetails,
    client: &Borrowed<'_>,
    words: &Words,
    bindings: &mut JigBindings,
) {
    b.open(Role::Region);
    let name = words
        .book()
        .say(words.language(), P::Project.id(), &[Hole::Number(u64::from(value.project))])
        .expect("checked chip project and exact supplied number");
    b.name(&name.text);
    number(b, words, P::Project, u64::from(value.project));
    crate::proposed_task::executor(b, value.executor, words);
    wall(b, words, P::PhaseSince, value.phase_since);
    if let Some(at) = value.run_since {
        wall(b, words, P::Placement, at);
    }
    if let Some(attempt) = value.attempt {
        number(b, words, P::Attempt, attempt);
    }
    match value.retry {
        Some(RetryNotice::Failed { class, count, allowance }) => {
            crate::cards::chip::failure(b, class, words);
            words.text(
                b,
                crate::phrases::jig::JigPhrase::RunRetryFailed,
                &[Hole::Number(u64::from(count)), Hole::Number(u64::from(allowance))],
            );
        }
        Some(RetryNotice::OwnerFailed { class, count, allowance }) => {
            hold_failure(b, class, words);
            words.text(
                b,
                crate::phrases::jig::JigPhrase::RunRetryFailed,
                &[Hole::Number(u64::from(count)), Hole::Number(u64::from(allowance))],
            );
        }
        Some(RetryNotice::Refused { count }) => {
            words.text(b, crate::phrases::jig::JigPhrase::RunRetryRefused, &[Hole::Number(u64::from(count))]);
        }
        None => {}
    }
    crate::owner_wake::policy(b, &value.wake, words.book(), words.language());
    if let Some(wait) = &value.person_wait {
        person_wait(b, wait, words);
    }
    if let Some(value) = &value.hold {
        hold(b, value, words, bindings);
    }
    crate::owner_budget::render(b, &value.budget, words);
    number(b, words, P::Activity, value.activity);
    text(b, words, if value.resources_observed { P::ResourcesObserved } else { P::ResourcesUnobserved }, &[]);
    for value in &value.resources {
        resource(b, value, words, bindings);
    }
    if value.resources_cut {
        text(b, words, P::ResourcesCut, &[]);
    }
    if let Some(part) = &value.procedure {
        text(b, words, P::Procedure, &[]);
        crate::proposed_task::part(b, part, words, bindings);
    }
    number(b, words, P::LiveDelegates, u64::from(value.live_delegates));
    match &value.closing_connectors {
        None => text(b, words, P::ConnectorsUnknown, &[]),
        Some(connectors) => {
            if connectors.is_empty() {
                text(b, words, P::ConnectorsEmpty, &[]);
            }
            for connector in connectors {
                number(b, words, P::Connector, u64::from(*connector));
            }
        }
    }
    if let Some(value) = &value.ending {
        ending(b, value, client, words, bindings);
    }
    if let Some(at) = value.ended_at {
        wall(b, words, P::EndedAt, at);
    }
    match &value.observation {
        Some(value) => observation(b, value, words),
        None => text(b, words, P::Unobserved, &[]),
    }
    b.close();
}
