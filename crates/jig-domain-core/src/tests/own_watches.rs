//! The core names own watch subjects from live and restored sign-ins (protocol/client.md, 7).
use super::*;

fn identity(party: u8) -> people::Identity {
    people::Identity {
        key: people::IdentityKey { provider: 0, subject: Box::new([party]) },
        login: Box::from(&b"watcher"[..]),
        name: Box::from(&b"Watcher"[..]),
    }
}

fn sign_in(core: &mut Core, limits: &Limits, party: u8) -> (u64, Box<[Record]>) {
    let out = watch_outputs(
        core,
        limits,
        Event::SignIn { held: None, reply_to: ReplyTo::new(Token::new(90)), identity: identity(party) },
    );
    let mut sign_in = None;
    let mut records = List::with_capacity(u32::try_from(out.len()).expect("bounded sign-in outputs"));
    for request in out {
        match request {
            Request::Write(Write::Save(record)) => records.push(record).expect("decision records"),
            Request::Held(value) => {
                if let Held::PeopleReply { reply: people::Reply::SignedIn { sign_in: number, .. }, .. } = *value {
                    sign_in = Some(number);
                }
            }
            Request::Write(Write::Erase(_) | Write::CopySettingsRevision(_))
            | Request::Ask { .. }
            | Request::Now(_)
            | Request::ProjectMade { .. }
            | Request::Decided => {}
        }
    }
    (sign_in.expect("live sign-in reply"), records.into_boxed())
}

fn signed_core() -> (Core, Limits, u64, Box<[Record]>) {
    let limits = limits();
    let mut configuration = config(41);
    configuration.owners = Box::new([]);
    let mut core = Core::new(configuration, &limits).expect("fixture core configuration");
    drop(watch_outputs(&mut core, &limits, Event::People(people::Event::Restored)));
    let (sign_in, records) = sign_in(&mut core, &limits, 1);
    (core, limits, sign_in, records)
}

fn open(core: &mut Core, limits: &Limits, sign_in: u64, watcher: u64, subject: Watched) -> Box<[Request]> {
    watch_outputs(core, limits, Event::Watch { watcher: Token::new(watcher), sign_in, subject, window: window() })
}

fn named(core: &Core, watcher: u64) -> &Subject {
    let binding = core.view_subjects.get(&Token::new(watcher)).expect("admitted watch");
    &core.view_names.get(binding.subject).expect("bound subject").subject
}

fn frame_party(out: &[Request]) -> u64 {
    let Request::Held(value) = &out[0] else { panic!("held delivery") };
    let Held::Delivery { document, .. } = value.as_ref() else { panic!("document delivery") };
    let Document::Frame(frame) = document.as_ref() else { panic!("frame delivery") };
    frame.party
}

#[test]
fn an_own_subject_names_the_sign_ins_party() {
    let (mut core, limits, sign_in, _) = signed_core();
    for (label, subject, expected) in [
        ("Frame", Watched::Frame, Subject::Frame { party: 1 }),
        ("Inbox", Watched::Inbox, Subject::Inbox { party: 1 }),
        ("Accounts", Watched::Accounts, Subject::Accounts { party: 1 }),
    ] {
        let out = open(&mut core, &limits, sign_in, 30, subject);
        assert_eq!(named(&core, 30), &expected, "{label}");
        assert_eq!(out.len(), 2, "{label}");
        drop(watch_outputs(&mut core, &limits, Event::Unwatch { watcher: Token::new(30) }));
        drop(watch_outputs(&mut core, &limits, Event::Delivered { watcher: Token::new(30) }));
    }
}

#[test]
fn an_own_subject_after_a_restart_names_the_restored_sign_ins_party() {
    let (original, limits, sign_in, records) = signed_core();
    let mut restored = Core::new(config(42), &limits).expect("fixture core configuration");
    assert_eq!(restored.restart_begin(false), RestartRequest::Step(RestartStep::LoadCore));
    let header = original.counters.deployment();
    assert_eq!(
        restored.restore_core(CoreRecord::Deployment(header), &limits),
        Restored::Deployment { commits: header.commits }
    );
    for record in records {
        match record {
            Record::People(record) => {
                drop(watch_outputs(&mut restored, &limits, Event::People(people::Event::Restore { record })));
            }
            Record::Core(record) => {
                assert_ne!(restored.restore_core(record, &limits), Restored::Rejected);
            }
            Record::Tasks(_) | Record::Notes(_) | Record::Accounts(_) => {
                panic!("sign-in saves only its people and header")
            }
        }
    }
    drop(watch_outputs(&mut restored, &limits, Event::People(people::Event::Restored)));
    assert_eq!(restored.restart_done(RestartStep::LoadCore), RestartRequest::Step(RestartStep::LoadInbox));
    let out = open(&mut restored, &limits, sign_in, 30, Watched::Frame);
    assert_eq!(named(&restored, 30), &Subject::Frame { party: 1 });
    assert_eq!(frame_party(&out), 1);
}

#[test]
fn two_parties_frames_stay_two_subjects() {
    let (mut core, limits, first, _) = signed_core();
    let (second, _) = sign_in(&mut core, &limits, 2);
    let first_out = open(&mut core, &limits, first, 30, Watched::Frame);
    let second_out = open(&mut core, &limits, second, 31, Watched::Frame);
    assert_eq!(named(&core, 30), &Subject::Frame { party: 1 });
    assert_eq!(named(&core, 31), &Subject::Frame { party: 2 });
    assert_eq!(core.view_names.len(), 2);
    assert_eq!(frame_party(&first_out), 1);
    assert_eq!(frame_party(&second_out), 2);
}

#[test]
fn an_own_subject_of_an_ended_sign_in_is_refused_signed_out() {
    let (mut core, limits, sign_in, _) = signed_core();
    drop(watch_outputs(
        &mut core,
        &limits,
        Event::People(people::Event::SignOut { sign_in, reply_to: ReplyTo::new(Token::new(91)) }),
    ));
    for (label, subject) in [("Frame", Watched::Frame), ("Inbox", Watched::Inbox), ("Accounts", Watched::Accounts)] {
        let out = open(&mut core, &limits, sign_in, 30, subject);
        assert_watch_refusal(&out[0], WatchRefusal::SignedOut);
        assert!(core.view_subjects.is_empty(), "{label}");
    }
}

#[test]
fn an_own_accounts_subject_names_the_sign_ins_party() {
    let (mut core, limits, first, _) = signed_core();
    let (second, _) = sign_in(&mut core, &limits, 2);
    drop(open(&mut core, &limits, first, 30, Watched::Accounts));
    drop(open(&mut core, &limits, second, 31, Watched::Accounts));
    assert_eq!(named(&core, 30), &Subject::Accounts { party: 1 });
    assert_eq!(named(&core, 31), &Subject::Accounts { party: 2 });
    assert_eq!(core.view_names.len(), 2);
}

fn watching_accounts() -> (Core, Limits) {
    let (mut core, limits, sign_in, _) = signed_core();
    drop(open(&mut core, &limits, sign_in, 30, Watched::Accounts));
    drop(watch_outputs(&mut core, &limits, Event::Delivered { watcher: Token::new(30) }));
    drop(finish_watch_outputs(&mut core, &limits));
    (core, limits)
}

fn accounts_marked(core: &Core) -> bool {
    core.changed_views.contains(&core.view_subjects.get(&Token::new(30)).unwrap().subject)
}

fn hello() -> fleet::Event {
    fleet::Event::Hello {
        worker: 0,
        channel: Token::new(77),
        hello: fleet::Hello {
            bounds: fixtures::bounds(),
            stop_bound: Duration::ZERO,
            slots: 1,
            workstreams: Box::new([]),
            hosting: Box::new([]),
        },
    }
}

#[test]
fn a_fleet_hello_marks_an_open_accounts_watch() {
    let (mut core, limits) = watching_accounts();
    drop(watch_outputs(&mut core, &limits, Event::Fleet(hello())));
    assert!(accounts_marked(&core));
}

#[test]
fn a_fleet_loss_marks_an_open_accounts_watch() {
    let (mut core, limits) = watching_accounts();
    drop(watch_outputs(&mut core, &limits, Event::Fleet(hello())));
    drop(finish_watch_outputs(&mut core, &limits));
    drop(watch_outputs(&mut core, &limits, Event::Fleet(fleet::Event::Lost { channel: Token::new(77) })));
    assert!(accounts_marked(&core));
}

#[test]
fn a_host_grace_expiry_marks_an_open_accounts_watch() {
    let (mut core, limits) = watching_accounts();
    drop(watch_outputs(&mut core, &limits, Event::Fleet(fleet::Event::Loaded)));
    drop(watch_outputs(&mut core, &limits, Event::Fleet(listed_host())));
    drop(finish_watch_outputs(&mut core, &limits));
    let now = core.fleet.next_deadline().expect("adopted worker grace");
    drop(fire(&mut core, &Env { now, wall: Wall::EPOCH, limits }, Timer::Fleet));
    assert!(accounts_marked(&core));
}

#[test]
fn a_role_change_marks_an_open_accounts_watch() {
    let (mut core, limits) = watching_accounts();
    drop(watch_outputs(
        &mut core,
        &limits,
        Event::People(people::Event::Roles {
            project: people::DEPLOYMENT,
            holdings: Box::new([people::Holding { person: 1, role: people::Role::Owner }]),
        }),
    ));
    assert!(accounts_marked(&core));
}

#[test]
fn a_period_ledger_save_leaves_an_accounts_watch_unmarked() {
    let (mut core, limits) = watching_accounts();
    drop(watch_outputs(&mut core, &limits, Event::Tasks(tasks::Event::Restored)));
    let out = watch_outputs(
        &mut core,
        &limits,
        Event::Tasks(tasks::Event::OpenPeriod {
            reply_to: ReplyTo::new(Token::new(91)),
            project: 1,
            period: 1,
            budget: 1000,
        }),
    );
    let mut saved = false;
    for request in &out {
        if let Request::Write(Write::Save(Record::Tasks(tasks::Stored::Ledger(_)))) = request {
            saved = true;
        }
    }
    assert!(saved, "exercise a real period-ledger save");
    assert!(!accounts_marked(&core));
}

#[test]
fn an_attempt_acknowledgement_leaves_an_accounts_watch_unmarked() {
    let (mut core, limits) = watching_accounts();
    drop(watch_outputs(
        &mut core,
        &limits,
        Event::Fleet(fleet::Event::Acknowledge { run: Token::new(90), attempt: Token::new(92) }),
    ));
    assert!(!accounts_marked(&core));
}

fn listed_host() -> fleet::Event {
    let fleet::Event::Hello { channel, worker, mut hello } = hello() else { panic!("hello fixture") };
    hello.hosting =
        Box::new([fleet::Hosted { run: Token::new(90), attempt: Token::new(92), phase: fleet::Phase::Active }]);
    fleet::Event::Hello { channel, worker, hello }
}

fn busy() -> fleet::Event {
    fleet::Event::Answer {
        channel: Token::new(77),
        run: Token::new(90),
        attempt: Token::new(92),
        answer: fleet::Answer::Busy,
        payload: Token::new(93),
    }
}

#[test]
fn a_host_drain_marks_an_open_accounts_watch() {
    let (mut core, limits) = watching_accounts();
    drop(watch_outputs(&mut core, &limits, Event::Fleet(listed_host())));
    drop(finish_watch_outputs(&mut core, &limits));
    drop(watch_outputs(&mut core, &limits, Event::Fleet(busy())));
    assert!(accounts_marked(&core));
}

#[test]
fn lifting_a_host_drain_marks_an_open_accounts_watch() {
    let (mut core, limits) = watching_accounts();
    drop(watch_outputs(&mut core, &limits, Event::Fleet(listed_host())));
    drop(watch_outputs(&mut core, &limits, Event::Fleet(busy())));
    drop(finish_watch_outputs(&mut core, &limits));
    drop(watch_outputs(
        &mut core,
        &limits,
        Event::Fleet(fleet::Event::Cancel { run: Token::new(90), attempt: Token::new(92) }),
    ));
    assert!(accounts_marked(&core));
}

#[test]
fn whose_routes_a_read_and_its_whole_reply_and_refuses_it_while_stopping() {
    let (mut core, limits, sign_in, _) = signed_core();
    let to = Token::new(987);
    let out =
        watch_outputs(&mut core, &limits, Event::People(people::Event::Whose { reply_to: ReplyTo::new(to), sign_in }));
    let mut replies = 0_u32;
    for request in out {
        match request {
            Request::Held(value) => {
                if let Held::PeopleReply { to: destination, reply: people::Reply::Signer(signer) } = *value {
                    assert_eq!(destination.into_token(), to);
                    assert_eq!(signer, Some(people::Signer { identity: identity(1), kind: people::Kind::Person }));
                    replies = replies.saturating_add(1);
                }
            }
            Request::Write(_) => panic!("Whose writes nothing"),
            Request::Ask { .. } | Request::Now(_) | Request::ProjectMade { .. } | Request::Decided => {}
        }
    }
    assert_eq!(replies, 1);
    drop(watch_outputs(&mut core, &limits, Event::Stopping));
    let out =
        watch_outputs(&mut core, &limits, Event::People(people::Event::Whose { reply_to: ReplyTo::new(to), sign_in }));
    let mut answered = false;
    for request in out {
        match request {
            Request::Held(value) => {
                if let Held::PeopleReply { to: destination, reply } = *value {
                    assert_eq!(destination.into_token(), to);
                    assert_eq!(reply, people::Reply::Again(people::Again::NotReady));
                    answered = true;
                }
            }
            Request::Write(_) => panic!("Whose writes nothing while stopping"),
            Request::Ask { .. } | Request::Now(_) | Request::ProjectMade { .. } | Request::Decided => {}
        }
    }
    assert!(answered);
}
