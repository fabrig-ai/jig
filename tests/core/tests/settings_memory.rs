//! Profile codec and settings-record edges remain representable.
use jig_protocol_config::{Profile, profile};
#[test]
fn settings_records_fit_the_supplied_profiles() {
    for selected in [Profile::Small, Profile::Standard] {
        let limits = profile(selected).core;
        assert!(jig_domain_core::valid(&limits));
        let store = jig_protocol_store::codec::limits(&limits).expect("profile codec limits");
        assert!(jig_protocol_store::core_v6::SettledCall::worst_case_bytes(&store.core).is_some());
        assert!(jig_protocol_store::core_v6::SettledCall::worst_case_heap(&store.core).is_some());
        assert!(jig_protocol_store::core_v6::CallRecord::worst_case_bytes(&store.core).is_some());
        assert!(jig_protocol_store::core_v6::CallRecord::worst_case_heap(&store.core).is_some());
        assert!(jig_protocol_store::core_v6::Row::worst_case_bytes(&store.core).is_some());
        assert!(jig_protocol_store::core_v6::Row::worst_case_heap(&store.core).is_some());
        assert!(jig_domain_core::settings::set_worst_case(&limits).is_some());
    }
}
