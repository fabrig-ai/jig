//! Ordered intervals for domain loads (protocol/store.md, 4.3, 7.1 and 7.2).
use crate::keys;
use alloc::boxed::Box;
use jig_domain_core::{self as core, RestoreRange};
use jig_domain_notes as notes;
use skein_kv::{KeyWriter, Range};
use skein_lib::List;

/// Concatenate a restart request's intervals in the order walked (store.md, 7.2).
#[must_use]
pub fn intervals(ranges: &[RestoreRange]) -> Option<Box<[Range]>> {
    let mut count = 0_u32;
    for range in ranges {
        count = count.checked_add(u32::try_from(restore(range).len()).ok()?)?;
    }
    let mut all = List::with_capacity(count);
    for range in ranges {
        for interval in restore(range) {
            all.push(interval).expect("counted restart intervals");
        }
    }
    Some(all.into_boxed())
}

/// A key's place in the restart walk, from the walk's own intervals (store.md, 7.2).
/// A key outside the requested intervals has no cursor.
#[must_use]
pub fn cursor(ranges: &[RestoreRange], key: &[u8]) -> Option<crate::Cursor> {
    for (index, interval) in intervals(ranges)?.iter().enumerate() {
        if key >= interval.start.as_ref()
            && match &interval.end {
                Some(end) => key < end.as_ref(),
                None => true,
            }
        {
            return Some(crate::Cursor { interval: u32::try_from(index).ok()?, after: Box::from(key) });
        }
    }
    None
}

/// The restart hands each live family up in the core's declared order.
#[must_use]
pub fn restore(range: &RestoreRange) -> Box<[Range]> {
    Box::new([match range {
        RestoreRange::Settings => Range::prefix(&[keys::JIG, 0, 8]),
        RestoreRange::Deployment => Range::prefix(&[keys::JIG, 0, 0]),
        // Issued names (4), then counter/accounts/sources (0 to 2); skip the installation (3).
        RestoreRange::Accounts => {
            return Box::new([
                Range::prefix(&[keys::JIG, keys::ACCOUNTS, 4]),
                Range {
                    start: Box::new([keys::JIG, keys::ACCOUNTS, 0]),
                    end: Some(Box::new([keys::JIG, keys::ACCOUNTS, 3])),
                },
            ]);
        }
        RestoreRange::People => {
            return Box::new([
                Range { start: Box::new([keys::JIG, 2, 0]), end: Some(Box::new([keys::JIG, 2, 7])) },
                Range::prefix(&[keys::JIG, 2, 8]),
            ]);
        }
        RestoreRange::Tasks => {
            return Box::new([
                Range { start: Box::new([keys::JIG, 1, 0]), end: Some(Box::new([keys::JIG, 1, 6])) },
                Range::prefix(&[keys::JIG, 1, 21]),
            ]);
        }
        RestoreRange::Core => Range { start: Box::new([keys::JIG, 0, 1]), end: Some(Box::new([keys::JIG, 0, 4])) },
        RestoreRange::Census(census) => return census_ranges(census),
    }])
}

/// The census reads the ended-read family whole, without selecting rows.
pub(crate) fn ended_reads() -> Range {
    Range::prefix(&[keys::JIG, 2, 7])
}

fn census_ranges(census: &core::Census) -> Box<[Range]> {
    let capacity = u32::try_from(census.results.len())
        .expect("bounded census parties")
        .checked_add(1)
        .expect("bounded census parties")
        .checked_add(u32::try_from(census.questions.len()).expect("bounded census questions"))
        .expect("bounded census parties, questions and conversation family");
    let mut ranges = List::with_capacity(capacity);
    ranges.push(ended_reads()).expect("one conversation family");
    for read in &census.results {
        let mut range = numbered_prefix(&[keys::JIG, 1, 13], read.party);
        let key = core::Key::Tasks(jig_domain_tasks::Key::Result { party: read.party, position: read.after });
        range.start = after(&encoded(&key));
        ranges.push(range).expect("one result interval per census party");
    }
    for &(task, position) in &census.questions {
        ranges
            .push(point(&core::Key::Tasks(jig_domain_tasks::Key::Activity { task, position })))
            .expect("one point per open question");
    }
    ranges.into_boxed()
}

/// One globally named entry or the ordered lines of one exact scope.
#[must_use]
pub fn notes(range: &notes::Range) -> Box<[Range]> {
    Box::new([match range {
        notes::Range::Entry { name } => point(&core::Key::Notes(notes::Key::Entry { name: *name })),
        notes::Range::Lines { scope, after: cursor } => {
            let first = encoded(&core::Key::Notes(notes::Key::Line { scope: scope.clone(), name: 0 }));
            let length = first.len().checked_sub(8).expect("the line's last field is its u64 name");
            let prefix = first.get(..length).expect("remove only the final name field");
            let mut range = Range::prefix(prefix);
            if let Some(name) = cursor {
                let key = encoded(&core::Key::Notes(notes::Key::Line { scope: scope.clone(), name: *name }));
                range.start = after(&key);
            }
            range
        }
    }])
}
fn encoded(key: &core::Key) -> Box<[u8]> {
    keys::encode(key, u32::MAX).expect("the domain's bounded key fits a u32 length")
}
pub(crate) fn point(key: &core::Key) -> Range {
    let start = encoded(key);
    let end = after(&start);
    Range { start, end: Some(end) }
}
fn after(key: &[u8]) -> Box<[u8]> {
    let mut writer = KeyWriter::new(u32::MAX);
    for &byte in key {
        writer.tag(byte);
    }
    writer.tag(0);
    writer.finish().expect("the domain's bounded key and one cursor byte fit a u32 length")
}

/// Scopes served from fixed keys and indexes; other scopes need their owner's record.
#[must_use]
pub fn scope(party: u64, scope: &core::reads::Scope) -> Option<Box<[Range]>> {
    use core::reads::Scope;
    Some(match scope {
        Scope::SettingsHistory { first, last, newest, .. } => {
            let mut ranges = List::with_capacity(2);
            if first < newest {
                ranges
                    .push(Range {
                        start: encoded(&core::Key::Core(core::CoreKey::Settings(core::settings::Key::History(*first)))),
                        end: point(&core::Key::Core(core::CoreKey::Settings(core::settings::Key::History(
                            (*last).min(newest.saturating_sub(1)),
                        ))))
                        .end,
                    })
                    .expect("history interval");
            }
            ranges
                .push(Range {
                    start: encoded(&core::Key::Core(core::CoreKey::Settings(core::settings::Key::Held(*first)))),
                    end: point(&core::Key::Core(core::CoreKey::Settings(core::settings::Key::Held(*last)))).end,
                })
                .expect("held interval");
            ranges.into_boxed()
        }
        Scope::Task { task } | Scope::PersonTask { task } | Scope::Result { task } => Box::new([
            point(&core::Key::Tasks(jig_domain_tasks::Key::Live(*task))),
            point(&core::Key::Tasks(jig_domain_tasks::Key::Ended(*task))),
        ]),
        Scope::Reply { task, activity } => {
            Box::new([point(&core::Key::Tasks(jig_domain_tasks::Key::Activity { task: *task, position: *activity }))])
        }
        Scope::History { task, subtree: false } => {
            Box::new([numbered_prefix(&[keys::JIG, 1, 9], *task), numbered_prefix(&[keys::JIG, 1, 10], *task)])
        }
        Scope::Proposal { proposer: jig_domain_people::Proposer::Party(owner), proposal } => {
            if *owner == party {
                Box::new([
                    point(&core::Key::Tasks(jig_domain_tasks::Key::PersonProposal(*proposal))),
                    point(&core::Key::Tasks(jig_domain_tasks::Key::DecidedProposal(*proposal))),
                ])
            } else {
                Box::new([])
            }
        }
        Scope::EndedDelegates { task } => Box::new([numbered_prefix(&[keys::JIG, 1, 17], *task)]),
        Scope::Tasks { query } => ended_tasks(query),
        Scope::Spend { project } => Box::new([
            numbered_prefix(&[keys::JIG, 1, 2, 3], u64::from(*project)),
            numbered_prefix(&[keys::JIG, 1, 7, 3], u64::from(*project)),
        ]),
        Scope::Notes { project } => {
            let project_scope = numbered_prefix(&[keys::JIG, 3, 1, 1], u64::from(*project));
            let goals = numbered_prefix(&[keys::JIG, 3, 1, 2], u64::from(*project));
            let resources = numbered_prefix(&[keys::JIG, 3, 1, 3], u64::from(*project));
            if *project == 0 {
                Box::new([Range::prefix(&[keys::JIG, 3, 1, 0]), project_scope, goals, resources])
            } else {
                Box::new([project_scope, goals, resources])
            }
        }
        Scope::ScopedNotes { scope, .. } => notes(&notes::Range::Lines { scope: scope.clone(), after: None }),
        Scope::Inbox { group, .. } => match group {
            core::reads::Group::Updates | core::reads::Group::Results | core::reads::Group::Replies => {
                Box::new([numbered_prefix(&[keys::JIG, 1, 12], party)])
            }
            core::reads::Group::Needs
            | core::reads::Group::Proposals
            | core::reads::Group::Escalations
            | core::reads::Group::Questions
            | core::reads::Group::PersonTasks => return None,
        },
        Scope::Proposal { proposer: jig_domain_people::Proposer::Task(_), .. }
        | Scope::Escalation { .. }
        | Scope::Question { .. }
        | Scope::Turn { .. }
        | Scope::History { subtree: true, .. }
        | Scope::Activity { .. } => return None,
    })
}
fn numbered_prefix(prefix: &[u8], number: u64) -> Range {
    let mut writer = KeyWriter::new(u32::MAX);
    for &byte in prefix {
        writer.tag(byte);
    }
    writer.u64(number);
    let bytes = writer.finish().expect("fixed-width family prefix fits a u32 length");
    Range::prefix(&bytes)
}

/// Direct delegate edges used to collect one bounded subtree (protocol/store.md, 7.3).
#[must_use]
pub fn delegates(requester: u64) -> Range {
    numbered_prefix(&[keys::JIG, 1, 18], requester)
}

fn ended_tasks(query: &core::documents::TaskQuery) -> Box<[Range]> {
    use core::reads::ended::{EndedPart, ended_part};
    let (tag, owner, label) = match ended_part(query) {
        EndedPart::ByParty { party, label } => (19, party, label),
        EndedPart::Delegates { requester } => return Box::new([numbered_prefix(&[keys::JIG, 1, 17], requester)]),
        EndedPart::ByProject { project, label } => (20, u64::from(project), label),
        EndedPart::None => return Box::new([]),
    };
    let mut writer = KeyWriter::new(u32::MAX);
    for byte in [keys::JIG, 1, tag] {
        writer.tag(byte);
    }
    writer.u64(owner);
    if let Some(label) = label {
        writer.u64(u64::from(label));
    }
    Box::new([Range::prefix(&writer.finish().expect("fixed-width ended prefix"))])
}
