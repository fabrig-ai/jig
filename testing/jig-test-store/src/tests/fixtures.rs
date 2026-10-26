use jig_test_connector as d;
use skein_lib::{Rng, Token, Wall};
pub const FIRST: u16 = 1;
pub const SECOND: u16 = 2;
pub use jig_test_connector_world::LIMITS;
pub fn path() -> d::Path {
    jig_test_connector_world::path(1, 1)
}
pub fn key() -> d::Key {
    d::Key { attempt: 1, completion: 2, position: 3, deployment: [0; 16], task: 4, purpose: 5 }
}
pub fn effect() -> d::Effect {
    d::Effect { kind: 1, resources: Box::new([path()]), purpose: 2, condition: Some(3), target: 4, state: 5 }
}
pub fn record_key(random: &mut Rng) -> (u16, d::RecordKey) {
    let connector = if random.below(2) == 0 { FIRST } else { SECOND };
    let number = random.next_u64();
    let record = match random.below(12) {
        0 => d::RecordKey::ApplicationObject(number),
        1 => d::RecordKey::ApplicationAnswer(Token::new(number)),
        2 => d::RecordKey::Procedure(number),
        3 => d::RecordKey::ProcedureDelegateOutcome(number),
        4 => d::RecordKey::Result(number),
        5 => d::RecordKey::Task(number),
        6 => d::RecordKey::Adoption { project: u32::try_from(number & 255).expect("byte"), path: random_path(random) },
        7 => d::RecordKey::Subscription { topic: u16::try_from(number & 255).expect("byte"), task: random.next_u64() },
        8 => d::RecordKey::Pool(random_path(random)),
        9 => d::RecordKey::Proposal(number),
        10 => d::RecordKey::Outbox(random_key(random)),
        _ => d::RecordKey::Made(random_key(random)),
    };
    (connector, record)
}
fn random_path(random: &mut Rng) -> d::Path {
    let mut segments = skein_lib::List::with_capacity(2);
    for _ in 0..=random.below(2) {
        segments.push(Box::from([u8::try_from(random.below(256)).expect("byte")])).expect("bounded path");
    }
    d::Path::new(segments, LIMITS.segment_bytes).expect("bounded path")
}
fn random_key(random: &mut Rng) -> d::Key {
    let mut deployment = [0; 16];
    for byte in &mut deployment {
        *byte = u8::try_from(random.below(256)).expect("byte");
    }
    d::Key {
        attempt: random.next_u64(),
        completion: u32::try_from(random.below(u64::from(u32::MAX))).expect("u32"),
        position: u32::try_from(random.below(u64::from(u32::MAX))).expect("u32"),
        deployment,
        task: random.next_u64(),
        purpose: random.next_u64(),
    }
}
pub fn records() -> Box<[d::Record]> {
    Box::new([
        application_object(),
        application_answer(),
        proposal(),
        procedure(),
        delegate_outcome(),
        result(),
        task(),
        adoption(),
        subscription(),
        pool(),
        outbox(),
        made(),
    ])
}
fn application_object() -> d::Record {
    d::Record::ApplicationObject { object: 1, now: d::ApplicationObject { revision: 2, value: 3 } }
}
fn application_answer() -> d::Record {
    d::Record::ApplicationAnswer {
        token: Token::new(1),
        until: Wall::from_nanos(2),
        outcome: d::ApplicationOutcome::CoreRefused {
            why: d::application_core::Refusal::Authority {
                lacking: d::application_core::Lack::Budget { needs: 3, left: 2 },
                proposable: Some(true),
            },
        },
    }
}
fn proposal() -> d::Record {
    d::Record::Proposal { number: 1, task: 2, project: 3, effect: effect() }
}
fn procedure() -> d::Record {
    d::Record::Procedure(d::ProcedureState {
        project: 1,
        task: 2,
        number: 3,
        resource: path(),
        index: 4,
        steps: 5,
        awaiting: true,
        delegate: None,
        revision: 0,
        deadline: Some(Wall::from_nanos(6)),
    })
}
fn delegate_outcome() -> d::Record {
    d::Record::ProcedureDelegateOutcome { task: 1, step: 2, child: Some(3), revisions: Box::new([]) }
}
fn result() -> d::Record {
    d::Record::Result { task: 1, code: 2 }
}
fn task() -> d::Record {
    d::Record::Task { task: 1, project: 2, resources: Box::new([path()]) }
}
fn adoption() -> d::Record {
    d::Record::Adoption { project: 1, path: path(), role: d::ResourceRole::Participating }
}
fn subscription() -> d::Record {
    d::Record::Subscription { topic: 1, task: 2, wake_at: 3, keep_at: 4 }
}
fn pool() -> d::Record {
    d::Record::Pool { path: path(), slots: 1 }
}
fn outbox() -> d::Record {
    d::Record::Outbox(d::OutboxEntry {
        number: 1,
        task: 2,
        key: key(),
        effect: effect(),
        attempt: Some(d::Attempt { number: 1, sent: Wall::from_nanos(2), deadline: Wall::from_nanos(3) }),
        phase: d::EffectPhase::Sent,
    })
}
fn made() -> d::Record {
    d::Record::Made { key: key(), resources: Box::new([path()]), state: 1 }
}

pub struct Component {
    pub store: jig_protocol_store::Store,
    pub files: jig_test_store_files::Files,
    pub limits: jig_protocol_store::Limits,
}
pub fn root_limits() -> jig_test_domain::Limits {
    jig_core_world::world::limits()
}
pub fn component() -> Component {
    let root = root_limits();
    let kv = skein_kv::Limits {
        key: 1024,
        value: 16_777_216,
        ops: 8192,
        commit: u32::MAX,
        queued: 8,
        queued_bytes: 34_359_738_360,
        budget: 4_194_304,
        segment: 8_589_934_590,
        snapshot_after: 4_294_967_295,
        chunk: 16_778_260,
        page: skein_kv::Page { rows: 2, bytes: 16_778_240 },
        deadline: skein_lib::Duration::from_secs(1),
    };
    let limits = jig_protocol_store::Limits {
        core: root.core,
        store: kv,
        secrets: kv,
        journal: root.journal,
        joins: 128,
        queue: 8,
        files: 8,
        reads: 4,
        orphan_erases: 1,
        held: 2,
    };
    let stores =
        jig_protocol_store::Stores { store: skein_kv::Store::memory(&kv), secrets: skein_kv::Store::memory(&kv) };
    let store = jig_protocol_store::Store::over(&limits, crate::application(&root.connector), &[], stores);
    let mut component = Component { store, files: jig_test_store_files::Files::new(), limits };
    let root = component.files.root();
    let mut above = skein_lib::Queue::with_capacity(jig_protocol_store::MAX_OUT_DOWN);
    let open = jig_protocol_store::Request::Open { owner: Token::new(900), root, installation: [0; 16] };
    send(&mut component, open, &mut above);
    assert!(matches_opened(&mut above));
    component
}
fn matches_opened(above: &mut skein_lib::Queue<jig_protocol_store::Event>) -> bool {
    match above.pop() {
        Some(jig_protocol_store::Event::Opened { last: 0, .. }) => above.is_empty(),
        Some(_) | None => false,
    }
}
pub fn routes() -> crate::route::Routes {
    let mut routes = crate::route::Routes::new(8);
    routes.configure(&root_limits());
    routes
}
fn send(
    component: &mut Component,
    request: jig_protocol_store::Request,
    above: &mut skein_lib::Queue<jig_protocol_store::Event>,
) {
    let mut below = jig_protocol_store::Below {
        store: skein_lib::Queue::with_capacity(skein_kv::MAX_OUT.below),
        secrets: skein_lib::Queue::with_capacity(skein_kv::MAX_OUT.below),
        transcripts: skein_lib::Queue::with_capacity(skein_kv::MAX_OUT.below),
    };
    let env = skein_lib::Env { now: skein_lib::Time::ZERO, wall: Wall::EPOCH, limits: component.limits };
    jig_protocol_store::down(&mut component.store, &env, request, above, &mut below);
    for _ in 0..128_u32 {
        let Some(request) = below.transcripts.pop() else {
            break;
        };
        let arrived = component.files.answer(request);
        jig_protocol_store::up(
            &mut component.store,
            &env,
            jig_protocol_store::Io::Transcripts(arrived),
            above,
            &mut below,
        );
    }
    assert!(below.store.is_empty() && below.secrets.is_empty() && below.transcripts.is_empty());
}
pub fn routed(component: &mut Component, routes: &mut crate::route::Routes, out: crate::StoreOut) -> Box<[crate::Up]> {
    let mut requests = skein_lib::Queue::with_capacity(64);
    requests.push(crate::route::down(routes, out).expect("routed request"));
    let mut found = skein_lib::List::with_capacity(128);
    for _ in 0..64_u32 {
        let Some(request) = requests.pop() else {
            break;
        };
        let mut above = skein_lib::Queue::with_capacity(jig_protocol_store::MAX_OUT_DOWN);
        send(component, request, &mut above);
        for _ in 0..jig_protocol_store::MAX_OUT_DOWN {
            let Some(event) = above.pop() else {
                break;
            };
            let mut outputs = skein_lib::Queue::with_capacity(8);
            crate::route::up(routes, event, &mut outputs);
            for _ in 0..8_u32 {
                let Some(output) = outputs.pop() else {
                    break;
                };
                match output {
                    crate::Up::Ask(request) => requests.push(request),
                    other @ (crate::Up::Restore(_) | crate::Up::Root(_)) => {
                        found.push(other).expect("bounded fixture");
                    }
                }
            }
        }
    }
    assert!(requests.is_empty());
    found.into_boxed()
}
pub fn header(commits: u64) -> jig_test_domain::Write {
    jig_test_domain::Write::Save(jig_test_domain::Record::Core(jig_domain_core::Record::Core(
        jig_domain_core::CoreRecord::Deployment(jig_domain_core::Deployment {
            id: [18; 16],
            tasks: 21,
            people: 0,
            sign_ins: 0,
            messages: 0,
            runs: 1,
            calls: 0,
            connector_rows: 0,
            projects: 0,
            applications: 0,
            commits,
        }),
    )))
}
pub fn root_record(record: d::Record) -> jig_test_domain::Record {
    jig_test_domain::Record::Connector { number: FIRST, record }
}
pub fn turn(commits: u64) -> jig_test_domain::Write {
    jig_test_domain::Write::Save(jig_test_domain::Record::Core(jig_domain_core::Record::Core(
        jig_domain_core::CoreRecord::Turn(jig_domain_core::TurnRecord {
            task: 21,
            attempt: 1,
            turn: u32::try_from(commits).expect("fixture turn"),
            opens_window: commits == 1,
            spent: 0,
            read: None,
            at: Wall::EPOCH,
            transcript: Box::from(b"opaque".as_slice()),
        }),
    )))
}

pub fn core_commit() -> crate::StoreOut {
    crate::StoreOut::Commit {
        number: 1,
        writes: Box::new([
            header(1),
            jig_test_domain::Write::Save(jig_test_domain::Record::Core(jig_domain_core::Record::People(
                jig_domain_people::Stored::Roles { project: 1, holdings: Box::new([]) },
            ))),
            jig_test_domain::Write::Save(jig_test_domain::Record::Core(jig_domain_core::Record::People(
                jig_domain_people::Stored::ReadResults { party: 1, through: 0 },
            ))),
        ]),
    }
}
pub fn commit_writes(request: jig_protocol_store::Request) -> Box<[jig_protocol_store::Write]> {
    match request {
        jig_protocol_store::Request::Commit { writes, .. } => writes,
        jig_protocol_store::Request::Rehearse { .. }
        | jig_protocol_store::Request::Back { .. }
        | jig_protocol_store::Request::Close { .. }
        | jig_protocol_store::Request::Switch { .. }
        | jig_protocol_store::Request::Site { .. }
        | jig_protocol_store::Request::Open { .. }
        | jig_protocol_store::Request::Restore { .. }
        | jig_protocol_store::Request::Load { .. }
        | jig_protocol_store::Request::Get { .. }
        | jig_protocol_store::Request::Read { .. }
        | jig_protocol_store::Request::Transcript { .. }
        | jig_protocol_store::Request::Secrets { .. } => unreachable!("fixture requests a commit"),
    }
}
pub fn ended_chat() -> Box<jig_domain_tasks::TaskRecord> {
    let mut world = jig_core_world::effects::World::new(18, false);
    let task = world.assigned.as_ref().expect("chat assigned").task;
    world.end(
        jig_domain_tasks::End::Finished {
            result: jig_domain_tasks::TaskResult::Report { words: Box::from(b"done".as_slice()) },
            cancel_delegates: false,
        },
        0,
    );
    match world
        .store
        .rows
        .get(&jig_test_domain::Key::Core(jig_domain_core::Key::Tasks(jig_domain_tasks::Key::Ended(task))))
        .expect("chat ended")
    {
        jig_test_domain::Record::Core(jig_domain_core::Record::Tasks(jig_domain_tasks::Stored::Ended(row))) => {
            row.clone()
        }
        jig_test_domain::Record::Core(_) | jig_test_domain::Record::Connector { .. } => unreachable!("ended task key"),
    }
}
