//! A holder's LLM keys and account changes, decided in the child.
use jig_accounts_world::{World, account, provider, providers, source};
use jig_domain_accounts::{
    Answer, Bound, Change, Event, FromComponent, Holder, KEY_VALID, Key, KeyFailure, Kind, Refusal, Request, Signing,
    State, Stored, ToComponent, USES, Use,
};
use skein_lib::{Duration, Token};
const HOLDER: Holder = Holder::Party(7);
const OTHER: Holder = Holder::Party(8);
const REQUEST: Token = Token::new(50);
const PROVIDER: u32 = 1;
const CARRIED: u64 = 1 << 63;

fn fresh() -> World {
    let mut world = World::with(providers(vec![provider(PROVIDER, true, true)]), &[]);
    world.step(Event::Want);
    world
}
fn change(world: &mut World, change: Change) -> Vec<Request> {
    world.step(Event::Change { request: REQUEST, holder: HOLDER, change })
}
fn refused(refusal: Refusal) -> Request {
    Request::Answer { request: REQUEST, answer: Answer::Refused { refusal } }
}
fn done() -> Request {
    Request::Answer { request: REQUEST, answer: Answer::Done }
}
fn add(world: &mut World, join: Option<u32>) -> u32 {
    let outputs = change(world, Change::AddKey { provider: PROVIDER, join, carried: CARRIED });
    outputs
        .iter()
        .find_map(
            |request| if let Request::Signing(Signing::Keep { source, .. }) = request { Some(*source) } else { None },
        )
        .expect("an admitted LLM key keep")
}
fn grant(world: &mut World, source: u32, generation: u64) -> Vec<Request> {
    world.step(Event::Component(FromComponent::Granted {
        form: jig_domain_accounts::Form::Credential(jig_domain_accounts::Scheme::Key),
        account: source,
        generation,
        valid: Duration::from_secs(1),
    }))
}
fn kept(world: &mut World, join: Option<u32>) -> u32 {
    let source = add(world, join);
    assert!(
        grant(world, source, 1).contains(&Request::Answer { request: REQUEST, answer: Answer::KeyKept { source } })
    );
    source
}
fn account_of(world: &World, source: u32) -> u32 {
    world.domain.source(source).expect("kept source").account
}

#[test]
fn a_key_makes_an_account_once_it_is_kept() {
    let mut world = fresh();
    let source = add(&mut world, None);
    assert_eq!(world.domain.accounts(), 0);
    assert!(world.domain.source(source).is_none());
    let outputs = grant(&mut world, source, 1);
    let account = account_of(&world, source);
    assert_eq!(world.domain.accounts(), 1);
    let entry = world.domain.account(account).expect("new account");
    assert_eq!(
        (entry.provider(), entry.holder(), entry.place(), entry.plan(), entry.uses()),
        (PROVIDER, HOLDER, 0, false, None)
    );
    assert!(entry.subject().is_empty() && entry.display().is_empty());
    assert!(outputs.iter().any(
        |request| matches!(request, Request::Save { record: Stored::Account { number, .. } } if *number == account)
    ));
    assert!(outputs.iter().any(|request| matches!(request, Request::Save { record: Stored::Source { number, kind: Kind::Key, .. } } if *number == source)));
}
#[test]
fn a_key_joins_the_holders_account_of_its_provider() {
    let mut world = fresh();
    let first = kept(&mut world, None);
    let account = account_of(&world, first);
    let second = kept(&mut world, Some(account));
    assert_eq!(account_of(&world, second), account);
    assert_eq!(world.domain.accounts(), 1);
    assert_eq!(
        world.domain.account(account).expect("account").sources().iter().map(|(p, s)| (*p, *s)).collect::<Vec<_>>(),
        vec![(0, first), (1, second)]
    );
}
#[test]
fn a_key_joining_another_holders_account_is_refused_unknown() {
    let mut world = fresh();
    let first = kept(&mut world, None);
    let account = account_of(&world, first);
    assert_eq!(
        world.step(Event::Change {
            request: REQUEST,
            holder: OTHER,
            change: Change::AddKey { provider: PROVIDER, join: Some(account), carried: CARRIED }
        }),
        vec![refused(Refusal::Unknown)]
    );
}
#[test]
fn a_key_is_granted_generation_one_and_a_replaced_key_the_next() {
    let mut world = fresh();
    let source = kept(&mut world, None);
    let account = account_of(&world, source);
    let first = world.domain.grant(account, world.now).expect("first grant");
    assert_eq!((first.account, first.generation, first.valid), (source, 1, KEY_VALID));
    assert_eq!(first.form, jig_domain_accounts::Form::Credential(jig_domain_accounts::Scheme::Key));
    assert_eq!(
        change(&mut world, Change::ReplaceKey { source, carried: CARRIED + 1 }),
        vec![Request::Signing(Signing::Keep { source, carried: CARRIED + 1, replaces: true })]
    );
    grant(&mut world, source, 2);
    assert_eq!(world.domain.grant(account, world.now).expect("replacement").generation, 2);
    assert!(world.step(Event::Component(FromComponent::Expiring { account: source, generation: 2 })).is_empty());
    assert!(!world.domain.source(source).expect("source").expiring);
}
#[test]
fn a_replaced_key_clears_a_refusal_and_a_limit() {
    for held in [false, true] {
        let mut world = fresh();
        let source = kept(&mut world, None);
        world.step(if held {
            Event::Limited { account: source, reset: None }
        } else {
            Event::Refused { account: source, code: Box::new([]), reason: Box::new([]) }
        });
        assert!(!world.domain.source(source).expect("source").usable);
        change(&mut world, Change::ReplaceKey { source, carried: CARRIED + 1 });
        let outputs = grant(&mut world, source, 2);
        assert!(world.domain.source(source).expect("replaced").usable);
        assert!(outputs.iter().any(|r| matches!(r, Request::Save { record: Stored::Source { held: None, .. } })));
    }
}
#[test]
fn a_failed_keep_makes_nothing_and_keeps_the_old_key() {
    let mut world = fresh();
    let source = add(&mut world, None);
    assert_eq!(
        world.step(Event::Component(FromComponent::KeepFailed { account: source, why: KeyFailure::TooLong })),
        vec![refused(Refusal::NotKept { why: KeyFailure::TooLong })]
    );
    assert_eq!(world.domain.accounts(), 0);
    let source = kept(&mut world, None);
    let account = account_of(&world, source);
    world.step(Event::Limited { account: source, reset: None });
    let before = world.domain.source(source);
    change(&mut world, Change::ReplaceKey { source, carried: CARRIED + 1 });
    assert_eq!(
        world.step(Event::Component(FromComponent::KeepFailed { account: source, why: KeyFailure::NotStored })),
        vec![refused(Refusal::NotKept { why: KeyFailure::NotStored })]
    );
    assert_eq!(world.domain.source(source), before);
    assert!(world.domain.account(account).is_some());
}
#[test]
fn keys_being_kept_are_bounded_with_the_sign_ins() {
    let mut world = fresh();
    let source = add(&mut world, None);
    assert_eq!(
        change(&mut world, Change::AddKey { provider: PROVIDER, join: None, carried: CARRIED }),
        vec![refused(Refusal::limited(
            Bound::SignIns,
            jig_domain_accounts::LimitKey::SignIns,
            jig_accounts_world::LIMITS.sign_ins
        ))]
    );
    assert_eq!(
        world.step(Event::SignIn { request: REQUEST, holder: HOLDER, by: 7, provider: PROVIDER }),
        vec![refused(Refusal::limited(
            Bound::SignIns,
            jig_domain_accounts::LimitKey::SignIns,
            jig_accounts_world::LIMITS.sign_ins
        ))]
    );
    world.step(Event::Component(FromComponent::KeepFailed { account: source, why: KeyFailure::NotStored }));
    world.step(Event::SignIn { request: REQUEST, holder: HOLDER, by: 7, provider: PROVIDER });
    assert_eq!(
        change(&mut world, Change::AddKey { provider: PROVIDER, join: None, carried: CARRIED }),
        vec![refused(Refusal::limited(
            Bound::SignIns,
            jig_domain_accounts::LimitKey::SignIns,
            jig_accounts_world::LIMITS.sign_ins
        ))]
    );
}
#[test]
fn a_resume_clears_a_limit_and_names_a_refusal_not_limited() {
    let mut world = fresh();
    let source = kept(&mut world, None);
    world.step(Event::Limited { account: source, reset: None });
    let outputs = change(&mut world, Change::Resume { source });
    assert_eq!(outputs.last(), Some(&done()));
    assert!(world.domain.source(source).expect("source").usable);
    world.step(Event::Refused { account: source, code: Box::new([]), reason: Box::new([]) });
    assert_eq!(change(&mut world, Change::Resume { source }), vec![refused(Refusal::NotLimited)]);
    assert_eq!(world.domain.source(source).expect("source").state, State::Refused);
}
#[test]
fn removing_a_key_erases_its_rows_and_forgets_its_value() {
    let mut world = fresh();
    let source = kept(&mut world, None);
    let account = account_of(&world, source);
    assert_eq!(
        change(&mut world, Change::RemoveKey { source }),
        vec![
            Request::Erase { key: Key::Source { account, number: source } },
            Request::Erase { key: Key::Account { number: account } },
            Request::Signing(Signing::Forget { source }),
            done()
        ]
    );
    assert!(world.domain.account(account).is_none());
    assert!(world.domain.source(source).is_none());
}
#[test]
fn removing_an_account_answers_when_each_sign_out_has() {
    let mut world = World::with(providers(vec![provider(PROVIDER, true, true)]), &[]);
    world.restore(vec![
        account(10, PROVIDER, HOLDER, b"subject"),
        source(10, 11, 0, Kind::SignIn),
        source(10, 12, 1, Kind::Key),
    ]);
    world.step(Event::Want);
    let outputs = change(&mut world, Change::Remove { account: 10 });
    assert!(outputs.contains(&Request::Signing(Signing::SignOut { source: 11 })));
    assert!(outputs.contains(&Request::Signing(Signing::Forget { source: 12 })));
    assert!(!outputs.iter().any(|r| matches!(r, Request::Answer { .. })));
    assert!(world.domain.account(10).is_none());
    assert_eq!(
        world.step(Event::SignedOut { source: 11, confirmed: false }),
        vec![Request::Answer {
            request: REQUEST,
            answer: Answer::Removed { confirmed: false, failed_hosts: Box::new([]) }
        }]
    );
    assert!(world.step(Event::SignedOut { source: 11, confirmed: true }).is_empty());
}
#[test]
fn a_removed_accounts_number_is_never_given_again() {
    let mut world = fresh();
    let first = kept(&mut world, None);
    let account = account_of(&world, first);
    change(&mut world, Change::Remove { account });
    let next = world.domain.next_number();
    let mut restarted = World::with(providers(vec![provider(PROVIDER, true, true)]), &[]);
    restarted.restore(vec![Stored::Next { number: next }]);
    restarted.step(Event::Want);
    let source = kept(&mut restarted, None);
    assert!(source > account && account_of(&restarted, source) > account);
}
#[test]
fn an_account_moves_among_the_holders_and_saves_each_shifted() {
    let mut world = fresh();
    let first = kept(&mut world, None);
    let first = account_of(&world, first);
    let second = kept(&mut world, None);
    let second = account_of(&world, second);
    let outputs = change(&mut world, Change::MoveAccount { account: second, place: 0 });
    assert_eq!(
        (world.domain.account(first).expect("first").place(), world.domain.account(second).expect("second").place()),
        (1, 0)
    );
    let saved = outputs.iter().filter(|r| matches!(r, Request::Save { record: Stored::Account { .. } })).count();
    assert_eq!(saved, 2);
    change(&mut world, Change::MoveAccount { account: second, place: u32::MAX });
    assert_eq!(world.domain.account(second).expect("second").place(), 1);
}
#[test]
fn a_source_moves_in_its_accounts_order() {
    let mut world = fresh();
    let first = kept(&mut world, None);
    let account = account_of(&world, first);
    let second = kept(&mut world, Some(account));
    let outputs = change(&mut world, Change::MoveSource { account, source: second, place: 0 });
    assert_eq!(
        world.domain.account(account).expect("account").sources().iter().map(|(p, s)| (*p, *s)).collect::<Vec<_>>(),
        vec![(0, second), (1, first)]
    );
    assert_eq!(outputs.iter().filter(|r| matches!(r, Request::Save { record: Stored::Source { .. } })).count(), 2);
}
#[test]
fn uses_are_bounded_saved_and_restored() {
    let mut world = fresh();
    let first = kept(&mut world, None);
    let account = account_of(&world, first);
    let uses: Box<[_]> = (0..USES).map(Use::Project).collect();
    let outputs = change(&mut world, Change::SetUses { account, uses: Some(uses.clone()) });
    assert_eq!(outputs.last(), Some(&done()));
    assert_eq!(world.domain.account(account).expect("account").uses(), Some(uses.as_ref()));
    let saved = outputs
        .iter()
        .find_map(
            |r| if let Request::Save { record: row @ Stored::Account { .. } } = r { Some(row.clone()) } else { None },
        )
        .expect("account saved");
    for invalid in [vec![Use::Party(7); 2], (0..=USES).map(Use::Project).collect()] {
        assert_eq!(
            change(&mut world, Change::SetUses { account, uses: Some(invalid.into_boxed_slice()) }),
            vec![refused(Refusal::Uses)]
        );
    }
    let mut restart = World::with(providers(vec![provider(PROVIDER, true, true)]), &[]);
    restart.restore(vec![saved, source(account, first, 0, Kind::Key)]);
    restart.step(Event::Want);
    assert_eq!(restart.domain.account(account).expect("restored").uses(), Some(uses.as_ref()));
    for invalid in [vec![Use::Party(7); 2], (0..=USES).map(Use::Project).collect()] {
        let mut restart = World::with(providers(vec![provider(PROVIDER, true, true)]), &[]);
        let mut row = jig_accounts_world::account(account, PROVIDER, HOLDER, b"");
        if let Stored::Account { uses, .. } = &mut row {
            *uses = Some(invalid.into_boxed_slice());
        }
        restart.restore(vec![row, source(account, first, 0, Kind::Key)]);
        restart.step(Event::Want);
        assert!(restart.domain.account(account).is_none());
        assert!(restart.domain.unrestored() >= 1);
    }
    for uses in [Some(Box::new([]) as Box<[Use]>), None] {
        change(&mut world, Change::SetUses { account, uses: uses.clone() });
        assert_eq!(world.domain.account(account).expect("account").uses(), uses.as_deref());
    }
}
#[test]
fn a_restart_names_each_party_source_before_wanting_it() {
    let mut world = World::with(
        providers(vec![provider(PROVIDER, true, true)]),
        &[account(1, PROVIDER, Holder::Deployment, b""), source(1, 2, 0, Kind::Key)],
    );
    world.restore(vec![
        account(10, PROVIDER, HOLDER, b"subject"),
        source(10, 12, 1, Kind::Key),
        source(10, 11, 0, Kind::SignIn),
    ]);
    let outputs = world.step(Event::Want);
    let names: Vec<_> = outputs.iter().filter(|r| matches!(r, Request::Signing(_))).cloned().collect();
    assert_eq!(
        names,
        vec![
            Request::Signing(Signing::Restore { source: 2, provider: PROVIDER, kind: Kind::Key }),
            Request::Signing(Signing::Restore { source: 11, provider: PROVIDER, kind: Kind::SignIn }),
            Request::Signing(Signing::Restore { source: 12, provider: PROVIDER, kind: Kind::Key }),
            Request::Signing(Signing::Restored)
        ]
    );
    let end = outputs.iter().position(|r| *r == Request::Signing(Signing::Restored)).expect("names end");
    assert!(
        outputs
            .iter()
            .enumerate()
            .filter(|(_, r)| matches!(r, Request::Component(ToComponent::Want { .. })))
            .all(|(i, _)| i > end)
    );
}
