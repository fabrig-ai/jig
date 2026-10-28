//! Durable task values translated exhaustively (protocol/store.md, 5.1).
use super::{activity, index, phase, proposals, state, value};
use crate::tasks_v9 as wire;
use alloc::boxed::Box;
use jig_domain_tasks as domain;
use skein_lib::{List, Wall};

fn optional_grace(source: Option<&domain::Grace>, limits: &wire::Limits) -> Result<Option<wire::Grace>, wire::Problem> {
    match source {
        Some(source) => Ok(Some(phase::grace(source, limits)?)),
        None => Ok(None),
    }
}
fn read_optional_grace(
    source: Option<wire::Grace>,
    limits: &wire::Limits,
) -> Result<Option<domain::Grace>, wire::Problem> {
    match source {
        Some(source) => Ok(Some(phase::read_grace(source, limits)?)),
        None => Ok(None),
    }
}

fn optional_retry_notice(
    source: Option<&domain::RetryNotice>,
    limits: &wire::Limits,
) -> Result<Option<wire::RetryNotice>, wire::Problem> {
    match source {
        Some(source) => Ok(Some(phase::retry_notice(source, limits)?)),
        None => Ok(None),
    }
}
fn read_optional_retry_notice(
    source: Option<wire::RetryNotice>,
    limits: &wire::Limits,
) -> Result<Option<domain::RetryNotice>, wire::Problem> {
    match source {
        Some(source) => Ok(Some(phase::read_retry_notice(source, limits)?)),
        None => Ok(None),
    }
}

fn optional_proposal(
    source: Option<&domain::Proposal>,
    limits: &wire::Limits,
    moved: Option<&[u64; 64]>,
) -> Result<Option<wire::Proposal>, wire::Problem> {
    match source {
        Some(source) => Ok(Some(proposals::proposal(source, limits, moved)?)),
        None => Ok(None),
    }
}
fn read_optional_proposal(
    source: Option<wire::Proposal>,
    limits: &wire::Limits,
) -> Result<Option<Box<domain::Proposal>>, wire::Problem> {
    match source {
        Some(source) => Ok(Some(Box::new(proposals::read_proposal(source, limits)?))),
        None => Ok(None),
    }
}

fn optional_person_answer(
    source: Option<&domain::PersonAnswer>,
    limits: &wire::Limits,
) -> Result<Option<wire::PersonAnswer>, wire::Problem> {
    match source {
        Some(source) => Ok(Some(state::person_answer(source, limits)?)),
        None => Ok(None),
    }
}
fn read_optional_person_answer(
    source: Option<wire::PersonAnswer>,
    limits: &wire::Limits,
) -> Result<Option<domain::PersonAnswer>, wire::Problem> {
    match source {
        Some(source) => Ok(Some(state::read_person_answer(source, limits)?)),
        None => Ok(None),
    }
}

fn optional_person_wait(
    source: Option<&domain::PersonWait>,
    limits: &wire::Limits,
) -> Result<Option<wire::PersonWait>, wire::Problem> {
    match source {
        Some(source) => Ok(Some(state::person_wait(source, limits)?)),
        None => Ok(None),
    }
}
fn read_optional_person_wait(
    source: Option<wire::PersonWait>,
    limits: &wire::Limits,
) -> Result<Option<domain::PersonWait>, wire::Problem> {
    match source {
        Some(source) => Ok(Some(state::read_person_wait(source, limits)?)),
        None => Ok(None),
    }
}

fn optional_recurring(
    source: Option<&domain::RecurringState>,
    limits: &wire::Limits,
    moved: Option<&[u64; 64]>,
) -> Result<Option<wire::RecurringState>, wire::Problem> {
    match source {
        Some(source) => Ok(Some(state::recurring_state(source, limits, moved)?)),
        None => Ok(None),
    }
}
fn read_optional_recurring(
    source: Option<wire::RecurringState>,
    limits: &wire::Limits,
) -> Result<Option<Box<domain::RecurringState>>, wire::Problem> {
    match source {
        Some(source) => Ok(Some(Box::new(state::read_recurring_state(source, limits)?))),
        None => Ok(None),
    }
}

fn optional_invalid(source: Option<domain::InvalidResult>) -> Option<wire::InvalidResult> {
    let source = source?;
    Some(state::invalid_result(source))
}
fn read_optional_invalid(source: Option<wire::InvalidResult>) -> Option<domain::InvalidResult> {
    let source = source?;
    Some(state::read_invalid_result(source))
}

fn question_lines(
    source: &[domain::QuestionLine],
    limits: &wire::Limits,
) -> Result<List<wire::QuestionLine>, wire::Problem> {
    let mut members = List::with_capacity(limits.task_history_question_lines);
    for member in source {
        let member = index::question_line(member, limits)?;
        if members.push(member).is_err() {
            return Err(problem(wire::Path::TaskHistoryQuestionLines));
        }
    }
    Ok(members)
}
fn read_question_lines(
    source: List<wire::QuestionLine>,
    limits: &wire::Limits,
) -> Result<Box<[domain::QuestionLine]>, wire::Problem> {
    let mut members = List::with_capacity(source.len());
    for member in source.into_boxed() {
        members.push(index::read_question_line(member, limits)?).expect("same number of members");
    }
    Ok(members.into_boxed())
}

fn proposal_rows(
    source: &[domain::ProposalRow],
    limits: &wire::Limits,
) -> Result<List<wire::ProposalRow>, wire::Problem> {
    let mut members = List::with_capacity(limits.task_history_proposal_rows);
    for member in source {
        let member = index::proposal_row(member, limits)?;
        if members.push(member).is_err() {
            return Err(problem(wire::Path::TaskHistoryProposalRows));
        }
    }
    Ok(members)
}
fn read_proposal_rows(
    source: List<wire::ProposalRow>,
    limits: &wire::Limits,
) -> Result<Box<[domain::ProposalRow]>, wire::Problem> {
    let mut members = List::with_capacity(source.len());
    for member in source.into_boxed() {
        members.push(index::read_proposal_row(member, limits)?).expect("same number of members");
    }
    Ok(members.into_boxed())
}

fn replies(source: &[domain::ReplyHint], limits: &wire::Limits) -> Result<List<wire::ReplyHint>, wire::Problem> {
    let mut members = List::with_capacity(limits.task_run_replies);
    for member in source {
        let member = state::reply_hint(member, limits)?;
        if members.push(member).is_err() {
            return Err(problem(wire::Path::TaskRunReplies));
        }
    }
    Ok(members)
}
fn read_replies(
    source: List<wire::ReplyHint>,
    limits: &wire::Limits,
) -> Result<Box<[domain::ReplyHint]>, wire::Problem> {
    let mut members = List::with_capacity(source.len());
    for member in source.into_boxed() {
        members.push(state::read_reply_hint(member, limits)?).expect("same number of members");
    }
    Ok(members.into_boxed())
}

fn holdings(source: &[domain::Holding], limits: &wire::Limits) -> Result<List<wire::Holding>, wire::Problem> {
    let mut members = List::with_capacity(limits.task_interests_holdings);
    for member in source {
        let member = phase::holding(member, limits)?;
        if members.push(member).is_err() {
            return Err(problem(wire::Path::TaskInterestsHoldings));
        }
    }
    Ok(members)
}
fn read_holdings(source: List<wire::Holding>, limits: &wire::Limits) -> Result<Box<[domain::Holding]>, wire::Problem> {
    let mut members = List::with_capacity(source.len());
    for member in source.into_boxed() {
        members.push(phase::read_holding(member, limits)?).expect("same number of members");
    }
    Ok(members.into_boxed())
}

fn questions(
    source: &[domain::QuestionCredit],
    limits: &wire::Limits,
) -> Result<List<wire::QuestionCredit>, wire::Problem> {
    let mut members = List::with_capacity(limits.task_interests_questions);
    for member in source {
        let member = state::question_credit(member, limits)?;
        if members.push(member).is_err() {
            return Err(problem(wire::Path::TaskInterestsQuestions));
        }
    }
    Ok(members)
}
fn read_questions(
    source: List<wire::QuestionCredit>,
    limits: &wire::Limits,
) -> Result<Box<[domain::QuestionCredit]>, wire::Problem> {
    let mut members = List::with_capacity(source.len());
    for member in source.into_boxed() {
        members.push(state::read_question_credit(member, limits)?).expect("same number of members");
    }
    Ok(members.into_boxed())
}

fn subscriptions(
    source: &[domain::Subscription],
    limits: &wire::Limits,
) -> Result<List<wire::Subscription>, wire::Problem> {
    let mut members = List::with_capacity(limits.task_interests_subscriptions);
    for member in source {
        let member = state::subscription(member, limits)?;
        if members.push(member).is_err() {
            return Err(problem(wire::Path::TaskInterestsSubscriptions));
        }
    }
    Ok(members)
}
fn read_subscriptions(
    source: List<wire::Subscription>,
    limits: &wire::Limits,
) -> Result<Box<[domain::Subscription]>, wire::Problem> {
    let mut members = List::with_capacity(source.len());
    for member in source.into_boxed() {
        members.push(state::read_subscription(member, limits)?).expect("same number of members");
    }
    Ok(members.into_boxed())
}

fn inbox(source: &[domain::Word], limits: &wire::Limits) -> Result<List<wire::Word>, wire::Problem> {
    let mut members = List::with_capacity(limits.task_interests_inbox);
    for member in source {
        let member = activity::word(member, limits)?;
        if members.push(member).is_err() {
            return Err(problem(wire::Path::TaskInterestsInbox));
        }
    }
    Ok(members)
}
fn read_inbox(source: List<wire::Word>, limits: &wire::Limits) -> Result<Box<[domain::Word]>, wire::Problem> {
    let mut members = List::with_capacity(source.len());
    for member in source.into_boxed() {
        members.push(activity::read_word(member, limits)?).expect("same number of members");
    }
    Ok(members.into_boxed())
}

fn saved(source: &[domain::SavedResource], limits: &wire::Limits) -> Result<List<wire::SavedResource>, wire::Problem> {
    let mut members = List::with_capacity(limits.task_interests_saved);
    for member in source {
        let member = state::saved_resource(member, limits)?;
        if members.push(member).is_err() {
            return Err(problem(wire::Path::TaskInterestsSaved));
        }
    }
    Ok(members)
}
fn read_saved(
    source: List<wire::SavedResource>,
    limits: &wire::Limits,
) -> Result<Box<[domain::SavedResource]>, wire::Problem> {
    let mut members = List::with_capacity(source.len());
    for member in source.into_boxed() {
        members.push(state::read_saved_resource(member, limits)?).expect("same number of members");
    }
    Ok(members.into_boxed())
}

/// Translate every retained task field.
pub fn task_record(
    source: &domain::TaskRecord,
    limits: &wire::Limits,
    moved: Option<&[u64; 64]>,
) -> Result<wire::TaskRecord, wire::Problem> {
    wire::TaskRecord::new(
        limits,
        wire::TaskRecordParts {
            history: task_history(source, limits)?,
            run: task_run(source, limits, moved)?,
            definition: task_definition(source, limits, moved)?,
            interests: task_interests(source, limits)?,
            outcome: task_outcome(source, limits)?,
        },
    )
}

fn task_history(source: &domain::TaskRecord, limits: &wire::Limits) -> Result<wire::TaskHistory, wire::Problem> {
    wire::TaskHistory::new(
        limits,
        wire::TaskHistoryParts {
            last_activity: source.last_activity.as_nanos(),
            ended_below: index::endings(&source.ended_below, limits)?,
            question_lines: question_lines(&source.question_lines, limits)?,
            hold: source.hold,
            placed: source.placed,
            proposal_rows: proposal_rows(&source.proposal_rows, limits)?,
            instructions: source.instructions.clone(),
            refused: refused(&source.refused, limits)?,
            grace: optional_grace(source.grace.as_ref(), limits)?,
            settings: wire::SettingsRevision::new(
                limits,
                wire::SettingsRevisionParts { task: source.settings.task, run: source.settings.run },
            )?,
            revisions: phase::revisions(&source.revisions, limits)?,
            created_at: source.created_at.as_nanos(),
            changed: source.changed.as_nanos(),
            phase_since: source.phase_since.as_nanos(),
            run_since: value::wall(source.run_since),
            retry_notice: optional_retry_notice(source.retry_notice.as_ref(), limits)?,
            ended_at: value::wall(source.ended_at),
            revision: source.revision,
        },
    )
}

fn task_run(
    source: &domain::TaskRecord,
    limits: &wire::Limits,
    moved: Option<&[u64; 64]>,
) -> Result<wire::TaskRun, wire::Problem> {
    wire::TaskRun::new(
        limits,
        wire::TaskRunParts {
            milestone: source.milestone,
            activity: source.activity,
            due_reason: value::why(source.due_reason, limits)?,
            narrowing: source.narrowing,
            result_position: source.result_position,
            last_reply: source.last_reply,
            replies: replies(&source.replies, limits)?,
            escalation: state::escalation(&source.escalation, limits)?,
            proposal: optional_proposal(source.proposal.as_deref(), limits, moved)?,
            result_proposal: source.result_proposal,
            number: source.number,
            project: source.project,
            requester: value::party(&source.requester, limits)?,
            run_spent: source.run_spent,
            run_reserved: source.run_reserved,
        },
    )
}

fn task_definition(
    source: &domain::TaskRecord,
    limits: &wire::Limits,
    moved: Option<&[u64; 64]>,
) -> Result<wire::TaskDefinition, wire::Problem> {
    wire::TaskDefinition::new(
        limits,
        wire::TaskDefinitionParts {
            label: source.label,
            flags: phase::flags(source.flags, limits)?,
            root: source.root,
            depth: source.depth,
            executor: value::executor(source.executor, limits)?,
            llm: value::llm(source.llm.as_ref(), limits)?,
            kept_account: source.kept_account,
            taken_by: source.taken_by,
            person_answer: optional_person_answer(source.person_answer.as_ref(), limits)?,
            person_wait: optional_person_wait(source.person_wait.as_ref(), limits)?,
            recurring: optional_recurring(source.recurring.as_deref(), limits, moved)?,
            tracked: source.tracked,
            spec: value::spec(&source.spec, limits)?,
            contract: value::contract(&source.contract, limits)?,
            authority: value::authority(&source.authority, limits, moved)?,
            numbers: phase::numbers(&source.numbers, limits)?,
            funder: value::funder(source.funder, limits)?,
            dependencies: dependencies(&source.dependencies, limits)?,
            waiting_on: waiting_on(&source.waiting_on, limits)?,
            added: added(&source.added, limits)?,
        },
    )
}

fn task_interests(source: &domain::TaskRecord, limits: &wire::Limits) -> Result<wire::TaskInterests, wire::Problem> {
    wire::TaskInterests::new(
        limits,
        wire::TaskInterestsParts {
            holdings: holdings(&source.holdings, limits)?,
            holds_taken: source.holds_taken,
            hold_wait_since: value::wall(source.hold_wait_since),
            hold_bound_since: value::wall(source.hold_bound_since),
            delegates: delegates(&source.delegates, limits)?,
            result_followups: result_followups(&source.result_followups, limits)?,
            references: references(&source.references, limits)?,
            questions: questions(&source.questions, limits)?,
            subscriptions: subscriptions(&source.subscriptions, limits)?,
            wake: phase::wake(&source.wake, limits)?,
            turn: source.turn,
            last_message: source.last_message,
            inbox: inbox(&source.inbox, limits)?,
            saved: saved(&source.saved, limits)?,
            ever_turned: wire::TranscriptState::new(
                limits,
                wire::TranscriptStateParts { ever_turned: source.ever_turned },
            )?,
            made: source.made,
        },
    )
}

fn task_outcome(source: &domain::TaskRecord, limits: &wire::Limits) -> Result<wire::TaskOutcome, wire::Problem> {
    wire::TaskOutcome::new(
        limits,
        wire::TaskOutcomeParts {
            attempt: source.attempt,
            last_answer: source.last_answer,
            tries: value::tries(&source.tries, limits)?,
            invalid_result: optional_invalid(source.invalid_result),
            refusals: source.refusals,
            phase: phase::phase(&source.phase, limits)?,
            room_wait: room_wait(source.room_wait, limits)?,
        },
    )
}
/// Read every retained task field.
pub fn read_task_record(source: wire::TaskRecord, limits: &wire::Limits) -> Result<domain::TaskRecord, wire::Problem> {
    let wire::TaskRecordParts { history, run, definition, interests, outcome } = source.into_parts();
    let history = history.into_parts();
    let run = run.into_parts();
    let definition = definition.into_parts();
    let interests = interests.into_parts();
    let outcome = outcome.into_parts();
    Ok(domain::TaskRecord {
        room_wait: read_room_wait(outcome.room_wait),
        label: definition.label,
        flags: phase::read_flags(definition.flags),
        last_activity: Wall::from_nanos(history.last_activity),
        ended_below: index::read_endings(history.ended_below, limits)?,
        question_lines: read_question_lines(history.question_lines, limits)?,
        hold: history.hold,
        placed: history.placed,
        proposal_rows: read_proposal_rows(history.proposal_rows, limits)?,
        instructions: history.instructions,
        refused: phase::read_references(history.refused),
        grace: read_optional_grace(history.grace, limits)?,
        settings: domain::SettingsRevision { task: history.settings.task(), run: *history.settings.run() },
        revisions: phase::read_revisions(history.revisions, limits)?,
        created_at: Wall::from_nanos(history.created_at),
        changed: Wall::from_nanos(history.changed),
        phase_since: Wall::from_nanos(history.phase_since),
        run_since: value::read_wall(history.run_since),
        retry_notice: read_optional_retry_notice(history.retry_notice, limits)?,
        ended_at: value::read_wall(history.ended_at),
        revision: history.revision,
        milestone: run.milestone,
        activity: run.activity,
        due_reason: value::read_why(run.due_reason),
        narrowing: run.narrowing,
        result_position: run.result_position,
        last_reply: run.last_reply,
        replies: read_replies(run.replies, limits)?,
        escalation: state::read_escalation(run.escalation, limits)?,
        proposal: read_optional_proposal(run.proposal, limits)?,
        result_proposal: run.result_proposal,
        number: run.number,
        project: run.project,
        requester: value::read_party(run.requester),
        run_spent: run.run_spent,
        run_reserved: run.run_reserved,
        root: definition.root,
        depth: definition.depth,
        executor: value::read_executor(definition.executor),
        llm: value::read_llm(definition.llm),
        kept_account: definition.kept_account,
        taken_by: definition.taken_by,
        person_answer: read_optional_person_answer(definition.person_answer, limits)?,
        person_wait: read_optional_person_wait(definition.person_wait, limits)?,
        recurring: read_optional_recurring(definition.recurring, limits)?,
        tracked: definition.tracked,
        spec: value::read_spec(definition.spec, limits)?,
        contract: value::read_contract(definition.contract, limits)?,
        authority: value::read_authority(definition.authority, limits)?,
        numbers: phase::read_numbers(definition.numbers),
        funder: value::read_funder(definition.funder),
        dependencies: phase::read_references(definition.dependencies),
        waiting_on: phase::read_references(definition.waiting_on),
        added: read_added(definition.added),
        holdings: read_holdings(interests.holdings, limits)?,
        holds_taken: interests.holds_taken,
        hold_wait_since: value::read_wall(interests.hold_wait_since),
        hold_bound_since: value::read_wall(interests.hold_bound_since),
        delegates: phase::read_references(interests.delegates),
        result_followups: phase::read_references(interests.result_followups),
        references: phase::read_references(interests.references),
        questions: read_questions(interests.questions, limits)?,
        subscriptions: read_subscriptions(interests.subscriptions, limits)?,
        wake: phase::read_wake(interests.wake, limits)?,
        turn: interests.turn,
        last_message: interests.last_message,
        inbox: read_inbox(interests.inbox, limits)?,
        saved: read_saved(interests.saved, limits)?,
        ever_turned: interests.ever_turned.into_parts().ever_turned,
        made: interests.made,
        attempt: outcome.attempt,
        last_answer: outcome.last_answer,
        tries: value::read_tries(outcome.tries),
        invalid_result: read_optional_invalid(outcome.invalid_result),
        refusals: outcome.refusals,
        phase: phase::read_phase(outcome.phase, limits)?,
    })
}
fn problem(path: wire::Path) -> wire::Problem {
    wire::Problem { path, reason: skein_codec::Reason::Bound }
}

fn refused(source: &[u64], limits: &wire::Limits) -> Result<List<wire::Number>, wire::Problem> {
    phase::references(source, limits.task_history_refused, wire::Path::TaskHistoryRefused, limits)
}

fn dependencies(source: &[u64], limits: &wire::Limits) -> Result<List<wire::Number>, wire::Problem> {
    phase::references(source, limits.task_definition_dependencies, wire::Path::TaskDefinitionDependencies, limits)
}

fn waiting_on(source: &[u64], limits: &wire::Limits) -> Result<List<wire::Number>, wire::Problem> {
    phase::references(source, limits.task_definition_waiting_on, wire::Path::TaskDefinitionWaitingOn, limits)
}

fn delegates(source: &[u64], limits: &wire::Limits) -> Result<List<wire::Number>, wire::Problem> {
    phase::references(source, limits.task_interests_delegates, wire::Path::TaskInterestsDelegates, limits)
}

fn result_followups(source: &[u64], limits: &wire::Limits) -> Result<List<wire::Number>, wire::Problem> {
    phase::references(source, limits.task_interests_result_followups, wire::Path::TaskInterestsResultFollowups, limits)
}

fn references(source: &[u64], limits: &wire::Limits) -> Result<List<wire::Number>, wire::Problem> {
    phase::references(source, limits.task_interests_references, wire::Path::TaskInterestsReferences, limits)
}

fn added(source: &[domain::Added], limits: &wire::Limits) -> Result<List<wire::Added>, wire::Problem> {
    let mut rows = List::with_capacity(limits.task_definition_added);
    for row in source {
        let row = wire::Added::new(
            limits,
            wire::AddedParts { task: row.task, at: row.at.as_nanos(), by: value::party(&row.by, limits)? },
        )?;
        if rows.push(row).is_err() {
            return Err(problem(wire::Path::TaskDefinitionAdded));
        }
    }
    Ok(rows)
}
fn read_added(source: List<wire::Added>) -> Box<[domain::Added]> {
    let mut rows = List::with_capacity(source.len());
    for row in source.into_boxed() {
        let wire::AddedParts { task, at, by } = row.into_parts();
        rows.push(domain::Added { task, at: Wall::from_nanos(at), by: value::read_party(by) })
            .expect("same number of additions");
    }
    rows.into_boxed()
}

fn room_wait(source: Option<domain::RoomWait>, limits: &wire::Limits) -> Result<Option<wire::RoomWait>, wire::Problem> {
    let Some(wait) = source else { return Ok(None) };
    let room = match wait.room {
        domain::LiveRoom::Deployment => wire::LiveRoom::Deployment,
        domain::LiveRoom::Project(project) => {
            wire::LiveRoom::Project(wire::RoomProject::new(limits, wire::RoomProjectParts { project })?)
        }
    };
    Ok(Some(wire::RoomWait::new(limits, wire::RoomWaitParts { room, order: wait.order })?))
}

fn read_room_wait(source: Option<wire::RoomWait>) -> Option<domain::RoomWait> {
    let wait = source?.into_parts();
    let room = match wait.room {
        wire::LiveRoom::Deployment => domain::LiveRoom::Deployment,
        wire::LiveRoom::Project(project) => domain::LiveRoom::Project(project.into_parts().project),
    };
    Some(domain::RoomWait { room, order: wait.order })
}
