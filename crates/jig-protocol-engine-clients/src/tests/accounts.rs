//! Complete accounts watch translation and independent nested limits.
use super::fixtures;
use crate::translate::{accounts, watches};
use fixtures::list;
use jig_client_protocol_wire::v19 as w;
use jig_domain_accounts as a;
use jig_domain_core::documents as d;
use jig_domain_people::CapacityAction as A;
use skein_lib::{Duration, List, Wall};
fn source_lines() -> List<d::SourceLine> {
    list([
        d::SourceLine {
            name: None,
            place: 2,
            number: 11,
            kind: a::Kind::SignIn,
            state: d::AccountState::Granted,
            usable: true,
            expiring: false,
            attention: true,
            spent_until: Some(Wall::from_nanos(101)),
        },
        d::SourceLine {
            name: None,
            place: 5,
            number: 12,
            kind: a::Kind::Key,
            state: d::AccountState::Later { until: None },
            usable: false,
            expiring: true,
            attention: false,
            spent_until: None,
        },
    ])
}
#[expect(clippy::manual_map, reason = "optional fixture facts follow the step crate lint configuration")]
fn document(end: Option<a::End>) -> d::Accounts {
    d::Accounts {
        readiness: List::with_capacity(0),
        readiness_cut: false,
        program_hosts: List::with_capacity(0),
        accounts: list([
            d::AccountLine {
                provider_name: None,
                manage_usage: None,
                number: 7,
                place: 3,
                uses: Some(list([a::Use::Party(23), a::Use::Project(29)])),
                plan: true,
                provider: Some(3),
                display: d::Text { bytes: Box::from(b"Personal".as_slice()), cut: true },
                holder: a::Holder::Party(23),
                sources: source_lines(),
                cut: true,
            },
            d::AccountLine {
                provider_name: None,
                manage_usage: None,
                number: 9,
                place: 4,
                uses: None,
                plan: false,
                provider: None,
                display: d::Text { bytes: Box::from([]), cut: false },
                holder: a::Holder::Deployment,
                sources: List::with_capacity(0),
                cut: false,
            },
        ]),
        signing: list([d::SignInLine { source: 19, provider: 3, holder: a::Holder::Party(23) }]),
        last_end: match end {
            Some(end) => Some(d::LastEnd { source: 11, end }),
            None => None,
        },
        usable: true,
        settled: false,
        cut: true,
    }
}
fn ends() -> Box<[a::End]> {
    Box::new([
        a::End::Made { account: 7, declined: false },
        a::End::Renewed { account: 7, declined: true },
        a::End::Refused { refusal: a::Refusal::NotLimited },
        a::End::Refused { refusal: a::Refusal::Uses },
        a::End::Refused { refusal: a::Refusal::NotKept { why: a::KeyFailure::NotStored } },
        a::End::Refused { refusal: a::Refusal::Limit { bound: a::Bound::HolderAccounts, number: None } },
        a::End::Refused { refusal: a::Refusal::Held { provider: 3 } },
        a::End::Refused { refusal: a::Refusal::Provider { provider: 3 } },
        a::End::Refused { refusal: a::Refusal::Subject },
        a::End::Refused { refusal: a::Refusal::Unknown },
        a::End::Refused { refusal: a::Refusal::Numbers },
        a::End::Refused { refusal: a::Refusal::Kind { kind: a::Kind::Program } },
        a::End::Refused { refusal: a::Refusal::Closing },
        a::End::Refused { refusal: a::Refusal::NotStarted },
        a::End::Refused { refusal: a::Refusal::NotSignedIn },
        a::End::Refused { refusal: a::Refusal::NotOurs },
        a::End::Refused { refusal: a::Refusal::Cancelled },
        a::End::Refused { refusal: a::Refusal::Moving },
        a::End::Failed { failure: a::Failure::Later { after: Some(Duration::from_nanos(103)) } },
        a::End::Failed { failure: a::Failure::Later { after: None } },
        a::End::Failed { failure: a::Failure::SignInAgain },
        a::End::Failed { failure: a::Failure::MissingScope { scope: 31 } },
        a::End::Failed { failure: a::Failure::Misconfigured },
        a::End::Failed { failure: a::Failure::Unloaded { at: a::Place::File, why: a::Unread::Stalled } },
        a::End::Failed { failure: a::Failure::Cancelled },
    ])
}
#[test]
fn an_accounts_watch_binds_its_party_at_core_admission() {
    let Some(jig_domain_core::Event::Watch { watcher, sign_in, subject, window }) =
        watches::watch(skein_lib::Token::new(17), 23, w::Subject::Accounts, 2)
    else {
        panic!("accounts watch event");
    };
    assert_eq!(watcher, skein_lib::Token::new(17));
    assert_eq!(sign_in, 23);
    assert_eq!(subject, d::Watched::Accounts);
    assert_eq!(window, d::Window { rows: 2, after: None });
}
#[test]
fn accounts_deliver_whole_with_every_terminal_outcome() {
    let limits = jig_client_test_records::limits();
    for (end, expected) in ends().into_iter().zip(jig_client_test_records::account_end_records()) {
        let source = d::Document::Accounts(Box::new(document(Some(end))));
        assert_eq!(
            watches::delivery(&source, 0, 2, &[], &limits),
            Ok(w::DeliveryValue::OwnerAccounts(jig_client_test_records::accounts_record(Some(expected))))
        );
    }
    assert_eq!(accounts::document(&document(None), &limits), Ok(jig_client_test_records::accounts_record(None)));
}
#[test]
fn each_accounts_piece_ceiling_is_refused_at_its_own_path() {
    let source = document(None);
    let limits = jig_client_test_records::limits();
    let rows = [
        (w::Limits { owner_accounts_accounts: 0, ..limits }, w::Path::OwnerAccountsAccounts),
        (w::Limits { owner_accounts_signing: 0, ..limits }, w::Path::OwnerAccountsSigning),
        (w::Limits { owner_account_line_sources: 0, ..limits }, w::Path::OwnerAccountLineSources),
        (w::Limits { owner_account_line_uses_some: 0, ..limits }, w::Path::OwnerAccountLineUses),
    ];
    for (limits, path) in rows {
        assert_eq!(accounts::document(&source, &limits), Err(w::Problem { path, reason: skein_codec::Reason::Bound }));
    }
}
#[test]
fn deployment_accounts_keep_identity_uses_and_ordered_sources() {
    let mut source = fixtures::deployment_document();
    let account = source.accounts.get_mut(0).expect("fixture account");
    account.place = 3;
    account.uses = Some(list([a::Use::Party(23), a::Use::Project(29)]));
    account.plan = true;
    account.display = d::Text { bytes: Box::from(b"Personal".as_slice()), cut: true };
    account.holder = a::Holder::Party(23);
    account.sources = source_lines();
    let limits = jig_client_test_records::limits();
    let encoded = crate::translate::deployment::account(account, &limits).unwrap();
    let line = jig_client_test_records::account_line_record();
    assert_eq!(encoded.place(), line.place());
    assert_eq!(encoded.uses(), line.uses());
    assert!(encoded.plan());
    assert_eq!(encoded.provider(), line.provider());
    assert_eq!(encoded.display(), line.display());
    assert_eq!(encoded.holder(), line.holder());
    assert_eq!(encoded.sources(), line.sources());
    let bound = w::Limits { owner_account_sources: 0, ..limits };
    assert_eq!(
        crate::translate::deployment::account(account, &bound),
        Err(w::Problem { path: w::Path::OwnerAccountSources, reason: skein_codec::Reason::Bound })
    );
}

fn deployment_terms() -> d::Account {
    let mut source = fixtures::deployment_document();
    let account = source.accounts.get_mut(0).unwrap();
    account.provider_name = Some(d::Text { bytes: Box::from(b"Arbitrary provider".as_slice()), cut: true });
    account.manage_usage = Some(Box::from(b"https://usage.example/".as_slice()));
    account.clone()
}
#[test]
fn deployment_terms_reach_the_wire() {
    let limits = jig_client_test_records::limits();
    let encoded = crate::translate::deployment::account(&deployment_terms(), &limits).unwrap();
    assert_eq!(encoded.provider_name().as_ref().unwrap().bytes(), b"Arbitrary provider");
    assert!(encoded.provider_name().as_ref().unwrap().cut());
    assert_eq!(encoded.manage_usage().as_deref(), Some(b"https://usage.example/".as_slice()));
}
#[test]
fn account_line_terms_reach_the_wire() {
    let limits = jig_client_test_records::limits();
    let account = deployment_terms();
    let mut account_line = document(None).accounts.get(0).unwrap().clone();
    account_line.provider_name = account.provider_name;
    account_line.manage_usage = account.manage_usage;
    let line = accounts::line(&account_line, &limits).unwrap();
    assert_eq!(line.provider_name().as_ref().unwrap().bytes(), b"Arbitrary provider");
    assert!(line.provider_name().as_ref().unwrap().cut());
    assert_eq!(line.manage_usage().as_deref(), Some(b"https://usage.example/".as_slice()));
}
#[test]
fn account_capacity_actions_reach_the_wire() {
    let limits = jig_client_test_records::limits();
    let source = fixtures::deployment_document();
    for (action, expected) in [
        (A::AccountSignIn, w::OwnerPersonCapacityAction::AccountSignIn),
        (A::AccountSignOut, w::OwnerPersonCapacityAction::AccountSignOut),
        (A::AccountChange, w::OwnerPersonCapacityAction::AccountChange),
    ] {
        let mut refusal = *source.person_room_refusals.get(0).unwrap();
        refusal.action = action;
        assert_eq!(crate::translate::deployment::person_refusal(&refusal, &limits).unwrap().action(), &expected);
    }
}
