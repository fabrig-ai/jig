//! Call decoding and answer rendering after the fleet authenticates a run.
use super::tools;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_ops_domain as root;
use jig_protocol_tools as protocol;
use skein_lib::{ReplyTo, Token, Wall};
use std::collections::BTreeMap;

type Envelope = (Box<[u8]>, Box<[u8]>);

pub struct Stand {
    limits: protocol::Limits,
    calls: BTreeMap<core::CallKey, Envelope>,
}

impl Stand {
    #[must_use]
    pub fn new(limits: core::Limits) -> Self {
        Self {
            limits: protocol::Limits {
                input_bytes: 65536,
                words: limits.tasks.message_bytes,
                batch: limits.tasks.batch,
                depth: 16,
            },
            calls: BTreeMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.calls.clear();
    }

    pub fn decode(&mut self, to: ReplyTo, run: Token, attempt: Token, call: &fleet::Call, wall: Wall) -> root::Event {
        let Some(name) = jig_protocol_charter::call_key(&call.name, attempt.raw()) else {
            return problem(to, call, protocol::Problem { field: b"name", why: protocol::Why::Malformed });
        };
        let key = core::CallKey {
            task: run.raw(),
            attempt: attempt.raw(),
            completion: name.completion,
            position: name.position,
        };
        let event = match protocol::decode(&call.tool, &call.input, &self.limits) {
            protocol::Decoded::Action(core::NamedAction::Settings(read)) if !call.writes => {
                root::Event::Core(core::Event::NamedAction { to, key, action: core::NamedAction::Settings(read) })
            }
            protocol::Decoded::Action(action) if call.writes => {
                root::Event::Core(core::Event::NamedAction { to, key, action })
            }
            protocol::Decoded::Delegate(batch) if call.writes => {
                root::Event::Core(core::Event::DelegateValidated { to, key, batch, stubs: Box::new([]) })
            }
            protocol::Decoded::Settings { .. } => {
                return problem(to, call, protocol::Problem { field: b"set", why: protocol::Why::Range });
            }
            protocol::Decoded::Problem(why) => return problem(to, call, why),
            protocol::Decoded::NotMine => match tools::decode(&call.tool, &call.input) {
                Ok(tools::Decoded::Read(read)) if !call.writes => {
                    return root::Event::ConnectorRead {
                        to,
                        task: run.raw(),
                        name: call.name.clone(),
                        tool: call.tool.clone(),
                        read,
                    };
                }
                Ok(tools::Decoded::Effect { effect, reason }) if call.writes => root::Event::Effect {
                    to,
                    key,
                    effect,
                    deadline: Wall::from_nanos(wall.as_nanos().saturating_add(call.deadline.as_nanos())),
                    proposal: reason,
                },
                Err(why) => return problem(to, call, why),
                Ok(tools::Decoded::Read(_) | tools::Decoded::Effect { .. }) => {
                    return problem(to, call, protocol::Problem { field: b"tool", why: protocol::Why::Type });
                }
            },
            protocol::Decoded::Action(_) | protocol::Decoded::Delegate(_) => {
                return problem(to, call, protocol::Problem { field: b"tool", why: protocol::Why::Type });
            }
        };
        self.calls.insert(key, (call.name.clone(), call.tool.clone()));
        event
    }

    pub fn settings_answer(
        &mut self,
        to: ReplyTo,
        key: core::CallKey,
        answer: &core::SettingsAnswer,
    ) -> Option<root::Event> {
        let (_, tool) = self.calls.remove(&key)?;
        let rendered = protocol::settings_answer(answer);
        Some(root::Event::CallAnswered {
            to,
            call: core::SettledCall {
                serial: 0,
                tool,
                answer: core::SettledAnswer::Host { error: rendered.error, body: rendered.text },
            },
        })
    }

    pub fn answer(&mut self, to: ReplyTo, key: core::CallKey, part: &core::CallPart, durable: bool) -> root::Event {
        let (name, tool) = self.calls.remove(&key).expect("protocol call envelope");
        let rendered = protocol::render(part);
        let call = core::SettledCall {
            serial: 0,
            tool,
            answer: core::SettledAnswer::Host { error: rendered.error, body: rendered.text },
        };
        if durable {
            root::Event::Core(core::Event::SettledCall { to, key, name, call })
        } else {
            root::Event::CallAnswered { to, call }
        }
    }

    pub fn busy(&mut self, to: ReplyTo, key: core::CallKey) -> root::Event {
        let (_, tool) = self.calls.remove(&key).expect("protocol note envelope");
        answered(to, tool, true, b"busy".as_slice().into())
    }

    pub fn refused(&mut self, to: ReplyTo, key: core::CallKey, tool: Box<[u8]>) -> root::Event {
        self.calls.remove(&key);
        answered(to, tool, true, b"unavailable".as_slice().into())
    }
}

fn problem(to: ReplyTo, call: &fleet::Call, why: protocol::Problem) -> root::Event {
    let rendered = protocol::problem(&call.tool, &why);
    answered(to, call.tool.clone(), rendered.error, rendered.text)
}
fn answered(to: ReplyTo, tool: Box<[u8]>, error: bool, body: Box<[u8]>) -> root::Event {
    root::Event::CallAnswered {
        to,
        call: core::SettledCall { serial: 0, tool, answer: core::SettledAnswer::Host { error, body } },
    }
}
