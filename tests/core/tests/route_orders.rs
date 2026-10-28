//! Synchronous sibling routes remain in one decision under watermark pressure.
use jig_core_world::direct::Driver;
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{ReplyTo, Token};

const FIRST: Token = Token::new(9701);
const LATER: Token = Token::new(9702);
const FIRST_KEY: [u8; 16] = [71; 16];
const LATER_KEY: [u8; 16] = [72; 16];
const PROJECT: u32 = 2;
// MakeProject fans out to Roles, OpenPeriod, PeriodNext and Decided.
const HANDOFFS: usize = 4;
const ONCE: usize = 1;

/// One entrance's route finishes within its decision, its widest fan-out
/// included, before a later entrance runs, with new work waiting at the
/// watermark.
#[test]
fn a_wide_route_finishes_within_its_decision_before_a_later_entrance() {
    let mut driver = Driver::project_maker(9701);
    let session = driver.session();
    driver.transactions.clear();
    driver.released.clear();
    driver.send(ask(session, FIRST, FIRST_KEY, people::Ask::MakeProject { name: "Tally".into(), policy: 1 }));
    driver.collect();
    driver.fill_to_watermark();
    let returned = driver.returned;
    driver.send_retaining(ask(
        session,
        LATER,
        LATER_KEY,
        people::Ask::Rename { project: PROJECT, name: "Later".into() },
    ));
    assert_eq!(driver.returned, returned + 1, "later change handed back at the watermark");
    assert_eq!(driver.events.len(), ONCE, "later change waits to be offered again");
    driver.settle();

    let first: Vec<_> = driver
        .transactions
        .iter()
        .enumerate()
        .filter(|(_, writes)| {
            writes.iter().any(|write|
        matches!(write, root::Write::Save(root::Record::Core(core::Record::People(people::Stored::Answer { key, .. })))
            if key.key == FIRST_KEY)
    )
        })
        .collect();
    assert_eq!(first.len(), ONCE, "the first entrance has one transaction");
    let (first_index, writes) = first[0];
    // Every sibling's durable consequence, plus the policy, is in that transaction.
    let mut consequences = Vec::new();
    for write in writes {
        match write {
            root::Write::Save(root::Record::Core(core::Record::People(people::Stored::Policy {
                project: PROJECT,
                name,
                ..
            }))) if name.as_ref() == "Tally" => consequences.push("policy"),
            root::Write::Save(root::Record::Core(core::Record::People(people::Stored::Roles {
                project: PROJECT,
                ..
            }))) => consequences.push("roles"),
            root::Write::Save(root::Record::Core(core::Record::Tasks(tasks::Stored::Ledger(
                tasks::FundingRecord { funder: tasks::Funder::Period { project: PROJECT, .. }, .. },
            )))) => consequences.push("period"),
            root::Write::Save(root::Record::Core(core::Record::People(people::Stored::Answer {
                key,
                outcome,
                ..
            }))) if key.key == FIRST_KEY => {
                assert_eq!(*outcome, people::Outcome::ProjectMade { project: PROJECT });
                consequences.push("answer");
            }
            _ => {}
        }
    }
    assert_eq!(consequences, ["policy", "roles", "period", "answer"], "all {HANDOFFS} hand-offs finish together");
    let later: Vec<_> = driver
        .transactions
        .iter()
        .enumerate()
        .filter(|(_, writes)| {
            writes.iter().any(|write|
        matches!(write, root::Write::Save(root::Record::Core(core::Record::People(people::Stored::Answer { key, .. })))
            if key.key == LATER_KEY)
    )
        })
        .collect();
    assert_eq!(later.len(), ONCE);
    assert!(later[0].0 > first_index);
    let replies: Vec<_> = driver
        .released
        .iter()
        .filter_map(|request| {
            if let root::Request::Deliver(root::Delivery::Core(core::Held::PeopleReply { to, reply })) = request
                && [ReplyTo::new(FIRST), ReplyTo::new(LATER)].contains(to)
            {
                Some((to, reply))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        replies,
        [
            (&ReplyTo::new(FIRST), &people::Reply::Outcome(people::Outcome::ProjectMade { project: PROJECT })),
            (&ReplyTo::new(LATER), &people::Reply::Outcome(people::Outcome::Renamed { project: PROJECT })),
        ]
    );
    assert!(driver.domain.quiescent() && !driver.stopped);
}

fn ask(session: u64, reply: Token, key: [u8; 16], ask: people::Ask) -> root::Event {
    root::Event::Core(core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(reply),
        sign_in: session,
        key,
        ask,
    }))
}
