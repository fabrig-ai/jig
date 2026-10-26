use super::fixtures;
use crate::{Delivery, Payload, Request, link};
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_host as host;
use jig_domain_tasks as tasks;
use skein_lib::{Duration, JournalRoom, Queue, Token};

#[test]
fn a_worker_s_hello_reaches_the_fleet_with_its_bounds() {
    let core::Event::Fleet(fleet::Event::Hello { worker: _, channel, hello }) =
        link::hello(fixtures::CHANNEL, 0, fixtures::hello())
    else {
        panic!("a hello reaches the fleet");
    };
    assert_eq!(channel, fixtures::CHANNEL);
    assert_eq!(hello.slots, 2);
    assert_eq!(hello.stop_bound, Duration::from_secs(10));
    assert_eq!(hello.workstreams.as_ref(), [4, 5]);
    assert_eq!(
        hello.hosting.as_ref(),
        [fleet::Hosted { run: fixtures::RUN, attempt: fixtures::ATTEMPT, phase: fleet::Phase::Waiting }]
    );
    assert_eq!(
        hello.bounds,
        fleet::Bounds {
            program_endpoints: Box::new([]),
            models: Box::new([fleet::ModelBound { name: b"small".as_slice().into(), window: 1024, output: 128 }]),
            turns: 8,
            spend: 100,
            time: Duration::from_secs(60),
            waiting: Duration::from_secs(30),
            checks: Duration::from_secs(1),
            message_bytes: 128,
            inbox: 4,
            unacknowledged: 2,
        }
    );
}
#[test]
fn a_turn_from_a_link_reaches_the_fleet_with_its_refusals() {
    let mut domain = fixtures::domain();
    let core::Event::Fleet(fleet::Event::Turn { channel, run, attempt, turn, body }) =
        link::up(&mut domain, fixtures::CHANNEL, fixtures::turn()).expect("routed turn")
    else {
        panic!("a turn reaches the fleet");
    };
    assert_eq!((channel, run, attempt, turn), (fixtures::CHANNEL, fixtures::RUN, fixtures::ATTEMPT, 1));
    let Some(Payload::Turn { cumulative, read, refused, opens_window, transcript, .. }) = domain.payloads.get(&body)
    else {
        panic!("turn bytes stay in the root");
    };
    assert_eq!((*cumulative, *read, *opens_window), (9, Some(7), true));
    assert_eq!(refused.as_ref(), [6]);
    assert_eq!(transcript.as_ref(), b"opaque turn");
}
#[test]
fn an_assignment_for_a_channel_leaves_as_the_hub_s_record() {
    let mut domain = fixtures::domain();
    let env = fixtures::env();
    domain.assignments.insert(fixtures::RUN.raw(), fixtures::assignment()).expect("one prepared run");
    let room = JournalRoom { writes: 0, held: 1 };
    let mut decision = domain.journal.decision(&room).expect("assignment room");
    let mut out = Queue::with_capacity(1);
    out.push(core::Request::Held(Box::new(core::Held::Assign {
        channel: fixtures::CHANNEL,
        run: fixtures::RUN,
        attempt: fixtures::ATTEMPT,
        activation: 3,
        assignment: fleet::Assignment { turns: Token::new(10), answered: Token::new(11) },
    })));
    crate::routes::route_core(&mut domain, &env, &mut decision, core::Requests::Out(out), &mut fixtures::work());
    domain.journal.accept(decision);
    let mut released = Queue::with_capacity(8);
    crate::release(&mut domain, &env, &mut released);
    let Request::Deliver(Delivery::Assigned { channel, assignment }) = released.pop().expect("assignment leaves")
    else {
        panic!("the typed assignment reaches the routing");
    };
    assert_eq!(channel, fixtures::CHANNEL);
    assert_eq!((assignment.task, assignment.attempt, assignment.conversation, assignment.charter), (2, 3, 1, 1));
    assert_eq!(assignment.run, fixtures::run());
    assert_eq!(assignment.transcript.as_ref(), [Box::<[u8]>::from(b"prior turn".as_slice())]);
    assert_eq!(assignment.grants[0].form.table(), jig_domain_accounts::GrantTable::Accounts);
}
#[test]
fn a_lost_link_reaches_the_fleet_as_its_channel_lost() {
    let event = link::lost(fixtures::CHANNEL);
    let core::Event::Fleet(fleet::Event::Lost { channel }) = event else {
        panic!("a lost link reaches the fleet");
    };
    assert_eq!(channel, fixtures::CHANNEL);
}
#[test]
fn a_fenced_result_is_handed_out_before_the_answer_is_read() {
    let mut domain = fixtures::domain();
    let (run, attempt, payload, answer) = received_answer(&mut domain, fixtures::answer());
    assert_eq!(answer, fleet::Answer::Ended);
    let mut work = fixtures::work();
    link::answer(&mut domain, run, attempt, payload, &mut work);
    assert!(work.is_empty());
    let Request::ReadResult { payload: handed, outcome } = domain.now.pop().expect("fenced result handout") else {
        panic!("opaque result leaves for reading");
    };
    assert_eq!(handed, payload);
    assert_eq!(outcome.as_ref(), b"opaque result");
    let Some(Payload::Answer { cumulative, read, refused, .. }) = domain.payloads.get(&payload) else {
        panic!("the answer stays retained");
    };
    assert_eq!((*cumulative, *read), (12, Some(7)));
    assert_eq!(refused.as_ref(), [6]);
    let event = link::result_event(&domain, payload, tasks::End::Parked).expect("result metadata");
    let core::Event::AnswerPayload { cumulative, end, read, refused, .. } = event else {
        panic!("core result input");
    };
    assert_eq!((cumulative, end, read), (12, tasks::End::Parked, Some(7)));
    assert_eq!(refused.as_ref(), [6]);
}
#[test]
fn channel_records_leave_as_the_hub_s_vocabulary() {
    let mut domain = fixtures::domain();
    domain.links.insert(fixtures::CHANNEL).expect("one connected link");
    for (held, expected) in fixtures::channel_records() {
        let Delivery::Link { channel, down } = link::down(&mut domain, held) else {
            panic!("the hub receives the record");
        };
        assert_eq!(channel, fixtures::CHANNEL);
        assert_eq!(down, expected);
    }
}

fn received_answer(domain: &mut crate::Domain, up: jig_domain_host::LinkUp) -> (Token, Token, Token, fleet::Answer) {
    let core::Event::Fleet(fleet::Event::Answer { run, attempt, answer, payload, .. }) =
        link::up(domain, fixtures::CHANNEL, up).expect("routed answer")
    else {
        panic!("an answer reaches the fleet");
    };
    (run, attempt, payload, answer)
}
#[test]
fn a_start_refusal_reaches_the_core_as_invalid_without_reading_a_result() {
    let mut domain = fixtures::domain();
    let (run, attempt, payload, answer) = received_answer(&mut domain, fixtures::start_refusal());
    assert_eq!(answer, fleet::Answer::Invalid);
    let mut work = fixtures::work();
    link::answer(&mut domain, run, attempt, payload, &mut work);
    assert!(domain.now.is_empty());
    let Some(crate::Work::Core(core::Event::AnswerPayload { end, .. })) = work.pop() else {
        panic!("invalid start reaches the core without a result codec");
    };
    assert_eq!(end, tasks::End::Failed(tasks::Class::Invalid));
}

fn core_request(request: core::Request) -> core::Requests {
    let mut out = Queue::with_capacity(1);
    out.push(request);
    core::Requests::Out(out)
}
fn gather_brief(
    domain: &mut crate::Domain,
    env: &skein_lib::Env<crate::Limits>,
    decision: &mut skein_lib::Decision<crate::Write, Delivery>,
    work: &mut Queue<crate::Work>,
) {
    use jig_domain_brief::{Core, Planned};
    let mut sections = skein_lib::List::with_capacity(2);
    for (kind, text) in [(Core::Task, b"request".as_slice()), (Core::Results, b"previous".as_slice())] {
        sections
            .push(Planned::Core { kind, text: text.into(), limit: 32, priority: 0, required: true })
            .expect("sections");
    }
    crate::routes::route_core(
        domain,
        env,
        decision,
        core_request(core::Request::Now(Box::new(core::Now::BriefCorePlanned {
            task: fixtures::RUN.raw(),
            parent: None,
            sections,
        }))),
        work,
    );
    let crate::Work::Core(core::Event::Brief(event)) = work.pop().expect("brief planned") else {
        panic!("brief child");
    };
    let mut completed = Queue::with_capacity(jig_domain_brief::gather_max_out(&env.limits.core.brief));
    jig_domain_brief::gather_step(
        &mut domain.core.brief,
        &skein_lib::Env { now: env.now, wall: env.wall, limits: env.limits.core.brief },
        event,
        &mut completed,
    );
    let jig_domain_brief::GatherRequest::Complete { brief, order } = completed.pop().expect("completed brief") else {
        panic!("complete typed sections");
    };
    crate::routes::route_core(
        domain,
        env,
        decision,
        core_request(core::Request::Now(Box::new(core::Now::CompleteBrief { brief, order }))),
        work,
    );
}
fn prepare_brief_assignment(
    domain: &mut crate::Domain,
    env: &skein_lib::Env<crate::Limits>,
    decision: &mut skein_lib::Decision<crate::Write, Delivery>,
    work: &mut Queue<crate::Work>,
) {
    let fixture = fixtures::assignment();
    domain.workspace_writes.insert(fixture.task, Box::default()).expect("workspace");
    crate::routes::route_core(
        domain,
        env,
        decision,
        core_request(core::Request::Now(Box::new(core::Now::RunPrepared {
            live: fixture.live,
            task: fixture.task,
            attempt: fixture.attempt,
            conversation: fixture.conversation,
            charter: fixture.charter,
            run: fixture.run,
            messages: fixture.messages,
            transcript: fixture.transcript,
            saved: Box::default(),
            answered: Box::default(),
            grants: fixture.grants,
        }))),
        work,
    );
    crate::routes::route_core(
        domain,
        env,
        decision,
        core_request(core::Request::Held(Box::new(core::Held::Assign {
            channel: fixtures::CHANNEL,
            run: fixtures::RUN,
            attempt: fixtures::ATTEMPT,
            activation: 3,
            assignment: fleet::Assignment { turns: fixtures::RUN, answered: fixtures::RUN },
        }))),
        work,
    );
}
#[test]
fn an_assignment_carries_its_brief_s_sections_in_order() {
    let mut domain = fixtures::domain();
    let env = fixtures::env();
    let mut decision = domain.journal.decision(&JournalRoom { writes: 0, held: 1 }).expect("assignment room");
    let mut work = fixtures::work();
    gather_brief(&mut domain, &env, &mut decision, &mut work);
    prepare_brief_assignment(&mut domain, &env, &mut decision, &mut work);
    domain.journal.accept(decision);
    let mut released = Queue::with_capacity(8);
    crate::release(&mut domain, &env, &mut released);
    let Request::Deliver(Delivery::Assigned { assignment, .. }) = released.pop().expect("assignment") else {
        panic!("typed assignment");
    };
    assert_eq!(
        assignment.brief.as_ref(),
        [
            jig_domain_brief::GatherPlaced::Core {
                kind: jig_domain_brief::Core::Task,
                text: b"request".as_slice().into()
            },
            jig_domain_brief::GatherPlaced::Core {
                kind: jig_domain_brief::Core::Results,
                text: b"previous".as_slice().into()
            },
        ]
    );
}

#[test]
fn program_source_lifecycle_routes_without_run_payloads() {
    use jig_domain_host::{LinkDown, LinkUp, Login, LoginState, SignOutResult, SourceName};
    let mut domain = fixtures::domain();
    domain.links.insert(fixtures::CHANNEL).expect("one connected worker");
    let name = SourceName::new(b"alice").expect("source name");
    let fleet_name = fleet::SourceName::new(b"alice").expect("source name");
    for (held, expected) in [
        (
            core::Held::Sources { channel: fixtures::CHANNEL, sources: Box::new([fleet_name]) },
            LinkDown::Sources { sources: Box::new([name]) },
        ),
        (core::Held::CheckSource { channel: fixtures::CHANNEL, name: fleet_name }, LinkDown::Check { name }),
        (core::Held::SignOutSource { channel: fixtures::CHANNEL, name: fleet_name }, LinkDown::SignOut { name }),
    ] {
        let Delivery::Link { channel, down } = link::down(&mut domain, Delivery::Core(held)) else {
            panic!("source down record")
        };
        assert_eq!(channel, fixtures::CHANNEL);
        assert_eq!(down, expected);
    }
    let before = domain.payloads.len();
    let inventory = link::up(
        &mut domain,
        fixtures::CHANNEL,
        LinkUp::Logins { logins: Box::new([Login { name, state: LoginState::Usable }]) },
    );
    let core::Event::Fleet(fleet::Event::Logins { channel, logins }) = inventory.expect("routed logins") else {
        panic!("typed fleet inventory")
    };
    assert_eq!(channel, fixtures::CHANNEL);
    assert_eq!(logins.as_ref(), [fleet::Login { name: fleet_name, state: fleet::LoginState::Usable }]);
    let result = link::up(&mut domain, fixtures::CHANNEL, LinkUp::SignedOut { name, result: SignOutResult::SignedOut });
    let core::Event::Fleet(fleet::Event::SignedOut { channel, name: actual, result }) =
        result.expect("routed sign-out")
    else {
        panic!("fleet sign-out result")
    };
    assert_eq!(channel, fixtures::CHANNEL);
    assert_eq!(actual, fleet_name);
    assert_eq!(result, fleet::SignOutResult::SignedOut);
    assert_eq!(domain.payloads.len(), before);
}

#[test]
#[expect(clippy::too_many_lines, reason = "one admitted projection and Stop decision plus its ordered terminals")]
fn stopping_continues_a_projection_deferred_watch_without_another_commit() {
    use skein_lib::{Env, Wall};
    let (mut domain, env, watcher) = settled_frame_watch(Wall::from_nanos(u64::MAX));
    let mut out = Queue::with_capacity(env.limits.journal.held);
    let mut policy_out = Queue::with_capacity(jig_domain_authority::POLICY_MAX_OUT);
    jig_domain_authority::step(
        &mut domain.core.authority,
        jig_domain_authority::Event::Policy {
            project: 1,
            policy: jig_domain_authority::Policy {
                ceiling: fixtures::authority(),
                projections: Box::new([]),
                escalation_role: None,
                period_spend: 100,
                roles: Box::new([]),
                requirements: Box::new([]),
            },
        },
        &mut policy_out,
    );
    assert_eq!(policy_out.pop(), Some(jig_domain_authority::PolicyFact::Added { project: 1 }));
    let goal = core::fresh(&mut domain.core.counters, core::Family::Task).expect("admitted fixture state");
    // Queue a real goal creation and Stop in one admitted decision, with
    // exactly one projection pass before acceptance.
    let room = JournalRoom { writes: env.limits.journal.writes, held: env.limits.journal.held };
    let mut decision = domain.journal.decision(&room).expect("admitted fixture state");
    let core_env = Env { now: env.now, wall: env.wall, limits: env.limits.core };
    let mut work = fixtures::work();
    for event in [
        tasks::Event::Check,
        tasks::Event::Restored,
        tasks::Event::OpenPeriod {
            reply_to: skein_lib::ReplyTo::new(Token::new(u64::MAX - 2)),
            project: 1,
            period: 1,
            budget: 100,
        },
        tasks::Event::Make {
            reply_to: skein_lib::ReplyTo::new(Token::new(u64::MAX - 3)),
            creator: tasks::Party::Deployment { project: 1 },
            batch: Box::new([tasks::New {
                number: goal,
                project: 1,
                label: 0,
                flags: tasks::Flags::PLAIN,
                executor: tasks::Executor::Agent { charter: 1 },
                llm: None,
                spec: tasks::Spec {
                    words: b"queued goal change".as_slice().into(),
                    parameters: Box::new([]),
                    inputs: Box::new([]),
                },
                contract: tasks::Contract::Report { words: 128 },
                deadline: tasks::Deadline::Named,
                authority: fixtures::run().authority,
                numbers: tasks::Numbers { budget: 100, spent: 0, spent_below: 0, reserved: 0 },
                funder: tasks::Funder::Period { project: 1, period: 1 },
                dependencies: Box::new([]),
                holdings: Box::new([]),
                wake: tasks::WakePolicy::DEFAULT,
                recurring: None,
                tracked: Some(0),
            }]),
        },
    ] {
        let requests = core::step(&mut domain.core, &core_env, core::Event::Tasks(event));
        crate::routes::route_core(&mut domain, &env, &mut decision, requests, &mut work);
    }
    assert!(domain.core.tasks.task(goal).is_some(), "the goal is live before Stop");
    let requests = core::step(&mut domain.core, &core_env, core::Event::Stopping);
    crate::routes::route_core(&mut domain, &env, &mut decision, requests, &mut work);
    for _ in 0..env.limits.routes {
        let Some(next) = work.pop() else { break };
        let crate::Work::Core(event) = next else { panic!("only the goal's core continuation") };
        let requests = core::step(&mut domain.core, &core_env, event);
        crate::routes::route_core(&mut domain, &env, &mut decision, requests, &mut work);
    }
    assert!(work.is_empty(), "ordinary handoffs finish before the projection pass");
    let requests = core::finish_decision(&mut domain.core, &core_env);
    let core::Requests::Out(ref outputs) = requests;
    let mut feeds = 0_u32;
    for request in outputs {
        if let core::Request::Ask { ask: core::Ask::ProjectGoal { .. }, .. } = request {
            feeds = feeds.checked_add(1).expect("bounded connectors");
        }
        if let core::Request::Held(held) = request
            && let core::Held::WatchEnded { .. } | core::Held::Drained = held.as_ref()
        {
            panic!("projection output must defer watch and drain terminals");
        }
    }
    assert_eq!(feeds, 2, "one projection per connector defers watches");
    crate::routes::route_core(&mut domain, &env, &mut decision, requests, &mut fixtures::work());
    let header = domain.core.counters.next_commit();
    crate::write(
        &mut domain,
        &mut decision,
        crate::Write::Save(crate::Record::Core(core::Record::Core(core::CoreRecord::Deployment(header)))),
    );
    domain.journal.accept(decision);
    crate::release(&mut domain, &env, &mut out);
    let Request::Commit { number, .. } = out.pop().expect("projection commit") else { panic!("commit first") };
    assert!(out.is_empty(), "nothing leaves ahead of the projection commit");
    crate::step(&mut domain, &env, crate::Event::Committed { number, fill: 0 });
    let mut endings = 0_u32;
    let mut drained = 0_u32;
    for _ in 0_u32..3 {
        crate::release(&mut domain, &env, &mut out);
        while let Some(request) = out.pop() {
            match request {
                Request::Commit { .. } => panic!("stopping continuation must make zero new commits"),
                Request::Deliver(Delivery::Core(core::Held::WatchEnded { watcher: got, why })) => {
                    assert_eq!((got, why), (watcher, jig_domain_views::End::Stopping));
                    endings = endings.checked_add(1).expect("bounded endings");
                }
                Request::Deliver(Delivery::Core(core::Held::Drained)) => {
                    drained = drained.checked_add(1).expect("one drain");
                }
                Request::Deliver(Delivery::Projection { .. }) => {}
                other @ (Request::ReadResult { .. }
                | Request::Busy { .. }
                | Request::Restart(_)
                | Request::Deliver(_)
                | Request::Now(_)
                | Request::Stop) => panic!("unexpected continuation output: {other:?}"),
            }
        }
    }
    assert_eq!((endings, drained), (1, 1));
}

fn settled_frame_watch(expiry: skein_lib::Wall) -> (crate::Domain, skein_lib::Env<crate::Limits>, Token) {
    use jig_domain_people as people;
    use skein_lib::Env;
    let mut domain = fixtures::domain();
    let env = fixtures::env();
    let people_env = Env { now: env.now, wall: env.wall, limits: env.limits.core.people };
    let mut ignored = Queue::with_capacity(people::max_out(&people_env.limits));
    for record in [
        people::Stored::Person {
            number: 1,
            identity: people::Identity {
                key: people::IdentityKey { provider: 0, subject: Box::new([1]) },
                login: b"owner".as_slice().into(),
                name: b"Owner".as_slice().into(),
            },
        },
        people::Stored::SignIn { number: 1, person: 1, expires: expiry },
    ] {
        people::step(&mut domain.core.people, &people_env, people::Event::Restore { record }, &mut ignored);
    }
    people::step(&mut domain.core.people, &people_env, people::Event::Restored, &mut ignored);
    let watcher = Token::new(901);
    crate::step(
        &mut domain,
        &env,
        crate::Event::Core(core::Event::Watch {
            watcher,
            sign_in: 1,
            subject: core::documents::Watched::Frame,
            window: core::documents::Window { rows: 1, after: None },
        }),
    );
    let mut out = Queue::with_capacity(env.limits.journal.held);
    crate::release(&mut domain, &env, &mut out);
    while let Some(request) = out.pop() {
        if let Request::Commit { number, .. } = request {
            crate::step(&mut domain, &env, crate::Event::Committed { number, fill: 0 });
        }
    }
    crate::release(&mut domain, &env, &mut out);
    while out.pop().is_some() {}
    crate::step(&mut domain, &env, crate::Event::Core(core::Event::Delivered { watcher }));
    (domain, env, watcher)
}

#[test]
fn direct_resume_observes_wall_expiry_without_firing_or_reordering_the_people_timer() {
    let expiry = skein_lib::Wall::from_nanos(10);
    let (mut domain, mut env, watcher) = settled_frame_watch(expiry);
    assert!(!domain.core.finish_owed());
    let commits = domain.core.counters.deployment().commits;
    env.wall = expiry;
    assert!(!domain.core.finish_owed(), "expiry belongs to a due timer, not a clocked finishing query");
    assert!(crate::timers_due(&domain, &env));
    let mut out = Queue::with_capacity(env.limits.journal.held);
    crate::release(&mut domain, &env, &mut out);
    crate::release(&mut domain, &env, &mut out);
    let Request::Deliver(Delivery::Core(core::Held::WatchEnded { watcher: got, why })) =
        out.pop().expect("admitted fixture state")
    else {
        panic!("current-clock expiry still closes the settled watch")
    };
    assert_eq!((got, why), (watcher, jig_domain_views::End::SignedOut));
    assert!(out.is_empty());
    assert_eq!(domain.core.counters.deployment().commits, commits, "Resume does not fire a writing timer");
    assert_eq!(domain.core.people.sign_in_person(1), Some(1), "timer event retains its order");
    assert_eq!(domain.core.people.next_wall_expiry(), Some(expiry));
    crate::step(&mut domain, &env, crate::Event::Timer(core::Timer::People));
    assert_eq!(domain.core.people.sign_in_person(1), None);
    assert_eq!(domain.core.counters.deployment().commits, commits.checked_add(1).expect("one timer commit"));
}

#[test]
fn the_hosts_endpoint_cap_is_the_fleets() {
    assert_eq!(host::PROGRAM_ENDPOINTS, fleet::PROGRAM_ENDPOINTS);
}

#[test]
fn a_text_piece_reaches_the_core_with_its_numbers_and_bytes() {
    let mut domain = fixtures::domain();
    for offset in [0, u32::MAX] {
        let event = link::up(
            &mut domain,
            fixtures::CHANNEL,
            host::LinkUp::Piece {
                run: fixtures::RUN,
                attempt: fixtures::ATTEMPT,
                piece: host::Piece {
                    turn: 3,
                    retry: 2,
                    kind: host::LiveKind::Text,
                    offset: u64::from(offset),
                    bytes: Box::from(&b"words"[..]),
                },
            },
        );
        let Some(core::Event::RunText { run, attempt, turn, retry, offset: actual, text, call }) = event else {
            panic!("text reaches core")
        };
        assert_eq!((run, attempt, turn, retry, actual), (fixtures::RUN, fixtures::ATTEMPT, 3, 2, offset));
        assert_eq!(text.as_ref(), b"words");
        assert_eq!(call, None);
    }
    assert_eq!(domain.pieces_dropped(), 0);
}

#[test]
fn unkept_piece_kinds_and_offsets_are_dropped_and_counted() {
    let mut domain = fixtures::domain();
    for (index, (kind, offset)) in [
        (host::LiveKind::Reasoning, 0),
        (
            host::LiveKind::Input { call: Box::from(&b"call"[..]), tool: Box::from(&b"tool"[..]) },
            u64::from(u32::MAX) + 1,
        ),
        (host::LiveKind::Text, u64::from(u32::MAX) + 1),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(
            link::up(
                &mut domain,
                fixtures::CHANNEL,
                host::LinkUp::Piece {
                    run: fixtures::RUN,
                    attempt: fixtures::ATTEMPT,
                    piece: host::Piece { turn: 3, retry: 2, kind, offset, bytes: Box::from(&b"words"[..]) },
                }
            )
            .is_none()
        );
        assert_eq!(domain.pieces_dropped(), u64::try_from(index + 1).expect("three cases"));
    }
}

#[test]
fn a_bound_failure_reaches_the_core_with_its_configured_value() {
    for (bound, expected) in [
        (host::Bound::Reasoning, tasks::Bound::Reasoning),
        (host::Bound::Metadata, tasks::Bound::Metadata),
        (host::Bound::Items, tasks::Bound::Items),
        (host::Bound::Answer, tasks::Bound::Answer),
        (host::Bound::Tools, tasks::Bound::Tools),
        (host::Bound::HistoryItems, tasks::Bound::HistoryItems),
        (host::Bound::History, tasks::Bound::History),
        (host::Bound::Window, tasks::Bound::Window),
        (host::Bound::Output, tasks::Bound::Output),
    ] {
        let mut domain = fixtures::domain();
        let host::LinkUp::Answer { run, attempt, mut answer } = fixtures::start_refusal() else {
            panic!("answer fixture")
        };
        answer.ending = host::Ending::Failed {
            failure: host::Failure::Run(host::RunFailure::Exceeded(host::Exceeded { bound, value: u64::MAX })),
            detail: Box::new([]),
            work: host::Work { left: None, saved: None },
        };
        let (run, attempt, payload, _) = received_answer(&mut domain, host::LinkUp::Answer { run, attempt, answer });
        let mut work = fixtures::work();
        link::answer(&mut domain, run, attempt, payload, &mut work);
        let Some(crate::Work::Core(core::Event::AnswerPayload { end, .. })) = work.pop() else {
            panic!("bound terminal")
        };
        assert_eq!(end, tasks::End::Exceeded(tasks::Exceeded { bound: expected, value: u64::MAX }));
    }
}

#[test]
fn an_input_piece_reaches_the_core_with_its_provider_identity() {
    let mut domain = fixtures::domain();
    let event = link::up(
        &mut domain,
        fixtures::CHANNEL,
        host::LinkUp::Piece {
            run: fixtures::RUN,
            attempt: fixtures::ATTEMPT,
            piece: host::Piece {
                turn: 3,
                retry: 2,
                offset: 0,
                bytes: Box::from(b"{}".as_slice()),
                kind: host::LiveKind::Input {
                    call: Box::from(b"call".as_slice()),
                    tool: Box::from(b"tool".as_slice()),
                },
            },
        },
    );
    let Some(core::Event::RunInput { run, attempt, turn, retry, offset, id, tool, input }) = event else {
        panic!("routed input")
    };
    assert_eq!((run, attempt, turn, retry, offset), (fixtures::RUN, fixtures::ATTEMPT, 3, 2, 0));
    assert_eq!(
        (id.as_ref(), tool.as_ref(), input.as_ref()),
        (b"call".as_slice(), b"tool".as_slice(), b"{}".as_slice())
    );
    assert_eq!(domain.pieces_dropped(), 0);
}
