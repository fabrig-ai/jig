//! Names and whole policy rows restore together (core.md, 6.1; store.md, 4.4).
use super::*;

const FIRST_PROJECT: u32 = 1;
const SECOND_PROJECT: u32 = 2;
const TWO_PROJECTS: u32 = 2;

fn restoring() -> (Core, Limits, people::PolicyValue) {
    let mut limits = limits();
    limits.authority.projects = TWO_PROJECTS;
    let mut config = config(41);
    let value = policy::snapshot(&config.seed_projects[0].policy.policy, &[], &[]).expect("bounded policy");
    config.defaults.rules = (authority::Domain::new(config.defaults.rules.clone(), limits.authority)
        .expect("two policies"))
    .rules()
    .clone();
    config.seed_projects = Box::new([]);
    let mut core = Core::new(config, &limits).expect("restoring core");
    core.counters.deployment.projects = u64::from(SECOND_PROJECT);
    core.restart_begin(false);
    core.restore_core(CoreRecord::Deployment(core.counters.deployment()), &limits);
    (core, limits, value)
}

#[test]
fn loaded_project_names_do_not_rejudge_historical_uniqueness() {
    let (mut core, limits, value) = restoring();
    assert!(core.restore_policy(&limits, FIRST_PROJECT, Box::from("Tally"), value.clone()));
    assert!(core.restore_policy(&limits, SECOND_PROJECT, Box::from("tally"), value));
    assert_eq!(core.names.get(&FIRST_PROJECT).unwrap().as_ref(), "Tally");
    assert_eq!(core.names.get(&SECOND_PROJECT).unwrap().as_ref(), "tally");
}

#[test]
fn loaded_project_names_keep_every_value_within_the_codec_bound() {
    let long = "a".repeat(65); // One byte past the name codec's bound.
    for (name, accepted) in [("", true), (long.as_str(), false), ("Ta\u{7}lly", true), (" Tally", true), ("12", true)] {
        let (mut core, limits, value) = restoring();
        assert_eq!(core.restore_policy(&limits, FIRST_PROJECT, Box::from(name), value), accepted, "{name:?}");
        let expected = if accepted { Some(Box::<str>::from(name)) } else { None };
        assert_eq!(core.names.get(&FIRST_PROJECT), expected.as_ref());
    }
}

#[test]
fn a_duplicate_project_row_keeps_the_first_whole_policy() {
    let (mut core, limits, value) = restoring();
    core.restore_policy(&limits, FIRST_PROJECT, Box::from("Tally"), value.clone());
    let before = core.authority.policy(FIRST_PROJECT).cloned();
    let mut changed = value;
    changed.period_spend = 1;
    assert!(!core.restore_policy(&limits, FIRST_PROJECT, Box::from("Other"), changed));
    assert_eq!(core.authority.policy(FIRST_PROJECT).cloned(), before);
    assert_eq!(core.names.get(&FIRST_PROJECT).expect("first name").as_ref(), "Tally");
    assert_eq!(core.projects.as_slice(), [FIRST_PROJECT]);
}

#[test]
fn a_restored_policy_past_the_current_rules_keeps_its_live_project() {
    let (mut core, limits, mut value) = restoring();
    value.ceiling.spend = core.authority.rules().ceiling.budget.spend + 1;
    assert!(core.restore_policy(&limits, FIRST_PROJECT, Box::from("Tally"), value));
    assert!(core.authority.policy(FIRST_PROJECT).is_some());
    assert_eq!(core.projects.as_slice(), [FIRST_PROJECT]);
    assert_eq!(core.names.get(&FIRST_PROJECT).unwrap().as_ref(), "Tally");
}

#[test]
fn oversized_project_policy_restore_keeps_its_named_failure() {
    let oversized = RESTORE_POLICY_BYTES.checked_add(1).expect("one past the policy byte budget");
    let (mut core, limits, configured) = restoring_sized_policy(oversized);
    let value = policy::snapshot(&configured.policy, &configured.permissions, &configured.templates)
        .expect("bounded policy snapshot");
    assert!(!core.restore_policy(&limits, RESTORE_PROJECT, Box::from("Tally"), value));
    assert_eq!(core.restart_failure(), Some(RestartStep::LoadCore));
    let _refused = core.restart_refuse();
    assert_eq!(core.restore_failure(), Some(RestartRequest::ProjectPolicy { project: RESTORE_PROJECT }));
    assert_eq!(core.restart_done(RestartStep::LoadCore), RestartRequest::ProjectPolicy { project: RESTORE_PROJECT });
    assert_eq!(core.restore_failure(), Some(RestartRequest::ProjectPolicy { project: RESTORE_PROJECT }));
    assert!(!core.restart_ready());
    assert!(core.projects.is_empty());
    assert!(core.names.is_empty());
    assert!(core.authority.policy(RESTORE_PROJECT).is_none());
}

#[test]
fn project_policy_restore_admits_the_policy_byte_budget() {
    let (mut core, limits, configured) = restoring_sized_policy(RESTORE_POLICY_BYTES);
    let value = policy::snapshot(&configured.policy, &configured.permissions, &configured.templates)
        .expect("bounded policy snapshot");
    assert!(core.restore_policy(&limits, RESTORE_PROJECT, Box::from("Tally"), value));
    assert_eq!(core.restore_failure(), None);
    assert_eq!(core.restart_done(RestartStep::LoadCore), RestartRequest::Step(RestartStep::LoadInbox));
    assert_eq!(core.authority.policy(RESTORE_PROJECT), Some(&configured.policy));
    assert_eq!(core.names.get(&RESTORE_PROJECT).unwrap().as_ref(), "Tally");
}
