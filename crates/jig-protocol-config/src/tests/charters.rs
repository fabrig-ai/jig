use crate::{What, tests::fixtures};

fn refuses(from: &[u8], to: &[u8], what: What) {
    let document = fixtures::with(fixtures::ENGINE_SMALL, from, to);
    let problems = fixtures::engine(&document).expect_err("refused charter");
    assert_eq!(problems.problems.get(0).expect("one problem").what, what);
}

fn a_charter_whose_budget_cannot_hold_its_reserve_is_refused() {
    // 8192 input plus 1024 output at 1 per 1000 rounds to 10; one reserve turn needs 20.
    refuses(br#""spend": 20"#, br#""spend": 19"#, What::Budget { charter: 1 });
}

#[test]
fn a_charter_whose_budget_holds_its_reserve_is_read() {
    let engine = fixtures::engine(fixtures::ENGINE_SMALL).expect("valid charter");
    assert_eq!(engine.charters.len(), 1);
    assert_eq!(engine.charters[0].spend, 20);
}

fn a_charter_turns_not_above_its_reserve_are_refused() {
    refuses(br#""turns": 4"#, br#""turns": 1"#, What::Reserve { charter: 1 });
}

fn a_charter_time_not_above_its_reserve_is_refused() {
    refuses(br#""time_ms": 30000"#, br#""time_ms": 5000"#, What::Reserve { charter: 1 });
}

fn a_charter_naming_an_unknown_model_is_refused() {
    refuses(br#""model": "small""#, br#""model": "absent""#, What::Refers);
}

#[test]
fn a_charter_may_name_models_at_two_endpoints() {
    let alternative = br#""models": [{"name":"other", "endpoint":"second", "window":8192,
        "output":1024, "prices":{"input":1,"cached":1,"output":1,"unit":1000}},"#;
    let endpoint = br#""endpoints": [{"name":"second", "number":2, "dialect":0,
        "provider":"fake-provider", "address":{"socket":"127.0.0.1:7102"},"transport":"plaintext",
        "headers":[],"connect_ms":10000,"handshake_ms":10000},"#;
    let document = fixtures::with(fixtures::ENGINE_SMALL, br#""models": ["#, alternative);
    let document = fixtures::with(&document, br#""endpoints": ["#, endpoint);
    let document = fixtures::with(&document, br#""models":["small"]"#, br#""models":["small","other"]"#);
    let document = fixtures::with(&document, br#""alternatives": []"#, br#""alternatives": ["other"]"#);
    let engine = fixtures::engine(&document).expect("runtime accounts are selected per provider");
    assert_eq!(engine.charters[0].policy.alternatives.len(), 1);
}

#[test]
fn charters_configuration_problems_name_the_invalid_fields() {
    table([
        (
            "a_charter_whose_budget_cannot_hold_its_reserve_is_refused",
            a_charter_whose_budget_cannot_hold_its_reserve_is_refused,
        ),
        ("a_charter_turns_not_above_its_reserve_are_refused", a_charter_turns_not_above_its_reserve_are_refused),
        ("a_charter_time_not_above_its_reserve_is_refused", a_charter_time_not_above_its_reserve_is_refused),
        ("a_charter_naming_an_unknown_model_is_refused", a_charter_naming_an_unknown_model_is_refused),
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
fn charter_documents_are_parsed_with_optional_headings() {
    let bytes = fixtures::with(fixtures::ENGINE_SMALL, br#""alternatives": []"#,
        br#""alternatives": [], "documents":[{"connector":1,"name":["resource"],"path":"rules.md","heading":"Rules"},{"connector":1,"name":["resource"],"path":"notes.md"}]"#);
    let engine = fixtures::engine(&bytes).expect("documents parsed");
    let documents = &engine.charters[0].documents;
    assert_eq!(documents.len(), 2);
    assert_eq!(documents[0].connector, 1);
    assert_eq!(documents[0].name[0].as_ref(), b"resource");
    assert_eq!(documents[0].path.as_ref(), b"rules.md");
    assert_eq!(documents[0].heading.as_deref(), Some(b"Rules".as_slice()));
    assert_eq!(documents[1].heading, None);
}
#[test]
fn charter_documents_override_and_start_capacity_are_checked() {
    for (documents, accepted, replacement) in [
        (
            2_u32,
            true,
            br#""limits":{"profile":"small","overrides":{"brief.sections":8,"core.tasks.charter_documents":2}}"#
                .as_slice(),
        ),
        (
            0,
            false,
            br#""limits":{"profile":"small","overrides":{"brief.sections":8,"core.tasks.charter_documents":0}}"#
                .as_slice(),
        ),
        (
            2_u32,
            true,
            br#""limits":{"profile":"small","overrides":{"brief.sections":8,"tasks.charter_documents":2}}"#.as_slice(),
        ),
        (
            2,
            false,
            br#""limits":{"profile":"small","overrides":{"brief.sections":7,"tasks.charter_documents":2}}"#.as_slice(),
        ),
        (
            3,
            true,
            br#""limits":{"profile":"small","overrides":{"brief.sections":9,"tasks.charter_documents":3}}"#.as_slice(),
        ),
        (
            0,
            false,
            br#""limits":{"profile":"small","overrides":{"brief.sections":8,"tasks.charter_documents":0}}"#.as_slice(),
        ),
    ] {
        let bytes = fixtures::with(fixtures::ENGINE_SMALL, br#""limits": {"profile": "small"}"#, replacement);
        let result = fixtures::engine(&bytes);
        if accepted {
            assert_eq!(result.unwrap().limits.core.tasks.charter_documents, documents);
        } else {
            let errors = result.unwrap_err();
            let mut names_documents = false;
            let mut names_sections = false;
            for problem in &errors.problems {
                names_documents |= problem.path == fixtures::path(&[b"limits", b"core.tasks.charter_documents"]);
                names_sections |= problem.path == fixtures::path(&[b"limits", b"brief.sections"]);
            }
            assert!(names_documents);
            if documents != 0 {
                assert!(names_sections);
            }
        }
    }
}
