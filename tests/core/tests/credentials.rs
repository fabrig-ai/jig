use jig_core_world::{
    effects::World,
    fixtures::{CREDENTIAL_RESOURCE, WORKSPACE_CREDENTIAL, ask_as},
    world,
};
use jig_domain_accounts as accounts;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_people as people;
use jig_test_domain as root;
use skein_lib::{Duration, Time, Token, Wall};

#[path = "credentials/move_retention.rs"]
mod move_retention;
#[path = "credentials/moves.rs"]
mod moves;

fn configured(seed: u64) -> World {
    let (config, limits) = world::with_workspace_credential(seed);
    let mut world = World::configured(seed, false, config, limits);
    let (now, wall) = world.clocks();
    world.set_clock(now, wall);
    world
}
fn grant(world: &mut World, generation: u64) {
    world.send(root::Event::Credential { number: WORKSPACE_CREDENTIAL, generation, valid: Duration::from_secs(60) });
}
fn delegated(seed: u64) -> World {
    let mut world = configured(seed);
    grant(&mut world, 3);
    world.delegate_on_resource(CREDENTIAL_RESOURCE);
    world
}
fn hello(world: &mut World) {
    let hosting = world
        .assignments
        .iter()
        .map(|(task, attempt)| fleet::Hosted {
            run: Token::new(*task),
            attempt: Token::new(*attempt),
            phase: fleet::Phase::Active,
        })
        .collect();
    world.send(root::Event::Core(core::Event::Fleet(fleet::Event::Hello {
        worker: 0,
        channel: Token::new(7),
        hello: fleet::Hello {
            bounds: jig_core_world::fixtures::bounds(),
            stop_bound: Duration::from_millis(100),
            slots: 2,
            workstreams: Box::new([]),
            hosting,
        },
    })));
}
fn received(world: &World) -> (Token, fleet::Grant) {
    let (_, task, _, grant) = world.received_grant().expect("host received a credential grant");
    (task, grant)
}
#[test]
fn a_run_needing_a_credential_waits_until_it_is_granted() {
    let mut world = configured(150);
    world.delegate_on_resource(CREDENTIAL_RESOURCE);
    assert_eq!(world.assignments.len(), 1);
    grant(&mut world, 3);
    assert_eq!(world.assignments.len(), 2);
}
#[test]
fn the_assignment_carries_the_credential_named_by_its_table() {
    let world = delegated(151);
    let assignment = world.assigned.as_ref().expect("the delegate is assigned");
    assert!(assignment.grants.contains(&accounts::Grant {
        form: accounts::Form::Application,
        account: WORKSPACE_CREDENTIAL,
        generation: 3,
        valid: Duration::from_secs(60),
    }));
}
#[test]
fn a_new_credential_generation_reaches_the_live_run() {
    let mut world = delegated(152);
    let task = world.assigned.as_ref().expect("delegate assigned").task;
    grant(&mut world, 4);
    assert_eq!(
        received(&world),
        (
            Token::new(task),
            fleet::Grant {
                form: fleet::Form::Application,
                account: WORKSPACE_CREDENTIAL,
                generation: 4,
                valid: Duration::from_secs(60),
            }
        )
    );
}
#[test]
fn an_adopted_run_is_sent_its_credential_again() {
    let mut world = delegated(153);
    let task = world.assigned.as_ref().expect("delegate assigned").task;
    world.restart_with(world::with_workspace_credential(153).0);
    grant(&mut world, 4);
    hello(&mut world);
    assert_eq!(
        received(&world),
        (
            Token::new(task),
            fleet::Grant {
                form: fleet::Form::Application,
                account: WORKSPACE_CREDENTIAL,
                generation: 4,
                valid: Duration::from_secs(60),
            }
        )
    );
}

/// The core world with a provider offering a sign-in, and room for a party's account beside the
/// configured one.
fn signing_configuration(seed: u64) -> (root::Config, root::Limits) {
    let (mut config, mut limits) = jig_core_world::effects::fixture(seed, false);
    config.core.settings.llm_providers = accounts::Providers {
        providers: Box::new([accounts::Provider {
            name: b"Fake".as_slice().into(),
            signed_in_plan: true,
            manage_usage: Some(b"https://usage.example/".as_slice().into()),
            number: PROVIDER,
            models: Box::new([b"scripted".as_slice().into()]),
            offers: accounts::Offers { sign_in: true, key: true, programs: Box::new([]) },
        }]),
    };
    limits.core.accounts.accounts = 2;
    // The accounts story also opens the deployment watch, beside the project role.
    limits.core.people.projects = 2;
    jig_core_world::peers::journal(&mut limits);
    (config, limits)
}
fn signing(seed: u64) -> World {
    let (config, limits) = signing_configuration(seed);
    World::configured(seed, false, config, limits)
}
const PROVIDER: u32 = 3;
fn ask(world: &mut World, key: u8, ask: people::Ask) -> Option<&people::Reply> {
    ask_as(world, world.sign_in.expect("a signed-in party"), key, ask);
    world.people_answers.last()
}
fn signed_in(world: &mut World) -> (u32, u32) {
    let sign_in = people::Ask::AccountSignIn { holder: people::AccountHolder::Own, provider: PROVIDER };
    let Some(people::Reply::Outcome(people::Outcome::AccountSignInStarted { source, address })) =
        ask(world, 40, sign_in).cloned()
    else {
        panic!("the sign-in started: {:?}", world.people_answers.last());
    };
    assert_eq!(&*address, format!("https://issuer.test/visit/{source}").as_bytes());
    let account = world.domain.accounts().source(source).expect("the signed-in source").account;
    (account, source)
}
fn row(key: accounts::Key) -> root::Key {
    root::Key::Core(core::Key::Accounts(key))
}
#[test]
fn a_partys_sign_in_reaches_the_accounts_child_and_its_account_is_saved() {
    let mut world = signing(154);
    let (account, source) = signed_in(&mut world);
    let made = world.domain.accounts().account(account).expect("the account made");
    assert!(matches!(made.holder(), accounts::Holder::Party(_)), "the party's own account");
    assert_eq!(made.subject(), b"signed-in");
    assert!(world.store.rows.contains_key(&row(accounts::Key::Account { number: account })));
    assert!(world.store.rows.contains_key(&row(accounts::Key::Source { account, number: source })));
}
#[test]
fn a_partys_sign_out_removes_its_source_and_account() {
    let mut world = signing(155);
    let (account, source) = signed_in(&mut world);
    let sign_out = people::Ask::AccountSignOut { holder: people::AccountHolder::Own, source };
    assert_eq!(
        ask(&mut world, 41, sign_out),
        Some(&people::Reply::Outcome(people::Outcome::AccountSignedOut {
            confirmed: true,
            failed_hosts: Box::new([])
        }))
    );
    assert!(world.domain.accounts().source(source).is_none());
    assert!(world.domain.accounts().account(account).is_none());
    assert!(!world.store.rows.contains_key(&row(accounts::Key::Account { number: account })));
    assert!(!world.store.rows.contains_key(&row(accounts::Key::Source { account, number: source })));
}

const CONFIGURED_SOURCE: u32 = 1;
const OWN_WATCH: u64 = 61_001;
const OTHER_WATCH: u64 = 61_002;
const DEPLOYMENT_WATCH: u64 = 61_003;

fn watch(world: &mut World, sign_in: u64, watcher: u64, subject: core::documents::Watched) {
    world.send(root::Event::Core(core::Event::Watch {
        watcher: Token::new(watcher),
        sign_in,
        subject,
        window: core::documents::Window { rows: 8, after: None },
    }));
}
fn take(world: &mut World, watcher: u64) {
    world.send(root::Event::Core(core::Event::Delivered { watcher: Token::new(watcher) }));
}
fn delivered(world: &World, watcher: u64) -> &core::documents::Document {
    world
        .client_peer
        .as_ref()
        .expect("client peer")
        .delivered
        .iter()
        .rev()
        .find_map(|(_, delivery)| {
            let root::Delivery::Core(core::Held::Delivery { watcher: name, document }) = delivery else { return None };
            if name.raw() == watcher { Some(document.as_ref()) } else { None }
        })
        .expect("the watch delivered")
}
fn accounts_document(world: &World, watcher: u64) -> &core::documents::Accounts {
    let core::documents::Document::Accounts(document) = delivered(world, watcher) else { panic!("accounts document") };
    document
}
fn start_account(world: &mut World) -> u32 {
    let sign_in = world.sign_in.expect("signed in");
    ask_as(world, sign_in, 42, people::Ask::AccountSignIn { holder: people::AccountHolder::Own, provider: PROVIDER });
    // The independent component answers within send; the first watch delivery remains in flight.
    world
        .domain
        .accounts()
        .account_views()
        .iter()
        .find_map(|fact| {
            let account = world.domain.accounts().account(fact.account).expect("account view");
            matches!(account.holder(), accounts::Holder::Party(_)).then_some(fact.source)
        })
        .expect("the fake component made the party's source")
}

#[test]
fn a_partys_accounts_watch_follows_its_sign_in_from_start_to_end() {
    let mut world = signing(156);
    world.send(root::Event::Core(core::Event::Account(accounts::Event::Component(accounts::FromComponent::Failed {
        account: CONFIGURED_SOURCE,
        failure: accounts::Failure::SignInAgain,
    }))));
    let first = world.sign_in.expect("first sign-in");
    let second = jig_core_world::fixtures::second_party(&mut world, b"accounts-observer", people::Role::Observer);
    world.sign_in = Some(first);
    world.client_peer = Some(jig_core_world::client_peer::ClientPeer::default());
    watch(&mut world, first, OWN_WATCH, core::documents::Watched::Accounts);
    watch(&mut world, second, OTHER_WATCH, core::documents::Watched::Accounts);
    let empty = accounts_document(&world, OWN_WATCH);
    assert!(empty.accounts.is_empty());
    assert!(empty.settled);
    assert!(!empty.usable);
    take(&mut world, OWN_WATCH);
    take(&mut world, OTHER_WATCH);
    let source = start_account(&mut world);
    let underway = accounts_document(&world, OWN_WATCH);
    assert_eq!(underway.signing.len(), 1);
    let pending = underway.signing.get(0).expect("the party's sign-in is under way");
    assert_eq!((pending.source, pending.provider), (source, PROVIDER));
    assert!(!underway.settled);
    take(&mut world, OWN_WATCH);
    let finished = accounts_document(&world, OWN_WATCH);
    let account = finished.accounts.get(0).expect("own account");
    let first_source = account.sources.get(0).expect("the account's first source");
    assert_eq!((first_source.number, first_source.place), (source, 0));
    assert!(finished.signing.is_empty());
    assert!(finished.settled);
    assert_eq!(
        finished.last_end,
        Some(core::documents::LastEnd {
            source,
            end: accounts::End::Made { account: account.number, declined: false }
        })
    );
    assert!(accounts_document(&world, OTHER_WATCH).accounts.is_empty());
    assert!(accounts_document(&world, OTHER_WATCH).signing.is_empty());
    assert_eq!(accounts_document(&world, OTHER_WATCH).last_end, None);
    assert!(world.client_peer.as_ref().expect("client peer").delivered.iter().all(|(_, delivery)| {
        let root::Delivery::Core(core::Held::Delivery { watcher, document }) = delivery else { return true };
        watcher.raw() != OTHER_WATCH || matches!(document.as_ref(), core::documents::Document::Accounts(doc) if doc.accounts.is_empty() && doc.signing.is_empty() && doc.last_end.is_none())
    }));
}

#[test]
fn the_deployment_watch_shows_the_deployments_accounts_and_not_a_partys_own() {
    let mut world = signing(157);
    let sign_in = world.sign_in.expect("signed in");
    let person = world
        .store
        .rows
        .values()
        .find_map(|record| {
            let root::Record::Core(core::Record::People(people::Stored::SignIn { number, person, .. })) = record else {
                return None;
            };
            if *number == sign_in { Some(*person) } else { None }
        })
        .expect("the store kept the live sign-in");
    world.send(root::Event::Core(core::Event::People(people::Event::Roles {
        project: 0,
        holdings: Box::new([people::Holding { person, role: people::Role::Owner }]),
    })));
    world.client_peer = Some(jig_core_world::client_peer::ClientPeer::default());
    watch(&mut world, sign_in, OWN_WATCH, core::documents::Watched::Accounts);
    watch(&mut world, sign_in, DEPLOYMENT_WATCH, core::documents::Watched::Deployment);
    take(&mut world, OWN_WATCH);
    take(&mut world, DEPLOYMENT_WATCH);
    let source = start_account(&mut world);
    take(&mut world, OWN_WATCH);
    let own = accounts_document(&world, OWN_WATCH).accounts.get(0).expect("own account").number;
    take(&mut world, DEPLOYMENT_WATCH);
    let core::documents::Document::Deployment(deployment) = delivered(&world, DEPLOYMENT_WATCH) else {
        panic!("deployment document")
    };
    assert_eq!(deployment.accounts.len(), 1);
    assert_eq!(deployment.accounts.get(0).expect("the restored deployment account").number, 0);
    assert!(!deployment.accounts.iter().any(|account| account.number == own));
    assert!(deployment.accounts.iter().all(|account| account.holder == accounts::Holder::Deployment));
    take(&mut world, OWN_WATCH);
    ask_as(&mut world, sign_in, 43, people::Ask::AccountSignOut { holder: people::AccountHolder::Own, source });
    assert!(accounts_document(&world, OWN_WATCH).accounts.is_empty());
}

#[test]
fn a_sources_form_reaches_the_fleet_as_the_component_named_it() {
    const SECOND_ANALYST: u32 = 2;
    let mut world = jig_core_world::fixtures::waiting_on_source(166, SECOND_ANALYST);
    world.announce_grant(SECOND_ANALYST, 1, accounts::Form::Credential(accounts::Scheme::Key));
    assert_eq!(
        world.assigned.as_ref().expect("the fleet started the run").grants[0].form,
        accounts::Form::Credential(accounts::Scheme::Key),
    );
    hello(&mut world);
    assert_eq!(received(&world).1.form, fleet::Form::Credential(fleet::Scheme::Key));
}

fn party(world: &World, sign_in: u64) -> u64 {
    world
        .store
        .rows
        .values()
        .find_map(|record| {
            if let root::Record::Core(core::Record::People(people::Stored::SignIn { number, person, .. })) = record
                && *number == sign_in
            {
                Some(*person)
            } else {
                None
            }
        })
        .expect("a signed-in party")
}

#[test]
fn a_plans_uses_naming_another_person_are_refused_naming_the_providers_rule() {
    let mut world = signing(167);
    let first = world.sign_in.expect("signed in");
    let second = jig_core_world::fixtures::second_party(&mut world, b"account-payer", people::Role::Member);
    let other = party(&world, second);
    world.sign_in = Some(first);
    let (account, _) = signed_in(&mut world);
    let answer = ask(
        &mut world,
        41,
        people::Ask::AccountChange {
            holder: people::AccountHolder::Own,
            change: people::AccountChange::SetUses { account, uses: Some(Box::new([people::Use::Party(other)])) },
        },
    );
    assert_eq!(answer, Some(&people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Provider))));
}

fn llm_key(digest: u8) -> people::Ask {
    people::Ask::AccountChange {
        holder: people::AccountHolder::Own,
        change: people::AccountChange::AddKey { provider: PROVIDER, join: None, digest: [digest; 32] },
    }
}
fn carry(world: &mut World, key: u8, ask: people::Ask, number: u64, digest: u8) {
    world.send(root::Event::Core(core::Event::Key {
        reply_to: skein_lib::ReplyTo::new(Token::new(u64::from(key))),
        sign_in: world.sign_in.expect("signed in"),
        key: [key; 16],
        ask: Box::new(ask),
        carried: core::Carried { number, digest: [digest; 32] },
    }));
}
fn unused_before_answer(world: &World, before: usize, number: u64) {
    assert_eq!(world.carried_unused.last(), Some(&(before + world.application_answers.len(), number)));
    assert_eq!(world.people_answers.len(), before + 1);
}
#[test]
fn a_key_ask_routes_to_the_child_and_is_answered() {
    let mut world = signing(168);
    carry(&mut world, 40, llm_key(7), 1_u64 << 63, 7);
    let Some(people::Reply::Outcome(people::Outcome::AccountKeyKept { source })) = world.people_answers.last().cloned()
    else {
        panic!("LLM key kept")
    };
    assert_eq!(world.domain.accounts().source(source).expect("kept LLM key").generation, Some(1));
    let before = world.people_answers.len();
    carry(
        &mut world,
        41,
        people::Ask::AccountChange {
            holder: people::AccountHolder::Own,
            change: people::AccountChange::ReplaceKey { source, digest: [8; 32] },
        },
        (1_u64 << 63) + 1,
        8,
    );
    assert_eq!(world.people_answers.len(), before + 1);
    assert_eq!(world.people_answers.last(), Some(&people::Reply::Outcome(people::Outcome::AccountKeyKept { source })));
    assert_eq!(world.domain.accounts().source(source).expect("replaced LLM key").generation, Some(2));
    assert!(world.carried_unused.is_empty());
}
#[test]
fn a_refused_key_ask_returns_its_carried_value_unused() {
    let mut world = signing(169);
    for (at, failure, expected) in [
        (0, accounts::KeyFailure::NotStored, people::Outcome::Again(people::Again::NotReady)),
        (
            1,
            accounts::KeyFailure::TooLong,
            people::Outcome::Refused(people::Refusal::fixed(people::LimitKind::Key, 256, people::LimitUnit::Bytes)),
        ),
    ] {
        world.fail_llm_key_keep = Some(failure);
        let before = world.people_answers.len();
        carry(&mut world, 40 + at, llm_key(7), (1_u64 << 63) + u64::from(at), 7);
        unused_before_answer(&world, before, (1_u64 << 63) + u64::from(at));
        let reply = if let people::Outcome::Again(why) = expected {
            people::Reply::Again(why)
        } else {
            people::Reply::Outcome(expected)
        };
        assert_eq!(world.people_answers.last(), Some(&reply));
    }
    let before = world.people_answers.len();
    carry(
        &mut world,
        42,
        people::Ask::AccountChange {
            holder: people::AccountHolder::Own,
            change: people::AccountChange::AddKey { provider: 99, join: None, digest: [7; 32] },
        },
        (1_u64 << 63) + 2,
        7,
    );
    unused_before_answer(&world, before, (1_u64 << 63) + 2);
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Refused(people::Refusal::NotOffered)))
    );
}
#[test]
fn a_key_ask_people_does_not_admit_returns_it_unused() {
    let mut world = signing(170);
    let first = world.sign_in.expect("signed in");
    carry(&mut world, 40, llm_key(7), 1_u64 << 63, 7);
    let expected = world.people_answers.last().cloned();
    for (digest, expected) in [(7, expected), (8, Some(people::Reply::Refused(people::Refusal::KeyConflict)))] {
        let before = world.people_answers.len();
        let number = (1_u64 << 63) + u64::from(digest);
        carry(&mut world, 40, llm_key(digest), number, digest);
        unused_before_answer(&world, before, number);
        assert_eq!(world.people_answers.last().cloned(), expected);
    }
    world.sign_in = Some(u64::MAX);
    let before = world.people_answers.len();
    carry(&mut world, 41, llm_key(7), (1_u64 << 63) + 9, 7);
    unused_before_answer(&world, before, (1_u64 << 63) + 9);
    assert_eq!(world.people_answers.last(), Some(&people::Reply::SignedOut));
    world.sign_in = Some(first);
    jig_core_world::fixtures::second_party(&mut world, b"not-owner", people::Role::Observer);
    let before = world.people_answers.len();
    carry(
        &mut world,
        42,
        people::Ask::AccountChange {
            holder: people::AccountHolder::Deployment,
            change: people::AccountChange::AddKey { provider: PROVIDER, join: None, digest: [7; 32] },
        },
        (1_u64 << 63) + 10,
        7,
    );
    unused_before_answer(&world, before, (1_u64 << 63) + 10);
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Role { held: None })))
    );
}
#[test]
fn an_llm_key_ask_with_the_wrong_envelope_is_not_offered() {
    let mut world = signing(171);
    assert_eq!(
        ask(&mut world, 40, llm_key(7)),
        Some(&people::Reply::Outcome(people::Outcome::Refused(people::Refusal::NotOffered)))
    );
    assert_eq!(
        ask(
            &mut world,
            41,
            people::Ask::AccountChange {
                holder: people::AccountHolder::Own,
                change: people::AccountChange::ReplaceKey { source: 99, digest: [7; 32] }
            }
        ),
        Some(&people::Reply::Outcome(people::Outcome::Refused(people::Refusal::NotOffered)))
    );
    for (key, ask, digest) in [
        (42, people::Ask::AccountSignIn { holder: people::AccountHolder::Own, provider: PROVIDER }, 7),
        (43, llm_key(7), 8),
    ] {
        let before = world.people_answers.len();
        carry(&mut world, key, ask, (1_u64 << 63) + u64::from(key), digest);
        unused_before_answer(&world, before, (1_u64 << 63) + u64::from(key));
        assert_eq!(world.people_answers.last(), Some(&people::Reply::Refused(people::Refusal::NotOffered)));
    }
}
#[test]
fn a_keys_uses_may_name_any_known_party_or_project() {
    let mut world = signing(172);
    let first = world.sign_in.expect("signed in");
    let second = jig_core_world::fixtures::second_party(&mut world, b"llm-key-payer", people::Role::Member);
    let other = party(&world, second);
    world.sign_in = Some(first);
    carry(&mut world, 40, llm_key(7), 1_u64 << 63, 7);
    let Some(people::Reply::Outcome(people::Outcome::AccountKeyKept { source })) = world.people_answers.last() else {
        panic!("LLM key kept")
    };
    let account = world.domain.accounts().source(*source).expect("LLM key source").account;
    assert_eq!(
        ask(
            &mut world,
            41,
            people::Ask::AccountChange {
                holder: people::AccountHolder::Own,
                change: people::AccountChange::SetUses {
                    account,
                    uses: Some(Box::new([people::Use::Party(other), people::Use::Project(1)]))
                }
            }
        ),
        Some(&people::Reply::Outcome(people::Outcome::AccountChanged))
    );
    assert_eq!(
        world.domain.accounts().account(account).expect("LLM key account").uses(),
        Some([accounts::Use::Party(other), accounts::Use::Project(1)].as_slice())
    );
}

#[test]
fn a_carried_llm_key_of_another_holders_shape_is_not_offered() {
    let mut world = signing(173);
    carry(&mut world, 40, llm_key(7), 1_u64 << 63, 7);
    let Some(people::Reply::Outcome(people::Outcome::AccountKeyKept { source })) = world.people_answers.last().cloned()
    else {
        panic!("LLM key kept")
    };
    let account = world.domain.accounts().source(source).expect("LLM key source").account;
    jig_core_world::fixtures::second_party(&mut world, b"another-holder", people::Role::Member);
    for (at, change) in [
        people::AccountChange::ReplaceKey { source, digest: [7; 32] },
        people::AccountChange::AddKey { provider: PROVIDER, join: Some(account), digest: [7; 32] },
    ]
    .into_iter()
    .enumerate()
    {
        let before = world.people_answers.len();
        let number = (1_u64 << 63) + u64::try_from(at).expect("two account changes") + 1;
        carry(
            &mut world,
            41 + u8::try_from(at).expect("two account changes"),
            people::Ask::AccountChange { holder: people::AccountHolder::Own, change },
            number,
            7,
        );
        unused_before_answer(&world, before, number);
        assert_eq!(
            world.people_answers.last(),
            Some(&people::Reply::Outcome(people::Outcome::Refused(people::Refusal::NotOffered)))
        );
    }
}
#[test]
fn a_carried_llm_key_is_returned_unused_while_stopping() {
    let mut world = signing(174);
    world.send(root::Event::Core(core::Event::Stopping));
    let before = world.people_answers.len();
    carry(&mut world, 40, llm_key(7), 1_u64 << 63, 7);
    unused_before_answer(&world, before, 1_u64 << 63);
    assert_eq!(world.people_answers.last(), Some(&people::Reply::Again(people::Again::NotReady)));
}

fn own_key_run(seed: u64) -> (World, u32, u32) {
    let (config, limits) = signing_configuration(seed);
    let initial = jig_core_world::fixtures::create_chat(
        &jig_core_world::fixtures::Creating::new(&config.core, limits.core.tasks.tree_tasks),
        1,
        0,
        b"make an effect".as_slice().into(),
    );
    let mut world = World::configured_with_rows(seed, false, config, limits, initial, Box::new([]));
    carry(&mut world, 40, llm_key(7), 1_u64 << 63, 7);
    let Some(people::Reply::Outcome(people::Outcome::AccountKeyKept { source })) = world.people_answers.last().cloned()
    else {
        panic!("own LLM key")
    };
    let account = world.domain.accounts().source(source).expect("own LLM key source").account;
    (world, account, source)
}

#[test]
fn a_source_switch_keeps_the_account_and_committed_conversation() {
    let (mut world, account, first) = own_key_run(180);
    let assignment = world.assigned.as_ref().expect("own LLM key assigns the task");
    let task = assignment.task;
    let conversation = assignment.conversation;
    assert_eq!(
        assignment.run.accounts.as_ref(),
        [jig_domain_authority::AccountBinding { provider: PROVIDER, account, source: first }]
    );
    world.turn(1, 1, None, b"committed plain and opaque conversation");
    world.end(jig_domain_tasks::End::Parked, 1);
    carry(
        &mut world,
        41,
        people::Ask::AccountChange {
            holder: people::AccountHolder::Own,
            change: people::AccountChange::AddKey { provider: PROVIDER, join: Some(account), digest: [8; 32] },
        },
        (1_u64 << 63) + 1,
        8,
    );
    let Some(people::Reply::Outcome(people::Outcome::AccountKeyKept { source: second })) =
        world.people_answers.last().cloned()
    else {
        panic!("joined source")
    };
    ask(
        &mut world,
        42,
        people::Ask::AccountChange {
            holder: people::AccountHolder::Own,
            change: people::AccountChange::MoveSource { account, source: second, place: 0 },
        },
    );
    world.say(91);
    let assignment = world.assigned.as_ref().expect("own LLM key assigns the task");
    assert_eq!(assignment.run.sources.as_ref(), [Some(second)]);
    assert_eq!(
        assignment.run.accounts.as_ref(),
        [jig_domain_authority::AccountBinding { provider: PROVIDER, account, source: second }]
    );
    assert_eq!(assignment.conversation, conversation);
    assert_eq!(
        assignment.transcript.as_ref(),
        [Box::<[u8]>::from(b"committed plain and opaque conversation".as_slice())]
    );
    let root::Record::Core(core::Record::Tasks(jig_domain_tasks::Stored::Live(row))) =
        &world.store.rows[&root::Key::Core(core::Key::Tasks(jig_domain_tasks::Key::Live(task)))]
    else {
        panic!("task pin")
    };
    assert_eq!(row.kept_account, Some(account));
    let root::Record::Core(core::Record::Core(core::CoreRecord::RunProof(proof))) =
        &world.store.rows[&root::Key::Core(core::Key::Core(core::CoreKey::RunProof(task)))]
    else {
        panic!("claim bindings")
    };
    assert_eq!(proof.accounts, assignment.run.accounts);
    assert!(claim_keeps_pin_and_bindings(&world, task, assignment.attempt, account, second));
}

#[test]
fn a_terminal_source_limit_waits_without_a_try_until_resume() {
    let (mut world, _, source) = own_key_run(181);
    let (task, attempt) = {
        let assigned = world.assigned.as_ref().expect("own LLM key assigns the task");
        (assigned.task, assigned.attempt)
    };
    let count = world.assignments.len();
    world.send(root::Event::Core(core::Event::Fleet(fleet::Event::Limited {
        channel: Token::new(7),
        run: Token::new(task),
        attempt: Token::new(attempt),
        account: source,
        reset: None,
    })));
    world.end(jig_domain_tasks::End::Failed(jig_domain_tasks::Class::Run), 1);
    assert_eq!(world.assignments.len(), count);
    let row = jig_core_world::fixtures::live_row(&world, task);
    assert_eq!(row.tries, jig_domain_tasks::Tries::NONE);
    assert_eq!((row.run_reserved, row.numbers.spent), (0, 1));
    ask(
        &mut world,
        41,
        people::Ask::AccountChange {
            holder: people::AccountHolder::Own,
            change: people::AccountChange::Resume { source },
        },
    );
    assert_eq!(world.assignments.len(), count + 1);
}

fn claim_keeps_pin_and_bindings(world: &World, task: u64, attempt: u64, account: u32, source: u32) -> bool {
    world.commits.iter().any(|writes| {
        let pin = writes.iter().any(|write| matches!(write,
            root::Write::Save(root::Record::Core(core::Record::Tasks(jig_domain_tasks::Stored::Live(row))))
            if row.number == task && row.attempt == attempt && row.kept_account == Some(account)));
        let bindings = writes.iter().any(|write| matches!(write,
            root::Write::Save(root::Record::Core(core::Record::Core(core::CoreRecord::RunProof(proof))))
            if proof.task == task && proof.attempt == attempt
                && proof.accounts.as_ref() == [jig_domain_authority::AccountBinding { provider: PROVIDER, account, source }]));
        pin && bindings
    })
}

fn program_configuration(seed: u64) -> (root::Config, root::Limits) {
    let (mut config, limits) = jig_core_world::effects::fixture(seed, false);
    config.core.settings.models[0].program = Some(b"claude_code".as_slice().into());
    config.core.defaults.charters[0].policy.model.program = Some(b"claude_code".as_slice().into());
    config.core.settings.llm_providers.providers[0].offers.programs = Box::new([b"claude_code".as_slice().into()]);
    (config, limits)
}
#[test]
fn an_adopted_program_run_is_sent_its_grant_again() {
    let (config, limits) = program_configuration(159);
    let initial = jig_core_world::fixtures::create_chat(
        &jig_core_world::fixtures::Creating::new(&config.core, limits.core.tasks.tree_tasks),
        1,
        0,
        b"make an effect".as_slice().into(),
    );
    let mut rows = jig_core_world::fixtures::account_rows(&config.core.settings.llm_providers).into_vec();
    for row in &mut rows {
        if let root::Record::Core(core::Record::Accounts(accounts::Stored::Account { holder, .. })) = row {
            *holder = accounts::Holder::Party(1);
        }
        if let root::Record::Core(core::Record::Accounts(accounts::Stored::Source { kind, name, .. })) = row {
            *kind = accounts::Kind::Program;
            *name = b"personal".as_slice().into();
        }
    }
    rows.push(root::Record::Core(core::Record::Accounts(accounts::Stored::Issued {
        number: 1,
        holder: accounts::Holder::Party(1),
        name: b"personal".as_slice().into(),
        signed_out: false,
    })));
    let mut world = World::configured_with_rows(159, false, config, limits, initial, rows.into_boxed_slice());
    let task = world.assigned.as_ref().expect("program run assigned").task;
    world.restart_with(program_configuration(159).0);
    hello(&mut world);
    let (received_task, grant) = received(&world);
    assert_eq!(
        (received_task, grant.form, grant.account, grant.generation),
        (Token::new(task), fleet::Form::Program(fleet::SourceName::new(b"personal").expect("source name")), 1, 2)
    );
    assert!(grant.valid > accounts::KEY_VALID);
}

#[test]
fn source_limit_wall_survives_a_corrected_accounts_watch_clock() {
    let (mut world, account, source) = own_key_run(182);
    let now = Time::from_nanos(1_000_000_000);
    world.set_clock(now, Wall::from_nanos(10_000_000_000));
    world.send(root::Event::Core(core::Event::Account(accounts::Event::Limited {
        account: source,
        reset: Some(Time::from_nanos(31_000_000_000)),
    })));
    world.set_clock(Time::from_nanos(2_000_000_000), Wall::from_nanos(20_000_000_000));
    world.client_peer = Some(jig_core_world::client_peer::ClientPeer::default());
    let sign_in = world.sign_in.expect("signed in");
    watch(&mut world, sign_in, OWN_WATCH, core::documents::Watched::Accounts);
    let doc = accounts_document(&world, OWN_WATCH);
    let line = doc.accounts.iter().find(|line| line.number == account).expect("own account");
    let line = line.sources.iter().find(|line| line.number == source).expect("limited source");
    let reset = Some(Wall::from_nanos(40_000_000_000));
    assert_eq!(line.state, core::documents::AccountState::Later { until: reset });
    assert_eq!(line.spent_until, reset);
    world.set_clock(Time::from_nanos(3_000_000_000), Wall::from_nanos(15_000_000_000));
    world.restart_with(signing_configuration(182).0);
    assert_eq!(
        world.domain.accounts().source(source).expect("restored own source").spent_until,
        Some(accounts::Deadline { at: Time::from_nanos(28_000_000_000), wall: Wall::from_nanos(40_000_000_000) })
    );
    world.set_clock(Time::from_nanos(4_000_000_000), Wall::from_nanos(5_000_000_000));
    world.client_peer = Some(jig_core_world::client_peer::ClientPeer::default());
    watch(&mut world, sign_in, OWN_WATCH, core::documents::Watched::Accounts);
    let doc = accounts_document(&world, OWN_WATCH);
    let line = doc.accounts.iter().find(|line| line.number == account).expect("restored own account");
    let line = line.sources.iter().find(|line| line.number == source).expect("restored own source");
    assert_eq!(line.state, core::documents::AccountState::Later { until: reset });
    assert_eq!(line.spent_until, reset);
}

#[test]
fn a_sign_in_is_finished_by_its_starters_pasted_address() {
    fn reply(world: &World) -> &people::Reply {
        world
            .client_peer
            .as_ref()
            .expect("client peer")
            .delivered
            .iter()
            .rev()
            .find_map(|(_, delivery)| {
                let root::Delivery::Core(core::Held::PeopleReply { reply, .. }) = delivery else { return None };
                Some(reply)
            })
            .expect("client reply")
    }
    let mut world = signing(210);
    world.wait_for_finish = true;
    world.finish_script.extend([Some(accounts::SignInRedirectWhy::State), None]);
    let first = world.sign_in.expect("first sign-in");
    let second = jig_core_world::fixtures::second_party(&mut world, b"paste-observer", people::Role::Observer);
    world.sign_in = Some(first);
    world.client_peer = Some(jig_core_world::client_peer::ClientPeer::default());
    watch(&mut world, first, OWN_WATCH, core::documents::Watched::Accounts);
    watch(&mut world, second, OTHER_WATCH, core::documents::Watched::Accounts);
    take(&mut world, OWN_WATCH);
    take(&mut world, OTHER_WATCH);
    ask(&mut world, 70, people::Ask::AccountSignIn { holder: people::AccountHolder::Own, provider: PROVIDER });
    let Some(people::Reply::Outcome(people::Outcome::AccountSignInStarted { source, .. })) =
        Some(reply(&world).clone())
    else {
        panic!("sign-in waiting")
    };
    assert_eq!(accounts_document(&world, OWN_WATCH).signing.get(0).expect("pending").source, source);
    take(&mut world, OWN_WATCH);
    for (key, sign_in, expected) in [
        (71, second, Some(people::SignInRedirectWhy::NotWaiting)),
        (72, first, Some(people::SignInRedirectWhy::State)),
        (73, first, None),
    ] {
        world.sign_in = Some(sign_in);
        let number = (3_u64 << 62) + u64::from(key);
        let before = world.client_peer.as_ref().expect("client peer").delivered.len();
        carry(
            &mut world,
            key,
            people::Ask::AccountChange {
                holder: people::AccountHolder::Own,
                change: people::AccountChange::FinishSignIn { source, digest: [key; 32] },
            },
            number,
            key,
        );
        let outcome = match expected {
            Some(why) => people::Outcome::Refused(people::Refusal::SignInRedirect { why }),
            None => people::Outcome::AccountFinishing { source },
        };
        assert_eq!(reply(&world), &people::Reply::Outcome(outcome));
        let deliveries = &world.client_peer.as_ref().expect("client peer").delivered;
        let unused = deliveries[before..]
            .iter()
            .position(|(_, delivery)| {
                matches!(delivery,
            root::Delivery::Core(core::Held::CarriedUnused { number: value }) if *value == number)
            })
            .expect("unused carried number");
        let answered = deliveries[before..]
            .iter()
            .position(|(_, delivery)| matches!(delivery, root::Delivery::Core(core::Held::PeopleReply { .. })))
            .expect("answered finish");
        assert!(unused < answered, "the clients component can erase before answering");
        assert_eq!(
            deliveries
                .iter()
                .filter(|(_, delivery)| matches!(delivery, root::Delivery::Core(core::Held::CarriedUnused { .. })))
                .count(),
            usize::from(key - 70)
        );
        if expected.is_some() {
            assert!(accounts_document(&world, OWN_WATCH).last_end.is_none());
            assert_eq!(accounts_document(&world, OWN_WATCH).signing.len(), 1);
        }
    }
    assert_eq!(world.finishes, [(source, (3_u64 << 62) + 72), (source, (3_u64 << 62) + 73)]);
    take(&mut world, OWN_WATCH);
    let own = accounts_document(&world, OWN_WATCH);
    assert!(own.signing.is_empty());
    assert!(
        matches!(own.last_end, Some(core::documents::LastEnd { source: ended, end: accounts::End::Made { .. } }) if ended == source)
    );
    assert_eq!(own.accounts.len(), 1);
    let other = accounts_document(&world, OTHER_WATCH);
    assert!(other.accounts.is_empty() && other.signing.is_empty() && other.last_end.is_none());
    assert!(world.client_peer.as_ref().expect("peer").delivered.iter().all(|(_, delivery)| {
        let root::Delivery::Core(core::Held::Delivery { watcher, document }) = delivery else { return true };
        watcher.raw() != OTHER_WATCH || matches!(document.as_ref(), core::documents::Document::Accounts(doc) if doc.accounts.is_empty() && doc.signing.is_empty() && doc.last_end.is_none())
    }));
}
