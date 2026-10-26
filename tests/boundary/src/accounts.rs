//! Accounts translate only their child's vocabulary and skein's driver.
const MANIFEST: &str = include_str!("../../../crates/jig-protocol-accounts/Cargo.toml");
#[test]
fn protocol_accounts_link_only_their_child_and_skein() {
    for line in MANIFEST.split("[dev-dependencies]").next().expect("manifest").lines() {
        let Some((key, _)) = line.split_once('=') else { continue };
        let key = key.trim();
        let Some(name) = key.strip_suffix(".workspace") else { continue };
        if name.starts_with("jig-") || name.starts_with("skein-") || name.starts_with("smith-") {
            assert!(
                ["skein-lib", "skein-codec", "skein-oauth", "skein-oauth-accounts", "jig-domain-accounts"]
                    .contains(&name),
                "{line}"
            );
        }
    }
}
