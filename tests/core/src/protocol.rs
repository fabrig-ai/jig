//! Protocol routing stand-in: bytes enter here after the fleet's fence.
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_protocol_tools as tools;
use jig_test_domain as root;
use skein_lib::{Duration, ReplyTo, Token};
use std::collections::BTreeMap;

type Envelope = (Box<[u8]>, Box<[u8]>);

#[derive(Clone, Debug)]
pub struct Stand {
    limits: tools::Limits,
    calls: BTreeMap<core::CallKey, Envelope>,
    settings_limits: jig_protocol_store::settings_v1::Limits,
    models: Box<[core::settings::ModelName]>,
}

impl Stand {
    #[must_use]
    pub fn new(limits: core::Limits) -> Self {
        Self::with_models(limits, &crate::world::config(0).core.settings.models)
    }
    #[must_use]
    pub fn with_models(limits: core::Limits, models: &[core::Model]) -> Self {
        Self {
            settings_limits: jig_protocol_store::codec::settings::limits(&limits).expect("settings codec limits"),
            models: core::settings::model_names(models),
            limits: tools::Limits {
                input_bytes: 8192,
                words: limits.tasks.message_bytes,
                batch: limits.tasks.batch,
                depth: 16,
            },
            calls: BTreeMap::new(),
        }
    }

    pub fn decode(&mut self, to: Token, run: Token, attempt: Token, call: &fleet::Call) -> Option<root::Event> {
        let to = ReplyTo::new(to);
        let decoded = tools::decode(&call.tool, &call.input, &self.limits);
        if let tools::Decoded::NotMine = decoded {
            return None;
        }
        let Some(name) = jig_protocol_charter::call_key(&call.name, attempt.raw()) else {
            return Some(unreadable(to, call, &tools::Problem { field: b"name", why: tools::Why::Malformed }));
        };
        let key = core::CallKey {
            task: run.raw(),
            attempt: attempt.raw(),
            completion: name.completion,
            position: name.position,
        };
        let event = match decoded {
            tools::Decoded::Settings { against, set, reason } => {
                let settings_key = core::settings::Key::Proposal {
                    proposer: jig_domain_people::Proposer::Task(key.task),
                    proposal: 1,
                };
                let decoded = jig_protocol_store::codec::settings::decode(
                    &settings_key,
                    &set,
                    &self.settings_limits,
                    &self.models,
                );
                let Ok(core::settings::Record::Proposal { value, .. }) = decoded else {
                    return Some(unreadable(to, call, &tools::Problem { field: b"set", why: tools::Why::Malformed }));
                };
                if value.against != against {
                    return Some(unreadable(
                        to,
                        call,
                        &tools::Problem { field: b"against", why: tools::Why::Malformed },
                    ));
                }
                core::Event::NamedAction {
                    to,
                    key,
                    action: core::NamedAction::ProposeSettings { against, items: value.items, reason },
                }
            }
            tools::Decoded::Action(action) => core::Event::NamedAction { to, key, action },
            tools::Decoded::Delegate(batch) => core::Event::DelegateValidated { to, key, batch, stubs: Box::new([]) },
            tools::Decoded::Problem(problem) => return Some(unreadable(to, call, &problem)),
            tools::Decoded::NotMine => return None,
        };
        self.calls.insert(key, (call.name.clone(), call.tool.clone()));
        Some(root::Event::Core(event))
    }

    pub fn settings_answer(
        &mut self,
        to: Token,
        key: core::CallKey,
        answer: &core::SettingsAnswer,
    ) -> Option<root::Event> {
        let (_, tool) = self.calls.remove(&key)?;
        let rendered = tools::settings_answer(answer);
        Some(root::Event::CallAnswered {
            to: ReplyTo::new(to),
            call: core::SettledCall {
                serial: 0,
                tool,
                answer: core::SettledAnswer::Host { error: rendered.error, body: rendered.text },
            },
        })
    }

    pub fn answer(
        &mut self,
        to: Token,
        key: core::CallKey,
        part: &core::CallPart,
        durable: bool,
    ) -> Option<root::Event> {
        let to = ReplyTo::new(to);
        let (name, tool) = self.calls.remove(&key)?;
        let rendered = tools::render(part);
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
}

fn unreadable(to: ReplyTo, call: &fleet::Call, problem: &tools::Problem) -> root::Event {
    let rendered = tools::problem(&call.tool, problem);
    root::Event::CallAnswered {
        to,
        call: core::SettledCall {
            serial: 0,
            tool: call.tool.clone(),
            answer: core::SettledAnswer::Host { error: rendered.error, body: rendered.text },
        },
    }
}

impl crate::effects::World {
    /// Relay a smith-named call through the fleet and the protocol stand-in.
    pub fn tool_call(&mut self, tool: &[u8], input: &[u8], writes: bool) -> Token {
        let completion = u32::try_from(self.host_answers.len() + 1)
            .expect("bounded story calls")
            .max(self.answers.iter().map(|(_, key, _)| key.completion + 1).max().unwrap_or(1));
        self.tool_call_at(completion, tool, input, writes)
    }

    /// Ask again under a chosen name, as a host retry does.
    pub fn tool_call_at(&mut self, completion: u32, tool: &[u8], input: &[u8], writes: bool) -> Token {
        let assignment = self.assigned.as_ref().expect("assigned run");
        let name = smith_channel::v5::CallName::new(
            &smith_channel::v5::CEILINGS,
            smith_channel::v5::CallNameParts { activation: assignment.attempt, completion, position: 0 },
        )
        .expect("bounded call name");
        let mut encoded = skein_lib::Writer::new(name.measure() as usize);
        name.encode(&mut encoded).expect("measured call name");
        self.relay_call(&encoded.finish(), tool, input, writes);
        Token::new(u64::from(completion))
    }

    pub(crate) fn relay_call(&mut self, name: &[u8], tool: &[u8], input: &[u8], writes: bool) {
        let assignment = self.assigned.as_ref().expect("assigned chat");
        self.send(root::Event::Core(core::Event::Fleet(fleet::Event::Relay {
            channel: Token::new(7),
            run: Token::new(assignment.task),
            attempt: Token::new(assignment.attempt),
            call: fleet::Call {
                name: name.into(),
                tool: tool.into(),
                writes,
                input: input.into(),
                deadline: Duration::from_secs(2),
            },
        })));
    }
}
