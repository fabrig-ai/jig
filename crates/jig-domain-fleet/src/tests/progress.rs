//! Live progress through the owner boundary and borrowed public observations.
use super::*;

#[test]
fn progress_is_fenced_and_preserves_receipt_times_without_placement_claims() {
    use crate::{Activity, Report};
    let mut h = Harness::new(LIMITS);
    h.hello(C1, 2, &[], &[]);
    assert_eq!(h.place(R1, A1, 1), C1);
    assert!(h.domain.observation(R1, A1).is_none());
    h.hello(C2, 2, &[], &[]);
    assert!(h.step(Event::Observe { channel: C2, run: R1, attempt: A1, report: Report::Started }).is_empty());
    assert!(h.domain.observation(R1, A1).is_none());
    h.env.wall = Wall::from_nanos(4_000_000_000);
    assert_eq!(
        &*h.step(Event::Observe { channel: C1, run: R1, attempt: A1, report: Report::Started }),
        &[Request::Observed { run: R1, attempt: A1 }]
    );
    let first = h.env.wall;
    let observation = h.domain.observation(R1, A1).unwrap();
    assert_eq!(observation.started, Some(first));
    assert_eq!(observation.since, first);
    h.env.wall = Wall::from_nanos(6_000_000_000);
    let tool = Activity::Tool { call: Box::from(&b"call"[..]), tool: Box::from(&b"inspect"[..]) };
    h.step(Event::Observe { channel: C1, run: R1, attempt: A1, report: Report::Activity(tool.clone()) });
    let tool_since = h.env.wall;
    h.env.wall = Wall::from_nanos(8_000_000_000);
    h.step(Event::Observe { channel: C1, run: R1, attempt: A1, report: Report::Activity(tool.clone()) });
    h.step(Event::Observe { channel: C1, run: R1, attempt: A1, report: Report::Started });
    let observation = h.domain.observation(R1, A1).unwrap();
    assert_eq!(observation.activity, tool);
    assert_eq!(observation.since, tool_since);
    assert_eq!(observation.started, Some(first));
    assert_eq!(observation.reported_at, h.env.wall);
    h.step(Event::Observe { channel: C1, run: R1, attempt: A1, report: Report::Activity(Activity::Waiting) });
    assert_eq!(h.domain.observation(R1, A1).unwrap().activity, Activity::Waiting);
    h.step(Event::Lost { channel: C1 });
    assert!(h.domain.observation(R1, A1).is_none());
    h.hello(C3, 2, &[], &[hosted(R1, A1, Phase::Waiting)]);
    assert!(h.domain.observation(R1, A1).is_none(), "hello invents no progress or start clock");
    assert!(h.step(Event::Observe { channel: C1, run: R1, attempt: A1, report: Report::Started }).is_empty());
    h.step(Event::Observe { channel: C3, run: R1, attempt: A1, report: Report::Activity(Activity::Waiting) });
    assert_eq!(h.domain.observation(R1, A1).unwrap().started, None);
    h.step(Event::Cancel { run: R1, attempt: A1 });
    assert!(h.domain.observation(R1, A1).is_none());
    assert!(h.step(Event::Observe { channel: C3, run: R1, attempt: A1, report: Report::Started }).is_empty());
    let fresh = Domain::new(&LIMITS, fixtures::needs(), Box::new([]));
    assert!(fresh.observation(R1, A1).is_none(), "best-effort observations are volatile");
}

#[test]
fn progress_names_are_whole_bounded_and_dropped_reports_do_not_replace_the_last_fact() {
    use crate::{Activity, Report};
    let mut h = Harness::new(LIMITS);
    h.hello(C1, 2, &[], &[]);
    h.place(R1, A1, 1);
    h.step(Event::Observe { channel: C1, run: R1, attempt: A1, report: Report::Activity(Activity::Writing) });
    h.step(Event::Observe { channel: C1, run: R1, attempt: A1, report: Report::Started });
    assert_eq!(
        h.domain.observation(R1, A1).unwrap().activity,
        Activity::Writing,
        "a delayed lifecycle fact does not erase already reported work"
    );
    for (call_bytes, tool_bytes) in [(65, 64), (64, 65)] {
        let dropped = h.domain.facts_lost();
        let report = Report::Activity(Activity::Tool {
            call: Box::from(&[1; 65][..call_bytes]),
            tool: Box::from(&[2; 65][..tool_bytes]),
        });
        assert!(h.step(Event::Observe { channel: C1, run: R1, attempt: A1, report }).is_empty());
        assert_eq!(h.domain.observation(R1, A1).unwrap().activity, Activity::Writing);
        assert_eq!(h.domain.facts_lost(), dropped, "diagnostic room is independent of progress");
    }
    h.step(Event::Observe {
        channel: C1,
        run: R1,
        attempt: A1,
        report: Report::Activity(Activity::Tool { call: Box::from(&[1; 64][..]), tool: Box::from(&[2; 64][..]) }),
    });
    let saved = h.domain.observation(R1, A1).unwrap();
    match &saved.activity {
        Activity::Tool { call, tool } => {
            assert_eq!(call.len(), 64);
            assert_eq!(tool.len(), 64);
        }
        Activity::Preparing
        | Activity::Starting
        | Activity::Active
        | Activity::Writing
        | Activity::Waiting
        | Activity::Long { .. }
        | Activity::Ending => panic!("whole tool observation"),
    }
    h.step(Event::Start {
        placement: None,
        version: crate::Version { charter: 1, revision: 1 },
        grants: Box::new([]),
        reply_to: to(A2),
        run: R1,
        attempt: A2,
        workstream: 1,
        kinds: Kinds::Workers,
        assignment: Assignment { turns: Token::new(0), answered: Token::new(0) },
    });
    assert!(h.domain.observation(R1, A1).is_none(), "replacement fences the previous observation");
    assert!(h.domain.observation(R1, A2).is_none(), "new attempt has no inherited fact");
}

fn live_progress() -> Harness {
    let mut harness = Harness::new(LIMITS);
    harness.hello(C1, 1, &[], &[]);
    assert_eq!(harness.place(R1, A1, 1), C1);
    harness
}

fn report_progress(harness: &mut Harness, report: crate::Report) {
    assert_eq!(
        harness.step(Event::Observe { channel: C1, run: R1, attempt: A1, report }).as_ref(),
        &[Request::Observed { run: R1, attempt: A1 }]
    );
}

#[test]
fn a_long_completion_is_observed_as_its_kind_and_span() {
    let mut harness = live_progress();
    let activity = crate::Activity::Long { operation: crate::Operation::Completion, span: Duration::from_secs(37) };
    report_progress(&mut harness, crate::Report::Activity(activity.clone()));
    assert_eq!(observation(&harness).activity, activity);
}

#[test]
fn a_count_refreshes_the_report_time_only() {
    let mut harness = live_progress();
    harness.env.wall = Wall::from_nanos(10);
    report_progress(&mut harness, crate::Report::Started);
    let before = observation(&harness).clone();
    harness.env.wall = Wall::from_nanos(20);
    report_progress(
        &mut harness,
        crate::Report::Count { kind: crate::CountKind::LlmFinished, elapsed: Duration::from_nanos(17), count: 3 },
    );
    let after = observation(&harness);
    assert_eq!(after.reported_at, Wall::from_nanos(20));
    assert_eq!(after.activity, before.activity);
    assert_eq!(after.since, before.since);
    assert_eq!(after.started, before.started);
    assert_eq!(after.channel, before.channel);
}

fn observation(harness: &Harness) -> &crate::Observation {
    harness.domain.observation(R1, A1).unwrap()
}
