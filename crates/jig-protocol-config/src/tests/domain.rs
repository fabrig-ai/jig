use crate::{What, tests::fixtures};

fn engine() -> crate::Engine {
    fixtures::engine(fixtures::ENGINE_SMALL).expect("whole engine fixture")
}
pub(super) fn core_config(document: &[u8]) -> jig_domain_core::Config {
    let mut engine = fixtures::engine(document).expect("checked engine");
    engine.charters[0].policy.instructions = Box::from(&b"Answer the question."[..]);
    crate::core(&engine, [3; 16], 11)
}
fn refuses(from: &[u8], to: &[u8], what: What) {
    let document = fixtures::with(fixtures::ENGINE_SMALL, from, to);
    let problems = fixtures::engine(&document).expect_err("refused domain configuration");
    assert_eq!(problems.problems.get(0).expect("one problem").what, what);
}
pub(super) fn two_projects(number: u8) -> Box<[u8]> {
    let prefix = br#""projects": ["#;
    let start = skein_lib::bytes::find(fixtures::ENGINE_SMALL, prefix).unwrap().checked_add(prefix.len()).unwrap();
    let end = fixtures::ENGINE_SMALL.len().checked_sub(3).unwrap();
    let project = fixtures::ENGINE_SMALL.get(start..end).unwrap();
    let second = fixtures::with(
        project,
        br#""number": 1"#,
        &[b'"', b'n', b'u', b'm', b'b', b'e', b'r', b'"', b':', b' ', number],
    );
    let mut writer = skein_lib::Writer::new(project.len().checked_add(second.len()).unwrap().checked_add(1).unwrap());
    writer.put(project).unwrap();
    writer.put(b",").unwrap();
    writer.put(&second).unwrap();
    fixtures::with(fixtures::ENGINE_SMALL, project, &writer.finish())
}
const CONFIGURED_PROJECTS: u32 = 2; // two_projects seeds projects 1 and 2.
const DEPLOYMENT_ROSTER: u32 = 1;
const ALL_ROSTERS: u32 = CONFIGURED_PROJECTS + DEPLOYMENT_ROSTER;

fn two_ownerless_projects(people_projects: u32) -> Box<[u8]> {
    let document = fixtures::with(
        &two_projects(b'2'),
        br#""owners": [{"provider": 1, "subject": "owner-a"}]"#,
        br#""owners": []"#,
    );
    let limits = fixtures::with(
        br#""profile": "small", "overrides": {"people.projects": CAP}"#,
        b"CAP",
        skein_lib::Decimal::of(u64::from(people_projects)).as_bytes(),
    );
    fixtures::with(&document, br#""profile": "small""#, &limits)
}
#[test]
fn configured_projects_above_the_overridden_people_capacity_are_refused() {
    let problems = fixtures::engine(&two_ownerless_projects(ALL_ROSTERS - DEPLOYMENT_ROSTER))
        .expect_err("three rosters exceed capacity two");
    assert_eq!(
        problems.problems.as_slice(),
        &[crate::Problem { path: crate::Path::root().member(b"projects"), what: What::Bound }]
    );
}
#[test]
fn configured_projects_at_or_below_the_overridden_people_capacity_parse() {
    for capacity in [ALL_ROSTERS, ALL_ROSTERS + DEPLOYMENT_ROSTER] {
        let engine =
            fixtures::engine(&two_ownerless_projects(capacity)).expect("two projects and the deployment roster fit");
        assert_eq!(engine.limits.core.people.projects, capacity);
        assert_eq!(engine.projects.len(), 2);
        assert_eq!([engine.projects[0].number, engine.projects[1].number], [1, 2]);
        assert!(engine.owners.is_empty(), "ownerless projects still need rosters");
    }
}
#[test]
fn a_configured_project_numbered_zero_is_refused() {
    let document =
        fixtures::with(fixtures::ENGINE_SMALL, br#""projects": [{"number": 1"#, br#""projects": [{"number": 0"#);
    let problems = fixtures::engine(&document).expect_err("zero is the deployment roster");
    assert_eq!(
        problems.problems.as_slice(),
        &[crate::Problem {
            path: fixtures::path(&[b"projects"]).child(crate::Segment::Index(0)).member(b"number"),
            what: What::Bound,
        }]
    );
}

fn with_grants(connector: u8) -> Box<[u8]> {
    let grant = br#""grants": [{"connector": 1, "kind": 2, "pattern": {"segments": ["service"], "last": "open", "value": ""}}]"#;
    let grant = fixtures::with(
        grant,
        br#""connector": 1"#,
        &[b'"', b'c', b'o', b'n', b'n', b'e', b'c', b't', b'o', b'r', b'"', b':', b' ', connector],
    );
    let mut document = fixtures::with(fixtures::ENGINE_SMALL, br#""grants": []"#, &grant);
    for _ in 0_u32..2 {
        document = fixtures::with(&document, br#""grants": []"#, &grant);
    }
    document
}
#[test]
fn the_small_engine_document_parses_whole() {
    let engine = engine();
    assert_eq!(engine.deployment.directory.as_ref(), b"state");
    assert_eq!(engine.deployment.seed, Some(7));
    assert_eq!(engine.providers.len(), 2);
}

#[test]
fn operations_below_delegates_plus_one_refuse_the_start() {
    refuses(
        br#""profile": "small""#,
        br#""profile": "small", "overrides": {"people.operations": 4, "tasks.project_goals": 4}"#,
        What::Operations { operations: 4, delegates: 4, goals: 4 },
    );
}

#[test]
fn operations_below_goals_per_project_refuse_the_start() {
    refuses(
        br#""profile": "small""#,
        br#""profile": "small", "overrides": {"people.operations": 7}"#,
        What::Operations { operations: 7, delegates: 4, goals: 8 },
    );
}

#[test]
fn the_shipped_profiles_hold_operations_per_request() {
    for (profile, expected) in [(crate::Profile::Small, (8, 4, 8)), (crate::Profile::Standard, (16, 8, 16))] {
        let limits = crate::profile(profile).core;
        assert_eq!((limits.people.operations, limits.tasks.delegates, limits.tasks.project_goals), expected);
        assert!(limits.people.operations > limits.tasks.delegates, "{profile:?}");
        assert!(limits.people.operations >= limits.tasks.project_goals, "{profile:?}");
    }
}
#[test]
fn the_cores_config_takes_the_charters_and_projects() {
    let core = core_config(fixtures::ENGINE_SMALL);
    assert_eq!(core.defaults.charters.len(), 1);
    assert_eq!(core.seed_projects.len(), 1);
    assert_eq!(core.seed_projects[0].number, 1);
    assert_eq!(core.defaults.charters[0].policy.instructions.as_ref(), b"Answer the question.");
}
#[test]
fn the_cores_policy_and_permissions_are_seeded_per_project() {
    let core = core_config(&two_projects(b'2'));
    assert_eq!(core.seed_projects[1].policy.policy.period_spend, 100);
    assert_eq!(
        core.seed_projects[1].policy.permissions.as_ref(),
        &[jig_domain_people::PermissionRole { connector: 1, permission: 1, role: 1 }]
    );
}
#[test]
fn the_cores_settings_take_the_deployment_provider() {
    let core = core_config(fixtures::ENGINE_SMALL);
    assert_eq!(core.settings.deployment_provider, 2);
}
#[test]
fn the_cores_limits_and_tools_take_the_profile_and_jigs_numbering() {
    let core = core_config(fixtures::ENGINE_SMALL);
    assert_eq!(core.settings.resume_bytes, 65536);
    assert_eq!(core.settings.tail_bytes, 2048);
    assert_eq!(core.hub_turns, 8);
    assert_eq!(core.settings.tools.message, jig_domain_authority::Tools(2));
}
#[test]
fn an_in_process_listener_tells_the_core_workers_share_its_process() {
    assert!(core_config(fixtures::ENGINE_SMALL).workers_in_process);
}
#[test]
fn a_socket_listener_tells_the_core_workers_have_their_own_process() {
    let document =
        fixtures::with(fixtures::ENGINE_SMALL, br#"{"in_process": "workers"}"#, br#"{"socket":"127.0.0.1:7102"}"#);
    assert!(!core_config(&document).workers_in_process);
}
#[test]
fn a_configured_seed_overrides_the_shells_draw() {
    assert_eq!(core_config(fixtures::ENGINE_SMALL).seed, 7);
}
#[test]
fn a_shell_seed_is_used_when_configuration_has_no_seed() {
    let document = fixtures::with(fixtures::ENGINE_SMALL, br#", "seed": 7"#, b"");
    assert_eq!(core_config(&document).seed, 11);
}
#[test]
fn a_project_numbered_twice_is_refused() {
    let problems = fixtures::engine(&two_projects(b'1')).expect_err("duplicate project");
    assert_eq!(problems.problems.get(0).unwrap().what, What::Unique);
}
fn an_owner_naming_an_unknown_provider_is_refused() {
    refuses(br#""provider": 1"#, br#""provider": 3"#, What::Refers);
}
fn a_message_bound_below_a_partys_words_is_refused() {
    refuses(br#""profile": "small""#, br#""profile": "small", "overrides": {"people.words": 4097}"#, What::Message);
}
fn a_retention_member_is_refused_as_unknown() {
    refuses(br#""directory": "state""#, br#""directory": "state", "retention": {}"#, What::Unknown);
}
fn a_projects_resources_member_is_refused_as_unknown() {
    refuses(br#""projects": [{"number": 1"#, br#""projects": [{"number": 1, "resources": []"#, What::Unknown);
}
fn a_projects_homes_member_is_refused_as_unknown() {
    refuses(br#""projects": [{"number": 1"#, br#""projects": [{"number": 1, "homes": []"#, What::Unknown);
}
fn two_deployment_providers_are_refused() {
    refuses(br#""kind": "link""#, br#""kind": "deployment""#, What::Unique);
}
fn a_deployment_provider_is_required() {
    refuses(br#""kind": "deployment""#, br#""kind": "application""#, What::Missing);
}
fn a_connector_numbered_as_the_cores_own_is_refused() {
    refuses(br#""connectors": [1]"#, br#""connectors": [65535]"#, What::Bound);
}
#[test]
fn a_ceiling_without_models_takes_every_configured_model() {
    let engine = engine();
    assert_eq!(engine.rules.ceiling.models, jig_domain_authority::Models::ALL);
    assert_eq!(engine.projects[0].policy.ceiling.models, jig_domain_authority::Models::ALL);
}
#[test]
fn a_ceilings_models_name_the_configured_models_it_keeps() {
    let ceiling = br#""ceiling": {"tools": 1023,"#;
    let named = fixtures::engine(&fixtures::with(
        fixtures::ENGINE_SMALL,
        ceiling,
        br#""ceiling": {"tools": 1023, "models": ["small"],"#,
    ));
    assert_eq!(named.unwrap().rules.ceiling.models, jig_domain_authority::Models::Named(0b1));
    let none = fixtures::engine(&fixtures::with(
        fixtures::ENGINE_SMALL,
        ceiling,
        br#""ceiling": {"tools": 1023, "models": [],"#,
    ));
    assert_eq!(none.unwrap().rules.ceiling.models, jig_domain_authority::Models::NONE);
}
fn a_ceiling_naming_an_unknown_model_is_refused() {
    fixtures::refuses(
        br#""ceiling": {"tools": 1023,"#,
        br#""ceiling": {"tools": 1023, "models": ["large"],"#,
        fixtures::path(&[b"rules", b"ceiling", b"models"]).child(crate::Segment::Index(0)),
        What::Refers,
    );
}
fn a_ceiling_naming_a_model_twice_is_refused() {
    fixtures::refuses(
        br#""ceiling": {"tools": 1023,"#,
        br#""ceiling": {"tools": 1023, "models": ["small", "small"],"#,
        fixtures::path(&[b"rules", b"ceiling", b"models"]).child(crate::Segment::Index(1)),
        What::Unique,
    );
}
const ROLE: &[u8] = br#""authority": {"tools": 1023,"#;
const NAMED: &[u8] = br#""authority": {"tools": 1023, "models": "#;
fn role_models(models: &[u8]) -> Result<crate::Engine, crate::Problems> {
    let mut writer = skein_lib::Writer::new(NAMED.len().checked_add(models.len()).unwrap().checked_add(1).unwrap());
    writer.put(NAMED).unwrap();
    writer.put(models).unwrap();
    writer.put(b",").unwrap();
    fixtures::engine(&fixtures::with(fixtures::ENGINE_SMALL, ROLE, &writer.finish()))
}
fn role(engine: &crate::Engine) -> jig_domain_authority::Models {
    engine.projects[0].policy.roles[0].authority.models
}
fn role_models_path(index: usize) -> crate::Path {
    fixtures::path(&[b"projects"])
        .child(crate::Segment::Index(0))
        .member(b"policy")
        .member(b"roles")
        .child(crate::Segment::Index(0))
        .member(b"authority")
        .member(b"models")
        .child(crate::Segment::Index(u32::try_from(index).unwrap()))
}
#[test]
fn a_roles_models_name_the_configured_models_it_keeps() {
    assert_eq!(role(&engine()), jig_domain_authority::Models::ALL, "absent names every configured model");
    assert_eq!(role(&role_models(br#"["small"]"#).unwrap()), jig_domain_authority::Models::Named(0b1));
    assert_eq!(role(&role_models(b"[]").unwrap()), jig_domain_authority::Models::NONE);
}
fn a_role_naming_an_unknown_model_is_refused() {
    let problems = role_models(br#"["large"]"#).unwrap_err();
    assert_eq!(problems.problems.get(0), Some(&crate::Problem { path: role_models_path(0), what: What::Refers }));
}
fn a_role_naming_a_model_twice_is_refused() {
    let problems = role_models(br#"["small", "small"]"#).unwrap_err();
    assert_eq!(problems.problems.get(0), Some(&crate::Problem { path: role_models_path(1), what: What::Unique }));
}
#[test]
fn each_owner_owns_every_seeded_project() {
    let engine = fixtures::engine(&two_projects(b'2')).unwrap();
    assert_eq!(engine.owners.len(), 2);
    assert_eq!(engine.owners[0].project, 1);
    assert_eq!(engine.owners[1].project, 2);
    assert_eq!(engine.owners[0].identity.subject.as_ref(), b"owner-a");
    assert_eq!(engine.owners[1].identity.subject.as_ref(), b"owner-a");
}
fn a_permission_naming_an_unknown_role_is_refused() {
    refuses(br#""permission": 1, "role": 1"#, br#""permission": 1, "role": 9"#, What::Refers);
}
fn a_project_policy_past_the_deployment_ceiling_is_refused() {
    refuses(
        br#""escalation_role": 1, "period_spend": 100"#,
        br#""escalation_role": 1, "period_spend": 101"#,
        What::Bound,
    );
}
#[test]
fn charter_instruction_files_are_named_with_the_available_policy_bytes() {
    let engine = engine();
    let files = crate::files(&engine);
    let file = instruction_file(&files);
    assert!(!file.private);
    assert_eq!(file.at, fixtures::path(&[b"charters"]).child(crate::Segment::Index(0)).member(b"instructions"));
    assert!(file.max < engine.limits.core.run_bytes);
}
fn a_message_bound_below_a_delegate_result_is_refused() {
    refuses(
        br#""profile": "small""#,
        br#""profile": "small", "overrides": {"tasks.result_bytes": 4097}"#,
        What::Message,
    );
}
#[test]
fn an_engines_memory_bytes_is_handed_on_unchecked() {
    let document = fixtures::with(fixtures::ENGINE_SMALL, br#""memory_bytes": 34359738368"#, br#""memory_bytes": 1"#);
    assert_eq!(fixtures::engine(&document).unwrap().memory, 1);
}
#[test]
fn an_engine_without_memory_bytes_is_refused() {
    let document = fixtures::with(fixtures::ENGINE_SMALL, br#""memory_bytes": 34359738368,"#, b"");
    let problems = fixtures::engine(&document).unwrap_err();
    assert_eq!(problems.problems.len(), 1);
    let problem = problems.problems.get(0).unwrap();
    assert_eq!(problem.path, fixtures::path(&[b"memory_bytes"]));
    assert_eq!(problem.what, What::Missing);
}
fn instruction_file(files: &[crate::File]) -> &crate::File {
    for file in files {
        if file.path.as_ref() == b"charters/chat.txt" {
            return file;
        }
    }
    panic!("instruction file configured")
}
#[test]
fn resource_grants_read_their_literal_pattern() {
    let engine = fixtures::engine(&with_grants(b'1')).unwrap();
    assert_eq!(engine.rules.ceiling.grants[0].pattern.segments.as_ref(), &[Box::<[u8]>::from(&b"service"[..])]);
    assert_eq!(engine.rules.ceiling.grants[0].pattern.last, jig_domain_authority::Last::Open(Box::default()));
}
#[test]
fn a_grant_naming_an_unknown_connector_is_refused() {
    let problems = fixtures::engine(&with_grants(b'2')).unwrap_err();
    assert_eq!(problems.problems.get(0).unwrap().what, What::Refers);
}
#[test]
fn a_requirement_reads_its_judge_and_freshness() {
    let requirement = br#""requirements": [{"connector": 1,"kind":2,"pattern":{"segments":[],"last":"open","value":""},"judge":{"connector":1,"requirement":3,"parameters":4},"guard":"observed","freshness_ms":5000,"must_be_guarded":false}]"#;
    let document = fixtures::with(fixtures::ENGINE_SMALL, br#""requirements": []"#, requirement);
    let engine = fixtures::engine(&document).unwrap();
    assert_eq!(engine.rules.requirements[0].judge.parameters, 4);
    assert_eq!(
        engine.rules.requirements[0].guard,
        jig_domain_authority::Guard::Observed { freshness: skein_lib::Duration::from_secs(5) }
    );
}
fn an_implication_table_not_transitively_closed_is_refused() {
    refuses(
        br#""implies": []"#,
        br#""implies": [{"connector":1,"kind":1,"implies":2},{"connector":1,"kind":2,"implies":3}]"#,
        What::Bound,
    );
}

#[test]
fn domain_configuration_problems_name_the_invalid_fields() {
    table([
        ("an_owner_naming_an_unknown_provider_is_refused", an_owner_naming_an_unknown_provider_is_refused),
        ("a_message_bound_below_a_partys_words_is_refused", a_message_bound_below_a_partys_words_is_refused),
        ("a_retention_member_is_refused_as_unknown", a_retention_member_is_refused_as_unknown),
        ("a_projects_resources_member_is_refused_as_unknown", a_projects_resources_member_is_refused_as_unknown),
        ("a_projects_homes_member_is_refused_as_unknown", a_projects_homes_member_is_refused_as_unknown),
        ("two_deployment_providers_are_refused", two_deployment_providers_are_refused),
        ("a_deployment_provider_is_required", a_deployment_provider_is_required),
        ("a_connector_numbered_as_the_cores_own_is_refused", a_connector_numbered_as_the_cores_own_is_refused),
        ("a_permission_naming_an_unknown_role_is_refused", a_permission_naming_an_unknown_role_is_refused),
        (
            "a_project_policy_past_the_deployment_ceiling_is_refused",
            a_project_policy_past_the_deployment_ceiling_is_refused,
        ),
        ("a_message_bound_below_a_delegate_result_is_refused", a_message_bound_below_a_delegate_result_is_refused),
        (
            "an_implication_table_not_transitively_closed_is_refused",
            an_implication_table_not_transitively_closed_is_refused,
        ),
        ("a_ceiling_naming_an_unknown_model_is_refused", a_ceiling_naming_an_unknown_model_is_refused),
        ("a_ceiling_naming_a_model_twice_is_refused", a_ceiling_naming_a_model_twice_is_refused),
        ("a_role_naming_an_unknown_model_is_refused", a_role_naming_an_unknown_model_is_refused),
        ("a_role_naming_a_model_twice_is_refused", a_role_naming_a_model_twice_is_refused),
    ]);
}

type Case = (&'static str, fn());

fn table<const N: usize>(cases: [Case; N]) {
    let mut failures = [None; N];
    let mut passed = true;
    for ((name, case), failure) in cases.into_iter().zip(&mut failures) {
        if std::panic::catch_unwind(case).is_err() {
            *failure = Some(name);
            passed = false;
        }
    }
    assert!(passed, "{failures:?}");
}

#[test]
fn the_catalogue_key_bound_follows_the_component() {
    let mut engine = engine();
    engine.limits.driver.client.document.token_bytes = 123;
    let core = crate::core(&engine, [0; 16], 1);
    assert_eq!(core.settings.key_bytes, engine.limits.driver.client.document.token_bytes);
}
#[test]
fn the_larger_carried_value_bound_keeps_the_catalogue_key_bound() {
    let engine = engine();
    let core = crate::core(&engine, [0; 16], 1);
    assert_eq!(core.settings.key_bytes, engine.limits.driver.client.document.token_bytes);
    assert!(core.settings.key_bytes < engine.limits.clients.wire.keyed_value_some);
}

#[test]
fn a_seed_policy_whose_whole_value_exceeds_its_byte_limit_refuses_start() {
    // Permission mappings fit 256 bytes, but the role and whole ceiling do not.
    refuses(br#""profile": "small""#, br#""profile": "small", "overrides": {"core.policy_bytes": 256}"#, What::Bound);
}

// Current people-v13 envelope, configuration.md, section 6.
const POLICY_MAX: u64 = 43_695;
const AMENDMENT_MAX: u32 = 8_192;
const LARGE_POLICY: u64 = 65_536;

#[test]
fn shipped_profiles_fit_the_effective_codec_envelope() {
    for profile in [crate::Profile::Small, crate::Profile::Standard] {
        let limits = crate::profile(profile).core;
        assert_eq!(crate::check_effective_limits(&limits), Ok(()), "{profile:?}");
        let derived = jig_protocol_store::codec::people::derived_limits(&limits).unwrap();
        assert_eq!(jig_protocol_store::people_v13::check_limits(&derived), Ok(()));
    }
}

#[test]
fn exported_maxima_are_the_last_codec_admitted_values() {
    for profile in [crate::Profile::Small, crate::Profile::Standard] {
        let limits = crate::profile(profile).core;
        assert_eq!(crate::policy_bytes_max(&limits), Some(POLICY_MAX), "{profile:?}");
        assert_eq!(crate::amendment_bytes_max(&limits), Some(AMENDMENT_MAX), "{profile:?}");
        let mut policy = limits;
        policy.policy_bytes = POLICY_MAX;
        assert_eq!(crate::check_effective_limits(&policy), Ok(()));
        assert!(jig_protocol_store::codec::limits(&policy).is_some());
        let mut amendment = limits;
        amendment.people.amendment_bytes = AMENDMENT_MAX;
        assert_eq!(crate::check_effective_limits(&amendment), Ok(()));
        assert!(jig_protocol_store::codec::limits(&amendment).is_some());
    }
}

fn policy_problem(value: u64) -> crate::Problem {
    let mut limits = crate::profile(crate::Profile::Small).core;
    limits.policy_bytes = value;
    crate::check_effective_limits(&limits).expect_err("policy exceeds the codec envelope")
}

#[test]
fn policy_past_the_envelope_names_its_binding_field() {
    let problem = policy_problem(LARGE_POLICY);
    assert_eq!(problem.path, fixtures::path(&[b"limits", b"core", b"policy_bytes"]));
    assert_eq!(
        problem.what,
        What::PolicyEnvelope {
            value: LARGE_POLICY,
            maximum: POLICY_MAX,
            bound: Some(crate::CodecBound { field: b"PolicyValue.permissions", needed: 98_304, ceiling: 65_536 }),
        }
    );
    assert_eq!(problem.codec_message().unwrap().as_ref(), b"limits.core.policy_bytes: 65536 exceeds the people-v13 codec envelope of 43695 bytes (PolicyValue.permissions would need 98304 bytes; ceiling 65536).");
}

#[test]
fn the_next_policy_byte_is_refused_at_the_binding_field() {
    let problem = policy_problem(POLICY_MAX + 1);
    assert_eq!(
        problem.what,
        What::PolicyEnvelope {
            value: POLICY_MAX + 1,
            maximum: POLICY_MAX,
            bound: Some(crate::CodecBound { field: b"PolicyValue.permissions", needed: 65_544, ceiling: 65_536 }),
        }
    );
}

#[test]
fn the_next_amendment_byte_is_refused_with_the_agreed_words() {
    let mut limits = crate::profile(crate::Profile::Small).core;
    limits.people.amendment_bytes = AMENDMENT_MAX + 1;
    let problem = crate::check_effective_limits(&limits).unwrap_err();
    assert_eq!(problem.path, fixtures::path(&[b"limits", b"people", b"amendment_bytes"]));
    assert_eq!(
        problem.what,
        What::AmendmentEnvelope {
            value: AMENDMENT_MAX + 1,
            maximum: AMENDMENT_MAX,
            bound: Some(crate::CodecBound { field: b"Segment.bytes", needed: 8_193, ceiling: 8_192 }),
        }
    );
    assert_eq!(
        problem.codec_message().unwrap().as_ref(),
        b"limits.people.amendment_bytes: 8193 exceeds the people-v13 codec envelope of 8192 bytes."
    );
}

#[test]
fn an_independent_ceiling_names_the_generated_first_failure() {
    let mut limits = crate::profile(crate::Profile::Small).core;
    limits.people.identity_bytes = 8_193;
    let problem = crate::check_effective_limits(&limits).unwrap_err();
    assert_eq!(problem.path, fixtures::path(&[b"limits", b"people", b"identity_bytes"]));
    assert_eq!(
        problem.what,
        What::CodecCeiling {
            bound: crate::CodecBound { field: b"IdentityKey.subject", needed: 8_193, ceiling: 8_192 },
        }
    );
    assert_eq!(crate::policy_bytes_max(&limits), None);
    assert_eq!(crate::amendment_bytes_max(&limits), None);
    assert_eq!(
        problem.codec_message().unwrap().as_ref(),
        b"limits.people.identity_bytes: IdentityKey.subject would need 8193; people-v13 codec ceiling 8192."
    );
}

#[test]
fn arithmetic_endpoints_are_refused_without_panicking() {
    let mut limits = crate::profile(crate::Profile::Small).core;
    limits.policy_bytes = u64::MAX;
    let problem = crate::check_effective_limits(&limits).unwrap_err();
    assert_eq!(problem.path, fixtures::path(&[b"limits", b"core", b"policy_bytes"]));
    assert_eq!(problem.what, What::PolicyEnvelope { value: u64::MAX, maximum: POLICY_MAX, bound: None });
    assert!(jig_protocol_store::codec::people::derived_limits(&limits).is_none());
    limits.policy_bytes = 16_384;
    limits.people.amendment_bytes = u32::MAX;
    let problem = crate::check_effective_limits(&limits).unwrap_err();
    assert_eq!(problem.path, fixtures::path(&[b"limits", b"people", b"amendment_bytes"]));
    assert_eq!(problem.what, What::AmendmentEnvelope { value: u32::MAX, maximum: AMENDMENT_MAX, bound: None });
}

#[test]
fn maxima_hold_the_other_budget_fixed_even_when_it_is_refused() {
    let mut limits = crate::profile(crate::Profile::Small).core;
    limits.people.amendment_bytes = AMENDMENT_MAX + 1;
    assert_eq!(crate::policy_bytes_max(&limits), None);
    limits.people.amendment_bytes = AMENDMENT_MAX;
    limits.policy_bytes = POLICY_MAX + 1;
    assert_eq!(crate::amendment_bytes_max(&limits), None);
}

#[test]
fn two_excessive_budgets_report_policy_then_amendment() {
    let mut limits = crate::profile(crate::Profile::Small).core;
    limits.policy_bytes = LARGE_POLICY;
    limits.people.amendment_bytes = AMENDMENT_MAX + 1;
    let policy = crate::check_effective_limits(&limits).unwrap_err();
    assert_eq!(policy, policy_problem(LARGE_POLICY));
    limits.policy_bytes = POLICY_MAX;
    let amendment = crate::check_effective_limits(&limits).unwrap_err();
    assert_eq!(amendment.path, fixtures::path(&[b"limits", b"people", b"amendment_bytes"]));
}

#[test]
fn other_store_families_are_checked_after_the_people_probe() {
    let mut limits = crate::profile(crate::Profile::Small).core;
    // The core-v6 SettledAnswer bytes ceiling is independent of the people codec.
    limits.call_answer_bytes = u32::MAX;
    assert!(jig_protocol_store::codec::people::limits(&limits).is_some());
    assert_eq!(
        crate::check_effective_limits(&limits),
        Err(crate::Problem { path: fixtures::path(&[b"limits"]), what: What::StoreCodec })
    );
}

#[test]
fn independent_item_ceilings_keep_their_own_field_and_bound() {
    let mut limits = crate::profile(crate::Profile::Small).core;
    limits.authority.roles = 9;
    let problem = crate::check_effective_limits(&limits).unwrap_err();
    assert_eq!(problem.path, fixtures::path(&[b"limits", b"authority", b"roles"]));
    assert_eq!(
        problem.what,
        What::CodecCeiling { bound: crate::CodecBound { field: b"PolicyValue.roles", needed: 9, ceiling: 8 } }
    );
    assert_eq!(
        problem.codec_message().unwrap().as_ref(),
        b"limits.authority.roles: PolicyValue.roles would need 9; people-v13 codec ceiling 8."
    );
}
