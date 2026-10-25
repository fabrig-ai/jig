use jig_client::{Answered, Ask, Asker, Key, Outcome, contract as c, shapes};
use jig_client_protocol_world::{client::Story, fixtures};
use skein_lib::{Duration, RequestKey, Time, Wall};
// Five seconds exceeds the finite first bound while remaining inside the policy ceiling.
const SPAN: Duration = Duration::from_secs(5);
const WAIT: Duration = Duration::from_secs(1);
const PROJECT: u32 = 1;
const FIRST_TASK: u64 = 1;
// The owner fixture starts with no proposals; the first explicit batch is proposal one.
const FIRST_PROPOSAL: u64 = 1;
// Only the first named request makes a durable answer; repeats replay it.
const FIRST_MAKE: usize = 1;
// The goal fixture explicitly asks for ten budget units instead of an admission allotment.
const GOAL_BUDGET: u64 = 10;
// A different template duration must produce another ask under the same name.
const CHANGED_SPAN: Duration = Duration::from_secs(6);
// An application name, independent of the local request keys below.
const NAME: RequestKey = [37; 16];
const CREATE: Key = Key { asker: Asker::JIG, number: 85 };
const PROPOSE: Key = Key { asker: Asker::JIG, number: 86 };
const REPLAY: Key = Key { asker: Asker::JIG, number: 87 };
const DECIDE: Key = Key { asker: Asker::JIG, number: 88 };
fn send(story: &mut Story, key: Key, ask: c::Ask) {
    story.send(key, Ask::Jig(Box::new(ask)));
    assert!(story.run_until(|s| !s.answered(key).is_empty(), WAIT));
}
fn deadline_after(story: &Story, before: Wall) {
    let at = story.task_deadline(FIRST_TASK).expect("made with deadline").as_nanos();
    assert!((before.as_nanos() + SPAN.as_nanos()..=story.wall().as_nanos() + SPAN.as_nanos()).contains(&at));
}
#[test]
fn a_span_crosses_the_client_wire_to_task_admission() {
    let mut story = Story::live_batch(fixtures::SEED, true);
    let mut tasks = fixtures::create_tasks();
    tasks.get_mut(0).expect("first task").deadline = c::NewDeadline::Span(SPAN);
    let before = story.wall();
    send(&mut story, CREATE, c::Ask::Create { project: PROJECT, tasks });
    deadline_after(&story, before);
}
fn chat_fixture() -> (Story, c::Me, c::Text) {
    let mut story = Story::live(fixtures::SEED);
    let mut me = story.shapes_me();
    me.projects.get_mut(0).expect("project").templates.get_mut(0).expect("chat template").deadline =
        c::TemplateDeadline::Span(SPAN);
    (story, me, jig_client_world::stories::text(b"Hello"))
}
#[test]
fn a_shaped_chat_span_reaches_the_engine() {
    let (mut story, me, words) = chat_fixture();
    let ask = shapes::chat(&me, PROJECT, words).expect("chat offered");
    let before = story.wall();
    send(&mut story, CREATE, ask);
    deadline_after(&story, before);
}
fn goal_fixture() -> (Story, c::Ask, c::Text) {
    let first = Wall::from_nanos(fixtures::wall(Time::ZERO).as_nanos() + WAIT.as_nanos());
    let mut story = Story::live_span_goal(fixtures::SEED, first);
    let mut me = story.shapes_me();
    let project = me.projects.get_mut(0).expect("project");
    assert_eq!(project.creating.as_ref().expect("creating").within.latest, Some(first));
    project.templates.get_mut(1).expect("goal template").deadline = c::TemplateDeadline::Span(SPAN);
    let words = jig_client_world::stories::text(b"goal");
    let draft = shapes::GoalDraft {
        words: words.clone(),
        executor: c::OwnerExecutor::Agent { charter: 1 },
        budget: GOAL_BUDGET,
        deadline: None,
        priority: Some(7),
        llm: None,
    };
    let shapes::Judged::Create(ask) = shapes::set_goal(&me, PROJECT, draft, words.clone()) else {
        panic!("span draft within first bounds")
    };
    let c::Ask::Create { tasks, .. } = &ask else { panic!("create") };
    assert_eq!(tasks.get(0).expect("goal").deadline, c::NewDeadline::Span(SPAN));
    (story, ask, words)
}
#[test]
fn a_shaped_goal_span_beyond_authority_is_not_automatically_proposed() {
    let (mut story, ask, _) = goal_fixture();
    send(&mut story, CREATE, ask);
    assert_eq!(
        story.answered(CREATE),
        vec![jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Refused {
            refusal: c::Refusal::Authority {
                lacking: c::Lacking::Deadline { asked: None, latest: None },
                proposable: Some(true)
            },
            at: None,
        }))]
    );
    assert_eq!(story.party_proposal(1), None);
    assert_eq!(story.task_deadline(FIRST_TASK), None);
}
#[test]
fn an_explicit_span_goal_proposal_can_follow_authority_refusal() {
    let (mut story, ask, words) = goal_fixture();
    let c::Ask::Create { tasks, .. } = &ask else { panic!("create") };
    let tasks = tasks.clone();
    send(&mut story, CREATE, ask);
    send(
        &mut story,
        PROPOSE,
        c::Ask::Propose { project: PROJECT, action: c::ProposeAction::Batch { tasks }, reason: words },
    );
    assert_eq!(
        story.answered(PROPOSE),
        vec![jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(
            c::OwnerOutcome::Proposed { proposal: 1, to: 0 }
        ))))]
    );
}

fn named_send(story: &mut Story, key: Key, ask: c::Ask) {
    story.send_named(key, NAME, Ask::Jig(Box::new(ask)));
}
fn settled(story: &mut Story, key: Key) {
    assert!(story.run_until(|s| !s.answered(key).is_empty() && !s.named_record_kept(NAME), WAIT));
}
fn lose_then_reload(story: &mut Story, key: Key, ask: c::Ask) {
    story.cut_answer();
    named_send(story, key, ask);
    assert!(story.run_until(|s| s.made_under(NAME) == FIRST_MAKE, WAIT));
    story.reload();
    settled(story, key);
}
fn restart_after_time_moves(story: &mut Story) {
    // A quarter of the owner fixture's retention exceeds SPAN but keeps the sign-in live.
    let offline = Duration::from_nanos(fixtures::owner_limits().core.people.request_retention.as_nanos() / 4);
    let resumed = story.wall().as_nanos() + offline.as_nanos();
    story.restart_engine(offline);
    assert!(story.run_until(
        |s| s.wall().as_nanos() >= resumed && s.client.link().state == jig_client::Link::Live,
        offline.saturating_add(WAIT)
    ));
}
fn proposal(me: &c::Me, words: c::Text) -> c::Ask {
    let c::Ask::Create { mut tasks, .. } = shapes::chat(me, PROJECT, words.clone()).expect("chat offered") else {
        panic!("chat creates")
    };
    let task = tasks.get_mut(0).expect("proposed chat");
    task.budget = c::NewBudget::Named;
    task.authority.spend = GOAL_BUDGET;
    c::Ask::Propose { project: PROJECT, action: c::ProposeAction::Batch { tasks }, reason: words }
}
#[test]
fn a_named_span_replayed_after_time_moves_keeps_its_first_task() {
    let (mut story, me, words) = chat_fixture();
    let ask = shapes::chat(&me, PROJECT, words.clone()).expect("chat offered");
    lose_then_reload(&mut story, CREATE, ask.clone());
    let first = story.answered(CREATE);
    let [
        Outcome::Answered(Answered::Jig(c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::Created {
            first: task,
            last,
            ..
        })))),
    ] = first.as_slice()
    else {
        panic!("one created task: {first:?}")
    };
    assert_eq!((*task, *last), (FIRST_TASK, FIRST_TASK));
    let deadline = story.task_deadline(FIRST_TASK).expect("span resolved by first admission");
    restart_after_time_moves(&mut story);
    let rebuilt = shapes::chat(&me, PROJECT, words).expect("chat offered after time moved");
    assert_eq!(rebuilt, ask);
    named_send(&mut story, REPLAY, rebuilt);
    settled(&mut story, REPLAY);
    assert_eq!(story.answered(REPLAY), first);
    assert_eq!(story.made_under(NAME), FIRST_MAKE);
    assert_eq!(story.task_deadline(FIRST_TASK), Some(deadline));
}
#[test]
fn a_named_proposal_keeps_its_span_across_client_and_engine_restart() {
    let (mut story, me, words) = chat_fixture();
    let ask = proposal(&me, words.clone());
    lose_then_reload(&mut story, PROPOSE, ask.clone());
    let first = story.answered(PROPOSE);
    let pending = story.party_proposal(FIRST_PROPOSAL).expect("pending proposal").clone();
    assert_eq!(pending.batch().first().expect("proposed task").deadline, jig_domain_tasks::Deadline::Span(SPAN));
    restart_after_time_moves(&mut story);
    assert_eq!(story.party_proposal(FIRST_PROPOSAL), Some(&pending));
    let rebuilt = proposal(&me, words);
    assert_eq!(rebuilt, ask);
    named_send(&mut story, REPLAY, rebuilt);
    settled(&mut story, REPLAY);
    assert_eq!(story.answered(REPLAY), first);
    assert_eq!(story.made_under(NAME), FIRST_MAKE);
    let before = story.wall();
    send(
        &mut story,
        DECIDE,
        c::Ask::Decide {
            project: PROJECT,
            on: c::DecideOn::Proposal {
                proposal: c::ProposalKey {
                    proposer: c::Proposer::Party(fixtures::FIRST_OWNER),
                    number: FIRST_PROPOSAL,
                },
                decision: c::Decision::Accepted,
                running: None,
            },
        },
    );
    deadline_after(&story, before);
}
#[test]
fn a_changed_template_under_the_same_name_is_a_conflict() {
    let (mut story, mut me, words) = chat_fixture();
    let ask = shapes::chat(&me, PROJECT, words.clone()).expect("chat offered");
    named_send(&mut story, CREATE, ask.clone());
    settled(&mut story, CREATE);
    let deadline = story.task_deadline(FIRST_TASK).expect("first task");
    me.projects.get_mut(0).expect("project").templates.get_mut(0).expect("chat template").deadline =
        c::TemplateDeadline::Span(CHANGED_SPAN);
    let changed = shapes::chat(&me, PROJECT, words).expect("changed chat offered");
    assert_ne!(changed, ask);
    named_send(&mut story, REPLAY, changed);
    settled(&mut story, REPLAY);
    assert_eq!(
        story.answered(REPLAY),
        vec![Outcome::Answered(Answered::Jig(c::Answer::Refused { refusal: c::Refusal::KeyConflict, at: None }))]
    );
    assert_eq!(story.made_under(NAME), FIRST_MAKE);
    assert_eq!(story.task_deadline(FIRST_TASK), Some(deadline));
}

#[test]
fn requester_template_authority_crosses_shapes_and_new_task_bytes() {
    let mut story = Story::requester_chat(fixtures::SEED);
    let me = story.shapes_me();
    assert_eq!(
        me.projects
            .get(0)
            .expect("requester chat fixture")
            .templates
            .get(0)
            .expect("requester chat fixture")
            .flags
            .ended_by,
        c::EndedBy::Requester
    );
    let ask = shapes::chat(&me, PROJECT, jig_client_world::stories::text(b"Hello")).expect("requester chat fixture");
    send(&mut story, CREATE, ask);
    assert_eq!(story.task_ended_by(FIRST_TASK).expect("requester chat fixture"), jig_domain_tasks::EndedBy::Requester);
    let mut finite = me;
    finite
        .projects
        .get_mut(0)
        .expect("requester chat fixture")
        .templates
        .get_mut(0)
        .expect("requester chat fixture")
        .flags
        .ended_by = c::EndedBy::Result;
    let ask =
        shapes::chat(&finite, PROJECT, jig_client_world::stories::text(b"Finite")).expect("requester chat fixture");
    send(&mut story, REPLAY, ask);
    assert_eq!(story.task_ended_by(FIRST_TASK + 1).expect("requester chat fixture"), jig_domain_tasks::EndedBy::Result);
}

#[test]
fn requester_without_conversation_is_refused_as_malformed() {
    let mut story = Story::requester_chat(fixtures::SEED);
    let me = story.shapes_me();
    let c::Ask::Create { project, mut tasks } =
        shapes::chat(&me, PROJECT, jig_client_world::stories::text(b"Hello")).expect("requester chat fixture")
    else {
        panic!("chat creation")
    };
    tasks.get_mut(0).expect("requester chat fixture").flags.conversation = false;
    send(&mut story, CREATE, c::Ask::Create { project, tasks });
    assert_eq!(
        story.answered(CREATE),
        vec![Outcome::Answered(Answered::Jig(c::Answer::Refused {
            refusal: c::Refusal::Malformed { at: 0, why: c::Malformed::EndedBy },
            at: None
        }))]
    );
}
