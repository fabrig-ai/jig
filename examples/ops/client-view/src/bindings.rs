//! Keep: owner-tagged bounded bindings, rebuilt each tree (view.md, 5).
//! Replace: application actions and family document identities.
use jig_client::contract::{ObjectKey, Revision};
use jig_client_view::Owner;
use jig_ops_client_domain::{Action, infrastructure};
use skein_lib::{List, Token};
use skein_ui::{Act, Binding};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Meaning {
    Action(Action),
    Until { object: Token, revision: Revision },
    NativeUntil { object: Token, revision: Revision },
    Object(ObjectKey),
    Section(jig_client_view::page::Section),
}
#[derive(Debug)]
struct Entry {
    token: Token,
    meaning: Meaning,
}
#[derive(Debug)]
pub struct Bindings {
    entries: List<Entry>,
    next: u64,
    connector: u16,
    text: u32,
}
impl Bindings {
    #[must_use]
    pub fn new(capacity: u32, text: u32, connector: u16) -> Self {
        Self { entries: List::with_capacity(capacity), next: 0, connector, text }
    }
    pub(crate) const fn text_limit(&self) -> u32 {
        self.text
    }
    pub fn begin(&mut self) {
        self.entries.clear();
    }
    #[must_use]
    pub fn insert(&mut self, meaning: Meaning) -> Binding {
        self.next = self.next.checked_add(1).expect("binding token fits");
        let token = Token::new(self.next);
        self.entries.push(Entry { token, meaning }).expect("binding capacity");
        Binding { owner: (Owner::Application { number: self.connector }).opaque(), token }
    }
    #[must_use]
    pub fn object(&self, token: Token) -> Option<ObjectKey> {
        for e in &self.entries {
            if e.token == token {
                return match &e.meaning {
                    Meaning::Object(key) => Some(*key),
                    Meaning::Action(_) | Meaning::Until { .. } | Meaning::NativeUntil { .. } | Meaning::Section(_) => {
                        None
                    }
                };
            }
        }
        None
    }
    #[must_use]
    pub fn section(&self, token: Token) -> Option<jig_client_view::page::Section> {
        for entry in &self.entries {
            if entry.token == token {
                return match &entry.meaning {
                    Meaning::Section(section) => Some(*section),
                    Meaning::Action(_) | Meaning::Until { .. } | Meaning::NativeUntil { .. } | Meaning::Object(_) => {
                        None
                    }
                };
            }
        }
        None
    }
}
#[must_use]
pub fn decode(bindings: &Bindings, token: Token, act: Act) -> Option<Action> {
    for e in &bindings.entries {
        if e.token == token {
            return match &e.meaning {
                Meaning::Action(action) => match act {
                    Act::Press | Act::Toggle | Act::Submit => Some(action.clone()),
                    Act::Input { .. } | Act::Drawing { .. } => None,
                },
                Meaning::Until { object, revision } => match act {
                    Act::Input { value } => {
                        if value.len() > usize::try_from(bindings.text).ok()? {
                            return None;
                        }
                        Some(Action::Infrastructure(infrastructure::Act::Until {
                            object: *object,
                            revision: *revision,
                            value: crate::time::wall(skein_ui::number(&value)?)?,
                        }))
                    }
                    Act::Press | Act::Toggle | Act::Submit | Act::Drawing { .. } => None,
                },
                Meaning::NativeUntil { object, revision } => match act {
                    Act::Input { value } => {
                        if value.len() > usize::try_from(bindings.text).ok()? {
                            return None;
                        }
                        Some(Action::Infrastructure(infrastructure::Act::NativeUntil {
                            object: *object,
                            revision: *revision,
                            value: skein_ui::number(&value)?,
                        }))
                    }
                    Act::Press | Act::Toggle | Act::Submit | Act::Drawing { .. } => None,
                },
                Meaning::Object(_) | Meaning::Section(_) => None,
            };
        }
    }
    None
}
