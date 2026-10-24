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
        let set = jig_domain_core::settings::set_worst_case(&limits).expect("bounded settings set");
        eprintln!(
            "pending sets {name}: {} * {set} = {}",
            limits.settings.proposals,
            u64::from(limits.settings.proposals) * set
        );
        assert!(bytes <= budget, "{name}: {bytes} > {budget}");
    }
}
