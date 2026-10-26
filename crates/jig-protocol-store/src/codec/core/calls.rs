//! Named calls and settled answers (protocol/store.md, 5.1).
use super::{
    Box, Limits, List, Wall, authority, decode_answer, decode_delivery_outcome, decode_escalation_outcome,
    decode_finding, decode_outbox_outcome, decode_proposal_outcome, domain, encode_answer, encode_delivery_outcome,
    encode_escalation_outcome, encode_finding, encode_outbox_outcome, encode_proposal_outcome, note_problem, notes,
    problem, task_problem, tasks, too_large, wire,
};
mod moved;
pub(super) fn encode_settled_answer(
    value: &domain::SettledAnswer,
    limits: &Limits,
) -> Result<wire::SettledAnswer, wire::Problem> {
    Ok(match value {
        domain::SettledAnswer::Host { error, body } => wire::SettledAnswer::Host(wire::SettledAnswerHost::new(
            &limits.core,
            wire::SettledAnswerHostParts { error: *error, body: body.clone() },
        )?),
        domain::SettledAnswer::Delivery { outcome, evidence } => {
            wire::SettledAnswer::Delivery(wire::SettledAnswerDelivery::new(
                &limits.core,
                wire::SettledAnswerDeliveryParts {
                    outcome: encode_delivery_outcome(outcome, limits),
                    evidence: evidence.clone(),
                },
            )?)
        }
    })
}
pub(super) fn decode_settled_answer(value: wire::SettledAnswer, limits: &Limits) -> domain::SettledAnswer {
    match value {
        wire::SettledAnswer::Host(value) => {
            let wire::SettledAnswerHostParts { error, body } = value.into_parts();
            domain::SettledAnswer::Host { error, body }
        }
        wire::SettledAnswer::Delivery(value) => {
            let wire::SettledAnswerDeliveryParts { outcome, evidence } = value.into_parts();
            domain::SettledAnswer::Delivery { outcome: decode_delivery_outcome(outcome, limits), evidence }
        }
    }
}

pub(super) fn encode_settled_call(
    value: &domain::SettledCall,
    limits: &Limits,
) -> Result<wire::SettledCall, wire::Problem> {
    let domain::SettledCall { serial, tool, answer } = value;
    wire::SettledCall::new(
        &limits.core,
        wire::SettledCallParts { serial: *serial, tool: tool.clone(), answer: encode_settled_answer(answer, limits)? },
    )
}
pub(super) fn decode_settled_call(value: wire::SettledCall, limits: &Limits) -> domain::SettledCall {
    let wire::SettledCallParts { serial, tool, answer } = value.into_parts();
    domain::SettledCall { serial, tool, answer: decode_settled_answer(answer, limits) }
}

pub(super) fn encode_call_key(value: &domain::CallKey, limits: &Limits) -> Result<wire::CallKey, wire::Problem> {
    let domain::CallKey { task, attempt, completion, position } = value;
    wire::CallKey::new(
        &limits.core,
        wire::CallKeyParts { task: *task, attempt: *attempt, completion: *completion, position: *position },
    )
}
pub(super) fn decode_call_key(value: wire::CallKey, _limits: &Limits) -> domain::CallKey {
    let wire::CallKeyParts { task, attempt, completion, position } = value.into_parts();
    domain::CallKey { task, attempt, completion, position }
}

pub(super) fn encode_call_part(value: &domain::CallPart, limits: &Limits) -> Result<wire::CallPart, wire::Problem> {
    Ok(match value {
        domain::CallPart::SettingsRefused(value) => encode_settings_refusal(value, limits)?,
        domain::CallPart::Effect { connector, entry, deadline, outcome } => {
            encode_part_effect(*connector, *entry, *deadline, *outcome, limits)?
        }
        domain::CallPart::Connector { connector } => wire::CallPart::Connector(wire::CallPartConnector::new(
            &limits.core,
            wire::CallPartConnectorParts { connector: *connector },
        )?),
        domain::CallPart::EffectDenied { answer, findings } => encode_part_effect_denied(*answer, findings, limits)?,
        domain::CallPart::EffectRefused(value) => wire::CallPart::EffectRefused(wire::CallPartEffectRefused::new(
            &limits.core,
            wire::CallPartEffectRefusedParts { value: encode_task_problem(value, limits)? },
        )?),
        domain::CallPart::ToolDenied { answer, findings } => encode_part_tool_denied(*answer, findings, limits)?,
        domain::CallPart::EscalationDecided { task, revision, outcome } => {
            encode_part_escalation_decided(*task, *revision, *outcome, limits)?
        }
        domain::CallPart::EscalationRefused(value) => {
            wire::CallPart::EscalationRefused(wire::CallPartEscalationRefused::new(
                &limits.core,
                wire::CallPartEscalationRefusedParts { value: encode_task_problem(value, limits)? },
            )?)
        }
        domain::CallPart::Proposed { proposal, holder } => wire::CallPart::Proposed(wire::CallPartProposed::new(
            &limits.core,
            wire::CallPartProposedParts { proposal: *proposal, holder: encode_holder(holder, limits)? },
        )?),
        domain::CallPart::ProposalDecided { proposal, outcome } => {
            encode_part_proposal_decided(*proposal, *outcome, limits)?
        }
        domain::CallPart::ProposalRefused(value) => {
            wire::CallPart::ProposalRefused(wire::CallPartProposalRefused::new(
                &limits.core,
                wire::CallPartProposalRefusedParts { value: encode_task_problem(value, limits)? },
            )?)
        }
        domain::CallPart::Again { task, required } => encode_again(*task, *required, limits)?,
        domain::CallPart::PassedUp { task } => encode_passed_up(*task, limits)?,
        domain::CallPart::Controlled => wire::CallPart::Controlled,
        domain::CallPart::Revised { task, revision } => wire::CallPart::Revised(wire::CallPartRevised::new(
            &limits.core,
            wire::CallPartRevisedParts { task: *task, revision: *revision },
        )?),
        domain::CallPart::Moved { task, now } => wire::CallPart::Moved(wire::CallPartMoved::new(
            &limits.core,
            wire::CallPartMovedParts { task: *task, now: moved::encode_moved(now, &limits.core)? },
        )?),
        domain::CallPart::ControlRefused(value) => wire::CallPart::ControlRefused(wire::CallPartControlRefused::new(
            &limits.core,
            wire::CallPartControlRefusedParts { value: encode_task_problem(value, limits)? },
        )?),
        domain::CallPart::ControlDenied { answer } => wire::CallPart::ControlDenied(wire::CallPartControlDenied::new(
            &limits.core,
            wire::CallPartControlDeniedParts { answer: encode_answer(answer, limits) },
        )?),
        domain::CallPart::Sent { message } => {
            wire::CallPart::Sent(wire::CallPartSent::new(&limits.core, wire::CallPartSentParts { message: *message })?)
        }
        domain::CallPart::Introduced => wire::CallPart::Introduced,
        domain::CallPart::MessageRefused(value) => wire::CallPart::MessageRefused(wire::CallPartMessageRefused::new(
            &limits.core,
            wire::CallPartMessageRefusedParts { value: encode_task_problem(value, limits)? },
        )?),
        domain::CallPart::Subscribed { subscription } => wire::CallPart::Subscribed(wire::CallPartSubscribed::new(
            &limits.core,
            wire::CallPartSubscribedParts { subscription: *subscription },
        )?),
        domain::CallPart::Unsubscribed => wire::CallPart::Unsubscribed,
        domain::CallPart::SubscriptionRefused(value) => {
            wire::CallPart::SubscriptionRefused(wire::CallPartSubscriptionRefused::new(
                &limits.core,
                wire::CallPartSubscriptionRefusedParts { value: encode_task_problem(value, limits)? },
            )?)
        }
        domain::CallPart::Delegated(value) => wire::CallPart::Delegated(wire::CallPartDelegated::new(
            &limits.core,
            wire::CallPartDelegatedParts { value: encode_call_part_delegated_value(value, limits)? },
        )?),
        domain::CallPart::DelegationDenied { answer, findings } => {
            encode_part_delegation_denied(*answer, findings, limits)?
        }
        domain::CallPart::DelegationRefused(value) => {
            wire::CallPart::DelegationRefused(wire::CallPartDelegationRefused::new(
                &limits.core,
                wire::CallPartDelegationRefusedParts { value: encode_task_problem(value, limits)? },
            )?)
        }
        domain::CallPart::NoteWritten { name, revision } => {
            wire::CallPart::NoteWritten(wire::CallPartNoteWritten::new(
                &limits.core,
                wire::CallPartNoteWrittenParts { name: *name, revision: *revision },
            )?)
        }
        domain::CallPart::NoteRecalled { entries, more } => encode_part_note_recalled(entries, *more, limits)?,
        domain::CallPart::NoteRefused(value) => encode_part_note_refused(*value, limits)?,
        domain::CallPart::Unavailable => wire::CallPart::Unavailable,
    })
}
#[expect(clippy::too_many_lines, reason = "one exhaustive translation preserves the stored call vocabulary")]
pub(super) fn decode_call_part(value: wire::CallPart, limits: &Limits) -> Result<domain::CallPart, wire::Problem> {
    Ok(match value {
        wire::CallPart::SettingsRefused(value) => decode_settings_refusal(value)?,
        wire::CallPart::Effect(value) => decode_part_effect(value, limits),
        wire::CallPart::Connector(value) => {
            let wire::CallPartConnectorParts { connector } = value.into_parts();
            domain::CallPart::Connector { connector }
        }
        wire::CallPart::EffectDenied(value) => {
            let wire::CallPartEffectDeniedParts { answer, findings } = value.into_parts();
            domain::CallPart::EffectDenied {
                answer: decode_answer(answer, limits),
                findings: decode_call_part_effect_denied_findings(findings, limits),
            }
        }
        wire::CallPart::EffectRefused(value) => {
            let wire::CallPartEffectRefusedParts { value } = value.into_parts();
            domain::CallPart::EffectRefused(decode_task_problem(value, limits)?)
        }
        wire::CallPart::ToolDenied(value) => {
            let wire::CallPartToolDeniedParts { answer, findings } = value.into_parts();
            domain::CallPart::ToolDenied {
                answer: decode_answer(answer, limits),
                findings: decode_call_part_tool_denied_findings(findings, limits),
            }
        }
        wire::CallPart::EscalationDecided(value) => {
            let wire::CallPartEscalationDecidedParts { task, revision, outcome } = value.into_parts();
            domain::CallPart::EscalationDecided { task, revision, outcome: decode_escalation_outcome(outcome, limits) }
        }
        wire::CallPart::EscalationRefused(value) => {
            let wire::CallPartEscalationRefusedParts { value } = value.into_parts();
            domain::CallPart::EscalationRefused(decode_task_problem(value, limits)?)
        }
        wire::CallPart::Proposed(value) => {
            let wire::CallPartProposedParts { proposal, holder } = value.into_parts();
            domain::CallPart::Proposed { proposal, holder: decode_holder(holder, limits) }
        }
        wire::CallPart::ProposalDecided(value) => {
            let wire::CallPartProposalDecidedParts { proposal, outcome } = value.into_parts();
            domain::CallPart::ProposalDecided { proposal, outcome: decode_proposal_outcome(outcome, limits) }
        }
        wire::CallPart::ProposalRefused(value) => {
            let wire::CallPartProposalRefusedParts { value } = value.into_parts();
            domain::CallPart::ProposalRefused(decode_task_problem(value, limits)?)
        }
        wire::CallPart::Again(value) => decode_again(value),
        wire::CallPart::PassedUp(value) => decode_passed_up(value),
        wire::CallPart::Controlled => domain::CallPart::Controlled,
        wire::CallPart::Revised(value) => decode_revised(value),
        wire::CallPart::Moved(value) => decode_moved(value),
        wire::CallPart::ControlRefused(value) => {
            let wire::CallPartControlRefusedParts { value } = value.into_parts();
            domain::CallPart::ControlRefused(decode_task_problem(value, limits)?)
        }
        wire::CallPart::ControlDenied(value) => {
            let wire::CallPartControlDeniedParts { answer } = value.into_parts();
            domain::CallPart::ControlDenied { answer: decode_answer(answer, limits) }
        }
        wire::CallPart::Sent(value) => {
            let wire::CallPartSentParts { message } = value.into_parts();
            domain::CallPart::Sent { message }
        }
        wire::CallPart::Introduced => domain::CallPart::Introduced,
        wire::CallPart::MessageRefused(value) => {
            let wire::CallPartMessageRefusedParts { value } = value.into_parts();
            domain::CallPart::MessageRefused(decode_task_problem(value, limits)?)
        }
        wire::CallPart::Subscribed(value) => {
            let wire::CallPartSubscribedParts { subscription } = value.into_parts();
            domain::CallPart::Subscribed { subscription }
        }
        wire::CallPart::Unsubscribed => domain::CallPart::Unsubscribed,
        wire::CallPart::SubscriptionRefused(value) => {
            let wire::CallPartSubscriptionRefusedParts { value } = value.into_parts();
            domain::CallPart::SubscriptionRefused(decode_task_problem(value, limits)?)
        }
        wire::CallPart::Delegated(value) => {
            let wire::CallPartDelegatedParts { value } = value.into_parts();
            domain::CallPart::Delegated(decode_call_part_delegated_value(value, limits))
        }
        wire::CallPart::DelegationDenied(value) => {
            let wire::CallPartDelegationDeniedParts { answer, findings } = value.into_parts();
            domain::CallPart::DelegationDenied {
                answer: decode_answer(answer, limits),
                findings: decode_call_part_delegation_denied_findings(findings, limits),
            }
        }
        wire::CallPart::DelegationRefused(value) => {
            let wire::CallPartDelegationRefusedParts { value } = value.into_parts();
            domain::CallPart::DelegationRefused(decode_task_problem(value, limits)?)
        }
        wire::CallPart::NoteWritten(value) => {
            let wire::CallPartNoteWrittenParts { name, revision } = value.into_parts();
            domain::CallPart::NoteWritten { name, revision }
        }
        wire::CallPart::NoteRecalled(value) => {
            let wire::CallPartNoteRecalledParts { entries, more } = value.into_parts();
            domain::CallPart::NoteRecalled { entries: decode_call_part_note_recalled_entries(entries, limits)?, more }
        }
        wire::CallPart::NoteRefused(value) => {
            let wire::CallPartNoteRefusedParts { value } = value.into_parts();
            domain::CallPart::NoteRefused(decode_note_refusal(value, limits))
        }
        wire::CallPart::Unavailable => domain::CallPart::Unavailable,
    })
}

pub(super) fn encode_call_record(
    value: &domain::CallRecord,
    limits: &Limits,
) -> Result<wire::CallRecord, wire::Problem> {
    let domain::CallRecord { key, part, settled } = value;
    wire::CallRecord::new(
        &limits.core,
        wire::CallRecordParts {
            key: encode_call_key(key, limits)?,
            part: encode_call_part(part, limits)?,
            settled: match settled {
                Some(value) => Some(encode_settled_call(value, limits)?),
                None => None,
            },
        },
    )
}
#[expect(clippy::manual_map, reason = "step code uses concrete matches (programming-model.md, 10)")]
pub(super) fn decode_call_record(
    value: wire::CallRecord,
    limits: &Limits,
) -> Result<domain::CallRecord, wire::Problem> {
    let wire::CallRecordParts { key, part, settled } = value.into_parts();
    Ok(domain::CallRecord {
        key: decode_call_key(key, limits),
        part: decode_call_part(part, limits)?,
        settled: match settled {
            Some(value) => Some(decode_settled_call(value, limits)),
            None => None,
        },
    })
}

fn encode_call_part_effect_denied_findings(
    values: &[authority::Finding],
    limits: &Limits,
) -> Result<List<wire::Finding>, wire::Problem> {
    let mut result = List::with_capacity(match u32::try_from(values.len()) {
        Ok(value) => value,
        Err(error) => return Err(too_large(error)),
    });
    for value in values {
        result.push(encode_finding(value, limits)?).expect("same number of members");
    }
    Ok(result)
}
fn decode_call_part_effect_denied_findings(values: List<wire::Finding>, limits: &Limits) -> Box<[authority::Finding]> {
    let mut result = List::with_capacity(values.len());
    for value in values.into_boxed() {
        result.push(decode_finding(value, limits)).expect("same number of members");
    }
    result.into_boxed()
}

fn encode_call_part_tool_denied_findings(
    values: &[authority::Finding],
    limits: &Limits,
) -> Result<List<wire::Finding>, wire::Problem> {
    let mut result = List::with_capacity(match u32::try_from(values.len()) {
        Ok(value) => value,
        Err(error) => return Err(too_large(error)),
    });
    for value in values {
        result.push(encode_finding(value, limits)?).expect("same number of members");
    }
    Ok(result)
}
fn decode_call_part_tool_denied_findings(values: List<wire::Finding>, limits: &Limits) -> Box<[authority::Finding]> {
    let mut result = List::with_capacity(values.len());
    for value in values.into_boxed() {
        result.push(decode_finding(value, limits)).expect("same number of members");
    }
    result.into_boxed()
}

fn encode_call_part_delegated_value(
    values: &[domain::Made],
    limits: &Limits,
) -> Result<List<wire::Made>, wire::Problem> {
    let mut result = List::with_capacity(match u32::try_from(values.len()) {
        Ok(value) => value,
        Err(error) => return Err(too_large(error)),
    });
    for value in values {
        result
            .push(wire::Made::new(
                &limits.core,
                wire::MadeParts { task: value.task, revision: value.revision, given: value.given },
            )?)
            .expect("same number of members");
    }
    Ok(result)
}
fn decode_call_part_delegated_value(values: List<wire::Made>, _limits: &Limits) -> Box<[domain::Made]> {
    let mut result = List::with_capacity(values.len());
    for value in values.into_boxed() {
        let wire::MadeParts { task, revision, given } = value.into_parts();
        result.push(domain::Made { task, revision, given }).expect("same number of members");
    }
    result.into_boxed()
}

fn encode_call_part_delegation_denied_findings(
    values: &[authority::Finding],
    limits: &Limits,
) -> Result<List<wire::Finding>, wire::Problem> {
    let mut result = List::with_capacity(match u32::try_from(values.len()) {
        Ok(value) => value,
        Err(error) => return Err(too_large(error)),
    });
    for value in values {
        result.push(encode_finding(value, limits)?).expect("same number of members");
    }
    Ok(result)
}
fn decode_call_part_delegation_denied_findings(
    values: List<wire::Finding>,
    limits: &Limits,
) -> Box<[authority::Finding]> {
    let mut result = List::with_capacity(values.len());
    for value in values.into_boxed() {
        result.push(decode_finding(value, limits)).expect("same number of members");
    }
    result.into_boxed()
}

fn encode_call_part_note_recalled_entries(
    values: &[notes::Entry],
    limits: &Limits,
) -> Result<List<wire::NoteBytes>, wire::Problem> {
    let mut result = List::with_capacity(match u32::try_from(values.len()) {
        Ok(value) => value,
        Err(error) => return Err(too_large(error)),
    });
    for value in values {
        result
            .push(wire::NoteBytes::new(
                &limits.core,
                wire::NoteBytesParts {
                    value: match crate::codec::notes::encode(&notes::Record::Entry(value.clone()), &limits.notes) {
                        Ok(value) => value,
                        Err(error) => return Err(note_problem(error)),
                    },
                },
            )?)
            .expect("same number of members");
    }
    Ok(result)
}
fn decode_call_part_note_recalled_entries(
    values: List<wire::NoteBytes>,
    limits: &Limits,
) -> Result<Box<[notes::Entry]>, wire::Problem> {
    let mut result = List::with_capacity(values.len());
    for value in values.into_boxed() {
        result
            .push(
                match match crate::codec::notes::decode(&value.into_parts().value, &limits.notes) {
                    Ok(value) => value,
                    Err(error) => return Err(note_problem(error)),
                } {
                    notes::Record::Entry(entry) => entry,
                    notes::Record::Line(_) => return Err(problem(skein_codec::Reason::Tag)),
                },
            )
            .expect("same number of members");
    }
    Ok(result.into_boxed())
}
pub(super) fn encode_note_refusal(value: notes::Refusal, limits: &Limits) -> Result<wire::NoteRefusal, wire::Problem> {
    Ok(match value {
        notes::Refusal::Busy => wire::NoteRefusal::Busy,
        notes::Refusal::Oversized => wire::NoteRefusal::Oversized,
        notes::Refusal::Full => wire::NoteRefusal::Full,
        notes::Refusal::Exists => wire::NoteRefusal::Exists,
        notes::Refusal::Missing => wire::NoteRefusal::Missing,
        notes::Refusal::Moved { revision } => wire::NoteRefusal::Moved(wire::NoteRefusalMoved::new(
            &limits.core,
            wire::NoteRefusalMovedParts { revision },
        )?),
        notes::Refusal::RevisionExhausted => wire::NoteRefusal::RevisionExhausted,
    })
}
pub(super) fn decode_note_refusal(value: wire::NoteRefusal, _limits: &Limits) -> notes::Refusal {
    match value {
        wire::NoteRefusal::Busy => notes::Refusal::Busy,
        wire::NoteRefusal::Oversized => notes::Refusal::Oversized,
        wire::NoteRefusal::Full => notes::Refusal::Full,
        wire::NoteRefusal::Exists => notes::Refusal::Exists,
        wire::NoteRefusal::Missing => notes::Refusal::Missing,
        wire::NoteRefusal::Moved(value) => {
            let wire::NoteRefusalMovedParts { revision } = value.into_parts();
            notes::Refusal::Moved { revision }
        }
        wire::NoteRefusal::RevisionExhausted => notes::Refusal::RevisionExhausted,
    }
}

fn encode_task_problem(value: &jig_domain_tasks::Problem, limits: &Limits) -> Result<wire::TaskBytes, wire::Problem> {
    let value = match crate::codec::tasks::carried::problem(value, &limits.tasks) {
        Ok(value) => value,
        Err(error) => return Err(task_problem(error)),
    };
    wire::TaskBytes::new(&limits.core, wire::TaskBytesParts { value })
}
fn decode_task_problem(value: wire::TaskBytes, limits: &Limits) -> Result<jig_domain_tasks::Problem, wire::Problem> {
    match crate::codec::tasks::carried::read_problem(&value.into_parts().value, &limits.tasks) {
        Ok(value) => Ok(value),
        Err(error) => Err(task_problem(error)),
    }
}

fn encode_part_effect(
    connector: u16,
    entry: u64,
    deadline: Wall,
    outcome: Option<domain::connector::OutboxOutcome>,
    limits: &Limits,
) -> Result<wire::CallPart, wire::Problem> {
    Ok(wire::CallPart::Effect(wire::CallPartEffect::new(
        &limits.core,
        wire::CallPartEffectParts {
            connector,
            entry,
            deadline: deadline.as_nanos(),
            outcome: match outcome {
                Some(value) => Some(encode_outbox_outcome(&value, limits)?),
                None => None,
            },
        },
    )?))
}

fn encode_part_effect_denied(
    answer: authority::Answer,
    findings: &[authority::Finding],
    limits: &Limits,
) -> Result<wire::CallPart, wire::Problem> {
    Ok(wire::CallPart::EffectDenied(wire::CallPartEffectDenied::new(
        &limits.core,
        wire::CallPartEffectDeniedParts {
            answer: encode_answer(&answer, limits),
            findings: encode_call_part_effect_denied_findings(findings, limits)?,
        },
    )?))
}

fn encode_part_tool_denied(
    answer: authority::Answer,
    findings: &[authority::Finding],
    limits: &Limits,
) -> Result<wire::CallPart, wire::Problem> {
    Ok(wire::CallPart::ToolDenied(wire::CallPartToolDenied::new(
        &limits.core,
        wire::CallPartToolDeniedParts {
            answer: encode_answer(&answer, limits),
            findings: encode_call_part_tool_denied_findings(findings, limits)?,
        },
    )?))
}

fn encode_part_delegation_denied(
    answer: authority::Answer,
    findings: &[authority::Finding],
    limits: &Limits,
) -> Result<wire::CallPart, wire::Problem> {
    Ok(wire::CallPart::DelegationDenied(wire::CallPartDelegationDenied::new(
        &limits.core,
        wire::CallPartDelegationDeniedParts {
            answer: encode_answer(&answer, limits),
            findings: encode_call_part_delegation_denied_findings(findings, limits)?,
        },
    )?))
}

fn encode_part_note_recalled(
    entries: &[notes::Entry],
    more: bool,
    limits: &Limits,
) -> Result<wire::CallPart, wire::Problem> {
    Ok(wire::CallPart::NoteRecalled(wire::CallPartNoteRecalled::new(
        &limits.core,
        wire::CallPartNoteRecalledParts { entries: encode_call_part_note_recalled_entries(entries, limits)?, more },
    )?))
}

fn encode_part_escalation_decided(
    task: u64,
    revision: u64,
    outcome: tasks::EscalationOutcome,
    limits: &Limits,
) -> Result<wire::CallPart, wire::Problem> {
    Ok(wire::CallPart::EscalationDecided(wire::CallPartEscalationDecided::new(
        &limits.core,
        wire::CallPartEscalationDecidedParts { task, revision, outcome: encode_escalation_outcome(&outcome, limits)? },
    )?))
}

fn encode_part_proposal_decided(
    proposal: u64,
    outcome: tasks::ProposalOutcome,
    limits: &Limits,
) -> Result<wire::CallPart, wire::Problem> {
    Ok(wire::CallPart::ProposalDecided(wire::CallPartProposalDecided::new(
        &limits.core,
        wire::CallPartProposalDecidedParts { proposal, outcome: encode_proposal_outcome(&outcome, limits) },
    )?))
}

#[expect(clippy::manual_map, reason = "step code uses concrete matches (programming-model.md, 10)")]
fn decode_part_effect(value: wire::CallPartEffect, limits: &Limits) -> domain::CallPart {
    let wire::CallPartEffectParts { connector, entry, deadline, outcome } = value.into_parts();
    domain::CallPart::Effect {
        connector,
        entry,
        deadline: Wall::from_nanos(deadline),
        outcome: match outcome {
            Some(value) => Some(decode_outbox_outcome(value, limits)),
            None => None,
        },
    }
}

fn decode_revised(value: wire::CallPartRevised) -> domain::CallPart {
    let wire::CallPartRevisedParts { task, revision } = value.into_parts();
    domain::CallPart::Revised { task, revision }
}
fn decode_moved(value: wire::CallPartMoved) -> domain::CallPart {
    let wire::CallPartMovedParts { task, now } = value.into_parts();
    domain::CallPart::Moved { task, now: moved::decode_moved(now) }
}

fn encode_again(task: u64, required: bool, limits: &Limits) -> Result<wire::CallPart, wire::Problem> {
    Ok(wire::CallPart::Again(wire::CallPartAgain::new(&limits.core, wire::CallPartAgainParts { task, required })?))
}
fn encode_passed_up(task: u64, limits: &Limits) -> Result<wire::CallPart, wire::Problem> {
    Ok(wire::CallPart::PassedUp(wire::CallPartPassedUp::new(&limits.core, wire::CallPartPassedUpParts { task })?))
}
fn decode_again(value: wire::CallPartAgain) -> domain::CallPart {
    let wire::CallPartAgainParts { task, required } = value.into_parts();
    domain::CallPart::Again { task, required }
}
fn decode_passed_up(value: wire::CallPartPassedUp) -> domain::CallPart {
    let wire::CallPartPassedUpParts { task } = value.into_parts();
    domain::CallPart::PassedUp { task }
}

fn encode_holder(value: &tasks::ProposalHolder, limits: &Limits) -> Result<wire::ProposalHolder, wire::Problem> {
    Ok(match value {
        tasks::ProposalHolder::Task(value) => {
            wire::ProposalHolder::Task(wire::Number::new(&limits.core, wire::NumberParts { value: *value })?)
        }
        tasks::ProposalHolder::Person(value) => {
            wire::ProposalHolder::Person(wire::Number::new(&limits.core, wire::NumberParts { value: *value })?)
        }
        tasks::ProposalHolder::Policy { project, kind } => wire::ProposalHolder::Policy(wire::PolicyHolder::new(
            &limits.core,
            wire::PolicyHolderParts { project: *project, kind: super::encode_proposal_kind(kind, limits) },
        )?),
    })
}
fn decode_holder(value: wire::ProposalHolder, limits: &Limits) -> tasks::ProposalHolder {
    match value {
        wire::ProposalHolder::Task(value) => tasks::ProposalHolder::Task(value.into_parts().value),
        wire::ProposalHolder::Person(value) => tasks::ProposalHolder::Person(value.into_parts().value),
        wire::ProposalHolder::Policy(value) => {
            let wire::PolicyHolderParts { project, kind } = value.into_parts();
            tasks::ProposalHolder::Policy { project, kind: super::decode_proposal_kind(kind, limits) }
        }
    }
}

fn people_problem(value: crate::people_v13::Problem) -> wire::Problem {
    problem(value.reason)
}

fn encode_settings_refusal(
    value: &jig_domain_people::Refusal,
    limits: &Limits,
) -> Result<wire::CallPart, wire::Problem> {
    let value = match crate::codec::people::refusal_bytes(value) {
        Ok(value) => value,
        Err(problem) => return Err(people_problem(problem)),
    };
    Ok(wire::CallPart::SettingsRefused(wire::SettingsRefusal::new(&limits.core, wire::SettingsRefusalParts { value })?))
}
fn decode_settings_refusal(value: wire::SettingsRefusal) -> Result<domain::CallPart, wire::Problem> {
    match crate::codec::people::read_refusal(&value.into_parts().value) {
        Ok(value) => Ok(domain::CallPart::SettingsRefused(value)),
        Err(problem) => Err(people_problem(problem)),
    }
}

fn encode_part_note_refused(value: notes::Refusal, limits: &Limits) -> Result<wire::CallPart, wire::Problem> {
    Ok(wire::CallPart::NoteRefused(wire::CallPartNoteRefused::new(
        &limits.core,
        wire::CallPartNoteRefusedParts { value: encode_note_refusal(value, limits)? },
    )?))
}
