use jig_core_world::effects::World;
use jig_core_world::fixtures::{finish_chat, live_row, member, unchanged};
use jig_domain_core as core;
use jig_domain_tasks as tasks;
use jig_test_connector as connector;
use jig_test_domain as root;
use skein_lib::{ReplyTo, Token};

const CONNECTOR: u16 = 1;
const PROCEDURE_CODE: u32 = 1;
const DELEGATE_CALL: u64 = 9001;
const AMEND_CALL: u64 = 9002;
const COMPLETION: u32 = 1;
const MOVED_COMPLETION: u32 = 2;

fn procedure_world(seed: u64) -> (World, u64) {
    program_world(seed, None)
}

fn program_world(seed: u64, actions: Option<Box<[connector::ProcedureAction]>>) -> (World, u64) {
    program_world_with_deadline(seed, actions, None)
}

fn program_world_with_deadline(
    seed: u64,
    actions: Option<Box<[connector::ProcedureAction]>>,
    deadline: Option<skein_lib::Wall>,
) -> (World, u64) {
    let (mut configuration, limits) = jig_core_world::faults::plan_fixture(seed);
    if let Some(actions) = actions {
        configuration.first.procedures[0].actions = actions;
        configuration.first.procedures[0].max_steps = 6;
    }
    let mut world = World::configured(seed, false, configuration, limits);
    world.hold_procedures = true;
    let root_run = world.assignments[0];
    let mut procedure =
        member(&world, tasks::Executor::Procedure { connector: CONNECTOR, code: PROCEDURE_CODE }, b"procedure");
    procedure.authority.budget.deadline = deadline;
    world.send(root::Event::Core(core::Event::DelegateValidated {
        to: ReplyTo::new(Token::new(DELEGATE_CALL)),
        key: core::CallKey { task: root_run.0, attempt: root_run.1, completion: COMPLETION, position: 0 },
        batch: Box::new([procedure]),
        stubs: Box::new([]),
    }));
    let core::CallPart::Delegated(numbers) = &world.answers.last().expect("procedure made").2 else {
        panic!("procedure refused")
    };
    let task = numbers[0].task;
    (world, task)
}

fn decide(world: &mut World, task: u64, decision: core::ProcedureStep) {
    let (step, number, code, _) = world.procedure_inputs.last().expect("procedure input");
    world.send(root::Event::Core(core::Event::ProcedureStep {
        task,
        step: *step,
        connector: *number,
        code: u32::from(*code),
        decision,
    }));
}

fn make_delegate(world: &mut World, procedure: u64) -> u64 {
    let mut child = member(world, tasks::Executor::Agent { charter: 1 }, b"delegate");
    child.authority.budget.spend = 5;
    child.authority.budget.deadline = None;
    child.authority.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
    decide(
        world,
        procedure,
        core::ProcedureStep { batch: Box::new([child]), ..core::ProcedureStep::new(core::StepEnd::Wait) },
    );
    live_row(world, procedure).delegates[0]
}

#[test]
fn a_procedure_hears_the_first_task_its_step_made_with_its_revision() {
    let (mut world, procedure) = procedure_world(461);
    let child = make_delegate(&mut world, procedure);
    let record = connector::Record::ProcedureDelegateOutcome {
        task: procedure,
        step: 1,
        child: Some(child),
        revisions: Box::new([(child, 0)]),
    };
    assert_eq!(
        world.store.rows.get(&root::Key::Connector { number: CONNECTOR, key: record.key() }),
        Some(&root::Record::Connector { number: CONNECTOR, record })
    );
}

#[test]
fn a_step_naming_a_moved_revision_is_refused_and_told_what_it_is_now() {
    let (mut world, procedure) = procedure_world(462);
    let child = make_delegate(&mut world, procedure);
    let run = world.assignments[0];
    let amendment = tasks::Amendment {
        tracked: None,
        addressed: None,
        instructions: None,
        release: None,
        spec: Some(tasks::Spec {
            asks: false,
            words: b"moved".as_slice().into(),
            parameters: Box::new([]),
            inputs: Box::new([]),
        }),
        wake: None,
        dependencies: None,
        authority: None,
        llm: None,
        reason: b"changed".as_slice().into(),
    };
    world.send(root::Event::Core(core::Event::NamedAction {
        to: ReplyTo::new(Token::new(AMEND_CALL)),
        key: core::CallKey { task: run.0, attempt: run.1, completion: MOVED_COMPLETION, position: 0 },
        action: core::NamedAction::Amend { target: child, revision: 0, amendment: amendment.clone() },
    }));
    world.send(root::Event::Core(core::Event::Tasks(tasks::Event::WakeProcedure { task: procedure })));
    let before = live_row(&world, child).clone();
    decide(
        &mut world,
        procedure,
        core::ProcedureStep {
            amendments: Box::new([core::StepAmendment { task: child, revision: 0, amendment }]),
            ..core::ProcedureStep::new(core::StepEnd::Wait)
        },
    );
    assert_eq!(live_row(&world, child), &before);
    let record = connector::Record::ProcedureDelegateOutcome {
        task: procedure,
        step: 2,
        child: None,
        revisions: Box::new([(child, 1)]),
    };
    assert_eq!(
        world.store.rows.get(&root::Key::Connector { number: CONNECTOR, key: record.key() }),
        Some(&root::Record::Connector { number: CONNECTOR, record })
    );
    assert_eq!(before.revisions.spec, 1);
}

const MESSAGE_WORDS: u64 = 12;
const SPEC_WORDS: u64 = 34;

fn delegate_program(action: connector::ProcedureAction) -> Box<[connector::ProcedureAction]> {
    Box::new([connector::ProcedureAction::Delegate { kinds: Box::new([1]) }, action, connector::ProcedureAction::Stall])
}

fn run_program(world: &mut World, task: u64) {
    let context = &world
        .procedure_inputs
        .iter()
        .rev()
        .find(|(_, _, _, context)| context.task == task)
        .expect("procedure input")
        .3;
    let event = root::procedure_input(
        context,
        u16::try_from(PROCEDURE_CODE).expect("test procedure code"),
        jig_test_connector_world::path(1, 1),
    );
    world.send(root::Event::Connector { number: CONNECTOR, event });
}

fn next_program_step(world: &mut World, task: u64) {
    world.send(root::Event::Core(core::Event::Tasks(tasks::Event::WakeProcedure { task })));
    run_program(world, task);
}

#[test]
fn a_procedure_amends_its_delegate_naming_the_revision_it_was_told() {
    let (mut world, procedure) = program_world(
        463,
        Some(delegate_program(connector::ProcedureAction::Amend { words: SPEC_WORDS, priority: None })),
    );
    run_program(&mut world, procedure);
    let child = live_row(&world, procedure).delegates[0];
    next_program_step(&mut world, procedure);
    assert_eq!(live_row(&world, child).spec.words.as_ref(), b"amended 34");
    assert_eq!(live_row(&world, child).revisions.spec, 1);
}

#[test]
fn a_procedure_steps_again_after_its_amendment_is_refused_as_moved() {
    let (mut world, procedure) = program_world(
        464,
        Some(delegate_program(connector::ProcedureAction::Amend { words: SPEC_WORDS, priority: None })),
    );
    run_program(&mut world, procedure);
    let child = live_row(&world, procedure).delegates[0];
    let run = world.assignments[0];
    let amendment = tasks::Amendment {
        spec: Some(tasks::Spec {
            asks: false,
            words: b"moved".as_slice().into(),
            parameters: Box::new([]),
            inputs: Box::new([]),
        }),
        tracked: None,
        addressed: None,
        instructions: None,
        release: None,
        wake: None,
        dependencies: None,
        authority: None,
        llm: None,
        reason: Box::new([]),
    };
    world.send(root::Event::Core(core::Event::NamedAction {
        to: ReplyTo::new(Token::new(AMEND_CALL)),
        key: core::CallKey { task: run.0, attempt: run.1, completion: MOVED_COMPLETION, position: 0 },
        action: core::NamedAction::Amend { target: child, revision: 0, amendment },
    }));
    next_program_step(&mut world, procedure);
    let key = root::Key::Connector { number: CONNECTOR, key: connector::RecordKey::Procedure(procedure) };
    let Some(root::Record::Connector { record: connector::Record::Procedure(state), .. }) = world.store.rows.get(&key)
    else {
        panic!("procedure state committed");
    };
    assert_eq!(state.steps, 3);
    assert_eq!(state.revision, 2);
    assert_eq!(live_row(&world, child).spec.words.as_ref(), b"amended 34");
    assert_eq!(live_row(&world, child).revisions.spec, 2);
}

#[test]
fn a_procedure_messages_its_delegate_in_its_step() {
    let (mut world, procedure) =
        program_world(465, Some(delegate_program(connector::ProcedureAction::Message { words: MESSAGE_WORDS })));
    run_program(&mut world, procedure);
    let child = live_row(&world, procedure).delegates[0];
    next_program_step(&mut world, procedure);
    assert!(live_row(&world, child).inbox.iter().any(|word| {
        word.from == tasks::Party::Task(procedure)
            && word.kind == tasks::MessageKind::Words
            && word.words.as_ref() == b"12"
    }));
}

#[test]
fn a_procedure_program_proposes_a_widening() {
    let (mut world, procedure) = program_world(466, Some(Box::new([connector::ProcedureAction::Propose { spend: 5 }])));
    run_program(&mut world, procedure);
    let row = live_row(&world, procedure);
    let proposal = row.proposal.as_ref().expect("procedure proposal");
    let mut authority = row.authority.clone();
    authority.budget.spend = 25;
    assert_eq!(proposal.action, tasks::ProposalAction::Widen { task: procedure, authority });
    assert_eq!(proposal.proposer, procedure);
}

#[test]
fn a_procedure_proposes_a_widening_while_its_message_commits() {
    let (mut world, procedure) = procedure_world(467);
    let child = make_delegate(&mut world, procedure);
    world.send(root::Event::Core(core::Event::Tasks(tasks::Event::WakeProcedure { task: procedure })));
    let mut authority = live_row(&world, procedure).authority.clone();
    authority.budget.spend += 5;
    let action = tasks::ProposalAction::Widen { task: procedure, authority };
    decide(
        &mut world,
        procedure,
        core::ProcedureStep {
            messages: Box::new([core::StepMessage {
                target: child,
                kind: tasks::MessageKind::Words,
                words: b"hello".as_slice().into(),
            }]),
            proposals: Box::new([action.clone()]),
            ..core::ProcedureStep::new(core::StepEnd::Wait)
        },
    );
    assert_eq!(live_row(&world, procedure).proposal.as_ref().expect("pending proposal").action, action);
    assert!(
        live_row(&world, child)
            .inbox
            .iter()
            .any(|word| word.words.as_ref() == b"hello" && word.from == tasks::Party::Task(procedure))
    );
    assert!(world.commits.iter().any(|commit| {
        let proposal = commit.iter().any(|write| {
            matches!(write,
                root::Write::Save(root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))))
                    if row.number == procedure && row.proposal.is_some()
            )
        });
        let message = commit.iter().any(|write| {
            matches!(write,
                root::Write::Save(root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))))
                    if row.number == child && row.inbox.iter().any(|word| word.words.as_ref() == b"hello")
            )
        });
        proposal && message
    }));
}

const CANCEL_GRACE: skein_lib::Duration = skein_lib::Duration::from_secs(5);
const PRIORITY: u32 = 7;

#[test]
fn a_procedure_cancels_its_delegate_with_a_grace_it_finishes_within() {
    let (mut world, procedure) =
        program_world(468, Some(delegate_program(connector::ProcedureAction::Cancel { grace: CANCEL_GRACE })));
    run_program(&mut world, procedure);
    let child = live_row(&world, procedure).delegates[0];
    next_program_step(&mut world, procedure);
    let grace = live_row(&world, child).grace.as_ref().expect("finish requested");
    assert_eq!(grace.by, tasks::Party::Task(procedure));
    assert_eq!(grace.reason.as_ref(), b"procedure cancelled delegate");
    assert_eq!(world.inbound.last().expect("finish word").number, grace.message);
    finish_chat(&mut world, 0, b"finished in time");
    let key = root::Key::Core(core::Key::Tasks(tasks::Key::Ended(child)));
    let Some(root::Record::Core(core::Record::Tasks(tasks::Stored::Ended(row)))) = world.store.rows.get(&key) else {
        panic!("delegate ended")
    };
    assert_eq!(
        row.phase,
        tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Report {
            words: b"finished in time".as_slice().into(),
        }))
    );
}

#[test]
fn a_procedure_asked_to_finish_by_a_grace_steps_as_on_an_amendment() {
    let (mut world, procedure) = procedure_world(469);
    decide(&mut world, procedure, core::ProcedureStep::new(core::StepEnd::Wait));
    let before = world.procedure_inputs.len();
    jig_core_world::stories::reads::request(
        &mut world,
        75,
        jig_domain_people::Ask::Cancel {
            project: 1,
            task: procedure,
            reason: b"finish procedure".as_slice().into(),
            grace: Some(CANCEL_GRACE),
        },
    );
    assert_eq!(world.procedure_inputs.len(), before + 1);
    let (step, _, _, context) = world.procedure_inputs.last().expect("finish wake");
    assert_eq!(*step, 2);
    assert_eq!(live_row(&world, procedure).grace.as_ref().expect("durable grace").reason.as_ref(), b"finish procedure");
    assert_eq!(
        context.inbox.last().expect("finish amendment").kind,
        tasks::MessageKind::FinishRequested { revision: 1 }
    );
    decide(
        &mut world,
        procedure,
        core::ProcedureStep::new(core::StepEnd::Result(tasks::TaskResult::Report {
            words: b"finished".as_slice().into(),
        })),
    );
    assert!(world.store.rows.contains_key(&root::Key::Core(core::Key::Tasks(tasks::Key::Ended(procedure)))));
}

fn move_delegate(world: &mut World, child: u64) {
    let run = world.assignments[0];
    world.send(root::Event::Core(core::Event::NamedAction {
        to: ReplyTo::new(Token::new(AMEND_CALL)),
        key: core::CallKey { task: run.0, attempt: run.1, completion: MOVED_COMPLETION, position: 0 },
        action: core::NamedAction::Amend {
            target: child,
            revision: 0,
            amendment: tasks::Amendment {
                tracked: Some(tasks::Tracking::Tracked { priority: 1 }),
                spec: Some(tasks::Spec {
                    asks: false,
                    words: b"new spec".as_slice().into(),
                    parameters: Box::new([]),
                    inputs: Box::new([]),
                }),
                ..unchanged(b"")
            },
        },
    }));
}

#[test]
fn a_procedures_priority_change_names_no_revision_and_commits_after_a_spec_change() {
    let (mut world, procedure) = program_world(
        470,
        Some(delegate_program(connector::ProcedureAction::Amend { words: SPEC_WORDS, priority: Some(PRIORITY) })),
    );
    run_program(&mut world, procedure);
    let child = live_row(&world, procedure).delegates[0];
    move_delegate(&mut world, child);
    assert_eq!(live_row(&world, child).revisions.spec, 1);
    next_program_step(&mut world, procedure);
    assert_eq!(live_row(&world, child).tracked, Some(PRIORITY));
    assert_eq!(live_row(&world, child).spec.words.as_ref(), b"new spec");
    assert_eq!(live_row(&world, child).revisions.spec, 1);
    let key = root::Key::Connector { number: CONNECTOR, key: connector::RecordKey::Procedure(procedure) };
    let Some(root::Record::Connector { record: connector::Record::Procedure(state), .. }) = world.store.rows.get(&key)
    else {
        panic!("procedure state committed")
    };
    assert_eq!(state.steps, 2, "the priority step had no moved retry");
}

fn assert_cancel_refused(world: &mut World, procedure: u64, child: u64, cancels: Box<[core::StepCancel]>) {
    world.send(root::Event::Core(core::Event::Tasks(tasks::Event::WakeProcedure { task: procedure })));
    let before = live_row(world, child).clone();
    let parent = live_row(world, procedure).clone();
    decide(
        world,
        procedure,
        core::ProcedureStep {
            messages: Box::new([core::StepMessage {
                target: child,
                kind: tasks::MessageKind::Words,
                words: b"atomic".as_slice().into(),
            }]),
            cancels,
            ..core::ProcedureStep::new(core::StepEnd::Wait)
        },
    );
    assert_eq!(live_row(world, child), &before);
    assert_eq!(live_row(world, procedure), &parent);
}

#[test]
fn a_cancel_of_an_unknown_task_leaves_the_whole_procedure_step_unmade() {
    let (mut world, procedure) = procedure_world(471);
    let child = make_delegate(&mut world, procedure);
    assert_cancel_refused(
        &mut world,
        procedure,
        child,
        Box::new([core::StepCancel { task: u64::MAX, reason: Box::new([]), grace: None }]),
    );
}

#[test]
fn a_cancel_of_itself_leaves_the_whole_procedure_step_unmade() {
    let (mut world, procedure) = procedure_world(472);
    let child = make_delegate(&mut world, procedure);
    assert_cancel_refused(
        &mut world,
        procedure,
        child,
        Box::new([core::StepCancel { task: procedure, reason: Box::new([]), grace: None }]),
    );
}

#[test]
fn a_cancel_past_the_grace_bound_leaves_the_whole_procedure_step_unmade() {
    let (mut world, procedure) = procedure_world(473);
    let child = make_delegate(&mut world, procedure);
    assert_cancel_refused(
        &mut world,
        procedure,
        child,
        Box::new([core::StepCancel {
            task: child,
            reason: Box::new([]),
            grace: Some(skein_lib::Duration::from_secs(601)),
        }]),
    );
}

#[test]
fn a_duplicate_cancel_leaves_the_whole_procedure_step_unmade() {
    let (mut world, procedure) = procedure_world(474);
    let child = make_delegate(&mut world, procedure);
    assert_cancel_refused(
        &mut world,
        procedure,
        child,
        Box::new([
            core::StepCancel { task: child, reason: Box::new([]), grace: None },
            core::StepCancel { task: child, reason: Box::new([]), grace: None },
        ]),
    );
}

const AMENDMENT_WALL: skein_lib::Wall = skein_lib::Wall::from_nanos(100_000_000);

fn kept_procedure(seed: u64) -> (World, u64, u64) {
    let (mut world, procedure) = procedure_world(seed);
    decide(&mut world, procedure, core::ProcedureStep::new(core::StepEnd::Wait));
    let parent = world.assignments[0];
    let dependency = member(&world, tasks::Executor::Agent { charter: 1 }, b"prerequisite");
    let dependency = jig_core_world::fixtures::delegate(&mut world, parent, 2, dependency);
    let mut amendment = unchanged(b"wait for prerequisite");
    amendment.dependencies = Some(Box::new([dependency]));
    world.set_clock(skein_lib::Time::from_nanos(AMENDMENT_WALL.as_nanos()), AMENDMENT_WALL);
    world.send(root::Event::Core(core::Event::NamedAction {
        to: ReplyTo::new(Token::new(AMEND_CALL)),
        key: core::CallKey { task: parent.0, attempt: parent.1, completion: 3, position: 0 },
        action: core::NamedAction::Amend { target: procedure, revision: 0, amendment },
    }));
    (world, procedure, dependency)
}

#[test]
fn a_kept_procedures_result_refuses_its_whole_step_including_its_message() {
    let (mut world, procedure, dependency) = kept_procedure(885);
    let before = live_row(&world, dependency).clone();
    let procedure_before = live_row(&world, procedure).clone();
    decide(
        &mut world,
        procedure,
        core::ProcedureStep {
            messages: Box::new([core::StepMessage {
                target: dependency,
                kind: tasks::MessageKind::Words,
                words: b"must not arrive".as_slice().into(),
            }]),
            ..core::ProcedureStep::new(core::StepEnd::Result(tasks::TaskResult::Report {
                words: b"done".as_slice().into(),
            }))
        },
    );
    assert_eq!(live_row(&world, dependency), &before);
    assert_eq!(live_row(&world, procedure), &procedure_before);
}

#[test]
fn a_procedure_receives_pending_dependencies_with_their_amendment_provenance() {
    let (world, procedure, dependency) = kept_procedure(886);
    let (_, _, _, context) = world
        .procedure_inputs
        .iter()
        .rev()
        .find(|(_, _, _, context)| context.task == procedure)
        .expect("kept procedure input");
    assert_eq!(context.pending.as_ref(), &[dependency]);
    assert_eq!(
        context.added.as_ref(),
        &[tasks::Added { task: dependency, at: AMENDMENT_WALL, by: tasks::Party::Task(world.assignments[0].0) }]
    );
    let connector::Event::Procedure { pending, added, .. } = root::procedure_input(
        context,
        u16::try_from(PROCEDURE_CODE).expect("test code"),
        jig_test_connector_world::path(1, 1),
    ) else {
        panic!("procedure event")
    };
    assert_eq!(pending.as_ref(), &[dependency]);
    assert_eq!(
        added.as_ref(),
        &[connector::Added {
            task: dependency,
            at: AMENDMENT_WALL,
            by: connector::application_core::Party::Task(world.assignments[0].0),
        }]
    );
}

#[test]
fn a_procedure_delegate_naming_no_deadline_ends_with_its_procedures() {
    let deadline = Some(skein_lib::Wall::from_nanos(20_000_000_000));
    let (mut world, procedure) = program_world_with_deadline(475, None, deadline);
    let child = make_delegate(&mut world, procedure);
    assert_eq!(live_row(&world, child).authority.budget.deadline, deadline);
}
