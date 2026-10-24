//! Single scopes use exact keys and retained positions (store.md, 7.3).
use super::{Failure, Spent, empty, joins};
use crate::{Below, Event, Get, Row, Store, TranscriptRead, gets, transcripts::read};
use alloc::boxed::Box;
use jig_domain_core::{self as core, reads};
use jig_domain_people::Proposer;
use jig_domain_tasks as tasks;
use skein_lib::{Queue, Token};

pub(super) fn record(store: &mut Store, key: &core::Key) -> Result<Option<core::Record>, Failure> {
    let row = gets::row(store, Token::new(0), Get::Jig(key.clone())).or(Err(Failure::Corrupt))?;
    let Some(row) = row else { return Ok(None) };
    match row {
        Row::Jig(record) => Ok(Some(record)),
        Row::Application { .. } => Err(Failure::Corrupt),
    }
}

pub(super) fn task(
    store: &mut Store,
    range: &reads::Range,
    task: u64,
    spent: &mut Spent,
) -> Result<reads::Loaded, Failure> {
    if spent.rows == 0 {
        return Ok(empty());
    }
    for key in [jig_domain_tasks::Key::Live(task), jig_domain_tasks::Key::Ended(task)] {
        let key = core::Key::Tasks(key);
        let Some(value) = record(store, &key)? else { continue };
        let position = reads::Position::Owner(key);
        if let Some(after) = &range.after
            && position <= *after
        {
            return Ok(empty());
        }
        let Some(owner) = joins::as_task(&value) else { return Err(Failure::Corrupt) };
        let owner = owner.clone();
        let related = core::Key::Core(core::CoreKey::Terminal { task, attempt: owner.attempt });
        let row = joins::row(store, value, Some(&owner), Some(related), spent)?;
        return Ok(reads::Loaded::Page { rows: Box::new([row]), next: None });
    }
    Ok(empty())
}

fn page(
    store: &mut Store,
    range: &reads::Range,
    value: core::Record,
    owner: Option<&tasks::TaskRecord>,
    related: Option<core::Key>,
    spent: &mut Spent,
) -> Result<reads::Loaded, Failure> {
    if !allowed(range, &value.key()) {
        return Ok(empty());
    }
    let row = joins::row(store, value, owner, related, spent)?;
    Ok(reads::Loaded::Page { rows: Box::new([row]), next: None })
}

pub(super) fn proposal(
    store: &mut Store,
    range: &reads::Range,
    proposer: Proposer,
    proposal: u64,
    spent: &mut Spent,
) -> Result<reads::Loaded, Failure> {
    match proposer {
        Proposer::Party(_) => {
            for key in [tasks::Key::PersonProposal(proposal), tasks::Key::DecidedProposal(proposal)] {
                let Some(value) = record(store, &core::Key::Tasks(key))? else { continue };
                let related = core::Key::Core(core::CoreKey::ProposalDecision { proposer, proposal });
                return page(store, range, value, None, Some(related), spent);
            }
            Ok(empty())
        }
        Proposer::Task(task) => {
            let Some(owner) = joins::owner(store, task, spent)? else { return Ok(empty()) };
            if let Some(key) = proposal_key(&owner, proposal) {
                let Some(value) = record(store, &key)? else { return Ok(empty()) };
                return page(store, range, value, Some(&owner), None, spent);
            }
            Ok(reads::Loaded::Gone)
        }
    }
}

pub(super) fn escalation(
    store: &mut Store,
    range: &reads::Range,
    task: u64,
    spent: &mut Spent,
) -> Result<reads::Loaded, Failure> {
    let Some(owner) = joins::owner(store, task, spent)? else { return Ok(empty()) };
    let key = core::Key::Core(core::CoreKey::EscalationDecision { task, revision: owner.escalation.revision() });
    let Some(value) = record(store, &key)? else { return Ok(empty()) };
    let related = core::Key::Tasks(tasks::Key::Milestone { task, position: owner.hold });
    page(store, range, value, Some(&owner), Some(related), spent)
}

pub(super) fn question(
    store: &mut Store,
    range: &reads::Range,
    task: u64,
    message: u64,
    spent: &mut Spent,
) -> Result<reads::Loaded, Failure> {
    let Some(owner) = joins::owner(store, task, spent)? else { return Ok(empty()) };
    for kept in &owner.question_lines {
        if kept.message == message {
            let key = core::Key::Tasks(tasks::Key::Activity { task, position: kept.line });
            let Some(value) = record(store, &key)? else { return Ok(empty()) };
            let mut related = None;
            if let Some(answer) = kept.answer {
                related = Some(core::Key::Tasks(tasks::Key::Activity { task: answer.task, position: answer.position }));
            }
            return page(store, range, value, Some(&owner), related, spent);
        }
    }
    Ok(reads::Loaded::Gone)
}

pub(super) fn reply(
    store: &mut Store,
    token: Token,
    range: &reads::Range,
    spent: &mut Spent,
    above: &mut Queue<Event>,
    below: &mut Below,
) -> Result<Option<reads::Loaded>, Failure> {
    let reads::Scope::Reply { task, activity } = range.scope else { return Err(Failure::Corrupt) };
    let key = core::Key::Tasks(tasks::Key::Activity { task, position: activity });
    if !allowed(range, &key) {
        return Ok(Some(empty()));
    }
    let Some(value) = record(store, &key)? else { return Ok(Some(empty())) };
    let core::Record::Tasks(tasks::Stored::Activity(line)) = &value else { return Err(Failure::Corrupt) };
    match line.line {
        tasks::Line::Turn { attempt, turn, .. } => read::party(
            store,
            token,
            TranscriptRead::Turn { task, attempt, turn },
            read::Party {
                activity: Some(activity),
                bytes: range.bytes,
                turn: None,
                problem: None,
                busy: false,
                frame_bytes: None,
            },
            above,
            below,
        ),
        tasks::Line::Words { .. }
        | tasks::Line::Heard(_)
        | tasks::Line::Notified { .. }
        | tasks::Line::RunBegan { .. }
        | tasks::Line::RunEnded { .. }
        | tasks::Line::Decided { .. }
        | tasks::Line::Kept { .. }
        | tasks::Line::Milestone { .. } => {
            let Some(owner) = joins::owner(store, task, spent)? else { return Ok(Some(empty())) };
            super::ready(page(store, range, value, Some(&owner), None, spent))
        }
    }
}

pub(super) fn turn(
    store: &mut Store,
    owner: Token,
    range: &reads::Range,
    above: &mut Queue<Event>,
    below: &mut Below,
) -> Result<Option<reads::Loaded>, Failure> {
    let reads::Scope::Turn { task, attempt, turn } = range.scope else { return Err(Failure::Corrupt) };
    let key = core::Key::Core(core::CoreKey::Turn { task, attempt, turn });
    if !allowed(range, &key) {
        return Ok(Some(empty()));
    }
    read::party(
        store,
        owner,
        TranscriptRead::Turn { task, attempt, turn },
        read::Party {
            activity: None,
            bytes: range.bytes,
            turn: Some((attempt, turn)),
            problem: None,
            busy: false,
            frame_bytes: None,
        },
        above,
        below,
    )
}

fn allowed(range: &reads::Range, key: &core::Key) -> bool {
    if range.rows == 0 {
        return false;
    }
    let Some(after) = &range.after else { return true };
    let position = reads::Position::Owner(key.clone());
    if range.backwards { position < *after } else { position > *after }
}

pub(crate) fn finish(
    store: &mut Store,
    task: u64,
    party: read::Party,
    turn: Option<core::TurnRecord>,
) -> Result<reads::Loaded, Failure> {
    let mut spent = Spent { rows: 1, bytes: u64::MAX, joins: store.limits.joins };
    if party.busy {
        return Ok(reads::Loaded::Busy);
    }
    if let Some(why) = party.problem {
        let owner = match joins::owner(store, task, &mut spent) {
            Ok(Some(owner)) => owner,
            Ok(None) => return Ok(empty()),
            Err(Failure::Cut) => return Ok(reads::Loaded::Busy),
            Err(Failure::Corrupt) => return Err(Failure::Corrupt),
        };
        return problem(task, &party, owner, why);
    }
    let mut related = None;
    let value = match party.activity {
        Some(activity) => {
            let key = core::Key::Tasks(tasks::Key::Activity { task, position: activity });
            let Some(value) = record(store, &key)? else { return Ok(empty()) };
            if let Some(turn) = turn {
                related = Some(core::Record::Core(core::CoreRecord::Turn(turn)));
            }
            spent.joins = spent.joins.checked_sub(1).ok_or(Failure::Cut)?;
            value
        }
        None => {
            let Some(turn) = turn else { return Ok(empty()) };
            core::Record::Core(core::CoreRecord::Turn(turn))
        }
    };
    let Some(owner) = joins::owner(store, task, &mut spent)? else { return Ok(empty()) };
    let mut row = joins::row(store, value, Some(&owner), None, &mut spent)?;
    row.related = related;
    let mut charged = row.owned_bytes().ok_or(Failure::Corrupt)?;
    if let Some(size) = party.frame_bytes {
        let evidence = match &row.related {
            Some(record) => record,
            None => &row.value,
        };
        let payload = turn_bytes(evidence)?;
        charged = charged
            .checked_sub(payload)
            .ok_or(Failure::Corrupt)?
            .checked_add(u64::from(size))
            .ok_or(Failure::Corrupt)?;
    }
    if charged > party.bytes {
        if party.turn.is_some() {
            let core::Record::Core(core::CoreRecord::Turn(turn)) = &row.value else { return Err(Failure::Corrupt) };
            let bytes = match party.frame_bytes {
                Some(size) => u64::from(size),
                None => u64::try_from(turn.transcript.len()).or(Err(Failure::Corrupt))?,
            };
            let why = reads::TurnFailure::TooLarge { bytes };
            let owner = row.owner.take().ok_or(Failure::Corrupt)?;
            return problem(task, &party, owner, why);
        }
        return Err(Failure::Cut);
    }
    Ok(reads::Loaded::Page { rows: Box::new([row]), next: None })
}

fn problem(
    task: u64,
    party: &read::Party,
    owner: Box<tasks::TaskRecord>,
    why: reads::TurnFailure,
) -> Result<reads::Loaded, Failure> {
    let (attempt, turn) = party.turn.ok_or(Failure::Corrupt)?;
    let Some(bytes) = reads::TurnFailure::owned_bytes(&owner) else { return Ok(reads::Loaded::Busy) };
    if bytes > party.bytes {
        return Ok(reads::Loaded::Busy);
    }
    Ok(reads::Loaded::TurnProblem { task, attempt, turn, owner, why })
}

/// Exact history key for the newest retained occurrence of a task proposal.
pub(crate) fn proposal_key(owner: &tasks::TaskRecord, proposal: u64) -> Option<core::Key> {
    for kept in &owner.proposal_rows {
        if kept.proposal == proposal {
            return Some(core::Key::Tasks(tasks::Key::History { task: owner.number, revision: kept.revision }));
        }
    }
    None
}

fn turn_bytes(record: &core::Record) -> Result<u64, Failure> {
    match record {
        core::Record::Core(record) => match record {
            core::CoreRecord::Turn(turn) => u64::try_from(turn.transcript.len()).or(Err(Failure::Corrupt)),
            core::CoreRecord::Settings(_)
            | core::CoreRecord::Deployment(_)
            | core::CoreRecord::Projection(_)
            | core::CoreRecord::Call(_)
            | core::CoreRecord::RunProof(_)
            | core::CoreRecord::Terminal(_)
            | core::CoreRecord::ProposalDecision(_)
            | core::CoreRecord::EscalationDecision(_) => Err(Failure::Corrupt),
        },
        core::Record::Tasks(_) | core::Record::People(_) | core::Record::Notes(_) | core::Record::Accounts(_) => {
            Err(Failure::Corrupt)
        }
    }
}
