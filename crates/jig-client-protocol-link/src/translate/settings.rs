#![expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
//! Exhaustive settings and history translations (protocol/client.md, 3).
use crate::Limits;
use alloc::boxed::Box;
use jig_client_domain::contract as c;
use jig_client_protocol_wire::v19 as w;
use skein_lib::{List, Reader, Wall, Writer, bytes};
pub fn wire_setting_number(value: &c::SettingNumber, limits: &w::Limits) -> Result<w::SettingNumber, w::Problem> {
    w::SettingNumber::new(limits, w::SettingNumberParts { number: value.number })
}
pub fn restore_setting_number(value: &w::SettingNumber, _limits: &Limits) -> Result<c::SettingNumber, w::Problem> {
    Ok(c::SettingNumber { number: value.number() })
}
pub fn wire_setting(value: &c::Setting, limits: &w::Limits) -> Result<w::Setting, w::Problem> {
    Ok(match value {
        c::Setting::Charter(item) => w::Setting::Charter(wire_setting_number(item, limits)?),
        c::Setting::Template(item) => w::Setting::Template(wire_setting_number(item, limits)?),
        c::Setting::Policy(item) => w::Setting::Policy(wire_setting_number(item, limits)?),
        c::Setting::Rules => w::Setting::Rules,
    })
}
pub fn restore_setting(value: &w::Setting, limits: &Limits) -> Result<c::Setting, w::Problem> {
    Ok(match value {
        w::Setting::Charter(item) => c::Setting::Charter(restore_setting_number(item, limits)?),
        w::Setting::Template(item) => c::Setting::Template(restore_setting_number(item, limits)?),
        w::Setting::Policy(item) => c::Setting::Policy(restore_setting_number(item, limits)?),
        w::Setting::Rules => c::Setting::Rules,
    })
}
pub fn wire_setting_model(value: &c::SettingModel, limits: &w::Limits) -> Result<w::SettingModel, w::Problem> {
    let mut efforts = List::with_capacity(value.efforts.len());
    for item in &value.efforts {
        efforts.push(bytes::copy_of(item.bytes())).expect("same capacity");
    }
    w::SettingModel::new(
        limits,
        w::SettingModelParts {
            dialect: value.dialect,
            endpoint: value.endpoint,
            program: match &value.program {
                Some(item) => Some(bytes::copy_of(item.bytes())),
                None => None,
            },
            name: bytes::copy_of(value.name.bytes()),
            window: value.window,
            output: value.output,
            input_price: value.input_price,
            cached_price: value.cached_price,
            cache_write_price: match &value.cache_write_price {
                Some(item) => Some(*item),
                None => None,
            },
            output_price: value.output_price,
            price_unit: value.price_unit,
            efforts,
            effort: match &value.effort {
                Some(item) => Some(bytes::copy_of(item.bytes())),
                None => None,
            },
        },
    )
}
pub fn restore_setting_model(value: &w::SettingModel, _limits: &Limits) -> Result<c::SettingModel, w::Problem> {
    let mut efforts = List::with_capacity(value.efforts().len());
    for item in value.efforts() {
        efforts.push(text(item, 32)?).expect("same capacity");
    }
    Ok(c::SettingModel {
        dialect: value.dialect(),
        endpoint: value.endpoint(),
        program: match value.program() {
            Some(item) => Some(text(item, 32)?),
            None => None,
        },
        name: text(value.name(), 4096)?,
        window: value.window(),
        output: value.output(),
        input_price: value.input_price(),
        cached_price: value.cached_price(),
        cache_write_price: match value.cache_write_price() {
            Some(item) => Some(*item),
            None => None,
        },
        output_price: value.output_price(),
        price_unit: value.price_unit(),
        efforts,
        effort: match value.effort() {
            Some(item) => Some(text(item, 32)?),
            None => None,
        },
    })
}
pub fn wire_setting_hosts(value: &c::SettingHosts, _limits: &w::Limits) -> Result<w::SettingHosts, w::Problem> {
    Ok(match value {
        c::SettingHosts::Workers => w::SettingHosts::Workers,
        c::SettingHosts::Engine => w::SettingHosts::Engine,
        c::SettingHosts::Both => w::SettingHosts::Both,
    })
}
pub fn restore_setting_hosts(value: &w::SettingHosts, _limits: &Limits) -> Result<c::SettingHosts, w::Problem> {
    Ok(match value {
        w::SettingHosts::Workers => c::SettingHosts::Workers,
        w::SettingHosts::Engine => c::SettingHosts::Engine,
        w::SettingHosts::Both => c::SettingHosts::Both,
    })
}
pub fn wire_charter(value: &c::Charter, limits: &w::Limits) -> Result<w::Charter, w::Problem> {
    let mut alternatives = List::with_capacity(value.alternatives.len());
    for item in &value.alternatives {
        alternatives.push(wire_setting_model(item, limits)?).expect("same capacity");
    }
    w::Charter::new(
        limits,
        w::CharterParts {
            skein_bools: [value.resume, value.inspect, value.modify, value.shell, value.agents],
            number: value.number,
            instructions: bytes::copy_of(value.instructions.bytes()),
            waiting: value.waiting,
            turns: value.turns,
            time: value.time,
            reserve_turns: value.reserve_turns,
            reserve_time: value.reserve_time,
            model: wire_setting_model(&value.model, limits)?,
            alternatives,
            call_timeout: value.call_timeout,
            hosts: wire_setting_hosts(&value.hosts, limits)?,
            spend: value.spend,
        },
    )
}
pub fn restore_charter(value: &w::Charter, limits: &Limits) -> Result<c::Charter, w::Problem> {
    let mut alternatives = List::with_capacity(value.alternatives().len());
    for item in value.alternatives() {
        alternatives.push(restore_setting_model(item, limits)?).expect("same capacity");
    }
    Ok(c::Charter {
        number: value.number(),
        instructions: text(value.instructions(), 1_114_240)?,
        waiting: value.waiting(),
        resume: value.resume(),
        turns: value.turns(),
        time: value.time(),
        reserve_turns: value.reserve_turns(),
        reserve_time: value.reserve_time(),
        model: restore_setting_model(value.model(), limits)?,
        alternatives,
        inspect: value.inspect(),
        modify: value.modify(),
        shell: value.shell(),
        agents: value.agents(),
        call_timeout: value.call_timeout(),
        hosts: restore_setting_hosts(value.hosts(), limits)?,
        spend: value.spend(),
    })
}
pub fn wire_setting_policy(value: &c::SettingPolicy, limits: &w::Limits) -> Result<w::SettingPolicy, w::Problem> {
    let mut projections = List::with_capacity(value.projections.len());
    for item in &value.projections {
        projections.push(super::asks::wire_owner_grant(item, limits)?).expect("same capacity");
    }
    let mut roles = List::with_capacity(value.roles.len());
    for item in &value.roles {
        roles.push(super::asks::wire_policy_edit_role(item, limits)?).expect("same capacity");
    }
    let mut requirements = List::with_capacity(value.requirements.len());
    for item in &value.requirements {
        requirements.push(super::asks::wire_policy_requirement(item, limits)?).expect("same capacity");
    }
    let mut permissions = List::with_capacity(value.permissions.len());
    for item in &value.permissions {
        permissions.push(super::asks::wire_policy_permission(*item, limits)?).expect("same capacity");
    }
    let mut templates = List::with_capacity(value.templates.len());
    for item in &value.templates {
        templates.push(wire_template_override(item, limits)?).expect("same capacity");
    }
    w::SettingPolicy::new(
        limits,
        w::SettingPolicyParts {
            escalation_role: match &value.escalation_role {
                Some(item) => Some(*item),
                None => None,
            },
            ceiling: super::asks::wire_owner_authority(&value.ceiling, limits)?,
            projections,
            period_spend: value.period_spend,
            roles,
            requirements,
            permissions,
            templates,
        },
    )
}
pub fn restore_setting_policy(value: &w::SettingPolicy, limits: &Limits) -> Result<c::SettingPolicy, w::Problem> {
    let mut projections = List::with_capacity(value.projections().len());
    for item in value.projections() {
        projections.push(super::documents::owner_grant_fields(item, limits)).expect("same capacity");
    }
    let mut roles = List::with_capacity(value.roles().len());
    for item in value.roles() {
        roles.push(restore_policy_edit_role(item, limits)?).expect("same capacity");
    }
    let mut requirements = List::with_capacity(value.requirements().len());
    for item in value.requirements() {
        requirements.push(super::documents::policy_requirement_fields(item, limits)).expect("same capacity");
    }
    let mut permissions = List::with_capacity(value.permissions().len());
    for item in value.permissions() {
        permissions.push(super::documents::policy_permission_fields(item, limits)).expect("same capacity");
    }
    let mut templates = List::with_capacity(value.templates().len());
    for item in value.templates() {
        templates.push(restore_template_override(item)).expect("same capacity");
    }
    Ok(c::SettingPolicy {
        escalation_role: match value.escalation_role() {
            Some(item) => Some(*item),
            None => None,
        },
        ceiling: super::documents::owner_authority_fields(value.ceiling(), limits)?,
        projections,
        period_spend: value.period_spend(),
        roles,
        requirements,
        permissions,
        templates,
    })
}
pub fn wire_rules(value: &c::Rules, limits: &w::Limits) -> Result<w::Rules, w::Problem> {
    let mut implies = List::with_capacity(value.implies.len());
    for item in &value.implies {
        implies
            .push(w::OwnerKindImplication::new(
                limits,
                w::OwnerKindImplicationParts { connector: item.connector, kind: item.kind, implies: item.implies },
            )?)
            .expect("same capacity");
    }
    let mut requirements = List::with_capacity(value.requirements.len());
    for item in &value.requirements {
        requirements.push(super::asks::wire_policy_requirement(item, limits)?).expect("same capacity");
    }
    w::Rules::new(
        limits,
        w::RulesParts {
            ceiling: super::asks::wire_owner_authority(&value.ceiling, limits)?,
            period_spend: value.period_spend,
            minimum_run_spend: value.minimum_run_spend,
            maximum_run_spend: value.maximum_run_spend,
            implies,
            requirements,
        },
    )
}
pub fn restore_rules(value: &w::Rules, limits: &Limits) -> Result<c::Rules, w::Problem> {
    let mut implies = List::with_capacity(value.implies().len());
    for item in value.implies() {
        implies
            .push(c::OwnerKindImplication { connector: item.connector(), kind: item.kind(), implies: item.implies() })
            .expect("same capacity");
    }
    let mut requirements = List::with_capacity(value.requirements().len());
    for item in value.requirements() {
        requirements.push(super::documents::policy_requirement_fields(item, limits)).expect("same capacity");
    }
    Ok(c::Rules {
        ceiling: super::documents::owner_authority_fields(value.ceiling(), limits)?,
        period_spend: value.period_spend(),
        minimum_run_spend: value.minimum_run_spend(),
        maximum_run_spend: value.maximum_run_spend(),
        implies,
        requirements,
    })
}
pub fn wire_setting_value(value: &c::SettingValue, limits: &w::Limits) -> Result<w::SettingValue, w::Problem> {
    Ok(match value {
        c::SettingValue::Charter(item) => w::SettingValue::Charter(Box::new(wire_charter(item, limits)?)),
        c::SettingValue::Template(item) => w::SettingValue::Template(Box::new(wire_template(item, limits)?)),
        c::SettingValue::Policy(item) => w::SettingValue::Policy(Box::new(wire_setting_policy(item, limits)?)),
        c::SettingValue::Rules(item) => w::SettingValue::Rules(Box::new(wire_rules(item, limits)?)),
    })
}
pub fn restore_setting_value(value: &w::SettingValue, limits: &Limits) -> Result<c::SettingValue, w::Problem> {
    Ok(match value {
        w::SettingValue::Charter(item) => c::SettingValue::Charter(Box::new(restore_charter(item, limits)?)),
        w::SettingValue::Template(item) => {
            c::SettingValue::Template(Box::new(super::documents::me::template(item, limits)?))
        }
        w::SettingValue::Policy(item) => c::SettingValue::Policy(Box::new(restore_setting_policy(item, limits)?)),
        w::SettingValue::Rules(item) => c::SettingValue::Rules(Box::new(restore_rules(item, limits)?)),
    })
}
pub fn wire_setting_set(value: &c::SettingSet, limits: &w::Limits) -> Result<w::SettingSet, w::Problem> {
    w::SettingSet::new(limits, w::SettingSetParts { value: wire_setting_value(&value.value, limits)? })
}
pub fn restore_setting_set(value: &w::SettingSet, limits: &Limits) -> Result<c::SettingSet, w::Problem> {
    Ok(c::SettingSet { value: restore_setting_value(value.value(), limits)? })
}
pub fn wire_setting_edit(value: &c::SettingEdit, limits: &w::Limits) -> Result<w::SettingEdit, w::Problem> {
    Ok(match value {
        c::SettingEdit::Set(item) => w::SettingEdit::Set(wire_setting_set(item, limits)?),
        c::SettingEdit::Removed => w::SettingEdit::Removed,
        c::SettingEdit::Reset => w::SettingEdit::Reset,
    })
}
pub fn restore_setting_edit(value: &w::SettingEdit, limits: &Limits) -> Result<c::SettingEdit, w::Problem> {
    Ok(match value {
        w::SettingEdit::Set(item) => c::SettingEdit::Set(restore_setting_set(item, limits)?),
        w::SettingEdit::Removed => c::SettingEdit::Removed,
        w::SettingEdit::Reset => c::SettingEdit::Reset,
    })
}
pub fn wire_setting_item(value: &c::SettingItem, limits: &w::Limits) -> Result<w::SettingItem, w::Problem> {
    w::SettingItem::new(
        limits,
        w::SettingItemParts {
            setting: wire_setting(&value.setting, limits)?,
            edit: wire_setting_edit(&value.edit, limits)?,
        },
    )
}
pub fn restore_setting_item(value: &w::SettingItem, limits: &Limits) -> Result<c::SettingItem, w::Problem> {
    Ok(c::SettingItem {
        setting: restore_setting(value.setting(), limits)?,
        edit: restore_setting_edit(value.edit(), limits)?,
    })
}
pub fn wire_running(value: &c::Running, _limits: &w::Limits) -> Result<w::Running, w::Problem> {
    Ok(match value {
        c::Running::Stays => w::Running::Stays,
        c::Running::Moves => w::Running::Moves,
    })
}
pub fn restore_running(value: &w::Running, _limits: &Limits) -> Result<c::Running, w::Problem> {
    Ok(match value {
        w::Running::Stays => c::Running::Stays,
        w::Running::Moves => c::Running::Moves,
    })
}
pub fn wire_change_settings(value: &c::ChangeSettings, limits: &w::Limits) -> Result<w::ChangeSettings, w::Problem> {
    w::ChangeSettings::new(
        limits,
        w::ChangeSettingsParts {
            against: value.against,
            items: encode_setting_item_pieces(&value.items, limits)?,
            running: wire_running(&value.running, limits)?,
        },
    )
}
pub fn restore_change_settings(value: &w::ChangeSettings, limits: &Limits) -> Result<c::ChangeSettings, w::Problem> {
    Ok(c::ChangeSettings {
        against: value.against(),
        items: decode_setting_item_pieces(value.items(), limits)?,
        running: restore_running(value.running(), limits)?,
    })
}
pub fn wire_outcome_settings_changed(
    value: &c::OutcomeSettingsChanged,
    limits: &w::Limits,
) -> Result<w::OutcomeSettingsChanged, w::Problem> {
    w::OutcomeSettingsChanged::new(limits, w::OutcomeSettingsChangedParts { revision: value.revision })
}
pub fn restore_outcome_settings_changed(
    value: &w::OutcomeSettingsChanged,
    _limits: &Limits,
) -> Result<c::OutcomeSettingsChanged, w::Problem> {
    Ok(c::OutcomeSettingsChanged { revision: value.revision() })
}
pub fn wire_refusal_stale(value: &c::RefusalStale, limits: &w::Limits) -> Result<w::RefusalStale, w::Problem> {
    w::RefusalStale::new(limits, w::RefusalStaleParts { newest: value.newest })
}
pub fn restore_refusal_stale(value: &w::RefusalStale, _limits: &Limits) -> Result<c::RefusalStale, w::Problem> {
    Ok(c::RefusalStale { newest: value.newest() })
}
pub fn wire_refusal_full(value: &c::RefusalFull, limits: &w::Limits) -> Result<w::RefusalFull, w::Problem> {
    let mut tasks = List::with_capacity(value.tasks.len());
    for item in &value.tasks {
        tasks.push(*item).expect("same capacity");
    }
    w::RefusalFull::new(limits, w::RefusalFullParts { revision: value.revision, tasks, more: value.more })
}
pub fn restore_refusal_full(value: &w::RefusalFull, _limits: &Limits) -> Result<c::RefusalFull, w::Problem> {
    let mut tasks = List::with_capacity(value.tasks().len());
    for item in value.tasks() {
        tasks.push(*item).expect("same capacity");
    }
    Ok(c::RefusalFull { revision: value.revision(), tasks, more: value.more() })
}
pub fn wire_setting_fault(value: &c::SettingFault, _limits: &w::Limits) -> Result<w::SettingFault, w::Problem> {
    Ok(match value {
        c::SettingFault::Shape => w::SettingFault::Shape,
        c::SettingFault::Count => w::SettingFault::Count,
        c::SettingFault::Reference => w::SettingFault::Reference,
        c::SettingFault::Rules => w::SettingFault::Rules,
        c::SettingFault::Budget => w::SettingFault::Budget,
        c::SettingFault::Grace => w::SettingFault::Grace,
        c::SettingFault::Owner => w::SettingFault::Owner,
    })
}
pub fn restore_setting_fault(value: &w::SettingFault, _limits: &Limits) -> Result<c::SettingFault, w::Problem> {
    Ok(match value {
        w::SettingFault::Shape => c::SettingFault::Shape,
        w::SettingFault::Count => c::SettingFault::Count,
        w::SettingFault::Reference => c::SettingFault::Reference,
        w::SettingFault::Rules => c::SettingFault::Rules,
        w::SettingFault::Budget => c::SettingFault::Budget,
        w::SettingFault::Grace => c::SettingFault::Grace,
        w::SettingFault::Owner => c::SettingFault::Owner,
    })
}
pub fn wire_malformed_settings(
    value: &c::MalformedSettings,
    limits: &w::Limits,
) -> Result<w::MalformedSettings, w::Problem> {
    w::MalformedSettings::new(
        limits,
        w::MalformedSettingsParts {
            setting: wire_setting(&value.setting, limits)?,
            fault: wire_setting_fault(&value.fault, limits)?,
        },
    )
}
pub fn restore_malformed_settings(
    value: &w::MalformedSettings,
    limits: &Limits,
) -> Result<c::MalformedSettings, w::Problem> {
    Ok(c::MalformedSettings {
        setting: restore_setting(value.setting(), limits)?,
        fault: restore_setting_fault(value.fault(), limits)?,
    })
}
pub fn wire_read_setting(value: &c::ReadSetting, limits: &w::Limits) -> Result<w::ReadSetting, w::Problem> {
    w::ReadSetting::new(
        limits,
        w::ReadSettingParts { setting: wire_setting(&value.setting, limits)?, revision: value.revision },
    )
}
pub fn restore_read_setting(value: &w::ReadSetting, limits: &Limits) -> Result<c::ReadSetting, w::Problem> {
    Ok(c::ReadSetting { setting: restore_setting(value.setting(), limits)?, revision: value.revision() })
}
pub fn wire_read_revision(value: &c::ReadRevision, limits: &w::Limits) -> Result<w::ReadRevision, w::Problem> {
    w::ReadRevision::new(limits, w::ReadRevisionParts { number: value.number })
}
pub fn restore_read_revision(value: &w::ReadRevision, _limits: &Limits) -> Result<c::ReadRevision, w::Problem> {
    Ok(c::ReadRevision { number: value.number() })
}
pub fn wire_read_revisions(value: &c::ReadRevisions, limits: &w::Limits) -> Result<w::ReadRevisions, w::Problem> {
    w::ReadRevisions::new(
        limits,
        w::ReadRevisionsParts {
            before: match &value.before {
                Some(item) => Some(*item),
                None => None,
            },
        },
    )
}
pub fn restore_read_revisions(value: &w::ReadRevisions, _limits: &Limits) -> Result<c::ReadRevisions, w::Problem> {
    Ok(c::ReadRevisions {
        before: match value.before() {
            Some(item) => Some(*item),
            None => None,
        },
    })
}
pub fn wire_setting_follows(value: &c::SettingFollows, _limits: &w::Limits) -> Result<w::SettingFollows, w::Problem> {
    Ok(match value {
        c::SettingFollows::Default => w::SettingFollows::Default,
        c::SettingFollows::Change => w::SettingFollows::Change,
    })
}
pub fn restore_setting_follows(value: &w::SettingFollows, _limits: &Limits) -> Result<c::SettingFollows, w::Problem> {
    Ok(match value {
        w::SettingFollows::Default => c::SettingFollows::Default,
        w::SettingFollows::Change => c::SettingFollows::Change,
    })
}
pub fn wire_setting_line(value: &c::SettingLine, limits: &w::Limits) -> Result<w::SettingLine, w::Problem> {
    w::SettingLine::new(
        limits,
        w::SettingLineParts {
            setting: wire_setting(&value.setting, limits)?,
            name: match &value.name {
                Some(item) => Some(wire_document_text(item, limits)?),
                None => None,
            },
            follows: wire_setting_follows(&value.follows, limits)?,
            last: value.last,
        },
    )
}
pub fn restore_setting_line(value: &w::SettingLine, limits: &Limits) -> Result<c::SettingLine, w::Problem> {
    Ok(c::SettingLine {
        setting: restore_setting(value.setting(), limits)?,
        name: match value.name() {
            Some(item) => Some(super::facts::document_text(item, limits)?),
            None => None,
        },
        follows: restore_setting_follows(value.follows(), limits)?,
        last: value.last(),
    })
}
pub fn wire_revision_party(value: &c::RevisionParty, limits: &w::Limits) -> Result<w::RevisionParty, w::Problem> {
    w::RevisionParty::new(limits, w::RevisionPartyParts { number: value.number })
}
pub fn restore_revision_party(value: &w::RevisionParty, _limits: &Limits) -> Result<c::RevisionParty, w::Problem> {
    Ok(c::RevisionParty { number: value.number() })
}
pub fn wire_revision_decider(value: &c::RevisionDecider, limits: &w::Limits) -> Result<w::RevisionDecider, w::Problem> {
    Ok(match value {
        c::RevisionDecider::FirstStart => w::RevisionDecider::FirstStart,
        c::RevisionDecider::Release => w::RevisionDecider::Release,
        c::RevisionDecider::Party(item) => w::RevisionDecider::Party(wire_revision_party(item, limits)?),
    })
}
pub fn restore_revision_decider(value: &w::RevisionDecider, limits: &Limits) -> Result<c::RevisionDecider, w::Problem> {
    Ok(match value {
        w::RevisionDecider::FirstStart => c::RevisionDecider::FirstStart,
        w::RevisionDecider::Release => c::RevisionDecider::Release,
        w::RevisionDecider::Party(item) => c::RevisionDecider::Party(restore_revision_party(item, limits)?),
    })
}
pub fn wire_revision_head(value: &c::RevisionHead, limits: &w::Limits) -> Result<w::RevisionHead, w::Problem> {
    w::RevisionHead::new(
        limits,
        w::RevisionHeadParts {
            number: value.number,
            against: value.against,
            at: value.at.as_nanos(),
            by: wire_revision_decider(&value.by, limits)?,
            proposal: match &value.proposal {
                Some(item) => Some(super::asks::wire_proposal_key(item, limits)?),
                None => None,
            },
            moved: value.moved,
        },
    )
}
pub fn restore_revision_head(value: &w::RevisionHead, limits: &Limits) -> Result<c::RevisionHead, w::Problem> {
    Ok(c::RevisionHead {
        number: value.number(),
        against: value.against(),
        at: Wall::from_nanos(value.at()),
        by: restore_revision_decider(value.by(), limits)?,
        proposal: match value.proposal() {
            Some(item) => Some(super::answers::proposal_key(item, limits)),
            None => None,
        },
        moved: value.moved(),
    })
}
pub fn wire_held_revision(value: &c::HeldRevision, limits: &w::Limits) -> Result<w::HeldRevision, w::Problem> {
    w::HeldRevision::new(
        limits,
        w::HeldRevisionParts { head: wire_revision_head(&value.head, limits)?, holders: value.holders },
    )
}
pub fn restore_held_revision(value: &w::HeldRevision, limits: &Limits) -> Result<c::HeldRevision, w::Problem> {
    Ok(c::HeldRevision { head: restore_revision_head(value.head(), limits)?, holders: value.holders() })
}
pub fn wire_settings_proposal(
    value: &c::SettingsProposal,
    limits: &w::Limits,
) -> Result<w::SettingsProposal, w::Problem> {
    w::SettingsProposal::new(
        limits,
        w::SettingsProposalParts {
            proposer: super::asks::wire_proposer(&value.proposer, limits)?,
            number: value.number,
        },
    )
}
pub fn restore_settings_proposal(
    value: &w::SettingsProposal,
    limits: &Limits,
) -> Result<c::SettingsProposal, w::Problem> {
    Ok(c::SettingsProposal { proposer: super::answers::proposer(value.proposer(), limits), number: value.number() })
}
pub fn wire_settings_document(
    value: &c::SettingsDocument,
    limits: &w::Limits,
) -> Result<w::SettingsDocument, w::Problem> {
    let mut settings = List::with_capacity(value.settings.len());
    for item in &value.settings {
        settings.push(wire_setting_line(item, limits)?).expect("same capacity");
    }
    let mut held = List::with_capacity(value.held.len());
    for item in &value.held {
        held.push(wire_held_revision(item, limits)?).expect("same capacity");
    }
    let mut pending = List::with_capacity(value.pending.len());
    for item in &value.pending {
        pending.push(wire_settings_proposal(item, limits)?).expect("same capacity");
    }
    w::SettingsDocument::new(limits, w::SettingsDocumentParts { newest: value.newest, settings, held, pending })
}
pub fn restore_settings_document(
    value: &w::SettingsDocument,
    limits: &Limits,
) -> Result<c::SettingsDocument, w::Problem> {
    let mut settings = List::with_capacity(value.settings().len());
    for item in value.settings() {
        settings.push(restore_setting_line(item, limits)?).expect("same capacity");
    }
    let mut held = List::with_capacity(value.held().len());
    for item in value.held() {
        held.push(restore_held_revision(item, limits)?).expect("same capacity");
    }
    let mut pending = List::with_capacity(value.pending().len());
    for item in value.pending() {
        pending.push(restore_settings_proposal(item, limits)?).expect("same capacity");
    }
    Ok(c::SettingsDocument { newest: value.newest(), settings, held, pending })
}
pub fn wire_setting_document(value: &c::SettingDocument, limits: &w::Limits) -> Result<w::SettingDocument, w::Problem> {
    w::SettingDocument::new(
        limits,
        w::SettingDocumentParts {
            setting: wire_setting(&value.setting, limits)?,
            revision: value.revision,
            value: wire_setting_value(&value.value, limits)?,
        },
    )
}
pub fn restore_setting_document(value: &w::SettingDocument, limits: &Limits) -> Result<c::SettingDocument, w::Problem> {
    Ok(c::SettingDocument {
        setting: restore_setting(value.setting(), limits)?,
        revision: value.revision(),
        value: restore_setting_value(value.value(), limits)?,
    })
}
pub fn wire_size(value: &c::Size, limits: &w::Limits) -> Result<w::Size, w::Problem> {
    w::Size::new(limits, w::SizeParts { bytes: value.bytes })
}
pub fn restore_size(value: &w::Size, _limits: &Limits) -> Result<c::Size, w::Problem> {
    Ok(c::Size { bytes: value.bytes() })
}
pub fn wire_revision_edit(value: &c::RevisionEdit, limits: &w::Limits) -> Result<w::RevisionEdit, w::Problem> {
    Ok(match value {
        c::RevisionEdit::Set(item) => w::RevisionEdit::Set(wire_setting_set(item, limits)?),
        c::RevisionEdit::TooLarge(item) => w::RevisionEdit::TooLarge(wire_size(item, limits)?),
        c::RevisionEdit::Removed => w::RevisionEdit::Removed,
        c::RevisionEdit::Reset => w::RevisionEdit::Reset,
    })
}
pub fn restore_revision_edit(value: &w::RevisionEdit, limits: &Limits) -> Result<c::RevisionEdit, w::Problem> {
    Ok(match value {
        w::RevisionEdit::Set(item) => c::RevisionEdit::Set(restore_setting_set(item, limits)?),
        w::RevisionEdit::TooLarge(item) => c::RevisionEdit::TooLarge(restore_size(item, limits)?),
        w::RevisionEdit::Removed => c::RevisionEdit::Removed,
        w::RevisionEdit::Reset => c::RevisionEdit::Reset,
    })
}
pub fn wire_revision_replaced(
    value: &c::RevisionReplaced,
    limits: &w::Limits,
) -> Result<w::RevisionReplaced, w::Problem> {
    Ok(match value {
        c::RevisionReplaced::Value(v) => w::RevisionReplaced::Value(wire_setting_set(v, limits)?),
        c::RevisionReplaced::TooLarge(v) => w::RevisionReplaced::TooLarge(wire_size(v, limits)?),
        c::RevisionReplaced::Default => w::RevisionReplaced::Default,
        c::RevisionReplaced::Removed => w::RevisionReplaced::Removed,
        c::RevisionReplaced::NotOffered => w::RevisionReplaced::NotOffered,
    })
}
pub fn restore_revision_replaced(
    value: &w::RevisionReplaced,
    limits: &Limits,
) -> Result<c::RevisionReplaced, w::Problem> {
    Ok(match value {
        w::RevisionReplaced::Value(v) => c::RevisionReplaced::Value(restore_setting_set(v, limits)?),
        w::RevisionReplaced::TooLarge(v) => c::RevisionReplaced::TooLarge(restore_size(v, limits)?),
        w::RevisionReplaced::Default => c::RevisionReplaced::Default,
        w::RevisionReplaced::Removed => c::RevisionReplaced::Removed,
        w::RevisionReplaced::NotOffered => c::RevisionReplaced::NotOffered,
    })
}
pub fn wire_revision_item(value: &c::RevisionItem, limits: &w::Limits) -> Result<w::RevisionItem, w::Problem> {
    w::RevisionItem::new(
        limits,
        w::RevisionItemParts {
            setting: wire_setting(&value.setting, limits)?,
            edit: wire_revision_edit(&value.edit, limits)?,
            replaced: wire_revision_replaced(&value.replaced, limits)?,
        },
    )
}
pub fn restore_revision_item(value: &w::RevisionItem, limits: &Limits) -> Result<c::RevisionItem, w::Problem> {
    Ok(c::RevisionItem {
        setting: restore_setting(value.setting(), limits)?,
        edit: restore_revision_edit(value.edit(), limits)?,
        replaced: restore_revision_replaced(value.replaced(), limits)?,
    })
}
pub fn wire_revision_document(
    value: &c::RevisionDocument,
    limits: &w::Limits,
) -> Result<w::RevisionDocument, w::Problem> {
    w::RevisionDocument::new(
        limits,
        w::RevisionDocumentParts {
            head: wire_revision_head(&value.head, limits)?,
            items: encode_revision_item_pieces(&value.items, limits)?,
        },
    )
}
pub fn restore_revision_document(
    value: &w::RevisionDocument,
    limits: &Limits,
) -> Result<c::RevisionDocument, w::Problem> {
    Ok(c::RevisionDocument {
        head: restore_revision_head(value.head(), limits)?,
        items: decode_revision_item_pieces(value.items(), limits)?,
    })
}
pub fn wire_revision_forgotten(
    value: &c::RevisionForgotten,
    limits: &w::Limits,
) -> Result<w::RevisionForgotten, w::Problem> {
    w::RevisionForgotten::new(limits, w::RevisionForgottenParts { oldest: value.oldest })
}
pub fn restore_revision_forgotten(
    value: &w::RevisionForgotten,
    _limits: &Limits,
) -> Result<c::RevisionForgotten, w::Problem> {
    Ok(c::RevisionForgotten { oldest: value.oldest() })
}
pub fn wire_revision_answer(value: &c::RevisionAnswer, limits: &w::Limits) -> Result<w::RevisionAnswer, w::Problem> {
    Ok(match value {
        c::RevisionAnswer::Revision(item) => w::RevisionAnswer::Revision(wire_revision_document(item, limits)?),
        c::RevisionAnswer::Forgotten(item) => w::RevisionAnswer::Forgotten(wire_revision_forgotten(item, limits)?),
        c::RevisionAnswer::Unknown => w::RevisionAnswer::Unknown,
    })
}
pub fn restore_revision_answer(value: &w::RevisionAnswer, limits: &Limits) -> Result<c::RevisionAnswer, w::Problem> {
    Ok(match value {
        w::RevisionAnswer::Revision(item) => c::RevisionAnswer::Revision(restore_revision_document(item, limits)?),
        w::RevisionAnswer::Forgotten(item) => c::RevisionAnswer::Forgotten(restore_revision_forgotten(item, limits)?),
        w::RevisionAnswer::Unknown => c::RevisionAnswer::Unknown,
    })
}
pub fn wire_read_revision_answer(
    value: &c::ReadRevisionAnswer,
    limits: &w::Limits,
) -> Result<w::ReadRevisionAnswer, w::Problem> {
    w::ReadRevisionAnswer::new(
        limits,
        w::ReadRevisionAnswerParts { value: wire_revision_answer(&value.value, limits)? },
    )
}
pub fn restore_read_revision_answer(
    value: &w::ReadRevisionAnswer,
    limits: &Limits,
) -> Result<c::ReadRevisionAnswer, w::Problem> {
    Ok(c::ReadRevisionAnswer { value: restore_revision_answer(value.value(), limits)? })
}
pub fn wire_revision_item_edit(
    value: &c::RevisionItemEdit,
    _limits: &w::Limits,
) -> Result<w::RevisionItemEdit, w::Problem> {
    Ok(match value {
        c::RevisionItemEdit::Set => w::RevisionItemEdit::Set,
        c::RevisionItemEdit::Removed => w::RevisionItemEdit::Removed,
        c::RevisionItemEdit::Reset => w::RevisionItemEdit::Reset,
    })
}
pub fn restore_revision_item_edit(
    value: &w::RevisionItemEdit,
    _limits: &Limits,
) -> Result<c::RevisionItemEdit, w::Problem> {
    Ok(match value {
        w::RevisionItemEdit::Set => c::RevisionItemEdit::Set,
        w::RevisionItemEdit::Removed => c::RevisionItemEdit::Removed,
        w::RevisionItemEdit::Reset => c::RevisionItemEdit::Reset,
    })
}
pub fn wire_revision_item_head(
    value: &c::RevisionItemHead,
    limits: &w::Limits,
) -> Result<w::RevisionItemHead, w::Problem> {
    w::RevisionItemHead::new(
        limits,
        w::RevisionItemHeadParts {
            setting: wire_setting(&value.setting, limits)?,
            edit: wire_revision_item_edit(&value.edit, limits)?,
        },
    )
}
pub fn restore_revision_item_head(
    value: &w::RevisionItemHead,
    limits: &Limits,
) -> Result<c::RevisionItemHead, w::Problem> {
    Ok(c::RevisionItemHead {
        setting: restore_setting(value.setting(), limits)?,
        edit: restore_revision_item_edit(value.edit(), limits)?,
    })
}
pub fn wire_revision_row(value: &c::RevisionRow, limits: &w::Limits) -> Result<w::RevisionRow, w::Problem> {
    let mut items = List::with_capacity(value.items.len());
    for item in &value.items {
        items.push(wire_revision_item_head(item, limits)?).expect("same capacity");
    }
    w::RevisionRow::new(
        limits,
        w::RevisionRowParts { head: wire_revision_head(&value.head, limits)?, items, more: value.more },
    )
}
pub fn restore_revision_row(value: &w::RevisionRow, limits: &Limits) -> Result<c::RevisionRow, w::Problem> {
    let mut items = List::with_capacity(value.items().len());
    for item in value.items() {
        items.push(restore_revision_item_head(item, limits)?).expect("same capacity");
    }
    Ok(c::RevisionRow { head: restore_revision_head(value.head(), limits)?, items, more: value.more() })
}
pub fn wire_revisions_page(value: &c::RevisionsPage, limits: &w::Limits) -> Result<w::RevisionsPage, w::Problem> {
    let mut rows = List::with_capacity(value.rows.len());
    for item in &value.rows {
        rows.push(wire_revision_row(item, limits)?).expect("same capacity");
    }
    w::RevisionsPage::new(
        limits,
        w::RevisionsPageParts {
            rows,
            next: match &value.next {
                Some(item) => Some(*item),
                None => None,
            },
            forgotten: value.forgotten,
        },
    )
}
pub fn restore_revisions_page(value: &w::RevisionsPage, limits: &Limits) -> Result<c::RevisionsPage, w::Problem> {
    let mut rows = List::with_capacity(value.rows().len());
    for item in value.rows() {
        rows.push(restore_revision_row(item, limits)?).expect("same capacity");
    }
    Ok(c::RevisionsPage {
        rows,
        next: match value.next() {
            Some(item) => Some(*item),
            None => None,
        },
        forgotten: value.forgotten(),
    })
}
fn encode_setting_item_pieces(values: &List<c::SettingItem>, limits: &w::Limits) -> Result<Box<[u8]>, w::Problem> {
    if values.len() > 1024 {
        return Err(super::too_large());
    }
    let mut rows = List::with_capacity(values.len());
    let mut size = 0_u32;
    for value in values {
        let row = wire_setting_item(value, limits)?;
        size = size.checked_add(row.measure()).ok_or(super::too_large())?.checked_add(4).ok_or(super::too_large())?;
        if size > limits.change_settings_items {
            return Err(super::too_large());
        }
        rows.push(row).expect("same capacity");
    }
    let mut writer = Writer::new(usize::try_from(size).expect("wire size fits"));
    for row in rows.into_boxed() {
        writer.put(&row.measure().to_be_bytes()).expect("measured");
        row.encode(&mut writer).expect("measured");
    }
    Ok(writer.finish())
}
fn decode_setting_item_pieces(values: &[u8], limits: &Limits) -> Result<List<c::SettingItem>, w::Problem> {
    let count = item_count(values, limits)?;
    let mut reader = Reader::new(values);
    let mut rows = List::with_capacity(count);
    for _ in 0_u32..=1024 {
        let Some(piece) = (match skein_codec::piece::split(&mut reader) {
            Ok(value) => value,
            Err(reason) => return Err(super::piece_problem(reason)),
        }) else {
            return Ok(rows);
        };
        let mut input = Reader::new(&piece);
        let row = w::SettingItem::decode(&limits.wire, &mut input)?;
        if !input.is_empty() {
            return Err(super::too_large());
        }
        if rows.push(restore_setting_item(&row, limits)?).is_err() {
            return Err(super::too_large());
        }
    }
    Err(super::too_large())
}
fn encode_revision_item_pieces(values: &List<c::RevisionItem>, limits: &w::Limits) -> Result<Box<[u8]>, w::Problem> {
    if values.len() > 1024 {
        return Err(super::too_large());
    }
    let mut rows = List::with_capacity(values.len());
    let mut size = 0_u32;
    for value in values {
        let row = wire_revision_item(value, limits)?;
        size = size.checked_add(row.measure()).ok_or(super::too_large())?.checked_add(4).ok_or(super::too_large())?;
        if size > limits.revision_document_items {
            return Err(super::too_large());
        }
        rows.push(row).expect("same capacity");
    }
    let mut writer = Writer::new(usize::try_from(size).expect("wire size fits"));
    for row in rows.into_boxed() {
        writer.put(&row.measure().to_be_bytes()).expect("measured");
        row.encode(&mut writer).expect("measured");
    }
    Ok(writer.finish())
}
fn decode_revision_item_pieces(values: &[u8], limits: &Limits) -> Result<List<c::RevisionItem>, w::Problem> {
    let count = item_count(values, limits)?;
    let mut reader = Reader::new(values);
    let mut rows = List::with_capacity(count);
    for _ in 0_u32..=1024 {
        let Some(piece) = (match skein_codec::piece::split(&mut reader) {
            Ok(value) => value,
            Err(reason) => return Err(super::piece_problem(reason)),
        }) else {
            return Ok(rows);
        };
        let mut input = Reader::new(&piece);
        let row = w::RevisionItem::decode(&limits.wire, &mut input)?;
        if !input.is_empty() {
            return Err(super::too_large());
        }
        if rows.push(restore_revision_item(&row, limits)?).is_err() {
            return Err(super::too_large());
        }
    }
    Err(super::too_large())
}

fn restore_policy_edit_role(value: &w::PolicyEditRole, limits: &Limits) -> Result<c::PolicyEditRole, w::Problem> {
    Ok(c::PolicyEditRole {
        number: value.number(),
        authority: super::documents::owner_authority_fields(value.authority(), limits)?,
        period_spend: value.period_spend(),
        requests: value.requests(),
        decides: value.decides(),
    })
}
fn wire_document_text(value: &c::DocumentText, limits: &w::Limits) -> Result<w::DocumentText, w::Problem> {
    w::DocumentText::new(limits, w::DocumentTextParts { bytes: bytes::copy_of(value.bytes()), cut: value.cut })
}
fn wire_template_override(value: &c::TemplateOverride, limits: &w::Limits) -> Result<w::TemplateOverride, w::Problem> {
    w::TemplateOverride::new(
        limits,
        w::TemplateOverrideParts {
            template: value.template,
            executor: match value.executor {
                Some(value) => Some(super::operations::executor(value, limits)?),
                None => None,
            },
            budget: match value.budget {
                Some(value) => Some(super::operations::budget(value, limits)?),
                None => None,
            },
            deadline: match value.deadline {
                Some(value) => Some(super::operations::deadline(value, limits)?),
                None => None,
            },
            priority: value.priority,
        },
    )
}
fn restore_template_override(value: &w::TemplateOverride) -> c::TemplateOverride {
    c::TemplateOverride {
        template: value.template(),
        executor: match value.executor() {
            Some(v) => Some(super::operations::restore_executor(v)),
            None => None,
        },
        budget: match value.budget() {
            Some(v) => Some(super::operations::restore_budget(v)),
            None => None,
        },
        deadline: match value.deadline() {
            Some(v) => Some(super::operations::restore_deadline(v)),
            None => None,
        },
        priority: *value.priority(),
    }
}
fn wire_template(value: &c::Template, limits: &w::Limits) -> Result<w::Template, w::Problem> {
    w::Template::new(
        limits,
        w::TemplateParts {
            number: value.number,
            name: wire_document_text(&value.name, limits)?,
            executor: super::operations::executor(value.executor, limits)?,
            contract: wire_task_contract(&value.contract, limits)?,
            shallower: value.shallower,
            budget: super::operations::budget(value.budget, limits)?,
            deadline: super::operations::deadline(value.deadline, limits)?,
            tracked: value.tracked,
            flags: w::TaskFlags::new(
                limits,
                w::TaskFlagsParts {
                    ended_by: match value.flags.ended_by {
                        c::EndedBy::Result => w::EndedBy::Result,
                        c::EndedBy::Requester => w::EndedBy::Requester,
                    },
                    conversation: value.flags.conversation,
                    listed: value.flags.listed,
                    retention: match value.flags.retention {
                        c::Retention::Kept => w::Retention::Kept,
                        c::Retention::ForgottenOnceRead => w::Retention::ForgottenOnceRead,
                    },
                },
            )?,
            label: value.label,
            grace: value.grace,
            llm: super::llm::wire_choice(value.llm.as_ref(), limits)?,
        },
    )
}
fn wire_task_contract(
    value: &c::TaskContractDocument,
    limits: &w::Limits,
) -> Result<w::TaskContractDocument, w::Problem> {
    Ok(match value {
        c::TaskContractDocument::Approval { state, part } => {
            w::TaskContractDocument::Approval(w::TaskContractDocumentApproval::new(
                limits,
                w::TaskContractDocumentApprovalParts {
                    state: bytes::copy_of(state),
                    part: wire_owner_part(part, limits)?,
                },
            )?)
        }
        c::TaskContractDocument::Report { words } => w::TaskContractDocument::Report(
            w::TaskContractDocumentReport::new(limits, w::TaskContractDocumentReportParts { words: *words })?,
        ),
        c::TaskContractDocument::Change { connector, kind, words } => {
            w::TaskContractDocument::Change(w::TaskContractDocumentChange::new(
                limits,
                w::TaskContractDocumentChangeParts { connector: *connector, kind: *kind, words: *words },
            )?)
        }
        c::TaskContractDocument::Verdict { choices, cut } => {
            let mut rows = List::with_capacity(choices.len());
            let mut size = 0_u32;
            for choice in choices {
                let mut fields = List::with_capacity(choice.fields.len());
                for field in &choice.fields {
                    fields
                        .push(w::FieldRule::new(
                            limits,
                            w::FieldRuleParts { name: bytes::copy_of(&field.name), max: field.max },
                        )?)
                        .expect("same capacity");
                }
                let row = w::VerdictChoice::new(
                    limits,
                    w::VerdictChoiceParts {
                        code: choice.code,
                        words: choice.words,
                        followups: choice.followups,
                        fields,
                    },
                )?;
                size = size
                    .checked_add(row.measure())
                    .ok_or(super::too_large())?
                    .checked_add(4)
                    .ok_or(super::too_large())?;
                rows.push(row).expect("same capacity");
            }
            if size > limits.task_contract_document_verdict_choices {
                return Err(super::too_large());
            }
            let mut writer = Writer::new(usize::try_from(size).expect("wire size fits"));
            for row in &rows {
                writer.put(&row.measure().to_be_bytes()).expect("measured");
                row.encode(&mut writer).expect("measured");
            }
            w::TaskContractDocument::Verdict(w::TaskContractDocumentVerdict::new(
                limits,
                w::TaskContractDocumentVerdictParts { choices: writer.finish(), cut: *cut },
            )?)
        }
    })
}
fn wire_owner_part(value: &c::OwnerPart, limits: &w::Limits) -> Result<w::OwnerPart, w::Problem> {
    let kind = match value.kind {
        c::OwnerPartKind::Resource => w::OwnerPartKind::Resource,
        c::OwnerPartKind::Pool => w::OwnerPartKind::Pool,
        c::OwnerPartKind::Result => w::OwnerPartKind::Result,
        c::OwnerPartKind::Procedure => w::OwnerPartKind::Procedure,
        c::OwnerPartKind::Effect => w::OwnerPartKind::Effect,
        c::OwnerPartKind::Outbox => w::OwnerPartKind::Outbox,
        c::OwnerPartKind::Hold => w::OwnerPartKind::Hold,
    };
    let key = match &value.key {
        c::OwnerPartKey::Number(value) => {
            w::OwnerPartKey::Number(w::OwnerPartKeyNumber::new(limits, w::OwnerPartKeyNumberParts { value: *value })?)
        }
        c::OwnerPartKey::Path(values) => {
            let mut rows = List::with_capacity(values.len());
            for value in values {
                rows.push(bytes::copy_of(value)).expect("same capacity");
            }
            w::OwnerPartKey::Path(w::OwnerPartKeyPath::new(limits, w::OwnerPartKeyPathParts { value: rows })?)
        }
    };
    w::OwnerPart::new(limits, w::OwnerPartParts { connector: value.connector, kind, key })
}

// Count the bounded pieces before reserving the actual rows; an empty change allocates no rows.
fn item_count(values: &[u8], limits: &Limits) -> Result<u32, w::Problem> {
    if u64::try_from(values.len()).expect("piece bytes fit u64") > limits.client.document_bytes {
        return Err(super::too_large());
    }
    let mut reader = Reader::new(values);
    for count in 0_u32..=1024 {
        let piece = match skein_codec::piece::split(&mut reader) {
            Ok(value) => value,
            Err(reason) => return Err(super::piece_problem(reason)),
        };
        if piece.is_none() {
            return Ok(count);
        }
    }
    Err(super::too_large())
}

fn text(value: &[u8], bound: u32) -> Result<c::Text, w::Problem> {
    match c::Text::new(bytes::copy_of(value), bound) {
        Ok(value) => Ok(value),
        Err(_) => Err(super::too_large()),
    }
}
