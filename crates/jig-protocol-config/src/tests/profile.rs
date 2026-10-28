use crate::{Limits, Profile, profile};

fn limits(profile_name: Profile) -> Limits {
    profile(profile_name)
}
fn assert_children_fit(profile_name: Profile) {
    let core = limits(profile_name).core;
    let bounds = [
        ("tasks", jig_domain_tasks::worst_case(&core.tasks)),
        ("authority", jig_domain_authority::worst_case(&core.authority)),
        ("people", jig_domain_people::worst_case(&core.people)),
        ("fleet", jig_domain_fleet::worst_case(&core.fleet)),
        ("brief", jig_domain_brief::gather_worst_case(&core.brief)),
        ("accounts", jig_domain_accounts::worst_case(&core.accounts)),
        ("notes", jig_domain_notes::worst_case(&core.notes)),
        ("views", jig_domain_views::worst_case(&core.views)),
    ];
    for (name, bytes) in bounds {
        assert!(bytes.is_some(), "{profile_name:?} {name} has a representable worst case");
    }
}
fn core_bytes(profile_name: Profile) -> Option<u64> {
    jig_domain_core::worst_case(&limits(profile_name).core)
}
fn hub_bytes(profile_name: Profile) -> Option<u64> {
    jig_domain_host::worst_case(&limits(profile_name).hub)
}

#[test]
fn the_small_profile_fits_every_childs_worst_case() {
    assert_children_fit(Profile::Small);
}
#[test]
fn the_standard_profile_fits_every_childs_worst_case() {
    assert_children_fit(Profile::Standard);
}
fn the_small_profile_fits_the_cores_worst_case() {
    assert!(core_bytes(Profile::Small).is_some(), "small core bound");
}
fn the_standard_profile_fits_the_cores_worst_case() {
    assert!(core_bytes(Profile::Standard).is_some(), "standard core bound");
}
fn the_small_profiles_hub_fits_its_worst_case() {
    assert!(hub_bytes(Profile::Small).is_some(), "small hub bound");
}
fn the_standard_profiles_hub_fits_its_worst_case() {
    assert!(hub_bytes(Profile::Standard).is_some(), "standard hub bound");
}

#[test]
fn profiles_fit_the_cores_worst_case() {
    table([
        ("the_small_profile_fits_the_cores_worst_case", the_small_profile_fits_the_cores_worst_case),
        ("the_standard_profile_fits_the_cores_worst_case", the_standard_profile_fits_the_cores_worst_case),
    ]);
}

#[test]
fn profiles_hubs_fit_their_worst_case() {
    table([
        ("the_small_profiles_hub_fits_its_worst_case", the_small_profiles_hub_fits_its_worst_case),
        ("the_standard_profiles_hub_fits_its_worst_case", the_standard_profiles_hub_fits_its_worst_case),
    ]);
}

#[test]
fn profiles_drivers_listen_for_each_sign_in_at_once() {
    for profile_name in [Profile::Small, Profile::Standard] {
        let limits = limits(profile_name);
        assert!(limits.driver.listeners >= 1, "{profile_name:?} listens for a sign-in");
        // A cancelled sign-in's listener counts until it settles (skein's oauth.md, 6.5).
        assert_eq!(limits.driver.listeners, 2 * limits.core.accounts.sign_ins, "{profile_name:?}");
        assert_eq!(
            limits.driver.accounts,
            limits.core.accounts.accounts * limits.core.accounts.sources + limits.core.accounts.sign_ins + 1
        );
        assert!(limits.driver.client.url_bytes <= jig_domain_accounts::ADDRESS);
        let crypto = skein_lib::CryptoRng::new(&skein_lib::Secret::new(Box::new([7; 32])), 1).unwrap();
        let driver = skein_oauth_accounts::Component::new(skein_lib::List::with_capacity(0), &limits.driver, crypto);
        assert!(driver.is_ok(), "{profile_name:?} driver is usable");
    }
}

type Case = (&'static str, fn());

fn table<const N: usize>(cases: [Case; N]) {
    let mut failures = [None; N];
    let mut passed = true;
    for ((name, case), failure) in cases.into_iter().zip(&mut failures) {
        if std::panic::catch_unwind(case).is_err() {
            *failure = Some(name);
            passed = false;
        }
    }
    assert!(passed, "{failures:?}");
}

#[test]
fn profiles_price_the_core_and_reference_roots_whole_decision() {
    for name in [Profile::Small, Profile::Standard] {
        let limits = limits(name);
        let room = jig_domain_core::route_bound(&limits.core).unwrap();
        // The reference root adds two connector output allowances of nine.
        assert_eq!(limits.journal.writes, room + 18, "{name:?}");
        assert_eq!(limits.journal.held, limits.journal.commits * limits.journal.writes, "{name:?}");
        assert_eq!(limits.store.store.ops, 2 * limits.journal.writes + 4, "{name:?}");
    }
}
