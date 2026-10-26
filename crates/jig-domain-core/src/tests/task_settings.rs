//! Settings revision facts in live and retained task documents.
use super::*;

#[test]
fn an_agent_task_document_carries_its_settings_revision() {
    let (core, limits) = make_tree();
    let row = core.tasks.task(2).unwrap();
    let task = task_from_row(&core, &limits, 1, row, Wall::EPOCH).unwrap();
    assert_eq!(task.settings, Some(TaskSettings { task: 1, run: None, newest: 1 }));
}

#[test]
fn a_task_document_names_only_a_different_live_run_revision() {
    let (mut core, limits) = make_tree();
    // The original live rows and the synthetic document's revisions still
    // read the unchanged charter after the newest head advances.
    for revision in [1, 6, 7] {
        let head = settings::Head { number: revision, against: revision - 1, ..core.current.head.clone() };
        core.held.heads.insert(revision, head).unwrap();
    }
    core.current.head.number = 9;
    let mut row = core.tasks.task(2).unwrap().clone();
    row.settings.task = 7;
    for (run, expected) in [(None, None), (Some(7), None), (Some(6), Some(6))] {
        row.settings.run = run;
        let task = task_from_row(&core, &limits, 1, &row, Wall::EPOCH).unwrap();
        assert_eq!(task.settings, Some(TaskSettings { task: 7, run: expected, newest: 9 }));
    }
}

#[test]
fn an_ended_agent_task_document_keeps_its_settings_revision() {
    let (core, limits) = make_tree();
    let mut row = core.tasks.task(2).unwrap().clone();
    row.phase = tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Report { words: Box::new([]) }));
    let task = task_from_row(&core, &limits, 1, &row, Wall::EPOCH).unwrap();
    assert_eq!(task.settings, Some(TaskSettings { task: 1, run: None, newest: 1 }));
}

#[test]
fn procedure_and_person_task_documents_carry_no_settings_revision() {
    let (core, limits) = make_tree();
    let mut row = core.tasks.task(2).unwrap().clone();
    row.phase = tasks::Phase::Waiting;
    for executor in
        [tasks::Executor::Procedure { connector: 1, code: 1 }, tasks::Executor::Person(tasks::PersonAddress::Person(1))]
    {
        row.executor = executor;
        let task = task_from_row(&core, &limits, 1, &row, Wall::EPOCH).unwrap();
        assert_eq!(task.settings, None);
    }
}
