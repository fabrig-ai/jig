//! Translate held external peer answers through their retained original child
//! (client/domain/contract.md, 4). The caller supplies only actual released packets;
//! this layer does not attest durability or choose a business outcome.
use super::{
    transport::{KeyedOwner, Pace, Reply, Transport, Watched},
    wire,
};
use crate::harness::ClientId;
use jig_client::Answered;
use jig_client::DomainLimits as Limits;
use jig_client_domain::contract as client;
use jig_client_scripted_engine as admission;
use jig_client_test_domain::{Event, connector_values::ConnectorLimits, family};
use jig_domain_people as people;
use jig_test_connector::ApplicationOutcome;
use skein_lib::{RequestEnvelope, Token, Wall};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Invalid {
    MissingRoute,
    OwnerRoute,
    Session,
    Payload,
    Document(wire::documents::Invalid),
    PendingDocument,
    Application(jig_client_test_domain::family::wire::Invalid),
}
/// A terminal provider exchange, outside the keyed request table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignInFailure {
    Refused(client::Refusal),
    SignedOut,
    Capacity,
}
#[derive(Debug)]
pub enum Incoming {
    Authenticated { client: ClientId, person: u64, expires: Wall },
    SignInAgain { client: ClientId, why: people::Again },
    SignInFailed { client: ClientId, failure: SignInFailure },
    Callback { client: ClientId, event: Event },
    SignedOut { client: ClientId, sign_in: u64 },
}
fn bounds(limits: &Limits) -> admission::Bounds {
    admission::Bounds {
        text: limits.text,
        rows: limits.rows,
        authority: crate::test::AUTHORITY_ENTRIES,
        document_bytes: limits.document_bytes,
        ..admission::Bounds::default()
    }
}

fn watched(transport: &Transport, watcher: Token) -> Result<(ClientId, Token, client::Subject), Invalid> {
    let route = transport.watch(watcher).ok_or(Invalid::MissingRoute)?;
    let Watched::Jig(subject) = &route.subject else { return Err(Invalid::OwnerRoute) };
    Ok((route.client, route.native, subject.clone()))
}

/// Translate a whole normal owner document under its retained original subject.
/// # Errors
/// Rejects a missing route, a mismatched identity, an unimplemented document
/// carrier or a whole value above the receiving bounds before marking its flight.
pub fn document(
    transport: &mut Transport,
    watcher: Token,
    source: &jig_domain_core::documents::Document,
    limits: &Limits,
) -> Result<Incoming, Invalid> {
    let (client, watch, subject) = watched(transport, watcher)?;
    let delivery = match source {
        jig_domain_core::documents::Document::Frame(value) => {
            if subject != client::Subject::Me || value.party != transport.party(client) {
                return Err(Invalid::OwnerRoute);
            }
            wire::documents::frame_delivery(value, limits, &transport.models, &transport.providers)
        }
        jig_domain_core::documents::Document::Task(value) => {
            let client::Subject::Task { task } = subject else { return Err(Invalid::OwnerRoute) };
            if value.chip.number != task {
                return Err(Invalid::OwnerRoute);
            }
            wire::documents::task_delivery(value, limits, &transport.models)
        }
        jig_domain_core::documents::Document::Chips(values) => {
            let client::Subject::Chips { tasks } = subject else { return Err(Invalid::OwnerRoute) };
            if values.iter().any(|chip| !tasks.iter().any(|task| *task == chip.number)) {
                return Err(Invalid::OwnerRoute);
            }
            wire::documents::chips_delivery(values, limits)
        }
        jig_domain_core::documents::Document::Run(value) => {
            let client::Subject::Run { task, .. } = subject else { return Err(Invalid::OwnerRoute) };
            if value.task != task {
                return Err(Invalid::OwnerRoute);
            }
            wire::documents::run_delivery(value, limits)
        }
        jig_domain_core::documents::Document::Lines(value) => {
            let client::Subject::Tasks { query, .. } = subject else {
                return Err(Invalid::OwnerRoute);
            };
            wire::documents::tasks_delivery(&query, value, limits)
        }
        jig_domain_core::documents::Document::Inbox(value) => {
            let client::Subject::Inbox { window } = subject else { return Err(Invalid::OwnerRoute) };
            if value.party != transport.party(client)
                || value.needs.capacity() > window
                || value.updates.capacity() > window
            {
                return Err(Invalid::OwnerRoute);
            }
            wire::documents::inbox_delivery(value, limits)
        }
        jig_domain_core::documents::Document::Tree(value) => {
            let client::Subject::Tree { task, window } = subject else { return Err(Invalid::OwnerRoute) };
            wire::documents::tree_delivery(task, window, value, limits)
        }
        jig_domain_core::documents::Document::Accounts(value) => {
            if subject != client::Subject::Accounts {
                return Err(Invalid::OwnerRoute);
            }
            wire::documents::accounts_delivery(value, limits)
        }
        jig_domain_core::documents::Document::Deployment(value) => {
            if subject != client::Subject::Deployment {
                return Err(Invalid::OwnerRoute);
            }
            wire::documents::deployment_delivery(value, limits)
        }
        jig_domain_core::documents::Document::Proposal(_)
        | jig_domain_core::documents::Document::Escalation(_)
        | jig_domain_core::documents::Document::Question(_)
        | jig_domain_core::documents::Document::PersonTask(_)
        | jig_domain_core::documents::Document::Result(_)
        | jig_domain_core::documents::Document::Project(_)
        | jig_domain_core::documents::Document::Settings(_)
        | jig_domain_core::documents::Document::Note(_) => return Err(Invalid::PendingDocument),
    }
    .map_err(Invalid::Document)?;
    assert!(transport.delivered(watcher, Pace::Document), "validated owner route");
    Ok(Incoming::Callback {
        client,
        event: crate::test::up(jig_client::Up::Watched { watch, event: client::WatchEvent::Delivered(delivery) }),
    })
}

/// A provisional suffix retains the actual retry identity and its independent pace.
/// # Errors
/// Rejects a missing/non-run route or an oversized whole stream envelope.
pub fn stream(
    transport: &mut Transport,
    watcher: Token,
    source: &jig_domain_views::Writing,
    limits: &Limits,
) -> Result<Incoming, Invalid> {
    let (client, watch, subject) = watched(transport, watcher)?;
    if !matches!(subject, client::Subject::Run { .. }) {
        return Err(Invalid::OwnerRoute);
    }
    let delivery = wire::documents::stream_delivery(source, limits).map_err(Invalid::Document)?;
    assert!(transport.delivered(watcher, Pace::Stream), "validated owner route");
    Ok(Incoming::Callback {
        client,
        event: crate::test::up(jig_client::Up::Watched { watch, event: client::WatchEvent::Delivered(delivery) }),
    })
}

/// Preserve the owner's exact terminal and remove only that original watch.
/// # Errors
/// Rejects a missing or application-owned watch route.
pub fn ended(transport: &mut Transport, watcher: Token, why: jig_domain_views::End) -> Result<Incoming, Invalid> {
    let end = match why {
        jig_domain_views::End::Unwatched => client::WatchEnd::Closed,
        jig_domain_views::End::Gone => client::WatchEnd::SubjectEnded,
        jig_domain_views::End::NotAllowed => client::WatchEnd::NotAllowed,
        jig_domain_views::End::SignedOut => client::WatchEnd::SignedOut,
        jig_domain_views::End::Stopping => client::WatchEnd::Stopping,
    };
    terminal(transport, watcher, end)
}

/// Exact immediate refusal, independent of the delivered document's ACK.
/// # Errors
/// Rejects an obsolete watch route without consuming another watch.
pub fn refused(
    transport: &mut Transport,
    watcher: Token,
    why: jig_domain_core::WatchRefusal,
) -> Result<Incoming, Invalid> {
    let end = match why {
        jig_domain_core::WatchRefusal::SignedOut => client::WatchEnd::SignedOut,
        jig_domain_core::WatchRefusal::NotAllowed => client::WatchEnd::NotAllowed,
        jig_domain_core::WatchRefusal::Unknown => client::WatchEnd::Unknown,
        jig_domain_core::WatchRefusal::Busy => client::WatchEnd::Busy,
    };
    terminal(transport, watcher, end)
}
/// Report an actual protocol watch drop, then let the caller close its owner route.
/// # Errors
/// Rejects an already retired watch; this never acknowledges its pending document.
pub fn dropped(transport: &mut Transport, watcher: Token) -> Result<Incoming, Invalid> {
    terminal(transport, watcher, client::WatchEnd::Dropped)
}
fn terminal(transport: &mut Transport, watcher: Token, end: client::WatchEnd) -> Result<Incoming, Invalid> {
    let route = transport.watch(watcher).ok_or(Invalid::MissingRoute)?.clone();
    let event = match route.subject {
        Watched::Jig(_) => crate::test::up(jig_client::Up::WatchEnded { watch: route.native, end }),
        Watched::Application { .. } => {
            let end = match end {
                client::WatchEnd::Closed => family::WatchEnd::Closed,
                client::WatchEnd::Dropped => family::WatchEnd::Dropped,
                client::WatchEnd::NotServed => family::WatchEnd::Unsupported,
                client::WatchEnd::Unknown => family::WatchEnd::Unknown,
                client::WatchEnd::NotAllowed => family::WatchEnd::NotAllowed,
                client::WatchEnd::SignedOut => family::WatchEnd::SignedOut,
                client::WatchEnd::SubjectEnded => family::WatchEnd::SubjectEnded,
                client::WatchEnd::Stopping => family::WatchEnd::Stopping,
                client::WatchEnd::Busy => family::WatchEnd::Busy,
            };
            Event::First(jig_client_test_domain::first::Event::Ended { watch: route.native, end })
        }
    };
    if end == client::WatchEnd::SignedOut {
        transport.signed_out(route.client);
    }
    transport.end_watch(watcher).expect("validated original route");
    Ok(Incoming::Callback { client: route.client, event })
}

/// Receive a whole application value under the supplied original subject.
/// # Errors
/// Rejects a wrong connector, scope/inner identity or complete allocation bound
/// before marking any normal document flight.
pub fn application_document(
    transport: &mut Transport,
    watcher: Token,
    connector: u16,
    source: jig_test_connector::ClientDocument,
    limits: &ConnectorLimits,
) -> Result<Incoming, Invalid> {
    let route = transport.watch(watcher).ok_or(Invalid::MissingRoute)?.clone();
    let Watched::Application { subject, scope } = route.subject else { return Err(Invalid::OwnerRoute) };
    if connector != jig_client_test_domain::FIRST {
        return Err(Invalid::OwnerRoute);
    }
    let supplied_scope = match subject.scope {
        family::Scope::Project(project) => jig_domain_people::ApplicationScope::Project(project),
        family::Scope::Deployment => jig_domain_people::ApplicationScope::Deployment,
    };
    if supplied_scope != scope {
        return Err(Invalid::OwnerRoute);
    }
    let value =
        jig_client_test_domain::family::wire::document(subject, source, limits).map_err(Invalid::Application)?;
    assert!(transport.delivered(watcher, Pace::Document), "validated original application route");
    Ok(Incoming::Callback {
        client: route.client,
        event: Event::First(jig_client_test_domain::first::Event::Watch {
            watch: route.native,
            event: family::WatchEvent::Delivered(family::Document::Connector(value)),
        }),
    })
}

/// Receive one exact application read terminal without changing its owner reason.
/// # Errors
/// Keeps the original route intact on wrong child/identity/whole allocation.
pub fn application_read(
    transport: &mut Transport,
    to: Token,
    connector: u16,
    source: Result<jig_test_connector::ClientDocument, jig_test_connector::DocumentMissing>,
    limits: &ConnectorLimits,
) -> Result<Incoming, Invalid> {
    let Reply::ApplicationRead { client, read, subject } = transport.reply(to).ok_or(Invalid::MissingRoute)?.clone()
    else {
        return Err(Invalid::OwnerRoute);
    };
    if connector != jig_client_test_domain::FIRST {
        return Err(Invalid::OwnerRoute);
    }
    let result = match source {
        Ok(source) => family::ReadResult::Value(family::Document::Connector(
            jig_client_test_domain::family::wire::document(subject, source, limits).map_err(Invalid::Application)?,
        )),
        Err(jig_test_connector::DocumentMissing::Unknown) => family::ReadResult::Unknown,
        Err(jig_test_connector::DocumentMissing::NotAllowed) => family::ReadResult::NotAllowed,
    };
    transport.take_reply(to).expect("validated original read route");
    Ok(Incoming::Callback { client, event: Event::First(jig_client_test_domain::first::Event::Read { read, result }) })
}

/// Translate the actual people owner reply, preserving its native route on failure.
/// # Errors
/// Refuses a missing/wrong child route, a sign-in for another party,
/// or a complete keyed body above its receiving bound.
pub fn people_reply(
    transport: &mut Transport,
    to: Token,
    reply: people::Reply,
    limits: &Limits,
) -> Result<Incoming, Invalid> {
    let translated = wire::answers::reply(reply, &transport.models, limits)
        .map_err(|why| Invalid::Document(wire::documents::Invalid::Owner(why)))?;
    translated_reply(transport, to, translated, limits)
}

/// # Errors
/// Refuses a missing or non-jig keyed route, or an oversized answer.
pub fn keyed_refusal(
    transport: &mut Transport,
    to: Token,
    answer: client::Answer,
    limits: &Limits,
) -> Result<Incoming, Invalid> {
    let Some(Reply::Keyed { owner: KeyedOwner::Jig, .. }) = transport.reply(to) else {
        return Err(Invalid::OwnerRoute);
    };
    translated_reply(transport, to, wire::answers::Reply::Final(answer), limits)
}

pub(super) fn translated_reply(
    transport: &mut Transport,
    to: Token,
    translated: wire::answers::Reply,
    limits: &Limits,
) -> Result<Incoming, Invalid> {
    let route = transport.reply(to).ok_or(Invalid::MissingRoute)?.clone();
    let incoming = match route {
        Reply::SignIn { client } => match translated {
            wire::answers::Reply::SignedIn { person, sign_in, expires } => {
                if transport.party(client) != person {
                    return Err(Invalid::Session);
                }
                transport.signed_in(client, person, sign_in);
                Incoming::Authenticated { client, person, expires }
            }
            wire::answers::Reply::Again(why) => Incoming::SignInAgain { client, why },
            wire::answers::Reply::SignedOut => Incoming::SignInFailed { client, failure: SignInFailure::SignedOut },
            wire::answers::Reply::Final(client::Answer::Refused { refusal: why, .. }) => {
                admission::admit_answer(&client::Answer::Refused { refusal: why.clone(), at: None }, &bounds(limits))
                    .map_err(|_| Invalid::Payload)?;
                Incoming::SignInFailed { client, failure: SignInFailure::Refused(why) }
            }
            wire::answers::Reply::Final(client::Answer::Done(_))
            | wire::answers::Reply::MovingOut { .. }
            | wire::answers::Reply::Applied { .. } => {
                return Err(Invalid::OwnerRoute);
            }
        },
        Reply::SignOut { client, sign_in } => {
            if translated != wire::answers::Reply::SignedOut {
                return Err(Invalid::OwnerRoute);
            }
            if transport.sign_in(client) == Some(sign_in) {
                transport.signed_out(client);
            }
            Incoming::SignedOut { client, sign_in }
        }
        Reply::Keyed { client, attempt, owner, .. } => {
            let why = match &translated {
                wire::answers::Reply::Again(people::Again::Full { held, room_at }) => {
                    Some(jig_client::Retry::Full { held: *held, room_at: *room_at })
                }
                wire::answers::Reply::SignedIn { .. }
                | wire::answers::Reply::Final(_)
                | wire::answers::Reply::MovingOut { .. }
                | wire::answers::Reply::Again(people::Again::Busy | people::Again::NotReady)
                | wire::answers::Reply::SignedOut
                | wire::answers::Reply::Applied { .. } => None,
            };
            let (envelope, body) = wire::answers::keyed(translated).map_err(|_| Invalid::OwnerRoute)?;
            let answer = match body {
                None => None,
                Some(value) => {
                    admission::admit_answer(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
                    Some(match owner {
                        KeyedOwner::Jig => Answered::Jig(value),
                        KeyedOwner::Application { connector: 1 } => {
                            let client::Answer::Refused { refusal: why, .. } = value else {
                                return Err(Invalid::OwnerRoute);
                            };
                            let why = client::EntranceRefusal::new(why).map_err(|_| Invalid::Payload)?;
                            crate::test::family_answer(family::Answer::Refused(why))
                        }
                        KeyedOwner::Application { .. } => return Err(Invalid::OwnerRoute),
                    })
                }
            };
            if envelope == RequestEnvelope::SignedOut {
                transport.signed_out(client);
            }
            Incoming::Callback {
                client,
                event: crate::test::up(jig_client::Up::Answer { attempt, envelope, answer, why }),
            }
        }
        Reply::Read { .. } | Reply::ApplicationRead { .. } => return Err(Invalid::OwnerRoute),
    };
    transport.take_reply(to).expect("validated original reply route");
    Ok(incoming)
}

/// Preserve a connector's complete actual terminal under its original child route.
/// # Errors
/// Refuses another connector/child, a missing original route or an oversized whole body.
pub fn application_reply(
    transport: &mut Transport,
    to: Token,
    connector: u16,
    outcome: ApplicationOutcome,
    limits: &ConnectorLimits,
) -> Result<Incoming, Invalid> {
    let route = transport.reply(to).ok_or(Invalid::MissingRoute)?;
    let Reply::Keyed { client, attempt, owner: KeyedOwner::Application { connector: actual }, .. } = route else {
        return Err(Invalid::OwnerRoute);
    };
    if *actual != connector || connector != 1 {
        return Err(Invalid::OwnerRoute);
    }
    let client = *client;
    let attempt = *attempt;
    let answer = jig_client_test_domain::family::wire::answer(outcome);
    if !jig_client_test_domain::connector_bounds::answer_fits(&answer, limits) {
        return Err(Invalid::Payload);
    }
    transport.take_reply(to).expect("validated original reply route");
    Ok(Incoming::Callback {
        client,
        event: crate::test::up(jig_client::Up::Answer {
            attempt,
            envelope: RequestEnvelope::Final,
            answer: Some(crate::test::family_answer(family::Answer::Connector(answer))),
            why: None,
        }),
    })
}

#[cfg(test)]
mod tests {
    const UTF8_WORDS: &[u8] = b"~\0";

    use super::*;
    fn transport() -> Transport {
        let mut value = Transport::new(client::VersionRange { first: 1, last: u32::MAX });
        for client in [ClientId(0), ClientId(1)] {
            value.bind(
                client,
                u64::MAX,
                people::Identity {
                    key: people::IdentityKey { provider: u16::MAX, subject: Box::from([255, 0]) },
                    login: Box::new([]),
                    name: Box::new([]),
                },
            );
        }
        value
    }
    fn run(task: u64) -> jig_domain_core::documents::Document {
        jig_domain_core::documents::Document::Run(jig_domain_core::documents::Run {
            task,
            attempt: None,
            committed_turn: u32::MAX,
            writing: None,
            call: None,
        })
    }
    #[test]
    fn provider_again_keeps_its_source_reason_without_a_keyed_callback_or_authentication() {
        for why in [people::Again::Busy, people::Again::NotReady] {
            let mut transport = transport();
            let to = transport.reply_to(Reply::SignIn { client: ClientId(1) }).into_token();
            let Incoming::SignInAgain { client, why: actual_why } =
                people_reply(&mut transport, to, people::Reply::Again(why), &crate::test::limits().client.domain)
                    .expect("actual provider terminal")
            else {
                panic!("provider exchange cannot manufacture a keyed final")
            };
            assert_eq!(client, ClientId(1));
            assert_eq!(actual_why, why);
            assert_eq!(transport.sign_in(ClientId(1)), None);
            assert!(transport.reply(to).is_none());
            assert_eq!(
                people_reply(&mut transport, to, people::Reply::Again(why), &crate::test::limits().client.domain).err(),
                Some(Invalid::MissingRoute)
            );
        }
    }

    #[test]
    fn provider_failures_keep_whole_refusal_or_signed_out_and_never_authenticate_or_become_keyed_answers() {
        for failure in [
            SignInFailure::Refused(client::Refusal::Limit {
                which: client::Limit::Admission { which: client::AdmissionLimit::Identity, bound: None },
            }),
            SignInFailure::SignedOut,
        ] {
            let mut transport = transport();
            let to = transport.reply_to(Reply::SignIn { client: ClientId(1) }).into_token();
            let source = match failure {
                SignInFailure::Refused(_) => {
                    people::Reply::Refused(people::Refusal::Limit { which: people::LimitKind::Identity, bound: None })
                }
                SignInFailure::SignedOut => people::Reply::SignedOut,
                SignInFailure::Capacity => unreachable!("immediate source terminal"),
            };
            let Incoming::SignInFailed { client, failure: actual } =
                people_reply(&mut transport, to, source, &crate::test::limits().client.domain)
                    .expect("whole original provider terminal")
            else {
                panic!("provider terminal is not a keyed callback")
            };
            assert_eq!(client, ClientId(1));
            assert_eq!(actual, failure);
            assert_eq!(transport.sign_in(ClientId(1)), None);
            assert!(transport.reply(to).is_none());
        }
        let mut transport = transport();
        transport.signed_in(ClientId(1), u64::MAX, u64::MAX);
        let to = transport.reply_to(Reply::SignIn { client: ClientId(1) }).into_token();
        let limits = jig_client::DomainLimits {
            document_bytes: 0,
            turn_text: crate::test::limits().client.domain.turn_text,
            ..crate::test::limits().client.domain
        };
        let source =
            people::Reply::Refused(people::Refusal::Limit { which: people::LimitKind::Arithmetic, bound: None });
        assert!(matches!(people_reply(&mut transport, to, source.clone(), &limits), Err(Invalid::Payload)));
        assert!(transport.reply(to).is_some(), "failed whole admission leaves the original exchange intact");
        let Incoming::SignInFailed { failure, .. } =
            people_reply(&mut transport, to, source, &crate::test::limits().client.domain).expect("whole refusal")
        else {
            panic!("provider refusal")
        };
        assert_eq!(
            failure,
            SignInFailure::Refused(client::Refusal::Limit {
                which: client::Limit::Admission { which: client::AdmissionLimit::Arithmetic, bound: None }
            })
        );
        assert_eq!(transport.sign_in(ClientId(1)), Some(u64::MAX), "the older actual session was not ended");
    }

    #[test]
    fn whole_task_watch_keeps_original_tab_identity_and_marks_only_normal_taken_after_admission() {
        let mut transport = transport();
        let native = Token::new(u64::MAX);
        let wrong = transport.open(ClientId(0), native, Watched::Jig(client::Subject::Task { task: 0 }));
        let owner = transport.open(ClientId(1), native, Watched::Jig(client::Subject::Task { task: u64::MAX }));
        let source = jig_domain_core::documents::Document::Task(Box::new(wire::task::tests::source()));
        let limits = wire::rows::tests::limits();
        assert!(matches!(document(&mut transport, wrong, &source, &limits), Err(Invalid::OwnerRoute)));
        assert!(matches!(
            document(
                &mut transport,
                owner,
                &source,
                &Limits { document_bytes: 0, turn_text: limits.turn_text, ..limits }
            ),
            Err(Invalid::Document(wire::documents::Invalid::Payload))
        ));
        assert!(transport.taken(ClientId(0), native).is_none());
        assert!(transport.taken(ClientId(1), native).is_none());
        let Incoming::Callback {
            client: ClientId(1),
            event:
                Event::Client {
                    event:
                        jig_client::Event::Up {
                            up:
                                jig_client::Up::Watched {
                                    watch,
                                    event: client::WatchEvent::Delivered(client::Delivery::Task(value)),
                                },
                        },
                },
        } = document(&mut transport, owner, &source, &limits).expect("whole original task")
        else {
            panic!("normal task callback")
        };
        assert_eq!(watch, native);
        assert!(value.context.is_none());
        assert_eq!(value.spec_revision.value, u64::MAX);
        assert_eq!(value.hold.as_ref().expect("owner hold").revision().value, 17);
        assert!(transport.taken(ClientId(0), native).is_none());
        assert!(
            matches!(transport.taken(ClientId(1), native), Some(jig_domain_core::Event::Delivered { watcher }) if watcher == owner)
        );
        assert!(transport.taken(ClientId(1), native).is_none());
    }

    #[test]
    fn normal_tree_checks_original_task_window_and_whole_admission_before_only_the_original_tab_taken() {
        let mut transport = transport();
        let native = Token::new(u64::MAX);
        let wrong = transport.open(ClientId(0), native, Watched::Jig(client::Subject::Tree { task: 0, window: 4 }));
        let small = transport.open(
            ClientId(0),
            Token::new(1),
            Watched::Jig(client::Subject::Tree { task: u64::MAX, window: 3 }),
        );
        let original =
            transport.open(ClientId(1), native, Watched::Jig(client::Subject::Tree { task: u64::MAX, window: 4 }));
        let source = jig_domain_core::documents::Document::Tree(wire::documents::tests::tree_source());
        let limits = wire::rows::tests::limits();
        assert!(matches!(
            document(&mut transport, wrong, &source, &limits),
            Err(Invalid::Document(wire::documents::Invalid::Identity))
        ));
        assert!(matches!(
            document(&mut transport, small, &source, &limits),
            Err(Invalid::Document(wire::documents::Invalid::Identity))
        ));
        assert!(matches!(
            document(
                &mut transport,
                original,
                &source,
                &Limits { document_bytes: 0, turn_text: limits.turn_text, ..limits }
            ),
            Err(Invalid::Document(wire::documents::Invalid::Payload))
        ));
        assert!(transport.taken(ClientId(0), native).is_none());
        assert!(transport.taken(ClientId(1), native).is_none());
        let Incoming::Callback {
            client: ClientId(1),
            event:
                Event::Client {
                    event:
                        jig_client::Event::Up {
                            up:
                                jig_client::Up::Watched {
                                    watch,
                                    event: client::WatchEvent::Delivered(client::Delivery::OwnerTree(value)),
                                },
                        },
                },
        } = document(&mut transport, original, &source, &limits).expect("admitted original tree flight")
        else {
            panic!("whole tree callback")
        };
        assert_eq!(watch, native);
        assert_eq!((value.root.task, value.rows.capacity(), value.folded.capacity()), (u64::MAX, 4, 4));
        assert!(transport.taken(ClientId(0), native).is_none());
        assert!(
            matches!(transport.taken(ClientId(1), native), Some(jig_domain_core::Event::Delivered { watcher }) if watcher == original)
        );
        assert!(transport.taken(ClientId(1), native).is_none());
    }

    #[test]
    fn normal_inbox_checks_original_party_window_and_whole_admission_before_exact_tab_taken() {
        let mut transport = transport();
        let native = Token::new(u64::MAX);
        let small = transport.open(ClientId(0), native, Watched::Jig(client::Subject::Inbox { window: 7 }));
        let owner = transport.open(ClientId(1), native, Watched::Jig(client::Subject::Inbox { window: 8 }));
        let mut source = wire::inbox::tests::source();
        let limits = wire::inbox::tests::limits();
        let wrong = jig_domain_core::documents::Document::Inbox(source.clone());
        assert!(matches!(document(&mut transport, owner, &wrong, &limits), Err(Invalid::OwnerRoute)));
        source.party = u64::MAX;
        let source = jig_domain_core::documents::Document::Inbox(source);
        assert!(matches!(document(&mut transport, small, &source, &limits), Err(Invalid::OwnerRoute)));
        assert!(matches!(
            document(
                &mut transport,
                owner,
                &source,
                &Limits { document_bytes: 100, turn_text: limits.turn_text, ..limits }
            ),
            Err(Invalid::Document(wire::documents::Invalid::Payload))
        ));
        assert!(transport.taken(ClientId(0), native).is_none());
        assert!(transport.taken(ClientId(1), native).is_none());
        let Incoming::Callback {
            client: ClientId(1),
            event:
                Event::Client {
                    event:
                        jig_client::Event::Up {
                            up:
                                jig_client::Up::Watched {
                                    watch,
                                    event: client::WatchEvent::Delivered(client::Delivery::OwnerInbox(value)),
                                },
                        },
                },
        } = document(&mut transport, owner, &source, &limits).expect("original owner flight")
        else {
            panic!("owner inbox callback")
        };
        assert_eq!(watch, native);
        assert_eq!(value.party, u64::MAX);
        assert_eq!(value.selection.as_ref().expect("source context").get(0).expect("row").goals.capacity(), 12);
        assert!(transport.taken(ClientId(0), native).is_none());
        assert!(
            matches!(transport.taken(ClientId(1), native), Some(jig_domain_core::Event::Delivered { watcher }) if watcher == owner)
        );
        assert!(transport.taken(ClientId(1), native).is_none());
    }

    #[test]
    fn normal_tasks_check_the_original_query_before_opening_the_exact_tab_flight() {
        let mut transport = transport();
        let native = Token::new(u64::MAX);
        let query = client::TaskQuery {
            project: Some(u32::MAX),
            requester: None,
            root: Some(0),
            parent: None,
            label: None,
            phase: Some(client::PhaseClass::Ended),
            executor: Some(client::ExecutorKind::Procedure),
            tracked: None,
            order: client::QueryOrder::Changed,
        };
        let watcher = transport.open(ClientId(1), native, Watched::Jig(client::Subject::Tasks { query, window: 3 }));
        let sibling = transport.open(
            ClientId(0),
            native,
            Watched::Jig(client::Subject::Tasks { query: client::TaskQuery { root: None, ..query }, window: 3 }),
        );
        let source = jig_domain_core::documents::Document::Lines(jig_domain_core::documents::Lines {
            query: wire::queries::query(&query).expect("original query"),
            lines: skein_lib::List::with_capacity(3),
            next: None,
        });
        let limits = Limits {
            rows: 4,
            turn_text: crate::test::limits().client.domain.turn_text,
            ..crate::test::limits().client.domain
        };
        assert!(matches!(
            document(&mut transport, sibling, &source, &limits),
            Err(Invalid::Document(wire::documents::Invalid::Identity))
        ));
        assert!(matches!(
            document(
                &mut transport,
                watcher,
                &source,
                &Limits { document_bytes: 0, turn_text: limits.turn_text, ..limits }
            ),
            Err(Invalid::Document(wire::documents::Invalid::Payload))
        ));
        assert!(transport.taken(ClientId(0), native).is_none());
        assert!(transport.taken(ClientId(1), native).is_none());
        let Incoming::Callback {
            client: ClientId(1),
            event:
                Event::Client {
                    event:
                        jig_client::Event::Up {
                            up:
                                jig_client::Up::Watched {
                                    watch,
                                    event: client::WatchEvent::Delivered(client::Delivery::Tasks(value)),
                                },
                        },
                },
        } = document(&mut transport, watcher, &source, &limits).expect("whole original Tasks")
        else {
            panic!("original Tasks route")
        };
        assert_eq!(watch, native);
        assert_eq!(query.phase, Some(client::PhaseClass::Ended));
        assert_eq!((value.rows.len(), value.rows.capacity()), (0, 3));
        assert!(!value.more);
        assert!(transport.taken(ClientId(0), native).is_none());
        assert!(matches!(transport.taken(ClientId(1), native),
            Some(jig_domain_core::Event::Delivered { watcher: actual }) if actual == watcher));
        assert!(transport.taken(ClientId(1), native).is_none());
    }
    #[test]
    fn whole_run_document_and_provisional_retry_keep_original_tokens_and_independent_acknowledgements() {
        let mut transport = transport();
        let native = Token::new(u64::MAX);
        let watcher =
            transport.open(ClientId(1), native, Watched::Jig(client::Subject::Run { task: 31, turn: None, offset: 0 }));
        let limits = crate::test::limits().client.domain;
        let Incoming::Callback {
            client: ClientId(1),
            event:
                Event::Client {
                    event:
                        jig_client::Event::Up {
                            up:
                                jig_client::Up::Watched {
                                    watch,
                                    event: client::WatchEvent::Delivered(client::Delivery::RunState(state)),
                                },
                        },
                },
        } = document(&mut transport, watcher, &run(31), &limits).expect("normal document")
        else {
            panic!("whole normal state under the original watch");
        };
        assert_eq!(watch, native);
        assert_eq!(state.task, 31);
        assert_eq!(state.writing, None);
        let source = jig_domain_views::Writing {
            writing: None,
            turn: jig_domain_views::TurnName { attempt: u64::MAX, turn: 17, retry: u32::MAX },
            from: 91,
            to: 93,
            text: Box::from(UTF8_WORDS),
            cut: true,
            call: None,
        };
        let Incoming::Callback {
            client: ClientId(1),
            event:
                Event::Client {
                    event:
                        jig_client::Event::Up {
                            up:
                                jig_client::Up::Watched {
                                    watch,
                                    event: client::WatchEvent::Delivered(client::Delivery::Run(text)),
                                },
                        },
                },
        } = stream(&mut transport, watcher, &source, &limits).expect("independent stream")
        else {
            panic!("actual provisional stream");
        };
        assert_eq!(watch, native);
        assert_eq!(text.turn.retry, u32::MAX);
        assert_eq!(text.from, 91);
        assert_eq!(text.through, 93);
        assert_eq!(text.text.bytes(), UTF8_WORDS);
        assert!(text.cut);
        assert!(
            matches!(transport.taken(ClientId(1), native), Some(jig_domain_core::Event::Delivered { watcher: actual }) if actual == watcher)
        );
        assert!(transport.taken(ClientId(1), native).is_none());
        assert!(matches!(transport.continued(ClientId(1), native, client::Subject::Run {
            task: 31, turn: Some(text.turn), offset: text.through,
        }).expect("exact continuation"), Some(jig_domain_core::Event::StreamDelivered { watcher: actual }) if actual == watcher));
        assert_eq!(
            transport.watch(watcher).expect("retained position").subject,
            Watched::Jig(client::Subject::Run { task: 31, turn: Some(text.turn), offset: 93 })
        );
    }
    #[test]
    fn rejected_document_identity_or_whole_envelope_never_creates_an_acknowledgement() {
        let mut transport = transport();
        let native = Token::new(17);
        let watcher =
            transport.open(ClientId(0), native, Watched::Jig(client::Subject::Run { task: 31, turn: None, offset: 0 }));
        let limits = crate::test::limits().client.domain;
        assert!(matches!(document(&mut transport, watcher, &run(32), &limits), Err(Invalid::OwnerRoute)));
        assert!(matches!(
            document(
                &mut transport,
                watcher,
                &run(31),
                &Limits { document_bytes: 0, turn_text: limits.turn_text, ..limits }
            ),
            Err(Invalid::Document(wire::documents::Invalid::Payload))
        ));
        assert!(transport.watch(watcher).is_some());
        assert!(transport.taken(ClientId(0), native).is_none());
        transport.end_watch(watcher).expect("actual detach");
        assert!(matches!(document(&mut transport, watcher, &run(31), &limits), Err(Invalid::MissingRoute)));
        assert!(transport.taken(ClientId(0), native).is_none());
    }
    #[test]
    fn exact_watch_terminal_clears_only_its_original_client_route_and_both_flights() {
        for (why, expected) in [
            (jig_domain_views::End::Unwatched, client::WatchEnd::Closed),
            (jig_domain_views::End::Gone, client::WatchEnd::SubjectEnded),
            (jig_domain_views::End::NotAllowed, client::WatchEnd::NotAllowed),
            (jig_domain_views::End::SignedOut, client::WatchEnd::SignedOut),
            (jig_domain_views::End::Stopping, client::WatchEnd::Stopping),
        ] {
            let mut transport = transport();
            transport.signed_in(ClientId(0), u64::MAX, 7);
            transport.signed_in(ClientId(1), u64::MAX, 8);
            let native = Token::new(17);
            let subject = client::Subject::Run { task: 31, turn: None, offset: 0 };
            let watcher = transport.open(ClientId(0), native, Watched::Jig(subject.clone()));
            let other = transport.open(ClientId(1), native, Watched::Jig(subject));
            assert!(transport.delivered(watcher, Pace::Document));
            assert!(transport.delivered(watcher, Pace::Stream));
            let Incoming::Callback {
                client: ClientId(0),
                event: Event::Client { event: jig_client::Event::Up { up: jig_client::Up::WatchEnded { watch, end } } },
            } = ended(&mut transport, watcher, why).expect("actual terminal")
            else {
                panic!("original terminal");
            };
            assert_eq!(watch, native);
            assert_eq!(end, expected);
            assert_eq!(
                transport.sign_in(ClientId(0)),
                if expected == client::WatchEnd::SignedOut { None } else { Some(7) }
            );
            assert_eq!(transport.sign_in(ClientId(1)), Some(8));
            assert!(transport.watch(watcher).is_none());
            assert!(transport.watch(other).is_some());
            assert!(transport.taken(ClientId(0), native).is_none());
            assert!(matches!(ended(&mut transport, watcher, why), Err(Invalid::MissingRoute)));
        }
    }
    #[test]
    fn a_sign_in_names_its_session_and_a_wrong_party_does_not_consume_the_route() {
        let mut transport = transport();
        let token = transport.reply_to(Reply::SignIn { client: ClientId(1) }).into_token();
        let reply = people::Reply::SignedIn { person: u64::MAX, sign_in: 0, expires: Wall::EPOCH };
        assert!(matches!(
            people_reply(
                &mut transport,
                token,
                people::Reply::SignedIn { person: 1, sign_in: 0, expires: Wall::EPOCH },
                &crate::test::limits().client.domain
            ),
            Err(Invalid::Session)
        ));
        assert!(transport.reply(token).is_some());
        assert!(matches!(
            people_reply(&mut transport, token, reply, &crate::test::limits().client.domain),
            Ok(Incoming::Authenticated { client: ClientId(1), person: u64::MAX, expires: Wall::EPOCH })
        ));
        assert_eq!(transport.sign_in(ClientId(1)), Some(0));
        assert_eq!(transport.sign_in(ClientId(0)), None);
        assert!(transport.reply(token).is_none());
    }
    #[test]
    fn sparse_entrance_refusal_returns_to_the_original_application_child_with_its_exact_attempt() {
        let mut transport = transport();
        let attempt = Token::new(u64::MAX);
        let token = transport
            .reply_to(Reply::Keyed {
                client: ClientId(1),
                attempt,
                key: [255; 16],
                owner: KeyedOwner::Application { connector: 1 },
            })
            .into_token();
        let why = people::Refusal::Moved { now: people::Moved::Person { revision: u64::MAX } };
        let Incoming::Callback {
            client,
            event:
                Event::Client {
                    event:
                        jig_client::Event::Up {
                            up:
                                jig_client::Up::Answer {
                                    attempt: actual,
                                    envelope,
                                    answer: Some(Answered::Jig(client::Answer::Refused { refusal: answer, at: None })),
                                    why: None,
                                },
                        },
                },
        } = people_reply(&mut transport, token, people::Reply::Refused(why), &crate::test::limits().client.domain)
            .expect("original child")
        else {
            panic!("original application refusal");
        };
        assert_eq!(client, ClientId(1));
        assert_eq!(actual, attempt);
        assert_eq!(envelope, RequestEnvelope::Final);
        assert_eq!(
            &answer,
            &client::Refusal::Moved { now: client::Revision { value: u64::MAX }, what: client::Moved::PersonTask }
        );
        assert!(transport.reply(token).is_none());
    }
    #[test]
    fn whole_connector_answer_cannot_cross_a_jig_or_other_connector_route_and_refuses_unchanged_overflow() {
        let mut transport = transport();
        let attempt = Token::new(0);
        let token = transport
            .reply_to(Reply::Keyed {
                client: ClientId(0),
                attempt,
                key: [0; 16],
                owner: KeyedOwner::Application { connector: 1 },
            })
            .into_token();
        let outcome = ApplicationOutcome::Role { held: u32::MAX };
        let mut limits = ConnectorLimits { rows: 4, paths: 4, segments: 4, segment_bytes: 16, bytes: 100_000 };
        assert!(matches!(application_reply(&mut transport, token, 2, outcome, &limits), Err(Invalid::OwnerRoute)));
        assert!(transport.reply(token).is_some());
        limits.bytes = 0;
        assert!(matches!(application_reply(&mut transport, token, 1, outcome, &limits), Err(Invalid::Payload)));
        assert!(transport.reply(token).is_some());
        limits.bytes = 100_000;
        let Incoming::Callback {
            client: ClientId(0),
            event:
                Event::Client {
                    event:
                        jig_client::Event::Up {
                            up:
                                jig_client::Up::Answer {
                                    attempt: actual,
                                    envelope: RequestEnvelope::Final,
                                    answer: Some(Answered::Family(record)),
                                    why: None,
                                },
                        },
                },
        } = application_reply(&mut transport, token, 1, outcome, &limits).expect("whole owner answer")
        else {
            panic!("application callback");
        };
        assert_eq!(actual, attempt);
        let family::Answer::Connector(answer) =
            family::wire::decode_answer(&record.bytes, &jig_core_world::world::limits().connector)
                .expect("whole application answer")
        else {
            panic!("connector answer")
        };
        assert_eq!(answer.value, outcome);
        let jig = transport
            .reply_to(Reply::Keyed { client: ClientId(1), attempt, key: [0; 16], owner: KeyedOwner::Jig })
            .into_token();
        assert!(matches!(application_reply(&mut transport, jig, 1, outcome, &limits), Err(Invalid::OwnerRoute)));
        assert!(transport.reply(jig).is_some());
    }
    fn application_subject() -> family::Subject {
        family::Subject { scope: family::Scope::Project(u32::MAX), kind: family::Kind::Result(u64::MAX) }
    }
    #[test]
    fn whole_application_documents_keep_supplied_scope_original_tab_token_and_normal_taken_only() {
        let mut transport = transport();
        let native = Token::new(17);
        let subject = application_subject();
        let watcher = transport.open(
            ClientId(0),
            native,
            Watched::Application { subject, scope: jig_domain_people::ApplicationScope::Project(u32::MAX) },
        );
        let other = transport.open(
            ClientId(1),
            native,
            Watched::Application { subject, scope: jig_domain_people::ApplicationScope::Project(u32::MAX) },
        );
        let limits = crate::test::limits().first.connector;
        assert!(matches!(
            application_document(
                &mut transport,
                watcher,
                2,
                jig_test_connector::ClientDocument::Result { task: u64::MAX, code: u16::MAX },
                &limits
            ),
            Err(Invalid::OwnerRoute)
        ));
        assert!(matches!(
            application_document(
                &mut transport,
                watcher,
                jig_client_test_domain::FIRST,
                jig_test_connector::ClientDocument::Result { task: 1, code: u16::MAX },
                &limits
            ),
            Err(Invalid::Application(jig_client_test_domain::family::wire::Invalid::Identity))
        ));
        assert!(transport.application_taken(ClientId(0), native).is_none());
        let Incoming::Callback {
            client: ClientId(0),
            event:
                Event::First(jig_client_test_domain::first::Event::Watch {
                    watch,
                    event: family::WatchEvent::Delivered(family::Document::Connector(value)),
                }),
        } = application_document(
            &mut transport,
            watcher,
            jig_client_test_domain::FIRST,
            jig_test_connector::ClientDocument::Result { task: u64::MAX, code: u16::MAX },
            &limits,
        )
        .expect("whole original owner value")
        else {
            panic!("original application child")
        };
        assert_eq!(watch, native);
        assert_eq!(value.subject, subject);
        assert_eq!(*value.value, jig_test_connector::ClientDocument::Result { task: u64::MAX, code: u16::MAX });
        assert!(
            matches!(transport.application_taken(ClientId(0), native), Some(jig_domain_core::Event::Delivered { watcher: actual }) if actual == watcher)
        );
        assert!(transport.application_taken(ClientId(0), native).is_none());
        assert!(transport.application_taken(ClientId(1), native).is_none());
        assert!(transport.watch(other).is_some());
    }
    #[test]
    fn application_read_missing_reasons_remain_distinct_and_only_consume_the_original_reply() {
        for (source, expected) in [
            (jig_test_connector::DocumentMissing::Unknown, family::ReadResult::Unknown),
            (jig_test_connector::DocumentMissing::NotAllowed, family::ReadResult::NotAllowed),
        ] {
            let mut transport = transport();
            let read = Token::new(17);
            let token = transport
                .reply_to(Reply::ApplicationRead { client: ClientId(0), read, subject: application_subject() })
                .into_token();
            let other = transport
                .reply_to(Reply::ApplicationRead { client: ClientId(1), read, subject: application_subject() })
                .into_token();
            let limits = crate::test::limits().first.connector;
            assert!(matches!(
                application_read(&mut transport, token, 2, Err(source), &limits),
                Err(Invalid::OwnerRoute)
            ));
            assert!(transport.reply(token).is_some());
            let Incoming::Callback {
                client: ClientId(0),
                event: Event::First(jig_client_test_domain::first::Event::Read { read: actual, result }),
            } = application_read(&mut transport, token, jig_client_test_domain::FIRST, Err(source), &limits)
                .expect("exact owner terminal")
            else {
                panic!("original application read")
            };
            assert_eq!(actual, read);
            assert_eq!(result, expected);
            assert!(transport.reply(token).is_none());
            assert!(transport.reply(other).is_some());
            assert!(matches!(
                application_read(&mut transport, token, jig_client_test_domain::FIRST, Err(source), &limits),
                Err(Invalid::MissingRoute)
            ));
        }
    }
    #[test]
    fn held_and_immediate_application_watch_departures_keep_owner_reasons_without_dropping_sibling_routes() {
        for (source, expected) in [
            (client::WatchEnd::Closed, family::WatchEnd::Closed),
            (client::WatchEnd::Dropped, family::WatchEnd::Dropped),
            (client::WatchEnd::Unknown, family::WatchEnd::Unknown),
            (client::WatchEnd::NotAllowed, family::WatchEnd::NotAllowed),
            (client::WatchEnd::SignedOut, family::WatchEnd::SignedOut),
            (client::WatchEnd::SubjectEnded, family::WatchEnd::SubjectEnded),
            (client::WatchEnd::Stopping, family::WatchEnd::Stopping),
            (client::WatchEnd::Busy, family::WatchEnd::Busy),
        ] {
            let mut transport = transport();
            let native = Token::new(17);
            let original = transport.open(
                ClientId(0),
                native,
                Watched::Application {
                    subject: application_subject(),
                    scope: jig_domain_people::ApplicationScope::Project(u32::MAX),
                },
            );
            let other = transport.open(
                ClientId(1),
                native,
                Watched::Application {
                    subject: application_subject(),
                    scope: jig_domain_people::ApplicationScope::Project(u32::MAX),
                },
            );
            let Incoming::Callback {
                client: ClientId(0),
                event: Event::First(jig_client_test_domain::first::Event::Ended { watch, end }),
            } = terminal(&mut transport, original, source).expect("exact terminal")
            else {
                panic!("original application watch")
            };
            assert_eq!(watch, native);
            assert_eq!(end, expected);
            assert!(transport.watch(original).is_none());
            assert!(transport.watch(other).is_some());
            assert!(transport.application_taken(ClientId(0), native).is_none());
        }
    }
}
