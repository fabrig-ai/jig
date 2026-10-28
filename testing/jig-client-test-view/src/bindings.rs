//! Family meanings are rebuilt with each tree; no token is reused (view.md, 5).
use jig_client_domain::contract::{ObjectKey, Revision};
use jig_client_test_domain::first;
use jig_client_view::Owner;
use skein_lib::{List, Token};
use skein_ui::{Act, Binding};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Meaning {
    SignIn,
    Action(first::Act),
    Parameters { object: Token, revision: Revision },
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
    text: u32,
}
impl Bindings {
    #[must_use]
    pub fn new(capacity: u32, text: u32) -> Self {
        Self { entries: List::with_capacity(capacity), next: 0, text }
    }
    pub(crate) const fn text_limit(&self) -> u32 {
        self.text
    }
    pub fn begin(&mut self) {
        self.entries.clear();
    }
    pub fn insert(&mut self, meaning: Meaning) -> Binding {
        self.next = self.next.checked_add(1).expect("family tokens fit");
        let token = Token::new(self.next);
        self.entries.push(Entry { token, meaning }).expect("family meanings fit");
        Binding { owner: (Owner::Application { number: jig_client_test_domain::FIRST }).opaque(), token }
    }
    #[must_use]
    pub fn object(&self, token: Token) -> Option<ObjectKey> {
        for entry in &self.entries {
            if entry.token == token {
                return match &entry.meaning {
                    Meaning::Object(key) => Some(*key),
                    Meaning::SignIn | Meaning::Action(_) | Meaning::Parameters { .. } | Meaning::Section(_) => None,
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
                    Meaning::SignIn | Meaning::Action(_) | Meaning::Parameters { .. } | Meaning::Object(_) => None,
                };
            }
        }
        None
    }
    #[must_use]
    pub fn sign_in(&self, token: Token) -> bool {
        for entry in &self.entries {
            if entry.token == token && entry.meaning == Meaning::SignIn {
                return true;
            }
        }
        false
    }
}
#[must_use]
pub fn decode(bindings: &Bindings, token: Token, act: Act) -> Option<first::Act> {
    for entry in &bindings.entries {
        if entry.token == token {
            return match &entry.meaning {
                Meaning::Action(action) => match act {
                    Act::Press | Act::Submit | Act::Toggle => Some(action.clone()),
                    Act::Input { .. } | Act::Drawing { .. } => None,
                },
                Meaning::Parameters { object, revision } => match act {
                    Act::Input { value } => {
                        if value.len() > usize::try_from(bindings.text).expect("text fits usize") {
                            return None;
                        }
                        Some(first::Act::Parameters {
                            object: *object,
                            revision: *revision,
                            value: skein_ui::number(&value)?,
                        })
                    }
                    Act::Press | Act::Submit | Act::Toggle | Act::Drawing { .. } => None,
                },
                Meaning::SignIn | Meaning::Object(_) | Meaning::Section(_) => None,
            };
        }
    }
    None
}
