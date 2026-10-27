//! Shared key generators and interval membership fixtures.
use crate::keys::Own;
use alloc::boxed::Box;
use jig_domain_core::{self as core, CoreKey, ProjectionKey};
use jig_domain_people::{self as people, Proposer};
use skein_lib::Rng;
pub fn number(rng: &mut Rng) -> u64 {
    match rng.below(5) {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => u64::from(u32::MAX),
        _ => u64::MAX,
    }
}
pub fn narrow(rng: &mut Rng) -> u32 {
    match rng.below(4) {
        0 => 0,
        1 => 1,
        2 => 2,
        _ => u32::MAX,
    }
}
pub fn own(rng: &mut Rng) -> Own {
    match rng.below(4) {
        0 => Own::Format,
        3 => Own::Layouts,
        1 => Own::Transcript { task: number(rng) },
        _ => Own::Attempt { task: number(rng), attempt: number(rng) },
    }
}
pub fn core_key(rng: &mut Rng) -> CoreKey {
    match rng.below(8) {
        0 => CoreKey::Deployment,
        1 => CoreKey::Projection(match rng.below(4) {
            0 => ProjectionKey::Goal(number(rng)),
            1 => ProjectionKey::Task { goal: number(rng), task: number(rng) },
            2 => ProjectionKey::Lifecycle { goal: number(rng), task: number(rng), position: number(rng) },
            _ => ProjectionKey::Revision { goal: number(rng), task: number(rng), revision: number(rng) },
        }),
        2 => CoreKey::RunProof(number(rng)),
        3 => CoreKey::Call(core::CallKey {
            task: number(rng),
            attempt: number(rng),
            completion: narrow(rng),
            position: narrow(rng),
        }),
        4 => CoreKey::Turn { task: number(rng), attempt: number(rng), turn: narrow(rng) },
        5 => CoreKey::Terminal { task: number(rng), attempt: number(rng) },
        6 => CoreKey::ProposalDecision {
            proposer: match rng.below(2) {
                0 => Proposer::Task(number(rng)),
                _ => Proposer::Party(number(rng)),
            },
            proposal: number(rng),
        },
        _ => CoreKey::EscalationDecision { task: number(rng), revision: number(rng) },
    }
}
pub fn people_key(rng: &mut Rng) -> people::Key {
    match rng.below(8) {
        0 => people::Key::Person(number(rng)),
        1 => people::Key::ReadResults { party: number(rng) },
        2 => people::Key::ReadConversation { party: number(rng), task: number(rng) },
        3 => people::Key::SignIn(number(rng)),
        4 => people::Key::Roles(narrow(rng)),
        5 => people::Key::Policy(narrow(rng)),
        7 => people::Key::EndedRead { task: number(rng), party: number(rng) },
        _ => {
            let mut key = [0; 16];
            for byte in &mut key {
                *byte = match rng.below(5) {
                    0 => 0,
                    1 => 1,
                    2 => 2,
                    3 => 254,
                    _ => 255,
                };
            }
            people::Key::Answer(people::RequestKey { person: number(rng), key })
        }
    }
}
pub fn key(rng: &mut Rng) -> core::Key {
    match rng.below(5) {
        0 => core::Key::Core(core_key(rng)),
        1 => core::Key::Tasks(tasks_key(rng)),
        2 => core::Key::People(people_key(rng)),
        3 => core::Key::Notes(notes_key(rng)),
        _ => core::Key::Accounts(accounts_key(rng)),
    }
}
pub fn accounts_key(rng: &mut Rng) -> jig_domain_accounts::Key {
    use jig_domain_accounts::Key;
    match rng.below(4) {
        0 => Key::Next,
        1 => Key::Account { number: narrow(rng) },
        2 => Key::Source { account: narrow(rng), number: narrow(rng) },
        _ => Key::Issued { number: narrow(rng) },
    }
}

pub fn funder(rng: &mut Rng) -> jig_domain_tasks::Funder {
    use jig_domain_tasks::Funder;
    match rng.below(4) {
        0 => Funder::Task(number(rng)),
        1 => Funder::Pool { project: narrow(rng), person: number(rng), period: number(rng) },
        2 => Funder::Recurring { project: narrow(rng), task: number(rng), period: number(rng) },
        _ => Funder::Period { project: narrow(rng), period: number(rng) },
    }
}
pub fn tasks_key(rng: &mut Rng) -> jig_domain_tasks::Key {
    use jig_domain_tasks::{Key, Newest, UpdateObject};
    match rng.below(22) {
        21 => Key::PlacedHold(number(rng)),
        0 => Key::Live(number(rng)),
        1 => Key::Stub(number(rng)),
        2 => Key::Ledger(funder(rng)),
        3 => Key::Writer(number(rng)),
        4 => Key::Pool(number(rng)),
        5 => Key::PersonProposal(number(rng)),
        6 => Key::Ended(number(rng)),
        13 => Key::Result { party: number(rng), position: number(rng) },
        17 => Key::EndedDelegate { requester: number(rng), task: number(rng) },
        19 => Key::EndedByParty {
            party: number(rng),
            label: narrow(rng),
            ended_at: Newest::of(number(rng)),
            task: number(rng),
        },
        20 => Key::EndedByProject {
            project: narrow(rng),
            label: narrow(rng),
            ended_at: Newest::of(number(rng)),
            task: number(rng),
        },
        18 => Key::Delegate { requester: number(rng), task: number(rng) },
        7 => Key::ClosedLedger(funder(rng)),
        8 => Key::DecidedProposal(number(rng)),
        9 => Key::History { task: number(rng), revision: number(rng) },
        10 => Key::Milestone { task: number(rng), position: number(rng) },
        11 => Key::Update {
            party: number(rng),
            at: Newest::of(number(rng)),
            task: Newest::of(number(rng)),
            object: match rng.below(3) {
                0 => UpdateObject::Result,
                1 => UpdateObject::Reply { message: Newest::of(number(rng)), activity: Newest::of(number(rng)) },
                _ => UpdateObject::Proposal { proposal: Newest::of(number(rng)) },
            },
        },
        _ => Key::Activity { task: number(rng), position: number(rng) },
    }
}
pub fn segment(rng: &mut Rng) -> Box<[u8]> {
    match rng.below(5) {
        0 => Box::new([]),
        1 => Box::new([0]),
        2 => Box::new([0, 0]),
        3 => Box::new([0, 255]),
        _ => Box::new([1]),
    }
}
pub fn pattern(rng: &mut Rng) -> jig_domain_notes::Pattern {
    use jig_domain_notes::Last;
    let mut segments = skein_lib::List::with_capacity(3);
    for _ in 0..rng.below(4) {
        segments.push(segment(rng)).expect("at most three segments");
    }
    let last = match rng.below(2) {
        0 => Last::Exact(segment(rng)),
        _ => Last::Open(segment(rng)),
    };
    jig_domain_notes::Pattern { segments: segments.into_boxed(), last }
}
pub fn notes_key(rng: &mut Rng) -> jig_domain_notes::Key {
    use jig_domain_notes::{Key, Scope};
    match rng.below(2) {
        0 => Key::Entry { name: number(rng) },
        _ => Key::Line {
            scope: match rng.below(4) {
                0 => Scope::Deployment,
                1 => Scope::Project { project: narrow(rng) },
                2 => Scope::Goal { project: narrow(rng), goal: number(rng) },
                _ => Scope::Resources {
                    project: narrow(rng),
                    connector: match rng.below(4) {
                        0 => 0,
                        1 => 1,
                        2 => 2,
                        _ => u16::MAX,
                    },
                    pattern: pattern(rng),
                },
            },
            name: number(rng),
        },
    }
}
pub fn pattern_key(segments: Box<[Box<[u8]>]>) -> jig_domain_notes::Key {
    use jig_domain_notes::{Key, Last, Pattern, Scope};
    Key::Line {
        scope: Scope::Resources {
            project: 1,
            connector: 1,
            pattern: Pattern { segments, last: Last::Exact(Box::new([])) },
        },
        name: 1,
    }
}

pub fn within(intervals: &[skein_kv::Range], key: &[u8]) -> bool {
    for interval in intervals {
        let before_end = match &interval.end {
            Some(end) => key < &**end,
            None => true,
        };
        if key >= &*interval.start && before_end {
            return true;
        }
    }
    false
}
