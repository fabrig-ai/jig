use super::fixtures;
use crate::translate;
use jig_client_protocol_wire::v19 as w;
fn me_preserves_two_projects_in_its_wire_record() {
    let limits = fixtures::wire_limits();
    assert_eq!(
        translate::owner::frame(&fixtures::frame_two_projects(), &[], &limits),
        Ok(fixtures::frame_two_projects_record())
    );
}
const PROJECT_ROOM: [u32; 2] = [2, 32];
#[test]
fn me_encoding_preserves_rows_despite_spare_list_capacity() {
    let limits = fixtures::wire_limits();
    for project_room in PROJECT_ROOM {
        let mut frame = fixtures::frame_two_projects();
        let mut projects = skein_lib::List::with_capacity(project_room);
        for project in frame.projects.into_boxed() {
            projects.push(project).expect("fixture project room");
        }
        frame.projects = projects;
        assert_eq!(
            translate::owner::frame(&frame, &[], &limits),
            Ok(fixtures::frame_two_projects_record()),
            "projects room {project_room}"
        );
    }
}
fn a_task_with_a_hold_becomes_its_record() {
    let limits = fixtures::wire_limits();
    assert_eq!(
        translate::task::document(&fixtures::task_with_hold(), &[], &limits),
        Ok(jig_client_test_records::task_with_hold_record())
    );
}
fn a_chip_in_each_phase_becomes_its_record() {
    let limits = fixtures::wire_limits();
    let phases = fixtures::phases();
    let records = jig_client_test_records::owner_chip_records();
    let source = fixtures::chip();
    for ((phase, _compact), record) in phases.into_iter().zip(records) {
        let mut chip = source.clone();
        chip.phase = phase;
        assert_eq!(translate::chips::chip(&chip, &limits), Ok(record));
    }
}
#[test]
fn me_overflow_reports_the_wire_bound_it_exceeds() {
    let frame = fixtures::frame_two_projects();
    let limits = fixtures::wire_limits();
    let rows = [
        (w::Limits { me_projects: 1, ..limits }, w::Path::MeProjects),
        (w::Limits { document_text_bytes: 1, ..limits }, w::Path::DocumentTextBytes),
    ];
    for (limits, path) in rows {
        assert_eq!(
            translate::owner::frame(&frame, &[], &limits),
            Err(w::Problem { path, reason: skein_codec::Reason::Bound })
        );
    }
}

#[test]
fn a_non_utf8_document_text_fails_translation() {
    let limits = fixtures::wire_limits();
    for bytes in [&[0xc3_u8][..], &[255_u8, b'/'][..]] {
        let value = jig_domain_core::documents::Text { bytes: bytes.into(), cut: true };
        assert_eq!(
            translate::owner::text(&value, &limits),
            Err(w::Problem { path: w::Path::DocumentTextBytes, reason: skein_codec::Reason::Utf8 })
        );
    }
}

fn a_run_writing_with_a_call_becomes_its_run_state() {
    let limits = fixtures::wire_limits();
    let source = fixtures::run_writing();
    assert_eq!(translate::live::run(&source, &limits), Ok(jig_client_test_records::run_writing_record()));
}
fn a_run_text_keeps_its_turn_retry_and_offsets() {
    let limits = fixtures::wire_limits();
    assert_eq!(translate::live::writing(&fixtures::writing(), &limits), Ok(jig_client_test_records::writing_record()));
}
fn an_inbox_with_both_groups_becomes_its_record() {
    let limits = fixtures::wire_limits();
    assert_eq!(
        translate::inbox::document(&fixtures::inbox_document(), &limits),
        Ok(jig_client_test_records::inbox_both_groups_record())
    );
}
fn a_deployment_document_becomes_its_record() {
    let limits = fixtures::wire_limits();
    assert_eq!(
        translate::deployment::document(&fixtures::deployment_document(), &limits),
        Ok(jig_client_test_records::deployment_rows_record())
    );
}
fn a_lines_document_becomes_its_tasks_rows_record() {
    let limits = fixtures::wire_limits();
    assert_eq!(
        translate::queries::lines(&fixtures::tasks_document(), &limits),
        Ok(jig_client_test_records::tasks_rows_record())
    );
}
fn a_tree_keeps_its_rows_folds_dependencies_and_opaque_cursor() {
    let limits = fixtures::wire_limits();
    assert_eq!(
        translate::live::tree(&fixtures::tree_document(), Some(jig_client_test_records::live_cursor_record()), &limits),
        Ok(jig_client_test_records::tree_rows_record())
    );
}
#[test]
fn live_nested_pieces_past_the_wire_bounds_are_refused_naming_them() {
    let source = fixtures::deployment_document();
    let limits = fixtures::wire_limits();
    let rows = [
        ("host runs", w::Limits { owner_host_runs: 0, ..limits }, w::Path::OwnerHostRuns),
        ("deployment hosts", w::Limits { owner_deployment_hosts: 0, ..limits }, w::Path::OwnerDeploymentHosts),
        ("deployment attempts", w::Limits { owner_deployment_attempts: 0, ..limits }, w::Path::OwnerDeploymentAttempts),
        ("account runs", w::Limits { owner_account_runs: 0, ..limits }, w::Path::OwnerAccountRuns),
        ("account waiting", w::Limits { owner_account_waiting: 0, ..limits }, w::Path::OwnerAccountWaiting),
        ("deployment accounts", w::Limits { owner_deployment_accounts: 0, ..limits }, w::Path::OwnerDeploymentAccounts),
    ];
    for (label, limits, path) in rows {
        assert_eq!(
            translate::deployment::document(&source, &limits),
            Err(w::Problem { path, reason: skein_codec::Reason::Bound }),
            "{label}"
        );
    }
}

#[test]
fn documents_become_their_literal_records() {
    table([
        ("me_preserves_two_projects_in_its_wire_record", me_preserves_two_projects_in_its_wire_record),
        ("a_task_with_a_hold_becomes_its_record", a_task_with_a_hold_becomes_its_record),
        ("a_chip_in_each_phase_becomes_its_record", a_chip_in_each_phase_becomes_its_record),
        ("a_run_writing_with_a_call_becomes_its_run_state", a_run_writing_with_a_call_becomes_its_run_state),
        ("a_run_text_keeps_its_turn_retry_and_offsets", a_run_text_keeps_its_turn_retry_and_offsets),
        ("an_inbox_with_both_groups_becomes_its_record", an_inbox_with_both_groups_becomes_its_record),
        ("a_deployment_document_becomes_its_record", a_deployment_document_becomes_its_record),
        ("a_lines_document_becomes_its_tasks_rows_record", a_lines_document_becomes_its_tasks_rows_record),
        (
            "a_tree_keeps_its_rows_folds_dependencies_and_opaque_cursor",
            a_tree_keeps_its_rows_folds_dependencies_and_opaque_cursor,
        ),
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

const TEMPLATE: u32 = 2;
const POOL_LEFT: u64 = 70;
const BUDGET: u64 = 50;
const TEMPLATE_CHARTER: u32 = 1;
const TEMPLATE_HOLDER_ROLE: u32 = 0;
#[test]
fn a_me_document_carries_each_project_s_template() {
    let mut frame = fixtures::frame_two_projects();
    for index in 0..frame.projects.len() {
        let project = frame.projects.get_mut(index).unwrap();
        let mut template = fixtures::template();
        template.number = TEMPLATE;
        template.budget = jig_domain_people::TemplateBudget::Amount(42);
        template.label = 5;
        project.templates = fixtures::list([template]);
    }
    let record = translate::owner::frame(&frame, &[], &w::CEILINGS).unwrap();
    let mut reader = skein_lib::Reader::new(record.projects());
    for source in &frame.projects {
        let bytes = skein_codec::piece::split(&mut reader).unwrap().unwrap();
        let project = w::MeProject::decode(&w::CEILINGS, &mut skein_lib::Reader::new(&bytes)).unwrap();
        assert_eq!(project.project(), source.project);
        let mut templates = skein_lib::Reader::new(project.templates());
        let bytes = skein_codec::piece::split(&mut templates).unwrap().unwrap();
        let template = w::Template::decode(&w::CEILINGS, &mut skein_lib::Reader::new(&bytes)).unwrap();
        assert_eq!(template.number(), TEMPLATE);
        assert_eq!(template.label(), 5);
        let w::TemplateBudget::Amount(budget) = template.budget() else { panic!("amount") };
        assert_eq!(budget.value(), 42);
    }
    assert_eq!(skein_codec::piece::split(&mut reader), Ok(None));
}
#[test]
fn a_me_document_carries_each_project_s_creating_bounds() {
    let frame = frame_with_creating_bounds();
    let record = translate::owner::frame(&frame, &[], &w::CEILINGS).unwrap();
    let mut reader = skein_lib::Reader::new(record.projects());
    for source in &frame.projects {
        let bytes = skein_codec::piece::split(&mut reader).unwrap().unwrap();
        let project = w::MeProject::decode(&w::CEILINGS, &mut skein_lib::Reader::new(&bytes)).unwrap();
        assert_eq!(project.project(), source.project);
        let creating = project.creating().as_ref().unwrap();
        assert_eq!(creating.within().budget(), BUDGET);
        assert_eq!(creating.within().pool_left(), POOL_LEFT);
        let w::Beyond::Proposed(beyond) = creating.beyond() else { panic!("proposed") };
        let w::Recipient::Role(to) = beyond.to() else { panic!("role") };
        assert_eq!(to.project(), source.project);
        assert_eq!(to.role(), TEMPLATE_HOLDER_ROLE);
    }
    assert_eq!(skein_codec::piece::split(&mut reader), Ok(None));
}
#[test]
fn a_project_document_carries_its_policy_s_template_overrides() {
    let mut project = fixtures::project_document();
    project.policy.templates = fixtures::list([jig_domain_people::TemplateOverride {
        template: TEMPLATE,
        executor: None,
        budget: Some(jig_domain_people::TemplateBudget::Amount(42)),
        deadline: None,
        priority: None,
    }]);
    let record = translate::objects::project(&fixtures::models(), &project, &w::CEILINGS).unwrap();
    let entry = record.policy().templates().get(0).unwrap();
    assert_eq!(entry.template(), TEMPLATE);
    assert_eq!(
        entry.budget(),
        &Some(w::TemplateBudget::Amount(
            w::TemplateAmount::new(&w::CEILINGS, w::TemplateAmountParts { value: 42 }).unwrap()
        ))
    );
}

fn frame_with_creating_bounds() -> jig_domain_core::documents::Frame {
    use jig_domain_core::acts;
    let mut frame = fixtures::frame_two_projects();
    for index in 0..frame.projects.len() {
        let project = frame.projects.get_mut(index).unwrap();
        project.creating = Some(acts::Creating {
            within: acts::Bounds::Create {
                authority: Box::new(fixtures::authority()),
                pool_left: POOL_LEFT,
                charters: fixtures::list([TEMPLATE_CHARTER]),
                procedures: skein_lib::List::with_capacity(0),
                latest: None,
                budget: BUDGET,
                maximum_budget: 100,
                maximum_deadline: None,
            },
            beyond: acts::Beyond::Proposed {
                to: acts::Recipient::Role { project: project.project, role: TEMPLATE_HOLDER_ROLE },
            },
        });
    }
    frame
}

const CREATION_CAPACITY: [(jig_domain_people::CapacityAction, w::OwnerPersonCapacityAction); 2] = [
    (jig_domain_people::CapacityAction::Create, w::OwnerPersonCapacityAction::Create),
    (jig_domain_people::CapacityAction::Compound, w::OwnerPersonCapacityAction::Compound),
];
#[test]
fn creating_and_compound_capacity_refusals_keep_the_operations_kind() {
    let deployment = fixtures::deployment_document();
    let mut refusal = *deployment.person_room_refusals.get(0).unwrap();
    for (action, expected) in CREATION_CAPACITY {
        refusal.action = action;
        let row = translate::deployment::person_refusal(&refusal, &fixtures::wire_limits()).unwrap();
        assert_eq!(row.action(), &expected);
        assert_eq!(row.key().person(), refusal.key.person);
        assert_eq!(row.key().key(), &refusal.key.key);
    }
}

#[test]
fn added_dependencies_and_kept_waits_cross_both_client_translations() {
    use jig_client_domain::contract as c;
    use jig_domain_core::documents as d;
    use jig_domain_tasks as t;
    use skein_lib::{List, Reader, Wall, Writer};
    let limits = fixtures::wire_limits();
    let mut task = fixtures::task_with_hold();
    let mut facts = List::with_capacity(2);
    facts
        .push(d::Dependency {
            number: 7,
            pending: true,
            added: Some(t::Added { task: 7, at: Wall::from_nanos(11), by: t::Party::Person(13) }),
            keeps: true,
            status: None,
        })
        .unwrap();
    facts
        .push(d::Dependency {
            number: 9,
            pending: false,
            added: Some(t::Added { task: 9, at: Wall::from_nanos(17), by: t::Party::Task(19) }),
            keeps: true,
            status: Some(t::Status::Done),
        })
        .unwrap();
    task.dependency_facts = facts.clone();
    task.chip.details.dependencies = facts;
    let mut pending = List::with_capacity(1);
    pending.push(7).unwrap();
    task.waits_on = d::WaitsOn::Dependencies { pending, count: 1 };
    let record = translate::task::document(&task, &[], &limits).unwrap();
    let mut out = Writer::new(usize::try_from(record.measure()).unwrap());
    record.encode(&mut out).unwrap();
    let bytes = out.finish();
    let record = w::Task::decode(&limits, &mut Reader::new(&bytes)).unwrap();
    let actual = jig_client_protocol_link::translate::documents::task(record, &fixtures::client_limits()).unwrap();
    let first = actual.dependency_facts.get(0).unwrap();
    assert_eq!((first.number, first.pending, first.keeps), (7, true, true));
    assert_eq!(first.added, Some(c::DependencyAdded { at: Wall::from_nanos(11), by: c::OwnerRequester::Person(13) }));
    let second = actual.dependency_facts.get(1).unwrap();
    assert_eq!(second.status, Some(c::OwnerTaskStatus::Done));
    assert_eq!(second.added.unwrap().by, c::OwnerRequester::Task(19));
    let c::WaitsOn::Dependencies { pending, count } = actual.waits_on else { panic!("kept wait") };
    assert_eq!((pending.as_slice(), count), ([7].as_slice(), 1));
    let c::ChipPhase::Owner { details, .. } = actual.chip.details.as_ref() else { panic!("owner chip") };
    assert_eq!(details.dependencies, actual.dependency_facts);
}

#[test]
fn a_circle_hold_keeps_its_order_and_cut_through_the_client() {
    use jig_client_domain::contract as c;
    use jig_domain_tasks as t;
    let mut task = fixtures::task_with_hold();
    let hold = task.hold.as_mut().unwrap();
    hold.reason = t::Hold::Circle;
    hold.circle = Some(t::Circle { tasks: Box::new([9, 7]), cut: true });
    let record = translate::task::document(&task, &[], &fixtures::wire_limits()).unwrap();
    let actual = jig_client_protocol_link::translate::documents::task(record, &fixtures::client_limits()).unwrap();
    let Some(c::TaskHold::Owner(hold)) = actual.hold else { panic!("owner hold") };
    assert_eq!(hold.reason, c::HoldCategory::Circle);
    let circle = hold.circle.unwrap();
    assert_eq!(circle.tasks.as_slice(), &[9, 7]);
    assert!(circle.cut);
}

#[test]
fn task_settings_revisions_cross_into_the_wire_document() {
    use jig_domain_core::documents::TaskSettings;
    let limits = fixtures::wire_limits();
    let mut source = fixtures::task_with_hold();
    for settings in [
        None,
        Some(TaskSettings { task: 7, run: None, newest: 9 }),
        Some(TaskSettings { task: 7, run: Some(6), newest: 9 }),
    ] {
        source.settings = settings;
        let record = translate::task::document(&source, &[], &limits).expect("task document");
        match (record.settings(), settings) {
            (Some(actual), Some(expected)) => {
                assert_eq!(actual.task(), expected.task);
                assert_eq!(*actual.run(), expected.run);
                assert_eq!(actual.newest(), expected.newest);
            }
            (None, None) => (),
            (Some(_), None) | (None, Some(_)) => panic!("settings presence"),
        }
    }
}
