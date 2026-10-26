//! Assemble outputs in the decision that owns them. A copying application
//! replaces its tools and connector results; charter conversion stays shared
//! (domain/root.md, 7; domain/hosts.md, 8).
use crate::domain::Work;
use crate::{Domain, Limits};
use alloc::boxed::Box;
use jig_domain_accounts as accounts;
use jig_domain_brief as brief;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_host as host;
use jig_domain_tasks as tasks;
use jig_ops_domain_connectors::Connector;
use jig_protocol_charter as charter;
use skein_lib::{Decimal, Env, List, Queue, ReplyTo, Token};

pub(crate) fn brief(order: Box<[brief::GatherPlaced]>) -> Box<[charter::Section]> {
    let mut sections = List::with_capacity(u32::try_from(order.len()).expect("bounded sections"));
    for placed in order {
        let section = match placed {
            brief::GatherPlaced::Core { kind, text } => charter::Section { title: Box::from(title(kind)), text },
            brief::GatherPlaced::CoreMissing { kind, .. } => {
                charter::Section { title: Box::from(title(kind)), text: Box::from(&b"[missing]"[..]) }
            }
            brief::GatherPlaced::Missing { .. } => {
                charter::Section { title: Box::from(&b"Context"[..]), text: Box::from(&b"[missing]"[..]) }
            }
            brief::GatherPlaced::Connector { .. } => unreachable!("ops offers no connector brief sections"),
        };
        sections.push(section).expect("one section per placement");
    }
    sections.into_boxed()
}
const fn title(kind: brief::Core) -> &'static [u8] {
    match kind {
        brief::Core::Task => b"Task",
        brief::Core::Lineage => b"Requesters",
        brief::Core::Results => b"Results",
        brief::Core::Plan => b"Plan",
        brief::Core::Attempts => b"Attempts",
        brief::Core::Calls => b"Calls",
        brief::Core::Waiting => b"Waiting",
        brief::Core::NotesIndex => b"Notes",
        brief::Core::TranscriptTail => b"Conversation",
    }
}
#[expect(clippy::too_many_arguments, reason = "assemble the owners parts without a shared sibling type")]
pub(crate) fn assignment(
    domain: &Domain,
    env: &Env<Limits>,
    task: u64,
    attempt: u64,
    conversation: u64,
    live: core::LiveAsk,
    run: core::RunCharter,
    sections: Box<[charter::Section]>,
    transcript: Box<[Box<[u8]>]>,
    messages: Box<[tasks::Word]>,
    grants: Box<[accounts::Grant]>,
) -> Option<host::Assignment> {
    let policy = run.policy;
    let sources = run.sources;
    let mut models = List::with_capacity(u32::try_from(policy.alternatives.len()).ok()?);
    for (index, value) in policy.alternatives.into_iter().enumerate() {
        models.push(charter::Llm { model: model(value), grant: *sources.get(index.checked_add(1)?)? }).ok()?;
    }
    let value = charter::charter(
        charter::Charter {
            mode: match run.ended_by {
                jig_domain_tasks::EndedBy::Result => charter::Mode::Finite,
                jig_domain_tasks::EndedBy::Requester => charter::Mode::Open,
            },
            instructions: policy.instructions,
            tools: domain.tools.clone(),
            wait: true,
            agents: policy.agents,
            workspace: charter::WorkspaceTools { inspect: false, modify: false, shell: false },
            conventions: None,
            contract: contract(run.contract)?,
            budget: charter::Budget {
                turns: policy.turns,
                spend: run.budget,
                time: policy.time,
                reserve: charter::Reserve { turns: policy.reserve_turns, time: policy.reserve_time },
            },
            model: charter::Llm { model: model(policy.model), grant: *sources.first()? },
            models: models.into_boxed(),
            waiting: policy.waiting,
            resumes: policy.resume,
        },
        sections,
        !transcript.is_empty(),
    )?;
    let encoded = charter::encode(value, &domain.charter_endpoints, &env.limits.charter)?;
    let mut answered = List::with_capacity(env.limits.core.call_records);
    for core::Settled { key, call: value } in domain.core.settled_calls(task, attempt) {
        let answer = match value.answer {
            core::SettledAnswer::Host { error, body } => host::SettledAnswer::Host { error, body },
            core::SettledAnswer::Delivery { .. } => unreachable!("engine charter has no delivery tool"),
        };
        let name = charter::call_name(&charter::CallKey {
            activation: key.attempt,
            completion: key.completion,
            position: key.position,
        });
        answered.push(host::AnsweredCall { name, tool: value.tool, answer }).ok()?;
    }
    let mut start_messages = List::with_capacity(env.limits.core.tasks.run_inbox);
    for word in messages {
        start_messages
            .push(host::StartMessage {
                name: Token::new(word.number),
                sender: sender(word.from),
                finish_requested: word.finish_requested(),
                words: jig_protocol_tools::words(&word),
            })
            .ok()?;
    }
    let mut held = List::with_capacity(u32::try_from(grants.len()).ok()?);
    for grant in grants {
        held.push(host::Grant {
            form: accounts_form(grant.form),
            account: grant.account,
            generation: grant.generation,
            valid: grant.valid,
        })
        .ok()?;
    }
    Some(host::Assignment {
        messages: start_messages.into_boxed(),
        assignment: host::RunAssignment {
            identity: charter::run_identity(&domain.deployment, task, conversation),
            run: Token::new(task),
            attempt: Token::new(attempt),
            workspace: None,
            save: false,
            live: host::LiveAsk { text: live.text, reasoning: live.reasoning, input: live.input },
            charter: encoded,
            grants: held.into_boxed(),
        },
        turns: transcript,
        answered: answered.into_boxed(),
    })
}
fn model(value: core::Model) -> charter::Model {
    charter::Model {
        prices: charter::Prices {
            input: value.input_price,
            cached: value.cached_price,
            cache_write: value.cache_write_price,
            output: value.output_price,
            unit: value.price_unit,
        },
        dialect: value.dialect,
        endpoint: value.endpoint,
        name: value.name,
        window: value.window,
        output: value.output,
        effort: value.effort,
    }
}
fn contract(value: tasks::Contract) -> Option<charter::Contract> {
    Some(match value {
        tasks::Contract::Approval { .. } => return None,
        tasks::Contract::Report { words } => charter::Contract {
            report: Some(charter::TextRule { max: words, fields: Box::new([]) }),
            failure: Some(charter::TextRule { max: words, fields: Box::new([]) }),
            verdicts: Box::new([]),
            change: None,
        },
        tasks::Contract::Verdict { choices } => {
            let mut verdicts = List::with_capacity(u32::try_from(choices.len()).expect("verdict choices"));
            for choice in choices {
                let mut fields = List::with_capacity(u32::try_from(choice.fields.len()).ok()?);
                for field in choice.fields {
                    fields.push(charter::FieldRule { name: field.name, max: field.max }).ok()?;
                }
                verdicts
                    .push(charter::VerdictRule {
                        name: Box::from(Decimal::of(u64::from(choice.code)).as_bytes()),
                        text_max: choice.words,
                        fields: fields.into_boxed(),
                        items: charter::Items { min: 0, max: 0, kinds: Box::new([]) },
                    })
                    .expect("one verdict rule");
            }
            charter::Contract {
                report: None,
                failure: Some(charter::TextRule { max: 1024, fields: Box::new([]) }),
                verdicts: verdicts.into_boxed(),
                change: None,
            }
        }
        tasks::Contract::Change { words, .. } => charter::Contract {
            report: None,
            failure: Some(charter::TextRule { max: words, fields: Box::new([]) }),
            verdicts: Box::new([]),
            change: Some(charter::ChangeRule {
                checks_must_pass: true,
                fields: Box::new([
                    charter::FieldRule { name: Box::from(&b"title"[..]), max: words },
                    charter::FieldRule { name: Box::from(&b"body"[..]), max: words },
                ]),
            }),
        },
    })
}
pub(crate) fn sender(value: tasks::Party) -> Box<[u8]> {
    match value {
        tasks::Party::Task(number) | tasks::Party::Person(number) => Box::from(Decimal::of(number).as_bytes()),
        tasks::Party::Deployment { .. } => Box::from(&b"deployment"[..]),
    }
}
pub(crate) fn host_answer(value: core::SettledAnswer) -> host::Relayed {
    match value {
        core::SettledAnswer::Host { error, body } => host::Relayed::Answer { error, text: body },
        core::SettledAnswer::Delivery { .. } => unreachable!("engine charter has no delivery tool"),
    }
}

pub(crate) fn ending(value: host::Ending) -> tasks::End {
    match value {
        host::Ending::Refused(_) => tasks::End::Refused,
        host::Ending::Parked { .. } => tasks::End::Parked,
        host::Ending::Failed { failure, .. } => match failure {
            host::Failure::Run(host::RunFailure::Exceeded(value)) => tasks::End::Exceeded(tasks::Exceeded {
                bound: match value.bound {
                    host::Bound::Reasoning => tasks::Bound::Reasoning,
                    host::Bound::Metadata => tasks::Bound::Metadata,
                    host::Bound::Items => tasks::Bound::Items,
                    host::Bound::Answer => tasks::Bound::Answer,
                    host::Bound::Tools => tasks::Bound::Tools,
                    host::Bound::HistoryItems => tasks::Bound::HistoryItems,
                    host::Bound::History => tasks::Bound::History,
                    host::Bound::Window => tasks::Bound::Window,
                    host::Bound::Output => tasks::Bound::Output,
                },
                value: value.value,
            }),
            host::Failure::Unprepared(host::Preparation::Transient) => tasks::End::Failed(tasks::Class::Transient),
            host::Failure::Unprepared(host::Preparation::Permanent { .. }) => {
                tasks::End::Failed(tasks::Class::Permanent)
            }
            host::Failure::Run(
                host::RunFailure::Model
                | host::RunFailure::Budget
                | host::RunFailure::Policy
                | host::RunFailure::Cancelled
                | host::RunFailure::Stale
                | host::RunFailure::Limited { .. }
                | host::RunFailure::Refused { .. },
            ) => tasks::End::Failed(tasks::Class::Run),
            host::Failure::Agent(_) | host::Failure::Cancelled(_) => tasks::End::Failed(tasks::Class::Agent),
        },
        host::Ending::Ended { .. } => unreachable!("ended results leave for protocol reading"),
    }
}

pub(crate) fn answer_payload(domain: &mut Domain, run: Token, attempt: Token, payload: Token, work: &mut Queue<Work>) {
    let ending = match domain.payloads.get(&payload).expect("fenced answer payload") {
        crate::boundary::Payload::Answer { task, attempt: retained, ending, .. } => {
            assert!(*task == run.raw() && *retained == attempt.raw(), "fenced answer identity");
            ending.clone()
        }
        crate::boundary::Payload::Turn { .. }
        | crate::boundary::Payload::Word(_)
        | crate::boundary::Payload::Settled(_) => unreachable!("answer family"),
    };
    match ending {
        host::Ending::Ended { outcome, .. } => {
            crate::domain::now(domain, crate::Output::ReadResult { payload, outcome });
        }
        other @ (host::Ending::Refused(_) | host::Ending::Parked { .. } | host::Ending::Failed { .. }) => {
            result_read(domain, payload, self::ending(other), work);
        }
    }
}

pub(crate) fn result_read(domain: &Domain, payload: Token, end: tasks::End, work: &mut Queue<Work>) {
    let Some(crate::boundary::Payload::Answer { task, attempt, spent, read, refused, ending, .. }) =
        domain.payloads.get(&payload)
    else {
        return;
    };
    work.push(Work::Core(core::Event::AnswerPayload {
        grants_refused: grants_refused(ending),
        run: Token::new(*task),
        attempt: Token::new(*attempt),
        payload,
        cumulative: *spent,
        end,
        saved: None,
        invalid_saved: false,
        read: *read,
        refused: refusal_names(refused),
    }));
}

/// Take the owner's answer once, preserving the core's held or now mark.
pub(crate) fn relayed(
    domain: &mut Domain,
    run: Token,
    attempt: Token,
    call: Box<[u8]>,
    answer: Token,
) -> Option<crate::Released> {
    let value = match domain.payloads.remove(&answer).expect("settled call token") {
        crate::boundary::Payload::Settled(value) => value,
        crate::boundary::Payload::Turn { .. }
        | crate::boundary::Payload::Answer { .. }
        | crate::boundary::Payload::Word(_) => unreachable!("settled call family"),
    };
    let binding = domain.host_calls.remove(&call)?;
    assert!(binding.run == run && binding.attempt == attempt, "hub relay fence");
    Some(crate::Released::Host(host::Event::Down(host::LinkDown::Relayed {
        run,
        attempt,
        call,
        answer: host_answer(value.answer),
    })))
}

pub(crate) fn read_answer(
    domain: &mut Domain,
    call: crate::domain::ReadCall,
    error: bool,
    body: Box<[u8]>,
    work: &mut Queue<Work>,
) {
    let answer = domain.payload(crate::boundary::Payload::Settled(core::SettledCall {
        serial: 0,
        tool: call.tool,
        answer: core::SettledAnswer::Host { error, body },
    }));
    work.push(Work::Core(core::Event::Fleet(jig_domain_fleet::Event::Relayed { to: ReplyTo::new(call.to), answer })));
}

pub(crate) use crate::message_refusals::{message_read, refusal_names, refusals};

pub(crate) fn connector_read(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    task: u64,
    tool: Box<[u8]>,
    read: jig_ops_domain_observability::Read,
    work: &mut Queue<Work>,
) {
    let binding = crate::domain::ReadCall { to: to.into_token(), tool };
    let number = jig_ops_domain_connectors::number(Connector::Observability);
    let description = crate::translate::read_description(number, &read);
    let mut findings =
        Queue::with_capacity(jig_domain_authority::max_out(&env.limits.core.authority).expect("findings"));
    if domain.core.connector_read_admit(task, description, &mut findings) != jig_domain_authority::Answer::Allow
        || domain.read_calls.len() == domain.read_calls.capacity()
    {
        read_answer(domain, binding, true, Box::from(&b"refused"[..]), work);
    } else {
        let token = domain.token();
        domain.read_calls.insert(token, binding).expect("admitted read continuation");
        work.push(Work::Observability(jig_ops_domain_observability::Event::Read { token, read }));
    }
}

pub(crate) fn fleet_grants(grants: &[host::Grant]) -> Box<[fleet::Grant]> {
    let mut translated = List::with_capacity(u32::try_from(grants.len()).expect("bounded grants"));
    for grant in grants {
        translated
            .push(fleet::Grant {
                form: fleet_form(grant.form),
                account: grant.account,
                generation: grant.generation,
                valid: grant.valid,
            })
            .expect("grant room");
    }
    translated.into_boxed()
}

pub(crate) fn host_grant(grant: fleet::Grant) -> host::Grant {
    host::Grant {
        form: host_form(grant.form),
        account: grant.account,
        generation: grant.generation,
        valid: grant.valid,
    }
}

fn accounts_form(form: accounts::Form) -> host::Form {
    match form {
        accounts::Form::Credential(scheme) => host::Form::Credential(match scheme {
            accounts::Scheme::Bearer => host::Scheme::Bearer,
            accounts::Scheme::Key => host::Scheme::Key,
        }),
        accounts::Form::Application => host::Form::Application,
        accounts::Form::Program(name) => {
            host::Form::Program(host::SourceName::new(name.as_bytes()).expect("source name follows the same rules"))
        }
    }
}

fn fleet_form(form: host::Form) -> fleet::Form {
    match form {
        host::Form::Credential(scheme) => fleet::Form::Credential(match scheme {
            host::Scheme::Bearer => fleet::Scheme::Bearer,
            host::Scheme::Key => fleet::Scheme::Key,
        }),
        host::Form::Application => fleet::Form::Application,
        host::Form::Program(name) => {
            fleet::Form::Program(fleet::SourceName::new(name.as_bytes()).expect("source name follows the same rules"))
        }
    }
}

fn host_form(form: fleet::Form) -> host::Form {
    match form {
        fleet::Form::Credential(scheme) => host::Form::Credential(match scheme {
            fleet::Scheme::Bearer => host::Scheme::Bearer,
            fleet::Scheme::Key => host::Scheme::Key,
        }),
        fleet::Form::Application => host::Form::Application,
        fleet::Form::Program(name) => {
            host::Form::Program(host::SourceName::new(name.as_bytes()).expect("source name follows the same rules"))
        }
    }
}

fn grants_refused(ending: &host::Ending) -> bool {
    match ending {
        host::Ending::Refused(refusal) => match refusal {
            host::Refusal::Invalid(invalid) => match invalid {
                host::Invalid::Start { cause, .. } => *cause == host::StartCause::Grants,
                host::Invalid::Charter
                | host::Invalid::Transcript
                | host::Invalid::Grants
                | host::Invalid::Messages => false,
            },
            host::Refusal::Busy => false,
        },
        host::Ending::Ended { .. } | host::Ending::Parked { .. } | host::Ending::Failed { .. } => false,
    }
}
