//! Common role admission; membership belongs to people and policy to authority.
use super::Act;
use crate::Core;
use jig_domain_people as people;

pub(crate) fn hold_project(core: &Core, project: u32, held: Option<people::Role>) -> Result<(), people::Refusal> {
    if !core.people.has_project(project) {
        return Err(people::Refusal::Unknown);
    }
    match held {
        Some(people::Role::Owner | people::Role::Maintainer) => Ok(()),
        Some(people::Role::Member | people::Role::Observer | people::Role::Policy { .. }) | None => {
            Err(people::Refusal::Role { held })
        }
    }
}

pub(crate) fn role(core: &Core, project: u32, held: Option<people::Role>, act: Act) -> Result<(), people::Refusal> {
    match act {
        Act::Reorder => match held {
            Some(people::Role::Owner | people::Role::Maintainer) => Ok(()),
            Some(people::Role::Member | people::Role::Observer | people::Role::Policy { .. }) | None => {
                Err(people::Refusal::Role { held })
            }
        },
        Act::Adopt | Act::Rename | Act::ChangePolicy | Act::SetRoles | Act::MakeService | Act::SetPool => {
            if !core.people.has_project(project) {
                return Err(people::Refusal::Unknown);
            }
            if held == Some(people::Role::Owner) { Ok(()) } else { Err(people::Refusal::Role { held }) }
        }
        Act::Message | Act::Answer | Act::Amend | Act::EditNote | Act::DeleteNote => match held {
            Some(
                people::Role::Owner | people::Role::Maintainer | people::Role::Member | people::Role::Policy { .. },
            ) => Ok(()),
            Some(people::Role::Observer) | None => Err(people::Refusal::Role { held }),
        },
        Act::Take
        | Act::HandBack
        | Act::Accept
        | Act::Reject
        | Act::PassUp
        | Act::Release
        | Act::LeaveHeld
        | Act::Stop
        | Act::Cancel
        | Act::Move
        | Act::Seen
        | Act::Withdraw => Ok(()),
    }
}
