use alloc::boxed::Box;
use jig_client_protocol_wire::v19 as w;
use jig_domain_core::documents as d;
use jig_domain_tasks as t;
use skein_lib::{Duration, List, Wall};
pub fn wire_limits() -> w::Limits {
    jig_client_test_records::limits()
}
pub fn text() -> d::Text {
    d::Text { bytes: Box::from([]), cut: false }
}
pub fn frame_two_projects() -> d::Frame {
    let mut projects = List::with_capacity(2);
    for (project, role) in [(3, jig_domain_people::Role::Owner), (5, jig_domain_people::Role::Observer)] {
        projects
            .push(d::FrameProjectRole {
                project,
                role,
                name: jig_domain_core::documents::Text { bytes: Box::from(&b"Tally"[..]), cut: false },
                pool: None,
                creating: None,
                models: jig_domain_authority::Models::NONE,
                templates: List::with_capacity(0),
            })
            .unwrap();
    }
    d::Frame {
        providers: List::with_capacity(0),
        providers_cut: false,
        key_bytes: 256,
        redirect_bytes: 4096,
        party: 7,
        kind: jig_domain_people::Kind::Person,
        login: d::Text { bytes: Box::from("é".as_bytes()), cut: true },
        name: text(),
        deployment_role: None,
        projects,
        needs: 7,
        need_counts: jig_domain_people::Needs { questions: 7, proposals: 7, escalations: 7, person_tasks: 7 },
        updates: d::Count::Known(7),
        results_seen: 7,
        retention: Duration::from_nanos(19),
        cut: false,
    }
}
pub fn wake() -> t::WakePolicy {
    t::WakePolicy {
        words: t::WakeRule::Never,
        notices: t::WakeRule::Never,
        news: t::WakeRule::Never,
        results: t::ResultsWake::Never,
        questions: false,
        answers: false,
        timers: false,
    }
}
pub fn budget() -> d::BudgetLine {
    d::BudgetLine {
        numbers: t::Numbers { budget: 7, spent: 7, spent_below: 7, reserved: 7 },
        left: 7,
        funder: t::Funder::Task(7),
        deadline: None,
    }
}
pub fn chip() -> d::Chip {
    d::Chip {
        number: 7,
        title: text(),
        executor: d::ExecutorKind::Agent,
        phase: d::PhaseLine::Waiting { dependencies: List::with_capacity(0), more: false, holds: false },
        details: Box::new(d::ChipDetails {
            dependencies: List::with_capacity(0),
            project: 7,
            executor: t::Executor::Agent { charter: 7 },
            phase_since: Wall::from_nanos(7),
            run_since: None,
            attempt: None,
            retry: None,
            wake: wake(),
            person_wait: None,
            hold: None,
            budget: budget(),
            activity: 7,
            resources: List::with_capacity(0),
            resources_cut: false,
            resources_observed: false,
            procedure: None,
            live_delegates: 7,
            closing_connectors: None,
            ending: None,
            ended_at: None,
            observation: None,
        }),
    }
}
pub fn phases() -> [(d::PhaseLine, w::Phase); 6] {
    [
        (d::PhaseLine::Waiting { dependencies: List::with_capacity(0), more: false, holds: false }, w::Phase::Waiting),
        (d::PhaseLine::Active(t::Active::Idle), w::Phase::Idle),
        (
            d::PhaseLine::Person {
                address: t::PersonAddress::Role(17),
                taken_by: Some(19),
                since: Some(Wall::from_nanos(23)),
            },
            w::Phase::Active,
        ),
        (d::PhaseLine::Closing(t::Stage::Effects), w::Phase::Closing),
        (d::PhaseLine::Held { reason: t::Hold::Uncertain { entry: 29 }, revision: 31 }, w::Phase::Held),
        (d::PhaseLine::Ended, w::Phase::Ended),
    ]
}
pub fn authority() -> d::AuthorityLine {
    d::AuthorityLine {
        models: t::Models::ALL,
        tools: t::Tools(7),
        grants: List::with_capacity(0),
        executors: List::with_capacity(0),
        tasks: 7,
        depth: 7,
        ceiling: t::Budget { spend: 7, deadline: None },
        notes: t::Scopes(7),
        note_resources: List::with_capacity(0),
    }
}
pub fn hold() -> d::HoldLine {
    d::HoldLine {
        placed: None,
        circle: None,
        account: None,
        source: None,
        reason: t::Hold::Uncertain { entry: 29 },
        revision: 31,
        before: d::PriorPhase::Waiting,
        escalation: None,
        tries: t::Tries { transient: 2, permanent: 3, run: 5, agent: 7, lost: 11, invalid: 13 },
        hold_wait_since: Some(Wall::from_nanos(17)),
    }
}
pub fn task_with_hold() -> d::Task {
    d::Task {
        dependency_facts: List::with_capacity(0),
        kept_account: None,
        run_wait: None,
        settings: Some(d::TaskSettings { task: 1, run: None, newest: 1 }),
        waits_on: d::WaitsOn::Nothing,
        label: 0,
        flags: t::Flags::PLAIN,
        last_activity: Wall::EPOCH,
        chip: chip(),
        acts: List::with_capacity(0),
        project: 7,
        goal: None,
        priority: None,
        executor: t::Executor::Agent { charter: 7 },
        requester: t::Party::Task(7),
        lineage: List::with_capacity(0),
        delegates: List::with_capacity(0),
        changed: Wall::from_nanos(7),
        spec_revision: 7,
        person_revision: None,
        taken_by: None,
        activity: 7,
        spec: d::SpecLine {
            words: text(),
            parameters: List::with_capacity(0),
            inputs: List::with_capacity(0),
            cut: false,
        },
        words: text(),
        parameters: List::with_capacity(0),
        instructions: None,
        llm: None,
        resources: List::with_capacity(0),
        inputs: List::with_capacity(0),
        dependencies: List::with_capacity(0),
        references: List::with_capacity(0),
        subscriptions: List::with_capacity(0),
        wake: wake(),
        contract: d::ContractLine::Approval {
            state: Box::from([]),
            part: t::Part { connector: 7, kind: t::PartKind::Resource, key: t::PartKey::Path(List::with_capacity(0)) },
        },
        result: None,
        budget: budget(),
        authority: authority(),
        hold: Some(hold()),
        proposals: List::with_capacity(0),
        cut: false,
    }
}

/// The shared v19 fixture with each engine-supplied project's required name.
pub fn frame_two_projects_record() -> jig_client_protocol_wire::v19::Me {
    let limits = w::CEILINGS;
    let mut parts = jig_client_test_records::frame_two_projects_record().into_parts();
    let mut reader = skein_lib::Reader::new(&parts.projects);
    let mut projects = List::with_capacity(2);
    let mut size = 0_usize;
    while let Some(bytes) = skein_codec::piece::split(&mut reader).expect("fixture pieces") {
        let project = w::MeProject::decode(&limits, &mut skein_lib::Reader::new(&bytes)).expect("fixture project");
        let mut project = project.into_parts();
        project.name =
            w::DocumentText::new(&limits, w::DocumentTextParts { bytes: Box::from(&b"Tally"[..]), cut: false })
                .expect("literal name");
        let project = w::MeProject::new(&limits, project).expect("named project");
        size = size
            .checked_add(usize::try_from(project.measure()).expect("fixture size"))
            .expect("fixture size")
            .checked_add(size_of::<u32>())
            .expect("piece header");
        projects.push(project).expect("two fixture projects");
    }
    let mut writer = skein_lib::Writer::new(size);
    for project in projects.into_boxed() {
        let mut encoded = skein_lib::Writer::new(usize::try_from(project.measure()).expect("fixture size"));
        project.encode(&mut encoded).expect("measured project");
        skein_codec::piece::write(&mut writer, &encoded.finish()).expect("measured piece");
    }
    parts.projects = writer.finish();
    w::Me::new(&limits, parts).expect("named frame fixture")
}
