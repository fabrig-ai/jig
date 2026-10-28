//! Fields retain text and count domain writes; client/domain/view.md, 18.5.
//! Jig 21 adds draft and confirmation policies. Person edits never advance writes.
use jig_client::contract::Text;
/// Why a person is typing into a field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Purpose {
    /// The first words of a chat.
    NewChat,
    /// Words for a task.
    Composer { task: u64 },
    /// The open answer form words.
    Answer,
    /// The open confirmation reason.
    Reason,
    /// Amendment words.
    Amendment,
    /// Amended instructions.
    Instructions,
    /// The description of the note being corrected.
    NoteDescription,
    /// The whole body of the note being corrected.
    NoteBody,
    /// A new goal specification.
    NewGoal,
}

/// Text and the count of domain writes that the view observes.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Field {
    pub text: Text,
    pub writes: u64,
}

use jig_client::contract::Limit;
use skein_lib::Map;
/// A bounded set of purposes, shared by pages and confirmations.
#[derive(Debug)]
pub struct Fields {
    rows: Map<Purpose, Field>,
    text: u32,
}
impl Fields {
    /// Allocates the configured purposes; jig 21 owns their draft policy.
    #[must_use]
    pub fn new(capacity: u32, text: u32) -> Self {
        Self { rows: Map::with_capacity(capacity), text }
    }
    /// The current text and its write count.
    #[must_use]
    pub fn get(&self, purpose: Purpose) -> Option<&Field> {
        self.rows.get(&purpose)
    }
    /// Removes a purpose when its page or confirmation policy no longer keeps it.
    pub fn remove(&mut self, purpose: Purpose) -> Option<Field> {
        self.rows.remove(&purpose)
    }
    /// A person's edit changes text and never advances the domain-write count.
    pub fn edit(&mut self, purpose: Purpose, text: Text) -> Result<(), Limit> {
        self.set(purpose, text, false)
    }
    /// A domain write advances the count before publishing the new text.
    pub fn write(&mut self, purpose: Purpose, text: Text) -> Result<(), Limit> {
        self.set(purpose, text, true)
    }
    /// Removes hidden composers when their party changes; unrelated forms remain owned by their policies.
    pub(crate) fn clear_composers(&mut self) {
        for _ in 0..self.rows.len() {
            let mut removed = None;
            for (purpose, _) in &self.rows {
                match purpose {
                    Purpose::Composer { .. } => {
                        removed = Some(*purpose);
                        break;
                    }
                    Purpose::NewChat
                    | Purpose::Answer
                    | Purpose::Reason
                    | Purpose::Amendment
                    | Purpose::Instructions
                    | Purpose::NoteDescription
                    | Purpose::NoteBody
                    | Purpose::NewGoal => {}
                }
            }
            match removed {
                Some(purpose) => {
                    self.rows.remove(&purpose);
                }
                None => break,
            }
        }
    }
    fn set(&mut self, purpose: Purpose, text: Text, written: bool) -> Result<(), Limit> {
        if u64::try_from(text.bytes().len()).expect("text length fits u64") > u64::from(self.text) {
            return Err(Limit::Words { limit: self.text });
        }
        match self.rows.get_mut(&purpose) {
            Some(field) => {
                if written {
                    field.writes = field.writes.checked_add(1).ok_or(Limit::Fields)?;
                }
                field.text = text;
                Ok(())
            }
            None => {
                let writes = u64::from(written);
                match self.rows.insert(purpose, Field { text, writes }) {
                    Ok(_) => Ok(()),
                    Err(_) => Err(Limit::Fields),
                }
            }
        }
    }
}
