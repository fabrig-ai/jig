//! Index ranges for the inbox's restart census (protocol/store.md 7.2).
use crate::Key;
use alloc::boxed::Box;
use jig_domain_tasks as tasks;
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ReadResults {
    pub party: u64,
    pub after: u64,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Census {
    /// Exact activity keys of unanswered questions, reconstructed at restart.
    pub questions: Box<[(u64, u64)]>,
    pub results: Box<[ReadResults]>,
}
impl Census {
    #[must_use]
    pub fn admits(&self, key: &Key) -> bool {
        self.interval(key).is_some()
    }
    /// The admitted key's place in the ordered census walk.
    #[must_use]
    pub fn interval(&self, key: &Key) -> Option<u32> {
        match key {
            Key::People(jig_domain_people::Key::EndedRead { .. }) => Some(0),
            Key::Tasks(tasks::Key::Activity { task, position }) => {
                for (index, key) in self.questions.iter().enumerate() {
                    if key == &(*task, *position) {
                        return u32::try_from(self.results.len())
                            .ok()?
                            .checked_add(1)?
                            .checked_add(u32::try_from(index).ok()?);
                    }
                }
                None
            }
            Key::Tasks(tasks::Key::Result { party, position }) => {
                for (index, read) in self.results.iter().enumerate() {
                    if read.party == *party && *position > read.after {
                        return u32::try_from(index).ok()?.checked_add(1);
                    }
                }
                None
            }
            Key::Tasks(
                tasks::Key::Live(_)
                | tasks::Key::Ended(_)
                | tasks::Key::PlacedHold(_)
                | tasks::Key::Stub(_)
                | tasks::Key::Ledger(_)
                | tasks::Key::ClosedLedger(_)
                | tasks::Key::DecidedProposal(_)
                | tasks::Key::Writer(_)
                | tasks::Key::Pool(_)
                | tasks::Key::PersonProposal(_)
                | tasks::Key::History { .. }
                | tasks::Key::Milestone { .. }
                | tasks::Key::Update { .. }
                | tasks::Key::EndedByParty { .. }
                | tasks::Key::EndedByProject { .. }
                | tasks::Key::EndedDelegate { .. }
                | tasks::Key::Delegate { .. },
            )
            | Key::Core(_)
            | Key::People(
                jig_domain_people::Key::Person(_)
                | jig_domain_people::Key::ReadConversation { .. }
                | jig_domain_people::Key::ReadResults { .. }
                | jig_domain_people::Key::SignIn(_)
                | jig_domain_people::Key::Roles(_)
                | jig_domain_people::Key::Policy(_)
                | jig_domain_people::Key::Answer(_)
                | jig_domain_people::Key::Acted(_),
            )
            | Key::Notes(_)
            | Key::Accounts(_) => None,
        }
    }
}
