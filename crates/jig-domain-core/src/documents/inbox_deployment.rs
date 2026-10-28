//! Party pages (`client/domain/pages.md`, sections 3 and 4;
//! `contract.md`, 5 and 7.1). Entries are bounded cache views;
//! needs count all live task projections across the party's projects. Reply
//! read positions and stored inbox rows are supplied by their owners.
use super::{Count, Cursor, Missing, Proposer, TaskLine, TaskLink, Window};
use crate::{Core, Limits};
use alloc::boxed::Box;
use jig_domain_people as people;
use skein_lib::{List, Map, Wall};

/// The object one inbox row opens; its own document carries content and acts.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum InboxObject {
    Question { message: u64 },
    Proposal { proposer: Proposer, number: u64 },
    Escalation { revision: u64 },
    PersonTask,
    Reply { message: u64 },
    Result { position: u64 },
}
/// One owner-supplied inbox reference, with a chat header when it is a reply.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct InboxEntry {
    pub reply: Option<Box<TaskLine>>,
    pub task: TaskLink,
    pub project: Option<u32>,
    pub object: InboxObject,
    /// Exact child activity to load, separate from a chat ordinal or notice message number.
    pub activity: Option<u64>,
    pub at: Option<Wall>,
    pub unread: Option<bool>,
    pub cursor: Cursor,
}
/// A bounded page across both groups. Needs precede updates. Known-time needs
/// are oldest first and updates newest first.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Inbox {
    pub party: u64,
    pub needs: List<InboxEntry>,
    pub updates: List<InboxEntry>,
    pub next: Option<Cursor>,
    /// Exact needs across every project the party currently holds a role in.
    pub cached_needs: u32,
    pub cached_updates: Count,
    pub results_seen: u64,
    pub updates_time_complete: bool,
    /// None is unsupplied; Some carries complete roots for each emitted row.
    pub selection: Option<List<super::InboxSelection>>,
}

pub(super) fn line_bound(limits: &Limits) -> usize {
    usize::try_from(limits.documents.lines).expect("line bound fits")
}
fn object(core: &Core, task: u64, kind: people::EntryKind) -> InboxObject {
    match kind {
        people::EntryKind::Question { message } => InboxObject::Question { message },
        people::EntryKind::Proposal { number } => {
            let proposer = match core.tasks.person_proposals().get(&number) {
                Some(row) if row.first() == task => Proposer::Party(row.proposer),
                Some(_) | None => Proposer::Task(task),
            };
            InboxObject::Proposal { proposer, number }
        }
        people::EntryKind::Escalation { revision } => InboxObject::Escalation { revision },
        people::EntryKind::PersonTask => InboxObject::PersonTask,
        people::EntryKind::Reply { message } | people::EntryKind::ChatReply { message } => {
            InboxObject::Reply { message }
        }
    }
}
fn updates(value: InboxObject) -> bool {
    match value {
        InboxObject::Reply { .. } | InboxObject::Result { .. } => true,
        InboxObject::Question { .. }
        | InboxObject::Proposal { .. }
        | InboxObject::Escalation { .. }
        | InboxObject::PersonTask => false,
    }
}
fn key(cursor: Cursor) -> Result<(bool, u64, u64, u64, InboxObject), Missing> {
    match cursor {
        Cursor::Inbox { task, object, at } => {
            let update = updates(object);
            let time = match at {
                Some(at) => at.as_nanos(),
                None => 0,
            };
            let order = match object {
                InboxObject::Result { position } => u64::MAX.checked_sub(position).expect("position fits"),
                InboxObject::Reply { message } => u64::MAX.checked_sub(message).expect("message fits"),
                InboxObject::Question { .. }
                | InboxObject::Proposal { .. }
                | InboxObject::Escalation { .. }
                | InboxObject::PersonTask => task,
            };
            Ok((
                update,
                if update { u64::MAX.checked_sub(time).expect("wall time fits") } else { time },
                order,
                task,
                object,
            ))
        }
        Cursor::Number { .. } => Err(Missing::Unknown),
    }
}
fn reply_activity(core: &Core, task: u64, object: InboxObject) -> Option<u64> {
    match object {
        InboxObject::Reply { message } => {
            let row = core.tasks.task(task)?;
            if core.conversation_requester(row).is_some() {
                for reply in &row.replies {
                    if reply.number == message {
                        return Some(reply.activity);
                    }
                }
            }
            let notice = row.person_wait?.notice?;
            if notice.message == message { Some(notice.activity) } else { None }
        }
        InboxObject::Question { .. }
        | InboxObject::Proposal { .. }
        | InboxObject::Escalation { .. }
        | InboxObject::PersonTask
        | InboxObject::Result { .. } => None,
    }
}

fn archived_conversation(core: &Core, limits: &Limits, party: u64, task: u64) -> Option<TaskLine> {
    let archive = core.archived_chats.get(&task)?;
    if archive.party != party || core.people.role(party, archive.project).is_none() {
        return None;
    }
    let mut line = archive.line.as_ref()?.clone();
    let conversation = line.conversation.as_mut()?;
    conversation.seen = core.people.conversation_seen(party, task)?;
    let unread = archive.last_reply.saturating_sub(conversation.seen);
    let capped = u32::try_from(unread.min(u64::from(limits.people.inbox_entries))).expect("bounded unread");
    conversation.unread =
        if unread > u64::from(limits.people.inbox_entries) { Count::AtLeast(capped) } else { Count::Known(capped) };
    Some(line)
}

fn inbox_reply(
    core: &Core,
    limits: &Limits,
    party: u64,
    task: u64,
    chat: bool,
) -> Result<Option<Box<TaskLine>>, Missing> {
    if !chat {
        return Ok(None);
    }
    match core.tasks.task(task) {
        Some(row) => Ok(Some(Box::new(super::task_line(core, limits, party, row, None)?))),
        None => match archived_conversation(core, limits, party, task) {
            Some(line) => Ok(Some(Box::new(line))),
            None => Ok(None),
        },
    }
}

#[expect(clippy::too_many_lines, reason = "one bounded projection preserves both groups and their parallel contexts")]
pub(super) fn inbox(core: &Core, limits: &Limits, party: u64, window: &Window) -> Result<Inbox, Missing> {
    super::inbox_selection_worst_case(window.rows, limits.tasks.depth).ok_or(Missing::TooLarge)?;
    let entries = core.people.inbox_needs(&limits.people, party).ok_or(Missing::Unknown)?;
    let recent = core.people.cached_updates(&limits.people, party).ok_or(Missing::Unknown)?;
    let mut ordered = Map::with_capacity(
        limits
            .people
            .inbox_entries
            .checked_mul(2)
            .ok_or(Missing::TooLarge)?
            .checked_add(limits.settings.proposals)
            .ok_or(Missing::TooLarge)?,
    );
    let cached_needs = core
        .people
        .needs(party)
        .ok_or(Missing::Unknown)?
        .checked_add(core.settings_needs(party))
        .ok_or(Missing::TooLarge)?;
    let counts = core.people.counts(&limits.people, party).ok_or(Missing::Unknown)?;
    if core.people.role(party, people::DEPLOYMENT) == Some(people::Role::Owner) {
        for (&(proposer, number), _) in &core.settings_proposals {
            let Proposer::Task(task) = proposer else { continue };
            let Some(row) = core.tasks.proposal(task, u64::from(number)) else { continue };
            let jig_domain_tasks::ProposalState::Pending { since, .. } = row.state else { continue };
            let object = InboxObject::Proposal { proposer, number: u64::from(number) };
            let cursor = Cursor::Inbox { task: jig_domain_tasks::PROPOSAL_WITHOUT_TASK, object, at: Some(since) };
            ordered
                .insert(
                    key(cursor)?,
                    (jig_domain_tasks::PROPOSAL_WITHOUT_TASK, 0, object, None, since, false, cursor, false),
                )
                .expect("bounded settings needs");
        }
        for (_, row) in core.tasks.person_proposals() {
            match row.action {
                jig_domain_tasks::PersonAction::Settings { .. } => {
                    let since = match row.state {
                        jig_domain_tasks::PersonProposalState::Pending { since } => since,
                        jig_domain_tasks::PersonProposalState::Accepted { .. }
                        | jig_domain_tasks::PersonProposalState::SettingsAccepted { .. }
                        | jig_domain_tasks::PersonProposalState::Rejected { .. }
                        | jig_domain_tasks::PersonProposalState::Withdrawn { .. } => continue,
                    };
                    let object = InboxObject::Proposal { proposer: Proposer::Party(row.proposer), number: row.number };
                    let cursor =
                        Cursor::Inbox { task: jig_domain_tasks::PROPOSAL_WITHOUT_TASK, object, at: Some(since) };
                    ordered
                        .insert(
                            key(cursor)?,
                            (jig_domain_tasks::PROPOSAL_WITHOUT_TASK, 0, object, None, since, false, cursor, false),
                        )
                        .expect("bounded settings needs");
                }
                jig_domain_tasks::PersonAction::Batch(_) | jig_domain_tasks::PersonAction::Amend { .. } => {}
            }
        }
    }
    for entry in &entries {
        if core.people.role(party, entry.project).is_none() {
            continue;
        }
        let value = object(core, entry.task, entry.kind);
        let cursor = Cursor::Inbox { task: entry.task, object: value, at: Some(entry.at) };
        ordered
            .insert(key(cursor)?, (entry.task, entry.project, value, None, entry.at, false, cursor, false))
            .expect("bounded need prefix");
    }
    for update in &recent {
        let (task, project, value, activity, at, chat) = match *update {
            people::Update::Result(row) => {
                (row.task, row.project, InboxObject::Result { position: row.position }, None, row.at, false)
            }
            people::Update::Reply(row) => {
                let value = object(core, row.task, row.kind);
                (row.task, row.project, value, reply_activity(core, row.task, value), row.at, false)
            }
            people::Update::ChatReply(row) => (
                row.entry.task,
                row.entry.project,
                object(core, row.entry.task, row.entry.kind),
                Some(row.activity),
                row.entry.at,
                true,
            ),
        };
        if core.people.role(party, project).is_none() {
            continue;
        }
        let cursor = Cursor::Inbox { task, object: value, at: Some(at) };
        ordered
            .insert(key(cursor)?, (task, project, value, activity, at, true, cursor, chat))
            .expect("bounded shared update prefix");
    }
    let after = match window.after {
        Some(cursor) => Some(key(cursor)?),
        None => None,
    };
    let mut needs = List::with_capacity(window.rows);
    let mut changes = List::with_capacity(window.rows);
    let mut selection = List::with_capacity(window.rows);
    let mut used = 0_u32;
    let mut last = None;
    let mut more = false;
    for (position, (task, project, object, activity, at, unread, cursor, chat)) in &ordered {
        if let Some(after) = after
            && *position <= after
        {
            continue;
        }
        if used == window.rows {
            more = true;
            break;
        }
        let reply = inbox_reply(core, limits, party, *task, *chat)?;
        if *chat && reply.is_none() {
            more = true;
            break;
        }
        let link = if *task == jig_domain_tasks::PROPOSAL_WITHOUT_TASK {
            TaskLink::Stored { number: jig_domain_tasks::PROPOSAL_WITHOUT_TASK, status: None }
        } else {
            match core.archived_chats.get(task) {
                Some(archive) if archive.party == party => {
                    TaskLink::Retained { chip: archive.chip.clone(), at: archive.at }
                }
                Some(_) | None => super::task_tree::link(core, limits, party, *task)?,
            }
        };
        let entry = InboxEntry {
            reply,
            task: link,
            project: Some(*project),
            object: *object,
            activity: *activity,
            at: Some(*at),
            unread: Some(*unread),
            cursor: *cursor,
        };
        let context = super::inbox_selection::live(core, limits, *task, *cursor)?;
        selection.push(context).expect("one context per bounded inbox row");
        if updates(*object) {
            changes.push(entry).expect("bounded updates");
        } else {
            needs.push(entry).expect("bounded needs");
        }
        used = used.checked_add(1).expect("bounded window");
        last = Some(*cursor);
    }
    Ok(Inbox {
        party,
        needs,
        updates: changes,
        next: if more { last } else { None },
        cached_needs,
        cached_updates: if counts.more_unread { Count::AtLeast(counts.unread) } else { Count::Known(counts.unread) },
        results_seen: core.people.results_seen(party).ok_or(Missing::Unknown)?,
        updates_time_complete: true,
        selection: Some(selection),
    })
}
