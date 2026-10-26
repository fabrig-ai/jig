//! Project asks reach the core with the correct scope (people.md, 5.1–5.2).
use super::*;
const PARTY: u64 = 1;
const SIGN_IN: u64 = 10;
const PROJECT: u32 = 1;
const KEY: u8 = 1;

#[test]
fn project_asks_route_with_their_own_scopes_role() {
    for (ask, project, role) in [
        (Ask::MakeProject { name: "Tally".into(), policy: 1 }, DEPLOYMENT, Some(Role::Owner)),
        (Ask::Rename { project: PROJECT, name: "Tally".into() }, PROJECT, Some(Role::Maintainer)),
    ] {
        let mut test = Test::new(LIMITS);
        test.signin(PARTY, SIGN_IN, identity(0, PARTY));
        for (project, role) in [(DEPLOYMENT, Role::Owner), (PROJECT, Role::Maintainer)] {
            test.send(Event::Roles { project, holdings: Box::new([Holding { person: PARTY, role }]) });
        }
        let rows = test.request(SIGN_IN, KEY, ask.clone());
        let routed = rows.iter().find_map(|row| {
            if let Request::Route { person, project, role, ask, .. } = row {
                Some((*person, *project, *role, ask.as_ref()))
            } else {
                None
            }
        });
        assert_eq!(routed, Some((PARTY, project, role, &ask)));
    }
}
#[test]
fn project_asks_without_a_role_are_left_for_the_core() {
    for ask in
        [Ask::MakeProject { name: "Tally".into(), policy: 1 }, Ask::Rename { project: PROJECT, name: "12".into() }]
    {
        let mut test = Test::new(LIMITS);
        test.signin(PARTY, SIGN_IN, identity(0, PARTY));
        assert!(
            test.request(SIGN_IN, KEY, ask)
                .iter()
                .any(|row| { if let Request::Route { role, .. } = row { role.is_none() } else { false } })
        );
    }
}

#[test]
fn settings_admission_is_the_deployment_owners_and_replays_by_digest() {
    let mut test = Test::new(LIMITS);
    test.signin(PARTY, SIGN_IN, identity(0, PARTY));
    let ask = Ask::ChangeSettings { against: 1, digest: [17; 32] };
    for held in [None, Some(Role::Member)] {
        if let Some(role) = held {
            test.send(Event::Roles { project: DEPLOYMENT, holdings: Box::new([Holding { person: PARTY, role }]) });
        }
        let rows = test.request(SIGN_IN, 2 + u8::from(held.is_some()), ask.clone());
        assert_eq!(reply(&rows), Reply::Outcome(Outcome::Refused(Refusal::Role { held })));
        assert_eq!(
            rows.iter().filter_map(|row| if let Request::Route { .. } = row { Some(()) } else { None }).count(),
            0
        );
    }
    test.send(Event::Roles { project: DEPLOYMENT, holdings: Box::new([Holding { person: PARTY, role: Role::Owner }]) });
    let rows = test.request(SIGN_IN, KEY, ask.clone());
    assert_eq!(
        rows.iter().find_map(|row| {
            if let Request::Route { person, project, role, .. } = row { Some((*person, *project, *role)) } else { None }
        }),
        Some((PARTY, DEPLOYMENT, Some(Role::Owner)))
    );
    let request = route(&rows);
    let outcome = Outcome::SettingsChanged { revision: 2 };
    assert_eq!(
        reply(&test.send(Event::Decided { request, outcome: outcome.clone() })),
        Reply::Outcome(outcome.clone())
    );
    assert_eq!(reply(&test.request(SIGN_IN, KEY, ask)), Reply::Outcome(outcome));
    assert_eq!(
        reply(&test.request(SIGN_IN, KEY, Ask::ChangeSettings { against: 1, digest: [18; 32] })),
        Reply::Refused(Refusal::KeyConflict)
    );
    assert_eq!(
        reply(&test.request(SIGN_IN, KEY, Ask::MakeProject { name: "Other".into(), policy: 1 })),
        Reply::Refused(Refusal::KeyConflict)
    );
}

#[test]
fn a_member_cannot_move_running_work_and_the_key_keeps_the_refusal() {
    let mut test = Test::new(LIMITS);
    test.signin(PARTY, SIGN_IN, identity(0, PARTY));
    let ask = Ask::MoveToNewest { revision: 7 };
    for held in [None, Some(Role::Member)] {
        if let Some(role) = held {
            test.send(Event::Roles { project: DEPLOYMENT, holdings: Box::new([Holding { person: PARTY, role }]) });
        }
        let key = 2 + u8::from(held.is_some());
        let rows = test.request(SIGN_IN, key, ask.clone());
        assert_eq!(reply(&rows), Reply::Outcome(Outcome::Refused(Refusal::Role { held })));
        assert_eq!(
            rows.iter().filter_map(|row| if let Request::Route { .. } = row { Some(()) } else { None }).count(),
            0
        );
    }
    test.send(Event::Roles { project: DEPLOYMENT, holdings: Box::new([Holding { person: PARTY, role: Role::Owner }]) });
    assert_eq!(
        reply(&test.request(SIGN_IN, 3, ask)),
        Reply::Outcome(Outcome::Refused(Refusal::Role { held: Some(Role::Member) }))
    );
}
#[test]
fn an_owners_move_to_newest_is_routed_and_its_answer_replayed() {
    let mut test = Test::new(LIMITS);
    test.signin(PARTY, SIGN_IN, identity(0, PARTY));
    test.send(Event::Roles { project: DEPLOYMENT, holdings: Box::new([Holding { person: PARTY, role: Role::Owner }]) });
    let ask = Ask::MoveToNewest { revision: 7 };
    let rows = test.request(SIGN_IN, KEY, ask.clone());
    assert!(rows.iter().any(|row| if let Request::Route { person, project, role, ask: routed, models, .. } = row {
        (*person, *project, *role) == (PARTY, DEPLOYMENT, Some(Role::Owner))
            && routed.as_ref() == &ask
            && models.is_empty()
    } else {
        false
    }));
    let outcome = Outcome::MovedToNewest { newest: 9, moved: 3, stayed: 1 };
    assert_eq!(
        reply(&test.send(Event::Decided { request: route(&rows), outcome: outcome.clone() })),
        Reply::Outcome(outcome.clone())
    );
    let replay = test.request(SIGN_IN, KEY, ask);
    assert_eq!(reply(&replay), Reply::Outcome(outcome));
    assert_eq!(
        replay.iter().filter_map(|row| if let Request::Route { .. } = row { Some(()) } else { None }).count(),
        0
    );
    assert_eq!(
        reply(&test.request(SIGN_IN, KEY, Ask::MoveToNewest { revision: 8 })),
        Reply::Refused(Refusal::KeyConflict)
    );
}
