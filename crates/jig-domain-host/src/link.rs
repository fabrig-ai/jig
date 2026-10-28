//! The engine link's vocabulary (domain/hosts.md, sections 2 and 6.1).
use crate::{Answer, Assignment, Grant, Hosting, Turn};
use alloc::boxed::Box;
use skein_lib::{Duration, Token};

#[derive(PartialEq, Eq, Debug)]
pub enum LinkDown {
    Sources { sources: Box<[SourceName]> },
    Check { name: SourceName },
    SignOut { name: SourceName },
    Assign { assignment: Assignment },
    Message { run: Token, attempt: Token, name: Token, sender: Box<[u8]>, words: Box<[u8]>, finish_requested: bool },
    Relayed { run: Token, attempt: Token, call: Box<[u8]>, answer: Relayed },
    TurnKept { run: Token, attempt: Token, turn: u32 },
    Acknowledge { run: Token, attempt: Token },
    Cancel { run: Token, attempt: Token },
    Park { run: Token, attempt: Token },
    Grant { run: Token, attempt: Token, grant: Grant },
}

#[derive(PartialEq, Eq, Debug)]
pub enum LinkUp {
    /// A piece of live content (hosts.md, 6.5).
    Piece {
        run: Token,
        attempt: Token,
        piece: crate::Piece,
    },
    Logins {
        logins: Box<[Login]>,
    },
    SignedOut {
        name: SourceName,
        result: SignOutResult,
    },
    Activity {
        run: Token,
        attempt: Token,
        activity: crate::Activity,
    },
    Unauthorized {
        run: Token,
        attempt: Token,
        account: u32,
        generation: u64,
    },
    Limited {
        run: Token,
        attempt: Token,
        account: u32,
        reset: Option<Duration>,
    },
    AccountRefused {
        run: Token,
        attempt: Token,
        account: u32,
        code: Box<[u8]>,
        reason: Box<[u8]>,
    },
    Turn {
        run: Token,
        attempt: Token,
        turn: Turn,
    },
    Call {
        run: Token,
        attempt: Token,
        call: Box<[u8]>,
        tool: Box<[u8]>,
        writes: bool,
        input: Box<[u8]>,
        deadline: Duration,
    },
    Answer {
        run: Token,
        attempt: Token,
        answer: Answer,
    },
}

#[derive(PartialEq, Eq, Hash, Debug)]
pub enum Relayed {
    Answer { error: bool, text: Box<[u8]> },
    Busy,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ModelBound {
    pub name: Box<[u8]>,
    pub window: u32,
    pub output: u32,
}

/// Maximum endpoint identities admitted in one host hello (hosts.md, 8.1).
/// Independent of the number of model declarations.
pub const PROGRAM_ENDPOINTS: u32 = 64;

/// Reject an oversized or ambiguous endpoint declaration before retaining it.
/// A quadratic prefix scan, bounded by 64 endpoints: at most 2,016 comparisons.
#[must_use]
pub fn valid_program_endpoints(endpoints: &[u32]) -> bool {
    if endpoints.len() > usize::try_from(PROGRAM_ENDPOINTS).expect("u32 fits usize") {
        return false;
    }
    for (at, endpoint) in endpoints.iter().enumerate() {
        if endpoints.get(..at).expect("prefix").contains(endpoint) {
            return false;
        }
    }
    true
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Bounds {
    pub program_endpoints: Box<[u32]>,
    pub models: Box<[ModelBound]>,
    pub turns: u32,
    pub spend: u64,
    pub time: Duration,
    pub waiting: Duration,
    pub checks: Duration,
    pub message_bytes: u32,
    pub inbox: u32,
    pub unacknowledged: u32,
}

#[derive(PartialEq, Eq, Hash, Debug)]
pub struct Hello {
    pub slots: u32,
    pub stop_bound: Duration,
    pub workstreams: Box<[u64]>,
    pub hosting: Box<[Hosting]>,
    pub bounds: Bounds,
}

/// A configuration-directory name (accounts.md, 3.4), never an account number.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct SourceName {
    length: u8,
    bytes: [u8; 32],
}
impl SourceName {
    #[must_use]
    pub fn new(value: &[u8]) -> Option<Self> {
        if value.is_empty() || value.len() > 32 {
            return None;
        }
        let mut bytes = [0; 32];
        for (index, byte) in value.iter().enumerate() {
            if !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && *byte != b'-' {
                return None;
            }
            *bytes.get_mut(index)? = *byte;
        }
        Some(Self { length: u8::try_from(value.len()).ok()?, bytes })
    }
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..usize::from(self.length)).expect("validated name")
    }
}
/// The program's bounded, printable payment method, without login contents.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LoginMethod {
    length: u8,
    bytes: [u8; 32],
}
impl LoginMethod {
    #[must_use]
    pub fn new(value: &[u8]) -> Option<Self> {
        if value.is_empty() || value.len() > 32 {
            return None;
        }
        let mut bytes = [0; 32];
        for (index, byte) in value.iter().enumerate() {
            if !(0x21..=0x7e).contains(byte) {
                return None;
            }
            *bytes.get_mut(index)? = *byte;
        }
        Some(Self { length: u8::try_from(value.len()).ok()?, bytes })
    }
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..usize::from(self.length)).expect("validated method")
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SourceFailure {
    Missing,
    Deadline,
    Unreadable,
    Exited,
    Unconfigured,
    Unknown,
    Closed,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LoginState {
    Usable,
    NotSignedIn,
    SignedInOtherwise(LoginMethod),
    Failed { reason: SourceFailure, detail: Box<[u8]> },
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Login {
    pub name: SourceName,
    pub state: LoginState,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SignOutResult {
    SignedOut,
    Failed { reason: SourceFailure, detail: Box<[u8]> },
}

#[cfg(test)]
mod source_tests {
    use super::{LoginMethod, SourceName};
    #[test]
    fn source_names_and_methods_have_separate_grammars() {
        for value in [b"a".as_slice(), b"source-123", &[b'z'; 32]] {
            assert_eq!(SourceName::new(value).expect("source name").as_bytes(), value);
        }
        for value in [b"".as_slice(), b"A", b"../a", b"a_b", &[b'a'; 33], b"\x80"] {
            assert!(SourceName::new(value).is_none(), "{value:?}");
        }
        for value in [b"!~".as_slice(), b"claude.ai", &[b'X'; 32]] {
            assert_eq!(LoginMethod::new(value).expect("printable method").as_bytes(), value);
        }
        for value in [b"".as_slice(), b"two words", b"\x7f", b"\n", &[b'x'; 33]] {
            assert!(LoginMethod::new(value).is_none(), "{value:?}");
        }
    }
}
