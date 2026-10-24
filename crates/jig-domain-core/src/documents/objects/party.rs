//! Party and task amendment offers share the same before/after edit document.
use super::{Box, Core, Limits, List, Missing, ProposalAction, tasks};

pub(super) fn amendment(
    core: &Core,
    limits: &Limits,
    party: u64,
    task: u64,
    change: &tasks::Amendment,
) -> Result<ProposalAction, Missing> {
    use super::super::{small, task_tree};
    let row = small::visible(core, party, task)?;
    Ok(ProposalAction::Amend {
        task: task_tree::link(core, limits, party, task)?,
        addressed: change.addressed,
        before: Box::new(super::amendment(core, limits, row, None)?),
        after: Box::new(super::amendment(core, limits, row, Some(change))?),
        reason: small::text(&change.reason, limits.documents.text_bytes),
        release: change.release,
    })
}

pub(super) fn action(
    core: &Core,
    limits: &Limits,
    party: u64,
    action: &tasks::PersonAction,
) -> Result<(ProposalAction, tasks::ProposalKind), Missing> {
    match action {
        tasks::PersonAction::Settings { against, items } => Ok((
            ProposalAction::Settings {
                against: *against,
                items: crate::documents::settings_heads(items),
                review: None,
                made: None,
            },
            tasks::ProposalKind::Settings,
        )),
        tasks::PersonAction::Batch(members) => {
            let mut batch = List::with_capacity(limits.documents.lines);
            for member in members {
                if batch.push(super::proposed(limits, member)?).is_err() {
                    return Err(Missing::TooLarge);
                }
            }
            Ok((ProposalAction::Batch(batch), tasks::ProposalKind::Batch))
        }
        tasks::PersonAction::Amend { task, amendment: change, .. } => {
            Ok((amendment(core, limits, party, *task, change)?, tasks::ProposalKind::Amend))
        }
    }
}
