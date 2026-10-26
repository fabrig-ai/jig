//! The component's own durable rows (protocol/store.md, sections 5.3 and 8.1).
use crate::component_v3 as wire;
use alloc::boxed::Box;
use skein_lib::{List, Reader, Writer};
pub use wire::{Count, CountParts};

/// Encode the shell's validated start wires (store.md 5.3).
#[must_use]
pub(crate) fn wires(wires: &[crate::Wire]) -> Box<[u8]> {
    let mut values = List::with_capacity(wire::CEILINGS.wires_wires);
    for value in wires {
        let value = wire::Wire::new(
            &wire::CEILINGS,
            wire::WireParts { name: Box::from(value.name.as_bytes()), version: value.version },
        )
        .expect("Store::wires validated each name");
        values.push(value).expect("Store::wires validated the count");
    }
    let row = wire::Wires::new(&wire::CEILINGS, wire::WiresParts { wires: values }).expect("validated wires");
    let mut writer = Writer::new(usize::try_from(row.measure()).expect("bounded wires row"));
    row.encode(&mut writer).expect("measured wires row");
    writer.finish()
}

/// Each owner's applied patch level (store.md 5.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Levels {
    pub skein: Option<u32>,
    pub smith: Option<u32>,
    pub jig: Option<u32>,
    pub application: Option<u32>,
}
impl Levels {
    fn encoded(self) -> wire::Levels {
        wire::Levels::new(
            &wire::CEILINGS,
            wire::LevelsParts { skein: self.skein, smith: self.smith, jig: self.jig, application: self.application },
        )
        .expect("levels have no bounded fields")
    }
    fn decoded(row: wire::Levels) -> Self {
        let wire::LevelsParts { skein, smith, jig, application } = row.into_parts();
        Self { skein, smith, jig, application }
    }
}

/// The component's format row.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Format {
    pub component: u16,
    pub core: u16,
    pub tasks: u16,
    pub people: u16,
    pub notes: u16,
    pub accounts: u16,
    pub settings: u16,
    pub application: u32,
    pub deployment: [u8; 16],
    pub levels: Levels,
}

impl Format {
    /// Encode this row with the component family's version.
    pub fn encode(&self) -> Result<Box<[u8]>, wire::Problem> {
        let row = wire::Format::new(
            &wire::CEILINGS,
            wire::FormatParts {
                component: self.component,
                core: self.core,
                tasks: self.tasks,
                people: self.people,
                notes: self.notes,
                accounts: self.accounts,
                settings: self.settings,
                application: self.application,
                deployment: self.deployment,
                levels: self.levels.encoded(),
            },
        )?;
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("encoded row fits usize"));
        row.encode(&mut writer).expect("measured room");
        Ok(writer.finish())
    }

    /// Decode one row, refusing another version or trailing bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, wire::Problem> {
        let wire::FormatParts {
            component,
            core,
            tasks,
            people,
            notes,
            accounts,
            settings,
            application,
            deployment,
            levels,
        } = wire::Format::decode(&wire::CEILINGS, &mut Reader::new(bytes))?.into_parts();
        Ok(Self {
            component,
            core,
            tasks,
            people,
            notes,
            accounts,
            settings,
            application,
            deployment,
            levels: Levels::decoded(levels),
        })
    }

    /// The component family's version a stored row begins with, read before
    /// the row decodes, so that opening can say which version it found.
    #[must_use]
    pub fn version(bytes: &[u8]) -> Option<u16> {
        Reader::new(bytes).u16()
    }
}

/// A generation's immutable manifest (store.md 2).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Manifest {
    pub generation: u64,
    pub from: Option<u64>,
    pub before: Levels,
    pub after: Levels,
    pub counts: List<Count>,
    pub store: u64,
    pub secrets: Option<u64>,
    pub written: u64,
}
impl Manifest {
    #[must_use]
    pub fn encode(self) -> Box<[u8]> {
        let row = wire::Manifest::new(
            &wire::CEILINGS,
            wire::ManifestParts {
                generation: self.generation,
                from: self.from,
                before: self.before.encoded(),
                after: self.after.encoded(),
                counts: self.counts,
                store: self.store,
                secrets: self.secrets,
                written: self.written,
            },
        )
        .expect("manifest counts fit the checked sets");
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("encoded manifest fits usize"));
        row.encode(&mut writer).expect("measured manifest room");
        writer.finish()
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, wire::Problem> {
        let wire::ManifestParts { generation, from, before, after, counts, store, secrets, written } =
            wire::Manifest::decode(&wire::CEILINGS, &mut Reader::new(bytes))?.into_parts();
        Ok(Self {
            generation,
            from,
            before: Levels::decoded(before),
            after: Levels::decoded(after),
            counts,
            store,
            secrets,
            written,
        })
    }
}

/// The component's transcript row.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Transcript {
    pub length: u64,
    pub window: u64,
}

impl Transcript {
    /// Encode this row with the component family's version.
    pub fn encode(&self) -> Result<Box<[u8]>, wire::Problem> {
        let row =
            wire::Transcript::new(&wire::CEILINGS, wire::TranscriptParts { length: self.length, window: self.window })?;
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("encoded row fits usize"));
        row.encode(&mut writer).expect("measured room");
        Ok(writer.finish())
    }

    /// Decode one row, refusing another version or trailing bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, wire::Problem> {
        let wire::TranscriptParts { length, window } =
            wire::Transcript::decode(&wire::CEILINGS, &mut Reader::new(bytes))?.into_parts();
        Ok(Self { length, window })
    }
}

/// The component's attempt row.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Attempt {
    pub first: u64,
}

impl Attempt {
    /// Encode this row with the component family's version.
    pub fn encode(&self) -> Result<Box<[u8]>, wire::Problem> {
        let row = wire::Attempt::new(&wire::CEILINGS, wire::AttemptParts { first: self.first })?;
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("encoded row fits usize"));
        row.encode(&mut writer).expect("measured room");
        Ok(writer.finish())
    }

    /// Decode one row, refusing another version or trailing bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, wire::Problem> {
        let wire::AttemptParts { first } =
            wire::Attempt::decode(&wire::CEILINGS, &mut Reader::new(bytes))?.into_parts();
        Ok(Self { first })
    }
}

/// Each jig family's layout fingerprint, kept by the first commit (store.md, 5.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Layouts {
    pub component: [u8; 32],
    pub core: [u8; 32],
    pub tasks: [u8; 32],
    pub people: [u8; 32],
    pub notes: [u8; 32],
    pub accounts: [u8; 32],
    pub settings: [u8; 32],
}

impl Layouts {
    /// Encode the seven fixed digests with the component family's version.
    #[must_use]
    pub fn encode(&self) -> Box<[u8]> {
        let row = wire::Layouts::new(
            &wire::CEILINGS,
            wire::LayoutsParts {
                component: self.component,
                core: self.core,
                tasks: self.tasks,
                people: self.people,
                notes: self.notes,
                accounts: self.accounts,
                settings: self.settings,
            },
        )
        .expect("layouts have no bounded fields");
        let mut writer = Writer::new(usize::try_from(row.measure()).expect("encoded row fits usize"));
        row.encode(&mut writer).expect("measured room");
        writer.finish()
    }

    /// Decode one complete row, refusing another version or trailing bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, wire::Problem> {
        let wire::LayoutsParts { component, core, tasks, people, notes, accounts, settings } =
            wire::Layouts::decode(&wire::CEILINGS, &mut Reader::new(bytes))?.into_parts();
        Ok(Self { component, core, tasks, people, notes, accounts, settings })
    }
}
