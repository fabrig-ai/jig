//! A run's settings reads share party documents without recording a decision (engine.md, 7.3).
use jig_core_world::{effects::World, fixtures};
use jig_domain_core::{self as core, documents, reads, settings};
use jig_domain_people as people;
#[path = "settings/support.rs"]
mod support;
use support::*;

fn run_read(world: &mut World, input: &[u8]) -> core::SettingsAnswer {
    world.tool_call(b"settings", input, false);
    world.settings_answers.pop().expect("settings read answer").2
}

#[test]
fn a_run_reads_the_inventory_a_setting_a_revision_and_a_history_page() {
    let (mut world, signed, _) = proposing_run(true);
    let item = template_edit(&world, 7);
    let proposal = proposed(&propose(&mut world, signed, 1, 1, Box::new([item])));
    let expected = inventory(&mut world, signed);
    assert_eq!(
        expected.pending.into_boxed().as_ref(),
        &[documents::SettingsProposal {
            proposer: people::Proposer::Party(OWNER),
            number: u32::try_from(proposal).expect("settings read fixture")
        }]
    );
    for (input, party_read) in [
        (br#"{"kind":"inventory"}"#.as_slice(), reads::Read::Settings),
        (
            br#"{"kind":"setting","setting":"charter","number":1,"revision":1}"#,
            reads::Read::Setting { setting: settings::Setting::Charter(1), revision: 1, proposal: None },
        ),
        (br#"{"kind":"revision","number":1}"#, reads::Read::Revision { number: 1 }),
        (br#"{"kind":"revisions"}"#, reads::Read::Revisions { before: None }),
    ] {
        let expected = read(&mut world, signed, party_read);
        let answer = run_read(&mut world, input);
        match (answer, expected) {
            (core::SettingsAnswer::Inventory(value), reads::Answer::Document(document)) => {
                assert_eq!(*document, documents::Document::Settings(Box::new(value)));
            }
            (core::SettingsAnswer::Setting(value), reads::Answer::Setting(expected)) => assert_eq!(value, *expected),
            (core::SettingsAnswer::Revision(value), reads::Answer::Revision(expected)) => assert_eq!(value, *expected),
            (core::SettingsAnswer::Revisions(value), reads::Answer::Revisions(expected)) => {
                assert_eq!(value, *expected);
            }
            other => panic!("matching party document: {other:?}"),
        }
    }
    let core::SettledAnswer::Host { error, body } = &world.host_answers.last().expect("rendered read").1.answer else {
        panic!("host text")
    };
    assert!(!error);
    assert_eq!(body.as_ref(), b"Revisions: 1 rows");
}

#[test]
fn a_run_reads_its_own_held_charter_by_revision() {
    let (mut world, signed, goal) = proposing_run(true);
    let original = world.domain.settings().charter(1).expect("settings read fixture").clone();
    let item = run_charter(&world, b"Revision two's complete instructions");
    changed(&change(&mut world, signed, 1, 1, Box::new([item])), 2);
    assert_eq!(fixtures::live_row(&world, goal).settings.run, Some(1));
    let newest = world.domain.settings().charter(1).expect("settings read fixture").clone();
    for (revision, charter) in [(1, original), (2, newest)] {
        let input = format!(r#"{{"kind":"setting","setting":"charter","number":1,"revision":{revision}}}"#);
        assert_eq!(
            run_read(&mut world, input.as_bytes()),
            core::SettingsAnswer::Setting(reads::SettingDocument {
                setting: settings::Setting::Charter(1),
                revision,
                value: settings::Value::Charter(Box::new(charter))
            })
        );
    }
    assert_eq!(
        run_read(&mut world, br#"{"kind":"setting","setting":"template","number":1,"revision":1}"#),
        core::SettingsAnswer::Unknown
    );
    assert_eq!(
        run_read(&mut world, br#"{"kind":"setting","setting":"charter","number":99,"revision":2}"#),
        core::SettingsAnswer::Unknown
    );
}

#[test]
fn a_run_without_the_settings_family_is_denied_its_read() {
    let (mut world, _, _) = proposing_run(false);
    let rows = world.store.rows.clone();
    for input in [
        br#"{"kind":"inventory"}"#.as_slice(),
        br#"{"kind":"setting","setting":"rules","revision":1}"#,
        br#"{"kind":"revision","number":1}"#,
        br#"{"kind":"revisions"}"#,
    ] {
        world.tool_call(b"settings", input, false);
        assert!(matches!(world.answers.last().expect("denied read").2, core::CallPart::ToolDenied { .. }));
    }
    assert!(world.settings_answers.is_empty());
    assert_eq!(world.store.rows, rows);
}

#[test]
fn a_settings_read_keeps_no_record_and_is_read_again() {
    const NAME: u32 = 1;
    let (mut world, signed, _) = proposing_run(true);
    let rows = world.store.rows.clone();
    let applied = world.store.applied;
    world.tool_call_at(NAME, b"settings", br#"{"kind":"inventory"}"#, false);
    let (_, first_key, core::SettingsAnswer::Inventory(first)) =
        world.settings_answers.pop().expect("settings read fixture")
    else {
        panic!("inventory")
    };
    assert_eq!(first.newest, 1);
    assert_eq!(world.store.rows, rows);
    assert_eq!(world.store.applied, applied);
    let item = template_edit(&world, 7);
    changed(&change(&mut world, signed, 1, 1, Box::new([item])), 2);
    let rows = world.store.rows.clone();
    world.tool_call_at(NAME, b"settings", br#"{"kind":"inventory"}"#, false);
    let (_, second_key, core::SettingsAnswer::Inventory(second)) =
        world.settings_answers.pop().expect("settings read fixture")
    else {
        panic!("fresh inventory")
    };
    assert_eq!(first_key, second_key);
    assert_eq!(second.newest, 2);
    assert_eq!(world.store.rows, rows);
    assert!(world.answers.is_empty());
}

#[test]
fn a_forgotten_revision_read_names_the_oldest_kept() {
    let (mut world, signed, _) = settings_run(true, Some(2));
    for against in 1..=4 {
        let item = template_edit(&world, u64::from(against));
        changed(
            &change(
                &mut world,
                signed,
                u8::try_from(against).expect("settings read fixture"),
                against,
                Box::new([item]),
            ),
            against + 1,
        );
    }
    assert_eq!(
        run_read(&mut world, br#"{"kind":"revision","number":2}"#),
        core::SettingsAnswer::Revision(reads::RevisionAnswer::Forgotten { oldest: 3 })
    );
    // Revision one remains held by this goal even outside history.
    assert!(
        matches!(run_read(&mut world, br#"{"kind":"revision","number":1}"#), core::SettingsAnswer::Revision(reads::RevisionAnswer::Revision { revision, .. }) if revision.head.number == 1)
    );
    assert_eq!(
        run_read(&mut world, br#"{"kind":"revision","number":0}"#),
        core::SettingsAnswer::Revision(reads::RevisionAnswer::Unknown)
    );
    let core::SettingsAnswer::Revisions(page) = run_read(&mut world, br#"{"kind":"revisions","before":3}"#) else {
        panic!("held page")
    };
    assert_eq!(page.rows.get(0).expect("settings read fixture").head.number, 1);
    assert!(page.forgotten);
    assert_eq!(page.next, None);
}

#[test]
fn a_settings_read_is_busy_when_the_shared_load_slots_are_full() {
    let (mut world, signed, _) = proposing_run(true);
    let slots = run_configuration().1.core.load_slots;
    world.sign_in = Some(signed);
    world.client_peer = None;
    let mut parties = jig_core_world::reads::Parties::new(world.store.applied);
    parties.hold_loads = true;
    world.reads = Some(parties);
    for number in 1..=slots {
        jig_core_world::reads::ask(&mut world, u64::from(number), reads::Read::Revision { number: 1 });
    }
    world.tool_call(b"settings", br#"{"kind":"revision","number":1}"#, false);
    assert!(matches!(world.answers.last().expect("settings read fixture").2, core::CallPart::Unavailable));
    assert!(world.settings_answers.is_empty());
    jig_core_world::reads::complete(&mut world);
    world.reads = None;
    assert!(matches!(
        run_read(&mut world, br#"{"kind":"revision","number":1}"#),
        core::SettingsAnswer::Revision(reads::RevisionAnswer::Revision { .. })
    ));
}

#[test]
fn a_charter_set_replaces_its_tools_and_retains_the_held_versions_mask() {
    let (mut world, signed, _) = proposing_run(true);
    let original = world.domain.settings().charter(1).expect("fixture charter").clone();
    let mut next = original.clone();
    next.policy.tools = jig_domain_authority::Tools(core::ToolFamilies::JIG.settings);
    let item = settings::Item {
        setting: settings::Setting::Charter(1),
        edit: settings::Edit::Set(settings::Value::Charter(Box::new(next.clone()))),
    };
    changed(&change(&mut world, signed, 1, 1, Box::new([item])), 2);
    for (revision, expected) in [(1, original), (2, next)] {
        assert_eq!(
            read(
                &mut world,
                signed,
                reads::Read::Setting { setting: settings::Setting::Charter(1), revision, proposal: None }
            ),
            reads::Answer::Setting(Box::new(reads::SettingDocument {
                setting: settings::Setting::Charter(1),
                revision,
                value: settings::Value::Charter(Box::new(expected)),
            }))
        );
    }
}
