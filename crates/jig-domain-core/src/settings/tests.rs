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
    let crate::Write::Save(crate::Record::Core(CoreRecord::Settings(Record::Extent { oldest: stored }))) =
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
        let writes = apply(&mut current, &reset, 3);
        let mut saved_reset = false;
        for write in writes.as_slice() {
            if let crate::Write::Save(crate::Record::Core(CoreRecord::Settings(Record::Default {
                setting: saved,
                default,
            }))) = write
            {
                saved_reset |= *saved == setting && default.revision == 3;
            }
        }
        assert!(saved_reset);
        assert_eq!(current.lookup(setting).1, Some((false, 3)));
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
        let _writes = apply(&mut current, &[Item { setting, edit: Edit::Reset }], 4);
        assert_eq!(current.lookup(setting), (Some(&default), Some((false, 4))));
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

fn version_head(core: &Core, revision: u32) -> Head {
    Head { number: revision, against: revision.saturating_sub(1), ..core.current.head.clone() }
}
fn held_fixture() -> (Core, Limits) {
    let (mut core, limits) = crate::tests::core();
    let mut charter = core.charter(1).unwrap().clone();
    charter.number = 7;
    charter.spend = 1;
    core.current.defaults.remove(&Setting::Charter(1)).unwrap();
    core.current
        .defaults
        .insert(Setting::Charter(7), Default { value: Value::Charter(Box::new(charter)), revision: 1 })
        .unwrap();
    (core, limits)
}
fn version_edit(core: &mut Core, limits: &Limits, number: u32, charter: u32, edit: Edit, keep_previous: bool) {
    let previous = core.current.newest();
    let items = [Item { setting: Setting::Charter(charter), edit }];
    let plan = core.held.plan(&core.current, &items, number, if keep_previous { Some(previous) } else { None }, limits);
    assert!(plan.versions() <= limits.settings.versions);
    let mut out = skein_lib::Queue::with_capacity(64);
    core.held.retain(&core.current, &plan, &mut out);
    if keep_previous {
        core.held.heads.insert(previous, core.current.head.clone()).unwrap();
    }
    let _writes = apply(&mut core.current, &items, number);
    core.current.head = version_head(core, number);
}
fn version_value(core: &Core, charter: u32, spend: u64) -> Edit {
    let source = match core.charter(charter) {
        Some(charter) => charter,
        None => core.charter(7).unwrap(),
    };
    let mut value = source.clone();
    value.number = charter;
    value.spend = spend;
    Edit::Set(Value::Charter(Box::new(value)))
}
fn held_versions() -> (Core, Limits) {
    let (mut core, limits) = held_fixture();
    let edit = version_value(&core, 7, 3);
    version_edit(&mut core, &limits, 3, 7, edit, true);
    core.current.head = version_head(&core, 4);
    let edit = version_value(&core, 7, 5);
    version_edit(&mut core, &limits, 5, 7, edit, true);
    core.current.head = version_head(&core, 6);
    (core, limits)
}
#[test]
fn a_revision_reads_the_latest_version_at_or_before_it() {
    let (core, _) = held_versions();
    for (revision, expected) in [(1, 1), (4, 3), (6, 5)] {
        assert_eq!(core.charter_at(7, revision).unwrap().spend, expected);
    }
    assert_eq!(
        core.charter_at(7, 1),
        core.held.versions.get(&VersionKey { charter: 7, revision: 1 }).unwrap().charter.as_deref()
    );
    assert!(core.charter_at(7, 2).is_none());
}
#[test]
fn a_removal_is_a_version_from_which_the_charter_is_not_offered() {
    let (mut core, limits) = held_fixture();
    let edit = version_value(&core, 8, 2);
    version_edit(&mut core, &limits, 2, 8, edit, false);
    core.current.head = version_head(&core, 3);
    version_edit(&mut core, &limits, 4, 8, Edit::Removed, true);
    core.current.head = version_head(&core, 5);
    assert!(!core.current.changed(Setting::Charter(8)));
    assert_eq!(core.charter_at(8, 3).unwrap().spend, 2);
    assert!(core.charter_at(8, 5).is_none());
    assert!(core.charter(8).is_none());
    assert!(core.held.versions.get(&VersionKey { charter: 8, revision: 4 }).unwrap().charter.is_none());
}
#[test]
fn versions_no_held_revision_reads_are_dropped() {
    let (mut core, limits) = held_versions();
    core.held.heads.remove(&1).unwrap();
    let plan = core.held.plan(&core.current, &[], core.current.newest(), None, &limits);
    let mut out = skein_lib::Queue::with_capacity(64);
    core.held.retain(&core.current, &plan, &mut out);
    assert!(!core.held.versions.contains_key(&VersionKey { charter: 7, revision: 1 }));
    assert!(core.held.versions.contains_key(&VersionKey { charter: 7, revision: 3 }));
    assert_eq!(core.charter_at(7, 4).unwrap().spend, 3);
}

#[test]
fn releasing_multiple_revisions_writes_one_extent_after_all_copies_and_erases() {
    let (mut core, mut limits) = held_versions();
    limits.settings.revisions = 2;
    limits.settings.versions = 2;
    core.current.oldest = 0;
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let mut out = skein_lib::Queue::with_capacity(release_writes(&limits).unwrap() + 1);
    core.release_revision(&env, 4, &mut out);
    core.release_revision(&env, 1, &mut out);
    out.push(crate::Request::Decided);
    core.finish_extent(0, &mut out);
    assert_eq!(out.len(), 8, "two copies, two Held erases, two Charter erases, one extent and the mark");
    let mut extents = 0_u32;
    for request in &out {
        if let crate::Request::Write(crate::Write::Save(crate::Record::Core(CoreRecord::Settings(Record::Extent {
            oldest,
        })))) = request
        {
            extents += 1;
            assert_eq!(*oldest, 1);
        }
    }
    assert_eq!(extents, 1);
    for _ in 0_u32..7 {
        if let Some(crate::Request::Decided) = out.pop() {
            panic!("the mark follows every write");
        }
    }
    let Some(crate::Request::Decided) = out.pop() else { panic!("decision mark") };
}
#[test]
fn a_reset_is_a_version_made_at_the_reset() {
    let (mut core, limits) = held_fixture();
    let original = core.charter(7).unwrap().clone();
    let edit = version_value(&core, 7, 3);
    version_edit(&mut core, &limits, 3, 7, edit, false);
    core.current.head = version_head(&core, 4);
    version_edit(&mut core, &limits, 5, 7, Edit::Reset, true);
    assert!(!core.current.changed(Setting::Charter(7)));
    assert_eq!(core.current.lookup(Setting::Charter(7)).1, Some((false, 5)));
    core.current.head = version_head(&core, 6);
    let edit = version_value(&core, 7, 7);
    version_edit(&mut core, &limits, 7, 7, edit, true);
    assert_eq!(core.charter_at(7, 4).unwrap().spend, 3);
    assert_eq!(core.charter_at(7, 6), Some(&original));
    assert_eq!(core.charter_at(7, 7).unwrap().spend, 7);
}
#[test]
fn an_erased_removal_bars_older_versions_from_later_revisions() {
    let (mut core, limits) = held_fixture();
    let edit = version_value(&core, 8, 2);
    version_edit(&mut core, &limits, 2, 8, edit, false);
    core.current.head = version_head(&core, 3);
    version_edit(&mut core, &limits, 4, 8, Edit::Removed, true);
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    core.tell_replaced_settings(&env, false, true);
    assert!(core.tasks.charter_models(8, 3).is_some());
    assert!(core.tasks.charter_models(8, 4).is_none());
    core.current.head = version_head(&core, 5);
    let edit = version_value(&core, 8, 6);
    version_edit(&mut core, &limits, 6, 8, edit, true);
    assert_eq!(core.charter_at(8, 3).unwrap().spend, 2);
    assert!(core.charter_at(8, 5).is_none());
    assert_eq!(core.charter_at(8, 6).unwrap().spend, 6);
    core.held.heads.remove(&3).unwrap();
    let plan = core.held.plan(&core.current, &[], core.current.newest(), None, &limits);
    let mut out = skein_lib::Queue::with_capacity(64);
    core.held.retain(&core.current, &plan, &mut out);
    assert!(!core.held.versions.contains_key(&VersionKey { charter: 8, revision: 2 }));
    assert!(core.charter_at(8, 5).is_none());
}

#[test]
fn a_barrier_leaves_with_the_last_older_value_even_while_absence_is_held() {
    let (mut core, limits) = held_fixture();
    let edit = version_value(&core, 8, 2);
    version_edit(&mut core, &limits, 2, 8, edit, false);
    core.current.head = version_head(&core, 3);
    version_edit(&mut core, &limits, 4, 8, Edit::Removed, true);
    core.held.heads.insert(4, version_head(&core, 4)).unwrap();
    core.current.head = version_head(&core, 5);
    core.held.heads.remove(&3).unwrap();
    let plan = core.held.plan(&core.current, &[], 5, None, &limits);
    let mut out = skein_lib::Queue::with_capacity(64);
    core.held.retain(&core.current, &plan, &mut out);
    assert!(core.held.versions.is_empty());
    assert!(core.charter_at(8, 4).is_none());
    assert!(core.charter_at(8, 5).is_none());
}

#[test]
fn removal_markers_fit_beside_offered_charters_and_held_versions() {
    let (mut core, mut limits) = held_fixture();
    limits.tasks.charters = 2;
    limits.settings.versions = 1;
    let mut unchanged = core.charter(7).unwrap().clone();
    unchanged.number = 9;
    core.current
        .defaults
        .insert(Setting::Charter(9), Default { value: Value::Charter(Box::new(unchanged)), revision: 1 })
        .unwrap();
    let items = [
        Item { setting: Setting::Charter(7), edit: Edit::Removed },
        Item { setting: Setting::Charter(8), edit: version_value(&core, 8, 2) },
    ];
    let plan = core.held.plan(&core.current, &items, 2, Some(1), &limits);
    assert_eq!(plan.versions(), 1, "the newest removal consumes no extra version");
    let mut out = skein_lib::Queue::with_capacity(64);
    core.held.retain(&core.current, &plan, &mut out);
    core.held.heads.insert(1, core.current.head.clone()).unwrap();
    let _writes = apply(&mut core.current, &items, 2);
    core.current.head = version_head(&core, 2);
    let models = crate::charters::held_models(&core, &limits);
    assert_eq!(models.len(), 4);
    assert_eq!(core.held.versions.len(), 1);
    assert!(core.charter_at(7, 1).is_some());
    assert!(core.charter_at(7, 2).is_none());
    let mut tasks_out = skein_lib::Queue::with_capacity(task_room(&limits));
    tasks::step(
        &mut core.tasks,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: task_limits(&limits) },
        tasks::Event::Charters { newest: 2, charters: models, efforts: crate::charters::efforts(&core.settings) },
        &mut tasks_out,
    );
    assert!(tasks_out.is_empty());
    assert!(core.tasks.charter_models(7, 1).is_some());
    assert!(core.tasks.charter_models(7, 2).is_none());
    assert!(core.tasks.charter_models(8, 2).is_some());
    assert!(core.tasks.charter_models(9, 2).is_some());
}

fn restore_settings_fixture() -> (Core, Limits) {
    let limits = limits();
    let mut core = Core::new(config(41), &limits).unwrap();
    let mut defaults = List::with_capacity(core.current.defaults.len());
    for (&setting, default) in &core.current.defaults {
        let mut default = default.clone();
        if let Setting::Charter(_) = setting {
            default.revision = 2;
        }
        defaults.push(Record::Default { setting, default }).unwrap();
    }
    for row in defaults.into_boxed() {
        assert!(core.restore_setting(row, &limits));
    }
    assert!(core.restore_setting(Record::Extent { oldest: 0 }, &limits));
    (core, limits)
}

fn restore_head(core: &mut Core, limits: &Limits, number: u32) -> bool {
    let head = version_head(core, number);
    core.restore_setting(Record::Revision { history: false, revision: Revision { head, items: Box::new([]) } }, limits)
}

#[test]
fn two_held_rows_and_a_charter_row_restore_before_tasks() {
    for order in [[1, 2], [2, 1]] {
        let (mut core, limits) = restore_settings_fixture();
        let charter = core.charter(1).unwrap().clone();
        for number in order {
            assert!(restore_head(&mut core, &limits, number));
        }
        assert!(core.restore_setting(
            Record::Charter {
                charter: 1,
                revision: 1,
                value: CharterVersion { charter: Some(Box::new(charter.clone())) },
            },
            &limits
        ));
        let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
        assert!(core.settings_restored(&env));
        assert_eq!(core.current.newest(), 2);
        assert_eq!(core.held.heads.len(), 1);
        assert_eq!(core.held.heads.get(&1).unwrap().number, 1);
        assert_eq!(
            core.held.versions.get(&VersionKey { charter: 1, revision: 1 }).unwrap().charter.as_deref(),
            Some(&charter)
        );
        assert!(core.tasks.charter_models(1, 1).is_some());
        assert!(!restore_head(&mut core, &limits, 3), "the settings range has ended");
    }
}

#[test]
fn a_charter_row_past_the_limit_refuses_without_retaining_it() {
    let (mut core, mut limits) = restore_settings_fixture();
    limits.settings.versions = 0;
    assert!(restore_head(&mut core, &limits, 2));
    assert!(!core.restore_setting(
        Record::Charter {
            charter: 1,
            revision: 1,
            value: CharterVersion { charter: Some(Box::new(core.charter(1).unwrap().clone())) },
        },
        &limits
    ));
    assert!(core.held.versions.is_empty());
    let refusal = RestartRequest::HeldPastLimit { which: crate::HeldLimit::Versions, held: 1, limit: 0 };
    assert_eq!(core.restore_failure(), Some(refusal));
    assert_eq!(core.restart_done(RestartStep::LoadCore), refusal);
}

#[test]
fn a_held_revision_left_without_holders_is_released_by_the_start() {
    let (mut core, limits) = restore_settings_fixture();
    assert!(restore_head(&mut core, &limits, 1));
    assert!(restore_head(&mut core, &limits, 2));
    let mut old = core.charter(1).unwrap().clone();
    old.policy.instructions = b"An orphan version".as_slice().into();
    assert!(core.restore_setting(
        Record::Charter { charter: 1, revision: 1, value: CharterVersion { charter: Some(Box::new(old)) } },
        &limits
    ));
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let crate::Requests::Out(mut out) = core.commit_restored_settings(&env, crate::room_max(&limits).unwrap().writes);
    let mut copied = false;
    let mut erased_head = false;
    let mut erased_version = false;
    let mut saved_extent = false;
    while let Some(request) = out.pop() {
        if let crate::Request::Write(write) = request {
            if let crate::Write::CopySettingsRevision(number) = &write {
                copied |= *number == 1;
            }
            if let crate::Write::Erase(key) = &write {
                erased_head |= *key == crate::Key::Core(crate::CoreKey::Settings(Key::Held(1)));
                erased_version |=
                    *key == crate::Key::Core(crate::CoreKey::Settings(Key::Charter { charter: 1, revision: 1 }));
            }
            if let crate::Write::Save(crate::Record::Core(CoreRecord::Settings(Record::Extent { oldest }))) = write {
                saved_extent |= oldest == 1;
            }
        }
    }
    assert!(core.held.heads.is_empty());
    assert!(core.held.versions.is_empty());
    assert!(copied);
    assert!(erased_head);
    assert!(erased_version);
    assert!(saved_extent);
}

#[test]
fn a_settings_commit_before_the_first_start_seed_keeps_the_supplied_defaults() {
    let limits = limits();
    let configured = config(41);
    let rules = configured.defaults.rules.clone();
    let mut core = Core::new(configured, &limits).unwrap();
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let crate::Requests::Out(out) = core.commit_restored_settings(&env, crate::room_max(&limits).unwrap().writes);
    assert!(out.is_empty());
    assert_eq!(core.current.rules(), &rules);
    let mut writes = skein_lib::Queue::with_capacity(crate::room_max(&limits).unwrap().writes);
    assert!(restart::first_start(&mut core, &env, &mut writes));
    assert!(!writes.is_empty());
    assert_eq!(core.current.newest(), 1);
}
