use jig_domain_people::{Act, ActGrant, ActKind, Again, InitialOwner, LimitKind, Outcome, Refusal, Reply};
use jig_people_world::{Settings, World, identity};
use skein_lib::{Duration, Wall};

const CONNECTOR: u16 = 1;
const OTHER: u16 = 2;
const PROJECT: u32 = 1;
const TASK: u64 = 10;
const KEY: [u8; 16] = [7; 16];
const DECIDED: Outcome = Outcome::Compound { project: PROJECT, operations: 1, first_message: 1 };
fn owners() -> Box<[InitialOwner]> {
    Box::new([InitialOwner { project: PROJECT, identity: identity(1).key }])
}
fn grants() -> Box<[ActGrant]> {
    [CONNECTOR, OTHER]
        .map(|connector| ActGrant {
            connector,
            may: Box::new([
                ActKind::Close,
                ActKind::Adopt,
                ActKind::MakeProject,
                ActKind::AddKey,
                ActKind::RemoveAccount,
            ]),
        })
        .into()
}
fn close() -> Act {
    Act::Close { project: PROJECT, task: TASK, grace: None, reason: b"finished".as_slice().into() }
}
fn world() -> World {
    World::configured(Settings::calm(950), owners(), grants())
}
fn answer(world: &World, call: u64, outcome: Outcome) {
    assert_eq!(world.reply(call), Some(Reply::Outcome(outcome)));
}

#[test]
fn a_granted_act_with_an_owner_is_routed_and_answered_once_durable() {
    let mut world = world();
    let call = world.act(CONNECTOR, KEY, close());
    assert_eq!(world.acts.len(), 1);
    world.decide_acts(&DECIDED);
    assert_eq!(world.reply(call), None);
    world.commit_all();
    answer(&world, call, DECIDED);
    world.assert_settled();
}
#[test]
fn an_act_not_granted_is_not_offered_and_unsaved() {
    let mut world = World::configured(Settings::calm(951), owners(), Box::new([]));
    let refused = world.act(CONNECTOR, KEY, close());
    answer(&world, refused, Outcome::Refused(Refusal::NotOffered));
    world.reconfigure(owners(), grants());
    let accepted = world.act(CONNECTOR, KEY, close());
    world.decide_acts(&DECIDED);
    world.commit_all();
    answer(&world, accepted, DECIDED);
    assert_eq!(world.stats().routes, 1);
}
#[test]
fn any_act_with_no_owner_lacks_the_role_and_is_unsaved() {
    let mut world = World::configured(Settings::calm(952), Box::new([]), grants());
    for act in [
        close(),
        Act::AddKey { provider: 1, join: None, digest: [7; 32] },
        Act::RemoveAccount { account: 1 },
        Act::MakeProject { maker: 1, name: "Tally".into(), policy: 1 },
        Act::Adopt {
            project: PROJECT,
            adoption: jig_domain_people::Adoption {
                resource: jig_domain_people::ResourceName {
                    connector: CONNECTOR,
                    path: Box::new([Box::from(&b"own"[..])]),
                },
                role: jig_domain_people::ResourceRole::Owned,
                options: Box::new([]),
            },
        },
    ] {
        let refused = world.act(CONNECTOR, KEY, act);
        answer(&world, refused, Outcome::Refused(Refusal::Role { held: None }));
    }
    world.reconfigure(owners(), grants());
    let accepted = world.act(CONNECTOR, KEY, close());
    world.decide_acts(&DECIDED);
    world.commit_all();
    answer(&world, accepted, DECIDED);
    assert_eq!(world.stats().routes, 1);
}
#[test]
fn a_saved_act_answers_again_whatever_the_grants_since() {
    let mut world = world();
    world.act(CONNECTOR, KEY, close());
    world.decide_acts(&DECIDED);
    world.commit_all();
    world.reconfigure(Box::new([]), Box::new([]));
    let replay = world.act(CONNECTOR, KEY, close());
    answer(&world, replay, DECIDED);
    assert!(world.acts.is_empty());
    assert_eq!(world.stats().routes, 1);
}
#[test]
fn acts_keys_are_the_connectors_own() {
    let mut world = world();
    world.roles(PROJECT, Box::new([]));
    world.signin(100, 1);
    world.commit_all();
    world.act(CONNECTOR, KEY, close());
    world.act(OTHER, KEY, close());
    world.request(100, KEY, jig_domain_people::Ask::MarkSeen { seen: jig_domain_people::Seen::Results { through: 0 } });
    assert_eq!(world.stats().routes, 3);
    assert_eq!(world.acts.len(), 2);
    world.decide_acts(&DECIDED);
    world.commit_all();
    world.assert_settled();
}
#[test]
fn a_pending_act_joins_and_each_send_is_answered_once() {
    let mut world = world();
    let calls = [world.act(CONNECTOR, KEY, close()), world.act(CONNECTOR, KEY, close())];
    assert_eq!(world.acts.len(), 1);
    world.decide_acts(&DECIDED);
    world.commit_all();
    for call in calls {
        answer(&world, call, DECIDED);
    }
    world.assert_settled();
}
#[test]
fn another_act_under_the_key_is_a_conflict() {
    let mut world = world();
    world.act(CONNECTOR, KEY, close());
    let changed = Act::Close { project: PROJECT, task: TASK + 1, grace: None, reason: Box::new([]) };
    let pending = world.act(CONNECTOR, KEY, changed.clone());
    answer(&world, pending, Outcome::Refused(Refusal::KeyConflict));
    world.decide_acts(&DECIDED);
    world.commit_all();
    let saved = world.act(CONNECTOR, KEY, changed);
    answer(&world, saved, Outcome::Refused(Refusal::KeyConflict));
}
#[test]
fn an_adoption_of_another_connectors_resource_is_not_offered() {
    let mut world = world();
    let act = Act::Adopt {
        project: PROJECT,
        adoption: jig_domain_people::Adoption {
            resource: jig_domain_people::ResourceName { connector: OTHER, path: Box::new([Box::from(&b"own"[..])]) },
            role: jig_domain_people::ResourceRole::Owned,
            options: Box::new([]),
        },
    };
    let call = world.act(CONNECTOR, KEY, act.clone());
    world.commit_all();
    answer(&world, call, Outcome::Refused(Refusal::NotOffered));
    world.reconfigure(Box::new([]), Box::new([]));
    let replay = world.act(CONNECTOR, KEY, act);
    answer(&world, replay, Outcome::Refused(Refusal::NotOffered));
    assert_eq!(world.stats().routes, 0);
}
#[test]
fn busy_acts_and_retryable_decisions_are_not_saved() {
    let mut settings = Settings::calm(953);
    settings.limits.pending = 1;
    settings.limits.waiters = 1;
    let mut world = World::configured(settings, owners(), grants());
    world.act(CONNECTOR, KEY, close());
    for (connector, key) in [(CONNECTOR, KEY), (OTHER, KEY)] {
        let call = world.act(connector, key, close());
        answer(&world, call, Outcome::Again(Again::Busy));
    }
    assert_eq!(world.capacity_forgotten(), 2, "both busy entrances report pressure");
    world.decide_acts(&Outcome::Again(Again::NotReady));
    world.commit_all();
    let call = world.act(CONNECTOR, KEY, close());
    world.decide_acts(&DECIDED);
    world.commit_all();
    answer(&world, call, DECIDED);
    assert_eq!(world.stats().routes, 2);
}
const ANSWER_PLACES: u32 = 1; // one deployment act reserves the whole tiny table
const PARTY_SIGN_IN: u64 = 100;
const REQUESTER: u64 = 1;
const PARTY_KEY: [u8; 16] = [8; 16];
const ANSWER_RETENTION: Duration = Duration::from_secs(120); // the people world's calm profile
const ACT_WALL: Wall = Wall::from_nanos(1_000_000_000);
fn reserved_answer_world(seed: u64) -> World {
    let mut settings = Settings::calm(seed);
    settings.limits.requests = ANSWER_PLACES;
    settings.limits.request_retention = ANSWER_RETENTION;
    let mut world = World::configured(settings, owners(), grants());
    world.roles(PROJECT, Box::new([]));
    world.signin(PARTY_SIGN_IN, REQUESTER);
    world.commit_all();
    world.set_wall(ACT_WALL);
    world.act(CONNECTOR, KEY, close());
    world
}
#[test]
fn acts_share_the_answer_room_and_retention() {
    let mut world = reserved_answer_world(954);
    let blocked = world.request(
        PARTY_SIGN_IN,
        KEY,
        jig_domain_people::Ask::Cancel { project: PROJECT, task: TASK, grace: None, reason: Box::new([]) },
    );
    assert_eq!(world.reply(blocked), Some(Reply::Again(Again::Full { held: ANSWER_PLACES, room_at: ACT_WALL })));
    world.decide_acts(&DECIDED);
    world.commit_all();
    let busy = world.act(OTHER, KEY, close());
    answer(&world, busy, Outcome::Again(Again::Busy));
    world.advance(ANSWER_RETENTION);
    let call = world.act(OTHER, KEY, close());
    world.decide_acts(&DECIDED);
    world.commit_all();
    answer(&world, call, DECIDED);
    assert_eq!(world.stats().routes, 2);
}
#[test]
fn out_of_bounds_acts_are_unsaved_with_the_partys_refusal() {
    let mut world = world();
    for (act, why) in [
        (Act::Close { project: PROJECT, task: 0, grace: None, reason: Box::new([]) }, Refusal::Unknown),
        (
            Act::Close {
                project: PROJECT,
                task: TASK,
                grace: None,
                reason: vec![
                    0;
                    usize::try_from(jig_people_world::LIMITS.words + 1)
                        .expect("bounded deployment act fixture")
                ]
                .into(),
            },
            Refusal::configured(
                LimitKind::Words,
                u64::from(jig_people_world::LIMITS.words),
                jig_domain_people::LimitKey::PeopleWords,
                jig_domain_people::LimitUnit::Bytes,
            ),
        ),
    ] {
        let call = world.act(CONNECTOR, KEY, act);
        answer(&world, call, Outcome::Refused(why));
    }
    let accepted = world.act(
        CONNECTOR,
        KEY,
        Act::Close {
            project: PROJECT,
            task: TASK,
            grace: Some(Duration::from_secs(9999)),
            reason: vec![0; usize::try_from(jig_people_world::LIMITS.words).expect("bounded deployment act fixture")]
                .into(),
        },
    );
    world.decide_acts(&DECIDED);
    world.commit_all();
    answer(&world, accepted, DECIDED);
}

#[test]
fn adoption_bounds_refuse_before_retaining_a_key() {
    use jig_domain_people::{Adoption, ResourceName, ResourceRole};
    let mut world = world();
    let path_bytes = usize::try_from(jig_people_world::LIMITS.amendment_bytes).expect("bounded deployment act fixture")
        - size_of::<Box<[u8]>>();
    let adoption = |bytes| Act::Adopt {
        project: PROJECT,
        adoption: Adoption {
            resource: ResourceName { connector: CONNECTOR, path: Box::new([vec![0; bytes].into_boxed_slice()]) },
            role: ResourceRole::Owned,
            options: Box::new([]),
        },
    };
    let call = world.act(CONNECTOR, KEY, adoption(path_bytes + 1));
    answer(
        &world,
        call,
        Outcome::Refused(Refusal::configured(
            LimitKind::Amendment,
            u64::from(jig_people_world::LIMITS.amendment_bytes),
            jig_domain_people::LimitKey::PeopleAmendmentBytes,
            jig_domain_people::LimitUnit::Bytes,
        )),
    );
    let call = world.act(CONNECTOR, KEY, adoption(path_bytes));
    world.decide_acts(&Outcome::Adopted { project: PROJECT });
    world.commit_all();
    answer(&world, call, Outcome::Adopted { project: PROJECT });
}

#[test]
fn full_party_answers_name_the_oldest_deployment_answer_expiry() {
    let mut world = reserved_answer_world(955);
    world.decide_acts(&DECIDED);
    world.commit_all();
    let blocked = world.request(
        PARTY_SIGN_IN,
        PARTY_KEY,
        jig_domain_people::Ask::Cancel { project: PROJECT, task: TASK, grace: None, reason: Box::new([]) },
    );
    assert_eq!(
        world.reply(blocked),
        Some(Reply::Again(Again::Full { held: ANSWER_PLACES, room_at: Wall::from_nanos(121_000_000_000) }))
    );
}
