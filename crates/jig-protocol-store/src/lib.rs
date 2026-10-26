//! The engine's store component (protocol/store.md, sections 2 and 3).
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
pub mod boundary;
pub mod codec;
mod commits;
mod directory;
pub mod format;
mod generations;
mod gets;
pub mod historical;
pub mod intervals;
mod loads;
mod open;
mod pages;
mod patching;
pub use patching::Counts;
pub use patching::sets::{JIG, KeptFamily, KeptMap, Keys, Owner, PatchSet, PatchSets};
mod reads;
mod restore;
mod secrets;
pub mod sizes;
pub mod transcripts;
use alloc::boxed::Box;
use skein_lib::{Env, Queue, Time, Wall};

/// The store component's bounds (protocol/store.md, section 13).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    pub core: jig_domain_core::Limits,
    pub store: skein_kv::Limits,
    pub secrets: skein_kv::Limits,
    /// The clients component's carried values at once (store.md, section 9).
    pub held: u32,
    pub journal: skein_lib::JournalLimits,
    /// Point gets available to a party page's joins (store.md, 13).
    pub joins: u32,
    pub queue: u32,
    pub files: u32,
    pub reads: u32,
    pub orphan_erases: u32,
}

/// Stores made by the caller, kept across a world's crash (store.md, 14).
#[derive(Debug)]
pub struct Stores {
    pub store: skein_kv::Store,
    pub secrets: skein_kv::Store,
}

/// The component's open and stopped states (store.md, 6 and 10).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Closed,
    Ready { next: u64, format_due: bool },
    Stopped,
}

#[derive(Debug)]
pub struct Store {
    limits: Limits,
    application: sizes::Application,
    codec: codec::Limits,
    layouts: codec::component::Layouts,
    /// The configured models' names, which the people family keeps a role's models by.
    names: Box<[jig_domain_core::settings::ModelName]>,
    stores: Stores,
    phase: Phase,
    directory: directory::Directory,
    secret_state: secrets::State,
    transcripts: transcripts::State,
    /// The installation's identifier (store.md 4.4): drawn by the shell and
    /// kept by an empty store's first commit, or restored from the store.
    installation: [u8; 16],
    patch_sets: PatchSets,
}

/// Proof that the store's three file drivers were seeded: io's `Store` names
/// its temporaries from a driver's randomness and refuses unseeded (store.md 10.1).
/// It proves three drivers were seeded, not that they are the three the shell
/// serves the store with.
#[derive(Clone, Copy, Debug)]
pub struct Seeded(());

/// Seeds each store driver from `seed` mixed with its index.
#[must_use]
pub fn seed(drivers: [&mut skein_io::file_layer::FileIo; 3], seed: u64) -> Seeded {
    for (index, driver) in drivers.into_iter().enumerate() {
        let index = u64::try_from(index).expect("three driver indices");
        driver.seed_randomness(
            seed ^ 0x9e37_79b9_7f4a_7c15_u64.wrapping_mul(index.checked_add(1).expect("three drivers")),
        );
    }
    Seeded(())
}

impl Store {
    /// A component over open stores; `models` are the configuration's, in its order.
    #[must_use]
    pub fn over(
        limits: &Limits,
        application: sizes::Application,
        models: &[jig_domain_core::Model],
        stores: Stores,
    ) -> Store {
        assert!(limits.store.is_usable() && limits.secrets.is_usable(), "both stores' limits are usable");
        assert!(
            limits.queue > 0 && limits.queue <= limits.store.queued && limits.files > 0,
            "transcript bounds fit the store queue"
        );
        assert!(
            limits.reads >= limits.core.load_slots.checked_add(limits.core.tasks.tasks).expect("bounded readers"),
            "transcript readers fit party loads and all live task preparations"
        );
        assert!(limits.orphan_erases > 0, "orphan cleanup makes progress");
        let codec = codec::limits(&limits.core).expect("the domain limits fit their families' ceilings");
        Store {
            limits: *limits,
            application,
            codec,
            layouts: format::layouts(),
            names: codec::names(models),
            stores,
            phase: Phase::Closed,
            directory: directory::Directory::new(),
            secret_state: secrets::State::new(limits.secrets.queued, limits.held),
            transcripts: transcripts::State::new(limits),
            installation: [0; 16],
            patch_sets: PatchSets::NONE,
        }
    }

    #[must_use]
    pub fn on_directory(
        limits: &Limits,
        application: sizes::Application,
        models: &[jig_domain_core::Model],
        _seeded: Seeded,
    ) -> Store {
        let stores =
            Stores { store: skein_kv::Store::new(&limits.store), secrets: skein_kv::Store::new(&limits.secrets) };
        let mut store = Store::over(limits, application, models, stores);
        store.directory.disk = true;
        store
    }

    /// Supplies the owners' static patch data (store.md 10.3). Price these
    /// sets with `worst_case_with_patches` when checking startup room.
    pub fn patch_sets(&mut self, sets: PatchSets) {
        self.patch_sets = sets;
    }

    #[must_use]
    pub fn into_stores(self) -> Stores {
        self.stores
    }
}

/// Most terminals from a component step, including a store batch.
pub const MAX_OUT_DOWN: u32 = skein_kv::MAX_OUT.above.saturating_mul(2);

/// Owned stores, component state and bounded temporary rows and operations.
#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    worst_case_with_patches(limits, &PatchSets::NONE)
}

/// Includes the static sets supplied through `Store::patch_sets`. The ordinary
/// `worst_case` prices `PatchSets::NONE`: limits alone cannot bound an
/// arbitrary number of kept schemas. Record bytes also bound nodes (at least one wire byte each).
#[must_use]
pub fn worst_case_with_patches(limits: &Limits, sets: &PatchSets) -> Option<u64> {
    let (common, serving) = serving_peak(limits)?;
    let serving = serving.checked_add(transcripts::patches::worst_case(limits, sets)?)?;
    let patching = patching::phase::worst_case(limits, sets)?;
    common.checked_add(serving.max(patching))
}

/// Shared resident state and the additional serving peak (store.md 13).
fn serving_peak(limits: &Limits) -> Option<(u64, u64)> {
    let state = u64::try_from(size_of::<Store>()).ok()?;
    let rows = u64::from(limits.store.value).checked_add(u64::from(limits.store.key))?.checked_mul(2)?;
    let decoded = decoded_worst_case(limits)?;
    // One byte per encoded row bounds carrier counts. Count wire values and
    // their translated records together, including the census's fetched task.
    let page = u64::from(limits.store.page.bytes)
        .checked_mul(decoded.checked_add(u64::from(limits.store.key))?)?
        .checked_add(u64::from(limits.store.page.bytes))?
        .checked_add(skein_lib::List::<pages::WalkRow>::worst_case(limits.store.page.bytes)?)?
        .checked_add(skein_lib::List::<Row>::worst_case(limits.store.page.bytes)?)?;
    let common = skein_kv::worst_case(&limits.store)?
        .checked_add(skein_kv::worst_case(&limits.secrets)?)?
        .checked_add(state)?
        .checked_add(secrets::worst_case(limits)?)?
        .checked_add(transcripts::worst_case(limits)?)?
        .checked_add(generations::worst_case()?)?
        .checked_add(names_worst_case()?)?;
    let serving = rows
        .checked_add(page)?
        .checked_add(decoded.checked_mul(u64::from(limits.joins))?)?
        .checked_add(skein_lib::List::<Box<jig_domain_tasks::TaskRecord>>::worst_case(limits.joins)?)?
        .checked_add(skein_lib::List::<u64>::worst_case(limits.core.tasks.depth)?)?
        .checked_add(skein_lib::List::<jig_domain_tasks::Edge>::worst_case(limits.core.tasks.tree_tasks)?)?
        .checked_add(skein_lib::Set::<u64>::worst_case(limits.joins)?)?
        .checked_add(u64::from(limits.store.commit))?
        .checked_add(skein_lib::List::<skein_kv::Op>::worst_case(limits.journal.writes.checked_add(1)?)?)?
        .checked_add(Queue::<skein_kv::Event>::worst_case(skein_kv::MAX_OUT.above)?)?
        .checked_add(Queue::<skein_io::file::Request>::worst_case(skein_kv::MAX_OUT.below)?)?
        .checked_add(Queue::<Event>::worst_case(MAX_OUT_DOWN)?)?
        .checked_add(question_restore_worst_case(limits)?)?
        .checked_add(commits::worst_case(limits)?)?;
    Some((common, serving))
}

// Two decoded records, including their wire values and the census's joined task.
pub(crate) fn decoded_worst_case(limits: &Limits) -> Option<u64> {
    let codec = codec::limits(&limits.core)?;
    core_v6::Row::worst_case_heap(&codec.core)?
        .checked_add(tasks_v9::Row::worst_case_heap(&codec.tasks)?)?
        .checked_add(people_v13::Row::worst_case_heap(&codec.people)?)?
        .checked_add(notes_v1::Row::worst_case_heap(&codec.notes)?)?
        .checked_add(accounts_v2::Row::worst_case_heap(&accounts_v2::CEILINGS)?)?
        .checked_add(settings_v1::Revision::worst_case_heap(&codec.settings)?)?
        .checked_add(jig_domain_core::settings::revision_worst_case(&limits.core)?)?
        .checked_add(u64::try_from(size_of::<jig_domain_tasks::TaskRecord>()).ok()?)?
        .checked_mul(2)
}

// The census's exact question keys become point ranges beside the combined
// restore intervals. Both arrays and their endpoints may coexist.
fn question_restore_worst_case(limits: &Limits) -> Option<u64> {
    let count = limits.core.tasks.tasks.checked_mul(limits.core.tasks.inbox_messages)?;
    skein_lib::List::<skein_kv::Range>::worst_case(count)?
        .checked_mul(2)?
        .checked_add(u64::from(count).checked_mul(u64::from(limits.store.key).checked_add(1)?)?.checked_mul(4)?)
}

/// At most one configured model per `Models` bit, each name within its bound.
fn names_worst_case() -> Option<u64> {
    jig_domain_core::settings::model_names_worst_case()?
        .checked_add(skein_lib::List::<Box<[u8]>>::worst_case(u64::BITS)?)?
        .checked_add(u64::from(u64::BITS).checked_mul(u64::from(jig_domain_core::MODEL_NAME_BYTES))?)
}

/// The memory component's requests (store.md, section 10).
pub fn down(store: &mut Store, env: &Env<Limits>, request: Request, above: &mut Queue<Event>, below: &mut Below) {
    if store.phase == Phase::Stopped {
        return;
    }
    match request {
        Request::Open { owner, root, installation } => {
            directory::open(store, env, owner, root, installation, above, below);
        }
        Request::Site { owner, patcher, result } => match store.phase {
            Phase::Ready { .. } => transcripts::read::site(store, env, owner, patcher, result, above, below),
            Phase::Closed | Phase::Stopped => patching::phase::site(store, env, owner, patcher, result, above, below),
        },
        Request::Commit { number, writes } => transcripts::write::begin(store, env, number, writes, above, below),
        Request::Restore { owner, range, after, rows, bytes } => {
            restore::restore(store, env, owner, restore::Page { range, after, rows, bytes }, above, below);
        }
        Request::Load { owner, range, after, rows, bytes } => {
            loads::load(store, owner, range, after, rows, bytes, above);
        }
        Request::Get { owner, key } => gets::get(store, owner, key, above),
        Request::Read { owner, range } => reads::read(store, owner, range, above, below),
        Request::Secrets { owner, request } => secrets::secrets(store, env, owner, request, above, below),
        Request::Transcript { owner, read, rows, bytes } => {
            transcripts::read::transcript(store, owner, read, rows, bytes, above, below);
        }
    }
}

/// File terminals routed through their independent drivers.
pub fn up(store: &mut Store, env: &Env<Limits>, io: Io, above: &mut Queue<Event>, below: &mut Below) {
    directory::up(store, env, io, above, below);
    if store.phase != Phase::Stopped {
        transcripts::read::fire(store, above, below);
        transcripts::write::fire(store, env, above, below);
    }
}

/// Bounded background store work and the orphan sweep.
pub fn fire(store: &mut Store, env: &Env<Limits>, above: &mut Queue<Event>, below: &mut Below) {
    if store.phase != Phase::Stopped {
        directory::fire(store, env, above, below);
        secrets::sweep(store, env, above, below);
        transcripts::read::fire(store, above, below);
        transcripts::write::fire(store, env, above, below);
    }
}

fn memory_env(limits: skein_kv::Limits) -> Env<skein_kv::Limits> {
    // Memory gets and commits use no clock or file operation.
    Env { now: Time::ZERO, wall: Wall::EPOCH, limits }
}

#[rustfmt::skip]
#[path = "generated/accounts_v2.rs"]
pub mod accounts_v2;
#[rustfmt::skip]
#[path = "generated/component_v2.rs"]
pub mod component_v2;
#[rustfmt::skip]
#[path = "generated/notes_v1.rs"]
pub mod notes_v1;
#[rustfmt::skip]
#[expect(clippy::similar_names, reason = "generated StoredActed retains its domain fields act and at")]
#[path = "generated/people_v13.rs"]
pub mod people_v13;
#[rustfmt::skip]
#[path = "generated/tasks_v9.rs"]
pub mod tasks_v9;
#[rustfmt::skip]
#[path = "generated/core_v6.rs"]
pub mod core_v6;
pub mod keys;
pub use boundary::{
    Arrived, Below, CARRIED, CarriedKind, Check, Cursor, Event, Family, Generations, Get, HELD, Io, Load, Older,
    OpenRefusal, Request, Restore, Root, Row, SecretOwner, SecretRow, SecretValue, Secrets, TranscriptRead, Write,
};

#[cfg(test)]
mod tests;

#[rustfmt::skip]
#[path = "generated/settings_v1.rs"]
pub mod settings_v1;
#[rustfmt::skip]
#[path = "generated/component_v3.rs"]
pub mod component_v3;
#[rustfmt::skip]
#[path = "generated/layouts.rs"]
mod layouts;

#[rustfmt::skip]
#[path = "generated/secrets_v1.rs"]
pub mod secrets_v1;
