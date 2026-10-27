//! Small live documents and the shared bounded chip construction.
use super::{Chip, Count, ExecutorKind, Frame, FrameProjectRole, Missing, PhaseLine, Run, Text};
use crate::{Core, Limits};
use alloc::boxed::Box;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::List;
use skein_lib::bytes::copy_of;

#[expect(clippy::disallowed_methods, reason = "client/domain/contract.md, 5.3 requires UTF-8 text validation")]
pub(super) fn text(bytes: &[u8], maximum: u32) -> Text {
    let source = core::str::from_utf8(bytes).expect("source text is UTF-8 at the engine entrance");
    let mut length = bytes.len().min(usize::try_from(maximum).expect("text bound fits usize"));
    for _ in 0_u8..3 {
        if source.is_char_boundary(length) {
            break;
        }
        length = length.saturating_sub(1);
    }
    Text { bytes: copy_of(bytes.get(..length).expect("bounded prefix")), cut: length < bytes.len() }
}

pub(super) fn visible(core: &Core, party: u64, number: u64) -> Result<&tasks::TaskRecord, Missing> {
    let row = core.tasks.task(number).ok_or(Missing::Unknown)?;
    if core.people.role(party, row.project).is_none() {
        return Err(Missing::NotAllowed);
    }
    match row.phase {
        tasks::Phase::Ended(_) => Err(Missing::Ended),
        tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Closing(_) | tasks::Phase::Held { .. } => {
            Ok(row)
        }
    }
}

pub(super) fn chip(core: &Core, limits: &Limits, party: u64, number: u64) -> Result<Chip, Missing> {
    let row = visible(core, party, number)?;
    chip_with_core(core, limits, party, row)
}

pub(super) fn chip_with_core(
    core: &Core,
    limits: &Limits,
    party: u64,
    row: &tasks::TaskRecord,
) -> Result<Chip, Missing> {
    let mut chip = chip_from_row(limits, row)?;
    chip.details.hold = super::task_tree::hold_for(core, limits, party, row)?;
    super::chip_phase::observe(core, limits, row, &mut chip.details)?;
    Ok(chip)
}

#[expect(clippy::manual_map, reason = "step code explicitly selects readiness without closures")]
pub(super) fn chip_from_row(limits: &Limits, row: &tasks::TaskRecord) -> Result<Chip, Missing> {
    let number = row.number;
    let first = skein_lib::bytes::find(&row.spec.words, b"\n").unwrap_or(row.spec.words.len());
    let title = text(row.spec.words.get(..first).expect("first line prefix"), limits.documents.text_bytes);
    let executor = match row.executor {
        tasks::Executor::Agent { .. } => ExecutorKind::Agent,
        tasks::Executor::Procedure { .. } => ExecutorKind::Procedure,
        tasks::Executor::Person(_) => ExecutorKind::Person,
    };
    let phase = match &row.phase {
        tasks::Phase::Waiting => {
            let mut dependencies = List::with_capacity(limits.documents.lines);
            for number in row.waiting_on.iter().take(usize::try_from(limits.documents.lines).expect("line count fits"))
            {
                dependencies.push(*number).expect("bounded dependency lines");
            }
            PhaseLine::Waiting {
                more: usize::try_from(dependencies.len()).expect("line count fits") < row.waiting_on.len(),
                dependencies,
                holds: !row.holds_taken && !row.holdings.is_empty(),
            }
        }
        tasks::Phase::Active(active) => match row.executor {
            tasks::Executor::Person(address) => PhaseLine::Person {
                address,
                taken_by: row.taken_by,
                since: match row.person_wait {
                    Some(wait) => Some(wait.since),
                    None => None,
                },
            },
            tasks::Executor::Agent { .. } | tasks::Executor::Procedure { .. } => PhaseLine::Active(*active),
        },
        tasks::Phase::Closing(closing) => PhaseLine::Closing(closing.stage),
        tasks::Phase::Held { why, .. } => PhaseLine::Held { reason: *why, revision: row.revisions.hold },
        tasks::Phase::Ended(_) => PhaseLine::Ended,
    };
    Ok(Chip { number, title, executor, phase, details: Box::new(super::chip_phase::from_row(limits, row)?) })
}

#[expect(clippy::manual_map, reason = "step code explicitly matches optional owner ledgers without closures")]
pub(super) fn frame(core: &Core, limits: &Limits, party: u64) -> Result<Frame, Missing> {
    let identity = core.people.identity(party).ok_or(Missing::Unknown)?;
    let roles = core.people.project_roles(party).ok_or(Missing::Unknown)?;
    let mut projects = List::with_capacity(limits.documents.lines);
    for role in roles.iter().take(usize::try_from(limits.documents.lines).expect("line count fits")) {
        let pool = match core.tasks.funding(tasks::Funder::Pool {
            project: role.project,
            person: party,
            period: core.period(role.project),
        }) {
            Some(record) => Some(super::project_note::ledger(record)),
            None => None,
        };
        let overrides = match core.template_overrides.get(&role.project) {
            Some(entries) => entries.as_ref(),
            None => &[],
        };
        let merged = crate::templates::current_merged(&core.current, overrides);
        let mut templates = List::with_capacity(limits.templates);
        for template in merged {
            templates.push(template).expect("configured template bound");
        }
        projects
            .push(FrameProjectRole {
                templates,
                project: role.project,
                role: role.role,
                name: text(
                    core.names.get(&role.project).expect("project role has a kept name").as_bytes(),
                    limits.documents.text_bytes,
                ),
                pool,
                creating: match crate::acts::bounds::creating(core, limits, party, role.project, Some(role.role)) {
                    crate::acts::Admission::Bounded { within, beyond } => {
                        Some(crate::acts::Creating { within, beyond })
                    }
                    crate::acts::Admission::Refused(_) => None,
                    crate::acts::Admission::Allowed | crate::acts::Admission::Proposed { .. } => {
                        unreachable!("creating is bounded or refused")
                    }
                },
                models: match core.authority.models(role.project, role.role.number()) {
                    Some(models) => models,
                    None => jig_domain_authority::Models::NONE,
                },
            })
            .expect("bounded project list");
    }
    let counts = core.people.counts(&limits.people, party).ok_or(Missing::Unknown)?;
    let updates = if counts.more_unread { Count::AtLeast(counts.unread) } else { Count::Known(counts.unread) };
    let (providers, providers_cut) = super::catalogue::offers(core, &limits.documents)?;
    Ok(Frame {
        providers,
        providers_cut,
        key_bytes: core.settings.key_bytes,
        redirect_bytes: core.settings.redirect_bytes,
        party,
        kind: core.people.kind(party).ok_or(Missing::Unknown)?,
        login: text(&identity.login, limits.documents.text_bytes),
        name: text(&identity.name, limits.documents.text_bytes),
        deployment_role: core.people.role(party, people::DEPLOYMENT),
        cut: projects.len() < roles.len(),
        projects,
        needs: counts.needs.checked_add(core.settings_needs(party)).ok_or(Missing::TooLarge)?,
        need_counts: core.party_need_counts(party).ok_or(Missing::Unknown)?,
        updates,
        results_seen: core.people.results_seen(party).ok_or(Missing::Unknown)?,
        retention: limits.people.request_retention,
    })
}

pub(super) fn run(core: &Core, limits: &Limits, party: u64, number: u64) -> Result<Run, Missing> {
    visible(core, party, number)?;
    let proof = match core.proofs.get(&number) {
        Some(proof) if proof.terminal.is_none() => proof,
        Some(_) | None => return Ok(Run { task: number, attempt: None, committed_turn: 0, writing: None, call: None }),
    };
    let committed_turn = match proof.turn {
        Some(turn) => turn.turn,
        None => 0,
    };
    let writing =
        match jig_domain_views::writing(&core.views, skein_lib::Token::new(number), limits.documents.text_bytes) {
            Some(writing) if writing.turn.attempt == proof.attempt && writing.turn.turn > committed_turn => {
                Some(writing)
            }
            Some(_) | None => None,
        };
    let call = jig_domain_views::current_call(
        &core.views,
        skein_lib::Token::new(number),
        proof.attempt,
        limits.documents.text_bytes,
    );
    Ok(Run { task: number, attempt: Some(proof.attempt), committed_turn, writing, call })
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_text_cut_inside_a_character_keeps_it_out_and_says_it_was_cut() {
        for (maximum, expected, cut) in [(0, "", true), (1, "a", true), (2, "a", true), (3, "aé", false)] {
            let actual = super::text("aé".as_bytes(), maximum);
            assert_eq!(actual.bytes.as_ref(), expected.as_bytes(), "bound {maximum}");
            assert_eq!(actual.cut, cut, "bound {maximum}");
        }
    }
}
