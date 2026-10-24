//! Volatile party inbox projections from the task child's durable rows.
//! The core routes these sibling facts on save and restore (domain/engine.md, 4.4).

mod census;
pub(crate) use census::{Progress, worst_case as census_worst_case};

use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::List;

use crate::{Core, Limits};

/// Inbox words, role-routed proposals and the four scalar references: a
/// person-held proposal, escalation, answerable person task and stall update.
pub(crate) fn waiting_capacity(limits: &Limits) -> Option<u32> {
    limits.tasks.inbox_messages.checked_mul(2)?.checked_add(limits.authority.roles)?.checked_add(4)
}

fn add(
    entries: &mut List<people::Entry>,
    row: &tasks::TaskRecord,
    whom: people::Whom,
    kind: people::EntryKind,
    at: skein_lib::Wall,
) {
    entries
        .push(people::Entry { task: row.number, project: row.project, whom, kind, at })
        .expect("one bounded projection per task state");
}

fn proposal_kind(kind: tasks::ProposalKind) -> authority::ProposalKind {
    match kind {
        tasks::ProposalKind::Settings => unreachable!("settings proposals have no task holder"),
        tasks::ProposalKind::Batch => authority::ProposalKind::Batch,
        tasks::ProposalKind::Effect => authority::ProposalKind::Effect,
        tasks::ProposalKind::Amend => authority::ProposalKind::Amend,
        tasks::ProposalKind::Widen => authority::ProposalKind::Widen,
        tasks::ProposalKind::Release => authority::ProposalKind::Escalation,
    }
}

pub(crate) fn questions_live(phase: &tasks::Phase) -> bool {
    match phase {
        tasks::Phase::Closing(_) | tasks::Phase::Ended(_) => false,
        tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Held { .. } => true,
    }
}

pub(crate) fn question_capacity(limits: &Limits) -> Option<u32> {
    limits.tasks.tasks.checked_mul(limits.tasks.inbox_messages)
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) struct ArchivedChat {
    pub(crate) party: u64,
    pub(crate) project: u32,
    pub(crate) at: Option<skein_lib::Wall>,
    pub(crate) chip: crate::documents::Chip,
    pub(crate) line: Option<crate::documents::TaskLine>,
    pub(crate) last_reply: u64,
}

pub(crate) fn reply_refs(limits: &Limits, row: &tasks::TaskRecord, party: u64) -> Box<[people::ReplyRef]> {
    let mut refs = List::with_capacity(limits.tasks.inbox_messages);
    for reply in &row.replies {
        refs.push(people::ReplyRef {
            entry: people::Entry {
                task: row.number,
                project: row.project,
                whom: people::Whom::Person(party),
                kind: people::EntryKind::ChatReply { message: reply.number },
                at: reply.at,
            },
            activity: reply.activity,
        })
        .expect("bounded owner hints");
    }
    refs.into_boxed()
}

pub(crate) fn worst_case(limits: &Limits) -> Option<u64> {
    let headers = limits.people.people.checked_mul(limits.people.inbox_entries)?.checked_add(1)?;
    let questions = question_capacity(limits)?;
    skein_lib::Map::<(u64, u64), tasks::ActivityLine>::worst_case(questions)?
        .checked_add(u64::from(questions).checked_mul(u64::from(limits.tasks.message_bytes))?)?
        .checked_add(List::<(u64, u64)>::worst_case(questions)?)?
        .checked_add(skein_lib::Map::<u64, ArchivedChat>::worst_case(headers)?)?
        .checked_add(
            u64::from(headers)
                .checked_mul(crate::documents::chip_heap_worst_case(&limits.documents)?.checked_mul(2)?)?,
        )?
        .checked_add(List::<u64>::worst_case(headers)?)?
        .checked_add(List::<people::ReplyRef>::worst_case(limits.tasks.inbox_messages)?)?
        .checked_add(List::<people::Update>::worst_case(limits.people.inbox_entries)?)
}

impl Core {
    /// Retain only activity named by an unanswered question's durable position.
    pub(crate) fn remember_question(&mut self, line: &tasks::ActivityLine) -> bool {
        let Some(owner) = self.tasks.task(line.task) else { return false };
        let tasks::Line::Words { message, kind: tasks::MessageKind::Question, .. } = &line.line else {
            return false;
        };
        let mut named = false;
        for position in &owner.question_lines {
            named |= position.message == *message && position.line == line.position && position.answer.is_none();
        }
        if !named {
            return false;
        }
        self.prune_questions(line.task);
        self.question_activity.insert((line.task, line.position), line.clone()).expect("bounded open questions");
        true
    }

    pub(crate) fn prune_questions(&mut self, task: u64) {
        let mut stale = List::with_capacity(self.question_activity.capacity());
        for (key, _) in &self.question_activity {
            if key.0 != task {
                continue;
            }
            let mut keep = false;
            if let Some(owner) = self.tasks.task(task)
                && questions_live(&owner.phase)
            {
                for position in &owner.question_lines {
                    keep |= position.line == key.1 && position.answer.is_none();
                }
            }
            if !keep {
                stale.push(*key).expect("bounded question snapshot");
            }
        }
        for key in stale.into_boxed() {
            drop(self.question_activity.remove(&key));
        }
    }
}

impl Core {
    /// The requesting party of a flagged conversation, live or loaded ended.
    #[must_use]
    pub fn conversation_requester(&self, row: &tasks::TaskRecord) -> Option<u64> {
        if !row.flags.conversation {
            return None;
        }
        match row.requester {
            tasks::Party::Person(party) => Some(party),
            tasks::Party::Task(_) | tasks::Party::Deployment { .. } => None,
        }
    }
    /// Keep whole committed archive facts only while the shared recent prefix may deliver them.
    pub(crate) fn cache_archived_chat(&mut self, limits: &Limits, row: &tasks::TaskRecord, seen: Option<u64>) {
        let party = match row.requester {
            tasks::Party::Person(party) => party,
            tasks::Party::Task(_) | tasks::Party::Deployment { .. } => return,
        };
        if self.people.role(party, row.project).is_none() {
            return;
        }
        let seen = match seen {
            Some(through) => Some(through),
            None => self.people.conversation_seen(party, row.number),
        };
        let line = if self.conversation_requester(row) == Some(party) && row.last_reply > 0 {
            Some(crate::documents::task_line(self, limits, party, row, seen).expect("authorized archive owner"))
        } else {
            None
        };
        let chip = crate::documents::chip_from_row(limits, row).expect("ended chip has no whole resource paths");
        assert!(
            self.archived_chats
                .insert(
                    row.number,
                    ArchivedChat {
                        party,
                        project: row.project,
                        at: row.ended_at,
                        chip,
                        line,
                        last_reply: row.last_reply
                    }
                )
                .is_ok(),
            "one temporary archive slot beyond the bounded prefix"
        );
        let mut names = List::with_capacity(self.archived_chats.capacity());
        for (number, _) in &self.archived_chats {
            names.push(*number).expect("archive snapshot bound");
        }
        for number in names.into_boxed() {
            let owner = self.archived_chats.get(&number).expect("snapshot archive").party;
            let mut needed = false;
            if let Some(updates) = self.people.cached_updates(&limits.people, owner) {
                for update in updates {
                    needed |= match update {
                        people::Update::ChatReply(reply) => reply.entry.task == number,
                        people::Update::Result(result) => result.task == number,
                        people::Update::Reply(entry) => entry.task == number,
                    };
                }
            }
            if !needed {
                drop(self.archived_chats.remove(&number));
            }
        }
    }

    /// Project the currently waiting person-facing references of one task.
    #[must_use]
    #[expect(clippy::too_many_lines, reason = "one exhaustive task projection covers all person-facing waiting kinds")]
    pub fn waiting_entries(&self, limits: &Limits, row: &tasks::TaskRecord) -> Box<[people::Entry]> {
        let capacity = waiting_capacity(limits).expect("validated task inbox projection bound");
        let mut entries = List::with_capacity(capacity);
        match &row.phase {
            tasks::Phase::Ended(_) | tasks::Phase::Closing(_) => return entries.into_boxed(),
            tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Held { .. } => {}
        }
        match row.requester {
            tasks::Party::Person(person) => {
                for line in &row.question_lines {
                    if line.answer.is_none()
                        && let Some(fact) = self.question_activity.get(&(row.number, line.line))
                    {
                        add(
                            &mut entries,
                            row,
                            people::Whom::Person(person),
                            people::EntryKind::Question { message: line.message },
                            fact.at,
                        );
                    }
                }
            }
            tasks::Party::Task(_) | tasks::Party::Deployment { .. } => {}
        }
        if let Some(person) = self.conversation_requester(row) {
            for reply in &row.replies {
                add(
                    &mut entries,
                    row,
                    people::Whom::Person(person),
                    people::EntryKind::ChatReply { message: reply.number },
                    reply.at,
                );
            }
        }
        if let Some(proposal) = &row.proposal {
            match proposal.state {
                tasks::ProposalState::Pending { holder, since } => match holder {
                    tasks::ProposalHolder::Person(person) => add(
                        &mut entries,
                        row,
                        people::Whom::Person(person),
                        people::EntryKind::Proposal { number: proposal.number },
                        since,
                    ),
                    tasks::ProposalHolder::Policy { project, kind } => {
                        if let Some(project_policy) = self.authority.policy(project) {
                            for policy in &project_policy.roles {
                                if !policy.decides.allows(proposal_kind(kind)) {
                                    continue;
                                }
                                add(
                                    &mut entries,
                                    row,
                                    people::Whom::Role { project, role: policy.number },
                                    people::EntryKind::Proposal { number: proposal.number },
                                    since,
                                );
                            }
                        }
                    }
                    tasks::ProposalHolder::Task(_) => {}
                },
                tasks::ProposalState::Accepted { .. }
                | tasks::ProposalState::Rejected { .. }
                | tasks::ProposalState::Withdrawn => {}
            }
        }
        match row.escalation {
            tasks::Escalation::Waiting { holder, revision, since, .. } => match holder {
                tasks::EscalationHolder::Person(person) => {
                    add(
                        &mut entries,
                        row,
                        people::Whom::Person(person),
                        people::EntryKind::Escalation { revision },
                        since,
                    );
                }
                tasks::EscalationHolder::Role { project, role } => {
                    add(
                        &mut entries,
                        row,
                        people::Whom::Role { project, role },
                        people::EntryKind::Escalation { revision },
                        since,
                    );
                }
                tasks::EscalationHolder::Task(_) => {}
            },
            tasks::Escalation::Unheld { .. }
            | tasks::Escalation::Routing { .. }
            | tasks::Escalation::Rejected { .. } => {}
        }
        if let Some(wait) = row.person_wait
            && let Some(notice) = wait.notice
            && let tasks::Party::Person(person) = row.requester
        {
            add(
                &mut entries,
                row,
                people::Whom::Person(person),
                people::EntryKind::Reply { message: notice.message },
                notice.at,
            );
        }
        let answerable = match row.phase {
            tasks::Phase::Active(tasks::Active::Due | tasks::Active::Idle) => true,
            tasks::Phase::Waiting
            | tasks::Phase::Active(_)
            | tasks::Phase::Held { .. }
            | tasks::Phase::Closing(_)
            | tasks::Phase::Ended(_) => false,
        };
        match row.executor {
            tasks::Executor::Person(_) if !answerable => {}
            tasks::Executor::Person(tasks::PersonAddress::Person(person)) => {
                if row.taken_by.is_none() {
                    let since = row.person_wait.expect("active person wait").since;
                    add(&mut entries, row, people::Whom::Person(person), people::EntryKind::PersonTask, since);
                }
            }
            tasks::Executor::Person(tasks::PersonAddress::Role(role)) => {
                let since = row.person_wait.expect("active person wait").since;
                let whom = match row.taken_by {
                    Some(person) => people::Whom::Person(person),
                    None => people::Whom::Role { project: row.project, role },
                };
                add(&mut entries, row, whom, people::EntryKind::PersonTask, since);
            }
            tasks::Executor::Agent { .. } | tasks::Executor::Procedure { .. } => {}
        }
        entries.into_boxed()
    }
}

/// Validate a bounded owner header before using its committed reply positions.
pub(crate) fn valid_header(core: &Core, limits: &Limits, row: &tasks::TaskRecord) -> bool {
    let deployment = core.counters.deployment();
    if row.number == 0
        || row.number > deployment.tasks
        || row.project == 0
        || row.last_reply > row.activity
        || row.replies.len() > usize::try_from(limits.tasks.inbox_messages).expect("hint bound fits")
    {
        return false;
    }
    let mut previous = None;
    for (index, hint) in row.replies.iter().enumerate() {
        if hint.number == 0
            || hint.number > row.last_reply
            || hint.activity == 0
            || hint.activity > row.activity
            || hint.attempt == 0
            || hint.attempt > deployment.runs
            || hint.attempt > row.attempt
            || (hint.attempt == row.attempt && hint.turn > row.turn)
            || hint.turn == 0
        {
            return false;
        }
        if let Some(key) = previous
            && key <= (hint.at, hint.number)
        {
            return false;
        }
        for old in row.replies.iter().take(index) {
            if old.number == hint.number || old.activity == hint.activity {
                return false;
            }
        }
        previous = Some((hint.at, hint.number));
    }
    row.last_reply == 0 || !row.replies.is_empty()
}

pub(crate) fn seen_conversation_loaded(
    core: &mut Core,
    env: &skein_lib::Env<Limits>,
    work: &mut skein_lib::Queue<crate::Event>,
    request: skein_lib::Token,
    row: Option<Box<tasks::TaskRecord>>,
    busy: bool,
) -> crate::Requests {
    let Some((party, people::Seen::Conversation { task, .. })) = core.people.seen_context(request) else {
        return crate::Requests::Out(skein_lib::Queue::with_capacity(0));
    };
    let event = if busy {
        people::Event::Decided { request, outcome: people::Outcome::Again(people::Again::Busy) }
    } else if let Some(row) = row {
        if row.number != task || !valid_header(core, &env.limits, &row) || !matches_ended(&row.phase) {
            people::Event::Decided { request, outcome: people::Outcome::Refused(people::Refusal::Unknown) }
        } else if core.conversation_requester(&row) != Some(party) || core.people.role(party, row.project).is_none() {
            people::Event::Decided {
                request,
                outcome: people::Outcome::Refused(people::Refusal::Role { held: core.people.role(party, row.project) }),
            }
        } else {
            assert!(
                core.ended_marks
                    .insert(
                        request,
                        crate::ended_marks::Loaded {
                            task,
                            party,
                            project: row.project,
                            last_reply: row.last_reply,
                            owner: crate::forget::Owner::of_row(&row),
                        }
                    )
                    .is_ok(),
                "one header per pending mark"
            );
            let mut out = skein_lib::Queue::with_capacity(2);
            out.push(crate::Request::Held(Box::new(crate::Held::Load(Box::new(crate::Now::LoadEndedRead {
                request,
                task,
                party,
            })))));
            out.push(crate::Request::Decided);
            return crate::Requests::Out(out);
        }
    } else {
        people::Event::Decided { request, outcome: people::Outcome::Refused(people::Refusal::Unknown) }
    };
    work.push(crate::Event::People(event));
    let mut out = skein_lib::Queue::with_capacity(1);
    out.push(crate::Request::Decided);
    crate::Requests::Out(out)
}
fn matches_ended(phase: &tasks::Phase) -> bool {
    match phase {
        tasks::Phase::Ended(_) => true,
        tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Closing(_) | tasks::Phase::Held { .. } => false,
    }
}

impl Core {
    /// Deployment settings have proposal identities and no task tally.
    #[must_use]
    pub(crate) fn settings_needs(&self, party: u64) -> u32 {
        if self.people.role(party, people::DEPLOYMENT) == Some(people::Role::Owner) {
            self.settings_proposals.len()
        } else {
            0
        }
    }
    #[must_use]
    pub(crate) fn party_need_counts(&self, party: u64) -> Option<people::Needs> {
        let mut counts = self.people.need_counts(party)?;
        counts.proposals = counts.proposals.checked_add(self.settings_needs(party))?;
        Some(counts)
    }
}
