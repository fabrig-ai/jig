//! Versions and deployment checked at open (protocol/store.md, 5.3).
use crate::boundary::{Family, OpenRefusal};
use crate::codec::component;
use crate::patching::plan::Levels;
use crate::{Older, Owner};

pub(crate) const NONE: Levels = Levels { skein: None, smith: None, jig: None, application: None };

pub(crate) fn levels(row: component::Levels) -> Levels {
    Levels { skein: row.skein, smith: row.smith, jig: row.jig, application: row.application }
}
pub(crate) fn recorded(levels: Levels) -> component::Levels {
    component::Levels { skein: levels.skein, smith: levels.smith, jig: levels.jig, application: levels.application }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Compared {
    Same,
    Behind,
}
/// Ahead wins over Unleveled, which wins over Behind, each upstream first (5.3).
pub(crate) fn compare(found: Levels, wanted: Levels) -> Result<Compared, OpenRefusal> {
    const OWNERS: [Owner; 4] = [Owner::Skein, Owner::Smith, Owner::Jig, Owner::Application];
    for owner in OWNERS {
        if found.of(owner) > wanted.of(owner) {
            return Err(OpenRefusal::Ahead { owner, found: found.of(owner), wanted: wanted.of(owner) });
        }
    }
    for owner in OWNERS {
        if found.of(owner).is_none() && wanted.of(owner).is_some() {
            return Err(OpenRefusal::OlderBuild { why: Older::Unleveled });
        }
    }
    for owner in OWNERS {
        if found.of(owner) < wanted.of(owner) {
            return Ok(Compared::Behind);
        }
    }
    Ok(Compared::Same)
}

#[must_use]
pub fn current(application: u32, deployment: [u8; 16]) -> component::Format {
    component::Format {
        component: 3,
        core: 6,
        tasks: 9,
        people: 13,
        notes: 1,
        accounts: 2,
        settings: 1,
        application,
        deployment,
        levels: recorded(NONE),
    }
}

pub fn check(found: &component::Format, wanted: &component::Format) -> Result<(), OpenRefusal> {
    check_versions(found, wanted, Compared::Same)
}

pub(crate) fn check_versions(
    found: &component::Format,
    wanted: &component::Format,
    compared: Compared,
) -> Result<(), OpenRefusal> {
    for (family, found, wanted) in [
        (Family::Component, u32::from(found.component), u32::from(wanted.component)),
        (Family::Core, u32::from(found.core), u32::from(wanted.core)),
        (Family::Tasks, u32::from(found.tasks), u32::from(wanted.tasks)),
        (Family::People, u32::from(found.people), u32::from(wanted.people)),
        (Family::Notes, u32::from(found.notes), u32::from(wanted.notes)),
        (Family::Accounts, u32::from(found.accounts), u32::from(wanted.accounts)),
        (Family::Settings, u32::from(found.settings), u32::from(wanted.settings)),
    ] {
        if found != wanted {
            return Err(OpenRefusal::Format { family, found, wanted });
        }
    }
    // No jig patch moves a family of jig's own yet: even Behind uses the
    // binary's versions and layouts. Application patches rewrite its version.
    if compared == Compared::Same && found.application != wanted.application {
        return Err(OpenRefusal::Format {
            family: Family::Application,
            found: found.application,
            wanted: wanted.application,
        });
    }
    if found.deployment != wanted.deployment {
        return Err(OpenRefusal::Deployment);
    }
    Ok(())
}

/// This build's layouts (store.md, 5.3): computed from the schemas built in,
/// committed beside the generated codecs, and checked against them by the drift test.
#[must_use]
pub fn layouts() -> component::Layouts {
    crate::layouts::LAYOUTS
}

/// Compare in Family's order, before any family's header is decoded.
pub fn check_layouts(found: &component::Layouts, wanted: &component::Layouts) -> Result<(), OpenRefusal> {
    for (family, found, wanted) in [
        (Family::Settings, found.settings, wanted.settings),
        (Family::Component, found.component, wanted.component),
        (Family::Core, found.core, wanted.core),
        (Family::Tasks, found.tasks, wanted.tasks),
        (Family::People, found.people, wanted.people),
        (Family::Notes, found.notes, wanted.notes),
        (Family::Accounts, found.accounts, wanted.accounts),
        (Family::Secrets, found.secrets, wanted.secrets),
        (Family::SkeinOauth, found.skein_oauth, wanted.skein_oauth),
        (Family::SmithTranscript, found.smith_transcript, wanted.smith_transcript),
    ] {
        if found != wanted {
            return Err(OpenRefusal::OlderBuild { why: Older::Layout(family) });
        }
    }
    Ok(())
}
