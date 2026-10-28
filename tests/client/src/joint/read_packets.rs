//! Retained original read routes at the protocol boundary (contract.md, 6).
//! Whole page admission and typed identity precede consuming a terminal route.
use super::{
    packets::Incoming,
    transport::{Reply, Transport},
    wire,
};
use jig_client::DomainLimits as Limits;
use jig_client_domain::contract as client;
#[cfg(test)]
use jig_client_test_domain::Event;
use jig_domain_core::{documents, reads};
use skein_lib::Token;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Invalid {
    MissingRoute,
    OwnerRoute,
    Document(wire::documents::Invalid),
    Page(wire::pages::Invalid),
    PendingValue,
    Historical(wire::historical::Invalid),
}

/// Translate a held answer for its exact original native query.
/// # Errors
/// Leaves the original route intact for mismatched queries, unsupported whole
/// carriers and values above the receiving bounds.
pub fn read_with_codec(
    transport: &mut Transport,
    to: Token,
    answer: &reads::Answer,
    limits: &Limits,
    codec: &jig_protocol_transcript::Limits,
) -> Result<Incoming, Invalid> {
    let route = transport.reply(to).ok_or(Invalid::MissingRoute)?;
    let Reply::Read { client, read, what } = route else { return Err(Invalid::OwnerRoute) };
    let (client, read) = (*client, *read);
    let result = match answer {
        reads::Answer::Setting(_) | reads::Answer::Revision(_) | reads::Answer::Revisions(_) => {
            return Err(Invalid::PendingValue);
        }
        reads::Answer::Holds(page) => holds_page(what, page, limits)?,
        reads::Answer::Tasks(page) => task_page(what, page, limits)?,
        reads::Answer::None(why) => client::ReadResult::None(missing(*why)),
        reads::Answer::Page(value) => {
            let value = match what {
                client::Read::Activity { task, .. } => {
                    wire::pages::activity_page_with_codec(*task, value, limits, codec)
                }
                client::Read::Notes { scope, description, .. } => {
                    wire::pages::notes_page(scope, description.as_ref(), value, limits, &transport.models)
                }
                client::Read::Inbox { .. } => wire::pages::inbox_page(what, value, limits),
                client::Read::History { task, subtree, .. } => {
                    wire::pages::history_page(*task, *subtree, value, limits, &transport.models)
                }
                client::Read::Ended { task, .. } => wire::ledger_pages::ended(*task, value, limits),
                client::Read::Spend { project, .. } => wire::ledger_pages::spend(*project, value, limits),
                client::Read::Holds { .. }
                | client::Read::Tasks { .. }
                | client::Read::Me
                | client::Read::Turn { .. }
                | client::Read::Task { .. }
                | client::Read::Proposal { .. }
                | client::Read::Escalation { .. }
                | client::Read::Question { .. }
                | client::Read::PersonTask { .. }
                | client::Read::Result { .. }
                | client::Read::Project { .. }
                | client::Read::Settings
                | client::Read::Setting(_)
                | client::Read::Revision(_)
                | client::Read::Revisions(_)
                | client::Read::Deployment => return Err(Invalid::PendingValue),
            }
            .map_err(Invalid::Page)?;
            client::ReadResult::Value(value)
        }
        reads::Answer::Document(value) => match value.as_ref() {
            documents::Document::Frame(value) => {
                if *what != client::Read::Me || value.party != transport.party(client) {
                    return Err(Invalid::OwnerRoute);
                }
                client::ReadResult::Value(
                    wire::documents::frame_read(value, limits, &transport.models, &transport.providers)
                        .map_err(Invalid::Document)?,
                )
            }
            documents::Document::Task(value) => task(what, value, limits, &transport.models)?,
            documents::Document::Proposal(value) => proposal(what, value, limits, &transport.models)?,
            documents::Document::Escalation(value) => escalation(what, value, limits, &transport.models)?,
            documents::Document::Project(value) => project(what, value, limits, &transport.models)?,
            documents::Document::Deployment(value) => deployment(what, value, limits)?,
            documents::Document::PersonTask(value) => {
                let client::Read::PersonTask { task } = what else { return Err(Invalid::OwnerRoute) };
                if *task != value.task.number {
                    return Err(Invalid::OwnerRoute);
                }
                client::ReadResult::Value(
                    wire::documents::person_task_read(value, limits, &transport.models).map_err(Invalid::Document)?,
                )
            }
            documents::Document::Result(value) => {
                let client::Read::Result { task } = what else { return Err(Invalid::OwnerRoute) };
                if *task != value.task.number {
                    return Err(Invalid::OwnerRoute);
                }
                client::ReadResult::Value(
                    wire::documents::result_read(value, limits, &transport.models).map_err(Invalid::Document)?,
                )
            }
            documents::Document::Question(value) => question(what, value, limits, &transport.models)?,
            documents::Document::Chips(_)
            | documents::Document::Run(_)
            | documents::Document::Tree(_)
            | documents::Document::Lines(_)
            | documents::Document::Note(_)
            | documents::Document::Accounts(_)
            | documents::Document::Settings(_)
            | documents::Document::Inbox(_) => return Err(Invalid::PendingValue),
        },
        reads::Answer::Historical(value) => historical_read(what, value, limits, codec, &transport.models)?,
    };
    transport.take_reply(to).expect("validated original read route");
    Ok(Incoming::Callback {
        client,
        event: crate::test::up(jig_client::Up::ReadDone { read, result: jig_client::Readout::Jig(result) }),
    })
}

fn proposal(
    what: &client::Read,
    value: &documents::Proposal,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::ReadResult, Invalid> {
    let client::Read::Proposal { proposal } = what else { return Err(Invalid::OwnerRoute) };
    if *proposal != wire::proposals::key(value.proposer, value.number) {
        return Err(Invalid::OwnerRoute);
    }
    Ok(client::ReadResult::Value(wire::documents::proposal_read(value, limits, models).map_err(Invalid::Document)?))
}

fn task(
    what: &client::Read,
    value: &documents::Task,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::ReadResult, Invalid> {
    let client::Read::Task { task } = what else { return Err(Invalid::OwnerRoute) };
    if *task != value.chip.number {
        return Err(Invalid::OwnerRoute);
    }
    Ok(client::ReadResult::Value(wire::documents::task_read(value, limits, models).map_err(Invalid::Document)?))
}

fn escalation(
    what: &client::Read,
    value: &documents::Escalation,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::ReadResult, Invalid> {
    let client::Read::Escalation { task } = what else { return Err(Invalid::OwnerRoute) };
    if *task != value.task.chip.number {
        return Err(Invalid::OwnerRoute);
    }
    Ok(client::ReadResult::Value(wire::documents::escalation_read(value, limits, models).map_err(Invalid::Document)?))
}

fn task_page(what: &client::Read, page: &reads::TaskPage, limits: &Limits) -> Result<client::ReadResult, Invalid> {
    if !matches!(what, client::Read::Tasks { .. }) {
        return Err(Invalid::OwnerRoute);
    }
    Ok(client::ReadResult::Value(
        wire::queries::page(page, limits).map_err(|why| Invalid::Page(wire::pages::Invalid::Owner(why)))?,
    ))
}

fn project(
    what: &client::Read,
    value: &documents::Project,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::ReadResult, Invalid> {
    let client::Read::Project { project } = what else { return Err(Invalid::OwnerRoute) };
    if *project != value.number {
        return Err(Invalid::OwnerRoute);
    }
    Ok(client::ReadResult::Value(wire::documents::project_read(value, limits, models).map_err(Invalid::Document)?))
}

/// Translate a fixture read with the shared small codec.
/// # Errors
/// Refuses the same routes and payloads as `read_with_codec`.
#[cfg(test)]
pub fn read(
    transport: &mut Transport,
    to: Token,
    answer: &reads::Answer,
    limits: &Limits,
) -> Result<Incoming, Invalid> {
    read_with_codec(transport, to, answer, limits, &wire::activity::tests::codec())
}

#[cfg(test)]
#[path = "read_projects.rs"]
mod project_tests;

#[cfg(test)]
#[path = "read_escalations.rs"]
mod escalation_tests;

#[cfg(test)]
#[path = "read_ledger_pages.rs"]
mod ledger_page_tests;

fn historical_read(
    what: &client::Read,
    value: &reads::Historical,
    limits: &Limits,
    codec: &jig_protocol_transcript::Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::ReadResult, Invalid> {
    if let reads::Past::Turn(record) = &value.value {
        wire::historical::turn(what, record, limits, codec).map_err(Invalid::Historical)
    } else {
        Ok(client::ReadResult::Value(wire::historical::read(what, value, limits, models).map_err(Invalid::Historical)?))
    }
}

fn holds_page(
    what: &client::Read,
    page: &documents::ProjectHolds,
    limits: &Limits,
) -> Result<client::ReadResult, Invalid> {
    let client::Read::Holds { project, .. } = what else { return Err(Invalid::OwnerRoute) };
    if *project != page.project || page.rows.len() > limits.rows as usize {
        return Err(Invalid::PendingValue);
    }
    let mut size = core::mem::size_of::<client::ProjectHolds>() as u64;
    let mut rows = Vec::new();
    for row in &page.rows {
        if row.hold.reason.len() > limits.text as usize {
            return Err(Invalid::PendingValue);
        }
        size = size
            .checked_add(
                (core::mem::size_of::<client::HoldRow>()
                    + row.hold.reason.len()
                    + core::mem::size_of_val(row.exempt.as_ref())) as u64,
            )
            .ok_or(Invalid::PendingValue)?;
        rows.push(client::HoldRow { hold: wire::chips::placed(&row.hold), held: row.held, exempt: row.exempt.clone() });
    }
    let after = page
        .after
        .as_ref()
        .map(|cursor| wire::cursor::encode(cursor, limits.text))
        .transpose()
        .map_err(|_| Invalid::PendingValue)?;
    if size > limits.document_bytes {
        return Err(Invalid::PendingValue);
    }
    Ok(client::ReadResult::Value(client::ReadValue::Holds(Box::new(client::ProjectHolds {
        project: page.project,
        rows: rows.into_boxed_slice(),
        after,
    }))))
}

#[cfg(test)]
mod tests {
    const UTF8_WORDS: &[u8] = b"~\0";

    use super::*;
    use crate::harness::ClientId;
    use jig_domain_people as people;
    use skein_lib::List;
    pub(super) fn transport() -> Transport {
        let mut value = Transport::new(client::VersionRange { first: 1, last: u32::MAX });
        value.bind(
            ClientId(0),
            17,
            people::Identity {
                key: people::IdentityKey { provider: 0, subject: Box::new([]) },
                login: Box::new([]),
                name: Box::new([]),
            },
        );
        value
    }
    #[test]
    fn a_task_page_past_one_page_carries_its_store_cursor_out_and_back() {
        use jig_domain_tasks as tasks;
        use skein_lib::Wall;
        const PARENT: u64 = 7;
        const DELEGATES: [u64; 2] = [11, 12];
        const READ: Token = Token::new(9041);
        // An application text bound that holds a task-query cursor: this one takes 47 bytes, measured below.
        let limits = Limits { text: 64, ..crate::test::limits().client.domain };
        let query = client::TaskQuery {
            project: None,
            requester: None,
            root: None,
            parent: Some(PARENT),
            label: None,
            phase: Some(client::PhaseClass::Ended),
            executor: None,
            tracked: None,
            order: client::QueryOrder::Ended,
        };
        let core_query = documents::TaskQuery {
            project: None,
            requester: None,
            root: None,
            parent: Some(PARENT),
            label: None,
            phase: Some(documents::PhaseClass::Ended),
            executor: None,
            tracked: None,
            order: documents::QueryOrder::Ended,
        };
        let original = client::Read::Tasks { query, live: None, ended: None };
        assert_eq!(
            wire::reads::read(&original, limits.text),
            Ok(reads::Read::Tasks { query: core_query, live: None, ended: None })
        );
        let retained = reads::Cursor::Store {
            scope: reads::Scope::Tasks { query: core_query },
            after: Some(reads::Position::Owner(jig_domain_core::Key::Tasks(tasks::Key::EndedDelegate {
                requester: PARENT,
                task: DELEGATES[1],
            }))),
        };
        let mut rows = List::with_capacity(limits.rows);
        for number in DELEGATES {
            let mut row = wire::rows::tests::task_source();
            row.chip.number = number;
            row.chip.details.resources = List::with_capacity(0);
            row.chip.details.live_delegates = 0;
            row.requester = tasks::Party::Task(PARENT);
            row.ended_at = Some(Wall::EPOCH);
            row.phases = documents::PhaseCounts::ZERO;
            rows.push(row).expect("one full ended page");
        }
        let answer = reads::Answer::Tasks(Box::new(reads::TaskPage {
            live: List::with_capacity(0),
            live_next: None,
            ended: reads::EndedGroup::Rows { rows, next: Some(retained.clone()), cut: false },
        }));
        let mut transport = transport();
        let to = transport.reply_to(Reply::Read { client: ClientId(0), read: READ, what: original }).into_token();
        let Incoming::Callback {
            client: actual_client,
            event:
                Event::Client {
                    event:
                        jig_client::Event::Up {
                            up:
                                jig_client::Up::ReadDone {
                                    read: actual_read,
                                    result:
                                        jig_client::Readout::Jig(client::ReadResult::Value(client::ReadValue::Tasks(page))),
                                },
                        },
                },
        } = read(&mut transport, to, &answer, &limits).expect("whole task page within the text bound")
        else {
            panic!("task page callback");
        };
        assert_eq!(actual_client, ClientId(0));
        assert_eq!(actual_read, READ);
        assert!(page.live.is_empty());
        assert_eq!(page.live_after, None);
        let client::EndedGroup::Rows { rows, after: Some(cursor), cut } = page.ended else {
            panic!("ended rows with a continuation");
        };
        assert_eq!(rows.len(), limits.rows);
        for (row, number) in rows.iter().zip(DELEGATES) {
            assert_eq!(row.chip.task, number);
            assert_eq!(row.requester, client::OwnerRequester::Task(PARENT));
            assert_eq!(row.chip.phase, client::Phase::Ended);
            assert_eq!(row.ended_at, Some(Wall::EPOCH));
        }
        assert!(!cut);
        let length = u32::try_from(cursor.value.bytes().len()).expect("bounded cursor length");
        assert_eq!(length, 47);
        assert!(length <= limits.text);
        assert_eq!(
            wire::reads::read(&client::Read::Tasks { query, live: None, ended: Some(cursor) }, limits.text),
            Ok(reads::Read::Tasks { query: core_query, live: None, ended: Some(retained) })
        );
        assert!(transport.reply(to).is_none());
    }

    #[test]
    fn proposal_reads_keep_the_original_maker_kind_and_whole_pending_value_before_route_consumption() {
        let mut transport = transport();
        let native = Token::new(u64::MAX);
        let mut source = wire::proposals::tests::source();
        let proposal = wire::proposals::key(source.proposer, source.number);
        let to = transport
            .reply_to(Reply::Read { client: ClientId(0), read: native, what: client::Read::Proposal { proposal } })
            .into_token();
        source.proposer = documents::Proposer::Party(u64::MAX);
        let wrong = reads::Answer::Document(Box::new(documents::Document::Proposal(Box::new(source.clone()))));
        let limits = wire::rows::tests::limits();
        assert_eq!(read(&mut transport, to, &wrong, &limits).err(), Some(Invalid::OwnerRoute));
        assert!(transport.reply(to).is_some());
        source.proposer = documents::Proposer::Task(u64::MAX);
        let whole = reads::Answer::Document(Box::new(documents::Document::Proposal(Box::new(source))));
        assert_eq!(
            read(&mut transport, to, &whole, &Limits { document_bytes: 0, turn_text: limits.turn_text, ..limits })
                .err(),
            Some(Invalid::Document(wire::documents::Invalid::Payload))
        );
        assert!(transport.reply(to).is_some());
        let Incoming::Callback {
            event:
                Event::Client {
                    event:
                        jig_client::Event::Up {
                            up:
                                jig_client::Up::ReadDone {
                                    read: actual,
                                    result:
                                        jig_client::Readout::Jig(client::ReadResult::Value(
                                            client::ReadValue::OwnerProposal(value),
                                        )),
                                },
                        },
                },
            ..
        } = read(&mut transport, to, &whole, &limits).expect("exact original pending proposal")
        else {
            panic!("whole pending proposal callback")
        };
        assert_eq!(actual, native);
        assert_eq!(value.key, proposal);
        assert_eq!(value.funding.capacity(), 4);
        assert!(value.reason.cut);
        assert!(transport.reply(to).is_none());
        assert_eq!(read(&mut transport, to, &whole, &limits).err(), Some(Invalid::MissingRoute));
    }
    #[test]
    fn person_task_reads_keep_original_task_and_whole_optional_results_before_consuming_the_terminal_route() {
        let mut transport = transport();
        let native = Token::new(u64::MAX);
        let mut source = wire::person_tasks::tests::source();
        let to = transport
            .reply_to(Reply::Read {
                client: ClientId(0),
                read: native,
                what: client::Read::PersonTask { task: source.task.number },
            })
            .into_token();
        let limits =
            Limits { rows: 8, turn_text: wire::rows::tests::limits().turn_text, ..wire::rows::tests::limits() };
        source.task.number = 0;
        let wrong = reads::Answer::Document(Box::new(documents::Document::PersonTask(Box::new(source.clone()))));
        assert_eq!(read(&mut transport, to, &wrong, &limits).err(), Some(Invalid::OwnerRoute));
        assert!(transport.reply(to).is_some());
        source.task.number = u64::MAX;
        let whole = reads::Answer::Document(Box::new(documents::Document::PersonTask(Box::new(source))));
        assert_eq!(
            read(&mut transport, to, &whole, &Limits { document_bytes: 0, turn_text: limits.turn_text, ..limits })
                .err(),
            Some(Invalid::Document(wire::documents::Invalid::Payload))
        );
        assert!(transport.reply(to).is_some());
        let Incoming::Callback {
            event:
                Event::Client {
                    event:
                        jig_client::Event::Up {
                            up:
                                jig_client::Up::ReadDone {
                                    read: actual,
                                    result:
                                        jig_client::Readout::Jig(client::ReadResult::Value(
                                            client::ReadValue::OwnerPersonTask(value),
                                        )),
                                },
                        },
                },
            ..
        } = read(&mut transport, to, &whole, &limits).expect("exact original person work")
        else {
            panic!("whole pending person read callback")
        };
        assert_eq!(actual, native);
        assert_eq!(value.task.task, u64::MAX);
        assert_eq!((value.results.len(), value.results.capacity()), (3, 4));
        assert!(value.results.get(0).expect("missing whole result").result.is_none());
        assert!(value.results.get(1).expect("complete result").result.is_some());
        assert!(transport.reply(to).is_none());
        assert_eq!(read(&mut transport, to, &whole, &limits).err(), Some(Invalid::MissingRoute));
    }
    #[test]
    fn task_reads_keep_original_identity_and_admit_whole_owner_state_before_consuming_the_route() {
        let mut transport = transport();
        let native = Token::new(u64::MAX);
        let mut source = wire::task::tests::source();
        let original = client::Read::Task { task: source.chip.number };
        let to = transport.reply_to(Reply::Read { client: ClientId(0), read: native, what: original }).into_token();
        let limits = wire::rows::tests::limits();
        source.chip.number = 0;
        let wrong = reads::Answer::Document(Box::new(documents::Document::Task(Box::new(source.clone()))));
        assert_eq!(read(&mut transport, to, &wrong, &limits).err(), Some(Invalid::OwnerRoute));
        assert!(transport.reply(to).is_some());
        source.chip.number = u64::MAX;
        let whole = reads::Answer::Document(Box::new(documents::Document::Task(Box::new(source))));
        assert_eq!(
            read(&mut transport, to, &whole, &Limits { document_bytes: 0, turn_text: limits.turn_text, ..limits })
                .err(),
            Some(Invalid::Document(wire::documents::Invalid::Payload))
        );
        assert!(transport.reply(to).is_some());
        let Incoming::Callback {
            event:
                Event::Client {
                    event:
                        jig_client::Event::Up {
                            up:
                                jig_client::Up::ReadDone {
                                    read: actual,
                                    result:
                                        jig_client::Readout::Jig(client::ReadResult::Value(client::ReadValue::Task(value))),
                                },
                        },
                },
            ..
        } = read(&mut transport, to, &whole, &limits).expect("whole original task")
        else {
            panic!("task callback")
        };
        assert_eq!(actual, native);
        assert!(value.context.is_none());
        assert_eq!(value.hold.as_ref().expect("owner hold").revision().value, 17);
        assert!(transport.reply(to).is_none());
        assert_eq!(read(&mut transport, to, &whole, &limits).err(), Some(Invalid::MissingRoute));
    }

    #[test]
    fn question_reads_keep_original_task_message_and_sender_before_consuming_exact_answer_route() {
        let mut transport = transport();
        let native = Token::new(u64::MAX);
        let mut source = wire::documents::tests::question_source();
        let original = client::Read::Question { task: source.task.number, message: source.message };
        let to = transport.reply_to(Reply::Read { client: ClientId(0), read: native, what: original }).into_token();
        let limits = wire::rows::tests::limits();
        source.message = 0;
        let wrong = reads::Answer::Document(Box::new(documents::Document::Question(source.clone())));
        assert_eq!(read(&mut transport, to, &wrong, &limits).err(), Some(Invalid::OwnerRoute));
        assert!(transport.reply(to).is_some());
        source.message = u64::MAX;
        source.from = jig_domain_tasks::Party::Deployment { project: 0 };
        let source = reads::Answer::Document(Box::new(documents::Document::Question(source)));
        assert_eq!(
            read(&mut transport, to, &source, &Limits { document_bytes: 0, turn_text: limits.turn_text, ..limits })
                .err(),
            Some(Invalid::Document(wire::documents::Invalid::Payload))
        );
        assert!(transport.reply(to).is_some());
        let Incoming::Callback {
            client: ClientId(0),
            event:
                Event::Client {
                    event:
                        jig_client::Event::Up {
                            up:
                                jig_client::Up::ReadDone {
                                    read: actual,
                                    result:
                                        jig_client::Readout::Jig(client::ReadResult::Value(client::ReadValue::Question(
                                            value,
                                        ))),
                                },
                        },
                },
        } = read(&mut transport, to, &source, &limits).expect("original whole question")
        else {
            panic!("question callback")
        };
        assert_eq!(actual, native);
        assert_eq!(value.message, u64::MAX);
        assert_eq!(value.from, Some(client::OwnerRequester::Deployment { project: 0 }));
        assert_eq!(value.words.bytes(), UTF8_WORDS);
        assert!(transport.reply(to).is_none());
        assert_eq!(read(&mut transport, to, &source, &limits).err(), Some(Invalid::MissingRoute));
    }

    #[test]
    fn inbox_read_admits_original_group_query_context_and_whole_rows_before_consuming_terminal() {
        let mut transport = transport();
        let native = Token::new(u64::MAX);
        let original = client::Read::Inbox {
            group: client::InboxReadGroup::Needs,
            project: None,
            kind: Some(client::InboxGroup::Questions),
            goal: Some(17),
            after: None,
        };
        let to =
            transport.reply_to(Reply::Read { client: ClientId(0), read: native, what: original.clone() }).into_token();
        let source = wire::inbox::tests::source();
        let mut rows = List::with_capacity(8);
        rows.push(reads::PageRow::Inbox(source.needs.get(0).expect("stored source").clone())).expect("source row");
        let mut page = reads::Page {
            rows,
            before: None,
            next: Some(reads::Cursor::Store {
                scope: reads::Scope::Inbox {
                    group: reads::Group::Updates,
                    filter: reads::Filter { project: None, goal: Some(17) },
                },
                after: None,
            }),
            cut: true,
            inbox_selection: source.selection,
        };
        let limits = wire::inbox::tests::limits();
        let wrong = reads::Answer::Page(Box::new(page.clone()));
        assert_eq!(
            read(&mut transport, to, &wrong, &limits).err(),
            Some(Invalid::Page(wire::pages::Invalid::Identity))
        );
        assert!(transport.reply(to).is_some());
        page.next = Some(reads::Cursor::Store {
            scope: wire::inbox::scope(&original, 200).expect("actual original query"),
            after: None,
        });
        let source = reads::Answer::Page(Box::new(page));
        assert_eq!(
            read(&mut transport, to, &source, &Limits { document_bytes: 100, turn_text: limits.turn_text, ..limits })
                .err(),
            Some(Invalid::Page(wire::pages::Invalid::Payload))
        );
        assert!(transport.reply(to).is_some());
        let Incoming::Callback {
            client: ClientId(0),
            event:
                Event::Client {
                    event:
                        jig_client::Event::Up {
                            up:
                                jig_client::Up::ReadDone {
                                    read: actual,
                                    result:
                                        jig_client::Readout::Jig(client::ReadResult::Value(client::ReadValue::OwnerInbox(
                                            value,
                                        ))),
                                },
                        },
                },
        } = read(&mut transport, to, &source, &limits).expect("complete original owner answer")
        else {
            panic!("original owner read")
        };
        assert_eq!(actual, native);
        assert!(value.cut);
        assert_eq!((value.rows.capacity(), value.rows.len()), (8, 1));
        assert_eq!(
            value.rows.get(0).expect("stored source").task,
            client::OwnerTaskLink::Stored { number: u64::MAX, status: None }
        );
        assert_eq!(
            value
                .selection
                .as_ref()
                .expect("original source ancestry")
                .get(0)
                .expect("roots")
                .goals
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![u64::MAX, 0]
        );
        assert!(transport.reply(to).is_none());
        assert_eq!(read(&mut transport, to, &source, &limits).err(), Some(Invalid::MissingRoute));
    }

    #[test]
    fn result_reads_admit_the_complete_source_before_consuming_the_original_task_and_reply() {
        let mut transport = transport();
        let native = Token::new(u64::MAX);
        let to = transport
            .reply_to(Reply::Read { client: ClientId(0), read: native, what: client::Read::Result { task: u64::MAX } })
            .into_token();
        let mut source = wire::results::tests::document();
        let limits = Limits {
            rows: 8,
            text: 200,
            turn_text: 200,
            document_bytes: 100_000,
            ..crate::test::limits().client.domain
        };
        source.task.number = 0;
        let wrong = reads::Answer::Document(Box::new(documents::Document::Result(Box::new(source.clone()))));
        assert_eq!(read(&mut transport, to, &wrong, &limits).err(), Some(Invalid::OwnerRoute));
        assert!(transport.reply(to).is_some());
        source.task.number = u64::MAX;
        let source = reads::Answer::Document(Box::new(documents::Document::Result(Box::new(source))));
        assert_eq!(
            read(&mut transport, to, &source, &Limits { document_bytes: 0, turn_text: limits.turn_text, ..limits })
                .err(),
            Some(Invalid::Document(wire::documents::Invalid::Payload))
        );
        assert!(transport.reply(to).is_some());
        let Incoming::Callback {
            client: ClientId(0),
            event:
                Event::Client {
                    event:
                        jig_client::Event::Up {
                            up:
                                jig_client::Up::ReadDone {
                                    read: actual,
                                    result:
                                        jig_client::Readout::Jig(client::ReadResult::Value(client::ReadValue::Result(
                                            value,
                                        ))),
                                },
                        },
                },
        } = read(&mut transport, to, &source, &limits).expect("whole actual Result")
        else {
            panic!("original Result callback")
        };
        assert_eq!(actual, native);
        assert_eq!(value.task.task, u64::MAX);
        assert_eq!(value.at, None);
        assert!(matches!(value.ending, client::Ending::Task(_)));
        assert_eq!(value.value, None);
        assert!(value.details.is_some());
        assert!(transport.reply(to).is_none());
        assert_eq!(read(&mut transport, to, &source, &limits).err(), Some(Invalid::MissingRoute));
    }

    #[test]
    fn actual_read_absence_keeps_its_owner_reason_and_original_native_token() {
        let mut transport = transport();
        let native = Token::new(u64::MAX);
        for (why, expected) in [
            (reads::Why::Unknown, client::ReadNone::Unknown),
            (reads::Why::Busy, client::ReadNone::Busy),
            (reads::Why::Gone, client::ReadNone::Gone),
        ] {
            let to = transport
                .reply_to(Reply::Read { client: ClientId(0), read: native, what: client::Read::Result { task: 31 } })
                .into_token();
            let Incoming::Callback {
                client: ClientId(0),
                event:
                    Event::Client {
                        event:
                            jig_client::Event::Up {
                                up: jig_client::Up::ReadDone { read: actual, result: jig_client::Readout::Jig(result) },
                            },
                    },
            } = read(&mut transport, to, &reads::Answer::None(why), &crate::test::limits().client.domain)
                .expect("actual terminal")
            else {
                panic!("original native read");
            };
            assert_eq!(actual, native);
            assert_eq!(result, client::ReadResult::None(expected));
            assert!(transport.reply(to).is_none());
            assert!(matches!(
                read(&mut transport, to, &reads::Answer::None(why), &crate::test::limits().client.domain),
                Err(Invalid::MissingRoute)
            ));
        }
    }
    #[test]
    fn whole_activity_read_rejects_reserved_overflow_before_consuming_the_original_query() {
        let mut transport = transport();
        let native = Token::new(0);
        let to = transport
            .reply_to(Reply::Read {
                client: ClientId(0),
                read: native,
                what: client::Read::Activity { task: 31, before: None },
            })
            .into_token();
        let answer = reads::Answer::Page(Box::new(reads::Page {
            before: None,
            inbox_selection: None,
            rows: List::with_capacity(3),
            next: None,
            cut: false,
        }));
        let limits = Limits {
            rows: 2,
            turn_text: crate::test::limits().client.domain.turn_text,
            ..crate::test::limits().client.domain
        };
        assert!(matches!(
            read(&mut transport, to, &answer, &limits),
            Err(Invalid::Page(wire::pages::Invalid::Owner(_)))
        ));
        assert!(transport.reply(to).is_some());
        let Incoming::Callback {
            event:
                Event::Client {
                    event:
                        jig_client::Event::Up {
                            up:
                                jig_client::Up::ReadDone {
                                    read: actual,
                                    result:
                                        jig_client::Readout::Jig(client::ReadResult::Value(client::ReadValue::Activity(
                                            page,
                                        ))),
                                },
                        },
                },
            ..
        } = read(&mut transport, to, &answer, &Limits { rows: 3, turn_text: limits.turn_text, ..limits })
            .expect("whole reserved collection")
        else {
            panic!("actual whole page");
        };
        assert_eq!(actual, native);
        assert_eq!(page.rows.capacity(), 3);
        assert!(page.rows.is_empty());
        assert_eq!(page.before, None);
        assert_eq!(page.after, None);
        assert!(transport.reply(to).is_none());
    }
    #[test]
    fn a_scoped_note_page_consumes_only_its_original_read_after_exact_query_and_whole_admission() {
        let mut transport = transport();
        let native = Token::new(u64::MAX);
        let limits = Limits {
            rows: 2,
            text: 200,
            turn_text: 200,
            document_bytes: 100_000,
            ..crate::test::limits().client.domain
        };
        let query = client::Read::Notes {
            scope: client::NoteScope::Project(u32::MAX),
            description: Some(client::Text::new(Box::new([]), limits.text).expect("explicit empty search")),
            after: None,
        };
        let to = transport.reply_to(Reply::Read { client: ClientId(0), read: native, what: query }).into_token();
        let mut page = reads::Page {
            before: None,
            inbox_selection: None,
            rows: List::with_capacity(2),
            next: Some(reads::Cursor::Store {
                scope: reads::Scope::ScopedNotes {
                    scope: jig_domain_notes::Scope::Project { project: 0 },
                    description: Some(Box::new([])),
                },
                after: None,
            }),
            cut: true,
        };
        assert!(matches!(
            read(&mut transport, to, &reads::Answer::Page(Box::new(page.clone())), &limits),
            Err(Invalid::Page(wire::pages::Invalid::Identity))
        ));
        assert!(transport.reply(to).is_some());
        page.next = Some(reads::Cursor::Store {
            scope: reads::Scope::ScopedNotes {
                scope: jig_domain_notes::Scope::Project { project: u32::MAX },
                description: Some(Box::new([])),
            },
            after: None,
        });
        assert!(matches!(
            read(
                &mut transport,
                to,
                &reads::Answer::Page(Box::new(page.clone())),
                &Limits { document_bytes: 0, turn_text: limits.turn_text, ..limits }
            ),
            Err(Invalid::Page(wire::pages::Invalid::Payload))
        ));
        assert!(transport.reply(to).is_some());
        let Incoming::Callback {
            client: ClientId(0),
            event:
                Event::Client {
                    event:
                        jig_client::Event::Up {
                            up:
                                jig_client::Up::ReadDone {
                                    read: original,
                                    result:
                                        jig_client::Readout::Jig(client::ReadResult::Value(client::ReadValue::Notes(value))),
                                },
                        },
                },
        } = read(&mut transport, to, &reads::Answer::Page(Box::new(page.clone())), &limits)
            .expect("whole original Notes query")
        else {
            panic!("original native read callback")
        };
        assert_eq!(original, native);
        assert!(value.cut);
        assert_eq!(value.rows.capacity(), 2);
        assert!(value.rows.is_empty());
        assert_eq!(value.before, None);
        assert_eq!(wire::cursor::decode(value.after.as_ref().expect("actual after"), limits.text), page.next);
        assert!(transport.reply(to).is_none());
        assert!(matches!(
            read(&mut transport, to, &reads::Answer::Page(Box::new(page)), &limits),
            Err(Invalid::MissingRoute)
        ));
    }
}

fn deployment(
    what: &client::Read,
    value: &documents::Deployment,
    limits: &Limits,
) -> Result<client::ReadResult, Invalid> {
    if *what != client::Read::Deployment {
        return Err(Invalid::OwnerRoute);
    }
    Ok(client::ReadResult::Value(wire::documents::deployment_read(value, limits).map_err(Invalid::Document)?))
}

#[cfg(test)]
#[path = "deployment_packets.rs"]
mod deployment_tests;

#[cfg(test)]
#[path = "historical_packets.rs"]
mod historical_tests;

fn missing(why: reads::Why) -> client::ReadNone {
    match why {
        reads::Why::TooLarge { bytes } => client::ReadNone::TooLarge { bytes },
        reads::Why::Malformed => client::ReadNone::Malformed,
        reads::Why::Unknown => client::ReadNone::Unknown,
        reads::Why::Busy => client::ReadNone::Busy,
        reads::Why::Gone => client::ReadNone::Gone,
        reads::Why::SignedOut => client::ReadNone::SignedOut,
    }
}

fn question(
    what: &client::Read,
    value: &documents::Question,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::ReadResult, Invalid> {
    let client::Read::Question { task, message } = what else { return Err(Invalid::OwnerRoute) };
    if *task != value.task.number || *message != value.message {
        return Err(Invalid::OwnerRoute);
    }
    Ok(client::ReadResult::Value(wire::documents::question_read(value, limits, models).map_err(Invalid::Document)?))
}

#[cfg(test)]
mod goal_query {
    use super::*;
    const READ: Token = Token::new(9040);
    #[test]
    fn a_task_query_answer_preserves_an_unrelated_read_route() {
        let mut transport = super::tests::transport();
        let to = transport
            .reply_to(Reply::Read {
                client: crate::ClientId(0),
                read: READ,
                what: client::Read::Project { project: 1 },
            })
            .into_token();
        let answer = reads::Answer::Tasks(Box::new(reads::TaskPage {
            live: skein_lib::List::with_capacity(0),
            live_next: None,
            ended: reads::EndedGroup::Rows { rows: skein_lib::List::with_capacity(0), next: None, cut: false },
        }));
        let translated = read(&mut transport, to, &answer, &wire::rows::tests::limits());
        assert_eq!(translated.err(), Some(Invalid::OwnerRoute));
        assert!(transport.reply(to).is_some(), "the unrelated route remains available");
    }
}
