#[path = "fixtures/create.rs"]
mod create;
pub use create::*;
#[path = "fixtures/operations.rs"]
mod operations;
pub use operations::*;
#[path = "fixtures/envelope.rs"]
mod envelope;
use super::super::*;
pub use envelope::*;
use skein_lib::{Duration, List, Secret, Wall, Writer};
pub const NUMBER: u64 = 41;
// Keep the old intake's 256-byte cap, enough for these small message records.
pub const MESSAGE_BYTES: u32 = 256;
pub fn message_bytes(size: usize) -> Box<[u8]> {
    let mut bytes = Writer::new(size);
    for _ in 0..size {
        bytes.put(&[0]).expect("bounded bytes");
    }
    bytes.finish()
}
pub fn wire() -> w::Limits {
    w::Limits {
        keyed_request: 256,
        owned_record: 256,
        ending_rows: 128,
        message_words: 64,
        amend_reason: 64,
        ..w::CEILINGS
    }
}
pub fn settings() -> Settings {
    let mut served = List::with_capacity(2);
    served.push(crate::Served { family: None, first: 19, last: 19 }).expect("one jig range");
    served.push(crate::Served { family: Some(9), first: 2, last: 2 }).expect("one family range");
    Settings {
        provider: None,
        addresses: Box::new([]),
        deployment_provider: 2,
        link: None,
        heartbeat: Duration::from_secs(5),
        rows: 32,
        served,
        providers: List::with_capacity(0),
        seed: Secret::new(Box::new([7; 32])),
        stream: 31,
        models: models(),
        key_bytes: 7934,
        redirect_bytes: 4096,
        moved_bytes: 70000,
    }
}
/// Two configured models: one with efforts, provider, cache-write price and description,
/// one with none of those optional values.
pub fn models() -> Box<[crate::Model]> {
    Box::new([
        crate::Model {
            name: Box::from(b"careful".as_slice()),
            provider: Some(3),
            window: 200_000,
            output: 32_000,
            efforts: Box::new([Box::from(b"low".as_slice()), Box::from(b"high".as_slice())]),
            prices: crate::Prices { input: 3, cached: 1, cache_write: Some(5), output: 15, unit: 1_000_000 },
            description: Some(Box::from(b"For careful work".as_slice())),
        },
        crate::Model {
            name: Box::from(b"quick".as_slice()),
            provider: None,
            window: 8192,
            output: 1024,
            efforts: Box::new([]),
            prices: crate::Prices { input: 1, cached: 1, cache_write: None, output: 2, unit: 1000 },
            description: None,
        },
    ])
}
pub fn env() -> Env<Limits> {
    Env {
        now: Time::ZERO,
        wall: Wall::EPOCH,
        limits: Limits {
            streams: 2,
            watches_per_stream: 4,
            queued_bytes: 65536,
            identity_bytes: 64,
            families: 2,
            providers: 0,
            mints: 2,
            provider_codes: 16,
            provider_tokens: 64,
            exchanges: 4,
            exchanges_per_client: 2,
            message_bytes: MESSAGE_BYTES,
            wire: wire(),
            link_span: Duration::from_secs(300),
        },
    }
}
pub fn start(limits: &w::Limits) -> w::Ask {
    w::Ask::Message(
        w::Message::new(limits, w::MessageParts { project: 3, task: 8, words: Box::from(b"hello".as_slice()) })
            .expect("message"),
    )
}
pub fn revision(value: u64) -> w::Revision {
    w::Revision::new(&wire(), w::RevisionParts { value }).expect("revision")
}
pub fn amendment(release: Option<w::Revision>, reason: &[u8]) -> w::Ask {
    let value = Box::new(
        w::Amend::new(
            &wire(),
            w::AmendParts {
                project: 3,
                task: 8,
                revision: Some(revision(6)),
                reason: Box::from(reason),
                change: w::Amendment::new(
                    &wire(),
                    w::AmendmentParts {
                        llm: None,
                        tracked: None,
                        addressed: None,
                        spec: None,
                        wake: None,
                        dependencies: Some(List::with_capacity(0)),
                        authority: None,
                        instructions: Some(Box::new([])),
                    },
                )
                .expect("change"),
            },
        )
        .expect("amend"),
    );
    match release {
        Some(revision) => compound(
            3,
            &[
                w::Operation::Amend(value),
                w::Operation::Release(
                    w::Release::new(&wire(), w::ReleaseParts { project: 3, task: 8, revision, again: None }).unwrap(),
                ),
            ],
        ),
        None => w::Ask::Amend(value),
    }
}
pub struct Exchange {
    pub component: Component,
    pub env: Env<Limits>,
    pub out: Queue<Request>,
    pub token: Box<[u8]>,
}
impl Exchange {
    pub fn new() -> Self {
        Self::with_env(env())
    }
    pub fn with_env(env: Env<Limits>) -> Self {
        Self::configured(env, settings())
    }
    pub fn configured(env: Env<Limits>, settings: Settings) -> Self {
        let mut out = Queue::with_capacity(8);
        let component = Component::new(settings, &env, &mut out).expect("starts");
        out.pop();
        let mut token = Writer::new(40);
        token.put(&NUMBER.to_be_bytes()).expect("number");
        token.put(&[7; 32]).expect("random bytes");
        Self { component, env, out, token: token.finish() }
    }
    pub fn send(&mut self, message: crate::Message) -> Token {
        let owner = self.component.name();
        self.component.down(
            &self.env,
            Event::Transport(crate::FromTransport::Exchange {
                exchange: owner,
                credential: Some(Secret::new(self.token.clone())),
                message,
            }),
            &mut self.out,
        );
        owner
    }
    pub fn stream_open(&mut self) -> Token {
        let owner = self.component.name();
        self.component.down(
            &self.env,
            Event::Transport(crate::FromTransport::StreamOpen {
                stream: owner,
                credential: Some(Secret::new(self.token.clone())),
            }),
            &mut self.out,
        );
        owner
    }
    pub fn authenticate(&mut self) {
        let Some(Request::Secrets(SecretsRequest::Get { owner })) = self.out.pop() else {
            panic!("digest get");
        };
        assert_eq!(owner, SecretOwner::SignIn(NUMBER));
        self.component.down(
            &self.env,
            Event::Secrets(SecretsAnswer::Got {
                owner: SecretOwner::SignIn(NUMBER),
                value: Some(Kept {
                    digest: sign_in::digest(&self.token),
                    expires: Wall::from_nanos(86_400_000_000_000),
                }),
            }),
            &mut self.out,
        );
        assert!(self.component.up(&self.env, &mut self.out));
    }
    pub fn response(&self, owner: Token) -> (u16, &[u8]) {
        let connection = self.component.connections.get(&owner).expect("connection");
        match connection.response.as_ref() {
            Some(crate::Reply::Record(body)) => (200, body),
            Some(crate::Reply::NotARecord) => (400, &[]),
            Some(crate::Reply::Conflict | crate::Reply::StreamFull(_)) => (409, &[]),
            None => match connection.ended {
                Some(crate::Ended::SignedOut) => (401, &[]),
                Some(crate::Ended::Busy | crate::Ended::Stopping) => (409, &[]),
                None => panic!("response"),
            },
        }
    }
}

pub fn other_asks() -> [(w::Ask, people::Ask); 17] {
    let limits = wire();
    let mut holdings = List::with_capacity(1);
    holdings
        .push(
            w::Holding::new(
                &limits,
                w::HoldingParts {
                    party: 7,
                    role: w::HeldRole::Policy(
                        w::HeldRolePolicy::new(&limits, w::HeldRolePolicyParts { role: u32::MAX }).expect("role"),
                    ),
                },
            )
            .expect("holding"),
        )
        .expect("room");
    let mut references = List::with_capacity(1);
    references.push(u64::MAX).expect("room");
    [
        write_case(&limits),
        answer_question_case(&limits),
        take_case(&limits),
        hand_back_case(&limits),
        release_case(&limits),
        stop_case(&limits),
        cancel_case(&limits),
        take_over_case(&limits),
        set_pool_case(&limits),
        mark_seen_case(&limits),
        answer_person_case(&limits),
        decide_proposal_case(&limits),
        set_roles_case(&limits, holdings),
        make_service_case(&limits),
        change_policy_case(&limits),
        delete_note_case(&limits),
        edit_note_case(&limits, references),
    ]
}
fn write_case(limits: &w::Limits) -> (w::Ask, people::Ask) {
    use jig_domain_people as p;
    (
        w::Ask::Message(
            w::Message::new(limits, w::MessageParts { project: 3, task: 8, words: Box::from(b"say".as_slice()) })
                .expect("Write"),
        ),
        p::Ask::Say { project: 3, task: 8, words: Box::from(b"say".as_slice()) },
    )
}
fn answer_question_case(limits: &w::Limits) -> (w::Ask, people::Ask) {
    use jig_domain_people as p;
    (
        answer(w::AnswerOn::Question(
            w::AnswerQuestion::new(
                limits,
                w::AnswerQuestionParts { task: 8, message: 19, words: Box::from(b"answer".as_slice()) },
            )
            .expect("AnswerQuestion"),
        )),
        p::Ask::AnswerQuestion { project: 3, task: 8, question: 19, words: Box::from(b"answer".as_slice()) },
    )
}
fn take_case(limits: &w::Limits) -> (w::Ask, people::Ask) {
    use jig_domain_people as p;
    (
        w::Ask::Take(w::Take::new(limits, w::TakeParts { project: 3, task: 8 }).expect("Take")),
        p::Ask::TakePerson { project: 3, task: 8 },
    )
}
fn hand_back_case(limits: &w::Limits) -> (w::Ask, people::Ask) {
    use jig_domain_people as p;
    (
        w::Ask::HandBack(w::HandBack::new(limits, w::HandBackParts { project: 3, task: 8 }).expect("HandBack")),
        p::Ask::HandBackPerson { project: 3, task: 8 },
    )
}
fn release_case(limits: &w::Limits) -> (w::Ask, people::Ask) {
    use jig_domain_people as p;
    (
        w::Ask::Release(
            w::Release::new(limits, w::ReleaseParts { project: 3, task: 8, revision: revision(6), again: Some(false) })
                .expect("Release"),
        ),
        p::Ask::Release { project: 3, task: 8, revision: 6, again: Some(false) },
    )
}
fn stop_case(limits: &w::Limits) -> (w::Ask, people::Ask) {
    use jig_domain_people as p;
    (
        w::Ask::Stop(w::Stop::new(limits, w::StopParts { project: 3, task: 8 }).expect("Stop")),
        p::Ask::Stop { project: 3, task: 8 },
    )
}
fn cancel_case(limits: &w::Limits) -> (w::Ask, people::Ask) {
    use jig_domain_people as p;
    (
        w::Ask::Cancel(
            w::Cancel::new(
                limits,
                w::CancelParts { grace: None, project: 3, task: 8, reason: Box::from(b"cancel".as_slice()) },
            )
            .expect("Cancel"),
        ),
        p::Ask::Cancel { project: 3, task: 8, reason: Box::from(b"cancel".as_slice()), grace: None },
    )
}
fn take_over_case(limits: &w::Limits) -> (w::Ask, people::Ask) {
    use jig_domain_people as p;
    (
        w::Ask::MoveTask(
            w::Move::new(limits, w::MoveParts { project: 3, task: 8, reason: Box::from(b"take".as_slice()) })
                .expect("move"),
        ),
        p::Ask::Move { project: 3, task: 8, reason: Box::from(b"take".as_slice()) },
    )
}
fn set_pool_case(limits: &w::Limits) -> (w::Ask, people::Ask) {
    use jig_domain_people as p;
    (
        w::Ask::SetPool(
            w::SetPool::new(limits, w::SetPoolParts { project: 3, party: u64::MAX, budget: u64::MAX })
                .expect("SetPool"),
        ),
        p::Ask::SetPool { project: 3, person: u64::MAX, budget: u64::MAX },
    )
}
fn mark_seen_case(limits: &w::Limits) -> (w::Ask, people::Ask) {
    use jig_domain_people as p;
    (
        w::Ask::Seen(
            w::Seen::new(
                limits,
                w::SeenParts {
                    through: w::SeenThrough::Conversation(
                        w::SeenConversation::new(limits, w::SeenConversationParts { task: 8, through: u64::MAX })
                            .expect("seen"),
                    ),
                },
            )
            .expect("MarkSeen"),
        ),
        p::Ask::MarkSeen { seen: p::Seen::Conversation { task: 8, through: u64::MAX } },
    )
}
fn answer_person_case(limits: &w::Limits) -> (w::Ask, people::Ask) {
    use jig_domain_people as p;
    (
        answer(w::AnswerOn::Person(
            w::AnswerPerson::new(
                limits,
                w::AnswerPersonParts {
                    task: 8,
                    revision: revision(6),
                    answer: w::PersonAnswer::Approved(
                        w::PersonAnswerApproved::new(
                            limits,
                            w::PersonAnswerApprovedParts { state: Box::new([255, 0]), words: Some(Box::new([])) },
                        )
                        .expect("answer"),
                    ),
                },
            )
            .expect("AnswerPerson"),
        )),
        p::Ask::AnswerPerson {
            project: 3,
            task: 8,
            revision: 6,
            result: p::PersonResult::Approved { state: Box::new([255, 0]), words: Some(Box::new([])) },
        },
    )
}
fn decide_proposal_case(limits: &w::Limits) -> (w::Ask, people::Ask) {
    use jig_domain_people as p;
    (
        decide(w::DecideOn::Proposal(
            w::DecideProposal::new(
                limits,
                w::DecideProposalParts {
                    proposal: w::ProposalKey::new(
                        limits,
                        w::ProposalKeyParts {
                            proposer: w::Proposer::Party(
                                w::ProposerParty::new(limits, w::ProposerPartyParts { value: u64::MAX })
                                    .expect("party"),
                            ),
                            number: 5,
                        },
                    )
                    .expect("proposal"),
                    decision: w::Decision::Accepted,
                    running: None,
                },
            )
            .expect("DecideProposal"),
        )),
        p::Ask::DecideProposal {
            project: 3,
            proposer: p::Proposer::Party(u64::MAX),
            proposal: 5,
            decision: p::ProposalDecision::Accept,
            running: None,
        },
    )
}
fn set_roles_case(limits: &w::Limits, holdings: List<w::Holding>) -> (w::Ask, people::Ask) {
    use jig_domain_people as p;
    (
        w::Ask::SetRoles(w::SetRoles::new(limits, w::SetRolesParts { project: 3, holdings }).expect("SetRoles")),
        p::Ask::SetRoles {
            project: 3,
            holdings: Box::new([p::Holding { person: 7, role: p::Role::Policy { role: u32::MAX } }]),
        },
    )
}
fn make_service_case(limits: &w::Limits) -> (w::Ask, people::Ask) {
    use jig_domain_people as p;
    (
        w::Ask::MakeService(
            w::MakeService::new(
                limits,
                w::MakeServiceParts { project: 3, name: Box::from(b"service".as_slice()), role: w::HeldRole::Owner },
            )
            .expect("MakeService"),
        ),
        p::Ask::MakeService { project: 3, name: Box::from(b"service".as_slice()), role: p::Role::Owner },
    )
}
fn change_policy_case(limits: &w::Limits) -> (w::Ask, people::Ask) {
    use jig_domain_people as p;
    (
        w::Ask::ChangePolicy(
            w::ChangePolicy::new(
                limits,
                w::ChangePolicyParts {
                    project: 3,
                    edit: w::PolicyEdit::ProjectSpend(
                        w::PolicyEditProjectSpend::new(
                            limits,
                            w::PolicyEditProjectSpendParts { period_spend: u64::MAX },
                        )
                        .expect("spend"),
                    ),
                },
            )
            .expect("ChangePolicy"),
        ),
        p::Ask::ChangePolicy { project: 3, change: p::PolicyChange::ProjectSpend { period_spend: u64::MAX } },
    )
}
fn delete_note_case(limits: &w::Limits) -> (w::Ask, people::Ask) {
    use jig_domain_people as p;
    (
        w::Ask::DeleteNote(
            w::DeleteNote::new(
                limits,
                w::DeleteNoteParts {
                    project: 3,
                    note: 9,
                    scope: w::NoteScope::Task(
                        w::NoteScopeTask::new(limits, w::NoteScopeTaskParts { project: 3, task: 8 }).expect("scope"),
                    ),
                    revision: revision(u64::from(u32::MAX)),
                },
            )
            .expect("DeleteNote"),
        ),
        p::Ask::EditNote {
            project: 3,
            name: 9,
            scope: Box::new(p::NoteScope::Goal { goal: 8 }),
            change: Box::new(p::NoteChange::Delete { recalled: u32::MAX }),
        },
    )
}
fn edit_note_case(limits: &w::Limits, references: List<u64>) -> (w::Ask, people::Ask) {
    use jig_domain_people as p;
    (
        w::Ask::EditNote(
            w::EditNote::new(
                limits,
                w::EditNoteParts {
                    project: 3,
                    note: 9,
                    scope: w::NoteScope::Deployment,
                    revision: revision(6),
                    description: Box::new([]),
                    body: Box::new([]),
                    references,
                },
            )
            .expect("EditNote"),
        ),
        p::Ask::EditNote {
            project: 3,
            name: 9,
            scope: Box::new(p::NoteScope::Deployment),
            change: Box::new(p::NoteChange::Correct {
                description: Box::new([]),
                body: Box::new([]),
                references: Box::new([u64::MAX]),
                recalled: 6,
            }),
        },
    )
}

pub fn authority_ask() -> (w::Ask, people::Ask) {
    let limits = wire();
    let pattern = w::OwnerPattern::new(
        &limits,
        w::OwnerPatternParts {
            segments: List::with_capacity(0),
            last: w::OwnerPatternEnd::Open(
                w::OwnerPatternEndOpen::new(&limits, w::OwnerPatternEndOpenParts { value: Box::new([255, 0]) })
                    .expect("end"),
            ),
        },
    )
    .expect("pattern");
    let grant = w::OwnerGrant::new(&limits, w::OwnerGrantParts { connector: 5, kind: 6, pattern: pattern.clone() })
        .expect("grant");
    let mut writer = Writer::new(usize::try_from(grant.measure()).expect("grant size"));
    grant.encode(&mut writer).expect("grant bytes");
    let bytes = writer.finish();
    let grants = piece(&bytes);
    let resource =
        w::OwnerResourceScope::new(&limits, w::OwnerResourceScopeParts { connector: 7, pattern }).expect("scope");
    let mut writer = Writer::new(usize::try_from(resource.measure()).expect("scope size"));
    resource.encode(&mut writer).expect("scope bytes");
    let bytes = writer.finish();
    let note_resources = piece(&bytes);
    let mut kinds = List::with_capacity(1);
    kinds
        .push(w::OwnerAuthorityExecutor::Role(
            w::OwnerAuthorityExecutorRole::new(&limits, w::OwnerAuthorityExecutorRoleParts { value: u32::MAX })
                .expect("executor"),
        ))
        .expect("room");
    let authority = w::OwnerAuthority::new(
        &limits,
        w::OwnerAuthorityParts {
            models: None,
            tools: u64::MAX,
            grants,
            delegation: w::OwnerDelegation::new(&limits, w::OwnerDelegationParts { kinds, tasks: 9, depth: 10 })
                .expect("delegation"),
            spend: u64::MAX,
            deadline: Some(99),
            notes: 255,
            note_resources,
        },
    )
    .expect("authority");
    let request = w::Ask::ChangePolicy(
        w::ChangePolicy::new(
            &limits,
            w::ChangePolicyParts {
                project: 3,
                edit: w::PolicyEdit::Role(
                    w::PolicyEditRoleValue::new(
                        &limits,
                        w::PolicyEditRoleValueParts {
                            value: w::PolicyEditRole::new(
                                &limits,
                                w::PolicyEditRoleParts {
                                    number: 11,
                                    authority,
                                    period_spend: 12,
                                    requests: 13,
                                    decides: 14,
                                },
                            )
                            .expect("policy role"),
                        },
                    )
                    .expect("role edit"),
                ),
            },
        )
        .expect("policy"),
    );
    (request, expected_authority())
}
fn piece(bytes: &[u8]) -> Box<[u8]> {
    let mut writer = Writer::new(bytes.len().checked_add(4).expect("piece size"));
    skein_codec::piece::write(&mut writer, bytes).expect("piece");
    writer.finish()
}
fn expected_authority() -> people::Ask {
    use jig_domain_people as p;
    p::Ask::ChangePolicy {
        project: 3,
        change: p::PolicyChange::Role(p::PolicyRole {
            number: 11,
            period_spend: 12,
            requests: 13,
            decides: 14,
            authority: p::Authority {
                tools: u64::MAX,
                models: None,
                grants: Box::new([p::Grant {
                    connector: 5,
                    kind: 6,
                    pattern: p::Pattern { segments: Box::new([]), last: p::Last::Open(Box::new([255, 0])) },
                }]),
                delegation: p::Delegation { kinds: Box::new([p::Executor::Role(u32::MAX)]), tasks: 9, depth: 10 },
                spend: u64::MAX,
                deadline: Some(Wall::from_nanos(99)),
                notes: 255,
                note_resources: Box::new([p::ResourceScope {
                    connector: 7,
                    pattern: p::Pattern { segments: Box::new([]), last: p::Last::Open(Box::new([255, 0])) },
                }]),
            },
        }),
    }
}
pub fn malformed_piece() -> w::Ask {
    let limits = wire();
    w::Ask::ChangePolicy(
        w::ChangePolicy::new(
            &limits,
            w::ChangePolicyParts {
                project: 3,
                edit: w::PolicyEdit::Projections(
                    w::PolicyEditProjections::new(
                        &limits,
                        w::PolicyEditProjectionsParts { grants: Box::new([0, 0, 0, 2, 0]) },
                    )
                    .expect("opaque piece"),
                ),
            },
        )
        .expect("policy"),
    )
}

#[path = "fixtures/answers.rs"]
mod answers;
pub use answers::*;
impl Exchange {
    pub fn ask(&mut self) -> (Token, ReplyTo) {
        self.ask_version(19)
    }
    pub fn ask_version(&mut self, version: u32) -> (Token, ReplyTo) {
        let body = keyed(w::Owner::Jig, version, encode_ask(&start(&self.env.limits.wire)));
        let owner = self.send(crate::Message::Request(body));
        self.authenticate();
        let Some(Request::Core(event)) = self.out.pop() else {
            panic!("core ask");
        };
        let core::Event::People(people::Event::Ask { reply_to, .. }) = *event else {
            panic!("expected ask");
        };
        (owner, reply_to)
    }
    pub fn reply(&mut self, to: ReplyTo, reply: people::Reply) {
        self.component.down(&self.env, Event::Reply { to, reply }, &mut self.out);
    }
    pub fn answer(&self, owner: Token) -> w::KeyedAnswer {
        let (status, bytes) = self.response(owner);
        assert_eq!(status, 200);
        w::KeyedAnswer::decode(&self.env.limits.wire, &mut skein_lib::Reader::new(bytes)).expect("wire answer")
    }
    pub fn final_answer(&self, owner: Token) -> w::Final {
        final_record(self.answer(owner), &self.env.limits.wire)
    }
    pub fn application(&mut self) -> (Token, ReplyTo) {
        let family =
            w::Owner::Family(w::Family::new(&self.env.limits.wire, w::FamilyParts { number: 9 }).expect("family"));
        let owner = self.send(crate::Message::Request(keyed(family, 2, Box::from(b"abc".as_slice()))));
        self.authenticate();
        let Some(Request::Application(crate::ToApplication::Request { reply_to, .. })) = self.out.pop() else {
            panic!("application ask");
        };
        (owner, reply_to)
    }
}

#[path = "fixtures/client.rs"]
mod client;
pub use client::client_limits;

#[path = "fixtures/services.rs"]
mod services;
pub use services::*;

#[path = "fixtures/documents.rs"]
mod documents;
pub use documents::*;

#[path = "fixtures/live.rs"]
mod live;
pub use live::*;

#[path = "fixtures/objects.rs"]
mod objects;
pub use objects::*;

#[path = "fixtures/pages.rs"]
mod pages;
pub use pages::*;

#[path = "fixtures/reads.rs"]
mod reads;
pub use reads::*;
#[path = "fixtures/watches.rs"]
mod watches;
pub use watches::*;
#[path = "fixtures/closing.rs"]
mod closing;
pub use closing::*;

pub fn translated(reply: &people::Reply, limits: &w::Limits) -> w::Final {
    let record = crate::translate::answers::answer(reply, &models(), limits).expect("translated answer");
    final_record(record, limits)
}
pub fn final_record(record: w::KeyedAnswer, limits: &w::Limits) -> w::Final {
    match record {
        w::KeyedAnswer::FinalAnswer(value) => {
            assert_eq!(value.owner(), &w::Owner::Jig);
            assert_eq!(value.version(), 19);
            w::Final::decode(limits, &mut skein_lib::Reader::new(value.record())).expect("jig answer")
        }
        other @ (w::KeyedAnswer::NotServed(_)
        | w::KeyedAnswer::Busy
        | w::KeyedAnswer::AnswersFull(_)
        | w::KeyedAnswer::NotReady
        | w::KeyedAnswer::SignedOut) => {
            panic!("expected final, got {other:?}")
        }
    }
}
