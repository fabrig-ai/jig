//! Facts decoded independently at the byte boundary, before the root sees them.
use crate::native::Received;
use jig_client::{Message, Request as ClientRequest, contract as c};
use jig_client_protocol_wire::v19 as w;
use jig_client_test_domain::{Event, Request};
use jig_client_world::harness::{Receipt, Sent};
use skein_lib::{Reader, RequestEnvelope, Token};

#[derive(PartialEq, Eq)]
pub(super) enum Callback {
    Reply(Token),
    Record(Box<[u8]>),
    Gone,
    Heartbeat,
}
pub(super) fn callback(event: &jig_client::Event) -> Option<Callback> {
    match event {
        jig_client::Event::Reply { exchange, .. } => Some(Callback::Reply(*exchange)),
        jig_client::Event::Streamed { message: jig_client::Streamed::Record(bytes) } => {
            Some(Callback::Record(bytes.clone()))
        }
        jig_client::Event::Streamed { message: jig_client::Streamed::Heartbeat } => Some(Callback::Heartbeat),
        jig_client::Event::Streamed { message: jig_client::Streamed::SignedOut } | jig_client::Event::StreamGone => {
            Some(Callback::Gone)
        }
        jig_client::Event::Start { .. }
        | jig_client::Event::StartUnlinked { .. }
        | jig_client::Event::SignIn { .. }
        | jig_client::Event::Authorize { .. }
        | jig_client::Event::SignOut
        | jig_client::Event::SignOutLost
        | jig_client::Event::Watch { .. }
        | jig_client::Event::Unwatch { .. }
        | jig_client::Event::Read { .. }
        | jig_client::Event::Send { .. }
        | jig_client::Event::Carry { .. }
        | jig_client::Event::Lost { .. }
        | jig_client::Event::Stored
        | jig_client::Event::NotStored
        | jig_client::Event::Up { .. } => None,
    }
}

#[must_use]
pub fn sent(request: &Request) -> Option<Sent> {
    let Request::Client { request } = request else { return None };
    client_sent(request)
}

/// Observe a linked client's request independently of its application's root type.
#[must_use]
pub fn client_sent(request: &ClientRequest) -> Option<Sent> {
    let ClientRequest::Exchange { exchange, message: Message::Request(bytes), .. } = request else { return None };
    let keyed = w::Keyed::decode(&w::CEILINGS, &mut Reader::new(bytes)).expect("keyed bytes");
    let mut sent = Sent { attempt: *exchange, key: *keyed.key(), card: None, revision: None, words: None };
    match keyed.owner() {
        w::Owner::Jig => {
            let ask = w::Ask::decode(&w::CEILINGS, &mut Reader::new(keyed.request())).expect("jig ask bytes");
            let decided = match ask {
                w::Ask::Seen(_) => return None,
                w::Ask::Compound(value) => {
                    let mut reader = Reader::new(value.operations());
                    let length = reader.u32().expect("piece length");
                    let bytes = reader.bytes(length).expect("first operation");
                    match w::Operation::decode(&w::CEILINGS, &mut Reader::new(bytes)).expect("operation") {
                        w::Operation::Message(v) => {
                            sent.words = Some((v.task(), v.words().to_vec()));
                            None
                        }
                        w::Operation::Amend(v) => {
                            v.revision().as_ref().map(|r| (c::ObjectKey::Task(v.task()), 1, r.value()))
                        }
                        w::Operation::Cancel(_)
                        | w::Operation::Release(_)
                        | w::Operation::Stop(_)
                        | w::Operation::MoveTask(_) => None,
                    }
                }
                w::Ask::Message(value) => {
                    sent.words = Some((value.task(), value.words().to_vec()));
                    None
                }
                w::Ask::Answer(value) => match value.on() {
                    w::AnswerOn::Question(v) => {
                        sent.words = Some((v.task(), v.words().to_vec()));
                        None
                    }
                    w::AnswerOn::Person(v) => Some((c::ObjectKey::PersonTask(v.task()), 3, v.revision().value())),
                },
                w::Ask::Release(value) => Some((c::ObjectKey::Task(value.task()), 2, value.revision().value())),
                w::Ask::Decide(value) => match value.on() {
                    w::DecideOn::Proposal(_) => None,
                    w::DecideOn::Escalation(v) => Some((c::ObjectKey::Escalation(v.task()), 2, v.revision().value())),
                },
                w::Ask::Amend(value) => {
                    value.revision().as_ref().map(|v| (c::ObjectKey::Task(value.task()), 1, v.value()))
                }
                w::Ask::EditNote(value) => Some((
                    c::ObjectKey::Note { project: value.project(), note: value.note() },
                    4,
                    value.revision().value(),
                )),
                w::Ask::DeleteNote(value) => Some((
                    c::ObjectKey::Note { project: value.project(), note: value.note() },
                    4,
                    value.revision().value(),
                )),
                w::Ask::Propose(value) => match value.action() {
                    w::ProposeAction::Settings(_) | w::ProposeAction::Batch(_) => None,
                    w::ProposeAction::Amendment(v) => Some((c::ObjectKey::Task(v.task()), 1, v.revision().value())),
                },
                w::Ask::Hold(_)
                | w::Ask::Lift(_)
                | w::Ask::Withdraw(_)
                | w::Ask::Create(_)
                | w::Ask::Take(_)
                | w::Ask::HandBack(_)
                | w::Ask::Stop(_)
                | w::Ask::Cancel(_)
                | w::Ask::MoveTask(_)
                | w::Ask::ChangePolicy(_)
                | w::Ask::SetRoles(_)
                | w::Ask::MakeService(_)
                | w::Ask::SignIn(_)
                | w::Ask::AccountChange(_)
                | w::Ask::SignOut(_)
                | w::Ask::Rename(_)
                | w::Ask::ChangeSettings(_)
                | w::Ask::MoveToNewest(_)
                | w::Ask::MakeProject(_)
                | w::Ask::SetPool(_) => None,
            };
            if let Some((key, kind, revision)) = decided {
                sent.card = Some(jig_client_world::observe::key(key));
                sent.revision = Some((kind, revision));
            }
        }
        w::Owner::Family(_) => {
            use jig_client_test_protocol_wire::v2 as family;
            let envelope = w::FamilyRequest::decode(&w::CEILINGS, &mut Reader::new(keyed.request()))
                .expect("family request envelope");
            let request = family::Request::decode(&family::CEILINGS, &mut Reader::new(envelope.record()))
                .expect("family request");
            let subject = jig_client_test_domain::family::Subject {
                scope: jig_client_test_domain::family::Scope::Project(request.project()),
                kind: jig_client_test_domain::family::Kind::Item(request.value().object()),
            };
            sent.card = Some(jig_client_world::harness::CardKey::of(jig_client_test_view::family_key(&subject)));
            sent.revision = Some((0, request.value().revision()));
        }
    }
    Some(sent)
}

#[must_use]
pub fn receipts(fact: &Received) -> Vec<Receipt> {
    let event = match fact {
        Received::Answer { attempt, answer } => Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Answer {
                    attempt: *attempt,
                    envelope: RequestEnvelope::Final,
                    answer: Some(jig_client::Answered::Jig(answer.clone())),
                    why: None,
                },
            },
        },
        Received::Delivery(c::Delivery::Me(value)) => {
            return vec![Receipt::Party { party: value.party.number, watch: None }];
        }
        Received::Delivery(value) => Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched { watch: Token::new(0), event: c::WatchEvent::Delivered(value.clone()) },
            },
        },
        Received::Read(result) => Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::ReadDone { read: Token::new(0), result: jig_client::Readout::Jig(result.clone()) },
            },
        },
        Received::SignedIn { .. } | Received::Sent { .. } => return Vec::new(),
    };
    jig_client_world::observe::received(&event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use skein_lib::{Overflow, Writer};

    const ATTEMPT: Token = Token::new(7);
    const REQUEST_KEY: [u8; 16] = [3; 16];
    const PROJECT: u32 = 1;
    const TASK: u64 = 8;

    fn encoded<T>(value: &T, measure: fn(&T) -> u32, encode: fn(&T, &mut Writer) -> Result<(), Overflow>) -> Box<[u8]> {
        let mut writer = Writer::new(usize::try_from(measure(value)).expect("bounded wire fixture"));
        encode(value, &mut writer).expect("measured wire fixture");
        writer.finish()
    }

    fn request(owner: w::Owner, version: u32, bytes: Box<[u8]>) -> ClientRequest {
        let keyed = w::Keyed::new(
            &w::CEILINGS,
            w::KeyedParts { key: REQUEST_KEY, owner, version, request: bytes, value: None },
        )
        .expect("bounded keyed fixture");
        ClientRequest::Exchange {
            exchange: ATTEMPT,
            credential: None,
            message: Message::Request(encoded(&keyed, w::Keyed::measure, w::Keyed::encode)),
        }
    }

    fn jig_request(ask: &c::Ask) -> ClientRequest {
        let ask = jig_client_protocol_link::translate::asks::ask(ask, &w::CEILINGS).expect("bounded jig fixture");
        request(w::Owner::Jig, jig_client_protocol_wire::VERSION, encoded(&ask, w::Ask::measure, w::Ask::encode))
    }

    #[test]
    fn encoded_top_level_seen_has_no_retained_answer_observation() {
        for through in [c::SeenThrough::Results { through: 1 }, c::SeenThrough::Conversation { task: TASK, through: 1 }]
        {
            assert!(client_sent(&jig_request(&c::Ask::Seen { through })).is_none(), "{through:?}");
        }
    }

    #[test]
    fn encoded_ordinary_requests_without_cards_keep_their_observations() {
        use jig_client_world::test::list;
        let words = c::Text::new(b"continue".as_slice().into(), 32).expect("fixture text");
        let asks = [
            (c::Ask::Stop { project: PROJECT, task: TASK }, None),
            (c::Ask::Take { project: PROJECT, task: TASK }, None),
            (
                c::Ask::Message { project: PROJECT, task: TASK, words: words.clone() },
                Some((TASK, b"continue".to_vec())),
            ),
            (
                c::Ask::Compound {
                    project: PROJECT,
                    operations: list([
                        c::Operation::Stop { project: PROJECT, task: TASK },
                        c::Operation::Message { project: PROJECT, task: TASK, words: words.clone() },
                    ]),
                },
                None,
            ),
            (
                c::Ask::Compound {
                    project: PROJECT,
                    operations: list([
                        c::Operation::Message { project: PROJECT, task: TASK, words },
                        c::Operation::Stop { project: PROJECT, task: TASK },
                    ]),
                },
                Some((TASK, b"continue".to_vec())),
            ),
        ];
        for (ask, words) in asks {
            let sent = client_sent(&jig_request(&ask)).expect("ordinary request observed");
            assert_eq!(
                (sent.attempt, sent.key, sent.card, sent.revision, sent.words),
                (ATTEMPT, REQUEST_KEY, None, None, words),
                "{ask:?}"
            );
        }
    }

    #[test]
    fn encoded_family_requests_keep_their_card_and_revision() {
        let record = crate::family::request(&jig_client_test_domain::family::Ask::Touch {
            project: PROJECT,
            item: TASK,
            revision: c::Revision { value: 5 },
            parameters: None,
        });
        let envelope = w::FamilyRequest::new(
            &w::CEILINGS,
            w::FamilyRequestParts {
                scope: w::Scope::Project(
                    w::ScopeProject::new(&w::CEILINGS, w::ScopeProjectParts { value: PROJECT })
                        .expect("bounded project fixture"),
                ),
                record: record.bytes,
            },
        )
        .expect("bounded family fixture");
        let owner = w::Owner::Family(
            w::Family::new(&w::CEILINGS, w::FamilyParts { number: record.family }).expect("bounded family fixture"),
        );
        let sent = client_sent(&request(
            owner,
            record.version,
            encoded(&envelope, w::FamilyRequest::measure, w::FamilyRequest::encode),
        ))
        .expect("family request observed");
        let card = jig_client_world::harness::CardKey::of(jig_client_test_view::family_key(
            &jig_client_test_domain::family::Subject {
                scope: jig_client_test_domain::family::Scope::Project(PROJECT),
                kind: jig_client_test_domain::family::Kind::Item(TASK),
            },
        ));
        assert_eq!(
            (sent.attempt, sent.key, sent.card, sent.revision, sent.words),
            (ATTEMPT, REQUEST_KEY, Some(card), Some((0, 5)), None)
        );
    }
}
