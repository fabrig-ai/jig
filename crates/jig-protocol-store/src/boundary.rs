//! Store requests and terminals (protocol/store.md, sections 3, 7 to 10).
use alloc::boxed::Box;
use jig_domain_core::{self as core, RestoreRange, reads};
use jig_domain_notes as notes;
use skein_io::file;
use skein_io::kernel::Fd;
use skein_lib::{Queue, Secret, Token};

/// What the engine's protocol layer asks of the component. Each request but
/// `Commit` names its asker's token; each has one terminal event.
#[derive(Debug)]
pub enum Request {
    /// Open both stores in the deployment's directory (store.md 10); the
    /// component's own rows and the core's header are checked before
    /// `Opened`. In skein-kv's memory mode, the worlds' component needs none.
    /// `installation` is the identifier the shell drew for this start
    /// (configuration.md 8): an empty store keeps it in its first commit; a
    /// store that holds one answers its own instead.
    Open { owner: Token, root: Root, installation: [u8; 16] },
    /// Activate the copy opened pending, once every earlier commit is durable
    /// (store.md 10.2, step 7). Once, after `Opened`; terminal `Switched` or `OpenRefused`.
    Switch { owner: Token },
    /// Commit `number` (store.md 6): its writes, in order, one skein-kv
    /// commit with the same number. Terminal: `Committed`, `Failed` or
    /// `FrameTooLarge`; a refusal stops the engine.
    Commit { number: u64, writes: Box<[Write]> },
    /// A page of one restart step's ranges (store.md 7.2), from `after`.
    Restore { owner: Token, range: Restore, after: Option<Cursor>, rows: u32, bytes: u64 },
    /// A page of a child's range (store.md 7.1), from `after`.
    Load { owner: Token, range: Load, after: Option<Cursor>, rows: u32, bytes: u64 },
    /// A party's read (store.md 7.3): its scope, position, direction and bounds.
    Read { owner: Token, range: reads::Range },
    /// One record by key.
    Get { owner: Token, key: Get },
    /// A transcript's turns (store.md 8.3).
    Transcript { owner: Token, read: TranscriptRead, rows: u32, bytes: u64 },
    /// The secrets' store (store.md 9).
    Secrets { owner: Token, request: Secrets },
    /// A written site's answer (store.md 10.3), with its machine handed back.
    Site { owner: Token, patcher: Box<skein_codec::patch::Patcher>, result: Result<(), skein_codec::patch::Halt> },
}

/// One write of a commit (store.md 6).
#[derive(Debug)]
pub enum Write {
    /// The start's component-owned wires row, from `Store::wires` (store.md 5.3).
    Wires,
    /// Copy a held settings revision to history in this commit.
    CopySettingsRevision(u32),
    /// jig's save: a put of its encoded key and record; a turn's save
    /// becomes a frame in its task's transcript and puts of its places.
    Save(core::Record),
    /// jig's erase.
    Erase(core::Key),
    /// A task's transcript dropped whole (store.md 8.5): its places erased,
    /// its file removed before the commit is submitted.
    DropTranscript { task: u64 },
    /// The application's put: its key's bytes (without the tag) and its value's.
    Put { key: Box<[u8]>, value: Box<[u8]> },
    /// The application's erase.
    Remove { key: Box<[u8]> },
}

/// What a restart step restores.
#[derive(Debug)]
pub enum Restore {
    /// jig's ranges, in order: `RestoreRange::LIVE` for `LoadCore`, the
    /// census for `LoadInbox`. Rows are handed up typed; the census's are
    /// the ended tasks its index rows name.
    Jig(Box<[RestoreRange]>),
    /// A connector's live intervals, relative to the application's tag.
    Application(Box<[skein_kv::Range]>),
}

/// A child's range to load.
#[derive(Debug)]
pub enum Load {
    /// The notes child's entry or a scope's lines.
    Notes(notes::Range),
    /// The application's intervals, relative to its tag.
    Application(Box<[skein_kv::Range]>),
}

/// A record to get.
#[derive(Debug)]
pub enum Get {
    Jig(core::Key),
    /// The application's key, relative to its tag.
    Application(Box<[u8]>),
}

/// A transcript read (store.md 8.3). Offsets are the component's, given
/// and taken back; none reads past what is committed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TranscriptRead {
    /// A resuming run's current window, from the turn the core names as its
    /// start; a window that starts at another turn is `Unread`.
    Window { task: u64, attempt: u64, turn: u32, after: Option<u64> },
    /// One turn, walked to from its attempt's first turn.
    Turn { task: u64, attempt: u64, turn: u32 },
    /// A page of a task's turns from either end; a fresh run's tail is
    /// `backwards: true, after: None`.
    Page { task: u64, backwards: bool, after: Option<u64> },
}

/// Whose secret (store.md 9).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum SecretOwner {
    /// A sign-in's digest, by the sign-in's number.
    SignIn,
    /// A service's credential's digest, by the service's number.
    Service,
    /// The application's own record, under its tag, by its number.
    Application,
    /// A signed-in source's record or an LLM key's value, by the source's number.
    Source,
}

/// The first number of the application's range that jig numbers itself,
/// for the values clients' requests carry (store.md 9); the application
/// numbers its own values below it.
pub const CARRIED: u64 = 1 << 63;

/// The first number of values held in memory for this run alone (store.md, section 9).
pub const HELD: u64 = CARRIED + (1 << 62);

/// A request to the secrets' store (store.md, section 9).
#[derive(Debug)]
pub enum Secrets {
    Keep {
        owner: SecretOwner,
        number: u64,
        value: Secret,
    },
    /// Keep `value` in place of the value whose digest is `replaces`, none
    /// when nothing is held: `Kept` once durable, or `Stale`, nothing
    /// written, when another value is held, durable or on its way.
    Replace {
        owner: SecretOwner,
        number: u64,
        value: Secret,
        replaces: Option<[u8; 32]>,
    },
    /// Atomically move a carried value into its owner's row, conditional on
    /// the target's digest as in `Replace`. Missing carried values are stale.
    Adopt {
        carried: u64,
        owner: SecretOwner,
        number: u64,
        replaces: Option<[u8; 32]>,
    },
    /// Adopt a decoded value, conditional on both the carried file's digest
    /// and the destination's digest. Both rows change in one commit (store.md 9).
    AdoptValue {
        carried: u64,
        holds: [u8; 32],
        owner: SecretOwner,
        number: u64,
        value: Secret,
        replaces: Option<[u8; 32]>,
    },
    /// Hold in memory, never written, under the next number from HELD (store.md, 9).
    /// Carried at once, nothing committed; Stale at the held-values bound.
    Hold {
        value: Secret,
    },
    /// Keep a value a client's request carried, under the next number of
    /// the application's carried range, from [`CARRIED`] up (store.md 9):
    /// `Carried` once durable, the next number moved in the same commit;
    /// `Stale`, nothing written, when that number already holds a value.
    Carry {
        value: Secret,
    },
    Erase {
        owner: SecretOwner,
        number: u64,
    },
    Get {
        owner: SecretOwner,
        number: u64,
    },
    Load {
        owner: SecretOwner,
        after: Option<u64>,
    },
}

#[derive(Debug)]
pub struct SecretRow {
    pub owner: SecretOwner,
    pub number: u64,
    pub value: Secret,
}

/// Where a page stopped: the interval of the request's list, and the last
/// key it returned (store.md 7.1: the next page starts just past it).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Cursor {
    pub interval: u32,
    pub after: Box<[u8]>,
}

/// One restored or loaded row.
#[derive(Debug)]
#[expect(clippy::large_enum_variant, reason = "bounded jig records travel by value; Row queues price their full size")]
pub enum Row {
    /// jig's record, decoded.
    Jig(core::Record),
    /// The application's row, its key relative to its tag.
    Application { key: Box<[u8]>, value: Box<[u8]> },
}

/// Whose format the format row disagrees on (store.md 5.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Family {
    Settings,
    Component,
    Core,
    Tasks,
    People,
    Notes,
    Accounts,
    Application,
}

/// What the check before the switch found (store.md 10.2, step 5).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Check {
    /// A patched record too large for a commit alone, by its key as `Patch` gives it.
    Large { key: Box<[u8]> },
    /// A kept family whose count of records the patches changed.
    Count { family: &'static str, before: u64, after: u64 },
    /// A row of the restart's ranges that does not load within the deployment's limits.
    Load { key: Box<[u8]> },
    /// An ended read position whose ended task the store does not hold.
    Join { key: Box<[u8]> },
    /// A file in the copy's secrets' store that is not private.
    Private { name: Box<[u8]> },
}
impl Check {
    /// Its refusal's words, in pieces with the deployment's directory between
    /// them, as `OpenRefusal::words` gives every refusal's (store.md 10.1).
    fn words(&self) -> &'static [&'static str] {
        match self {
            Self::Large { .. } => &[
                "The check before the switch found a patched record too large for a commit alone in the copy of the store this start patched in ",
                ", and the generation it patched from stays active, unchanged. Every start of this build checks a fresh copy the same way: start the build before it again, which opens that generation as it was.",
            ],
            Self::Count { .. } => &[
                "The check before the switch found a family whose count of records the patches changed in the copy of the store this start patched in ",
                ", and the generation it patched from stays active, unchanged. Every start of this build checks a fresh copy the same way: start the build before it again, which opens that generation as it was.",
            ],
            Self::Load { .. } => &[
                "The check before the switch found a record the restart could not load within the deployment's limits in the copy of the store this start patched in ",
                ", and the generation it patched from stays active, unchanged. Every start of this build checks a fresh copy the same way: start the build before it again, which opens that generation as it was.",
            ],
            Self::Join { .. } => &[
                "The check before the switch found a record naming an ended task the store does not hold in the copy of the store this start patched in ",
                ", and the generation it patched from stays active, unchanged. Every start of this build checks a fresh copy the same way: start the build before it again, which opens that generation as it was.",
            ],
            Self::Private { .. } => &[
                "The check before the switch found a file in the secrets' store of the copy this start patched in ",
                " that is not private to the service's user, and the generation it patched from stays active, unchanged. A start makes its copy's files private, so another process or user changed one: make sure nothing else writes in ",
                ", and start again.",
            ],
        }
    }
}

/// Why a store refuses to open, saying which (store.md 5.3, 6, 10).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum OpenRefusal {
    /// The format row names another version of `family`.
    Format { family: Family, found: u32, wanted: u32 },
    /// The format row names another deployment than the header.
    Deployment,
    /// skein-kv's recovered last commit is not the header's.
    Numbers { store: u64, header: u64 },
    /// A file failed, or a store did not recover.
    Failed,
    /// A store from an older build is left untouched (store.md 2 and 10.1).
    OlderBuild { why: Older },
    /// The deployment's generations refuse (store.md 10.1).
    Generations { why: Generations },
    /// A level past the binary's (5.3); `found: None` for a component
    /// version past the binary's, its levels unread.
    Ahead { owner: crate::Owner, found: Option<u32>, wanted: Option<u32> },
    /// A patch refused a record of the copy (10.2): the key as the walk
    /// handed it to the owner (`Event::Site`'s).
    Patch { owner: crate::Owner, patch: u32, key: Box<[u8]>, halt: skein_codec::patch::Halt },
    /// The binary's sets do not plan (10.3).
    Set { owner: crate::Owner, patch: u32 },
    /// The check before the switch failed (10.2, step 5); the generation
    /// patched from stays active, unchanged.
    Check { why: Check },
}
/// A refusal whose fix moves the generations (store.md 2), `why` saying
/// which version or build.
macro_rules! older {
    ($why:literal) => {
        older!(" holds a store from an older build, ", $why)
    };
    ($clause:literal, $why:literal) => {
        &[
            "",
            concat!(
                $clause,
                $why,
                ", and this start has changed nothing in it. Before the first release a store is not translated, so every start refuses this one until it is moved. To start afresh, stop the service, move `active`, `generations/` and `transcripts/` out of ",
            ),
            " together, and start again: nothing is deleted, and the next start makes a new, empty store. The new store has no tasks, sign-ins, accounts or keys, and every client signs in again.",
        ]
    };
}

impl OpenRefusal {
    /// The refusal's text in pieces, in order: the deployment's directory goes
    /// between each two consecutive pieces, wherever store.md has `<deployment>`.
    /// No allocation, no formatting.
    #[must_use]
    pub fn words(&self) -> &'static [&'static str] {
        match self {
            Self::Format { family, .. } => match family {
                Family::Settings => {
                    older!(" holds a store whose ", "settings family is at another version than this build's")
                }
                Family::Component => {
                    older!(" holds a store whose ", "component family is at another version than this build's")
                }
                Family::Core => older!(" holds a store whose ", "core family is at another version than this build's"),
                Family::Tasks => {
                    older!(" holds a store whose ", "tasks family is at another version than this build's")
                }
                Family::People => {
                    older!(" holds a store whose ", "people family is at another version than this build's")
                }
                Family::Notes => {
                    older!(" holds a store whose ", "notes family is at another version than this build's")
                }
                Family::Accounts => {
                    older!(" holds a store whose ", "accounts family is at another version than this build's")
                }
                Family::Application => {
                    older!(" holds a store whose ", "application family is at another version than this build's")
                }
            },
            Self::Deployment => &[
                "The store in ",
                " names another deployment than its header does, so its files come from two deployments, and this start has changed nothing in it. Restore ",
                " whole from one backup, or, to start afresh, stop the service, move `active`, `generations/` and `transcripts/` out of ",
                " together, and start again: nothing is deleted, and the next start makes a new, empty store.",
            ],
            Self::Numbers { .. } => &[
                "The store in ",
                " does not end at the commit its header records, so a file was lost, cut or restored apart from the others, and this start has changed nothing in it. Restore ",
                " whole from one backup, or, to start afresh, stop the service, move `active`, `generations/` and `transcripts/` out of ",
                " together, and start again: nothing is deleted, and the next start makes a new, empty store.",
            ],
            Self::Failed => &[
                "A file in ",
                " could not be read or written, or its store could not be recovered, while this start opened it, and the start stopped there. Check the disk's space and the directory's permissions, and start again. If the next start fails the same way, restore ",
                " from a backup.",
            ],
            Self::OlderBuild { why: Older::Unleveled } => &[
                "",
                " holds a store from before the first release, which records no patch levels, and this start has changed nothing in it. A store from before the first release is not translated, so every start refuses this one until it is moved. To start afresh, stop the service, move `active`, `generations/` and `transcripts/` out of ",
                " together, and start again: nothing is deleted, and the next start makes a new, empty store. The new store has no tasks, sign-ins, accounts or keys, and every client signs in again.",
            ],
            Self::Ahead { .. } => &[
                "",
                " holds a store from a later build, at patch levels past this build's, and this start has changed nothing in it. No patch runs backwards: start the later build again, or restore ",
                " from a backup this build made.",
            ],
            Self::Patch { .. } => &[
                "A patch refused a record while this start patched a copy of the store in ",
                ", and the generation it patched from stays active, unchanged. Every start of this build refuses the same record: start the build before it again, which opens that generation as it was.",
            ],
            Self::Set { .. } => &[
                "This build's patches do not plan, and this start has changed nothing in ",
                ". The build is wrong: start the build before it again.",
            ],
            Self::Check { why } => why.words(),
            Self::OlderBuild { why: Older::Unrecorded } => older!("made before its families' layouts were recorded"),
            Self::OlderBuild { why: Older::Layout(family) } => match family {
                Family::Settings => older!("whose settings family's layout differs from this build's"),
                Family::Component => older!("whose component family's layout differs from this build's"),
                Family::Core => older!("whose core family's layout differs from this build's"),
                Family::Tasks => older!("whose tasks family's layout differs from this build's"),
                Family::People => older!("whose people family's layout differs from this build's"),
                Family::Notes => older!("whose notes family's layout differs from this build's"),
                Family::Accounts => older!("whose accounts family's layout differs from this build's"),
                Family::Application => older!("whose application family's layout differs from this build's"),
            },
            Self::OlderBuild { why: Older::Directory } => &[
                "",
                " holds a store from an older build, in the layout before generations (`store/`, `secrets/` or `transcripts/` at its top), and this start has changed nothing in it. Before the first release a store is not translated, so every start refuses this one until it is moved. To start afresh, stop the service, move `store/`, `secrets/` and `transcripts/` out of ",
                " together, and start again: nothing is deleted, and the next start makes a new, empty store. The new store has no tasks, sign-ins, accounts or keys, and every client signs in again.",
            ],
            Self::Generations { why: Generations::Busy } => &[
                "Another process has ",
                " open, and this start has changed nothing in it. Stop that process first, then start again.",
            ],
            Self::Generations { why: Generations::Unreadable } => &[
                "The `active` file in ",
                " does not name a generation, and this start has changed nothing in it. Restore ",
                " from a backup, or write `active` again to name a generation under `generations/` that stands, and start again.",
            ],
            Self::Generations { why: Generations::Missing { .. } } => &[
                "The generation `active` names in ",
                " has lost its `store/` or `secrets/`, and this start has changed nothing in it. Restore ",
                " from a backup, or write `active` again to name a generation under `generations/` that stands, and start again.",
            ],
            Self::Generations { why: Generations::Unnamed } => &[
                "",
                " holds generations that no `active` names, and this start has changed nothing in it. Name one again from a backup of ",
                ", or write `active` to name the generation under `generations/` to open, and start again.",
            ],
        }
    }
}
/// Which older build (store.md 5.3 and 10.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Older {
    /// A store before an owner's baseline, which records no patch level (5.3).
    Unleveled,
    /// A store with no layouts row, made before they were recorded (store.md 5.3).
    Unrecorded,
    /// A family's fingerprint is not this build's at its version (store.md 5.3).
    Layout(Family),
    /// The layout before generations, with no active file (store.md 2).
    Directory,
}
/// A refusal while finding the deployment's generation (store.md 10.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Generations {
    /// Another open holds the deployment lock.
    Busy,
    /// Active is not decimal digits and one newline within 21 bytes.
    Unreadable,
    /// Generations stand that no active names, beyond a cut first start.
    Unnamed,
    /// The active generation's maps do not both stand as directories.
    Missing { generation: u64 },
}

/// What the component answers.
#[derive(Debug)]
pub enum Event {
    /// Both stores are open and checked; `last` is the last durable commit.
    /// `installation` is the installation's identifier (store.md 4.4): the
    /// kept one, or, in an empty store, the drawn one its first commit keeps.
    Opened {
        owner: Token,
        last: u64,
        installation: [u8; 16],
        pending: bool,
    },
    /// The pending copy's manifest is durable and active has been replaced (10.2).
    Switched {
        owner: Token,
    },
    OpenRefused {
        owner: Token,
        why: OpenRefusal,
    },
    /// Commit `number` is durable, and every earlier one; `fill` is the
    /// map's bytes (skein-kv's `used`), for the core's high-water mark.
    Committed {
        number: u64,
        fill: u64,
    },
    /// Commit `number` failed or was refused; the engine stops (store.md 6).
    Failed {
        number: u64,
    },
    /// Commit refused before any of its data IO: this turn's complete encoded
    /// frame exceeds the page byte bound (store.md 6 and 8.2). In-process only.
    FrameTooLarge {
        number: u64,
        task: u64,
        bytes: u32,
        limit: u32,
    },
    /// A restore's page; `next` is none once the step's ranges are done.
    Restored {
        owner: Token,
        rows: Box<[Row]>,
        next: Option<Cursor>,
    },
    /// A load's page; `next` is none once the range is done.
    Loaded {
        owner: Token,
        rows: Box<[Row]>,
        next: Option<Cursor>,
    },
    /// A read's page, busy, or gone.
    Read {
        owner: Token,
        result: reads::Loaded,
    },
    Got {
        owner: Token,
        row: Option<Row>,
    },
    /// A transcript's turns; `next` is the offset to continue from after
    /// a nonempty prefix. An empty page is terminal (store.md 8.3).
    Turns {
        owner: Token,
        turns: Box<[core::TurnRecord]>,
        next: Option<u64>,
    },
    /// A transcript read failed: a frame's checksum, a window at another
    /// turn, a dropped transcript, or no complete turn fitting the direct
    /// read's room while committed bytes remain (store.md 8.3, 8.5).
    Unread {
        owner: Token,
    },
    /// A secrets request refused until this start's mark is durable (store.md 5.3).
    SecretsNotReady {
        owner: Token,
    },
    Kept {
        owner: Token,
    },
    /// A carried value kept, durable, under the application's `number`.
    Carried {
        owner: Token,
        number: u64,
    },
    /// A replace that named another value than the one held, or a carry
    /// whose next number holds one.
    Stale {
        owner: Token,
    },
    Erased {
        owner: Token,
    },
    Secret {
        owner: Token,
        value: Option<Secret>,
    },
    SecretsLoaded {
        owner: Token,
        rows: Box<[SecretRow]>,
        next: Option<u64>,
    },
    /// A stored row did not decode: the request's terminal; the engine
    /// stops (store.md 7.1).
    Corrupt {
        owner: Token,
    },
    /// The shell sets this record's key in `of`'s Context, runs its generated
    /// written function over `patcher.parts()`, and answers `Request::Site`
    /// (store.md 10.3). Application keys are relative to their tag; every
    /// other owner's keys are the map's whole keys. While serving, a frame's
    /// key is its task's places key followed by its eight-byte big-endian
    /// file offset (store.md 8.6).
    Site {
        owner: Token,
        of: crate::Owner,
        key: Box<[u8]>,
        site: skein_codec::patch::Site,
        at: skein_codec::value::Node,
        patcher: Box<skein_codec::patch::Patcher>,
    },
    /// A patch's walk is done and committed (store.md 10.2, step 4).
    Patched {
        owner: Token,
        of: crate::Owner,
        patch: u32,
        counts: crate::Counts,
    },
}

/// A directory adopted by the service (io.md, section 9).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Root {
    pub token: Token,
    pub fd: Fd,
}

/// Independent file drivers (store.md, section 13).
#[derive(Debug)]
pub struct Below {
    pub store: Queue<file::Request>,
    pub secrets: Queue<file::Request>,
    pub transcripts: Queue<file::Request>,
}

/// A file terminal and its descriptor, read by the service alone.
#[derive(Debug)]
pub struct Arrived {
    pub event: file::Event,
    pub fd: Option<Fd>,
}

#[derive(Debug)]
pub enum Io {
    Store(Arrived),
    Secrets(Arrived),
    Transcripts(Arrived),
}
