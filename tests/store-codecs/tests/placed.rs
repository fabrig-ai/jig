//! Placed hold storage keeps the active family's tags and bounds.
use jig_domain_tasks as tasks;
use jig_protocol_store::{codec, tasks_v9 as wire};
use skein_lib::{Reader, Writer};

#[test]
fn placed_hold_smallest_and_full_goldens_round_trip() {
    for bytes in [
        include_bytes!("../../../crates/jig-protocol-store/golden/tasks/record_placed_hold_smallest.bin").as_slice(),
        include_bytes!("../../../crates/jig-protocol-store/golden/tasks/record_placed_hold_full.bin").as_slice(),
    ] {
        let mut reader = Reader::new(bytes);
        let hold = wire::PlacedHold::decode(&wire::CEILINGS, &mut reader).expect("placed hold golden");
        assert!(reader.is_empty());
        let mut writer = Writer::new(hold.measure() as usize);
        hold.encode(&mut writer).expect("measured golden");
        assert_eq!(writer.finish().as_ref(), bytes);
    }
}

#[test]
fn hold_placed_round_trips_with_its_number_at_the_existing_tag() {
    for hold in [0, 1, u64::MAX] {
        let value = tasks::Hold::Placed { hold };
        let encoded = codec::tasks::phase::hold(&value, &wire::CEILINGS).expect("number");
        let mut writer = Writer::new(encoded.measure() as usize);
        encoded.encode(&mut writer).expect("measured hold");
        let bytes = writer.finish();
        assert_eq!(bytes[0], 4);
        let decoded = wire::Hold::decode(&wire::CEILINGS, &mut Reader::new(&bytes)).expect("hold");
        assert_eq!(codec::tasks::phase::read_hold(decoded, &wire::CEILINGS), Ok(value));
    }
}

#[test]
fn placed_hold_rows_round_trip_and_enforce_configured_reason_and_exemption_bounds() {
    let profile = jig_protocol_config::profile(jig_protocol_config::Profile::Small);
    let limits = codec::tasks::limits(&profile.core.tasks).expect("small bounds");
    let value = tasks::PlacedHold {
        number: 71,
        project: 1,
        owner: tasks::Owner::Party { person: 1 },
        mode: tasks::Mode::Stop,
        scope: tasks::Scope::Subtree { task: 1 },
        reason: vec![b'x'; profile.core.tasks.message_bytes as usize].into_boxed_slice(),
        placed_at: 1234,
        revision: 7,
        exempt: vec![3; profile.core.tasks.exemptions as usize].into_boxed_slice(),
        lifted: false,
    };
    for owner in [value.owner, tasks::Owner::Task { task: 1 }] {
        for (mode, scope, lifted) in
            [(tasks::Mode::Stop, value.scope, false), (tasks::Mode::Drain, tasks::Scope::Project, true)]
        {
            let row =
                tasks::Stored::PlacedHold(Box::new(tasks::PlacedHold { owner, mode, scope, lifted, ..value.clone() }));
            let bytes = codec::tasks::encode(&row, &limits, &[]).expect("configured hold");
            assert_eq!(codec::tasks::decode(&bytes, &limits, &[]), Ok(row));
        }
    }
    for (reason, exempt) in [
        (vec![0; profile.core.tasks.message_bytes as usize + 1], value.exempt.to_vec()),
        (value.reason.to_vec(), vec![3; profile.core.tasks.exemptions as usize + 1]),
    ] {
        let row = tasks::Stored::PlacedHold(Box::new(tasks::PlacedHold {
            reason: reason.into_boxed_slice(),
            exempt: exempt.into_boxed_slice(),
            ..value.clone()
        }));
        assert!(codec::tasks::encode(&row, &limits, &[]).is_err());
    }
}
