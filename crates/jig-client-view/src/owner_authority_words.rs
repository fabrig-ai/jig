//! Checked authority words and selected language (client/domain/view.md, 7).
//! Phrase IDs 3000..3022 are reserved for the owner authority vocabulary.
use crate::Owner;
use alloc::boxed::Box;
use skein_lib::{List, Wall, bytes::copy_of};
use skein_ui::phrases::{
    Form, Hole, HoleKind, HoleShape, Invalid, Language, Phrase, PhraseId, PhraseLimits, PhraseShape, Phrasebook,
    PluralClass, PluralRule, Segment, SpanUnit, WallFormat,
};

/// The closed reusable authority phrase vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthorityPhrase {
    Tools,
    Tasks,
    Depth,
    Spend,
    Notes,
    Grants,
    Delegation,
    Resources,
    Connector,
    Kind,
    Charter,
    Procedure,
    Role,
    Segments,
    Exact,
    Open,
    Cut,
    Deadline,
    OmittedSegments,
    ModelsDefault,
    ModelsAll,
    ModelsNamed,
    ModelsEmpty,
}
/// All phrases, in their stable owner vocabulary order.
pub const ALL: [AuthorityPhrase; 23] = [
    AuthorityPhrase::Tools,
    AuthorityPhrase::Tasks,
    AuthorityPhrase::Depth,
    AuthorityPhrase::Spend,
    AuthorityPhrase::Notes,
    AuthorityPhrase::Grants,
    AuthorityPhrase::Delegation,
    AuthorityPhrase::Resources,
    AuthorityPhrase::Connector,
    AuthorityPhrase::Kind,
    AuthorityPhrase::Charter,
    AuthorityPhrase::Procedure,
    AuthorityPhrase::Role,
    AuthorityPhrase::Segments,
    AuthorityPhrase::Exact,
    AuthorityPhrase::Open,
    AuthorityPhrase::Cut,
    AuthorityPhrase::Deadline,
    AuthorityPhrase::OmittedSegments,
    AuthorityPhrase::ModelsDefault,
    AuthorityPhrase::ModelsAll,
    AuthorityPhrase::ModelsNamed,
    AuthorityPhrase::ModelsEmpty,
];
impl AuthorityPhrase {
    /// Stable phrase identity, available when composing owner language data.
    #[must_use]
    pub fn id(self) -> PhraseId {
        let number = match self {
            Self::Tools => 3000,
            Self::Tasks => 3001,
            Self::Depth => 3002,
            Self::Spend => 3003,
            Self::Notes => 3004,
            Self::Grants => 3005,
            Self::Delegation => 3006,
            Self::Resources => 3007,
            Self::Connector => 3008,
            Self::Kind => 3009,
            Self::Charter => 3010,
            Self::Procedure => 3011,
            Self::Role => 3012,
            Self::Segments => 3013,
            Self::Exact => 3014,
            Self::Open => 3015,
            Self::Cut => 3016,
            Self::Deadline => 3017,
            Self::OmittedSegments => 3018,
            Self::ModelsDefault => 3019,
            Self::ModelsAll => 3020,
            Self::ModelsNamed => 3021,
            Self::ModelsEmpty => 3022,
        };
        PhraseId { owner: Owner::Jig.opaque(), number }
    }
    fn hole(self) -> Option<HoleKind> {
        match self {
            Self::Tools
            | Self::Tasks
            | Self::Depth
            | Self::Notes
            | Self::Connector
            | Self::Kind
            | Self::Charter
            | Self::Procedure
            | Self::Role => Some(HoleKind::Number),
            Self::Spend => Some(HoleKind::Amount),
            Self::Deadline => Some(HoleKind::Wall),
            Self::Grants
            | Self::Delegation
            | Self::Resources
            | Self::Segments
            | Self::Exact
            | Self::Open
            | Self::Cut
            | Self::OmittedSegments
            | Self::ModelsDefault
            | Self::ModelsAll
            | Self::ModelsNamed
            | Self::ModelsEmpty => None,
        }
    }
}
/// Phrase contracts independent of language word order.
#[must_use]
pub fn shapes() -> Box<[PhraseShape]> {
    let mut shapes = List::with_capacity(23);
    for phrase in ALL {
        let holes: Box<[HoleShape]> = match phrase.hole() {
            Some(kind) => Box::new([HoleShape { kind, bytes: 0 }]),
            None => Box::new([]),
        };
        shapes.push(PhraseShape { id: phrase.id(), holes, plural: None }).expect("closed authority shapes fit");
    }
    shapes.into_boxed()
}
/// Explicit English language data; callers select a checked loaded language.
#[must_use]
pub fn english() -> Language {
    let mut phrases = List::with_capacity(23);
    for phrase in ALL {
        let words: &[u8] = match phrase {
            AuthorityPhrase::Tools => b"Tool bits: ",
            AuthorityPhrase::Tasks => b"Delegate tasks: ",
            AuthorityPhrase::Depth => b"Delegate depth: ",
            AuthorityPhrase::Spend => b"Spend ceiling: ",
            AuthorityPhrase::Notes => b"Note scope bits: ",
            AuthorityPhrase::Grants => b"Grants",
            AuthorityPhrase::Delegation => b"Allowed delegation executors",
            AuthorityPhrase::Resources => b"Note resources",
            AuthorityPhrase::Connector => b"Connector: ",
            AuthorityPhrase::Kind => b"Kind: ",
            AuthorityPhrase::Charter => b"Charter: ",
            AuthorityPhrase::Procedure => b"Procedure: ",
            AuthorityPhrase::Role => b"Role: ",
            AuthorityPhrase::Segments => b"Literal segments",
            AuthorityPhrase::Exact => b"Exact ending",
            AuthorityPhrase::Open => b"Open ending",
            AuthorityPhrase::Cut => b"More are held than shown",
            AuthorityPhrase::Deadline => b"Deadline: ",
            AuthorityPhrase::OmittedSegments => "…".as_bytes(),
            AuthorityPhrase::ModelsDefault => b"Keep default models",
            AuthorityPhrase::ModelsAll => b"All models",
            AuthorityPhrase::ModelsNamed => b"Allowed models",
            AuthorityPhrase::ModelsEmpty => b"No models",
        };
        let segments: Box<[Segment]> = match phrase.hole() {
            Some(kind) => Box::new([Segment::Text(copy_of(words)), Segment::Hole { index: 0, kind }]),
            None => Box::new([Segment::Text(copy_of(words))]),
        };
        phrases
            .push(Phrase { id: phrase.id(), forms: Box::new([Form { class: PluralClass::Other, segments }]) })
            .expect("closed authority phrases fit");
    }
    Language {
        plural: PluralRule::English,
        span_unit: SpanUnit::Nanos,
        wall: WallFormat::Nanos,
        phrases: phrases.into_boxed(),
    }
}
/// A checked closed phrasebook with selected language and person's UTC offset.
#[derive(Debug)]
pub struct OwnerAuthorityWords {
    words: crate::words::Words,
}
impl OwnerAuthorityWords {
    /// Check every language whole before retaining it and its selected index.
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
        let book = Phrasebook::load(shapes(), languages, limits)?;
        Ok(Self { words: crate::words::Words::new(book, language, offset_minutes) })
    }
    /// Maximum saying capacity across all loaded languages.
    #[must_use]
    pub fn capacity(&self, phrase: AuthorityPhrase) -> u32 {
        self.words.book().capacity(phrase.id()).expect("whole closed phrasebook checked")
    }
    /// Say a checked phrase using typed holes in the selected language.
    #[must_use]
    pub fn say(&self, phrase: AuthorityPhrase, holes: &[Hole]) -> Box<[u8]> {
        self.words.book().say(self.words.language(), phrase.id(), holes).expect("checked phrase and typed facts").text
    }
    /// Apply the person's UTC offset before the language formats the instant.
    #[must_use]
    pub fn adjusted_wall(&self, at: Wall) -> Wall {
        self.words.adjusted_wall(at)
    }
    /// Borrow the same checked book for shared fragment helpers.
    #[must_use]
    pub fn words(&self) -> &crate::words::Words {
        &self.words
    }
}
