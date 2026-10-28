use crate::{Profile, profile};

#[test]
fn supplied_profiles_have_valid_domain_limits() {
    for selected in [Profile::Small, Profile::Standard] {
        let limits = profile(selected);
        assert!(jig_domain_core::valid(&limits.core), "{selected:?} core");
        assert!(jig_domain_host::valid(&limits.hub), "{selected:?} hub");
    }
}
