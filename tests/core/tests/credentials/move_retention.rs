//! A move's completion must remain possible at the retained-answer boundary.
use super::*;

const SEED: u64 = 211;
// The fixture's chat, the account sign-in, and its move out each retain one answer.
const LAST_REQUEST: u32 = 3;
// The same setup plus both transfer answers.
const WHOLE_MOVE: u32 = 4;
// One transfer occupies the only pending slot; Written must take that same slot.
const TRANSFER_PENDING: u32 = 1;
const EXPORT: u8 = 50;
const FRESH_EXPORT: u8 = 51;
const WRITTEN: u8 = 52;
const RETRY: u8 = 53;

fn change(change: people::AccountChange) -> people::Ask {
    people::Ask::AccountChange { holder: people::AccountHolder::Own, change }
}

fn signed(requests: u32) -> (World, u32, u32) {
    let (config, mut limits) = signing_configuration(SEED);
    limits.core.people.requests = requests;
    limits.core.people.pending = TRANSFER_PENDING;
    jig_core_world::peers::journal(&mut limits);
    let mut world = World::configured(SEED, false, config, limits);
    let (account, source) = signed_in(&mut world);
    world.announce_grant(source, 1, accounts::Form::Credential(accounts::Scheme::Bearer));
    (world, account, source)
}

fn moving(requests: u32) -> (World, u32, u32, u32) {
    let (mut world, account, source) = signed(requests);
    let number = world.domain.accounts().next_number();
    ask(&mut world, EXPORT, change(people::AccountChange::MoveOut { account }));
    world.take_moved(source).expect("the export received its fake record");
    (world, account, source, number)
}

fn fresh_session(world: &mut World) {
    let identity = world
        .store
        .rows
        .values()
        .find_map(|record| {
            if let root::Record::Core(core::Record::People(people::Stored::Person { identity, .. })) = record {
                Some(identity.clone())
            } else {
                None
            }
        })
        .expect("the fixture's person survives restart");
    world.send(root::Event::Core(core::Event::SignIn {
        held: None,
        reply_to: skein_lib::ReplyTo::new(Token::new(9000)),
        identity,
    }));
}

fn pass_time(world: &mut World, duration: Duration) {
    let (now, wall) = world.clocks();
    world.fire_due(now.saturating_add(duration), Wall::from_nanos(wall.as_nanos() + duration.as_nanos()));
    // People expires one retained answer per turn; drain each due turn before asking again.
    let (now, wall) = world.clocks();
    while world
        .domain
        .core_deadlines(now, wall)
        .into_iter()
        .any(|(timer, due)| timer == core::Timer::People && due.is_some_and(|due| due <= now))
    {
        world.send(root::Event::Timer(core::Timer::People));
    }
    fresh_session(world);
}

fn deadline(world: &mut World) {
    pass_time(world, signing_configuration(SEED).1.core.accounts.move_deadline);
}

fn answer_count(world: &World) -> usize {
    world
        .store
        .rows
        .values()
        .filter(|row| {
            matches!(
                row,
                root::Record::Core(core::Record::People(people::Stored::Answer { .. } | people::Stored::Acted { .. }))
            )
        })
        .count()
}

fn retry(world: &mut World, source: u32) -> Option<people::Reply> {
    ask(world, RETRY, change(people::AccountChange::Resume { source })).cloned()
}

fn export_again(world: &mut World, account: u32, source: u32) {
    let number = world.domain.accounts().next_number();
    assert_eq!(
        ask(world, FRESH_EXPORT, change(people::AccountChange::MoveOut { account })),
        Some(&people::Reply::Outcome(people::Outcome::AccountMovingOut { account, number, source }))
    );
}

#[test]
fn an_unacknowledged_move_can_export_again_after_restart() {
    let (mut world, account, source, _) = moving(signing_configuration(SEED).1.core.people.requests);
    deadline(&mut world);
    world.restart_with(signing_configuration(SEED).0);
    fresh_session(&mut world);
    export_again(&mut world, account, source);
}

#[test]
fn two_free_places_admit_the_move_and_written() {
    let (mut world, account, source, number) = moving(WHOLE_MOVE);
    let written = ask(&mut world, WRITTEN, change(people::AccountChange::Written { number })).cloned();
    let replay = ask(&mut world, WRITTEN, change(people::AccountChange::Written { number })).cloned();
    let after = retry(&mut world, source);
    assert_eq!(
        (
            written,
            replay,
            matches!(after, Some(people::Reply::Again(people::Again::Full { held: WHOLE_MOVE, .. }))),
            answer_count(&world),
            world.domain.accounts().account(account).is_none(),
            world.domain.accounts().source(source).is_none()
        ),
        (
            Some(people::Reply::Outcome(people::Outcome::AccountChanged)),
            Some(people::Reply::Outcome(people::Outcome::AccountChanged)),
            true,
            usize::try_from(WHOLE_MOVE).expect("tiny answer count"),
            true,
            true
        )
    );
}

#[test]
fn one_free_place_refuses_without_touching_the_source() {
    let (mut world, account, source) = signed(LAST_REQUEST);
    let fact = world.domain.accounts().source(source).expect("signed-in source");
    let generation = fact.generation;
    let before = world.domain.accounts().next_deadline();
    let next = world.domain.accounts().next_number();
    let refused = ask(&mut world, EXPORT, change(people::AccountChange::MoveOut { account })).cloned();
    let fact = world.domain.accounts().source(source).expect("source unchanged");
    assert_eq!(
        (
            matches!(refused, Some(people::Reply::Again(people::Again::Full { held: 2, .. }))),
            fact.state,
            fact.usable,
            fact.generation,
            world.domain.accounts().next_deadline(),
            world.domain.accounts().next_number(),
            world.take_moved(source).is_none()
        ),
        (true, accounts::State::Granted, true, generation, before, next, true)
    );
}

#[test]
fn the_deadline_frees_the_unwritten_moves_second_place() {
    let (mut world, _, source, _) = moving(WHOLE_MOVE);
    let before = retry(&mut world, source);
    deadline(&mut world);
    let after = retry(&mut world, source);
    assert_eq!(
        (
            matches!(before, Some(people::Reply::Again(people::Again::Full { held: WHOLE_MOVE, .. }))),
            after,
            answer_count(&world)
        ),
        (
            true,
            Some(people::Reply::Outcome(people::Outcome::Refused(people::Refusal::NotOffered))),
            usize::try_from(WHOLE_MOVE).expect("tiny answer count")
        )
    );
}

#[test]
fn restarting_keeps_only_the_moves_stored_answer_place() {
    let (mut world, _, source, _) = moving(WHOLE_MOVE);
    world.restart_with(signing_configuration(SEED).0);
    fresh_session(&mut world);
    assert_eq!(
        (retry(&mut world, source), answer_count(&world)),
        (
            Some(people::Reply::Outcome(people::Outcome::Refused(people::Refusal::NotOffered))),
            usize::try_from(WHOLE_MOVE).expect("tiny answer count")
        )
    );
}

#[test]
fn an_existing_full_store_waits_for_answers_to_expire() {
    let (mut world, account, source, _) = moving(WHOLE_MOVE);
    // These three committed rows are also the old admission's last-slot result.
    // Cold restore at that table size has no memory-only Written reservation.
    let (config, mut limits) = signing_configuration(SEED);
    limits.core.people.requests = LAST_REQUEST;
    jig_core_world::peers::journal(&mut limits);
    world.restart_with_limits(config, limits);
    fresh_session(&mut world);
    let before = ask(&mut world, FRESH_EXPORT, change(people::AccountChange::MoveOut { account })).cloned();
    pass_time(&mut world, limits.core.people.request_retention);
    let number = world.domain.accounts().next_number();
    let after = ask(&mut world, FRESH_EXPORT, change(people::AccountChange::MoveOut { account })).cloned();
    assert_eq!(
        (matches!(before, Some(people::Reply::Again(people::Again::Full { held: LAST_REQUEST, .. }))), after),
        (true, Some(people::Reply::Outcome(people::Outcome::AccountMovingOut { account, number, source })))
    );
}

#[test]
fn a_refused_withdrawal_releases_its_written_answer_place() {
    let (mut world, account, source) = signed(WHOLE_MOVE);
    world.send(root::Event::Core(core::Event::Account(accounts::Event::Component(accounts::FromComponent::Failed {
        account: source,
        failure: accounts::Failure::SignInAgain,
    }))));
    let refused = ask(&mut world, EXPORT, change(people::AccountChange::MoveOut { account })).cloned();
    let after = retry(&mut world, source);
    assert_eq!(
        (refused, after, answer_count(&world)),
        (
            Some(people::Reply::Outcome(people::Outcome::Refused(people::Refusal::AccountMove {
                why: people::AccountMoveWhy::NotSignedIn
            }))),
            Some(people::Reply::Outcome(people::Outcome::Refused(people::Refusal::NotOffered))),
            usize::try_from(WHOLE_MOVE).expect("tiny answer count")
        )
    );
}

#[test]
fn closing_accounts_releases_the_unwritten_moves_answer_place() {
    let (mut world, _, source, _) = moving(WHOLE_MOVE);
    world.skip_restart_oracle("this fixture deliberately closes the accounts child while leaving the root open");
    world.send(root::Event::Core(core::Event::Account(accounts::Event::Close)));
    // The request reaches the closed accounts child instead of failing people's capacity check.
    assert_eq!(retry(&mut world, source), Some(people::Reply::Again(people::Again::NotReady)));
}
