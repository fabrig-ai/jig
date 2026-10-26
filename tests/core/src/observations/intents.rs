//! Explicit peer asks, deduplicated by their boundary identities.
use super::{core, people, r, tasks};
#[path = "named.rs"]
mod named;
use jig_test_domain::Record;
use skein_lib::Token;
use std::collections::{BTreeMap, BTreeSet};

fn signed_party(store: &crate::Store, sign_in: u64) -> Option<u64> {
    store.rows.values().find_map(|row| match row {
        Record::Core(core::Record::People(people::Stored::SignIn { number, person, .. })) if *number == sign_in => {
            Some(*person)
        }
        Record::Core(_) | Record::Connector { .. } => None,
    })
}

#[derive(Clone, Debug, Default)]
pub struct Intents {
    named: named::Names,
    parties: BTreeMap<(u64, [u8; 16]), bool>,
    replies: BTreeMap<Token, (u64, [u8; 16])>,
    calls: BTreeMap<core::CallKey, bool>,
    terminals: BTreeSet<(u64, u64)>,
}

impl Intents {
    #[must_use]
    pub fn configured(configuration: &jig_test_domain::Config) -> Self {
        Self { named: named::Names::configured(configuration), ..Self::default() }
    }

    pub fn inbound_root(&mut self, store: &crate::Store, event: &mut jig_test_domain::Event) -> Vec<r::Observed> {
        use jig_test_domain::Event;
        if let Event::Core(core::Event::People(people::Event::Ask { models: _, reply_to, sign_in, key, ask })) = event {
            let person = signed_party(store, *sign_in);
            return person.map_or_else(Vec::new, |person| self.party(reply_to, person, *key, ask));
        }
        if let Event::Core(core::Event::ProposeSettings { reply_to, sign_in, key, .. }) = event {
            let person = signed_party(store, *sign_in);
            return person.map_or_else(Vec::new, |person| self.party_proposal(reply_to, person, *key));
        }
        if let Event::Core(event) = event {
            return self.inbound(event);
        }
        if let Event::Answer { task, attempt, end, .. } = event {
            return self.terminal(*task, *attempt, end);
        }
        if let Event::EffectCall { key, proposal: Some(_), .. } = event {
            return self.propose(*key);
        }
        Vec::new()
    }

    pub fn submitted(&mut self, store: &crate::Store) {
        self.named.submitted(store);
    }

    pub fn durable(&mut self, store: &crate::Store) -> Vec<r::Observed> {
        let mut observed = self.named.durable(store.applied);
        observed.extend(
            store
                .rows
                .values()
                .flat_map(|row| match row {
                    jig_test_domain::Record::Core(record) => self.record(record),
                    jig_test_domain::Record::Connector { .. } => Vec::new(),
                })
                .collect::<Vec<_>>(),
        );
        observed
    }

    pub fn party(
        &mut self,
        reply: &mut skein_lib::ReplyTo,
        person: u64,
        key: [u8; 16],
        ask: &people::Ask,
    ) -> Vec<r::Observed> {
        let mut observed = named::party(ask);
        if matches!(ask, people::Ask::Propose { .. }) {
            observed.extend(self.party_proposal(reply, person, key));
        }
        observed
    }

    fn party_proposal(&mut self, reply: &mut skein_lib::ReplyTo, person: u64, key: [u8; 16]) -> Vec<r::Observed> {
        self.replies.insert(Token::new(super::destination(reply)), (person, key));
        if let std::collections::btree_map::Entry::Vacant(entry) = self.parties.entry((person, key)) {
            entry.insert(true);
            vec![r::Observed::ProposeAsked { proposer: r::Source::Person(person) }]
        } else {
            Vec::new()
        }
    }

    pub fn inbound(&mut self, event: &core::Event) -> Vec<r::Observed> {
        let observed = named::run(event);
        if !observed.is_empty() {
            return observed;
        }
        if let core::Event::NamedAction { key, action, .. } = event
            && matches!(
                action,
                core::NamedAction::ProposeSettings { .. }
                    | core::NamedAction::Propose { .. }
                    | core::NamedAction::ProposeBatch { .. }
                    | core::NamedAction::ProposeGoal { .. }
            )
        {
            return self.propose(*key);
        }
        if let core::Event::AnswerPayload { run, attempt, end, .. } = event {
            return self.terminal(run.raw(), attempt.raw(), end);
        }
        Vec::new()
    }

    pub fn propose(&mut self, key: core::CallKey) -> Vec<r::Observed> {
        if let std::collections::btree_map::Entry::Vacant(entry) = self.calls.entry(key) {
            entry.insert(true);
            vec![r::Observed::ProposeAsked { proposer: r::Source::Task(key.task) }]
        } else {
            Vec::new()
        }
    }

    pub fn terminal(&mut self, task: u64, attempt: u64, end: &tasks::End) -> Vec<r::Observed> {
        if matches!(end, tasks::End::FinishedWithFollowups { .. }) && self.terminals.insert((task, attempt)) {
            vec![r::Observed::ProposeAsked { proposer: r::Source::Task(task) }]
        } else {
            Vec::new()
        }
    }

    pub fn reply(&mut self, to: Token, reply: &people::Reply) -> Vec<r::Observed> {
        if let Some((person, key)) = self.replies.remove(&to) {
            let observed = self.party_answer(
                person,
                key,
                matches!(reply, people::Reply::Outcome(people::Outcome::Proposed { .. })),
            );
            if matches!(reply, people::Reply::Again(_)) {
                self.parties.remove(&(person, key));
            }
            return observed;
        }
        Vec::new()
    }

    fn party_answer(&mut self, person: u64, key: [u8; 16], proposed: bool) -> Vec<r::Observed> {
        if let Some(waiting) = self.parties.get_mut(&(person, key))
            && std::mem::take(waiting)
            && !proposed
        {
            return vec![r::Observed::ProposeRefused { proposer: r::Source::Person(person) }];
        }
        Vec::new()
    }

    pub fn record(&mut self, record: &core::Record) -> Vec<r::Observed> {
        match record {
            core::Record::People(people::Stored::Answer { key, outcome, .. }) => {
                self.party_answer(key.person, key.key, matches!(outcome, people::Outcome::Proposed { .. }))
            }
            core::Record::Core(core::CoreRecord::Call(row)) => {
                if let Some(waiting) = self.calls.get_mut(&row.key)
                    && std::mem::take(waiting)
                    && !matches!(row.part, core::CallPart::Proposed { .. })
                {
                    return vec![r::Observed::ProposeRefused { proposer: r::Source::Task(row.key.task) }];
                }
                Vec::new()
            }
            core::Record::Core(_)
            | core::Record::Tasks(_)
            | core::Record::People(_)
            | core::Record::Notes(_)
            | core::Record::Accounts(_) => Vec::new(),
        }
    }
}
