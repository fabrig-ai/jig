//! Whole domain values cross the settings schema without an encoded-record envelope.
use super::fixtures;
use crate::codec;
use jig_domain_authority as a;
use jig_domain_core::{self as core, settings as s};
use jig_domain_fleet as fleet;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{Duration, Wall};

fn model() -> core::Model {
    core::Model {
        dialect: 3,
        endpoint: 4,
        program: Some(Box::from(b"program".as_slice())),
        name: Box::from(b"model".as_slice()),
        window: 8192,
        output: 1024,
        input_price: 1,
        cached_price: 2,
        cache_write_price: Some(3),
        output_price: 4,
        price_unit: 1000,
        efforts: Box::new([Box::from(b"low".as_slice()), Box::from(b"high".as_slice())]),
        effort: Some(Box::from(b"high".as_slice())),
    }
}
fn charter() -> core::Charter {
    core::Charter {
        number: 1,
        hosts: fleet::Kinds::Both,
        spend: 100,
        policy: core::RunPolicy {
            instructions: Box::from(b"instructions\nwhole".as_slice()),
            waiting: Duration::from_secs(2),
            resume: true,
            turns: 4,
            time: Duration::from_secs(30),
            reserve_turns: 1,
            reserve_time: Duration::from_secs(5),
            model: model(),
            alternatives: Box::new([model()]),
            inspect: true,
            modify: false,
            shell: true,
            agents: false,
            call_timeout: Duration::from_secs(1),
        },
    }
}
fn pattern() -> a::Pattern {
    a::Pattern { segments: Box::new([Box::from(b"one".as_slice())]), last: a::Last::Open(Box::from(b"two".as_slice())) }
}
fn authority() -> a::Authority {
    a::Authority {
        tools: a::Tools(7),
        models: a::Models::from_parts(3, false),
        grants: Box::new([a::Grant { connector: 1, kind: 2, pattern: pattern() }]),
        delegation: a::Delegation {
            kinds: Box::new([a::Executor::Charter(1), a::Executor::Procedure(2), a::Executor::Role(0)]),
            tasks: 2,
            depth: 1,
        },
        budget: a::Budget { spend: 100, deadline: Some(Wall::from_nanos(50)) },
        notes: a::Scopes(7),
        note_resources: Box::new([a::ResourceScope { connector: 1, pattern: pattern() }]),
    }
}
fn requirement() -> a::Requirement {
    a::Requirement {
        connector: 1,
        kind: 2,
        pattern: pattern(),
        judge: a::Judge { connector: 1, requirement: 3, parameters: 4 },
        guard: a::Guard::Observed { freshness: Duration::from_secs(1) },
        must_be_guarded: true,
    }
}
fn rules() -> a::Rules {
    a::Rules {
        ceiling: authority(),
        period_spend: 500,
        minimum_run_spend: 1,
        maximum_run_spend: 100,
        implies: a::Implies::new(Box::new([]), 0).unwrap(),
        requirements: Box::new([requirement()]),
    }
}
fn policy() -> core::ConfiguredPolicy {
    core::ConfiguredPolicy {
        policy: a::Policy {
            escalation_role: Some(0),
            ceiling: authority(),
            projections: Box::new([a::Grant { connector: 1, kind: 2, pattern: pattern() }]),
            period_spend: 400,
            roles: Box::new([a::Role {
                number: 0,
                authority: authority(),
                period_spend: 300,
                requests: a::Requests::ALL,
                decides: a::Proposals::ALL,
            }]),
            requirements: Box::new([requirement()]),
        },
        permissions: Box::new([people::PermissionRole { connector: 1, permission: 2, role: 0 }]),
        templates: Box::new([people::TemplateOverride {
            template: 1,
            executor: Some(people::TemplateExecutor::Chosen),
            budget: Some(people::TemplateBudget::Amount(20)),
            deadline: Some(people::TemplateDeadline::Span(Duration::from_secs(5))),
            priority: Some(3),
        }]),
    }
}
pub(super) fn template() -> core::Template {
    core::Template {
        number: 1,
        name: Box::from(b"template".as_slice()),
        executor: people::TemplateExecutor::Charter(1),
        contract: tasks::Contract::Verdict {
            choices: Box::new([tasks::Verdict {
                code: 1,
                words: 3,
                followups: 0,
                fields: Box::new([tasks::FieldRule { name: Box::from(b"field".as_slice()), max: 8 }]),
            }]),
        },
        shallower: 1,
        budget: people::TemplateBudget::AsAllowed { cap: Some(99) },
        deadline: people::TemplateDeadline::Given,
        tracked: Some(4),
        flags: tasks::Flags {
            conversation: true,
            listed: false,
            retention: tasks::Retention::ForgottenOnceRead,
            ended_by: tasks::EndedBy::Requester,
        },
        label: 7,
        grace: Some(Duration::from_secs(1)),
        llm: Some(tasks::LlmChoice {
            account: Some(3),
            main: tasks::ModelEffort { model: 0, effort: Some(1) },
            subagents: Box::new([tasks::ModelEffort { model: 1, effort: None }]),
        }),
    }
}
#[test]
fn each_settings_record_round_trips_its_whole_domain_value() {
    let mut domain = fixtures::CORE_LIMITS;
    domain.run_bytes = 4096;
    domain.authority.executors = 3;
    let limits = codec::limits(&domain).unwrap();
    let head = s::Head {
        number: 3,
        against: 2,
        at: Wall::from_nanos(45),
        by: s::Decider::Party(7),
        proposal: Some((people::Proposer::Task(8), 9)),
        moved: true,
    };
    let mut rows = skein_lib::List::with_capacity(52);
    for row in [
        s::Record::Extent { oldest: 1 },
        s::Record::Revision {
            history: false,
            revision: s::Revision { head: s::Head { number: 1, against: 0, ..head.clone() }, items: Box::new([]) },
        },
        s::Record::Charter { charter: 1, revision: 2, value: s::CharterVersion { charter: Some(Box::new(charter())) } },
        s::Record::Charter { charter: 1, revision: 3, value: s::CharterVersion { charter: None } },
    ] {
        rows.push(row).expect("settings fixture count");
    }
    for (setting, value) in [
        (s::Setting::Charter(1), s::Value::Charter(Box::new(charter()))),
        (s::Setting::Template(1), s::Value::Template(Box::new(template()))),
        (s::Setting::Policy(1), s::Value::Policy(Box::new(policy()))),
        (s::Setting::Rules, s::Value::Rules(Box::new(rules()))),
    ] {
        rows.push(s::Record::Default { setting, default: s::Default { value: value.clone(), revision: 2 } })
            .expect("settings fixture count");
        rows.push(s::Record::Changed { setting, changed: s::Changed { value: Some(value.clone()), revision: 3 } })
            .expect("settings fixture count");
        rows.push(s::Record::Changed { setting, changed: s::Changed { value: None, revision: 3 } })
            .expect("settings fixture count");
        let items = Box::new([
            s::Item { setting, edit: s::Edit::Set(value.clone()) },
            s::Item { setting, edit: s::Edit::Removed },
            s::Item { setting, edit: s::Edit::Reset },
        ]);
        for replaced in [
            s::Replaced::Value(Box::new(value.clone())),
            s::Replaced::Default,
            s::Replaced::Removed,
            s::Replaced::NotOffered,
        ] {
            for history in [false, true] {
                rows.push(s::Record::Revision {
                    history,
                    revision: s::Revision {
                        head: head.clone(),
                        items: Box::new([s::RevisionItem {
                            setting,
                            edit: s::Edit::Set(value.clone()),
                            replaced: replaced.clone(),
                        }]),
                    },
                })
                .expect("settings fixture count");
            }
        }
        rows.push(s::Record::Proposal {
            proposer: people::Proposer::Party(6),
            proposal: 7,
            value: s::Proposal { against: 2, items },
        })
        .expect("settings fixture count");
    }
    let names = Box::new([catalogue_name(b"one"), catalogue_name(b"two")]);
    for row in rows.into_boxed() {
        let row = core::Record::Core(core::CoreRecord::Settings(row));
        let bytes = codec::encode(&row, &limits, &*names).expect("bounded settings row");
        assert_eq!(codec::decode(&row.key(), &bytes, &limits, &*names), Some(row.clone()), "{:?}", row.key());
        assert!(codec::decode(&row.key(), &bytes[..bytes.len() - 1], &limits, &*names).is_none());
    }
}

#[test]
fn frozen_model_masks_and_template_choices_follow_names_after_a_catalogue_reorder() {
    let mut domain = fixtures::CORE_LIMITS;
    domain.authority.executors = 3;
    let limits = codec::limits(&domain).unwrap();
    let before = [catalogue_name(b"one"), catalogue_name(b"two")];
    let after = [before[1].clone(), before[0].clone()];
    let mut rules = rules();
    rules.ceiling.models = a::Models::Named(1);
    let rows = [
        s::Record::Changed {
            setting: s::Setting::Rules,
            changed: s::Changed { revision: 1, value: Some(s::Value::Rules(Box::new(rules))) },
        },
        s::Record::Changed {
            setting: s::Setting::Template(1),
            changed: s::Changed { revision: 1, value: Some(s::Value::Template(Box::new(template()))) },
        },
    ];
    for value in rows {
        let row = core::Record::Core(core::CoreRecord::Settings(value));
        let bytes = codec::encode(&row, &limits, &before).unwrap();
        let core::Record::Core(core::CoreRecord::Settings(s::Record::Changed { changed, .. })) =
            codec::decode(&row.key(), &bytes, &limits, &after).unwrap()
        else {
            panic!("changed setting")
        };
        match changed.value.unwrap() {
            s::Value::Rules(value) => assert_eq!(value.ceiling.models, a::Models::Named(2)),
            s::Value::Template(value) => {
                let choice = value.llm.unwrap();
                assert_eq!(choice.main.model, 1);
                assert_eq!(choice.subagents[0].model, 0);
            }
            s::Value::Policy(_) | s::Value::Charter(_) => panic!("fixture setting"),
        }
    }
}

fn catalogue_name(name: &[u8]) -> s::ModelName {
    s::ModelName {
        name: Box::from(name),
        efforts: Box::new([Box::from(b"low".as_slice()), Box::from(b"high".as_slice())]),
    }
}
#[test]
fn live_settings_remap_efforts_by_name_and_refuse_missing_efforts() {
    let limits = codec::limits(&fixtures::CORE_LIMITS).unwrap();
    let before = [catalogue_name(b"one"), catalogue_name(b"two")];
    let mut after = before.clone();
    after[0].efforts = Box::new([Box::from(b"high".as_slice()), Box::from(b"low".as_slice())]);
    let value = s::Value::Template(Box::new(template()));
    for original in [
        s::Record::Default {
            setting: s::Setting::Template(1),
            default: s::Default { value: value.clone(), revision: 2 },
        },
        s::Record::Changed {
            setting: s::Setting::Template(1),
            changed: s::Changed { value: Some(value.clone()), revision: 2 },
        },
        s::Record::Proposal {
            proposer: people::Proposer::Party(1),
            proposal: 1,
            value: s::Proposal {
                against: 1,
                items: Box::new([s::Item { setting: s::Setting::Template(1), edit: s::Edit::Set(value.clone()) }]),
            },
        },
    ] {
        let original = core::Record::Core(core::CoreRecord::Settings(original));
        let bytes = codec::encode(&original, &limits, &before).unwrap();
        for (names, expected_model, expected_effort) in
            [(&after, 0, 0), (&[catalogue_name(b"gone"), catalogue_name(b"two")], u8::MAX, 1)]
        {
            let decoded = codec::decode(&original.key(), &bytes, &limits, names).unwrap();
            let core::Record::Core(core::CoreRecord::Settings(record)) = decoded else { panic!("settings") };
            let value = match record {
                s::Record::Default { default, .. } => default.value,
                s::Record::Changed { changed, .. } => changed.value.unwrap(),
                s::Record::Proposal { value, .. } => match value.items[0].edit.clone() {
                    s::Edit::Set(value) => value,
                    s::Edit::Removed | s::Edit::Reset => panic!("set"),
                },
                s::Record::Revision { .. } | s::Record::Extent { .. } | s::Record::Charter { .. } => {
                    panic!("live fixture")
                }
            };
            let s::Value::Template(value) = value else { panic!("template") };
            assert_eq!(
                value.llm.unwrap().main,
                tasks::ModelEffort { model: expected_model, effort: Some(expected_effort) }
            );
        }
    }
    let mut missing = before.clone();
    missing[0].efforts = Box::new([Box::from(b"low".as_slice())]);
    let row = core::Record::Core(core::CoreRecord::Settings(s::Record::Changed {
        setting: s::Setting::Template(1),
        changed: s::Changed { value: Some(value), revision: 2 },
    }));
    let bytes = codec::encode(&row, &limits, &before).unwrap();
    let Some(core::Record::Core(core::CoreRecord::Settings(s::Record::Changed { changed, .. }))) =
        codec::decode(&row.key(), &bytes, &limits, &missing)
    else {
        panic!("changed")
    };
    let s::Value::Template(value) = changed.value.unwrap() else { panic!("template") };
    assert_eq!(value.llm.unwrap().main.model, u8::MAX);
}

#[test]
fn replaced_masks_and_template_choices_follow_a_reordered_catalogue() {
    let mut domain = fixtures::CORE_LIMITS;
    domain.authority.executors = 3;
    let limits = codec::limits(&domain).unwrap();
    let before = [catalogue_name(b"one"), catalogue_name(b"two")];
    let after = [before[1].clone(), before[0].clone()];
    let mut policy = policy();
    policy.policy.ceiling.models = a::Models::Named(1);
    policy.policy.roles[0].authority.models = a::Models::Named(1);
    let mut rules = rules();
    rules.ceiling.models = a::Models::Named(1);
    let revision = s::Revision {
        head: s::Head {
            number: 2,
            against: 1,
            at: Wall::EPOCH,
            by: s::Decider::Party(1),
            proposal: None,
            moved: false,
        },
        items: Box::new([
            s::RevisionItem {
                setting: s::Setting::Policy(1),
                edit: s::Edit::Removed,
                replaced: s::Replaced::Value(Box::new(s::Value::Policy(Box::new(policy)))),
            },
            s::RevisionItem {
                setting: s::Setting::Template(1),
                edit: s::Edit::Removed,
                replaced: s::Replaced::Value(Box::new(s::Value::Template(Box::new(template())))),
            },
            s::RevisionItem {
                setting: s::Setting::Rules,
                edit: s::Edit::Reset,
                replaced: s::Replaced::Value(Box::new(s::Value::Rules(Box::new(rules)))),
            },
        ]),
    };
    let row = core::Record::Core(core::CoreRecord::Settings(s::Record::Revision { history: true, revision }));
    let bytes = codec::encode(&row, &limits, &before).unwrap();
    let core::Record::Core(core::CoreRecord::Settings(s::Record::Revision { revision, .. })) =
        codec::decode(&row.key(), &bytes, &limits, &after).unwrap()
    else {
        panic!("revision")
    };
    let s::Replaced::Value(value) = &revision.items[0].replaced else { panic!("policy before-state") };
    let s::Value::Policy(policy) = value.as_ref() else { panic!("policy") };
    assert_eq!(policy.policy.ceiling.models, a::Models::Named(2));
    assert_eq!(policy.policy.roles[0].authority.models, a::Models::Named(2));
    let s::Replaced::Value(value) = &revision.items[1].replaced else { panic!("template before-state") };
    let s::Value::Template(template) = value.as_ref() else { panic!("template") };
    let llm = template.llm.as_ref().unwrap();
    assert_eq!(llm.main.model, 1);
    assert_eq!(llm.subagents[0].model, 0);
    let s::Replaced::Value(value) = &revision.items[2].replaced else { panic!("rules before-state") };
    let s::Value::Rules(rules) = value.as_ref() else { panic!("rules") };
    assert_eq!(rules.ceiling.models, a::Models::Named(2));
}

#[test]
fn revision_piece_bounds_and_malformed_lengths_name_the_items() {
    use crate::settings_v1 as w;
    use skein_lib::{Reader, Writer};
    let limits = codec::limits(&fixtures::CORE_LIMITS).unwrap();
    let row = s::Record::Revision {
        history: true,
        revision: s::Revision {
            head: s::Head {
                number: 2,
                against: 1,
                at: Wall::EPOCH,
                by: s::Decider::Party(1),
                proposal: None,
                moved: false,
            },
            items: Box::new([s::RevisionItem {
                setting: s::Setting::Template(1),
                edit: s::Edit::Removed,
                replaced: s::Replaced::Default,
            }]),
        },
    };
    let encoded = codec::settings::encode(&row, &limits.settings, &[]).unwrap();
    let mut parts = w::Revision::decode(&limits.settings, &mut Reader::new(&encoded)).unwrap().into_parts();
    let valid = parts.items.clone();
    let mut oversized = Writer::new(usize::try_from(limits.settings.revision_items).unwrap() + 1);
    for _ in 0..=limits.settings.revision_items {
        oversized.put(&[0]).unwrap();
    }
    let mut trailing = Writer::new(valid.len() + 1);
    trailing.put(&valid).unwrap();
    trailing.put(&[0]).unwrap();
    let mut short = valid.clone();
    short[3] += 1;
    for (items, reason) in [
        (oversized.finish(), skein_codec::Reason::Bound),
        (Box::from([0_u8].as_slice()), skein_codec::Reason::Malformed),
        (trailing.finish(), skein_codec::Reason::Malformed),
        (short, skein_codec::Reason::Malformed),
    ] {
        parts.items = items;
        let wire = w::Revision::new(&w::CEILINGS, parts.clone()).unwrap();
        let mut out = Writer::new(usize::try_from(wire.measure()).unwrap());
        wire.encode(&mut out).unwrap();
        let error =
            codec::settings::decode(&s::Key::History(2), &out.finish(), &limits.settings, &[], &limits.settings_domain)
                .unwrap_err();
        assert_eq!(error.path, w::Path::RevisionItems);
        assert_eq!(error.reason, reason);
    }
}
