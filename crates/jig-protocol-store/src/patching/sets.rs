//! Static owner data handed in by the shell (protocol/store.md 5.3 and 10.3).

/// Whose patch levels the format row keeps, upstream first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Owner {
    Skein,
    Smith,
    Jig,
    Application,
}

#[derive(Clone, Copy, Debug)]
pub struct PatchSet {
    pub owner: Owner,
    pub name: &'static str,
    pub families: &'static [KeptFamily],
    /// Patch texts in number order.
    pub patches: &'static [&'static str],
    /// The owner's mark across which windows start fresh.
    pub fresh: Option<&'static str>,
}
/// Which of the deployment's two maps a kept family's records live in (store.md 9, 10.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeptMap {
    Store,
    Secrets,
}

#[derive(Clone, Copy, Debug)]
pub struct KeptFamily {
    pub name: &'static str,
    /// Kept texts in version order.
    pub texts: &'static [&'static str],
    pub map: KeptMap,
    pub keys: &'static [Keys],
    /// The declaration used as a transcript frame root.
    pub frames: Option<&'static str>,
    /// Ranges whose every record is an effect in flight, its key naming
    /// it (store.md 10.3), in `Keys`' form. Empty: none.
    pub flights: &'static [Keys],
}
#[derive(Clone, Copy, Debug)]
pub struct Keys {
    /// Inclusive; application's own bytes, whole map keys for other owners.
    pub from: &'static [u8],
    /// Exclusive, in the same key space as `from`.
    pub to: &'static [u8],
    /// Declaration of the records in this range.
    pub root: &'static str,
}
#[derive(Clone, Copy, Debug)]
pub struct PatchSets {
    /// At most one set per owner.
    pub sets: &'static [PatchSet],
}
impl PatchSets {
    pub const NONE: PatchSets = PatchSets { sets: &[] };
}

/// Jig's store and held-value families (store.md 4.4, 9 and 10.3).
/// Component-managed rows and the installation at 01 04 03 are excluded from patch walks.
pub const JIG: PatchSet = PatchSet {
    owner: Owner::Jig,
    name: "jig",
    patches: crate::patches::PATCHES,
    fresh: None,
    families: &[
        KeptFamily {
            name: "jig-store-component",
            texts: crate::patches::kept::jig_store_component::TEXTS,
            map: KeptMap::Store,
            frames: None,
            flights: &[],
            keys: &[],
        },
        KeptFamily {
            name: "jig-store-core",
            texts: crate::patches::kept::jig_store_core::TEXTS,
            map: KeptMap::Store,
            frames: Some("TurnRecord"),
            flights: &[],
            keys: &[Keys { from: &[1, 0, 0], to: &[1, 0, 8], root: "Row" }],
        },
        KeptFamily {
            name: "jig-store-tasks",
            texts: crate::patches::kept::jig_store_tasks::TEXTS,
            map: KeptMap::Store,
            frames: None,
            flights: &[],
            keys: &[Keys { from: &[1, 1], to: &[1, 2], root: "Row" }],
        },
        KeptFamily {
            name: "jig-store-people",
            texts: crate::patches::kept::jig_store_people::TEXTS,
            map: KeptMap::Store,
            frames: None,
            flights: &[],
            keys: &[Keys { from: &[1, 2], to: &[1, 3], root: "Row" }],
        },
        KeptFamily {
            name: "jig-store-notes",
            texts: crate::patches::kept::jig_store_notes::TEXTS,
            map: KeptMap::Store,
            frames: None,
            flights: &[],
            keys: &[Keys { from: &[1, 3], to: &[1, 4], root: "Row" }],
        },
        KeptFamily {
            name: "jig-store-accounts",
            texts: crate::patches::kept::jig_store_accounts::TEXTS,
            map: KeptMap::Store,
            frames: None,
            flights: &[],
            keys: &[
                Keys { from: &[1, 4, 0], to: &[1, 4, 3], root: "Row" },
                Keys { from: &[1, 4, 4], to: &[1, 4, 5], root: "Row" },
            ],
        },
        KeptFamily {
            name: "jig-store-settings",
            texts: crate::patches::kept::jig_store_settings::TEXTS,
            map: KeptMap::Store,
            frames: None,
            flights: &[],
            keys: &[
                Keys { from: &[1, 0, 8, 0], to: &[1, 0, 8, 1], root: "Default" },
                Keys { from: &[1, 0, 8, 1], to: &[1, 0, 8, 2], root: "Changed" },
                Keys { from: &[1, 0, 8, 2], to: &[1, 0, 8, 3], root: "Revision" },
                Keys { from: &[1, 0, 8, 3], to: &[1, 0, 8, 4], root: "CharterVersion" },
                Keys { from: &[1, 0, 8, 4], to: &[1, 0, 8, 5], root: "Proposal" },
                Keys { from: &[1, 0, 8, 5], to: &[1, 0, 8, 6], root: "Extent" },
                Keys { from: &[1, 0, 9], to: &[1, 0, 10], root: "Revision" },
            ],
        },
        KeptFamily {
            name: "jig-store-secrets",
            texts: crate::patches::kept::jig_store_secrets::TEXTS,
            map: KeptMap::Secrets,
            frames: None,
            flights: &[],
            keys: &[
                Keys { from: &[1, 0], to: &[1, 2], root: "Digest" },
                Keys { from: &[3, 0], to: &[3, 1], root: "Source" },
                Keys { from: &[2, 0, 128], to: &[2, 0, 192], root: "Carried" },
                Keys { from: &[2, 1], to: &[2, 2], root: "NextCarried" },
            ],
        },
        KeptFamily {
            name: "jig-accounts",
            texts: jig_protocol_accounts::patches::kept::jig_accounts::TEXTS,
            map: KeptMap::Secrets,
            frames: None,
            flights: &[],
            keys: &[],
        },
    ],
};
