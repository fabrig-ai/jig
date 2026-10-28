//! Private values kept before their owners use them (store.md, section 9).
use crate::{
    Below, CARRIED, CarriedKind, Event, HELD, Limits, Phase, SecretOwner, SecretRow, SecretValue, Secrets, Store,
    directory, keys, open,
};
pub(super) mod codec;
use alloc::boxed::Box;
use skein_lib::{Env, List, Map, Queue, Secret, Token, sha256};

#[derive(Debug)]
enum Pending {
    /// A keep or an erase on its way: its asker, its key, and the digest of
    /// what the key holds once it is durable, none once erased, which a
    /// replace of the key states before then.
    Write {
        owner: Token,
        key: Box<[u8]>,
        holds: Option<[u8; 32]>,
    },
    /// A carried value on its way under `number`, the counter moved past it.
    Carry {
        owner: Token,
        key: Box<[u8]>,
        holds: [u8; 32],
        number: u64,
    },
    Adopt {
        owner: Token,
        key: Box<[u8]>,
        holds: [u8; 32],
        carried: Box<[u8]>,
    },
    Sweep,
}
#[derive(Debug)]
pub(crate) struct State {
    pending: Map<u64, Pending>,
    next: u64,
    sweeping: bool,
    swept: bool,
    after: Option<Box<[u8]>>,
    waiting: bool,
    /// The next carried number, once read from its row.
    carried: Option<u64>,
    /// Memory-only values; Secret redacts its Debug. Neither table nor counter is persisted.
    held: Map<u64, Secret>,
    next_held: u64,
}
impl State {
    pub fn new(capacity: u32, held: u32) -> State {
        State {
            pending: Map::with_capacity(capacity),
            next: 1,
            sweeping: false,
            swept: false,
            after: None,
            waiting: false,
            carried: None,
            held: Map::with_capacity(held),
            next_held: HELD,
        }
    }
}
pub(crate) fn worst_case(limits: &Limits) -> Option<u64> {
    Map::<u64, Pending>::worst_case(limits.secrets.queued)?
        .checked_add(Map::<u64, Secret>::worst_case(limits.held)?)?
        .checked_add(u64::from(limits.held).checked_mul(u64::from(limits.secrets.value))?)?
        .checked_add(u64::from(limits.secrets.key).checked_mul(2)?.checked_mul(u64::from(limits.secrets.queued))?)?
        .checked_add(u64::from(limits.secrets.key))?
        .checked_add(List::<SecretRow>::worst_case(limits.secrets.page.rows)?)?
        .checked_add(List::<skein_kv::Row>::worst_case(limits.secrets.page.rows)?)?
        .checked_add(List::<skein_kv::Op>::worst_case(limits.secrets.ops)?)?
        // A conditional write holds its encoded replacement and durable row;
        // its digest borrows that row's payload without a decoded buffer.
        .checked_add(u64::from(limits.secrets.value).checked_mul(2)?)?
        .checked_add(32)?
        .checked_add(u64::from(limits.secrets.page.bytes).checked_mul(2)?)
}
fn prefix(owner: SecretOwner) -> [u8; 2] {
    match owner {
        SecretOwner::SignIn => [1, 0],
        SecretOwner::Service => [1, 1],
        SecretOwner::Application => [2, 0],
        SecretOwner::Source => [3, 0],
    }
}
pub(super) fn key(owner: SecretOwner, number: u64, limit: u32) -> Option<Box<[u8]>> {
    let [tag, owner] = prefix(owner);
    let mut key = skein_kv::KeyWriter::new(limit);
    key.tag(tag).tag(owner).u64(number);
    key.finish()
}
/// Application ranges are relative to their secrets prefix (store.md 9, 10.3).
pub(crate) fn application(relative: &[u8], limit: u32) -> Option<Box<[u8]>> {
    let mut key = skein_kv::KeyWriter::new(limit);
    for byte in prefix(SecretOwner::Application) {
        key.tag(byte);
    }
    for &byte in relative {
        key.tag(byte);
    }
    key.finish()
}
pub(crate) fn of_application(key: &[u8]) -> Option<&[u8]> {
    key.strip_prefix(&prefix(SecretOwner::Application))
}
fn read_key(key: &[u8]) -> Option<(SecretOwner, u64)> {
    let mut reader = skein_kv::KeyReader::new(key);
    let tag = reader.tag()?;
    let kind = reader.tag()?;
    let owner = match tag {
        1 => match kind {
            0 => SecretOwner::SignIn,
            1 => SecretOwner::Service,
            _ => return None,
        },
        2 | 3 => {
            if kind != 0 {
                return None;
            }
            if tag == 2 { SecretOwner::Application } else { SecretOwner::Source }
        }
        _ => return None,
    };
    let number = reader.u64()?;
    if !reader.is_empty() {
        return None;
    }
    Some((owner, number))
}
struct Loaded {
    rows: Box<[skein_kv::Row]>,
    next: Option<Box<[u8]>>,
}
fn load(store: &mut Store, env: &Env<Limits>, range: skein_kv::Range, max: skein_kv::Page) -> Option<Loaded> {
    let kv_env = directory::env(store, env, true);
    let mut above = Queue::uncapped();
    let mut below = Queue::uncapped();
    skein_kv::down(
        &mut store.stores.secrets,
        &kv_env,
        skein_kv::Request::Load { owner: Token::new(0), range, max },
        &mut above,
        &mut below,
    );
    assert!(below.is_empty(), "a secrets page issues no files");
    match above.pop()? {
        skein_kv::Event::Loaded { rows, next, .. } => Some(Loaded { rows, next }),
        skein_kv::Event::Opened { .. }
        | skein_kv::Event::Committed { .. }
        | skein_kv::Event::Refused { .. }
        | skein_kv::Event::Failed { .. }
        | skein_kv::Event::Got { .. }
        | skein_kv::Event::Closed { .. }
        | skein_kv::Event::OpenFailed { .. }
        | skein_kv::Event::Recovered { .. }
        | skein_kv::Event::RecoveryFailed { .. } => None,
    }
}
fn submit(
    store: &mut Store,
    env: &Env<Limits>,
    ops: Box<[skein_kv::Op]>,
    pending: Pending,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    let number = store.secret_state.next;
    let Some(next) = number.checked_add(1) else {
        store.phase = Phase::Stopped;
        return;
    };
    if store.secret_state.pending.insert(number, pending).is_err() {
        store.phase = Phase::Stopped;
        return;
    }
    store.secret_state.next = next;
    let kv_env = directory::env(store, env, true);
    let mut events = Queue::uncapped();
    skein_kv::down(
        &mut store.stores.secrets,
        &kv_env,
        skein_kv::Request::Commit { owner: Token::new(number), ops },
        &mut events,
        &mut below.secrets,
    );
    directory::events_up(store, true, &mut events, above, below);
}
pub(crate) fn committed(store: &mut Store, owner: Token, above: &mut Queue<Event>) {
    let Some(pending) = store.secret_state.pending.remove(&owner.raw()) else {
        store.phase = Phase::Stopped;
        return;
    };
    match pending {
        Pending::Write { owner, holds: Some(_), .. } | Pending::Adopt { owner, .. } => {
            above.push(Event::Kept { owner });
        }
        Pending::Write { owner, holds: None, .. } => above.push(Event::Erased { owner }),
        Pending::Carry { owner, number, .. } => above.push(Event::Carried { owner, number }),
        Pending::Sweep => store.secret_state.waiting = false,
    }
}
/// The durable value under `key`; `Err` when the store answered otherwise.
fn durable(store: &mut Store, env: &Env<Limits>, key: Box<[u8]>, below: &mut Below) -> Result<Option<Box<[u8]>>, ()> {
    let kv_env = directory::env(store, env, true);
    let mut events = Queue::uncapped();
    skein_kv::down(
        &mut store.stores.secrets,
        &kv_env,
        skein_kv::Request::Get { owner: Token::new(0), key },
        &mut events,
        &mut below.secrets,
    );
    match events.pop().expect("a secret get answers in its step") {
        skein_kv::Event::Got { value, .. } => Ok(value),
        skein_kv::Event::Opened { .. }
        | skein_kv::Event::Committed { .. }
        | skein_kv::Event::Refused { .. }
        | skein_kv::Event::Failed { .. }
        | skein_kv::Event::Loaded { .. }
        | skein_kv::Event::Closed { .. }
        | skein_kv::Event::OpenFailed { .. }
        | skein_kv::Event::Recovered { .. }
        | skein_kv::Event::RecoveryFailed { .. } => Err(()),
    }
}
/// The digest of what `key` holds: the latest write on its way, else the
/// durable value; none when nothing is held.
fn held(store: &mut Store, env: &Env<Limits>, key: &[u8], below: &mut Below) -> Result<Option<[u8; 32]>, ()> {
    for (_, pending) in store.secret_state.pending.iter().rev() {
        match pending {
            Pending::Write { key: written, holds, .. } if **written == *key => return Ok(*holds),
            Pending::Carry { key: written, holds, .. } if **written == *key => return Ok(Some(*holds)),
            Pending::Adopt { key: written, holds, .. } if **written == *key => return Ok(Some(*holds)),
            Pending::Adopt { carried, .. } if **carried == *key => return Ok(None),
            Pending::Write { .. } | Pending::Carry { .. } | Pending::Adopt { .. } | Pending::Sweep => {}
        }
    }
    match durable(store, env, Box::from(key), below)? {
        Some(value) => {
            let (kind, number) = read_key(key).ok_or(())?;
            Ok(Some(codec::durable_digest(kind, number, &value).ok_or(())?))
        }
        None => Ok(None),
    }
}
fn ready(store: &Store) -> bool {
    match store.phase {
        Phase::Ready { .. } => store.mark_durable(),
        Phase::Closed | Phase::Stopped => false,
    }
}

pub(crate) fn secrets(
    store: &mut Store,
    env: &Env<Limits>,
    owner: Token,
    request: Secrets,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    if !ready(store) {
        above.push(Event::SecretsNotReady { owner });
        return;
    }
    match request {
        Secrets::Keep { owner: kind, number, value } => keep(store, env, owner, (kind, number), value, above, below),
        Secrets::Replace { owner: kind, number, value, replaces } => {
            let Some(value) = prepare(store, kind, number, value) else {
                store.phase = Phase::Stopped;
                return;
            };
            let Some(key) = key(kind, number, store.limits.secrets.key) else {
                store.phase = Phase::Stopped;
                return;
            };
            let Ok(holds) = held(store, env, &key, below) else {
                corrupt(store, owner, above);
                return;
            };
            if holds == replaces {
                write(store, env, owner, key, value, above, below);
            } else {
                above.push(Event::Stale { owner });
            }
        }
        Secrets::Adopt { carried, owner: kind, number, replaces } => {
            adopt(store, env, owner, (kind, number), (carried, replaces), above, below);
        }
        Secrets::AdoptValue { carried, holds, owner: kind, number, value, replaces } => {
            adopt_value(store, env, owner, (kind, number), (carried, holds, replaces, value), above, below);
        }
        Secrets::Hold { value } => hold(store, owner, value, above),
        Secrets::Carry { kind, value } => carry(store, env, owner, (kind, value), above, below),
        Secrets::Erase { owner: kind, number } => {
            if kind == SecretOwner::Application && number >= HELD {
                let _removed = store.secret_state.held.remove(&number);
                above.push(Event::Erased { owner });
                return;
            }
            let Some(key) = key(kind, number, store.limits.secrets.key) else {
                store.phase = Phase::Stopped;
                return;
            };
            let ops = Box::new([skein_kv::Op::Erase { key: key.clone() }]);
            submit(store, env, ops, Pending::Write { owner, key, holds: None }, above, below);
        }
        Secrets::Get { owner: kind, number } => get(store, env, owner, (kind, number), above, below),
        Secrets::Load { owner: kind, after } => {
            let mut range = skein_kv::Range::prefix(&prefix(kind));
            if let Some(number) = after {
                let Some(number) = number.checked_add(1) else {
                    above.push(Event::SecretsLoaded { owner, rows: Box::new([]), next: None });
                    return;
                };
                let Some(key) = key(kind, number, store.limits.secrets.key) else {
                    store.phase = Phase::Stopped;
                    return;
                };
                range.start = key;
            }
            let Some(Loaded { rows, next }) = load(store, env, range, store.limits.secrets.page) else {
                store.phase = Phase::Stopped;
                return;
            };
            let mut found = List::with_capacity(store.limits.secrets.page.rows);
            let mut last = None;
            for row in rows {
                let Some((kind, number)) = read_key(&row.key) else {
                    store.phase = Phase::Stopped;
                    return;
                };
                last = Some(number);
                let Some(value) = codec::decode(kind, number, row.value) else {
                    corrupt(store, owner, above);
                    return;
                };
                found.push(SecretRow { owner: kind, number, value }).expect("bounded secret page");
            }
            above.push(Event::SecretsLoaded {
                owner,
                rows: found.into_boxed(),
                next: if next.is_some() { last } else { None },
            });
        }
    }
}
/// Both rows change in one secrets commit; pending digests guard concurrent adopts.
fn adopt(
    store: &mut Store,
    env: &Env<Limits>,
    owner: Token,
    (kind, number): (SecretOwner, u64),
    (carried, replaces): (u64, Option<[u8; 32]>),
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    if kind != SecretOwner::Source {
        store.phase = Phase::Stopped;
        return;
    }
    if !(CARRIED..HELD).contains(&carried) {
        above.push(Event::Stale { owner });
        return;
    }
    let Some(target) = key(kind, number, store.limits.secrets.key) else {
        store.phase = Phase::Stopped;
        return;
    };
    let Some(carried) = key(SecretOwner::Application, carried, store.limits.secrets.key) else {
        store.phase = Phase::Stopped;
        return;
    };
    let Ok(target_digest) = held(store, env, &target, below) else {
        corrupt(store, owner, above);
        return;
    };
    let Ok(carried_digest) = held(store, env, &carried, below) else {
        corrupt(store, owner, above);
        return;
    };
    if target == carried || target_digest != replaces || carried_digest.is_none() {
        above.push(Event::Stale { owner });
        return;
    }
    let value = match durable(store, env, carried.clone(), below) {
        Ok(Some(value)) => value,
        Ok(None) => {
            above.push(Event::Stale { owner });
            return;
        }
        Err(()) => {
            store.phase = Phase::Stopped;
            return;
        }
    };
    let value = match codec::decode(SecretOwner::Application, read_key(&carried).expect("carried key").1, value) {
        Some(SecretValue::Carried { kind: CarriedKind::Key, value }) => value,
        Some(SecretValue::Carried { kind: CarriedKind::Value | CarriedKind::Moved, .. }) => {
            above.push(Event::Stale { owner });
            return;
        }
        Some(
            SecretValue::Digest { .. }
            | SecretValue::Record(_)
            | SecretValue::Key(_)
            | SecretValue::Application(_)
            | SecretValue::Held(_),
        )
        | None => {
            corrupt(store, owner, above);
            return;
        }
    };
    let holds = sha256::digest(value.expose());
    if Some(holds) != carried_digest {
        above.push(Event::Stale { owner });
        return;
    }
    let Some(value) = encode(store, SecretValue::Key(value)) else {
        store.phase = Phase::Stopped;
        return;
    };
    let ops =
        Box::new([skein_kv::Op::Put { key: target.clone(), value }, skein_kv::Op::Erase { key: carried.clone() }]);
    submit(store, env, ops, Pending::Adopt { owner, key: target, holds, carried }, above, below);
}
/// The carried digest pins the decoded identity the child accepted. Pending
/// digests include both writes, so a racing adoption cannot consume it twice.
fn adopt_value(
    store: &mut Store,
    env: &Env<Limits>,
    owner: Token,
    (kind, number): (SecretOwner, u64),
    (carried, holds, replaces, value): (u64, [u8; 32], Option<[u8; 32]>, SecretValue),
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    let Some(Encoded { value, holds: digest }) = prepare(store, kind, number, value) else {
        store.phase = Phase::Stopped;
        return;
    };
    if !(CARRIED..HELD).contains(&carried) {
        above.push(Event::Stale { owner });
        return;
    }
    let Some(target) = key(kind, number, store.limits.secrets.key) else {
        store.phase = Phase::Stopped;
        return;
    };
    let Some(carried) = key(SecretOwner::Application, carried, store.limits.secrets.key) else {
        store.phase = Phase::Stopped;
        return;
    };
    let Ok(target_digest) = held(store, env, &target, below) else {
        corrupt(store, owner, above);
        return;
    };
    let Ok(carried_digest) = held(store, env, &carried, below) else {
        corrupt(store, owner, above);
        return;
    };
    if target == carried || target_digest != replaces || carried_digest != Some(holds) {
        above.push(Event::Stale { owner });
        return;
    }
    let holds = digest;
    let ops =
        Box::new([skein_kv::Op::Put { key: target.clone(), value }, skein_kv::Op::Erase { key: carried.clone() }]);
    submit(store, env, ops, Pending::Adopt { owner, key: target, holds, carried }, above, below);
}
fn too_large(store: &Store, value: &Secret) -> bool {
    value.len() > usize::try_from(store.limits.secrets.value).expect("secret limit fits usize")
}
struct Encoded {
    value: Box<[u8]>,
    holds: [u8; 32],
}
fn encode(store: &Store, value: SecretValue) -> Option<Box<[u8]>> {
    let bytes = codec::encode(value, store.limits.secrets.value)?;
    if bytes.len() > usize::try_from(store.limits.secrets.value).expect("secret limit fits usize") {
        return None;
    }
    Some(bytes)
}
fn prepare(store: &Store, kind: SecretOwner, number: u64, value: SecretValue) -> Option<Encoded> {
    if !codec::accepts(kind, number, &value) {
        return None;
    }
    let holds = codec::digest(&value);
    Some(Encoded { value: encode(store, value)?, holds })
}
fn keep(
    store: &mut Store,
    env: &Env<Limits>,
    owner: Token,
    (kind, number): (SecretOwner, u64),
    value: SecretValue,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    let Some(value) = prepare(store, kind, number, value) else {
        store.phase = Phase::Stopped;
        return;
    };
    let Some(key) = key(kind, number, store.limits.secrets.key) else {
        store.phase = Phase::Stopped;
        return;
    };
    write(store, env, owner, key, value, above, below);
}
fn write(
    store: &mut Store,
    env: &Env<Limits>,
    owner: Token,
    key: Box<[u8]>,
    value: Encoded,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    let ops = Box::new([skein_kv::Op::Put { key: key.clone(), value: value.value }]);
    submit(store, env, ops, Pending::Write { owner, key, holds: Some(value.holds) }, above, below);
}
fn corrupt(store: &mut Store, owner: Token, above: &mut Queue<Event>) {
    store.phase = Phase::Stopped;
    above.push(Event::Corrupt { owner });
}
/// The row holding the next carried number, beside the application's
/// values (store.md 9).
fn counter_key(limit: u32) -> Option<Box<[u8]>> {
    let [tag, _] = prefix(SecretOwner::Application);
    let mut key = skein_kv::KeyWriter::new(limit);
    key.tag(tag).tag(1);
    key.finish()
}
/// The next carried number: the counter row's, read once, else the range's
/// first; `Err` when the store answered otherwise or the row is not one.
fn next_carried(store: &mut Store, env: &Env<Limits>, below: &mut Below) -> Result<u64, ()> {
    if let Some(next) = store.secret_state.carried {
        return Ok(next);
    }
    let key = counter_key(store.limits.secrets.key).ok_or(())?;
    let next = match durable(store, env, key, below)? {
        None => CARRIED,
        Some(row) => {
            let next = codec::next(&row).ok_or(())?;
            if next < CARRIED {
                return Err(());
            }
            next
        }
    };
    store.secret_state.carried = Some(next);
    Ok(next)
}
/// Keep a carried value under the next number, never one already holding
/// a value, and move the counter in the same commit.
fn carry(
    store: &mut Store,
    env: &Env<Limits>,
    owner: Token,
    (kind, value): (CarriedKind, Secret),
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    let holds = sha256::digest(value.expose());
    let Some(value) = encode(store, SecretValue::Carried { kind, value }) else {
        store.phase = Phase::Stopped;
        return;
    };
    if store.limits.secrets.value < codec::NEXT_BYTES {
        store.phase = Phase::Stopped;
        return;
    }
    let Ok(number) = next_carried(store, env, below) else {
        corrupt(store, owner, above);
        return;
    };
    if number >= HELD {
        above.push(Event::Stale { owner });
        return;
    }
    let Some(after) = number.checked_add(1) else {
        above.push(Event::Stale { owner });
        return;
    };
    let (Some(key), Some(counter)) =
        (key(SecretOwner::Application, number, store.limits.secrets.key), counter_key(store.limits.secrets.key))
    else {
        store.phase = Phase::Stopped;
        return;
    };
    match held(store, env, &key, below) {
        Ok(None) => {}
        Ok(Some(_)) => return above.push(Event::Stale { owner }),
        Err(()) => {
            store.phase = Phase::Stopped;
            return;
        }
    }
    store.secret_state.carried = Some(after);
    let ops = Box::new([
        skein_kv::Op::Put { key: counter, value: codec::counter(after) },
        skein_kv::Op::Put { key: key.clone(), value },
    ]);
    submit(store, env, ops, Pending::Carry { owner, key, holds, number }, above, below);
}
pub(crate) fn start_sweep(store: &mut Store) {
    if !store.secret_state.swept {
        store.secret_state.sweeping = true;
    }
}
pub(crate) fn sweep(store: &mut Store, env: &Env<Limits>, above: &mut Queue<Event>, below: &mut Below) {
    if !store.mark_durable()
        || !store.secret_state.sweeping
        || store.secret_state.waiting
        || store.phase == Phase::Stopped
    {
        return;
    }
    // People's family alone. The accounts component sweeps unnamed Source
    // rows after its restart inventory, preserving configured sources.
    // Application values remain: a carried number can hold a kept value.
    let mut range = skein_kv::Range::prefix(&[1]);
    if let Some(after) = store.secret_state.after.take() {
        range.start = after;
    }
    let max = skein_kv::Page {
        rows: store.limits.secrets.page.rows.min(store.limits.secrets.ops),
        bytes: store.limits.secrets.page.bytes,
    };
    let Some(Loaded { rows, next }) = load(store, env, range, max) else {
        store.phase = Phase::Stopped;
        return;
    };
    let mut ops = List::with_capacity(store.limits.secrets.ops);
    for row in rows {
        let Some((kind, number)) = read_key(&row.key) else {
            store.phase = Phase::Stopped;
            return;
        };
        let people = match kind {
            SecretOwner::SignIn => jig_domain_people::Key::SignIn(number),
            SecretOwner::Service => jig_domain_people::Key::Person(number),
            SecretOwner::Application | SecretOwner::Source => {
                store.phase = Phase::Stopped;
                return;
            }
        };
        let Some(key) = keys::encode(&jig_domain_core::Key::People(people), store.limits.store.key) else {
            store.phase = Phase::Stopped;
            return;
        };
        let Ok(value) = open::get(store, Token::new(0), key) else {
            store.phase = Phase::Stopped;
            return;
        };
        if value.is_none() {
            ops.push(skein_kv::Op::Erase { key: row.key }).expect("one op per swept row");
        }
    }
    store.secret_state.after = next;
    if store.secret_state.after.is_none() {
        store.secret_state.sweeping = false;
        store.secret_state.swept = true;
    }
    if !ops.is_empty() {
        store.secret_state.waiting = true;
        submit(store, env, ops.into_boxed(), Pending::Sweep, above, below);
    }
}

/// A held value never enters the kv store or the pending commits (store.md, section 9).
fn hold(store: &mut Store, owner: Token, value: Secret, above: &mut Queue<Event>) {
    if too_large(store, &value) {
        store.phase = Phase::Stopped;
        return;
    }
    let number = store.secret_state.next_held;
    let Some(next) = number.checked_add(1) else {
        above.push(Event::Stale { owner });
        return;
    };
    if store.secret_state.held.insert(number, value).is_err() {
        above.push(Event::Stale { owner });
        return;
    }
    store.secret_state.next_held = next;
    above.push(Event::Carried { owner, number });
}

#[expect(clippy::manual_map, reason = "step code translates optional payloads without closures")]
fn get(
    store: &mut Store,
    env: &Env<Limits>,
    owner: Token,
    (kind, number): (SecretOwner, u64),
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    if kind == SecretOwner::Application && number >= HELD {
        let value = match store.secret_state.held.get(&number) {
            Some(value) => Some(SecretValue::Held(value.clone())),
            None => None,
        };
        above.push(Event::Secret { owner, value });
        return;
    }
    let Some(key) = key(kind, number, store.limits.secrets.key) else {
        store.phase = Phase::Stopped;
        return;
    };
    match durable(store, env, key, below) {
        Ok(Some(value)) => match codec::decode(kind, number, value) {
            Some(value) => above.push(Event::Secret { owner, value: Some(value) }),
            None => corrupt(store, owner, above),
        },
        Ok(None) => above.push(Event::Secret { owner, value: None }),
        Err(()) => store.phase = Phase::Stopped,
    }
}

/// A switch waits for the private commits as well as the public ones.
pub(crate) fn settled(store: &Store) -> bool {
    store.secret_state.pending.is_empty()
}
