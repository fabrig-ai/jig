//! The reference root's store transport (protocol/store.md, 3 and 7).
use crate::{StoreOut, Up, keys, live, records};
use alloc::boxed::Box;
use jig_domain_core as core;
use jig_domain_notes as notes;
use jig_domain_people as people;
use jig_protocol_store::{self as store, Event, Request, Restore, Row};
use jig_test_domain as root;
use skein_lib::{List, Map, Queue, Token};

/// A bounded owner table, configured once with its root's admitted limits.
#[derive(Debug)]
pub struct Routes {
    pending: Map<Token, Flight>,
    next: u64,
    commit: u64,
    limits: Option<root::Limits>,
}
#[derive(Debug)]
enum Flight {
    Restart { step: core::RestartStep, census: Option<core::Census> },
    Read(Token),
    Notes(Token),
    Historical(core::Now),
    Window { task: u64, attempt: u64, turn: u32 },
}
impl Routes {
    #[must_use]
    pub fn new(table: u32) -> Routes {
        Routes { pending: Map::with_capacity(table), next: 0, commit: 0, limits: None }
    }
    /// Give the routing the configured decode and page bounds before use.
    pub fn configure(&mut self, limits: &root::Limits) {
        assert!(self.pending.is_empty(), "configure before routing requests");
        self.limits = Some(*limits);
    }
    /// Whether every routed store request has terminated.
    #[must_use]
    pub fn idle(&self) -> bool {
        self.pending.is_empty()
    }
    fn keep(&mut self, flight: Flight) -> Option<Token> {
        self.next = self.next.checked_add(1)?;
        let owner = Token::new(self.next);
        self.pending.insert(owner, flight).ok()?;
        Some(owner)
    }
}
/// Translate one root request, retaining only its continuation's identity.
#[must_use]
pub fn down(routes: &mut Routes, out: StoreOut) -> Option<Request> {
    let limits = routes.limits?;
    Some(match out {
        StoreOut::Commit { number, writes } => {
            let mut translated = List::with_capacity(u32::try_from(writes.len()).ok()?);
            for write in writes {
                let write = match write {
                    root::Write::Wires => store::Write::Wires,
                    root::Write::CopySettingsRevision(number) => store::Write::CopySettingsRevision(number),
                    root::Write::Save(root::Record::Core(row)) => store::Write::Save(row),
                    root::Write::Erase(root::Key::Core(key)) => store::Write::Erase(key),
                    root::Write::Save(root::Record::Connector { number, record }) => {
                        store::Write::Put { key: keys::encode(number, &record.key()), value: records::encode(&record) }
                    }
                    root::Write::Erase(root::Key::Connector { number, key }) => {
                        store::Write::Remove { key: keys::encode(number, &key) }
                    }
                };
                translated.push(write).expect("same number of writes");
            }
            routes.commit = number;
            Request::Commit { number, writes: translated.into_boxed() }
        }
        StoreOut::Restart { step, census } => {
            if step == core::RestartStep::Switch {
                let owner = routes.keep(Flight::Restart { step, census })?;
                return Some(Request::Switch { owner });
            }
            match &step {
                core::RestartStep::LoadCore | core::RestartStep::RestoreConnector { .. } => {}
                core::RestartStep::LoadInbox => {
                    census.as_ref()?;
                }
                core::RestartStep::Switch
                | core::RestartStep::AdoptRuns
                | core::RestartStep::ReadAfresh { .. }
                | core::RestartStep::SettleOutbox { .. }
                | core::RestartStep::Open => return None,
            }
            let owner = routes.keep(Flight::Restart { step, census })?;
            restart(routes, owner, None)?
        }
        StoreOut::ReadLoad { load, range } => {
            let owner = routes.keep(Flight::Read(load))?;
            Request::Read { owner, range: *range }
        }
        StoreOut::NotesLoad { owner, range } => {
            let owner = routes.keep(Flight::Notes(owner))?;
            Request::Load {
                owner,
                range: store::Load::Notes(range),
                after: None,
                rows: limits.core.notes.load_rows,
                bytes: limits.core.load_bytes,
            }
        }
        StoreOut::Now(now) => {
            if let Some(key) = store::historical::key(&now) {
                let owner = routes.keep(Flight::Historical(now))?;
                return Some(Request::Get { owner, key: store::Get::Jig(key) });
            }
            let (task, attempt, turn) = window(now)?;
            let owner = routes.keep(Flight::Window { task, attempt, turn })?;
            transcript(routes, owner, None)?
        }
    })
}
fn restart(routes: &Routes, owner: Token, after: Option<store::Cursor>) -> Option<Request> {
    let (step, census) = match routes.pending.get(&owner)? {
        Flight::Restart { step, census } => (step, census),
        Flight::Read(_) | Flight::Notes(_) | Flight::Historical(_) | Flight::Window { .. } => return None,
    };
    let range = match step {
        core::RestartStep::LoadCore => Restore::Jig(Box::new(core::RestoreRange::LIVE)),
        core::RestartStep::LoadInbox => Restore::Jig(Box::new([core::RestoreRange::Census(census.clone()?)])),
        core::RestartStep::RestoreConnector { connector } => Restore::Application(Box::new([live(*connector)])),
        core::RestartStep::Switch
        | core::RestartStep::AdoptRuns
        | core::RestartStep::ReadAfresh { .. }
        | core::RestartStep::SettleOutbox { .. }
        | core::RestartStep::Open => return None,
    };
    Some(Request::Restore { owner, range, after, rows: 2, bytes: routes.limits?.core.load_bytes })
}
fn transcript(routes: &Routes, owner: Token, after: Option<u64>) -> Option<Request> {
    let (task, attempt, turn) = match routes.pending.get(&owner)? {
        Flight::Window { task, attempt, turn } => (task, attempt, turn),
        Flight::Restart { .. } | Flight::Read(_) | Flight::Notes(_) | Flight::Historical(_) => return None,
    };
    let limits = routes.limits?;
    Some(Request::Transcript {
        owner,
        read: store::TranscriptRead::Window { task: *task, attempt: *attempt, turn: *turn, after },
        rows: 2,
        bytes: u64::from(limits.core.resume_bytes),
    })
}
fn window(now: core::Now) -> Option<(u64, u64, u32)> {
    match now {
        core::Now::StartPreparation { task, from: Some((attempt, turn)), .. } => Some((task, attempt, turn)),
        core::Now::RelayBusy { .. }
        | core::Now::CallBusy { .. }
        | core::Now::Relayed { .. }
        | core::Now::EffectAnswer { .. }
        | core::Now::SettledCallRefused { .. }
        | core::Now::Call { .. }
        | core::Now::DropCall { .. }
        | core::Now::Undelivered { .. }
        | core::Now::SignInRefused { .. }
        | core::Now::WatchRefused { .. }
        | core::Now::Account(_)
        | core::Now::RunText { .. }
        | core::Now::NotesIndexed { .. }
        | core::Now::NotesRecalled { .. }
        | core::Now::NotesRefused { .. }
        | core::Now::NoteBusy { .. }
        | core::Now::DropPayload { .. }
        | core::Now::DropAssignment { .. }
        | core::Now::TurnPayload { .. }
        | core::Now::AnswerPayload { .. }
        | core::Now::AcceptedTurn { .. }
        | core::Now::RefusedPayload { .. }
        | core::Now::Activate { .. }
        | core::Now::PrepareAgent { .. }
        | core::Now::StartPreparation { from: None, .. }
        | core::Now::CompleteBrief { .. }
        | core::Now::BriefCorePlanned { .. }
        | core::Now::WorkspaceRequest { .. }
        | core::Now::RunPreparationFailed { .. }
        | core::Now::RunPrepared { .. }
        | core::Now::RestoreRefused
        | core::Now::SeenConversation { .. }
        | core::Now::LoadEndedRead { .. }
        | core::Now::HistoricalPersonTask { .. }
        | core::Now::EndedTask { .. }
        | core::Now::PartyInputs { .. }
        | core::Now::HistoricalProposal { .. }
        | core::Now::DecidedProposal { .. }
        | core::Now::HistoricalEscalation { .. } => None,
    }
}
/// Translate a terminal, preserving pages and requesting their continuations.
pub fn up(routes: &mut Routes, event: Event, into: &mut Queue<Up>) {
    match event {
        // The reference shells never hand patch sets. Return the unknown
        // written site to its open's asker through their ordinary request path.
        Event::Site { owner, patcher, site, .. } => {
            into.push(Up::Ask(Request::Site { owner, patcher, result: Err(skein_codec::patch::Halt::NoSite(site)) }));
        }
        Event::Committed { number, fill } => into.push(Up::Root(root::Event::Committed { number, fill })),
        Event::Failed { number } | Event::FrameTooLarge { number, .. } => {
            into.push(Up::Root(root::Event::Failed { number }));
        }
        Event::Restored { owner, rows, next } => restored(routes, owner, rows, next, into),
        Event::Read { owner, result } => {
            if let Some(Flight::Read(load)) = routes.pending.remove(&owner) {
                into.push(Up::Root(root::Event::Core(core::Event::ReadLoaded { load, result })));
            }
        }
        Event::Loaded { owner, rows, next } => loaded(routes, owner, rows, next, into),
        Event::Got { owner, row } => {
            if let Some(Flight::Historical(now)) = routes.pending.remove(&owner) {
                let row = match row {
                    Some(Row::Jig(row)) => Some(row),
                    None => None,
                    Some(Row::Application { .. }) => {
                        stop(routes, into);
                        return;
                    }
                };
                if let Some(key) = store::historical::next(&now, &row) {
                    assert!(routes.pending.insert(owner, Flight::Historical(now)).is_ok(), "historical slot retained");
                    into.push(Up::Ask(Request::Get { owner, key: store::Get::Jig(key) }));
                    return;
                }
                if let Some(event) = store::historical::answer(now, row) {
                    into.push(Up::Root(root::Event::Core(event)));
                }
            }
        }
        Event::Turns { owner, turns, next } => {
            let Some(Flight::Window { task, .. }) = routes.pending.get(&owner) else {
                return;
            };
            let mut rows = List::with_capacity(u32::try_from(turns.len()).expect("bounded store page"));
            for turn in turns {
                let limits = jig_protocol_transcript::Limits::new(
                    u32::try_from(turn.transcript.len()).expect("stored turn bound"),
                    jig_protocol_transcript::TurnLimits {
                        messages: u32::MAX,
                        blocks: u32::MAX,
                        text: u32::try_from(turn.transcript.len()).expect("stored turn bound"),
                    },
                );
                let text = jig_protocol_transcript::brief_text(&limits, &turn.transcript).ok();
                rows.push((turn, text)).expect("bounded store page");
            }
            into.push(Up::Root(root::Event::TranscriptLoaded {
                task: *task,
                rows: rows.into_boxed(),
                done: next.is_none(),
            }));
            if let Some(after) = next {
                if let Some(request) = transcript(routes, owner, Some(after)) {
                    into.push(Up::Ask(request));
                }
            } else {
                routes.pending.remove(&owner);
            }
        }
        Event::Unread { owner } => {
            if let Some(Flight::Window { task, .. }) = routes.pending.remove(&owner) {
                into.push(Up::Root(root::Event::Core(core::Event::TranscriptUnread { task })));
            }
        }
        Event::SecretsNotReady { .. } => unreachable!("the root store route handles public records only"),
        Event::Corrupt { owner } => {
            routes.pending.remove(&owner);
            stop(routes, into);
        }
        Event::OpenRefused { .. } => stop(routes, into),
        Event::Switched { owner } => {
            if let Some(Flight::Restart { step: core::RestartStep::Switch, .. }) = routes.pending.remove(&owner) {
                into.push(Up::Root(root::Event::Switched));
            }
        }
        // The shell driving open owns progress; this root route prints no lines.
        Event::Patched { .. }
        | Event::Opened { .. }
        | Event::Kept { .. }
        | Event::Carried { .. }
        | Event::Stale { .. }
        | Event::Erased { .. }
        | Event::Secret { .. }
        | Event::SecretsLoaded { .. } => {}
    }
}
fn loaded(routes: &mut Routes, owner: Token, rows: Box<[Row]>, next: Option<store::Cursor>, into: &mut Queue<Up>) {
    if let Some(Flight::Notes(owner)) = routes.pending.remove(&owner) {
        let mut records = List::with_capacity(u32::try_from(rows.len()).expect("bounded page"));
        for row in rows {
            match row {
                Row::Jig(core::Record::Notes(record)) => {
                    records.push(record).expect("same count");
                }
                Row::Jig(_) | Row::Application { .. } => {
                    stop(routes, into);
                    return;
                }
            }
        }
        let event = notes::Event::Loaded { owner, rows: notes::Rows { records }, more: next.is_some() };
        into.push(Up::Root(root::Event::Core(core::Event::Notes(event))));
    }
}
fn restored(routes: &mut Routes, owner: Token, rows: Box<[Row]>, next: Option<store::Cursor>, into: &mut Queue<Up>) {
    let Some(Flight::Restart { step, .. }) = routes.pending.get(&owner) else {
        return;
    };
    let step = *step;
    let limits = routes.limits.expect("configured routing");
    for row in rows {
        let record = match row {
            Row::Jig(row) => root::Record::Core(row),
            Row::Application { key, value } => {
                let Some((number, _)) = keys::decode(&key, &limits.connector) else {
                    stop(routes, into);
                    return;
                };
                let Ok(record) = records::decode(&value, &limits.connector) else {
                    stop(routes, into);
                    return;
                };
                if keys::encode(number, &record.key()) != key {
                    stop(routes, into);
                    return;
                }
                root::Record::Connector { number, record }
            }
        };
        into.push(Up::Restore(record));
    }
    if let Some(after) = next {
        if let Some(request) = restart(routes, owner, Some(after)) {
            into.push(Up::Ask(request));
        }
    } else {
        routes.pending.remove(&owner);
        if step == core::RestartStep::LoadCore {
            into.push(Up::Root(root::Event::Core(core::Event::People(people::Event::Restored))));
        }
        into.push(Up::Root(root::Event::RestartDone(step)));
    }
}
fn stop(routes: &Routes, into: &mut Queue<Up>) {
    into.push(Up::Root(root::Event::Failed { number: routes.commit }));
}
