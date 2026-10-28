#![expect(clippy::manual_map, reason = "step code matches optional owner facts without closures")]
//! Whole owner facts encoded without clipping (protocol/client.md, 3).
use super::{pieces, problem, w};
use alloc::boxed::Box;
use jig_domain_core::documents as d;
use jig_domain_tasks as t;
use skein_lib::{List, bytes};
pub fn text(value: &d::Text, limits: &w::Limits) -> Result<w::DocumentText, w::Problem> {
    w::DocumentText::new(limits, w::DocumentTextParts { bytes: bytes::copy_of(&value.bytes), cut: value.cut })
}
pub fn held_role(value: jig_domain_people::Role, limits: &w::Limits) -> Result<w::HeldRole, w::Problem> {
    Ok(match value {
        jig_domain_people::Role::Owner => w::HeldRole::Owner,
        jig_domain_people::Role::Maintainer => w::HeldRole::Maintainer,
        jig_domain_people::Role::Member => w::HeldRole::Member,
        jig_domain_people::Role::Observer => w::HeldRole::Observer,
        jig_domain_people::Role::Policy { role } => {
            w::HeldRole::Policy(w::HeldRolePolicy::new(limits, w::HeldRolePolicyParts { role })?)
        }
    })
}
pub fn funder(value: t::Funder, limits: &w::Limits) -> Result<w::Funder, w::Problem> {
    Ok(match value {
        t::Funder::Task(number) => w::Funder::Task(w::FunderTask::new(limits, w::FunderTaskParts { value: number })?),
        t::Funder::Pool { project, person, period } => {
            w::Funder::Pool(w::FunderPool::new(limits, w::FunderPoolParts { project, person, period })?)
        }
        t::Funder::Recurring { project, task, period } => {
            w::Funder::Recurring(w::FunderRecurring::new(limits, w::FunderRecurringParts { project, task, period })?)
        }
        t::Funder::Period { project, period } => {
            w::Funder::Period(w::FunderPeriod::new(limits, w::FunderPeriodParts { project, period })?)
        }
    })
}
pub fn numbers(value: t::Numbers, limits: &w::Limits) -> Result<w::OwnerNumbers, w::Problem> {
    w::OwnerNumbers::new(
        limits,
        w::OwnerNumbersParts {
            budget: value.budget,
            spent: value.spent,
            spent_below: value.spent_below,
            reserved: value.reserved,
        },
    )
}
pub fn budget(value: &d::BudgetLine, limits: &w::Limits) -> Result<w::OwnerBudget, w::Problem> {
    w::OwnerBudget::new(
        limits,
        w::OwnerBudgetParts {
            numbers: numbers(value.numbers, limits)?,
            left: value.left,
            funder: funder(value.funder, limits)?,
            deadline: match value.deadline {
                Some(item) => Some(item.as_nanos()),
                None => None,
            },
        },
    )
}
pub fn ledger(value: &d::Ledger, limits: &w::Limits) -> Result<w::PoolLedger, w::Problem> {
    w::PoolLedger::new(
        limits,
        w::PoolLedgerParts {
            source: funder(value.source, limits)?,
            parent: match value.parent {
                Some(item) => Some(funder(item, limits)?),
                None => None,
            },
            numbers: numbers(value.numbers, limits)?,
            left: value.left,
            closed: value.closed,
            began: match value.began {
                Some(item) => Some(item.as_nanos()),
                None => None,
            },
            next: match value.next {
                Some(item) => Some(item.as_nanos()),
                None => None,
            },
        },
    )
}
pub fn executor(value: t::Executor, limits: &w::Limits) -> Result<w::OwnerExecutor, w::Problem> {
    Ok(match value {
        t::Executor::Agent { charter } => {
            w::OwnerExecutor::Agent(w::OwnerExecutorAgent::new(limits, w::OwnerExecutorAgentParts { charter })?)
        }
        t::Executor::Procedure { connector, code } => w::OwnerExecutor::Procedure(w::OwnerExecutorProcedure::new(
            limits,
            w::OwnerExecutorProcedureParts { connector, code },
        )?),
        t::Executor::Person(address) => w::OwnerExecutor::Person(w::OwnerExecutorPerson::new(
            limits,
            w::OwnerExecutorPersonParts { value: super::chips::address(address, limits)? },
        )?),
    })
}
pub fn requester(value: t::Party, limits: &w::Limits) -> Result<w::OwnerRequester, w::Problem> {
    Ok(match value {
        t::Party::Task(number) => {
            w::OwnerRequester::Task(w::OwnerRequesterTask::new(limits, w::OwnerRequesterTaskParts { value: number })?)
        }
        t::Party::Person(number) => w::OwnerRequester::Person(w::OwnerRequesterPerson::new(
            limits,
            w::OwnerRequesterPersonParts { value: number },
        )?),
        t::Party::Deployment { project } => w::OwnerRequester::Deployment(w::OwnerRequesterDeployment::new(
            limits,
            w::OwnerRequesterDeploymentParts { project },
        )?),
    })
}
pub fn wake_rule(value: t::WakeRule, limits: &w::Limits) -> Result<w::WakeRule, w::Problem> {
    Ok(match value {
        t::WakeRule::Never => w::WakeRule::Never,
        t::WakeRule::Immediate => w::WakeRule::Immediate,
        t::WakeRule::Batch { count, age } => {
            w::WakeRule::Batch(w::WakeRuleBatch::new(limits, w::WakeRuleBatchParts { count, age })?)
        }
    })
}
pub fn wake(value: t::WakePolicy, limits: &w::Limits) -> Result<w::OwnerWakePolicy, w::Problem> {
    w::OwnerWakePolicy::new(
        limits,
        w::OwnerWakePolicyParts {
            words: wake_rule(value.words, limits)?,
            notices: wake_rule(value.notices, limits)?,
            news: wake_rule(value.news, limits)?,
            results: match value.results {
                t::ResultsWake::Never => w::ResultsWake::Never,
                t::ResultsWake::Each => w::ResultsWake::Each,
                t::ResultsWake::LastOrFailure => w::ResultsWake::LastOrFailure,
            },
            questions: value.questions,
            answers: value.answers,
            timers: value.timers,
        },
    )
}
fn project(
    value: &d::FrameProjectRole,
    models: &[crate::Model],
    limits: &w::Limits,
) -> Result<w::MeProject, w::Problem> {
    w::MeProject::new(
        limits,
        w::MeProjectParts {
            project: value.project,
            name: text(&value.name, limits)?,
            role: held_role(value.role, limits)?,
            pool: match &value.pool {
                Some(item) => Some(ledger(item, limits)?),
                None => None,
            },
            templates: super::me::templates(&value.templates, models, limits)?,
            creating: match &value.creating {
                Some(value) => Some(super::me::creating(models, value, limits)?),
                None => None,
            },
            models: super::llm::models(value.models, models, limits)?,
        },
    )
}
pub fn frame(value: &d::Frame, models: &[crate::Model], limits: &w::Limits) -> Result<w::Me, w::Problem> {
    let mut projects = List::with_capacity(value.projects.len());
    for item in &value.projects {
        projects.push(project(item, models, limits)?).expect("supplied projects");
    }
    let updates = match value.updates {
        d::Count::Known(value) => w::Count::new(limits, w::CountParts { value, limited: false })?,
        d::Count::AtLeast(value) => w::Count::new(limits, w::CountParts { value, limited: true })?,
    };
    w::Me::new(
        limits,
        w::MeParts {
            providers: super::catalogue::offers(&value.providers, limits.me_providers, limits)?,
            providers_cut: value.providers_cut,
            key_bytes: value.key_bytes,
            redirect_bytes: value.redirect_bytes,
            words_bytes: value.words_bytes,
            amendment_bytes: value.amendment_bytes,
            party: w::MeParty::new(
                limits,
                w::MePartyParts {
                    number: value.party,
                    kind: match value.kind {
                        jig_domain_people::Kind::Person => w::PartyKind::Person,
                        jig_domain_people::Kind::Service => w::PartyKind::Service,
                    },
                    login: text(&value.login, limits)?,
                    name: text(&value.name, limits)?,
                },
            )?,
            projects: me_projects_pieces(&projects, limits.me_projects, w::Path::MeProjects)?,
            needs: w::Needs::new(
                limits,
                w::NeedsParts {
                    questions: value.need_counts.questions,
                    proposals: value.need_counts.proposals,
                    escalations: value.need_counts.escalations,
                    person_tasks: value.need_counts.person_tasks,
                },
            )?,
            needs_total: value.needs,
            updates,
            results_seen: value.results_seen,
            cut: value.cut,
            retention: value.retention,
            deployment_role: match value.deployment_role {
                Some(item) => Some(held_role(item, limits)?),
                None => None,
            },
        },
    )
}

pub fn part_kind(value: t::PartKind, _limits: &w::Limits) -> Result<w::OwnerPartKind, w::Problem> {
    Ok(match value {
        t::PartKind::Resource => w::OwnerPartKind::Resource,
        t::PartKind::Pool => w::OwnerPartKind::Pool,
        t::PartKind::Result => w::OwnerPartKind::Result,
        t::PartKind::Procedure => w::OwnerPartKind::Procedure,
        t::PartKind::Effect => w::OwnerPartKind::Effect,
        t::PartKind::Outbox => w::OwnerPartKind::Outbox,
        t::PartKind::Hold => w::OwnerPartKind::Hold,
    })
}
pub fn part(value: &t::Part, limits: &w::Limits) -> Result<w::OwnerPart, w::Problem> {
    let key = match &value.key {
        t::PartKey::Number(number) => {
            w::OwnerPartKey::Number(w::OwnerPartKeyNumber::new(limits, w::OwnerPartKeyNumberParts { value: *number })?)
        }
        t::PartKey::Path(value) => {
            w::OwnerPartKey::Path(w::OwnerPartKeyPath::new(limits, w::OwnerPartKeyPathParts { value: value.clone() })?)
        }
    };
    w::OwnerPart::new(
        limits,
        w::OwnerPartParts { connector: value.connector, kind: part_kind(value.kind, limits)?, key },
    )
}
pub fn pattern(
    segments: &List<Box<[u8]>>,
    last: &d::PatternEnd,
    limits: &w::Limits,
) -> Result<w::OwnerPattern, w::Problem> {
    let last = match last {
        d::PatternEnd::Exact(value) => w::OwnerPatternEnd::Exact(w::OwnerPatternEndExact::new(
            limits,
            w::OwnerPatternEndExactParts { value: bytes::copy_of(value) },
        )?),
        d::PatternEnd::Open(value) => w::OwnerPatternEnd::Open(w::OwnerPatternEndOpen::new(
            limits,
            w::OwnerPatternEndOpenParts { value: bytes::copy_of(value) },
        )?),
    };
    w::OwnerPattern::new(limits, w::OwnerPatternParts { segments: segments.clone(), last })
}
pub fn grant(value: &d::GrantLine, limits: &w::Limits) -> Result<w::OwnerGrant, w::Problem> {
    w::OwnerGrant::new(
        limits,
        w::OwnerGrantParts {
            connector: value.connector,
            kind: value.kind,
            pattern: pattern(&value.segments, &value.last, limits)?,
        },
    )
}
pub fn authority(
    models: &[crate::Model],
    value: &d::AuthorityLine,
    limits: &w::Limits,
) -> Result<w::OwnerAuthorityDocument, w::Problem> {
    let mut grants = List::with_capacity(value.grants.len());
    for item in &value.grants {
        grants.push(grant(item, limits)?).expect("supplied grants");
    }
    let grants = grants_pieces(
        &grants,
        limits.owner_authority_grants.min(w::CEILINGS.owner_authority_grants),
        w::Path::OwnerAuthorityGrants,
    )?;
    let mut scopes = List::with_capacity(value.note_resources.len());
    for item in &value.note_resources {
        scopes
            .push(w::OwnerResourceScope::new(
                limits,
                w::OwnerResourceScopeParts {
                    connector: item.connector,
                    pattern: pattern(&item.segments, &item.last, limits)?,
                },
            )?)
            .expect("supplied scopes");
    }
    let note_resources = scopes_pieces(
        &scopes,
        limits.owner_authority_note_resources.min(w::CEILINGS.owner_authority_note_resources),
        w::Path::OwnerAuthorityNoteResources,
    )?;
    let mut kinds = List::with_capacity(value.executors.capacity());
    for item in &value.executors {
        kinds.push(authority_executor(*item, limits)?).expect("supplied executors");
    }
    w::OwnerAuthorityDocument::new(
        limits,
        w::OwnerAuthorityDocumentParts {
            cut: false,
            authority: w::OwnerAuthority::new(
                limits,
                w::OwnerAuthorityParts {
                    models: super::llm::authority(
                        jig_domain_authority::Models::from_parts(value.models.bits(), value.models.is_all()),
                        models,
                        limits,
                    )?,
                    tools: value.tools.0,
                    grants,
                    delegation: w::OwnerDelegation::new(
                        limits,
                        w::OwnerDelegationParts { kinds, tasks: value.tasks, depth: value.depth },
                    )?,
                    spend: value.ceiling.spend,
                    deadline: match value.ceiling.deadline {
                        Some(item) => Some(item.as_nanos()),
                        None => None,
                    },
                    notes: value.notes.0,
                    note_resources,
                },
            )?,
        },
    )
}

pub fn authority_executor(
    value: t::AuthorityExecutor,
    limits: &w::Limits,
) -> Result<w::OwnerAuthorityExecutor, w::Problem> {
    Ok(match value {
        t::AuthorityExecutor::Charter(number) => w::OwnerAuthorityExecutor::Charter(
            w::OwnerAuthorityExecutorCharter::new(limits, w::OwnerAuthorityExecutorCharterParts { value: number })?,
        ),
        t::AuthorityExecutor::Procedure(number) => w::OwnerAuthorityExecutor::Procedure(
            w::OwnerAuthorityExecutorProcedure::new(limits, w::OwnerAuthorityExecutorProcedureParts { value: number })?,
        ),
        t::AuthorityExecutor::Role(number) => w::OwnerAuthorityExecutor::Role(w::OwnerAuthorityExecutorRole::new(
            limits,
            w::OwnerAuthorityExecutorRoleParts { value: number },
        )?),
    })
}
pub fn grants_pieces(rows: &List<w::OwnerGrant>, bound: u32, path: w::Path) -> Result<Box<[u8]>, w::Problem> {
    let mut encoded = List::with_capacity(rows.len());
    for row in rows {
        let mut out = skein_lib::Writer::new(usize::try_from(row.measure()).expect("u32 record measure fits usize"));
        match row.encode(&mut out) {
            Ok(()) => (),
            Err(_) => return Err(problem(skein_codec::Reason::Bound)),
        }
        encoded.push(out.finish()).expect("supplied piece count");
    }
    pieces(&encoded, bound, path)
}
pub fn scopes_pieces(rows: &List<w::OwnerResourceScope>, bound: u32, path: w::Path) -> Result<Box<[u8]>, w::Problem> {
    let mut encoded = List::with_capacity(rows.len());
    for row in rows {
        let mut out = skein_lib::Writer::new(usize::try_from(row.measure()).expect("u32 record measure fits usize"));
        match row.encode(&mut out) {
            Ok(()) => (),
            Err(_) => return Err(problem(skein_codec::Reason::Bound)),
        }
        encoded.push(out.finish()).expect("supplied piece count");
    }
    pieces(&encoded, bound, path)
}
pub fn specification(value: &d::SpecLine, limits: &w::Limits) -> Result<w::SpecificationDocument, w::Problem> {
    let mut parameters = List::with_capacity(value.parameters.capacity());
    for item in &value.parameters {
        parameters.push(parameter(item, limits)?).expect("supplied parameters");
    }
    w::SpecificationDocument::new(
        limits,
        w::SpecificationDocumentParts {
            specification: w::DocumentSpecification::new(
                limits,
                w::DocumentSpecificationParts {
                    words: text(&value.words, limits)?,
                    parameters,
                    inputs: value.inputs.clone(),
                },
            )?,
            cut: value.cut,
        },
    )
}
fn parameter(value: &d::Parameter, limits: &w::Limits) -> Result<w::ParameterDocument, w::Problem> {
    Ok(match value {
        d::Parameter::Number { name, value } => w::ParameterDocument::Number(w::ParameterDocumentNumber::new(
            limits,
            w::ParameterDocumentNumberParts { name: *name, value: *value },
        )?),
        d::Parameter::Bytes { name, value } => w::ParameterDocument::Bytes(w::ParameterDocumentBytes::new(
            limits,
            w::ParameterDocumentBytesParts { name: *name, value: text(value, limits)? },
        )?),
        d::Parameter::Resource { name, part } => {
            if part.kind != t::PartKind::Resource {
                return Err(problem(skein_codec::Reason::Malformed));
            }
            let resource = match part.key {
                t::PartKey::Number(number) => number,
                t::PartKey::Path(_) => return Err(problem(skein_codec::Reason::Malformed)),
            };
            w::ParameterDocument::Resource(w::ParameterDocumentResource::new(
                limits,
                w::ParameterDocumentResourceParts { name: *name, connector: part.connector, resource },
            )?)
        }
    })
}
pub fn subscriptions(value: &List<t::Subscription>, limits: &w::Limits) -> Result<w::Subscriptions, w::Problem> {
    let mut entries = List::with_capacity(value.capacity());
    for item in value {
        entries.push(subscription(*item, limits)?).expect("supplied subscriptions");
    }
    w::Subscriptions::new(limits, w::SubscriptionsParts { entries })
}

pub fn subscription_kind(value: t::SubscriptionKind, limits: &w::Limits) -> Result<w::SubscriptionKind, w::Problem> {
    Ok(match value {
        t::SubscriptionKind::Task { target, held, result } => w::SubscriptionKind::Task(w::SubscriptionKindTask::new(
            limits,
            w::SubscriptionKindTaskParts { target, held, result },
        )?),
        t::SubscriptionKind::Timer { at, period } => w::SubscriptionKind::Timer(w::SubscriptionKindTimer::new(
            limits,
            w::SubscriptionKindTimerParts { at: at.as_nanos(), period },
        )?),
        t::SubscriptionKind::Topic { connector, topic } => w::SubscriptionKind::Topic(w::SubscriptionKindTopic::new(
            limits,
            w::SubscriptionKindTopicParts { connector, topic },
        )?),
    })
}
pub fn subscription(value: t::Subscription, limits: &w::Limits) -> Result<w::Subscription, w::Problem> {
    w::Subscription::new(
        limits,
        w::SubscriptionParts { number: value.number, kind: subscription_kind(value.kind, limits)? },
    )
}
pub fn contract(value: &d::ContractLine, limits: &w::Limits) -> Result<w::TaskContractDocument, w::Problem> {
    Ok(match value {
        d::ContractLine::Approval { state, part: value } => {
            w::TaskContractDocument::Approval(w::TaskContractDocumentApproval::new(
                limits,
                w::TaskContractDocumentApprovalParts { state: bytes::copy_of(state), part: part(value, limits)? },
            )?)
        }
        d::ContractLine::Report { words } => w::TaskContractDocument::Report(w::TaskContractDocumentReport::new(
            limits,
            w::TaskContractDocumentReportParts { words: *words },
        )?),
        d::ContractLine::Change { connector, kind, words } => {
            w::TaskContractDocument::Change(w::TaskContractDocumentChange::new(
                limits,
                w::TaskContractDocumentChangeParts { connector: *connector, kind: *kind, words: *words },
            )?)
        }
        d::ContractLine::Verdict { choices, cut } => {
            let mut rows = List::with_capacity(choices.len());
            for item in choices {
                let mut fields = List::with_capacity(super::count(item.fields.len())?);
                for field in &item.fields {
                    fields
                        .push(w::FieldRule::new(
                            limits,
                            w::FieldRuleParts { name: bytes::copy_of(&field.name), max: field.max },
                        )?)
                        .expect("supplied field rules");
                }
                rows.push(w::VerdictChoice::new(
                    limits,
                    w::VerdictChoiceParts { code: item.code, words: item.words, followups: item.followups, fields },
                )?)
                .expect("supplied choices");
            }
            w::TaskContractDocument::Verdict(w::TaskContractDocumentVerdict::new(
                limits,
                w::TaskContractDocumentVerdictParts {
                    choices: choices_pieces(
                        &rows,
                        limits
                            .task_contract_document_verdict_choices
                            .min(w::CEILINGS.task_contract_document_verdict_choices),
                        w::Path::TaskContractDocumentVerdictChoices,
                    )?,
                    cut: *cut,
                },
            )?)
        }
    })
}

pub fn choices_pieces(rows: &List<w::VerdictChoice>, bound: u32, path: w::Path) -> Result<Box<[u8]>, w::Problem> {
    let mut encoded = List::with_capacity(rows.len());
    for row in rows {
        let mut out = skein_lib::Writer::new(usize::try_from(row.measure()).expect("u32 record measure fits usize"));
        match row.encode(&mut out) {
            Ok(()) => (),
            Err(_) => return Err(problem(skein_codec::Reason::Bound)),
        }
        encoded.push(out.finish()).expect("supplied piece count");
    }
    pieces(&encoded, bound, path)
}

pub fn me_projects_pieces(rows: &List<w::MeProject>, bound: u32, path: w::Path) -> Result<Box<[u8]>, w::Problem> {
    let mut encoded = List::with_capacity(rows.len());
    for row in rows {
        let mut out = skein_lib::Writer::new(usize::try_from(row.measure()).expect("u32 record measure fits usize"));
        match row.encode(&mut out) {
            Ok(()) => (),
            Err(_) => return Err(problem(skein_codec::Reason::Bound)),
        }
        encoded.push(out.finish()).expect("supplied piece count");
    }
    pieces(&encoded, bound, path)
}
