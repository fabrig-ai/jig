//! Shared admitted program names and fixed worker membership (domain/root.md, 2).
//! These are configuration values; the crate holds no channel or domain state.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
use alloc::boxed::Box;

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
    /// Reconstitute a bounded stored name; admission's spelling rules are
    /// applied when a name is declared, never again during a cold start.
    #[must_use]
    pub fn from_stored(value: &[u8]) -> Option<Self> {
        if value.len() > 32 {
            return None;
        }
        let mut bytes = [0; 32];
        for (index, byte) in value.iter().enumerate() {
            *bytes.get_mut(index)? = *byte;
        }
        Some(Self { length: u8::try_from(value.len()).ok()?, bytes })
    }
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..usize::from(self.length)).expect("validated name")
    }
}

/// A position admitted from immutable configured membership.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Worker {
    position: u32,
}
impl Worker {
    #[must_use]
    pub const fn number(self) -> u32 {
        self.position
    }
}

/// A startup value exceeds the priced configured membership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    Workers { limit: u32 },
    WorkerName { worker: u32, limit: u32 },
}

/// Owns the immutable names and mints positions only within that membership.
#[derive(Debug)]
pub struct Membership {
    names: Box<[Box<[u8]>]>,
}
impl Membership {
    #[must_use]
    pub fn empty() -> Self {
        Self { names: Box::new([]) }
    }
    /// # Errors
    /// A configured worker count or name exceeds its priced bound.
    pub fn check(names: &[Box<[u8]>], workers: u32, name_bytes: u32) -> Result<(), Problem> {
        let Ok(count) = u32::try_from(names.len()) else { return Err(Problem::Workers { limit: workers }) };
        if count > workers {
            return Err(Problem::Workers { limit: workers });
        }
        for (index, name) in names.iter().enumerate() {
            let worker = u32::try_from(index).expect("membership count fits u32");
            let Ok(bytes) = u32::try_from(name.len()) else {
                return Err(Problem::WorkerName { worker, limit: name_bytes });
            };
            if bytes > name_bytes {
                return Err(Problem::WorkerName { worker, limit: name_bytes });
            }
        }
        Ok(())
    }
    /// # Errors
    /// A configured worker count or name exceeds its priced bound.
    pub fn new(names: Box<[Box<[u8]>]>, workers: u32, name_bytes: u32) -> Result<Self, Problem> {
        Self::check(&names, workers, name_bytes)?;
        Ok(Self { names })
    }
    #[must_use]
    pub fn len(&self) -> u32 {
        u32::try_from(self.names.len()).expect("admitted membership count fits u32")
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
    #[must_use]
    pub fn worker(&self, position: u32) -> Option<Worker> {
        self.get(position)?;
        Some(Worker { position })
    }
    #[must_use]
    pub fn get(&self, position: u32) -> Option<&[u8]> {
        Some(self.names.get(usize::try_from(position).ok()?)?)
    }
    #[must_use]
    pub fn name(&self, worker: Worker) -> &[u8] {
        self.get(worker.position).expect("admitted configured position")
    }
}

#[cfg(test)]
mod tests;

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
