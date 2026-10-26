use jig_core_world::effects::{World, fixture};
use jig_domain_authority as authority;
use jig_domain_core as core;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{Env, Queue, ReplyTo, Time, Token};

fn recovered(world: &World, seed: u64, requirement: bool) -> (core::Core, Env<core::Limits>) {
    let (configuration, limits) = fixture(seed, requirement);
    recovered_with(world, configuration, &limits)
}

fn recovered_with(
    world: &World,
    configuration: root::Config,
    limits: &root::Limits,
) -> (core::Core, Env<core::Limits>) {
    let env = Env { now: Time::from_nanos(world.wall_time().as_nanos()), wall: world.wall_time(), limits: limits.core };
    let mut core = core::Core::new(configuration.core, &env.limits).expect("fixture core configuration");
    for row in world.store.rows.values() {
        if let root::Record::Core(core::Record::Core(row @ core::CoreRecord::Deployment(_))) = row {
            assert!(matches!(core.restore_core(row.clone(), &env.limits), core::Restored::Deployment { .. }));
        }
    }
    jig_core_world::fixtures::restore_project_policies(&mut core, &env.limits, &world.store);
    for row in world.store.rows.values() {
        if let root::Record::Core(core::Record::Tasks(row)) = row {
            if !row.key().live() {
                continue;
            }
            assert!(core.restore_task_row(row));
            let _requests =
                core::step(&mut core, &env, core::Event::Tasks(tasks::Event::Restore { record: row.clone() }));
        }
    }
    for row in world.store.rows.values() {
        if let root::Record::Core(core::Record::Core(
            row @ (core::CoreRecord::RunProof(_) | core::CoreRecord::Call(_)),
        )) = row
        {
            assert_eq!(core.restore_core(row.clone(), &env.limits), core::Restored::Live);
        }
    }
    core::step(&mut core, &env, core::Event::Tasks(tasks::Event::Check));
    let _requests = core::step(&mut core, &env, core::Event::Tasks(tasks::Event::Restored));
    (core, env)
}

fn description() -> core::connector::EffectDescription {
    let mut state = [0; 32];
    state[31] = 13;
    core::connector::EffectDescription {
        waits_for_dependencies: false,
        connector: 1,
        purpose: 19,
        form: core::connector::EffectForm::Set,
        recovery: core::connector::Recovery::Keyed,
        effect: authority::Effect {
            connector: 1,
            kind: 5,
            name: authority::Name { segments: Box::new([Box::from([1]), Box::from([1])]) },
            state,
            price: Some(3),
            access: authority::EffectAccess::Owned,
            additional: Box::new([]),
            guards: Box::new([]),
        },
    }
}

fn begin(core: &mut core::Core, env: &Env<core::Limits>, origin: core::EffectOrigin) -> authority::Judge {
    let owner = Token::new(9900);
    let _requests = core::step(core, env, core::Event::EffectStart { owner, connector: 1, origin });
    let core::Requests::Out(mut requests) = core::step(
        core,
        env,
        core::Event::EffectConnector(core::connector::Event::Described { owner, description: Box::new(description()) }),
    );
    for _ in 0..requests.len() {
        if let core::Request::Ask { ask: core::Ask::Effect(core::connector::Ask::Judge { judge, .. }), .. } =
            requests.pop().expect("request count")
        {
            return judge;
        }
    }
    panic!("the described effect asks its external judge");
}

fn verdict(core: &mut core::Core, env: &Env<core::Limits>, judge: authority::Judge) -> Vec<core::Request> {
    let core::Requests::Out(mut requests) = core::step(
        core,
        env,
        core::Event::EffectConnector(core::connector::Event::Verdict {
            owner: Token::new(9900),
            judge,
            verdict: authority::Verdict::Met,
            at: env.wall,
            guarded: false,
            state: description().effect.state,
        }),
    );
    let mut outputs = Vec::new();
    for _ in 0..requests.len() {
        outputs.push(requests.pop().expect("request count"));
    }
    outputs
}

fn a_delayed_judge_cannot_keep_or_charge_an_effect_after_its_calling_run_is_cancelled() {
    let world = World::new(129, true);
    let key = world.call_key(1);
    let (mut core, env) = recovered(&world, 129, true);
    let judge = begin(
        &mut core,
        &env,
        core::EffectOrigin::Call { to: ReplyTo::new(Token::new(9901)), key, deadline: env.wall },
    );
    let _requests = core::step(
        &mut core,
        &env,
        core::Event::Tasks(tasks::Event::Control {
            reply_to: ReplyTo::new(Token::new(9902)),
            by: tasks::Party::Person(1),
            task: key.task,
            control: tasks::Control::Cancel {
                reason: b"cancel while the judge reads".as_slice().into(),
                grace: None,
                message: None,
            },
        }),
    );
    let before = core.tasks.task(key.task).expect("closing task").numbers;
    let outputs = verdict(&mut core, &env, judge);
    assert!(outputs.iter().any(|request| matches!(
        request,
        core::Request::Ask { ask: core::Ask::Effect(core::connector::Ask::Drop { .. }), .. }
    )));
    assert!(!outputs.iter().any(|request| matches!(
        request,
        core::Request::Write(_)
            | core::Request::Held(_)
            | core::Request::Ask { ask: core::Ask::Effect(core::connector::Ask::Keep { .. }), .. }
    )));
    assert_eq!(core.tasks.task(key.task).expect("closing task").numbers, before);
    assert!(!core.pending_calls.contains_key(&key));
}

fn a_delayed_judge_is_checked_against_the_policy_that_exists_when_it_completes() {
    let world = World::new(130, true);
    let key = world.call_key(1);
    let (mut core, env) = recovered(&world, 130, true);
    let judge = begin(
        &mut core,
        &env,
        core::EffectOrigin::Call { to: ReplyTo::new(Token::new(9910)), key, deadline: env.wall },
    );
    let mut policy = core.authority.policy(1).expect("policy").clone();
    policy.requirements[0].must_be_guarded = true;
    policy.requirements[0].guard = authority::Guard::Guarded;
    let mut facts = Queue::with_capacity(authority::POLICY_MAX_OUT);
    authority::step(&mut core.authority, authority::Event::Policy { project: 1, policy }, &mut facts);
    assert_eq!(facts.pop(), Some(authority::PolicyFact::Changed { project: 1 }));
    let before = core.tasks.task(key.task).expect("live task").numbers;
    let outputs = verdict(&mut core, &env, judge);
    assert!(outputs.iter().any(|request| matches!(request, core::Request::Write(core::Write::Save(core::Record::Core(core::CoreRecord::Call(row)))) if matches!(row.part, core::CallPart::EffectDenied { answer: authority::Answer::Refuse, .. }))));
    assert!(!outputs.iter().any(|request| matches!(
        request,
        core::Request::Ask { ask: core::Ask::Effect(core::connector::Ask::Keep { .. }), .. }
    )));
    assert_eq!(core.tasks.task(key.task).expect("live task").numbers, before);
}

fn a_delayed_judge_cannot_commit_a_step_of_a_cancelled_procedure() {
    let mut world = World::new(132, true);
    world.delegate();
    let task = world.procedure.expect("procedure");
    let parent = world.assignments[0].0;
    let (mut core, env) = recovered(&world, 132, true);
    let _requests = core::step(&mut core, &env, core::Event::Tasks(tasks::Event::WakeProcedure { task }));
    let step = core.tasks.task(task).expect("procedure").attempt + 1;
    let judge = begin(&mut core, &env, core::EffectOrigin::Procedure { task, step, entry: None });
    let _requests = core::step(
        &mut core,
        &env,
        core::Event::Tasks(tasks::Event::Control {
            reply_to: ReplyTo::new(Token::new(9930)),
            by: tasks::Party::Task(parent),
            task,
            control: tasks::Control::Cancel {
                reason: b"procedure withdrawn".as_slice().into(),
                grace: None,
                message: None,
            },
        }),
    );
    let before = core.tasks.task(task).expect("closing procedure").numbers;
    let outputs = verdict(&mut core, &env, judge);
    assert!(outputs.iter().any(|request| matches!(
        request,
        core::Request::Ask { ask: core::Ask::Effect(core::connector::Ask::Drop { .. }), .. }
    )));
    assert!(!outputs.iter().any(|request| matches!(
        request,
        core::Request::Write(_)
            | core::Request::Held(_)
            | core::Request::Ask { ask: core::Ask::Effect(core::connector::Ask::Keep { .. }), .. }
    )));
    assert_eq!(core.tasks.task(task).expect("closing procedure").numbers, before);
}

fn every_connector_must_acknowledge_closure_and_duplicates_cannot_replace_a_missing_acknowledgement() {
    let world = World::new(133, false);
    let key = world.call_key(1);
    let (mut core, env) = recovered(&world, 133, false);
    let _requests = core::step(
        &mut core,
        &env,
        core::Event::Tasks(tasks::Event::Control {
            reply_to: ReplyTo::new(Token::new(9940)),
            by: tasks::Party::Person(1),
            task: key.task,
            control: tasks::Control::Cancel {
                reason: b"close all outboxes".as_slice().into(),
                grace: None,
                message: None,
            },
        }),
    );
    let _requests = core::step(
        &mut core,
        &env,
        core::Event::AnswerPayload {
            grants_refused: false,
            read: None,
            refused: Box::new([]),
            run: Token::new(key.task),
            attempt: Token::new(key.attempt),
            payload: Token::new(9941),
            cumulative: 0,
            end: tasks::End::Parked,
            saved: None,
            invalid_saved: false,
        },
    );
    for connector in [1, 1, 99] {
        let core::Requests::Out(mut requests) = core::step(
            &mut core,
            &env,
            core::Event::EffectConnector(core::connector::Event::Closed { task: key.task, connector }),
        );
        for _ in 0..requests.len() {
            assert!(!matches!(
                requests.pop().expect("request count"),
                core::Request::Ask { ask: core::Ask::Release { .. }, .. }
            ));
        }
        assert!(matches!(
            core.tasks.task(key.task).expect("still closing").phase,
            tasks::Phase::Closing(tasks::Closing { stage: tasks::Stage::Effects, .. })
        ));
    }
    let core::Requests::Out(mut requests) = core::step(
        &mut core,
        &env,
        core::Event::EffectConnector(core::connector::Event::Closed { task: key.task, connector: 2 }),
    );
    let mut releases = 0;
    for _ in 0..requests.len() {
        if matches!(requests.pop().expect("request count"), core::Request::Ask { ask: core::Ask::Release { .. }, .. }) {
            releases += 1;
        }
    }
    assert_eq!(releases, 2);
}

fn a_conflicting_late_terminal_cannot_replace_an_already_committed_answer() {
    let mut world = World::new(131, false);
    let key = world.call_key(1);
    world.end(tasks::End::Parked, 0);
    let (mut core, env) = recovered(&world, 131, false);
    let core::Requests::Out(mut outputs) = core::step(
        &mut core,
        &env,
        core::Event::AnswerPayload {
            grants_refused: false,
            read: None,
            refused: Box::new([]),
            run: Token::new(key.task),
            attempt: Token::new(key.attempt),
            payload: Token::new(9920),
            cumulative: 0,
            end: tasks::End::Finished {
                result: tasks::TaskResult::Report { words: b"different late answer".as_slice().into() },
                cancel_delegates: false,
            },
            saved: None,
            invalid_saved: false,
        },
    );
    for _ in 0..outputs.len() {
        if let core::Request::Write(core::Write::Save(core::Record::Core(core::CoreRecord::Terminal(row)))) =
            outputs.pop().expect("request count")
        {
            assert_eq!(row.end, tasks::End::Parked);
        }
    }
    assert_eq!(
        core.proofs.get(&key.task).expect("retained proof").terminal.as_ref().expect("canonical terminal").end,
        tasks::End::Parked
    );
    core.remember_unpriced_terminal(Token::new(key.task), Token::new(key.attempt), tasks::End::Refused);
    let core::Requests::Out(mut requests) = core::step(
        &mut core,
        &env,
        core::Event::RefusedPayload {
            request: Token::new(9921),
            problem: tasks::Problem {
                task: Some(key.task),
                why: tasks::Refusal::State,
                blocked_by: None,
                capacity: None,
            },
            payload: Some(core::PayloadRefusal::Answer { task: key.task, attempt: key.attempt }),
        },
    );
    for _ in 0..requests.len() {
        assert!(!matches!(requests.pop().expect("request count"), core::Request::Write(_)));
    }
    assert_eq!(
        core.proofs.get(&key.task).expect("retained proof").terminal.as_ref().expect("canonical terminal").end,
        tasks::End::Parked
    );
}

fn late_busy_or_refused_descriptions_cannot_answer_a_cancelled_run() {
    let world = World::new(135, true);
    let key = world.call_key(1);
    for busy in [true, false] {
        let (mut core, env) = recovered(&world, 135, true);
        let owner = Token::new(9950);
        let _requests = core::step(
            &mut core,
            &env,
            core::Event::EffectStart {
                owner,
                connector: 1,
                origin: core::EffectOrigin::Call { to: ReplyTo::new(Token::new(9951)), key, deadline: env.wall },
            },
        );
        let _requests = core::step(
            &mut core,
            &env,
            core::Event::Tasks(tasks::Event::Control {
                reply_to: ReplyTo::new(Token::new(9952)),
                by: tasks::Party::Person(1),
                task: key.task,
                control: tasks::Control::Cancel {
                    reason: b"cancel while describing".as_slice().into(),
                    grace: None,
                    message: None,
                },
            }),
        );
        let event = if busy {
            core::connector::Event::DescribeBusy { owner }
        } else {
            core::connector::Event::DescribeRefused { owner }
        };
        let core::Requests::Out(mut requests) = core::step(&mut core, &env, core::Event::EffectConnector(event));
        for _ in 0..requests.len() {
            assert!(!matches!(
                requests.pop().expect("request count"),
                core::Request::Write(_) | core::Request::Held(_) | core::Request::Now(_)
            ));
        }
        assert!(!core.pending_calls.contains_key(&key));
    }
}

// Each scenario keeps its own fresh world and all original assertions.
// A shared entrypoint avoids one process startup per short scenario.
#[test]
fn effect_fences_scenarios() {
    a_delayed_judge_cannot_keep_or_charge_an_effect_after_its_calling_run_is_cancelled();
    a_delayed_judge_is_checked_against_the_policy_that_exists_when_it_completes();
    a_delayed_judge_cannot_commit_a_step_of_a_cancelled_procedure();
    every_connector_must_acknowledge_closure_and_duplicates_cannot_replace_a_missing_acknowledgement();
    a_conflicting_late_terminal_cannot_replace_an_already_committed_answer();
    late_busy_or_refused_descriptions_cannot_answer_a_cancelled_run();
}

fn dependency_world(seed: u64, executor: tasks::Executor) -> (World, u64, u64) {
    use jig_core_world::fixtures::{delegate, member};
    let (configuration, limits) = jig_core_world::faults::plan_fixture(seed);
    let mut world = World::configured(seed, false, configuration, limits);
    world.hold_procedures = true;
    let parent = world.assignments[0];
    let target = member(&world, executor, b"kept task");
    let target = delegate(&mut world, parent, 1, target);
    let dependency = member(&world, tasks::Executor::Agent { charter: 1 }, b"prerequisite");
    let dependency = delegate(&mut world, parent, 2, dependency);
    (world, target, dependency)
}

fn add_dependency(world: &mut World, target: u64, dependency: u64) {
    let parent = world.assignments[0];
    world.send(root::Event::Core(dependency_edit(parent, target, dependency)));
}

fn dependency_edit(parent: (u64, u64), target: u64, dependency: u64) -> core::Event {
    let mut amendment = jig_core_world::fixtures::unchanged(b"wait for prerequisite");
    amendment.dependencies = Some(Box::new([dependency]));
    core::Event::NamedAction {
        to: ReplyTo::new(Token::new(9950)),
        key: core::CallKey { task: parent.0, attempt: parent.1, completion: 3, position: 0 },
        action: core::NamedAction::Amend { target, revision: 0, amendment },
    }
}

fn plan_core(world: &World, seed: u64) -> (core::Core, Env<core::Limits>) {
    let (configuration, limits) = jig_core_world::faults::plan_fixture(seed);
    recovered_with(world, configuration, &limits)
}

fn describe_effect(
    core: &mut core::Core,
    env: &Env<core::Limits>,
    origin: core::EffectOrigin,
    waits_for_dependencies: bool,
) -> Vec<core::Request> {
    let owner = Token::new(9900);
    let _requests = core::step(core, env, core::Event::EffectStart { owner, connector: 1, origin });
    let mut description = description();
    description.waits_for_dependencies = waits_for_dependencies;
    let core::Requests::Out(mut requests) = core::step(
        core,
        env,
        core::Event::EffectConnector(core::connector::Event::Described { owner, description: Box::new(description) }),
    );
    let mut outputs = Vec::new();
    while let Some(request) = requests.pop() {
        outputs.push(request);
    }
    outputs
}

fn procedure_origin(core: &core::Core, task: u64) -> core::EffectOrigin {
    let (_, _, step) = core.tasks.procedure_due(task).expect("procedure due");
    core::EffectOrigin::Procedure { task, step, entry: None }
}

#[test]
fn a_kept_procedures_waiting_effect_is_refused_naming_its_pending_dependency() {
    const SEED: u64 = 880;
    let (mut world, task, dependency) = dependency_world(SEED, tasks::Executor::Procedure { connector: 1, code: 1 });
    add_dependency(&mut world, task, dependency);
    let (mut core, env) = plan_core(&world, SEED);
    let before = core.tasks.task(task).expect("live procedure").clone();
    let origin = procedure_origin(&core, task);
    let outputs = describe_effect(&mut core, &env, origin, true);
    let problem = tasks::Problem::naming(Some(task), tasks::Refusal::Kept, &[dependency]);
    assert!(outputs.iter().any(|request| matches!(request,
        core::Request::Ask { connector: 1, ask: core::Ask::StepRefused { task: number, why: core::StepRefusal::Refused(why), .. } }
            if *number == task && *why == problem
    )));
    assert!(outputs.iter().any(|request| matches!(
        request,
        core::Request::Ask {
            ask: core::Ask::Effect(core::connector::Ask::Drop { answer: authority::Answer::Refuse, .. }),
            ..
        }
    )));
    assert_eq!(core.tasks.task(task), Some(&before));
}

#[test]
fn a_kept_procedures_non_waiting_effect_is_admitted() {
    const SEED: u64 = 881;
    let (mut world, task, dependency) = dependency_world(SEED, tasks::Executor::Procedure { connector: 1, code: 1 });
    add_dependency(&mut world, task, dependency);
    let (mut core, env) = plan_core(&world, SEED);
    let origin = procedure_origin(&core, task);
    let outputs = describe_effect(&mut core, &env, origin, false);
    assert!(outputs.iter().any(|request| matches!(
        request,
        core::Request::Ask { ask: core::Ask::Effect(core::connector::Ask::Keep { .. }), .. }
    )));
    assert_eq!(core.tasks.task(task).expect("live procedure").waiting_on.as_ref(), &[dependency]);
}

#[test]
fn a_procedures_waiting_effect_is_admitted_after_its_dependencys_met() {
    const SEED: u64 = 882;
    let (mut world, task, dependency) = dependency_world(SEED, tasks::Executor::Procedure { connector: 1, code: 1 });
    add_dependency(&mut world, task, dependency);
    let &(task_number, attempt) =
        world.assignments.iter().find(|(number, _)| *number == dependency).expect("dependency assigned");
    world.send(root::Event::Answer {
        read: None,
        refused: Box::new([]),
        channel: Token::new(7),
        task: task_number,
        attempt,
        cumulative: 0,
        end: tasks::End::Finished {
            result: tasks::TaskResult::Report { words: b"prerequisite done".as_slice().into() },
            cancel_delegates: false,
        },
    });
    let (mut core, env) = plan_core(&world, SEED);
    let origin = procedure_origin(&core, task);
    let outputs = describe_effect(&mut core, &env, origin, true);
    assert!(outputs.iter().any(|request| matches!(
        request,
        core::Request::Ask { ask: core::Ask::Effect(core::connector::Ask::Keep { .. }), .. }
    )));
    assert!(core.tasks.task(task).expect("live procedure").waiting_on.is_empty());
    assert!(
        jig_core_world::fixtures::live_row(&world, task)
            .inbox
            .iter()
            .any(|word| word.kind == tasks::MessageKind::Met { dependency, removed: false })
    );
}

#[test]
fn a_kept_agent_effect_call_is_answered_with_its_pending_dependency() {
    const SEED: u64 = 883;
    let (mut world, task, dependency) = dependency_world(SEED, tasks::Executor::Agent { charter: 1 });
    add_dependency(&mut world, task, dependency);
    let &(task, attempt) = world.assignments.iter().find(|(number, _)| *number == task).expect("kept run still live");
    let (mut core, env) = plan_core(&world, SEED);
    let key = core::CallKey { task, attempt, completion: 1, position: 0 };
    let origin = core::EffectOrigin::Call { to: ReplyTo::new(Token::new(9951)), key, deadline: env.wall };
    let outputs = describe_effect(&mut core, &env, origin, true);
    let problem = tasks::Problem::naming(Some(task), tasks::Refusal::Kept, &[dependency]);
    assert!(outputs.iter().any(|request| matches!(request,
        core::Request::Held(held) if matches!(held.as_ref(), core::Held::CallAnswer { key: named, part: core::CallPart::EffectRefused(why), .. } if *named == key && *why == problem)
    )));
    assert_eq!(core.call_parts.get(&key), Some(&core::CallPart::EffectRefused(problem)));
}

#[test]
fn a_kept_tasks_refused_effect_is_denied_rather_than_kept() {
    const SEED: u64 = 885;
    let (mut world, task, dependency) = dependency_world(SEED, tasks::Executor::Agent { charter: 1 });
    add_dependency(&mut world, task, dependency);
    let &(task, attempt) = world.assignments.iter().find(|(number, _)| *number == task).expect("kept run still live");
    let (mut core, env) = plan_core(&world, SEED);
    let key = core::CallKey { task, attempt, completion: 1, position: 0 };
    let owner = Token::new(9900);
    let origin = core::EffectOrigin::Call { to: ReplyTo::new(Token::new(9952)), key, deadline: env.wall };
    let _requests = core::step(&mut core, &env, core::Event::EffectStart { owner, connector: 1, origin });
    let mut description = description();
    description.waits_for_dependencies = true;
    // Described for another connector, so the check refuses it outright.
    description.connector = 2;
    let _requests = core::step(
        &mut core,
        &env,
        core::Event::EffectConnector(core::connector::Event::Described { owner, description: Box::new(description) }),
    );
    assert!(matches!(
        core.call_parts.get(&key),
        Some(core::CallPart::EffectDenied { answer: authority::Answer::Refuse, .. })
    ));
}

#[test]
fn a_dependency_added_while_judges_answer_refuses_a_waiting_effect() {
    const SEED: u64 = 884;
    let (world, task, dependency) = dependency_world(SEED, tasks::Executor::Agent { charter: 1 });
    let parent = world.assignments[0];
    let &(task, attempt) = world.assignments.iter().find(|(number, _)| *number == task).expect("live caller");
    let (mut core, env) = plan_core(&world, SEED);
    let (configuration, _) = fixture(SEED, true);
    let policy = configuration.core.seed_projects[0].policy.policy.clone();
    let mut facts = Queue::with_capacity(authority::POLICY_MAX_OUT);
    authority::step(&mut core.authority, authority::Event::Policy { project: 1, policy }, &mut facts);
    let key = core::CallKey { task, attempt, completion: 1, position: 0 };
    let origin = core::EffectOrigin::Call { to: ReplyTo::new(Token::new(9952)), key, deadline: env.wall };
    let outputs = describe_effect(&mut core, &env, origin, true);
    let judge = outputs
        .into_iter()
        .find_map(|request| match request {
            core::Request::Ask { ask: core::Ask::Effect(core::connector::Ask::Judge { judge, .. }), .. } => Some(judge),
            core::Request::Write(_)
            | core::Request::Ask { .. }
            | core::Request::Held(_)
            | core::Request::Now(_)
            | core::Request::ProjectMade { .. }
            | core::Request::Decided => None,
        })
        .expect("judge outstanding");
    let _requests = core::step(&mut core, &env, dependency_edit(parent, task, dependency));
    let outputs = verdict(&mut core, &env, judge);
    let problem = tasks::Problem::naming(Some(task), tasks::Refusal::Kept, &[dependency]);
    assert!(outputs.iter().any(|request| matches!(request,
        core::Request::Held(held) if matches!(held.as_ref(), core::Held::CallAnswer { part: core::CallPart::EffectRefused(why), .. } if *why == problem)
    )));
    assert!(!outputs.iter().any(|request| matches!(
        request,
        core::Request::Ask { ask: core::Ask::Effect(core::connector::Ask::Keep { .. }), .. }
    )));
}
