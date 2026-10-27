#![expect(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "ordinary tests inspect complete semantic trees"
)]
use super::cards::{CLIENT, contains, limits, shared_words, text};
use jig_client::contract::*;
use skein_lib::{List, Wall};
use skein_ui::{Builder, Role, Tree};
fn render(refusal: Refusal) -> Tree {
    let mut builder = Builder::new(&limits());
    builder.open(Role::Page);
    shared_words().refusal(&mut builder, &refusal, CLIENT.text);
    builder.close();
    let tree = builder.finish();
    assert!(tree.len() <= crate::refusal_nodes(&CLIENT).unwrap().checked_add(1).unwrap());
    super::cards_access::check(&tree);
    tree
}
#[test]
#[expect(clippy::too_many_lines, reason = "all independent engine refusal categories are checked together")]
fn engine_refusals_keep_categories_supplied_hold_reasons_and_absent_ending_details() {
    for which in [
        AdmissionLimit::Words,
        AdmissionLimit::Identity,
        AdmissionLimit::Roles,
        AdmissionLimit::Arithmetic,
        AdmissionLimit::Duplicate,
        AdmissionLimit::Empty,
        AdmissionLimit::Batch,
        AdmissionLimit::ProjectLiveTasks,
        AdmissionLimit::LiveTasks,
        AdmissionLimit::Tree,
        AdmissionLimit::Depth,
        AdmissionLimit::Delegates,
        AdmissionLimit::Subscription,
        AdmissionLimit::Dependencies,
        AdmissionLimit::Cycle,
        AdmissionLimit::Spec,
        AdmissionLimit::Contract,
        AdmissionLimit::Authority,
        AdmissionLimit::Inputs,
        AdmissionLimit::Attempts,
        AdmissionLimit::LiveDelegates,
        AdmissionLimit::Restore,
        AdmissionLimit::Read,
        AdmissionLimit::Turn,
        AdmissionLimit::HoldKind,
        AdmissionLimit::HoldTaken,
        AdmissionLimit::Holds,
        AdmissionLimit::InboxFull,
        AdmissionLimit::Notes,
        AdmissionLimit::Revision,
        AdmissionLimit::Question,
        AdmissionLimit::Priority,
        AdmissionLimit::Policy,
        AdmissionLimit::Resource,
        AdmissionLimit::Service,
    ] {
        let tree = render(Refusal::Limit { which: Limit::Admission { which } });
        assert!(
            tree.nodes()
                .iter()
                .any(|node| node.text.iter().any(|run| run.text.starts_with(b"Engine admission limit: ")))
        );
        assert_eq!(tree.len(), 2);
    }
    for held in [
        None,
        Some(HeldRole::Owner),
        Some(HeldRole::Maintainer),
        Some(HeldRole::Member),
        Some(HeldRole::Observer),
        Some(HeldRole::Policy { role: 7 }),
        Some(HeldRole::Policy { role: u32::MAX }),
    ] {
        let tree = render(Refusal::Role { held });
        assert!(contains(&tree, b"The held role does not permit this action"));
        assert_eq!(tree.len(), 3);
    }
    for lacking in [
        Lacking::Approver,
        Lacking::Budget { needed: 8, left: 3 },
        Lacking::GrantKind { kind: 9 },
        Lacking::Funding,
        Lacking::TaskKind { executor: None },
        Lacking::Depth { needed: None, allowed: 7 },
        Lacking::Deadline { asked: None, latest: None },
        Lacking::Tasks { needed: None, left: 4 },
        Lacking::Tool,
        Lacking::Scope,
        Lacking::Models,
        Lacking::Reference,
        Lacking::ResourceAccess,
        Lacking::ResourceUnavailable,
        Lacking::Shape,
        Lacking::Again { required: true },
        Lacking::Again { required: false },
        Lacking::Account,
        Lacking::Writer,
        Lacking::Required { connector: 2, kind: 9 },
        Lacking::Failed { connector: 2, kind: 9 },
        Lacking::Unguarded { connector: 2, kind: 9 },
    ] {
        let tree = render(Refusal::Authority { lacking, proposable: None });
        assert!(tree.len() > 1);
        assert!(!contains(&tree, b"Nobody in this project may accept this proposal"));
    }
    for refusal in [Refusal::Taken { by: 7 }, Refusal::NotWaiting, Refusal::NoFurther] {
        assert!(render(refusal).len() > 1);
    }
    for how in [
        Ending::Failed { reason: None },
        Ending::Deleted,
        Ending::Cancelled { by: Actor::ProjectDeployment { project: 6 }, reason: None, at: Some(Wall::EPOCH) },
        Ending::Decided { by: 7, party: None, how: None, at: Wall::EPOCH },
    ] {
        let tree = render(Refusal::Ended { how });
        assert!(!contains(&tree, b"Accepted"));
        assert!(!contains(&tree, b"Rejected"));
    }
    let mut failures = List::with_capacity(2);
    for attempt in [1, 2] {
        failures
            .push(AttemptFailure {
                attempt,
                class: FailureClass::HostLost,
                reason: text(b"whole failure reason"),
                at: Wall::EPOCH,
            })
            .unwrap();
    }
    let tree = render(Refusal::Moved {
        now: Revision { value: 23 },
        what: Moved::Hold { reason: Some(CurrentHold::Document(Box::new(HoldReason::Tries { failures }))) },
    });
    assert!(contains(&tree, b"Current revision 23"));
    assert!(contains(&tree, b"Reason: whole failure reason"));
    assert!(contains(&tree, b"Attempt 1"));
    assert!(contains(&tree, b"Attempt 2"));
}

#[test]
fn sparse_current_hold_facts_keep_only_supplied_categories_and_numeric_identities() {
    let mut reasons = Vec::from([
        HoldCategory::Circle,
        HoldCategory::Stalled,
        HoldCategory::Dependency { task: 123 },
        HoldCategory::Stopped,
        HoldCategory::Placed { hold: 123 },
        HoldCategory::Drift,
        HoldCategory::Effects,
        HoldCategory::EffectFailed,
        HoldCategory::Uncertain { entry: 123 },
        HoldCategory::Budget,
        HoldCategory::Deadline,
        HoldCategory::HoldsWaited,
        HoldCategory::ResourceUnavailable,
        HoldCategory::ResourceAccess,
        HoldCategory::PoolLost { pool: 123 },
        HoldCategory::Procedure,
    ]);
    for class in [
        HoldFailure::Transient,
        HoldFailure::Permanent,
        HoldFailure::Run,
        HoldFailure::Agent,
        HoldFailure::Lost,
        HoldFailure::Invalid,
    ] {
        reasons.push(HoldCategory::Failures { class });
    }
    for reason in reasons {
        let tree = render(Refusal::Moved {
            now: Revision { value: 23 },
            what: Moved::Hold { reason: Some(CurrentHold::Reason(reason)) },
        });
        assert!(contains(&tree, b"Current revision 23"));
        assert!(tree.len() >= 4);
        assert!(!tree.nodes().iter().any(|node| node.role == Role::Time));
        assert!(!contains(&tree, b"Attempt "));
        assert!(!contains(&tree, b"Reason: "));
        match reason {
            HoldCategory::Exceeded(_) => unreachable!("bound phrases have their own table"),
            HoldCategory::Circle => assert!(contains(&tree, b"Held for a circle of waits")),
            HoldCategory::Dependency { .. }
            | HoldCategory::Placed { .. }
            | HoldCategory::Uncertain { .. }
            | HoldCategory::PoolLost { .. } => assert!(contains(&tree, b"123")),
            HoldCategory::Failures { .. }
            | HoldCategory::Stalled
            | HoldCategory::Stopped
            | HoldCategory::Drift
            | HoldCategory::Effects
            | HoldCategory::EffectFailed
            | HoldCategory::Budget
            | HoldCategory::Deadline
            | HoldCategory::HoldsWaited
            | HoldCategory::ResourceUnavailable
            | HoldCategory::ResourceAccess
            | HoldCategory::Procedure => {}
        }
    }
    for (part, label) in [
        (Part::Panel { connector: 2, task: 123 }, b"Task panel".as_slice()),
        (Part::Result { connector: 2, task: 123 }, b"Connector result".as_slice()),
        (Part::Hold { connector: 2, task: 123 }, b"Connector hold".as_slice()),
    ] {
        let tree = render(Refusal::Moved {
            now: Revision { value: 23 },
            what: Moved::Hold { reason: Some(CurrentHold::Document(Box::new(HoldReason::Procedure { part }))) },
        });
        assert!(contains(&tree, label));
        assert!(contains(&tree, b"123"));
    }
}

#[test]
fn outbox_hold_facts_show_the_actual_entry_scope_without_a_proposal_identity() {
    for scope in [PartScope::Project(u32::MAX), PartScope::Deployment] {
        let tree = render(Refusal::Moved {
            now: Revision { value: 9 },
            what: Moved::Hold {
                reason: Some(CurrentHold::Document(Box::new(HoldReason::EffectUncertain {
                    part: Part::Outbox { connector: 2, scope, entry: u64::MAX },
                }))),
            },
        });
        assert!(contains(&tree, b"Outbox entry 18446744073709551615"));
        assert!(!contains(&tree, b"Proposal "));
        match scope {
            PartScope::Project(_) => assert!(contains(&tree, b"Project 4294967295")),
            PartScope::Deployment => assert!(contains(&tree, b"Deployment")),
        }
    }
}

#[test]
fn the_account_sign_in_limits_name_each_refusal() {
    for (which, words) in [
        (AdmissionLimit::Accounts, b"Engine admission limit: accounts".as_slice()),
        (AdmissionLimit::HolderAccounts, b"Engine admission limit: holder accounts".as_slice()),
        (AdmissionLimit::Sources, b"Engine admission limit: sources".as_slice()),
        (AdmissionLimit::SignIns, b"Engine admission limit: account sign-ins".as_slice()),
    ] {
        let tree = render(Refusal::Limit { which: Limit::Admission { which } });
        assert!(contains(&tree, words));
    }
}

#[test]
fn each_configured_bound_names_its_allowance_in_the_reference_view() {
    for (bound, expected) in [
        (
            Bound::Reasoning,
            b"Held: the model's reasoning went past its configured allowance of 18446744073709551615 bytes".as_slice(),
        ),
        (
            Bound::Metadata,
            b"Held: the model's metadata went past its configured allowance of 18446744073709551615 bytes",
        ),
        (Bound::Items, b"Held: the model's answer went past its configured allowance of 18446744073709551615 items"),
        (Bound::Answer, b"Held: the model's answer went past its configured allowance of 18446744073709551615 bytes"),
        (Bound::Tools, b"Held: the run's tools went past their configured allowance of 18446744073709551615 tools"),
        (
            Bound::HistoryItems,
            b"Held: the run's request went past its configured allowance of 18446744073709551615 messages and blocks",
        ),
        (Bound::History, b"Held: the run's history no longer fits its configured allowance, even compacted"),
        (Bound::Window, b"Held: the run no longer fits the model's window of 18446744073709551615 tokens"),
        (Bound::Output, b"Held: the request went past the model's output ceiling of 18446744073709551615 tokens"),
    ] {
        let reason = HoldCategory::Exceeded(Exceeded { bound, value: u64::MAX });
        let tree = render(Refusal::Moved {
            now: Revision { value: 23 },
            what: Moved::Hold { reason: Some(CurrentHold::Reason(reason)) },
        });
        assert!(contains(&tree, expected), "{bound:?}");
    }
}

#[test]
fn project_name_refusals_say_the_failed_rule_and_projects_limit() {
    assert!(contains(&render(Refusal::NameTaken), b"Another project has that name"));
    assert!(contains(
        &render(Refusal::Limit { which: Limit::Admission { which: AdmissionLimit::Projects } }),
        b"The deployment has no room for another project"
    ));
    for (rule, words) in [
        (NameRule::Empty, b"The project name is empty".as_slice()),
        (NameRule::Long, b"The project name is longer than 64 bytes"),
        (NameRule::Control, b"The project name contains a control character"),
        (NameRule::Space, b"The project name has white space at an end"),
        (NameRule::Digits, b"The project name is digits alone"),
    ] {
        assert!(contains(&render(Refusal::Malformed { at: 0, why: Malformed::Name(rule) }), words));
    }
}

#[test]
fn account_move_refusals_show_the_supplied_reason() {
    for (why, words) in [
        (AccountMoveWhy::NotSignedIn, b"not signed in".as_slice()),
        (AccountMoveWhy::NotOurs, b"not ours"),
        (AccountMoveWhy::Held, b"held by another holder"),
        (AccountMoveWhy::Cancelled, b"cancelled"),
        (AccountMoveWhy::Moving, b"moving"),
    ] {
        assert!(contains(&render(Refusal::AccountMove { why }), words));
    }
}
#[test]
fn moving_out_words_show_only_account_and_move_numbers() {
    let mut builder = Builder::new(&limits());
    builder.open(Role::Page);
    let outcome = Outcome::Made {
        what: Made::MovingOut {
            account: 19,
            number: 23,
            record: skein_lib::Secret::new(Box::from(b"private moved record".as_slice())),
        },
    };
    shared_words().outcome(&mut builder, &outcome);
    builder.close();
    let tree = builder.finish();
    assert!(contains(&tree, b"Account 19: moving out, move 23"));
    assert!(!contains(&tree, b"private moved record"));
}

#[test]
fn settings_refusals_name_each_setting_fault_and_retained_holder_fact() {
    for (setting, name) in [
        (Setting::Charter(SettingNumber { number: 2 }), b"charter 2".as_slice()),
        (Setting::Template(SettingNumber { number: 1 }), b"template 1".as_slice()),
        (Setting::Policy(SettingNumber { number: 3 }), b"policy 3".as_slice()),
        (Setting::Rules, b"the rules".as_slice()),
    ] {
        for (fault, words) in [
            (SettingFault::Shape, b"out of shape".as_slice()),
            (SettingFault::Count, b"count limit".as_slice()),
            (SettingFault::Reference, b"not offered".as_slice()),
            (SettingFault::Rules, b"above the deployment rules".as_slice()),
            (SettingFault::Budget, b"short of one costliest completion".as_slice()),
            (SettingFault::Grace, b"past the allowed span".as_slice()),
            (SettingFault::Owner, b"cannot be a root requester".as_slice()),
        ] {
            let tree =
                render(Refusal::Malformed { at: 0, why: Malformed::Settings(MalformedSettings { setting, fault }) });
            assert!(contains(&tree, name));
            assert!(contains(&tree, words));
        }
    }
    assert!(contains(&render(Refusal::Stale(RefusalStale { newest: 29 })), b"newest revision 29"));
    let mut tasks = List::with_capacity(2);
    tasks.push(11).unwrap();
    tasks.push(13).unwrap();
    let tree = render(Refusal::Full(RefusalFull { revision: 7, tasks, more: 17 }));
    for words in [b"oldest held revision 7".as_slice(), b"held by task 11", b"held by task 13", b"17 more tasks"] {
        assert!(contains(&tree, words));
    }
    assert!(contains(&render(Refusal::Malformed { at: 0, why: Malformed::SettingsUnchanged }), b"settings unchanged"));
    assert!(contains(&render(Refusal::Malformed { at: 0, why: Malformed::Running }), b"whether running work moves"));
    let mut builder = Builder::new(&limits());
    builder.open(Role::Page);
    shared_words().outcome(&mut builder, &Outcome::SettingsChanged(OutcomeSettingsChanged { revision: 31 }));
    builder.close();
    assert!(contains(&builder.finish(), b"Settings changed; revision 31"));
}
