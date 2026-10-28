//! A task's LLM choice on the wire reaches the core by configuration's indices, and a refused
//! model comes back by its name (contract.md, 3.2 and 4; domain/tasks.md, 3).
use super::{amendment_setup, final_answer, keyed, member_setup, owner_authority, route_real_core, w};
use jig_core_world::fixtures::live_row;
use jig_domain_tasks as tasks;
use jig_engine_clients_world::fixtures;
use skein_lib::{List, Writer};

/// A choice by name: a main model and effort, then each sub-agent's model and effort.
fn choice(main: (&[u8], Option<&[u8]>), subagents: &[(&[u8], Option<&[u8]>)]) -> w::LlmChoice {
    let limits = fixtures::limits().wire;
    let named = |(model, effort): (&[u8], Option<&[u8]>)| {
        w::ModelEffort::new(&limits, w::ModelEffortParts { model: model.into(), effort: effort.map(Box::from) })
            .expect("model and effort")
    };
    let mut items = List::with_capacity(u32::try_from(subagents.len()).expect("few sub-agents"));
    for item in subagents {
        items.push(named(*item)).expect("same capacity");
    }
    w::LlmChoice::new(&limits, w::LlmChoiceParts { account: None, main: named(main), subagents: items })
        .expect("choice")
}
/// A Create of one agent task in the member's project, run by the configured charter.
fn create(llm: w::LlmChoice) -> w::Ask {
    let limits = fixtures::limits().wire;
    let task = w::NewTask::new(
        &limits,
        w::NewTaskParts {
            deadline: w::NewDeadline::Named,
            executor: w::OwnerExecutor::Agent(
                w::OwnerExecutorAgent::new(&limits, w::OwnerExecutorAgentParts { charter: 1 }).expect("agent"),
            ),
            spec: w::Specification::new(
                &limits,
                w::SpecificationParts {
                    asks: false,
                    words: b"chosen".as_slice().into(),
                    parameters: List::with_capacity(0),
                    inputs: List::with_capacity(0),
                },
            )
            .expect("spec"),
            contract: w::NewContract::Report(
                w::ContractReport::new(&limits, w::ContractReportParts { words: 16 }).expect("report"),
            ),
            authority: owner_authority(20),
            budget: w::NewBudget::Named,
            wake: None,
            dependencies: List::with_capacity(0),
            named: Box::new([]),
            tracked: None,
            label: 0,
            flags: w::TaskFlags::new(
                &limits,
                w::TaskFlagsParts {
                    ended_by: w::EndedBy::Result,
                    conversation: false,
                    listed: false,
                    retention: w::Retention::Kept,
                },
            )
            .expect("flags"),
            llm: Some(llm),
        },
    )
    .expect("task");
    let size = usize::try_from(task.measure()).expect("task size");
    let mut out = Writer::new(size.checked_add(4).expect("piece size"));
    out.put(&u32::try_from(size).expect("task size").to_be_bytes()).expect("piece length");
    task.encode(&mut out).expect("task bytes");
    w::Ask::Create(w::Create::new(&limits, w::CreateParts { project: 1, tasks: out.finish() }).expect("create"))
}
/// An Amend of task 2 that changes only its LLM choice.
fn amend(revision: u64, llm: w::LlmChoice) -> w::Ask {
    let limits = fixtures::limits().wire;
    let change = w::Amendment::new(
        &limits,
        w::AmendmentParts {
            addressed: None,
            spec: None,
            wake: None,
            dependencies: None,
            authority: None,
            instructions: None,
            tracked: None,
            llm: Some(llm),
        },
    )
    .expect("change");
    w::Ask::Amend(Box::new(
        w::Amend::new(
            &limits,
            w::AmendParts {
                project: 1,
                task: 2,
                revision: Some(w::Revision::new(&limits, w::RevisionParts { value: revision }).expect("revision")),
                change,
                reason: b"think harder".as_slice().into(),
            },
        )
        .expect("amend"),
    ))
}
fn refused_llm(answer: &w::Final) -> (&[u8], w::LlmWhy) {
    let w::Final::Refused(refused) = answer else { panic!("refused: {answer:?}") };
    let w::Refusal::Llm(row) = refused.refusal() else { panic!("an LLM refusal: {:?}", refused.refusal()) };
    (row.model(), row.why().clone())
}
const SCRIPTED: u8 = 0;
const LOW: u8 = 0;
const HIGH: u8 = 1;

#[test]
fn a_create_naming_a_model_and_its_effort_keeps_them_by_configuration_s_indices() {
    let (mut peer, mut world, bearer) = member_setup(360);
    let llm = choice((b"scripted", Some(b"high")), &[(b"scripted", Some(b"low"))]);
    let socket = peer.post(b"/jig/v1/request", keyed([62; 16], &create(llm)), Some(&bearer));
    assert_eq!(route_real_core(&mut peer, &mut world), [[62; 16]], "core reply: {:?}", world.people_answers.last());
    let w::Final::Done(done) = final_answer(&peer, socket) else { panic!("done") };
    let w::Outcome::Owner(owner) = done.outcome() else { panic!("owner outcome") };
    let w::OwnerOutcome::Created(created) = owner.value() else { panic!("created: {:?}", owner.value()) };
    assert_eq!(
        live_row(&world, created.first()).llm,
        Some(tasks::LlmChoice {
            account: None,
            main: tasks::ModelEffort { model: SCRIPTED, effort: Some(HIGH) },
            subagents: Box::new([tasks::ModelEffort { model: SCRIPTED, effort: Some(LOW) }]),
        })
    );
}
#[test]
fn a_model_or_an_effort_configuration_lacks_is_refused_by_its_name_before_the_core() {
    let (mut peer, mut world, bearer) = member_setup(361);
    for (llm, model, why) in [
        (choice((b"unheard", None), &[]), b"unheard".as_slice(), w::LlmWhy::Unconfigured),
        (choice((b"scripted", Some(b"medium")), &[]), b"scripted".as_slice(), w::LlmWhy::Effort),
    ] {
        let socket = peer.post(b"/jig/v1/request", keyed([63; 16], &create(llm)), Some(&bearer));
        assert_eq!(route_real_core(&mut peer, &mut world), Vec::<[u8; 16]>::new(), "no ask reaches the core");
        assert_eq!(refused_llm(&final_answer(&peer, socket)), (model, why));
    }
}
#[test]
fn a_model_the_core_refuses_comes_back_by_its_name() {
    let (mut peer, mut world, bearer) = member_setup(362);
    let llm = choice((b"scripted", Some(b"low")), &[(b"scripted", Some(b"low")), (b"scripted", Some(b"high"))]);
    let socket = peer.post(b"/jig/v1/request", keyed([64; 16], &create(llm)), Some(&bearer));
    assert_eq!(route_real_core(&mut peer, &mut world), [[64; 16]]);
    assert_eq!(refused_llm(&final_answer(&peer, socket)), (b"scripted".as_slice(), w::LlmWhy::Repeated));
}
#[test]
fn an_amend_naming_a_choice_changes_the_one_the_task_s_next_run_takes() {
    let (mut peer, mut world, bearer) = amendment_setup(363);
    let before = live_row(&world, 2);
    let default = tasks::LlmChoice {
        account: None,
        main: tasks::ModelEffort { model: SCRIPTED, effort: Some(LOW) },
        subagents: Box::new([]),
    };
    assert_eq!(before.llm, Some(default), "the charter's default, recorded at creation");
    let revision = before.revisions.spec;
    let llm = choice((b"scripted", Some(b"high")), &[]);
    let socket = peer.post(b"/jig/v1/request", keyed([65; 16], &amend(revision, llm)), Some(&bearer));
    assert_eq!(route_real_core(&mut peer, &mut world), [[65; 16]], "core reply: {:?}", world.people_answers.last());
    let w::Final::Done(_) = final_answer(&peer, socket) else { panic!("amended") };
    let chosen = tasks::LlmChoice {
        account: None,
        main: tasks::ModelEffort { model: SCRIPTED, effort: Some(HIGH) },
        subagents: Box::new([]),
    };
    assert_eq!(live_row(&world, 2).llm, Some(chosen));
}
