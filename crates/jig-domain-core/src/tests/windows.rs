use super::*;

fn window_core(resume_bytes: u32) -> Core {
    let mut config = config(1);
    config.settings.resume_bytes = resume_bytes;
    let mut core = Core::new(config, &limits()).expect("fixture core configuration");
    core.begin_transcript(1, 1);
    window(&mut core).window = Some((1, 1));
    core
}

fn window(core: &mut Core) -> &mut Transcript {
    core.transcripts.get_mut(&1).expect("begun window")
}

fn resumed(core: &mut Core) -> Box<[Box<[u8]>]> {
    let transcript = core.transcripts.remove(&1).expect("loaded window");
    core.resumed_turns(transcript)
}

fn append(core: &mut Core, number: u32, body: &[u8]) {
    assert!(core.append_transcript(
        1,
        TurnRecord {
            task: 1,
            attempt: 1,
            turn: number,
            spent: 0,
            read: None,
            opens_window: number == 1,
            at: Wall::EPOCH,
            transcript: body.into(),
        },
        Some(body.into()),
    ));
}

#[test]
fn many_one_byte_turns_start_fresh_when_their_lengths_pass_the_limit() {
    let mut core = window_core(200);
    for number in 1..=40 {
        append(&mut core, number, b"a");
    }
    assert!(!core.transcript_oversized(1));
    assert_eq!(window(&mut core).turns.len(), 40);
    append(&mut core, 41, b"b");
    assert!(core.transcript_oversized(1));
    assert_eq!(window(&mut core).turns.len(), 40);
    assert!(resumed(&mut core).is_empty());
}

#[test]
fn empty_turns_count_their_lengths_and_resume_in_order() {
    let mut core = window_core(9);
    append(&mut core, 1, b"");
    append(&mut core, 2, b"a");
    assert_eq!(resumed(&mut core).as_ref(), [Box::<[u8]>::from(&b""[..]), Box::from(&b"a"[..])]);
}

#[test]
fn an_oversized_resume_turn_keeps_its_text_independently() {
    let mut core = window_core(8);
    append(&mut core, 1, b"abcdefgh");
    let tail = read_brief_part(
        &core,
        BriefTextLimits { parts: 1, read_bytes: 64, brief_bytes: 64 },
        1,
        BriefPart::TranscriptTail,
    )
    .expect("window tail");
    assert_eq!(tail.as_ref(), b"abcdefgh");
}

#[test]
fn a_limit_smaller_than_a_length_retains_no_turn() {
    let mut core = window_core(3);
    append(&mut core, 1, b"");
    assert!(core.transcript_oversized(1));
    assert!(window(&mut core).turns.is_empty());
}

fn tail(core: &Core, bytes: u32) -> Box<[u8]> {
    read_brief_part(
        core,
        BriefTextLimits { parts: 1, read_bytes: bytes, brief_bytes: 256 },
        1,
        BriefPart::TranscriptTail,
    )
    .expect("rendered tail")
}

#[test]
fn a_fresh_brief_keeps_whole_newest_texts_under_its_own_budget() {
    let (mut core, mut limits) = make_tree();
    limits.brief_core_budgets.tail = 90;
    limits.brief_core_budgets.task = 128;
    core.settings.tail_bytes = 90;
    core.current.test_charter_mut(1).policy.resume = false;
    send_task(&mut core, &limits, tasks::Event::Prepare { reply_to: ReplyTo::new(Token::new(1)), task: 1 });
    let context = core.tasks.run_context(1).unwrap();
    core.contexts.insert(1, context).unwrap();
    core.begin_transcript(1, 1);
    window(&mut core).window = Some((1, 1));
    for (number, body) in [(1, [b'a'; 40]), (2, [b'b'; 40]), (3, [b'c'; 40])] {
        append(&mut core, number, &body);
    }
    let mut expected = [b'c'; 80];
    expected.get_mut(..40).unwrap().fill(b'b');
    assert_eq!(tail(&core, 90).as_ref(), expected);
    let out = watch_outputs(&mut core, &limits, Event::BriefNotes { task: 1, lines: List::with_capacity(0), more: 0 });
    let mut planned = false;
    for request in out {
        if let Request::Now(now) = request
            && let Now::BriefCorePlanned { sections, .. } = *now
        {
            for section in &sections {
                if let jig_domain_brief::Planned::Core {
                    kind: jig_domain_brief::Core::TranscriptTail,
                    text,
                    limit,
                    ..
                } = section
                {
                    assert_eq!(*limit, 90);
                    assert_eq!(text.as_ref(), expected);
                    planned = true;
                }
            }
        }
    }
    assert!(planned, "fresh brief plans its text tail");
}

#[test]
fn a_missing_rendering_is_a_readable_line() {
    let mut core = window_core(256);
    assert!(core.append_transcript(
        1,
        TurnRecord {
            task: 1,
            attempt: 1,
            turn: 1,
            spent: 0,
            read: None,
            opens_window: true,
            at: Wall::EPOCH,
            transcript: Box::from(&[0xff, 0x80][..]),
        },
        None
    ));
    assert_eq!(tail(&core, 128).as_ref(), b"[a turn not readable as text]\n");
}

#[test]
fn an_oversized_text_and_a_smaller_read_keep_utf8_suffixes_within_the_bound() {
    let mut core = window_core(256);
    core.settings.tail_bytes = 26;
    // The 7-byte suffix starts in the middle of the first é.
    append(&mut core, 1, "some earlier words éééé".as_bytes());
    let text = tail(&core, 128);
    assert_eq!(text.as_ref(), "[earlier text cut]\nééé".as_bytes());
    assert!(text.len() <= 26);
    let text = tail(&core, 24);
    assert_eq!(text.as_ref(), "[earlier text cut]\néé".as_bytes());
    assert!(text.len() <= 24);
    assert_eq!(resumed(&mut core).as_ref(), [Box::<[u8]>::from("some earlier words éééé".as_bytes())]);
}

fn closed_window() -> (Core, Limits) {
    let (mut core, limits) = watch_core();
    core.counters.deployment.runs = 7;
    let proof = core.proofs.get_mut(&1).unwrap();
    proof.window = Some(CurrentWindow { attempt: 1, turn: 1, closed: true });
    core.begin_transcript(1, 1);
    (core, limits)
}

#[test]
fn beginning_a_transcript_keeps_its_windows_closed_flag() {
    let (core, _) = closed_window();
    let transcript = core.transcripts.get(&1).unwrap();
    assert!(transcript.closed);
    assert_eq!(transcript.window, Some((1, 1)));
}

fn assert_closed_prepares(tail_bytes: u32, body: &[u8], expected_tail: &[u8]) {
    let (mut core, limits) = closed_window();
    core.settings.tail_bytes = tail_bytes;
    let tries = core.tasks.task(1).unwrap().tries;
    let refusals = core.tasks.task(1).unwrap().refusals;
    core.current.test_charter_mut(1).policy.resume = true;
    run_choice::host(&mut core, &limits, Token::new(7), &[b"scripted"]);
    let mut accounts_out = Queue::with_capacity(accounts::max_out(&limits.accounts));
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.accounts };
    accounts::step(&mut core.accounts, &env, accounts::Event::Want, &mut accounts_out);
    while accounts_out.pop().is_some() {}
    accounts::step(
        &mut core.accounts,
        &env,
        accounts::Event::Component(accounts::FromComponent::Granted {
            form: accounts::Form::Credential(accounts::Scheme::Bearer),
            account: 1,
            generation: 1,
            valid: Duration::from_secs(60),
        }),
        &mut accounts_out,
    );
    send_task(&mut core, &limits, tasks::Event::Prepare { reply_to: ReplyTo::new(Token::new(1)), task: 1 });
    core.contexts.insert(1, core.tasks.run_context(1).unwrap()).unwrap();
    append(&mut core, 1, body);
    assert_eq!(tail(&core, tail_bytes).as_ref(), expected_tail);
    let _out = watch_outputs(&mut core, &limits, Event::BriefAssembled { task: 1, ready: true });
    let attempt = *core.workspace_pending.get(&1).unwrap();
    let out = watch_outputs(
        &mut core,
        &limits,
        Event::WorkspacePrepared { task: 1, attempt, writes: Some(Box::new([])), credentials: Box::new([]) },
    );
    let mut prepared = false;
    for request in out {
        if let Request::Now(now) = request
            && let Now::RunPrepared { run, transcript, conversation, .. } = *now
        {
            assert!(!run.policy.resume);
            assert!(transcript.is_empty());
            assert_eq!(conversation, attempt);
            prepared = true;
        }
    }
    assert!(prepared, "closed window prepares a run");
    assert_eq!(core.proofs.get(&1).unwrap().window, None);
    assert_eq!(core.tasks.task(1).unwrap().tries, tries);
    assert_eq!(core.tasks.task(1).unwrap().refusals, refusals);
}

#[test]
fn a_closed_window_within_the_limit_prepares_without_resuming() {
    assert_closed_prepares(128, b"closed text", b"closed text");
}

#[test]
fn a_closed_tail_keeps_newest_text_within_its_budget() {
    let (mut core, _) = closed_window();
    core.settings.tail_bytes = 4;
    append(&mut core, 1, b"old");
    append(&mut core, 2, b"tail");
    assert_eq!(tail(&core, 4).as_ref(), b"tail");
    assert!(resumed(&mut core).is_empty());
}

// engine.md 7.2: the cut marker takes 19 bytes; this budget cannot carry it.
const TINY_TAIL: u32 = 8;

#[test]
fn a_closed_tail_needing_a_cut_omits_text_without_marker_room() {
    assert_closed_prepares(TINY_TAIL, b"text that needs cutting", b"");
}

#[test]
fn a_short_whole_closed_tail_fits_below_the_marker_size() {
    assert_closed_prepares(TINY_TAIL, b"fits all", b"fits all");
}

// Three claims put the window after the conversation's first attempt and
// before the current claim, exposing the lower bound's turn-zero hole.
#[test]
fn a_loaded_window_keeps_its_original_turn() {
    const TASK: u64 = 1;
    for turn in [0, 1] {
        let (mut core, limits) = core();
        let mut proof = RunProof::claimed(TASK, 3, 1, None, fleet::HostKind::Worker);
        proof.accounts = Box::new([authority::AccountBinding { provider: 1, account: 0, source: 1 }]);
        proof.placement = Some(Token::new(7));
        proof.turn = Some(TurnProof { turn: 1, cumulative: 0, read: None });
        proof.window = Some(CurrentWindow { attempt: 2, turn, closed: false });
        core.restoring_proofs
            .insert(TASK, RestoringProof { attempt: 3, turn: 1, run_spent: 0, last_answer: None })
            .unwrap();
        assert!(core.restore_proof(proof.clone(), limits.tasks.result_bytes));
        assert_eq!(core.proofs.get(&TASK), Some(&proof), "window turn {turn}");
    }
}

// engine.md, 7.2: a mark crosses even a claim that committed no window.
#[test]
fn beginning_a_crossed_claims_transcript_without_a_window_keeps_it_fresh() {
    let (mut core, _) = core();
    let mut proof = RunProof::claimed(1, 1, 1, None, fleet::HostKind::Worker);
    proof.crossed = true;
    core.proofs.insert(1, proof).unwrap();
    core.begin_transcript(1, 1);
    let transcript = core.transcripts.get(&1).unwrap();
    assert!(transcript.closed, "a crossed claim cannot resume even without a window");
    assert_eq!(transcript.window, None);
}
