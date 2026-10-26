//! Project numbers fit the public u32 space without spending a refused number (core.md, 6.1).
use super::*;
const POLICY: u32 = 1;
const MAKER: u64 = 1;
// The seed and two makes, plus the deployment's roster.
const PROJECTS: u32 = 3;
const ROSTERS: u32 = PROJECTS + 1;
fn project_core() -> (Core, Limits) {
    let mut limits = limits();
    limits.authority.projects = PROJECTS;
    limits.people.projects = ROSTERS;
    let mut config = config(41);
    config.defaults.rules =
        (authority::Domain::new(config.defaults.rules.clone(), limits.authority).expect("room")).rules().clone();
    let seed = &config.seed_projects[0].policy;
    config.defaults.policies = Map::with_capacity(1);
    config
        .defaults
        .policies
        .insert(
            POLICY,
            ConfiguredPolicy {
                policy: seed.policy.clone(),
                permissions: Box::new([]),
                templates: seed.templates.clone(),
            },
        )
        .expect("one configured policy");
    let mut core = Core::new(config, &limits).expect("project fixture");
    let _restart = core.restart_begin(false);
    let _seed =
        step(&mut core, &Env { now: Time::ZERO, wall: Wall::EPOCH, limits }, Event::People(people::Event::Restored));
    (core, limits)
}
#[test]
fn the_last_u32_project_number_fits_and_the_next_make_changes_nothing() {
    let (mut core, limits) = project_core();
    core.counters.deployment.projects = u64::from(u32::MAX) - 1;
    let made = core.make_project(&limits, MAKER, "Last".into(), POLICY).expect("last project number");
    assert_eq!(made.project, u32::MAX);
    let header = core.counters.deployment();
    assert_eq!(
        core.make_project(&limits, MAKER, "Beyond".into(), POLICY).err(),
        Some(people::Refusal::Limit { which: people::LimitKind::Projects })
    );
    assert_eq!(core.counters.deployment(), header);
    assert_eq!(core.projects.as_slice(), [1, u32::MAX]);
    assert_eq!(core.names.get(&u32::MAX).expect("made project's name").as_ref(), "Last");
}
