//! Closed owner budget/ledger sayings (client/domain/view.md, section 7).
//! IDs 6000..6012 are independent of the shared Funder sayings.
use crate::words::Words;
use crate::{Owner, proposed_task_words};
use alloc::boxed::Box;
use jig_client::words::{self, DecisionWord as D, Word};
const FUNDERS: [Word; 4] = [
    Word::Decision(D::FunderTask),
    Word::Decision(D::FunderPool),
    Word::Decision(D::FunderRecurring),
    Word::Decision(D::FunderPeriod),
];
use skein_lib::{List, bytes::copy_of};
use skein_ui::phrases::{
    Form, HoleKind, HoleShape, Invalid, Language, Phrase, PhraseId, PhraseLimits, PhraseShape, Phrasebook, PluralClass,
    PluralRule, Segment, SpanUnit, WallFormat,
};

/// Exact accounting labels and explicit ledger state/timing labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetPhrase {
    Budget,
    Spent,
    SpentBelow,
    Reserved,
    Left,
    Funder,
    Deadline,
    Source,
    Parent,
    Open,
    Closed,
    Began,
    Next,
}
/// Stable independent phrase order for normal Words composition.
pub const ALL: [BudgetPhrase; 13] = [
    BudgetPhrase::Budget,
    BudgetPhrase::Spent,
    BudgetPhrase::SpentBelow,
    BudgetPhrase::Reserved,
    BudgetPhrase::Left,
    BudgetPhrase::Funder,
    BudgetPhrase::Deadline,
    BudgetPhrase::Source,
    BudgetPhrase::Parent,
    BudgetPhrase::Open,
    BudgetPhrase::Closed,
    BudgetPhrase::Began,
    BudgetPhrase::Next,
];
impl BudgetPhrase {
    /// Identity available to the normal checked Words composition.
    #[must_use]
    pub fn id(self) -> PhraseId {
        let number = match self {
            Self::Budget => 6000,
            Self::Spent => 6001,
            Self::SpentBelow => 6002,
            Self::Reserved => 6003,
            Self::Left => 6004,
            Self::Funder => 6005,
            Self::Deadline => 6006,
            Self::Source => 6007,
            Self::Parent => 6008,
            Self::Open => 6009,
            Self::Closed => 6010,
            Self::Began => 6011,
            Self::Next => 6012,
        };
        PhraseId { owner: Owner::Jig.opaque(), number }
    }
    fn hole(self) -> Option<HoleKind> {
        match self {
            Self::Budget | Self::Spent | Self::SpentBelow | Self::Reserved | Self::Left => Some(HoleKind::Amount),
            Self::Deadline | Self::Began | Self::Next => Some(HoleKind::Wall),
            Self::Funder | Self::Source | Self::Parent | Self::Open | Self::Closed => None,
        }
    }
}

/// Budget-only closed shapes; shared Funder shapes are composed by their owner.
#[must_use]
pub fn shapes() -> Box<[PhraseShape]> {
    let mut shapes = List::with_capacity(13);
    for phrase in ALL {
        let holes: Box<[HoleShape]> = match phrase.hole() {
            Some(kind) => Box::new([HoleShape { kind, bytes: 0 }]),
            None => Box::new([]),
        };
        shapes.push(PhraseShape { id: phrase.id(), holes, plural: None }).expect("closed budget shapes fit");
    }
    shapes.into_boxed()
}

/// Budget-only English data for selected-language composition, never built by rendering.
#[must_use]
pub fn language() -> Language {
    let mut phrases = List::with_capacity(13);
    for phrase in ALL {
        let text: &[u8] = match phrase {
            BudgetPhrase::Budget => b"Budget: ",
            BudgetPhrase::Spent => b"Spent directly: ",
            BudgetPhrase::SpentBelow => b"Settled spend below: ",
            BudgetPhrase::Reserved => b"Reserved for funded work: ",
            BudgetPhrase::Left => b"Left: ",
            BudgetPhrase::Funder => b"Funded by",
            BudgetPhrase::Deadline => b"Deadline: ",
            BudgetPhrase::Source => b"Ledger source",
            BudgetPhrase::Parent => b"Parent funder",
            BudgetPhrase::Open => b"Ledger is open",
            BudgetPhrase::Closed => b"Ledger is closed",
            BudgetPhrase::Began => b"Period began: ",
            BudgetPhrase::Next => b"Next period begins: ",
        };
        let segments: Box<[Segment]> = match phrase.hole() {
            Some(kind) => Box::new([Segment::Text(copy_of(text)), Segment::Hole { index: 0, kind }]),
            None => Box::new([Segment::Text(copy_of(text))]),
        };
        phrases
            .push(Phrase { id: phrase.id(), forms: Box::new([Form { class: PluralClass::Other, segments }]) })
            .expect("closed budget phrases fit");
    }
    Language {
        plural: PluralRule::English,
        span_unit: SpanUnit::Nanos,
        wall: WallFormat::Nanos,
        phrases: phrases.into_boxed(),
    }
}

/// Exact closed size of the budget and shared proposed-task vocabulary.
#[must_use]
pub fn standalone_phrase_count() -> u32 {
    u32::try_from(ALL.len())
        .expect("budget phrase count fits")
        .checked_add(u32::try_from(proposed_task_words::ALL.len()).expect("part phrase count fits"))
        .expect("combined phrase count fits")
        .checked_add(u32::try_from(FUNDERS.len()).expect("closed funder count"))
        .expect("public funder sayings fit")
}

fn standalone_shapes() -> Box<[PhraseShape]> {
    let mut combined = List::with_capacity(standalone_phrase_count());
    for shape in shapes() {
        combined.push(shape).expect("budget shape count is closed");
    }
    for shape in proposed_task_words::shapes() {
        combined.push(shape).expect("shared proposed-task shape count is closed");
    }
    for word in FUNDERS {
        combined.push(word.shape(0)).expect("public funder shapes fit");
    }
    combined.into_boxed()
}

/// Explicit standalone English data, including the shared Funder vocabulary.
/// Normal applications instead compose the independent budget language data.
#[must_use]
pub fn standalone_language() -> Language {
    let mut result = language();
    let mut combined = List::with_capacity(standalone_phrase_count());
    for phrase in result.phrases {
        combined.push(phrase).expect("budget phrase count is closed");
    }
    for phrase in proposed_task_words::language().phrases {
        combined.push(phrase).expect("shared proposed-task phrase count is closed");
    }
    let public = words::english();
    for word in FUNDERS {
        for phrase in &public.phrases {
            if phrase.id == word.id() {
                combined.push(phrase.clone()).expect("public funder words fit");
            }
        }
    }
    result.phrases = combined.into_boxed();
    result
}

/// One checked book for standalone use, also borrowed by normal renderer APIs.
#[derive(Debug)]
pub struct OwnerBudgetWords {
    words: Words,
}
impl OwnerBudgetWords {
    /// Supplied languages include budget sayings and the shared proposed-task sayings.
    pub fn load(
        languages: Box<[Language]>,
        limits: &PhraseLimits,
        language: u16,
        offset_minutes: i32,
    ) -> Result<Self, Invalid> {
        if usize::from(language) >= languages.len() {
            return Err(Invalid::Language);
        }
        if u64::from(offset_minutes.unsigned_abs()).checked_mul(60_000_000_000).is_none() {
            return Err(Invalid::Overflow);
        }
        let book = Phrasebook::load(standalone_shapes(), languages, limits)?;
        Ok(Self { words: Words::new(book, language, offset_minutes) })
    }
    /// Borrow the same selected language and offset; no second book or fallback.
    #[must_use]
    pub fn words(&self) -> &Words {
        &self.words
    }
}
