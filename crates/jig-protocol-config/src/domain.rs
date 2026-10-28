//! Deployment, initial identities and connector numbers (configuration.md, 3).
use crate::{CodecBound, Directories, Directory, Path, Problem, Problems, What, check, read};
use alloc::boxed::Box;
use jig_domain_people::{IdentityKey, InitialOwner};
use skein_json::Document;
use skein_lib::List;

#[derive(Debug)]
pub struct Deployment {
    pub directory: Box<[u8]>,
    pub seed: Option<u64>,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProviderKind {
    Link,
    Application,
    Deployment,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Provider {
    pub number: u16,
    pub kind: ProviderKind,
}

pub(crate) fn deployment(
    document: &Document,
    at: u32,
    path: &Path,
    directories: &Directories,
    problems: &mut Problems,
) -> Option<Deployment> {
    let mut object = check::object(document, at, path, problems)?;
    let directory = check::path(&mut object, document, b"directory", path, Directory::Data, directories, problems);
    let mut seed = None;
    if let Some(at) = object.member(document, b"seed") {
        match read::integer(document, at, u64::MAX) {
            Ok(value) => seed = Some(value),
            Err(what) => problems.push(path.member(b"seed"), what),
        }
    }
    object.finish(document, path, problems);
    Some(Deployment { directory: directory?, seed })
}
pub(crate) fn providers(document: &Document, at: u32, path: &Path, problems: &mut Problems) -> Option<Box<[Provider]>> {
    let items = items(document, at, document.len(), path, problems)?;
    let mut providers = List::<Provider>::with_capacity(items.len());
    let mut deployments = 0_u32;
    for (index, at) in items.iter().enumerate() {
        let path = check::index(path, index);
        let Some(mut object) = check::object(document, *at, &path, problems) else { continue };
        let number = check::short(&mut object, document, b"number", &path, problems);
        let kind = check::text(&mut object, document, b"kind", &path, problems);
        object.finish(document, &path, problems);
        if let Some(number) = number
            && let Some(kind) = kind
        {
            let kind = match kind.as_ref() {
                b"link" => ProviderKind::Link,
                b"application" => ProviderKind::Application,
                b"deployment" => {
                    deployments = deployments.saturating_add(1);
                    if deployments > 1 {
                        problems.push(path.member(b"kind"), What::Unique);
                    }
                    ProviderKind::Deployment
                }
                _ => {
                    problems.push(path.member(b"kind"), What::Form);
                    continue;
                }
            };
            for previous in &providers {
                if previous.number == number {
                    problems.push(path.member(b"number"), What::Unique);
                    break;
                }
            }
            providers.push(Provider { number, kind }).expect("bounded provider input");
        }
    }
    if deployments == 0 {
        problems.push(path.clone(), What::Missing);
    }
    Some(providers.into_boxed())
}
pub(crate) fn owners(
    document: &Document,
    at: u32,
    engine: &crate::Engine,
    problems: &mut Problems,
) -> Box<[InitialOwner]> {
    let path = &Path::root().member(b"owners");
    let mut owners = List::<InitialOwner>::with_capacity(engine.limits.core.people.initial_owners);
    let Some(items) = items(document, at, engine.limits.core.people.initial_owners, path, problems) else {
        return owners.into_boxed();
    };
    for (index, at) in items.iter().enumerate() {
        let path = check::index(path, index);
        let Some(mut object) = check::object(document, *at, &path, problems) else { continue };
        let provider = check::short(&mut object, document, b"provider", &path, problems);
        let subject = check::text(&mut object, document, b"subject", &path, problems);
        object.finish(document, &path, problems);
        if let Some(provider) = provider
            && let Some(subject) = subject
        {
            let mut known = false;
            for configured in &engine.providers {
                known |= configured.number == provider;
            }
            if !known {
                problems.push(path.member(b"provider"), What::Refers);
            }
            if subject.len() > usize::try_from(engine.limits.core.people.identity_bytes).expect("identity bound") {
                problems.push(path.member(b"subject"), What::Bound);
            }
            let identity = IdentityKey { provider, subject };
            for previous in &owners {
                if previous.identity == identity {
                    problems.push(path.clone(), What::Unique);
                    break;
                }
            }
            // With no projects, retain one bootstrap entry per owner for the deployment.
            // initial_owners still bounds retained entries, not distinct identities.
            if engine.projects.is_empty()
                && owners
                    .push(InitialOwner { project: jig_domain_people::DEPLOYMENT, identity: identity.clone() })
                    .is_err()
            {
                problems.push(path.clone(), What::Bound);
            }
            for project in &engine.projects {
                if owners.push(InitialOwner { project: project.number, identity: identity.clone() }).is_err() {
                    problems.push(path.clone(), What::Bound);
                    break;
                }
            }
        }
    }
    owners.into_boxed()
}
pub(crate) fn connectors(
    document: &Document,
    at: u32,
    path: &Path,
    max: u32,
    problems: &mut Problems,
) -> Option<Box<[u16]>> {
    let items = items(document, at, max, path, problems)?;
    let mut numbers = List::with_capacity(max);
    for (index, at) in items.iter().enumerate() {
        let path = check::index(path, index);
        match read::integer(document, *at, u64::from(u16::MAX)) {
            Ok(number) => {
                let number = u16::try_from(number).expect("u16 checked");
                if number == jig_domain_core::RECURRING {
                    problems.push(path.clone(), What::Bound);
                }
                if numbers.as_slice().contains(&number) {
                    problems.push(path.clone(), What::Unique);
                }
                numbers.push(number).expect("bounded connector input");
            }
            Err(what) => problems.push(path, what),
        }
    }
    Some(numbers.into_boxed())
}
pub(crate) fn items(document: &Document, at: u32, max: u32, path: &Path, problems: &mut Problems) -> Option<List<u32>> {
    match read::list(document, at, max) {
        Ok(items) => Some(items),
        Err(what) => {
            problems.push(path.clone(), what);
            None
        }
    }
}
fn operations(limits: &jig_domain_core::Limits, problems: &mut Problems) {
    if u64::from(limits.people.operations)
        < (u64::from(limits.tasks.delegates) + 1).max(u64::from(limits.tasks.project_goals))
    {
        problems.push(
            Path::root().member(b"limits"),
            What::Operations {
                operations: limits.people.operations,
                delegates: limits.tasks.delegates,
                goals: limits.tasks.project_goals,
            },
        );
    }
}
pub(crate) fn whole(engine: &crate::Engine, problems: &mut Problems) {
    if engine.limits.driver.client.url_bytes > jig_domain_accounts::ADDRESS {
        problems.push(Path::root().member(b"limits").member(b"driver.url_bytes"), What::Bound);
    }
    let limits = &engine.limits.core;
    let settings_path = Path::root().member(b"limits").member(b"core").member(b"settings.history");
    if limits.settings.proposals == 0 {
        problems.push(Path::root().member(b"limits").member(b"core").member(b"settings.proposals"), What::Bound);
    }
    if limits.settings.revisions == 0 {
        problems.push(Path::root().member(b"limits").member(b"core").member(b"settings.revisions"), What::Bound);
    }
    if limits.settings.versions == 0 {
        problems.push(Path::root().member(b"limits").member(b"core").member(b"settings.versions"), What::Bound);
    }
    if limits.settings.history == 0 {
        problems.push(settings_path, What::Bound);
    }
    if match jig_domain_core::settings::writes_worst_case(limits) {
        Some(writes) => writes > engine.limits.journal.writes,
        None => true,
    } {
        problems.push(Path::root().member(b"limits").member(b"journal.writes"), What::Bound);
    }
    if engine.projects.len() >= usize::try_from(limits.people.projects).expect("project capacity fits usize") {
        problems.push(Path::root().member(b"projects"), What::Bound);
    }
    operations(limits, problems);
    if let Err(what) = check_message(limits) {
        problems.push(Path::root().member(b"limits"), what);
    }
    if let Err(problem) = check_effective_limits(limits) {
        problems.push(problem.path, problem.what);
    }
    if !jig_domain_core::valid(limits) {
        problems.push(Path::root().member(b"limits"), What::Bound);
    }
    if !jig_domain_host::valid(&engine.limits.hub) {
        problems.push(Path::root().member(b"limits"), What::Bound);
    }
}

/// Checks of document seed projects that depend on the settings defaults.
pub(crate) fn seeds(engine: &crate::Engine, problems: &mut Problems) {
    for (index, project) in engine.projects.iter().enumerate() {
        let path = check::index(&Path::root().member(b"projects"), index);
        if !check_policy(engine, project.number, &project.policy, &project.roles, &project.templates, &path, problems) {
            problems.push(path.member(b"policy"), What::Bound);
        }
    }
}

/// Checks of settings supplied by the document; passed settings use the core's check.
pub(crate) fn document_defaults(engine: &crate::Engine, problems: &mut Problems) {
    for (index, policy) in engine.policies.iter().enumerate() {
        let path = check::index(&Path::root().member(b"policies"), index);
        if !check_policy(engine, policy.number, &policy.policy, &policy.roles, &policy.templates, &path, problems) {
            problems.push(path, What::Bound);
        }
    }
    authority_refs(engine, &engine.rules.ceiling, &Path::root().member(b"rules").member(b"ceiling"), None, problems);
    requirement_refs(
        engine,
        &engine.rules.requirements,
        &Path::root().member(b"rules").member(b"requirements"),
        problems,
    );
    for (index, pair) in engine.rules.implies.entries().iter().enumerate() {
        connector(
            engine,
            pair.connector,
            &check::index(&Path::root().member(b"rules").member(b"implies"), index).member(b"connector"),
            problems,
        );
    }
}

pub(crate) fn check_message(limits: &jig_domain_core::Limits) -> Result<(), What> {
    if limits.people.words > limits.tasks.message_bytes
        || limits.tasks.result_bytes > limits.tasks.message_bytes
        || limits.tasks.message_bytes < 24
    {
        return Err(What::Message);
    }
    Ok(())
}
fn check_policy(
    engine: &crate::Engine,
    number: u32,
    policy: &jig_domain_authority::Policy,
    roles: &[jig_domain_people::PermissionRole],
    templates: &[jig_domain_people::TemplateOverride],
    path: &Path,
    problems: &mut Problems,
) -> bool {
    let admitted = if engine.defaults == crate::Defaults::Passed {
        jig_domain_authority::policy_refusal(&engine.rules, &engine.limits.core.authority, policy).is_none()
    } else {
        let Some(mut authority) = jig_domain_authority::Domain::new(engine.rules.clone(), engine.limits.core.authority)
        else {
            return false;
        };
        let mut facts = skein_lib::Queue::with_capacity(jig_domain_authority::POLICY_MAX_OUT);
        jig_domain_authority::step(
            &mut authority,
            jig_domain_authority::Event::Policy { project: number, policy: policy.clone() },
            &mut facts,
        );
        match facts.pop().expect("one policy admission fact") {
            jig_domain_authority::PolicyFact::Added { .. } | jig_domain_authority::PolicyFact::Changed { .. } => true,
            jig_domain_authority::PolicyFact::Refused { .. } | jig_domain_authority::PolicyFact::Dropped { .. } => {
                false
            }
        }
    };
    crate::templates::check_overrides(engine, templates, &path.member(b"policy").member(b"templates"), problems);
    for (index, role) in roles.iter().enumerate() {
        let path = check::index(&path.member(b"roles"), index);
        connector(engine, role.connector, &path.member(b"connector"), problems);
        let mut known = false;
        for configured in &policy.roles {
            known |= configured.number == role.role;
        }
        if !known {
            problems.push(path.member(b"role"), What::Refers);
        }
    }
    authority_refs(engine, &policy.ceiling, &path.member(b"policy").member(b"ceiling"), Some(policy), problems);
    for (index, role) in policy.roles.iter().enumerate() {
        let path = check::index(&path.member(b"policy").member(b"roles"), index);
        authority_refs(engine, &role.authority, &path.member(b"authority"), Some(policy), problems);
    }
    grant_refs(engine, &policy.projections, &path.member(b"policy").member(b"projections"), problems);
    requirement_refs(engine, &policy.requirements, &path.member(b"policy").member(b"requirements"), problems);
    let bytes = match jig_domain_core::policy_snapshot(policy, roles, templates) {
        Some(value) => jig_domain_people::policy_bytes(&value),
        None => None,
    };
    let bounded = match bytes {
        Some(bytes) => bytes <= engine.limits.core.policy_bytes,
        None => false,
    };
    admitted && bounded
}

fn connector(engine: &crate::Engine, number: u16, path: &Path, problems: &mut Problems) {
    if !engine.connectors.contains(&number) {
        problems.push(path.clone(), What::Refers);
    }
}
fn grant_refs(engine: &crate::Engine, grants: &[jig_domain_authority::Grant], path: &Path, problems: &mut Problems) {
    for (index, grant) in grants.iter().enumerate() {
        connector(engine, grant.connector, &check::index(path, index).member(b"connector"), problems);
    }
}
fn authority_refs(
    engine: &crate::Engine,
    value: &jig_domain_authority::Authority,
    path: &Path,
    policy: Option<&jig_domain_authority::Policy>,
    problems: &mut Problems,
) {
    grant_refs(engine, &value.grants, &path.member(b"grants"), problems);
    for (index, scope) in value.note_resources.iter().enumerate() {
        connector(
            engine,
            scope.connector,
            &check::index(&path.member(b"note_resources"), index).member(b"connector"),
            problems,
        );
    }
    for (index, executor) in value.delegation.kinds.iter().enumerate() {
        let path = check::index(&path.member(b"delegation").member(b"kinds"), index).member(b"number");
        match executor {
            jig_domain_authority::Executor::Charter(number) => {
                let mut known = false;
                for charter in &engine.charters {
                    known |= charter.number == *number;
                }
                if !known {
                    problems.push(path, What::Refers);
                }
            }
            jig_domain_authority::Executor::Role(number) => {
                if let Some(policy) = policy {
                    let mut known = false;
                    for role in &policy.roles {
                        known |= role.number == *number;
                    }
                    if !known {
                        problems.push(path, What::Refers);
                    }
                }
            }
            jig_domain_authority::Executor::Procedure(_) => {}
        }
    }
}
fn requirement_refs(
    engine: &crate::Engine,
    requirements: &[jig_domain_authority::Requirement],
    path: &Path,
    problems: &mut Problems,
) {
    for (index, requirement) in requirements.iter().enumerate() {
        let path = check::index(path, index);
        connector(engine, requirement.connector, &path.member(b"connector"), problems);
        connector(engine, requirement.judge.connector, &path.member(b"judge").member(b"connector"), problems);
    }
}

/// Check the effective core limits that will be handed to the store (configuration.md, 6).
///
/// This probes the people family and the combined store codec without changing any
/// limits. Scalar refusals name the field that fails at the envelope's next byte.
/// Independent people ceilings retain the generated first failure. Arithmetic and
/// refusals from other store families have distinct named problems.
pub fn check_effective_limits(limits: &jig_domain_core::Limits) -> Result<(), Problem> {
    if codec_admits(limits) {
        return Ok(());
    }
    // Isolate independent ceilings before diagnosing either byte budget. These
    // are probe inputs only; the caller's effective limits are never changed.
    let mut baseline = *limits;
    baseline.policy_bytes = 0;
    baseline.people.amendment_bytes = 0;
    let Some(derived) = jig_protocol_store::codec::people::derived_limits(&baseline) else {
        return Err(Problem { path: Path::root().member(b"limits"), what: What::CodecArithmetic });
    };
    if let Err(problem) = jig_protocol_store::people_v13::check_limits(&derived) {
        let configured = match jig_protocol_store::codec::people::derived_limits(limits) {
            Some(configured) => configured,
            None => derived,
        };
        let Some(at) = people_bound(&configured, problem.path) else {
            return Err(Problem { path: Path::root().member(b"limits"), what: What::StoreCodec });
        };
        return Err(Problem { path: codec_path(at.name), what: What::CodecCeiling { bound: at.bound } });
    }
    if !codec_admits(&baseline) {
        return Err(Problem { path: Path::root().member(b"limits"), what: What::StoreCodec });
    }
    if let Some(maximum) = policy_bytes_max(limits)
        && limits.policy_bytes > maximum
    {
        return Err(policy_envelope(limits, maximum));
    }
    if let Some(maximum) = amendment_bytes_max(limits)
        && limits.people.amendment_bytes > maximum
    {
        return Err(amendment_envelope(limits, maximum));
    }
    // Neither axis admits even zero with the other held fixed. Diagnose both
    // excessive budgets in order, using isolated probe inputs only.
    let maximum = policy_bytes_max(&baseline).expect("zero policy was admitted");
    if limits.policy_bytes > maximum {
        baseline.policy_bytes = limits.policy_bytes;
        return Err(policy_envelope(&baseline, maximum));
    }
    Err(Problem { path: Path::root().member(b"limits"), what: What::StoreCodec })
}

fn policy_envelope(limits: &jig_domain_core::Limits, maximum: u64) -> Problem {
    let mut edge = *limits;
    edge.policy_bytes = maximum.checked_add(1).expect("policy arithmetic refuses u64::MAX");
    Problem {
        path: codec_path(b"core.policy_bytes"),
        what: What::PolicyEnvelope { value: limits.policy_bytes, maximum, bound: binding_bound(&edge, limits) },
    }
}

fn amendment_envelope(limits: &jig_domain_core::Limits, maximum: u32) -> Problem {
    let mut edge = *limits;
    edge.people.amendment_bytes = maximum.checked_add(1).expect("amendment arithmetic refuses u32::MAX");
    Problem {
        path: codec_path(b"people.amendment_bytes"),
        what: What::AmendmentEnvelope {
            value: limits.people.amendment_bytes,
            maximum,
            bound: binding_bound(&edge, limits),
        },
    }
}

/// Largest policy byte budget admitted by the complete codec with all other
/// effective limits unchanged. `None` means even a zero budget is refused.
/// The search makes at most 64 probes after the zero-budget admission check.
#[must_use]
pub fn policy_bytes_max(limits: &jig_domain_core::Limits) -> Option<u64> {
    let mut candidate = *limits;
    candidate.policy_bytes = 0;
    if !codec_admits(&candidate) {
        return None;
    }
    let mut admitted = 0_u64;
    let mut refused = u64::MAX; // people derivation narrows policy bytes to u32
    for _ in 0..u64::BITS {
        let span = refused.checked_sub(admitted)?;
        if span <= 1 {
            break;
        }
        let middle = admitted.checked_add(span.checked_div(2)?)?;
        candidate.policy_bytes = middle;
        if codec_admits(&candidate) {
            admitted = middle;
        } else {
            refused = middle;
        }
    }
    Some(admitted)
}

/// Largest amendment byte budget admitted by the complete codec with all other
/// effective limits unchanged. `None` means even a zero budget is refused.
/// The search makes at most 32 probes after the zero-budget admission check.
#[must_use]
pub fn amendment_bytes_max(limits: &jig_domain_core::Limits) -> Option<u32> {
    let mut candidate = *limits;
    candidate.people.amendment_bytes = 0;
    if !codec_admits(&candidate) {
        return None;
    }
    let mut admitted = 0_u32;
    let mut refused = u32::MAX; // member_dependencies requires checked 3 * amendment
    for _ in 0..u32::BITS {
        let span = refused.checked_sub(admitted)?;
        if span <= 1 {
            break;
        }
        let middle = admitted.checked_add(span.checked_div(2)?)?;
        candidate.people.amendment_bytes = middle;
        if codec_admits(&candidate) {
            admitted = middle;
        } else {
            refused = middle;
        }
    }
    Some(admitted)
}

fn codec_admits(limits: &jig_domain_core::Limits) -> bool {
    jig_protocol_store::codec::people::limits(limits).is_some() && jig_protocol_store::codec::limits(limits).is_some()
}

fn binding_bound(edge: &jig_domain_core::Limits, configured: &jig_domain_core::Limits) -> Option<CodecBound> {
    let edge = jig_protocol_store::codec::people::derived_limits(edge)?;
    let problem = jig_protocol_store::people_v13::check_limits(&edge).err()?;
    let configured = jig_protocol_store::codec::people::derived_limits(configured)?;
    Some(people_bound(&configured, problem.path)?.bound)
}

pub(crate) fn codec_path(name: &[u8]) -> Path {
    let path = Path::root().member(b"limits");
    match skein_lib::bytes::find(name, b".") {
        Some(dot) => path
            .member(name.get(..dot).expect("dot is within name"))
            .member(name.get(dot.checked_add(1).expect("name length fits usize")..).expect("dot is within name")),
        None => path.member(name),
    }
}

struct PeopleBound {
    name: &'static [u8],
    bound: CodecBound,
}

// This table translates generated limit paths, never derives bounds. Values and
// ceilings come from the codec's records; their arithmetic has one owner.
// Keep one mapping per line so the translation can be read as a table.
#[rustfmt::skip]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "Path is shared with decoding; non-limit paths must return None without panicking"
)]
fn people_bound(
    limits: &jig_protocol_store::people_v13::Limits,
    path: jig_protocol_store::people_v13::Path,
) -> Option<PeopleBound> {
    use jig_protocol_store::people_v13::{CEILINGS, Path as P};
    macro_rules! b {
        ($name:literal, $words:literal, $field:ident) => {
            Some(PeopleBound {
                name: $name,
                bound: CodecBound { field: $words, needed: limits.$field, ceiling: CEILINGS.$field },
            })
        };
    }
    match path {
        P::SegmentBytes => b!(b"authority.segment_bytes", b"Segment.bytes", segment_bytes),
        P::IdentityKeySubject => b!(b"people.identity_bytes", b"IdentityKey.subject", identity_key_subject),
        P::IdentityLogin => b!(b"people.identity_bytes", b"Identity.login", identity_login),
        P::IdentityName => b!(b"people.identity_bytes", b"Identity.name", identity_name),
        P::LastExactValue => b!(b"authority.segment_bytes", b"LastExact.value", last_exact_value),
        P::LastOpenValue => b!(b"authority.segment_bytes", b"LastOpen.value", last_open_value),
        P::PatternSegments => b!(b"core.policy_bytes", b"Pattern.segments", pattern_segments),
        P::DelegationKinds => b!(b"core.policy_bytes", b"Delegation.kinds", delegation_kinds),
        P::AuthorityGrants => b!(b"core.policy_bytes", b"Authority.grants", authority_grants),
        P::AuthorityNoteResources => b!(b"core.policy_bytes", b"Authority.note_resources", authority_note_resources),
        P::PolicyValueProjections => b!(b"authority.grants", b"PolicyValue.projections", policy_value_projections),
        P::PolicyValueRoles => b!(b"authority.roles", b"PolicyValue.roles", policy_value_roles),
        P::PolicyValueRequirements => b!(b"authority.requirements", b"PolicyValue.requirements", policy_value_requirements),
        P::PolicyValuePermissions => b!(b"core.policy_bytes", b"PolicyValue.permissions", policy_value_permissions),
        P::PolicyValueTemplates => b!(b"core.templates", b"PolicyValue.templates", policy_value_templates),
        P::PolicyChangeRequirementsRequirements => b!(b"people.amendment_bytes", b"PolicyChangeRequirements.requirements", policy_change_requirements_requirements),
        P::PolicyChangeProjectionsGrants => b!(b"people.amendment_bytes", b"PolicyChangeProjections.grants", policy_change_projections_grants),
        P::PolicyChangePermissionsMappings => b!(b"people.amendment_bytes", b"PolicyChangePermissions.mappings", policy_change_permissions_mappings),
        P::PolicyChangeTemplatesOverrides => b!(b"core.templates", b"PolicyChangeTemplates.overrides", policy_change_templates_overrides),
        P::NoteChangeCorrectDescription => b!(b"people.amendment_bytes", b"NoteChangeCorrect.description", note_change_correct_description),
        P::NoteChangeCorrectBody => b!(b"people.amendment_bytes", b"NoteChangeCorrect.body", note_change_correct_body),
        P::NoteChangeCorrectReferences => b!(b"people.amendment_bytes", b"NoteChangeCorrect.references", note_change_correct_references),
        P::ResourceNamePath => b!(b"people.amendment_bytes", b"ResourceName.path", resource_name_path),
        P::AdoptionOptions => b!(b"people.amendment_bytes", b"Adoption.options", adoption_options),
        P::PersonResultApprovedState => b!(b"people.words", b"PersonResultApproved.state", person_result_approved_state),
        P::PersonResultApprovedWords => b!(b"people.words", b"PersonResultApproved.words", person_result_approved_words_some),
        P::PersonResultReportWords => b!(b"people.words", b"PersonResultReport.words", person_result_report_words),
        P::PersonResultVerdictWords => b!(b"people.words", b"PersonResultVerdict.words", person_result_verdict_words),
        P::PersonResultFailureReason => b!(b"people.words", b"PersonResultFailure.reason", person_result_failure_reason),
        P::ProposalDecisionRejectReason => b!(b"people.words", b"ProposalDecisionReject.reason", proposal_decision_reject_reason),
        P::EscalationDecisionRejectReason => b!(b"people.words", b"EscalationDecisionReject.reason", escalation_decision_reject_reason),
        P::ParameterBytesValue => b!(b"people.amendment_bytes", b"ParameterBytes.value", parameter_bytes_value),
        P::SpecWords => b!(b"people.amendment_bytes", b"Spec.words", spec_words),
        P::SpecParameters => b!(b"people.amendment_bytes", b"Spec.parameters", spec_parameters),
        P::SpecInputs => b!(b"people.amendment_bytes", b"Spec.inputs", spec_inputs),
        P::LlmChoiceSubagents => b!(b"people-v13.LlmChoice.subagents", b"LlmChoice.subagents", llm_choice_subagents),
        P::AmendmentInstructions => b!(b"people.amendment_bytes", b"Amendment.instructions", amendment_instructions_some),
        P::AmendmentDependencies => b!(b"people.amendment_bytes", b"Amendment.dependencies", amendment_dependencies_some),
        P::AmendmentReason => b!(b"people.words", b"Amendment.reason", amendment_reason),
        P::FieldRuleName => b!(b"people.amendment_bytes", b"FieldRule.name", field_rule_name),
        P::VerdictFields => b!(b"people.amendment_bytes", b"Verdict.fields", verdict_fields),
        P::ContractVerdictChoices => b!(b"people.amendment_bytes", b"ContractVerdict.choices", contract_verdict_choices),
        P::MemberWords => b!(b"people.words", b"Member.words", member_words),
        P::MemberParameters => b!(b"people.amendment_bytes", b"Member.parameters", member_parameters),
        P::MemberInputs => b!(b"people.amendment_bytes", b"Member.inputs", member_inputs),
        P::MemberDependencies => b!(b"people.amendment_bytes", b"Member.dependencies", member_dependencies),
        P::MemberNamed => b!(b"core.policy_bytes", b"Member.named", member_named),
        P::OperationCancelReason => b!(b"people.words", b"OperationCancel.reason", operation_cancel_reason),
        P::OperationMoveReason => b!(b"people.words", b"OperationMove.reason", operation_move_reason),
        P::OperationMessageWords => b!(b"people.words", b"OperationMessage.words", operation_message_words),
        P::AskCompoundOperations => b!(b"people.operations", b"AskCompound.operations", ask_compound_operations),
        P::AskCreateBatch => b!(b"people.batch", b"AskCreate.batch", ask_create_batch),
        P::AskMakeServiceName => b!(b"people.words", b"AskMakeService.name", ask_make_service_name),
        P::ActionBatchMembers => b!(b"people.batch", b"ActionBatch.members", action_batch_members),
        P::AskProposeReason => b!(b"people.words", b"AskPropose.reason", ask_propose_reason),
        P::AskCancelReason => b!(b"people.words", b"AskCancel.reason", ask_cancel_reason),
        P::AskMoveReason => b!(b"people.words", b"AskMove.reason", ask_move_reason),
        P::AskSayWords => b!(b"people.words", b"AskSay.words", ask_say_words),
        P::AskAnswerQuestionWords => b!(b"people.words", b"AskAnswerQuestion.words", ask_answer_question_words),
        P::AskSetRolesHoldings => b!(b"people.holdings", b"AskSetRoles.holdings", ask_set_roles_holdings),
        P::AskRenameName => b!(b"people-v13.AskRename.name", b"AskRename.name", ask_rename_name),
        P::RefusalFullTasks => b!(b"tasks.circle", b"RefusalFull.tasks", refusal_full_tasks),
        P::AskMakeProjectName => b!(b"people-v13.AskMakeProject.name", b"AskMakeProject.name", ask_make_project_name),
        P::AccountChangeDeclareProgramName => b!(b"people-v13.AccountChangeDeclareProgram.name", b"AccountChangeDeclareProgram.name", account_change_declare_program_name),
        P::AccountChangeAddKeyDigest => b!(b"people-v13.AccountChangeAddKey.digest", b"AccountChangeAddKey.digest", account_change_add_key_digest),
        P::AccountChangeReplaceKeyDigest => b!(b"people-v13.AccountChangeReplaceKey.digest", b"AccountChangeReplaceKey.digest", account_change_replace_key_digest),
        P::AccountChangeSetUsesUses => b!(b"people.words", b"AccountChangeSetUses.uses", account_change_set_uses_uses_some),
        P::AccountChangeMoveInDigest => b!(b"people-v13.AccountChangeMoveIn.digest", b"AccountChangeMoveIn.digest", account_change_move_in_digest),
        P::StoredRolesHoldings => b!(b"people.holdings", b"StoredRoles.holdings", stored_roles_holdings),
        P::ModelNameName => b!(b"people-v13.ModelName.name", b"ModelName.name", model_name_name),
        P::StoredPolicyName => b!(b"people-v13.StoredPolicy.name", b"StoredPolicy.name", stored_policy_name),
        P::StoredPolicyModels => b!(b"people-v13.StoredPolicy.models", b"StoredPolicy.models", stored_policy_models),
        P::RefusalCircleTasks => b!(b"tasks.circle", b"RefusalCircle.tasks", refusal_circle_tasks),
        P::RefusalKeptPending => b!(b"tasks.dependencies", b"RefusalKept.pending", refusal_kept_pending),
        P::OutcomeAccountSignInStartedAddress => b!(b"people-v13.OutcomeAccountSignInStarted.address", b"OutcomeAccountSignInStarted.address", outcome_account_sign_in_started_address),
        P::AccountHostFailureName => b!(b"people.program_host_name_bytes", b"AccountHostFailure.name", account_host_failure_name),
        P::OutcomeAccountProgramDeclaredName => b!(b"people-v13.OutcomeAccountProgramDeclared.name", b"OutcomeAccountProgramDeclared.name", outcome_account_program_declared_name),
        P::OutcomeAccountSignedOutFailedHosts => b!(b"people.program_hosts", b"OutcomeAccountSignedOut.failed_hosts", outcome_account_signed_out_failed_hosts),
        P::OutcomeAccountRemovedFailedHosts => b!(b"people.program_hosts", b"OutcomeAccountRemoved.failed_hosts", outcome_account_removed_failed_hosts),
        P::ActCloseReason => b!(b"people.words", b"ActClose.reason", act_close_reason),
        P::ActMakeProjectName => b!(b"people-v13.ActMakeProject.name", b"ActMakeProject.name", act_make_project_name),
        P::StoredAnswerModels => b!(b"people-v13.StoredAnswer.models", b"StoredAnswer.models", stored_answer_models),
        // Path is shared with decoding; check_limits names only limit paths today.
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_non_limit_people_path_has_no_bound() {
        use jig_protocol_store::people_v13::{CEILINGS, Path};

        assert!(super::people_bound(&CEILINGS, Path::SegmentTail).is_none());
    }
}
