use jig_domain_tasks::{
    self as tasks, Active, End, Event, Executor, Funder, Party, Phase, ProcedureDecision, TaskResult,
};
use jig_tasks_world::{LIMITS, Reply, World, task};
use skein_lib::ReplyTo;

const REQUESTER: u64 = 9;
const ROOT: u64 = 1;
const PROCEDURE: u64 = 2;
const CHILD: u64 = 3;
const NEXT_CHILD: u64 = 4;
const AMENDMENT: u64 = 10;
const MESSAGE: u64 = 11;

fn step(world: &mut World, task: u64, step: u64, decision: ProcedureDecision) -> Reply {
    let reply_to = world.to();
    let key = reply_to.into_token().raw();
    let read = world.contexts.get(&task).and_then(|context| context.inbox.last().map(|word| word.number));
    world.send(Event::Procedure { read, reply_to: ReplyTo::new(skein_lib::Token::new(key)), task, step, decision });
    world.replies[&key].clone()
}

#[test]
fn a_procedure_steps_through_delegates_to_its_result() {
    let mut world = World::new(201, LIMITS);
    world.make(Party::Person(9), vec![task(1, &[])]);
    let mut procedure = task(2, &[]);
    procedure.executor = Executor::Procedure { connector: 1, code: 1 };
    procedure.funder = Funder::Task(1);
    world.make(Party::Task(1), vec![procedure]);
    assert_eq!(world.record(2).phase, Phase::Active(Active::Due));
    let mut child = task(3, &[]);
    child.funder = Funder::Task(2);
    assert_eq!(
        step(
            &mut world,
            2,
            1,
            ProcedureDecision { batch: Box::new([child]), ..ProcedureDecision::new(tasks::ProcedureEnd::Wait) }
        ),
        Reply::Made(vec![3])
    );
    assert_eq!(world.record(2).phase, Phase::Active(Active::Idle));
    world.claim(3, 3);
    world.terminal(3, End::Finished { result: TaskResult::Report { words: Box::new([7]) }, cancel_delegates: false });
    world.settle(3);
    assert_eq!(world.record(2).phase, Phase::Active(Active::Due));
    assert_eq!(
        step(
            &mut world,
            2,
            2,
            ProcedureDecision::new(tasks::ProcedureEnd::Result(TaskResult::Report { words: Box::new([8]) }))
        ),
        Reply::Done
    );
    world.settle(2);
    assert!(
        matches!(world.results.get(&2), Some(tasks::Ending::Done(TaskResult::Report { words })) if words.as_ref() == [8])
    );
    let before = world.records.clone();
    world.restart();
    assert_eq!(world.records, before);
}

fn procedure_with_child() -> World {
    let mut world = World::new(202, LIMITS);
    world.make(Party::Person(REQUESTER), vec![task(ROOT, &[])]);
    let mut procedure = task(PROCEDURE, &[]);
    procedure.executor = Executor::Procedure { connector: 1, code: 1 };
    procedure.funder = Funder::Task(ROOT);
    world.make(Party::Task(ROOT), vec![procedure]);
    let mut child = task(CHILD, &[]);
    child.funder = Funder::Task(PROCEDURE);
    step(
        &mut world,
        PROCEDURE,
        1,
        ProcedureDecision { batch: Box::new([child]), ..ProcedureDecision::new(tasks::ProcedureEnd::Wait) },
    );
    world.send(Event::WakeProcedure { task: PROCEDURE });
    world
}

fn amendment() -> tasks::ProcedureAmendment {
    tasks::ProcedureAmendment {
        task: CHILD,
        message: AMENDMENT,
        stop_run: false,
        amendment: tasks::Amendment {
            tracked: None,
            addressed: None,
            instructions: None,
            release: None,
            spec: Some(tasks::Spec {
                words: b"amended".as_slice().into(),
                parameters: Box::new([]),
                inputs: Box::new([]),
            }),
            wake: None,
            dependencies: None,
            authority: None,
            llm: None,
            reason: b"evidence".as_slice().into(),
        },
    }
}

fn message(words: &[u8]) -> tasks::ProcedureMessage {
    tasks::ProcedureMessage {
        task: CHILD,
        word: tasks::Word {
            number: MESSAGE,
            from: Party::Task(PROCEDURE),
            kind: tasks::MessageKind::Words,
            words: words.into(),
            at: skein_lib::Wall::EPOCH,
            hits: 1,
            eligible: true,
        },
    }
}

#[test]
fn a_step_amending_and_messaging_commits_as_one_decision() {
    let mut world = procedure_with_child();
    let decision = ProcedureDecision {
        amendments: Box::new([amendment()]),
        messages: Box::new([message(b"hello")]),
        ..ProcedureDecision::new(tasks::ProcedureEnd::Wait)
    };
    assert_eq!(step(&mut world, PROCEDURE, 2, decision), Reply::Done);
    assert_eq!(world.record(CHILD).spec.words.as_ref(), b"amended");
    assert_eq!(world.record(CHILD).revisions.spec, 1);
    assert_eq!(
        world.record(CHILD).inbox.iter().find(|word| word.number == MESSAGE).expect("step message").words.as_ref(),
        b"hello"
    );
    let before = world.records.clone();
    world.restart();
    assert_eq!(world.records, before);
}

#[test]
fn a_step_with_one_refused_part_changes_nothing() {
    let mut world = procedure_with_child();
    let before = world.records.clone();
    let decision = ProcedureDecision {
        amendments: Box::new([amendment()]),
        messages: Box::new([message(b"")]),
        ..ProcedureDecision::new(tasks::ProcedureEnd::Wait)
    };
    assert_eq!(
        step(&mut world, PROCEDURE, 2, decision),
        Reply::Refused(tasks::Problem {
            limit: None,
            task: Some(CHILD),
            why: tasks::Refusal::Read,
            blocked_by: None,
            capacity: None,
        })
    );
    assert_eq!(world.records, before);
}

#[test]
fn a_step_counts_its_batch_before_admitting_a_tracking_amendment() {
    let mut world = procedure_with_child();
    world.env.limits.project_goals = 1;
    let mut child = task(NEXT_CHILD, &[]);
    child.funder = Funder::Task(PROCEDURE);
    child.numbers.budget = 0;
    child.authority.budget.spend = 0;
    child.tracked = Some(1);
    let mut part = amendment();
    part.amendment.tracked = Some(tasks::Tracking::Tracked { priority: 2 });
    let before = world.records.clone();
    let decision = ProcedureDecision {
        batch: Box::new([child]),
        amendments: Box::new([part]),
        ..ProcedureDecision::new(tasks::ProcedureEnd::Wait)
    };
    assert_eq!(
        step(&mut world, PROCEDURE, 2, decision),
        Reply::Refused(tasks::Problem {
            limit: None,
            task: Some(CHILD),
            why: tasks::Refusal::Goals,
            blocked_by: None,
            capacity: None,
        })
    );
    assert_eq!(world.records, before);
}

const PROPOSAL: u64 = 12;
const NEXT_PROPOSAL: u64 = 13;
const PROPOSAL_ROOM_CASES: [(&str, bool); 2] = [("existing proposal", true), ("delegate reservation", false)];

fn proposal(world: &World, number: u64) -> tasks::Proposal {
    tasks::Proposal {
        number,
        proposer: PROCEDURE,
        project: world.record(PROCEDURE).project,
        action: tasks::ProposalAction::Widen { task: PROCEDURE, authority: world.record(PROCEDURE).authority.clone() },
        reason: Box::new([]),
        as_holder: false,
        state: tasks::ProposalState::Pending {
            holder: tasks::ProposalHolder::Task(ROOT),
            since: skein_lib::Wall::EPOCH,
        },
    }
}

#[test]
fn a_step_with_no_room_for_its_proposal_keeps_its_message_uncommitted() {
    for (case, pending) in PROPOSAL_ROOM_CASES {
        let mut world = procedure_with_child();
        let cause = if pending {
            let proposal = proposal(&world, PROPOSAL);
            let reply_to = world.to();
            world.send(Event::Propose { reply_to, proposal });
            tasks::CapacityCause::Proposals { limit: 1 }
        } else {
            world.env.limits.inbox_messages = 1;
            tasks::CapacityCause::Inbox { task: PROCEDURE, messages: 1, bytes: LIMITS.inbox_bytes }
        };
        let before = world.records.clone();
        let decision = ProcedureDecision {
            messages: Box::new([message(b"hello")]),
            proposals: Box::new([proposal(&world, NEXT_PROPOSAL)]),
            ..ProcedureDecision::new(tasks::ProcedureEnd::Wait)
        };
        assert_eq!(
            step(&mut world, PROCEDURE, 2, decision),
            Reply::Refused(tasks::Problem::capacity(
                Some(PROCEDURE),
                tasks::Refusal::Busy,
                tasks::Capacity {
                    project: Some(1),
                    by: Party::Task(PROCEDURE),
                    action: tasks::CapacityAction::Proposal { number: NEXT_PROPOSAL },
                    cause,
                },
            )),
            "{case}"
        );
        assert_eq!(world.records, before, "{case}");
    }
}
