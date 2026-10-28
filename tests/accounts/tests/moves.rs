//! Moves run entirely at the child's boundary (accounts.md, section 6).
use jig_accounts_world::{LIMITS, World, account, provider, providers, source};
use jig_domain_accounts::{
    Answer, Bound, Change, End, Event, Failure, Form, FromComponent, Grant, Held, Holder, Key, KeyFailure, Kind,
    Refusal, Request, Scheme, Signing, State, Stored, ToComponent, Turned, Use,
};
use skein_lib::{Duration, Time, Token};
use std::collections::BTreeMap;

const ALICE: Holder = Holder::Party(9);
const BOB: Holder = Holder::Party(4);
const CARRIED: u64 = 1 << 63;
fn fresh(rows: &[Stored]) -> World {
    World::with(providers(vec![provider(1, true, true), provider(2, false, true)]), rows)
}
fn started(rows: &[Stored]) -> World {
    let mut world = fresh(rows);
    world.step(Event::Want);
    world
}
fn rows(key: bool) -> Vec<Stored> {
    let mut row = account(5, 1, ALICE, b"alice");
    if let Stored::Account { display, .. } = &mut row {
        *display = Box::from(b"Al".as_slice());
    }
    let mut rows = vec![row, source(5, 6, 0, Kind::SignIn)];
    if key {
        rows.push(source(5, 7, 1, Kind::Key));
    }
    rows
}
fn change(request: u64, holder: Holder, change: Change) -> Event {
    Event::Change { request: Token::new(request), holder, change }
}
fn answer(request: u64, answer: Answer) -> Request {
    Request::Answer { request: Token::new(request), answer }
}
fn refused(request: u64, refusal: Refusal) -> Vec<Request> {
    vec![answer(request, Answer::Refused { refusal })]
}
fn save(record: Stored) -> Request {
    Request::Save { record }
}
fn moving(account: u32, number: u32, place: u32) -> Stored {
    Stored::Source { account, number, place, kind: Kind::SignIn, name: Box::new([]), held: Some(Held::Moving) }
}
fn want(source: u32) -> Request {
    Request::Component(ToComponent::Want { account: source })
}
fn grant(source: u32, generation: u64) -> Event {
    Event::Component(FromComponent::Granted {
        account: source,
        generation,
        valid: Duration::from_secs(100),
        form: Form::Credential(Scheme::Bearer),
    })
}
fn moving_fact(world: &World, source: u32) {
    let fact = world.domain.source(source).expect("the moving source stays");
    assert_eq!((fact.state, fact.usable, fact.generation), (State::Moving, false, None));
    assert!(!world.domain.usable(fact.account));
    assert_eq!(world.domain.source_grant(source, world.now), None);
}
fn start_out(world: &mut World) -> u32 {
    let number = world.domain.next_number();
    assert_eq!(
        world.step(change(1, ALICE, Change::MoveOut { account: 5 })),
        vec![
            save(Stored::Next { number: number + 1 }),
            save(moving(5, 6, 0)),
            Request::Component(ToComponent::Release { account: 6 }),
            Request::Signing(Signing::Withdraw { source: 6, display: Box::from(b"Al".as_slice()) }),
        ]
    );
    moving_fact(world, 6);
    number
}
fn withdrawn(world: &mut World, number: u32) {
    assert_eq!(
        world.step(Event::Withdrawn { source: 6, readable: true }),
        vec![answer(1, Answer::MovingOut { account: 5, number, source: 6 })]
    );
}
fn comes_back(source: u32) -> Vec<Request> {
    vec![Request::Signing(Signing::Return { source }), Request::Signing(Signing::Refresh { source }), want(source)]
}
fn start_in(world: &mut World, holder: Holder) -> u32 {
    let source = world.domain.next_number();
    assert_eq!(
        world.step(change(2, holder, Change::MoveIn { carried: CARRIED })),
        vec![save(Stored::Next { number: source + 1 }), Request::Signing(Signing::Open { source, carried: CARRIED })]
    );
    source
}
fn opened(source: u32, provider: u32, subject: &[u8]) -> Event {
    Event::Opened { source, provider, subject: subject.into(), display: Box::from(b"Moved".as_slice()) }
}
fn open(world: &mut World, source: u32, subject: &[u8]) {
    assert_eq!(
        world.step(opened(source, 1, subject)),
        vec![Request::Signing(Signing::Adopt { source, carried: CARRIED })]
    );
}
fn commit(store: &mut BTreeMap<Key, Stored>, requests: &[Request]) {
    for request in requests {
        match request {
            Request::Save { record } => {
                store.insert(record.key(), record.clone());
            }
            Request::Erase { key } => {
                store.remove(key);
            }
            Request::Programs { .. }
            | Request::CheckProgram { .. }
            | Request::SignOutProgram { .. }
            | Request::Component(_)
            | Request::Signing(_)
            | Request::Answer { .. }
            | Request::Granted { .. }
            | Request::Closed => {}
        }
    }
}
fn restart(store: &BTreeMap<Key, Stored>, source: u32) -> World {
    let mut again = fresh(&store.values().cloned().collect::<Vec<_>>());
    assert_eq!(
        again.step(Event::Want),
        vec![
            Request::Signing(Signing::Restore { source, provider: 1, kind: Kind::SignIn }),
            Request::Signing(Signing::Restored),
            Request::Signing(Signing::Refresh { source }),
            want(source),
        ]
    );
    moving_fact(&again, source);
    again
}

#[test]
fn a_move_out_withdraws_the_source_and_answers_with_the_moves_number() {
    let mut world = started(&rows(false));
    world.step(grant(6, 1));
    assert!(world.domain.usable(5));
    let number = start_out(&mut world);
    assert_ne!(number, 6);
    assert_eq!(world.domain.next_deadline(), None, "the client's wait starts at its answer");
    for event in [
        grant(6, 2),
        Event::Component(FromComponent::Expiring { account: 6, generation: 1 }),
        Event::Component(FromComponent::Failed { account: 6, failure: Failure::SignInAgain }),
        Event::Unauthorized { account: 6, generation: 1 },
        Event::Limited { account: 6, reset: None },
        Event::Refused { account: 6, code: Box::new([]), reason: Box::new([]) },
        Event::RunFailed { account: 6, turned: Turned::Refused },
        Event::RunFailed { account: 6, turned: Turned::Limited { reset: None } },
    ] {
        assert!(world.step(event).is_empty(), "the withdrawn generation says nothing");
        moving_fact(&world, 6);
    }
    world.now = Time::from_nanos(5_000_000_000);
    withdrawn(&mut world, number);
    assert_eq!(world.domain.next_deadline(), Some(world.now.saturating_add(LIMITS.move_deadline)));
    assert!(world.step(Event::Withdrawn { source: 6, readable: true }).is_empty());
    assert!(world.facts().iter().any(|fact| fact.source == 6 && fact.state == State::Moving && !fact.usable));
}

#[test]
fn a_written_move_erases_the_source_and_its_account() {
    let mut world = started(&rows(false));
    let number = start_out(&mut world);
    assert_eq!(
        world.step(change(3, ALICE, Change::Written { number })),
        refused(3, Refusal::Cancelled),
        "still withdrawing"
    );
    withdrawn(&mut world, number);
    assert_eq!(world.step(change(3, BOB, Change::Written { number })), refused(3, Refusal::Cancelled));
    assert_eq!(
        world.step(change(3, ALICE, Change::Written { number })),
        vec![
            Request::Erase { key: Key::Source { account: 5, number: 6 } },
            Request::Erase { key: Key::Account { number: 5 } },
            Request::Signing(Signing::Forget { source: 6 }),
            answer(3, Answer::Done)
        ]
    );
    assert!(world.domain.account(5).is_none());
    assert!(world.domain.source(6).is_none());
    assert_eq!(world.domain.next_deadline(), None);
    let next = start_in(&mut world, ALICE);
    assert!(next > number, "the account, source and move numbers are never reused");
}

#[test]
fn a_written_move_keeps_an_account_with_another_source() {
    let mut world = started(&rows(true));
    let number = start_out(&mut world);
    withdrawn(&mut world, number);
    assert_eq!(
        world.step(change(3, ALICE, Change::Written { number })),
        vec![
            Request::Erase { key: Key::Source { account: 5, number: 6 } },
            save(source(5, 7, 0, Kind::Key)),
            Request::Signing(Signing::Forget { source: 6 }),
            answer(3, Answer::Done),
        ]
    );
    assert_eq!(world.domain.account(5).expect("the key keeps the account").sources().get(&0), Some(&7));
    assert!(world.domain.source(6).is_none());
}

#[test]
fn a_move_with_no_written_by_its_deadline_comes_back_refreshed_once() {
    let mut world = started(&rows(false));
    let number = start_out(&mut world);
    withdrawn(&mut world, number);
    assert!(world.fire_at(59).is_empty());
    assert!(!world.domain.is_due(world.now));
    assert_eq!(world.fire_at(60), comes_back(6));
    moving_fact(&world, 6);
    assert_eq!(world.domain.next_deadline(), None);
    assert!(world.fire_at(61).is_empty());
    assert_eq!(
        world.step(grant(6, 2)),
        vec![
            Request::Granted {
                grant: Grant {
                    form: Form::Credential(Scheme::Bearer),
                    account: 6,
                    generation: 2,
                    valid: Duration::from_secs(100)
                }
            },
            save(source(5, 6, 0, Kind::SignIn)),
        ]
    );
    assert!(world.domain.usable(5));
    assert_eq!(world.domain.source(6).expect("returned").state, State::Granted);
    assert_eq!(world.domain.grant(5, world.now).expect("lent").generation, 2);
}

#[test]
fn a_late_written_is_refused_cancelled() {
    let mut world = started(&rows(false));
    let number = start_out(&mut world);
    withdrawn(&mut world, number);
    world.fire_at(60);
    assert_eq!(world.step(change(3, ALICE, Change::Written { number })), refused(3, Refusal::Cancelled));
    assert_eq!(world.step(change(4, ALICE, Change::Written { number: 99 })), refused(4, Refusal::Cancelled));
    moving_fact(&world, 6);
}

#[test]
fn a_restart_while_moving_refreshes_before_it_wants() {
    for cancelled in [false, true] {
        let initial = rows(false);
        let mut store = initial.iter().map(|row| (row.key(), row.clone())).collect::<BTreeMap<_, _>>();
        let mut world = started(&initial);
        let number = world.domain.next_number();
        commit(&mut store, &world.step(change(1, ALICE, Change::MoveOut { account: 5 })));
        withdrawn(&mut world, number);
        if cancelled {
            commit(&mut store, &world.fire_at(60));
        }
        let mut again = restart(&store, 6);
        assert_eq!(again.step(change(3, ALICE, Change::Written { number })), refused(3, Refusal::Cancelled));
    }
}

#[test]
fn an_unreadable_record_is_refused_not_signed_in_and_comes_back() {
    let mut world = started(&rows(false));
    start_out(&mut world);
    let mut expected = refused(1, Refusal::NotSignedIn);
    expected.extend(comes_back(6));
    assert_eq!(world.step(Event::Withdrawn { source: 6, readable: false }), expected);
    moving_fact(&world, 6);
    assert_eq!(world.domain.next_deadline(), None);
    assert!(world.step(Event::Withdrawn { source: 6, readable: false }).is_empty());
}

#[test]
fn moving_out_needs_a_signed_in_source_with_a_record() {
    for kind in [Kind::Key, Kind::None] {
        let mut world = started(&[account(5, 1, ALICE, b""), source(5, 6, 0, kind)]);
        let next = world.domain.next_number();
        assert_eq!(world.step(change(1, ALICE, Change::MoveOut { account: 5 })), refused(1, Refusal::NotSignedIn));
        assert_eq!(world.domain.next_number(), next);
    }
    for failure in [
        Failure::SignInAgain,
        Failure::Unloaded { at: jig_domain_accounts::Place::File, why: jig_domain_accounts::Unread::Unreadable },
    ] {
        let mut world = started(&rows(false));
        world.step(Event::Component(FromComponent::Failed { account: 6, failure }));
        assert_eq!(world.step(change(1, ALICE, Change::MoveOut { account: 5 })), refused(1, Refusal::NotSignedIn));
    }
}

#[test]
fn while_moving_sign_out_removal_renewal_and_another_move_are_refused() {
    // Two slots let a sign-in or arrival reach its subject while a move occupies one.
    let mut world = World::with_limits(
        jig_domain_accounts::Limits { sign_ins: 2, ..LIMITS },
        providers(vec![provider(1, true, true)]),
        &rows(true),
    );
    world.step(Event::Want);
    let number = start_out(&mut world);
    for answered in [false, true] {
        if answered {
            withdrawn(&mut world, number);
        }
        for event in [
            Event::SignOut { request: Token::new(3), holder: ALICE, source: 6 },
            change(3, ALICE, Change::Remove { account: 5 }),
            change(3, ALICE, Change::MoveOut { account: 5 }),
        ] {
            assert_eq!(world.step(event), refused(3, Refusal::Moving));
        }
        assert!(
            world
                .step(change(3, ALICE, Change::MoveSource { account: 5, source: 6, place: 0 }))
                .contains(&answer(3, Answer::Done))
        );
        assert!(
            world
                .step(change(3, ALICE, Change::MoveAccount { account: 5, place: 0 }))
                .contains(&answer(3, Answer::Done))
        );
        assert_eq!(
            world.step(change(3, ALICE, Change::SetUses { account: 5, uses: Some(Box::new([Use::Party(9)])) })).last(),
            Some(&answer(3, Answer::Done))
        );
        let incoming = world.domain.next_number();
        assert_eq!(
            world.step(Event::SignIn { request: Token::new(4), holder: ALICE, by: 9, provider: 1 }).last(),
            Some(&Request::Signing(Signing::SignIn { source: incoming, provider: 1 }))
        );
        world.step(Event::Visited { source: incoming, address: Box::new([]) });
        assert_eq!(
            world.step(Event::SignedIn {
                source: incoming,
                subject: Box::from(b"alice".as_slice()),
                display: Box::new([]),
                declined: false
            }),
            vec![Request::Signing(Signing::SignOut { source: incoming })]
        );
        assert_eq!(world.ended().last().expect("refused renewal").end, End::Refused { refusal: Refusal::Moving });
        let incoming = start_in(&mut world, ALICE);
        assert_eq!(
            world.step(opened(incoming, 1, b"alice")),
            [vec![Request::Signing(Signing::Forget { source: incoming })], refused(2, Refusal::Moving)].concat()
        );
    }
    moving_fact(&world, 6);
}

#[test]
fn a_move_in_makes_an_account_refreshed_once_before_it_is_usable() {
    let mut world = started(&[]);
    let source = start_in(&mut world, ALICE);
    assert!(world.domain.source(source).is_none());
    open(&mut world, source, b"alice");
    let made = Stored::Account {
        number: 1,
        provider: 1,
        holder: ALICE,
        place: 0,
        uses: None,
        subject: Box::from(b"alice".as_slice()),
        display: Box::from(b"Moved".as_slice()),
    };
    assert_eq!(
        world.step(Event::Adopted { source, kept: true }),
        vec![
            save(Stored::Next { number: 2 }),
            save(made),
            save(moving(1, source, 0)),
            Request::Signing(Signing::Refresh { source }),
            want(source),
            answer(2, Answer::MovedIn { account: 1, renewed: false }),
        ]
    );
    moving_fact(&world, source);
    assert!(world.step(Event::Adopted { source, kept: true }).is_empty());
    world.step(grant(source, 2));
    assert!(world.domain.usable(1));
    assert_eq!(world.domain.grant(1, world.now).expect("new generation").generation, 2);
}

#[test]
fn a_move_in_renews_the_holders_account_of_its_subject() {
    let mut world = started(&rows(true));
    let source = start_in(&mut world, ALICE);
    open(&mut world, source, b"alice");
    assert_eq!(
        world.step(Event::Adopted { source, kept: true }),
        vec![
            Request::Erase { key: Key::Source { account: 5, number: 6 } },
            Request::Signing(Signing::Replaced { source: 6, by: source }),
            save(moving(5, source, 0)),
            Request::Signing(Signing::Refresh { source }),
            want(source),
            answer(2, Answer::MovedIn { account: 5, renewed: true }),
        ]
    );
    assert_eq!(world.domain.accounts(), 1);
    assert_eq!(world.domain.account(5).expect("renewed").display(), b"Al");
    assert_eq!(world.domain.account(5).expect("renewed").sources().get(&1), Some(&7));
    assert!(world.domain.source(6).is_none());
    moving_fact(&world, source);
}

#[test]
fn a_move_in_is_refused_for_another_holders_subject_held() {
    let mut world = started(&rows(false));
    let source = start_in(&mut world, BOB);
    assert_eq!(
        world.step(opened(source, 1, b"alice")),
        [vec![Request::Signing(Signing::Forget { source })], refused(2, Refusal::Held { provider: 1 })].concat()
    );
    assert_eq!(world.domain.accounts(), 1);
    assert!(world.step(Event::Adopted { source, kept: true }).is_empty());
}

#[test]
fn a_move_in_is_refused_for_the_deployment_where_sign_in_brings_a_plan() {
    for (holder, provider) in [(Holder::Deployment, 1), (ALICE, 2), (ALICE, 99)] {
        let mut world = started(&[]);
        let source = start_in(&mut world, holder);
        assert_eq!(
            world.step(opened(source, provider, b"alice")),
            [vec![Request::Signing(Signing::Forget { source })], refused(2, Refusal::Provider { provider })].concat()
        );
        assert_eq!(world.domain.accounts(), 0);
    }
}

#[test]
fn a_move_in_not_ours_is_refused() {
    let mut world = started(&[]);
    let source = start_in(&mut world, ALICE);
    assert_eq!(world.step(Event::NotOurs { source }), refused(2, Refusal::NotOurs));
    assert_eq!(world.domain.accounts(), 0);
    assert!(world.step(Event::NotOurs { source }).is_empty());
    assert!(world.step(Event::Adopted { source, kept: true }).is_empty());
}

#[test]
fn a_move_in_whose_adoption_failed_is_refused_not_kept() {
    let mut world = started(&[]);
    let source = start_in(&mut world, ALICE);
    open(&mut world, source, b"alice");
    assert_eq!(
        world.step(Event::Adopted { source, kept: false }),
        refused(2, Refusal::NotKept { why: KeyFailure::NotStored })
    );
    assert_eq!(world.domain.accounts(), 0);
}

#[test]
fn a_move_in_counts_against_the_sign_ins_bound() {
    let mut world = started(&rows(false));
    let source = start_in(&mut world, ALICE);
    for event in [
        change(3, ALICE, Change::MoveIn { carried: CARRIED }),
        change(3, ALICE, Change::MoveOut { account: 5 }),
        change(3, ALICE, Change::AddKey { provider: 1, join: None, carried: CARRIED }),
        Event::SignIn { request: Token::new(3), holder: ALICE, by: 9, provider: 1 },
    ] {
        assert_eq!(
            world.step(event),
            refused(
                3,
                Refusal::limited(
                    Bound::SignIns,
                    jig_domain_accounts::LimitKey::SignIns,
                    jig_accounts_world::LIMITS.sign_ins
                )
            )
        );
    }
    world.step(Event::NotOurs { source });
    let number = start_out(&mut world);
    assert_eq!(
        world.step(change(3, ALICE, Change::MoveIn { carried: CARRIED })),
        refused(
            3,
            Refusal::limited(
                Bound::SignIns,
                jig_domain_accounts::LimitKey::SignIns,
                jig_accounts_world::LIMITS.sign_ins
            )
        )
    );
    withdrawn(&mut world, number);
    world.fire_at(60);
    start_in(&mut world, ALICE);
}

#[test]
fn close_refuses_a_withdrawing_move_and_keeps_the_hold() {
    for answered in [false, true] {
        let initial = rows(false);
        let mut store = initial.iter().map(|row| (row.key(), row.clone())).collect::<BTreeMap<_, _>>();
        let mut world = started(&initial);
        let number = world.domain.next_number();
        commit(&mut store, &world.step(change(1, ALICE, Change::MoveOut { account: 5 })));
        if answered {
            withdrawn(&mut world, number);
        }
        let mut expected = if answered { vec![] } else { refused(1, Refusal::Closing) };
        expected
            .extend([Request::Component(ToComponent::Release { account: 6 }), Request::Component(ToComponent::Close)]);
        let closing = world.step(Event::Close);
        assert_eq!(closing, expected);
        commit(&mut store, &closing);
        assert_eq!(store.get(&Key::Source { account: 5, number: 6 }), Some(&moving(5, 6, 0)));
        assert_eq!(world.domain.next_deadline(), None);
        for change_ in
            [Change::MoveOut { account: u32::MAX }, Change::Written { number }, Change::MoveIn { carried: CARRIED }]
        {
            assert_eq!(world.step(change(3, BOB, change_)), refused(3, Refusal::Closing));
        }
        assert!(world.step(Event::Withdrawn { source: 6, readable: true }).is_empty());
        assert_eq!(world.step(Event::Component(FromComponent::Closed)), vec![Request::Closed]);
        restart(&store, 6);
    }
}

#[test]
fn a_restart_after_a_move_in_before_its_first_grant_refreshes_again() {
    let mut world = started(&[]);
    let mut store = BTreeMap::new();
    let source = world.domain.next_number();
    commit(&mut store, &world.step(change(2, ALICE, Change::MoveIn { carried: CARRIED })));
    open(&mut world, source, b"alice");
    commit(&mut store, &world.step(Event::Adopted { source, kept: true }));
    restart(&store, source);
}

#[test]
fn a_move_out_waits_for_a_key_keep_joining_its_account() {
    let mut world = World::with_limits(
        jig_domain_accounts::Limits { sign_ins: 2, ..LIMITS },
        providers(vec![provider(1, true, true)]),
        &rows(false),
    );
    world.step(Event::Want);
    let source = world.domain.next_number();
    assert_eq!(
        world.step(change(3, ALICE, Change::AddKey { provider: 1, join: Some(5), carried: CARRIED })),
        vec![
            save(Stored::Next { number: source + 1 }),
            Request::Signing(Signing::Keep { source, carried: CARRIED, replaces: false }),
        ]
    );
    let next = world.domain.next_number();
    assert_eq!(
        world.step(change(1, ALICE, Change::MoveOut { account: 5 })),
        refused(1, Refusal::Limit { bound: Bound::SignIns, number: None })
    );
    assert_eq!(world.domain.next_number(), next);
    assert_eq!(world.domain.next_deadline(), None);
    assert_eq!(world.domain.source(6).expect("the sign-in stays").state, State::Asking);
    world.step(Event::Component(FromComponent::Granted {
        account: source,
        generation: 1,
        valid: Duration::from_secs(100),
        form: Form::Credential(Scheme::Key),
    }));
    start_out(&mut world);
}

#[test]
fn a_key_keep_cannot_join_an_account_while_its_source_moves() {
    let mut world = World::with_limits(
        jig_domain_accounts::Limits { sign_ins: 2, ..LIMITS },
        providers(vec![provider(1, true, true)]),
        &rows(false),
    );
    world.step(Event::Want);
    let number = start_out(&mut world);
    for answered in [false, true] {
        if answered {
            withdrawn(&mut world, number);
        }
        let next = world.domain.next_number();
        assert_eq!(
            world.step(change(3, ALICE, Change::AddKey { provider: 1, join: Some(5), carried: CARRIED })),
            refused(3, Refusal::Moving)
        );
        assert_eq!(world.domain.next_number(), next);
        moving_fact(&world, 6);
    }
}

#[test]
fn a_returned_moves_refresh_needing_sign_in_lifts_its_moving_hold() {
    for failure in [Failure::SignInAgain, Failure::MissingScope { scope: 1 }] {
        let mut world = started(&rows(false));
        let number = start_out(&mut world);
        withdrawn(&mut world, number);
        world.fire_at(60);
        assert_eq!(
            world.step(Event::Component(FromComponent::Failed { account: 6, failure })),
            vec![save(source(5, 6, 0, Kind::SignIn))]
        );
        let fact = world.domain.source(6).expect("the source needs its holder");
        assert_eq!((fact.state, fact.usable, fact.generation), (State::NeedsSignIn, false, None));
        assert!(fact.attention);
        assert_eq!(world.step(change(3, ALICE, Change::MoveOut { account: 5 })), refused(3, Refusal::NotSignedIn));
    }
}

#[test]
fn a_move_in_checks_its_subject_again_after_adoption() {
    let mut world = World::with_limits(
        jig_domain_accounts::Limits { sign_ins: 2, ..LIMITS },
        providers(vec![provider(1, true, true)]),
        &rows(false),
    );
    world.step(Event::Want);
    let arriving = start_in(&mut world, ALICE);
    open(&mut world, arriving, b"alice");
    start_out(&mut world);
    assert_eq!(
        world.step(Event::Adopted { source: arriving, kept: true }),
        vec![
            Request::Signing(Signing::Forget { source: arriving }),
            answer(2, Answer::Refused { refusal: Refusal::Moving }),
        ]
    );
    moving_fact(&world, 6);
    assert!(world.domain.source(arriving).is_none());

    let mut world = World::with_limits(
        jig_domain_accounts::Limits { sign_ins: 2, ..LIMITS },
        providers(vec![provider(1, true, true)]),
        &[],
    );
    world.step(Event::Want);
    let arriving = start_in(&mut world, ALICE);
    open(&mut world, arriving, b"alice");
    let signed_in = world.domain.next_number();
    world.step(Event::SignIn { request: Token::new(3), holder: BOB, by: 4, provider: 1 });
    world.step(Event::Visited { source: signed_in, address: Box::new([]) });
    world.step(Event::SignedIn {
        source: signed_in,
        subject: Box::from(b"alice".as_slice()),
        display: Box::new([]),
        declined: false,
    });
    assert_eq!(
        world.step(Event::Adopted { source: arriving, kept: true }),
        vec![
            Request::Signing(Signing::Forget { source: arriving }),
            answer(2, Answer::Refused { refusal: Refusal::Held { provider: 1 } }),
        ]
    );
    assert_eq!(world.domain.accounts(), 1);
    assert!(world.domain.source(arriving).is_none());
    assert_eq!(
        world
            .domain
            .account(world.domain.source(signed_in).expect("the other holder's source stays").account)
            .expect("the other holder's account stays")
            .holder(),
        BOB
    );
}
