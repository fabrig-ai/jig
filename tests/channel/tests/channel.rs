use jig_channel_world::{
    fixtures,
    stories::{Opening, Story},
    world,
};

fn run(story: Story) -> world::Outcome {
    jig_channel_world::stories::run(fixtures::SEED, story)
}
use skein_world::domain::Verdict;

#[test]
fn a_person_s_chat_runs_on_the_test_worker_and_is_acknowledged_once() {
    let outcome = world::run(fixtures::calm(fixtures::SEED), fixtures::one_chat(b"Hello"));
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert_eq!(outcome.turns_committed, 1);
    assert_eq!(outcome.answers_acknowledged, 1);
}

#[test]
fn a_new_application_credential_generation_reaches_a_live_attempt() {
    let outcome = run(Story::ApplicationGeneration);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert_eq!(outcome.agent_starts, 1);
    assert_eq!(outcome.turns_committed, 1);
    assert_eq!(outcome.answers_acknowledged, 1);
}

#[test]
fn a_key_grant_reaches_the_provider_as_a_key() {
    let outcome = run(Story::Key { replaced: false });
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert_eq!(outcome.agent_starts, 1);
    assert_eq!(outcome.answers_acknowledged, 1);
}

#[test]
fn a_key_replaced_mid_run_reaches_the_running_worker() {
    let outcome = run(Story::Key { replaced: true });
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert_eq!(outcome.agent_starts, 1);
    assert_eq!(outcome.answers_acknowledged, 1);
}

#[test]
fn a_run_takes_turns_and_progresses_before_its_answer() {
    let outcome = run(Story::Progress);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert_eq!(outcome.turns_committed, 2);
    assert_eq!(outcome.answers_acknowledged, 1);
}

#[test]
fn a_wrong_secret_is_refused_as_unauthorized() {
    let outcome = run(Story::Opening(Opening::Secret));
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert_eq!(outcome.refusal, Some(256));
    assert_eq!(outcome.turns_committed, 0);
}

#[test]
fn openings_with_another_family_version_application_or_terms_are_refused() {
    for (label, case, expected) in [
        ("version", Opening::Version, Some(1)),
        ("application", Opening::Application, Some(257)),
        ("terms", Opening::Terms, Some(2)),
        ("family", Opening::Family, Some(3)),
        ("garbage", Opening::Garbage, Some(3)),
    ] {
        let outcome = run(Story::Opening(case));
        assert_eq!(outcome.verdict, Verdict::Passed, "{label}");
        assert_eq!(outcome.refusal, expected, "{label}");
        assert_eq!(outcome.turns_committed, 0, "{label}");
    }
}

#[test]
fn a_held_commit_is_acknowledged_only_after_it_is_durable() {
    let outcome = run(Story::HeldCommit);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert_eq!(outcome.turns_committed, 1);
    assert_eq!(outcome.answers_acknowledged, 1);
}

#[test]
fn a_partition_seen_late_by_the_engine_places_no_attempt_early() {
    use skein_lib::Duration;
    let outcome = run(Story::Partition { engine: Duration::from_secs(18), worker: Duration::from_secs(16) });
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert_eq!(outcome.turns_committed, 1);
    assert_eq!(outcome.answers_acknowledged, 1);
}

#[test]
fn a_restarted_engine_adopts_the_worker_s_run_at_its_hello() {
    let before = jig_channel_world::stories::before_provider_query(fixtures::calm(fixtures::SEED));
    let outcome = run(Story::Restart { before });
    assert!(outcome.adopting_hellos > 0);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert_eq!(outcome.turns_committed, 1);
    assert_eq!(outcome.answers_acknowledged, 1);
}

#[test]
fn a_link_cut_mid_frame_redials_and_each_turn_reaches_the_engine_once() {
    let outcome = run(Story::MidFrame);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert!(outcome.reset_injected);
    assert_eq!(outcome.turns_committed, 1);
    assert_eq!(outcome.answers_acknowledged, 1);
}

#[test]
fn an_undecodable_charter_is_refused_as_invalid_before_the_agent_starts() {
    let outcome = run(Story::InvalidCharter);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert_eq!(outcome.turns_committed, 0);
    assert_eq!(outcome.answers_acknowledged, 0);
    assert_eq!(outcome.agent_starts, 0);
}

#[test]
fn a_relayed_call_reaches_the_agent_and_is_answered_once() {
    let outcome = run(Story::Call);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert!(outcome.named_call_error_observed, "the next actual model query carries the original call's error");
    assert!(outcome.calls_busy > 0, "the concurrent valid calls reach fleet capacity");
    assert_eq!(outcome.calls_answered, outcome.calls_busy + 2, "busy retries eventually return both recall answers");
    assert_eq!(outcome.turns_committed, 3, "oversized recall, later valid recall, then finish");
    assert_eq!(outcome.answers_acknowledged, 1);
    assert!(outcome.services_empty, "engine and worker drain after the real Smith agent finishes");
    assert!(outcome.shut_down);
}

#[test]
fn a_conversations_reply_reaches_the_views_before_its_turn() {
    let outcome = run(Story::Facts);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert!(outcome.activity_seen);
    assert_eq!(
        outcome.text_prefixes.iter().map(AsRef::as_ref).collect::<Vec<&[u8]>>(),
        [b"Hello".as_slice(), b"Hello from", b"Hello from worker"]
    );
    assert_eq!(outcome.text_bytes, b"Hello from worker".len());
    assert!(outcome.text_committed);
    assert_eq!(outcome.turns_committed, 2);
    assert_eq!(outcome.answers_acknowledged, 1);
}

#[test]
fn an_assignment_carries_its_tasks_inbox_to_the_agent() {
    let outcome = run(Story::Inbox);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert_eq!(outcome.start_messages, 1);
    assert_eq!(outcome.messages_ended, 1);
}

#[test]
fn a_run_waits_and_its_next_message_resumes_it() {
    let outcome = run(Story::Wait);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert!(outcome.waited);
    assert_eq!(outcome.turns_committed, 2);
    assert_eq!(outcome.messages_ended, 1);
}

#[test]
fn an_undecodable_transcript_is_refused_before_the_resumed_agent_starts() {
    let outcome = run(Story::InvalidTranscript);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert_eq!(outcome.agent_starts, 1);
    assert_eq!(outcome.answers_acknowledged, 1);
}

#[test]
fn a_resumed_run_is_sent_only_its_current_window() {
    let outcome = run(Story::Window);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert_eq!(outcome.windows, 1);
    assert_eq!(outcome.resumed_turns, Some(3));
    assert!(outcome.identity_kept);
}

#[test]
fn a_worker_that_stops_reading_is_lost_by_its_stall_deadline() {
    let outcome = run(Story::Stalled);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert!(outcome.channel_closed);
    assert!(outcome.heartbeats > 0, "the non-reading worker kept sending heartbeats");
}
#[test]
fn a_worker_that_goes_quiet_is_lost() {
    let outcome = run(Story::Quiet);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert!(outcome.channel_closed);
    assert_eq!(outcome.heartbeats, 0);
}

#[test]
fn a_worker_dialling_again_replaces_its_old_live_channel() {
    let outcome = run(Story::Replaced);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert!(outcome.channel_closed);
}
#[test]
fn losing_a_link_within_the_grace_keeps_the_native_run() {
    let outcome = run(Story::Reconnect);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert!(outcome.reset_injected);
    assert_eq!(outcome.agent_starts, 1);
    assert_eq!(outcome.answers_acknowledged, 1);
}
#[test]
fn losing_a_link_past_the_grace_cancels_then_ends_its_run() {
    let outcome = run(Story::BeyondGrace);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert!(outcome.contact_cancelled);
    assert_eq!(outcome.answers_acknowledged, 1);
}
#[test]
fn a_worker_shutdown_stops_its_native_run() {
    let outcome = run(Story::Shutdown);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert!(outcome.shut_down);
    assert_eq!(outcome.agent_starts, 1);
    assert!(outcome.shutdown_cancelled);
    assert_eq!(outcome.answers_acknowledged, 1);
}

#[test]
fn a_message_refused_by_the_run_is_committed_with_its_next_turn() {
    use jig_channel_world::stories::MessageRefusal;
    use jig_protocol_worker_wire::MessageRefusal as Wire;
    let mut too_large = Some(run(Story::Refusals(MessageRefusal::TooLarge)));
    let recorded =
        too_large.as_ref().expect("native refusal outcome").recorded.clone().expect("native turn and finish report");
    for (label, reason, expected) in [
        ("too large", MessageRefusal::TooLarge, Wire::TooLarge),
        ("full", MessageRefusal::Full, Wire::Full),
        ("name in use", MessageRefusal::NameInUse, Wire::NameInUse),
    ] {
        let outcome = if matches!(reason, MessageRefusal::Full | MessageRefusal::NameInUse) {
            jig_channel_world::stories::refusal(fixtures::SEED, reason, recorded.clone())
        } else {
            too_large.take().expect("one native TooLarge case")
        };
        assert_eq!(outcome.verdict, Verdict::Passed, "{label}");
        assert!(outcome.refused_messages.contains(&expected), "{label}: {:?}", outcome.refused_messages);
        assert!(outcome.messages_ended > 0, "{label}");
        assert!(outcome.refusals_kept, "{label}: refused names must be durable");
    }
}

#[test]
fn a_message_refused_while_ending_is_committed_with_the_answer() {
    let outcome = run(Story::Refusals(jig_channel_world::stories::MessageRefusal::Ending));
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert!(outcome.refused_messages.contains(&jig_protocol_worker_wire::MessageRefusal::Ending));
    assert!(outcome.messages_ended > 0);
    assert!(outcome.refusals_kept, "refused names must be durable");
}

#[test]
fn a_restarted_engine_keeps_a_refusal_carried_by_its_native_run() {
    let outcome = run(Story::Refusals(jig_channel_world::stories::MessageRefusal::Replay));
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert_eq!(outcome.agent_starts, 1);
    assert!(outcome.adopting_hellos > 0);
    assert!(outcome.refused_messages.contains(&jig_protocol_worker_wire::MessageRefusal::TooLarge));
    assert!(outcome.refusals_kept);
}

#[test]
fn a_shutdown_mid_run_drains_with_nothing_waiting_for_a_commit() {
    let outcome = run(Story::Drain);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert!(outcome.engine_drained);
    assert_eq!(outcome.commits_waiting_at_drain, 0);
    assert_eq!(outcome.turns_committed, 2);
    assert_eq!(outcome.answers_acknowledged, 1);
    assert_eq!(outcome.agent_starts, 1, "the kept turn finishes on the restarted engine");
}

#[test]
fn a_restart_reads_back_only_what_was_durable() {
    let outcome = run(Story::DurableRestart);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert!(outcome.durable_at_restart > 0, "earlier real commits were durable");
    let held = outcome.held_at_restart.expect("kill while the real turn commit is held");
    assert_eq!(held, outcome.durable_at_restart + 1);
    for number in 1..=outcome.durable_at_restart {
        assert!(outcome.recovered_commits.contains(&number), "durable commit {number} was read back");
    }
    assert!(!outcome.recovered_commits.contains(&held), "held commit {held} was absent at recovery");
    assert_eq!(outcome.turns_committed, 1);
    assert_eq!(outcome.answers_acknowledged, 1);
}

#[test]
fn two_providers_keep_real_source_bindings_across_engine_restart() {
    let outcome = run(Story::Providers);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert!(outcome.engine_drained);
    let expected = (Some(1), vec![(1, 1, 0), (2, 3, 2)]);
    assert_eq!(outcome.providers_before, expected);
    assert_eq!(outcome.providers_after, expected);
}
