//! Task-derived inbox census, one shared recent prefix, and explicit read marks
//! (`domain/people.md`, section 6). Owner rows rebuild volatile counts after
//! positions restore. Ended conversations retain positions until the owner forgets them.
#[path = "inbox/moving.rs"]
mod moving;
pub(super) use moving::move_chat;

use super::{ChatTotal, Domain, Need, Pending, Phase, Recent, decided, role};
use crate::{
    Again, Ask, Counts, Entry, EntryKind, Key, Limits, Needs, Outcome, ReplyRef, Request, ResultRef, Role, Seen,
    Stored, Update, Whom,
};
use alloc::boxed::Box;
use skein_lib::{Env, Id, List, Map, Queue, Token, Wall};

impl Domain {
    /// The person who must answer this received question (domain/people.md, 6).
    #[must_use]
    pub fn question_person(&self, task: u64, question: u64) -> Option<u64> {
        let needs = self.needs.get(&task)?;
        for need in needs {
            if need.kind == (EntryKind::Question { message: question }) {
                match need.whom {
                    Whom::Person(person) => return Some(person),
                    Whom::Role { .. } => {}
                }
            }
        }
        None
    }

    /// Last explicitly accepted results position for a known party, zero before any mark.
    #[must_use]
    pub fn results_seen(&self, person: u64) -> Option<u64> {
        if !self.ready() || !self.people.contains_key(&person) {
            return None;
        }
        Some(*self.results_seen.get(&person).unwrap_or(&0))
    }

    /// Currently cached unread result references, newest first; missing older entries are loaded from ended tasks.
    #[must_use]
    pub fn cached_results(&self, person: u64) -> Option<&[ResultRef]> {
        if !self.ready() || !self.people.contains_key(&person) {
            return None;
        }
        match self.unread.get(&person) {
            Some(entries) => Some(entries.as_ref()),
            None => Some(&[]),
        }
    }

    /// Bounded newest-first task-derived references for a current person and exact project roles.
    /// Older references are obtained by the root from stored task rows.
    #[must_use]
    pub fn cached_waiting(&self, limits: &Limits, person: u64) -> Option<Box<[Entry]>> {
        if !self.ready() || !self.people.contains_key(&person) {
            return None;
        }
        let mut visible = List::with_capacity(limits.inbox_entries);
        if let Some(entries) = self.waiting.get(&Whom::Person(person)) {
            for &entry in entries {
                match entry.kind {
                    EntryKind::Reply { .. } | EntryKind::ChatReply { .. } if !unread_reply(self, person, entry) => {
                        continue;
                    }
                    EntryKind::Reply { .. }
                    | EntryKind::ChatReply { .. }
                    | EntryKind::Proposal { .. }
                    | EntryKind::Escalation { .. }
                    | EntryKind::Question { .. }
                    | EntryKind::PersonTask => {}
                }
                insert_newest(&mut visible, entry);
            }
        }
        for (project, holdings) in &self.roles {
            for holding in holdings {
                if holding.person == person
                    && let Some(entries) =
                        self.waiting.get(&Whom::Role { project: *project, role: role_number(holding.role) })
                {
                    for &entry in entries {
                        insert_newest(&mut visible, entry);
                    }
                }
            }
        }
        Some(visible.into_boxed())
    }

    /// The globally oldest live needs, selected before the visible entry limit.
    #[must_use]
    pub fn inbox_needs(&self, limits: &Limits, person: u64) -> Option<Box<[Entry]>> {
        self.identity(person)?;
        let mut entries = List::with_capacity(limits.inbox_entries);
        for (task, needs) in &self.needs {
            for &need in needs {
                if addressed(self, person, need) {
                    insert_oldest(
                        &mut entries,
                        Entry { task: *task, project: need.project, whom: need.whom, kind: need.kind, at: need.at },
                    );
                }
            }
        }
        Some(entries.into_boxed())
    }

    /// The newest still relevant updates from one global prefix. Seen entries keep
    /// their places internally so selective marks and backward wall changes cannot
    /// expose an older reference ahead of an omitted store row.
    #[must_use]
    pub fn cached_updates(&self, limits: &Limits, person: u64) -> Option<Box<[Update]>> {
        self.identity(person)?;
        let mut result = List::with_capacity(limits.inbox_entries);
        if let Some(recent) = self.recent.get(&person) {
            for &item in recent {
                if item.gone {
                    continue;
                }
                let visible = match item.update {
                    Update::Result(row) => row.position > *self.results_seen.get(&person).unwrap_or(&0),
                    Update::ChatReply(reply) => unread_reply(self, person, reply.entry),
                    Update::Reply(entry) => {
                        let mut present = false;
                        if let Some(entries) = self.replies.get(&entry.task) {
                            for old in entries {
                                present |= *old == entry;
                            }
                        }
                        present && unread_reply(self, person, entry)
                    }
                };
                if visible {
                    result.push(item.update).expect("recent prefix subset");
                }
            }
        }
        Some(result.into_boxed())
    }

    /// Exact live needs across the party's current projects and roles, including uncached entries.
    /// Tallies are volatile projections rebuilt from task rows, never durable inbox state.
    #[must_use]
    pub fn needs(&self, person: u64) -> Option<u32> {
        let counts = self.need_counts(person)?;
        let total = counts.questions.checked_add(counts.proposals).expect("validated task projection bound");
        let total = total.checked_add(counts.escalations).expect("validated task projection bound");
        Some(total.checked_add(counts.person_tasks).expect("validated task projection bound"))
    }

    /// Every live addressed need by kind, independent of the recent cache's capacity.
    #[must_use]
    pub fn need_counts(&self, person: u64) -> Option<Needs> {
        self.identity(person)?;
        let mut counts = Needs { questions: 0, proposals: 0, escalations: 0, person_tasks: 0 };
        for (_, tallies) in &self.needs {
            for tally in tallies {
                if !addressed(self, person, *tally) {
                    continue;
                }
                let count = match tally.kind {
                    EntryKind::Question { .. } => &mut counts.questions,
                    EntryKind::Proposal { .. } => &mut counts.proposals,
                    EntryKind::Escalation { .. } => &mut counts.escalations,
                    EntryKind::PersonTask => &mut counts.person_tasks,
                    EntryKind::Reply { .. } | EntryKind::ChatReply { .. } => continue,
                };
                *count = count.checked_add(1).expect("validated task projection bound");
            }
        }
        Some(counts)
    }

    /// Whether this task currently contributes to the party's exact need count.
    #[must_use]
    pub fn task_needs(&self, person: u64, task: u64) -> bool {
        match self.needs.get(&task) {
            Some(tallies) => {
                for tally in tallies {
                    if addressed(self, person, *tally) {
                        return true;
                    }
                }
                false
            }
            None => false,
        }
    }

    /// Requester represented by the live conversation projection.
    #[must_use]
    pub fn chat_owner(&self, task: u64) -> Option<u64> {
        let chat = self.chat_totals.get(&task)?;
        Some(chat.party)
    }

    /// Live read positions for the core's join after its task census (engine.md, 6).
    #[must_use]
    pub fn read_conversations(&self) -> &Map<(u64, u64), u64> {
        &self.conversations_seen
    }

    /// An ended owner witnessed by the core's census resolves any leftover live
    /// positions. Ended positions are loaded separately, never kept as live ones.
    pub fn restore_ended_conversation(&mut self, task: u64) {
        loop {
            let mut resolved = None;
            for ((party, conversation), _) in &self.conversations_seen {
                if *conversation == task {
                    resolved = Some((*party, *conversation));
                    break;
                }
            }
            let Some(key) = resolved else { break };
            let _old = self.conversations_seen.remove(&key);
        }
    }

    /// Last position explicitly marked seen over one conversation, zero before any mark.
    #[must_use]
    pub fn conversation_seen(&self, person: u64, task: u64) -> Option<u64> {
        self.identity(person)?;
        Some(*self.conversations_seen.get(&(person, task)).unwrap_or(&0))
    }

    /// Exact reply count of a live chat, capped for its conversation line.
    #[must_use]
    pub fn chat_unread(&self, limits: &Limits, person: u64, task: u64) -> Option<Counts> {
        self.identity(person)?;
        let last_reply = match self.chat_totals.get(&task) {
            Some(chat) if chat.party == person => chat.last_reply,
            Some(_) | None => 0,
        };
        let unseen = last_reply.saturating_sub(self.conversation_seen(person, task)?);
        Some(Counts {
            needs: 0,
            unread: u32::try_from(unseen.min(u64::from(limits.inbox_entries))).expect("capped reply count"),
            more_unread: unseen > u64::from(limits.inbox_entries),
        })
    }

    /// Recover the authenticated owner and exact scope of one still pending keyed mark.
    #[must_use]
    pub fn seen_context(&self, request: Token) -> Option<(u64, Seen)> {
        let flight = self.pending.get(Id::<Pending>::from_token(request))?.party()?;
        if let Ask::MarkSeen { seen } = flight.ask { Some((flight.key.person, seen)) } else { None }
    }

    /// Exact needs and updates counted up to `inbox_entries`, independently of visible cache room.
    #[must_use]
    pub fn counts(&self, limits: &Limits, person: u64) -> Option<Counts> {
        let needs = self.needs(person)?;
        let mut unread = 0_u128;
        for ((owner, project), count) in &self.chat_updates {
            if *owner == person && role(self, person, *project).is_some() {
                unread = unread.checked_add(*count).expect("issued reply census");
            }
        }
        for ((owner, project), refs) in &self.result_census {
            if *owner == person && role(self, person, *project).is_some() {
                unread = unread
                    .checked_add(u128::try_from(refs.len()).expect("bounded results"))
                    .expect("bounded result census");
            }
        }
        for (_, entries) in &self.replies {
            for entry in entries {
                if matches_notice(entry.kind) && unread_reply(self, person, *entry) {
                    unread = unread.checked_add(1).expect("bounded notice counts");
                }
            }
        }
        Some(Counts {
            needs,
            unread: u32::try_from(unread.min(u128::from(limits.inbox_entries))).expect("capped updates"),
            more_unread: unread > u128::from(limits.inbox_entries),
        })
    }

    /// Keep the newest `inbox_entries` plus one unread result witness; the store pages the rest.
    pub fn remember_result(&mut self, limits: &Limits, person: u64, entry: ResultRef) {
        if !self.ready()
            || !self.people.contains_key(&person)
            || !self.has_project(entry.project)
            || entry.position <= self.results_seen(person).unwrap_or(0)
        {
            return;
        }
        remember_project_result(self, limits, person, entry);
        let cap = limits.inbox_entries.checked_add(1).expect("bounded unread census");
        if cap == 0 {
            return;
        }
        let mut entries = List::with_capacity(cap);
        if let Some(old) = self.unread.get(&person) {
            for cached in old {
                if cached.task == entry.task || cached.position == entry.position {
                    return;
                }
            }
            let mut inserted = false;
            for &cached in old {
                if !inserted && entry.position > cached.position {
                    entries.push(entry).expect("new entry fits before a cached entry");
                    inserted = true;
                }
                if entries.room() > 0 {
                    entries.push(cached).expect("cache has room");
                }
            }
            if !inserted && entries.room() > 0 {
                entries.push(entry).expect("cache has room for latest entry");
            }
        } else {
            entries.push(entry).expect("nonzero cache bound");
        }
        let saved = self.unread.insert(person, entries.into_boxed());
        assert!(saved.is_ok(), "one cache per admitted person");
        cache_update(self, limits, person, Update::Result(entry));
    }
}

fn role_number(role: Role) -> u32 {
    role.number()
}

fn newer(left: Entry, right: Entry) -> bool {
    left.at > right.at || (left.at == right.at && (left.task, left.kind) > (right.task, right.kind))
}

fn insert_newest(entries: &mut List<Entry>, entry: Entry) {
    if entries.capacity() == 0 {
        return;
    }
    let mut next = List::with_capacity(entries.capacity());
    let mut inserted = false;
    for &old in &*entries {
        if old.task == entry.task && old.kind == entry.kind {
            return;
        }
        if !inserted && newer(entry, old) {
            next.push(entry).expect("cache has room before older entry");
            inserted = true;
        }
        if next.room() > 0 {
            next.push(old).expect("cache has room for prior entry");
        }
    }
    if !inserted && next.room() > 0 {
        next.push(entry).expect("cache has room for new entry");
    }
    *entries = next;
}

fn insert_oldest(entries: &mut List<Entry>, entry: Entry) {
    if entries.capacity() == 0 {
        return;
    }
    let mut next = List::with_capacity(entries.capacity());
    let mut inserted = false;
    for &old in &*entries {
        if old.task == entry.task && old.kind == entry.kind {
            return;
        }
        if !inserted && newer(old, entry) {
            next.push(entry).expect("cache has room before older entry");
            inserted = true;
        }
        if next.room() > 0 {
            next.push(old).expect("cache has room for prior entry");
        }
    }
    if !inserted && next.room() > 0 {
        next.push(entry).expect("cache has room for new entry");
    }
    *entries = next;
}

fn update_key(update: Update) -> (Wall, u64, u64, u8) {
    let (task, at, position, kind) = match update {
        Update::Result(row) => (row.task, row.at, row.position, 0),
        Update::Reply(row) => match row.kind {
            EntryKind::Reply { message } => (row.task, row.at, message, 1),
            EntryKind::ChatReply { .. }
            | EntryKind::Proposal { .. }
            | EntryKind::Escalation { .. }
            | EntryKind::Question { .. }
            | EntryKind::PersonTask => unreachable!("notice update kind"),
        },
        Update::ChatReply(reply) => match reply.entry.kind {
            EntryKind::ChatReply { message } => (reply.entry.task, reply.entry.at, message, 1),
            EntryKind::Reply { .. }
            | EntryKind::Proposal { .. }
            | EntryKind::Escalation { .. }
            | EntryKind::Question { .. }
            | EntryKind::PersonTask => unreachable!("chat update kind"),
        },
    };
    (at, position, u64::MAX.checked_sub(task).expect("task identity fits"), kind)
}
fn cache_update(domain: &mut Domain, limits: &Limits, party: u64, update: Update) {
    if limits.inbox_entries == 0 || !domain.people.contains_key(&party) {
        return;
    }
    let mut result = List::with_capacity(limits.inbox_entries);
    let mut inserted = false;
    if let Some(old) = domain.recent.get(&party) {
        for &item in old {
            if item.update == update {
                return;
            }
            if !inserted && update_key(update) > update_key(item.update) {
                result.push(Recent { update, gone: false }).expect("room before older reference");
                inserted = true;
            }
            if result.room() > 0 {
                result.push(item).expect("bounded prior reference");
            }
        }
    }
    if !inserted && result.room() > 0 {
        result.push(Recent { update, gone: false }).expect("recent prefix room");
    }
    assert!(domain.recent.insert(party, result.into_boxed()).is_ok(), "one recent prefix per known party");
}

fn matches_notice(kind: EntryKind) -> bool {
    match kind {
        EntryKind::Reply { .. } => true,
        EntryKind::ChatReply { .. }
        | EntryKind::Proposal { .. }
        | EntryKind::Escalation { .. }
        | EntryKind::Question { .. }
        | EntryKind::PersonTask => false,
    }
}

struct State {
    party: u64,
    project: u32,
    task: u64,
    last_reply: u64,
    live: bool,
    seen: Option<u64>,
}
fn chat_state(domain: &mut Domain, state: State) {
    let State { party, project, task, last_reply, live, seen } = state;
    if !domain.ready() || !domain.people.contains_key(&party) || !domain.has_project(project) {
        return;
    }
    let seen = seen.unwrap_or(*domain.conversations_seen.get(&(party, task)).unwrap_or(&0));
    let old = match domain.chat_totals.get(&task) {
        Some(old) => {
            assert!(
                old.party == party && old.project == project && old.last_reply <= last_reply,
                "immutable chat owner and monotone replies"
            );
            old.last_reply
        }
        None => 0,
    };
    let delta = last_reply.saturating_sub(seen).checked_sub(old.saturating_sub(seen)).expect("reply count is monotone");
    let count = *domain.chat_updates.get(&(party, project)).unwrap_or(&0);
    assert!(
        domain
            .chat_updates
            .insert((party, project), count.checked_add(u128::from(delta)).expect("issued attempts bound all replies"))
            .is_ok(),
        "one reply count per party"
    );
    if live {
        assert!(
            domain.chat_totals.insert(task, ChatTotal { party, project, last_reply }).is_ok(),
            "one live chat header per task"
        );
    } else {
        let _old = domain.chat_totals.remove(&task);
    }
}

pub(super) fn conversation_seen_decision(
    domain: &mut Domain,
    env: &Env<Limits>,
    request: Token,
    project: u32,
    last_reply: u64,
    out: &mut Queue<Request>,
) {
    let Some((party, seen @ Seen::Conversation { task, through })) = domain.seen_context(request) else { return };
    if !domain.conversations_seen.contains_key(&(party, task))
        && domain.conversations_seen.len() == domain.conversations_seen.capacity()
        && through != 0
    {
        return decided(domain, env, Id::<Pending>::from_token(request), Outcome::Again(Again::Busy), out);
    }
    let old = *domain.conversations_seen.get(&(party, task)).unwrap_or(&0);
    let delta = last_reply
        .saturating_sub(old)
        .checked_sub(last_reply.saturating_sub(old.max(through)))
        .expect("a mark only decreases unread");
    let count = *domain.chat_updates.get(&(party, project)).unwrap_or(&0);
    assert!(
        domain
            .chat_updates
            .insert(
                (party, project),
                count.checked_sub(u128::from(delta)).expect("complete chat census precedes marks")
            )
            .is_ok(),
        "known party aggregate"
    );
    decided(domain, env, Id::<Pending>::from_token(request), Outcome::MarkedSeen { seen }, out);
}

fn unread_reply(domain: &Domain, person: u64, entry: Entry) -> bool {
    let (message, seen) = match entry.kind {
        EntryKind::Reply { message } => (message, *domain.results_seen.get(&person).unwrap_or(&0)),
        EntryKind::ChatReply { message } => {
            (message, *domain.conversations_seen.get(&(person, entry.task)).unwrap_or(&0))
        }
        EntryKind::Proposal { .. }
        | EntryKind::Escalation { .. }
        | EntryKind::Question { .. }
        | EntryKind::PersonTask => return false,
    };
    entry.whom == Whom::Person(person) && role(domain, person, entry.project).is_some() && message > seen
}

pub(super) fn mark_seen(domain: &mut Domain, limits: &Limits, party: u64, seen: Seen, out: &mut Queue<Request>) {
    match seen {
        Seen::Results { through } => {
            if through <= *domain.results_seen.get(&party).unwrap_or(&0) {
                return;
            }
            assert!(domain.results_seen.insert(party, through).is_ok(), "one result position per party");
            if let Some(cached) = domain.unread.get(&party) {
                let mut kept = List::with_capacity(limits.inbox_entries.checked_add(1).expect("bounded census"));
                for &entry in cached {
                    if entry.position > through {
                        kept.push(entry).expect("cached subset");
                    }
                }
                assert!(domain.unread.insert(party, kept.into_boxed()).is_ok(), "admitted result cache");
            }
            let mut projects = List::with_capacity(domain.result_census.capacity());
            for ((owner, project), _) in &domain.result_census {
                if *owner == party {
                    projects.push(*project).expect("project census keys");
                }
            }
            for project in projects.into_boxed() {
                let mut kept = List::with_capacity(limits.inbox_entries.checked_add(1).expect("result witness"));
                for &entry in domain.result_census.get(&(party, project)).expect("snapshot key") {
                    if entry.position > through {
                        kept.push(entry).expect("result subset");
                    }
                }
                assert!(
                    domain.result_census.insert((party, project), kept.into_boxed()).is_ok(),
                    "existing project census"
                );
            }
            out.push(Request::Save { record: Stored::ReadResults { party, through } });
        }
        Seen::Conversation { task, through } => {
            if through <= *domain.conversations_seen.get(&(party, task)).unwrap_or(&0) {
                return;
            }
            assert!(
                domain.conversations_seen.insert((party, task), through).is_ok(),
                "bounded live conversation positions"
            );
            out.push(Request::Save { record: Stored::ReadConversation { party, task, through } });
        }
    }
}

fn forget_chat(domain: &mut Domain, limits: &Limits, task: u64, out: &mut Queue<Request>) {
    let mut owners = List::with_capacity(limits.people);
    for (party, _) in &domain.recent {
        owners.push(*party).expect("one recent prefix per party");
    }
    for party in owners.into_boxed() {
        let mut kept = List::with_capacity(limits.inbox_entries);
        if let Some(old) = domain.recent.get(&party) {
            for &item in old {
                let forgotten = match item.update {
                    Update::ChatReply(reply) => reply.entry.task == task,
                    Update::Result(_) | Update::Reply(_) => false,
                };
                kept.push(Recent { update: item.update, gone: item.gone || forgotten }).expect("same prefix");
            }
        }
        assert!(domain.recent.insert(party, kept.into_boxed()).is_ok(), "existing prefix");
    }
    if let Some(chat) = domain.chat_totals.remove(&task) {
        let unseen = chat.last_reply.saturating_sub(*domain.conversations_seen.get(&(chat.party, task)).unwrap_or(&0));
        let count = *domain.chat_updates.get(&(chat.party, chat.project)).unwrap_or(&0);
        assert!(
            domain
                .chat_updates
                .insert(
                    (chat.party, chat.project),
                    count.checked_sub(u128::from(unseen)).expect("live chat count retained")
                )
                .is_ok(),
            "known party aggregate"
        );
    }
    let mut parties = List::with_capacity(limits.people);
    for ((party, conversation), _) in &domain.conversations_seen {
        if *conversation == task {
            parties.push(*party).expect("at most one position per party and conversation");
        }
    }
    for party in parties.into_boxed() {
        let _old = domain.conversations_seen.remove(&(party, task));
        out.push(Request::Erase { key: Key::ReadConversation { party, task } });
    }
    drop(domain.replies.remove(&task));
}

fn addressed(domain: &Domain, person: u64, need: Need) -> bool {
    let Some(held) = role(domain, person, need.project) else { return false };
    match need.whom {
        Whom::Person(owner) => owner == person,
        Whom::Role { project, role } => project == need.project && held.number() == role,
    }
}

fn replace_needs(domain: &mut Domain, limits: &Limits, task: u64, entries: &[Entry]) {
    assert!(
        entries.len() <= usize::try_from(limits.inbox_recipients).expect("projection bound fits"),
        "root bounds each task projection"
    );
    let mut tallies: List<Need> = List::with_capacity(limits.inbox_recipients);
    for entry in entries {
        if entry.task != task || entry.project == 0 {
            continue;
        }
        match entry.kind {
            EntryKind::Reply { .. } | EntryKind::ChatReply { .. } => continue,
            EntryKind::Proposal { .. }
            | EntryKind::Escalation { .. }
            | EntryKind::Question { .. }
            | EntryKind::PersonTask => {}
        }
        tallies
            .push(Need { project: entry.project, whom: entry.whom, kind: entry.kind, at: entry.at })
            .expect("root bounds each task's need recipients");
    }
    drop(domain.needs.remove(&task));
    if !tallies.is_empty() {
        assert!(domain.needs.insert(task, tallies.into_boxed()).is_ok(), "one tally per live task");
    }
}

pub(super) fn replace_waiting(domain: &mut Domain, limits: &Limits, task: u64, entries: &[Entry]) {
    if task == 0 || domain.phase == Phase::Failed {
        return;
    }
    replace_needs(domain, limits, task, entries);
    for &entry in entries {
        if let (Whom::Person(party), EntryKind::Reply { .. }) = (entry.whom, entry.kind) {
            cache_update(domain, limits, party, Update::Reply(entry));
        }
    }
    let mut replies = List::with_capacity(limits.inbox_recipients);
    for &entry in entries {
        match entry.kind {
            EntryKind::Reply { .. } | EntryKind::ChatReply { .. } if entry.task == task && entry.project != 0 => {
                replies.push(entry).expect("bounded reply projection");
            }
            EntryKind::Reply { .. }
            | EntryKind::ChatReply { .. }
            | EntryKind::Proposal { .. }
            | EntryKind::Escalation { .. }
            | EntryKind::Question { .. }
            | EntryKind::PersonTask => {}
        }
    }
    drop(domain.replies.remove(&task));
    if !replies.is_empty() {
        assert!(domain.replies.insert(task, replies.into_boxed()).is_ok(), "one reply projection per task");
    }
    let mut recipients = List::with_capacity(domain.waiting.capacity());
    for (whom, _) in &domain.waiting {
        recipients.push(*whom).expect("one key per cached recipient");
    }
    for &whom in &recipients {
        let Some(old) = domain.waiting.remove(&whom) else { continue };
        let mut kept = List::with_capacity(limits.inbox_entries);
        for entry in old {
            if entry.task != task {
                kept.push(entry).expect("cached subset fits");
            }
        }
        if !kept.is_empty() {
            let saved = domain.waiting.insert(whom, kept.into_boxed());
            assert!(saved.is_ok(), "removed recipient slot remains available");
        }
    }
    for &entry in entries {
        if entry.task != task || entry.project == 0 {
            continue;
        }
        // A role with no holder has no party inbox to cache. The number of distinct held
        // roles is bounded by holdings, even when a policy defines more roles.
        match entry.whom {
            Whom::Role { project, role } => {
                if !held_role(domain, project, role) {
                    continue;
                }
            }
            Whom::Person(_) => {}
        }
        let old = domain.waiting.remove(&entry.whom);
        let mut cached = List::with_capacity(limits.inbox_entries);
        if let Some(old) = old {
            for prior in old {
                cached.push(prior).expect("cached recipient bound");
            }
        }
        insert_newest(&mut cached, entry);
        if !cached.is_empty() {
            drop(domain.waiting.insert(entry.whom, cached.into_boxed()));
        }
    }
}

pub(super) fn prune_unheld_roles(domain: &mut Domain, project: u32) {
    let mut stale = List::with_capacity(domain.waiting.capacity());
    for (whom, _) in &domain.waiting {
        match *whom {
            Whom::Role { project: entry_project, role } => {
                if entry_project == project && !held_role(domain, project, role) {
                    stale.push(*whom).expect("one cached role per waiting key");
                }
            }
            Whom::Person(_) => {}
        }
    }
    for whom in &stale {
        domain.waiting.remove(whom);
    }
}

fn held_role(domain: &Domain, project: u32, role: u32) -> bool {
    let Some(holdings) = domain.roles.get(&project) else {
        return false;
    };
    for holding in holdings {
        if holding.role.number() == role {
            return true;
        }
    }
    false
}

#[expect(clippy::too_many_arguments, reason = "one bounded sibling census fact")]
pub(super) fn chat_state_event(
    domain: &mut Domain,
    env: &Env<Limits>,
    party: u64,
    project: u32,
    task: u64,
    last_reply: u64,
    live: bool,
    seen: Option<u64>,
    replies: Box<[ReplyRef]>,
    out: &mut Queue<Request>,
) {
    let through = seen.unwrap_or(*domain.conversations_seen.get(&(party, task)).unwrap_or(&0));
    chat_state(domain, State { party, project, task, last_reply, live, seen });
    assert!(
        replies.len() <= usize::try_from(env.limits.inbox_recipients).expect("reply projection bound"),
        "bounded owner hints"
    );
    for reply in replies {
        cache_update(domain, &env.limits, party, Update::ChatReply(reply));
    }
    if !live {
        super::ended::ended_state(domain, party, task, through, seen.is_none(), out);
    }
}

pub(super) fn forget_event(
    domain: &mut Domain,
    env: &Env<Limits>,
    task: u64,
    project: u32,
    party: Option<u64>,
    _last_reply: u64,
    out: &mut Queue<Request>,
) {
    if let Some(party) = party
        && domain.people.contains_key(&party)
        && domain.has_project(project)
    {
        out.push(Request::Erase { key: Key::EndedRead { task, party } });
    }
    forget_chat(domain, &env.limits, task, out);
}

fn remember_project_result(domain: &mut Domain, limits: &Limits, party: u64, entry: ResultRef) {
    let cap = limits.inbox_entries.checked_add(1).expect("result witness");
    let mut kept = List::with_capacity(cap);
    let mut inserted = false;
    if let Some(old) = domain.result_census.get(&(party, entry.project)) {
        for &value in old {
            if value.task == entry.task || value.position == entry.position {
                return;
            }
            if !inserted && entry.position > value.position {
                kept.push(entry).expect("newer result prefix");
                inserted = true;
            }
            if kept.room() > 0 {
                kept.push(value).expect("bounded result prefix");
            }
        }
    }
    if !inserted && kept.room() > 0 {
        kept.push(entry).expect("result prefix room");
    }
    assert!(
        domain.result_census.insert((party, entry.project), kept.into_boxed()).is_ok(),
        "one census per party/project"
    );
}
