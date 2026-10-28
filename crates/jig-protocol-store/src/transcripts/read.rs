//! Reads bounded by durable places, walking complete frames (store.md, 8.3).
use super::{File, Stage, frame};
use crate::{Arrived, Below, Event, Limits, Phase, Store, TranscriptRead, codec, keys, open, reads};
use alloc::boxed::Box;
use jig_domain_core::{self as domain, TurnRecord};
use jig_domain_tasks as tasks;
use skein_io::file;
use skein_lib::{List, Queue, Reader, Token, Writer};
mod party;
pub(crate) const OWNER: Token = Token::new(0x5453);
#[derive(Debug)]
pub(crate) struct Party {
    pub activity: Option<u64>,
    pub bytes: u64,
    pub turn: Option<(u64, u32)>,
    pub problem: Option<domain::reads::TurnFailure>,
    pub cut: Option<reads::Failure>,
    pub frame_bytes: Option<u32>,
}
#[derive(Debug)]
enum Scan {
    Window { attempt: u64, turn: u32, after: Option<u64>, checked: bool },
    Turn { attempt: u64, turn: u32 },
    Page,
}
#[derive(Debug)]
enum Step {
    Waiting,
    Opening,
    Evicting { task: u64 },
    Length { offset: u64, bytes: Writer },
    Header { offset: u64, length: u32, bytes: Writer },
    Sized { offset: u64, length: u32, turn: TurnRecord, size: u32, bytes: Writer },
    Frame { offset: u64, length: u32, bytes: Writer },
    Site { offset: u64, length: u32, patch: frame::Patch, of: crate::Owner },
}
struct Decoded {
    offset: u64,
    length: u32,
    size: u32,
    charge_frame: bool,
    turn: TurnRecord,
}
#[derive(Debug)]
pub(crate) struct Read {
    task: u64,
    end: u64,
    position: u64,
    backwards: bool,
    scan: Scan,
    step: Step,
    turns: List<TurnRecord>,
    budget: u64,
    party: Option<Party>,
    activity: Option<Activity>,
}
pub(crate) fn transcript(
    store: &mut Store,
    owner: Token,
    read: TranscriptRead,
    rows: u32,
    bytes: u64,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    assert!(!store.transcripts.reads.contains_key(&owner), "one transcript read per owner");
    assert!(store.transcripts.reads.len() < store.limits.reads, "startup bounds cover transcript reads");
    let Some(read) = prepare(store, owner, read, rows, bytes) else {
        above.push(Event::Unread { owner });
        return;
    };
    store.transcripts.reads.insert(owner, read).expect("read slot checked at admission");
    fire(store, above, below);
}
pub(crate) use party::start as party;
/// Pending activity lines keep their original page order while one transcript walk joins them.
#[derive(Debug)]
struct Activity {
    rows: Box<[domain::reads::Row]>,
    next: Option<domain::reads::Position>,
    bytes: u64,
    bound: u64,
    index: usize,
}
impl Activity {
    fn identity(&self) -> Option<(u64, u64, u32)> {
        for row in self.rows.get(self.index..).expect("page index within rows") {
            if let domain::Record::Tasks(tasks::Stored::Activity(line)) = &row.value
                && let tasks::Line::Turn { attempt, turn, .. } = line.line
            {
                return Some((line.task, attempt, turn));
            }
        }
        None
    }
    fn charge_words(&mut self) -> Result<bool, ()> {
        for index in self.index..self.rows.len() {
            let row = self.rows.get(index).expect("bounded page index");
            if let domain::Record::Tasks(tasks::Stored::Activity(line)) = &row.value
                && let tasks::Line::Turn { .. } = line.line
            {
                return Ok(false);
            }
            self.bytes = self.bytes.checked_sub(row.owned_bytes().ok_or(())?).ok_or(())?;
            self.index = index.checked_add(1).expect("bounded page index");
        }
        Ok(true)
    }
    fn unread(&mut self) {
        if let Some((task, attempt, turn)) = self.identity() {
            self.rows.get_mut(self.index).expect("pending turn row").unjoined =
                Some(domain::reads::UnjoinedTurn::Unread { task, attempt, turn });
            self.index = self.index.checked_add(1).expect("bounded page index");
        }
    }
    fn finish(self, cut: bool) -> domain::reads::Loaded {
        let next = if cut && self.index > 0 {
            Some(domain::reads::Position::Owner(
                self.rows
                    .get(self.index.checked_sub(1).expect("accepted row"))
                    .expect("bounded page index")
                    .value
                    .key(),
            ))
        } else {
            self.next
        };
        let mut rows = List::with_capacity(u32::try_from(self.index).expect("bounded activity page"));
        for (index, row) in self.rows.into_iter().enumerate() {
            if index == self.index {
                break;
            }
            rows.push(row).expect("accepted page prefix");
        }
        domain::reads::Loaded::Page { rows: rows.into_boxed(), next }
    }
}
pub(crate) fn activity(
    store: &mut Store,
    owner: Token,
    rows: Box<[domain::reads::Row]>,
    next: Option<domain::reads::Position>,
    range: &domain::reads::Range,
    above: &mut Queue<Event>,
    below: &mut Below,
) -> Result<Option<domain::reads::Loaded>, reads::Failure> {
    let mut page = Activity { rows, next, bytes: range.bytes, bound: range.bytes, index: 0 };
    let done = page.charge_words().or(Err(reads::Failure::Corrupt))?;
    if done {
        return Ok(Some(page.finish(false)));
    }
    assert!(!store.transcripts.reads.contains_key(&owner), "one transcript read per owner");
    if store.transcripts.reads.len() == store.limits.reads {
        return Ok(Some(domain::reads::Loaded::Busy));
    }
    let (task, attempt, turn) = page.identity().expect("pending turn line");
    let request = if range.after.is_none() {
        TranscriptRead::Page { task, backwards: true, after: None }
    } else {
        TranscriptRead::Turn { task, attempt, turn }
    };
    let Some(mut read) = prepare(store, owner, request, 1, u64::from(store.limits.store.page.bytes)) else {
        page.unread();
        return Ok(Some(page.finish(true)));
    };
    read.activity = Some(page);
    store.transcripts.reads.insert(owner, read).expect("activity read slot checked");
    fire(store, above, below);
    Ok(None)
}
fn prepare(store: &mut Store, owner: Token, request: TranscriptRead, rows: u32, bytes: u64) -> Option<Read> {
    match store.phase {
        Phase::Ready { .. } => {}
        Phase::Closed | Phase::Stopped => return None,
    }
    let task = match request {
        TranscriptRead::Window { task, .. } | TranscriptRead::Turn { task, .. } | TranscriptRead::Page { task, .. } => {
            task
        }
    };
    let key = keys::encode_own(&keys::Own::Transcript { task }, store.limits.store.key)?;
    let row = open::get(store, owner, key).ok()??;
    let place = codec::component::Transcript::decode(&row).ok()?;
    let (position, backwards, scan) = match request {
        TranscriptRead::Window { attempt, turn, after, .. } => {
            if place.window >= place.length
                || after.unwrap_or(place.window) < place.window
                || after.unwrap_or(place.window) > place.length
            {
                return None;
            }
            (place.window, false, Scan::Window { attempt, turn, after, checked: false })
        }
        TranscriptRead::Turn { attempt, turn, .. } => {
            let key = keys::encode_own(&keys::Own::Attempt { task, attempt }, store.limits.store.key)?;
            let row = open::get(store, owner, key).ok()??;
            let offset = codec::component::Attempt::decode(&row).ok()?.first;
            if offset >= place.length {
                return None;
            }
            (offset, false, Scan::Turn { attempt, turn })
        }
        TranscriptRead::Page { backwards, after, .. } => {
            let offset = after.unwrap_or(if backwards { place.length } else { 0 });
            if offset > place.length {
                return None;
            }
            (offset, backwards, Scan::Page)
        }
    };
    Some(Read {
        task,
        end: place.length,
        position,
        backwards,
        scan,
        step: Step::Waiting,
        turns: List::with_capacity(rows.min(store.limits.store.page.rows)),
        budget: bytes.min(u64::from(store.limits.store.page.bytes)),
        party: None,
        activity: None,
    })
}
pub(crate) fn fire(store: &mut Store, above: &mut Queue<Event>, below: &mut Below) {
    if store.transcripts.reading.is_some() {
        return;
    }
    match store.transcripts.stage {
        Stage::Idle => {}
        Stage::Opening
        | Stage::Evicting { .. }
        | Stage::Making
        | Stage::RootSync
        | Stage::DirectoryOpen
        | Stage::Creating
        | Stage::Writing
        | Stage::Syncing
        | Stage::DirectorySync
        | Stage::DirectoryClose
        | Stage::Dropping => return,
    }
    let owner = match store.transcripts.reads.first() {
        Some((owner, _)) => *owner,
        None => return,
    };
    let mut read = store.transcripts.reads.remove(&owner).expect("waiting reader");
    store.transcripts.reading = Some(owner);
    let ready = match store.transcripts.files.get(&read.task) {
        Some(file) if file.readable => next(store, &mut read, below),
        Some(file) => {
            read.step = Step::Evicting { task: read.task };
            below.transcripts.push(file::Request::Close { owner: OWNER, file: file.token });
            Ok(false)
        }
        None => {
            waiting_file(store, &mut read, below);
            Ok(false)
        }
    };
    settle(store, owner, read, ready, above);
}
fn waiting_file(store: &mut Store, read: &mut Read, below: &mut Below) {
    if store.transcripts.files.len() == store.limits.files {
        let mut oldest = None;
        for (task, file) in &store.transcripts.files {
            if let Some((_, touched)) = oldest
                && file.touched >= touched
            {
                continue;
            }
            oldest = Some((*task, file.touched));
        }
        let (task, _) = oldest.expect("a full nonempty file set");
        read.step = Step::Evicting { task };
        let file = store.transcripts.files.get(&task).expect("oldest file").token;
        below.transcripts.push(file::Request::Close { owner: OWNER, file });
    } else {
        opening(store, read, below);
    }
}
fn opening(store: &Store, read: &mut Read, below: &mut Below) {
    read.step = Step::Opening;
    below.transcripts.push(file::Request::OpenRead {
        owner: OWNER,
        root: store.directory.transcripts.expect("opened transcripts root").fd,
        name: super::path(read.task),
    });
}
fn next(store: &mut Store, read: &mut Read, below: &mut Below) -> Result<bool, ()> {
    if read.position == if read.backwards { 0 } else { read.end } {
        return if read.activity.is_some() { Err(()) } else { Ok(true) };
    }
    let checking_window = match read.scan {
        Scan::Window { checked, .. } => !checked,
        Scan::Turn { .. } | Scan::Page => false,
    };
    if read.turns.room() == 0 && !checking_window {
        return Ok(true);
    }
    let offset = if read.backwards { read.position.checked_sub(4).ok_or(())? } else { read.position };
    if offset.checked_add(4).ok_or(())? > read.end {
        return Err(());
    }
    read.step = Step::Length { offset, bytes: Writer::new(4) };
    demand(store, read, offset, 4, below)?;
    Ok(false)
}
fn demand(store: &mut Store, read: &Read, offset: u64, max: u32, below: &mut Below) -> Result<(), ()> {
    let touch = store.transcripts.touch.checked_add(1).ok_or(())?;
    store.transcripts.touch = touch;
    let file = store.transcripts.files.get_mut(&read.task).ok_or(())?;
    file.touched = touch;
    below.transcripts.push(file::Request::ReadAt { owner: OWNER, file: file.token, offset, max });
    Ok(())
}
pub(crate) fn arrived(store: &mut Store, arrived: Arrived, above: &mut Queue<Event>, below: &mut Below) {
    let Some(owner) = store.transcripts.reading else { return };
    let mut read = store.transcripts.reads.remove(&owner).expect("active reader");
    let result = match arrival(arrived.event) {
        Some(event) => advance(store, &mut read, event, above, below),
        None => Err(()),
    };
    settle(store, owner, read, result, above);
}
fn advance(
    store: &mut Store,
    read: &mut Read,
    event: Arrival,
    above: &mut Queue<Event>,
    below: &mut Below,
) -> Result<bool, ()> {
    let step = core::mem::replace(&mut read.step, Step::Waiting);
    match step {
        Step::Opening => {
            let file = match event {
                Arrival::Opened(file) => file,
                Arrival::Closed | Arrival::Read(_) => return Err(()),
            };
            store
                .transcripts
                .files
                .insert(
                    read.task,
                    File { token: file, touched: store.transcripts.touch, writable: false, readable: true },
                )
                .expect("bounded file slot after open");
            next(store, read, below)
        }
        Step::Evicting { task } => {
            match event {
                Arrival::Closed => {}
                Arrival::Opened(_) | Arrival::Read(_) => return Err(()),
            }
            let _: Option<File> = store.transcripts.files.remove(&task);
            opening(store, read, below);
            Ok(false)
        }
        Step::Length { offset, bytes } => length(store, read, event, offset, bytes, below),
        Step::Sized { offset, length, turn, size, mut bytes } => {
            if !append(event, &mut bytes)? {
                demand(
                    store,
                    read,
                    offset
                        .checked_add(u64::from(length))
                        .ok_or(())?
                        .checked_sub(4)
                        .ok_or(())?
                        .checked_add(u64::try_from(bytes.written()).expect("length"))
                        .ok_or(())?,
                    u32::try_from(bytes.room()).expect("length"),
                    below,
                )?;
                read.step = Step::Sized { offset, length, turn, size, bytes };
                return Ok(false);
            }
            if Reader::new(&bytes.finish()).u32() != Some(length) {
                return Err(());
            }
            sized(store, read, offset, turn, size, below)
        }
        Step::Header { offset, length, mut bytes } => {
            if !append(event, &mut bytes)? {
                demand(
                    store,
                    read,
                    offset.checked_add(u64::try_from(bytes.written()).expect("header")).ok_or(())?,
                    u32::try_from(bytes.room()).expect("header"),
                    below,
                )?;
                read.step = Step::Header { offset, length, bytes };
                return Ok(false);
            }
            header(store, read, offset, length, bytes.finish(), below)
        }
        Step::Frame { offset, length, mut bytes } => {
            if !append(event, &mut bytes)? {
                let consumed = u64::try_from(bytes.written()).expect("bounded frame");
                let max = u32::try_from(bytes.room()).expect("bounded frame");
                demand(store, read, offset.checked_add(consumed).ok_or(())?, max, below)?;
                read.step = Step::Frame { offset, length, bytes };
                return Ok(false);
            }
            match frame::Decode::begin(&bytes.finish(), &mut store.transcripts.machines, &store.codec).or(Err(()))? {
                frame::Decode::Done { turn, size } => {
                    let charged_frame =
                        store.transcripts.machines.body.is_some() || store.transcripts.machines.turn.is_some();
                    if charged_frame && let Some(party) = &mut read.party {
                        party.frame_bytes = Some(size);
                    }
                    accept(store, read, Decoded { offset, length, turn, size, charge_frame: charged_frame }, below)
                }
                frame::Decode::Patching(patch) => patching(store, read, offset, length, patch, above, below),
            }
        }
        Step::Waiting | Step::Site { .. } => Err(()),
    }
}
fn length(
    store: &mut Store,
    read: &mut Read,
    event: Arrival,
    offset: u64,
    mut bytes: Writer,
    below: &mut Below,
) -> Result<bool, ()> {
    if !append(event, &mut bytes)? {
        let consumed = u64::try_from(bytes.written()).expect("four byte length");
        let max = u32::try_from(bytes.room()).expect("four byte length");
        demand(store, read, offset.checked_add(consumed).ok_or(())?, max, below)?;
        read.step = Step::Length { offset, bytes };
        return Ok(false);
    }
    let length = Reader::new(&bytes.finish()).u32().ok_or(())?;
    let max = crate::core_v6::TurnRecord::worst_case_bytes(&store.codec.core).ok_or(())?.checked_add(12).ok_or(())?;
    if length < 12 || u64::from(length) > max {
        return Err(());
    }
    let start = if read.backwards { read.position.checked_sub(u64::from(length)).ok_or(())? } else { read.position };
    if start.checked_add(u64::from(length)).ok_or(())? > read.end {
        return Err(());
    }
    if read.activity.is_some() || party::single(read) {
        if length < 48 {
            if store.transcripts.machines.body.is_some() {
                read.step =
                    Step::Frame { offset: start, length, bytes: Writer::new(usize::try_from(length).expect("frame")) };
                demand(store, read, start, length, below)?;
                return Ok(false);
            }
            return Err(());
        }
        read.step = Step::Header { offset: start, length, bytes: Writer::new(36) };
        demand(store, read, start, 36, below)?;
        return Ok(false);
    }
    read.step = Step::Frame { offset: start, length, bytes: Writer::new(usize::try_from(length).expect("u32 frame")) };
    demand(store, read, start, length, below)?;
    Ok(false)
}
// Read only the fixed fields and payload length before deciding whether bytes fit.
fn header(
    store: &mut Store,
    read: &mut Read,
    offset: u64,
    length: u32,
    bytes: Box<[u8]>,
    below: &mut Below,
) -> Result<bool, ()> {
    if bytes.len() == 36 {
        return extend_header(store, read, offset, length, bytes, below);
    }
    let mut prefix = Reader::new(&bytes);
    if prefix.u32() != Some(length) {
        return Err(());
    }
    let versions = if store.transcripts.machines.turn.is_some() { 2 } else { 0 };
    let fields = bytes.len().checked_sub(8).ok_or(())?.checked_sub(versions).ok_or(())?;
    let mut body = Writer::new(fields.checked_add(4).ok_or(())?);
    body.put(prefix.bytes(u32::try_from(fields).or(Err(()))?).ok_or(())?).or(Err(()))?;
    body.put(&0_u32.to_be_bytes()).or(Err(()))?;
    let turn = codec::core::read_turn(&body.finish(), &codec::core_limits(&store.codec)).or(Err(()))?;
    let size = prefix.u32().ok_or(())?;
    if let Some(machine) = &store.transcripts.machines.turn {
        if size < 2 {
            return Err(());
        }
        let version = prefix.u16().ok_or(())?;
        let patcher = machine.patcher.as_ref().ok_or(())?;
        if patcher.schema(version).is_none() || version > machine.last {
            return Err(());
        }
        if version < machine.last {
            return whole_from_head(store, read, offset, length, &bytes, below);
        }
    }
    let expected = u64::try_from(bytes.len().checked_sub(versions).ok_or(())?)
        .or(Err(()))?
        .checked_add(u64::from(size))
        .ok_or(())?
        .checked_add(8)
        .ok_or(())?;
    if u64::from(length) != expected || size > store.codec.core.turn_record_transcript {
        return Err(());
    }
    let charged_frame = store.transcripts.machines.body.is_some() || store.transcripts.machines.turn.is_some();
    let size = if charged_frame { length } else { size };
    if charged_frame && let Some(party) = &mut read.party {
        party.frame_bytes = Some(length);
    }
    if party::single(read) {
        if party::header(store, read, &turn, size)? {
            if read.party.as_ref().expect("party").problem.is_some() {
                read.step = Step::Sized { offset, length, turn, size, bytes: Writer::new(4) };
                demand(
                    store,
                    read,
                    offset.checked_add(u64::from(length)).ok_or(())?.checked_sub(4).ok_or(())?,
                    4,
                    below,
                )?;
                return Ok(false);
            }
            return Ok(true);
        }
        read.step = Step::Frame { offset, length, bytes: Writer::new(usize::try_from(length).expect("bounded frame")) };
        demand(store, read, offset, length, below)?;
        return Ok(false);
    }
    activity_header(store, read, offset, length, turn, size, below)
}
fn activity_header(
    store: &mut Store,
    read: &mut Read,
    offset: u64,
    length: u32,
    turn: TurnRecord,
    size: u32,
    below: &mut Below,
) -> Result<bool, ()> {
    let page = read.activity.as_mut().expect("activity header only");
    let identity = (turn.task, turn.attempt, turn.turn);
    let sought = page.identity().ok_or(())?;
    let selected = match read.scan {
        Scan::Turn { attempt, turn } => {
            if identity.0 != read.task || identity.1 != attempt || identity.2 > turn {
                return Err(());
            }
            identity.2 == turn
        }
        Scan::Page => true,
        Scan::Window { .. } => return Err(()),
    };
    if selected {
        if sought != identity {
            return Err(());
        }
        let row = page.rows.get_mut(page.index).expect("pending turn row");
        row.related = Some(domain::Record::Core(domain::CoreRecord::Turn(turn.clone())));
        let cost = row.owned_bytes().ok_or(())?.checked_add(u64::from(size)).ok_or(())?;
        row.related = None;
        if cost > page.bound {
            if !read.backwards {
                read.step = Step::Sized { offset, length, turn, size, bytes: Writer::new(4) };
                demand(
                    store,
                    read,
                    offset.checked_add(u64::from(length)).ok_or(())?.checked_sub(4).ok_or(())?,
                    4,
                    below,
                )?;
                return Ok(false);
            }
            return sized(store, read, offset, turn, size, below);
        }
        if cost > page.bytes {
            return Ok(true);
        }
    }
    read.step = Step::Frame { offset, length, bytes: Writer::new(usize::try_from(length).expect("bounded frame")) };
    demand(store, read, offset, length, below)?;
    Ok(false)
}
fn sized(
    store: &mut Store,
    read: &mut Read,
    offset: u64,
    turn: TurnRecord,
    size: u32,
    below: &mut Below,
) -> Result<bool, ()> {
    if read.party.is_some() {
        return Ok(true);
    }
    let page = read.activity.as_mut().expect("activity size");
    let row = page.rows.get_mut(page.index).expect("pending turn row");
    row.unjoined = Some(domain::reads::UnjoinedTurn::TooLarge {
        task: turn.task,
        attempt: turn.attempt,
        turn: turn.turn,
        spent: turn.spent,
        read: turn.read,
        at: turn.at,
        bytes: u64::from(size),
    });
    page.bytes = page.bytes.checked_sub(row.owned_bytes().ok_or(())?).ok_or(())?;
    page.index = page.index.checked_add(1).expect("bounded page index");
    older(store, read, offset, below)
}
fn older(store: &mut Store, read: &mut Read, offset: u64, below: &mut Below) -> Result<bool, ()> {
    let page = read.activity.as_mut().expect("activity walk");
    let Ok(done) = page.charge_words() else { return Ok(true) };
    read.position = offset;
    read.backwards = true;
    read.scan = Scan::Page;
    if done { Ok(true) } else { next(store, read, below) }
}
fn append(event: Arrival, buffer: &mut Writer) -> Result<bool, ()> {
    let bytes = match event {
        Arrival::Read(bytes) => bytes,
        Arrival::Opened(_) | Arrival::Closed => return Err(()),
    };
    if bytes.is_empty() {
        return Err(());
    }
    if buffer.put(&bytes).is_err() {
        return Err(());
    }
    Ok(buffer.room() == 0)
}
fn accept(store: &mut Store, read: &mut Read, frame: Decoded, below: &mut Below) -> Result<bool, ()> {
    let Decoded { offset, length, size, charge_frame, turn } = frame;
    if turn.task != read.task {
        return Err(());
    }
    let found = match &mut read.scan {
        Scan::Window { attempt, turn: number, after, checked } => {
            if !*checked {
                if turn.attempt != *attempt || turn.turn != *number || !turn.opens_window {
                    return Err(());
                }
                *checked = true;
                if let Some(after) = *after
                    && after != offset
                {
                    read.position = after;
                    return next(store, read, below);
                }
            }
            true
        }
        Scan::Turn { attempt, turn: number } => {
            if turn.attempt != *attempt || turn.turn > *number {
                return Err(());
            }
            turn.turn == *number
        }
        Scan::Page => true,
    };
    if found && let Some(page) = &mut read.activity {
        if page.identity() != Some((turn.task, turn.attempt, turn.turn)) {
            return Err(());
        }
        let row = page.rows.get_mut(page.index).expect("pending turn row");
        let payload = u64::try_from(turn.transcript.len()).or(Err(()))?;
        row.related = Some(domain::Record::Core(domain::CoreRecord::Turn(turn)));
        let mut bytes = row.owned_bytes().ok_or(())?;
        if charge_frame {
            bytes = bytes.checked_sub(payload).ok_or(())?.checked_add(u64::from(size)).ok_or(())?;
        }
        if bytes > page.bytes {
            row.related = None;
            return Ok(true);
        }
        page.bytes = page.bytes.checked_sub(bytes).expect("accepted turn fits");
        page.index = page.index.checked_add(1).expect("bounded page index");
        return older(store, read, offset, below);
    }
    if found {
        let single = party::single(read);
        if read.turns.room() == 0 || (!single && u64::from(size) > read.budget) {
            return Ok(true);
        }
        if !single {
            read.budget = read.budget.checked_sub(u64::from(size)).expect("frame within budget");
        }
        read.turns.push(turn).expect("next checked row capacity");
    }
    read.position = if read.backwards { offset } else { offset.checked_add(u64::from(length)).ok_or(())? };
    match read.scan {
        Scan::Turn { .. } => {
            if found {
                Ok(true)
            } else if read.position == read.end {
                if party::single(read) { Ok(true) } else { Err(()) }
            } else {
                next(store, read, below)
            }
        }
        Scan::Window { .. } | Scan::Page => next(store, read, below),
    }
}
fn settle(store: &mut Store, owner: Token, mut read: Read, result: Result<bool, ()>, above: &mut Queue<Event>) {
    if result != Ok(false)
        && let Some(mut page) = read.activity.take()
    {
        store.transcripts.reading = None;
        if result.is_err() {
            page.unread();
        }
        let cut = result.is_err() || page.index < page.rows.len();
        above.push(Event::Read { owner, result: page.finish(cut) });
        return;
    }
    match result {
        Ok(false) => {
            store.transcripts.reads.insert(owner, read).expect("active read retains its slot");
        }
        Ok(true) => {
            if read.party.is_some() {
                store.transcripts.reading = None;
                let result = party::finish(store, read, false);
                reads::answer(store, owner, reads::ready(result), above);
                return;
            }
            let end = if read.backwards { 0 } else { read.end };
            if read.turns.is_empty() && read.position != end {
                store.transcripts.reading = None;
                above.push(Event::Unread { owner });
                return;
            }
            let next = match read.scan {
                Scan::Turn { .. } => None,
                Scan::Window { .. } | Scan::Page => {
                    if read.position == end {
                        None
                    } else {
                        Some(read.position)
                    }
                }
            };
            let mut turns = read.turns.into_boxed();
            if read.backwards {
                turns.reverse();
            }
            store.transcripts.reading = None;
            above.push(Event::Turns { owner, turns, next });
        }
        Err(()) => {
            store.transcripts.reading = None;
            match &read.party {
                Some(_) => {
                    let result = party::finish(store, read, true);
                    reads::answer(store, owner, reads::ready(result), above);
                }
                None => above.push(Event::Unread { owner }),
            }
        }
    }
}
enum Arrival {
    Opened(Token),
    Closed,
    Read(Box<[u8]>),
}
fn arrival(event: file::Event) -> Option<Arrival> {
    match event {
        file::Event::Opened { file, .. } => Some(Arrival::Opened(file)),
        file::Event::Closed { .. } => Some(Arrival::Closed),
        file::Event::Read { bytes, .. } => Some(Arrival::Read(bytes)),
        file::Event::Made { .. }
        | file::Event::Refused { .. }
        | file::Event::Locked { .. }
        | file::Event::Busy { .. }
        | file::Event::Loaded { .. }
        | file::Event::Scanned { .. }
        | file::Event::Stored { .. }
        | file::Event::Conflict { .. }
        | file::Event::TooLarge { .. }
        | file::Event::Stated { .. }
        | file::Event::Written { .. }
        | file::Event::Cut { .. }
        | file::Event::Synced { .. }
        | file::Event::Renamed { .. }
        | file::Event::Removed { .. }
        | file::Event::Listed { .. }
        | file::Event::Failed { .. }
        | file::Event::Cancelled { .. } => None,
    }
}

/// Continue from the bytes already read: a behind head is never reread.
fn whole_from_head(
    store: &mut Store,
    read: &mut Read,
    offset: u64,
    length: u32,
    head: &[u8],
    below: &mut Below,
) -> Result<bool, ()> {
    let mut bytes = Writer::new(usize::try_from(length).expect("u32 frame"));
    bytes.put(head).or(Err(()))?;
    let consumed = u64::try_from(bytes.written()).or(Err(()))?;
    let max = u32::try_from(bytes.room()).or(Err(()))?;
    if max == 0 {
        return Err(());
    }
    read.step = Step::Frame { offset, length, bytes };
    demand(store, read, offset.checked_add(consumed).ok_or(())?, max, below)?;
    Ok(false)
}

fn patching(
    store: &mut Store,
    read: &mut Read,
    offset: u64,
    length: u32,
    mut patch: frame::Patch,
    above: &mut Queue<Event>,
    below: &mut Below,
) -> Result<bool, ()> {
    match patch.next(&mut store.transcripts.machines, &store.codec).or(Err(()))? {
        frame::Progress::Written { of, site, at } => {
            let key = keys::encode_own(&keys::Own::Transcript { task: read.task }, store.limits.store.key).ok_or(())?;
            let mut name = Writer::new(key.len().checked_add(8).ok_or(())?);
            name.put(&key).or(Err(()))?;
            name.put(&offset.to_be_bytes()).or(Err(()))?;
            let machine = match of {
                crate::Owner::Jig => store.transcripts.machines.body.as_mut(),
                crate::Owner::Smith => store.transcripts.machines.turn.as_mut(),
                crate::Owner::Skein | crate::Owner::Application => return Err(()),
            }
            .ok_or(())?;
            let patcher = machine.patcher.take().ok_or(())?;
            let owner = store.transcripts.reading.expect("active read owns the frame machine");
            above.push(Event::Site { owner, of, key: name.finish(), site, at, patcher });
            read.step = Step::Site { offset, length, patch, of };
            Ok(false)
        }
        frame::Progress::Done { mut turn, size } => {
            if let Some(party) = &mut read.party {
                party.frame_bytes = Some(size);
            }
            // Fixed metadata is judged exactly as a head's, after patching.
            let transcript = core::mem::replace(&mut turn.transcript, Box::new([]));
            if party::single(read) && party::header(store, read, &turn, size)? {
                return Ok(true);
            }
            if let Some(page) = &mut read.activity {
                let selected = match read.scan {
                    Scan::Turn { attempt, turn: number } => turn.attempt == attempt && turn.turn == number,
                    Scan::Page => true,
                    Scan::Window { .. } => return Err(()),
                };
                if selected {
                    if page.identity() != Some((turn.task, turn.attempt, turn.turn)) {
                        return Err(());
                    }
                    let row = page.rows.get_mut(page.index).expect("pending row");
                    row.related = Some(domain::Record::Core(domain::CoreRecord::Turn(turn.clone())));
                    let cost = row.owned_bytes().ok_or(())?.checked_add(u64::from(size)).ok_or(())?;
                    row.related = None;
                    if cost > page.bound {
                        return sized(store, read, offset, turn, size, below);
                    }
                    if cost > page.bytes {
                        return Ok(true);
                    }
                }
            }
            turn.transcript = transcript;
            accept(store, read, Decoded { offset, length, turn, size, charge_frame: true }, below)
        }
    }
}

/// A serving site belongs to the active read alone. A stray goes through the
/// existing open-refusal path, including its fail-stop in Ready.
pub(crate) fn site(
    store: &mut Store,
    env: &skein_lib::Env<Limits>,
    owner: Token,
    patcher: Box<skein_codec::patch::Patcher>,
    result: Result<(), skein_codec::patch::Halt>,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    let waiting = if store.transcripts.reading == Some(owner) {
        match store.transcripts.reads.get(&owner) {
            Some(read) => match read.step {
                Step::Site { .. } => true,
                Step::Waiting
                | Step::Opening
                | Step::Evicting { .. }
                | Step::Length { .. }
                | Step::Header { .. }
                | Step::Sized { .. }
                | Step::Frame { .. } => false,
            },
            None => false,
        }
    } else {
        false
    };
    if !waiting {
        crate::patching::phase::site(store, env, owner, patcher, result, above, below);
        return;
    }
    let mut read = store.transcripts.reads.remove(&owner).expect("site's active reader");
    let step = core::mem::replace(&mut read.step, Step::Waiting);
    let resumed = match step {
        Step::Site { offset, length, patch, of } => {
            let machine = match of {
                crate::Owner::Jig => store.transcripts.machines.body.as_mut(),
                crate::Owner::Smith => store.transcripts.machines.turn.as_mut(),
                crate::Owner::Skein | crate::Owner::Application => unreachable!("only frames' owners stop"),
            }
            .expect("site's machine");
            assert!(machine.patcher.is_none(), "one outstanding site on the machine");
            machine.patcher = Some(patcher);
            if result.is_err() { Err(()) } else { patching(store, &mut read, offset, length, patch, above, below) }
        }
        Step::Waiting
        | Step::Opening
        | Step::Evicting { .. }
        | Step::Length { .. }
        | Step::Header { .. }
        | Step::Sized { .. }
        | Step::Frame { .. } => unreachable!("checked waiting site"),
    };
    settle(store, owner, read, resumed, above);
}

fn extend_header(
    store: &mut Store,
    read: &mut Read,
    offset: u64,
    length: u32,
    bytes: Box<[u8]>,
    below: &mut Below,
) -> Result<bool, ()> {
    if let Some(machine) = &store.transcripts.machines.body {
        let version = skein_codec::version_of(bytes.get(4..).ok_or(())?).or(Err(()))?;
        if version != machine.last {
            return whole_from_head(store, read, offset, length, &bytes, below);
        }
    }
    let header_size = match *bytes.get(35).ok_or(())? {
        0 => 48_u32,
        1 => 56_u32,
        _ => return Err(()),
    };
    if length < header_size.checked_add(8).ok_or(())? {
        return Err(());
    }
    let header_size =
        if store.transcripts.machines.turn.is_some() { header_size.checked_add(2).ok_or(())? } else { header_size };
    if length < header_size.checked_add(8).ok_or(())? {
        return Err(());
    }
    let tail = header_size.checked_sub(36).expect("fixed header");
    let mut header = Writer::new(usize::try_from(header_size).expect("fixed header"));
    header.put(&bytes).or(Err(()))?;
    read.step = Step::Header { offset, length, bytes: header };
    demand(store, read, offset.checked_add(36).ok_or(())?, tail, below)?;
    Ok(false)
}
