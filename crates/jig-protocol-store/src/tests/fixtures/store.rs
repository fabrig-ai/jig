//! Memory component fixtures (protocol/store.md, section 14).
use super::{
    APPLICATION, CORE_LIMITS, DEPLOYMENT, INSTALLATION, KEY_LIMIT, calls, core_rows, identity, note_entry, note_line,
    task_history, task_record,
};
use crate::{Arrived, Below, Io, Root, SecretOwner, Secrets};
use crate::{Event, Limits, Request, Store, Stores, Write, codec, format, keys};
use alloc::boxed::Box;
use jig_domain_core::{CoreKey, CoreRecord, Key, Record};
use skein_io::{file, kernel::Fd};
use skein_lib::{Duration, Env, JournalLimits, List, Queue, Token};

pub const OTHER_DEPLOYMENT: [u8; 16] = [9; 16];
pub const ROOT: Root = Root { token: Token::new(900), fd: Fd::new(3) };

fn kv_limits() -> skein_kv::Limits {
    skein_kv::Limits {
        key: KEY_LIMIT,
        value: 16_777_216,
        ops: 16,
        commit: 134_217_728,
        queued: 2,
        queued_bytes: 268_435_456,
        budget: 16_384,
        segment: 268_435_456,
        snapshot_after: 134_217_728,
        chunk: 16_778_260,
        page: skein_kv::Page { rows: 2, bytes: 16_778_240 },
        deadline: Duration::from_nanos(1_000_000_000),
    }
}

pub fn limits() -> Limits {
    Limits {
        core: CORE_LIMITS,
        store: kv_limits(),
        secrets: kv_limits(),
        journal: JournalLimits { commits: 2, writes: 4, held: 4, now: 4, release: 4 },
        joins: 32,
        queue: 2,
        files: 2,
        reads: 4,
        orphan_erases: 1,
        held: 2,
    }
}

pub fn stores() -> Stores {
    stores_with(&limits())
}

fn stores_with(limits: &Limits) -> Stores {
    Stores { store: skein_kv::Store::memory(&limits.store), secrets: skein_kv::Store::memory(&limits.secrets) }
}

pub fn plant(stores: &mut Stores, ops: &[skein_kv::Op]) {
    let mut above = Queue::with_capacity(skein_kv::MAX_OUT.above);
    let mut below = Queue::with_capacity(skein_kv::MAX_OUT.below);
    skein_kv::down(
        &mut stores.store,
        &crate::memory_env(kv_limits()),
        skein_kv::Request::Commit { owner: Token::new(0), ops: Box::from(ops) },
        &mut above,
        &mut below,
    );
    assert!(below.is_empty(), "memory commits issue no files");
    let Some(skein_kv::Event::Committed { .. }) = above.pop() else {
        panic!("fixture commit must fit");
    };
    assert!(above.is_empty());
}

pub fn format_put(deployment: [u8; 16]) -> skein_kv::Op {
    skein_kv::Op::Put {
        key: keys::encode_own(&keys::Own::Format, KEY_LIMIT).expect("format key fits"),
        value: format::current(APPLICATION.version, deployment).encode().expect("format fits"),
    }
}

pub fn layouts_put() -> skein_kv::Op {
    skein_kv::Op::Put {
        key: keys::encode_own(&keys::Own::Layouts, KEY_LIMIT).expect("layouts key fits"),
        value: format::layouts().encode(),
    }
}

pub fn header_put(deployment: [u8; 16], commits: u64) -> skein_kv::Op {
    let record = deployment_record(deployment, commits);
    skein_kv::Op::Put {
        key: keys::encode(&Key::Core(CoreKey::Deployment), KEY_LIMIT).expect("header key fits"),
        value: codec::encode(&record, &codec::limits(&CORE_LIMITS).expect("fixture limits"), &[]).expect("header fits"),
    }
}

pub fn installation_put(id: [u8; 16]) -> skein_kv::Op {
    skein_kv::Op::Put {
        key: keys::installation(KEY_LIMIT).expect("installation key fits"),
        value: codec::accounts::Installation { id }.encode(),
    }
}

fn deployment_record(deployment: [u8; 16], commits: u64) -> Record {
    let mut header = core_rows::deployment();
    header.id = deployment;
    header.commits = commits;
    Record::Core(CoreRecord::Deployment(header))
}

pub fn over(stores: Stores) -> Store {
    Store::over(&limits(), APPLICATION, &[], stores)
}

pub fn run(store: &mut Store, request: Request) -> Box<[Event]> {
    let mut above = Queue::with_capacity(crate::MAX_OUT_DOWN);
    let mut below = below();
    crate::down(store, &env(), request, &mut above, &mut below);
    let mut events = List::with_capacity(crate::MAX_OUT_DOWN);
    for _ in 0..crate::MAX_OUT_DOWN {
        if let Some(event) = above.pop() {
            events.push(event).expect("one terminal");
        }
    }
    events.into_boxed()
}

pub fn opened() -> Store {
    opened_with_budget(16_384)
}

pub fn opened_with_budget(budget: u64) -> Store {
    let mut limits = limits();
    limits.store.budget = budget;
    let mut store = Store::over(&limits, APPLICATION, &[], stores_with(&limits));
    let events = run(&mut store, Request::Open { owner: Token::new(1), root: ROOT, installation: INSTALLATION });
    let [Event::Opened { owner: _, last: 0, installation: _ }] = &*events else {
        panic!("empty store opens: {events:?}");
    };
    store
}

pub fn commit(number: u64, writes: Box<[Write]>) -> Request {
    Request::Commit { number, writes }
}

pub fn header(commits: u64) -> Write {
    save(header_record(commits))
}

pub fn save(record: Record) -> Write {
    Write::Save(record)
}

pub fn get_kv(store: &mut Store, key: &[u8]) -> Option<Box<[u8]>> {
    let mut above = Queue::with_capacity(skein_kv::MAX_OUT.above);
    let mut below = Queue::with_capacity(skein_kv::MAX_OUT.below);
    skein_kv::down(
        &mut store.stores.store,
        &crate::memory_env(store.limits.store),
        skein_kv::Request::Get { owner: Token::new(0), key: Box::from(key) },
        &mut above,
        &mut below,
    );
    assert!(below.is_empty(), "memory gets issue no files");
    let Some(skein_kv::Event::Got { value, .. }) = above.pop() else {
        panic!("memory get answers immediately");
    };
    assert!(above.is_empty());
    value
}

pub const ALICE: u64 = 11;
pub const LIVE: u64 = 21;
pub const ENDED: u64 = 22;
pub const PROJECT: u32 = 3;

pub fn committed(store: &mut Store, writes: Box<[Write]>) {
    let number = store.stores.store.last().checked_add(1).expect("fixture next commit");
    let events = run(store, commit(number, writes));
    let [Event::Committed { number: found, .. }] = &*events else {
        panic!("fixture commit fits: {events:?}");
    };
    assert_eq!(*found, number);
}

pub fn header_record(commits: u64) -> Record {
    deployment_record(DEPLOYMENT, commits)
}

pub fn person(number: u64) -> Record {
    Record::People(jig_domain_people::Stored::Person { number, identity: identity() })
}

pub fn live_task(number: u64) -> Record {
    let mut row = task_record();
    row.number = number;
    Record::Tasks(jig_domain_tasks::Stored::Live(Box::new(row)))
}

pub fn ended_task(number: u64) -> Record {
    let mut row = task_record();
    row.number = number;
    row.phase = jig_domain_tasks::Phase::Ended(jig_domain_tasks::Ending::Done(jig_domain_tasks::TaskResult::Report {
        words: Box::from(b"result".as_slice()),
    }));
    Record::Tasks(jig_domain_tasks::Stored::Ended(Box::new(row)))
}

pub fn history(task: u64, revision: u64) -> Record {
    let mut row = task_history();
    row.task = task;
    row.revision = revision;
    Record::Tasks(jig_domain_tasks::Stored::History(row))
}

pub fn run_proof(task: u64) -> Record {
    let mut row = core_rows::run();
    row.task = task;
    Record::Core(CoreRecord::RunProof(row))
}

pub fn call(task: u64, completion: u32) -> Record {
    let CoreRecord::Call(mut row) = calls::call(jig_domain_core::CallPart::Controlled) else {
        panic!("call fixture");
    };
    row.key.task = task;
    row.key.completion = completion;
    Record::Core(CoreRecord::Call(row))
}

pub fn notes_entry(name: u64) -> Record {
    let mut row = note_entry();
    row.name = name;
    Record::Notes(jig_domain_notes::Record::Entry(row))
}

pub fn result_index(party: u64, position: u64, task: u64) -> Record {
    Record::Tasks(jig_domain_tasks::Stored::Result(jig_domain_tasks::ResultRow { party, position, task }))
}

pub fn notes_line(scope: &jig_domain_notes::Scope, name: u64) -> Record {
    let mut row = note_line();
    row.scope = scope.clone();
    row.name = name;
    Record::Notes(jig_domain_notes::Record::Line(row))
}

pub fn env() -> Env<Limits> {
    Env { now: skein_lib::Time::ZERO, wall: skein_lib::Wall::EPOCH, limits: limits() }
}
pub fn below() -> Below {
    Below {
        store: Queue::with_capacity(skein_kv::MAX_OUT.below),
        secrets: Queue::with_capacity(skein_kv::MAX_OUT.below),
        transcripts: Queue::with_capacity(skein_kv::MAX_OUT.below),
    }
}
fn unseeded_driver() -> skein_io::file_layer::FileIo {
    skein_io::file_layer::FileIo::new(8, 1, 32, limits().store.deadline)
}
pub fn directory() -> (Store, Below) {
    let mut drivers = [unseeded_driver(), unseeded_driver(), unseeded_driver()];
    let [store, secrets, transcripts] = &mut drivers;
    // Scripted terminals need no randomness; reuse the fixture's installation bytes as their seed.
    let seeded = crate::seed(
        [store, secrets, transcripts],
        u64::from_le_bytes(INSTALLATION.get(..8).expect("installation seed").try_into().expect("eight bytes")),
    );
    let mut store = Store::on_directory(&limits(), APPLICATION, &[], seeded);
    let mut below = below();
    let mut above = Queue::with_capacity(crate::MAX_OUT_DOWN);
    crate::down(
        &mut store,
        &env(),
        Request::Open { owner: Token::new(1), root: ROOT, installation: INSTALLATION },
        &mut above,
        &mut below,
    );
    assert!(above.is_empty());
    (store, below)
}
pub fn answer(store: &mut Store, io: Io) -> (Box<[Event]>, Below) {
    let mut above = Queue::with_capacity(crate::MAX_OUT_DOWN);
    let mut below = below();
    crate::up(store, &env(), io, &mut above, &mut below);
    (events(&mut above), below)
}
pub fn events(above: &mut Queue<Event>) -> Box<[Event]> {
    let mut events = List::with_capacity(crate::MAX_OUT_DOWN);
    for _ in 0..crate::MAX_OUT_DOWN {
        if let Some(event) = above.pop() {
            events.push(event).expect("bounded events");
        }
    }
    events.into_boxed()
}
pub fn sign_in_secret(number: u64, byte: u8) -> Secrets {
    Secrets::Keep {
        owner: SecretOwner::SignIn,
        number,
        value: crate::SecretValue::Digest { expiry: 0, digest: [byte; 32] },
    }
}
pub fn codec_limits() -> codec::Limits {
    codec::limits(&CORE_LIMITS).expect("fixture codec limits")
}
pub fn turn(task: u64, attempt: u64, number: u32, bytes: &[u8]) -> jig_domain_core::TurnRecord {
    let mut row = core_rows::turn();
    row.task = task;
    row.attempt = attempt;
    row.turn = number;
    row.transcript = Box::from(bytes);
    row.opens_window = false;
    row
}
pub fn window_turn(task: u64, attempt: u64, number: u32, bytes: &[u8]) -> jig_domain_core::TurnRecord {
    let mut row = turn(task, attempt, number, bytes);
    row.opens_window = true;
    row
}
pub fn opened_with_transcripts() -> (Store, Below) {
    let mut store = opened();
    committed(&mut store, Box::new([header(1)]));
    (store, below())
}
#[derive(Clone)]
pub enum Answer {
    Opened { len: u64 },
    Missing,
    Made,
    Written,
    Synced,
    Closed,
    Read { bytes: Box<[u8]> },
    Cut,
    Removed,
    NotFound,
}
pub fn script(store: &mut Store, mut below: Below, answers: &[Answer]) -> Box<[Event]> {
    let count =
        u32::try_from(answers.len()).expect("bounded script").checked_mul(crate::MAX_OUT_DOWN).expect("bounded events");
    let mut found = List::with_capacity(count);
    let base =
        store.transcripts.touch.checked_mul(128).expect("bounded script").checked_add(1_000).expect("fixture token");
    for (index, answer) in answers.iter().enumerate() {
        let request = below.transcripts.pop().expect("transcript request");
        assert!(below.transcripts.is_empty());
        let event = match answer {
            Answer::Opened { len } => {
                let (file::Request::OpenWriteNoFollow { owner, .. }
                | file::Request::CreateNoFollow { owner, .. }
                | file::Request::OpenDirectory { owner, .. }
                | file::Request::OpenRead { owner, .. }) = request
                else {
                    panic!("open operation");
                };
                let token = base.checked_add(u64::try_from(index).expect("script index")).expect("fixture token");
                file::Event::Opened { owner, file: Token::new(token), len: *len }
            }
            Answer::Missing => {
                let (file::Request::OpenWriteNoFollow { owner, .. } | file::Request::OpenRead { owner, .. }) = request
                else {
                    panic!("missing open");
                };
                file::Event::Failed { owner, error: skein_io::kernel::Error::NotFound, committed: false, residue: None }
            }
            Answer::Made => {
                let file::Request::MakeDirectory { owner, .. } = request else {
                    panic!("make directory");
                };
                file::Event::Made { owner }
            }
            Answer::Written => {
                let file::Request::WriteAt { owner, .. } = request else {
                    panic!("write frame");
                };
                file::Event::Written { owner }
            }
            Answer::Synced => {
                let (file::Request::Sync { owner, .. } | file::Request::SyncDirectory { owner, .. }) = request else {
                    panic!("sync operation");
                };
                file::Event::Synced { owner }
            }
            Answer::Read { bytes } => {
                let file::Request::ReadAt { owner, max, .. } = request else {
                    panic!("read frame");
                };
                assert!(bytes.len() <= usize::try_from(max).expect("read size"));
                file::Event::Read { owner, bytes: bytes.clone() }
            }
            Answer::Removed => {
                let file::Request::Remove { owner, .. } = request else {
                    panic!("remove operation");
                };
                file::Event::Removed { owner }
            }
            Answer::NotFound => {
                let file::Request::Remove { owner, .. } = request else {
                    panic!("missing removal");
                };
                file::Event::Failed { owner, error: skein_io::kernel::Error::NotFound, committed: false, residue: None }
            }
            Answer::Cut => {
                let file::Request::Cut { owner, .. } = request else {
                    panic!("cut operation");
                };
                file::Event::Cut { owner }
            }
            Answer::Closed => {
                let file::Request::Close { owner, .. } = request else {
                    panic!("close operation");
                };
                file::Event::Closed { owner }
            }
        };
        let (events, next) = self::answer(store, Io::Transcripts(Arrived { event, fd: Some(Fd::new(10)) }));
        for event in events {
            found.push(event).expect("bounded script events");
        }
        below = next;
    }
    found.into_boxed()
}

pub fn written(store: &mut Store, mut below: Below, turns: &[jig_domain_core::TurnRecord]) -> Box<[u8]> {
    let mut disk = List::with_capacity(4096);
    for _ in 0..turns.len().checked_mul(3).expect("bounded answers").checked_add(1).expect("opening") {
        let Some(request) = below.transcripts.pop() else { break };
        let event = match request {
            file::Request::OpenWriteNoFollow { owner, .. } => {
                file::Event::Opened { owner, file: Token::new(700), len: 0 }
            }
            file::Request::WriteAt { owner, bytes, .. } => {
                for byte in bytes {
                    disk.push(byte).expect("fixture disk");
                }
                file::Event::Written { owner }
            }
            file::Request::Sync { owner, .. } => file::Event::Synced { owner },
            file::Request::MakeDirectory { .. }
            | file::Request::OpenPrivate { .. }
            | file::Request::Create { .. }
            | file::Request::CreateNoFollow { .. }
            | file::Request::OpenRead { .. }
            | file::Request::OpenReadNoFollow { .. }
            | file::Request::OpenWrite { .. }
            | file::Request::OpenDirectory { .. }
            | file::Request::Load { .. }
            | file::Request::Scan { .. }
            | file::Request::Store { .. }
            | file::Request::Stat { .. }
            | file::Request::ReadAt { .. }
            | file::Request::Cut { .. }
            | file::Request::Close { .. }
            | file::Request::SyncDirectory { .. }
            | file::Request::Rename { .. }
            | file::Request::Remove { .. }
            | file::Request::ListPage { .. }
            | file::Request::Lock { .. }
            | file::Request::List { .. } => panic!("unexpected transcript write"),
        };
        let (events, next) = answer(store, Io::Transcripts(Arrived { event, fd: None }));
        if !events.is_empty() {
            let [Event::Committed { .. }] = &*events else {
                panic!("fixture turn commit");
            };
        }
        below = next;
    }
    assert!(below.transcripts.is_empty());
    disk.into_boxed()
}
