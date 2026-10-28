//! Whole selected page rows and their owner continuations (protocol/client.md, 3 and 6).
use super::{cursor, w};
use alloc::boxed::Box;
use jig_domain_core::reads;
mod documents;
mod rows;

pub fn page(
    models: &[crate::Model],
    read: &reads::Read,
    page: &reads::Page,
    limits: &w::Limits,
) -> Result<w::ReadValue, w::Problem> {
    let invalid = w::Problem { path: w::Path::ReadValueTag, reason: skein_codec::Reason::Tag };
    check_edges(read, page)?;
    let before = match &page.before {
        Some(value) => Some(wire_cursor(value, limits)?),
        None => None,
    };
    let after = match &page.next {
        Some(value) => Some(wire_cursor(value, limits)?),
        None => None,
    };
    Ok(match read {
        reads::Read::Activity { task, .. } => documents::activity(*task, page, before, after, limits)?,
        reads::Read::History { task, subtree, .. } => {
            documents::history(models, *task, *subtree, page, before, after, limits)?
        }
        reads::Read::Inbox { .. } => documents::inbox(page, before, after, limits)?,
        reads::Read::EndedDelegates { .. } => documents::ended(page, before, after, limits)?,
        reads::Read::Spend { project, .. } => documents::spend(*project, page, before, after, limits)?,
        reads::Read::Notes { .. } | reads::Read::ScopedNotes { .. } => {
            documents::notes(models, page, before, after, limits)?
        }
        reads::Read::Holds { .. }
        | reads::Read::Tasks { .. }
        | reads::Read::Application { .. }
        | reads::Read::Frame
        | reads::Read::Task { .. }
        | reads::Read::Proposal { .. }
        | reads::Read::Escalation { .. }
        | reads::Read::Question { .. }
        | reads::Read::PersonTask { .. }
        | reads::Read::Result { .. }
        | reads::Read::Reply { .. }
        | reads::Read::Turn { .. }
        | reads::Read::Project { .. }
        | reads::Read::Settings
        | reads::Read::Setting { .. }
        | reads::Read::Revision { .. }
        | reads::Read::Revisions { .. }
        | reads::Read::Deployment => return Err(invalid),
    })
}

fn check_edges(read: &reads::Read, page: &reads::Page) -> Result<(), w::Problem> {
    let invalid = w::Problem { path: w::Path::CursorValue, reason: skein_codec::Reason::Tag };
    for cursor in [&page.before, &page.next] {
        let Some(cursor) = cursor else { continue };
        let matches = match cursor {
            reads::Cursor::Live { scope, .. } | reads::Cursor::Store { scope, .. } => {
                let expected = match read {
                    reads::Read::Activity { task, .. } => reads::Scope::Activity { task: *task },
                    reads::Read::History { task, subtree, .. } => {
                        reads::Scope::History { task: *task, subtree: *subtree }
                    }
                    reads::Read::EndedDelegates { task, .. } => reads::Scope::EndedDelegates { task: *task },
                    reads::Read::Inbox { group, filter, .. } => reads::Scope::Inbox { group: *group, filter: *filter },
                    reads::Read::Tasks { query, .. } => reads::Scope::Tasks { query: *query },
                    reads::Read::Spend { project, .. } => reads::Scope::Spend { project: *project },
                    reads::Read::Notes { project, .. } => reads::Scope::Notes { project: *project },
                    reads::Read::ScopedNotes { scope, description, .. } => {
                        reads::Scope::ScopedNotes { scope: scope.clone(), description: description.clone() }
                    }
                    reads::Read::Holds { .. }
                    | reads::Read::Application { .. }
                    | reads::Read::Frame
                    | reads::Read::Task { .. }
                    | reads::Read::Proposal { .. }
                    | reads::Read::Escalation { .. }
                    | reads::Read::Question { .. }
                    | reads::Read::PersonTask { .. }
                    | reads::Read::Result { .. }
                    | reads::Read::Reply { .. }
                    | reads::Read::Turn { .. }
                    | reads::Read::Project { .. }
                    | reads::Read::Settings
                    | reads::Read::Setting { .. }
                    | reads::Read::Revision { .. }
                    | reads::Read::Revisions { .. }
                    | reads::Read::Deployment => return Err(invalid),
                };
                *scope == expected
            }
        };
        if !matches {
            return Err(invalid);
        }
    }
    Ok(())
}

fn wire_cursor(value: &reads::Cursor, limits: &w::Limits) -> Result<w::Cursor, w::Problem> {
    let bytes = cursor::encode(value, limits.cursor_value.min(w::CEILINGS.cursor_value))
        .ok_or(w::Problem { path: w::Path::CursorValue, reason: skein_codec::Reason::Bound })?;
    w::Cursor::new(limits, w::CursorParts { value: bytes })
}

pub(super) fn tasks(page: &reads::TaskPage, limits: &w::Limits) -> Result<w::ReadValue, w::Problem> {
    let ended = match &page.ended {
        reads::EndedGroup::None => w::EndedGroup::None,
        reads::EndedGroup::Rows { rows, next, cut } => w::EndedGroup::Rows(w::EndedRows::new(
            limits,
            w::EndedRowsParts {
                rows: task_pieces(rows, limits, limits.ended_rows_rows, w::Path::EndedRowsRows)?,
                after: match next {
                    Some(v) => Some(wire_cursor(v, limits)?),
                    None => None,
                },
                cut: *cut,
            },
        )?),
    };
    Ok(w::ReadValue::Tasks(w::TaskPage::new(
        limits,
        w::TaskPageParts {
            live: task_pieces(&page.live, limits, limits.task_page_live, w::Path::TaskPageLive)?,
            live_after: match &page.live_next {
                Some(v) => Some(wire_cursor(v, limits)?),
                None => None,
            },
            ended,
        },
    )?))
}
fn task_pieces(
    lines: &skein_lib::List<jig_domain_core::documents::TaskLine>,
    limits: &w::Limits,
    bound: u32,
    path: w::Path,
) -> Result<Box<[u8]>, w::Problem> {
    let mut pieces = skein_lib::List::with_capacity(lines.len());
    for line in lines {
        let line = super::queries::line(line, limits)?;
        let mut writer = skein_lib::Writer::new(usize::try_from(line.measure()).expect("measure fits"));
        line.encode(&mut writer).expect("measured line");
        pieces.push(writer.finish()).expect("same line count");
    }
    super::pieces(&pieces, bound, path)
}

pub fn holds(
    read: &reads::Read,
    page: &jig_domain_core::documents::ProjectHolds,
    limits: &w::Limits,
) -> Result<w::ReadValue, w::Problem> {
    let invalid = w::Problem { path: w::Path::ReadValueTag, reason: skein_codec::Reason::Tag };
    let reads::Read::Holds { project, .. } = read else { return Err(invalid) };
    if *project != page.project {
        return Err(invalid);
    }
    if let Some(after) = &page.after {
        let reads::Cursor::Live {
            scope: reads::Scope::Holds { project: owner },
            after: jig_domain_core::documents::Cursor::Number { number },
            store_after: None,
        } = after
        else {
            return Err(invalid);
        };
        if owner != project
            || match page.rows.last() {
                Some(row) => row.hold.number != *number,
                None => true,
            }
        {
            return Err(invalid);
        }
    }
    let mut rows = skein_lib::List::with_capacity(u32::try_from(page.rows.len()).expect("bounded rows"));
    for row in &page.rows {
        let mut exempt = skein_lib::List::with_capacity(u32::try_from(row.exempt.len()).expect("bounded exemptions"));
        for task in &row.exempt {
            exempt.push(*task).expect("same exemptions");
        }
        rows.push(w::HoldRow::new(
            limits,
            w::HoldRowParts { hold: super::chips::placed(&row.hold, limits)?, held_tasks: row.held, exempt },
        )?)
        .expect("same rows");
    }
    Ok(w::ReadValue::Holds(w::ProjectHolds::new(
        limits,
        w::ProjectHoldsParts {
            project: page.project,
            rows,
            after: match &page.after {
                Some(value) => Some(wire_cursor(value, limits)?),
                None => None,
            },
        },
    )?))
}
