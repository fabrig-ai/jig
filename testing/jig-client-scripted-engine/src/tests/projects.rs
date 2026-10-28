use super::*;

fn owned() -> Script {
    let mut script = script();
    let owner = script.truth.frames.get_mut(&1).unwrap();
    owner.deployment_role = Some(HeldRole::Owner);
    owner.projects.get_mut(0).unwrap().role = HeldRole::Owner;
    script
}
fn make(name: &[u8], policy: u32) -> Ask {
    Ask::MakeProject { name: text(name), policy }
}
fn refused_as(engine: &mut Engine, party: u64, ask: &Ask, refusal: Refusal) {
    let before = engine.truth().clone();
    let (answer, changes) = engine.decide(party, ask, Wall::EPOCH).unwrap();
    assert_eq!(answer, Answer::Refused { refusal, at: None });
    assert!(changes.is_empty());
    assert_eq!(engine.truth(), &before);
}

#[test]
fn project_names_check_each_rule_before_changing_truth() {
    let mut engine = Engine::new(owned(), 1).unwrap();
    for (name, rule) in [
        (String::new(), NameRule::Empty),
        (format!("{}é", "a".repeat(63)), NameRule::Long),
        ("Ta\u{7}lly".to_owned(), NameRule::Control),
        (" Tally".to_owned(), NameRule::Space),
        ("Tally\u{a0}".to_owned(), NameRule::Space),
        ("123".to_owned(), NameRule::Digits),
        (" \u{7}".to_owned(), NameRule::Control),
    ] {
        for ask in [make(name.as_bytes(), 1), Ask::Rename { project: 1, name: text(name.as_bytes()) }] {
            refused_as(&mut engine, 1, &ask, Refusal::Malformed { at: 0, why: Malformed::Name(rule) });
        }
    }
}

#[test]
fn project_names_at_the_byte_bound_and_with_internal_spaces_are_accepted() {
    let mut engine = Engine::new(owned(), 1).unwrap();
    for name in [format!("{}é", "a".repeat(62)), "Ta lly".to_owned(), "١٢".to_owned()] {
        let (answer, _) = engine.decide(1, &make(name.as_bytes(), 1), Wall::EPOCH).unwrap();
        assert!(matches!(answer, Answer::Done(Outcome::Owner(OwnerOutcome::ProjectMade { .. }))));
    }
}

#[test]
fn only_the_scoped_owner_can_make_or_rename() {
    let mut engine = Engine::new(owned(), 1).unwrap();
    for ask in [make(b"Tally", 1), Ask::Rename { project: 1, name: text(b"Tally") }] {
        refused_as(&mut engine, 2, &ask, Refusal::Role { held: Some(HeldRole::Policy { role: 1 }) });
    }
}
fn configured_policy() -> Engine {
    let mut script = owned();
    script.truth.policies = Some(vec![2]);
    Engine::new(script, 1).unwrap()
}
#[test]
fn a_make_with_an_unconfigured_policy_is_refused() {
    refused_as(&mut configured_policy(), 1, &make(b"Tally", 1), Refusal::Unknown);
}
#[test]
fn a_make_accepts_the_scripts_configured_policy() {
    let mut engine = configured_policy();
    let (answer, _) = engine.decide(1, &make(b"Tally", 2), Wall::EPOCH).unwrap();
    assert_eq!(answer, Answer::Done(Outcome::Owner(OwnerOutcome::ProjectMade { project: 2 })));
}

fn with_hidden_project() -> Engine {
    let mut script = owned();
    let hidden = script.truth.frames.get_mut(&2).unwrap().projects.get_mut(0).unwrap();
    hidden.project = 100;
    hidden.name = DocumentText::complete(text(b"Hidden"));
    Engine::new(script, 1).unwrap()
}
#[test]
fn project_names_collide_with_hidden_projects() {
    let mut engine = with_hidden_project();
    refused_as(&mut engine, 1, &make(b"hidden", 1), Refusal::NameTaken);
    refused_as(&mut engine, 1, &Ask::Rename { project: 1, name: text(b"HIDDEN") }, Refusal::NameTaken);
}
#[test]
fn a_rename_excludes_its_own_project_from_collisions() {
    let mut engine = Engine::new(owned(), 1).unwrap();
    let (answer, _) = engine.decide(1, &Ask::Rename { project: 1, name: text(b"PROJECT") }, Wall::EPOCH).unwrap();
    assert_eq!(answer, Answer::Done(Outcome::Owner(OwnerOutcome::Renamed { project: 1 })));
}
#[test]
fn a_make_is_numbered_past_every_project_held_by_the_script() {
    let mut engine = with_hidden_project();
    let (answer, _) = engine.decide(1, &make(b"Tally", 1), Wall::EPOCH).unwrap();
    assert_eq!(answer, Answer::Done(Outcome::Owner(OwnerOutcome::ProjectMade { project: 101 })));
}

#[test]
fn making_a_project_without_room_is_refused_with_the_deployments_limit() {
    let mut script = owned();
    script.bounds.rows = 1;
    // The standard creating offer reserves more than one row; this fixture needs no offer.
    for frame in script.truth.frames.values_mut() {
        frame.projects.get_mut(0).unwrap().creating = None;
    }
    let mut engine = Engine::new(script, 1).unwrap();
    refused_as(
        &mut engine,
        1,
        &make(b"Tally", 1),
        Refusal::Limit { which: Limit::Admission { which: AdmissionLimit::Projects, bound: None } },
    );
}
