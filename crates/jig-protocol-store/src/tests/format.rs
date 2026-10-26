//! The format row and the sizes at start (store.md 5.3, 5.4).
use super::fixtures;
use crate::boundary::{Family, OpenRefusal};
use crate::{codec, format, sizes};
use jig_domain_core as core;
use skein_lib::{Duration, JournalLimits};

// People v11 retains the whole project authority as well as its roles.
const JOURNAL: JournalLimits = JournalLimits { commits: 2, writes: 4, held: 4, now: 4, release: 4 };
fn wanted() -> codec::component::Format {
    format::current(fixtures::APPLICATION.version, fixtures::DEPLOYMENT)
}
fn store() -> skein_kv::Limits {
    skein_kv::Limits {
        key: 1024,
        value: 16_777_216,
        ops: 16,
        commit: 134_217_728,
        queued: 2,
        queued_bytes: 268_435_456,
        budget: 16_384,
        segment: 268_435_456,
        snapshot_after: 134_217_728,
        chunk: 16_778_260,
        page: skein_kv::Page { rows: 2, bytes: 16_778_240 },
        deadline: Duration::from_nanos(1_000_000_000),
    }
}
fn check(store: &skein_kv::Limits) -> Result<sizes::Sizes, sizes::SizeRefusal> {
    sizes::check(&fixtures::CORE_LIMITS, &fixtures::APPLICATION, &JOURNAL, store, 0)
}
#[test]
fn a_format_row_of_another_tasks_version_is_refused_naming_tasks() {
    let wanted = format::current(fixtures::APPLICATION.version, fixtures::DEPLOYMENT);
    let mut found = wanted;
    found.tasks = 5;
    assert_eq!(format::check(&found, &wanted), Err(OpenRefusal::Format { family: Family::Tasks, found: 5, wanted: 9 }));
}
#[test]
fn a_format_row_of_another_deployment_is_refused() {
    let wanted = wanted();
    let found = codec::component::Format { deployment: [9; 16], ..wanted };
    assert_eq!(format::check(&found, &wanted), Err(OpenRefusal::Deployment));
}
#[test]
fn the_current_format_row_is_accepted() {
    assert_eq!(format::check(&wanted(), &wanted()), Ok(()));
}
#[test]
fn another_family_is_refused_naming_the_first_difference() {
    let wanted = wanted();
    for (family, found, version, expected) in [
        (Family::Component, codec::component::Format { component: 2, core: 6, ..wanted }, 2, 3),
        (Family::Core, codec::component::Format { core: 5, tasks: 4, ..wanted }, 5, 6),
        (Family::People, codec::component::Format { people: 10, notes: 2, ..wanted }, 10, 13),
        (Family::Notes, codec::component::Format { notes: 2, accounts: 2, ..wanted }, 2, 1),
        (Family::Accounts, codec::component::Format { accounts: 1, application: 2, ..wanted }, 1, 2),
        (Family::Settings, codec::component::Format { settings: 0, ..wanted }, 0, 1),
        (Family::Application, codec::component::Format { application: 2, deployment: [9; 16], ..wanted }, 2, 1),
    ] {
        assert_eq!(
            format::check(&found, &wanted),
            Err(OpenRefusal::Format { family, found: version, wanted: expected })
        );
    }
}
fn a_key_limit_below_the_largest_note_line_is_refused() {
    let mut limits = store();
    limits.key = 111;
    assert_eq!(check(&limits), Err(sizes::SizeRefusal::Key { needs: 112, has: 111 }));
}
fn a_value_limit_below_the_largest_task_record_is_refused() {
    let mut limits = store();
    limits.value = 1;
    assert_eq!(check(&limits), Err(sizes::SizeRefusal::Value { needs: 377_108, has: 1 }));
}
fn a_value_limit_below_the_largest_core_turn_is_refused() {
    let mut limits = store();
    limits.value = 80_948;
    assert_eq!(check(&limits), Err(sizes::SizeRefusal::Value { needs: 377_108, has: 80_948 }));
}
fn a_page_smaller_than_one_row_is_refused() {
    let mut limits = store();
    limits.page.bytes = 81_060;
    assert_eq!(check(&limits), Err(sizes::SizeRefusal::Page { needs: 377_220, has: 81_060 }));
}
#[test]
fn fixture_limits_fit_their_store() {
    assert_eq!(check(&store()), Ok(sizes::Sizes { key: 112, value: 377_108, ops: 11, commit: 17_741 }));
}
fn too_few_operations_are_refused() {
    let mut limits = store();
    limits.ops = 10;
    assert_eq!(check(&limits), Err(sizes::SizeRefusal::Ops { needs: 11, has: 10 }));
}
fn too_few_commit_bytes_are_refused() {
    let mut limits = store();
    limits.commit = 17_740;
    assert_eq!(check(&limits), Err(sizes::SizeRefusal::Commit { needs: 17_741, has: 17_740 }));
}
fn too_few_in_flight_slots_are_refused() {
    let mut limits = store();
    limits.queued = 1;
    assert_eq!(check(&limits), Err(sizes::SizeRefusal::Queue { needs: 2, has: 1 }));
}
fn a_journal_bound_past_the_queue_is_refused_at_start() {
    let journal = JournalLimits { commits: 3, ..JOURNAL };
    assert_eq!(
        sizes::check(&fixtures::CORE_LIMITS, &fixtures::APPLICATION, &journal, &store(), 0),
        Err(sizes::SizeRefusal::Queue { needs: 3, has: 2 }),
    );
}
fn too_few_queued_bytes_are_refused() {
    let mut limits = store();
    limits.queued_bytes = 35_481;
    assert_eq!(check(&limits), Err(sizes::SizeRefusal::QueuedBytes { needs: 35_482, has: 35_481 }));
}
#[test]
fn a_domain_limit_past_its_family_ceiling_is_refused_at_start() {
    let mut domain = fixtures::CORE_LIMITS;
    domain.notes.body_bytes = u32::MAX;
    assert_eq!(sizes::check(&domain, &fixtures::APPLICATION, &JOURNAL, &store(), 0), Err(sizes::SizeRefusal::Ceiling));
}
#[test]
fn an_application_key_that_cannot_hold_its_tag_is_refused() {
    let application = sizes::Application { key: u32::MAX, ..fixtures::APPLICATION };
    assert_eq!(
        sizes::check(&fixtures::CORE_LIMITS, &application, &JOURNAL, &store(), 0),
        Err(sizes::SizeRefusal::Ceiling)
    );
}
fn codec_limits() -> codec::Limits {
    codec::limits(&fixtures::CORE_LIMITS).expect("fixture bounds")
}
#[test]
fn every_childs_record_dispatches_to_its_own_codec() {
    let limits = codec_limits();
    for row in [
        core::Record::Core(core::CoreRecord::Deployment(fixtures::core_rows::deployment())),
        core::Record::Tasks(jig_domain_tasks::Stored::Live(Box::new(fixtures::task_record()))),
        core::Record::People(jig_domain_people::Stored::ReadResults { party: 7, through: 3 }),
        core::Record::Notes(jig_domain_notes::Record::Entry(fixtures::note_entry())),
    ] {
        let bytes = codec::encode(&row, &limits, &[]).expect("record fits");
        assert_eq!(codec::decode(&row.key(), &bytes, &limits, &[]), Some(row));
    }
}
#[test]
fn each_childs_corrupt_record_is_refused_by_dispatch() {
    let limits = codec_limits();
    for key in [
        core::Key::Core(core::CoreKey::Deployment),
        core::Key::Tasks(jig_domain_tasks::Key::Live(1)),
        core::Key::People(jig_domain_people::Key::Person(1)),
        core::Key::Notes(jig_domain_notes::Key::Entry { name: 1 }),
    ] {
        assert_eq!(codec::decode(&key, &[], &limits, &[]), None);
    }
}
#[test]
fn the_largest_note_line_key_reaches_its_calculated_bound() {
    let mut segments = skein_lib::List::with_capacity(fixtures::NOTES_LIMITS.pattern_bytes);
    for _ in 0..fixtures::NOTES_LIMITS.pattern_bytes {
        segments.push(Box::<[u8]>::default()).expect("segment count fits");
    }
    let key = core::Key::Notes(jig_domain_notes::Key::Line {
        scope: jig_domain_notes::Scope::Resources {
            project: u32::MAX,
            connector: u16::MAX,
            pattern: jig_domain_notes::Pattern {
                segments: segments.into_boxed(),
                last: jig_domain_notes::Last::Exact(Box::new([0; 16])),
            },
        },
        name: u64::MAX,
    });
    assert_eq!(crate::keys::encode(&key, 112).expect("largest line fits").len(), 112);
    assert_eq!(crate::keys::encode(&key, 111), None);
}
#[test]
fn larger_application_encodings_are_included_in_the_bounds() {
    let application = sizes::Application { version: 1, key: 256, value: 9_000_000 };
    assert_eq!(
        sizes::check(&fixtures::CORE_LIMITS, &application, &JOURNAL, &store(), 0),
        Ok(sizes::Sizes { key: 257, value: 9_000_000, ops: 11, commit: 19_336 }),
    );
}
#[test]
fn a_journal_whose_operation_count_overflows_is_refused() {
    let journal = JournalLimits { writes: u32::MAX, ..JOURNAL };
    assert_eq!(
        sizes::check(&fixtures::CORE_LIMITS, &fixtures::APPLICATION, &journal, &store(), 0),
        Err(sizes::SizeRefusal::Ceiling),
    );
}

#[test]
fn a_map_budget_past_the_commit_envelope_is_refused() {
    for (label, budget, expected) in [
        ("exact_u32_commit", 4_294_965_938, Ok(sizes::Sizes { key: 112, value: 377_108, ops: 11, commit: u32::MAX })),
        ("one_byte_past_u32", 4_294_965_939, Err(sizes::SizeRefusal::Ceiling)),
        ("u64_sum_overflows", u64::MAX, Err(sizes::SizeRefusal::Ceiling)),
    ] {
        let limits = skein_kv::Limits { budget, commit: u32::MAX, queued_bytes: u64::MAX, ..store() };
        assert_eq!(check(&limits), expected, "{label}");
    }
}

#[test]
fn store_limits_below_their_required_sizes_are_refused() {
    table([
        ("a_key_limit_below_the_largest_note_line_is_refused", a_key_limit_below_the_largest_note_line_is_refused),
        (
            "a_value_limit_below_the_largest_task_record_is_refused",
            a_value_limit_below_the_largest_task_record_is_refused,
        ),
        ("a_value_limit_below_the_largest_core_turn_is_refused", a_value_limit_below_the_largest_core_turn_is_refused),
        ("a_page_smaller_than_one_row_is_refused", a_page_smaller_than_one_row_is_refused),
        ("too_few_operations_are_refused", too_few_operations_are_refused),
        ("too_few_commit_bytes_are_refused", too_few_commit_bytes_are_refused),
        ("too_few_in_flight_slots_are_refused", too_few_in_flight_slots_are_refused),
        ("a_journal_bound_past_the_queue_is_refused_at_start", a_journal_bound_past_the_queue_is_refused_at_start),
        ("too_few_queued_bytes_are_refused", too_few_queued_bytes_are_refused),
    ]);
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
#[expect(clippy::disallowed_macros, reason = "ordinary tests embed the eight schema fixtures independently")]
fn current_layouts_are_the_eight_distinct_runtime_schema_fingerprints() {
    let layouts = format::layouts();
    let current = wanted();
    let families = [
        (layouts.component, current.component, include_bytes!("../../schema/component-v3.schema").as_slice()),
        (layouts.core, current.core, include_bytes!("../../schema/core-v6.schema").as_slice()),
        (layouts.tasks, current.tasks, include_bytes!("../../schema/tasks-v9.schema").as_slice()),
        (layouts.people, current.people, include_bytes!("../../schema/people-v13.schema").as_slice()),
        (layouts.notes, current.notes, include_bytes!("../../schema/notes-v1.schema").as_slice()),
        (layouts.accounts, current.accounts, include_bytes!("../../schema/accounts-v2.schema").as_slice()),
        (layouts.settings, current.settings, include_bytes!("../../schema/settings-v1.schema").as_slice()),
        (layouts.secrets, 1, include_bytes!("../../schema/secrets-v1.schema").as_slice()),
    ];
    let mut digests = skein_lib::Set::with_capacity(u32::try_from(families.len()).expect("eight families"));
    for (digest, version, text) in families {
        let schema = skein_codec::schema::parse(text).expect("current schema parses");
        assert_eq!(schema.version(), version);
        assert_eq!(digest, schema.fingerprints().family().digest());
        assert!(digests.insert(digest).expect("eight families fit"), "families have distinct layouts");
    }
}

#[test]
fn differing_layouts_name_the_first_family_in_family_order() {
    let wanted = format::layouts();
    let mut settings = wanted;
    settings.settings[0] ^= 1;
    settings.component[0] ^= 1;
    let mut secrets = wanted;
    secrets.secrets[0] ^= 1;
    secrets.skein_oauth[0] ^= 1;
    secrets.smith_transcript[0] ^= 1;
    let mut oauth = wanted;
    oauth.skein_oauth[0] ^= 1;
    oauth.smith_transcript[0] ^= 1;
    for (found, family) in [(settings, Family::Settings), (secrets, Family::Secrets), (oauth, Family::SkeinOauth)] {
        assert_eq!(
            format::check_layouts(&found, &wanted),
            Err(OpenRefusal::OlderBuild { why: crate::Older::Layout(family) })
        );
    }
}

#[test]
fn the_manifest_at_both_bounds_reaches_its_whole_file_price() {
    let ceilings = &crate::component_v3::CEILINGS;
    let mut counts = skein_lib::List::with_capacity(ceilings.manifest_counts);
    for _ in 0..ceilings.manifest_counts {
        counts
            .push(
                codec::component::Count::new(
                    ceilings,
                    codec::component::CountParts {
                        family: Box::from(
                            "f".repeat(usize::try_from(ceilings.count_family).expect("name fits")).as_bytes(),
                        ),
                        before: u64::MAX,
                        after: u64::MAX,
                    },
                )
                .expect("family name at its bound"),
            )
            .expect("families at their bound");
    }
    let levels = crate::patching::plan::Levels {
        skein: Some(u32::MAX),
        smith: Some(u32::MAX),
        jig: Some(u32::MAX),
        application: Some(u32::MAX),
    };
    let manifest = crate::generations::manifest(
        u64::MAX,
        Some(u64::MAX),
        levels,
        levels,
        &counts.into_boxed(),
        u64::MAX,
        Some(u64::MAX),
        skein_lib::Wall::EPOCH,
    );
    assert_eq!(sizes::MANIFEST_BYTES, 5464);
    assert_eq!(manifest.len(), usize::try_from(sizes::MANIFEST_BYTES).expect("manifest fits"));
}

#[test]
fn whole_file_admission_checks_the_manifests_exact_bound() {
    for (bound, expected) in [
        (
            sizes::MANIFEST_BYTES - 1,
            Err(sizes::SizeRefusal::Whole { needs: sizes::MANIFEST_BYTES, has: sizes::MANIFEST_BYTES - 1 }),
        ),
        (sizes::MANIFEST_BYTES, Ok(())),
    ] {
        assert_eq!(sizes::whole(bound), expected, "whole-file bound {bound}");
    }
}

#[test]
#[expect(clippy::disallowed_macros, reason = "the kept schema copy is pinned against smith's generated code")]
fn upstream_layouts_equal_the_kept_families_fingerprints() {
    let layouts = format::layouts();
    assert_eq!(layouts.skein_oauth, skein_oauth::FINGERPRINT);
    let smith = skein_codec::schema::parse(include_bytes!("fixtures/smith-transcript-v6.schema"))
        .expect("checked smith schema");
    assert_eq!(layouts.smith_transcript, smith.fingerprints().family().digest());
}
