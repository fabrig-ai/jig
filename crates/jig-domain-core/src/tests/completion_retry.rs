use super::*;

fn report(core: &mut Core, limits: &Limits, attempt: u64, retry: u32, offset: u32, text: &[u8]) -> Box<[Request]> {
    watch_outputs(
        core,
        limits,
        Event::RunText {
            run: Token::new(1),
            attempt: Token::new(attempt),
            turn: 1,
            retry,
            offset,
            text: Box::from(text),
            call: None,
        },
    )
}
fn current_stream(out: &[Request]) -> &Request {
    for request in out {
        match request {
            Request::Now(now) => match now.as_ref() {
                Now::RunText { .. } => return request,
                Now::Relayed { .. }
                | Now::EffectAnswer { .. }
                | Now::SettledCallRefused { .. }
                | Now::Call { .. }
                | Now::RelayBusy { .. }
                | Now::CallBusy { .. }
                | Now::DropCall { .. }
                | Now::Undelivered { .. }
                | Now::SignInRefused { .. }
                | Now::WatchRefused { .. }
                | Now::Account(..)
                | Now::NotesIndexed { .. }
                | Now::NotesRecalled { .. }
                | Now::NotesRefused { .. }
                | Now::NoteBusy { .. }
                | Now::DropPayload { .. }
                | Now::DropAssignment { .. }
                | Now::TurnPayload { .. }
                | Now::AnswerPayload { .. }
                | Now::AcceptedTurn { .. }
                | Now::RefusedPayload { .. }
                | Now::Activate { .. }
                | Now::PrepareAgent { .. }
                | Now::StartPreparation { .. }
                | Now::SeenConversation { .. }
                | Now::LoadEndedRead { .. }
                | Now::HistoricalProposal { .. }
                | Now::DecidedProposal { .. }
                | Now::HistoricalPersonTask { .. }
                | Now::EndedTask { .. }
                | Now::PartyInputs { .. }
                | Now::CompleteBrief { .. }
                | Now::BriefCorePlanned { .. }
                | Now::WorkspaceRequest { .. }
                | Now::RunPreparationFailed { .. }
                | Now::RunPrepared { .. }
                | Now::HistoricalEscalation { .. }
                | Now::RestoreRefused => {}
            },
            Request::Write(..)
            | Request::Ask { .. }
            | Request::Held(..)
            | Request::ProjectMade { .. }
            | Request::Decided => {}
        }
    }
    panic!("current supplied retry reaches the public stream boundary")
}
fn only_decided(out: &[Request]) -> bool {
    for request in out {
        match request {
            Request::Decided => {}
            Request::ProjectMade { .. }
            | Request::Write(..)
            | Request::Ask { .. }
            | Request::Held(..)
            | Request::Now(..) => return false,
        }
    }
    true
}
fn has_write(out: &[Request]) -> bool {
    for request in out {
        match request {
            Request::Write(..) => return true,
            Request::ProjectMade { .. }
            | Request::Decided
            | Request::Ask { .. }
            | Request::Held(..)
            | Request::Now(..) => {}
        }
    }
    false
}
fn has_watch_end(out: &[Request]) -> bool {
    for request in out {
        match request {
            Request::Held(held) => match held.as_ref() {
                Held::WatchEnded { .. } => return true,
                Held::ProjectionRecovery { .. }
                | Held::ApplicationDelivery { .. }
                | Held::ApplicationWatchEnded { .. }
                | Held::ReadLoad { .. }
                | Held::Read { .. }
                | Held::Applied { .. }
                | Held::CarriedUnused { .. }
                | Held::Delivery { .. }
                | Held::MakeEffect { .. }
                | Held::SettledCall { .. }
                | Held::Assign { .. }
                | Held::Grant { .. }
                | Held::Relayed { .. }
                | Held::Inbound { .. }
                | Held::Relay { .. }
                | Held::Acted { .. }
                | Held::PeopleReply { .. }
                | Held::CallAnswer { .. }
                | Held::Result { .. }
                | Held::Acknowledge { .. }
                | Held::TaskTerminalAcknowledged { .. }
                | Held::TaskTurnKept { .. }
                | Held::AcknowledgeTurn { .. }
                | Held::Drained
                | Held::Cancel { .. }
                | Held::Sources { .. }
                | Held::CheckSource { .. }
                | Held::SignOutSource { .. }
                | Held::SourceSignedOut { .. }
                | Held::Refuse { .. }
                | Held::TurnBusy { .. }
                | Held::StopRun { .. }
                | Held::Load(..)
                | Held::NotesLoad { .. }
                | Held::NotesWritten { .. }
                | Held::NotesDeleted { .. } => {}
            },
            Request::Write(..)
            | Request::Ask { .. }
            | Request::Now(..)
            | Request::ProjectMade { .. }
            | Request::Decided => {}
        }
    }
    false
}

#[test]
fn an_unwatched_empty_retry_opens_as_its_current_supplied_identity() {
    let (mut core, limits) = watch_core();
    assert!(only_decided(&report(&mut core, &limits, 7, 0, 0, b"older")));
    assert!(only_decided(&report(&mut core, &limits, 7, 1, 0, b"")));
    let opened = watch_outputs(
        &mut core,
        &limits,
        Event::WatchRun {
            watcher: Token::new(30),
            sign_in: 10,
            task: 1,
            turn: Some(views::TurnName { attempt: 7, turn: 1, retry: 0 }),
            offset: 3,
        },
    );
    assert_stream(current_stream(&opened), 30, views::TurnName { attempt: 7, turn: 1, retry: 1 }, 0, 0, b"", false);
    assert!(!has_write(&opened));
}

#[test]
fn the_last_watch_can_close_and_reopen_after_an_unwatched_retry() {
    let (mut core, limits) = watch_core();
    open_run_watch(&mut core, &limits, 30);
    report(&mut core, &limits, 7, 0, 0, b"original");
    watch_outputs(&mut core, &limits, Event::StreamDelivered { watcher: Token::new(30) });
    let ended = watch_outputs(&mut core, &limits, Event::Unwatch { watcher: Token::new(30) });
    assert!(has_watch_end(&ended));
    report(&mut core, &limits, 7, 1, 0, b"replacement");
    let opened = watch_outputs(
        &mut core,
        &limits,
        Event::Watch { watcher: Token::new(31), sign_in: 10, subject: Watched::Run { task: 1 }, window: window() },
    );
    assert_stream(
        current_stream(&opened),
        31,
        views::TurnName { attempt: 7, turn: 1, retry: 1 },
        0,
        11,
        b"replacement",
        false,
    );
}

#[test]
fn opaque_content_calls_are_attempt_fenced_without_deriving_a_text_turn() {
    let (mut core, limits) = watch_core();
    report(&mut core, &limits, 7, 1, 0, b"text");
    let call = views::Call {
        identity: views::CallIdentity { owner: Token::new(91), sequence: 8, retry: 2, id: Box::from(&[0xff, 0][..]) },
        tool: Box::from(&b"wait"[..]),
        input: Box::from(&b"{}"[..]),
        cut: false,
        began: None,
    };
    let stale = watch_outputs(
        &mut core,
        &limits,
        Event::RunCall { run: Token::new(1), attempt: Token::new(6), call: call.clone() },
    );
    assert!(only_decided(&stale));
    let supplied = watch_outputs(
        &mut core,
        &limits,
        Event::RunCall { run: Token::new(1), attempt: Token::new(7), call: call.clone() },
    );
    assert!(only_decided(&supplied), "unwatched content decides nothing");
    report(&mut core, &limits, 6, 99, 0, b"stale activation");
    let opened = watch_outputs(
        &mut core,
        &limits,
        Event::Watch { watcher: Token::new(30), sign_in: 10, subject: Watched::Run { task: 1 }, window: window() },
    );
    let Request::Now(now) = current_stream(&opened) else { panic!("now stream") };
    let Now::RunText { turn, text, call: Some(shown), .. } = now.as_ref() else { panic!("opaque current call") };
    assert_eq!(*turn, views::TurnName { attempt: 7, turn: 1, retry: 1 });
    assert_eq!(text.as_ref(), b"text");
    assert_eq!(shown.identity, call.identity);
    assert_eq!(shown.began, Some(Wall::EPOCH));
}

fn content_call(id: &[u8]) -> views::Call {
    views::Call {
        identity: views::CallIdentity { owner: Token::new(91), sequence: 8, retry: 2, id: Box::from(id) },
        tool: Box::from(&b"wait"[..]),
        input: Box::from(&b"{}"[..]),
        cut: false,
        began: None,
    }
}
fn delivered_run(out: &[Request]) -> Option<&Run> {
    for request in out {
        match request {
            Request::Held(held) => {
                if let Held::Delivery { document, .. } = held.as_ref()
                    && let Document::Run(run) = document.as_ref()
                {
                    return Some(run);
                }
            }
            Request::Write(..)
            | Request::Ask { .. }
            | Request::Now(..)
            | Request::ProjectMade { .. }
            | Request::Decided => {}
        }
    }
    None
}

#[test]
fn a_call_only_report_makes_a_paced_run_document_without_inventing_a_text_turn() {
    let (mut core, limits) = watch_core();
    open_run_watch(&mut core, &limits, 30);
    let call = content_call(&[0xff, 0, 0x80]);
    let out = watch_outputs(
        &mut core,
        &limits,
        Event::RunCall { run: Token::new(1), attempt: Token::new(7), call: call.clone() },
    );
    let shown = delivered_run(&out).expect("current call changes the ordinary paced document");
    assert_eq!((shown.task, shown.attempt, shown.committed_turn), (1, Some(7), 0));
    assert!(shown.writing.is_none());
    let current = shown.call.as_ref().unwrap();
    assert_eq!(current.identity, call.identity);
    assert_eq!((&current.tool, &current.input), (&call.tool, &call.input));
    assert_eq!(current.began, Some(Wall::EPOCH));
    assert!(!has_write(&out));
    let later = content_call(b"different");
    let held = watch_outputs(
        &mut core,
        &limits,
        Event::RunCall { run: Token::new(1), attempt: Token::new(7), call: later.clone() },
    );
    assert!(only_decided(&held), "one whole run document remains in flight");
    let taken = watch_outputs(&mut core, &limits, Event::Delivered { watcher: Token::new(30) });
    let latest = delivered_run(&taken).expect("taking the old document releases current state");
    assert!(latest.writing.is_none());
    assert_eq!(latest.call.as_ref().unwrap().identity, later.identity);
    let stale = watch_outputs(&mut core, &limits, Event::RunCall { run: Token::new(1), attempt: Token::new(6), call });
    assert!(only_decided(&stale));
}

#[test]
fn committed_main_text_keeps_its_independent_call_on_reopen_and_end_releases_it() {
    let (mut core, limits) = watch_core();
    report(&mut core, &limits, 7, 0, 0, b"accepted main text");
    let call = content_call(b"same");
    watch_outputs(
        &mut core,
        &limits,
        Event::RunCall { run: Token::new(1), attempt: Token::new(7), call: call.clone() },
    );
    core.proofs.get_mut(&1).unwrap().turn = Some(TurnProof { turn: 1, cumulative: 0, read: None });
    finish_watch_outputs(&mut core, &limits);
    let opened = watch_outputs(
        &mut core,
        &limits,
        Event::Watch { watcher: Token::new(30), sign_in: 10, subject: Watched::Run { task: 1 }, window: window() },
    );
    let shown = delivered_run(&opened).unwrap();
    assert_eq!(shown.committed_turn, 1);
    assert!(shown.writing.is_none());
    assert_eq!(shown.call.as_ref().unwrap().identity, call.identity);
    assert_eq!(shown.call.as_ref().unwrap().began, Some(Wall::EPOCH));
    watch_outputs(&mut core, &limits, Event::Unwatch { watcher: Token::new(30) });
    finish_watch_outputs(&mut core, &limits);
    let reopened = watch_outputs(
        &mut core,
        &limits,
        Event::Watch { watcher: Token::new(31), sign_in: 10, subject: Watched::Run { task: 1 }, window: window() },
    );
    assert_eq!(delivered_run(&reopened).unwrap().call, shown.call);
    core.proofs.get_mut(&1).unwrap().terminal =
        Some(TerminalRecord { task: 1, attempt: 7, cumulative: 0, end: tasks::End::Parked });
    watch_outputs(&mut core, &limits, Event::Tasks(tasks::Event::Hold { task: 1, why: tasks::Hold::Budget }));
    finish_watch_outputs(&mut core, &limits);
    let Document::Run(ended) = make(&core, &limits, 1, &Subject::Run { task: 1 }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!("ended run document")
    };
    assert_eq!(ended.attempt, None);
    assert!(ended.writing.is_none());
    assert!(ended.call.is_none());
    assert!(only_decided(&watch_outputs(
        &mut core,
        &limits,
        Event::RunCall { run: Token::new(1), attempt: Token::new(7), call }
    )));
}

fn assert_current_call_clear_is_delivered(turn: u32, retry: u32) {
    let (mut core, limits) = watch_core();
    report(&mut core, &limits, 7, 0, 0, b"older main text");
    open_run_watch(&mut core, &limits, 30);
    let call = content_call(&[0xff, 0, 0x80]);
    let shown = watch_outputs(
        &mut core,
        &limits,
        Event::RunCall { run: Token::new(1), attempt: Token::new(7), call: call.clone() },
    );
    let old = delivered_run(&shown).expect("whole current call is in flight");
    assert_eq!(old.call.as_ref().unwrap().identity, call.identity);
    let stale = report(&mut core, &limits, 6, 99, 0, b"");
    assert!(only_decided(&stale));
    let replaced = watch_outputs(
        &mut core,
        &limits,
        Event::RunText {
            run: Token::new(1),
            attempt: Token::new(7),
            turn,
            retry,
            offset: 0,
            text: Box::new([]),
            call: None,
        },
    );
    assert!(delivered_run(&replaced).is_none(), "normal document pacing keeps the old flight");
    assert!(!has_write(&replaced));
    assert_eq!(old.call.as_ref().unwrap().identity, call.identity, "captured old document stays immutable");
    let taken = watch_outputs(&mut core, &limits, Event::Delivered { watcher: Token::new(30) });
    let current = delivered_run(&taken).expect("taking the old document releases the actual call clear");
    assert_eq!((current.attempt, current.committed_turn), (Some(7), 0));
    assert!(current.call.is_none());
    let writing = current.writing.as_ref().expect("the supplied empty completion starts writing");
    assert_eq!(writing.turn, views::TurnName { attempt: 7, turn, retry });
    assert_eq!((writing.from, writing.to), (0, 0));
    assert!(writing.text.is_empty());
    assert!(writing.call.is_none());
    assert!(!has_write(&taken));
}

#[test]
fn an_empty_retry_delivers_the_independent_call_clear_through_normal_document_pacing() {
    assert_current_call_clear_is_delivered(1, 1);
}

#[test]
fn a_new_main_turn_delivers_the_independent_call_clear_without_a_tool_end_fact() {
    assert_current_call_clear_is_delivered(2, 0);
}
