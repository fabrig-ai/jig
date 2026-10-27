use jig_domain_tasks::{self as tasks, Contract, Event, Executor, Party, PersonAddress, Phase, TaskResult, Verdict};
use jig_tasks_world::{LIMITS, Reply, World, task};

fn response(world: &mut World, event: impl FnOnce(skein_lib::ReplyTo) -> Event) -> Reply {
    let reply_to = world.to();
    world.send(event(reply_to));
    world.replies.last_key_value().expect("terminal reply").1.clone()
}

fn notified(world: &World, task: u64) -> &tasks::ActivityLine {
    world
        .records
        .values()
        .find_map(|record| match record {
            tasks::Stored::Activity(line) if line.task == task && matches!(line.line, tasks::Line::Notified { .. }) => {
                Some(line)
            }
            tasks::Stored::Activity(_)
            | tasks::Stored::Milestone(_)
            | tasks::Stored::PersonProposal(_)
            | tasks::Stored::History(_)
            | tasks::Stored::Live(_)
            | tasks::Stored::Ended(_)
            | tasks::Stored::Stub(_)
            | tasks::Stored::Ledger(_)
            | tasks::Stored::Writer(_)
            | tasks::Stored::Result(_)
            | tasks::Stored::EndedByParty(_)
            | tasks::Stored::EndedByProject(_)
            | tasks::Stored::EndedDelegate(_)
            | tasks::Stored::Delegate(_)
            | tasks::Stored::Update(_)
            | tasks::Stored::Pool(_)
            | tasks::Stored::ClosedLedger(_)
            | tasks::Stored::PlacedHold(_)
            | tasks::Stored::DecidedProposal(_) => None,
        })
        .expect("immutable outward notification")
}

#[test]
fn a_person_task_is_taken_handed_back_and_answered() {
    let mut world = World::new(220, LIMITS);
    let mut new = task(1, &[]);
    new.executor = Executor::Person(PersonAddress::Role(7));
    new.contract = Contract::Verdict {
        choices: Box::new([
            Verdict { fields: Box::new([]), code: 1, words: 8, followups: 0 },
            Verdict { fields: Box::new([]), code: 2, words: 8, followups: 0 },
        ]),
    };
    assert_eq!(world.make(Party::Person(9), vec![new]), Reply::Made(vec![1]));
    assert_eq!(world.record(1).phase, Phase::Active(tasks::Active::Due));
    assert!(!world.activations.contains(&1), "a person task does not claim a worker");

    assert_eq!(response(&mut world, |reply_to| Event::TakePerson { reply_to, task: 1, person: 10 }), Reply::Done);
    assert_eq!(world.record(1).taken_by, Some(10));
    assert!(matches!(
        response(&mut world, |reply_to| Event::TakePerson { reply_to, task: 1, person: 11 }),
        Reply::Refused(_)
    ));
    world.restart();
    assert_eq!(world.record(1).taken_by, Some(10));
    assert_eq!(response(&mut world, |reply_to| Event::HandBackPerson { reply_to, task: 1, person: 10 }), Reply::Done);
    assert_eq!(world.record(1).taken_by, None);
    assert_eq!(response(&mut world, |reply_to| Event::TakePerson { reply_to, task: 1, person: 11 }), Reply::Done);
    assert!(matches!(
        response(&mut world, |reply_to| Event::AnswerPerson {
            reply_to,
            task: 1,
            person: 11,
            result: TaskResult::Verdict { fields: Box::new([]), code: 3, words: Box::new([]) },
        }),
        Reply::Refused(_)
    ));
    assert_eq!(
        response(&mut world, |reply_to| Event::AnswerPerson {
            reply_to,
            task: 1,
            person: 11,
            result: TaskResult::Verdict { fields: Box::new([]), code: 2, words: Box::new([1]) },
        }),
        Reply::Done
    );
    world.settle(1);
    assert!(matches!(world.results[&1], tasks::Ending::Done(TaskResult::Verdict { code: 2, .. })));
}

#[test]
fn a_direct_person_task_accepts_only_its_addressees_answer() {
    let mut limits = LIMITS;
    limits.person_stall = skein_lib::Duration::from_secs(1);
    let mut world = World::new(221, limits);
    let mut new = task(1, &[]);
    new.executor = Executor::Person(PersonAddress::Person(10));
    assert_eq!(world.make(Party::Person(9), vec![new]), Reply::Made(vec![1]));
    world.elapse(skein_lib::Duration::from_secs(1));
    let notice = world.record(1).person_wait.expect("answerable wait").notice.expect("personal requester was notified");
    let tasks::Line::Notified { recipient, word } = &notified(&world, 1).line else { panic!() };
    assert_eq!(*recipient, Party::Person(9));
    assert_eq!(word.number, notice.message);
    assert_eq!(word.kind, tasks::MessageKind::PersonStalled { task: 1 });
    assert_eq!(notified(&world, 1).position, notice.activity);
    assert!(matches!(
        response(&mut world, |reply_to| Event::AnswerPerson {
            reply_to,
            task: 1,
            person: 11,
            result: TaskResult::Report { words: Box::new([1]) },
        }),
        Reply::Refused(_)
    ));
    assert_eq!(
        response(&mut world, |reply_to| Event::AnswerPerson {
            reply_to,
            task: 1,
            person: 10,
            result: TaskResult::Report { words: Box::new([1]) },
        }),
        Reply::Done
    );
    world.settle(1);
    assert!(matches!(world.results[&1], tasks::Ending::Done(TaskResult::Report { .. })));
    world.restart();
    assert_eq!(notified(&world, 1).at, notice.at, "a person can read the notice after the waiting entry clears");
}

#[test]
fn a_persons_wait_starts_after_dependencies_and_notifies_its_requester_once_across_restart() {
    let mut limits = LIMITS;
    limits.person_stall = skein_lib::Duration::from_secs(10);
    let mut world = World::new(224, limits);
    assert_eq!(world.make(Party::Person(9), vec![task(1, &[])]), Reply::Made(vec![1]));
    let mut person = task(2, &[3]);
    person.executor = Executor::Person(PersonAddress::Role(7));
    assert_eq!(world.make(Party::Task(1), vec![person, task(3, &[])]), Reply::Made(vec![2, 3]));
    assert_eq!(world.record(2).person_wait, None);
    world.elapse(skein_lib::Duration::from_secs(5));
    world.claim_fresh(3);
    world.finish(3);
    world.settle(3);
    let wait = world.record(2).person_wait.expect("person becomes answerable after its dependency");
    assert_eq!(wait.since, skein_lib::Wall::from_nanos(5_000_000_000));
    assert_eq!(wait.deadline, skein_lib::Wall::from_nanos(15_000_000_000));
    assert_eq!(wait.stall, limits.person_stall);
    assert_eq!(wait.notice, None);
    world.send(Event::PersonStallNotice { task: 2, message: 100 });
    assert_eq!(world.record(2).person_wait, Some(wait), "a premature root notice cannot shorten the wait");
    world.elapse(skein_lib::Duration::from_secs(9));
    world.restart();
    assert_eq!(response(&mut world, |reply_to| Event::TakePerson { reply_to, task: 2, person: 10 }), Reply::Done);
    assert_eq!(response(&mut world, |reply_to| Event::HandBackPerson { reply_to, task: 2, person: 10 }), Reply::Done);
    assert_eq!(world.record(2).person_wait, Some(wait), "claims do not restart the question");
    response(&mut world, |reply_to| Event::Place {
        reply_to,
        hold: tasks::PlacedHold {
            number: 91,
            project: 1,
            owner: tasks::Owner::Party { person: 9 },
            mode: tasks::Mode::Stop,
            scope: tasks::Scope::Subtree { task: 2 },
            reason: Box::new([]),
            placed_at: 0,
            revision: 1,
            exempt: Box::new([]),
            lifted: false,
        },
    });
    world.elapse(skein_lib::Duration::from_secs(1));
    assert_eq!(world.record(2).person_wait, Some(wait), "held tasks do not send a stall notice");
    world.restart();
    assert_eq!(
        response(&mut world, |reply_to| Event::Control {
            reply_to,
            by: Party::Person(9),
            task: 2,
            control: tasks::Control::Release { again: None, message: None },
        }),
        Reply::Done
    );
    world.elapse(skein_lib::Duration::ZERO);
    let noticed = world.record(2).person_wait.expect("waiting facts retained");
    let notice = noticed.notice.expect("deadline issued a notice");
    assert_eq!(notice.at, wait.deadline);
    let delivered: Vec<_> = world
        .record(1)
        .inbox
        .iter()
        .filter(|word| word.kind == tasks::MessageKind::PersonStalled { task: 2 })
        .collect();
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].number, notice.message);
    let tasks::Line::Notified { recipient, word } = &notified(&world, 2).line else { panic!() };
    assert_eq!(*recipient, Party::Task(1));
    assert_eq!(word, delivered[0]);
    assert_eq!(notified(&world, 2).position, notice.activity);
    assert!(
        world.records.values().any(|record| matches!(record,
            tasks::Stored::Activity(tasks::ActivityLine { task: 1, line: tasks::Line::Heard(news), .. })
                if news.number == notice.message
        )),
        "requester activity retains the admitted news"
    );
    world.restart();
    world.elapse(skein_lib::Duration::from_secs(1));
    assert_eq!(world.record(2).person_wait, Some(noticed));
    assert_eq!(
        world.record(1).inbox.iter().filter(|word| word.kind == tasks::MessageKind::PersonStalled { task: 2 }).count(),
        1
    );
}

#[test]
fn person_stall_news_and_the_result_keep_room_before_ordinary_words() {
    let mut limits = LIMITS;
    limits.inbox_messages = 2;
    limits.run_inbox = 2;
    limits.person_stall = skein_lib::Duration::from_secs(1);
    let mut world = World::new(225, limits);
    let mut parent = task(1, &[]);
    parent.wake.news = tasks::WakeRule::Never;
    assert_eq!(world.make(Party::Person(9), vec![parent]), Reply::Made(vec![1]));
    world.claim_fresh(1);
    assert_eq!(world.terminal(1, tasks::End::Parked), Reply::Acknowledged(tasks::Accepted::New));
    let mut person = task(2, &[]);
    person.executor = Executor::Person(PersonAddress::Person(10));
    assert_eq!(world.make(Party::Task(1), vec![person]), Reply::Made(vec![2]));
    let got = response(&mut world, |reply_to| Event::Message {
        reply_to,
        project: 1,
        task: 1,
        word: tasks::Word {
            number: 50,
            from: Party::Person(9),
            kind: tasks::MessageKind::Words,
            words: Box::new([1]),
            at: skein_lib::Wall::EPOCH,
            hits: 1,
            eligible: true,
        },
    });
    assert!(matches!(got, Reply::Refused(tasks::Problem { why: tasks::Refusal::Busy, .. })));
    world.elapse(skein_lib::Duration::from_secs(1));
    assert_eq!(
        world.record(1).phase,
        Phase::Active(tasks::Active::Idle),
        "stall news respects its requester's wake policy"
    );
    assert_eq!(
        response(&mut world, |reply_to| Event::AnswerPerson {
            reply_to,
            task: 2,
            person: 10,
            result: TaskResult::Report { words: Box::new([1]) }
        }),
        Reply::Done
    );
    world.settle(2);
    let inbox = &world.record(1).inbox;
    assert_eq!(inbox.len(), 2);
    assert!(inbox.iter().any(|word| word.kind == tasks::MessageKind::PersonStalled { task: 2 }));
    assert!(inbox.iter().any(|word| word.kind == tasks::MessageKind::Result(tasks::ResultKind::Report)));
}

#[test]
fn an_approval_exceeding_the_tasks_result_bound_leaves_it_answerable() {
    let mut world = World::new(458, LIMITS);
    let mut new = task(1, &[]);
    new.executor = Executor::Person(PersonAddress::Person(9));
    let state = vec![5; usize::try_from(LIMITS.result_bytes).expect("small bound")].into_boxed_slice();
    new.contract = Contract::Approval {
        state: state.clone(),
        part: tasks::Part { connector: 1, kind: tasks::PartKind::Resource, key: tasks::PartKey::Number(1) },
    };
    assert_eq!(world.make(Party::Person(9), vec![new]), Reply::Made(vec![1]));
    assert!(matches!(
        response(&mut world, |reply_to| Event::AnswerPerson {
            reply_to,
            task: 1,
            person: 9,
            result: TaskResult::Approved { state: state.clone(), words: Some(Box::new([7])) },
        }),
        Reply::Refused(_)
    ));
    assert_eq!(world.record(1).person_answer, None);
    assert_eq!(
        response(&mut world, |reply_to| Event::AnswerPerson {
            reply_to,
            task: 1,
            person: 9,
            result: TaskResult::Approved { state: state.clone(), words: None },
        }),
        Reply::Done
    );
    world.settle(1);
    assert_eq!(world.results[&1], tasks::Ending::Done(TaskResult::Approved { state, words: None }));
}

fn goal_proposal(number: u64) -> tasks::PersonProposal {
    let mut goal = task(number, &[]);
    goal.tracked = Some(1);
    goal.funder = tasks::Funder::Pool { project: 1, person: 9, period: 0 };
    tasks::PersonProposal {
        number,
        proposer: 9,
        project: 1,
        action: tasks::PersonAction::Batch(Box::new([goal])),
        reason: Box::from(b"proposed batch".as_slice()),
        state: tasks::PersonProposalState::Pending { since: skein_lib::Wall::EPOCH },
    }
}

fn reject_goal_proposal(world: &mut World, number: u64) {
    assert_eq!(
        response(world, |reply_to| Event::DecidePersonProposal {
            reply_to,
            proposer: 9,
            proposal: number,
            by: Party::Person(10),
            message: Some(number + 10),
            decision: tasks::ProposalDecision::Reject { reason: Box::new([1]) },
            revision: None,
        }),
        Reply::Done
    );
}

#[test]
fn a_decided_person_proposal_is_erased_and_saved_as_decided() {
    let mut world = World::new(250, LIMITS);
    assert_eq!(
        response(&mut world, |reply_to| Event::ProposePerson { reply_to, proposal: goal_proposal(1) }),
        Reply::Done
    );
    reject_goal_proposal(&mut world, 1);
    assert!(!world.records.contains_key(&tasks::Key::PersonProposal(1)));
    let Some(tasks::Stored::DecidedProposal(row)) = world.records.get(&tasks::Key::DecidedProposal(1)) else {
        panic!("the decided proposal is history")
    };
    assert!(matches!(row.state, tasks::PersonProposalState::Rejected { by: Party::Person(10), .. }));
}

#[test]
fn a_restart_restores_no_decided_person_proposal() {
    let mut world = World::new(251, LIMITS);
    assert_eq!(
        response(&mut world, |reply_to| Event::ProposePerson { reply_to, proposal: goal_proposal(1) }),
        Reply::Done
    );
    reject_goal_proposal(&mut world, 1);
    world.restart();
    assert_eq!(
        response(&mut world, |reply_to| Event::ProposePerson { reply_to, proposal: goal_proposal(2) }),
        Reply::Done
    );
    assert!(world.records.contains_key(&tasks::Key::PersonProposal(2)), "startup completed without historical rows");
    assert!(world.records.contains_key(&tasks::Key::DecidedProposal(1)), "history remains durable");
}

#[test]
fn a_live_proposal_leaves_the_open_shape_to_core() {
    const PROPOSAL: u64 = 1;
    const PERSON: u64 = 9;
    let rows = [
        ("person", Executor::Person(PersonAddress::Person(PERSON)), true),
        ("procedure", Executor::Procedure { connector: 1, code: 1 }, true),
        ("no conversation", Executor::Agent { charter: 1 }, false),
    ];
    for (name, executor, conversation) in rows {
        let mut world = World::new(252, LIMITS);
        let mut proposal = goal_proposal(PROPOSAL);
        let tasks::PersonAction::Batch(batch) = &mut proposal.action else { panic!("batch fixture") };
        for member in batch {
            member.executor = executor;
            member.flags.ended_by = tasks::EndedBy::Requester;
            member.flags.conversation = conversation;
        }
        let records = world.records.len();
        let replies = world.replies.len();
        assert_eq!(
            response(&mut world, |reply_to| Event::ProposePerson { reply_to, proposal: proposal.clone() }),
            Reply::Done,
            "{name}"
        );
        assert_eq!(world.records.len(), records + 1, "{name}");
        assert_eq!(world.replies.len(), replies + 1, "{name}");
        assert_eq!(
            world.records.get(&tasks::Key::PersonProposal(PROPOSAL)),
            Some(&tasks::Stored::PersonProposal(Box::new(proposal))),
            "{name}"
        );
    }
}
