//! Settings' whole values and bounded history on client wire 19 (contract.md, 6).
use super::{me, objects, owner, w};
use alloc::boxed::Box;
use jig_domain_authority as a;
use jig_domain_core::{self as core, documents as d, reads, settings as s};
use jig_domain_tasks as t;
use skein_lib::{List, Writer};

mod change;
pub(crate) use change::{change, proposed};

#[must_use]
pub fn setting(value: &w::Setting) -> s::Setting {
    match value {
        w::Setting::Charter(v) => s::Setting::Charter(v.number()),
        w::Setting::Template(v) => s::Setting::Template(v.number()),
        w::Setting::Policy(v) => s::Setting::Policy(v.number()),
        w::Setting::Rules => s::Setting::Rules,
    }
}
pub fn name(value: s::Setting, limits: &w::Limits) -> Result<w::Setting, w::Problem> {
    Ok(match value {
        s::Setting::Charter(number) => {
            w::Setting::Charter(w::SettingNumber::new(limits, w::SettingNumberParts { number })?)
        }
        s::Setting::Template(number) => {
            w::Setting::Template(w::SettingNumber::new(limits, w::SettingNumberParts { number })?)
        }
        s::Setting::Policy(number) => {
            w::Setting::Policy(w::SettingNumber::new(limits, w::SettingNumberParts { number })?)
        }
        s::Setting::Rules => w::Setting::Rules,
    })
}
pub fn head(value: &s::Head, limits: &w::Limits) -> Result<w::RevisionHead, w::Problem> {
    w::RevisionHead::new(
        limits,
        w::RevisionHeadParts {
            number: value.number,
            against: value.against,
            at: value.at.as_nanos(),
            by: match value.by {
                s::Decider::FirstStart => w::RevisionDecider::FirstStart,
                s::Decider::Release => w::RevisionDecider::Release,
                s::Decider::Party(number) => {
                    w::RevisionDecider::Party(w::RevisionParty::new(limits, w::RevisionPartyParts { number })?)
                }
            },
            proposal: match value.proposal {
                Some((proposer, number)) => {
                    Some(super::task::proposal(&d::ProposalRef { proposer, number: u64::from(number) }, limits)?)
                }
                None => None,
            },
            moved: value.moved,
        },
    )
}
pub fn document(value: &d::Settings, limits: &w::Limits) -> Result<w::SettingsDocument, w::Problem> {
    let mut settings = List::with_capacity(value.settings.len());
    for line in &value.settings {
        settings
            .push(w::SettingLine::new(
                limits,
                w::SettingLineParts {
                    setting: name(line.setting, limits)?,
                    name: match &line.name {
                        Some(value) => Some(owner::text(value, limits)?),
                        None => None,
                    },
                    follows: match line.follows {
                        d::Follows::Default => w::SettingFollows::Default,
                        d::Follows::Change => w::SettingFollows::Change,
                    },
                    last: line.last,
                },
            )?)
            .expect("settings line count");
    }
    let mut held = List::with_capacity(value.held.len());
    for row in &value.held {
        held.push(w::HeldRevision::new(
            limits,
            w::HeldRevisionParts { head: head(&row.head, limits)?, holders: row.holders },
        )?)
        .expect("held count");
    }
    let mut pending = List::with_capacity(value.pending.len());
    for row in &value.pending {
        let proposer =
            super::task::proposal(&d::ProposalRef { proposer: row.proposer, number: u64::from(row.number) }, limits)?
                .into_parts()
                .proposer;
        pending
            .push(w::SettingsProposal::new(limits, w::SettingsProposalParts { proposer, number: row.number })?)
            .expect("proposal count");
    }
    w::SettingsDocument::new(limits, w::SettingsDocumentParts { newest: value.newest, settings, held, pending })
}
fn model(value: &core::Model, limits: &w::Limits) -> Result<w::SettingModel, w::Problem> {
    let mut efforts = List::with_capacity(super::count(value.efforts.len())?);
    for effort in &value.efforts {
        efforts.push(effort.clone()).expect("effort count");
    }
    w::SettingModel::new(
        limits,
        w::SettingModelParts {
            dialect: value.dialect,
            endpoint: value.endpoint,
            program: value.program.clone(),
            name: value.name.clone(),
            window: value.window,
            output: value.output,
            input_price: value.input_price,
            cached_price: value.cached_price,
            cache_write_price: value.cache_write_price,
            output_price: value.output_price,
            price_unit: value.price_unit,
            efforts,
            effort: value.effort.clone(),
        },
    )
}
fn charter(value: &core::Charter, limits: &w::Limits) -> Result<w::Charter, w::Problem> {
    let policy = &value.policy;
    let mut alternatives = List::with_capacity(super::count(policy.alternatives.len())?);
    for row in &policy.alternatives {
        alternatives.push(model(row, limits)?).expect("model count");
    }
    w::Charter::new(
        limits,
        w::CharterParts {
            skein_bools: [policy.resume, policy.inspect, policy.modify, policy.shell, policy.agents],
            number: value.number,
            instructions: policy.instructions.clone(),
            waiting: policy.waiting,
            turns: policy.turns,
            time: policy.time,
            reserve_turns: policy.reserve_turns,
            reserve_time: policy.reserve_time,
            model: model(&policy.model, limits)?,
            alternatives,
            call_timeout: policy.call_timeout,
            hosts: match value.hosts {
                jig_domain_fleet::Kinds::Workers => w::SettingHosts::Workers,
                jig_domain_fleet::Kinds::Engine => w::SettingHosts::Engine,
                jig_domain_fleet::Kinds::Both => w::SettingHosts::Both,
            },
            spend: value.spend,
        },
    )
}
fn authority(
    value: &a::Authority,
    models: &[crate::Model],
    limits: &w::Limits,
) -> Result<w::OwnerAuthority, w::Problem> {
    objects::goal::authority(models, &core::task_authority(value), limits)
}
fn pattern(value: &a::Pattern, limits: &w::Limits) -> Result<w::OwnerPattern, w::Problem> {
    objects::goal::pattern(
        &t::Pattern {
            segments: value.segments.clone(),
            last: match &value.last {
                a::Last::Exact(v) => t::Last::Exact(v.clone()),
                a::Last::Open(v) => t::Last::Open(v.clone()),
            },
        },
        limits,
    )
}
fn grant(value: &a::Grant, limits: &w::Limits) -> Result<w::OwnerGrant, w::Problem> {
    w::OwnerGrant::new(
        limits,
        w::OwnerGrantParts { connector: value.connector, kind: value.kind, pattern: pattern(&value.pattern, limits)? },
    )
}
fn requirements(values: &[a::Requirement], limits: &w::Limits) -> Result<List<w::PolicyRequirement>, w::Problem> {
    let mut rows = List::with_capacity(super::count(values.len())?);
    for v in values {
        rows.push(w::PolicyRequirement::new(
            limits,
            w::PolicyRequirementParts {
                connector: v.connector,
                kind: v.kind,
                pattern: pattern(&v.pattern, limits)?,
                judge: w::PolicyJudge::new(
                    limits,
                    w::PolicyJudgeParts {
                        connector: v.judge.connector,
                        requirement: v.judge.requirement,
                        parameters: v.judge.parameters,
                    },
                )?,
                guard: match v.guard {
                    a::Guard::Guarded => w::PolicyGuard::Guarded,
                    a::Guard::Observed { freshness } => w::PolicyGuard::Observed(w::PolicyGuardObserved::new(
                        limits,
                        w::PolicyGuardObservedParts { freshness },
                    )?),
                },
                must_be_guarded: v.must_be_guarded,
            },
        )?)
        .expect("requirement count");
    }
    Ok(rows)
}
fn policy(
    value: &core::ConfiguredPolicy,
    models: &[crate::Model],
    limits: &w::Limits,
) -> Result<w::SettingPolicy, w::Problem> {
    let p = &value.policy;
    let mut projections = List::with_capacity(super::count(p.projections.len())?);
    for v in &p.projections {
        projections.push(grant(v, limits)?).expect("projection count");
    }
    let mut roles = List::with_capacity(super::count(p.roles.len())?);
    for v in &p.roles {
        roles
            .push(w::PolicyEditRole::new(
                limits,
                w::PolicyEditRoleParts {
                    number: v.number,
                    authority: authority(&v.authority, models, limits)?,
                    period_spend: v.period_spend,
                    requests: v.requests.0,
                    decides: v.decides.0,
                },
            )?)
            .expect("role count");
    }
    let mut permissions = List::with_capacity(super::count(value.permissions.len())?);
    for v in &value.permissions {
        permissions
            .push(w::PolicyPermission::new(
                limits,
                w::PolicyPermissionParts { connector: v.connector, permission: v.permission, role: v.role },
            )?)
            .expect("permission count");
    }
    let mut overrides = List::with_capacity(super::count(value.templates.len())?);
    for v in &value.templates {
        overrides.push(v.clone()).expect("template override count");
    }
    w::SettingPolicy::new(
        limits,
        w::SettingPolicyParts {
            escalation_role: p.escalation_role,
            ceiling: authority(&p.ceiling, models, limits)?,
            projections,
            period_spend: p.period_spend,
            roles,
            requirements: requirements(&p.requirements, limits)?,
            permissions,
            templates: me::overrides(&overrides, limits)?,
        },
    )
}
fn rules(value: &a::Rules, models: &[crate::Model], limits: &w::Limits) -> Result<w::Rules, w::Problem> {
    let mut implies = List::with_capacity(super::count(value.implies.entries().len())?);
    for v in value.implies.entries() {
        implies
            .push(w::OwnerKindImplication::new(
                limits,
                w::OwnerKindImplicationParts { connector: v.connector, kind: v.kind, implies: v.implies },
            )?)
            .expect("implication count");
    }
    w::Rules::new(
        limits,
        w::RulesParts {
            ceiling: authority(&value.ceiling, models, limits)?,
            period_spend: value.period_spend,
            minimum_run_spend: value.minimum_run_spend,
            maximum_run_spend: value.maximum_run_spend,
            implies,
            requirements: requirements(&value.requirements, limits)?,
        },
    )
}
pub fn value(value: &s::Value, models: &[crate::Model], limits: &w::Limits) -> Result<w::SettingValue, w::Problem> {
    Ok(match value {
        s::Value::Charter(v) => w::SettingValue::Charter(Box::new(charter(v, limits)?)),
        s::Value::Template(v) => w::SettingValue::Template(Box::new(me::template(v, models, limits)?)),
        s::Value::Policy(v) => w::SettingValue::Policy(Box::new(policy(v, models, limits)?)),
        s::Value::Rules(v) => w::SettingValue::Rules(Box::new(rules(v, models, limits)?)),
    })
}
pub fn setting_document(
    v: &reads::SettingDocument,
    models: &[crate::Model],
    limits: &w::Limits,
) -> Result<w::SettingDocument, w::Problem> {
    w::SettingDocument::new(
        limits,
        w::SettingDocumentParts {
            setting: name(v.setting, limits)?,
            revision: v.revision,
            value: value(&v.value, models, limits)?,
        },
    )
}
pub fn revision(
    v: &reads::RevisionAnswer,
    models: &[crate::Model],
    limits: &w::Limits,
) -> Result<w::RevisionAnswer, w::Problem> {
    Ok(match v {
        reads::RevisionAnswer::Unknown => w::RevisionAnswer::Unknown,
        reads::RevisionAnswer::Forgotten { oldest } => w::RevisionAnswer::Forgotten(w::RevisionForgotten::new(
            limits,
            w::RevisionForgottenParts { oldest: *oldest },
        )?),
        reads::RevisionAnswer::Revision { revision, value_bytes } => {
            let mut pieces = List::with_capacity(super::count(revision.items.len())?);
            for item in &revision.items {
                let edit = match &item.edit {
                    s::Edit::Set(v) => {
                        let value = value(v, models, limits)?;
                        let bytes = u64::from(value.measure());
                        if bytes > (*value_bytes).min(u64::from(limits.owned_record)) {
                            w::RevisionEdit::TooLarge(w::Size::new(limits, w::SizeParts { bytes })?)
                        } else {
                            w::RevisionEdit::Set(w::SettingSet::new(limits, w::SettingSetParts { value })?)
                        }
                    }
                    s::Edit::Removed => w::RevisionEdit::Removed,
                    s::Edit::Reset => w::RevisionEdit::Reset,
                };
                let replaced = match &item.replaced {
                    s::Replaced::Value(v) => {
                        let value = value(v, models, limits)?;
                        let bytes = u64::from(value.measure());
                        if bytes > (*value_bytes).min(u64::from(limits.owned_record)) {
                            w::RevisionReplaced::TooLarge(w::Size::new(limits, w::SizeParts { bytes })?)
                        } else {
                            w::RevisionReplaced::Value(w::SettingSet::new(limits, w::SettingSetParts { value })?)
                        }
                    }
                    s::Replaced::Default => w::RevisionReplaced::Default,
                    s::Replaced::Removed => w::RevisionReplaced::Removed,
                    s::Replaced::NotOffered => w::RevisionReplaced::NotOffered,
                };
                let item = w::RevisionItem::new(
                    limits,
                    w::RevisionItemParts { setting: name(item.setting, limits)?, edit, replaced },
                )?;
                let mut out = Writer::new(usize::try_from(item.measure()).expect("measured item"));
                item.encode(&mut out).expect("measured item");
                pieces.push(out.finish()).expect("item count");
            }
            w::RevisionAnswer::Revision(w::RevisionDocument::new(
                limits,
                w::RevisionDocumentParts {
                    head: head(&revision.head, limits)?,
                    items: super::pieces(&pieces, limits.revision_document_items, w::Path::RevisionDocumentItems)?,
                },
            )?)
        }
    })
}
pub fn revisions(v: &reads::RevisionsPage, limits: &w::Limits) -> Result<w::RevisionsPage, w::Problem> {
    let mut rows = List::with_capacity(v.rows.len());
    for row in &v.rows {
        let mut items = List::with_capacity(row.items.len());
        for item in &row.items {
            items
                .push(w::RevisionItemHead::new(
                    limits,
                    w::RevisionItemHeadParts {
                        setting: name(item.setting, limits)?,
                        edit: match item.edit {
                            reads::RevisionEdit::Set => w::RevisionItemEdit::Set,
                            reads::RevisionEdit::Removed => w::RevisionItemEdit::Removed,
                            reads::RevisionEdit::Reset => w::RevisionItemEdit::Reset,
                        },
                    },
                )?)
                .expect("preview count");
        }
        rows.push(w::RevisionRow::new(
            limits,
            w::RevisionRowParts { head: head(&row.head, limits)?, items, more: row.more },
        )?)
        .expect("page count");
    }
    w::RevisionsPage::new(limits, w::RevisionsPageParts { rows, next: v.next, forgotten: v.forgotten })
}

pub(crate) fn item_heads(
    items: &List<reads::RevisionItemHead>,
    limits: &w::Limits,
) -> Result<List<w::RevisionItemHead>, w::Problem> {
    let mut result = List::with_capacity(items.len());
    for item in items {
        result
            .push(w::RevisionItemHead::new(
                limits,
                w::RevisionItemHeadParts { setting: name(item.setting, limits)?, edit: item_edit(item.edit) },
            )?)
            .expect("bounded heads");
    }
    Ok(result)
}
fn item_edit(edit: reads::RevisionEdit) -> w::RevisionItemEdit {
    match edit {
        reads::RevisionEdit::Set => w::RevisionItemEdit::Set,
        reads::RevisionEdit::Removed => w::RevisionItemEdit::Removed,
        reads::RevisionEdit::Reset => w::RevisionItemEdit::Reset,
    }
}
fn review_value(
    v: Option<&s::Value>,
    bound: u64,
    models: &[crate::Model],
    limits: &w::Limits,
) -> Result<w::ReviewValue, w::Problem> {
    Ok(match v {
        None => w::ReviewValue::None,
        Some(v) => {
            let value = value(v, models, limits)?;
            let bytes = u64::from(value.measure());
            if bytes > bound.min(u64::from(limits.owned_record)) {
                w::ReviewValue::TooLarge(w::Size::new(limits, w::SizeParts { bytes })?)
            } else {
                w::ReviewValue::Value(w::SettingSet::new(limits, w::SettingSetParts { value })?)
            }
        }
    })
}
pub(crate) fn review(
    v: &d::SettingsReview,
    models: &[crate::Model],
    limits: &w::Limits,
) -> Result<w::SettingsReview, w::Problem> {
    let mut pieces = List::with_capacity(v.items.len());
    let mut used = 0_u64;
    let mut cut = false;
    for item in &v.items {
        let row = w::ReviewItem::new(
            limits,
            w::ReviewItemParts {
                setting: name(item.setting, limits)?,
                edit: item_edit(item.edit),
                now: review_value(item.now.as_ref(), v.value_bytes, models, limits)?,
                origin: match item.origin {
                    Some((changed, revision)) => {
                        let revision = w::OriginRevision::new(limits, w::OriginRevisionParts { revision })?;
                        if changed { w::SettingOrigin::Change(revision) } else { w::SettingOrigin::Default(revision) }
                    }
                    None => w::SettingOrigin::NotOffered,
                },
                would_be: review_value(item.would_be.as_ref(), v.value_bytes, models, limits)?,
            },
        )?;
        used = used
            .checked_add(u64::from(row.measure()))
            .expect("bounded review bytes")
            .checked_add(4)
            .expect("frame bytes");
        if used > u64::from(limits.settings_review_items) {
            cut = true;
            break;
        }
        let mut bytes = Writer::new(usize::try_from(row.measure()).expect("measured review item"));
        row.encode(&mut bytes).expect("measured review item");
        pieces.push(bytes.finish()).expect("review count");
    }
    w::SettingsReview::new(
        limits,
        w::SettingsReviewParts {
            newest: v.newest,
            items: super::pieces(&pieces, limits.settings_review_items, w::Path::SettingsReviewItems)?,
            cut,
        },
    )
}
