//! Stored people views, excluding derived inboxes and process-local flights (people.md, 3–5).
use super::Domain;
use crate::{Key, Stored};
use alloc::boxed::Box;

impl Domain {
    /// Copy one retained row without recomputing admission or expiry.
    #[must_use]
    pub fn stored_record(&self, key: &Key) -> Option<Stored> {
        Some(match *key {
            Key::Person(number) => Stored::Person { number, identity: self.people.get(&number)?.clone() },
            Key::ReadResults { party } => Stored::ReadResults { party, through: *self.results_seen.get(&party)? },
            Key::ReadConversation { party, task } => {
                Stored::ReadConversation { party, task, through: *self.conversations_seen.get(&(party, task))? }
            }
            Key::SignIn(number) => {
                let row = self.sign_ins.get(&number)?;
                Stored::SignIn { number, person: row.person, expires: row.expires }
            }
            Key::Roles(project) => Stored::Roles { project, holdings: self.roles.get(&project)?.clone() },
            Key::Answer(key) => {
                let row = self.answers.get(&key)?;
                Stored::Answer {
                    key,
                    ask: Box::new(row.ask.clone()),
                    models: row.models.clone(),
                    outcome: row.outcome.clone(),
                    at: row.at,
                }
            }
            Key::Acted(key) => {
                let row = self.acted.get(&key)?;
                Stored::Acted { key, act: row.act.clone(), outcome: row.outcome.clone(), at: row.at }
            }
            Key::Policy(_) | Key::EndedRead { .. } => return None,
        })
    }
}
