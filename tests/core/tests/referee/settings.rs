use super::*;

#[test]
fn the_referee_catches_a_task_made_on_an_older_revision() {
    let mut fake = Fake::new(Recovery::Keyed);
    let mut rows = snapshot();
    rows.newest = 1;
    let violation = fake.commit(1, rows).expect_err("new task retained revision zero");
    assert_eq!(violation.promise, Promise::Ownership);
    assert!(violation.why.contains("made or moved"));
}

#[test]
fn the_referee_catches_a_task_moved_to_an_older_revision() {
    let mut fake = Fake::ready(Recovery::Keyed);
    let mut rows = snapshot();
    rows.newest = 2;
    rows.tasks.get_mut(&1).expect("task").settings.task = 1;
    let violation = fake.commit(2, rows).expect_err("move skipped newest");
    assert_eq!(violation.promise, Promise::Ownership);
    assert!(violation.why.contains("made or moved"));
}

#[test]
fn the_referee_catches_an_assignment_on_another_revision() {
    let mut fake = Fake::ready(Recovery::Keyed);
    let mut assignment = assigned(10);
    let Observed::Assigned { settings, .. } = &mut assignment else { unreachable!() };
    *settings = 1;
    let violation = fake.send(assignment).expect_err("delivered revision differs from prepared run");
    assert_eq!(violation.promise, Promise::Ownership);
    assert!(violation.why.contains("prepared under"));
}

#[test]
fn moving_a_task_preserves_its_live_runs_revision_and_survives_restart() {
    let mut fake = Fake::ready(Recovery::Keyed);
    let mut rows = snapshot();
    rows.newest = 1;
    rows.tasks.get_mut(&1).expect("task").settings.task = 1;
    fake.commit(2, rows.clone()).expect("move to newest keeps run zero");
    fake.send(Observed::Restart).expect("external evidence survives restart");
    fake.send(assigned(10)).expect("live run still uses revision zero");
    rows.tasks.get_mut(&1).expect("task").settings.run = Some(1);
    fake.commit(3, rows).expect("next preparation uses moved revision");
    let mut assignment = assigned(10);
    let Observed::Assigned { settings, .. } = &mut assignment else { unreachable!() };
    *settings = 1;
    fake.send(assignment).expect("next prepared run uses revision one");
}
