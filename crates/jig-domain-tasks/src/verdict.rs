//! Closed named fields accompany a verdict without interpreting their values
//! (domain/tasks.md, section 3; programming-model.md, sections 4.4 and 6).
use crate::{FieldRule, ResultField, Verdict};

pub(crate) fn fields_bytes(fields: &[ResultField]) -> Option<usize> {
    if fields.len() > usize::try_from(crate::VERDICT_FIELDS).expect("field ceiling fits usize") {
        return None;
    }
    let mut bytes = 0_usize;
    for field in fields {
        if field.name.is_empty() {
            return None;
        }
        bytes = bytes.checked_add(field.name.len())?.checked_add(field.value.len())?;
    }
    Some(bytes)
}

pub(crate) fn rules_valid(choice: &Verdict, limit: u32) -> bool {
    let bound = usize::try_from(limit).expect("u32 fits usize");
    if choice.fields.len() > usize::try_from(crate::VERDICT_FIELDS.min(limit)).expect("field bound fits usize") {
        return false;
    }
    let mut bytes = usize::try_from(choice.words).expect("u32 fits usize");
    for (at, field) in choice.fields.iter().enumerate() {
        if field.name.is_empty() {
            return false;
        }
        bytes = match bytes.checked_add(field.name.len()) {
            Some(bytes) => bytes,
            None => return false,
        };
        bytes = match bytes.checked_add(usize::try_from(field.max).expect("u32 fits usize")) {
            Some(bytes) if bytes <= bound => bytes,
            Some(_) | None => return false,
        };
        for earlier in choice.fields.iter().take(at) {
            if earlier.name == field.name {
                return false;
            }
        }
    }
    bytes <= bound
}

pub(crate) fn fields_valid(rules: &[FieldRule], fields: &[ResultField]) -> bool {
    if fields.len() != rules.len() {
        return false;
    }
    for (at, field) in fields.iter().enumerate() {
        for earlier in fields.iter().take(at) {
            if earlier.name == field.name {
                return false;
            }
        }
        let mut found = false;
        for rule in rules {
            if field.name == rule.name {
                if field.value.len() > usize::try_from(rule.max).expect("u32 fits usize") {
                    return false;
                }
                found = true;
            }
        }
        if !found {
            return false;
        }
    }
    true
}
