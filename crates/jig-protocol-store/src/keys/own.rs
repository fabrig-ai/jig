//! Component rows (protocol/store.md, sections 5.3 and 8.1).
use skein_kv::{KeyReader, KeyWriter};
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Own {
    Format,
    Transcript { task: u64 },
    Attempt { task: u64, attempt: u64 },
    Layouts,
    Wires,
}
pub(super) fn write(writer: &mut KeyWriter, key: &Own) {
    match key {
        Own::Wires => {
            writer.tag(4);
        }
        Own::Layouts => {
            writer.tag(3);
        }
        Own::Format => {
            writer.tag(0);
        }
        Own::Transcript { task } => {
            writer.tag(1);
            writer.u64(*task);
        }
        Own::Attempt { task, attempt } => {
            writer.tag(2);
            writer.u64(*task);
            writer.u64(*attempt);
        }
    }
}
pub(super) fn read(reader: &mut KeyReader<'_>) -> Option<Own> {
    Some(match reader.tag()? {
        0 => Own::Format,
        3 => Own::Layouts,
        4 => Own::Wires,
        1 => Own::Transcript { task: reader.u64()? },
        2 => Own::Attempt { task: reader.u64()?, attempt: reader.u64()? },
        _ => return None,
    })
}
