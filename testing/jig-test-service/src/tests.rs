mod clients;
mod engine;
mod family;
mod first_period;
pub(crate) mod fixtures;
mod server;
mod startup;
mod stop;
mod worker;
use crate::tests::fixtures::{engine_document, problem, with};
use jig_protocol_config::{Path, Problem, What};

#[test]
fn the_document_names_the_files_its_caller_reads() {
    let names = crate::named(&engine_document()).expect("fixture document");
    assert_eq!(names.len(), 4);
    for (name, expected) in names.iter().zip(fixtures::NAMES) {
        assert_eq!(name.as_ref(), expected);
    }
}

#[test]
fn both_jig_profiles_have_a_bounded_service() {
    for (label, name, value, commit, ops) in [
        ("small", jig_protocol_config::Profile::Small, 1_567_861, 355_308_810, 41_606),
        ("standard", jig_protocol_config::Profile::Standard, 10_664_789, 901_830_282, 303_350),
    ] {
        let limits = crate::limits::profile_limits(name);
        let sizes = jig_protocol_store::sizes::check(
            &limits.root.core,
            &jig_test_store::application(&limits.root.connector),
            &limits.root.journal,
            &limits.store.store,
            limits.store.orphan_erases,
        )
        .expect(label);
        assert_eq!((sizes.key, sizes.value, sizes.commit, sizes.ops), (2079, value, commit, ops), "{label}");
        assert!(crate::limits::worst_case(&limits).is_some(), "{label}");
        crate::store::limits(&limits.root, &limits.store).expect(label);
    }
}

#[test]
fn a_document_with_an_unknown_member_is_refused_naming_its_path() {
    let document = with(&engine_document(), b"\"service\":", b"\"extra\":true,\"service\":");
    assert_eq!(problem(&document), Problem { path: Path::root().member(b"extra"), what: What::Unknown });
}

#[test]
fn a_named_file_missing_from_those_handed_is_refused() {
    let document = engine_document();
    let mut files = fixtures::files(&document);
    let last = files.entries.len().checked_sub(1).expect("credential file");
    assert_eq!(files.entries.get(last).expect("credential file").0.as_ref(), b"app.secret");
    files.entries.truncate(last);
    let problems =
        crate::startup(&document, &files, None, &fixtures::seeds(), None, None).err().expect("startup refused");
    assert_eq!(
        problems.problems.get(0),
        Some(&Problem { path: Path::root().member(b"application").member(b"credential"), what: What::Unreadable })
    );
}

#[test]
fn a_worst_case_past_the_memory_is_refused_saying_by_how_much() {
    let bytes = fixtures::service_bytes(&engine_document());
    let memory = fixtures::decimal(bytes.checked_sub(1).expect("nonempty service"));
    let document = with(&engine_document(), b"1152921504606846976", &memory);
    let found = crate::startup(&document, &fixtures::files(&document), None, &fixtures::seeds(), None, None)
        .err()
        .expect("startup refused");
    assert_eq!(
        found.problems.get(0),
        Some(&Problem { path: Path::root().member(b"engine").member(b"memory_bytes"), what: What::Memory { over: 1 } })
    );
}
