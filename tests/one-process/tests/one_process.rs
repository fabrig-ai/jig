use jig_one_process_world::{fixtures, world};
use skein_world::domain::Verdict;

#[test]
fn a_person_s_chat_is_answered_by_the_in_process_worker() {
    let outcome = world::run(fixtures::calm(fixtures::SEED), fixtures::one_person_chat(b"Hello"));
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert_eq!(outcome.chats_made, 1);
    assert_eq!(outcome.runs_answered, 1);
}

#[test]
fn a_signal_mid_run_drains_then_ends_the_worker_with_nothing_waiting() {
    let outcome = world::drain_mid_run(fixtures::calm(fixtures::SEED), false);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert!(outcome.drained);
    assert_eq!(outcome.chats_made, 1);
    assert_eq!(outcome.runs_answered, 0);
}

#[test]
fn the_next_start_settles_the_worker_s_claims_at_once() {
    let outcome = world::drain_mid_run(fixtures::calm(fixtures::SEED), true);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert!(outcome.drained);
    assert_eq!(outcome.claims_settled_at_restart, 1);
}

#[test]
fn a_kill_before_a_commit_loses_nothing_a_person_saw() {
    // Engine submissions from spawn, measured against main's 168 (generation
    // 1's manifest adds ten, the start's mark a write and sync): before opening
    // the first turn's transcript shard.
    const BEFORE_TURN_COMMIT: u32 = 180;
    let outcome = crash(skein_world::Cut::Kill, BEFORE_TURN_COMMIT);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert_eq!(outcome.restarts, 1);
    assert!(outcome.drained);
}
#[test]
fn a_power_loss_mid_run_restarts_and_adopts_nothing_from_the_dead_worker() {
    // Engine submissions from spawn, measured against main's 183 (generation
    // 1's manifest adds ten, the start's mark a write and sync): before closing
    // agent stdout.
    const BEFORE_AGENT_OUTPUT_CLOSE: u32 = 195;
    let outcome = crash(skein_world::Cut::PowerLoss, BEFORE_AGENT_OUTPUT_CLOSE);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert_eq!(outcome.restarts, 1);
    assert_eq!(outcome.claims_settled_at_restart, 1);
    assert!(outcome.drained);
}

fn crash(cut: skein_world::Cut, before: u32) -> world::Outcome {
    world::crash(fixtures::calm(fixtures::SEED), cut, before)
}

#[test]
fn an_open_text_reply_parks_after_one_call() {
    let chat = text_chat(fixtures::Chat::OpenText);
    assert_eq!(chat.calls, 1);
    assert_eq!(chat.reply, fixtures::TEXT);
    assert_eq!(chat.last_reply, 1);
    assert_eq!(chat.phase, jig_domain_tasks::Phase::Active(jig_domain_tasks::Active::Idle));
    assert_eq!(chat.tries, jig_domain_tasks::Tries::NONE);
    assert!(chat.idle_elapsed);
    assert_eq!(chat.claims, 1);
    assert_eq!(chat.mode, Some(smith_charter::Mode::Open));
}

#[test]
fn a_finite_text_reply_fails_as_a_run_and_retries() {
    let chat = text_chat(fixtures::Chat::FiniteText);
    assert_eq!(chat.reply, [fixtures::TEXT, fixtures::TEXT].concat());
    assert_eq!(chat.last_reply, 2);
    assert_eq!(chat.calls, 2, "initial completion and one unfinished nudge");
    assert_eq!(chat.tries.run, 1);
    assert_eq!(chat.tries.invalid, 0);
    assert!(matches!(chat.phase, jig_domain_tasks::Phase::Active(jig_domain_tasks::Active::BackingOff { .. })));
    assert_eq!(chat.claims, 2);
    assert_eq!(chat.mode, Some(smith_charter::Mode::Finite));
}

fn text_chat(chat: fixtures::Chat) -> jig_one_process_world::referee::ChatOutcome {
    let mut settings = fixtures::calm(fixtures::SEED);
    settings.chat = chat;
    let outcome = world::run(settings, Box::new([jig_one_process_world::Script::start_chat(b"Hello")]));
    assert_eq!(outcome.verdict, Verdict::Passed);
    outcome.chat.expect("text scenario evidence")
}
