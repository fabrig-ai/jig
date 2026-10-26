//! Named run calls, turns and terminals routed to tasks, authority and the root
//! (`domain/engine.md`, sections 4.4 and 7). Delegation and preparation
//! are separate families so each route file stays under 1,500 lines.

use super::NoteRoute;
use super::run_delegation::propose_batch;
use crate::{
    CallKey, CallPart, CallRecord, Core, CoreRecord, EscalationChoice, Event, Held, Limits, NamedAction,
    PayloadRefusal, ProposalChoice, Record, Request, Requests, RoutedCall, ToolKind, Write,
};
use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_fleet as fleet;
use jig_domain_notes as notes;
use jig_domain_tasks as tasks;
use skein_lib::{Env, Queue, ReplyTo, Token};

pub(super) fn call_decision(core: &mut Core, out: &mut Queue<Request>, to: ReplyTo, key: CallKey, part: CallPart) {
    if core.decide_named_call(key, part.clone()) {
        out.push(Request::Write(Write::Save(Record::Core(CoreRecord::Call(CallRecord {
            key,
            part: part.clone(),
            settled: core.call_settled.get(&key).cloned(),
        })))));
    }
    out.push(Request::Held(Box::new(Held::CallAnswer { to, key, part })));
}

pub(super) fn escalation_refused(
    core: &mut Core,
    to: ReplyTo,
    key: CallKey,
    task: u64,
    why: tasks::Refusal,
) -> Requests {
    let mut out = Queue::with_capacity(3);
    call_decision(
        core,
        &mut out,
        to,
        key,
        CallPart::EscalationRefused(tasks::Problem { task: Some(task), why, blocked_by: None, capacity: None }),
    );
    out.push(Request::Decided);
    Requests::Out(out)
}

pub(super) fn proposal_refused(core: &mut Core, to: ReplyTo, key: CallKey, why: tasks::Refusal) -> Requests {
    let mut out = Queue::with_capacity(3);
    call_decision(
        core,
        &mut out,
        to,
        key,
        CallPart::ProposalRefused(tasks::Problem { task: Some(key.task), why, blocked_by: None, capacity: None }),
    );
    out.push(Request::Decided);
    Requests::Out(out)
}

#[expect(clippy::too_many_lines, reason = "one exhaustive authenticated task proposal dispatcher")]
pub(super) fn named_proposal_accept(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    to: ReplyTo,
    key: CallKey,
    proposal: tasks::Proposal,
) -> Requests {
    let token = to.into_token();
    let to = ReplyTo::new(token);
    if super::proposal_revisions::call(core, work, token, key, &proposal) {
        let mut out = Queue::with_capacity(1);
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    let result_followups = core.tasks.result_proposal(proposal.proposer, proposal.number);
    if let Err(why) = core.proposal_accept_allowed(
        &env.limits,
        proposal.proposer,
        key.task,
        proposal.project,
        &proposal.action,
        env.wall,
    ) {
        return proposal_refused(core, to, key, why);
    }
    match proposal.action {
        tasks::ProposalAction::Effect { connector, .. } | tasks::ProposalAction::ProcedureEffect { connector, .. } => {
            assert!(core.pending_calls.insert(key, true).is_ok(), "effect acceptance call room");
            let owner = to.into_token();
            return crate::effects::start(
                core,
                env,
                owner,
                connector,
                crate::EffectOrigin::Accept {
                    to: ReplyTo::new(owner),
                    key: Some(key),
                    proposal: Box::new(proposal),
                    by: tasks::Party::Task(key.task),
                },
            );
        }
        tasks::ProposalAction::Settings { .. }
        | tasks::ProposalAction::Batch(_)
        | tasks::ProposalAction::Amend { .. }
        | tasks::ProposalAction::Widen { .. }
        | tasks::ProposalAction::Release { .. } => {}
    }
    let Some(message) = crate::fresh(&mut core.counters, crate::Family::Message) else {
        return proposal_refused(core, to, key, tasks::Refusal::Busy);
    };
    let token = to.into_token();
    let event = match proposal.action {
        tasks::ProposalAction::Settings { .. } => unreachable!("tasks cannot accept settings"),
        tasks::ProposalAction::Effect { .. } | tasks::ProposalAction::ProcedureEffect { .. } => {
            unreachable!("effect accepted through its connector")
        }
        tasks::ProposalAction::Batch(mut batch) => {
            super::deadlines::resolve(&mut batch, env.wall);
            let creator =
                if proposal.as_holder { tasks::Party::Task(key.task) } else { tasks::Party::Task(proposal.proposer) };
            for member in &mut batch {
                member.funder = tasks::Funder::Task(key.task);
            }
            if result_followups {
                tasks::Event::MakeResultFollowups {
                    reply_to: ReplyTo::new(token),
                    proposer: proposal.proposer,
                    proposal: proposal.number,
                    batch,
                }
            } else {
                tasks::Event::Make { reply_to: ReplyTo::new(token), creator, batch }
            }
        }
        tasks::ProposalAction::Amend { task, amendment, .. } => {
            let Some(message) = crate::fresh(&mut core.counters, crate::Family::Message) else {
                return proposal_refused(core, ReplyTo::new(token), key, tasks::Refusal::Busy);
            };
            let Some(current) = core.tasks.delegation(task) else {
                return proposal_refused(core, ReplyTo::new(token), key, tasks::Refusal::Unknown);
            };
            let after = crate::translate::authority_value(amendment.authority.as_ref().unwrap_or(&current.authority));
            let before = crate::translate::authority_value(&current.authority);
            let stop_run = !authority::at_most(&before, &after, &core.authority.rules().implies);
            tasks::Event::Amend {
                places: Box::new([]),
                funder: Some(tasks::Funder::Task(key.task)),
                reply_to: ReplyTo::new(token),
                by: tasks::Party::Task(key.task),
                task,
                message,
                stop_run,
                amendment,
            }
        }
        tasks::ProposalAction::Widen { task, authority } => {
            let Some(message) = crate::fresh(&mut core.counters, crate::Family::Message) else {
                return proposal_refused(core, ReplyTo::new(token), key, tasks::Refusal::Busy);
            };
            tasks::Event::Amend {
                places: Box::new([]),
                funder: Some(tasks::Funder::Task(key.task)),
                reply_to: ReplyTo::new(token),
                by: tasks::Party::Task(key.task),
                task,
                message,
                stop_run: false,
                amendment: tasks::Amendment {
                    tracked: None,
                    addressed: None,
                    release: None,
                    instructions: None,
                    spec: None,
                    wake: None,
                    dependencies: None,
                    authority: Some(authority),
                    llm: None,
                    reason: proposal.reason,
                },
            }
        }
        tasks::ProposalAction::Release { task } => tasks::Event::Control {
            reply_to: ReplyTo::new(token),
            by: tasks::Party::Task(key.task),
            task,
            control: tasks::Control::Release { again: None, message: None },
        },
    };
    assert!(core.pending_calls.insert(key, true).is_ok(), "proposal acceptance room reserved");
    assert!(
        core.routing_calls.insert(
            token,
            RoutedCall::Accepting { key, proposer: proposal.proposer, proposal: proposal.number, message }
        ) == Ok(None),
        "one acceptance route"
    );
    work.push(Event::Tasks(event));
    let mut out = Queue::with_capacity(1);
    out.push(Request::Decided);
    Requests::Out(out)
}

#[expect(clippy::too_many_arguments, reason = "one named proposal decision carries its call and proposal identity")]
pub(super) fn named_proposal_decide(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    to: ReplyTo,
    key: CallKey,
    proposer: u64,
    proposal: u64,
    choice: ProposalChoice,
) -> Requests {
    let Some(pending) = core.tasks.proposal(proposer, proposal) else {
        return proposal_refused(core, to, key, tasks::Refusal::Unknown);
    };
    let current = match pending.state {
        tasks::ProposalState::Pending { holder, .. } => holder,
        tasks::ProposalState::Accepted { .. }
        | tasks::ProposalState::Rejected { .. }
        | tasks::ProposalState::Withdrawn => {
            return proposal_refused(core, to, key, tasks::Refusal::State);
        }
    };
    if current != tasks::ProposalHolder::Task(key.task) {
        return proposal_refused(core, to, key, tasks::Refusal::Reference);
    }
    let next = match choice {
        ProposalChoice::Accept => return named_proposal_accept(core, env, work, to, key, pending),
        ProposalChoice::Reject { reason } => tasks::ProposalDecision::Reject { reason },
        ProposalChoice::Pass => {
            let Some(holder) = core.proposal_holder(&env.limits, proposer, &pending.action, Some(current), env.wall)
            else {
                return proposal_refused(core, to, key, tasks::Refusal::Reference);
            };
            tasks::ProposalDecision::Pass { holder }
        }
    };
    let message = match next {
        tasks::ProposalDecision::Reject { .. } => match crate::fresh(&mut core.counters, crate::Family::Message) {
            Some(message) => Some(message),
            None => return proposal_refused(core, to, key, tasks::Refusal::Busy),
        },
        tasks::ProposalDecision::Pass { .. } => None,
        tasks::ProposalDecision::Accept => unreachable!("accept routed through action"),
    };
    let token = to.into_token();
    assert!(core.pending_calls.insert(key, true).is_ok(), "proposal decision room reserved");
    assert!(core.routing_calls.insert(token, RoutedCall::Decide { key, proposal }) == Ok(None), "one route");
    work.push(Event::Tasks(tasks::Event::DecideProposal {
        reply_to: ReplyTo::new(token),
        proposer,
        proposal,
        message,
        by: tasks::Party::Task(key.task),
        decision: next,
    }));
    let mut out = Queue::with_capacity(1);
    out.push(Request::Decided);
    Requests::Out(out)
}

#[expect(clippy::too_many_arguments, reason = "one named proposal retains its action and call correlation")]
pub(crate) fn named_propose(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    to: ReplyTo,
    key: CallKey,
    mut action: tasks::ProposalAction,
    reason: Box<[u8]>,
    as_holder: bool,
) -> Requests {
    if reason.len() > usize::try_from(env.limits.tasks.message_bytes).expect("u32 fits usize") {
        return proposal_refused(core, to, key, tasks::Refusal::Read);
    }
    let Some(context) = core.tasks.delegation(key.task) else {
        return proposal_refused(core, to, key, tasks::Refusal::Unknown);
    };
    if let tasks::ProposalAction::Widen { task, .. } = &mut action {
        *task = key.task;
    }
    core.keep_proposal_models(key.task, &mut action);
    if let tasks::ProposalAction::Amend { task, revision, amendment } = &action
        && let Err(jig_domain_people::Refusal::Moved { now }) = core.party_amend_revision(*task, *revision, amendment)
    {
        let mut out = Queue::with_capacity(3);
        call_decision(core, &mut out, to, key, CallPart::Moved { task: *task, now });
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    if let Err(why) = core.proposal_admit_action(context.project, &action, env.wall) {
        return proposal_refused(core, to, key, why);
    }
    let Some(holder) = core.proposal_holder(&env.limits, key.task, &action, None, env.wall) else {
        return proposal_refused(core, to, key, tasks::Refusal::Reference);
    };
    if !crate::acts::bounds::proposal_has_recipient(core, holder) {
        return proposal_refused(core, to, key, tasks::Refusal::Unproposable);
    }
    let Some(number) = crate::fresh(&mut core.counters, crate::Family::Message) else {
        return proposal_refused(core, to, key, tasks::Refusal::Busy);
    };
    let token = to.into_token();
    assert!(core.pending_calls.insert(key, true).is_ok(), "proposal call room reserved");
    assert!(core.routing_calls.insert(token, RoutedCall::Propose { key, proposal: number }) == Ok(None), "one route");
    work.push(Event::Tasks(tasks::Event::Propose {
        reply_to: ReplyTo::new(token),
        proposal: tasks::Proposal {
            number,
            proposer: key.task,
            project: context.project,
            action,
            reason,
            as_holder,
            state: tasks::ProposalState::Pending { holder, since: env.wall },
        },
    }));
    let mut out = Queue::with_capacity(1);
    out.push(Request::Decided);
    Requests::Out(out)
}

#[expect(clippy::too_many_arguments, reason = "one named holder choice carries its call, task and revision")]
pub(super) fn named_escalation(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    to: ReplyTo,
    key: CallKey,
    task: u64,
    revision: u64,
    choice: EscalationChoice,
) -> Requests {
    let Some(context) = core.tasks.escalation(task) else {
        return escalation_refused(core, to, key, task, tasks::Refusal::Unknown);
    };
    let (current, holder) = match context.escalation {
        tasks::Escalation::Waiting { revision, holder, .. } => (revision, holder),
        tasks::Escalation::Unheld { .. } | tasks::Escalation::Routing { .. } | tasks::Escalation::Rejected { .. } => {
            return escalation_refused(core, to, key, task, tasks::Refusal::State);
        }
    };
    if let Err(jig_domain_people::Refusal::Moved { now }) = core.hold_revision(task, revision) {
        let mut out = Queue::with_capacity(3);
        call_decision(core, &mut out, to, key, CallPart::Moved { task, now });
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    if current != revision || holder != tasks::EscalationHolder::Task(key.task) {
        return escalation_refused(core, to, key, task, tasks::Refusal::Reference);
    }
    let role = core.people.role(context.requester, context.project);
    let mut passed_up = false;
    let semantic = match choice {
        EscalationChoice::Release => {
            if core.escalation_recipient(&env.limits, &context, role) == Some(holder) {
                tasks::EscalationDecision::Release
            } else {
                let Some(next) = core.escalation_recipient_after(&env.limits, &context, role, Some(holder)) else {
                    return escalation_refused(core, to, key, task, tasks::Refusal::Funding);
                };
                passed_up = true;
                tasks::EscalationDecision::Pass { holder: next }
            }
        }
        EscalationChoice::Reject { reason } => tasks::EscalationDecision::Reject { reason },
        EscalationChoice::Pass => {
            let Some(next) = core.escalation_recipient_after(&env.limits, &context, role, Some(holder)) else {
                return escalation_refused(core, to, key, task, tasks::Refusal::Reference);
            };
            tasks::EscalationDecision::Pass { holder: next }
        }
    };
    let entry = match semantic {
        tasks::EscalationDecision::Pass { .. } => match crate::fresh(&mut core.counters, crate::Family::Message) {
            Some(entry) => Some(entry),
            None => return escalation_refused(core, to, key, task, tasks::Refusal::Busy),
        },
        tasks::EscalationDecision::Release | tasks::EscalationDecision::Reject { .. } => None,
    };
    let token = to.into_token();
    assert!(core.pending_calls.insert(key, true).is_ok(), "task escalation call room reserved");
    assert!(
        core.routing_calls.insert(token, RoutedCall::Escalation { key, task, revision, passed_up }) == Ok(None),
        "one route"
    );
    work.push(Event::TaskEscalation(tasks::Event::DecideEscalation {
        reply_to: ReplyTo::new(token),
        task,
        revision,
        by: key.task,
        entry,
        decision: semantic,
    }));
    let mut out = Queue::with_capacity(1);
    out.push(Request::Decided);
    Requests::Out(out)
}

#[expect(clippy::too_many_lines, reason = "one generic named-call vocabulary routes its finite task continuations")]
pub(super) fn named_action(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    to: ReplyTo,
    key: CallKey,
    action: NamedAction,
) -> Requests {
    assert!(core.current_proof(key.task, key.attempt), "named action belongs to current claim");
    if let Some(part) = core.call_parts.get(&key) {
        let mut out = Queue::with_capacity(2);
        out.push(Request::Held(Box::new(Held::CallAnswer { to, key, part: part.clone() })));
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    if super::amendment_reads::waiting(core, key) {
        let mut out = Queue::with_capacity(1);
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    if let Some(part) = core.authorize_tool(key, named_tool_kind(&action)) {
        let mut out = Queue::with_capacity(3);
        call_decision(core, &mut out, to, key, part);
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    let token = to.into_token();
    let mut out = Queue::with_capacity(3);
    match action {
        NamedAction::ProposeSettings { against, items, reason } => {
            return super::settings::propose_run(
                core,
                env,
                work,
                ReplyTo::new(token),
                key,
                crate::settings::Proposed { against, items, reason },
            );
        }
        NamedAction::Message { target, kind, words } => {
            let Some(context) = core.tasks.delegation(key.task) else {
                call_decision(
                    core,
                    &mut out,
                    ReplyTo::new(token),
                    key,
                    CallPart::MessageRefused(tasks::Problem {
                        task: Some(key.task),
                        why: tasks::Refusal::Unknown,
                        blocked_by: None,
                        capacity: None,
                    }),
                );
                out.push(Request::Decided);
                return Requests::Out(out);
            };
            let project = context.project;
            let Some(number) = crate::fresh(&mut core.counters, crate::Family::Message) else {
                call_decision(
                    core,
                    &mut out,
                    ReplyTo::new(token),
                    key,
                    CallPart::MessageRefused(tasks::Problem {
                        task: Some(target),
                        why: tasks::Refusal::Busy,
                        blocked_by: None,
                        capacity: None,
                    }),
                );
                out.push(Request::Decided);
                return Requests::Out(out);
            };
            assert!(core.pending_calls.insert(key, true).is_ok(), "call record room reserved");
            assert!(core.routing_calls.insert(token, RoutedCall::Message(key)) == Ok(None), "one live routed call");
            work.push(Event::Tasks(tasks::Event::Message {
                reply_to: ReplyTo::new(token),
                project,
                task: target,
                word: tasks::Word {
                    number,
                    from: tasks::Party::Task(key.task),
                    kind,
                    words,
                    at: env.wall,
                    hits: 1,
                    eligible: false,
                },
            }));
        }
        NamedAction::Introduce { left, right } => {
            assert!(core.pending_calls.insert(key, true).is_ok(), "call record room reserved");
            assert!(core.routing_calls.insert(token, RoutedCall::Introduce(key)) == Ok(None), "one live routed call");
            work.push(Event::Tasks(tasks::Event::Introduce {
                reply_to: ReplyTo::new(token),
                by: key.task,
                left,
                right,
            }));
        }
        NamedAction::Subscribe { kind } => {
            let Some(subscription) = crate::fresh(&mut core.counters, crate::Family::Message) else {
                call_decision(
                    core,
                    &mut out,
                    ReplyTo::new(token),
                    key,
                    CallPart::SubscriptionRefused(tasks::Problem {
                        task: Some(key.task),
                        why: tasks::Refusal::Busy,
                        blocked_by: None,
                        capacity: None,
                    }),
                );
                out.push(Request::Decided);
                return Requests::Out(out);
            };
            assert!(core.pending_calls.insert(key, true).is_ok(), "call record room reserved");
            assert!(
                core.routing_calls.insert(token, RoutedCall::Subscribe { key, subscription }) == Ok(None),
                "one live routed call"
            );
            work.push(Event::Tasks(tasks::Event::Subscribe {
                reply_to: ReplyTo::new(token),
                task: key.task,
                subscription: tasks::Subscription { number: subscription, kind },
            }));
        }
        NamedAction::Unsubscribe { subscription } => {
            assert!(core.pending_calls.insert(key, true).is_ok(), "call record room reserved");
            assert!(core.routing_calls.insert(token, RoutedCall::Unsubscribe(key)) == Ok(None), "one live routed call");
            work.push(Event::Tasks(tasks::Event::Unsubscribe {
                reply_to: ReplyTo::new(token),
                task: key.task,
                subscription,
            }));
        }
        NamedAction::Release { target, revision, again } => {
            let message = match core.release_admit(target, revision, again) {
                Ok(message) => message,
                Err(why) => {
                    let part = match why {
                        jig_domain_people::Refusal::Moved { now } => CallPart::Moved { task: target, now },
                        jig_domain_people::Refusal::Authority {
                            lacking: jig_domain_people::Lack::Again { required },
                            ..
                        } => CallPart::Again { task: target, required },
                        why @ (jig_domain_people::Refusal::Unknown | jig_domain_people::Refusal::Limit { .. }) => {
                            CallPart::ControlRefused(tasks::Problem {
                                task: Some(target),
                                why: if why == jig_domain_people::Refusal::Unknown {
                                    tasks::Refusal::Unknown
                                } else {
                                    tasks::Refusal::Busy
                                },
                                blocked_by: None,
                                capacity: None,
                            })
                        }
                        jig_domain_people::Refusal::Stale { .. }
                        | jig_domain_people::Refusal::Full { .. }
                        | jig_domain_people::Refusal::Circle { .. }
                        | jig_domain_people::Refusal::Dependency { .. }
                        | jig_domain_people::Refusal::Kept { .. }
                        | jig_domain_people::Refusal::Role { .. }
                        | jig_domain_people::Refusal::Authority { .. }
                        | jig_domain_people::Refusal::Ended { .. }
                        | jig_domain_people::Refusal::Taken { .. }
                        | jig_domain_people::Refusal::NotWaiting
                        | jig_domain_people::Refusal::NoFurther
                        | jig_domain_people::Refusal::Provider
                        | jig_domain_people::Refusal::SignInRedirect { .. }
                        | jig_domain_people::Refusal::AccountMove { .. }
                        | jig_domain_people::Refusal::NameTaken
                        | jig_domain_people::Refusal::NotOffered
                        | jig_domain_people::Refusal::NotWellFormed { .. }
                        | jig_domain_people::Refusal::NoEscalationHolder
                        | jig_domain_people::Refusal::Llm { .. }
                        | jig_domain_people::Refusal::Account { .. }
                        | jig_domain_people::Refusal::KeyConflict => unreachable!(
                            "shared release admission returns revision, again, unknown or arithmetic refusals"
                        ),
                    };
                    call_decision(core, &mut out, ReplyTo::new(token), key, part);
                    out.push(Request::Decided);
                    return Requests::Out(out);
                }
            };
            assert!(core.pending_calls.insert(key, true).is_ok(), "call record room reserved");
            assert!(
                core.routing_calls.insert(token, RoutedCall::Control { key, target, revised: true }) == Ok(None),
                "one live routed call"
            );
            work.push(Event::Tasks(tasks::Event::Control {
                reply_to: ReplyTo::new(token),
                by: tasks::Party::Task(key.task),
                task: target,
                control: tasks::Control::Release { again, message },
            }));
        }
        NamedAction::Control { target, control } => {
            return super::run_controls::control(core, work, ReplyTo::new(token), key, target, control);
        }
        NamedAction::Amend { target, revision, amendment } => {
            return named_amend(
                core,
                env,
                work,
                token,
                key,
                target,
                revision,
                amendment,
                super::amendment_reads::Evidence::new(&env.limits.tasks),
            );
        }
        NamedAction::DecideEscalation { task, revision, choice } => {
            return named_escalation(core, env, work, ReplyTo::new(token), key, task, revision, choice);
        }
        NamedAction::WithdrawProposal { proposal } => {
            if core.tasks.proposal(key.task, proposal).is_none() {
                assert!(core.pending_calls.insert(key, true).is_ok(), "proposal call room reserved");
                out.push(Request::Now(Box::new(crate::Now::DecidedProposal {
                    to: ReplyTo::new(token),
                    key,
                    proposal,
                })));
                out.push(Request::Decided);
                return Requests::Out(out);
            }
            assert!(core.pending_calls.insert(key, true).is_ok(), "proposal call room reserved");
            assert!(
                core.routing_calls.insert(token, RoutedCall::Withdraw { key, proposal, moved: None }) == Ok(None),
                "one route"
            );
            work.push(Event::Tasks(tasks::Event::WithdrawProposal {
                reply_to: ReplyTo::new(token),
                proposer: key.task,
                proposal,
            }));
        }
        NamedAction::DecideProposal { proposer, proposal, choice } => {
            return named_proposal_decide(core, env, work, ReplyTo::new(token), key, proposer, proposal, choice);
        }
        NamedAction::Propose { action, reason, as_holder } => {
            return named_propose(core, env, work, ReplyTo::new(token), key, action, reason, as_holder);
        }
        NamedAction::ProposeBatch { batch, reason, as_holder } => {
            return propose_batch(core, env, ReplyTo::new(token), key, batch, reason, as_holder, None);
        }
        NamedAction::ProposeGoal { goal, priority, reason } => {
            return propose_batch(core, env, ReplyTo::new(token), key, Box::new([*goal]), reason, true, Some(priority));
        }
        NamedAction::Note { mut entry, recalled } => {
            if recalled.is_none() && entry.name != 0 {
                call_decision(core, &mut out, ReplyTo::new(token), key, CallPart::NoteRefused(notes::Refusal::Exists));
                out.push(Request::Decided);
                return Requests::Out(out);
            }
            if recalled.is_some() && entry.name == 0 {
                call_decision(core, &mut out, ReplyTo::new(token), key, CallPart::NoteRefused(notes::Refusal::Missing));
                out.push(Request::Decided);
                return Requests::Out(out);
            }
            if entry.name == 0 {
                let Some(name) = crate::fresh(&mut core.counters, crate::Family::Message) else {
                    call_decision(
                        core,
                        &mut out,
                        ReplyTo::new(token),
                        key,
                        CallPart::NoteRefused(notes::Refusal::Full),
                    );
                    out.push(Request::Decided);
                    return Requests::Out(out);
                };
                entry.name = name;
            }
            entry.author = notes::Author::Task { task: key.task, attempt: key.attempt };
            assert!(core.pending_calls.insert(key, true) == Ok(None), "note call record room reserved");
            let owner = core.begin_note_route(NoteRoute::Call { to: ReplyTo::new(token), key });
            work.push(Event::Notes(notes::Event::Write { owner, entry, recalled }));
        }
        NamedAction::Recall { by, page } => {
            assert!(core.pending_calls.insert(key, true) == Ok(None), "recall call record room reserved");
            let owner = core.begin_note_route(NoteRoute::Call { to: ReplyTo::new(token), key });
            work.push(Event::Notes(notes::Event::Recall { owner, by, page }));
        }
    }
    out.push(Request::Decided);
    Requests::Out(out)
}

pub(super) fn named_tool_kind(action: &NamedAction) -> ToolKind {
    match action {
        NamedAction::ProposeSettings { .. } => ToolKind::Settings,
        NamedAction::Message { target, .. } => ToolKind::Message { target: *target },
        NamedAction::Introduce { left, .. } => ToolKind::Message { target: *left },
        NamedAction::Subscribe { .. } | NamedAction::Unsubscribe { .. } => ToolKind::Subscribe,
        NamedAction::Control { .. } | NamedAction::Amend { .. } | NamedAction::Release { .. } => ToolKind::Control,
        NamedAction::DecideEscalation { .. }
        | NamedAction::WithdrawProposal { .. }
        | NamedAction::DecideProposal { .. } => ToolKind::Decide,
        NamedAction::Propose { .. } | NamedAction::ProposeBatch { .. } | NamedAction::ProposeGoal { .. } => {
            ToolKind::Propose
        }
        NamedAction::Note { entry, .. } => ToolKind::Note { scope: entry.scope.clone() },
        NamedAction::Recall { by, .. } => ToolKind::Recall { by: by.clone() },
    }
}

pub(super) fn route_named_answer(core: &mut Core, to: ReplyTo, key: CallKey, part: CallPart) -> Requests {
    assert!(core.current_proof(key.task, key.attempt), "named call belongs to current claim");
    let mut out = Queue::with_capacity(3);
    call_decision(core, &mut out, to, key, part);
    out.push(Request::Decided);
    Requests::Out(out)
}

pub(super) fn route_refused_payload(
    core: &mut Core,
    work: &mut Queue<Event>,
    request: Token,
    problem: tasks::Problem,
    payload: Option<PayloadRefusal>,
) -> Requests {
    match payload {
        Some(PayloadRefusal::Turn { task, attempt, turn }) if problem.task == Some(task) => {
            work.push(Event::Fleet(fleet::Event::TurnBusy {
                run: Token::new(task),
                attempt: Token::new(attempt),
                turn,
            }));
        }
        Some(PayloadRefusal::Answer { task, attempt })
            if problem.task == Some(task) && core.current_proof(task, attempt) =>
        {
            if core.committed_terminal(task, attempt).is_some() {
                let mut out = Queue::with_capacity(1);
                out.push(Request::Decided);
                return Requests::Out(out);
            }
            let proof = core.proofs.get_mut(&task).expect("answer proof reserved");
            proof.terminal = Some(crate::TerminalRecord {
                task,
                attempt,
                cumulative: match proof.turn {
                    Some(turn) => turn.cumulative,
                    None => 0,
                },
                end: tasks::End::Failed(tasks::Class::Invalid),
            });
            work.push(Event::Tasks(tasks::Event::Activation {
                reply_to: ReplyTo::new(request),
                task,
                attempt,
                end: tasks::End::Failed(tasks::Class::Invalid),
                saved: None,
                cause: tasks::Cause::Unpriced,
            }));
        }
        Some(PayloadRefusal::Turn { .. } | PayloadRefusal::Answer { .. }) | None => {}
    }
    let mut out = Queue::with_capacity(1);
    out.push(Request::Decided);
    Requests::Out(out)
}

#[expect(clippy::too_many_arguments, reason = "replay retains the original keyed amendment")]
pub(super) fn named_amend(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    token: Token,
    key: CallKey,
    target: u64,
    revision: u64,
    mut amendment: tasks::Amendment,
    evidence: super::amendment_reads::Evidence,
) -> Requests {
    let mut out = Queue::with_capacity(3);
    if let Some(part) = core.authorize_tool(key, ToolKind::Control) {
        call_decision(core, &mut out, ReplyTo::new(token), key, part);
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    if let Some(next) = super::amendment_reads::missing(
        core,
        &env.limits.tasks,
        tasks::Party::Task(key.task),
        target,
        amendment.dependencies.as_deref(),
        &evidence,
    ) {
        super::result_reads::wait(
            core,
            work,
            &mut out,
            next,
            super::amendment_reads::AmendmentRead {
                replay: super::amendment_reads::Replay::Agent { token, key, target, revision, amendment },
                evidence,
            },
        );
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    core.keep_models(target, &mut amendment);
    let stop_run = match core.task_amend_admit(key.task, target, &amendment, env.limits.tasks.depth, &env.limits.tasks)
    {
        Ok(stop_run) => stop_run,
        Err(crate::TaskAmendDenied::Refused { task, why }) => {
            call_decision(
                core,
                &mut out,
                ReplyTo::new(token),
                key,
                CallPart::ControlRefused(tasks::Problem { task: Some(task), why, blocked_by: None, capacity: None }),
            );
            out.push(Request::Decided);
            return Requests::Out(out);
        }
        Err(crate::TaskAmendDenied::Denied(answer)) => {
            call_decision(core, &mut out, ReplyTo::new(token), key, CallPart::ControlDenied { answer });
            out.push(Request::Decided);
            return Requests::Out(out);
        }
    };
    let moved = match core.party_amend_revision(target, revision, &amendment) {
        Ok(()) => None,
        Err(jig_domain_people::Refusal::Moved { now }) => Some(now),
        Err(_) => unreachable!("admitted amendment target exists"),
    };
    if let Some(now) = moved {
        call_decision(core, &mut out, ReplyTo::new(token), key, CallPart::Moved { task: target, now });
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    let Some(message) = crate::fresh(&mut core.counters, crate::Family::Message) else {
        call_decision(
            core,
            &mut out,
            ReplyTo::new(token),
            key,
            CallPart::ControlRefused(tasks::Problem {
                task: Some(target),
                why: tasks::Refusal::Busy,
                blocked_by: None,
                capacity: None,
            }),
        );
        out.push(Request::Decided);
        return Requests::Out(out);
    };
    assert!(core.pending_calls.insert(key, true).is_ok(), "call record room reserved");
    assert!(
        core.routing_calls.insert(token, RoutedCall::Control { key, target, revised: true }) == Ok(None),
        "one amendment route"
    );
    work.push(Event::Tasks(tasks::Event::Amend {
        places: evidence.places.into_boxed(),
        funder: None,
        reply_to: ReplyTo::new(token),
        by: tasks::Party::Task(key.task),
        task: target,
        message,
        stop_run,
        amendment,
    }));
    out.push(Request::Decided);
    Requests::Out(out)
}
