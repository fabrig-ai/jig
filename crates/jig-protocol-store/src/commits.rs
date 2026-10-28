//! Root commits, numbered with skein-kv's (protocol/store.md, 6 and 11.2).
use crate::{Event, Store, Write, codec, format, keys};
use alloc::boxed::Box;
use jig_domain_core::{CoreRecord, Record};
use skein_lib::{List, Map, Queue, Token};

pub(crate) fn submit(
    store: &mut Store,
    env: &skein_lib::Env<crate::Limits>,
    number: u64,
    ops: Box<[skein_kv::Op]>,
    above: &mut Queue<Event>,
    below: &mut crate::Below,
) {
    let kv_env = crate::directory::env(store, env, false);
    let mut events = Queue::with_capacity(skein_kv::MAX_OUT.above);
    skein_kv::down(
        &mut store.stores.store,
        &kv_env,
        skein_kv::Request::Commit { owner: Token::new(number), ops },
        &mut events,
        &mut below.store,
    );
    crate::directory::events_up(store, false, &mut events, above, below);
}
#[derive(Debug)]
pub(crate) enum Refusal {
    Failed,
    FrameTooLarge { task: u64, bytes: u32, limit: u32 },
}

pub(crate) fn operations(
    store: &mut Store,
    number: u64,
    writes: Box<[Write]>,
    format_due: bool,
) -> Result<crate::transcripts::Pending, Refusal> {
    if writes.len() > usize::try_from(store.limits.journal.writes).ok().ok_or(Refusal::Failed)? {
        return Err(Refusal::Failed);
    }
    let capacity = most_ops(store.limits.journal.writes, store.limits.orphan_erases).ok_or(Refusal::Failed)?;
    let mut ops = List::with_capacity(capacity);
    let mut frames = Queue::uncapped();
    let mut drops = Queue::uncapped();
    let mut bytes = 26;
    let mut deployment = None;
    let mut wires = false;
    for write in writes {
        match write {
            Write::Wires => {
                if crate::patching::switch::pending(store) {
                    return Err(Refusal::Failed);
                }
                wires = true;
                ops.push(operation(store, Write::Wires).ok_or(Refusal::Failed)?).ok().ok_or(Refusal::Failed)?;
            }
            Write::DropTranscript { task } => {
                if crate::patching::switch::pending(store) {
                    return Err(Refusal::Failed);
                }
                crate::transcripts::drop::queue(store, task, number).ok_or(Refusal::Failed)?;
                ops.push(operation(store, Write::DropTranscript { task }).ok_or(Refusal::Failed)?)
                    .ok()
                    .ok_or(Refusal::Failed)?;
                drops.push(task);
            }
            Write::Save(Record::Core(CoreRecord::Turn(turn))) => {
                if crate::patching::switch::pending(store) {
                    return Err(Refusal::Failed);
                }
                let body = crate::transcripts::frame::encode(&turn, &store.codec).ok_or(Refusal::Failed)?;
                let bytes = u32::try_from(body.len()).ok().ok_or(Refusal::Failed)?;
                let limit = store.limits.store.page.bytes;
                if bytes > limit {
                    return Err(Refusal::FrameTooLarge { task: turn.task, bytes, limit });
                }
                let offset = crate::transcripts::place(store, number, &turn, u64::from(bytes), &mut ops)
                    .ok_or(Refusal::Failed)?;
                frames.push(crate::transcripts::Frame { task: turn.task, offset, bytes: body });
            }
            write @ (Write::Save(
                Record::Core(
                    CoreRecord::Deployment(_)
                    | CoreRecord::Settings(_)
                    | CoreRecord::Projection(_)
                    | CoreRecord::Call(_)
                    | CoreRecord::RunProof(_)
                    | CoreRecord::Terminal(_)
                    | CoreRecord::ProposalDecision(_)
                    | CoreRecord::EscalationDecision(_),
                )
                | Record::Tasks(_)
                | Record::People(_)
                | Record::Notes(_)
                | Record::Accounts(_),
            )
            | Write::CopySettingsRevision(_)
            | Write::Erase(_)
            | Write::Put { .. }
            | Write::Remove { .. }) => {
                if let Some(id) = header_id(&write) {
                    deployment = Some(id);
                }
                ops.push(operation(store, write).ok_or(Refusal::Failed)?).ok().ok_or(Refusal::Failed)?;
            }
        }
    }
    if format_due {
        let id = deployment.expect("the first root commit saves its deployment header");
        let mut row = format::current(store.application.version, id);
        row.levels = format::recorded(crate::patching::plan::binary(&store.patch_sets));
        ops.push(skein_kv::Op::Put {
            key: keys::encode_own(&keys::Own::Format, store.limits.store.key).ok_or(Refusal::Failed)?,
            value: row.encode().ok().ok_or(Refusal::Failed)?,
        })
        .ok()
        .ok_or(Refusal::Failed)?;
        ops.push(skein_kv::Op::Put {
            key: keys::encode_own(&keys::Own::Layouts, store.limits.store.key).ok_or(Refusal::Failed)?,
            value: store.layouts.encode(),
        })
        .ok()
        .ok_or(Refusal::Failed)?;
        ops.push(skein_kv::Op::Put {
            key: keys::installation(store.limits.store.key).ok_or(Refusal::Failed)?,
            value: codec::accounts::Installation { id: store.installation }.encode(),
        })
        .ok()
        .ok_or(Refusal::Failed)?;
    }
    crate::transcripts::drop::erases(store, &mut ops).ok_or(Refusal::Failed)?;
    let mut bounded = List::with_capacity(capacity);
    for op in unique(ops.into_boxed(), capacity).ok_or(Refusal::Failed)? {
        append(&mut bounded, op, &mut bytes, store.limits.store.commit).ok_or(Refusal::Failed)?;
    }
    store.mark.accept(wires, number);
    Ok(crate::transcripts::Pending { number, frames, ops: bounded.into_boxed(), drops })
}
fn operation(store: &mut Store, write: Write) -> Option<skein_kv::Op> {
    let limit = store.limits.store.key;
    match write {
        Write::Wires => Some(skein_kv::Op::Put {
            key: keys::encode_own(&keys::Own::Wires, limit)?,
            value: codec::component::wires(store.wires),
        }),
        Write::CopySettingsRevision(number) => copy_settings(store, number),
        Write::Save(record) => Some(skein_kv::Op::Put {
            key: keys::encode(&record.key(), limit)?,
            value: codec::encode(&record, &store.codec, &store.names)?,
        }),
        Write::Erase(key) => Some(skein_kv::Op::Erase { key: keys::encode(&key, limit)? }),
        Write::Put { key, value } => Some(skein_kv::Op::Put { key: keys::application(&key, limit)?, value }),
        Write::Remove { key } => Some(skein_kv::Op::Erase { key: keys::application(&key, limit)? }),
        Write::DropTranscript { task } => {
            Some(skein_kv::Op::Erase { key: keys::encode_own(&keys::Own::Transcript { task }, limit)? })
        }
    }
}

fn header_id(write: &Write) -> Option<[u8; 16]> {
    match write {
        Write::Save(Record::Core(CoreRecord::Deployment(header))) => Some(header.id),
        Write::Save(
            Record::Core(
                CoreRecord::Settings(_)
                | CoreRecord::Projection(_)
                | CoreRecord::Call(_)
                | CoreRecord::RunProof(_)
                | CoreRecord::Turn(_)
                | CoreRecord::Terminal(_)
                | CoreRecord::ProposalDecision(_)
                | CoreRecord::EscalationDecision(_),
            )
            | Record::Tasks(_)
            | Record::People(_)
            | Record::Notes(_)
            | Record::Accounts(_),
        )
        | Write::CopySettingsRevision(_)
        | Write::Wires
        | Write::Erase(_)
        | Write::Put { .. }
        | Write::Remove { .. }
        | Write::DropTranscript { .. } => None,
    }
}

fn append(ops: &mut List<skein_kv::Op>, op: skein_kv::Op, bytes: &mut u64, limit: u32) -> Option<()> {
    let key = u64::try_from(op.key().len()).ok()?;
    let size = match &op {
        skein_kv::Op::Put { key: _, value } => key.checked_add(u64::try_from(value.len()).ok()?)?.checked_add(9)?,
        skein_kv::Op::Erase { key: _ } => key.checked_add(5)?,
    };
    let total = bytes.checked_add(size)?;
    if total > u64::from(limit) {
        return None;
    }
    ops.push(op).ok()?;
    *bytes = total;
    Some(())
}

/// Last writes replace their first slot; encoded keys are the map's identity.
fn unique(ops: Box<[skein_kv::Op]>, capacity: u32) -> Option<Box<[skein_kv::Op]>> {
    let mut positions = Map::with_capacity(capacity);
    let mut kept = List::with_capacity(capacity);
    for op in ops {
        if let Some(position) = positions.get(op.key()).copied() {
            *kept.get_mut(position).expect("each indexed operation has its first slot") = op;
        } else {
            positions.insert(skein_lib::bytes::copy_of(op.key()), kept.len()).ok()?;
            kept.push(op).ok()?;
        }
    }
    Some(kept.into_boxed())
}

/// A commit's most operations: two turn places per write, the first commit's
/// own rows (Format, Layouts and the installation's identifier), and the
/// orphans' erases it carries.
pub(crate) fn most_ops(writes: u32, orphan_erases: u32) -> Option<u32> {
    writes.checked_mul(2)?.checked_add(3)?.checked_add(orphan_erases)
}

fn copy_settings(store: &mut Store, number: u32) -> Option<skein_kv::Op> {
    use jig_domain_core::{CoreKey, Key, settings};
    let held = Key::Core(CoreKey::Settings(settings::Key::Held(number)));
    let history = Key::Core(CoreKey::Settings(settings::Key::History(number)));
    let mut above = Queue::with_capacity(skein_kv::MAX_OUT.above);
    let mut below = Queue::with_capacity(skein_kv::MAX_OUT.below);
    skein_kv::down(
        &mut store.stores.store,
        &crate::memory_env(store.limits.store),
        skein_kv::Request::Get { owner: Token::new(0), key: keys::encode(&held, store.limits.store.key)? },
        &mut above,
        &mut below,
    );
    assert!(below.is_empty(), "settings history copies read the in-memory map");
    let Some(skein_kv::Event::Got { value: Some(value), .. }) = above.pop() else { return None };
    Some(skein_kv::Op::Put { key: keys::encode(&history, store.limits.store.key)?, value })
}
