//! The shipped service and agent, killed and restarted over one durable store.
mod support;
use jig_client_protocol_world::native::Script;

/// The fake provider checks the credential presented by the real run.
fn peer() -> skein_fake_peers::llm::Peer {
    let mut script = jig_channel_world::fixtures::one_chat(b"Hello");
    script.result = br#"{"form":"report","text":"Process answer","fields":{}}"#.as_slice().into();
    jig_channel_world::fixtures::provider(
        Box::new([skein_fake_llm_domain::api::Script {
            cue: Box::default(),
            turns: jig_channel_world::fixtures::provider_turns(&script),
        }]),
        skein_llm::Credential::bearer(b"token".as_slice().into()),
        skein_lib::Duration::ZERO,
        skein_fake_peers::Transport::Tls,
        skein_fake_llm_protocol::documents::Provider::OpenAi,
    )
}

#[test]
fn a_killed_jig_test_finds_its_work_after_restart() {
    let directory = support::scratch();
    support::configuration(directory.path());
    assert_eq!(support::run(directory.path(), Script::answered_chat(b"Hello", support::ANSWER), Some(peer())), (1, 1));
    assert_eq!(support::run(directory.path(), Script::found_chat(support::ANSWER), None), (1, 0));
}

#[test]
fn the_explicit_d2_preseed_reaches_the_shipped_driver_without_a_configured_account() {
    let directory = support::scratch();
    support::configuration(directory.path());
    support::d2::preseed(directory.path(), br#"{"access_token":"token","expires_at":4102444800}"#);
    assert_eq!(
        support::run_d2(directory.path(), Script::answered_chat(b"Hello", support::ANSWER), Some(peer())),
        (1, 1)
    );
}

#[test]
fn an_agent_trace_captures_content_without_the_grant_or_authorization_header() {
    let directory = support::scratch();
    support::configuration(directory.path());
    support::trace::configure(directory.path());
    assert_eq!(support::run(directory.path(), Script::answered_chat(b"Hello", support::ANSWER), Some(peer())), (1, 1));
    support::trace::check(directory.path(), Some(b"token"));
}

#[path = "support/browser.rs"]
mod browser;
#[test]
fn jig_test_serves_its_bundle_s_page_under_the_reference_policy() {
    let directory = support::scratch();
    browser::configuration(directory.path(), &browser::fixture_bundle(directory.path()));
    let mut binary = browser::start(directory.path());
    let link = browser::printed_link_line(&mut binary);
    let (status, headers, body) = browser::get(&link, "/tasks/7");
    assert_eq!(status, 200);
    assert_eq!(browser::field(&headers, "content-security-policy"), Some(browser::POLICY));
    assert_eq!(body, browser::PAGE);
}

#[test]
fn a_party_signs_in_through_a_client_and_opens_the_address_at_the_issuer() {
    let directory = support::scratch();
    accounts::configuration(directory.path());
    accounts::run(directory.path());
}

#[test]
fn sigterm_ends_without_a_pending_account_sign_in() {
    accounts::sigterm(false, None);
}

#[test]
fn sigterm_ends_with_an_unvisited_account_sign_in() {
    accounts::sigterm(true, None);
}

/// Vary the deadline itself to distinguish a sign-in wait from an unbounded hang.
#[test]
fn sigterm_with_a_pending_sign_in_reaches_its_deadline() {
    const SIGN_IN_DEADLINE_MS: u64 = 2000;
    accounts::sigterm(true, Some(SIGN_IN_DEADLINE_MS));
}

// Reuse the account sign-in world's browser source; it remains a private test helper.
use jig_protocol_accounts_world::fixtures;
#[path = "../../../tests/protocol-accounts/src/visitor.rs"]
mod account_sign_in_visitor;

#[path = "support/accounts.rs"]
mod accounts;

#[path = "support/account_keys.rs"]
mod account_keys;
#[test]
fn a_party_adds_replaces_and_removes_a_key() {
    let directory = support::scratch();
    account_keys::configuration(directory.path());
    account_keys::run(directory.path());
}

#[expect(dead_code, reason = "only the live binary uses the DNS-resolving configuration entry")]
#[path = "support/live_sign_in.rs"]
mod live_sign_in;

/// The live sign-in story on fake peers: a person's plan signed in, used for an answer and signed out.
#[test]
fn a_person_signs_in_at_the_fake_issuer_and_the_live_story_answers_and_signs_out() {
    let directory = support::scratch();
    let peers = live_sign_in::Peers::Fake;
    let clock = skein_shell::Clock::new();
    let started = clock.now().now;
    let limits = live_sign_in::configuration_at(directory.path(), "127.0.0.1:34443", peers);
    let outcome = live_sign_in::run(directory.path(), started, peers);
    live_sign_in::check(directory.path(), &limits, outcome, peers);
}

/// A provider that refuses the plan's grant fails the live story at once, with the run's words.
#[test]
fn a_provider_refusing_the_plan_s_grant_fails_the_live_story_in_words() {
    let directory = support::scratch();
    let peers = live_sign_in::Peers::Refusing;
    let clock = skein_shell::Clock::new();
    let started = clock.now().now;
    live_sign_in::configuration_at(directory.path(), "127.0.0.1:34443", peers);
    let failed = std::panic::catch_unwind(|| live_sign_in::run(directory.path(), started, peers))
        .expect_err("a refused grant fails the live story");
    let why = failed.downcast_ref::<String>().expect("the judge's words");
    assert!(why.starts_with("live chat task=1 can't answer: "), "{why}");
    assert!(why.contains("retry=Some(OwnerFailed { class: Run"), "{why}");
    let trace = support::trace::failures(directory.path());
    eprintln!("{trace}");
    assert!(trace.contains(r#""class":"unauthorized""#) && trace.contains(r#""status":401"#), "{trace}");
}
