//! The format row and header checked before restart (protocol/store.md, 5.3 and 10).
use crate::patching::{machines, phase::Pending, plan};
use crate::{Event, Family, Older, OpenRefusal, Phase, Store, codec, format, keys};
use alloc::boxed::Box;
use jig_domain_core::{CoreKey, CoreRecord, Key, Record};
use skein_lib::{Queue, Token};

pub(crate) fn open(store: &mut Store, owner: Token, above: &mut Queue<Event>, below: &mut crate::Below) {
    if store.phase != Phase::Closed {
        refuse(store, owner, OpenRefusal::Failed, above);
        return;
    }
    let format_key = keys::encode_own(&keys::Own::Format, store.limits.store.key).expect("format key fits");
    let header_key = keys::encode(&Key::Core(CoreKey::Deployment), store.limits.store.key).expect("header key fits");
    let Ok(found) = get(store, owner, format_key) else {
        refuse(store, owner, OpenRefusal::Failed, above);
        return;
    };
    let Ok(header) = get(store, owner, header_key) else {
        refuse(store, owner, OpenRefusal::Failed, above);
        return;
    };
    let last = store.stores.store.last();
    let Some(found) = found else {
        if header.is_some() {
            refuse(
                store,
                owner,
                OpenRefusal::Format {
                    family: Family::Component,
                    found: 0,
                    wanted: u32::from(format::current(store.application.version, [0; 16]).component),
                },
                above,
            );
        } else if last == 0 {
            ready(store, owner, last, true, above, below);
        } else {
            refuse(store, owner, OpenRefusal::Numbers { store: last, header: 0 }, above);
        }
        return;
    };
    let binary = plan::binary(&store.patch_sets);
    let Some(found) = decode(store, owner, &found, binary.jig, above) else { return };
    let found_levels = format::levels(found.levels);
    let compared = match format::compare(found_levels, binary) {
        Ok(compared) => compared,
        Err(why) => return refuse(store, owner, why, above),
    };
    // Refuse another family's version before decoding its header.
    let wanted = format::current(store.application.version, found.deployment);
    if let Err(why) = format::check_versions(&found, &wanted, compared) {
        refuse(store, owner, why, above);
        return;
    }
    if let Err(why) = check_layouts(store, owner) {
        refuse(store, owner, why, above);
        return;
    }
    let Some(header) = header else {
        refuse(store, owner, OpenRefusal::Numbers { store: last, header: 0 }, above);
        return;
    };
    let Some(header) = codec::decode(&Key::Core(CoreKey::Deployment), &header, &store.codec, &store.names) else {
        corrupt(store, owner, above);
        return;
    };
    match header {
        Record::Core(CoreRecord::Deployment(header)) => {
            let wanted = format::current(store.application.version, header.id);
            if let Err(why) = format::check_versions(&found, &wanted, compared) {
                refuse(store, owner, why, above);
            } else if header.commits != last {
                refuse(store, owner, OpenRefusal::Numbers { store: last, header: header.commits }, above);
            } else {
                if compared == format::Compared::Behind {
                    match pending(store, found_levels, binary) {
                        Ok(pending) => store.directory.patch = Some(Box::new(pending)),
                        Err(why) => return refuse(store, owner, why, above),
                    }
                }
                restore_installation(store, owner, last, above, below);
            }
        }
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
        | Record::Accounts(_) => corrupt(store, owner, above),
    }
}

fn pending(store: &Store, found: plan::Levels, binary: plan::Levels) -> Result<Pending, OpenRefusal> {
    let plan = match plan::plan(&store.patch_sets, &found, &binary) {
        Ok(plan) => plan,
        Err(why) => return Err(OpenRefusal::Set { owner: why.owner, patch: why.patch }),
    };
    let machines = match machines::make(&store.patch_sets, &plan, &found, machines::Rooms::of(&store.limits)) {
        Ok(machines) => machines,
        Err(why) => return Err(OpenRefusal::Set { owner: why.owner, patch: why.patch }),
    };
    Ok(Pending { plan, machines, found })
}

fn check_layouts(store: &mut Store, owner: Token) -> Result<(), OpenRefusal> {
    let key = keys::encode_own(&keys::Own::Layouts, store.limits.store.key).expect("layouts key fits");
    let bytes = get(store, owner, key)?.ok_or(OpenRefusal::OlderBuild { why: Older::Unrecorded })?;
    let Ok(found) = codec::component::Layouts::decode(&bytes) else {
        return Err(OpenRefusal::OlderBuild { why: Older::Layout(Family::Component) });
    };
    format::check_layouts(&found, &store.layouts)
}

/// The identifier kept by the first commit replaces the drawn one (4.4); a
/// checked store without it is corrupt.
fn restore_installation(
    store: &mut Store,
    owner: Token,
    last: u64,
    above: &mut Queue<Event>,
    below: &mut crate::Below,
) {
    let key = keys::installation(store.limits.store.key).expect("installation key fits");
    let Ok(found) = get(store, owner, key) else {
        refuse(store, owner, OpenRefusal::Failed, above);
        return;
    };
    let Some(bytes) = found else {
        corrupt(store, owner, above);
        return;
    };
    let Ok(kept) = codec::accounts::Installation::decode(&bytes) else {
        corrupt(store, owner, above);
        return;
    };
    store.installation = kept.id;
    ready(store, owner, last, false, above, below);
}

pub(crate) fn get(store: &mut Store, owner: Token, key: Box<[u8]>) -> Result<Option<Box<[u8]>>, OpenRefusal> {
    let mut above = Queue::with_capacity(skein_kv::MAX_OUT.above);
    let mut below = Queue::with_capacity(skein_kv::MAX_OUT.below);
    skein_kv::down(
        &mut store.stores.store,
        &crate::memory_env(store.limits.store),
        skein_kv::Request::Get { owner, key },
        &mut above,
        &mut below,
    );
    assert!(below.is_empty(), "a memory get issues no file operation");
    match above.pop().ok_or(OpenRefusal::Failed)? {
        skein_kv::Event::Got { owner: _, value } => Ok(value),
        skein_kv::Event::Opened { .. }
        | skein_kv::Event::Committed { .. }
        | skein_kv::Event::Refused { .. }
        | skein_kv::Event::Failed { .. }
        | skein_kv::Event::Loaded { .. }
        | skein_kv::Event::Closed { .. }
        | skein_kv::Event::OpenFailed { .. }
        | skein_kv::Event::Recovered { .. }
        | skein_kv::Event::RecoveryFailed { .. } => Err(OpenRefusal::Failed),
    }
}

fn ready(
    store: &mut Store,
    owner: Token,
    last: u64,
    format_due: bool,
    above: &mut Queue<Event>,
    below: &mut crate::Below,
) {
    let Some(next) = last.checked_add(1) else {
        refuse(store, owner, OpenRefusal::Failed, above);
        return;
    };
    if let Err(why) = plan::bounds(&store.patch_sets) {
        refuse(store, owner, OpenRefusal::Set { owner: why.owner, patch: why.patch }, above);
        return;
    }
    // A behind open crosses Ready on its way to the copy. Its map machines
    // already exist; the copy's serving machines are made after its check.
    if store.directory.patch.is_none() {
        store.transcripts.machines = match crate::transcripts::patches::make(&store.patch_sets, &store.limits) {
            Ok(machines) => machines,
            Err(why) => {
                refuse(store, owner, OpenRefusal::Set { owner: why.owner, patch: 0 }, above);
                return;
            }
        };
    }
    store.phase = Phase::Ready { next, format_due };
    store.directory.owner = owner;
    crate::directory::checked(store, above, below);
}

pub(crate) fn refuse(store: &mut Store, owner: Token, why: OpenRefusal, above: &mut Queue<Event>) {
    store.phase = Phase::Stopped;
    above.push(Event::OpenRefused { owner, why });
}

fn corrupt(store: &mut Store, owner: Token, above: &mut Queue<Event>) {
    store.phase = Phase::Stopped;
    above.push(Event::Corrupt { owner });
}

/// A later component's levels are unread; a same-version row that no longer
/// decodes is an older layout, including L's row before levels were added.
fn decode(
    store: &mut Store,
    owner: Token,
    bytes: &[u8],
    jig: Option<u32>,
    above: &mut Queue<Event>,
) -> Option<codec::component::Format> {
    let wanted_version = format::current(store.application.version, [0; 16]).component;
    if let Some(version) = codec::component::Format::version(bytes)
        && version > wanted_version
    {
        refuse(store, owner, OpenRefusal::Ahead { owner: crate::Owner::Jig, found: None, wanted: jig }, above);
        return None;
    }
    let Ok(found) = codec::component::Format::decode(bytes) else {
        // Another version of the component's own rows says which (5.3).
        let wanted = format::current(store.application.version, [0; 16]).component;
        match codec::component::Format::version(bytes) {
            Some(version) if version != wanted => refuse(
                store,
                owner,
                OpenRefusal::Format { family: Family::Component, found: u32::from(version), wanted: u32::from(wanted) },
                above,
            ),
            Some(_) => refuse(store, owner, OpenRefusal::OlderBuild { why: Older::Layout(Family::Component) }, above),
            None => corrupt(store, owner, above),
        }
        return None;
    };
    Some(found)
}
