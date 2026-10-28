//! Whole owner vocabulary shared by document translations (contract.md, 5).
//! This layer copies opaque names, paths, codes and loss facts. It owns no
//! application scope or document policy and never clips a receiving value.
use jig_client::DomainLimits as Limits;
use jig_client_domain::contract as client;
use jig_domain_core::documents;
use jig_domain_tasks as tasks;
use skein_lib::List;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Invalid {
    /// Settings use the real wire world, as their read selectors do.
    Settings,
    TooLarge,
    /// A model or effort index does not name a configured choice.
    Llm,
    /// A provider catalogue or payer observation has inconsistent public facts.
    Catalogue,
    /// A program name, method or configured host name is malformed.
    Program,
    /// The whole offered-act allocation exceeds the receiving offer bound.
    OfferCapacity {
        allocated: u32,
        maximum: u32,
    },
    /// A numbered specification resource was supplied with another part shape.
    ParameterResource,
    /// Coarse and concrete executor facts supplied by the same owner disagree.
    Executor,
}

pub(super) fn bytes(value: &[u8], limits: &Limits) -> Result<Box<[u8]>, Invalid> {
    if value.len() > usize::try_from(limits.text).map_err(|_| Invalid::TooLarge)? {
        return Err(Invalid::TooLarge);
    }
    Ok(value.into())
}

pub(super) fn list<T, U>(
    value: &List<T>,
    limits: &Limits,
    map: impl Fn(&T) -> Result<U, Invalid>,
) -> Result<List<U>, Invalid> {
    if value.capacity() > limits.rows {
        return Err(Invalid::TooLarge);
    }
    let mut result = List::with_capacity(value.capacity());
    for row in value {
        result.push(map(row)?).map_err(|_| Invalid::TooLarge)?;
    }
    Ok(result)
}

/// # Errors
/// Refuses inconsistent executor or specification resource facts, any whole text/list allocation above bounds.
pub fn text(value: &documents::Text, limits: &Limits) -> Result<client::DocumentText, Invalid> {
    Ok(client::DocumentText {
        bytes: client::Text::new(bytes(&value.bytes, limits)?, limits.text).map_err(|_| Invalid::TooLarge)?,
        cut: value.cut,
    })
}

#[must_use]
pub fn held_role(value: jig_domain_people::Role) -> client::HeldRole {
    match value {
        jig_domain_people::Role::Owner => client::HeldRole::Owner,
        jig_domain_people::Role::Maintainer => client::HeldRole::Maintainer,
        jig_domain_people::Role::Member => client::HeldRole::Member,
        jig_domain_people::Role::Observer => client::HeldRole::Observer,
        jig_domain_people::Role::Policy { role } => client::HeldRole::Policy { role },
    }
}

/// # Errors
/// Refuses inconsistent executor or specification resource facts, any whole text/list allocation above bounds.
pub fn frame(
    value: &documents::Frame,
    limits: &Limits,
    models: &[jig_domain_core::Model],
    providers: &jig_domain_accounts::Providers,
) -> Result<client::Me, Invalid> {
    if value.projects.capacity() > limits.projects {
        return Err(Invalid::TooLarge);
    }
    let projects = list(&value.projects, limits, |row| {
        Ok(client::MeProject {
            models: super::llm::models(row.models, models, providers, limits)?,
            templates: super::me::templates(&row.templates, limits, models)?,
            creating: row.creating.as_ref().map(|v| super::me::creating(v, limits, models)).transpose()?,
            project: row.project,
            name: text(&row.name, limits)?,
            role: held_role(row.role),
            pool: row.pool.as_ref().map(ledger),
        })
    })?;
    Ok(client::Me {
        providers: super::catalogue::offers(&value.providers, limits)?,
        providers_cut: value.providers_cut,
        key_bytes: value.key_bytes,
        redirect_bytes: value.redirect_bytes,
        words_bytes: value.words_bytes,
        amendment_bytes: value.amendment_bytes,
        party: client::MeParty {
            number: value.party,
            kind: match value.kind {
                jig_domain_people::Kind::Person => client::PartyKind::Person,
                jig_domain_people::Kind::Service => client::PartyKind::Service,
            },
            login: text(&value.login, limits)?,
            name: text(&value.name, limits)?,
        },
        projects,
        needs: client::Needs {
            questions: value.need_counts.questions,
            proposals: value.need_counts.proposals,
            escalations: value.need_counts.escalations,
            person_tasks: value.need_counts.person_tasks,
        },
        needs_total: value.needs,
        updates: match value.updates {
            documents::Count::Known(value) => client::Count { value, limited: false },
            documents::Count::AtLeast(value) => client::Count { value, limited: true },
        },
        results_seen: value.results_seen,
        cut: value.cut,
        retention: value.retention,
        deployment_role: value.deployment_role.map(held_role),
    })
}

/// # Errors
/// Refuses inconsistent executor or specification resource facts, any whole text/list allocation above bounds.
pub fn specification(value: &documents::SpecLine, limits: &Limits) -> Result<client::SpecificationDocument, Invalid> {
    let parameters = list(&value.parameters, limits, |value| match value {
        documents::Parameter::Number { name, value } => {
            Ok(client::ParameterDocument::Number { name: *name, value: *value })
        }
        documents::Parameter::Bytes { name, value } => {
            Ok(client::ParameterDocument::Bytes { name: *name, value: text(value, limits)? })
        }
        documents::Parameter::Resource { name, part } => match part.kind {
            tasks::PartKind::Resource => match &part.key {
                tasks::PartKey::Number(resource) => Ok(client::ParameterDocument::Resource {
                    name: *name,
                    connector: part.connector,
                    resource: *resource,
                }),
                tasks::PartKey::Path(_) => Err(Invalid::ParameterResource),
            },
            tasks::PartKind::Pool
            | tasks::PartKind::Result
            | tasks::PartKind::Procedure
            | tasks::PartKind::Effect
            | tasks::PartKind::Outbox
            | tasks::PartKind::Hold => Err(Invalid::ParameterResource),
        },
    })?;
    Ok(client::SpecificationDocument {
        specification: client::DocumentSpecification {
            words: text(&value.words, limits)?,
            parameters,
            inputs: list(&value.inputs, limits, |number| Ok(*number))?,
        },
        cut: value.cut,
    })
}

/// Copy the supplied snapshot as a whole. The enclosing producer decides
/// whether instructions are current effective text or a retained own override;
/// this state neither reconstructs that context nor adds a person revision.
/// # Errors
/// Refuses inconsistent executor or specification resource facts, any whole text/list allocation above bounds.
pub fn amendment(
    value: &documents::AmendmentState,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::OwnerAmendmentState, Invalid> {
    Ok(client::OwnerAmendmentState {
        addressed: value.addressed.map(|value| match value {
            tasks::PersonAddress::Person(number) => client::PersonAddress::Party(number),
            tasks::PersonAddress::Role(number) => client::PersonAddress::Role(number),
        }),
        taken_by: value.taken_by,
        instructions: value.instructions.as_ref().map(|value| text(value, limits)).transpose()?,
        spec: specification(&value.spec, limits)?,
        authority: authority(&value.authority, limits, models)?,
        wake: wake(value.wake),
        dependencies: list(&value.dependencies, limits, |number| Ok(*number))?,
        cut: value.cut,
    })
}

/// # Errors
/// Refuses inconsistent executor or specification resource facts, any whole text/list allocation above bounds.
pub fn part(value: &tasks::Part, limits: &Limits) -> Result<client::OwnerPart, Invalid> {
    Ok(client::OwnerPart {
        connector: value.connector,
        kind: match value.kind {
            tasks::PartKind::Resource => client::OwnerPartKind::Resource,
            tasks::PartKind::Pool => client::OwnerPartKind::Pool,
            tasks::PartKind::Result => client::OwnerPartKind::Result,
            tasks::PartKind::Procedure => client::OwnerPartKind::Procedure,
            tasks::PartKind::Effect => client::OwnerPartKind::Effect,
            tasks::PartKind::Outbox => client::OwnerPartKind::Outbox,
            tasks::PartKind::Hold => client::OwnerPartKind::Hold,
        },
        key: match &value.key {
            tasks::PartKey::Number(number) => client::OwnerPartKey::Number(*number),
            tasks::PartKey::Path(segments) => client::OwnerPartKey::Path(list(
                segments,
                &Limits { rows: limits.segments, turn_text: limits.turn_text, ..*limits },
                |value| bytes(value, limits),
            )?),
        },
    })
}

pub(super) fn pattern(
    segments: &List<Box<[u8]>>,
    last: &documents::PatternEnd,
    limits: &Limits,
) -> Result<client::OwnerPattern, Invalid> {
    Ok(client::OwnerPattern {
        segments: list(segments, &Limits { rows: limits.segments, turn_text: limits.turn_text, ..*limits }, |value| {
            bytes(value, limits)
        })?,
        last: match last {
            documents::PatternEnd::Exact(value) => client::OwnerPatternEnd::Exact(bytes(value, limits)?),
            documents::PatternEnd::Open(value) => client::OwnerPatternEnd::Open(bytes(value, limits)?),
        },
    })
}

/// # Errors
/// Refuses inconsistent executor or specification resource facts, any whole text/list allocation above bounds.
pub fn authority(
    value: &documents::AuthorityLine,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::OwnerAuthorityDocument, Invalid> {
    let limits = &Limits { rows: crate::test::AUTHORITY_ENTRIES, turn_text: limits.turn_text, ..*limits };
    Ok(client::OwnerAuthorityDocument {
        authority: client::OwnerAuthority {
            models: Some(self::models(value.models, models)?),
            tools: value.tools.0,
            grants: list(&value.grants, limits, |value| {
                Ok(client::OwnerGrant {
                    connector: value.connector,
                    kind: value.kind,
                    pattern: pattern(&value.segments, &value.last, limits)?,
                })
            })?,
            delegation: client::OwnerDelegation {
                kinds: list(&value.executors, limits, |value| {
                    Ok(match value {
                        tasks::AuthorityExecutor::Charter(number) => client::OwnerAuthorityExecutor::Charter(*number),
                        tasks::AuthorityExecutor::Procedure(number) => {
                            client::OwnerAuthorityExecutor::Procedure(*number)
                        }
                        tasks::AuthorityExecutor::Role(number) => client::OwnerAuthorityExecutor::Role(*number),
                    })
                })?,
                tasks: value.tasks,
                depth: value.depth,
            },
            spend: value.ceiling.spend,
            deadline: value.ceiling.deadline,
            notes: value.notes.0,
            note_resources: list(&value.note_resources, limits, |value| {
                Ok(client::OwnerResourceScope {
                    connector: value.connector,
                    pattern: pattern(&value.segments, &value.last, limits)?,
                })
            })?,
        },
        cut: false,
    })
}

/// # Errors
/// Refuses inconsistent executor or specification resource facts, any whole text/list allocation above bounds.
pub fn contract(value: &documents::ContractLine, limits: &Limits) -> Result<client::TaskContractDocument, Invalid> {
    Ok(match value {
        documents::ContractLine::Approval { state, part: value } => {
            client::TaskContractDocument::Approval { state: bytes(state, limits)?, part: part(value, limits)? }
        }
        documents::ContractLine::Report { words } => client::TaskContractDocument::Report { words: *words },
        documents::ContractLine::Verdict { choices, cut } => client::TaskContractDocument::Verdict {
            choices: list(choices, limits, |value| {
                let capacity = u32::try_from(value.fields.len()).map_err(|_| Invalid::TooLarge)?;
                if capacity > limits.rows {
                    return Err(Invalid::TooLarge);
                }
                let mut fields = List::with_capacity(capacity);
                for field in &value.fields {
                    fields
                        .push(client::FieldRule { name: bytes(&field.name, limits)?, max: field.max })
                        .map_err(|_| Invalid::TooLarge)?;
                }
                Ok(client::VerdictChoice { code: value.code, words: value.words, followups: value.followups, fields })
            })?,
            cut: *cut,
        },
        documents::ContractLine::Change { connector, kind, words } => {
            client::TaskContractDocument::Change { connector: *connector, kind: *kind, words: *words }
        }
    })
}

#[must_use]
pub fn executor(value: tasks::Executor) -> client::OwnerExecutor {
    match value {
        tasks::Executor::Agent { charter } => client::OwnerExecutor::Agent { charter },
        tasks::Executor::Procedure { connector, code } => client::OwnerExecutor::Procedure { connector, code },
        tasks::Executor::Person(address) => client::OwnerExecutor::Person(match address {
            tasks::PersonAddress::Person(number) => client::PersonAddress::Party(number),
            tasks::PersonAddress::Role(number) => client::PersonAddress::Role(number),
        }),
    }
}

#[must_use]
pub fn funder(value: tasks::Funder) -> client::Funder {
    match value {
        tasks::Funder::Task(number) => client::Funder::Task(number),
        tasks::Funder::Pool { project, person, period } => client::Funder::Pool { project, person, period },
        tasks::Funder::Recurring { project, task, period } => client::Funder::Recurring { project, task, period },
        tasks::Funder::Period { project, period } => client::Funder::Period { project, period },
    }
}

#[must_use]
pub fn numbers(value: tasks::Numbers) -> client::OwnerNumbers {
    client::OwnerNumbers {
        budget: value.budget,
        spent: value.spent,
        spent_below: value.spent_below,
        reserved: value.reserved,
    }
}

#[must_use]
pub fn budget(value: &documents::BudgetLine) -> client::OwnerBudget {
    client::OwnerBudget {
        numbers: numbers(value.numbers),
        left: value.left,
        funder: funder(value.funder),
        deadline: value.deadline,
    }
}

pub fn ledger(value: &documents::Ledger) -> client::PoolLedger {
    client::PoolLedger {
        source: funder(value.source),
        parent: value.parent.map(funder),
        numbers: numbers(value.numbers),
        left: value.left,
        closed: value.closed,
        began: value.began,
        next: value.next,
    }
}

#[must_use]
pub fn wake(value: tasks::WakePolicy) -> client::OwnerWakePolicy {
    client::OwnerWakePolicy {
        words: wake_rule(value.words),
        notices: wake_rule(value.notices),
        news: wake_rule(value.news),
        results: match value.results {
            tasks::ResultsWake::Never => client::ResultsWake::Never,
            tasks::ResultsWake::Each => client::ResultsWake::Each,
            tasks::ResultsWake::LastOrFailure => client::ResultsWake::LastOrFailure,
        },
        questions: value.questions,
        answers: value.answers,
        timers: value.timers,
    }
}

fn wake_rule(value: tasks::WakeRule) -> client::WakeRule {
    match value {
        tasks::WakeRule::Never => client::WakeRule::Never,
        tasks::WakeRule::Immediate => client::WakeRule::Immediate,
        tasks::WakeRule::Batch { count, age } => client::WakeRule::Batch { count, age },
    }
}

/// # Errors
/// Refuses inconsistent executor or specification resource facts, any whole text/list allocation above bounds.
pub fn subscriptions(value: &List<tasks::Subscription>, limits: &Limits) -> Result<client::Subscriptions, Invalid> {
    Ok(client::Subscriptions { entries: list(value, limits, |value| Ok(subscription(*value)))? })
}

fn subscription(value: tasks::Subscription) -> client::Subscription {
    client::Subscription {
        number: value.number,
        kind: match value.kind {
            tasks::SubscriptionKind::Task { target, held, result } => {
                client::SubscriptionKind::Task { target, held, result }
            }
            tasks::SubscriptionKind::Timer { at, period } => client::SubscriptionKind::Timer { at, period },
            tasks::SubscriptionKind::Topic { connector, topic } => client::SubscriptionKind::Topic { connector, topic },
        },
    }
}

pub(super) fn models(
    value: tasks::Models,
    models: &[jig_domain_core::Model],
) -> Result<client::AuthorityModels, Invalid> {
    if value.is_all() {
        return Ok(client::AuthorityModels::All);
    }
    let count = u32::try_from(models.len()).map_err(|_| Invalid::TooLarge)?;
    if count > tasks::MODELS {
        return Err(Invalid::TooLarge);
    }
    let mut names = List::with_capacity(count);
    for (at, model) in models.iter().enumerate() {
        let bit = 1_u64.checked_shl(u32::try_from(at).map_err(|_| Invalid::Llm)?).ok_or(Invalid::Llm)?;
        if value.bits() & bit != 0 {
            names.push(model.name.clone()).map_err(|_| Invalid::TooLarge)?;
        }
    }
    let mut names = names.into_boxed();
    names.sort_unstable();
    let mut result = List::with_capacity(u32::try_from(names.len()).map_err(|_| Invalid::TooLarge)?);
    for name in names {
        result.push(name).map_err(|_| Invalid::TooLarge)?;
    }
    Ok(client::AuthorityModels::Named(result))
}

#[cfg(test)]
pub(super) mod tests {
    const UTF8_TEXTS: [&[u8]; 2] = [b"~\0", b"~/"];

    use super::*;

    pub(crate) fn amendment_source() -> documents::AmendmentState {
        let mut dependencies = List::with_capacity(4);
        dependencies.push(u64::MAX).expect("declared dependency room");
        dependencies.push(0).expect("declared dependency room");
        documents::AmendmentState {
            addressed: Some(tasks::PersonAddress::Role(u32::MAX)),
            taken_by: Some(u64::MAX),
            instructions: None,
            spec: value_source(),
            authority: documents::AuthorityLine {
                models: tasks::Models::ALL,
                tools: tasks::Tools(u64::MAX),
                grants: List::with_capacity(3),
                executors: List::with_capacity(2),
                tasks: u32::MAX,
                depth: u32::MAX,
                ceiling: tasks::Budget { spend: u64::MAX, deadline: None },
                notes: tasks::Scopes(u8::MAX),
                note_resources: List::with_capacity(1),
            },
            wake: tasks::WakePolicy {
                words: tasks::WakeRule::Never,
                notices: tasks::WakeRule::Batch { count: u32::MAX, age: skein_lib::Duration::ZERO },
                news: tasks::WakeRule::Immediate,
                results: tasks::ResultsWake::LastOrFailure,
                questions: false,
                answers: true,
                timers: false,
            },
            dependencies,
            cut: false,
        }
    }

    #[test]
    fn amendment_snapshots_keep_absent_empty_and_cut_instructions_without_reconstructing_an_override_or_revision() {
        let limits = Limits { text: 3, turn_text: 3, rows: 4, ..crate::test::limits().client.domain };
        let contexts = [
            None,
            Some(documents::Text { bytes: Box::new([]), cut: false }),
            Some(documents::Text { bytes: Box::from(UTF8_TEXTS[0]), cut: false }),
            Some(documents::Text { bytes: Box::new([]), cut: true }),
            Some(documents::Text { bytes: Box::from(UTF8_TEXTS[0]), cut: true }),
        ];
        for instructions in contexts {
            for addressed in
                [None, Some(tasks::PersonAddress::Person(u64::MAX)), Some(tasks::PersonAddress::Role(u32::MAX))]
            {
                let mut source = amendment_source();
                source.instructions = instructions.clone();
                source.addressed = addressed;
                let actual = amendment(&source, &limits, &[jig_core_world::world::scripted_model()])
                    .expect("whole supplied snapshot fits");
                assert_eq!(
                    actual.instructions.as_ref().map(|value| value.bytes.bytes()),
                    instructions.as_ref().map(|value| value.bytes.as_ref())
                );
                assert_eq!(
                    actual.instructions.as_ref().map(|value| value.cut),
                    instructions.as_ref().map(|value| value.cut)
                );
                assert_eq!(
                    actual.addressed,
                    addressed.map(|value| match value {
                        tasks::PersonAddress::Person(number) => client::PersonAddress::Party(number),
                        tasks::PersonAddress::Role(number) => client::PersonAddress::Role(number),
                    })
                );
                assert_eq!(actual.taken_by, Some(u64::MAX));
                assert_eq!(actual.dependencies.capacity(), 4);
                assert_eq!(actual.dependencies.as_slice(), &[u64::MAX, 0]);
                assert_eq!(actual.authority.authority.tools, u64::MAX);
                assert_eq!(actual.authority.authority.grants.capacity(), 3);
                assert_eq!(actual.wake.results, client::ResultsWake::LastOrFailure);
                assert!(!actual.cut);
            }
        }
    }

    #[test]
    fn amendment_snapshot_independent_loss_and_reserved_capacity_refuse_whole_values_without_trimming() {
        let limits = Limits { text: 3, turn_text: 3, rows: 4, ..crate::test::limits().client.domain };
        for (spec_loss, dependency_loss) in [(false, false), (true, false), (false, true)] {
            let mut source = amendment_source();
            source.spec.cut = spec_loss;
            source.cut = dependency_loss;
            let actual = amendment(&source, &limits, &[jig_core_world::world::scripted_model()])
                .expect("independent loss facts fit");
            assert_eq!(actual.spec.cut, spec_loss);
            assert!(actual.spec.specification.words.cut, "supplied word loss is independent of list loss");
            assert!(!actual.authority.cut);
            assert_eq!(actual.cut, dependency_loss);
        }
        let source = amendment_source();
        assert_eq!(
            amendment(
                &source,
                &Limits { rows: 3, turn_text: limits.turn_text, ..limits },
                &[jig_core_world::world::scripted_model()]
            ),
            Err(Invalid::TooLarge)
        );
        assert_eq!(
            amendment(&source, &Limits { text: 2, turn_text: 2, ..limits }, &[jig_core_world::world::scripted_model()]),
            Err(Invalid::TooLarge)
        );
        assert_eq!(source.dependencies.capacity(), 4);
        assert_eq!(source.dependencies.as_slice(), &[u64::MAX, 0]);
        assert_eq!(source.spec.words.bytes.as_ref(), b"a/b");
        assert!(source.instructions.is_none());
    }

    #[test]
    fn owner_budget_and_ledger_wire_copy_all_accounting_fields_and_optional_funding_clocks() {
        let numbers = tasks::Numbers { budget: 1, spent: 2, spent_below: 3, reserved: u64::MAX };
        let expected = client::OwnerNumbers { budget: 1, spent: 2, spent_below: 3, reserved: u64::MAX };
        let funders = [
            (tasks::Funder::Task(u64::MAX), client::Funder::Task(u64::MAX)),
            (
                tasks::Funder::Pool { project: u32::MAX, person: u64::MAX, period: 0 },
                client::Funder::Pool { project: u32::MAX, person: u64::MAX, period: 0 },
            ),
            (
                tasks::Funder::Recurring { project: 0, task: u64::MAX, period: u64::MAX },
                client::Funder::Recurring { project: 0, task: u64::MAX, period: u64::MAX },
            ),
            (
                tasks::Funder::Period { project: u32::MAX, period: u64::MAX },
                client::Funder::Period { project: u32::MAX, period: u64::MAX },
            ),
        ];
        for (source, expected_funder) in funders {
            for deadline in [None, Some(skein_lib::Wall::EPOCH)] {
                let source_budget = documents::BudgetLine { numbers, left: 17, funder: source, deadline };
                let actual = budget(&source_budget);
                assert_eq!(actual.numbers, expected);
                assert_eq!(actual.left, 17);
                assert_eq!(actual.funder, expected_funder);
                assert_eq!(actual.deadline, deadline);
                for parent in [None, Some(tasks::Funder::Task(0))] {
                    for closed in [false, true] {
                        let source_ledger = documents::Ledger {
                            source,
                            parent,
                            numbers,
                            left: 29,
                            closed,
                            began: deadline,
                            next: if deadline.is_none() { Some(skein_lib::Wall::EPOCH) } else { None },
                        };
                        let actual = ledger(&source_ledger);
                        assert_eq!(actual.source, expected_funder);
                        assert_eq!(actual.parent, parent.map(|_| client::Funder::Task(0)));
                        assert_eq!(actual.numbers, expected);
                        assert_eq!(actual.left, 29);
                        assert_eq!(actual.closed, closed);
                        assert_eq!(actual.began, source_ledger.began);
                        assert_eq!(actual.next, source_ledger.next);
                    }
                }
            }
        }
    }

    #[test]
    fn specification_wire_keeps_source_input_numbers_allocated_capacity_and_independent_loss_facts() {
        let mut parameters = List::with_capacity(4);
        parameters
            .push(documents::Parameter::Bytes {
                name: u32::MAX,
                value: documents::Text { bytes: Box::from(UTF8_TEXTS[0]), cut: true },
            })
            .expect("the complete supplied fixture is admitted");
        let mut inputs = List::with_capacity(3);
        inputs.push(u64::MAX).expect("the complete supplied fixture is admitted");
        inputs.push(0).expect("the complete supplied fixture is admitted");
        let value = documents::SpecLine {
            words: documents::Text { bytes: b"a/b".as_slice().into(), cut: false },
            parameters,
            inputs,
            cut: false,
        };
        let limits = Limits { text: 3, turn_text: 3, rows: 4, ..crate::test::limits().client.domain };
        let actual = specification(&value, &limits).expect("the complete supplied fixture is admitted");
        assert_eq!(actual.specification.inputs.capacity(), 3);
        assert_eq!(actual.specification.inputs.as_slice(), &[u64::MAX, 0]);
        assert_eq!(actual.specification.parameters.capacity(), 4);
        assert!(!actual.cut);
        assert!(!actual.specification.words.cut);
        let client::ParameterDocument::Bytes { name, value } =
            &actual.specification.parameters.get(0).expect("the complete supplied row is retained")
        else {
            panic!("opaque byte parameter")
        };
        assert_eq!(*name, u32::MAX);
        assert_eq!(value.bytes.bytes(), UTF8_TEXTS[0]);
        assert!(value.cut);
        assert!(actual.whole().is_none());
        assert_eq!(specification(&value_source(), &Limits { text: 2, turn_text: 2, ..limits }), Err(Invalid::TooLarge));
    }

    fn value_source() -> documents::SpecLine {
        documents::SpecLine {
            words: documents::Text { bytes: b"a/b".as_slice().into(), cut: true },
            parameters: List::with_capacity(0),
            inputs: List::with_capacity(0),
            cut: false,
        }
    }

    #[test]
    fn raw_owner_parts_keep_all_kinds_and_literal_segments_without_scope_or_numeric_inference() {
        for kind in [
            tasks::PartKind::Resource,
            tasks::PartKind::Pool,
            tasks::PartKind::Result,
            tasks::PartKind::Procedure,
            tasks::PartKind::Effect,
            tasks::PartKind::Outbox,
        ] {
            let mut segments: List<Box<[u8]>> = List::with_capacity(3);
            segments.push(b"a/b".as_slice().into()).expect("the complete supplied fixture is admitted");
            segments.push(Box::new([255, 0])).expect("the complete supplied fixture is admitted");
            let source = tasks::Part { connector: u16::MAX, kind, key: tasks::PartKey::Path(segments) };
            let limits = Limits { rows: 3, segments: 3, text: 3, turn_text: 3, ..crate::test::limits().client.domain };
            let actual = part(&source, &limits).expect("the complete supplied fixture is admitted");
            assert_eq!(actual.connector, u16::MAX);
            let client::OwnerPartKey::Path(path) = actual.key else { panic!("literal segments stay a path") };
            assert_eq!(path.capacity(), 3);
            assert_eq!(path.get(0).expect("the complete supplied row is retained").as_ref(), b"a/b");
            assert_eq!(path.get(1).expect("the complete supplied row is retained").as_ref(), &[255, 0]);
            assert_eq!(
                part(&source, &Limits { rows: 2, segments: 2, turn_text: limits.turn_text, ..limits }),
                Err(Invalid::TooLarge)
            );
            assert_eq!(part(&source, &Limits { text: 2, turn_text: 2, ..limits }), Err(Invalid::TooLarge));
        }
    }
    #[test]
    fn owner_authority_and_verdict_wire_keep_opaque_patterns_machine_names_and_projection_cuts() {
        let limits = Limits { rows: 4, text: 3, turn_text: 3, ..crate::test::limits().client.domain };
        let mut segments: List<Box<[u8]>> = List::with_capacity(3);
        segments.push(b"a/b".as_slice().into()).expect("the complete supplied fixture is admitted");
        segments.push(Box::new([255, 0])).expect("the complete supplied fixture is admitted");
        let mut grants = List::with_capacity(2);
        grants
            .push(documents::GrantLine {
                connector: u16::MAX,
                kind: u16::MAX,
                segments: segments.clone(),
                last: documents::PatternEnd::Open(Box::new([0, 255])),
            })
            .expect("the complete supplied fixture is admitted");
        let mut executors = List::with_capacity(3);
        executors.push(tasks::AuthorityExecutor::Role(u32::MAX)).expect("the complete supplied fixture is admitted");
        let mut resources = List::with_capacity(4);
        resources
            .push(documents::NoteScope {
                connector: u16::MAX,
                segments: segments.clone(),
                last: documents::PatternEnd::Exact(Box::new([255, 0])),
            })
            .expect("the complete supplied fixture is admitted");
        for deadline in [None, Some(skein_lib::Wall::EPOCH)] {
            let source = documents::AuthorityLine {
                models: tasks::Models::ALL,
                tools: tasks::Tools(u64::MAX),
                grants: grants.clone(),
                executors: executors.clone(),
                tasks: u32::MAX,
                depth: u32::MAX,
                ceiling: tasks::Budget { spend: u64::MAX, deadline },
                notes: tasks::Scopes(u8::MAX),
                note_resources: resources.clone(),
            };
            let actual = authority(&source, &limits, &[jig_core_world::world::scripted_model()])
                .expect("the complete supplied fixture is admitted");
            assert!(!actual.cut);
            assert_eq!(actual.authority.tools, u64::MAX);
            assert_eq!(actual.authority.notes, u8::MAX);
            assert_eq!(actual.authority.deadline, deadline);
            assert_eq!(actual.authority.grants.capacity(), 2);
            assert_eq!(actual.authority.note_resources.capacity(), 4);
            assert_eq!(
                actual
                    .authority
                    .grants
                    .get(0)
                    .expect("the complete supplied row is retained")
                    .pattern
                    .segments
                    .capacity(),
                3
            );
            assert_eq!(
                actual.authority.grants.get(0).expect("the complete supplied row is retained").pattern.last,
                client::OwnerPatternEnd::Open(Box::new([0, 255]))
            );
            assert_eq!(
                actual.authority.note_resources.get(0).expect("the complete supplied row is retained").pattern.last,
                client::OwnerPatternEnd::Exact(Box::new([255, 0]))
            );
            assert_eq!(
                actual.authority.delegation.kinds.get(0).expect("the complete supplied row is retained"),
                &client::OwnerAuthorityExecutor::Role(u32::MAX)
            );
            let mut oversized = source.clone();
            oversized.note_resources = List::with_capacity(crate::test::AUTHORITY_ENTRIES + 1);
            assert_eq!(
                authority(
                    &oversized,
                    &Limits { rows: 3, turn_text: limits.turn_text, ..limits },
                    &[jig_core_world::world::scripted_model()]
                ),
                Err(Invalid::TooLarge)
            );
        }
        assert_whole_verdict_contract(&limits);
    }
    fn assert_whole_verdict_contract(limits: &Limits) {
        let mut choices = List::with_capacity(4);
        choices
            .push(tasks::Verdict {
                code: u32::MAX,
                words: u32::MAX,
                followups: u32::MAX,
                fields: Box::new([tasks::FieldRule { name: Box::new([255, 0]), max: u32::MAX }]),
            })
            .expect("the complete supplied fixture is admitted");
        let actual = contract(&documents::ContractLine::Verdict { choices, cut: true }, limits)
            .expect("the complete supplied fixture is admitted");
        let client::TaskContractDocument::Verdict { choices, cut } = actual else {
            panic!("exact closed verdict form")
        };
        assert!(cut);
        assert_eq!(choices.capacity(), 4);
        assert_eq!(choices.get(0).expect("the complete supplied row is retained").code, u32::MAX);
        assert_eq!(choices.get(0).expect("the complete supplied row is retained").words, u32::MAX);
        assert_eq!(choices.get(0).expect("the complete supplied row is retained").followups, u32::MAX);
        assert_eq!(
            choices
                .get(0)
                .expect("the complete supplied row is retained")
                .fields
                .get(0)
                .expect("the complete supplied field is retained")
                .name
                .as_ref(),
            &[255, 0]
        );
        assert_eq!(
            choices
                .get(0)
                .expect("the complete supplied row is retained")
                .fields
                .get(0)
                .expect("the complete supplied field is retained")
                .max,
            u32::MAX
        );
    }
    #[test]
    fn owner_wake_and_subscriptions_keep_independent_rules_and_absent_or_zero_timer_periods() {
        let age = skein_lib::Duration::from_nanos(u64::MAX);
        let value = wake(tasks::WakePolicy {
            words: tasks::WakeRule::Never,
            notices: tasks::WakeRule::Batch { count: u32::MAX, age },
            news: tasks::WakeRule::Immediate,
            results: tasks::ResultsWake::Each,
            questions: false,
            answers: true,
            timers: false,
        });
        assert_eq!(value.words, client::WakeRule::Never);
        assert_eq!(value.notices, client::WakeRule::Batch { count: u32::MAX, age });
        assert_eq!(value.news, client::WakeRule::Immediate);
        assert_eq!(value.results, client::ResultsWake::Each);
        assert!(!value.questions);
        assert!(value.answers);
        assert!(!value.timers);
        let limits = Limits {
            rows: 4,
            turn_text: crate::test::limits().client.domain.turn_text,
            ..crate::test::limits().client.domain
        };
        let mut source = List::with_capacity(4);
        source
            .push(tasks::Subscription {
                number: 11,
                kind: tasks::SubscriptionKind::Timer { at: skein_lib::Wall::EPOCH, period: None },
            })
            .expect("the complete supplied fixture is admitted");
        source
            .push(tasks::Subscription {
                number: 12,
                kind: tasks::SubscriptionKind::Timer {
                    at: skein_lib::Wall::EPOCH,
                    period: Some(skein_lib::Duration::ZERO),
                },
            })
            .expect("the complete supplied fixture is admitted");
        source
            .push(tasks::Subscription {
                number: u64::MAX,
                kind: tasks::SubscriptionKind::Topic { connector: u16::MAX, topic: u64::MAX },
            })
            .expect("the complete supplied fixture is admitted");
        let actual = subscriptions(&source, &limits).expect("the complete supplied fixture is admitted");
        assert_eq!(actual.entries.capacity(), 4);
        assert_eq!(
            actual.entries.get(0).expect("the complete supplied row is retained").kind,
            client::SubscriptionKind::Timer { at: skein_lib::Wall::EPOCH, period: None }
        );
        assert_eq!(
            actual.entries.get(1).expect("the complete supplied row is retained").kind,
            client::SubscriptionKind::Timer { at: skein_lib::Wall::EPOCH, period: Some(skein_lib::Duration::ZERO) }
        );
        assert_eq!(actual.entries.get(2).expect("the complete supplied row is retained").number, u64::MAX);
        assert_eq!(
            actual.entries.get(2).expect("the complete supplied row is retained").kind,
            client::SubscriptionKind::Topic { connector: u16::MAX, topic: u64::MAX }
        );
        assert_eq!(
            subscriptions(&source, &Limits { rows: 3, turn_text: limits.turn_text, ..limits }),
            Err(Invalid::TooLarge)
        );
    }

    #[test]
    fn me_preserves_identity_counts_memberships_and_project_offers() {
        let roles = [
            (jig_domain_people::Role::Owner, client::HeldRole::Owner),
            (jig_domain_people::Role::Maintainer, client::HeldRole::Maintainer),
            (jig_domain_people::Role::Member, client::HeldRole::Member),
            (jig_domain_people::Role::Observer, client::HeldRole::Observer),
            (jig_domain_people::Role::Policy { role: u32::MAX }, client::HeldRole::Policy { role: u32::MAX }),
        ];
        let mut projects = List::with_capacity(7);
        for (role, _) in roles {
            projects
                .push(documents::FrameProjectRole {
                    project: u32::MAX,
                    role,
                    name: jig_domain_core::documents::Text { bytes: Box::from(&b"A"[..]), cut: false },
                    pool: None,
                    creating: None,
                    models: jig_domain_authority::Models::NONE,
                    templates: List::with_capacity(0),
                })
                .expect("the complete supplied fixture is admitted");
        }
        let source = documents::Frame {
            providers: skein_lib::List::with_capacity(0),
            providers_cut: false,
            key_bytes: 256,
            redirect_bytes: 4096,
            words_bytes: 65_536,
            amendment_bytes: 65_536,
            party: u64::MAX,
            kind: jig_domain_people::Kind::Service,
            login: documents::Text { bytes: Box::from(UTF8_TEXTS[1]), cut: true },
            name: documents::Text { bytes: Box::new([]), cut: false },
            deployment_role: None,
            projects,
            needs: 123,
            need_counts: jig_domain_people::Needs {
                questions: 1,
                proposals: 2,
                escalations: 3,
                person_tasks: u32::MAX,
            },
            updates: documents::Count::AtLeast(u32::MAX),
            results_seen: u64::MAX,
            retention: skein_lib::Duration::from_nanos(u64::MAX),
            cut: true,
        };
        let limits = Limits { projects: 7, rows: 7, text: 2, turn_text: 2, ..crate::test::limits().client.domain };
        let translated = frame(&source, &limits, &[], &jig_domain_accounts::Providers::none())
            .expect("the complete supplied fixture is admitted");
        assert_eq!(translated.party.number, u64::MAX);
        assert_eq!(translated.party.kind, client::PartyKind::Service);
        assert_eq!(translated.party.login.bytes.bytes(), UTF8_TEXTS[1]);
        assert!(translated.party.login.cut);
        assert_eq!(translated.party.name.bytes.bytes(), b"");
        assert!(!translated.party.name.cut);
        assert_eq!(translated.needs_total, 123);
        assert_eq!(
            translated.needs,
            client::Needs { questions: 1, proposals: 2, escalations: 3, person_tasks: u32::MAX }
        );
        assert_eq!(translated.updates, client::Count { value: u32::MAX, limited: true });
        assert_eq!(translated.results_seen, u64::MAX);
        assert!(translated.cut);
        assert_eq!(translated.retention, source.retention);
        assert_eq!(translated.deployment_role, None);
        assert_eq!(translated.projects.capacity(), 7);
        for (row, (_, expected)) in translated.projects.iter().zip(roles) {
            assert_eq!(row.role, expected);
            assert_eq!(row.project, u32::MAX);
            assert_eq!(row.name.bytes(), b"A");
            assert_eq!(row.pool, None);
        }
        assert_eq!(
            frame(
                &source,
                &Limits { projects: 6, turn_text: limits.turn_text, ..limits },
                &[],
                &jig_domain_accounts::Providers::none()
            ),
            Err(Invalid::TooLarge)
        );
        assert_eq!(
            frame(&source, &Limits { text: 1, turn_text: 1, ..limits }, &[], &jig_domain_accounts::Providers::none()),
            Err(Invalid::TooLarge)
        );
        let mut known = source;
        known.updates = documents::Count::Known(0);
        known.kind = jig_domain_people::Kind::Person;
        let translated = frame(&known, &limits, &[], &jig_domain_accounts::Providers::none())
            .expect("the complete supplied fixture is admitted");
        assert_eq!(translated.updates, client::Count { value: 0, limited: false });
        assert_eq!(translated.party.kind, client::PartyKind::Person);
    }
}
