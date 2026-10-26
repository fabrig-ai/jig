//! Secrets family ceilings stay those of their producers (store.md 9).
use jig_protocol_store::secrets_v1::CEILINGS;
#[test]
fn the_saved_record_bound_is_oauths_ceiling() {
    assert_eq!(
        u64::from(CEILINGS.source_record_record),
        skein_oauth::v3::SavedToken::worst_case_bytes(&skein_oauth::v3::CEILINGS).expect("OAuth bound")
    );
}
#[test]
fn carried_and_key_bounds_are_the_client_wires_ceiling() {
    let bound = jig_client_protocol_wire::v19::CEILINGS.keyed_value_some;
    for ceiling in
        [CEILINGS.source_key_key, CEILINGS.carried_key_key, CEILINGS.carried_value_value, CEILINGS.carried_moved_moved]
    {
        assert_eq!(ceiling, bound);
    }
}
