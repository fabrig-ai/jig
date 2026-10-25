//! Core-owned address fields (protocol/store.md, section 4.1).
use jig_domain_core::{CallKey, CoreKey, ProjectionKey};
use jig_domain_people::Proposer;
use skein_kv::{KeyReader, KeyWriter};
pub(super) fn write(writer: &mut KeyWriter, key: &CoreKey) {
    match key {
        CoreKey::Settings(key) => super::settings::write(writer, key),
        CoreKey::Deployment => {
            writer.tag(0);
        }
        CoreKey::Projection(key) => {
            writer.tag(1);
            projection(writer, key);
        }
        CoreKey::RunProof(task) => {
            writer.tag(2);
            writer.u64(*task);
        }
        CoreKey::Call(key) => {
            writer.tag(3);
            writer.u64(key.task);
            writer.u64(key.attempt);
            writer.u64(u64::from(key.completion));
            writer.u64(u64::from(key.position));
        }
        CoreKey::Turn { task, attempt, turn } => {
            writer.tag(4);
            writer.u64(*task);
            writer.u64(*attempt);
            writer.u64(u64::from(*turn));
        }
        CoreKey::Terminal { task, attempt } => {
            writer.tag(5);
            writer.u64(*task);
            writer.u64(*attempt);
        }
        CoreKey::ProposalDecision { proposer, proposal } => {
            writer.tag(6);
            match proposer {
                Proposer::Task(number) => {
                    writer.tag(0);
                    writer.u64(*number);
                }
                Proposer::Party(number) => {
                    writer.tag(1);
                    writer.u64(*number);
                }
            }
            writer.u64(*proposal);
        }
        CoreKey::EscalationDecision { task, revision } => {
            writer.tag(7);
            writer.u64(*task);
            writer.u64(*revision);
        }
    }
}
fn projection(writer: &mut KeyWriter, key: &ProjectionKey) {
    match key {
        ProjectionKey::Failure { goal, connector } => {
            writer.tag(4);
            writer.u64(*goal);
            writer.u64(u64::from(*connector));
        }
        ProjectionKey::Goal(goal) => {
            writer.tag(0);
            writer.u64(*goal);
        }
        ProjectionKey::Task { goal, task } => {
            writer.tag(1);
            writer.u64(*goal);
            writer.u64(*task);
        }
        ProjectionKey::Lifecycle { goal, task, position } => {
            writer.tag(2);
            writer.u64(*goal);
            writer.u64(*task);
            writer.u64(*position);
        }
        ProjectionKey::Revision { goal, task, revision } => {
            writer.tag(3);
            writer.u64(*goal);
            writer.u64(*task);
            writer.u64(*revision);
        }
    }
}
pub(super) fn read(reader: &mut KeyReader<'_>) -> Option<CoreKey> {
    Some(match reader.tag()? {
        0 => CoreKey::Deployment,
        1 => CoreKey::Projection(read_projection(reader)?),
        2 => CoreKey::RunProof(reader.u64()?),
        3 => CoreKey::Call(CallKey {
            task: reader.u64()?,
            attempt: reader.u64()?,
            completion: u32::try_from(reader.u64()?).ok()?,
            position: u32::try_from(reader.u64()?).ok()?,
        }),
        4 => CoreKey::Turn { task: reader.u64()?, attempt: reader.u64()?, turn: u32::try_from(reader.u64()?).ok()? },
        5 => CoreKey::Terminal { task: reader.u64()?, attempt: reader.u64()? },
        6 => {
            let proposer = match reader.tag()? {
                0 => Proposer::Task(reader.u64()?),
                1 => Proposer::Party(reader.u64()?),
                _ => return None,
            };
            CoreKey::ProposalDecision { proposer, proposal: reader.u64()? }
        }
        8 => CoreKey::Settings(super::settings::read(reader, false)?),
        9 => CoreKey::Settings(super::settings::read(reader, true)?),
        7 => CoreKey::EscalationDecision { task: reader.u64()?, revision: reader.u64()? },
        _ => return None,
    })
}
fn read_projection(reader: &mut KeyReader<'_>) -> Option<ProjectionKey> {
    Some(match reader.tag()? {
        0 => ProjectionKey::Goal(reader.u64()?),
        4 => ProjectionKey::Failure { goal: reader.u64()?, connector: u16::try_from(reader.u64()?).ok()? },
        1 => ProjectionKey::Task { goal: reader.u64()?, task: reader.u64()? },
        2 => ProjectionKey::Lifecycle { goal: reader.u64()?, task: reader.u64()?, position: reader.u64()? },
        3 => ProjectionKey::Revision { goal: reader.u64()?, task: reader.u64()?, revision: reader.u64()? },
        _ => return None,
    })
}
