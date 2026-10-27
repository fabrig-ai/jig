//! Closed request fields translate from their supplied owner vocabulary
//! (client/domain/contract.md, 3), preserving absent words and numeric widths.
use jig_client_domain::contract as client;
use jig_domain_people as people;

#[must_use]
pub fn wake(value: client::OwnerWakePolicy) -> people::WakePolicy {
    people::WakePolicy {
        markers: value.markers,
        words: wake_rule(value.words),
        notices: wake_rule(value.notices),
        news: wake_rule(value.news),
        results: match value.results {
            client::ResultsWake::Never => people::ResultsWake::Never,
            client::ResultsWake::Each => people::ResultsWake::Each,
            client::ResultsWake::LastOrFailure => people::ResultsWake::LastOrFailure,
        },
        questions: value.questions,
        answers: value.answers,
        timers: value.timers,
    }
}

fn wake_rule(value: client::WakeRule) -> people::WakeRule {
    match value {
        client::WakeRule::Never => people::WakeRule::Never,
        client::WakeRule::Immediate => people::WakeRule::Immediate,
        client::WakeRule::Batch { count, age } => people::WakeRule::Batch { count, age },
    }
}

#[must_use]
pub fn person_answer(value: &client::PersonAnswer) -> people::PersonResult {
    match value {
        client::PersonAnswer::Choice { option, reason } => {
            people::PersonResult::Verdict { code: *option, words: Box::from(reason.bytes()) }
        }
        client::PersonAnswer::Approved { state, words } => people::PersonResult::Approved {
            state: state.clone(),
            words: words.as_ref().map(|value| Box::from(value.bytes())),
        },
        client::PersonAnswer::Words(value) => people::PersonResult::Report { words: Box::from(value.bytes()) },
        client::PersonAnswer::Cannot { reason } => people::PersonResult::Failure { reason: Box::from(reason.bytes()) },
    }
}

#[must_use]
pub fn seen(value: client::SeenThrough) -> people::Seen {
    match value {
        client::SeenThrough::Results { through } => people::Seen::Results { through },
        client::SeenThrough::Conversation { task: chat, through } => people::Seen::Conversation { task: chat, through },
    }
}

pub(super) fn tracking(value: client::Tracking) -> people::Tracking {
    match value {
        client::Tracking::Tracked { priority } => people::Tracking::Tracked { priority },
        client::Tracking::Untracked => people::Tracking::Untracked,
    }
}
pub(super) fn template(value: &client::TemplateOverride) -> people::TemplateOverride {
    people::TemplateOverride {
        template: value.template,
        executor: value.executor.map(template_executor),
        budget: value.budget.map(template_budget),
        deadline: value.deadline.map(template_deadline),
        priority: value.priority,
    }
}
fn template_executor(value: client::TemplateExecutor) -> people::TemplateExecutor {
    match value {
        client::TemplateExecutor::Charter(v) => people::TemplateExecutor::Charter(v),
        client::TemplateExecutor::Chosen => people::TemplateExecutor::Chosen,
    }
}
fn template_budget(value: client::TemplateBudget) -> people::TemplateBudget {
    match value {
        client::TemplateBudget::Amount(v) => people::TemplateBudget::Amount(v),
        client::TemplateBudget::AsAllowed { cap } => people::TemplateBudget::AsAllowed { cap },
        client::TemplateBudget::Given => people::TemplateBudget::Given,
    }
}
fn template_deadline(value: client::TemplateDeadline) -> people::TemplateDeadline {
    match value {
        client::TemplateDeadline::None => people::TemplateDeadline::None,
        client::TemplateDeadline::Span(v) => people::TemplateDeadline::Span(v),
        client::TemplateDeadline::Given => people::TemplateDeadline::Given,
    }
}

#[cfg(test)]
mod tests {
    const UTF8_TEXTS: [&[u8]; 1] = [b"~/\0"];

    use super::*;

    fn text(value: &[u8]) -> client::Text {
        client::Text::new(Box::from(value), 64).expect("the complete supplied fixture is admitted")
    }

    #[test]
    fn person_answer_translation_keeps_opaque_approval_state_absent_or_empty_words_and_failures() {
        for words in [None, Some(text(b"")), Some(text(b"literal/words"))] {
            let value = client::PersonAnswer::Approved { state: Box::from([0, 255, b'/', 1]), words: words.clone() };
            assert_eq!(
                person_answer(&value),
                people::PersonResult::Approved {
                    state: Box::from([0, 255, b'/', 1]),
                    words: words.map(|value| Box::from(value.bytes()))
                }
            );
        }
        let cases = [
            (
                client::PersonAnswer::Choice { option: u32::MAX, reason: text(b"") },
                people::PersonResult::Verdict { code: u32::MAX, words: Box::new([]) },
            ),
            (
                client::PersonAnswer::Words(text(b"report/bytes")),
                people::PersonResult::Report { words: Box::from(b"report/bytes".as_slice()) },
            ),
            (
                client::PersonAnswer::Cannot { reason: text(UTF8_TEXTS[0]) },
                people::PersonResult::Failure { reason: Box::from(UTF8_TEXTS[0]) },
            ),
        ];
        for (given, expected) in cases {
            assert_eq!(person_answer(&given), expected);
        }
    }

    #[test]
    fn seen_translation_preserves_closed_variants_and_full_width_identities() {
        assert_eq!(
            seen(client::SeenThrough::Results { through: u64::MAX }),
            people::Seen::Results { through: u64::MAX }
        );
        assert_eq!(
            seen(client::SeenThrough::Conversation { task: u64::MAX, through: u64::MAX }),
            people::Seen::Conversation { task: u64::MAX, through: u64::MAX }
        );
    }
}
