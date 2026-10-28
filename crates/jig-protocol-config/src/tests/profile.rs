use crate::{Profile, profile};

#[test]
fn supplied_profiles_have_valid_domain_limits() {
    for selected in [Profile::Small, Profile::Standard] {
        let limits = profile(selected);
        assert!(jig_domain_core::valid(&limits.core), "{selected:?} core");
        assert!(jig_domain_host::valid(&limits.hub), "{selected:?} hub");
    }
}

#[test]
fn child_validity_keeps_semantic_and_count_refusals() {
    let limits = profile(Profile::Small).core;
    let mut brief = limits.brief;
    brief.briefs = 0;
    assert!(!jig_domain_brief::valid(&brief));
    brief = limits.brief;
    brief.sections = 0;
    assert!(!jig_domain_brief::valid(&brief));
    brief = limits.brief;
    brief.read_bytes = 0;
    assert!(!jig_domain_brief::valid(&brief));
    brief = limits.brief;
    brief.brief_bytes = 0;
    assert!(!jig_domain_brief::valid(&brief));
    let mut notes = limits.notes;
    notes.scopes = 0;
    assert!(!jig_domain_notes::valid(&notes));
    notes = limits.notes;
    notes.entries_per_scope = 0;
    assert!(!jig_domain_notes::valid(&notes));
    notes = limits.notes;
    notes.load_rows = 0;
    assert!(!jig_domain_notes::valid(&notes));
    notes = limits.notes;
    notes.recalled = 0;
    assert!(!jig_domain_notes::valid(&notes));
    notes = limits.notes;
    notes.scopes = u32::MAX;
    assert!(!jig_domain_notes::valid(&notes));
    let mut views = limits.views;
    views.runs = 0;
    assert!(!jig_domain_views::valid(&views));
    views = limits.views;
    views.watchers = 0;
    assert!(!jig_domain_views::valid(&views));
    views = limits.views;
    views.watchers_per_party = 0;
    assert!(!jig_domain_views::valid(&views));
    views = limits.views;
    views.subjects_per_change = 0;
    assert!(!jig_domain_views::valid(&views));
    views = limits.views;
    views.text_bytes = 0;
    assert!(!jig_domain_views::valid(&views));
    views = limits.views;
    views.watchers = u32::MAX;
    assert!(!jig_domain_views::valid(&views));
    let mut fleet = limits.fleet;
    fleet.worker_name_bytes = jig_domain_fleet::WORKER_NAME + 1;
    assert!(!jig_domain_fleet::valid(&fleet));
    fleet = limits.fleet;
    fleet.workers = u32::MAX;
    fleet.engine_slots = 1;
    assert!(!jig_domain_fleet::valid(&fleet));
    fleet = limits.fleet;
    fleet.history = u32::MAX;
    assert!(!jig_domain_fleet::valid(&fleet));
}
