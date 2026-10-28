//! Exact offer fences survive decisions and cold restart (tasks.md, section 9).
use jig_core_world::effects::World;
use jig_core_world::fixtures::{ask_as, delegate, live_row, member};
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{Duration, ReplyTo, Token};

const DECISION_SEED: u64 = 461;
// tasks.md 8.2: one unread offer occupies the entire test run's inbox.
const SINGLE_SLOT: u32 = 1;
// The middle task has budget 20 and reserves at most 10 for its own run.
const LEAF_BUDGET: u64 = 5;

fn proof(world: &World, task: u64) -> &core::RunProof {
    let root::Record::Core(core::Record::Core(core::CoreRecord::RunProof(row))) =
        &world.store.rows[&root::Key::Core(core::Key::Core(core::CoreKey::RunProof(task)))]
    else {
        panic!("durable run proof")
    };
    row
}

fn action(world: &mut World, run: (u64, u64), completion: u32, action: core::NamedAction) {
    world.send(root::Event::Core(core::Event::NamedAction {
        to: ReplyTo::new(Token::new(9500 + u64::from(completion))),
        key: core::CallKey { task: run.0, attempt: run.1, completion, position: 0 },
        action,
    }));
}

fn say(world: &mut World, task: u64, key: u8) -> u64 {
    let sign_in = world.sign_in.expect("signed in");
    ask_as(world, sign_in, key, people::Ask::Say { project: 1, task, words: b"next".as_slice().into() });
    world
        .people_answers
        .iter()
        .rev()
        .find_map(|reply| match reply {
            people::Reply::Outcome(people::Outcome::Said { message, .. }) => Some(*message),
            people::Reply::Signer(_)
            | people::Reply::SignedIn { .. }
            | people::Reply::SignedOut
            | people::Reply::Outcome(_)
            | people::Reply::Again(_)
            | people::Reply::Full { .. }
            | people::Reply::Refused(_) => None,
        })
        .expect("durable words")
}

fn turn(world: &mut World, run: (u64, u64), number: u32, read: u64) {
    world.send(root::Event::Turn {
        channel: Token::new(7),
        task: run.0,
        attempt: run.1,
        turn: number,
        opens_window: number == 1,
        cumulative: 1,
        read: Some(read),
        refused: Box::new([]),
        transcript: b"reads the decided entry".as_slice().into(),
    });
}

fn reconnect(world: &mut World, seed: u64) {
    let hosting = world
        .store
        .rows
        .values()
        .filter_map(|row| match row {
            root::Record::Core(core::Record::Core(core::CoreRecord::RunProof(proof))) if proof.terminal.is_none() => {
                Some(fleet::Hosted {
                    run: Token::new(proof.task),
                    attempt: Token::new(proof.attempt),
                    phase: fleet::Phase::Active,
                })
            }
            root::Record::Core(_) | root::Record::Connector { .. } => None,
        })
        .collect();
    let config = jig_core_world::faults::plan_fixture(seed).0;
    world.restart_with(config);
    assert!(world.domain.ready(), "{:?}", world.trace);
    world.send(root::Event::Core(core::Event::Fleet(fleet::Event::Hello {
        worker: 0,
        channel: Token::new(7),
        hello: fleet::Hello {
            bounds: jig_core_world::fixtures::bounds(),
            stop_bound: Duration::from_millis(100),
            slots: 3,
            workstreams: Box::new([]),
            hosting,
        },
    })));
    world.grant_account(1);
}

fn entry_world(proposal: bool) -> (World, (u64, u64), u64) {
    let (config, mut limits) = jig_core_world::faults::plan_fixture(DECISION_SEED);
    limits.core.tasks.run_inbox = SINGLE_SLOT;
    let mut world = World::configured(DECISION_SEED, false, config, limits);
    let holder = world.assignments[0];
    let child = member(&world, tasks::Executor::Agent { charter: 1 }, b"delegate");
    let child = delegate(&mut world, holder, 1, child);
    if proposal {
        let child_run = *world.assignments.iter().find(|(task, _)| *task == child).expect("child run");
        let authority = live_row(&world, child).authority.clone();
        action(
            &mut world,
            child_run,
            1,
            core::NamedAction::Propose {
                action: tasks::ProposalAction::Widen { task: child, authority },
                reason: b"review this".as_slice().into(),
                as_holder: false,
            },
        );
    } else {
        world
            .send(root::Event::Core(core::Event::Tasks(tasks::Event::Hold { task: child, why: tasks::Hold::Stopped })));
    }
    (world, holder, child)
}

fn decide(world: &mut World, holder: (u64, u64), child: u64, entry: u64, proposal: bool) {
    if proposal {
        assert_eq!(live_row(world, child).proposal.as_ref().expect("pending proposal").number, entry);
        action(
            world,
            holder,
            2,
            core::NamedAction::DecideProposal {
                proposer: child,
                proposal: entry,
                choice: core::ProposalChoice::Reject { reason: b"no".as_slice().into() },
            },
        );
        assert!(live_row(world, child).proposal.is_none(), "decision removes the holder's virtual entry");
    } else {
        let revision = live_row(world, child).revisions.hold;
        action(
            world,
            holder,
            2,
            core::NamedAction::DecideEscalation { task: child, revision, choice: core::EscalationChoice::Pass },
        );
        assert!(matches!(
            live_row(world, child).escalation,
            tasks::Escalation::Waiting { holder: tasks::EscalationHolder::Person(_), .. }
        ));
    }
}

#[test]
fn decisions_keep_their_offered_fence_and_room_across_each_restart_cut() {
    for proposal in [false, true] {
        for cut in 0..=3 {
            let (mut world, holder, child) = entry_world(proposal);
            let entry = *proof(&world, holder.0).outstanding.first().expect("entry really offered");
            if cut == 1 {
                reconnect(&mut world, DECISION_SEED);
            }
            let next = say(&mut world, holder.0, 71);
            decide(&mut world, holder, child, entry, proposal);
            assert_eq!(proof(&world, holder.0).outstanding.as_ref(), [entry], "decision retains offer");
            if cut == 2 {
                reconnect(&mut world, DECISION_SEED);
            }
            assert!(next > entry);
            assert!(!world.inbound.iter().any(|word| word.number == next), "decision frees no room");
            turn(&mut world, holder, 1, entry);
            assert_eq!(live_row(&world, holder.0).turn, 1);
            assert_eq!(live_row(&world, holder.0).run_spent, 1);
            assert_eq!(proof(&world, holder.0).outstanding.as_ref(), [next], "proposal={proposal} cut={cut}");
            if cut == 3 {
                reconnect(&mut world, DECISION_SEED);
            }
            assert_eq!(proof(&world, holder.0).attempt, holder.1);
            assert_eq!(proof(&world, holder.0).outstanding.as_ref(), [next], "proposal={proposal} cut={cut}");
            let rows = world.store.rows.clone();
            let commits = world.store.applied;
            turn(&mut world, holder, 1, entry); // lost acknowledgement: same exact payload
            assert_eq!(world.store.rows, rows);
            assert_eq!(world.store.applied, commits);
            turn(&mut world, holder, 2, entry); // unchanged committed read is normalized
            assert_eq!(live_row(&world, holder.0).turn, 2);
            assert_eq!(live_row(&world, holder.0).run_spent, 1);
            world.send(root::Event::Answer {
                channel: Token::new(7),
                task: holder.0,
                attempt: holder.1,
                cumulative: 1,
                read: Some(entry),
                refused: Box::new([]),
                end: tasks::End::Parked,
            });
            assert!(
                world.store.rows.values().any(|row| matches!(row,
                root::Record::Core(core::Record::Core(core::CoreRecord::Terminal(terminal)))
                if terminal.task == holder.0 && terminal.attempt == holder.1 && terminal.end == tasks::End::Parked)),
                "answer uses a committed read: {:?}",
                world.trace
            );
            assert!(proof(&world, holder.0).attempt > holder.1, "unread next message wakes a new attempt");
            assert_eq!(
                proof(&world, holder.0).outstanding.as_ref(),
                [next],
                "new attempt keeps only its own assignment's offers"
            );
        }
    }
}

#[test]
fn a_passed_escalation_keeps_its_party_holder_after_restart() {
    let (mut world, holder, child) = entry_world(false);
    let entry = *proof(&world, holder.0).outstanding.first().expect("escalation offered to the holder");
    decide(&mut world, holder, child, entry, false);
    let before = live_row(&world, child).escalation.clone();
    let (now, wall) = world.clocks();
    world.set_clock(now, wall);
    world.hold_switch = true;
    world.restart_with(jig_core_world::faults::plan_fixture(DECISION_SEED).0);
    assert_eq!(
        world.domain.core().tasks.task(child).expect("linked child").escalation,
        before,
        "Live({child}) loads the committed party holder before activation"
    );
    world.finish_switch();
    assert_eq!(
        live_row(&world, child).escalation,
        before,
        "Live({child}) keeps the committed party holder after activation at the same clock"
    );
}

#[test]
fn an_old_proposal_passed_below_a_live_runs_high_water_is_not_an_offer() {
    let (config, limits) = jig_core_world::faults::plan_fixture(462);
    let mut world = World::configured(462, false, config, limits);
    let holder = world.assignments[0];
    let middle = member(&world, tasks::Executor::Agent { charter: 1 }, b"middle holder");
    let middle = delegate(&mut world, holder, 1, middle);
    let middle_run = *world.assignments.iter().find(|(task, _)| *task == middle).expect("middle run");
    let mut leaf = member(&world, tasks::Executor::Agent { charter: 1 }, b"proposer");
    leaf.authority.budget.spend = LEAF_BUDGET;
    leaf.authority.delegation.tasks = 0;
    leaf.authority.delegation.depth = 0;
    let leaf = delegate(&mut world, middle_run, 1, leaf);
    let leaf_run = *world.assignments.iter().find(|(task, _)| *task == leaf).expect("leaf run");
    let authority = live_row(&world, leaf).authority.clone();
    action(
        &mut world,
        leaf_run,
        1,
        core::NamedAction::Propose {
            action: tasks::ProposalAction::Widen { task: leaf, authority },
            reason: b"review this".as_slice().into(),
            as_holder: false,
        },
    );
    let old = live_row(&world, leaf).proposal.as_ref().expect("pending proposal").number;
    say(&mut world, holder.0, 72);
    action(
        &mut world,
        middle_run,
        2,
        core::NamedAction::DecideProposal { proposer: leaf, proposal: old, choice: core::ProposalChoice::Pass },
    );
    assert!(matches!(&live_row(&world, leaf).proposal, Some(proposal)
        if proposal.number == old && matches!(proposal.state, tasks::ProposalState::Pending { holder: tasks::ProposalHolder::Task(task), .. } if task == holder.0)));
    assert!(proof(&world, holder.0).offered > Some(old));
    assert!(!proof(&world, holder.0).outstanding.contains(&old));
    let rows = world.store.rows.clone();
    let commits = world.store.applied;
    turn(&mut world, holder, 1, old);
    assert_eq!(world.store.rows, rows);
    assert_eq!(world.store.applied, commits);
}
