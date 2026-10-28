//! LLM account sign-in through both real client protocol boundaries.
use jig_client::{Answered, Ask, Asker, Key, Outcome, contract as c};
use jig_client_protocol_world::{client::Story, fixtures};
use skein_lib::Duration;

const FIRST: Key = Key { asker: Asker(7), number: 1 };
const SECOND: Key = Key { asker: Asker(7), number: 2 };
const WAIT: Duration = Duration::from_secs(1);
fn ask() -> Ask {
    Ask::Jig(Box::new(c::Ask::SignIn { holder: c::AccountHolder::Own, provider: 3 }))
}
fn answered(story: &mut Story, key: Key) -> Vec<Outcome> {
    assert!(story.run_until(|s| !s.answered(key).is_empty(), WAIT));
    story.answered(key)
}
fn started(answers: &[Outcome]) -> (u32, &[u8]) {
    let [
        Outcome::Answered(Answered::Jig(c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::AccountSignInStarted {
            source,
            address,
        })))),
    ] = answers
    else {
        panic!("one account sign-in started answer: {answers:?}")
    };
    (*source, address)
}
#[test]
fn a_sign_in_asked_through_a_client_is_answered_with_the_address() {
    let mut story = Story::account_sign_in(fixtures::SEED, true);
    story.send(FIRST, ask());
    let answers = answered(&mut story, FIRST);
    let (source, address) = started(&answers);
    assert_eq!(address, format!("https://issuer.test/visit/{source}").as_bytes());
}
#[test]
fn a_sign_in_for_a_provider_with_none_is_not_offered() {
    let mut story = Story::account_sign_in(fixtures::SEED, false);
    story.send(FIRST, ask());
    assert_eq!(
        answered(&mut story, FIRST),
        vec![Outcome::Answered(Answered::Jig(c::Answer::Refused { refusal: c::Refusal::NotOffered, at: None }))]
    );
}
#[test]
fn a_sign_in_beyond_the_sign_ins_bound_is_refused_naming_it() {
    // Another provider's, so that the second sign-in does not replace the first.
    let mut story = Story::account_sign_in_two_providers(fixtures::SEED);
    story.hold_account_sign_in_burst();
    story.send(FIRST, ask());
    story.send(SECOND, Ask::Jig(Box::new(c::Ask::SignIn { holder: c::AccountHolder::Own, provider: 4 })));
    assert!(story.run_until(|s| s.account_sign_in_burst_len() == 2, WAIT));
    story.release_account_sign_in_burst();
    assert_eq!(
        answered(&mut story, SECOND),
        vec![Outcome::Answered(Answered::Jig(c::Answer::Refused {
            refusal: c::Refusal::Limit {
                which: c::Limit::Admission {
                    which: c::AdmissionLimit::SignIns,
                    bound: Some(c::LimitBound::configured(1, c::LimitKey::AccountsSignIns, c::LimitUnit::Items))
                }
            },
            at: None,
        }))]
    );
    started(&answered(&mut story, FIRST));
}
#[test]
fn a_sign_out_is_answered_with_confirmed() {
    let mut story = Story::account_sign_in(fixtures::SEED, true);
    story.send(FIRST, ask());
    let (source, _) = started(&answered(&mut story, FIRST));
    story.send(SECOND, Ask::Jig(Box::new(c::Ask::SignOut { holder: c::AccountHolder::Own, source })));
    assert_eq!(
        answered(&mut story, SECOND),
        vec![Outcome::Answered(Answered::Jig(c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::AccountSignedOut {
            confirmed: true,
            failed_hosts: Box::new([])
        }))))]
    );
}
#[test]
fn a_sign_in_sent_again_after_its_answer_gets_the_same_answer() {
    let mut story = Story::account_sign_in(fixtures::SEED, true);
    story.send_named(FIRST, [37; 16], ask());
    let first = answered(&mut story, FIRST);
    started(&first);
    story.send_named(SECOND, [37; 16], ask());
    assert_eq!(answered(&mut story, SECOND), first);
}

const ACCOUNTS: jig_client::Handle = jig_client::Handle { asker: Asker(7), number: 1 };
fn accounts(story: &Story) -> Option<&c::OwnerAccounts> {
    let jig_client::Latest::Jig(c::Delivery::OwnerAccounts(value)) = story.client.document(ACCOUNTS)?.latest? else {
        panic!("an accounts document");
    };
    Some(value)
}
#[test]
fn an_accounts_watch_delivers_the_partys_completed_sign_in() {
    let mut story = Story::account_sign_in(fixtures::SEED, true);
    story.watch(ACCOUNTS, jig_client::Watchable::Jig(c::Subject::Accounts));
    assert!(story.run_until(|s| accounts(s).is_some(), WAIT));
    let initial = accounts(&story).expect("initial accounts");
    assert!(initial.accounts.is_empty());
    assert!(initial.signing.is_empty());
    assert!(initial.settled);
    story.send(FIRST, ask());
    let (source, _) = started(&answered(&mut story, FIRST));
    assert!(story.run_until(|s| accounts(s).is_some_and(|d| d.settled && d.last_end.is_some()), WAIT));
    story.grant_account_sign_in(source);
    assert!(story.run_until(
        |s| accounts(s).is_some_and(|d| d.usable
            && d.settled
            && d.last_end.is_some()
            && d.accounts.iter().any(|a| a.sources.iter().any(|s| s.state == c::OwnerAccountState::Granted))),
        WAIT
    ));
    let document = accounts(&story).expect("completed sign-in accounts");
    assert!(document.usable);
    assert!(document.settled);
    assert!(document.signing.is_empty());
    let [account] = document.accounts.as_slice() else { panic!("one signed-in account: {document:?}") };
    assert_eq!(account.provider, Some(3));
    assert_eq!(account.holder, c::OwnerAccountHolder::Party(fixtures::FIRST_OWNER));
    let [grant] = account.sources.as_slice() else { panic!("one sign-in source: {account:?}") };
    assert_eq!(grant.number, source);
    assert_eq!(grant.kind, c::OwnerAccountKind::SignIn);
    assert_eq!(grant.state, c::OwnerAccountState::Granted);
    assert!(grant.usable);
    assert_eq!(
        document.last_end,
        Some(c::OwnerAccountLastEnd {
            source,
            end: c::OwnerAccountEnd::Made { account: account.number, declined: false }
        })
    );
}

#[test]
fn an_ordinary_party_receives_the_public_catalogue() {
    let mut story = Story::ordinary_account_party(fixtures::SEED);
    let me = jig_client::Handle { asker: Asker(7), number: 2 };
    story.watch(me, jig_client::Watchable::Jig(c::Subject::Me));
    assert!(story.run_until(|s| s.client.document(me).is_some_and(|d| d.latest.is_some()), WAIT));
    let jig_client::Latest::Jig(c::Delivery::Me(value)) =
        story.client.document(me).expect("watched me").latest.expect("delivered me")
    else {
        panic!("me")
    };
    assert!(value.deployment_role.is_none());
    assert_eq!(value.key_bytes, 256);
    let [offer] = value.providers.as_slice() else { panic!("one public provider") };
    assert_eq!(offer.provider, 3);
    assert_eq!(offer.key_bytes, value.key_bytes);
    assert_eq!(offer.name.bytes.bytes(), b"Fake");
    assert!(offer.sign_in && offer.signed_in_plan);
    assert_eq!(offer.manage_usage.as_deref(), Some(b"https://usage.example/".as_slice()));
}
#[test]
fn an_account_limit_refreshes_payer_readiness() {
    let mut story = Story::account_sign_in(fixtures::SEED, true);
    story.watch(ACCOUNTS, jig_client::Watchable::Jig(c::Subject::Accounts));
    assert!(story.run_until(|s| accounts(s).is_some(), WAIT));
    assert_eq!(
        accounts(&story).expect("watched accounts").readiness.get(0).expect("own payer readiness").readiness,
        c::Readiness::Usable
    );
    story.limit_account_source(1);
    assert!(story.run_until(
        |s| accounts(s).is_some_and(|d| d.readiness.get(0).is_some_and(|r| r.readiness == c::Readiness::Unavailable)),
        WAIT
    ));
}
