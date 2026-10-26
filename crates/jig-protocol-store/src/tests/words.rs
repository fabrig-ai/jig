//! Refusals pinned to their design text (store.md 2 and 10.1).
#![expect(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    reason = "text assertions use ordinary Rust (programming-model.md 10.2)"
)]
use crate::{Check, Generations, Older, OpenRefusal};

#[expect(clippy::disallowed_macros, reason = "the task pins refusal words to the design itself")]
const DESIGN: &str = include_str!("../../../../docs/design/protocol/store.md");
pub(super) const FORMAT: &str = "putting the family's lower-case name in place of <family>:\n\n  > ";
pub(super) const DEPLOYMENT: &str = "deployment than its header (5.3):\n\n  > ";
pub(super) const NUMBERS: &str = "the one the header records (section 6):\n\n  > ";
pub(super) const FAILED: &str = "cannot recover (section 10):\n\n  > ";

fn unwrapped(start: &str, end: &str) -> String {
    let text = DESIGN.split_once(start).expect("design's refusal start").1;
    let text = text.split_once(end).expect("design's refusal end").0;
    text.lines().map(|line| line.trim().trim_start_matches("> ")).collect::<Vec<_>>().join(" ")
}
fn says(refusal: OpenRefusal, start: &str, end: &str) {
    assert_eq!(refusal.words().join("`<deployment>`"), unwrapped(start, end));
}

pub(super) fn has_deployment_slots(events: &[crate::Event], start: &str) {
    let [crate::Event::OpenRefused { why, .. }] = events else { panic!("one refusal: {events:?}") };
    let text = unwrapped(start, "\n\n-");
    assert!(!why.words().is_empty());
    let pieces = text.matches("`<deployment>`").count().checked_add(1).expect("design's pieces fit");
    assert_eq!(why.words().len(), pieces);
}

#[test]
fn a_format_refusal_names_its_family_and_says_to_move_the_generations() {
    use crate::Family;
    let settings = unwrapped(FORMAT, "\n\n-").replace("<family>", "settings");
    for (family, name) in [
        (Family::Settings, "settings"),
        (Family::Component, "component"),
        (Family::Core, "core"),
        (Family::Tasks, "tasks"),
        (Family::People, "people"),
        (Family::Notes, "notes"),
        (Family::Accounts, "accounts"),
        (Family::Application, "application"),
    ] {
        let words = OpenRefusal::Format { family, found: 2, wanted: 1 }.words().join("`<deployment>`");
        assert_eq!(words, settings.replacen("whose settings", &["whose ", name].concat(), 1), "{family:?}");
    }
}

#[test]
fn a_deployment_refusal_says_to_restore_whole_or_move_the_generations() {
    says(OpenRefusal::Deployment, DEPLOYMENT, "\n\n-");
}

#[test]
fn a_numbers_refusal_says_to_restore_whole_or_move_the_generations() {
    says(OpenRefusal::Numbers { store: 2, header: 1 }, NUMBERS, "\n\n-");
}

#[test]
fn a_failed_refusal_says_to_check_space_and_permissions_then_restore() {
    says(OpenRefusal::Failed, FAILED, "\n\n-");
}

#[test]
fn todays_layouts_refusal_says_to_move_its_three_directories() {
    says(OpenRefusal::OlderBuild { why: Older::Directory }, "  > ", "\n\n- **`OpenRefusal::Format`");
}
#[test]
fn a_busy_refusal_says_to_stop_the_other_process() {
    says(OpenRefusal::Generations { why: Generations::Busy }, "- `Busy`: \"", "\"");
}
#[test]
fn an_unreadable_refusal_says_to_restore_or_name_a_generation() {
    says(OpenRefusal::Generations { why: Generations::Unreadable }, "- `Unreadable`: \"", "\"");
}
#[test]
fn a_missing_refusal_says_to_restore_or_name_a_generation() {
    says(OpenRefusal::Generations { why: Generations::Missing { generation: 7 } }, "- `Missing`: \"", "\"");
}
#[test]
fn an_unnamed_refusal_says_to_name_its_generation() {
    says(OpenRefusal::Generations { why: Generations::Unnamed }, "- `Unnamed`: \"", "\"");
}
#[test]
fn an_unrecorded_refusal_says_to_move_the_generations() {
    says(OpenRefusal::OlderBuild { why: Older::Unrecorded }, "`Unrecorded`'s:\n\n  > ", "\n\n  `Layout`'s");
}
#[test]
fn a_layout_refusal_names_its_family_and_says_to_move_the_generations() {
    use crate::Family;
    says(
        OpenRefusal::OlderBuild { why: Older::Layout(Family::Settings) },
        "(the others name theirs):\n\n  > ",
        "\n\n### 5.4",
    );
    let settings = unwrapped("(the others name theirs):\n\n  > ", "\n\n### 5.4");
    for (family, clause) in [
        (Family::Component, "whose component family's"),
        (Family::Core, "whose core family's"),
        (Family::Tasks, "whose tasks family's"),
        (Family::People, "whose people family's"),
        (Family::Notes, "whose notes family's"),
        (Family::Accounts, "whose accounts family's"),
        (Family::Secrets, "whose secrets family's"),
        (Family::SkeinOauth, "whose kept skein-oauth records'"),
        (Family::SmithTranscript, "whose kept smith turns'"),
        (Family::Application, "whose application family's"),
    ] {
        let words = OpenRefusal::OlderBuild { why: Older::Layout(family) }.words().join("`<deployment>`");
        assert_eq!(words, settings.replacen("whose settings family's", clause, 1), "{family:?}");
    }
}

#[test]
fn level_and_patch_refusals_say_the_designs_words() {
    for (refusal, start) in [
        (OpenRefusal::OlderBuild { why: Older::Unleveled }, "- `OlderBuild { why: Unleveled }`: \""),
        (OpenRefusal::Ahead { owner: crate::Owner::Jig, found: Some(2), wanted: Some(1) }, "- `Ahead`: \""),
        (
            OpenRefusal::Patch {
                owner: crate::Owner::Application,
                patch: 2,
                key: Box::new([]),
                halt: skein_codec::patch::Halt::Order,
            },
            "- `Patch`: \"",
        ),
        (OpenRefusal::Set { owner: crate::Owner::Application, patch: 2 }, "- `Set`: \""),
        (
            OpenRefusal::InFlight { owner: crate::Owner::Application, family: "fixture", count: 0, keys: Box::new([]) },
            "- `InFlight`: \"",
        ),
    ] {
        says(refusal, start, "\"");
    }
}

#[test]
fn check_refusals_say_the_designs_words() {
    for (why, start) in [
        (Check::Large { key: Box::new([]) }, "- `Check { why: Large }`: \""),
        (Check::Count { family: "fixture-label", before: 3, after: 2 }, "- `Check { why: Count }`: \""),
        (Check::Load { key: Box::new([]) }, "- `Check { why: Load }`: \""),
        (Check::Join { key: Box::new([]) }, "- `Check { why: Join }`: \""),
        (Check::Private { name: Box::new([]) }, "- `Check { why: Private }`: \""),
    ] {
        says(OpenRefusal::Check { why }, start, "\"");
    }
}
