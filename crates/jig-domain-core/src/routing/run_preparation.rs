//! Run admission, briefs and workspace preparation across the task, brief,
//! notes and account children (`domain/engine.md`, sections 4.4 and 7).

use super::NoteRoute;
use super::children::remember_due;
use crate::{Ask, Core, Event, Limits, Now, Request, Requests, RunCharter, RunProof};
use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_brief as brief;
use jig_domain_notes as notes;
use jig_domain_tasks as tasks;
use skein_lib::{Env, List, Queue, ReplyTo, Token};

pub(super) fn brief_assembled(core: &mut Core, work: &mut Queue<Event>, task: u64, ready: bool) -> Requests {
    let mut out = Queue::with_capacity(2);
    if !ready {
        work.push(Event::PreparationFailed { task });
        out.push(Request::Now(Box::new(Now::RunPreparationFailed { task })));
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    let Some(context) = core.contexts.get(&task).cloned() else {
        out.push(Request::Now(Box::new(Now::RunPreparationFailed { task })));
        out.push(Request::Decided);
        return Requests::Out(out);
    };
    let Some(attempt) = crate::fresh(&mut core.counters, crate::Family::Run) else {
        work.push(Event::Tasks(tasks::Event::PreparationFailed { task }));
        out.push(Request::Now(Box::new(Now::RunPreparationFailed { task })));
        out.push(Request::Decided);
        return Requests::Out(out);
    };
    assert!(core.workspace_pending.insert(task, attempt) == Ok(None), "one workspace flight per task");
    out.push(Request::Now(Box::new(Now::WorkspaceRequest { task, attempt, context })));
    out.push(Request::Decided);
    Requests::Out(out)
}

#[expect(
    clippy::too_many_lines,
    reason = "one fenced preparation builds the exact allowance, grants and claim together"
)]
#[expect(clippy::manual_map, reason = "the strict subset translates windows with an explicit match")]
pub(super) fn workspace_prepared(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    task: u64,
    attempt: u64,
    writes: Option<Box<[authority::Write]>>,
    credentials: Box<[u32]>,
) -> Requests {
    let mut out = Queue::with_capacity(2);
    if core.workspace_pending.remove(&task) != Some(attempt) {
        out.push(Request::Now(Box::new(Now::RunPreparationFailed { task })));
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    let Some(context) = core.contexts.get(&task).cloned() else {
        out.push(Request::Now(Box::new(Now::RunPreparationFailed { task })));
        out.push(Request::Decided);
        return Requests::Out(out);
    };
    drop(core.dependency_results.remove(&task));
    let transcript = core.transcripts.remove(&task).expect("rendered task owns loaded transcript");
    let Some(writes) = writes else {
        work.push(Event::Tasks(tasks::Event::Hold { task, why: tasks::Hold::Effects }));
        out.push(Request::Now(Box::new(Now::RunPreparationFailed { task })));
        out.push(Request::Decided);
        return Requests::Out(out);
    };
    if credentials.len() > usize::try_from(env.limits.accounts.credentials).expect("u32 credential limit fits usize") {
        work.push(Event::Tasks(tasks::Event::Hold { task, why: tasks::Hold::Effects }));
        out.push(Request::Now(Box::new(Now::RunPreparationFailed { task })));
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    drop(core.workspace_credentials.insert(task, credentials).expect("live task credential room"));
    let Some(current) = core.tasks.run_context(task) else { return wait_for_account(core, work, context) };
    if current != context {
        drop(core.workspace_credentials.remove(&task));
        return wait_for_account(core, work, current);
    }
    let allowance = match core.run_admission(&current, env.wall, writes) {
        crate::RunAdmission::Allow(allowance) => allowance,
        crate::RunAdmission::Wait(_) => return wait_for_account(core, work, current),
        crate::RunAdmission::Hold(why) => {
            work.push(Event::Tasks(tasks::Event::Hold { task, why }));
            out.push(Request::Now(Box::new(Now::RunPreparationFailed { task })));
            out.push(Request::Decided);
            return Requests::Out(out);
        }
    };
    let mut grants = List::with_capacity(env.limits.fleet.grants);
    for binding in &allowance.choice.accounts {
        let fact = core.accounts.source(binding.source).expect("selected source");
        if fact.kind != jig_domain_accounts::Kind::None {
            let Some(grant) = core.accounts.source_grant(binding.source, env.now) else {
                return wait_for_account(core, work, current);
            };
            grants.push(grant).expect("provider grant room");
        }
    }
    for number in core.run_credentials(task) {
        let Some(grant) = core.accounts.credential_grant(*number, env.now) else {
            return wait_for_account(core, work, context);
        };
        let mut present = false;
        for kept in &grants {
            if kept.form.table() == grant.form.table() && kept.account == grant.account {
                present = true;
            }
        }
        if !present {
            grants.push(grant).expect("credential grant room");
        }
    }
    if !core.proofs.contains_key(&task) && core.proofs.len() == core.proofs.capacity() {
        work.push(Event::Tasks(tasks::Event::PreparationFailed { task }));
        out.push(Request::Now(Box::new(Now::RunPreparationFailed { task })));
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    let resume = !transcript.closed
        && core.charter_of(task).policy.resume
        && context.tries.transient == 0
        && transcript.bytes <= u64::from(core.settings.resume_bytes);
    let conversation = if resume && transcript.last.is_some() { transcript.from } else { attempt };
    let messages = start_messages(context.inbox, env.limits.tasks.run_inbox);
    let mut proof = start_proof(core, task, attempt, conversation, &messages);
    proof.host = allowance.choice.placement.kind;
    proof.placement = Some(allowance.choice.placement.channel);
    proof.accounts.clone_from(&allowance.choice.accounts);
    proof.window = if resume {
        match transcript.window {
            Some((attempt, turn)) => Some(crate::CurrentWindow { attempt, turn, closed: false }),
            None => None,
        }
    } else {
        None
    };
    proof.credentials = core.workspace_credentials.remove(&task).expect("prepared credentials");
    let previous = core.proofs.insert(task, proof).expect("claim proof reserved before child mutation");
    if let Some(previous) = previous {
        assert!(core.replaced.insert(task, previous) == Ok(None), "one pending replacement per task");
    }
    let turns = if resume { core.resumed_turns(transcript) } else { Box::new([]) };
    let answered = answered_calls(core, task, attempt, env.limits.call_records);
    let mut sources = List::with_capacity(tasks::MODELS.checked_add(1).expect("main source"));
    let mut models = List::with_capacity(tasks::MODELS.checked_add(1).expect("main model"));
    models.push(&allowance.choice.policy.model).expect("main model");
    for model in &allowance.choice.policy.alternatives {
        models.push(model).expect("retained alternatives");
    }
    for model in &models {
        let provider = core.provider(model).expect("selected model provider");
        let mut grant = None;
        for binding in &allowance.choice.accounts {
            if binding.provider == provider
                && core.accounts.source(binding.source).expect("selected source").kind
                    != jig_domain_accounts::Kind::None
            {
                grant = Some(binding.source);
            }
        }
        sources.push(grant).expect("one grant per retained model");
    }
    let mut policy = allowance.choice.policy;
    if let Some(instructions) = &core.tasks.task(task).expect("live task").instructions {
        policy.instructions.clone_from(instructions);
    }
    policy.resume = resume;
    let run = RunCharter {
        revision: core.tasks.task(task).expect("prepared task").settings.run.expect("prepared revision"),
        ended_by: context.ended_by,
        sources: sources.into_boxed(),
        accounts: allowance.choice.accounts,
        placement: allowance.choice.placement,
        policy,
        contract: context.contract,
        authority: context.authority,
        budget: allowance.spend,
    };
    assert!(core.claiming.insert(task, attempt) == Ok(None), "one pending claim per task");
    out.push(Request::Now(Box::new(Now::RunPrepared {
        live: crate::LiveAsk {
            text: core.tasks.task(task).expect("live task").flags.conversation,
            reasoning: false,
            input: core.tasks.task(task).expect("live task").flags.conversation,
        },
        task,
        attempt,
        conversation,
        charter: core.charter_of(task).number,
        run: Box::new(run),
        messages,
        saved: context.saved,
        transcript: turns,
        answered,
        grants: grants.into_boxed(),
    })));
    out.push(Request::Decided);
    Requests::Out(out)
}

fn answered_calls(core: &Core, task: u64, attempt: u64, limit: u32) -> Box<[crate::CallKey]> {
    let mut answered = List::with_capacity(limit);
    let mut after = None;
    for _ in 0..core.call_parts.len() {
        let mut next = None;
        for (&key, _) in &core.call_parts {
            if key.task == task
                && key.attempt < attempt
                && match after {
                    Some(previous) => key > previous,
                    None => true,
                }
                && match next {
                    Some(previous) => key < previous,
                    None => true,
                }
            {
                next = Some(key);
            }
        }
        let Some(key) = next else { break };
        answered.push(key).expect("retained call bound");
        after = Some(key);
    }
    answered.into_boxed()
}

fn start_messages(inbox: Box<[tasks::Word]>, most: u32) -> Box<[tasks::Word]> {
    let mut messages = List::with_capacity(most);
    for word in inbox {
        if messages.len() == most {
            break;
        }
        messages.push(word).expect("start inbox bound");
    }
    messages.into_boxed()
}

fn start_proof(_core: &Core, task: u64, attempt: u64, conversation: u64, messages: &[tasks::Word]) -> RunProof {
    let mut offered = None;
    let mut outstanding = List::with_capacity(u32::try_from(messages.len()).expect("bounded start inbox"));
    for word in messages {
        offered = Some(word.number);
        outstanding.push(word.number).expect("one id per assigned word");
    }
    let mut proof = RunProof::claimed(task, attempt, conversation, offered, jig_domain_fleet::HostKind::Worker);
    proof.outstanding = outstanding.into_boxed();
    proof
}

fn wait_for_account(core: &mut Core, work: &mut Queue<Event>, context: Box<tasks::RunContext>) -> Requests {
    let task = context.task;
    super::result_reads::clear(core, task);
    drop(core.contexts.remove(&task));
    work.push(Event::Tasks(tasks::Event::PreparationWaiting { task }));
    let mut out = Queue::with_capacity(2);
    out.push(Request::Now(Box::new(Now::RunPreparationFailed { task })));
    out.push(Request::Decided);
    Requests::Out(out)
}

pub(super) fn start_brief(core: &mut Core, env: &Env<Limits>, work: &mut Queue<Event>, task: u64) -> Requests {
    let mut out = Queue::with_capacity(1);
    let Some(context) = core.contexts.get(&task) else {
        out.push(Request::Decided);
        return Requests::Out(out);
    };
    let Some(current) = core.tasks.task(task) else {
        out.push(Request::Decided);
        return Requests::Out(out);
    };
    if current.phase != tasks::Phase::Active(tasks::Active::Preparing)
        || core.tasks.run_message(task) != Some(context.last_message)
    {
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    if !core.dependency_results.contains_key(&task)
        && let Some(ids) = core.preparation_ids(task)
        && !ids.is_empty()
    {
        return super::result_reads::begin(core, work, task, ids);
    }
    if core.notes.busy() {
        core.pending_note_briefs.insert(task, true).expect("brief count bounds waiting notes");
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    let mut scopes = List::with_capacity(env.limits.notes.scopes);
    if let Some(goal) = core.goal_of(current) {
        scopes.push(notes::Scope::Goal { project: context.project, goal }).expect("goal scope room");
    }
    scopes.push(notes::Scope::Project { project: context.project }).expect("project scope room");
    scopes.push(notes::Scope::Deployment).expect("deployment scope room");
    for resource in &context.authority.note_resources {
        let last = match &resource.pattern.last {
            tasks::Last::Exact(value) => notes::Last::Exact(value.clone()),
            tasks::Last::Open(value) => notes::Last::Open(value.clone()),
        };
        scopes
            .push(notes::Scope::Resources {
                project: context.project,
                connector: resource.connector,
                pattern: notes::Pattern { segments: resource.pattern.segments.clone(), last },
            })
            .expect("admitted note resource scope room");
    }
    let owner = core.begin_note_route(NoteRoute::Brief { task });
    work.push(Event::Notes(notes::Event::Index { owner, scopes, most: env.limits.notes.lines }));
    out.push(Request::Decided);
    Requests::Out(out)
}

pub(super) fn plan_brief(
    core: &Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    task: u64,
    lines: List<notes::Line>,
    more: u32,
) -> Requests {
    let mut out = Queue::with_capacity(2);
    let Some(context) = core.contexts.get(&task) else {
        out.push(Request::Decided);
        return Requests::Out(out);
    };
    let Some(current) = core.tasks.task(task) else {
        out.push(Request::Decided);
        return Requests::Out(out);
    };
    if current.phase != tasks::Phase::Active(tasks::Active::Preparing)
        || core.tasks.run_message(task) != Some(context.last_message)
    {
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    let text_limits = crate::BriefTextLimits {
        parts: env.limits.brief_parts,
        read_bytes: env.limits.brief.read_bytes,
        brief_bytes: env.limits.brief.brief_bytes,
    };
    let mut wanted = List::with_capacity(env.limits.brief.sections);
    let Some(task_text) = crate::read_brief_part(core, text_limits, task, crate::BriefPart::Spec) else {
        work.push(Event::Tasks(tasks::Event::PreparationFailed { task }));
        out.push(Request::Decided);
        return Requests::Out(out);
    };
    wanted
        .push(brief::Planned::Core {
            kind: brief::Core::Task,
            text: task_text,
            limit: env.limits.brief_core_budgets.task,
            priority: 0,
            required: true,
        })
        .expect("task brief room");
    if !context.dependencies.is_empty() || !context.spec.inputs.is_empty() {
        let Some(text) = crate::read_brief_part(core, text_limits, task, crate::BriefPart::Dependencies) else {
            work.push(Event::Tasks(tasks::Event::PreparationFailed { task }));
            out.push(Request::Decided);
            return Requests::Out(out);
        };
        wanted
            .push(brief::Planned::Core {
                kind: brief::Core::Results,
                text,
                limit: env.limits.brief_core_budgets.dependencies,
                priority: 1,
                required: true,
            })
            .expect("dependency result section room");
    }
    if tail_wanted(core, task, context.tries.transient != 0) {
        let Some(text) = crate::read_brief_part(core, text_limits, task, crate::BriefPart::TranscriptTail) else {
            work.push(Event::Tasks(tasks::Event::PreparationFailed { task }));
            out.push(Request::Decided);
            return Requests::Out(out);
        };
        wanted
            .push(brief::Planned::Core {
                kind: brief::Core::TranscriptTail,
                text,
                limit: env.limits.brief_core_budgets.tail,
                priority: 2,
                required: true,
            })
            .expect("tail brief room");
    }
    plan_note_index(&mut wanted, &env.limits, &lines, more);
    if !context.delegates.is_empty()
        && wanted.room() > 0
        && let Some(text) = crate::read_brief_part(core, text_limits, task, crate::BriefPart::Delegates)
    {
        wanted
            .push(brief::Planned::Core {
                kind: brief::Core::Plan,
                text,
                limit: env.limits.brief_core_budgets.plan,
                priority: 3,
                required: false,
            })
            .expect("delegate section room");
    }
    if context.tries != tasks::Tries::NONE
        && wanted.room() > 0
        && let Some(text) = crate::read_brief_part(core, text_limits, task, crate::BriefPart::Attempts)
    {
        wanted
            .push(brief::Planned::Core {
                kind: brief::Core::Attempts,
                text,
                limit: env.limits.brief_core_budgets.attempts,
                priority: 4,
                required: false,
            })
            .expect("attempt brief room");
    }
    let parent = brief_parent(context.requester);
    out.push(Request::Now(Box::new(Now::BriefCorePlanned { task, parent, sections: wanted })));
    out.push(Request::Decided);
    Requests::Out(out)
}

fn tail_wanted(core: &Core, task: u64, transient: bool) -> bool {
    let transcript = core.transcripts.get(&task).expect("brief owns a loaded transcript");
    if transcript.closed {
        return transcript.last.is_some();
    }
    core.transcript_oversized(task) || transient || !core.charter_of(task).policy.resume
}

pub(super) fn brief_parent(requester: tasks::Party) -> Option<u64> {
    match requester {
        tasks::Party::Task(parent) => Some(parent),
        tasks::Party::Person(_) | tasks::Party::Deployment { .. } => None,
    }
}

pub(super) fn plan_note_index(
    wanted: &mut List<brief::Planned>,
    limits: &Limits,
    lines: &List<notes::Line>,
    more: u32,
) {
    if wanted.room() > 0 {
        wanted
            .push(brief::Planned::Core {
                kind: brief::Core::NotesIndex,
                text: crate::note_index_text(lines, more, limits.brief.read_bytes.min(limits.brief_core_budgets.notes)),
                limit: limits.brief_core_budgets.notes,
                priority: 4,
                required: false,
            })
            .expect("note index section room");
    }
}

pub(super) fn route_prepared_agent(
    core: &mut Core,
    context: Box<tasks::RunContext>,
    transcript_waiter: Option<Token>,
    busy: bool,
) -> Requests {
    let mut out = Queue::with_capacity(2);
    if busy {
        remember_due(core, context);
    } else {
        let number = context.task;
        let previous_attempt = context.previous_attempt;
        let fresh = context.tries.transient != 0;
        super::result_reads::clear(core, number);
        assert!(core.contexts.insert(number, context).is_ok(), "bounded activation context");
        core.begin_transcript(number, previous_attempt);
        let transcript = core.transcripts.get_mut(&number).expect("begun transcript");
        if fresh && !transcript.closed {
            transcript.window = None;
        }
        out.push(Request::Now(Box::new(Now::StartPreparation {
            task: number,
            transcript_waiter,
            from: core.transcripts.get(&number).expect("begun transcript").window,
        })));
    }
    out.push(Request::Decided);
    Requests::Out(out)
}

pub(super) fn route_activate(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    context: Box<tasks::RunContext>,
    ready: bool,
) -> Requests {
    let number = context.task;
    let mut out = Queue::with_capacity(2);
    let Some(context) = core.tasks.run_context(number) else {
        out.push(Request::Decided);
        return Requests::Out(out);
    };
    if core.tasks.task(number).expect("current activation").phase != tasks::Phase::Active(tasks::Active::Due) {
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    if !ready || core.restart_step().is_some() || core.restart_failure().is_some() {
        remember_due(core, context);
    } else if core.tasks.recurring_template(number).is_none() {
        match context.executor {
            tasks::Executor::Person(_) => {}
            tasks::Executor::Procedure { connector, .. } => {
                if let Some(step) = context.previous_attempt.checked_add(1) {
                    out.push(Request::Ask { connector, ask: Ask::StartProcedure { context, step } });
                } else {
                    work.push(Event::Tasks(tasks::Event::Hold { task: number, why: tasks::Hold::Effects }));
                }
            }
            tasks::Executor::Agent { .. } => match core.run_admission(&context, env.wall, Box::new([])) {
                crate::RunAdmission::Allow(_) => out.push(Request::Now(Box::new(Now::PrepareAgent { context }))),
                crate::RunAdmission::Wait(_) => remember_due(core, context),
                crate::RunAdmission::Hold(why) => {
                    work.push(Event::Tasks(tasks::Event::Hold { task: number, why }));
                }
            },
        }
    }
    out.push(Request::Decided);
    Requests::Out(out)
}

pub(super) fn route_preparation_failed(core: &mut Core, work: &mut Queue<Event>, task: u64) -> Requests {
    super::result_reads::clear(core, task);
    drop(core.contexts.remove(&task));
    drop(core.dependency_results.remove(&task));
    drop(core.transcripts.remove(&task));
    work.push(Event::Tasks(tasks::Event::PreparationFailed { task }));
    let mut out = Queue::with_capacity(1);
    out.push(Request::Decided);
    Requests::Out(out)
}

pub(super) fn route_claim_refused(core: &mut Core, work: &mut Queue<Event>, task: u64) -> Requests {
    let mut out = Queue::with_capacity(2);
    if core.claiming.remove(&task).is_some() {
        core.restore_replaced_proof(task);
        out.push(Request::Now(Box::new(Now::DropAssignment { task })));
        work.push(Event::Tasks(tasks::Event::PreparationFailed { task }));
    }
    out.push(Request::Decided);
    Requests::Out(out)
}

#[expect(clippy::manual_map, reason = "the strict subset uses explicit matches without closures")]
pub(super) fn route_claim_prepared(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    task: u64,
    attempt: u64,
    budget: u64,
    writes: Box<[tasks::Name]>,
) -> Requests {
    if core.claiming.get(&task) == Some(&attempt) {
        if let Some(context) = core.contexts.remove(&task) {
            let valid = match core.tasks.run_context(task) {
                Some(current) if current == context => match core.run_admission(&current, env.wall, Box::new([])) {
                    crate::RunAdmission::Allow(allowance) => {
                        let proof = core.proofs.get(&task).expect("pending proof");
                        allowance.spend == budget
                            && allowance.choice.accounts == proof.accounts
                            && Some(allowance.choice.placement.channel) == proof.placement
                    }
                    crate::RunAdmission::Wait(_) | crate::RunAdmission::Hold(_) => false,
                },
                Some(_) | None => false,
            };
            if !valid {
                let _: Option<u64> = core.claiming.remove(&task);
                core.restore_replaced_proof(task);
                work.push(Event::Tasks(tasks::Event::PreparationWaiting { task }));
                let mut out = Queue::with_capacity(2);
                out.push(Request::Now(Box::new(Now::DropAssignment { task })));
                out.push(Request::Decided);
                return Requests::Out(out);
            }
        }

        let earlier_transcript = match core.proofs.get(&task) {
            Some(proof) => proof.conversation != attempt,
            None => false,
        };
        let turned = match core.tasks.task(task) {
            Some(row) => row.ever_turned,
            None => false,
        };
        let resumed = earlier_transcript && turned;
        work.push(Event::Tasks(tasks::Event::Claim {
            account: match core.proofs.get(&task) {
                Some(proof) => match proof.accounts.first() {
                    Some(binding) => Some(binding.account),
                    None => None,
                },
                None => None,
            },
            reply_to: ReplyTo::new(Token::new(task)),
            task,
            attempt,
            resumed,
            budget,
            writes,
        }));
    }
    let mut out = Queue::with_capacity(1);
    out.push(Request::Decided);
    Requests::Out(out)
}

pub(super) fn transcript_unread(core: &mut Core, work: &mut Queue<Event>, task: u64) -> Requests {
    if let Some(transcript) = core.transcripts.get(&task)
        && transcript.closed
    {
        let previous_attempt = transcript.previous_attempt;
        drop(core.transcripts.remove(&task));
        core.begin_transcript(task, previous_attempt);
        core.transcripts.get_mut(&task).expect("restarted empty transcript").window = None;
        work.push(Event::StartBrief { task });
    } else if core.transcripts.remove(&task).is_some() {
        super::result_reads::clear(core, task);
        drop(core.contexts.remove(&task));
        drop(core.dependency_results.remove(&task));
        work.push(Event::Tasks(tasks::Event::PreparationUnread { task }));
    }
    let mut out = Queue::with_capacity(1);
    out.push(Request::Decided);
    Requests::Out(out)
}

#[cfg(test)]
#[path = "../tests/closed_unread.rs"]
mod tests;
