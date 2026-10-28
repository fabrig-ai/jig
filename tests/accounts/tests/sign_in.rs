//! A party's sign-in and sign-out as accounts.md, 3.3 and section 5 have them: decided by the
//! child, asked of the component, each change saved or erased in the step that makes it.
use jig_accounts_world::{World, account, provider, providers, source};
use jig_domain_accounts::{
    Answer, Bound, DISPLAY, End, Ended, Event, Failure, Form, FromComponent, Held, Holder, Key, Kind, Refusal, Request,
    Scheme, Signing, State, Stored, ToComponent,
};
use skein_lib::{Duration, Token};
use std::collections::BTreeMap;

const ALICE: Holder = Holder::Party(9);
const BOB: Holder = Holder::Party(4);

/// Providers 1 and 2 offer a sign-in, 3 a key alone; none is a `none` provider, so the start
/// makes nothing.
fn fresh() -> World {
    World::with(providers(vec![provider(1, true, true), provider(2, true, false), provider(3, false, true)]), &[])
}
fn started(records: Vec<Stored>) -> World {
    let mut world = fresh();
    world.restore(records);
    world.step(Event::Want);
    world
}
fn sign_in(request: u64, holder: Holder, provider: u32) -> Event {
    Event::SignIn { request: Token::new(request), holder, by: request, provider }
}
fn sign_out(request: u64, holder: Holder, source: u32) -> Event {
    Event::SignOut { request: Token::new(request), holder, source }
}
fn visited(source: u32) -> Event {
    Event::Visited { source, address: b"https://issuer.test/visit".as_slice().into() }
}
fn signed_in(source: u32, subject: &[u8], display: &[u8], declined: bool) -> Event {
    Event::SignedIn { source, subject: subject.into(), display: display.into(), declined }
}
fn answer(request: u64, answer: Answer) -> Request {
    Request::Answer { request: Token::new(request), answer }
}
fn refused(request: u64, refusal: Refusal) -> Vec<Request> {
    vec![answer(request, Answer::Refused { refusal })]
}
fn ended(request: u64, source: u32, end: End) -> Vec<Ended> {
    vec![Ended { request: Token::new(request), source, by: request, end }]
}
fn save(record: Stored) -> Request {
    Request::Save { record }
}
fn want(source: u32) -> Request {
    Request::Component(ToComponent::Want { account: source })
}
/// A sign-in starts: its source's number taken and the counter saved, the component asked, and
/// the account sign-in waits for its visit.
fn starts(_request: u64, provider: u32, source: u32) -> Vec<Request> {
    vec![save(Stored::Next { number: source + 1 }), Request::Signing(Signing::SignIn { source, provider })]
}
/// The store's family as the core commits it.
fn commit(store: &mut BTreeMap<Key, Stored>, requests: &[Request]) {
    for request in requests {
        match request {
            Request::Save { record } => drop(store.insert(record.key(), record.clone())),
            Request::Erase { key } => drop(store.remove(key)),
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
fn places(world: &World, account: u32) -> Vec<(u32, u32)> {
    let account = world.domain.account(account).expect("kept");
    account.sources().iter().map(|(place, source)| (*place, *source)).collect()
}

#[test]
fn a_sign_in_to_a_new_subject_makes_an_account_that_restores() {
    let mut store = BTreeMap::new();
    let mut world = started(vec![]);
    let asked = world.step(sign_in(1, ALICE, 1));
    assert_eq!(asked, starts(1, 1, 0));
    commit(&mut store, &asked);
    let made = Stored::Account {
        place: 0,
        uses: None,
        number: 1,
        provider: 1,
        holder: ALICE,
        subject: Box::from(&b"alice"[..]),
        display: Box::from(&b"Al"[..]),
    };
    world.step(visited(0));
    let asked = world.step(signed_in(0, b"alice", b"Al", false));
    assert_eq!(asked, vec![save(Stored::Next { number: 2 }), save(made), save(source(1, 0, 0, Kind::SignIn)), want(0)]);
    commit(&mut store, &asked);
    assert_eq!(world.ended(), ended(1, 0, End::Made { account: 1, declined: false }));
    let account = world.domain.account(1).expect("made");
    assert_eq!(
        (account.holder(), account.provider(), account.subject(), account.display()),
        (ALICE, 1, &b"alice"[..], &b"Al"[..])
    );
    assert_eq!(places(&world, 1), vec![(0, 0)]);
    assert_eq!(world.domain.source(0).expect("its source").state, State::Asking);
    let mut again = fresh();
    again.restore(store.into_values().collect());
    assert_eq!(
        again.step(Event::Want),
        vec![
            Request::Signing(Signing::Restore { source: 0, provider: 1, kind: Kind::SignIn }),
            Request::Signing(Signing::Restored),
            want(0)
        ],
        "its records restore it, and the component is asked for it"
    );
    assert_eq!((again.domain.account(1).expect("restored").display(), places(&again, 1)), (&b"Al"[..], vec![(0, 0)]));
    world.step(sign_in(2, BOB, 2));
    let long = vec![b'd'; usize::try_from(DISPLAY).expect("small") + 1];
    world.step(visited(2));
    world.step(signed_in(2, b"bob", &long, false));
    assert_eq!(world.domain.account(3).expect("made").display(), b"", "a display name too long is not kept");
}

#[test]
fn a_sign_in_to_a_held_subject_renews_its_account() {
    let mut world = started(vec![
        account(5, 1, ALICE, b"alice"),
        source(5, 6, 0, Kind::SignIn),
        source(5, 7, 1, Kind::Key),
        account(10, 2, ALICE, b"kept"),
        source(10, 11, 0, Kind::Key),
        Stored::Next { number: 12 },
    ]);
    assert_eq!(world.step(sign_in(1, ALICE, 1)), starts(1, 1, 12));
    world.step(visited(12));
    assert_eq!(
        world.step(signed_in(12, b"alice", b"other", false)),
        vec![
            Request::Erase { key: Key::Source { account: 5, number: 6 } },
            Request::Signing(Signing::Replaced { source: 6, by: 12 }),
            save(source(5, 12, 0, Kind::SignIn)),
            want(12),
        ],
        "the new source takes the replaced one's place; the component revokes its record where it must"
    );
    assert_eq!(world.ended(), ended(1, 12, End::Renewed { account: 5, declined: false }));
    assert_eq!(places(&world, 5), vec![(0, 12), (1, 7)]);
    assert!(world.domain.source(6).is_none());
    assert_eq!(world.domain.account(5).expect("kept").display(), b"", "the holder's display name stays");
    assert_eq!(world.step(sign_in(2, ALICE, 2)), starts(2, 2, 13));
    world.step(visited(13));
    assert_eq!(world.step(signed_in(13, b"kept", b"", false)), vec![save(source(10, 13, 1, Kind::SignIn)), want(13)]);
    assert_eq!(world.ended(), ended(2, 13, End::Renewed { account: 10, declined: false }));
    assert_eq!(places(&world, 10), vec![(0, 11), (1, 13)], "with no signed-in source, it joins after the last");
}

#[test]
fn a_sign_in_to_a_subject_another_holder_holds_is_refused_and_revoked() {
    let mut world = started(vec![account(5, 1, ALICE, b"alice"), source(5, 6, 0, Kind::SignIn)]);
    assert_eq!(world.step(sign_in(3, BOB, 1)), starts(3, 1, 7));
    world.step(visited(7));
    assert_eq!(world.step(signed_in(7, b"alice", b"", false)), vec![Request::Signing(Signing::SignOut { source: 7 })]);
    assert_eq!(world.ended(), ended(3, 7, End::Refused { refusal: Refusal::Held { provider: 1 } }));
    assert_eq!((world.domain.accounts(), places(&world, 5)), (1, vec![(0, 6)]));
    world.step(sign_in(4, BOB, 1));
    world.step(visited(8));
    assert_eq!(world.step(signed_in(8, b"", b"", false)), vec![Request::Signing(Signing::SignOut { source: 8 })]);
    assert_eq!(world.ended(), ended(4, 8, End::Refused { refusal: Refusal::Subject }));
}

#[test]
fn plan_usage_declined_keeps_the_sign_in_and_refuses_the_source() {
    let mut world = started(vec![]);
    world.step(sign_in(1, ALICE, 1));
    let refused = Stored::Source {
        account: 1,
        number: 0,
        place: 0,
        kind: Kind::SignIn,
        name: Box::new([]),
        held: Some(Held::Refused),
    };
    world.step(visited(0));
    let asked = world.step(signed_in(0, b"alice", b"", true));
    assert_eq!(asked[2..], [save(refused), want(0)]);
    assert_eq!(world.ended(), ended(1, 0, End::Made { account: 1, declined: true }));
    let granted = FromComponent::Granted {
        form: Form::Credential(Scheme::Bearer),
        account: 0,
        generation: 1,
        valid: Duration::from_secs(100),
    };
    world.step(Event::Component(granted));
    let fact = world.domain.source(0).expect("kept");
    assert_eq!((fact.state, fact.usable, fact.attention), (State::Refused, false, true), "until its holder acts");
}

#[test]
fn each_sign_in_refusal_names_its_bound_or_rule() {
    let mut world = fresh();
    assert_eq!(world.step(sign_in(1, ALICE, 1)), refused(1, Refusal::Closing), "before the start");
    world.step(Event::Want);
    assert_eq!(world.step(sign_in(2, ALICE, 3)), refused(2, Refusal::Provider { provider: 3 }));
    assert_eq!(world.step(sign_in(3, ALICE, 9)), refused(3, Refusal::Provider { provider: 9 }));
    assert_eq!(world.step(sign_in(4, ALICE, 1)), starts(4, 1, 0));
    assert_eq!(
        world.step(sign_in(5, BOB, 2)),
        refused(
            5,
            Refusal::limited(
                Bound::SignIns,
                jig_domain_accounts::LimitKey::SignIns,
                jig_accounts_world::LIMITS.sign_ins
            )
        )
    );
    assert_eq!(
        world.step(Event::SignInFailed { source: 0, failure: Failure::Cancelled }),
        refused(4, Refusal::NotStarted)
    );
    assert_eq!(world.ended(), ended(4, 0, End::Failed { failure: Failure::Cancelled }));
    world.step(visited(0));
    assert_eq!(world.step(signed_in(0, b"alice", b"", false)), vec![], "an ended sign-in makes nothing");
    assert_eq!(world.step(sign_in(6, BOB, 2)), starts(6, 2, 1), "its room is free again");
    let mut world = started(vec![Stored::Next { number: u32::MAX }]);
    assert_eq!(world.step(sign_in(7, ALICE, 1)), refused(7, Refusal::Numbers));
}

#[test]
fn a_sign_out_removes_the_source_and_answers_once_revoked() {
    let mut world = started(vec![
        account(5, 1, ALICE, b"alice"),
        source(5, 6, 0, Kind::SignIn),
        source(5, 7, 1, Kind::Key),
        account(10, 2, ALICE, b"solo"),
        source(10, 11, 0, Kind::SignIn),
    ]);
    assert_eq!(world.step(sign_out(1, BOB, 6)), refused(1, Refusal::Unknown), "only its holder signs it out");
    assert_eq!(world.step(sign_out(1, ALICE, 9)), refused(1, Refusal::Unknown));
    assert_eq!(world.step(sign_out(1, ALICE, 7)), refused(1, Refusal::Kind { kind: Kind::Key }));
    assert_eq!(
        world.step(sign_out(2, ALICE, 6)),
        vec![
            Request::Erase { key: Key::Source { account: 5, number: 6 } },
            save(source(5, 7, 0, Kind::Key)),
            Request::Signing(Signing::SignOut { source: 6 })
        ]
    );
    assert_eq!((places(&world, 5), world.domain.source(6)), (vec![(0, 7)], None));
    assert_eq!(
        world.step(Event::SignedOut { source: 6, confirmed: false }),
        vec![answer(2, Answer::SignedOut { confirmed: false, failed_hosts: Box::new([]) })],
        "unconfirmed, its holder disconnects the application at the provider"
    );
    assert_eq!(
        world.step(sign_out(3, ALICE, 11)),
        vec![
            Request::Erase { key: Key::Source { account: 10, number: 11 } },
            Request::Erase { key: Key::Account { number: 10 } },
            Request::Signing(Signing::SignOut { source: 11 })
        ],
        "an account goes with its last source, as the store keeps none without one"
    );
    assert!(world.domain.account(10).is_none());
    assert_eq!(
        world.step(Event::SignedOut { source: 11, confirmed: true }),
        vec![answer(3, Answer::SignedOut { confirmed: true, failed_hosts: Box::new([]) })]
    );
    assert_eq!(world.step(Event::SignedOut { source: 11, confirmed: true }), vec![], "answered once");
}

#[test]
fn an_account_sign_in_is_answered_started_with_the_address_when_the_component_visits() {
    let mut world = started(vec![]);
    assert_eq!(world.step(sign_in(1, ALICE, 1)), starts(1, 1, 0));
    let address = b"https://issuer.test/visit".as_slice().into();
    assert_eq!(world.step(visited(0)), vec![answer(1, Answer::Started { source: 0, address })]);
    assert_eq!(world.domain.facts_lost(), 0);
}

#[test]
fn a_second_account_visit_or_the_visit_of_no_account_sign_in_is_ignored() {
    let mut world = started(vec![]);
    assert!(world.step(visited(0)).is_empty());
    world.step(sign_in(1, ALICE, 1));
    world.step(visited(0));
    assert!(world.step(visited(0)).is_empty());
    assert!(world.step(visited(99)).is_empty());
    assert_eq!(world.domain.facts_lost(), 0);
}

#[test]
fn an_account_sign_in_that_ends_before_its_visit_is_answered_not_started() {
    let mut world = started(vec![]);
    world.step(sign_in(1, ALICE, 1));
    assert_eq!(
        world.step(Event::SignInFailed { source: 0, failure: Failure::Cancelled }),
        refused(1, Refusal::NotStarted)
    );
    assert_eq!(world.ended(), ended(1, 0, End::Failed { failure: Failure::Cancelled }));
    assert!(world.step(visited(0)).is_empty());
    world.step(sign_in(2, ALICE, 1));
    let requests = world.step(signed_in(1, b"alice", b"", false));
    assert_eq!(requests.first(), refused(2, Refusal::NotStarted).first());
    assert_eq!(requests.iter().filter(|r| matches!(r, Request::Answer { .. })).count(), 1);
    assert_eq!(world.ended(), ended(2, 1, End::Made { account: 2, declined: false }));
}

#[test]
fn an_account_sign_in_that_visited_is_not_answered_again_when_it_ends() {
    for fails in [false, true] {
        let mut world = started(vec![]);
        world.step(sign_in(1, ALICE, 1));
        world.step(visited(0));
        let event = if fails {
            Event::SignInFailed { source: 0, failure: Failure::Cancelled }
        } else {
            signed_in(0, b"alice", b"", false)
        };
        let requests = world.step(event);
        assert!(!requests.iter().any(|r| matches!(r, Request::Answer { .. })));
        assert_eq!(world.ended().len(), 1);
        assert!(world.step(visited(0)).is_empty());
    }
}

#[test]
fn a_close_cancels_each_account_sign_in_and_answers_it_closing_before_its_visit() {
    for visit in [false, true] {
        let mut world = started(vec![]);
        world.step(sign_in(1, ALICE, 1));
        if visit {
            world.step(visited(0));
        }
        let mut expected = vec![Request::Signing(Signing::CancelSignIn { source: 0 })];
        if !visit {
            expected.extend(refused(1, Refusal::Closing));
        }
        expected.push(Request::Component(ToComponent::Close));
        assert_eq!(world.step(Event::Close), expected);
        assert!(world.step(Event::Close).is_empty());
        assert!(world.step(visited(0)).is_empty());
        assert_eq!(world.step(Event::Component(FromComponent::Closed)), vec![Request::Closed]);
    }
}
