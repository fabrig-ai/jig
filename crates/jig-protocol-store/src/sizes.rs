//! Checked store admission bounds and phase peaks (protocol/store.md, 5.4, 6 and 13).
use crate::{accounts_v2, codec, component_v3, core_v6, notes_v1, people_v13, tasks_v9};
use jig_domain_core as core;

/// Largest current full turn frame for the admitted transcript byte bound,
/// including metadata and framing (protocol/store.md, 8.1 and 8.6).
/// Reject an out-of-codec bound or arithmetic overflow; never clamp it.
#[must_use]
pub fn frame(domain: &core::Limits, transcript_bytes: u32) -> Option<u32> {
    let mut limits = codec::limits(domain)?.core;
    if transcript_bytes > limits.turn_record_transcript {
        return None;
    }
    limits.turn_record_transcript = transcript_bytes;
    let body = u32::try_from(core_v6::TurnRecord::worst_case_bytes(&limits)?).ok()?;
    body.checked_add(crate::transcripts::frame::WRAPPER_BYTES)
}

/// A restart page can carry a maximum stored row and its census owner.
/// This is the two-row workspace already priced by the store (store.md, 13),
/// independent of an authenticated read's `load_bytes` allowance.
#[must_use]
pub fn restore_bytes(store: &skein_kv::Limits) -> u64 {
    u64::from(store.key)
        .checked_add(u64::from(store.value))
        .expect("two u32 bounds fit u64")
        .checked_mul(2)
        .expect("two rows with u32 bounds fit u64")
}

/// Largest whole manifest: 88 fixed bytes, plus a length and two counts
/// beside each family's name (protocol/store.md, 2, 10.3 and 13).
pub const MANIFEST_BYTES: u32 = {
    let family = 20_u32.checked_add(component_v3::CEILINGS.count_family).expect("manifest family fits u32");
    let counts = component_v3::CEILINGS.manifest_counts.checked_mul(family).expect("manifest counts fit u32");
    88_u32.checked_add(counts).expect("manifest fits u32")
};

/// The application's largest encodings, before the component's key tag.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Application {
    pub version: u32,
    pub key: u32,
    pub value: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sizes {
    pub key: u32,
    pub value: u32,
    pub ops: u32,
    pub commit: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SizeRefusal {
    Key { needs: u32, has: u32 },
    Value { needs: u32, has: u32 },
    Ops { needs: u32, has: u32 },
    Commit { needs: u32, has: u32 },
    Page { needs: u32, has: u32 },
    Queue { needs: u32, has: u32 },
    QueuedBytes { needs: u64, has: u64 },
    Whole { needs: u32, has: u32 },
    Ceiling,
}

/// Refuse a file driver's whole-file bound below the manifest's worst case
/// (protocol/store.md, 13). The shell uses this bound to make its drivers.
pub fn whole(bound: u32) -> Result<(), SizeRefusal> {
    if bound < MANIFEST_BYTES {
        return Err(SizeRefusal::Whole { needs: MANIFEST_BYTES, has: bound });
    }
    Ok(())
}

/// Shared resident state plus the patch phase alone (protocol/store.md, 13).
#[must_use]
pub fn patching_peak(limits: &crate::Limits, sets: &crate::PatchSets) -> Option<u64> {
    let (common, _) = crate::serving_peak(limits)?;
    common.checked_add(crate::patching::phase::worst_case(limits, sets)?)
}

/// Shared resident state plus serving alone, its frame machines among it
/// (protocol/store.md, 8.6 and 13).
#[must_use]
pub fn serving_peak(limits: &crate::Limits, sets: &crate::PatchSets) -> Option<u64> {
    let (common, serving) = crate::serving_peak(limits)?;
    common.checked_add(serving)?.checked_add(crate::transcripts::patches::worst_case(limits, sets)?)
}

/// Whether patch planning or patching owns the current step's retained state
/// (protocol/store.md, 10.2 and 13). Observers sample both sides of a call
/// so its entry and exit transitions are priced as patching too.
#[must_use]
pub fn is_patching(store: &crate::Store) -> bool {
    store.directory.patch.is_some() || crate::directory::patching(store).is_some()
}

pub fn check(
    domain: &core::Limits,
    application: &Application,
    journal: &skein_lib::JournalLimits,
    store: &skein_kv::Limits,
    orphan_erases: u32,
) -> Result<Sizes, SizeRefusal> {
    let sizes = largest(domain, application, journal, orphan_erases, store.budget).ok_or(SizeRefusal::Ceiling)?;
    if sizes.key > store.key {
        return Err(SizeRefusal::Key { needs: sizes.key, has: store.key });
    }
    if sizes.value > store.value {
        return Err(SizeRefusal::Value { needs: sizes.value, has: store.value });
    }
    if sizes.ops > store.ops {
        return Err(SizeRefusal::Ops { needs: sizes.ops, has: store.ops });
    }
    if sizes.commit > store.commit {
        return Err(SizeRefusal::Commit { needs: sizes.commit, has: store.commit });
    }
    // skein-kv pages count the row's key and value payloads.
    let page = sizes.key.checked_add(sizes.value).ok_or(SizeRefusal::Ceiling)?;
    if page > store.page.bytes {
        return Err(SizeRefusal::Page { needs: page, has: store.page.bytes });
    }
    if journal.commits > store.queued {
        return Err(SizeRefusal::Queue { needs: journal.commits, has: store.queued });
    }
    let queued = u64::from(journal.commits).checked_mul(u64::from(sizes.commit)).ok_or(SizeRefusal::Ceiling)?;
    if queued > store.queued_bytes {
        return Err(SizeRefusal::QueuedBytes { needs: queued, has: store.queued_bytes });
    }
    Ok(sizes)
}

fn largest(
    domain: &core::Limits,
    application: &Application,
    journal: &skein_lib::JournalLimits,
    orphan_erases: u32,
    budget: u64,
) -> Option<Sizes> {
    let limits = codec::limits(domain)?;
    // A resources line has 32 fixed bytes, three per empty segment, and at
    // most two per payload byte (escaped zero). Notes bounds both count and sum.
    // The largest fixed key is a tasks update: tags plus five widened numbers.
    let key = domain.notes.pattern_bytes.checked_mul(5)?.checked_add(32)?.max(44).max(application.key.checked_add(1)?);
    // Tasks price additions and kept lines at dependencies, circle holds at circle;
    // people prices the same refusal names through codec::limits (store.md, 5.1).
    // Transcript bodies are frames in files; this sizes map values only.
    let mut map_core = limits.core;
    map_core.turn_record_transcript = 0;
    let value = u32::try_from(
        core_v6::Row::worst_case_bytes(&map_core)?
            .max(tasks_v9::Row::worst_case_bytes(&limits.tasks)?)
            .max(people_v13::Row::worst_case_bytes(&limits.people)?)
            .max(notes_v1::Row::worst_case_bytes(&limits.notes)?)
            .max(component_v3::worst_case_bytes(&component_v3::CEILINGS)?)
            .max(accounts_v2::worst_case_bytes(&accounts_v2::CEILINGS)?)
            .max(
                core::settings::revision_worst_case(domain)?.checked_add(16)?.checked_add(
                    u64::from(jig_domain_tasks::MODELS)
                        .checked_mul(u64::from(core::MODEL_NAME_BYTES).checked_add(4)?.checked_add(4)?.checked_add(
                            u64::from(core::EFFORTS).checked_mul(u64::from(core::EFFORT_BYTES).checked_add(4)?)?,
                        )?)?
                        .checked_add(4)?,
                )?,
            )
            .max(u64::from(application.value)),
    )
    .ok()?;
    // Each journal write can produce two turn places; the first commit also
    // inserts Format, Layouts and the installation's identifier. Each put
    // costs a tag and two u32 lengths; a skein-kv frame costs 22 header bytes
    // and four checksum bytes.
    let ops = crate::commits::most_ops(journal.writes, orphan_erases)?;
    let commit = u32::try_from(
        budget.checked_add(u64::from(key).checked_add(9)?.checked_mul(u64::from(ops))?)?.checked_add(26)?,
    )
    .ok()?;
    Some(Sizes { key, value, ops, commit })
}
