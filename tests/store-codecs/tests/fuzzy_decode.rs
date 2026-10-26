//! Arbitrary and mutated family bytes (protocol/store.md, sections 5.1 and 14).
use jig_protocol_store::{accounts_v2, component_v3, notes_v1, people_v13};
use skein_lib::{Reader, Rng, Writer};
use std::{fs, path::Path};

fn notes(bytes: &[u8]) {
    if let Ok(row) = notes_v1::Row::decode(&notes_v1::CEILINGS, &mut Reader::new(bytes)) {
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("measured length"));
        row.encode(&mut writer).expect("measured room");
        assert_eq!(writer.finish().as_ref(), bytes);
    }
}
fn component(bytes: &[u8]) {
    if let Ok(row) = component_v3::Layouts::decode(&component_v3::CEILINGS, &mut Reader::new(bytes)) {
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("measured length"));
        row.encode(&mut writer).expect("measured room");
        assert_eq!(writer.finish().as_ref(), bytes);
    }
    if let Ok(row) = component_v3::Format::decode(&component_v3::CEILINGS, &mut Reader::new(bytes)) {
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("measured length"));
        row.encode(&mut writer).expect("measured room");
        assert_eq!(writer.finish().as_ref(), bytes);
    }
    if let Ok(row) = component_v3::Transcript::decode(&component_v3::CEILINGS, &mut Reader::new(bytes)) {
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("measured length"));
        row.encode(&mut writer).expect("measured room");
        assert_eq!(writer.finish().as_ref(), bytes);
    }
    if let Ok(row) = component_v3::Attempt::decode(&component_v3::CEILINGS, &mut Reader::new(bytes)) {
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("measured length"));
        row.encode(&mut writer).expect("measured room");
        assert_eq!(writer.finish().as_ref(), bytes);
    }
}
fn accounts(bytes: &[u8]) {
    if let Ok(row) = accounts_v2::Installation::decode(&accounts_v2::CEILINGS, &mut Reader::new(bytes)) {
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("measured length"));
        row.encode(&mut writer).expect("measured room");
        assert_eq!(writer.finish().as_ref(), bytes);
    }
    if let Ok(row) = accounts_v2::Row::decode(&accounts_v2::CEILINGS, &mut Reader::new(bytes)) {
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("measured length"));
        row.encode(&mut writer).expect("measured room");
        assert_eq!(writer.finish().as_ref(), bytes);
    }
}
fn arbitrary(seed: u64, check: fn(&[u8])) {
    let mut rng = Rng::new(seed);
    let mut buffer = [0_u8; 128];
    for _ in 0_u32..10_000 {
        let length = usize::try_from(rng.below(129)).expect("bounded length");
        for byte in buffer.get_mut(..length).expect("within buffer") {
            *byte = u8::try_from(rng.next_u64() & 0xff).expect("low byte");
        }
        check(buffer.get(..length).expect("within buffer"));
    }
}
fn mutated(family: &str, seed: u64, check: fn(&[u8])) {
    let root = skein_scratch::paths::workspace().join("crates/jig-protocol-store/golden").join(family);
    mutate_goldens(&root, seed, check);
}
fn mutate_goldens(root: &Path, seed: u64, check: fn(&[u8])) {
    let mut rng = Rng::new(seed);
    let mut paths: Vec<_> =
        fs::read_dir(root).expect("golden directory").map(|entry| entry.expect("golden entry").path()).collect();
    paths.sort();
    for path in paths {
        if !path.file_name().expect("file name").to_string_lossy().ends_with("_full.bin") {
            continue;
        }
        let golden = fs::read(path).expect("golden readable");
        check(&golden);
        for _ in 0_u32..100 {
            let mut bytes = golden.clone();
            if !bytes.is_empty() {
                let index =
                    usize::try_from(rng.below(u64::try_from(bytes.len()).expect("length fits"))).expect("index fits");
                *bytes.get_mut(index).expect("chosen byte") = u8::try_from(rng.next_u64() & 0xff).expect("low byte");
            }
            check(&bytes);
        }
    }
}
#[test]
fn arbitrary_notes_rows_never_panic_and_round_trip_when_they_decode() {
    arbitrary(0x5701_e014, notes);
}
#[test]
fn mutated_notes_goldens_never_panic() {
    mutated("notes", 0x5702_e014, notes);
}
#[test]
fn arbitrary_component_rows_never_panic_and_round_trip_when_they_decode() {
    arbitrary(0x5703_e014, component);
}
#[test]
fn mutated_component_goldens_never_panic() {
    mutated("component", 0x5704_e014, component);
}
#[test]
fn arbitrary_accounts_rows_never_panic_and_round_trip_when_they_decode() {
    arbitrary(0x570a_e014, accounts);
}
#[test]
fn mutated_accounts_goldens_never_panic() {
    mutated("accounts", 0x570b_e014, accounts);
}

fn tasks(bytes: &[u8]) {
    use jig_protocol_store::tasks_v9;
    if let Ok(row) = tasks_v9::Row::decode(&tasks_v9::CEILINGS, &mut Reader::new(bytes)) {
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("measured length"));
        row.encode(&mut writer).expect("measured room");
        assert_eq!(writer.finish().as_ref(), bytes);
    }
}
#[test]
fn arbitrary_tasks_rows_never_panic_and_round_trip_when_they_decode() {
    arbitrary(0x5705_e014, tasks);
}
#[test]
fn mutated_tasks_goldens_never_panic() {
    mutated("tasks", 0x5706_e014, tasks);
}

fn people(bytes: &[u8]) {
    if let Ok(row) = people_v13::Row::decode(&people_v13::CEILINGS, &mut Reader::new(bytes)) {
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("measured length"));
        row.encode(&mut writer).expect("measured room");
        assert_eq!(writer.finish().as_ref(), bytes);
    }
}

#[test]
fn arbitrary_people_rows_never_panic_and_round_trip_when_they_decode() {
    arbitrary(0x5701_e019, people);
}
#[test]
fn mutated_people_goldens_never_panic() {
    mutated("people", 0x5702_e019, people);
}

fn core(bytes: &[u8]) {
    use jig_protocol_store::core_v6;
    if let Ok(row) = core_v6::Row::decode(&core_v6::CEILINGS, &mut Reader::new(bytes)) {
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("measured length"));
        row.encode(&mut writer).expect("measured room");
        assert_eq!(writer.finish().as_ref(), bytes);
    }
}
#[test]
fn arbitrary_core_rows_never_panic_and_round_trip_when_they_decode() {
    arbitrary(0x5703_e019, core);
}
#[test]
fn mutated_core_goldens_never_panic() {
    mutated("core", 0x5704_e019, core);
}

fn frames(bytes: &[u8]) {
    use jig_protocol_store::{codec, core_v6, notes_v1, tasks_v9, transcripts::frame};
    let limits = codec::Limits {
        settings_domain: jig_protocol_config::profile(jig_protocol_config::Profile::Small).core,
        settings: jig_protocol_store::settings_v1::CEILINGS,
        core: core_v6::CEILINGS,
        tasks: tasks_v9::CEILINGS,
        people: people_v13::CEILINGS,
        notes: notes_v1::CEILINGS,
    };
    if let Ok(turn) = frame::decode(bytes, &limits) {
        assert_eq!(frame::encode(&turn, &limits).expect("decoded frame fits").as_ref(), bytes);
    }
}
#[test]
fn arbitrary_frames_never_panic_and_round_trip_when_they_decode() {
    arbitrary(0x5701_e017, frames);
}
#[test]
fn mutated_and_short_frames_never_panic() {
    use jig_protocol_store::{codec, core_v6, notes_v1, tasks_v9, transcripts::frame};
    let limits = codec::Limits {
        settings_domain: jig_protocol_config::profile(jig_protocol_config::Profile::Small).core,
        settings: jig_protocol_store::settings_v1::CEILINGS,
        core: core_v6::CEILINGS,
        tasks: tasks_v9::CEILINGS,
        people: people_v13::CEILINGS,
        notes: notes_v1::CEILINGS,
    };
    let turn = core_v6::TurnRecord::new(
        &limits.core,
        core_v6::TurnRecordParts {
            opens_window: true,
            task: 11,
            attempt: 2,
            turn: 1,
            spent: 3,
            read: Some(4),
            at: 5,
            transcript: Box::from(b"turn bytes".as_slice()),
        },
    )
    .expect("bounded turn");
    let mut writer = Writer::new(usize::try_from(turn.measure()).expect("body size"));
    turn.encode(&mut writer).expect("measured body");
    let codec = codec::core::Limits { core: limits.core, tasks: limits.tasks, notes: limits.notes };
    let turn = codec::core::read_turn(&writer.finish(), &codec).expect("valid body");
    let bytes = frame::encode(&turn, &limits).expect("valid frame");
    frames(&bytes);
    for index in 0..bytes.len() {
        let mut changed = bytes.clone();
        *changed.get_mut(index).expect("chosen byte") ^= 1;
        frames(&changed);
        frames(bytes.get(..index).expect("short frame"));
    }
}

fn test_connector(bytes: &[u8]) {
    use jig_test_store::v2;
    if let Ok(row) = v2::Row::decode(&v2::CEILINGS, &mut Reader::new(bytes)) {
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("measured length"));
        row.encode(&mut writer).expect("measured room");
        assert_eq!(writer.finish().as_ref(), bytes);
    }
}
#[test]
fn arbitrary_test_connector_rows_never_panic_and_round_trip_when_they_decode() {
    arbitrary(0x5701_e018, test_connector);
}

#[test]
fn mutated_test_connector_goldens_never_panic() {
    let root = skein_scratch::paths::workspace().join("testing/jig-test-store/golden");
    mutate_goldens(&root, 0x5702_e018, test_connector);
}

fn settings(bytes: &[u8]) {
    use jig_protocol_store::settings_v1 as wire;
    if let Ok(row) = wire::Default::decode(&wire::CEILINGS, &mut Reader::new(bytes)) {
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("settings record length"));
        row.encode(&mut writer).expect("measured room");
        assert_eq!(writer.finish().as_ref(), bytes);
    }
    if let Ok(row) = wire::Changed::decode(&wire::CEILINGS, &mut Reader::new(bytes)) {
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("settings record length"));
        row.encode(&mut writer).expect("measured room");
        assert_eq!(writer.finish().as_ref(), bytes);
    }
    if let Ok(row) = wire::Revision::decode(&wire::CEILINGS, &mut Reader::new(bytes)) {
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("settings record length"));
        row.encode(&mut writer).expect("measured room");
        assert_eq!(writer.finish().as_ref(), bytes);
    }
    if let Ok(row) = wire::Extent::decode(&wire::CEILINGS, &mut Reader::new(bytes)) {
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("settings record length"));
        row.encode(&mut writer).expect("measured room");
        assert_eq!(writer.finish().as_ref(), bytes);
    }
    if let Ok(row) = wire::CharterVersion::decode(&wire::CEILINGS, &mut Reader::new(bytes)) {
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("settings record length"));
        row.encode(&mut writer).expect("measured room");
        assert_eq!(writer.finish().as_ref(), bytes);
    }
    if let Ok(row) = wire::Proposal::decode(&wire::CEILINGS, &mut Reader::new(bytes)) {
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("settings record length"));
        row.encode(&mut writer).expect("measured room");
        assert_eq!(writer.finish().as_ref(), bytes);
    }
}
#[test]
fn arbitrary_and_mutated_settings_records_never_panic_and_round_trip() {
    arbitrary(0x57aa_0191, settings);
    mutated("settings", 0x57aa_0192, settings);
}
