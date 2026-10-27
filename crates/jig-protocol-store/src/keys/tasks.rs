//! Task and funding addresses (protocol/store.md, section 4.1).
use jig_domain_tasks::{Funder, Key, Newest, UpdateObject};
use skein_kv::{KeyReader, KeyWriter};
pub(super) fn write(writer: &mut KeyWriter, key: &Key) {
    match key {
        Key::PlacedHold(hold) => {
            writer.tag(21);
            writer.u64(*hold);
        }
        Key::Live(task) => {
            writer.tag(0);
            writer.u64(*task);
        }
        Key::Stub(task) => {
            writer.tag(1);
            writer.u64(*task);
        }
        Key::Ledger(value) => {
            writer.tag(2);
            funder(writer, value);
        }
        Key::Writer(slot) => {
            writer.tag(3);
            writer.u64(*slot);
        }
        Key::Pool(slot) => {
            writer.tag(4);
            writer.u64(*slot);
        }
        Key::PersonProposal(proposal) => {
            writer.tag(5);
            writer.u64(*proposal);
        }
        Key::Ended(task) => {
            writer.tag(6);
            writer.u64(*task);
        }
        Key::ClosedLedger(value) => {
            writer.tag(7);
            funder(writer, value);
        }
        Key::DecidedProposal(proposal) => {
            writer.tag(8);
            writer.u64(*proposal);
        }
        Key::History { task, revision } => {
            writer.tag(9);
            writer.u64(*task);
            writer.u64(*revision);
        }
        Key::Milestone { task, position } => {
            writer.tag(10);
            writer.u64(*task);
            writer.u64(*position);
        }
        Key::Activity { task, position } => {
            writer.tag(11);
            writer.u64(*task);
            writer.u64(*position);
        }
        Key::Result { party, position } => {
            writer.tag(13);
            writer.u64(*party);
            writer.u64(*position);
        }
        Key::EndedByParty { party, label, ended_at, task } => {
            ended_index(writer, 19, *party, *label, *ended_at, *task);
        }
        Key::EndedByProject { project, label, ended_at, task } => {
            ended_index(writer, 20, u64::from(*project), *label, *ended_at, *task);
        }
        Key::EndedDelegate { requester, task } => {
            writer.tag(17);
            writer.u64(*requester);
            writer.u64(*task);
        }
        Key::Delegate { requester, task } => {
            writer.tag(18);
            writer.u64(*requester);
            writer.u64(*task);
        }
        Key::Update { party, at, task, object } => {
            writer.tag(12);
            writer.u64(*party);
            writer.u64(at.raw());
            writer.u64(task.raw());
            update_object(writer, object);
        }
    }
}
pub(super) fn read(reader: &mut KeyReader<'_>) -> Option<Key> {
    Some(match reader.tag()? {
        21 => Key::PlacedHold(reader.u64()?),
        0 => Key::Live(reader.u64()?),
        1 => Key::Stub(reader.u64()?),
        2 => Key::Ledger(read_funder(reader)?),
        3 => Key::Writer(reader.u64()?),
        4 => Key::Pool(reader.u64()?),
        5 => Key::PersonProposal(reader.u64()?),
        6 => Key::Ended(reader.u64()?),
        7 => Key::ClosedLedger(read_funder(reader)?),
        8 => Key::DecidedProposal(reader.u64()?),
        9 => Key::History { task: reader.u64()?, revision: reader.u64()? },
        10 => Key::Milestone { task: reader.u64()?, position: reader.u64()? },
        11 => Key::Activity { task: reader.u64()?, position: reader.u64()? },
        12 => Key::Update {
            party: reader.u64()?,
            at: Newest::from_raw(reader.u64()?),
            task: Newest::from_raw(reader.u64()?),
            object: match reader.tag()? {
                0 => UpdateObject::Result,
                1 => UpdateObject::Reply {
                    message: Newest::from_raw(reader.u64()?),
                    activity: Newest::from_raw(reader.u64()?),
                },
                2 => UpdateObject::Proposal { proposal: Newest::from_raw(reader.u64()?) },
                _ => return None,
            },
        },
        13 => Key::Result { party: reader.u64()?, position: reader.u64()? },
        19 => Key::EndedByParty {
            party: reader.u64()?,
            label: u32::try_from(reader.u64()?).ok()?,
            ended_at: Newest::from_raw(reader.u64()?),
            task: reader.u64()?,
        },
        20 => Key::EndedByProject {
            project: u32::try_from(reader.u64()?).ok()?,
            label: u32::try_from(reader.u64()?).ok()?,
            ended_at: Newest::from_raw(reader.u64()?),
            task: reader.u64()?,
        },
        17 => Key::EndedDelegate { requester: reader.u64()?, task: reader.u64()? },
        18 => Key::Delegate { requester: reader.u64()?, task: reader.u64()? },
        _ => return None,
    })
}
fn funder(writer: &mut KeyWriter, key: &Funder) {
    match key {
        Funder::Task(task) => {
            writer.tag(0);
            writer.u64(*task);
        }
        Funder::Pool { project, person, period } => {
            writer.tag(1);
            writer.u64(u64::from(*project));
            writer.u64(*person);
            writer.u64(*period);
        }
        Funder::Recurring { project, task, period } => {
            writer.tag(2);
            writer.u64(u64::from(*project));
            writer.u64(*task);
            writer.u64(*period);
        }
        Funder::Period { project, period } => {
            writer.tag(3);
            writer.u64(u64::from(*project));
            writer.u64(*period);
        }
    }
}
fn read_funder(reader: &mut KeyReader<'_>) -> Option<Funder> {
    Some(match reader.tag()? {
        0 => Funder::Task(reader.u64()?),
        1 => Funder::Pool { project: u32::try_from(reader.u64()?).ok()?, person: reader.u64()?, period: reader.u64()? },
        2 => Funder::Recurring {
            project: u32::try_from(reader.u64()?).ok()?,
            task: reader.u64()?,
            period: reader.u64()?,
        },
        3 => Funder::Period { project: u32::try_from(reader.u64()?).ok()?, period: reader.u64()? },
        _ => return None,
    })
}

fn update_object(writer: &mut KeyWriter, object: &UpdateObject) {
    match object {
        UpdateObject::Proposal { proposal } => {
            writer.tag(2);
            writer.u64(proposal.raw());
        }
        UpdateObject::Result => {
            writer.tag(0);
        }
        UpdateObject::Reply { message, activity } => {
            writer.tag(1);
            writer.u64(message.raw());
            writer.u64(activity.raw());
        }
    }
}

fn ended_index(writer: &mut KeyWriter, tag: u8, owner: u64, label: u32, at: Newest, task: u64) {
    writer.tag(tag);
    writer.u64(owner);
    writer.u64(u64::from(label));
    writer.u64(at.raw());
    writer.u64(task);
}
