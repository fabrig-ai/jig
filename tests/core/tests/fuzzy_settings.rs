//! Random settings decisions agree with their durable rows through cold restarts.
#[path = "settings/support.rs"]
mod support;
use jig_domain_core::{self as core, settings};
use jig_domain_people as people;
use jig_test_domain as root;
use skein_lib::{Duration, Rng};
use support::*;

fn agree(world: &jig_core_world::effects::World) {
    let current = world.domain.settings();
    let mut expected = std::collections::BTreeMap::new();
    let mut changes = std::collections::BTreeMap::new();
    for row in world.store.rows.values() {
        if let root::Record::Core(core::Record::Core(core::CoreRecord::Settings(row))) = row {
            match row {
                settings::Record::Default { setting, default } => {
                    expected.insert(*setting, default.value.clone());
                }
                settings::Record::Changed { setting, changed } => {
                    changes.insert(*setting, changed.value.clone());
                }
                settings::Record::Revision { .. }
                | settings::Record::Extent { .. }
                | settings::Record::Charter { .. }
                | settings::Record::Proposal { .. } => {}
            }
        }
    }
    for (setting, value) in changes {
        match value {
            Some(value) => {
                expected.insert(setting, value);
            }
            None => {
                expected.remove(&setting);
            }
        }
    }
    assert_eq!(current.offered().as_slice(), expected.keys().copied().collect::<Vec<_>>());
    for (setting, value) in expected {
        assert_eq!(current.value(setting), Some(&value));
    }
    assert_eq!(revision(world, current.newest(), false).head, *current.newest_head());
}
#[test]
fn random_small_settings_changes_and_stale_requests_survive_restarts() {
    let count =
        std::env::var("JIG_CORE_SETTINGS_SEEDS").ok().map_or(24, |value| value.parse::<u64>().expect("seed count"));
    let mut coverage = [false; 5];
    for seed in 713..713 + count {
        let result = std::panic::catch_unwind(|| {
            let mut reached = [false; 5];
            let mut rng = Rng::new(seed);
            let mut world = start(2);
            let signed = sign_in(&mut world);
            assert_eq!(revision(&world, 1, false).head.by, settings::Decider::FirstStart);
            assert!(world.store.rows.contains_key(&key(settings::Key::Default(settings::Setting::Rules))));
            assert!(matches!(
                ask(&mut world, signed, 1, people::Ask::MakeProject { name: "Random project".into(), policy: 1 }),
                people::Reply::Outcome(people::Outcome::ProjectMade { .. })
            ));
            assert!(world.store.rows.values().any(|value| *value == record(settings::Record::Extent { oldest: 0 })));
            for step in 0..16_u8 {
                let previous = world.domain.settings().newest();
                let stale = rng.below(4) == 0;
                reached[0] |= stale;
                let against = previous - u32::from(stale);
                let operation = rng.below(3);
                reached[usize::try_from(operation + 1).expect("three edits")] = true;
                let item = match operation {
                    0 => settings::Item { setting: settings::Setting::Template(1), edit: settings::Edit::Reset },
                    1 => template_edit(&world, 6 + rng.below(8)),
                    _ => {
                        let mut charter = world.domain.settings().charter(1).expect("settings fixture").clone();
                        charter.policy.instructions =
                            format!("random settings {seed} {step}").into_bytes().into_boxed_slice();
                        settings::Item {
                            setting: settings::Setting::Charter(1),
                            edit: settings::Edit::Set(settings::Value::Charter(Box::new(charter))),
                        }
                    }
                };
                let before_rows = settings_rows(&world);
                let before = world.domain.settings().value(item.setting).cloned();
                let answer = change(&mut world, signed, step + 10, against, Box::new([item.clone()]));
                if stale {
                    assert_eq!(settings_rows(&world), before_rows);
                    assert_eq!(
                        answer,
                        people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Stale { newest: previous }))
                    );
                } else if world.domain.settings().value(item.setting) == before.as_ref() {
                    assert_eq!(
                        answer,
                        people::Reply::Outcome(people::Outcome::Refused(people::Refusal::NotWellFormed {
                            at: 0,
                            why: people::Malformed::SettingsUnchanged
                        }))
                    );
                } else {
                    changed(&answer, previous + 1);
                }
                agree(&world);
                if rng.below(4) == 0 {
                    reached[4] = true;
                    restart(&mut world, configuration().0);
                    agree(&world);
                }
            }
            let me = frame(&mut world, signed);
            assert_eq!(
                me.projects.get(0).expect("settings fixture").templates.get(0).expect("settings fixture").grace,
                world.domain.settings().template(1).expect("settings fixture").grace
            );
            assert!(
                world.domain.settings().template(1).expect("settings fixture").grace.expect("settings fixture")
                    >= Duration::from_secs(5)
            );
            restart(&mut world, configuration().0);
            agree(&world);
            reached
        });
        let Ok(reached) = result else { panic!("settings replay seed {seed}") };
        for at in 0..coverage.len() {
            coverage[at] |= reached[at];
        }
    }
    assert_eq!(coverage, [true; 5], "stale, reset, template set, charter set and restart all reached");
}

fn pending_settings(world: &jig_core_world::effects::World) -> Vec<(u64, u32, settings::Proposal)> {
    world
        .store
        .rows
        .values()
        .filter_map(|row| match row {
            root::Record::Core(core::Record::Core(core::CoreRecord::Settings(settings::Record::Proposal {
                proposer: people::Proposer::Party(person),
                proposal,
                value,
            }))) => Some((*person, *proposal, value.clone())),
            root::Record::Core(_) | root::Record::Connector { .. } => None,
        })
        .collect()
}
fn proposal_sets_agree(world: &mut jig_core_world::effects::World, signed: u64) {
    let sets = pending_settings(world);
    let pending = inventory(world, signed).pending;
    assert_eq!(pending.len(), u32::try_from(sets.len()).expect("bounded sets"));
    let task_rows: Vec<_> = world
        .store
        .rows
        .values()
        .filter_map(|row| match row {
            root::Record::Core(core::Record::Tasks(jig_domain_tasks::Stored::PersonProposal(row)))
                if row.action.is_settings() =>
            {
                Some((row.proposer, row.number, row.action.clone()))
            }
            root::Record::Core(_) | root::Record::Connector { .. } => None,
        })
        .collect();
    assert_eq!(sets.len(), task_rows.len());
    for (person, proposal, set) in sets {
        assert_eq!(set.against, world.domain.settings().newest());
        assert!(pending.iter().any(|row| row.proposer == people::Proposer::Party(person) && row.number == proposal));
        assert!(task_rows.iter().any(|(proposer, number, action)| *proposer == person
            && *number == u64::from(proposal)
            && *action
                == jig_domain_tasks::PersonAction::Settings {
                    against: set.against,
                    items: settings::proposal_heads(&set.items)
                }));
        let jig_domain_core::documents::ProposalAction::Settings { against, review: Some(review), .. } =
            proposal_document(world, signed, person, u64::from(proposal)).action
        else {
            panic!("pending settings review")
        };
        assert_eq!(against, set.against);
        assert_eq!(review.items.len(), u32::try_from(set.items.len()).expect("bounded items"));
        for (shown, item) in review.items.iter().zip(&set.items) {
            assert_eq!(shown.setting, item.setting);
            let expected = match &item.edit {
                settings::Edit::Set(value) => Some(value),
                settings::Edit::Removed => None,
                settings::Edit::Reset => world.domain.settings().default_value(item.setting),
            };
            assert_eq!(shown.would_be.as_ref(), expected);
        }
    }
}
#[test]
fn random_settings_proposals_decisions_and_restarts_keep_all_three_owners_in_agreement() {
    const SEEDS: u64 = 8;
    const STEPS: u8 = 32;
    let mut coverage = [false; 6];
    let count =
        std::env::var("JIG_CORE_SETTINGS_SEEDS").ok().map_or(SEEDS, |value| value.parse::<u64>().expect("seed count"));
    let first = std::env::var("JIG_SETTINGS_PROPOSALS_SEED")
        .map_or(713, |value| value.parse::<u64>().expect("settings proposal seed"));
    for seed in first..first.checked_add(count).expect("settings seed range") {
        let result = std::panic::catch_unwind(|| {
            let mut reached = [false; 6];
            let mut rng = Rng::new(seed);
            let mut world = start(4);
            let signed = sign_in(&mut world);
            for step in 0..STEPS {
                let operation = usize::try_from(rng.below(6)).expect("six operations");
                reached[operation] = true;
                let pending = pending_settings(&world).first().cloned();
                let key = 20 + step;
                match operation {
                    0 => {
                        let item = template_edit(&world, 6 + rng.below(10));
                        let against = world.domain.settings().newest();
                        propose(&mut world, signed, key, against, Box::new([item]));
                    }
                    1 => {
                        if let Some((person, proposal, _)) = pending {
                            decide(
                                &mut world,
                                signed,
                                key,
                                person,
                                u64::from(proposal),
                                people::ProposalDecision::Accept,
                                Some(people::Running::Stays),
                            );
                        }
                    }
                    2 => {
                        if let Some((person, proposal, _)) = pending {
                            decide(
                                &mut world,
                                signed,
                                key,
                                person,
                                u64::from(proposal),
                                people::ProposalDecision::Reject { reason: b"random rejection".as_slice().into() },
                                None,
                            );
                        }
                    }
                    3 => {
                        if let Some((_, proposal, _)) = pending {
                            ask(
                                &mut world,
                                signed,
                                key,
                                people::Ask::Withdraw { project: 0, proposal: u64::from(proposal) },
                            );
                        }
                    }
                    4 => {
                        let item = template_edit(&world, 6 + rng.below(10));
                        let against = world.domain.settings().newest();
                        change(&mut world, signed, key, against, Box::new([item]));
                    }
                    5 => {
                        restart(&mut world, configuration().0);
                    }
                    _ => unreachable!(),
                }
                agree(&world);
                proposal_sets_agree(&mut world, signed);
            }
            reached
        });
        let Ok(reached) = result else { panic!("settings proposal replay seed {seed}") };
        for (covered, reached) in coverage.iter_mut().zip(reached) {
            *covered |= reached;
        }
    }
    assert_eq!(coverage, [true; 6]);
}
