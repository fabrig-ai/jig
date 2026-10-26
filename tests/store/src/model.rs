//! Independent whole-commit prefixes (protocol/store.md, 14).
use jig_domain_core as core;
use jig_protocol_store::Write;
use std::collections::BTreeMap;

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct State {
    pub rows: BTreeMap<core::Key, core::Record>,
    pub application: BTreeMap<Box<[u8]>, Box<[u8]>>,
    pub turns: BTreeMap<u64, Vec<core::TurnRecord>>,
}

#[derive(Debug)]
pub struct Model {
    pub acknowledged: u64,
    pub prefixes: BTreeMap<u64, State>,
}

impl Default for Model {
    fn default() -> Self {
        Self { acknowledged: 0, prefixes: BTreeMap::from([(0, State::default())]) }
    }
}

impl Model {
    pub fn stage(&mut self, number: u64, writes: &[Write]) {
        assert_eq!(number, self.acknowledged + 1, "only one uncertain commit");
        let mut state = self.prefixes.get(&self.acknowledged).expect("acknowledged prefix").clone();
        for write in writes {
            match write {
                Write::Wires => {}
                Write::Save(core::Record::Core(core::CoreRecord::Turn(turn))) => {
                    state.turns.entry(turn.task).or_default().push(turn.clone());
                }
                Write::CopySettingsRevision(number) => {
                    let key = core::Key::Core(core::CoreKey::Settings(core::settings::Key::Held(*number)));
                    let core::Record::Core(core::CoreRecord::Settings(core::settings::Record::Revision {
                        revision,
                        ..
                    })) = state.rows.get(&key).expect("held revision").clone()
                    else {
                        panic!("revision row")
                    };
                    let row = core::Record::Core(core::CoreRecord::Settings(core::settings::Record::Revision {
                        history: true,
                        revision,
                    }));
                    state.rows.insert(row.key(), row);
                }
                Write::Save(row) => {
                    state.rows.insert(row.key(), row.clone());
                }
                Write::Erase(key) => {
                    state.rows.remove(key);
                }
                Write::DropTranscript { task } => {
                    state.turns.remove(task);
                }
                Write::Put { key, value } => {
                    state.application.insert(key.clone(), value.clone());
                }
                Write::Remove { key } => {
                    state.application.remove(key);
                }
            }
        }
        self.prefixes.insert(number, state);
    }

    pub fn acknowledge(&mut self, number: u64) {
        assert_eq!(number, self.acknowledged + 1);
        self.acknowledged = number;
    }

    /// A patched open's commits are the store's own (store.md 10.2): the
    /// acknowledged prefix moves to its last as the root wrote it, and the
    /// fixtures' model, not this one, checks the records' patched bytes.
    pub fn patched(&mut self, last: u64) {
        let state = self.prefixes.get(&self.acknowledged).expect("acknowledged prefix").clone();
        self.prefixes.insert(last, state);
        self.acknowledged = last;
    }

    pub fn recovered(&mut self, last: u64) {
        assert!(self.prefixes.contains_key(&last), "recovered a model prefix");
        self.acknowledged = last;
        self.prefixes.retain(|number, _| *number <= last);
    }
}
