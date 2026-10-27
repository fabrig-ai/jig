//! Core pricing at both supplied profiles, including the settings decision's scratch.
use jig_protocol_config::{Profile, profile};
#[test]
fn settings_decisions_fit_the_supplied_profiles() {
    for (name, selected, budget) in
        [("small", Profile::Small, 32_u64 << 30), ("standard", Profile::Standard, 1024_u64 << 30)]
    {
        let limits = profile(selected).core;
        let bytes = jig_domain_core::worst_case(&limits).expect("bounded core");
        eprintln!("core share {name}: {bytes}");
        eprintln!("brief share {name}: {}", jig_domain_brief::gather_worst_case(&limits.brief).expect("bounded brief"));
        eprintln!(
            "conversation {name}: {}; settled row inline: {}; key inline: {}",
            jig_domain_core::conversation_worst_case(&limits).expect("bounded conversation"),
            size_of::<jig_domain_core::SettledCall>(),
            size_of::<jig_domain_core::CallKey>()
        );
        let store = jig_protocol_store::codec::limits(&limits).expect("profile codec limits");
        eprintln!(
            "core codec {name}: settled bytes={}, heap={}; call bytes={}, heap={}; row bytes={}, heap={}",
            jig_protocol_store::core_v6::SettledCall::worst_case_bytes(&store.core).expect("settled bytes"),
            jig_protocol_store::core_v6::SettledCall::worst_case_heap(&store.core).expect("settled heap"),
            jig_protocol_store::core_v6::CallRecord::worst_case_bytes(&store.core).expect("call bytes"),
            jig_protocol_store::core_v6::CallRecord::worst_case_heap(&store.core).expect("call heap"),
            jig_protocol_store::core_v6::Row::worst_case_bytes(&store.core).expect("row bytes"),
            jig_protocol_store::core_v6::Row::worst_case_heap(&store.core).expect("row heap")
        );
        let set = jig_domain_core::settings::set_worst_case(&limits).expect("bounded settings set");
        eprintln!(
            "pending sets {name}: {} * {set} = {}",
            limits.settings.proposals,
            u64::from(limits.settings.proposals) * set
        );
        assert!(bytes <= budget, "{name}: {bytes} > {budget}");
    }
}
