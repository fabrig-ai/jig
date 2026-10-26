use jig_core_world::{
    effects::fixture,
    reads::{ask, latest},
    stories::reads::{request, world},
};
use jig_domain_core::{documents, reads};
use jig_domain_people as people;
const PROJECT: u32 = 1;
const GOAL: u32 = 2;
const CHANGE: u8 = 1;
const CHANGE_AGAIN: u8 = 2;
const READ: u64 = 1;
const SEED: u64 = 909;
const ME: u64 = 1;

#[test]
fn a_template_override_survives_a_restart_in_the_projects_policy() {
    let mut world = world(SEED);
    request(
        &mut world,
        CHANGE,
        people::Ask::ChangePolicy {
            project: PROJECT,
            change: people::PolicyChange::Templates { overrides: Box::new([override_goal()]) },
        },
    );
    world.restart(SEED, false);
    ask(&mut world, READ, reads::Read::Project { project: PROJECT });
    let reads::Answer::Document(document) = latest(&world) else { panic!("project document") };
    let documents::Document::Project(project) = document.as_ref() else { panic!("project document") };
    assert_eq!(project.policy.templates.as_slice(), &[override_goal()]);
}

#[test]
fn a_stored_override_of_a_dropped_template_is_restored_at_start() {
    let mut world = world(SEED);
    request(
        &mut world,
        CHANGE,
        people::Ask::ChangePolicy {
            project: PROJECT,
            change: people::PolicyChange::Templates { overrides: Box::new([override_goal()]) },
        },
    );
    // This story restores a template override. Restoring older held revisions
    // is a later task, so finish the fixture's unrelated conversation first.
    jig_core_world::fixtures::finish_chat(&mut world, 0, b"Done");
    let mut config = fixture(SEED, false).0;
    config.core.defaults.templates = Box::new([jig_core_world::fixtures::templates()[0].clone()]);
    world.restart_with(config);
    ask(&mut world, READ, reads::Read::Project { project: PROJECT });
    let reads::Answer::Document(document) = latest(&world) else { panic!("project document") };
    let documents::Document::Project(project) = document.as_ref() else { panic!("project document") };
    assert_eq!(project.policy.templates.as_slice(), &[override_goal()]);
}

#[test]
fn a_policy_change_naming_a_dropped_template_is_refused() {
    let mut world = world(SEED);
    request(
        &mut world,
        CHANGE,
        people::Ask::ChangePolicy {
            project: PROJECT,
            change: people::PolicyChange::Templates { overrides: Box::new([override_goal()]) },
        },
    );
    // This story restores a template override. Restoring older held revisions
    // is a later task, so finish the fixture's unrelated conversation first.
    jig_core_world::fixtures::finish_chat(&mut world, 0, b"Done");
    let mut config = fixture(SEED, false).0;
    config.core.defaults.templates = Box::new([jig_core_world::fixtures::templates()[0].clone()]);
    world.restart_with(config);
    request(
        &mut world,
        CHANGE_AGAIN,
        people::Ask::ChangePolicy {
            project: PROJECT,
            change: people::PolicyChange::Templates { overrides: Box::new([override_goal()]) },
        },
    );
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Unknown)))
    );
}

fn override_goal() -> people::TemplateOverride {
    people::TemplateOverride { template: GOAL, executor: None, budget: None, deadline: None, priority: Some(5) }
}

#[test]
fn me_serves_the_projects_templates_merged_with_its_overrides() {
    let mut world = world(SEED);
    request(
        &mut world,
        CHANGE,
        people::Ask::ChangePolicy {
            project: PROJECT,
            change: people::PolicyChange::Templates { overrides: Box::new([override_goal()]) },
        },
    );
    ask(&mut world, READ, reads::Read::Frame);
    let reads::Answer::Document(document) = latest(&world) else { panic!("me document") };
    let documents::Document::Frame(frame) = document.as_ref() else { panic!("me document") };
    let mut expected = jig_core_world::fixtures::templates();
    expected[1].tracked = Some(5);
    assert_eq!(frame.projects.get(0).expect("project").templates.as_slice(), expected.as_ref());
}

#[test]
fn a_template_override_marks_the_owners_me() {
    let mut world = jig_core_world::watches::world(SEED, true);
    let before =
        world.watches.as_ref().expect("watches").deliveries.iter().filter(|(watcher, _, _)| *watcher == ME).count();
    request(
        &mut world,
        CHANGE,
        people::Ask::ChangePolicy {
            project: PROJECT,
            change: people::PolicyChange::Templates { overrides: Box::new([override_goal()]) },
        },
    );
    let watches = world.watches.as_ref().expect("watches");
    assert_eq!(watches.deliveries.iter().filter(|(watcher, _, _)| *watcher == ME).count(), before + 1);
    let documents::Document::Frame(frame) = jig_core_world::watches::latest(&world, ME) else { panic!("me document") };
    assert_eq!(frame.projects.get(0).expect("project").templates.get(1).expect("goal").tracked, Some(5));
    watches.finish();
}
