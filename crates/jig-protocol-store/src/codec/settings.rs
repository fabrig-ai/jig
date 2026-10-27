//! Typed settings records (protocol/store.md, sections 4.4 and 5).
#![expect(clippy::manual_map, reason = "step code translates optional fields through exhaustive matches")]
#![expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "typed encoders uniformly borrow the domain values they translate"
)]
use crate::settings_v1 as wire;
use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_core::{self as core, settings};
use jig_domain_fleet as fleet;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{List, Reader, Wall, Writer};
fn bound() -> wire::Problem {
    wire::Problem { path: wire::Path::DefaultValue, reason: skein_codec::Reason::Bound }
}
fn encode_kinds(value: &fleet::Kinds, _limits: &wire::Limits) -> wire::Kinds {
    match value {
        fleet::Kinds::Workers => wire::Kinds::Workers,
        fleet::Kinds::Engine => wire::Kinds::Engine,
        fleet::Kinds::Both => wire::Kinds::Both,
    }
}
fn decode_kinds(value: wire::Kinds, _limits: &wire::Limits) -> fleet::Kinds {
    match value {
        wire::Kinds::Workers => fleet::Kinds::Workers,
        wire::Kinds::Engine => fleet::Kinds::Engine,
        wire::Kinds::Both => fleet::Kinds::Both,
    }
}
fn encode_setting(value: &settings::Setting, limits: &wire::Limits) -> Result<wire::Setting, wire::Problem> {
    Ok(match value {
        settings::Setting::Charter(entry) => {
            wire::Setting::Charter(wire::CharterNumber::new(limits, wire::CharterNumberParts { number: *entry })?)
        }
        settings::Setting::Template(entry) => {
            wire::Setting::Template(wire::TemplateNumber::new(limits, wire::TemplateNumberParts { number: *entry })?)
        }
        settings::Setting::Policy(entry) => {
            wire::Setting::Policy(wire::PolicyNumber::new(limits, wire::PolicyNumberParts { number: *entry })?)
        }
        settings::Setting::Rules => wire::Setting::Rules,
    })
}
fn decode_setting(value: wire::Setting, _limits: &wire::Limits) -> settings::Setting {
    match value {
        wire::Setting::Charter(entry) => {
            let wire::CharterNumberParts { number } = entry.into_parts();
            settings::Setting::Charter(number)
        }
        wire::Setting::Template(entry) => {
            let wire::TemplateNumberParts { number } = entry.into_parts();
            settings::Setting::Template(number)
        }
        wire::Setting::Policy(entry) => {
            let wire::PolicyNumberParts { number } = entry.into_parts();
            settings::Setting::Policy(number)
        }
        wire::Setting::Rules => settings::Setting::Rules,
    }
}
fn encode_last(value: &authority::Last, limits: &wire::Limits) -> Result<wire::Last, wire::Problem> {
    Ok(match value {
        authority::Last::Exact(entry) => {
            wire::Last::Exact(wire::LastExact::new(limits, wire::LastExactParts { bytes: (*entry).clone() })?)
        }
        authority::Last::Open(entry) => {
            wire::Last::Open(wire::LastOpen::new(limits, wire::LastOpenParts { bytes: (*entry).clone() })?)
        }
    })
}
fn decode_last(value: wire::Last, _limits: &wire::Limits) -> authority::Last {
    match value {
        wire::Last::Exact(entry) => {
            let wire::LastExactParts { bytes } = entry.into_parts();
            authority::Last::Exact(bytes)
        }
        wire::Last::Open(entry) => {
            let wire::LastOpenParts { bytes } = entry.into_parts();
            authority::Last::Open(bytes)
        }
    }
}
fn encode_executor(value: &authority::Executor, limits: &wire::Limits) -> Result<wire::Executor, wire::Problem> {
    Ok(match value {
        authority::Executor::Charter(entry) => {
            wire::Executor::Charter(wire::ExecutorCharter::new(limits, wire::ExecutorCharterParts { number: *entry })?)
        }
        authority::Executor::Procedure(entry) => wire::Executor::Procedure(wire::ExecutorProcedure::new(
            limits,
            wire::ExecutorProcedureParts { number: *entry },
        )?),
        authority::Executor::Role(entry) => {
            wire::Executor::Role(wire::ExecutorRole::new(limits, wire::ExecutorRoleParts { number: *entry })?)
        }
    })
}
fn decode_executor(value: wire::Executor, _limits: &wire::Limits) -> authority::Executor {
    match value {
        wire::Executor::Charter(entry) => {
            let wire::ExecutorCharterParts { number } = entry.into_parts();
            authority::Executor::Charter(number)
        }
        wire::Executor::Procedure(entry) => {
            let wire::ExecutorProcedureParts { number } = entry.into_parts();
            authority::Executor::Procedure(number)
        }
        wire::Executor::Role(entry) => {
            let wire::ExecutorRoleParts { number } = entry.into_parts();
            authority::Executor::Role(number)
        }
    }
}
fn encode_guard(value: &authority::Guard, limits: &wire::Limits) -> Result<wire::Guard, wire::Problem> {
    Ok(match value {
        authority::Guard::Guarded => wire::Guard::Guarded,
        authority::Guard::Observed { freshness } => {
            wire::Guard::Observed(wire::Observed::new(limits, wire::ObservedParts { freshness: *freshness })?)
        }
    })
}
fn decode_guard(value: wire::Guard, _limits: &wire::Limits) -> authority::Guard {
    match value {
        wire::Guard::Guarded => authority::Guard::Guarded,
        wire::Guard::Observed(entry) => {
            let wire::ObservedParts { freshness } = entry.into_parts();
            authority::Guard::Observed { freshness }
        }
    }
}
fn encode_template_executor(
    value: &people::TemplateExecutor,
    limits: &wire::Limits,
) -> Result<wire::TemplateExecutor, wire::Problem> {
    Ok(match value {
        people::TemplateExecutor::Charter(entry) => wire::TemplateExecutor::Charter(wire::TemplateCharter::new(
            limits,
            wire::TemplateCharterParts { number: *entry },
        )?),
        people::TemplateExecutor::Chosen => wire::TemplateExecutor::Chosen,
    })
}
fn decode_template_executor(value: wire::TemplateExecutor, _limits: &wire::Limits) -> people::TemplateExecutor {
    match value {
        wire::TemplateExecutor::Charter(entry) => {
            let wire::TemplateCharterParts { number } = entry.into_parts();
            people::TemplateExecutor::Charter(number)
        }
        wire::TemplateExecutor::Chosen => people::TemplateExecutor::Chosen,
    }
}
fn encode_template_budget(
    value: &people::TemplateBudget,
    limits: &wire::Limits,
) -> Result<wire::TemplateBudget, wire::Problem> {
    Ok(match value {
        people::TemplateBudget::Amount(entry) => {
            wire::TemplateBudget::Amount(wire::Amount::new(limits, wire::AmountParts { amount: *entry })?)
        }
        people::TemplateBudget::AsAllowed { cap } => {
            wire::TemplateBudget::AsAllowed(wire::AsAllowed::new(limits, wire::AsAllowedParts { cap: *cap })?)
        }
        people::TemplateBudget::Given => wire::TemplateBudget::Given,
    })
}
fn decode_template_budget(value: wire::TemplateBudget, _limits: &wire::Limits) -> people::TemplateBudget {
    match value {
        wire::TemplateBudget::Amount(entry) => {
            let wire::AmountParts { amount } = entry.into_parts();
            people::TemplateBudget::Amount(amount)
        }
        wire::TemplateBudget::AsAllowed(entry) => {
            let wire::AsAllowedParts { cap } = entry.into_parts();
            people::TemplateBudget::AsAllowed { cap }
        }
        wire::TemplateBudget::Given => people::TemplateBudget::Given,
    }
}
fn encode_template_deadline(
    value: &people::TemplateDeadline,
    limits: &wire::Limits,
) -> Result<wire::TemplateDeadline, wire::Problem> {
    Ok(match value {
        people::TemplateDeadline::None => wire::TemplateDeadline::None,
        people::TemplateDeadline::Span(entry) => {
            wire::TemplateDeadline::Span(wire::Span::new(limits, wire::SpanParts { span: *entry })?)
        }
        people::TemplateDeadline::Given => wire::TemplateDeadline::Given,
    })
}
fn decode_template_deadline(value: wire::TemplateDeadline, _limits: &wire::Limits) -> people::TemplateDeadline {
    match value {
        wire::TemplateDeadline::None => people::TemplateDeadline::None,
        wire::TemplateDeadline::Span(entry) => {
            let wire::SpanParts { span } = entry.into_parts();
            people::TemplateDeadline::Span(span)
        }
        wire::TemplateDeadline::Given => people::TemplateDeadline::Given,
    }
}
fn encode_part_kind(value: &tasks::PartKind, _limits: &wire::Limits) -> wire::PartKind {
    match value {
        tasks::PartKind::Resource => wire::PartKind::Resource,
        tasks::PartKind::Pool => wire::PartKind::Pool,
        tasks::PartKind::Result => wire::PartKind::Result,
        tasks::PartKind::Procedure => wire::PartKind::Procedure,
        tasks::PartKind::Effect => wire::PartKind::Effect,
        tasks::PartKind::Outbox => wire::PartKind::Outbox,
        tasks::PartKind::Hold => wire::PartKind::Hold,
    }
}
fn decode_part_kind(value: wire::PartKind, _limits: &wire::Limits) -> tasks::PartKind {
    match value {
        wire::PartKind::Resource => tasks::PartKind::Resource,
        wire::PartKind::Pool => tasks::PartKind::Pool,
        wire::PartKind::Result => tasks::PartKind::Result,
        wire::PartKind::Procedure => tasks::PartKind::Procedure,
        wire::PartKind::Effect => tasks::PartKind::Effect,
        wire::PartKind::Outbox => tasks::PartKind::Outbox,
        wire::PartKind::Hold => tasks::PartKind::Hold,
    }
}
fn encode_part_key(value: &tasks::PartKey, limits: &wire::Limits) -> Result<wire::PartKey, wire::Problem> {
    Ok(match value {
        tasks::PartKey::Path(entry) => {
            let mut list_1 = List::with_capacity(limits.part_path_segments);
            for entry in entry {
                let element = encode_segment(entry, limits)?;
                if list_1.push(element).is_err() {
                    return Err(bound());
                }
            }
            wire::PartKey::Path(wire::PartPath::new(limits, wire::PartPathParts { segments: list_1 })?)
        }
        tasks::PartKey::Number(entry) => {
            wire::PartKey::Number(wire::PartNumber::new(limits, wire::PartNumberParts { number: *entry })?)
        }
    })
}
fn decode_part_key(value: wire::PartKey, limits: &wire::Limits) -> tasks::PartKey {
    match value {
        wire::PartKey::Path(entry) => {
            let wire::PartPathParts { segments } = entry.into_parts();
            let mut list_2 = List::with_capacity((segments).len());
            for entry in (segments).into_boxed() {
                let element = decode_segment(entry, limits);
                list_2.push(element).expect("decoded list bound");
            }
            tasks::PartKey::Path(list_2)
        }
        wire::PartKey::Number(entry) => {
            let wire::PartNumberParts { number } = entry.into_parts();
            tasks::PartKey::Number(number)
        }
    }
}
fn encode_contract(value: &tasks::Contract, limits: &wire::Limits) -> Result<wire::Contract, wire::Problem> {
    Ok(match value {
        tasks::Contract::Approval { state, part } => wire::Contract::Approval(wire::Approval::new(
            limits,
            wire::ApprovalParts { state: (*(state)).clone(), part: encode_part(part, limits)? },
        )?),
        tasks::Contract::Report { words } => {
            wire::Contract::Report(wire::Report::new(limits, wire::ReportParts { words: *words })?)
        }
        tasks::Contract::Verdict { choices } => {
            let mut list_3 = List::with_capacity(limits.verdict_choices_choices);
            for entry in choices {
                let element = encode_verdict(entry, limits)?;
                if list_3.push(element).is_err() {
                    return Err(bound());
                }
            }
            wire::Contract::Verdict(wire::VerdictChoices::new(limits, wire::VerdictChoicesParts { choices: list_3 })?)
        }
        tasks::Contract::Change { connector, kind, words } => wire::Contract::Change(wire::Change::new(
            limits,
            wire::ChangeParts { connector: *connector, kind: *kind, words: *words },
        )?),
    })
}
fn decode_contract(value: wire::Contract, limits: &wire::Limits) -> tasks::Contract {
    match value {
        wire::Contract::Approval(entry) => {
            let wire::ApprovalParts { state, part } = entry.into_parts();
            tasks::Contract::Approval { state, part: decode_part(part, limits) }
        }
        wire::Contract::Report(entry) => {
            let wire::ReportParts { words } = entry.into_parts();
            tasks::Contract::Report { words }
        }
        wire::Contract::Verdict(entry) => {
            let wire::VerdictChoicesParts { choices } = entry.into_parts();
            let mut list_4 = List::with_capacity((choices).len());
            for entry in (choices).into_boxed() {
                let element = decode_verdict(entry, limits);
                list_4.push(element).expect("decoded list bound");
            }
            tasks::Contract::Verdict { choices: list_4.into_boxed() }
        }
        wire::Contract::Change(entry) => {
            let wire::ChangeParts { connector, kind, words } = entry.into_parts();
            tasks::Contract::Change { connector, kind, words }
        }
    }
}
fn encode_retention(value: &tasks::Retention, _limits: &wire::Limits) -> wire::Retention {
    match value {
        tasks::Retention::Kept => wire::Retention::Kept,
        tasks::Retention::ForgottenOnceRead => wire::Retention::ForgottenOnceRead,
    }
}
fn decode_retention(value: wire::Retention, _limits: &wire::Limits) -> tasks::Retention {
    match value {
        wire::Retention::Kept => tasks::Retention::Kept,
        wire::Retention::ForgottenOnceRead => tasks::Retention::ForgottenOnceRead,
    }
}
fn encode_ended_by(value: &tasks::EndedBy, _limits: &wire::Limits) -> wire::EndedBy {
    match value {
        tasks::EndedBy::Result => wire::EndedBy::Result,
        tasks::EndedBy::Requester => wire::EndedBy::Requester,
    }
}
fn decode_ended_by(value: wire::EndedBy, _limits: &wire::Limits) -> tasks::EndedBy {
    match value {
        wire::EndedBy::Result => tasks::EndedBy::Result,
        wire::EndedBy::Requester => tasks::EndedBy::Requester,
    }
}
fn encode_value(value: &settings::Value, limits: &wire::Limits) -> Result<wire::Value, wire::Problem> {
    Ok(match value {
        settings::Value::Charter(entry) => wire::Value::Charter(Box::new(encode_charter(entry, limits)?)),
        settings::Value::Template(entry) => wire::Value::Template(Box::new(encode_template(entry, limits)?)),
        settings::Value::Policy(entry) => wire::Value::Policy(Box::new(encode_configured_policy(entry, limits)?)),
        settings::Value::Rules(entry) => wire::Value::Rules(Box::new(encode_rules(entry, limits)?)),
    })
}
fn decode_value(value: wire::Value, limits: &wire::Limits) -> Result<settings::Value, wire::Problem> {
    Ok(match value {
        wire::Value::Charter(entry) => settings::Value::Charter(Box::new(decode_charter(*entry, limits))),
        wire::Value::Template(entry) => settings::Value::Template(Box::new(decode_template(*entry, limits))),
        wire::Value::Policy(entry) => settings::Value::Policy(Box::new(decode_configured_policy(*entry, limits))),
        wire::Value::Rules(entry) => settings::Value::Rules(Box::new(decode_rules(*entry, limits)?)),
    })
}
fn encode_decider(value: &settings::Decider, limits: &wire::Limits) -> Result<wire::Decider, wire::Problem> {
    Ok(match value {
        settings::Decider::FirstStart => wire::Decider::FirstStart,
        settings::Decider::Release => wire::Decider::Release,
        settings::Decider::Party(entry) => {
            wire::Decider::Party(wire::Party::new(limits, wire::PartyParts { number: *entry })?)
        }
    })
}
fn decode_decider(value: wire::Decider, _limits: &wire::Limits) -> settings::Decider {
    match value {
        wire::Decider::FirstStart => settings::Decider::FirstStart,
        wire::Decider::Release => settings::Decider::Release,
        wire::Decider::Party(entry) => {
            let wire::PartyParts { number } = entry.into_parts();
            settings::Decider::Party(number)
        }
    }
}
fn encode_proposer(value: &people::Proposer, limits: &wire::Limits) -> Result<wire::Proposer, wire::Problem> {
    Ok(match value {
        people::Proposer::Task(entry) => {
            wire::Proposer::Task(wire::Party::new(limits, wire::PartyParts { number: *entry })?)
        }
        people::Proposer::Party(entry) => {
            wire::Proposer::Party(wire::Party::new(limits, wire::PartyParts { number: *entry })?)
        }
    })
}
fn decode_proposer(value: wire::Proposer, _limits: &wire::Limits) -> people::Proposer {
    match value {
        wire::Proposer::Task(entry) => {
            let wire::PartyParts { number } = entry.into_parts();
            people::Proposer::Task(number)
        }
        wire::Proposer::Party(entry) => {
            let wire::PartyParts { number } = entry.into_parts();
            people::Proposer::Party(number)
        }
    }
}
fn encode_edit(value: &settings::Edit, limits: &wire::Limits) -> Result<wire::Edit, wire::Problem> {
    Ok(match value {
        settings::Edit::Set(entry) => {
            wire::Edit::Set(wire::Set::new(limits, wire::SetParts { value: encode_value(entry, limits)? })?)
        }
        settings::Edit::Removed => wire::Edit::Removed,
        settings::Edit::Reset => wire::Edit::Reset,
    })
}
fn decode_edit(value: wire::Edit, limits: &wire::Limits) -> Result<settings::Edit, wire::Problem> {
    Ok(match value {
        wire::Edit::Set(entry) => {
            let wire::SetParts { value } = entry.into_parts();
            settings::Edit::Set(decode_value(value, limits)?)
        }
        wire::Edit::Removed => settings::Edit::Removed,
        wire::Edit::Reset => settings::Edit::Reset,
    })
}
fn encode_text(value: &[u8], limits: &wire::Limits) -> Result<wire::Text, wire::Problem> {
    let field_bytes = Box::from(value);
    wire::Text::new(limits, wire::TextParts { bytes: field_bytes })
}
fn decode_text(value: wire::Text, _limits: &wire::Limits) -> Box<[u8]> {
    let wire::TextParts { bytes } = value.into_parts();

    bytes
}
fn encode_model(value: &core::Model, limits: &wire::Limits) -> Result<wire::Model, wire::Problem> {
    let field_dialect = value.dialect;
    let field_endpoint = value.endpoint;
    let field_program = match &value.program {
        Some(entry) => Some(entry.clone()),
        None => None,
    };
    let field_name = value.name.clone();
    let field_window = value.window;
    let field_output = value.output;
    let field_input_price = value.input_price;
    let field_cached_price = value.cached_price;
    let field_cache_write_price = value.cache_write_price;
    let field_output_price = value.output_price;
    let field_price_unit = value.price_unit;
    let mut list_5 = List::with_capacity(limits.model_efforts);
    for entry in &value.efforts {
        let element = encode_text(entry, limits)?;
        if list_5.push(element).is_err() {
            return Err(bound());
        }
    }
    let field_efforts = list_5;
    let field_effort = match &value.effort {
        Some(entry) => Some(entry.clone()),
        None => None,
    };
    wire::Model::new(
        limits,
        wire::ModelParts {
            dialect: field_dialect,
            endpoint: field_endpoint,
            program: field_program,
            name: field_name,
            window: field_window,
            output: field_output,
            input_price: field_input_price,
            cached_price: field_cached_price,
            cache_write_price: field_cache_write_price,
            output_price: field_output_price,
            price_unit: field_price_unit,
            efforts: field_efforts,
            effort: field_effort,
        },
    )
}
fn decode_model(value: wire::Model, limits: &wire::Limits) -> core::Model {
    let wire::ModelParts {
        dialect,
        endpoint,
        program,
        name,
        window,
        output,
        input_price,
        cached_price,
        cache_write_price,
        output_price,
        price_unit,
        efforts,
        effort,
    } = value.into_parts();
    let field_dialect = dialect;
    let field_endpoint = endpoint;
    let field_program = program;
    let field_name = name;
    let field_window = window;
    let field_output = output;
    let field_input_price = input_price;
    let field_cached_price = cached_price;
    let field_cache_write_price = cache_write_price;
    let field_output_price = output_price;
    let field_price_unit = price_unit;
    let mut list_6 = List::with_capacity((efforts).len());
    for entry in (efforts).into_boxed() {
        let element = decode_text(entry, limits);
        list_6.push(element).expect("decoded list bound");
    }
    let field_efforts = list_6.into_boxed();
    let field_effort = effort;
    core::Model {
        dialect: field_dialect,
        endpoint: field_endpoint,
        program: field_program,
        name: field_name,
        window: field_window,
        output: field_output,
        input_price: field_input_price,
        cached_price: field_cached_price,
        cache_write_price: field_cache_write_price,
        output_price: field_output_price,
        price_unit: field_price_unit,
        efforts: field_efforts,
        effort: field_effort,
    }
}
fn encode_run_policy(value: &core::RunPolicy, limits: &wire::Limits) -> Result<wire::RunPolicy, wire::Problem> {
    let field_instructions = value.instructions.clone();
    let field_waiting = value.waiting;
    let field_resume = value.resume;
    let field_turns = value.turns;
    let field_time = value.time;
    let field_reserve_turns = value.reserve_turns;
    let field_reserve_time = value.reserve_time;
    let field_model = encode_model(&value.model, limits)?;
    let mut list_7 = List::with_capacity(limits.run_policy_alternatives);
    for entry in &value.alternatives {
        let element = encode_model(entry, limits)?;
        if list_7.push(element).is_err() {
            return Err(bound());
        }
    }
    let field_alternatives = list_7;
    let field_inspect = value.inspect;
    let field_modify = value.modify;
    let field_shell = value.shell;
    let field_agents = value.agents;
    let field_call_timeout = value.call_timeout;
    wire::RunPolicy::new(
        limits,
        wire::RunPolicyParts {
            tools: value.tools.0,
            instructions: field_instructions,
            waiting: field_waiting,
            turns: field_turns,
            time: field_time,
            reserve_turns: field_reserve_turns,
            reserve_time: field_reserve_time,
            model: field_model,
            alternatives: field_alternatives,
            call_timeout: field_call_timeout,
            skein_bools: [field_resume, field_inspect, field_modify, field_shell, field_agents],
        },
    )
}
fn decode_run_policy(value: wire::RunPolicy, limits: &wire::Limits) -> core::RunPolicy {
    let wire::RunPolicyParts {
        tools,
        instructions,
        waiting,
        turns,
        time,
        reserve_turns,
        reserve_time,
        model,
        alternatives,
        call_timeout,
        skein_bools: [resume, inspect, modify, shell, agents],
    } = value.into_parts();
    let field_instructions = instructions;
    let field_waiting = waiting;
    let field_resume = resume;
    let field_turns = turns;
    let field_time = time;
    let field_reserve_turns = reserve_turns;
    let field_reserve_time = reserve_time;
    let field_model = decode_model(model, limits);
    let mut list_8 = List::with_capacity((alternatives).len());
    for entry in (alternatives).into_boxed() {
        let element = decode_model(entry, limits);
        list_8.push(element).expect("decoded list bound");
    }
    let field_alternatives = list_8.into_boxed();
    let field_inspect = inspect;
    let field_modify = modify;
    let field_shell = shell;
    let field_agents = agents;
    let field_call_timeout = call_timeout;
    core::RunPolicy {
        tools: authority::Tools(tools),
        instructions: field_instructions,
        waiting: field_waiting,
        resume: field_resume,
        turns: field_turns,
        time: field_time,
        reserve_turns: field_reserve_turns,
        reserve_time: field_reserve_time,
        model: field_model,
        alternatives: field_alternatives,
        inspect: field_inspect,
        modify: field_modify,
        shell: field_shell,
        agents: field_agents,
        call_timeout: field_call_timeout,
    }
}
fn encode_charter(value: &core::Charter, limits: &wire::Limits) -> Result<wire::Charter, wire::Problem> {
    let field_number = value.number;
    let field_policy = encode_run_policy(&value.policy, limits)?;
    let field_hosts = encode_kinds(&value.hosts, limits);
    let field_spend = value.spend;
    wire::Charter::new(
        limits,
        wire::CharterParts { number: field_number, policy: field_policy, hosts: field_hosts, spend: field_spend },
    )
}
fn decode_charter(value: wire::Charter, limits: &wire::Limits) -> core::Charter {
    let wire::CharterParts { number, policy, hosts, spend } = value.into_parts();
    let field_number = number;
    let field_policy = decode_run_policy(policy, limits);
    let field_hosts = decode_kinds(hosts, limits);
    let field_spend = spend;
    core::Charter { number: field_number, policy: field_policy, hosts: field_hosts, spend: field_spend }
}
fn encode_segment(value: &[u8], limits: &wire::Limits) -> Result<wire::Segment, wire::Problem> {
    let field_bytes = Box::from(value);
    wire::Segment::new(limits, wire::SegmentParts { bytes: field_bytes })
}
fn decode_segment(value: wire::Segment, _limits: &wire::Limits) -> Box<[u8]> {
    let wire::SegmentParts { bytes } = value.into_parts();

    bytes
}
fn encode_pattern(value: &authority::Pattern, limits: &wire::Limits) -> Result<wire::Pattern, wire::Problem> {
    let mut list_9 = List::with_capacity(limits.pattern_segments);
    for entry in &value.segments {
        let element = encode_segment(entry, limits)?;
        if list_9.push(element).is_err() {
            return Err(bound());
        }
    }
    let field_segments = list_9;
    let field_last = encode_last(&value.last, limits)?;
    wire::Pattern::new(limits, wire::PatternParts { segments: field_segments, last: field_last })
}
fn decode_pattern(value: wire::Pattern, limits: &wire::Limits) -> authority::Pattern {
    let wire::PatternParts { segments, last } = value.into_parts();
    let mut list_10 = List::with_capacity((segments).len());
    for entry in (segments).into_boxed() {
        let element = decode_segment(entry, limits);
        list_10.push(element).expect("decoded list bound");
    }
    let field_segments = list_10.into_boxed();
    let field_last = decode_last(last, limits);
    authority::Pattern { segments: field_segments, last: field_last }
}
fn encode_grant(value: &authority::Grant, limits: &wire::Limits) -> Result<wire::Grant, wire::Problem> {
    let field_connector = value.connector;
    let field_kind = value.kind;
    let field_pattern = encode_pattern(&value.pattern, limits)?;
    wire::Grant::new(limits, wire::GrantParts { connector: field_connector, kind: field_kind, pattern: field_pattern })
}
fn decode_grant(value: wire::Grant, limits: &wire::Limits) -> authority::Grant {
    let wire::GrantParts { connector, kind, pattern } = value.into_parts();
    let field_connector = connector;
    let field_kind = kind;
    let field_pattern = decode_pattern(pattern, limits);
    authority::Grant { connector: field_connector, kind: field_kind, pattern: field_pattern }
}
fn encode_resource_scope(
    value: &authority::ResourceScope,
    limits: &wire::Limits,
) -> Result<wire::ResourceScope, wire::Problem> {
    let field_connector = value.connector;
    let field_pattern = encode_pattern(&value.pattern, limits)?;
    wire::ResourceScope::new(limits, wire::ResourceScopeParts { connector: field_connector, pattern: field_pattern })
}
fn decode_resource_scope(value: wire::ResourceScope, limits: &wire::Limits) -> authority::ResourceScope {
    let wire::ResourceScopeParts { connector, pattern } = value.into_parts();
    let field_connector = connector;
    let field_pattern = decode_pattern(pattern, limits);
    authority::ResourceScope { connector: field_connector, pattern: field_pattern }
}
fn encode_delegation(value: &authority::Delegation, limits: &wire::Limits) -> Result<wire::Delegation, wire::Problem> {
    let mut list_11 = List::with_capacity(limits.delegation_kinds);
    for entry in &value.kinds {
        let element = encode_executor(entry, limits)?;
        if list_11.push(element).is_err() {
            return Err(bound());
        }
    }
    let field_kinds = list_11;
    let field_tasks = value.tasks;
    let field_depth = value.depth;
    wire::Delegation::new(limits, wire::DelegationParts { kinds: field_kinds, tasks: field_tasks, depth: field_depth })
}
fn decode_delegation(value: wire::Delegation, limits: &wire::Limits) -> authority::Delegation {
    let wire::DelegationParts { kinds, tasks, depth } = value.into_parts();
    let mut list_12 = List::with_capacity((kinds).len());
    for entry in (kinds).into_boxed() {
        let element = decode_executor(entry, limits);
        list_12.push(element).expect("decoded list bound");
    }
    let field_kinds = list_12.into_boxed();
    let field_tasks = tasks;
    let field_depth = depth;
    authority::Delegation { kinds: field_kinds, tasks: field_tasks, depth: field_depth }
}
fn encode_models(value: &authority::Models, limits: &wire::Limits) -> Result<wire::Models, wire::Problem> {
    let field_bits = value.bits();
    let field_all = value.is_all();
    wire::Models::new(limits, wire::ModelsParts { bits: field_bits, all: field_all })
}
fn decode_models(value: wire::Models, _limits: &wire::Limits) -> authority::Models {
    let wire::ModelsParts { bits, all } = value.into_parts();
    let field_bits = bits;
    let field_all = all;
    authority::Models::from_parts(field_bits, field_all)
}
fn encode_budget(value: &authority::Budget, limits: &wire::Limits) -> Result<wire::Budget, wire::Problem> {
    let field_spend = value.spend;
    let field_deadline = match value.deadline {
        Some(deadline) => Some(deadline.as_nanos()),
        None => None,
    };
    wire::Budget::new(limits, wire::BudgetParts { spend: field_spend, deadline: field_deadline })
}
fn decode_budget(value: wire::Budget, _limits: &wire::Limits) -> authority::Budget {
    let wire::BudgetParts { spend, deadline } = value.into_parts();
    let field_spend = spend;
    let field_deadline = match deadline {
        Some(deadline) => Some(Wall::from_nanos(deadline)),
        None => None,
    };
    authority::Budget { spend: field_spend, deadline: field_deadline }
}
fn encode_authority(value: &authority::Authority, limits: &wire::Limits) -> Result<wire::Authority, wire::Problem> {
    let field_tools = value.tools.0;
    let field_models = encode_models(&value.models, limits)?;
    let mut list_13 = List::with_capacity(limits.authority_grants);
    for entry in &value.grants {
        let element = encode_grant(entry, limits)?;
        if list_13.push(element).is_err() {
            return Err(bound());
        }
    }
    let field_grants = list_13;
    let field_delegation = encode_delegation(&value.delegation, limits)?;
    let field_budget = encode_budget(&value.budget, limits)?;
    let field_notes = value.notes.0;
    let mut list_14 = List::with_capacity(limits.authority_note_resources);
    for entry in &value.note_resources {
        let element = encode_resource_scope(entry, limits)?;
        if list_14.push(element).is_err() {
            return Err(bound());
        }
    }
    let field_note_resources = list_14;
    wire::Authority::new(
        limits,
        wire::AuthorityParts {
            tools: field_tools,
            models: field_models,
            grants: field_grants,
            delegation: field_delegation,
            budget: field_budget,
            notes: field_notes,
            note_resources: field_note_resources,
        },
    )
}
fn decode_authority(value: wire::Authority, limits: &wire::Limits) -> authority::Authority {
    let wire::AuthorityParts { tools, models, grants, delegation, budget, notes, note_resources } = value.into_parts();
    let field_tools = authority::Tools(tools);
    let field_models = decode_models(models, limits);
    let mut list_15 = List::with_capacity((grants).len());
    for entry in (grants).into_boxed() {
        let element = decode_grant(entry, limits);
        list_15.push(element).expect("decoded list bound");
    }
    let field_grants = list_15.into_boxed();
    let field_delegation = decode_delegation(delegation, limits);
    let field_budget = decode_budget(budget, limits);
    let field_notes = authority::Scopes(notes);
    let mut list_16 = List::with_capacity((note_resources).len());
    for entry in (note_resources).into_boxed() {
        let element = decode_resource_scope(entry, limits);
        list_16.push(element).expect("decoded list bound");
    }
    let field_note_resources = list_16.into_boxed();
    authority::Authority {
        tools: field_tools,
        models: field_models,
        grants: field_grants,
        delegation: field_delegation,
        budget: field_budget,
        notes: field_notes,
        note_resources: field_note_resources,
    }
}
fn encode_judge(value: &authority::Judge, limits: &wire::Limits) -> Result<wire::Judge, wire::Problem> {
    let field_connector = value.connector;
    let field_requirement = value.requirement;
    let field_parameters = value.parameters;
    wire::Judge::new(
        limits,
        wire::JudgeParts { connector: field_connector, requirement: field_requirement, parameters: field_parameters },
    )
}
fn decode_judge(value: wire::Judge, _limits: &wire::Limits) -> authority::Judge {
    let wire::JudgeParts { connector, requirement, parameters } = value.into_parts();
    let field_connector = connector;
    let field_requirement = requirement;
    let field_parameters = parameters;
    authority::Judge { connector: field_connector, requirement: field_requirement, parameters: field_parameters }
}
fn encode_requirement(
    value: &authority::Requirement,
    limits: &wire::Limits,
) -> Result<wire::Requirement, wire::Problem> {
    let field_connector = value.connector;
    let field_kind = value.kind;
    let field_pattern = encode_pattern(&value.pattern, limits)?;
    let field_judge = encode_judge(&value.judge, limits)?;
    let field_guard = encode_guard(&value.guard, limits)?;
    let field_must_be_guarded = value.must_be_guarded;
    wire::Requirement::new(
        limits,
        wire::RequirementParts {
            connector: field_connector,
            kind: field_kind,
            pattern: field_pattern,
            judge: field_judge,
            guard: field_guard,
            must_be_guarded: field_must_be_guarded,
        },
    )
}
fn decode_requirement(value: wire::Requirement, limits: &wire::Limits) -> authority::Requirement {
    let wire::RequirementParts { connector, kind, pattern, judge, guard, must_be_guarded } = value.into_parts();
    let field_connector = connector;
    let field_kind = kind;
    let field_pattern = decode_pattern(pattern, limits);
    let field_judge = decode_judge(judge, limits);
    let field_guard = decode_guard(guard, limits);
    let field_must_be_guarded = must_be_guarded;
    authority::Requirement {
        connector: field_connector,
        kind: field_kind,
        pattern: field_pattern,
        judge: field_judge,
        guard: field_guard,
        must_be_guarded: field_must_be_guarded,
    }
}
fn encode_implication(
    value: &authority::Implication,
    limits: &wire::Limits,
) -> Result<wire::Implication, wire::Problem> {
    let field_connector = value.connector;
    let field_kind = value.kind;
    let field_implies = value.implies;
    wire::Implication::new(
        limits,
        wire::ImplicationParts { connector: field_connector, kind: field_kind, implies: field_implies },
    )
}
fn decode_implication(value: wire::Implication, _limits: &wire::Limits) -> authority::Implication {
    let wire::ImplicationParts { connector, kind, implies } = value.into_parts();
    let field_connector = connector;
    let field_kind = kind;
    let field_implies = implies;
    authority::Implication { connector: field_connector, kind: field_kind, implies: field_implies }
}
fn encode_rules(value: &authority::Rules, limits: &wire::Limits) -> Result<wire::Rules, wire::Problem> {
    let field_ceiling = encode_authority(&value.ceiling, limits)?;
    let field_period_spend = value.period_spend;
    let field_minimum_run_spend = value.minimum_run_spend;
    let field_maximum_run_spend = value.maximum_run_spend;
    let mut list_17 = List::with_capacity(limits.rules_implies);
    for entry in value.implies.entries() {
        let element = encode_implication(entry, limits)?;
        if list_17.push(element).is_err() {
            return Err(bound());
        }
    }
    let field_implies = list_17;
    let mut list_18 = List::with_capacity(limits.rules_requirements);
    for entry in &value.requirements {
        let element = encode_requirement(entry, limits)?;
        if list_18.push(element).is_err() {
            return Err(bound());
        }
    }
    let field_requirements = list_18;
    wire::Rules::new(
        limits,
        wire::RulesParts {
            ceiling: field_ceiling,
            period_spend: field_period_spend,
            minimum_run_spend: field_minimum_run_spend,
            maximum_run_spend: field_maximum_run_spend,
            implies: field_implies,
            requirements: field_requirements,
        },
    )
}
fn decode_rules(value: wire::Rules, limits: &wire::Limits) -> Result<authority::Rules, wire::Problem> {
    let wire::RulesParts { ceiling, period_spend, minimum_run_spend, maximum_run_spend, implies, requirements } =
        value.into_parts();
    let field_ceiling = decode_authority(ceiling, limits);
    let field_period_spend = period_spend;
    let field_minimum_run_spend = minimum_run_spend;
    let field_maximum_run_spend = maximum_run_spend;
    let mut list_19 = List::with_capacity((implies).len());
    for entry in (implies).into_boxed() {
        let element = decode_implication(entry, limits);
        list_19.push(element).expect("decoded list bound");
    }
    let field_implies = authority::Implies::new(list_19.into_boxed(), limits.rules_implies).ok_or(bound())?;
    let mut list_20 = List::with_capacity((requirements).len());
    for entry in (requirements).into_boxed() {
        let element = decode_requirement(entry, limits);
        list_20.push(element).expect("decoded list bound");
    }
    let field_requirements = list_20.into_boxed();
    Ok(authority::Rules {
        ceiling: field_ceiling,
        period_spend: field_period_spend,
        minimum_run_spend: field_minimum_run_spend,
        maximum_run_spend: field_maximum_run_spend,
        implies: field_implies,
        requirements: field_requirements,
    })
}
fn encode_role(value: &authority::Role, limits: &wire::Limits) -> Result<wire::Role, wire::Problem> {
    let field_number = value.number;
    let field_authority = encode_authority(&value.authority, limits)?;
    let field_period_spend = value.period_spend;
    let field_requests = value.requests.0;
    let field_decides = value.decides.0;
    wire::Role::new(
        limits,
        wire::RoleParts {
            number: field_number,
            authority: field_authority,
            period_spend: field_period_spend,
            requests: field_requests,
            decides: field_decides,
        },
    )
}
fn decode_role(value: wire::Role, limits: &wire::Limits) -> authority::Role {
    let wire::RoleParts { number, authority, period_spend, requests, decides } = value.into_parts();
    let field_number = number;
    let field_authority = decode_authority(authority, limits);
    let field_period_spend = period_spend;
    let field_requests = authority::Requests(requests);
    let field_decides = authority::Proposals(decides);
    authority::Role {
        number: field_number,
        authority: field_authority,
        period_spend: field_period_spend,
        requests: field_requests,
        decides: field_decides,
    }
}
fn encode_policy(value: &authority::Policy, limits: &wire::Limits) -> Result<wire::Policy, wire::Problem> {
    let field_escalation_role = value.escalation_role;
    let field_ceiling = encode_authority(&value.ceiling, limits)?;
    let mut list_21 = List::with_capacity(limits.policy_projections);
    for entry in &value.projections {
        let element = encode_grant(entry, limits)?;
        if list_21.push(element).is_err() {
            return Err(bound());
        }
    }
    let field_projections = list_21;
    let field_period_spend = value.period_spend;
    let mut list_22 = List::with_capacity(limits.policy_roles);
    for entry in &value.roles {
        let element = encode_role(entry, limits)?;
        if list_22.push(element).is_err() {
            return Err(bound());
        }
    }
    let field_roles = list_22;
    let mut list_23 = List::with_capacity(limits.policy_requirements);
    for entry in &value.requirements {
        let element = encode_requirement(entry, limits)?;
        if list_23.push(element).is_err() {
            return Err(bound());
        }
    }
    let field_requirements = list_23;
    wire::Policy::new(
        limits,
        wire::PolicyParts {
            escalation_role: field_escalation_role,
            ceiling: field_ceiling,
            projections: field_projections,
            period_spend: field_period_spend,
            roles: field_roles,
            requirements: field_requirements,
        },
    )
}
fn decode_policy(value: wire::Policy, limits: &wire::Limits) -> authority::Policy {
    let wire::PolicyParts { escalation_role, ceiling, projections, period_spend, roles, requirements } =
        value.into_parts();
    let field_escalation_role = escalation_role;
    let field_ceiling = decode_authority(ceiling, limits);
    let mut list_24 = List::with_capacity((projections).len());
    for entry in (projections).into_boxed() {
        let element = decode_grant(entry, limits);
        list_24.push(element).expect("decoded list bound");
    }
    let field_projections = list_24.into_boxed();
    let field_period_spend = period_spend;
    let mut list_25 = List::with_capacity((roles).len());
    for entry in (roles).into_boxed() {
        let element = decode_role(entry, limits);
        list_25.push(element).expect("decoded list bound");
    }
    let field_roles = list_25.into_boxed();
    let mut list_26 = List::with_capacity((requirements).len());
    for entry in (requirements).into_boxed() {
        let element = decode_requirement(entry, limits);
        list_26.push(element).expect("decoded list bound");
    }
    let field_requirements = list_26.into_boxed();
    authority::Policy {
        escalation_role: field_escalation_role,
        ceiling: field_ceiling,
        projections: field_projections,
        period_spend: field_period_spend,
        roles: field_roles,
        requirements: field_requirements,
    }
}
fn encode_permission_role(
    value: &people::PermissionRole,
    limits: &wire::Limits,
) -> Result<wire::PermissionRole, wire::Problem> {
    let field_connector = value.connector;
    let field_permission = value.permission;
    let field_role = value.role;
    wire::PermissionRole::new(
        limits,
        wire::PermissionRoleParts { connector: field_connector, permission: field_permission, role: field_role },
    )
}
fn decode_permission_role(value: wire::PermissionRole, _limits: &wire::Limits) -> people::PermissionRole {
    let wire::PermissionRoleParts { connector, permission, role } = value.into_parts();
    let field_connector = connector;
    let field_permission = permission;
    let field_role = role;
    people::PermissionRole { connector: field_connector, permission: field_permission, role: field_role }
}
fn encode_template_override(
    value: &people::TemplateOverride,
    limits: &wire::Limits,
) -> Result<wire::TemplateOverride, wire::Problem> {
    let field_template = value.template;
    let field_executor = match &value.executor {
        Some(entry) => Some(encode_template_executor(entry, limits)?),
        None => None,
    };
    let field_budget = match &value.budget {
        Some(entry) => Some(encode_template_budget(entry, limits)?),
        None => None,
    };
    let field_deadline = match &value.deadline {
        Some(entry) => Some(encode_template_deadline(entry, limits)?),
        None => None,
    };
    let field_priority = value.priority;
    wire::TemplateOverride::new(
        limits,
        wire::TemplateOverrideParts {
            template: field_template,
            executor: field_executor,
            budget: field_budget,
            deadline: field_deadline,
            priority: field_priority,
        },
    )
}
fn decode_template_override(value: wire::TemplateOverride, limits: &wire::Limits) -> people::TemplateOverride {
    let wire::TemplateOverrideParts { template, executor, budget, deadline, priority } = value.into_parts();
    let field_template = template;
    let field_executor = match executor {
        Some(entry) => Some(decode_template_executor(entry, limits)),
        None => None,
    };
    let field_budget = match budget {
        Some(entry) => Some(decode_template_budget(entry, limits)),
        None => None,
    };
    let field_deadline = match deadline {
        Some(entry) => Some(decode_template_deadline(entry, limits)),
        None => None,
    };
    let field_priority = priority;
    people::TemplateOverride {
        template: field_template,
        executor: field_executor,
        budget: field_budget,
        deadline: field_deadline,
        priority: field_priority,
    }
}
fn encode_configured_policy(
    value: &core::ConfiguredPolicy,
    limits: &wire::Limits,
) -> Result<wire::ConfiguredPolicy, wire::Problem> {
    let field_policy = encode_policy(&value.policy, limits)?;
    let mut list_27 = List::with_capacity(limits.configured_policy_permissions);
    for entry in &value.permissions {
        let element = encode_permission_role(entry, limits)?;
        if list_27.push(element).is_err() {
            return Err(bound());
        }
    }
    let field_permissions = list_27;
    let mut list_28 = List::with_capacity(limits.configured_policy_templates);
    for entry in &value.templates {
        let element = encode_template_override(entry, limits)?;
        if list_28.push(element).is_err() {
            return Err(bound());
        }
    }
    let field_templates = list_28;
    wire::ConfiguredPolicy::new(
        limits,
        wire::ConfiguredPolicyParts {
            policy: field_policy,
            permissions: field_permissions,
            templates: field_templates,
        },
    )
}
fn decode_configured_policy(value: wire::ConfiguredPolicy, limits: &wire::Limits) -> core::ConfiguredPolicy {
    let wire::ConfiguredPolicyParts { policy, permissions, templates } = value.into_parts();
    let field_policy = decode_policy(policy, limits);
    let mut list_29 = List::with_capacity((permissions).len());
    for entry in (permissions).into_boxed() {
        let element = decode_permission_role(entry, limits);
        list_29.push(element).expect("decoded list bound");
    }
    let field_permissions = list_29.into_boxed();
    let mut list_30 = List::with_capacity((templates).len());
    for entry in (templates).into_boxed() {
        let element = decode_template_override(entry, limits);
        list_30.push(element).expect("decoded list bound");
    }
    let field_templates = list_30.into_boxed();
    core::ConfiguredPolicy { policy: field_policy, permissions: field_permissions, templates: field_templates }
}
fn encode_field_rule(value: &tasks::FieldRule, limits: &wire::Limits) -> Result<wire::FieldRule, wire::Problem> {
    let field_name = value.name.clone();
    let field_max = value.max;
    wire::FieldRule::new(limits, wire::FieldRuleParts { name: field_name, max: field_max })
}
fn decode_field_rule(value: wire::FieldRule, _limits: &wire::Limits) -> tasks::FieldRule {
    let wire::FieldRuleParts { name, max } = value.into_parts();
    let field_name = name;
    let field_max = max;
    tasks::FieldRule { name: field_name, max: field_max }
}
fn encode_verdict(value: &tasks::Verdict, limits: &wire::Limits) -> Result<wire::Verdict, wire::Problem> {
    let field_code = value.code;
    let field_words = value.words;
    let field_followups = value.followups;
    let mut list_31 = List::with_capacity(limits.verdict_fields);
    for entry in &value.fields {
        let element = encode_field_rule(entry, limits)?;
        if list_31.push(element).is_err() {
            return Err(bound());
        }
    }
    let field_fields = list_31;
    wire::Verdict::new(
        limits,
        wire::VerdictParts { code: field_code, words: field_words, followups: field_followups, fields: field_fields },
    )
}
fn decode_verdict(value: wire::Verdict, limits: &wire::Limits) -> tasks::Verdict {
    let wire::VerdictParts { code, words, followups, fields } = value.into_parts();
    let field_code = code;
    let field_words = words;
    let field_followups = followups;
    let mut list_32 = List::with_capacity((fields).len());
    for entry in (fields).into_boxed() {
        let element = decode_field_rule(entry, limits);
        list_32.push(element).expect("decoded list bound");
    }
    let field_fields = list_32.into_boxed();
    tasks::Verdict { code: field_code, words: field_words, followups: field_followups, fields: field_fields }
}
fn encode_part(value: &tasks::Part, limits: &wire::Limits) -> Result<wire::Part, wire::Problem> {
    let field_connector = value.connector;
    let field_kind = encode_part_kind(&value.kind, limits);
    let field_key = encode_part_key(&value.key, limits)?;
    wire::Part::new(limits, wire::PartParts { connector: field_connector, kind: field_kind, key: field_key })
}
fn decode_part(value: wire::Part, limits: &wire::Limits) -> tasks::Part {
    let wire::PartParts { connector, kind, key } = value.into_parts();
    let field_connector = connector;
    let field_kind = decode_part_kind(kind, limits);
    let field_key = decode_part_key(key, limits);
    tasks::Part { connector: field_connector, kind: field_kind, key: field_key }
}
fn encode_flags(value: &tasks::Flags, limits: &wire::Limits) -> Result<wire::Flags, wire::Problem> {
    let field_conversation = value.conversation;
    let field_listed = value.listed;
    let field_retention = encode_retention(&value.retention, limits);
    let field_ended_by = encode_ended_by(&value.ended_by, limits);
    wire::Flags::new(
        limits,
        wire::FlagsParts {
            conversation: field_conversation,
            listed: field_listed,
            retention: field_retention,
            ended_by: field_ended_by,
        },
    )
}
fn decode_flags(value: wire::Flags, limits: &wire::Limits) -> tasks::Flags {
    let wire::FlagsParts { conversation, listed, retention, ended_by } = value.into_parts();
    let field_conversation = conversation;
    let field_listed = listed;
    let field_retention = decode_retention(retention, limits);
    let field_ended_by = decode_ended_by(ended_by, limits);
    tasks::Flags {
        conversation: field_conversation,
        listed: field_listed,
        retention: field_retention,
        ended_by: field_ended_by,
    }
}
fn encode_model_effort(value: &tasks::ModelEffort, limits: &wire::Limits) -> Result<wire::ModelEffort, wire::Problem> {
    let field_model = value.model;
    let field_effort = value.effort;
    wire::ModelEffort::new(limits, wire::ModelEffortParts { model: field_model, effort: field_effort })
}
fn decode_model_effort(value: wire::ModelEffort, _limits: &wire::Limits) -> tasks::ModelEffort {
    let wire::ModelEffortParts { model, effort } = value.into_parts();
    let field_model = model;
    let field_effort = effort;
    tasks::ModelEffort { model: field_model, effort: field_effort }
}
fn encode_llm_choice(value: &tasks::LlmChoice, limits: &wire::Limits) -> Result<wire::LlmChoice, wire::Problem> {
    let field_account = value.account;
    let field_main = encode_model_effort(&value.main, limits)?;
    let mut list_33 = List::with_capacity(limits.llm_choice_subagents);
    for entry in &value.subagents {
        let element = encode_model_effort(entry, limits)?;
        if list_33.push(element).is_err() {
            return Err(bound());
        }
    }
    let field_subagents = list_33;
    wire::LlmChoice::new(
        limits,
        wire::LlmChoiceParts { account: field_account, main: field_main, subagents: field_subagents },
    )
}
fn decode_llm_choice(value: wire::LlmChoice, limits: &wire::Limits) -> tasks::LlmChoice {
    let wire::LlmChoiceParts { account, main, subagents } = value.into_parts();
    let field_account = account;
    let field_main = decode_model_effort(main, limits);
    let mut list_34 = List::with_capacity((subagents).len());
    for entry in (subagents).into_boxed() {
        let element = decode_model_effort(entry, limits);
        list_34.push(element).expect("decoded list bound");
    }
    let field_subagents = list_34.into_boxed();
    tasks::LlmChoice { account: field_account, main: field_main, subagents: field_subagents }
}
fn encode_template(value: &core::Template, limits: &wire::Limits) -> Result<wire::Template, wire::Problem> {
    let field_number = value.number;
    let field_name = value.name.clone();
    let field_executor = encode_template_executor(&value.executor, limits)?;
    let field_contract = encode_contract(&value.contract, limits)?;
    let field_shallower = value.shallower;
    let field_budget = encode_template_budget(&value.budget, limits)?;
    let field_deadline = encode_template_deadline(&value.deadline, limits)?;
    let field_tracked = value.tracked;
    let field_flags = encode_flags(&value.flags, limits)?;
    let field_label = value.label;
    let field_grace = value.grace;
    let field_llm = match &value.llm {
        Some(entry) => Some(encode_llm_choice(entry, limits)?),
        None => None,
    };
    wire::Template::new(
        limits,
        wire::TemplateParts {
            number: field_number,
            name: field_name,
            executor: field_executor,
            contract: field_contract,
            shallower: field_shallower,
            budget: field_budget,
            deadline: field_deadline,
            tracked: field_tracked,
            flags: field_flags,
            label: field_label,
            grace: field_grace,
            llm: field_llm,
        },
    )
}
fn decode_template(value: wire::Template, limits: &wire::Limits) -> core::Template {
    let wire::TemplateParts {
        number,
        name,
        executor,
        contract,
        shallower,
        budget,
        deadline,
        tracked,
        flags,
        label,
        grace,
        llm,
    } = value.into_parts();
    let field_number = number;
    let field_name = name;
    let field_executor = decode_template_executor(executor, limits);
    let field_contract = decode_contract(contract, limits);
    let field_shallower = shallower;
    let field_budget = decode_template_budget(budget, limits);
    let field_deadline = decode_template_deadline(deadline, limits);
    let field_tracked = tracked;
    let field_flags = decode_flags(flags, limits);
    let field_label = label;
    let field_grace = grace;
    let field_llm = match llm {
        Some(entry) => Some(decode_llm_choice(entry, limits)),
        None => None,
    };
    core::Template {
        number: field_number,
        name: field_name,
        executor: field_executor,
        contract: field_contract,
        shallower: field_shallower,
        budget: field_budget,
        deadline: field_deadline,
        tracked: field_tracked,
        flags: field_flags,
        label: field_label,
        grace: field_grace,
        llm: field_llm,
    }
}
fn encode_proposal_id(
    value: &(people::Proposer, u32),
    limits: &wire::Limits,
) -> Result<wire::ProposalId, wire::Problem> {
    let field_proposer = encode_proposer(&value.0, limits)?;
    let field_number = value.1;
    wire::ProposalId::new(limits, wire::ProposalIdParts { proposer: field_proposer, number: field_number })
}
fn decode_proposal_id(value: wire::ProposalId, limits: &wire::Limits) -> (people::Proposer, u32) {
    let wire::ProposalIdParts { proposer, number } = value.into_parts();
    let field_proposer = decode_proposer(proposer, limits);
    let field_number = number;
    (field_proposer, field_number)
}
fn encode_head(value: &settings::Head, limits: &wire::Limits) -> Result<wire::Head, wire::Problem> {
    let field_number = value.number;
    let field_against = value.against;
    let field_at = value.at.as_nanos();
    let field_by = encode_decider(&value.by, limits)?;
    let field_proposal = match &value.proposal {
        Some(entry) => Some(encode_proposal_id(entry, limits)?),
        None => None,
    };
    let field_moved = value.moved;
    wire::Head::new(
        limits,
        wire::HeadParts {
            number: field_number,
            against: field_against,
            at: field_at,
            by: field_by,
            proposal: field_proposal,
            moved: field_moved,
        },
    )
}
fn decode_head(value: wire::Head, limits: &wire::Limits) -> settings::Head {
    let wire::HeadParts { number, against, at, by, proposal, moved } = value.into_parts();
    let field_number = number;
    let field_against = against;
    let field_at = Wall::from_nanos(at);
    let field_by = decode_decider(by, limits);
    let field_proposal = match proposal {
        Some(entry) => Some(decode_proposal_id(entry, limits)),
        None => None,
    };
    let field_moved = moved;
    settings::Head {
        number: field_number,
        against: field_against,
        at: field_at,
        by: field_by,
        proposal: field_proposal,
        moved: field_moved,
    }
}
fn encode_item(value: &settings::Item, limits: &wire::Limits) -> Result<wire::Item, wire::Problem> {
    let field_setting = encode_setting(&value.setting, limits)?;
    let field_edit = encode_edit(&value.edit, limits)?;
    wire::Item::new(limits, wire::ItemParts { setting: field_setting, edit: field_edit })
}
fn decode_item(value: wire::Item, limits: &wire::Limits) -> Result<settings::Item, wire::Problem> {
    let wire::ItemParts { setting, edit } = value.into_parts();
    let field_setting = decode_setting(setting, limits);
    let field_edit = decode_edit(edit, limits)?;
    Ok(settings::Item { setting: field_setting, edit: field_edit })
}
fn encode_default(
    value: &settings::Default,
    limits: &wire::Limits,
    names: &[settings::ModelName],
) -> Result<wire::Default, wire::Problem> {
    let field_value = encode_value(&value.value, limits)?;
    let field_revision = value.revision;
    wire::Default::new(
        limits,
        wire::DefaultParts { models: encode_names(names, limits)?, value: field_value, revision: field_revision },
    )
}
fn decode_default(
    value: wire::Default,
    limits: &wire::Limits,
    names: &[settings::ModelName],
) -> Result<settings::Default, wire::Problem> {
    let wire::DefaultParts { models, value, revision } = value.into_parts();
    let field_value = decode_value(value, limits)?;
    let field_revision = revision;
    let mut field_value = field_value;
    remap_value(&mut field_value, &model_moves(&models, names), &models, names);
    Ok(settings::Default { value: field_value, revision: field_revision })
}
fn encode_changed(
    value: &settings::Changed,
    limits: &wire::Limits,
    names: &[settings::ModelName],
) -> Result<wire::Changed, wire::Problem> {
    let field_value = match &value.value {
        Some(entry) => Some(encode_value(entry, limits)?),
        None => None,
    };
    let field_revision = value.revision;
    wire::Changed::new(
        limits,
        wire::ChangedParts { models: encode_names(names, limits)?, value: field_value, revision: field_revision },
    )
}
fn decode_changed(
    value: wire::Changed,
    limits: &wire::Limits,
    names: &[settings::ModelName],
) -> Result<settings::Changed, wire::Problem> {
    let wire::ChangedParts { models, value, revision } = value.into_parts();
    let field_value = match value {
        Some(entry) => Some(decode_value(entry, limits)?),
        None => None,
    };
    let field_revision = revision;
    let mut field_value = field_value;
    if let Some(value) = &mut field_value {
        remap_value(value, &model_moves(&models, names), &models, names);
    }
    Ok(settings::Changed { value: field_value, revision: field_revision })
}
fn encode_revision(
    value: &settings::Revision,
    limits: &wire::Limits,
    names: &[settings::ModelName],
) -> Result<wire::Revision, wire::Problem> {
    let field_head = encode_head(&value.head, limits)?;
    let mut list_35 = List::with_capacity(limits.revision_items);
    for entry in &value.items {
        let element = encode_item(entry, limits)?;
        if list_35.push(element).is_err() {
            return Err(bound());
        }
    }
    let field_items = list_35;
    wire::Revision::new(
        limits,
        wire::RevisionParts { models: encode_names(names, limits)?, head: field_head, items: field_items },
    )
}
fn decode_revision(
    value: wire::Revision,
    limits: &wire::Limits,
    names: &[settings::ModelName],
) -> Result<settings::Revision, wire::Problem> {
    let wire::RevisionParts { models, head, items } = value.into_parts();
    let field_head = decode_head(head, limits);
    let mut list_36 = List::with_capacity((items).len());
    for entry in (items).into_boxed() {
        let element = decode_item(entry, limits)?;
        list_36.push(element).expect("decoded list bound");
    }
    let field_items = list_36.into_boxed();
    let mut field_items = field_items;
    let moved = model_moves(&models, names);
    for item in &mut field_items {
        match &mut item.edit {
            settings::Edit::Set(value) => remap_value(value, &moved, &models, names),
            settings::Edit::Removed | settings::Edit::Reset => {}
        }
    }
    Ok(settings::Revision { head: field_head, items: field_items })
}
fn encode_charter_version(
    value: &settings::CharterVersion,
    limits: &wire::Limits,
) -> Result<wire::CharterVersion, wire::Problem> {
    let field_charter = match &value.charter {
        Some(charter) => Some(encode_charter(charter, limits)?),
        None => None,
    };
    wire::CharterVersion::new(limits, wire::CharterVersionParts { charter: field_charter })
}
fn decode_charter_version(value: wire::CharterVersion, limits: &wire::Limits) -> settings::CharterVersion {
    let wire::CharterVersionParts { charter } = value.into_parts();
    let field_charter = match charter {
        Some(charter) => Some(Box::new(decode_charter(charter, limits))),
        None => None,
    };
    settings::CharterVersion { charter: field_charter }
}
fn encode_proposal(
    value: &settings::Proposal,
    limits: &wire::Limits,
    names: &[settings::ModelName],
) -> Result<wire::Proposal, wire::Problem> {
    let field_against = value.against;
    let mut list_37 = List::with_capacity(limits.proposal_items);
    for entry in &value.items {
        let element = encode_item(entry, limits)?;
        if list_37.push(element).is_err() {
            return Err(bound());
        }
    }
    let field_items = list_37;
    wire::Proposal::new(
        limits,
        wire::ProposalParts { models: encode_names(names, limits)?, against: field_against, items: field_items },
    )
}
fn decode_proposal(
    value: wire::Proposal,
    limits: &wire::Limits,
    names: &[settings::ModelName],
) -> Result<settings::Proposal, wire::Problem> {
    let wire::ProposalParts { models, against, items } = value.into_parts();
    let field_against = against;
    let mut list_38 = List::with_capacity((items).len());
    for entry in (items).into_boxed() {
        let element = decode_item(entry, limits)?;
        list_38.push(element).expect("decoded list bound");
    }
    let field_items = list_38.into_boxed();
    let mut field_items = field_items;
    let moved = model_moves(&models, names);
    for item in &mut field_items {
        match &mut item.edit {
            settings::Edit::Set(value) => remap_value(value, &moved, &models, names),
            settings::Edit::Removed | settings::Edit::Reset => {}
        }
    }
    Ok(settings::Proposal { against: field_against, items: field_items })
}

#[must_use]
pub fn limits(domain: &core::Limits) -> Option<wire::Limits> {
    let mut limits = wire::CEILINGS;
    limits.text_bytes = core::EFFORT_BYTES;
    limits.model_name = core::MODEL_NAME_BYTES;
    limits.model_name_name = core::MODEL_NAME_BYTES;
    limits.model_name_efforts = core::EFFORTS;
    limits.model_name_efforts_item = core::EFFORT_BYTES;
    limits.run_policy_instructions = domain.run_bytes;
    limits.segment_bytes = domain.authority.segment_bytes.max(domain.tasks.hold_bytes);
    limits.last_exact_bytes = domain.authority.segment_bytes;
    limits.last_open_bytes = domain.authority.segment_bytes;
    limits.pattern_segments = domain.authority.segments;
    limits.delegation_kinds = domain.authority.executors;
    limits.authority_grants = domain.authority.grants;
    limits.authority_note_resources = domain.authority.grants;
    limits.rules_implies = domain.authority.implications;
    limits.rules_requirements = domain.authority.requirements;
    limits.policy_projections = domain.authority.grants;
    limits.policy_roles = domain.authority.roles;
    limits.policy_requirements = domain.authority.requirements;
    limits.configured_policy_permissions =
        u32::try_from(domain.policy_bytes.checked_div(u64::try_from(size_of::<people::PermissionRole>()).ok()?)?)
            .ok()?;
    limits.configured_policy_templates = domain.templates;
    limits.field_rule_name = domain.tasks.result_bytes;
    limits.verdict_fields = domain.tasks.result_bytes;
    limits.part_path_segments = domain.tasks.hold_segments;
    limits.approval_state = domain.tasks.result_bytes;
    limits.verdict_choices_choices = domain.tasks.contract_choices;
    limits.template_name = domain.documents.text_bytes;
    limits.revision_items = settings::capacity(domain)?.checked_mul(2)?;
    limits.proposal_items = limits.revision_items;
    if wire::check_limits(&limits).is_err() {
        return None;
    }
    Some(limits)
}

pub fn encode(
    record: &settings::Record,
    limits: &wire::Limits,
    names: &[settings::ModelName],
) -> Result<Box<[u8]>, wire::Problem> {
    match record {
        settings::Record::Default { default, .. } => {
            let value = encode_default(default, limits, names)?;
            let mut writer = Writer::new(usize::try_from(value.measure()).expect("settings row size"));
            value.encode(&mut writer).expect("measured row");
            Ok(writer.finish())
        }
        settings::Record::Changed { changed, .. } => {
            let value = encode_changed(changed, limits, names)?;
            let mut writer = Writer::new(usize::try_from(value.measure()).expect("settings row size"));
            value.encode(&mut writer).expect("measured row");
            Ok(writer.finish())
        }
        settings::Record::Revision { revision, .. } => {
            let value = encode_revision(revision, limits, names)?;
            let mut writer = Writer::new(usize::try_from(value.measure()).expect("settings row size"));
            value.encode(&mut writer).expect("measured row");
            Ok(writer.finish())
        }
        settings::Record::Extent { oldest } => {
            let value = wire::Extent::new(limits, wire::ExtentParts { oldest: *oldest })?;
            let mut writer = Writer::new(usize::try_from(value.measure()).expect("extent size"));
            value.encode(&mut writer).expect("measured row");
            Ok(writer.finish())
        }
        settings::Record::Charter { value, .. } => {
            let value = encode_charter_version(value, limits)?;
            let mut writer = Writer::new(usize::try_from(value.measure()).expect("settings row size"));
            value.encode(&mut writer).expect("measured row");
            Ok(writer.finish())
        }
        settings::Record::Proposal { value, .. } => {
            let value = encode_proposal(value, limits, names)?;
            let mut writer = Writer::new(usize::try_from(value.measure()).expect("settings row size"));
            value.encode(&mut writer).expect("measured row");
            Ok(writer.finish())
        }
    }
}
pub fn decode(
    key: &settings::Key,
    bytes: &[u8],
    limits: &wire::Limits,
    names: &[settings::ModelName],
) -> Result<settings::Record, wire::Problem> {
    let mut reader = Reader::new(bytes);
    Ok(match key {
        settings::Key::Default(setting) => settings::Record::Default {
            setting: *setting,
            default: decode_default(wire::Default::decode(limits, &mut reader)?, limits, names)?,
        },
        settings::Key::Changed(setting) => settings::Record::Changed {
            setting: *setting,
            changed: decode_changed(wire::Changed::decode(limits, &mut reader)?, limits, names)?,
        },
        settings::Key::Held(_) | settings::Key::History(_) => {
            let history = match key {
                settings::Key::History(_) => true,
                settings::Key::Held(_) => false,
                settings::Key::Default(_)
                | settings::Key::Changed(_)
                | settings::Key::Charter { .. }
                | settings::Key::Proposal { .. }
                | settings::Key::Extent => unreachable!("revision key"),
            };
            settings::Record::Revision {
                history,
                revision: decode_revision(wire::Revision::decode(limits, &mut reader)?, limits, names)?,
            }
        }
        settings::Key::Extent => {
            settings::Record::Extent { oldest: wire::Extent::decode(limits, &mut reader)?.into_parts().oldest }
        }
        settings::Key::Charter { charter, revision } => settings::Record::Charter {
            charter: *charter,
            revision: *revision,
            value: decode_charter_version(wire::CharterVersion::decode(limits, &mut reader)?, limits),
        },
        settings::Key::Proposal { proposer, proposal } => settings::Record::Proposal {
            proposer: *proposer,
            proposal: *proposal,
            value: decode_proposal(wire::Proposal::decode(limits, &mut reader)?, limits, names)?,
        },
    })
}

// Like the people family's live policy rows, settings remember the catalogue's
// order. Frozen masks and template choices follow names when that order moves.
fn encode_names(names: &[settings::ModelName], limits: &wire::Limits) -> Result<List<wire::ModelName>, wire::Problem> {
    let mut result = List::with_capacity(jig_domain_tasks::MODELS);
    for name in names {
        if result
            .push(wire::ModelName::new(
                limits,
                wire::ModelNameParts { name: name.name.clone(), efforts: effort_names(&name.efforts)? },
            )?)
            .is_err()
        {
            return Err(bound());
        }
    }
    Ok(result)
}
fn model_moves(kept: &List<wire::ModelName>, names: &[settings::ModelName]) -> [u64; 64] {
    let mut moved = [0; 64];
    for (bit, name) in kept.iter().enumerate() {
        for (today, configured) in names.iter().enumerate() {
            if configured.name.as_ref() == name.name() {
                let today = u32::try_from(today).expect("configured model count");
                *moved.get_mut(bit).expect("bounded stored catalogue") = 1_u64.checked_shl(today).unwrap_or(0);
                break;
            }
        }
    }
    moved
}
fn remap_mask(value: &mut authority::Models, moved: &[u64; 64]) {
    if value.is_all() {
        return;
    }
    let mut bits = 0;
    for (bit, &today) in moved.iter().enumerate() {
        let bit = u32::try_from(bit).expect("model bit");
        if value.bits() & 1_u64.checked_shl(bit).unwrap_or(0) != 0 {
            bits |= today;
        }
    }
    *value = authority::Models::Named(bits);
}
fn remap_model(value: &mut u8, moved: &[u64; 64]) {
    let bit = moved.get(usize::from(*value)).copied().unwrap_or(0);
    *value = if bit == 0 { u8::MAX } else { u8::try_from(bit.trailing_zeros()).expect("model index") };
}
fn effort_names(names: &[Box<[u8]>]) -> Result<List<Box<[u8]>>, wire::Problem> {
    let mut result = List::with_capacity(core::EFFORTS);
    for name in names {
        if result.push(name.clone()).is_err() {
            return Err(bound());
        }
    }
    // ModelName's constructor validates each effort's byte length too.
    Ok(result)
}
fn remap_choice(
    value: &mut tasks::ModelEffort,
    moved: &[u64; 64],
    kept: &List<wire::ModelName>,
    names: &[settings::ModelName],
) {
    let old_model = value.model;
    remap_model(&mut value.model, moved);
    let Some(effort) = value.effort else { return };
    let Some(old) = kept.get(u32::from(old_model)) else {
        value.model = u8::MAX;
        return;
    };
    let Some(name) = old.efforts().get(u32::from(effort)) else {
        value.model = u8::MAX;
        return;
    };
    let Some(today) = names.get(usize::from(value.model)) else { return };
    for (at, configured) in today.efforts.iter().enumerate() {
        if configured == name {
            value.effort = Some(u8::try_from(at).expect("effort count"));
            return;
        }
    }
    value.model = u8::MAX;
}
fn remap_value(
    value: &mut settings::Value,
    moved: &[u64; 64],
    kept: &List<wire::ModelName>,
    names: &[settings::ModelName],
) {
    match value {
        settings::Value::Policy(value) => {
            remap_mask(&mut value.policy.ceiling.models, moved);
            for role in &mut value.policy.roles {
                remap_mask(&mut role.authority.models, moved);
            }
        }
        settings::Value::Rules(value) => remap_mask(&mut value.ceiling.models, moved),
        settings::Value::Template(value) => {
            if let Some(choice) = &mut value.llm {
                remap_choice(&mut choice.main, moved, kept, names);
                for model in &mut choice.subagents {
                    remap_choice(model, moved, kept, names);
                }
            }
        }
        settings::Value::Charter(_) => {}
    }
}
