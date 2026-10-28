use super::revisions::LIMITS;
use crate::{Contract, LimitKey, NumericLimit, TaskResult, run};
use alloc::boxed::Box;

#[test]
fn immutable_result_words_have_a_fixed_ceiling_and_equality_is_admitted() {
    let contract = Contract::Report { words: 3 };
    let past = TaskResult::Report { words: Box::from(b"four".as_slice()) };
    assert_eq!(
        run::result_limit(&contract, &past, &LIMITS),
        Some(NumericLimit { key: LimitKey::ContractWords, value: 3 })
    );
    let equal = TaskResult::Report { words: Box::from(b"tri".as_slice()) };
    assert_eq!(run::result_limit(&contract, &equal, &LIMITS), None);
    let wrong = TaskResult::Verdict { code: 1, words: Box::new([]), fields: Box::new([]) };
    assert_eq!(run::result_limit(&contract, &wrong, &LIMITS), None);
}
#[test]
fn an_oversized_failure_reason_names_result_bytes_and_equality_is_admitted() {
    let mut limits = LIMITS;
    limits.result_bytes = 2;
    let contract = Contract::Report { words: 2 };
    let past = TaskResult::Failure { reason: Box::from(b"abc".as_slice()) };
    assert_eq!(
        run::result_limit(&contract, &past, &limits),
        Some(NumericLimit { key: LimitKey::ResultBytes, value: 2 })
    );
    let equal = TaskResult::Failure { reason: Box::from(b"ab".as_slice()) };
    assert_eq!(run::result_limit(&contract, &equal, &limits), None);
}
