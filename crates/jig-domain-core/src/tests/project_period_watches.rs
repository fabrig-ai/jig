//! Frame watches signal project period changes; rereads keep each project's own period.
//! `client/domain/contract.md`, 6, 7.1–7.2; `domain/engine.md`, 11.

use super::*;

const CONFIGURED_PROJECTS: u32 = 2; // The fixture seeds projects 1 and 2.
const DEPLOYMENT_ROSTER: u32 = 1;

fn period_core(configured: u64) -> (Core, Limits) {
    let (auth, mut limits) = watch_core();
    limits.authority.projects = CONFIGURED_PROJECTS;
    limits.people.projects = CONFIGURED_PROJECTS + DEPLOYMENT_ROSTER;
    limits.tasks.funders = 8;
    limits.views.watchers = 4;
    limits.views.watchers_per_party = 4;
    limits.documents.lines = 4;
    let mut config = config(61);
    let policy = config.seed_projects[0].policy.policy.clone();
    config.defaults.rules =
        (authority::Domain::new(config.defaults.rules.clone(), authority::Limits { projects: 2, ..limits.authority })
            .unwrap())
        .rules()
        .clone();
    let seed = |number: u32, name: &str| SeedProject {
        number,
        name: name.into(),
        policy: ConfiguredPolicy { policy: policy.clone(), permissions: Box::new([]), templates: Box::new([]) },
    };
    config.seed_projects = Box::new([seed(1, "Project 1"), seed(2, "Project 2")]);
    let mut core = seeded_core(config, &limits).expect("fixture core configuration");
    core.people = auth.people;
    send_task(&mut core, &limits, tasks::Event::Check);
    send_task(&mut core, &limits, tasks::Event::Restored);
    let opened = watch_outputs(
        &mut core,
        &limits,
        Event::Tasks(tasks::Event::OpenPeriod {
            reply_to: ReplyTo::new(Token::new(1)),
            project: 1,
            period: 1,
            budget: 1000,
        }),
    );
    assert!(has_write(&opened));
    let carved = watch_outputs(
        &mut core,
        &limits,
        Event::Tasks(tasks::Event::CarvePool {
            reply_to: ReplyTo::new(Token::new(2)),
            project: 1,
            person: 1,
            period: 1,
            budget: 177,
        }),
    );
    assert!(has_write(&carved));
    if configured > 0 {
        watch_outputs(&mut core, &limits, Event::Period { project: 2, period: configured, budget: 1000, next: None });
    }
    assert_eq!(finish_watch_outputs(&mut core, &limits).len(), 1);
    (core, limits)
}

fn has_write(out: &[Request]) -> bool {
    for request in out {
        match request {
            Request::Write(_) => return true,
            Request::ProjectMade { .. }
            | Request::Held(_)
            | Request::Now(_)
            | Request::Decided
            | Request::Ask { .. } => {}
        }
    }
    false
}

fn frame_delivery(out: &[Request], watcher: Token) -> Option<Frame> {
    let mut delivered = None;
    for request in out {
        if let Request::Held(held) = request
            && let Held::Delivery { watcher: actual, document } = held.as_ref()
            && *actual == watcher
        {
            let Document::Frame(frame) = document.as_ref() else { panic!("watched frame") };
            assert!(delivered.is_none(), "one whole delivery per watcher");
            delivered = Some(frame.clone());
        }
    }
    delivered
}

fn open_frame(core: &mut Core, limits: &Limits, watcher: Token) -> Frame {
    let initial =
        watch_outputs(core, limits, Event::Watch { watcher, sign_in: 10, subject: Watched::Frame, window: window() });
    frame_delivery(&initial, watcher).expect("authenticated first frame delivery")
}

fn assert_frame_project_pool(frame: &Frame, pool: Option<&Ledger>) {
    let mut named = 0_u32;
    for row in &frame.projects {
        if row.project == 1 {
            assert_eq!(row.pool.as_ref(), pool);
            named = named.checked_add(1).expect("bounded frame projects");
        }
    }
    assert_eq!(named, 1, "the frame names the project to reread");
}

fn held_project_read(core: &mut Core, limits: &Limits) -> (Project, Box<Held>) {
    let out = watch_outputs(
        core,
        limits,
        Event::Read { reply: ReplyTo::new(Token::new(90)), sign_in: 10, read: reads::Read::Project { project: 1 } },
    );
    let mut answer = None;
    for request in out {
        match request {
            Request::Held(held) => {
                let Held::Read { answer: reads::Answer::Document(document), .. } = held.as_ref() else {
                    panic!("authenticated held live read")
                };
                let Document::Project(project) = document.as_ref() else { panic!("project read") };
                assert!(answer.is_none(), "one held answer per read");
                answer = Some((project.as_ref().clone(), held));
            }
            Request::ProjectMade { .. } | Request::Decided => {}
            Request::Write(_) | Request::Now(_) | Request::Ask { .. } => panic!("read decides nothing"),
        }
    }
    answer.expect("authenticated project read answer")
}

fn period_outputs(core: &mut Core, limits: &Limits, project: u32, period: u64, budget: u64) -> Box<[Request]> {
    let first = watch_outputs(core, limits, Event::Period { project, period, budget, next: None });
    let finished = finish_watch_outputs(core, limits);
    let count = first.len().checked_add(finished.len()).expect("bounded period outputs");
    let mut out = List::with_capacity(u32::try_from(count).unwrap());
    for request in first.into_iter().chain(finished) {
        out.push(request).unwrap();
    }
    out.into_boxed()
}

fn assert_observed_pool(project: &Project, before: &Project) {
    assert_eq!(project.number, 1);
    assert_eq!(project.current_period, before.current_period);
    assert_eq!(project.pools, before.pools);
    assert_eq!(project.configured_pools.capacity(), 4);
    assert!(!project.configured_pools_cut);
    assert!(!project.cut);
    assert_eq!(project.pools.len(), 1);
    let pool = project.pools.get(0).unwrap();
    assert_eq!(pool.source, tasks::Funder::Pool { project: 1, person: 1, period: 1 });
    assert_eq!(pool.parent, Some(tasks::Funder::Period { project: 1, period: 1 }));
    assert_eq!(pool.numbers, tasks::Numbers { budget: 177, spent: 0, spent_below: 0, reserved: 0 });
    assert_eq!(pool.left, 177);
    assert!(!pool.closed);
    assert_eq!(pool.began, None);
    assert_eq!(pool.next, None);
}

#[test]
fn a_cross_project_period_holds_the_frame_signal_and_project_reread_until_durability() {
    let (mut core, limits) = period_core(1);
    let watcher = Token::new(30);
    let initial_frame = open_frame(&mut core, &limits, watcher);
    let (before, _) = held_project_read(&mut core, &limits);
    assert_eq!(before.configured_period, 1);
    assert_eq!(before.configured_pools, before.pools);
    assert_frame_project_pool(&initial_frame, before.pools.get(0));
    assert!(frame_delivery(&watch_outputs(&mut core, &limits, Event::Delivered { watcher }), watcher).is_none());
    let out = period_outputs(&mut core, &limits, 2, 2, 1000);
    let mut funding_writes = 0_u32;
    for request in &out {
        if let Request::Write(Write::Save(Record::Tasks(record))) = request {
            let tasks::Stored::Ledger(row) = record else {
                let tasks::Stored::ClosedLedger(row) = record else { panic!("no task in the period fixture") };
                assert_eq!(row.funder, tasks::Funder::Period { project: 2, period: 1 });
                assert!(row.closed);
                continue;
            };
            let tasks::Funder::Period { project, period } = row.funder else { panic!("no local pool mutation") };
            assert_eq!(project, 2, "the reread project emitted no funding write");
            assert_eq!(period, 2);
            funding_writes = funding_writes.checked_add(1).expect("bounded period outputs");
        }
    }
    assert!(funding_writes > 0);
    let frame = frame_delivery(&out, watcher).expect("cross-project frame signal is held");
    assert_frame_project_pool(&frame, before.pools.get(0));
    let room = room_max(&limits).unwrap();
    let mut journal = skein_lib::Journal::<Write, Box<Held>>::new(&skein_lib::JournalLimits {
        commits: 1,
        writes: room.writes,
        held: room.held,
        now: 1,
        release: room.held,
    });
    let mut decision = journal.decision(&skein_lib::JournalRoom { writes: room.writes, held: room.held }).unwrap();
    let mut writes = 0_u32;
    for request in out {
        match request {
            Request::Write(write) => {
                decision.write(write).unwrap();
                writes = writes.checked_add(1).expect("bounded period outputs");
            }
            Request::Held(held) => decision.hold(held).unwrap(),
            Request::Now(_) | Request::ProjectMade { .. } | Request::Decided => {}
            Request::Ask { .. } => panic!("no connector or recurring work in this fixture"),
        }
    }
    assert!(writes > 0, "the new project period is a durable decision");
    journal.accept(decision);
    let commit = journal.commit().expect("the period's own commit").number;
    let (after, held_read) = held_project_read(&mut core, &limits);
    assert_eq!(after.configured_period, 1);
    assert_eq!(after.configured_pools, before.pools);
    assert_observed_pool(&after, &before);
    let mut read = journal.decision(&skein_lib::JournalRoom { writes: 0, held: 1 }).unwrap();
    read.hold(held_read).unwrap();
    journal.accept(read);
    assert!(journal.commit().is_none(), "the separate read makes no commit");
    let mut released = Queue::with_capacity(room.held);
    assert_eq!(journal.release(&mut released), skein_lib::Released::None);
    assert!(released.is_empty(), "both the signal and reread wait for the period commit");
    journal.committed(commit);
    assert_eq!(journal.release(&mut released), skein_lib::Released::Some);
    let mut observed = 0_u32;
    let mut read_answers = 0_u32;
    while let Some(held) = released.pop() {
        if let Held::Delivery { watcher: actual, document } = held.as_ref() {
            assert_eq!(*actual, watcher);
            assert_eq!(document.as_ref(), &Document::Frame(frame.clone()));
            observed = observed.checked_add(1).expect("bounded period outputs");
        } else if let Held::Read { answer: reads::Answer::Document(document), .. } = held.as_ref() {
            assert_eq!(document.as_ref(), &Document::Project(Box::new(after.clone())));
            let Held::Read { reply, .. } = *held else { panic!("read answer") };
            assert_eq!(reply.into_token(), Token::new(90));
            read_answers = read_answers.checked_add(1).expect("one read answer");
        } else {
            panic!("only the frame signal and separate project read are held");
        }
    }
    assert_eq!(observed, 1);
    assert_eq!(read_answers, 1);
    assert!(frame_delivery(&watch_outputs(&mut core, &limits, Event::Delivered { watcher }), watcher).is_none());
}

#[test]
fn inflight_frame_signals_coalesce_before_rereading_the_latest_project_target() {
    let (mut core, limits) = period_core(1);
    let watcher = Token::new(30);
    let first = open_frame(&mut core, &limits, watcher);
    let original_frame = first.clone();
    let (before, _) = held_project_read(&mut core, &limits);
    assert_eq!(before.configured_period, 1);
    assert_frame_project_pool(&first, before.pools.get(0));
    for period in [2, 3, 4] {
        let out = period_outputs(&mut core, &limits, 2, period, 1000);
        assert!(frame_delivery(&out, watcher).is_none(), "the initial frame delivery is still in flight");
    }
    assert_eq!(first, original_frame, "the already made frame delivery stays whole");
    assert_eq!(before.configured_period, 1, "the already made project read stays whole");
    let out = watch_outputs(&mut core, &limits, Event::Delivered { watcher });
    let current_frame = frame_delivery(&out, watcher).expect("one coalesced frame delivery");
    assert_frame_project_pool(&current_frame, before.pools.get(0));
    let (current, _) = held_project_read(&mut core, &limits);
    assert_eq!(current.configured_period, 1);
    assert_eq!(current.configured_pools, before.pools);
    assert_observed_pool(&current, &before);
    assert!(frame_delivery(&watch_outputs(&mut core, &limits, Event::Delivered { watcher }), watcher).is_none());
    assert!(frame_delivery(&finish_watch_outputs(&mut core, &limits), watcher).is_none());
}

#[test]
fn an_equal_cross_project_period_changes_neither_frame_signal_nor_project_target() {
    let (mut core, limits) = period_core(1);
    let watcher = Token::new(30);
    open_frame(&mut core, &limits, watcher);
    let (before, _) = held_project_read(&mut core, &limits);
    watch_outputs(&mut core, &limits, Event::Delivered { watcher });
    let out = period_outputs(&mut core, &limits, 2, 1, 700);
    assert!(frame_delivery(&out, watcher).is_none());
    let (after, _) = held_project_read(&mut core, &limits);
    assert_eq!(after, before);
}

#[test]
fn zero_older_over_budget_and_unknown_periods_change_neither_frame_signal_nor_project_target() {
    for (project, period, budget) in [(2, 0, 1000), (2, 1, 1000), (2, 3, 1001), (9, 3, 1000)] {
        let (mut core, limits) = period_core(2);
        let watcher = Token::new(30);
        open_frame(&mut core, &limits, watcher);
        let (before, _) = held_project_read(&mut core, &limits);
        assert_eq!(before.configured_period, 1);
        watch_outputs(&mut core, &limits, Event::Delivered { watcher });
        assert!(frame_delivery(&period_outputs(&mut core, &limits, project, period, budget), watcher).is_none());
        let (after, _) = held_project_read(&mut core, &limits);
        assert_eq!(after, before);
    }
}
