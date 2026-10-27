//! Single-turn admission reads fixed metadata before allocating opaque payloads.
use super::{Below, Event, Party, Queue, Read, Store, Token, TranscriptRead, TurnRecord, domain, fire, prepare, reads};
pub(crate) fn start(
    store: &mut Store,
    owner: Token,
    request: TranscriptRead,
    party: Party,
    above: &mut Queue<Event>,
    below: &mut Below,
) -> Result<Option<jig_domain_core::reads::Loaded>, reads::Failure> {
    assert!(!store.transcripts.reads.contains_key(&owner), "one transcript read per owner");
    if store.transcripts.reads.len() == store.limits.reads {
        return Ok(Some(jig_domain_core::reads::Loaded::Busy));
    }
    let task = match request {
        TranscriptRead::Window { task, .. } | TranscriptRead::Turn { task, .. } | TranscriptRead::Page { task, .. } => {
            task
        }
    };
    let Some(mut read) = prepare(store, owner, request, 1, u64::from(store.limits.store.page.bytes)) else {
        return reads::ready(reads::finish(store, task, party, None));
    };
    read.party = Some(party);
    store.transcripts.reads.insert(owner, read).expect("party read slot checked at admission");
    fire(store, above, below);
    Ok(None)
}

pub(super) fn header(store: &mut Store, read: &mut Read, turn: &TurnRecord, size: u32) -> Result<bool, ()> {
    let party = read.party.as_ref().ok_or(())?;
    let (attempt, number) = party.turn.ok_or(())?;
    if turn.task != read.task || turn.attempt != attempt {
        return Err(());
    }
    if turn.turn > number {
        return Ok(true);
    }
    if turn.turn != number {
        return Ok(false);
    }
    let probe = Party { activity: None, bytes: u64::MAX, turn: None, problem: None, cut: None, frame_bytes: None };
    let result = match reads::finish(store, read.task, probe, Some(turn.clone())) {
        Ok(result) => result,
        Err(error @ (reads::Failure::Cut | reads::Failure::Joins)) => {
            read.party.as_mut().expect("party").cut = Some(error);
            return Ok(true);
        }
        Err(reads::Failure::Corrupt) => return Err(()),
    };
    let domain::reads::Loaded::Page { rows, .. } = result else { return Err(()) };
    let Some(row) = rows.first() else { return Err(()) };
    let cost = row.owned_bytes().ok_or(())?.checked_add(u64::from(size)).ok_or(())?;
    if cost > party.bytes {
        read.party.as_mut().expect("party").problem =
            Some(domain::reads::TurnFailure::TooLarge { bytes: u64::from(size) });
        return Ok(true);
    }
    Ok(false)
}

pub(super) fn finish(store: &mut Store, mut read: Read, failed: bool) -> Result<domain::reads::Loaded, reads::Failure> {
    let mut party = read.party.take().expect("party read");
    if failed && party.turn.is_some() {
        party.problem = Some(domain::reads::TurnFailure::Malformed);
    }
    let turn = read.turns.into_boxed().into_iter().next();
    reads::finish(store, read.task, party, turn)
}

pub(super) fn single(read: &Read) -> bool {
    match &read.party {
        Some(party) => party.turn.is_some(),
        None => false,
    }
}
