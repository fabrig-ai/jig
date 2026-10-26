use crate::{Decoded, Limits, Problem, Why, json, nested, notes};
use alloc::boxed::Box;
use jig_domain_core as core;
use jig_domain_tasks as tasks;
use skein_lib::Wall;

#[must_use]
pub fn decode(tool: &[u8], input: &[u8], limits: &Limits) -> Decoded {
    let mut known = false;
    for item in crate::declarations() {
        if item.name == tool {
            known = true;
        }
    }
    if !known {
        return Decoded::NotMine;
    }
    match read(tool, input, limits) {
        Ok(decoded) => decoded,
        Err(problem) => Decoded::Problem(problem),
    }
}

fn read(tool: &[u8], input: &[u8], limits: &Limits) -> Result<Decoded, Problem> {
    let value = json::parse(input, limits)?;
    let action = match tool {
        b"settings" => core::NamedAction::Settings(settings_read(&value)?),
        b"propose_settings" => return settings(&value, limits),
        b"delegate" => return Ok(Decoded::Delegate(delegates(value.required(b"batch")?, limits)?)),
        b"message" => message(&value)?,
        b"cancel" => core::NamedAction::Control {
            target: value.required(b"target")?.number()?,
            control: tasks::Control::Cancel {
                reason: value.required(b"reason")?.text()?,
                grace: match value.field(b"grace")? {
                    Some(value) => Some(skein_lib::Duration::from_nanos(value.number()?)),
                    None => None,
                },
                message: None,
            },
        },
        b"release" => core::NamedAction::Release {
            target: value.required(b"target")?.number()?,
            revision: value.required(b"revision")?.number()?,
            again: match value.field(b"again")? {
                Some(value) => Some(value.boolean()?),
                None => None,
            },
        },
        b"introduce" => core::NamedAction::Introduce {
            left: value.required(b"left")?.number()?,
            right: value.required(b"right")?.number()?,
        },
        b"withdraw" => core::NamedAction::WithdrawProposal { proposal: value.required(b"proposal")?.number()? },
        b"unsubscribe" => core::NamedAction::Unsubscribe { subscription: value.required(b"subscription")?.number()? },
        b"amend" => amend(&value)?,
        b"decide" => match value.required(b"kind")?.text()?.as_ref() {
            b"proposal" => {
                let choice = match value.required(b"decision")?.text()?.as_ref() {
                    b"accept" => core::ProposalChoice::Accept,
                    b"reject" => core::ProposalChoice::Reject { reason: value.required(b"reason")?.text()? },
                    b"pass" => core::ProposalChoice::Pass,
                    _ => return Err(Problem { field: b"decision", why: Why::Range }),
                };
                core::NamedAction::DecideProposal {
                    proposer: value.required(b"proposer")?.number()?,
                    proposal: value.required(b"proposal")?.number()?,
                    choice,
                }
            }
            b"escalation" => {
                let choice = match value.required(b"decision")?.text()?.as_ref() {
                    b"release" => core::EscalationChoice::Release,
                    b"reject" => core::EscalationChoice::Reject { reason: value.required(b"reason")?.text()? },
                    b"pass" => core::EscalationChoice::Pass,
                    _ => return Err(Problem { field: b"decision", why: Why::Range }),
                };
                core::NamedAction::DecideEscalation {
                    task: value.required(b"task")?.number()?,
                    revision: value.required(b"revision")?.number()?,
                    choice,
                }
            }
            _ => return Err(Problem { field: b"kind", why: Why::Range }),
        },
        b"subscribe" => {
            let kind = match value.required(b"kind")?.text()?.as_ref() {
                b"task" => tasks::SubscriptionKind::Task {
                    target: value.required(b"target")?.number()?,
                    held: match value.field(b"held")? {
                        Some(value) => value.boolean()?,
                        None => false,
                    },
                    result: true,
                },
                b"timer" => tasks::SubscriptionKind::Timer {
                    at: Wall::from_nanos(value.required(b"at")?.number()?),
                    period: match value.field(b"period")? {
                        Some(value) => Some(skein_lib::Duration::from_nanos(value.number()?)),
                        None => None,
                    },
                },
                _ => return Err(Problem { field: b"kind", why: Why::Range }),
            };
            core::NamedAction::Subscribe { kind }
        }
        b"propose" => propose(&value, limits)?,
        b"note" => {
            let (entry, recalled) = notes::new(&value)?;
            core::NamedAction::Note { entry, recalled }
        }
        b"recall" => {
            let (by, page) = notes::recall(&value)?;
            core::NamedAction::Recall { by, page }
        }
        _ => return Err(Problem { field: b"", why: Why::Malformed }),
    };
    Ok(Decoded::Action(action))
}

pub fn batch(input: &[u8], limits: &Limits) -> Result<Box<[core::Delegate]>, Problem> {
    let value = json::parse(input, limits)?;
    delegates(value.required(b"batch")?, limits)
}

fn delegates(value: &json::Value, limits: &Limits) -> Result<Box<[core::Delegate]>, Problem> {
    if value.array()?.len() > usize::try_from(limits.batch).expect("u32 fits usize") {
        return Err(Problem { field: b"batch", why: Why::TooLarge });
    }
    let batch = nested::delegates(value)?;
    let mut allowed = false;
    for member in &batch {
        if let jig_domain_people::Budget::AsAllowed { .. } = member.budget {
            if allowed {
                return Err(Problem { field: b"budget", why: Why::Malformed });
            }
            allowed = true;
        }
    }
    Ok(batch)
}

fn message(value: &json::Value) -> Result<core::NamedAction, Problem> {
    let kind = match value.required(b"form")?.text()?.as_ref() {
        b"words" => tasks::MessageKind::Words,
        b"question" => tasks::MessageKind::Question,
        b"answer" => tasks::MessageKind::Answer { question: value.required(b"question")?.number()? },
        _ => return Err(Problem { field: b"form", why: Why::Range }),
    };
    Ok(core::NamedAction::Message {
        target: value.required(b"target")?.number()?,
        kind,
        words: value.required(b"words")?.text()?,
    })
}

fn propose(value: &json::Value, limits: &Limits) -> Result<core::NamedAction, Problem> {
    let reason = value.required(b"reason")?.text()?;
    let kind = value.required(b"action")?.text()?;
    Ok(match kind.as_ref() {
        b"amend" => core::NamedAction::Propose {
            action: tasks::ProposalAction::Amend {
                task: value.required(b"target")?.number()?,
                revision: value.required(b"revision")?.number()?,
                amendment: nested::amendment(value.required(b"amendment")?)?,
            },
            reason,
            as_holder: false,
        },
        b"widen" => core::NamedAction::Propose {
            action: tasks::ProposalAction::Widen {
                task: 0,
                authority: nested::authority(value.required(b"authority")?)?,
            },
            reason,
            as_holder: false,
        },
        b"batch" => core::NamedAction::ProposeBatch {
            batch: delegates(value.required(b"batch")?, limits)?,
            reason,
            as_holder: false,
        },
        _ => return Err(Problem { field: b"action", why: Why::Range }),
    })
}

fn amend(value: &json::Value) -> Result<core::NamedAction, Problem> {
    let amendment = nested::amendment(value.required(b"amendment")?)?;
    let priority = match amendment.tracked {
        Some(tasks::Tracking::Tracked { .. }) => true,
        Some(tasks::Tracking::Untracked) | None => false,
    };
    let revision = match value.field(b"revision")? {
        Some(value) => value.number()?,
        None if priority
            && amendment.spec.is_none()
            && amendment.wake.is_none()
            && amendment.dependencies.is_none()
            && amendment.authority.is_none()
            && amendment.instructions.is_none()
            && amendment.llm.is_none() =>
        {
            0
        }
        None => return Err(Problem { field: b"revision", why: Why::Missing }),
    };
    Ok(core::NamedAction::Amend { target: value.required(b"target")?.number()?, revision, amendment })
}

fn settings(value: &json::Value, limits: &Limits) -> Result<Decoded, Problem> {
    let set = value.required(b"set")?;
    let set = match &set.kind {
        json::Kind::String(value) => value,
        json::Kind::Object(_)
        | json::Kind::Array(_)
        | json::Kind::Number(_)
        | json::Kind::Boolean(_)
        | json::Kind::Null => {
            return Err(Problem { field: b"set", why: Why::Type });
        }
    };
    let Ok(count) = u32::try_from(set.len() / 2) else {
        return Err(Problem { field: b"set", why: Why::TooLarge });
    };
    if set.len() % 2 != 0 || count > limits.input_bytes {
        return Err(Problem { field: b"set", why: Why::Malformed });
    }
    let mut bytes = skein_lib::List::with_capacity(count);
    for pair in set.chunks_exact(2) {
        let high = hex(*pair.first().expect("two hex digits"))?;
        let low = hex(*pair.get(1).expect("two hex digits"))?;
        let byte = high.checked_mul(16).expect("high hex nibble").checked_add(low).expect("low hex nibble");
        bytes.push(byte).expect("bounded settings bytes");
    }
    Ok(Decoded::Settings {
        against: value.required(b"against")?.small()?,
        set: bytes.into_boxed(),
        reason: value.required(b"reason")?.text()?,
    })
}
fn hex(byte: u8) -> Result<u8, Problem> {
    match byte {
        b'0'..=b'9' => Ok(byte.checked_sub(b'0').expect("decimal digit")),
        b'a'..=b'f' => Ok(byte.checked_sub(b'a').expect("hex letter").checked_add(10).expect("hex digit")),
        b'A'..=b'F' => Ok(byte.checked_sub(b'A').expect("hex letter").checked_add(10).expect("hex digit")),
        _ => Err(Problem { field: b"set", why: Why::Range }),
    }
}

fn settings_read(value: &json::Value) -> Result<core::SettingsRead, Problem> {
    Ok(match value.required(b"kind")?.text()?.as_ref() {
        b"inventory" => core::SettingsRead::Inventory,
        b"setting" => {
            let setting = match value.required(b"setting")?.text()?.as_ref() {
                b"charter" => core::settings::Setting::Charter(value.required(b"number")?.small()?),
                b"template" => core::settings::Setting::Template(value.required(b"number")?.small()?),
                b"policy" => core::settings::Setting::Policy(value.required(b"number")?.small()?),
                b"rules" => core::settings::Setting::Rules,
                _ => return Err(Problem { field: b"setting", why: Why::Range }),
            };
            core::SettingsRead::Setting { setting, revision: value.required(b"revision")?.small()? }
        }
        b"revision" => core::SettingsRead::Revision { number: value.required(b"number")?.small()? },
        b"revisions" => core::SettingsRead::Revisions {
            before: match value.field(b"before")? {
                Some(value) => Some(value.small()?),
                None => None,
            },
        },
        _ => return Err(Problem { field: b"kind", why: Why::Range }),
    })
}
