//! Task settings revisions survive the complete task record's bytes.
use jig_client_protocol_wire::v19 as w;
use skein_lib::{Reader, Writer};

#[test]
fn task_settings_round_trip_with_and_without_a_run_revision() {
    for run in [None, Some(6)] {
        let mut parts = jig_client_test_records::task_record().into_parts();
        parts.settings = Some(
            w::TaskSettings::new(&w::CEILINGS, w::TaskSettingsParts { task: 7, run, newest: 9 }).expect("settings"),
        );
        let task = w::Task::new(&w::CEILINGS, parts).expect("task");
        let mut out = Writer::new(usize::try_from(task.measure()).expect("bounded task"));
        task.encode(&mut out).expect("encode task");
        let bytes = out.finish();
        let mut input = Reader::new(&bytes);
        assert_eq!(w::Task::decode(&w::CEILINGS, &mut input), Ok(task));
        assert!(input.is_empty());
    }
}
