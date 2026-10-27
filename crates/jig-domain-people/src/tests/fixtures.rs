//! Shared amendment payloads for the tests.
/// An amendment retaining every field, with the supplied explanation.
#[must_use]
pub(super) fn unchanged(reason: &[u8]) -> Amendment {
    Amendment {
        llm: None,
        tracked: None,
        addressed: None,
        instructions: None,
        spec: None,
        wake: None,
        dependencies: None,
        authority: None,
        reason: reason.into(),
    }
}

use super::*;
const NO_TOOLS: u64 = 0;
pub(super) fn member(words: &[u8]) -> Member {
    Member {
        llm: None,
        deadline: Deadline::Named,
        executor: TaskExecutor::Agent { charter: 1 },
        words: words.into(),
        parameters: Box::new([]),
        inputs: Box::new([]),
        budget: Budget::Named,
        named: Box::new([]),
        contract: Contract::Report { words: 8 },
        authority: Authority {
            tools: NO_TOOLS,
            models: None,
            grants: Box::new([]),
            delegation: Delegation { kinds: Box::new([]), tasks: 0, depth: 0 },
            spend: 1,
            deadline: None,
            notes: 0,
            note_resources: Box::new([]),
        },
        wake: WakePolicy {
            markers: 0,
            words: WakeRule::Never,
            notices: WakeRule::Never,
            news: WakeRule::Never,
            results: ResultsWake::Never,
            questions: false,
            answers: false,
            timers: false,
        },
        dependencies: Box::new([]),
        tracked: None,
        label: 0,
        flags: Flags::PLAIN,
    }
}
