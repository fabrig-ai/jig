use super::*;
use crate::tests::{RESTORE_POLICY_BYTES, RESTORE_PROJECT, config, limits, restoring_sized_policy};
use crate::{Core, CoreRecord, Limits, RestartRequest, RestartStep, Restored};
use jig_domain_tasks as tasks;
use skein_lib::{Duration, Env, Time};

fn template() -> Template {
    Template {
        number: 1,
        name: Box::from(b"chat".as_slice()),
        executor: people::TemplateExecutor::Charter(1),
        contract: tasks::Contract::Report { words: 1 },
        shallower: 0,
        budget: people::TemplateBudget::Given,
        deadline: people::TemplateDeadline::None,
        tracked: None,
        flags: tasks::Flags::PLAIN,
        label: 0,
        grace: None,
        llm: None,
    }
}
fn charter(current: &mut Current) -> &mut Charter {
    match &mut current.defaults.get_mut(&Setting::Charter(1)).unwrap().value {
        Value::Charter(value) => value,
        Value::Template(_) | Value::Policy(_) | Value::Rules(_) => unreachable!(),
    }
}
#[test]
fn lookups_apply_sets_removals_additions_and_resets_without_a_third_set() {
    let mut current = Current::new(config(1).defaults, &limits());
    let original = current.charter(1).unwrap().clone();
    let mut changed = original.clone();
    changed.spend += 1;
    current
        .changes
        .insert(Setting::Charter(1), Changed { value: Some(Value::Charter(Box::new(changed.clone()))), revision: 2 })
        .unwrap();
    assert_eq!(current.charter(1), Some(&changed));
    current.changes.insert(Setting::Charter(1), Changed { value: None, revision: 3 }).unwrap();
    assert!(current.charter(1).is_none());
    let mut added = original.clone();
    added.number = 2;
    current
        .changes
        .insert(Setting::Charter(2), Changed { value: Some(Value::Charter(Box::new(added.clone()))), revision: 4 })
        .unwrap();
    assert_eq!(current.charter(2), Some(&added));
    current.changes.remove(&Setting::Charter(1));
    assert_eq!(current.charter(1), Some(&original));
    assert_eq!(current.offered().as_slice(), &[Setting::Charter(1), Setting::Charter(2), Setting::Rules]);
}
fn fault_case(fault: Fault) {
    let mut config = config(1);
    let limits = limits();
    let mut policy = config.seed_projects[0].policy.clone();
    config.defaults.templates = Box::new([template()]);
    let mut current = Current::new(config.defaults, &limits);
    let setting = match fault {
        Fault::Shape => {
            let count = limits.run_bytes.checked_add(1).expect("one past the run bound");
            let mut bytes = List::with_capacity(count);
            for _ in 0..count {
                bytes.push(b'x').expect("instruction byte count");
            }
            charter(&mut current).policy.instructions = bytes.into_boxed();
            Setting::Charter(1)
        }
        Fault::Count => {
            let original = current.charter(1).unwrap().clone();
            let extra = limits.tasks.charters.checked_add(1).expect("one past the charter bound");
            for number in 2..=extra {
                let mut value = original.clone();
                value.number = number;
                current
                    .defaults
                    .insert(Setting::Charter(number), Default { value: Value::Charter(Box::new(value)), revision: 1 })
                    .unwrap();
            }
            Setting::Charter(extra)
        }
        Fault::Reference => {
            let mut value = template();
            value.executor = people::TemplateExecutor::Charter(999);
            current
                .changes
                .insert(Setting::Template(1), Changed { value: Some(Value::Template(Box::new(value))), revision: 2 })
                .unwrap();
            Setting::Template(1)
        }
        Fault::Rules => {
            policy.policy.period_spend =
                current.rules().period_spend.checked_add(1).expect("one past the policy period bound");
            current
                .changes
                .insert(Setting::Policy(1), Changed { value: Some(Value::Policy(Box::new(policy))), revision: 2 })
                .unwrap();
            Setting::Policy(1)
        }
        Fault::Budget => {
            let value = charter(&mut current);
            value.policy.turns = value.policy.reserve_turns;
            Setting::Charter(1)
        }
        Fault::Grace => {
            let mut value = template();
            value.grace = Some(Duration::from_secs(86_400));
            current
                .changes
                .insert(Setting::Template(1), Changed { value: Some(Value::Template(Box::new(value))), revision: 2 })
                .unwrap();
            Setting::Template(1)
        }
        Fault::Owner => {
            policy.policy.roles[0].requests = authority::Requests(authority::Requests::ALL.0 & !1);
            current
                .changes
                .insert(Setting::Policy(1), Changed { value: Some(Value::Policy(Box::new(policy))), revision: 2 })
                .unwrap();
            Setting::Policy(1)
        }
    };
    assert_eq!(
        check(&current, &config.settings, &config.connectors, &limits),
        Err(Failure { setting, fault }),
        "{fault:?}"
    );
}

#[test]
fn the_whole_check_names_shape_and_its_setting() {
    fault_case(Fault::Shape);
}
#[test]
fn the_whole_check_names_count_and_its_setting() {
    fault_case(Fault::Count);
}
#[test]
fn the_whole_check_names_reference_and_its_setting() {
    fault_case(Fault::Reference);
}
#[test]
fn the_whole_check_names_rules_and_its_setting() {
    fault_case(Fault::Rules);
}
#[test]
fn the_whole_check_names_budget_and_its_setting() {
    fault_case(Fault::Budget);
}
#[test]
fn the_whole_check_names_grace_and_its_setting() {
    fault_case(Fault::Grace);
}
#[test]
fn the_whole_check_names_owner_and_its_setting() {
    fault_case(Fault::Owner);
}

#[test]
fn pruning_a_lowered_history_limit_uses_only_the_commits_remaining_room() {
    let (writes, oldest) = revision_writes(None, List::with_capacity(0), 1, 20, 2, 4);
    assert_eq!(oldest, 4);
    assert_eq!(writes.len(), 4);
    let crate::Write::Save(crate::Record::Core(crate::CoreRecord::Settings(Record::Extent { oldest: stored }))) =
        &writes.as_slice()[3]
    else {
        panic!("the extent closes the pruning commit")
    };
    assert_eq!(*stored, 4);
    let (writes, oldest) = revision_writes(None, List::with_capacity(0), oldest, 20, 2, 32);
    assert_eq!(oldest, 18);
    assert_eq!(writes.len(), 15);
    let (writes, oldest) = revision_writes(None, List::with_capacity(0), oldest, 20, 2, 32);
    assert!(writes.is_empty());
    assert_eq!(oldest, 18);
}

#[test]
fn the_whole_check_refuses_unconfigured_model_bits() {
    let config = config(1);
    let mut current = Current::new(config.defaults, &limits());
    let mut rules = current.rules().clone();
    rules.ceiling.models = authority::Models::Named(2);
    current
        .changes
        .insert(Setting::Rules, Changed { value: Some(Value::Rules(Box::new(rules))), revision: 2 })
        .unwrap();
    assert_eq!(
        check(&current, &config.settings, &config.connectors, &limits()),
        Err(Failure { setting: Setting::Rules, fault: Fault::Reference })
    );
}
#[test]
fn the_whole_check_refuses_unconfigured_contract_connectors() {
    let config = config(1);
    let mut current = Current::new(config.defaults, &limits());
    let mut value = template();
    value.contract = tasks::Contract::Change { connector: u16::MAX, kind: 1, words: 1 };
    current
        .changes
        .insert(Setting::Template(1), Changed { value: Some(Value::Template(Box::new(value))), revision: 2 })
        .unwrap();
    assert_eq!(
        check(&current, &config.settings, &config.connectors, &limits()),
        Err(Failure { setting: Setting::Template(1), fault: Fault::Reference })
    );
}

#[test]
fn decision_edits_apply_each_kind_over_defaults_and_additions() {
    let limits = limits();
    let mut config = config(1);
    config.defaults.templates = Box::new([template()]);
    let policy = config.seed_projects[0].policy.clone();
    config.defaults.policies = Map::with_capacity(1);
    config.defaults.policies.insert(1, policy).unwrap();
    let mut current = Current::new(config.defaults, &limits);
    for setting in [Setting::Charter(1), Setting::Template(1), Setting::Policy(1), Setting::Rules] {
        let original = current.value(setting).unwrap().clone();
        let items = [Item { setting, edit: Edit::Set(original.clone()) }];
        assert!(!changes_current(&current, &items));
        apply(&mut current, &items, 2);
        assert_eq!(current.value(setting), Some(&original));
        assert_eq!(current.changes.get(&setting).unwrap().revision, 2);
        let reset = [Item { setting, edit: Edit::Reset }];
        apply(&mut current, &reset, 3);
        assert!(!current.changed(setting));
        if setting == Setting::Rules {
            assert_eq!(
                change_shape(&[Item { setting, edit: Edit::Removed }], &limits),
                Err(Failure { setting, fault: Fault::Shape })
            );
        } else {
            let removed = [Item { setting, edit: Edit::Removed }];
            assert!(changes_current(&current, &removed));
            apply(&mut current, &removed, 4);
            assert!(current.value(setting).is_none());
            assert!(!changes_current(&current, &removed));
            apply(&mut current, &reset, 5);
            assert_eq!(current.value(setting), Some(&original));
        }
        let added = match original {
            Value::Charter(mut value) => {
                value.number = 2;
                Some((Setting::Charter(2), Value::Charter(value)))
            }
            Value::Template(mut value) => {
                value.number = 2;
                Some((Setting::Template(2), Value::Template(value)))
            }
            Value::Policy(value) => Some((Setting::Policy(2), Value::Policy(value))),
            Value::Rules(_) => None,
        };
        if let Some((setting, value)) = added {
            for erase in [Edit::Removed, Edit::Reset] {
                apply(&mut current, &[Item { setting, edit: Edit::Set(value.clone()) }], 6);
                assert_eq!(current.value(setting), Some(&value));
                apply(&mut current, &[Item { setting, edit: erase }], 7);
                assert!(!current.changed(setting));
                assert!(current.value(setting).is_none());
            }
        }
    }
    assert!(!changes_current(&current, &[]));
}

#[test]
fn zero_numbered_edits_and_repeated_settings_are_malformed() {
    let limits = limits();
    for setting in [Setting::Charter(0), Setting::Template(0), Setting::Policy(0)] {
        for edit in [Edit::Removed, Edit::Reset] {
            assert_eq!(change_shape(&[Item { setting, edit }], &limits), Err(Failure { setting, fault: Fault::Shape }));
        }
    }
    let setting = Setting::Template(1);
    assert_eq!(
        change_shape(&[Item { setting, edit: Edit::Removed }, Item { setting, edit: Edit::Reset }], &limits),
        Err(Failure { setting, fault: Fault::Shape })
    );
}

#[test]
fn current_lookup_keeps_each_kinds_default_change_and_removal_origin() {
    let mut config = config(1);
    config.defaults.templates = Box::new([template()]);
    config.defaults.policies = Map::with_capacity(1);
    config.defaults.policies.insert(1, config.seed_projects[0].policy.clone()).unwrap();
    let mut current = Current::new(config.defaults, &limits());
    for setting in [Setting::Charter(1), Setting::Template(1), Setting::Policy(1), Setting::Rules] {
        let default = current.value(setting).unwrap().clone();
        assert_eq!(current.lookup(setting), (Some(&default), Some((false, 1))));
        current.changes.insert(setting, Changed { value: Some(default.clone()), revision: 2 }).unwrap();
        assert_eq!(current.lookup(setting), (Some(&default), Some((true, 2))));
        current.changes.insert(setting, Changed { value: None, revision: 3 }).unwrap();
        assert_eq!(current.lookup(setting), (None, Some((true, 3))));
        current.changes.remove(&setting);
        assert_eq!(current.lookup(setting), (Some(&default), Some((false, 1))));
    }
    assert_eq!(current.lookup(Setting::Template(999)), (None, None));
}

#[test]
fn restoring_settings_proposals_requires_the_deployment_scope_without_a_project_policy() {
    let (mut core, _) = crate::tests::core();
    core.counters.deployment.messages = 1;
    core.counters.deployment.people = 1;
    let mut row = jig_domain_tasks::PersonProposal {
        number: 1,
        proposer: 1,
        project: jig_domain_people::DEPLOYMENT,
        action: jig_domain_tasks::PersonAction::Settings {
            against: 1,
            items: Box::new([jig_domain_tasks::SettingHead {
                setting: jig_domain_tasks::Setting::Template(1),
                edit: jig_domain_tasks::SettingEdit::Removed,
            }]),
        },
        reason: b"review".as_slice().into(),
        state: jig_domain_tasks::PersonProposalState::Pending { since: Wall::EPOCH },
    };
    assert!(core.restore_task_row(&jig_domain_tasks::Stored::PersonProposal(Box::new(row.clone()))));
    row.project = 1;
    assert!(!core.restore_task_row(&jig_domain_tasks::Stored::PersonProposal(Box::new(row))));
}

fn stored_policy_change(bytes: u64) -> (Core, Limits, Record) {
    let (mut core, limits, configured) = restoring_sized_policy(bytes);
    let mut defaults = List::with_capacity(core.current.defaults.len());
    for (&setting, default) in &core.current.defaults {
        defaults.push(Record::Default { setting, default: default.clone() }).expect("stored default count");
    }
    for record in defaults.into_boxed() {
        let _restored = core.restore_core(CoreRecord::Settings(record), &limits);
    }
    for record in [
        Record::Revision {
            history: false,
            revision: Revision { head: core.current.head.clone(), items: Box::new([]) },
        },
        Record::Extent { oldest: 0 },
    ] {
        let _restored = core.restore_core(CoreRecord::Settings(record), &limits);
    }
    let changed = Record::Changed {
        setting: Setting::Policy(RESTORE_PROJECT),
        changed: Changed { revision: 1, value: Some(Value::Policy(Box::new(configured))) },
    };
    (core, limits, changed)
}

#[test]
fn oversized_settings_policy_restore_keeps_its_named_failure() {
    let oversized = RESTORE_POLICY_BYTES.checked_add(1).expect("one past the policy byte budget");
    let (mut core, limits, record) = stored_policy_change(oversized);
    assert_eq!(core.restore_core(CoreRecord::Settings(record), &limits), Restored::Rejected);
    assert_eq!(core.restart_failure(), Some(RestartStep::LoadCore));
    let _refused = core.restart_refuse();
    let failure = RestartRequest::Settings {
        failure: Failure { setting: Setting::Policy(RESTORE_PROJECT), fault: Fault::Shape },
        changed: true,
    };
    assert_eq!(core.restore_failure(), Some(failure));
    assert_eq!(core.restart_done(RestartStep::LoadCore), failure);
    assert_eq!(core.restore_failure(), Some(failure));
    assert!(!core.current.changed(Setting::Policy(RESTORE_PROJECT)));
    assert!(!core.restart_ready());
}

#[test]
fn settings_policy_restore_admits_the_policy_byte_budget() {
    let (mut core, limits, record) = stored_policy_change(RESTORE_POLICY_BYTES);
    assert_eq!(core.restore_core(CoreRecord::Settings(record), &limits), Restored::Live);
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    assert!(core.settings_restored(&env));
    assert_eq!(core.restore_failure(), None);
    assert_eq!(core.restart_done(RestartStep::LoadCore), RestartRequest::Step(RestartStep::LoadInbox));
    assert!(core.current.policy(RESTORE_PROJECT).is_some());
}
