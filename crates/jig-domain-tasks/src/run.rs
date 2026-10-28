//! Fenced agent-task preparation, claims, activation terminals and holds
//! (domain/tasks.md, sections 5.2, 5.5–5.7). Root supplies run
//! events; tasks checks lifecycle/result shape and emits bounded state changes.
//! Priced inputs use combined accounting admission; exact replay is root-owned.
pub(crate) mod ends;

use crate::domain::{Domain, entrance, fact, publish, record, refused, task_mut};
use crate::{
    Accepted, Active, Class, Closing, Contract, End, Ending, Fact, Hold, InvalidResult, Limits, Phase, Refusal,
    Request, SavedResource, Stage, TaskResult, Tries, Was,
};
use alloc::boxed::Box;
use skein_lib::{Env, Queue, ReplyTo};

pub(crate) fn saved_within(saved: Option<&[SavedResource]>, limits: &Limits) -> bool {
    let Some(resources) = saved else { return true };
    if resources.len() > usize::try_from(limits.saved_resources).expect("u32 fits usize") {
        return false;
    }
    let mut previous = None;
    for resource in resources {
        if resource.path.is_empty()
            || resource.path.len() > usize::try_from(limits.authority_segments).expect("u32 fits usize")
        {
            return false;
        }
        let mut bytes = 0usize;
        for segment in &resource.path {
            let Some(sum) = bytes.checked_add(segment.len()) else { return false };
            bytes = sum;
        }
        if bytes > usize::try_from(limits.authority_bytes).expect("u32 fits usize") {
            return false;
        }
        if let Some(before) = previous
            && before >= resource
        {
            return false;
        }
        previous = Some(resource);
    }
    true
}

pub(crate) fn prepare(domain: &mut Domain, env: &Env<Limits>, to: ReplyTo, number: u64, out: &mut Queue<Request>) {
    let to = match entrance(domain, to, number) {
        Ok(to) => to,
        Err((to, why)) => return refused(to, Some(number), why, out),
    };
    let kept = ends::kept(domain, number);
    let moves = domain.task(number).expect("entrance names task").flags.conversation && domain.may_move(number);
    let newest = domain.newest();
    let task = task_mut(domain, number).expect("entrance names task");
    match task.record.executor {
        crate::Executor::Agent { .. } => {}
        crate::Executor::Procedure { .. } | crate::Executor::Person(_) => {
            return refused(to, Some(number), Refusal::Executor, out);
        }
    }
    if task.record.phase != Phase::Active(Active::Due) || kept.blocks_claims() {
        return refused(to, Some(number), Refusal::State, out);
    }
    if moves {
        task.record.settings.task = newest;
    }
    task.record.settings.run = Some(task.record.settings.task);
    task.record.phase = Phase::Active(Active::Preparing);
    publish(domain, env, number, out);
    out.push(Request::Done { reply_to: to });
}

#[expect(clippy::too_many_arguments, reason = "claim carries the caller's bounded write set")]
pub(crate) fn claim(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    number: u64,
    attempt: u64,
    resumed: bool,
    budget: u64,
    account: Option<u32>,
    writes: &[crate::Name],
    out: &mut Queue<Request>,
) {
    let to = match entrance(domain, to, number) {
        Ok(to) => to,
        Err((to, why)) => return refused(to, Some(number), why, out),
    };
    let task = record(domain, number).expect("entrance names task");
    if attempt <= task.attempt {
        return refused(to, Some(number), Refusal::Attempt, out);
    }
    if task.phase != Phase::Active(Active::Preparing) || ends::kept(domain, number).blocks_claims() {
        return refused(to, Some(number), Refusal::State, out);
    }
    let fits = match crate::funders::available(task.numbers) {
        Some(left) => budget != 0 && budget <= left,
        None => false,
    };
    if !fits {
        return refused(to, Some(number), Refusal::Funding, out);
    }
    match crate::writers::claim(domain, env, number, attempt, writes, out) {
        Ok(()) => {}
        Err(Some(resource)) => return out.push(Request::WriterWaiting { reply_to: to, task: Some(number), resource }),
        Err(None) => return refused(to, Some(number), Refusal::Holds, out),
    }
    let task = task_mut(domain, number).expect("entrance names task");
    task.record.numbers.reserved = task.record.numbers.reserved.checked_add(budget).expect("available budget");
    task.record.run_reserved = budget;
    if let Some(account) = account {
        task.record.kept_account = Some(account);
    }
    task.record.attempt = attempt;
    task.record.run_since = None;
    task.record.retry_notice = None;
    task.record.run_spent = 0;
    task.record.invalid_result = None;
    task.record.turn = 0;
    task.record.refused = Box::new([]);
    task.record.phase = Phase::Active(Active::Claimed { attempt });
    let why = task.record.due_reason;
    crate::activity::append(&mut task.record, env.wall, crate::Line::RunBegan { attempt, why, resumed }, out);
    publish(domain, env, number, out);
    fact(domain, Fact::Claimed { task: number, attempt });
    out.push(Request::Done { reply_to: to });
}

pub(crate) fn started(domain: &mut Domain, env: &Env<Limits>, number: u64, attempt: u64, out: &mut Queue<Request>) {
    if !domain.ready() {
        return;
    }
    if let Some(task) = task_mut(domain, number)
        && task.record.phase == Phase::Active(Active::Claimed { attempt })
    {
        task.record.phase = Phase::Active(Active::Running { attempt });
        task.record.run_since = Some(env.wall);
        publish(domain, env, number, out);
    }
}

/// The claimed attempt whose answer this phase has not heard.
#[must_use]
pub fn run_attempt(phase: &Phase) -> Option<u64> {
    match phase {
        Phase::Active(Active::Claimed { attempt } | Active::Running { attempt })
        | Phase::Closing(Closing { stage: Stage::Run { attempt }, .. })
        | Phase::Held {
            was:
                Was::Active(Active::Claimed { attempt } | Active::Running { attempt })
                | Was::Closing(Closing { stage: Stage::Run { attempt }, .. }),
            ..
        } => Some(*attempt),
        Phase::Waiting
        | Phase::Active(Active::Idle | Active::Due | Active::Preparing | Active::BackingOff { .. })
        | Phase::Closing(Closing {
            stage: Stage::Delegates | Stage::Effects | Stage::Releases | Stage::Settled, ..
        })
        | Phase::Held {
            was:
                Was::Waiting
                | Was::Active(Active::Idle | Active::Due | Active::Preparing | Active::BackingOff { .. })
                | Was::Closing(Closing {
                    stage: Stage::Delegates | Stage::Effects | Stage::Releases | Stage::Settled, ..
                }),
            ..
        }
        | Phase::Ended(_) => None,
    }
}

pub(crate) fn valid_result(contract: &Contract, result: &TaskResult, limits: &Limits) -> bool {
    invalid_result(contract, result, limits).is_none()
}

pub(crate) fn invalid_result(contract: &Contract, result: &TaskResult, limits: &Limits) -> Option<InvalidResult> {
    match result {
        TaskResult::Approved { state, .. } => match contract {
            Contract::Approval { state: expected, .. } => {
                if state != expected {
                    Some(InvalidResult::Form)
                } else if result_bytes(result) > usize::try_from(limits.result_bytes).expect("u32 fits usize") {
                    Some(InvalidResult::Words)
                } else {
                    None
                }
            }
            Contract::Report { .. } | Contract::Verdict { .. } | Contract::Change { .. } => Some(InvalidResult::Form),
        },
        TaskResult::Failure { reason } => match contract {
            Contract::Approval { .. }
            | Contract::Report { .. }
            | Contract::Verdict { .. }
            | Contract::Change { .. } => {
                if reason.len() <= usize::try_from(limits.result_bytes).expect("u32 fits usize") {
                    None
                } else {
                    Some(InvalidResult::Words)
                }
            }
        },
        TaskResult::Report { words } => match contract {
            Contract::Report { words: limit } => {
                if words.len() <= usize::try_from(*limit).expect("u32 fits usize") {
                    None
                } else {
                    Some(InvalidResult::Words)
                }
            }
            Contract::Approval { .. } | Contract::Verdict { .. } | Contract::Change { .. } => Some(InvalidResult::Form),
        },
        TaskResult::Verdict { code, words, fields } => match contract {
            Contract::Verdict { choices } => {
                for choice in choices {
                    if choice.code == *code {
                        return if words.len() > usize::try_from(choice.words).expect("u32 fits usize")
                            || result_bytes(result) > usize::try_from(limits.result_bytes).expect("u32 fits usize")
                        {
                            Some(InvalidResult::Words)
                        } else if !crate::verdict::fields_valid(&choice.fields, fields) {
                            Some(InvalidResult::Form)
                        } else {
                            None
                        };
                    }
                }
                Some(InvalidResult::Verdict)
            }
            Contract::Approval { .. } | Contract::Report { .. } | Contract::Change { .. } => Some(InvalidResult::Form),
        },
        TaskResult::Change { connector: actual_connector, kind: actual_kind, words, .. } => match contract {
            Contract::Change { connector, kind, words: limit } => {
                if connector != actual_connector || kind != actual_kind {
                    Some(InvalidResult::Change)
                } else if words.len() > usize::try_from(*limit).expect("u32 fits usize") {
                    Some(InvalidResult::Words)
                } else {
                    None
                }
            }
            Contract::Approval { .. } | Contract::Report { .. } | Contract::Verdict { .. } => Some(InvalidResult::Form),
        },
    }
}

/// Check the admitted verdict's follow-up allowance as well as its result words.
pub(crate) fn invalid_followups(
    contract: &Contract,
    result: &TaskResult,
    followups: &crate::ResultFollowups,
    limits: &Limits,
) -> Option<InvalidResult> {
    if let Some(reason) = invalid_result(contract, result, limits) {
        return Some(reason);
    }
    let count = match followups {
        crate::ResultFollowups::Delegates(batch) => batch.len(),
        crate::ResultFollowups::Proposal(proposal) => match &proposal.action {
            crate::ProposalAction::Batch(batch) => batch.len(),
            crate::ProposalAction::Settings { .. }
            | crate::ProposalAction::Effect { .. }
            | crate::ProposalAction::ProcedureEffect { .. }
            | crate::ProposalAction::Amend { .. }
            | crate::ProposalAction::Widen { .. }
            | crate::ProposalAction::Release { .. }
            | crate::ProposalAction::ReleasePlaced { .. } => {
                return Some(InvalidResult::Form);
            }
        },
    };
    match (contract, result) {
        (Contract::Verdict { choices }, TaskResult::Verdict { code, .. }) => {
            for choice in choices {
                if choice.code == *code {
                    return if count > 0 && count <= usize::try_from(choice.followups).expect("u32 fits usize") {
                        None
                    } else {
                        Some(InvalidResult::Followups)
                    };
                }
            }
            Some(InvalidResult::Verdict)
        }
        (
            Contract::Approval { .. } | Contract::Report { .. } | Contract::Change { .. } | Contract::Verdict { .. },
            _,
        ) => Some(InvalidResult::Form),
    }
}

pub(crate) fn result_bytes(result: &TaskResult) -> usize {
    match result {
        TaskResult::Report { words } | TaskResult::Change { words, .. } => words.len(),
        TaskResult::Verdict { words, fields, .. } => match crate::verdict::fields_bytes(fields) {
            Some(bytes) => bytes.saturating_add(words.len()),
            None => usize::MAX,
        },
        TaskResult::Approved { state, words } => state.len().saturating_add(match words {
            Some(words) => words.len(),
            None => 0,
        }),
        TaskResult::Failure { reason } => reason.len(),
    }
}

fn ending(result: TaskResult) -> Ending {
    match result {
        TaskResult::Failure { reason } => Ending::Failed { reason },
        TaskResult::Approved { .. }
        | TaskResult::Report { .. }
        | TaskResult::Verdict { .. }
        | TaskResult::Change { .. } => Ending::Done(result),
    }
}

#[expect(clippy::too_many_arguments, reason = "one fenced activation terminal")]
#[expect(clippy::too_many_lines, reason = "one terminal normalizes the result and settles its current attempt")]
pub(crate) fn activation(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    number: u64,
    attempt: u64,
    end: End,
    saved: Option<Box<[SavedResource]>>,
    out: &mut Queue<Request>,
) {
    if !domain.ready() {
        return refused(to, Some(number), Refusal::NotReady, out);
    }
    let Some(old) = record(domain, number) else {
        return refused(to, Some(number), Refusal::Unknown, out);
    };
    if old.last_answer == Some(attempt) {
        return out.push(Request::Acknowledged { reply_to: to, task: number, attempt, accepted: Accepted::Already });
    }
    if run_attempt(&old.phase) != Some(attempt) {
        return refused(to, Some(number), Refusal::Attempt, out);
    }
    if !saved_within(saved.as_deref(), &env.limits) {
        return refused(to, Some(number), Refusal::Contract, out);
    }
    let lost_writer = match &end {
        End::Failed(Class::Lost) => true,
        End::Failed(_)
        | End::Exceeded(_)
        | End::Parked
        | End::Refused
        | End::Recheck
        | End::Finished { .. }
        | End::FinishedWithFollowups { .. } => false,
    };
    let held = match &old.phase {
        Phase::Held { why, source, .. } => Some((*why, source.clone())),
        Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Ended(_) => None,
    };
    let closing = match &old.phase {
        Phase::Closing(closing) | Phase::Held { was: Was::Closing(closing), .. } => Some(closing.clone()),
        Phase::Waiting
        | Phase::Active(_)
        | Phase::Held { was: Was::Waiting | Was::Active(_), .. }
        | Phase::Ended(_) => None,
    };
    let narrowed = old.narrowing;
    let kept = ends::kept(domain, number);
    let discarded = !narrowed && kept.discards(&end);
    let end = if narrowed || discarded { End::Parked } else { end };
    let result_followups = match &end {
        End::FinishedWithFollowups { .. } => true,
        End::Finished { .. } | End::Exceeded(_) | End::Parked | End::Failed(_) | End::Refused | End::Recheck => false,
    };
    let invalid = match &end {
        End::Finished { result, .. } => invalid_result(&old.contract, result, &env.limits),
        End::FinishedWithFollowups { result, followups } => {
            invalid_followups(&old.contract, result, followups, &env.limits)
        }
        End::Exceeded(_) | End::Parked | End::Failed(_) | End::Refused | End::Recheck => None,
    };
    let end = match end {
        End::Finished { result, cancel_delegates } => {
            if invalid.is_none() {
                End::Finished { result, cancel_delegates }
            } else {
                End::Failed(Class::Invalid)
            }
        }
        End::FinishedWithFollowups { result, .. } => {
            if invalid.is_none() {
                End::Finished { result, cancel_delegates: false }
            } else {
                End::Failed(Class::Invalid)
            }
        }
        End::Exceeded(_) | End::Parked | End::Failed(_) | End::Refused | End::Recheck => end,
    };
    let draining = domain.parking.get(&number) == Some(&attempt);
    let parked = kept.parks(&end);
    let end = if parked { End::Parked } else { end };
    let refuses_live = match &end {
        End::Finished { cancel_delegates, .. } => !cancel_delegates && !result_followups,
        End::FinishedWithFollowups { .. } => unreachable!("follow-ups normalized"),
        End::Exceeded(_) | End::Parked | End::Failed(_) | End::Refused | End::Recheck => false,
    };
    if closing.is_none() && refuses_live && !old.delegates.is_empty() {
        return out
            .push(Request::Refused { reply_to: to, problem: crate::Problem::live_delegates(number, &old.delegates) });
    }
    let end_kind = if narrowed || closing.is_some() {
        crate::EndKind::Cancelled
    } else {
        match &end {
            End::Parked => crate::EndKind::Parked,
            End::Finished { .. } => crate::EndKind::Finished,
            End::Exceeded(_) => crate::EndKind::Failed { class: Class::Run },
            End::Failed(class) => crate::EndKind::Failed { class: *class },
            End::Refused | End::Recheck => crate::EndKind::Refused,
            End::FinishedWithFollowups { .. } => unreachable!("follow-ups normalized"),
        }
    };
    if draining {
        domain.parking.remove(&number);
    }
    let next = if let Some(mut closing) = closing {
        closing.stage = Stage::Delegates;
        match &mut closing.ending {
            Ending::Cancelled { result: partial, .. } => match end {
                End::Finished { result, .. } => *partial = Some(result),
                End::FinishedWithFollowups { .. } => unreachable!("follow-ups normalized"),
                End::Exceeded(_) | End::Parked | End::Failed(_) | End::Refused | End::Recheck => {}
            },
            Ending::Done(_) | Ending::Failed { .. } => {}
        }
        Phase::Closing(closing)
    } else {
        match end {
            End::Finished { result, cancel_delegates } => {
                if cancel_delegates {
                    crate::closing::cancel_delegates(domain, env, number, b"", out);
                }
                Phase::Closing(Closing { stage: Stage::Delegates, ending: ending(result) })
            }
            End::FinishedWithFollowups { .. } => unreachable!("follow-ups normalized"),
            End::Parked => {
                let task = task_mut(domain, number).expect("terminal names live task");
                task.record.tries = Tries::NONE;
                task.record.refusals = 0;
                if (narrowed || draining) && !kept.blocks_claims() {
                    Phase::Active(Active::Due)
                } else {
                    Phase::Active(Active::Idle)
                }
            }
            End::Exceeded(value) => exceeded(domain, number, value),
            End::Failed(class) => failure(domain, env, number, class),
            End::Refused => pause(domain, env, number),
            End::Recheck => {
                if kept.blocks_claims() {
                    Phase::Active(Active::Idle)
                } else {
                    Phase::Active(Active::Due)
                }
            }
        }
    };
    let task = task_mut(domain, number).expect("terminal names live task");
    task.record.invalid_result = invalid;
    task.record.numbers.reserved =
        task.record.numbers.reserved.checked_sub(task.record.run_reserved).expect("run held");
    task.record.run_reserved = 0;
    task.record.last_answer = Some(attempt);
    task.record.narrowing = false;
    let spent = task.record.run_spent;
    crate::activity::append(&mut task.record, env.wall, crate::Line::RunEnded { attempt, end: end_kind, spent }, out);
    if parked {
        let pending = task.record.waiting_on.clone();
        crate::activity::append(&mut task.record, env.wall, crate::Line::Kept { attempt, pending }, out);
    }
    if let Some(tags) = saved {
        task.record.saved = tags;
    }
    task.record.phase = match held {
        Some((why, source)) => Phase::Held { was: was(next), why, source },
        None => next,
    };
    crate::writers::answered(domain, env, number, attempt, lost_writer, out);
    if (discarded || parked || !kept.finishes_alone() && end_kind == crate::EndKind::Parked)
        && let Some(grace) = record(domain, number).expect("answered task live").grace.clone()
    {
        crate::closing::cancel_tree(domain, env, number, grace.by, &grace.reason, out);
    }
    crate::wake::after_run(domain, env, number, out);
    publish(domain, env, number, out);
    if (narrowed || draining || end_kind == crate::EndKind::Refused)
        && record(domain, number).expect("amended task live").phase == Phase::Active(Active::Due)
    {
        crate::domain::activate(domain, number, out);
    }
    out.push(Request::Acknowledged { reply_to: to, task: number, attempt, accepted: Accepted::New });
}

pub(crate) fn was(phase: Phase) -> Was {
    match phase {
        Phase::Waiting => Was::Waiting,
        Phase::Active(active) => Was::Active(active),
        Phase::Closing(closing) => Was::Closing(closing),
        Phase::Held { was, .. } => was,
        Phase::Ended(_) => unreachable!("ended task cannot be held"),
    }
}

fn wall_after(env: &Env<Limits>, duration: skein_lib::Duration) -> skein_lib::Wall {
    skein_lib::Wall::from_nanos(env.wall.as_nanos().saturating_add(duration.as_nanos()))
}

fn failure(domain: &mut Domain, env: &Env<Limits>, number: u64, class: Class) -> Phase {
    let task = task_mut(domain, number).expect("failure names live task");
    task.record.tries.add(class);
    task.record.refusals = 0;
    let times = task.record.tries.of(class);
    let retry = env.limits.retries.of(class);
    task.record.retry_notice = Some(crate::RetryNotice::Failed { class, count: times, allowance: retry.retries });
    fact(domain, Fact::Failed { task: number, class });
    if times > retry.retries {
        fact(domain, Fact::Held { task: number, why: Hold::Failures(class) });
        Phase::Held { was: Was::Active(Active::Due), why: Hold::Failures(class), source: None }
    } else {
        let duration = crate::failures::backoff(times, retry, &mut domain.rng);
        Phase::Active(Active::BackingOff { until: wall_after(env, duration) })
    }
}

fn pause(domain: &mut Domain, env: &Env<Limits>, number: u64) -> Phase {
    let task = task_mut(domain, number).expect("pause names live task");
    task.record.refusals = task.record.refusals.saturating_add(1);
    let times = task.record.refusals;
    task.record.retry_notice = Some(crate::RetryNotice::Refused { count: times });
    let duration = crate::failures::backoff(times, env.limits.retries.transient, &mut domain.rng);
    Phase::Active(Active::BackingOff { until: wall_after(env, duration) })
}

pub(crate) fn preparation_waiting(domain: &mut Domain, env: &Env<Limits>, number: u64, out: &mut Queue<Request>) {
    if domain.ready()
        && let Some(task) = record(domain, number)
        && task.phase == Phase::Active(Active::Preparing)
        && !ends::kept(domain, number).blocks_claims()
    {
        task_mut(domain, number).expect("preparing task").record.phase = Phase::Active(Active::Due);
        publish(domain, env, number, out);
        crate::domain::activate(domain, number, out);
    }
}

pub(crate) fn preparation_failed(domain: &mut Domain, env: &Env<Limits>, number: u64, out: &mut Queue<Request>) {
    if !domain.ready() {
        return;
    }
    if let Some(task) = record(domain, number)
        && task.phase == Phase::Active(Active::Preparing)
    {
        let phase = pause(domain, env, number);
        task_mut(domain, number).expect("prepared task exists").record.phase = phase;
        publish(domain, env, number, out);
    }
}

pub(crate) fn preparation_unread(domain: &mut Domain, env: &Env<Limits>, number: u64, out: &mut Queue<Request>) {
    if domain.ready()
        && let Some(task) = record(domain, number)
        && task.phase == Phase::Active(Active::Preparing)
    {
        let phase = failure(domain, env, number, Class::Transient);
        task_mut(domain, number).expect("prepared task exists").record.phase = phase;
        publish(domain, env, number, out);
    }
}

pub(crate) fn hold(domain: &mut Domain, env: &Env<Limits>, number: u64, why: Hold, out: &mut Queue<Request>) {
    hold_from(domain, env, number, why, None, out);
}

pub(crate) fn hold_from(
    domain: &mut Domain,
    env: &Env<Limits>,
    number: u64,
    why: Hold,
    source: Option<Box<crate::HoldSource>>,
    out: &mut Queue<Request>,
) {
    if crate::placed::is_placed(why) {
        return;
    }
    if let Some(plan) = check_stop(domain, &crate::compound::Staged::new(0), env, number, why, source) {
        apply_stop(domain, env, plan, out);
    }
}

pub(crate) struct StopPlan {
    pub(crate) number: u64,
    pub(crate) why: Hold,
    pub(crate) attempt: Option<u64>,
    pub(crate) next: Phase,
}

pub(crate) fn check_stop(
    domain: &Domain,
    staged: &crate::compound::Staged,
    env: &Env<Limits>,
    number: u64,
    why: Hold,
    source: Option<Box<crate::HoldSource>>,
) -> Option<StopPlan> {
    if !crate::hold_origin::valid(&env.limits, why, source.as_deref()) {
        return None;
    }
    if !why.valid() {
        return None;
    }
    if !domain.ready() {
        return None;
    }
    let task = staged.record(domain, number)?;
    match task.phase {
        Phase::Held { why: current, source: ref previous, .. }
            if current == why && (source.is_none() || previous == &source) =>
        {
            return None;
        }
        Phase::Ended(_) => return None,
        Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Held { .. } => {}
    }
    let next = match task.phase.clone() {
        Phase::Held { was, source: previous, why: before } => {
            Phase::Held { was, why, source: if before == why && source.is_none() { previous } else { source } }
        }
        Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Ended(_) => {
            Phase::Held { was: was(task.phase.clone()), why, source }
        }
    };
    Some(StopPlan { number, why, attempt: run_attempt(&task.phase), next })
}

pub(crate) fn apply_stop(domain: &mut Domain, env: &Env<Limits>, plan: StopPlan, out: &mut Queue<Request>) {
    let StopPlan { number, why, attempt, next } = plan;
    if let Some(attempt) = attempt {
        out.push(Request::Stop { task: number, attempt });
    }
    task_mut(domain, number).expect("hold names live task").record.phase = next;
    publish(domain, env, number, out);
    fact(domain, Fact::Held { task: number, why });
}

fn exceeded(domain: &mut Domain, number: u64, exceeded: crate::Exceeded) -> Phase {
    let task = task_mut(domain, number).expect("exceeded names live task");
    task.record.refusals = 0;
    task.record.retry_notice = None;
    let why = Hold::Exceeded(exceeded);
    fact(domain, Fact::Held { task: number, why });
    Phase::Held { was: Was::Active(Active::Due), why, source: None }
}
