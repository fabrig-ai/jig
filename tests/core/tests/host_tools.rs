use jig_core_world::effects::World;
const CHILD: u64 = 2;
const REQUESTER: u64 = 1;
const EFFECT: u64 = 1;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_test_domain as root;
use skein_lib::{Duration, Token};

fn relay(world: &mut World, name: &[u8], tool: &[u8], input: &[u8]) {
    let assignment = world.assigned.as_ref().expect("assigned chat");
    world.send(root::Event::Core(core::Event::Fleet(fleet::Event::Relay {
        channel: Token::new(7),
        run: Token::new(assignment.task),
        attempt: Token::new(assignment.attempt),
        call: fleet::Call {
            name: name.into(),
            tool: tool.into(),
            input: input.into(),
            writes: true,
            deadline: Duration::from_secs(2),
        },
    })));
}

fn answer(world: &World) -> &core::SettledAnswer {
    &world.host_answers.last().expect("the host received a rendered answer").1.answer
}

#[test]
fn a_refusal_call_with_an_invalid_owning_attempt_is_rejected() {
    const SEED: u64 = 917;
    const UNKNOWN: u64 = 999_999;
    let mut world = World::new(SEED, false);
    let mut key = world.call_key(1);
    key.attempt = 0;
    let (now, wall) = world.clocks();
    let env = skein_lib::Env { now, wall, limits: jig_core_world::effects::fixture(SEED, false).1 };
    let row = root::Record::Core(core::Record::Core(core::CoreRecord::Call(core::CallRecord {
        key,
        part: core::CallPart::MessageRefused(jig_domain_tasks::Problem::new(
            Some(UNKNOWN),
            jig_domain_tasks::Refusal::Unknown,
        )),
        settled: None,
    })));
    assert!(!root::restore_record(&mut world.domain, &env, row));
}

#[test]
fn an_unknown_introduction_target_diagnostic_reads_after_restart() {
    const SEED: u64 = 913;
    const UNKNOWN: u64 = 999_999;
    const COMPLETION: u32 = 2; // The fixture's delegation is completion one.
    let mut world = World::new(SEED, false);
    world.hold_procedures = true;
    world.delegate();
    let child = world.procedure.expect("referenced delegate");
    // Introduction authorizes the known left peer; the tasks writer diagnoses
    // the unknown right peer with the same retained MessageRefused variant.
    let input = format!(r#"{{"left":{child},"right":{UNKNOWN}}}"#);
    world.tool_call_at(COMPLETION, b"introduce", input.as_bytes(), true);
    let hosted = world.assigned.as_ref().expect("running agent");
    let hosting = fleet::Hosted {
        run: Token::new(hosted.task),
        attempt: Token::new(hosted.attempt),
        phase: fleet::Phase::Active,
    };
    world.restart(SEED, false);
    assert!(world.domain.ready(), "a committed Unknown diagnostic must restore");
    world.send(root::Event::Core(core::Event::Fleet(fleet::Event::Hello {
        worker: 0,
        channel: Token::new(7),
        hello: fleet::Hello {
            bounds: jig_core_world::fixtures::bounds(),
            stop_bound: Duration::from_millis(100),
            slots: 1,
            workstreams: Box::new([]),
            hosting: Box::new([hosting]),
        },
    })));
    world.host_answers.clear();
    world.tool_call_at(COMPLETION, b"introduce", input.as_bytes(), true);
    assert_eq!(answer(&world), &core::SettledAnswer::Host { error: true, body: b"Refused: unknown".as_slice().into() });
}

#[test]
fn a_message_call_is_decoded_decided_and_answered_in_words() {
    let mut world = World::new(170, false);
    world.hold_procedures = true;
    world.delegate();
    let task = world.procedure.expect("delegated child");
    let input = format!(r#"{{"target":{task},"form":"words","words":"is the build green?"}}"#);
    world.tool_call(b"message", input.as_bytes(), true);
    assert_eq!(answer(&world), &core::SettledAnswer::Host { error: false, body: b"Sent message 1".as_slice().into() });
    let Some(root::Record::Core(core::Record::Tasks(jig_domain_tasks::Stored::Live(child)))) =
        world.store.rows.get(&root::Key::Core(core::Key::Tasks(jig_domain_tasks::Key::Live(task))))
    else {
        panic!("child");
    };
    assert_eq!(child.inbox.last().expect("message delivered").words.as_ref(), b"is the build green?");
}

#[test]
fn a_call_whose_input_does_not_decode_is_answered_an_error_and_commits_nothing() {
    let mut world = World::new(171, false);
    let commits = world.store.applied;
    world.tool_call(b"message", br#"{"form":"words","words":"is the build green?"}"#, true);
    assert_eq!(world.store.applied, commits, "a call that does not decode decides nothing");
    assert_eq!(
        answer(&world),
        &core::SettledAnswer::Host { error: true, body: b"message: the field target is missing".as_slice().into() }
    );
}

#[test]
fn a_call_asked_again_after_an_error_is_decoded_again() {
    let mut world = World::new(172, false);
    world.hold_procedures = true;
    world.delegate();
    world.tool_call(b"message", b"{}", true);
    let name = world.host_answers.last().expect("error").0.clone();
    let task = world.procedure.expect("delegated child");
    let input = format!(r#"{{"target":{task},"form":"words","words":"try again"}}"#);
    relay(&mut world, &name, b"message", input.as_bytes());
    assert_eq!(answer(&world), &core::SettledAnswer::Host { error: false, body: b"Sent message 1".as_slice().into() });
}

#[test]
fn a_call_whose_name_does_not_decode_is_answered_an_error() {
    let mut world = World::new(173, false);
    let commits = world.store.applied;
    relay(&mut world, b"bad-name", b"message", br#"{"target":1,"form":"words","words":"hello"}"#);
    assert_eq!(world.store.applied, commits);
    assert_eq!(
        answer(&world),
        &core::SettledAnswer::Host { error: true, body: b"message: the field name is malformed".as_slice().into() }
    );
}

#[test]
fn a_delegate_call_makes_its_batch_and_answers_their_numbers() {
    let mut world = World::new(174, false);
    world.hold_procedures = true;
    world.tool_call(b"delegate", br#"{"batch":[{"executor":{"kind":"procedure","connector":1,"code":1},"spec":{"words":"check"},"contract":{"kind":"report","words":64},"authority":{"tools":0,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},"budget":{"spend":20},"notes":0}}]}"#, true);
    assert_eq!(
        answer(&world),
        &core::SettledAnswer::Host { error: false, body: b"Delegated tasks: 2 at spec revision 0".as_slice().into() }
    );
    assert_eq!(world.procedure, Some(2));
}

#[test]
fn an_amend_naming_an_old_spec_revision_is_answered_moved() {
    let mut world = World::new(176, false);
    world.hold_procedures = true;
    world.delegate();
    let task = world.procedure.expect("delegated child");
    let amend = format!(
        r#"{{"target":{task},"revision":0,"amendment":{{"spec":{{"words":"check again"}},"reason":"new evidence"}}}}"#
    );
    world.tool_call(b"amend", amend.as_bytes(), true);
    world.turn(1, 0, None, b"delegate and amendment answered");
    world.turn(2, 0, None, b"record the answers");
    world.tool_call(b"amend", amend.as_bytes(), true);
    assert_eq!(
        answer(&world),
        &core::SettledAnswer::Host { error: true, body: b"Moved: task 2 is at spec revision 1 now".as_slice().into() }
    );
}

#[test]
fn an_amend_naming_its_spec_revision_is_done_with_the_next() {
    let mut world = World::new(177, false);
    world.hold_procedures = true;
    world.delegate();
    let task = world.procedure.expect("delegated child");
    let amend = format!(
        r#"{{"target":{task},"revision":0,"amendment":{{"spec":{{"words":"check again"}},"reason":"new evidence"}}}}"#
    );
    world.tool_call(b"amend", amend.as_bytes(), true);
    assert_eq!(
        answer(&world),
        &core::SettledAnswer::Host { error: false, body: b"Done: task 2 is at spec revision 1".as_slice().into() }
    );
}

#[test]
fn agents_tools_scenario_family() {
    use jig_core_world::{agents_tools::STORIES, effects::Cut};
    for story in STORIES {
        let commits = (story.run)(&mut World::new(190, false), None);
        for cut in [Cut::Submitted(commits), Cut::Durable(commits)] {
            let mut world = World::new(190, false);
            (story.run)(&mut world, Some(cut));
            assert_eq!(world.reached_cut, Some(cut), "{} reaches its cut", story.name);
        }
    }
}

#[test]
fn a_release_naming_an_old_hold_revision_is_answered_moved() {
    let story = jig_core_world::agents_tools::STORIES
        .iter()
        .find(|story| story.name == "release moved")
        .expect("release story");
    (story.run)(&mut World::new(178, false), None);
}
#[test]
fn a_release_naming_its_hold_revision_lifts_the_hold() {
    let mut world = World::new(179, false);
    world.hold_procedures = true;
    world.delegate();
    world.send(root::Event::Core(core::Event::Tasks(jig_domain_tasks::Event::Hold {
        task: CHILD,
        why: jig_domain_tasks::Hold::Budget,
    })));
    world.tool_call(b"release", format!(r#"{{"target":{CHILD},"revision":1}}"#).as_bytes(), true);
    assert_eq!(
        answer(&world),
        &core::SettledAnswer::Host {
            error: false,
            body: format!("Done: task {CHILD} is at spec revision 0").as_bytes().into()
        }
    );
    assert_ne!(jig_core_world::fixtures::live_row(&world, CHILD).revisions.hold, 0);
    assert!(!matches!(jig_core_world::fixtures::live_row(&world, CHILD).phase, jig_domain_tasks::Phase::Held { .. }));
}
#[test]
fn a_release_of_an_uncertain_effect_without_again_is_refused() {
    let mut world = World::new(180, false);
    world.hold_procedures = true;
    world.delegate();
    world.send(root::Event::Core(core::Event::Tasks(jig_domain_tasks::Event::Hold {
        task: CHILD,
        why: jig_domain_tasks::Hold::Uncertain { entry: EFFECT },
    })));
    world.tool_call(b"release", format!(r#"{{"target":{CHILD},"revision":1}}"#).as_bytes(), true);
    assert_eq!(
        answer(&world),
        &core::SettledAnswer::Host {
            error: true,
            body: format!("Refused: task {CHILD} holds an effect whose outcome is uncertain; say again: true or false")
                .as_bytes()
                .into()
        }
    );
    assert_eq!(jig_core_world::fixtures::live_row(&world, CHILD).revisions.hold, 1);
}
#[test]
fn a_decide_naming_an_old_escalation_revision_is_answered_moved() {
    let mut world = World::new(181, false);
    world.hold_procedures = true;
    world.delegate();
    for why in [jig_domain_tasks::Hold::Stopped, jig_domain_tasks::Hold::Budget] {
        world.send(root::Event::Core(core::Event::Tasks(jig_domain_tasks::Event::Hold { task: CHILD, why })));
    }
    world.tool_call(
        b"decide",
        format!(r#"{{"kind":"escalation","task":{CHILD},"revision":1,"decision":"release"}}"#).as_bytes(),
        true,
    );
    assert_eq!(
        answer(&world),
        &core::SettledAnswer::Host {
            error: true,
            body: format!("Moved: task {CHILD} is held at hold revision 2 now, for its budget").as_bytes().into()
        }
    );
}

#[test]
fn a_held_delegates_notice_reaches_its_requester_with_its_hold_revision() {
    let story = jig_core_world::agents_tools::STORIES
        .iter()
        .find(|story| story.name == "held notice")
        .expect("held notice story");
    (story.run)(&mut World::new(182, false), None);
}

const PROPOSE_AMEND: &[u8] = br#"{"action":"amend","target":2,"revision":0,"amendment":{"spec":{"words":"proposed"},"reason":"evidence"},"reason":"please"}"#;
const DIRECT_AMEND: &[u8] = br#"{"target":2,"revision":0,"amendment":{"spec":{"words":"newer"},"reason":"evidence"}}"#;
const ACCEPT_REQUEST: u8 = 71;
const PARTY_PROPOSE_REQUEST: u8 = 72;
const STALE_DECISION_AT: skein_lib::Wall = skein_lib::Wall::from_nanos(92_000_000);
fn proposed(world: &World) -> u64 {
    let part = &world.answers.last().expect("proposed answer").2;
    let core::CallPart::Proposed { proposal, .. } = part else { panic!("proposed: {part:?}") };
    *proposal
}
fn accept(world: &mut World, proposer: jig_domain_people::Proposer, proposal: u64) {
    jig_core_world::fixtures::ask_as(
        world,
        world.sign_in.expect("signed in"),
        ACCEPT_REQUEST,
        jig_domain_people::Ask::DecideProposal {
            project: 1,
            proposer,
            proposal,
            decision: jig_domain_people::ProposalDecision::Accept,
            running: None,
        },
    );
}
#[test]
fn a_proposed_amendment_naming_an_old_revision_is_answered_moved() {
    let mut world = World::new(194, false);
    world.hold_procedures = true;
    world.delegate();
    world.tool_call(b"amend", DIRECT_AMEND, true);
    world.turn(1, 0, None, b"record calls");
    world.turn(2, 0, None, b"record calls");
    world.tool_call(b"propose", PROPOSE_AMEND, true);
    assert_eq!(
        answer(&world),
        &core::SettledAnswer::Host { error: true, body: b"Moved: task 2 is at spec revision 1 now".as_slice().into() }
    );
}
#[test]
fn a_propose_call_answers_its_number_and_to_whom() {
    let mut world = World::new(195, false);
    world.hold_procedures = true;
    world.delegate();
    world.tool_call(b"propose", PROPOSE_AMEND, true);
    assert_eq!(
        answer(&world),
        &core::SettledAnswer::Host { error: false, body: b"Proposed 1, to person 1".as_slice().into() }
    );
}
#[test]
fn an_amendment_proposal_accepted_after_its_spec_moved_is_stale() {
    use jig_domain_people as people;
    let mut world = World::new(196, false);
    world.hold_procedures = true;
    world.delegate();
    world.tool_call(b"propose", PROPOSE_AMEND, true);
    let proposal = proposed(&world);
    world.turn(1, 0, None, b"record calls");
    world.turn(2, 0, None, b"record calls");
    world.tool_call(b"amend", DIRECT_AMEND, true);
    accept(&mut world, people::Proposer::Task(REQUESTER), proposal);
    jig_core_world::fixtures::refusal(&world, people::Refusal::Moved { now: people::Moved::Spec { revision: 1 } });
    assert_eq!(jig_core_world::fixtures::live_row(&world, CHILD).spec.words.as_ref(), b"newer");
    let row = world
        .store
        .rows
        .values()
        .find_map(|row| {
            if let root::Record::Core(core::Record::Tasks(jig_domain_tasks::Stored::History(row))) = row
                && row.change == jig_domain_tasks::Change::ProposalWithdrawn
                && let Some(row) = &row.proposal
                && row.number == proposal
            {
                return Some(row);
            }
            None
        })
        .expect("withdrawn proposal history");
    assert_eq!(row.state, jig_domain_tasks::ProposalState::Withdrawn);
}
#[test]
fn a_partys_amendment_proposal_accepted_after_its_spec_moved_is_stale() {
    use jig_domain_people as people;
    let mut world = World::new(197, false);
    world.hold_procedures = true;
    world.delegate();
    let sign_in = world.sign_in.expect("party");
    let mut amendment = jig_core_world::fixtures::person_unchanged(b"evidence");
    amendment.spec =
        Some(people::Spec { words: b"proposed".as_slice().into(), parameters: Box::new([]), inputs: Box::new([]) });
    jig_core_world::fixtures::ask_as(
        &mut world,
        sign_in,
        PARTY_PROPOSE_REQUEST,
        people::Ask::Propose {
            project: 1,
            action: people::Action::Amend { task: CHILD, revision: 0, amendment: Box::new(amendment) },
            reason: b"please".as_slice().into(),
        },
    );
    let Some(people::Reply::Outcome(people::Outcome::Proposed { proposal, .. })) = world.people_answers.last().cloned()
    else {
        panic!("party proposed: {:?}", world.people_answers.last())
    };
    world.tool_call(b"amend", DIRECT_AMEND, true);
    world.set_clock(skein_lib::Time::ZERO, STALE_DECISION_AT);
    accept(&mut world, people::Proposer::Party(REQUESTER), proposal);
    jig_core_world::fixtures::refusal(&world, people::Refusal::Moved { now: people::Moved::Spec { revision: 1 } });
    assert_eq!(jig_core_world::fixtures::live_row(&world, CHILD).spec.words.as_ref(), b"newer");
    let key = root::Key::Core(core::Key::Tasks(jig_domain_tasks::Key::DecidedProposal(proposal)));
    let Some(root::Record::Core(core::Record::Tasks(jig_domain_tasks::Stored::DecidedProposal(row)))) =
        world.store.rows.get(&key)
    else {
        panic!("withdrawn party proposal")
    };
    assert_eq!(row.state, jig_domain_tasks::PersonProposalState::Withdrawn { at: STALE_DECISION_AT });
}
#[test]
fn a_delegate_given_as_allowed_answers_what_it_was_given() {
    jig_core_world::agents_tools::delegate_given(&mut World::new(198, false), None);
}
#[test]
fn a_delegate_call_answers_its_tasks_spec_revisions() {
    let mut world = World::new(199, false);
    world.hold_procedures = true;
    world.delegate();
    assert_eq!(
        &world.answers.last().expect("delegate answer").2,
        &core::CallPart::Delegated(Box::new([core::Made { task: CHILD, revision: 0, given: None }]))
    );
}

const TRACK_CHILD: &[u8] = br#"{"target":2,"revision":0,"amendment":{"tracked":{"priority":7},"reason":"track"}}"#;
const PRIORITISE_CHILD: &[u8] = br#"{"target":2,"amendment":{"tracked":{"priority":8},"reason":"urgent"}}"#;
const UNTRACK_CHILD: &[u8] = br#"{"target":2,"revision":0,"amendment":{"tracked":"untracked","reason":"finished"}}"#;
const CANCEL_CHILD: &[u8] = br#"{"target":2,"reason":"wrap up","grace":5000000000}"#;

#[test]
fn a_cancel_with_a_grace_asks_the_delegate_to_finish_first() {
    let mut world = World::new(201, false);
    world.hold_procedures = true;
    world.delegate();
    world.tool_call(b"cancel", CANCEL_CHILD, true);
    assert_eq!(answer(&world), &core::SettledAnswer::Host { error: false, body: b"Done".as_slice().into() });
    let child = jig_core_world::fixtures::live_row(&world, CHILD);
    let grace = child.grace.as_ref().expect("finish requested before cancellation");
    assert_eq!(grace.by, jig_domain_tasks::Party::Task(REQUESTER));
    assert_eq!(grace.reason.as_ref(), b"wrap up");
    assert_eq!(child.inbox.last().expect("finish word").number, grace.message);
}
#[test]
fn a_runs_priority_change_without_a_revision_commits_after_a_spec_change() {
    let mut world = World::new(202, false);
    world.hold_procedures = true;
    world.delegate();
    world.tool_call(b"amend", TRACK_CHILD, true);
    world.turn(1, 0, None, b"record calls");
    world.turn(2, 0, None, b"record calls");
    world.tool_call(b"amend", DIRECT_AMEND, true);
    world.turn(3, 0, None, b"record spec amendment");
    world.tool_call(b"amend", PRIORITISE_CHILD, true);
    assert_eq!(
        answer(&world),
        &core::SettledAnswer::Host { error: false, body: b"Done: task 2 is at spec revision 1".as_slice().into() }
    );
    let child = jig_core_world::fixtures::live_row(&world, CHILD);
    assert_eq!(child.tracked, Some(8));
    assert_eq!(child.spec.words.as_ref(), b"newer");
    assert_eq!(child.revisions.spec, 1);
}
#[test]
fn a_runs_amendment_untracks_its_delegate() {
    let mut world = World::new(203, false);
    world.hold_procedures = true;
    world.delegate();
    world.tool_call(b"amend", TRACK_CHILD, true);
    assert_eq!(jig_core_world::fixtures::live_row(&world, CHILD).tracked, Some(7));
    world.turn(1, 0, None, b"record calls");
    world.turn(2, 0, None, b"record calls");
    world.tool_call(b"amend", UNTRACK_CHILD, true);
    assert_eq!(
        answer(&world),
        &core::SettledAnswer::Host { error: false, body: b"Done: task 2 is at spec revision 0".as_slice().into() }
    );
    assert_eq!(jig_core_world::fixtures::live_row(&world, CHILD).tracked, None);
}

#[test]
fn withdrawing_an_accepted_proposal_answers_already_accepted() {
    jig_core_world::agents_tools::withdraw_decided(&mut World::new(204, false), None);
}
const UNKNOWN_PROPOSAL: &[u8] = br#"{"proposal":42}"#;
#[test]
fn withdrawing_an_unknown_proposal_is_refused_unknown() {
    let mut world = World::new(205, false);
    world.tool_call(b"withdraw", UNKNOWN_PROPOSAL, true);
    assert_eq!(
        world.answers.last().expect("withdraw answer").2,
        core::CallPart::ProposalRefused(jig_domain_tasks::Problem {
            task: Some(REQUESTER),
            why: jig_domain_tasks::Refusal::Unknown,
            blocked_by: None,
            capacity: None,
        })
    );
}
