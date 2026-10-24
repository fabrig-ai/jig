use jig_client_protocol_wire::v19 as w;
use jig_domain_core as core;
use jig_domain_people as people;
use jig_engine_clients_world::fixtures::{self, Peer};
use skein_lib::{
    Writer,
    base64::{self, Alphabet},
};
#[path = "requests/llm.rs"]
mod llm;
fn bearer(peer: &mut Peer, sign_in: u64, person: u64) -> Box<[u8]> {
    let link = peer.printed_link();
    let exchange = peer.post_link(&link);
    peer.answer_signed_in(sign_in, person);
    peer.answer_kept(sign_in);
    let token = peer.sign_in_answer(exchange);
    let mut out = Writer::new(61);
    out.put(b"Bearer ").expect("prefix");
    base64::encode(Alphabet::Url, token.token().as_ref().expect("native token").expose(), &mut out).expect("bearer");
    out.finish()
}
fn request() -> Box<[u8]> {
    let limits = fixtures::limits().wire;
    let ask = w::Ask::Message(
        w::Message::new(&limits, w::MessageParts { project: 3, task: 8, words: Box::from(b"hello".as_slice()) })
            .expect("ask"),
    );
    let mut out = Writer::new(usize::try_from(ask.measure()).expect("ask size"));
    ask.encode(&mut out).expect("ask bytes");
    let record = w::Keyed::new(
        &limits,
        w::KeyedParts { key: [5; 16], owner: w::Owner::Jig, version: 19, request: out.finish(), value: None },
    )
    .expect("keyed request");
    let mut out = Writer::new(usize::try_from(record.measure()).expect("record size"));
    record.encode(&mut out).expect("request bytes");
    out.finish()
}
#[test]
fn a_native_keyed_request_reaches_the_core_after_its_bearer_is_checked() {
    let mut peer = Peer::started(&fixtures::settings());
    let bearer = fixtures::signed_in(&mut peer);
    peer.post(b"/jig/v1/request", request(), Some(&bearer));
    let Some(core::Event::People(people::Event::Ask { sign_in, key, ask, .. })) = peer.take_core() else {
        panic!("authenticated ask");
    };
    assert_eq!(sign_in, fixtures::FIRST_SIGN_IN);
    assert_eq!(key, [5; 16]);
    assert_eq!(ask, people::Ask::Say { project: 3, task: 8, words: Box::from(b"hello".as_slice()) });
}

#[test]
fn a_native_request_waits_for_the_released_reply_and_takes_its_task() {
    let mut peer = Peer::started(&fixtures::settings());
    let bearer = fixtures::signed_in(&mut peer);
    let socket = peer.post(b"/jig/v1/request", request(), Some(&bearer));
    let Some(core::Event::People(people::Event::Ask { reply_to, .. })) = peer.take_core() else {
        panic!("ask");
    };
    assert_eq!(peer.answered(socket), None);
    peer.answer_request(reply_to, people::Reply::Outcome(people::Outcome::Said { task: 8, message: 9 }));
    let limits = fixtures::limits().wire;
    let record = w::KeyedAnswer::decode(&limits, &mut skein_lib::Reader::new(peer.answered(socket).expect("answer")))
        .expect("wire answer");
    let w::KeyedAnswer::FinalAnswer(answer) = record else {
        panic!("final answer");
    };
    assert_eq!(answer.owner(), &w::Owner::Jig);
    assert_eq!(answer.version(), 19);
    let outcome = w::Outcome::Owner(
        w::OutcomeOwner::new(
            &limits,
            w::OutcomeOwnerParts {
                value: w::OwnerOutcome::Said(
                    w::OwnerOutcomeSaid::new(&limits, w::OwnerOutcomeSaidParts { task: 8, message: 9 }).expect("said"),
                ),
            },
        )
        .expect("owner"),
    );
    assert_eq!(
        w::Final::decode(&limits, &mut skein_lib::Reader::new(answer.record())),
        Ok(w::Final::Done(w::Done::new(&limits, w::DoneParts { outcome }).expect("done")))
    );
}

fn owner_authority(spend: u64) -> w::OwnerAuthority {
    let limits = fixtures::limits().wire;
    w::OwnerAuthority::new(
        &limits,
        w::OwnerAuthorityParts {
            models: None,
            tools: 0,
            grants: Box::new([]),
            spend,
            deadline: None,
            notes: 0,
            note_resources: Box::new([]),
            delegation: w::OwnerDelegation::new(
                &limits,
                w::OwnerDelegationParts { kinds: skein_lib::List::with_capacity(0), tasks: 0, depth: 0 },
            )
            .expect("delegation"),
        },
    )
    .expect("authority")
}
fn amend_request(revision: u64, spend: u64) -> Box<[u8]> {
    let limits = fixtures::limits().wire;
    let authority = owner_authority(spend);
    let ask = w::Ask::Amend(Box::new(
        w::Amend::new(
            &limits,
            w::AmendParts {
                project: 1,
                task: 2,
                revision: Some(w::Revision::new(&limits, w::RevisionParts { value: revision }).expect("revision")),
                change: w::Amendment::new(
                    &limits,
                    w::AmendmentParts {
                        llm: None,
                        tracked: None,
                        addressed: None,
                        spec: None,
                        wake: None,
                        dependencies: None,
                        authority: Some(authority),
                        instructions: None,
                    },
                )
                .expect("change"),
                reason: b"more work".as_slice().into(),
            },
        )
        .expect("amend"),
    ));
    keyed([61; 16], &ask)
}
/// A keyed request carrying `ask` under `key`.
fn keyed(key: [u8; 16], ask: &w::Ask) -> Box<[u8]> {
    let limits = fixtures::limits().wire;
    let mut out = Writer::new(usize::try_from(ask.measure()).expect("ask size"));
    ask.encode(&mut out).expect("ask bytes");
    let keyed = w::Keyed::new(
        &limits,
        w::KeyedParts { key, owner: w::Owner::Jig, version: 19, request: out.finish(), value: None },
    )
    .expect("keyed");
    let mut out = Writer::new(usize::try_from(keyed.measure()).expect("keyed size"));
    keyed.encode(&mut out).expect("keyed bytes");
    out.finish()
}
fn route_real_core(peer: &mut Peer, world: &mut jig_core_world::effects::World) -> Vec<[u8; 16]> {
    let mut keys = Vec::new();
    while let Some(event) = peer.take_core() {
        let core::Event::People(people::Event::Ask { models, reply_to, sign_in, key, ask }) = event else {
            panic!("authenticated ask")
        };
        let token = reply_to.into_token();
        keys.push(key);
        world.send(jig_test_domain::Event::Core(core::Event::People(people::Event::Ask {
            models,
            reply_to: skein_lib::ReplyTo::new(token),
            sign_in,
            key,
            ask,
        })));
        peer.answer_request(
            skein_lib::ReplyTo::new(token),
            world.people_answers.last().expect("real core reply").clone(),
        );
    }
    keys
}
fn final_answer(peer: &Peer, socket: skein_lib::Token) -> w::Final {
    let limits = fixtures::limits().wire;
    let w::KeyedAnswer::FinalAnswer(answer) =
        w::KeyedAnswer::decode(&limits, &mut skein_lib::Reader::new(peer.answered(socket).expect("http answer")))
            .expect("answer")
    else {
        panic!("final")
    };
    w::Final::decode(&limits, &mut skein_lib::Reader::new(answer.record())).expect("owner answer")
}
/// A member signed in through the peer, its asks routed to the real core.
fn member_setup(seed: u64) -> (Peer, jig_core_world::effects::World, Box<[u8]>) {
    let world = jig_core_world::stories::offers::world(seed, Some(people::Role::Member));
    let mut peer = Peer::started(&real_core_settings());
    let bearer = bearer(&mut peer, world.sign_in.expect("member"), 1);
    (peer, world, bearer)
}
/// A member signed in through the peer, with task 2 created in project 1.
fn amendment_setup(seed: u64) -> (Peer, jig_core_world::effects::World, Box<[u8]>) {
    let (peer, mut world, bearer) = member_setup(seed);
    let sign_in = world.sign_in.expect("member");
    jig_core_world::fixtures::ask_as(
        &mut world,
        sign_in,
        CREATE_TASK,
        people::Ask::Create { project: 1, batch: Box::new([jig_core_world::fixtures::people_member(b"work", 20)]) },
    );
    (peer, world, bearer)
}
/// A peer's settings naming the core world's configured models, as configuration fills both.
fn real_core_settings() -> jig_protocol_engine_clients::Settings {
    let model = jig_core_world::world::scripted_model();
    jig_protocol_engine_clients::Settings {
        models: Box::new([jig_protocol_engine_clients::Model {
            name: model.name,
            provider: None,
            window: model.window,
            output: model.output,
            efforts: model.efforts,
            prices: jig_protocol_engine_clients::Prices {
                input: model.input_price,
                cached: model.cached_price,
                cache_write: model.cache_write_price,
                output: model.output_price,
                unit: model.price_unit,
            },
            description: None,
        }]),
        ..fixtures::settings()
    }
}

#[test]
fn a_v9_propose_beyond_the_role_is_answered_proposed() {
    let (mut peer, mut world, bearer) = amendment_setup(349);
    let revision = jig_core_world::fixtures::live_row(&world, 2).revisions.spec;
    let socket = peer.post(b"/jig/v1/request", propose_request(revision, 150), Some(&bearer));
    assert_eq!(
        route_real_core(&mut peer, &mut world),
        [[61; 16]],
        "real core reply: {:?}",
        world.people_answers.last()
    );
    let w::Final::Done(done) = final_answer(&peer, socket) else { panic!("done") };
    let w::Outcome::Owner(owner) = done.outcome() else { panic!("owner outcome") };
    let w::OwnerOutcome::Proposed(row) = owner.value() else { panic!("proposed") };
    assert_eq!(row.proposal(), 1);
    assert_eq!(jig_core_world::fixtures::live_row(&world, 2).numbers.budget, 20);
}
#[test]
fn a_v9_propose_sent_again_replays_the_same_proposal() {
    let (mut peer, mut world, bearer) = amendment_setup(350);
    let revision = jig_core_world::fixtures::live_row(&world, 2).revisions.spec;
    let body = propose_request(revision, 150);
    let first = peer.post(b"/jig/v1/request", body.clone(), Some(&bearer));
    assert_eq!(
        route_real_core(&mut peer, &mut world),
        [[61; 16]],
        "real core reply: {:?}",
        world.people_answers.last()
    );
    let original = final_answer(&peer, first);
    let second = peer.post(b"/jig/v1/request", body, Some(&bearer));
    assert_eq!(
        route_real_core(&mut peer, &mut world),
        [[61; 16]],
        "real core reply: {:?}",
        world.people_answers.last()
    );
    assert_eq!(final_answer(&peer, second), original);
}
#[test]
fn a_v9_amend_beyond_the_ceiling_stays_refused() {
    let (mut peer, mut world, bearer) = amendment_setup(351);
    let revision = jig_core_world::fixtures::live_row(&world, 2).revisions.spec;
    let socket = peer.post(b"/jig/v1/request", amend_request(revision, 2000), Some(&bearer));
    assert_eq!(route_real_core(&mut peer, &mut world), [[61; 16]]);
    let w::Final::Refused(refused) = final_answer(&peer, socket) else { panic!("refused") };
    let w::Refusal::Authority(row) = refused.refusal() else { panic!("authority refusal: {:?}", refused.refusal()) };
    assert_eq!(row.proposable(), &Some(false));
    assert_eq!(jig_core_world::fixtures::live_row(&world, 2).proposal, None);
}

fn propose_request(revision: u64, spend: u64) -> Box<[u8]> {
    let limits = fixtures::limits().wire;
    let keyed = w::Keyed::decode(&limits, &mut skein_lib::Reader::new(&amend_request(revision, spend)))
        .expect("bounded proposal fixture")
        .into_parts();
    let w::Ask::Amend(value) =
        w::Ask::decode(&limits, &mut skein_lib::Reader::new(&keyed.request)).expect("bounded proposal fixture")
    else {
        panic!("amend fixture")
    };
    let v = value.into_parts();
    let ask = w::Ask::Propose(
        w::Propose::new(
            &limits,
            w::ProposeParts {
                project: v.project,
                action: w::ProposeAction::Amendment(Box::new(
                    w::ProposeAmendment::new(
                        &limits,
                        w::ProposeAmendmentParts {
                            task: v.task,
                            revision: v.revision.expect("amendment revision"),
                            change: v.change,
                        },
                    )
                    .expect("bounded proposal fixture"),
                )),
                reason: v.reason,
            },
        )
        .expect("bounded proposal fixture"),
    );
    let mut out = Writer::new(usize::try_from(ask.measure()).expect("ask size"));
    ask.encode(&mut out).expect("bounded proposal fixture");
    let keyed =
        w::Keyed::new(&limits, w::KeyedParts { request: out.finish(), ..keyed }).expect("bounded proposal fixture");
    let mut out = Writer::new(usize::try_from(keyed.measure()).expect("keyed size"));
    keyed.encode(&mut out).expect("bounded proposal fixture");
    out.finish()
}

const CREATE_TASK: u8 = 30;

fn carrying_request(value: &[u8]) -> Box<[u8]> {
    let limits = fixtures::limits().wire;
    let request =
        w::FamilyRequest::new(&limits, w::FamilyRequestParts { scope: w::Scope::Own, record: Box::new([255]) })
            .expect("family request");
    let mut out = Writer::new(usize::try_from(request.measure()).expect("request size"));
    request.encode(&mut out).expect("family request bytes");
    let owner = w::Owner::Family(w::Family::new(&limits, w::FamilyParts { number: 1 }).expect("family"));
    let keyed = w::Keyed::new(
        &limits,
        w::KeyedParts {
            key: [6; 16],
            owner,
            version: 1,
            request: out.finish(),
            value: Some(skein_lib::Secret::new(Box::from(value))),
        },
    )
    .expect("carrying request");
    let mut out = Writer::new(usize::try_from(keyed.measure()).expect("keyed size"));
    keyed.encode(&mut out).expect("carrying bytes");
    out.finish()
}
#[test]
fn a_carried_value_is_kept_before_the_application_sees_its_request() {
    use jig_protocol_engine_clients::{Event, FromApplication, Served, ToApplication};
    let mut settings = fixtures::settings();
    settings.served = skein_lib::List::with_capacity(2);
    settings.served.push(Served { family: None, first: 19, last: 19 }).expect("jig");
    settings.served.push(Served { family: Some(1), first: 1, last: 1 }).expect("family");
    let mut peer = Peer::started(&settings);
    let bearer = fixtures::signed_in(&mut peer);
    let socket = peer.post(b"/jig/v1/request", carrying_request(b"a carried key"), Some(&bearer));
    assert_eq!(peer.carry_asked().map(skein_lib::Secret::expose), Some(b"a carried key".as_slice()));
    assert!(peer.take_application().is_none(), "the application waits for the kept value");
    let number = peer.keep_carried();
    let Some(ToApplication::Request { reply_to, record, carried, .. }) = peer.take_application() else {
        panic!("application request")
    };
    assert_eq!(record.as_ref(), [255]);
    assert_eq!(carried, Some(core::Carried { number, digest: skein_lib::sha256::digest(b"a carried key") }));
    peer.event(Event::Application(FromApplication::Answered { to: reply_to, family: 1, record: Box::new([7]) }));
    let limits = fixtures::limits().wire;
    let Ok(w::KeyedAnswer::FinalAnswer(answer)) =
        w::KeyedAnswer::decode(&limits, &mut skein_lib::Reader::new(peer.answered(socket).expect("http answer")))
    else {
        panic!("final answer")
    };
    assert_eq!(answer.record(), [7]);
    assert_eq!(peer.carried(number).map(skein_lib::Secret::expose), Some(b"a carried key".as_slice()), "used, so kept");
}

#[test]
fn settings_proposals_and_running_choices_reach_the_core() {
    let limits = fixtures::limits().wire;
    let propose = w::Ask::Propose(
        w::Propose::new(
            &limits,
            w::ProposeParts {
                project: 0,
                reason: Box::from(b"settings".as_slice()),
                action: w::ProposeAction::Settings(
                    w::ProposeSettings::new(&limits, w::ProposeSettingsParts { against: 6, items: Box::new([]) })
                        .expect("settings proposal"),
                ),
            },
        )
        .expect("propose"),
    );
    let proposal = w::ProposalKey::new(
        &limits,
        w::ProposalKeyParts {
            proposer: w::Proposer::Party(
                w::ProposerParty::new(&limits, w::ProposerPartyParts { value: 3 }).expect("party"),
            ),
            number: 7,
        },
    )
    .expect("proposal key");
    let mut asks = vec![propose];
    for running in [w::Running::Stays, w::Running::Moves] {
        asks.push(w::Ask::Decide(
            w::Decide::new(
                &limits,
                w::DecideParts {
                    project: 0,
                    on: w::DecideOn::Proposal(
                        w::DecideProposal::new(
                            &limits,
                            w::DecideProposalParts {
                                proposal: proposal.clone(),
                                decision: w::Decision::Accepted,
                                running: Some(running),
                            },
                        )
                        .expect("proposal decision"),
                    ),
                },
            )
            .expect("decide"),
        ));
    }
    let mut peer = Peer::started(&fixtures::settings());
    let bearer = fixtures::signed_in(&mut peer);
    for ask in asks {
        let socket = peer.post(b"/jig/v1/request", keyed([71; 16], &ask), Some(&bearer));
        let event = peer.take_core().expect("settings requests reach the core");
        let reply_to = if let core::Event::ProposeSettings { reply_to, .. } = event {
            reply_to
        } else {
            let core::Event::People(people::Event::Ask { reply_to, .. }) = event else { panic!("settings decision") };
            reply_to
        };
        assert!(peer.answered(socket).is_none(), "the answer waits for the core decision");
        peer.answer_request(reply_to, people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Unknown)));
        assert!(peer.answered(socket).is_some(), "the released answer closes this exchange");
    }
}
