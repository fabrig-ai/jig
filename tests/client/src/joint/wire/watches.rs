//! Live watches preserve their exact owner subject, window and run position
//! (client/domain/contract.md, 7). The sign-in supplies the authenticated party;
//! translation does not choose visibility or reinterpret a role's project.
use jig_client_domain::contract as client;
use jig_domain_core::{Event, WatchRefusal, documents};
use jig_domain_views as views;
use skein_lib::Token;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Invalid {
    Settings,
    DeploymentRequester,
}

/// # Errors
/// Refuses a mismatched executor role project or any whole window/subject allocation above the receiving bounds.
pub fn watch(watcher: Token, sign_in: u64, value: &client::Subject, default_rows: u32) -> Result<Event, Invalid> {
    if let client::Subject::Run { task, turn, offset } = value {
        return Ok(Event::WatchRun {
            watcher,
            sign_in,
            task: *task,
            turn: turn.map(|name| views::TurnName { attempt: name.attempt, turn: name.number, retry: name.retry }),
            offset: *offset,
        });
    }
    let (subject, window) = subject(value, default_rows)?;
    Ok(Event::Watch { watcher, sign_in, subject, window })
}

fn subject(value: &client::Subject, default_rows: u32) -> Result<(documents::Watched, documents::Window), Invalid> {
    let (subject, rows) = match value {
        client::Subject::Settings => return Err(Invalid::Settings),
        client::Subject::Me => (documents::Watched::Frame, default_rows),
        client::Subject::Task { task } => (documents::Watched::Task { task: *task }, default_rows),
        client::Subject::Tree { task, window } => (documents::Watched::Tree { task: *task }, *window),
        client::Subject::Chips { tasks } => (documents::Watched::Chips { tasks: tasks.clone() }, default_rows),
        client::Subject::Inbox { window } => (documents::Watched::Inbox, *window),
        client::Subject::Tasks { query, window } => {
            (documents::Watched::Query { query: super::queries::query(query)? }, *window)
        }
        client::Subject::Run { task, turn: _, offset: _ } => (documents::Watched::Run { task: *task }, default_rows),
        client::Subject::Accounts => (documents::Watched::Accounts, default_rows),
        client::Subject::Deployment => (documents::Watched::Deployment, default_rows),
    };
    Ok((subject, documents::Window { rows, after: None }))
}

#[must_use]
pub fn ended(value: views::End) -> client::WatchEnd {
    match value {
        views::End::Unwatched => client::WatchEnd::Closed,
        views::End::Gone => client::WatchEnd::SubjectEnded,
        views::End::TooLinked { joins } => client::WatchEnd::TooLinked { joins },
        views::End::NotAllowed => client::WatchEnd::NotAllowed,
        views::End::SignedOut => client::WatchEnd::SignedOut,
        views::End::Stopping => client::WatchEnd::Stopping,
    }
}

#[must_use]
pub fn refused(value: WatchRefusal) -> client::WatchEnd {
    match value {
        WatchRefusal::SignedOut => client::WatchEnd::SignedOut,
        WatchRefusal::NotAllowed => client::WatchEnd::NotAllowed,
        WatchRefusal::Unknown => client::WatchEnd::Unknown,
        WatchRefusal::Busy => client::WatchEnd::Busy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_ended_watch_selector_keeps_its_exact_live_query_identity_without_a_terminal_outcome() {
        let query = client::TaskQuery {
            project: Some(u32::MAX),
            requester: Some(client::OwnerRequester::Person(1)),
            root: Some(u64::MAX),
            parent: None,
            label: None,
            phase: Some(client::PhaseClass::Ended),
            executor: None,
            tracked: None,
            order: client::QueryOrder::Changed,
        };
        let actual = super::super::queries::query(&query).expect("ended query");
        assert_eq!(actual.phase, Some(documents::PhaseClass::Ended));
        assert_eq!(actual.root, Some(u64::MAX));
        assert_eq!(actual.requester, Some(jig_domain_tasks::Party::Person(1)));
        assert_eq!(actual.executor, None);
    }
    use skein_lib::List;

    #[test]
    fn run_watch_translation_keeps_absent_and_full_width_turn_positions_on_the_actual_producer() {
        for turn in [None, Some(client::ProvisionalTurnName { attempt: u64::MAX, number: u32::MAX, retry: u32::MAX })] {
            let result = watch(Token::new(71), 19, &client::Subject::Run { task: u64::MAX, turn, offset: u32::MAX }, 2)
                .expect("the complete supplied fixture is admitted");
            let Event::WatchRun { watcher, sign_in, task, turn: actual, offset } = result else {
                panic!("run positions require the named producer")
            };
            assert_eq!(watcher, Token::new(71));
            assert_eq!(sign_in, 19);
            assert_eq!(task, u64::MAX);
            assert_eq!(offset, u32::MAX);
            assert_eq!(
                actual,
                turn.map(|name| views::TurnName { attempt: name.attempt, turn: name.number, retry: name.retry })
            );
        }
    }

    #[test]
    fn watch_subject_translation_keeps_allocated_chip_identities_authentication_and_explicit_windows() {
        let mut chips = List::with_capacity(4);
        chips.push(u64::MAX).expect("the complete supplied fixture is admitted");
        chips.push(1).expect("the complete supplied fixture is admitted");
        let values = [
            ("Me", client::Subject::Me, documents::Watched::Frame, 5),
            ("Task", client::Subject::Task { task: u64::MAX }, documents::Watched::Task { task: u64::MAX }, 5),
            (
                "Tree",
                client::Subject::Tree { task: 9, window: u32::MAX },
                documents::Watched::Tree { task: 9 },
                u32::MAX,
            ),
            ("Chips", client::Subject::Chips { tasks: chips.clone() }, documents::Watched::Chips { tasks: chips }, 5),
            ("Inbox", client::Subject::Inbox { window: 0 }, documents::Watched::Inbox, 0),
            ("Deployment", client::Subject::Deployment, documents::Watched::Deployment, 5),
        ];
        for (label, value, expected, rows) in values {
            assert_eq!(
                subject(&value, 5).expect("the complete supplied fixture is admitted"),
                (expected, documents::Window { rows, after: None }),
                "{label}"
            );
        }
    }

    #[test]
    fn a_task_watch_query_keeps_every_key_and_refuses_a_deployment_requester() {
        let query = client::TaskQuery {
            project: Some(8),
            requester: None,
            root: Some(u64::MAX),
            parent: None,
            label: None,
            phase: Some(client::PhaseClass::Working),
            executor: Some(client::ExecutorKind::Procedure),
            tracked: None,
            order: client::QueryOrder::Changed,
        };
        assert_eq!(
            super::super::queries::query(&query).expect("query"),
            documents::TaskQuery {
                project: Some(8),
                requester: None,
                root: Some(u64::MAX),
                parent: None,
                label: None,
                phase: Some(documents::PhaseClass::Working),
                executor: Some(documents::ExecutorKind::Procedure),
                tracked: None,
                order: documents::QueryOrder::Changed
            }
        );
        let query = client::TaskQuery { requester: Some(client::OwnerRequester::Deployment { project: 8 }), ..query };
        assert_eq!(super::super::queries::query(&query), Err(Invalid::DeploymentRequester));
    }
}
