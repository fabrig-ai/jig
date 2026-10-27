#![expect(clippy::disallowed_macros, reason = "tests are ordinary Rust (programming-model.md, 10.2)")]
use super::fixtures::{document, path};
use crate::{Limits, Problems, Profile, What, overrides, profile};

fn small() -> Limits {
    profile(Profile::Small)
}
fn parse(bytes: &[u8]) -> Result<Limits, Problems> {
    overrides::parse(&document(bytes), 0, &path(&[b"limits"]))
}
fn problem(bytes: &[u8], segments: &[&[u8]], what: What) {
    let errors = parse(bytes).unwrap_err();
    assert_eq!(errors.problems.len(), 1);
    assert_eq!(errors.problems.get(0).unwrap().path, path(segments));
    assert_eq!(errors.problems.get(0).unwrap().what, what);
}

#[test]
fn a_store_budget_override_sets_the_budget_and_what_follows_from_it() {
    const BUDGET: u64 = 16_777_216;
    for profile_name in [Profile::Small, Profile::Standard] {
        for owner in ["store", "secrets"] {
            let original = profile(profile_name);
            let mut expected = original;
            let store = if owner == "store" { &mut expected.store.store } else { &mut expected.store.secrets };
            *store = profile::store_budget(*store, BUDGET).unwrap();
            assert_eq!(store.budget, BUDGET);
            assert_eq!(u64::from(store.commit), BUDGET + u64::from(store.ops) * (u64::from(store.key) + 9) + 26);
            assert_eq!(store.queued_bytes, u64::from(store.commit) * u64::from(store.queued));
            assert_eq!(store.segment, u64::from(store.commit) * 2);
            assert_eq!(store.snapshot_after, u64::from(store.commit));
            let name = if profile_name == Profile::Small { "small" } else { "standard" };
            let configuration = format!(r#"{{"profile":"{name}","overrides":{{"{owner}.budget":{BUDGET}}}}}"#);
            assert_eq!(parse(configuration.as_bytes()), Ok(expected));
        }
    }
}

#[test]
fn the_profiles_derive_their_store_limits_by_the_same_function() {
    for profile_name in [Profile::Small, Profile::Standard] {
        let limits = profile(profile_name);
        for store in [limits.store.store, limits.store.secrets] {
            // Clear the derived fields so retaining stale constants cannot pass.
            let base = skein_kv::Limits { commit: 0, queued_bytes: 0, segment: 0, snapshot_after: 0, ..store };
            assert_eq!(profile::store_budget(base, store.budget), Some(store));
        }
    }
}

#[test]
fn a_store_budget_too_small_for_the_profile_is_refused_naming_it() {
    for profile_name in [Profile::Small, Profile::Standard] {
        let limits = profile(profile_name);
        for (owner, store) in [("store", limits.store.store), ("secrets", limits.store.secrets)] {
            let name = if profile_name == Profile::Small { "small" } else { "standard" };
            let minimum = u64::from(store.value);
            let admitted = format!(r#"{{"profile":"{name}","overrides":{{"{owner}.budget":{minimum}}}}}"#);
            let found = parse(admitted.as_bytes()).expect("one largest value fits at the bound");
            assert_eq!(if owner == "store" { found.store.store.budget } else { found.store.secrets.budget }, minimum);
            for budget in [0, minimum - 1] {
                let refused = format!(r#"{{"profile":"{name}","overrides":{{"{owner}.budget":{budget}}}}}"#);
                let field = format!("{owner}.budget");
                problem(refused.as_bytes(), &[b"limits", b"overrides", field.as_bytes()], What::Override);
            }
        }
    }
}

#[test]
fn store_budget_overrides_refuse_unknown_fields_and_unrepresentable_commits() {
    for owner in ["store", "secrets"] {
        for field in ["absent", "commit", "queued_bytes", "segment", "snapshot_after", "value"] {
            let name = format!("{owner}.{field}");
            assert_eq!(overrides::apply(&mut small(), name.as_bytes(), 1), Err(What::Override));
        }
        for budget in [u64::MAX, u64::from(u32::MAX)] {
            let configuration = format!(r#"{{"profile":"small","overrides":{{"{owner}.budget":{budget}}}}}"#);
            let field = format!("{owner}.budget");
            refused(configuration.as_bytes(), field.as_bytes());
        }
    }
}

#[test]
fn the_composed_reader_checks_both_store_budget_overrides_before_use() {
    use super::fixtures::{ENGINE_SMALL, engine, with};
    for owner in ["store", "secrets"] {
        let minimum = small().store.store.value;
        for budget in [minimum, minimum - 1] {
            let replacement = format!(r#"{{"profile":"small","overrides":{{"{owner}.budget":{budget}}}}}"#);
            let configuration = with(ENGINE_SMALL, br#"{"profile": "small"}"#, replacement.as_bytes());
            let result = engine(&configuration);
            if budget == minimum {
                let found = result.expect("composed reader admits the boundary budget").limits;
                assert_eq!(
                    if owner == "store" { found.store.store.budget } else { found.store.secrets.budget },
                    u64::from(minimum)
                );
            } else {
                let errors = result.unwrap_err();
                let field = format!("{owner}.budget");
                assert_eq!(errors.problems.len(), 1);
                assert_eq!(errors.problems.get(0).unwrap().path, path(&[b"limits", b"overrides", field.as_bytes()]));
                assert_eq!(errors.problems.get(0).unwrap().what, What::Override);
            }
        }
    }
}

#[test]
fn an_override_replaces_one_limit_of_the_profile() {
    let mut expected = small();
    expected.core.tasks.inbox_messages = 9;
    assert_eq!(parse(br#"{"profile":"small","overrides":{"tasks.inbox_messages":9}}"#), Ok(expected));
}
fn an_override_of_an_unknown_name_is_refused() {
    assert_eq!(overrides::apply(&mut small(), b"tasks.absent", 1), Err(What::Override));
}
fn an_override_past_its_type_is_refused() {
    assert_eq!(overrides::apply(&mut small(), b"hub.slots", 4_294_967_296), Err(What::Override));
}
fn an_override_that_breaks_a_worst_case_is_refused_naming_it() {
    problem(
        br#"{"profile":"small","overrides":{"tasks.tasks":0}}"#,
        &[b"limits", b"overrides", b"tasks.tasks"],
        What::Override,
    );
}
#[test]
fn overrides_past_a_familys_bound_are_each_reported_once() {
    let errors =
        parse(br#"{"profile":"small","overrides":{"tasks.tasks":0,"tasks.inbox_messages":1,"tasks.run_inbox":1}}"#)
            .unwrap_err();
    assert_eq!(errors.problems.len(), 3);
    assert_eq!(errors.past, 0);
    for (index, name) in
        [b"tasks.tasks".as_slice(), b"tasks.inbox_messages", b"tasks.run_inbox"].into_iter().enumerate()
    {
        let found = errors.problems.get(u32::try_from(index).unwrap()).unwrap();
        assert_eq!(found.what, What::Override);
        assert_eq!(found.path, path(&[b"limits", b"overrides", name]));
    }
}

#[test]
fn every_limit_of_the_small_profile_has_a_name() {
    for &(name, value) in SMALL {
        let mut limits = small();
        assert_eq!(overrides::apply(&mut limits, name, value), Ok(()), "{name:?}");
        assert_eq!(limits, small(), "{name:?}");
    }
}
#[test]
fn duration_overrides_read_as_milliseconds() {
    let mut expected = small();
    expected.link.ping = skein_lib::Duration::from_millis(6_000);
    assert_eq!(parse(br#"{"profile":"small","overrides":{"link.ping_ms":6000}}"#), Ok(expected));
}
fn a_duration_override_past_its_type_is_refused() {
    assert_eq!(overrides::apply(&mut small(), b"link.ping_ms", u64::MAX), Err(What::Override));
}
#[test]
fn a_limit_with_a_u64_type_keeps_its_width() {
    let mut limits = small();
    assert_eq!(overrides::apply(&mut limits, b"core.fill_mark", u64::MAX), Ok(()));
    assert_eq!(limits.core.fill_mark, u64::MAX);
}
fn an_unknown_profile_is_refused_naming_it() {
    problem(br#"{"profile":"other"}"#, &[b"limits", b"profile"], What::Override);
}
#[test]
fn the_standard_profile_can_be_selected() {
    assert_eq!(parse(br#"{"profile":"standard"}"#), Ok(profile(Profile::Standard)));
}
fn a_missing_profile_is_refused_naming_it() {
    problem(b"{}", &[b"limits", b"profile"], What::Missing);
}
fn an_unknown_limits_member_is_refused_naming_it() {
    problem(br#"{"profile":"small","absent":1}"#, &[b"limits", b"absent"], What::Unknown);
}
fn an_override_named_twice_is_refused_naming_it() {
    problem(
        br#"{"profile":"small","overrides":{"hub.slots":2,"hub.slots":3}}"#,
        &[b"limits", b"overrides", b"hub.slots"],
        What::Duplicate,
    );
}
#[test]
fn overrides_are_checked_together() {
    let mut expected = small();
    expected.core.tasks.run_inbox = 1;
    expected.core.tasks.inbox_messages = 1;
    expected.core.people.inbox_entries = 1;
    assert_eq!(
        parse(br#"{"profile":"small","overrides":{"tasks.inbox_messages":1,"tasks.run_inbox":1,"people.inbox_entries":1}}"#),
        Ok(expected)
    );
}
fn a_non_integer_override_is_refused_naming_it() {
    problem(
        br#"{"profile":"small","overrides":{"hub.slots":{}}}"#,
        &[b"limits", b"overrides", b"hub.slots"],
        What::Override,
    );
}

fn a_whole_compound_limit_is_refused() {
    assert_eq!(overrides::apply(&mut small(), b"tasks.retries", 1), Err(What::Override));
}
#[test]
fn a_compound_member_override_replaces_only_that_member() {
    let mut expected = small();
    expected.core.tasks.retries.transient.retries = 4;
    assert_eq!(parse(br#"{"profile":"small","overrides":{"tasks.retries.transient.retries":4}}"#), Ok(expected));
}
fn an_invalid_compound_member_is_refused_naming_it() {
    problem(
        br#"{"profile":"small","overrides":{"documents.versions.minimum":2}}"#,
        &[b"limits", b"overrides", b"documents.versions.minimum"],
        What::Override,
    );
}

const SMALL: &[(&[u8], u64)] = &[
    (b"store.budget", 268_435_456),
    (b"secrets.budget", 268_435_456),
    (b"tasks.tasks", 8),
    (b"tasks.funders", 8),
    (b"tasks.project_tasks", 8),
    (b"tasks.project_goals", 8),
    (b"tasks.tree_tasks", 32),
    (b"tasks.depth", 8),
    (b"tasks.delegates", 4),
    (b"tasks.references", 8),
    (b"tasks.subscriptions", 4),
    (b"tasks.batch", 2),
    (b"tasks.dependencies", 8),
    (b"tasks.circle", 8),
    (b"tasks.holdings", 4),
    (b"tasks.hold_kinds", 8),
    (b"tasks.resource_reports", 128),
    (b"tasks.pools", 8),
    (b"tasks.hold_segments", 4),
    (b"tasks.hold_bytes", 1_024),
    (b"tasks.hold_waiters", 8),
    (b"tasks.hold_wait_ms", 120_000),
    (b"tasks.grace_ms", 600_000),
    (b"tasks.inputs", 8),
    (b"tasks.spec_bytes", 4_096),
    (b"tasks.parameters", 4),
    (b"tasks.result_bytes", 512),
    (b"tasks.run_inbox", 8),
    (b"tasks.inbox_messages", 8),
    (b"tasks.inbox_bytes", 4_096),
    (b"tasks.message_bytes", 4_096),
    (b"tasks.person_stall_ms", 3_600_000),
    (b"tasks.proposal_stall_ms", 3_600_000),
    (b"tasks.escalation_stall_ms", 3_600_000),
    (b"tasks.saved_resources", 4),
    (b"tasks.contract_choices", 1),
    (b"tasks.charters", 4),
    (b"tasks.authority_grants", 4),
    (b"tasks.authority_segments", 4),
    (b"tasks.authority_bytes", 2_048),
    (b"tasks.executor_kinds", 4),
    (b"tasks.facts", 32),
    (b"tasks.room_refusals", 8),
    (b"people.people", 4),
    (b"people.inbox_entries", 8),
    (b"people.inbox_tasks", 16),
    (b"people.inbox_recipients", 32),
    (b"people.sign_ins", 8),
    (b"people.projects", 3),
    (b"people.holdings", 8),
    (b"people.batch", 2),
    (b"people.operations", 8),
    (b"people.initial_owners", 2),
    (b"people.requests", 32),
    (b"people.room_refusals", 8),
    (b"people.pending", 8),
    (b"people.waiters", 4),
    (b"people.identity_bytes", 1_024),
    (b"people.words", 2_048),
    (b"people.amendment_bytes", 8_192),
    (b"people.sign_in_lifetime_ms", 86_400_000),
    (b"people.request_retention_ms", 86_400_000),
    (b"people.facts", 32),
    (b"fleet.workers", 2),
    (b"fleet.needs_bytes", 65_536),
    (b"fleet.grants", 8),
    (b"fleet.engine_slots", 0),
    (b"fleet.slots", 2),
    (b"fleet.workstreams", 8),
    (b"fleet.attempts", 8),
    (b"fleet.calls", 32),
    (b"fleet.call_name_bytes", 64),
    (b"fleet.progress_bytes", 1_024),
    (b"fleet.turns", 32),
    (b"fleet.grace_ms", 120_000),
    (b"fleet.facts", 32),
    (b"fleet.history", 8),
    (b"authority.projects", 2),
    (b"authority.roles", 4),
    (b"authority.requirements", 4),
    (b"authority.facts", 16),
    (b"authority.grants", 4),
    (b"authority.executors", 4),
    (b"authority.segments", 4),
    (b"authority.segment_bytes", 256),
    (b"authority.implications", 8),
    (b"authority.batch", 2),
    (b"authority.accounts", 4),
    (b"authority.writes", 4),
    (b"brief.briefs", 4),
    (b"brief.sections", 4),
    (b"brief.read_bytes", 8_192),
    (b"brief.brief_bytes", 16_384),
    (b"accounts.accounts", 4),
    (b"accounts.credentials", 3),
    (b"accounts.backoff_base_ms", 2_000),
    (b"accounts.backoff_max_ms", 60_000),
    (b"accounts.retry_ms", 30_000),
    (b"accounts.move_deadline_ms", 60_000),
    (b"accounts.spent_attention_ms", 300_000),
    (b"accounts.facts", 32),
    (b"accounts.holder_accounts", 4),
    (b"accounts.sources", 3),
    (b"accounts.program_names", 12),
    (b"accounts.sign_ins", 2),
    (b"accounts.sign_in_deadline_ms", 300_000),
    (b"views.runs", 4),
    (b"views.watchers", 4),
    (b"views.watchers_per_party", 4),
    (b"views.subjects_per_change", 32),
    (b"views.text_bytes", 65_536),
    (b"views.facts", 32),
    (b"notes.scopes", 8),
    (b"notes.entries_per_scope", 16),
    (b"notes.pattern_bytes", 256),
    (b"notes.description_bytes", 256),
    (b"notes.body_bytes", 4_096),
    (b"notes.references", 8),
    (b"notes.load_rows", 4),
    (b"notes.lines", 4),
    (b"notes.recalled", 4),
    (b"documents.rows", 4),
    (b"documents.text_bytes", 4_096),
    (b"documents.chips", 4),
    (b"documents.lines", 4),
    (b"core.brief_core_budgets.task", 4_096),
    (b"core.brief_core_budgets.dependencies", 2_048),
    (b"core.brief_core_budgets.attempts", 1_024),
    (b"core.brief_core_budgets.plan", 2_048),
    (b"core.brief_core_budgets.notes", 2_048),
    (b"core.templates", 8),
    (b"core.fill_mark", 268_435_456),
    (b"core.connectors", 2),
    (b"core.resume_bytes", 65_536),
    (b"core.run_bytes", 16_384),
    (b"core.policy_bytes", 16_384),
    (b"core.escalation_reason_bytes", 512),
    (b"core.load_slots", 4),
    (b"core.load_bytes", 65_536),
    (b"core.call_records", 32),
    (b"core.call_answer_bytes", 32_768),
    (b"core.brief_parts", 8),
    (b"hub.slots", 2),
    (b"hub.accounts", 8),
    (b"hub.charter_bytes", 65_536),
    (b"hub.transcript_bytes", 262_144),
    (b"hub.delivery_evidence_bytes", 4_096),
    (b"hub.turn_bytes", 65_536),
    (b"hub.outcome_bytes", 4_096),
    (b"hub.detail_bytes", 4_096),
    (b"hub.held", 8),
    (b"hub.inbox", 8),
    (b"hub.event_bytes", 65_536),
    (b"hub.run_calls", 8),
    (b"hub.facts", 64),
    (b"hub.told", 64),
    (b"hub.piece_bytes", 4096),
    (b"hub.turns", 8),
    (b"hub.turn_queue_bytes", 524_288),
    (b"link.chunk", 16_384),
    (b"link.name", 32),
    (b"link.secret", 256),
    (b"link.names", 8),
    (b"link.opening", 4),
    (b"link.handshake_ms", 10_000),
    (b"link.hello_ms", 10_000),
    (b"link.ping_ms", 5_000),
    (b"link.silence_ms", 15_000),
    (b"link.stall_ms", 30_000),
    (b"link.backoff_ms", 250),
    (b"link.ceiling_ms", 30_000),
    (b"link.proved_ms", 60_000),
    (b"link.busy_ms", 2_000),
    (b"link.facts", 4470),
    (b"accounts.token", 7934),
    (b"tasks.retries.transient.retries", 3),
    (b"tasks.retries.transient.base_ms", 1_000),
    (b"tasks.retries.transient.max_ms", 30_000),
    (b"tasks.retries.permanent.retries", 3),
    (b"tasks.retries.permanent.base_ms", 1_000),
    (b"tasks.retries.permanent.max_ms", 30_000),
    (b"tasks.retries.run.retries", 3),
    (b"tasks.retries.run.base_ms", 1_000),
    (b"tasks.retries.run.max_ms", 30_000),
    (b"tasks.retries.agent.retries", 3),
    (b"tasks.retries.agent.base_ms", 1_000),
    (b"tasks.retries.agent.max_ms", 30_000),
    (b"tasks.retries.lost.retries", 3),
    (b"tasks.retries.lost.base_ms", 1_000),
    (b"tasks.retries.lost.max_ms", 30_000),
    (b"tasks.retries.invalid.retries", 3),
    (b"tasks.retries.invalid.base_ms", 1_000),
    (b"tasks.retries.invalid.max_ms", 30_000),
    (b"documents.versions.minimum", 1),
    (b"documents.versions.maximum", 1),
];

fn overrides_past_their_types_are_refused() {
    table([
        ("an_override_past_its_type_is_refused", an_override_past_its_type_is_refused),
        ("a_duration_override_past_its_type_is_refused", a_duration_override_past_its_type_is_refused),
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
fn override_document_problems_name_the_invalid_fields() {
    table([
        (
            "an_override_that_breaks_a_worst_case_is_refused_naming_it",
            an_override_that_breaks_a_worst_case_is_refused_naming_it,
        ),
        ("an_unknown_profile_is_refused_naming_it", an_unknown_profile_is_refused_naming_it),
        ("a_missing_profile_is_refused_naming_it", a_missing_profile_is_refused_naming_it),
        ("an_unknown_limits_member_is_refused_naming_it", an_unknown_limits_member_is_refused_naming_it),
        ("an_override_named_twice_is_refused_naming_it", an_override_named_twice_is_refused_naming_it),
        ("a_non_integer_override_is_refused_naming_it", a_non_integer_override_is_refused_naming_it),
        ("an_invalid_compound_member_is_refused_naming_it", an_invalid_compound_member_is_refused_naming_it),
    ]);
}

#[test]
fn sign_ins_at_once_and_the_sign_in_deadline_reach_the_driver() {
    let limits =
        parse(br#"{"profile":"small","overrides":{"accounts.sign_ins":3,"accounts.sign_in_deadline_ms":120000}}"#)
            .unwrap();
    assert_eq!((limits.core.accounts.sign_ins, limits.driver.listeners), (3, 6));
    assert_eq!(limits.driver.client.sign_in_time, skein_lib::Duration::from_secs(120));
}
fn no_sign_ins_at_once_is_refused() {
    refused(br#"{"profile":"small","overrides":{"accounts.sign_ins":0}}"#, b"accounts.sign_ins");
}
fn sign_ins_past_the_drivers_sockets_are_refused() {
    refused(br#"{"profile":"small","overrides":{"accounts.sign_ins":7}}"#, b"accounts.sign_ins");
}
fn a_holders_accounts_past_the_deployments_are_refused() {
    refused(br#"{"profile":"small","overrides":{"accounts.holder_accounts":5}}"#, b"accounts.holder_accounts");
}
fn no_sources_per_account_is_refused() {
    refused(br#"{"profile":"small","overrides":{"accounts.sources":0}}"#, b"accounts.sources");
}
fn a_sign_in_deadline_of_zero_is_refused() {
    refused(br#"{"profile":"small","overrides":{"accounts.sign_in_deadline_ms":0}}"#, b"accounts.sign_in_deadline_ms");
}
/// The small profile's override in `bytes`, of `name`, is refused naming it.
fn refused(bytes: &[u8], name: &[u8]) {
    problem(bytes, &[b"limits", b"overrides", name], What::Override);
}
#[test]
fn accounts_bounds_past_their_limits_are_refused_naming_them() {
    table([
        ("no_sign_ins_at_once_is_refused", no_sign_ins_at_once_is_refused),
        ("sign_ins_past_the_drivers_sockets_are_refused", sign_ins_past_the_drivers_sockets_are_refused),
        ("a_holders_accounts_past_the_deployments_are_refused", a_holders_accounts_past_the_deployments_are_refused),
        ("no_sources_per_account_is_refused", no_sources_per_account_is_refused),
        ("a_sign_in_deadline_of_zero_is_refused", a_sign_in_deadline_of_zero_is_refused),
    ]);
}

#[test]
fn invalid_override_names_and_values_are_refused() {
    table([
        ("an_override_of_an_unknown_name_is_refused", an_override_of_an_unknown_name_is_refused),
        ("a_whole_compound_limit_is_refused", a_whole_compound_limit_is_refused),
        ("overrides_past_their_types_are_refused", overrides_past_their_types_are_refused),
    ]);
}

#[test]
fn document_segments_follow_the_largest_admitted_name_after_overrides() {
    const CASES: [(&[u8], u32); 3] = [
        (br#"{"profile":"small","overrides":{"tasks.hold_segments":6}}"#, 6),
        (br#"{"profile":"small","overrides":{"tasks.authority_segments":7}}"#, 7),
        (br#"{"profile":"small","overrides":{"authority.segments":8}}"#, 8),
    ];
    for (configuration, segments) in CASES {
        let limits = parse(configuration).expect("admitted name bounds");
        assert_eq!(limits.core.documents.segments, segments, "{configuration:?}");
        assert_eq!(limits.core.documents.lines, 4, "short lists stay bounded independently");
    }
}

#[test]
fn document_segments_have_no_operator_override() {
    assert_eq!(overrides::apply(&mut small(), b"documents.segments", 8), Err(What::Override));
    assert_eq!(overrides::apply(&mut small(), b"documents.grants", 8), Err(What::Override));
    assert_eq!(overrides::apply(&mut small(), b"documents.executors", 8), Err(What::Override));
    assert_eq!(overrides::apply(&mut small(), b"documents.implications", 8), Err(What::Override));
}

#[test]
fn the_accounts_move_deadline_is_positive_and_overridden_in_milliseconds() {
    for profile in [Profile::Small, Profile::Standard] {
        assert_eq!(crate::profile(profile).core.accounts.move_deadline, skein_lib::Duration::from_secs(60));
    }
    let mut expected = small();
    expected.core.accounts.move_deadline = skein_lib::Duration::from_millis(12_345);
    assert_eq!(parse(br#"{"profile":"small","overrides":{"accounts.move_deadline_ms":12345}}"#), Ok(expected));
    refused(br#"{"profile":"small","overrides":{"accounts.move_deadline_ms":0}}"#, b"accounts.move_deadline_ms");
}

#[test]
fn the_tail_has_its_own_profile_budget_and_override() {
    let mut expected = small();
    assert_eq!(expected.core.brief_core_budgets.tail, 2048);
    let before = jig_domain_core::worst_case(&expected.core).unwrap();
    expected.core.brief_core_budgets.tail = 90;
    assert_eq!(parse(br#"{"profile":"small","overrides":{"core.brief_core_budgets.tail":90}}"#), Ok(expected));
    assert!(jig_domain_core::worst_case(&expected.core).unwrap() < before);
}

#[test]
fn settings_history_is_positive_and_can_override_the_profile() {
    assert_eq!(small().core.settings.history, 64);
    assert_eq!(
        parse(br#"{"profile":"small","overrides":{"core.settings.history":2}}"#).unwrap().core.settings.history,
        2
    );
    problem(
        br#"{"profile":"small","overrides":{"core.settings.history":0}}"#,
        &[b"limits", b"overrides", b"core.settings.history"],
        What::Override,
    );
}

#[test]
fn a_journal_too_small_for_settings_is_refused_before_start() {
    let mut engine = super::fixtures::engine(super::fixtures::ENGINE_SMALL).unwrap();
    engine.limits.journal.writes = 1;
    let mut problems = Problems::empty();
    crate::domain::whole(&engine, &mut problems);
    let mut found = false;
    for problem in problems.problems.as_slice() {
        found |= problem.path == path(&[b"limits", b"journal.writes"]) && problem.what == What::Bound;
    }
    assert!(found, "the refusal names the journal write limit");
}

#[test]
fn a_policy_override_past_the_codec_envelope_is_refused_at_its_path() {
    let errors = parse(br#"{"profile":"small","overrides":{"core.policy_bytes":65536}}"#).unwrap_err();
    assert_eq!(errors.problems.len(), 1);
    let problem = errors.problems.get(0).unwrap();
    assert_eq!(problem.path, path(&[b"limits", b"overrides", b"core.policy_bytes"]));
    assert_eq!(problem.codec_message().unwrap().as_ref(), b"limits.overrides.core.policy_bytes: 65536 exceeds the people-v13 codec envelope of 43695 bytes (PolicyValue.permissions would need 98304 bytes; ceiling 65536).");
}

#[test]
fn whole_admission_checks_the_limits_handed_to_the_store() {
    let mut engine = super::fixtures::engine(super::fixtures::ENGINE_SMALL).unwrap();
    engine.limits.core.policy_bytes = 65_536;
    let mut problems = Problems::empty();
    crate::domain::whole(&engine, &mut problems);
    let problem = problems.problems.get(0).unwrap();
    assert_eq!(problem.path, path(&[b"limits", b"core", b"policy_bytes"]));
    assert_eq!(problem.codec_message().unwrap().as_ref(), b"limits.core.policy_bytes: 65536 exceeds the people-v13 codec envelope of 43695 bytes (PolicyValue.permissions would need 98304 bytes; ceiling 65536).");
}

#[test]
fn codec_admission_keeps_a_composed_documents_override_prefix() {
    let document = document(br#"{"profile":"small","overrides":{"core.policy_bytes":43696}}"#);
    let prefix = path(&[b"engine", b"limits"]);
    let problems = overrides::parse(&document, 0, &prefix).unwrap_err();
    assert_eq!(problems.problems.get(0).unwrap().path, prefix.member(b"overrides").member(b"core.policy_bytes"));
}

#[test]
fn pending_settings_proposals_are_positive_and_keep_the_profile_defaults() {
    assert_eq!(small().core.settings.proposals, 4);
    assert_eq!(profile(Profile::Standard).core.settings.proposals, 8);
    assert_eq!(
        parse(br#"{"profile":"small","overrides":{"core.settings.proposals":2}}"#).unwrap().core.settings.proposals,
        2
    );
    problem(
        br#"{"profile":"small","overrides":{"core.settings.proposals":0}}"#,
        &[b"limits", b"overrides", b"core.settings.proposals"],
        What::Override,
    );
}
#[test]
fn the_journal_prices_each_pending_settings_withdrawal() {
    let mut limits = small().core;
    limits.settings.proposals = 1;
    let one = jig_domain_core::settings::writes_worst_case(&limits).expect("write room");
    limits.settings.proposals = 2;
    let two = jig_domain_core::settings::writes_worst_case(&limits).expect("write room");
    // Set erase, pending erase, terminal row, decision archive and proposer update.
    assert_eq!(two - one, 5);
}

#[test]
fn settings_revision_limits_are_positive_and_size_the_children() {
    for (profile_name, revisions, versions) in [(Profile::Small, 4, 8), (Profile::Standard, 8, 16)] {
        let limits = profile(profile_name).core;
        assert_eq!(limits.settings.revisions, revisions);
        assert_eq!(limits.settings.versions, versions);
        assert_eq!(limits.tasks.revisions, revisions);
        assert_eq!(limits.tasks.versions, versions);
        assert_eq!(limits.fleet.versions, versions);
    }
    let changed =
        parse(br#"{"profile":"small","overrides":{"core.settings.revisions":2,"core.settings.versions":3}}"#).unwrap();
    assert_eq!(changed.core.settings.revisions, 2);
    assert_eq!(changed.core.tasks.revisions, 2);
    assert_eq!(changed.core.settings.versions, 3);
    assert_eq!(changed.core.tasks.versions, 3);
    assert_eq!(changed.core.fleet.versions, 3);
    for (document, name) in [
        (
            br#"{"profile":"small","overrides":{"core.settings.revisions":0}}"#.as_slice(),
            b"core.settings.revisions".as_slice(),
        ),
        (
            br#"{"profile":"small","overrides":{"core.settings.versions":0}}"#.as_slice(),
            b"core.settings.versions".as_slice(),
        ),
    ] {
        problem(document, &[b"limits", b"overrides", name], What::Override);
    }
    for revisions in [true, false] {
        let mut engine = super::fixtures::engine(super::fixtures::ENGINE_SMALL).unwrap();
        let name = if revisions {
            engine.limits.core.settings.revisions = 0;
            b"settings.revisions".as_slice()
        } else {
            engine.limits.core.settings.versions = 0;
            b"settings.versions".as_slice()
        };
        let mut problems = Problems::empty();
        crate::domain::whole(&engine, &mut problems);
        let mut found = false;
        for problem in problems.problems.as_slice() {
            if problem.path == path(&[b"limits", b"core", name]) && problem.what == What::Bound {
                found = true;
            }
        }
        assert!(found);
    }
}
