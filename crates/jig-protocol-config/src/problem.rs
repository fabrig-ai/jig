//! Bounded diagnostics at paths from the whole document root (configuration.md, 2).
use alloc::boxed::Box;
use skein_lib::{Decimal, List, Writer, bytes};
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Segment {
    Member(Box<[u8]>),
    Index(u32),
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Path {
    pub segments: List<Segment>,
}
impl Path {
    #[must_use]
    pub fn root() -> Self {
        Self { segments: List::with_capacity(0) }
    }
    #[must_use]
    pub fn child(&self, segment: Segment) -> Self {
        let mut segments = List::with_capacity(self.segments.len().checked_add(1).expect("document path fits u32"));
        for item in &self.segments {
            segments.push(item.clone()).expect("path capacity includes ancestors");
        }
        segments.push(segment).expect("path capacity includes child");
        Self { segments }
    }
    #[must_use]
    pub fn member(&self, name: &[u8]) -> Self {
        self.child(Segment::Member(bytes::copy_of(name)))
    }
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Problem {
    pub path: Path,
    pub what: What,
}
/// A required codec bound, read from the derivation and its generated ceilings.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CodecBound {
    pub field: &'static [u8],
    pub needed: u32,
    pub ceiling: u32,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum What {
    Json,
    Unknown,
    /// This member's setting lives in the store when the application passes defaults.
    Stored,
    Missing,
    Duplicate,
    Kind,
    Bound,
    Form,
    Unique,
    Refers,
    Plaintext,
    Confidential,
    NoProvider,
    Digest,
    /// PKCS#8 PEM only; includes the local key-generation instruction, never key bytes.
    ProviderKey {
        generate: &'static str,
    },
    Reserve {
        charter: u32,
    },
    Budget {
        charter: u32,
    },
    Grace {
        template: u32,
    },
    Operations {
        operations: u32,
        delegates: u32,
        goals: u32,
    },
    Message,
    Silence,
    StopBound,
    /// Policy bytes exceed the effective people codec envelope (configuration.md, 6).
    PolicyEnvelope {
        value: u64,
        maximum: u64,
        /// Absent only when the configured derivation overflows.
        bound: Option<CodecBound>,
    },
    /// Amendment bytes exceed the envelope; retain the binding bound even though the words are short.
    AmendmentEnvelope {
        value: u32,
        maximum: u32,
        bound: Option<CodecBound>,
    },
    /// An independent people-v13 field exceeds its generated ceiling.
    CodecCeiling {
        bound: CodecBound,
    },
    /// The people derivation cannot represent a bound.
    CodecArithmetic,
    /// The combined store codec refuses a family outside the people envelope.
    StoreCodec,
    Override,
    Unreadable,
    /// A relative path that is empty or has a `..` component (configuration.md, 10).
    Relative,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Problems {
    pub problems: List<Problem>,
    pub past: u32,
}
pub const PROBLEMS: u32 = 32;
impl Problems {
    #[must_use]
    pub fn empty() -> Self {
        Self { problems: List::with_capacity(PROBLEMS), past: 0 }
    }
    pub fn push(&mut self, path: Path, what: What) {
        if self.problems.push(Problem { path, what }).is_err() {
            self.past = self.past.saturating_add(1);
        }
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.problems.is_empty() && self.past == 0
    }
}

impl Problem {
    /// Render the codec-envelope diagnostic, including this problem's document path.
    /// Other configuration problems keep their existing structured vocabulary.
    #[must_use]
    pub fn codec_message(&self) -> Option<Box<[u8]>> {
        let value;
        let maximum;
        let bound;
        let detailed;
        match self.what {
            What::PolicyEnvelope { value: configured, maximum: admitted, bound: required } => {
                value = configured;
                maximum = admitted;
                bound = required;
                detailed = true;
            }
            What::AmendmentEnvelope { value: configured, maximum: admitted, bound: required } => {
                value = u64::from(configured);
                maximum = u64::from(admitted);
                bound = required;
                detailed = false;
            }
            What::CodecCeiling { bound } => return codec_ceiling_message(&self.path, bound),
            What::CodecArithmetic => {
                return codec_text_message(&self.path, b"people-v13 codec bound arithmetic overflows.");
            }
            What::StoreCodec => {
                return codec_text_message(&self.path, b"effective limits exceed the combined store codec envelope.");
            }
            What::Json
            | What::Unknown
            | What::Stored
            | What::Missing
            | What::Duplicate
            | What::Kind
            | What::Bound
            | What::Form
            | What::Unique
            | What::Refers
            | What::Plaintext
            | What::Confidential
            | What::NoProvider
            | What::Digest
            | What::ProviderKey { .. }
            | What::Reserve { .. }
            | What::Budget { .. }
            | What::Grace { .. }
            | What::Operations { .. }
            | What::Message
            | What::Silence
            | What::StopBound
            | What::Override
            | What::Unreadable
            | What::Relative => return None,
        }
        let value = Decimal::of(value);
        let maximum = Decimal::of(maximum);
        let (needed, ceiling) = match bound {
            Some(bound) => (Decimal::of(u64::from(bound.needed)), Decimal::of(u64::from(bound.ceiling))),
            None => (Decimal::of(0), Decimal::of(0)),
        };
        let mut parts = List::with_capacity(13);
        parts.push(value.as_bytes()).expect("diagnostic parts");
        parts.push(b" exceeds the people-v13 codec envelope of ").expect("diagnostic parts");
        parts.push(maximum.as_bytes()).expect("diagnostic parts");
        parts.push(b" bytes").expect("diagnostic parts");
        if detailed {
            match bound {
                Some(bound) => {
                    parts.push(b" (").expect("diagnostic parts");
                    parts.push(bound.field).expect("diagnostic parts");
                    parts.push(b" would need ").expect("diagnostic parts");
                    parts.push(needed.as_bytes()).expect("diagnostic parts");
                    parts.push(b" bytes; ceiling ").expect("diagnostic parts");
                    parts.push(ceiling.as_bytes()).expect("diagnostic parts");
                    parts.push(b")").expect("diagnostic parts");
                }
                None => {
                    parts.push(b" (derived byte bounds overflow)").expect("diagnostic parts");
                }
            }
        }
        parts.push(b".").expect("diagnostic parts");
        codec_parts_message(&self.path, parts.as_slice())
    }
}

fn codec_ceiling_message(path: &Path, bound: CodecBound) -> Option<Box<[u8]>> {
    let needed = Decimal::of(u64::from(bound.needed));
    let ceiling = Decimal::of(u64::from(bound.ceiling));
    codec_parts_message(
        path,
        &[bound.field, b" would need ", needed.as_bytes(), b"; people-v13 codec ceiling ", ceiling.as_bytes(), b"."],
    )
}
fn codec_text_message(path: &Path, text: &[u8]) -> Option<Box<[u8]>> {
    codec_parts_message(path, &[text])
}
fn codec_parts_message(path: &Path, parts: &[&[u8]]) -> Option<Box<[u8]>> {
    let mut length = 2_usize; // colon and space between the path and its words
    for (index, segment) in path.segments.iter().enumerate() {
        length = length.checked_add(match segment {
            Segment::Member(name) => name.len().checked_add(usize::from(index > 0))?,
            Segment::Index(index) => Decimal::of(u64::from(*index)).as_bytes().len().checked_add(2)?,
        })?;
    }
    for part in parts {
        length = length.checked_add(part.len())?;
    }
    let mut writer = Writer::new(length);
    for (index, segment) in path.segments.iter().enumerate() {
        match segment {
            Segment::Member(name) => {
                if index > 0 {
                    writer.put(b".").expect("diagnostic path measured");
                }
                writer.put(name).expect("diagnostic path measured");
            }
            Segment::Index(index) => {
                writer.put(b"[").expect("diagnostic path measured");
                writer.put(Decimal::of(u64::from(*index)).as_bytes()).expect("diagnostic path measured");
                writer.put(b"]").expect("diagnostic path measured");
            }
        }
    }
    writer.put(b": ").expect("diagnostic measured");
    for part in parts {
        writer.put(part).expect("diagnostic measured");
    }
    Some(writer.finish())
}
