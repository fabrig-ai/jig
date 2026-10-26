//! Decode attested calls and render their terminals outside the root (channel.md, 6.3).
use alloc::boxed::Box;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_tasks as tasks;
use jig_protocol_tools as kit;
use jig_test_domain as root;
use skein_lib::{Map, ReplyTo, Token};

type CallName = (Box<[u8]>, Box<[u8]>);
#[derive(Debug)]
pub(super) struct Tools {
    limits: kit::Limits,
    calls: Map<core::CallKey, CallName>,
}
impl Tools {
    pub fn new(limits: &core::Limits) -> Self {
        Self {
            limits: kit::Limits {
                input_bytes: limits.call_answer_bytes,
                words: limits.tasks.message_bytes,
                batch: limits.tasks.batch,
                depth: 16,
            },
            calls: Map::with_capacity(limits.call_records),
        }
    }
    pub fn decode(&mut self, to: ReplyTo, run: Token, attempt: Token, call: &fleet::Call) -> root::Event {
        let Some(name) = jig_protocol_charter::call_key(&call.name, attempt.raw()) else {
            return invalid(to, call, b"name", kit::Why::Malformed);
        };
        let key = core::CallKey {
            task: run.raw(),
            attempt: attempt.raw(),
            completion: name.completion,
            position: name.position,
        };
        if self.calls.len() == self.calls.capacity() && self.calls.get(&key).is_none() {
            return invalid(to, call, b"call", kit::Why::TooLarge);
        }
        let event = match kit::decode(&call.tool, &call.input, &self.limits) {
            kit::Decoded::Action(action) => core::Event::NamedAction { to, key, action },
            kit::Decoded::Delegate(batch) => core::Event::DelegateValidated { to, key, batch, stubs: Box::default() },
            kit::Decoded::Settings { .. } => return invalid(to, call, b"set", kit::Why::Range),
            kit::Decoded::Problem(problem) => return invalid(to, call, problem.field, problem.why),
            kit::Decoded::NotMine => return invalid(to, call, b"tool", kit::Why::Range),
        };
        self.calls.insert(key, (call.name.clone(), call.tool.clone())).expect("admitted call correlation");
        root::Event::Core(event)
    }
    pub fn render(
        &mut self,
        to: ReplyTo,
        key: core::CallKey,
        part: &core::CallPart,
        durable: bool,
    ) -> Option<root::Event> {
        let (name, tool) = self.calls.remove(&key)?;
        let rendered = kit::render(part);
        let call = core::SettledCall {
            serial: 0,
            tool,
            answer: core::SettledAnswer::Host { error: rendered.error, body: rendered.text },
        };
        Some(if durable {
            root::Event::Core(core::Event::SettledCall { to, key, name, call })
        } else {
            root::Event::CallAnswered { to, call }
        })
    }
    pub fn forget_name(&mut self, name: &[u8]) {
        let mut found = None;
        for (key, (candidate, _)) in &self.calls {
            if candidate.as_ref() == name {
                found = Some(*key);
                break;
            }
        }
        if let Some(key) = found {
            drop(self.calls.remove(&key));
        }
    }
    pub fn forget(&mut self, key: core::CallKey) {
        drop(self.calls.remove(&key));
    }
}
fn invalid(to: ReplyTo, call: &fleet::Call, field: &'static [u8], why: kit::Why) -> root::Event {
    let rendered = kit::problem(&call.tool, &kit::Problem { field, why });
    root::Event::CallAnswered {
        to,
        call: core::SettledCall {
            serial: 0,
            tool: call.tool.clone(),
            answer: core::SettledAnswer::Host { error: rendered.error, body: rendered.text },
        },
    }
}
pub(super) fn result(payload: Token, bytes: &[u8]) -> root::Event {
    let end = match jig_protocol_charter::result(bytes) {
        Ok(jig_protocol_charter::RunResult::Report { words }) => {
            tasks::End::Finished { result: tasks::TaskResult::Report { words }, cancel_delegates: false }
        }
        Ok(jig_protocol_charter::RunResult::Failure { reason }) => {
            tasks::End::Finished { result: tasks::TaskResult::Failure { reason }, cancel_delegates: false }
        }
        Ok(jig_protocol_charter::RunResult::Verdict { label, fields, words }) => verdict(label, fields, words),
        Err(_) => tasks::End::Failed(tasks::Class::Invalid),
    };
    root::Event::ResultRead { payload, end }
}
fn verdict(label: Box<[u8]>, fields: Box<[jig_protocol_charter::Field]>, words: Box<[u8]>) -> tasks::End {
    let Some(code) = number(&label) else {
        return tasks::End::Failed(tasks::Class::Invalid);
    };
    let mut mapped = skein_lib::List::with_capacity(u32::try_from(fields.len()).expect("bounded result fields"));
    for field in fields {
        mapped.push(tasks::ResultField { name: field.name, value: field.value }).expect("field room");
    }
    tasks::End::Finished {
        result: tasks::TaskResult::Verdict { code, fields: mapped.into_boxed(), words },
        cancel_delegates: false,
    }
}

pub(super) fn number(bytes: &[u8]) -> Option<u32> {
    if bytes.is_empty() {
        return None;
    }
    let mut value = 0_u32;
    for byte in bytes {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(u32::from(byte.checked_sub(b'0')?))?;
    }
    Some(value)
}

pub(super) fn charter(
    value: root::Assignment,
    names: &[jig_protocol_charter::EndpointName],
    deployment: &[u8; 16],
) -> Option<jig_domain_host::Assignment> {
    use jig_domain_host as host;
    use jig_protocol_charter as charter;
    use skein_lib::List;
    let policy = value.run.policy;
    let sources = value.run.sources;
    let mut models = List::with_capacity(u32::try_from(policy.alternatives.len()).ok()?);
    for (index, candidate) in policy.alternatives.into_iter().enumerate() {
        models.push(charter::Llm { model: model(candidate), grant: *sources.get(index.checked_add(1)?)? }).ok()?;
    }
    let mut sections = List::with_capacity(u32::try_from(value.brief.len()).ok()?);
    for placed in value.brief {
        let section = match placed {
            jig_domain_brief::GatherPlaced::Core { kind, text } => charter::Section { title: title(kind).into(), text },
            jig_domain_brief::GatherPlaced::CoreMissing { kind, .. } => {
                charter::Section { title: title(kind).into(), text: b"[missing]".as_slice().into() }
            }
            jig_domain_brief::GatherPlaced::Missing { .. } => {
                charter::Section { title: b"Context".as_slice().into(), text: b"[missing]".as_slice().into() }
            }
            jig_domain_brief::GatherPlaced::Connector { .. } => return None,
        };
        sections.push(section).ok()?;
    }
    let translated = charter::charter(
        charter::Charter {
            mode: mode(value.run.ended_by),
            instructions: policy.instructions,
            tools: declarations(policy.call_timeout, value.run.authority.tools.0)?,
            wait: true,
            agents: policy.agents,
            workspace: charter::WorkspaceTools { inspect: false, modify: false, shell: false },
            conventions: None,
            contract: contract(value.run.contract)?,
            budget: charter::Budget {
                turns: policy.turns,
                spend: value.run.budget,
                time: policy.time,
                reserve: charter::Reserve { turns: policy.reserve_turns, time: policy.reserve_time },
            },
            model: charter::Llm { model: model(policy.model), grant: *sources.first()? },
            models: models.into_boxed(),
            waiting: policy.waiting,
            resumes: policy.resume,
        },
        sections.into_boxed(),
        !value.transcript.is_empty(),
    )?;
    let encoded = charter::encode(translated, names, &charter::limits())?;
    let mut messages = List::with_capacity(u32::try_from(value.messages.len()).ok()?);
    for word in value.messages {
        messages
            .push(host::StartMessage {
                name: Token::new(word.number),
                sender: sender(word.from),
                finish_requested: word.finish_requested(),
                words: jig_protocol_tools::words(&word),
            })
            .ok()?;
    }
    let mut answered = List::with_capacity(u32::try_from(value.answered.len()).ok()?);
    for call in value.answered {
        answered.push(replay(call)).ok()?;
    }
    let mut grants = List::with_capacity(u32::try_from(value.grants.len()).ok()?);
    for grant in value.grants {
        grants
            .push(host::Grant {
                form: host_form(grant.form),
                account: grant.account,
                generation: grant.generation,
                valid: grant.valid,
            })
            .ok()?;
    }
    Some(host::Assignment {
        assignment: host::RunAssignment {
            run: Token::new(value.task),
            attempt: Token::new(value.attempt),
            identity: charter::run_identity(deployment, value.task, value.conversation),
            workspace: None,
            save: false,
            live: host::LiveAsk { text: value.live.text, reasoning: value.live.reasoning, input: value.live.input },
            charter: encoded,
            grants: grants.into_boxed(),
        },
        turns: value.transcript,
        answered: answered.into_boxed(),
        messages: messages.into_boxed(),
    })
}
fn replay(value: core::Settled) -> jig_domain_host::AnsweredCall {
    use jig_domain_host as host;
    let core::Settled { key, call } = value;
    host::AnsweredCall {
        name: jig_protocol_charter::call_name(&jig_protocol_charter::CallKey {
            activation: key.attempt,
            completion: key.completion,
            position: key.position,
        }),
        tool: call.tool,
        answer: match call.answer {
            core::SettledAnswer::Host { error, body } => host::SettledAnswer::Host { error, body },
            core::SettledAnswer::Delivery { outcome, evidence } => {
                host::SettledAnswer::Delivery { outcome: delivery(outcome), evidence }
            }
        },
    }
}
fn declarations(timeout: skein_lib::Duration, families: u64) -> Option<Box<[jig_protocol_charter::Tool]>> {
    let mut tools = skein_lib::List::with_capacity(u32::try_from(kit::declarations().len()).ok()?);
    for declaration in kit::declarations() {
        if families & family(declaration.name) == 0 {
            continue;
        }
        tools
            .push(jig_protocol_charter::Tool {
                name: declaration.name.into(),
                description: declaration.description.into(),
                schema: declaration.schema.into(),
                timeout,
                effect: if declaration.writes {
                    jig_protocol_charter::ToolEffect::Write
                } else {
                    jig_protocol_charter::ToolEffect::Read
                },
            })
            .ok()?;
    }
    Some(tools.into_boxed())
}
fn model(value: core::Model) -> jig_protocol_charter::Model {
    jig_protocol_charter::Model {
        prices: jig_protocol_charter::Prices {
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
fn contract(value: tasks::Contract) -> Option<jig_protocol_charter::Contract> {
    Some(match value {
        tasks::Contract::Approval { .. } => return None,
        tasks::Contract::Report { words } => jig_protocol_charter::Contract {
            report: Some(jig_protocol_charter::TextRule { max: words, fields: Box::new([]) }),
            failure: Some(jig_protocol_charter::TextRule { max: words, fields: Box::new([]) }),
            verdicts: Box::new([]),
            change: None,
        },
        tasks::Contract::Verdict { choices } => {
            let mut verdicts = skein_lib::List::with_capacity(u32::try_from(choices.len()).expect("verdict choices"));
            for choice in choices {
                let mut fields = skein_lib::List::with_capacity(u32::try_from(choice.fields.len()).ok()?);
                for field in choice.fields {
                    fields.push(jig_protocol_charter::FieldRule { name: field.name, max: field.max }).ok()?;
                }
                verdicts
                    .push(jig_protocol_charter::VerdictRule {
                        name: Box::from(skein_lib::Decimal::of(u64::from(choice.code)).as_bytes()),
                        text_max: choice.words,
                        fields: fields.into_boxed(),
                        items: jig_protocol_charter::Items { min: 0, max: 0, kinds: Box::new([]) },
                    })
                    .expect("one verdict rule");
            }
            jig_protocol_charter::Contract {
                report: None,
                failure: Some(jig_protocol_charter::TextRule { max: 1024, fields: Box::new([]) }),
                verdicts: verdicts.into_boxed(),
                change: None,
            }
        }
        tasks::Contract::Change { words, .. } => jig_protocol_charter::Contract {
            report: None,
            failure: Some(jig_protocol_charter::TextRule { max: words, fields: Box::new([]) }),
            verdicts: Box::new([]),
            change: Some(jig_protocol_charter::ChangeRule {
                checks_must_pass: true,
                fields: Box::new([
                    jig_protocol_charter::FieldRule { name: Box::from(&b"title"[..]), max: words },
                    jig_protocol_charter::FieldRule { name: Box::from(&b"body"[..]), max: words },
                ]),
            }),
        },
    })
}
const fn delivery(value: core::DeliveryOutcome) -> jig_domain_host::DeliveryOutcome {
    match value {
        core::DeliveryOutcome::Delivered => jig_domain_host::DeliveryOutcome::Delivered,
        core::DeliveryOutcome::Nothing => jig_domain_host::DeliveryOutcome::Nothing,
        core::DeliveryOutcome::Stale => jig_domain_host::DeliveryOutcome::Stale,
        core::DeliveryOutcome::Refused | core::DeliveryOutcome::Invalid => jig_domain_host::DeliveryOutcome::Refused,
        core::DeliveryOutcome::Failed => jig_domain_host::DeliveryOutcome::Failed,
    }
}
/// Preserve the task-owned message kind when translating its opaque words.
fn sender(value: tasks::Party) -> Box<[u8]> {
    match value {
        tasks::Party::Task(number) | tasks::Party::Person(number) => {
            Box::from(skein_lib::Decimal::of(number).as_bytes())
        }
        tasks::Party::Deployment { .. } => Box::from(&b"deployment"[..]),
    }
}
const fn title(kind: jig_domain_brief::Core) -> &'static [u8] {
    match kind {
        jig_domain_brief::Core::Task => b"Task",
        jig_domain_brief::Core::Lineage => b"Requesters",
        jig_domain_brief::Core::Results => b"Results",
        jig_domain_brief::Core::Plan => b"Plan",
        jig_domain_brief::Core::Attempts => b"Attempts",
        jig_domain_brief::Core::Calls => b"Calls",
        jig_domain_brief::Core::Waiting => b"Waiting",
        jig_domain_brief::Core::NotesIndex => b"Notes",
        jig_domain_brief::Core::TranscriptTail => b"Conversation",
    }
}

pub(super) fn worst_case(limits: &core::Limits) -> Option<u64> {
    Map::<core::CallKey, (Box<[u8]>, Box<[u8]>)>::worst_case(limits.call_records)?
        .checked_add(u64::from(limits.call_records).checked_mul(u64::from(limits.call_answer_bytes))?)
}

fn family(name: &[u8]) -> u64 {
    let jig = core::ToolFamilies::JIG;
    match name {
        b"delegate" => jig.delegate.0,
        b"message" | b"introduce" => jig.message.0,
        b"cancel" | b"release" | b"amend" => jig.control.0,
        b"decide" | b"withdraw" => jig.decide.0,
        b"propose" => jig.propose.0,
        b"subscribe" | b"unsubscribe" => jig.subscribe.0,
        b"note" => jig.note.0,
        b"recall" => jig.recall.0,
        _ => 0,
    }
}

fn host_form(form: jig_domain_accounts::Form) -> jig_domain_host::Form {
    match form {
        jig_domain_accounts::Form::Credential(scheme) => jig_domain_host::Form::Credential(match scheme {
            jig_domain_accounts::Scheme::Bearer => jig_domain_host::Scheme::Bearer,
            jig_domain_accounts::Scheme::Key => jig_domain_host::Scheme::Key,
        }),
        jig_domain_accounts::Form::Application => jig_domain_host::Form::Application,
        jig_domain_accounts::Form::Program(name) => jig_domain_host::Form::Program(
            jig_domain_host::SourceName::new(name.as_bytes()).expect("source name follows the same rules"),
        ),
    }
}

fn mode(value: jig_domain_tasks::EndedBy) -> jig_protocol_charter::Mode {
    match value {
        jig_domain_tasks::EndedBy::Result => jig_protocol_charter::Mode::Finite,
        jig_domain_tasks::EndedBy::Requester => jig_protocol_charter::Mode::Open,
    }
}
