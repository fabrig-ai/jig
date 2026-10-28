//! Kept-task stories at the core's public entrances (engine.md, section 15;
//! tasks.md, section 5.7). Assertions observe durable rows and party deliveries.
use super::*;
use jig_core_world::fixtures::unchanged;

fn amend_dependency(world: &mut World, parent: (u64, u64), target: u64, dependency: u64) {
    let mut amendment = unchanged(b"wait for prerequisite");
    amendment.dependencies = Some(Box::new([dependency]));
    world.send(root::Event::Core(core::Event::NamedAction {
        to: ReplyTo::new(Token::new(9100)),
        key: core::CallKey { task: parent.0, attempt: parent.1, completion: 10, position: 0 },
        action: core::NamedAction::Amend { target, revision: live_row(world, target).revisions.spec, amendment },
    }));
    assert_eq!(live_row(world, target).waiting_on.as_ref(), [dependency]);
}

fn ended(world: &World, task: u64, words: &[u8]) {
    let row = world.store.rows.get(&root::Key::Core(core::Key::Tasks(tasks::Key::Ended(task))));
    assert!(matches!(row, Some(root::Record::Core(core::Record::Tasks(tasks::Stored::Ended(row))))
        if row.phase == tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Report { words: words.into() }))));
}

#[test]
fn a_persons_answer_is_refused_while_kept_and_taken_after_met() {
    let (configuration, limits) = person_world_fixture(880);
    let mut world = World::configured(880, false, configuration, limits);
    let parent = world.assignments[0];
    let tasks::Party::Person(person) = live_row(&world, parent.0).requester else { panic!("person requester") };
    let choice = member(&world, tasks::Executor::Person(tasks::PersonAddress::Person(person)), b"answer");
    let choice = delegate(&mut world, parent, 1, choice);
    let dependency = member(&world, tasks::Executor::Agent { charter: 1 }, b"prerequisite");
    let dependency = delegate(&mut world, parent, 2, dependency);
    amend_dependency(&mut world, parent, choice, dependency);
    let before = live_row(&world, choice).clone();
    let answer = |revision| people::Ask::AnswerPerson {
        project: 1,
        task: choice,
        revision,
        result: people::PersonResult::Report { words: b"answered".as_slice().into() },
    };
    ask(&mut world, 210, answer(before.revisions.person));
    jig_core_world::fixtures::refusal(&world, people::Refusal::Kept { pending: Box::new([dependency]) });
    assert_eq!(live_row(&world, choice), &before);
    let run = running(&world, dependency);
    finish(&mut world, run, b"prerequisite done");
    assert!(live_row(&world, choice).waiting_on.is_empty());
    assert!(
        live_row(&world, choice)
            .inbox
            .iter()
            .any(|word| word.kind == tasks::MessageKind::Met { dependency, removed: false })
    );
    let revision = live_row(&world, choice).revisions.person;
    ask(&mut world, 211, answer(revision));
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::PersonAnswered { task: choice }))
    );
    ended(&world, choice, b"answered");
}

#[test]
fn a_dependency_reorders_write_hold_grants_and_both_waiters_end_in_that_order() {
    let mut world = plan_world(881);
    configure_resources(&mut world);
    let parent = world.assignments[0];
    let mut children = Vec::new();
    for completion in 1..=3 {
        let child = resource_member(&world, 1, b"exclusive work");
        children.push(delegate(&mut world, parent, completion, child));
    }
    let [holder, first, second] = children.as_slice() else { panic!("holder and two waiters") };
    assert!(live_row(&world, *holder).holds_taken);
    for waiter in [first, second] {
        assert_eq!(live_row(&world, *waiter).phase, tasks::Phase::Waiting);
        assert!(!live_row(&world, *waiter).holds_taken);
    }
    amend_dependency(&mut world, parent, *first, *second);
    let start = world.commits.len();
    for task in [holder, second, first] {
        assert!(live_row(&world, *task).holds_taken);
        let run = running(&world, *task);
        finish(&mut world, run, b"exclusive done");
        ended(&world, *task, b"exclusive done");
    }
    let mut grants = Vec::new();
    let mut ends = Vec::new();
    for commit in &world.commits[start..] {
        for write in commit {
            if let root::Write::Save(root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row)))) = write
                && [*first, *second].contains(&row.number)
                && row.holds_taken
                && !grants.contains(&row.number)
            {
                grants.push(row.number);
            }
            if let root::Write::Save(root::Record::Core(core::Record::Tasks(tasks::Stored::Ended(row)))) = write
                && [*first, *second].contains(&row.number)
            {
                ends.push(row.number);
            }
        }
    }
    assert_eq!(grants, [*second, *first]);
    assert_eq!(ends, [*second, *first]);
}

fn kept_pair(seed: u64) -> (World, u64, u64, (u64, u64)) {
    let mut world = plan_world(seed);
    let parent = world.assignments[0];
    let child = member(&world, tasks::Executor::Agent { charter: 1 }, b"kept report");
    let kept = delegate(&mut world, parent, 1, child);
    let dependency = member(&world, tasks::Executor::Agent { charter: 1 }, b"prerequisite");
    let dependency = delegate(&mut world, parent, 2, dependency);
    let run = running(&world, kept);
    amend_dependency(&mut world, parent, kept, dependency);
    (world, kept, dependency, run)
}

#[test]
fn a_party_reads_the_kept_agents_activity_naming_its_pending_dependency() {
    let (mut world, kept, dependency, run) = kept_pair(882);
    finish(&mut world, run, b"saved report");
    world.reads = Some(jig_core_world::reads::Parties::new(world.store.applied));
    jig_core_world::reads::ask(&mut world, 1, core::reads::Read::Activity { task: kept, before: None });
    let core::reads::Answer::Page(page) = jig_core_world::reads::latest(&world) else { panic!("activity page") };
    assert!(page.rows.iter().any(|row| matches!(row,
        core::reads::PageRow::Activity { line, .. }
            if line.task == kept && line.line == tasks::Line::Kept { attempt: run.1, pending: Box::new([dependency]), question: None })));
}

#[test]
fn a_kept_delegate_restarts_then_wakes_and_finishes_after_its_dependency_is_done() {
    let (mut world, kept, dependency, run) = kept_pair(883);
    finish(&mut world, run, b"saved report");
    let dependency_run = running(&world, dependency);
    let parent = world.assignments[0];
    let assignments = world.assignments.iter().filter(|(task, _)| *task == kept).count();
    let (configuration, limits) = jig_core_world::faults::plan_fixture(883);
    world.restart_with(configuration);
    // The surviving worker reconnects with its two live runs. Credentials are
    // lent anew after a cold restart, as in the core world's restart stories.
    world.send(root::Event::Core(core::Event::Fleet(jig_domain_fleet::Event::Hello {
        worker: 0,
        channel: Token::new(7),
        hello: jig_domain_fleet::Hello {
            bounds: jig_core_world::fixtures::bounds(),
            stop_bound: skein_lib::Duration::from_millis(100),
            slots: limits.core.fleet.slots,
            workstreams: Box::new([]),
            hosting: [parent, dependency_run]
                .map(|(task, attempt)| jig_domain_fleet::Hosted {
                    run: Token::new(task),
                    attempt: Token::new(attempt),
                    phase: jig_domain_fleet::Phase::Active,
                })
                .into(),
        },
    })));
    world.send(root::Event::Core(core::Event::Account(jig_domain_accounts::Event::Component(
        jig_domain_accounts::FromComponent::Granted {
            form: jig_domain_accounts::Form::Credential(jig_domain_accounts::Scheme::Bearer),
            account: 1,
            generation: 1,
            valid: skein_lib::Duration::from_secs(60),
        },
    ))));
    assert_eq!(live_row(&world, kept).waiting_on.as_ref(), [dependency]);
    assert_eq!(live_row(&world, kept).phase, tasks::Phase::Active(tasks::Active::Idle));
    assert_eq!(world.assignments.iter().filter(|(task, _)| *task == kept).count(), assignments);
    let dependency_run = running(&world, dependency);
    finish(&mut world, dependency_run, b"prerequisite done");
    let resumed = running(&world, kept);
    assert_ne!(resumed, run);
    assert!(live_row(&world, kept).waiting_on.is_empty());
    assert!(
        live_row(&world, kept)
            .inbox
            .iter()
            .any(|word| word.kind == tasks::MessageKind::Met { dependency, removed: false })
    );
    finish(&mut world, resumed, b"resumed report");
    ended(&world, kept, b"resumed report");
}

#[test]
fn an_idle_kept_agents_inbox_waits_without_waking_then_delivers_after_met() {
    let mut world = plan_world(884);
    let parent = world.assignments[0];
    let child = member(&world, tasks::Executor::Agent { charter: 1 }, b"idle report");
    let kept = delegate(&mut world, parent, 1, child);
    let run = running(&world, kept);
    world.send(root::Event::Answer {
        read: None,
        refused: Box::new([]),
        channel: Token::new(7),
        task: run.0,
        attempt: run.1,
        cumulative: 0,
        end: tasks::End::Parked,
    });
    let child = member(&world, tasks::Executor::Agent { charter: 1 }, b"prerequisite");
    let dependency = delegate(&mut world, parent, 2, child);
    amend_dependency(&mut world, parent, kept, dependency);
    let assigned = world.assignments.len();
    let delivered = world.inbound.len();
    ask(&mut world, 212, people::Ask::Say { project: 1, task: kept, words: b"waiting words".as_slice().into() });
    let message = live_row(&world, kept)
        .inbox
        .iter()
        .find(|word| word.words.as_ref() == b"waiting words")
        .expect("message waits in durable inbox")
        .clone();
    world.advance(skein_lib::Duration::from_millis(1));
    assert_eq!(live_row(&world, kept).phase, tasks::Phase::Active(tasks::Active::Idle));
    assert_eq!(world.assignments.len(), assigned);
    assert!(!world.inbound[delivered..].iter().any(|word| word.number == message.number));
    let dependency_run = running(&world, dependency);
    finish(&mut world, dependency_run, b"prerequisite done");
    assert!(live_row(&world, kept).waiting_on.is_empty());
    assert_eq!(world.assignments.len(), assigned + 1);
    let assignment = world.assigned.as_ref().expect("kept task resumes");
    assert_eq!(assignment.task, kept);
    assert_eq!(assignment.messages.iter().filter(|word| word.number == message.number).count(), 1);
    assert!(assignment.messages.iter().any(|word| word.kind == tasks::MessageKind::Met { dependency, removed: false }));
}
