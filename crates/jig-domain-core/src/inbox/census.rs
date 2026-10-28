//! Rebuild live projections once, then name the indexed ended owners (store.md 7.2).
use crate::{Census, Core, Limits, ReadResults};
use jig_domain_people as people;
use skein_lib::{Env, List, Map, Queue, Time, Wall};

#[derive(Debug)]
pub(crate) struct Progress {
    unread_result: u64,
    read_conversation: u64,
}

pub(crate) fn worst_case(limits: &Limits) -> Option<u64> {
    Map::<u64, Progress>::worst_case(limits.people.people)?
        .checked_add(List::<u64>::worst_case(limits.people.people)?)?
        .checked_add(List::<ReadResults>::worst_case(limits.people.people)?)?
        .checked_add(List::<(u64, u64)>::worst_case(super::question_capacity(limits)?)?)
}
impl Core {
    pub fn census(&mut self, limits: &Limits) -> Census {
        assert!(
            self.restart_step() == Some(crate::RestartStep::LoadInbox) && self.people.ready(),
            "people restored before census"
        );
        let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people };
        let mut out = Queue::uncapped();
        for task in self.tasks.view_tasks() {
            let row = self.tasks.task(task.number).expect("live view names its owner");
            if let Some(party) = self.conversation_requester(row) {
                people::step(
                    &mut self.people,
                    &env,
                    people::Event::ChatState {
                        party,
                        project: row.project,
                        task: row.number,
                        last_reply: row.last_reply,
                        live: true,
                        seen: None,
                        replies: super::reply_refs(limits, row, party),
                    },
                    &mut out,
                );
            }
            let entries = self.waiting_entries(limits, row);
            people::step(&mut self.people, &env, people::Event::Waiting { task: row.number, entries }, &mut out);
            assert!(out.is_empty(), "restored live projections decide nothing");
        }
        for (_, proposal) in self.tasks.person_proposals() {
            let entries = self.person_proposal_entries(proposal);
            people::step(&mut self.people, &env, people::Event::Waiting { task: proposal.first(), entries }, &mut out);
            assert!(out.is_empty(), "restored pending proposals decide nothing");
        }
        let mut questions = List::with_capacity(super::question_capacity(limits).expect("validated question bound"));
        for task in self.tasks.view_tasks() {
            let row = self.tasks.task(task.number).expect("live view owner");
            if !super::questions_live(&row.phase) {
                continue;
            }
            for line in &row.question_lines {
                if line.answer.is_none() {
                    questions.push((row.number, line.line)).expect("bounded open questions");
                }
            }
        }
        let mut results = List::with_capacity(limits.people.people);
        for party in &self.people.parties() {
            self.census_progress
                .insert(*party, Progress { unread_result: 0, read_conversation: 0 })
                .expect("bounded census parties");
            results
                .push(ReadResults {
                    party: *party,
                    after: self.people.results_seen(*party).expect("known restored party"),
                })
                .expect("bounded restored parties");
        }
        Census { questions: questions.into_boxed(), results: results.into_boxed() }
    }
}

impl Core {
    #[must_use]
    pub fn restore_inbox_row(&mut self, limits: &Limits, record: &crate::Record) -> bool {
        use jig_domain_tasks as tasks;
        if self.restart_step() != Some(crate::RestartStep::LoadInbox) || !self.people.ready() {
            return false;
        }
        if let crate::Record::Tasks(tasks::Stored::Activity(line)) = record {
            if !self.remember_question(line) {
                return false;
            }
            let owner = self.tasks.task(line.task).expect("named question owner");
            let entries = self.waiting_entries(limits, owner);
            let mut out = Queue::uncapped();
            let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people };
            people::step(&mut self.people, &env, people::Event::Waiting { task: line.task, entries }, &mut out);
            return out.is_empty();
        }
        if let crate::Record::People(people::Stored::EndedRead { task, party, through }) = record {
            if self.census_position.is_some() {
                return false;
            }
            self.census_position = Some((*task, *party, *through));
            return true;
        }
        let crate::Record::Tasks(tasks::Stored::Ended(row)) = record else { return false };
        let seen = match self.census_position.take() {
            Some((task, party, through))
                if task == row.number && row.requester == tasks::Party::Person(party) && row.flags.conversation =>
            {
                Some(through)
            }
            Some(_) => return false,
            None => None,
        };
        if !super::matches_ended(&row.phase) || !super::valid_header(self, limits, row) {
            return false;
        }
        let mut out = Queue::uncapped();
        let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people };
        match row.requester {
            tasks::Party::Person(party) => {
                if self.people.identity(party).is_none() {
                    return false;
                }
                let Some(at) = row.ended_at else { return false };
                if row.result_position == 0 || row.result_position > self.counters.deployment().messages {
                    return false;
                }
                let progress = self.census_progress.get_mut(&party).expect("party census initialised");
                let mark = if seen.is_none() { &mut progress.unread_result } else { &mut progress.read_conversation };
                let position = if seen.is_none() { row.result_position } else { row.number };
                if position <= *mark {
                    return true;
                }
                *mark = position;
                self.people.remember_result(
                    &limits.people,
                    party,
                    people::ResultRef {
                        task: row.number,
                        project: row.project,
                        at,
                        position: row.result_position,
                        forgettable: (row.flags.retention == tasks::Retention::ForgottenOnceRead).then_some(
                            people::Forgettable {
                                label: row.label,
                                listed: row.flags.listed,
                                turns_read: match seen {
                                    Some(through) => through >= row.last_reply,
                                    None => false,
                                },
                                last_reply: row.flags.conversation.then_some(row.last_reply),
                            },
                        ),
                    },
                );
                if let Some(through) = seen {
                    people::step(
                        &mut self.people,
                        &env,
                        people::Event::ChatState {
                            party,
                            project: row.project,
                            task: row.number,
                            last_reply: row.last_reply,
                            live: false,
                            seen: Some(through),
                            replies: super::reply_refs(limits, row, party),
                        },
                        &mut out,
                    );
                }
            }
            tasks::Party::Task(_) | tasks::Party::Deployment { .. } => {}
        }
        // Resolve while the validated ended owner is held, before the bounded
        // archive cache can evict it. Historical owners are not joined.
        self.people.restore_ended_conversation(row.number);
        if seen.is_some() || !self.archived_chats.contains_key(&row.number) {
            self.cache_archived_chat(limits, row, seen);
        }
        out.is_empty()
    }
}
