//! Bounds for notes (jig's domain/engine.md, sections 10 and 13).

/// The parent's fixed bounds for notes and store pages.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    /// Scope indexes kept in memory at once.
    pub scopes: u32,
    /// Entries admitted in one scope.
    pub entries_per_scope: u32,
    /// Largest connector pattern, in bytes.
    pub pattern_bytes: u32,
    /// Largest one-line description, in bytes.
    pub description_bytes: u32,
    /// Largest body, in bytes.
    pub body_bytes: u32,
    /// Task references in one entry.
    pub references: u32,
    /// Records in one store load page.
    pub load_rows: u32,
    /// Index lines in one answer.
    pub lines: u32,
    /// Entries in one recall page.
    pub recalled: u32,
}

/// Representability of the child limits (programming-model.md, 6.3).
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    limits.scopes > 0
        && limits.entries_per_scope > 0
        && limits.load_rows > 0
        && limits.recalled > 0
        && limits.scopes.checked_mul(limits.entries_per_scope).is_some()
}
