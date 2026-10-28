use jig_channel_world::program_runs::{self, Case, Run};
use serde_json::json;
use smith_fake_claude::{Decision, End, Seen, Step, Story, Turn};
fn finish() -> Turn {
    Turn {
        steps: vec![],
        end: End::Success { structured_output: Some(json!({"outcome":{"form":"report","text":"Hello","fields":{}}})) },
    }
}
fn story(turns: Vec<Turn>) -> Story {
    Story { turns, ..Story::default() }
}
fn transcript(turn: &jig_domain_core::TurnRecord) -> smith_transcript::Turn {
    smith_transcript::Turn::decode(&smith_transcript::CEILINGS, &mut skein_lib::Reader::new(&turn.transcript))
        .expect("committed transcript")
}
fn finished(run: &Run) {
    assert!(
        run.terminals.iter().any(|answer| matches!(
            &answer.end,
            jig_domain_tasks::End::Finished {
                result: jig_domain_tasks::TaskResult::Report { words }, ..
            } if words.as_ref() == b"Hello"
        )),
        "{run:?}"
    );
}
fn same_source(run: &Run) {
    // The initial activation and the activation after parking.
    const STARTS: u32 = 2;
    assert_eq!(run.starts, STARTS);
    assert_eq!(run.grants.len(), STARTS as usize);
    assert_eq!(&run.grants[0].1, &run.grants[1].1);
    assert_eq!((&run.grants[0].2, &run.grants[0].3), (&run.grants[1].2, &run.grants[1].3));
}
#[test]
fn a_program_run_answers_through_claude() {
    let run = program_runs::run(Case::Answer, &story(vec![finish()]));
    assert!(run.observations.iter().any(|row| row.arguments.first().is_some_and(|arg| arg == "-p")), "{run:?}");
    assert!(!run.turns.is_empty());
    assert_eq!(run.grants, vec![(1, 1, 2, b"alice".to_vec())]);
    finished(&run);
}
fn limited(reset: Option<u64>) -> Run {
    let run = program_runs::run(
        Case::Limited,
        &story(vec![Turn { steps: vec![Step::RateLimit { resets_at: reset }], end: End::default() }]),
    );
    assert_eq!(run.limited.len(), 1, "{run:?}");
    assert_eq!(run.limited[0].account(), 1);
    assert_eq!(run.authenticated_failures, vec![1], "notice reaches the recorded core binding");
    run
}
#[test]
fn a_usage_limit_reaches_the_core_as_limited() {
    let reset = skein_sim::Config::calm().wall.as_nanos() / 1_000_000_000 + 123;
    let run = limited(Some(reset));
    let remaining = run.limited[0].reset().expect("remaining reset");
    assert!(remaining > skein_lib::Duration::from_secs(120) && remaining <= skein_lib::Duration::from_secs(123));
    assert!(matches!(run.held.as_slice(), [(1, Some(jig_domain_accounts::Held::Limited { reset: Some(_) }))]));
}
#[test]
fn a_usage_limit_without_a_reset_reaches_the_core_as_limited() {
    let run = limited(None);
    assert_eq!(*run.limited[0].reset(), None);
    assert_eq!(run.held, vec![(1, Some(jig_domain_accounts::Held::Limited { reset: None }))]);
}
#[test]
fn a_login_lost_mid_run_reaches_the_core_as_unauthorized() {
    let run = program_runs::run(
        Case::Unauthorized,
        &story(vec![Turn { steps: vec![Step::LoseLogin, Step::Text("continue".into())], end: End::default() }]),
    );
    assert_eq!(run.unauthorized.len(), 1, "{run:?}");
    assert_eq!(run.unauthorized[0].account(), 1);
    assert_eq!(run.unauthorized[0].generation(), 2);
    assert_eq!(run.authenticated_failures, vec![1], "notice crosses the core's source authentication");
    assert!(run.terminals.iter().any(|answer| matches!(answer.end, jig_domain_tasks::End::Failed(_))));
}
#[test]
fn a_source_signed_out_mid_run_ends_its_run_at_the_next_instruction() {
    let run = program_runs::run(
        Case::SignOut,
        &story(vec![Turn { steps: vec![Step::Call { tool: "wait".into(), input: json!({}) }], end: End::default() }]),
    );
    assert!(run.signed_out, "{run:?}");
    assert!(run.observations.iter().any(|row| row.arguments == ["auth", "logout"]));
    assert!(run.terminals.iter().all(|answer| answer.end == jig_domain_tasks::End::Recheck), "{run:?}");
    assert_eq!(run.unauthorized.len(), 1, "the signed-out login ends the live run");
    assert_eq!(run.authenticated_failures, vec![1]);
    assert!(
        run.observations.iter().any(|row| row.arguments.first().is_some_and(|arg| arg == "-p")),
        "Claude was running"
    );
}
#[test]
fn a_waiting_program_run_resumes_on_its_source() {
    let run = program_runs::run(
        Case::Waiting,
        &story(vec![
            Turn { steps: vec![Step::Call { tool: "wait".into(), input: json!({}) }], end: End::default() },
            finish(),
        ]),
    );
    assert!(run.terminals.iter().any(|answer| answer.end == jig_domain_tasks::End::Parked), "{run:?}");
    same_source(&run);
    assert!(
        run.observations.iter().any(|row| row.arguments.iter().any(|arg| arg == "--resume")),
        "the committed conversation resumes"
    );
    assert!(run.received.iter().any(|line| line.to_string().contains("After waiting")));
    finished(&run);
}
#[test]
fn an_open_program_run_replies_parks_and_resumes() {
    const REPLY: &str = "Hello again";
    let run = program_runs::run(
        Case::Open,
        &story(vec![Turn { steps: vec![Step::Text(REPLY.into())], end: End::default() }]),
    );
    same_source(&run);
    assert_eq!(run.grants, vec![(1, 1, 2, b"alice".to_vec()), (2, 1, 2, b"alice".to_vec())]);
    assert_eq!(
        run.terminals.iter().map(|answer| (answer.attempt, &answer.end)).collect::<Vec<_>>(),
        vec![(1, &jig_domain_tasks::End::Parked), (2, &jig_domain_tasks::End::Parked)],
        "{run:?}"
    );
    assert_eq!(run.sent_after_park, Some(1), "the person's next message follows the first durable park");
    assert_eq!(run.messages, vec![(1, 1, b"Hello".to_vec()), (2, 2, b"After waiting".to_vec())]);
    assert_eq!(
        run.turns.iter().map(|turn| (turn.attempt, turn.turn, turn.read)).collect::<Vec<_>>(),
        vec![(1, 1, Some(1)), (2, 1, Some(2))]
    );
    for turn in &run.turns {
        let transcript = transcript(turn);
        assert!(
            transcript.messages().iter().any(|message| {
                *message.role() == smith_transcript::Role::Assistant
                    && message.blocks().iter().any(
                        |block| matches!(block, smith_transcript::Block::Text(text) if text.text() == REPLY.as_bytes()),
                    )
            }),
            "each activation commits its text reply"
        );
    }
    let task = run.task.as_ref().expect("durable Open task");
    assert_eq!(task.flags.ended_by, jig_domain_tasks::EndedBy::Requester);
    assert_eq!(task.last_message, 2);
    assert!(task.inbox.is_empty(), "both committed fences take their messages");
    assert!(!run.run_waits.is_empty(), "the client observed the task's admission");
    assert!(!run.run_waits.iter().any(|wait| matches!(
        wait,
        Some(jig_client::contract::TaskRunWait::Refused(jig_client::contract::TaskWaitRefused::Program(_)))
    )));

    let invocations: Vec<_> = run
        .observations
        .iter()
        .filter(|row| row.arguments.first().is_some_and(|arg| arg == "-p"))
        .map(|row| &row.arguments)
        .collect();
    let [first, second] = invocations.as_slice() else { panic!("two program processes: {run:?}") };
    let argument = |args: &[String], flag: &str| {
        args.windows(2).find(|pair| pair[0] == flag).map(|pair| pair[1].clone()).expect("program session")
    };
    let session = argument(first, "--session-id");
    assert_eq!(argument(second, "--resume"), session);
    assert!(!first.iter().any(|arg| arg == "--resume"));
    assert!(!second.iter().any(|arg| arg == "--session-id"));
    assert!(invocations.iter().all(|args| !args.iter().any(|arg| arg == "--json-schema")));
    let [first_message, second_message] = run.received.as_slice() else { panic!("one user message per process") };
    assert_eq!(first_message["type"], "user");
    assert_eq!(second_message["type"], "user");
    assert!(second_message["message"]["content"].to_string().contains("After waiting"));
}
#[test]
fn a_granted_built_in_call_asks_smith_s_policy() {
    let mut turn = finish();
    turn.steps = vec![Step::Builtin {
        tool: "Read".into(),
        input: json!({"file_path":"/etc/passwd"}),
        result: "must not run".into(),
    }];
    let run = program_runs::run(Case::Builtin, &story(vec![turn]));
    assert!(run.seen.iter().any(|seen| matches!(seen,Seen::Settings { ask,.. } if ask.iter().any(|tool|tool=="Read"))));
    assert!(
        run.seen.iter().any(|seen| matches!(
            seen,
            Seen::Prompted { tool, decision: Decision::Deny(reason) } if tool == "Read" && !reason.is_empty()
        )),
        "granted family still asks policy: {run:?}"
    );
    let mut told = false;
    for turn in &run.turns {
        let transcript = transcript(turn);
        for message in transcript.messages().as_slice() {
            told |= message
                .blocks()
                .iter()
                .any(|block| matches!(block,smith_transcript::Block::Call(call) if call.name()==b"Read"));
        }
    }
    assert!(told, "the core's transcript shows the call");
    finished(&run);
}
