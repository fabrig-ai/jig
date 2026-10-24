//! Settings proposal admission on the client's wire (`contract.md`, 3.2).
use jig_client_protocol_wire::v19 as w;
use jig_domain_core::{self as core, settings as s};
use jig_engine_clients_world::{Peer, fixtures};
use skein_lib::Token;
use skein_lib::{List, Writer};

fn send_proposal(project: u32) -> (Peer, Token) {
    let limits = fixtures::limits().wire;
    let mut peer = Peer::started(&fixtures::settings());
    let bearer = fixtures::signed_in(&mut peer);
    let setting = w::Setting::Template(
        w::SettingNumber::new(&limits, w::SettingNumberParts { number: 2 }).expect("template number"),
    );
    let mut items = List::with_capacity(1);
    items
        .push(
            w::SettingItem::new(&limits, w::SettingItemParts { setting, edit: w::SettingEdit::Removed })
                .expect("remove item"),
        )
        .expect("one item");
    let size = items.iter().map(|item| usize::try_from(item.measure()).expect("item size")).sum::<usize>();
    let mut bytes = Writer::new(4 + size);
    bytes.put(&u32::try_from(size).expect("item length").to_be_bytes()).expect("setting item length");
    for item in &items {
        item.encode(&mut bytes).expect("item encoding");
    }
    let items = bytes.finish();
    let settings = w::ProposeSettings::new(&limits, w::ProposeSettingsParts { against: 1, items }).expect("proposal");
    let action = w::ProposeAction::Settings(settings);
    let ask = w::Ask::Propose(
        w::Propose::new(&limits, w::ProposeParts { project, action, reason: b"scope probe".as_slice().into() })
            .expect("propose"),
    );
    let mut bytes = Writer::new(usize::try_from(ask.measure()).expect("ask size"));
    ask.encode(&mut bytes).expect("ask encoding");
    let request = w::Keyed::new(
        &limits,
        w::KeyedParts { key: [1; 16], owner: w::Owner::Jig, version: 19, request: bytes.finish(), value: None },
    )
    .expect("keyed request");
    let mut bytes = Writer::new(usize::try_from(request.measure()).expect("request size"));
    request.encode(&mut bytes).expect("request encoding");
    let exchange = peer.post(b"/jig/v1/request", bytes.finish(), Some(&bearer));
    (peer, exchange)
}

#[test]
fn deployment_settings_items_reach_the_core_whole() {
    let (mut peer, exchange) = send_proposal(0);
    let event = peer.take_core();
    let Some(core::Event::ProposeSettings { change, .. }) = event else {
        panic!("settings event: {event:?}; answer {:?}", peer.answered(exchange))
    };
    assert_eq!(change.against, 1);
    assert_eq!(change.reason.as_ref(), b"scope probe");
    assert_eq!(change.items.as_ref(), &[s::Item { setting: s::Setting::Template(2), edit: s::Edit::Removed }]);
    assert!(peer.answered(exchange).is_none(), "a proposal waits for the core's durable reply");
}

#[test]
fn settings_proposals_are_scoped_to_the_deployment() {
    let (mut peer, exchange) = send_proposal(1);
    assert!(peer.take_core().is_none(), "project scope must not become deployment settings");
    assert_eq!(peer.answered(exchange), Some([].as_slice()));
}

#[test]
fn a_real_pending_review_survives_both_wire_translations() {
    use jig_core_world::{effects::World, reads};
    use jig_domain_people as people;
    use jig_test_domain as root;
    use skein_lib::{Reader, ReplyTo};
    let mut world = World::new(6002, false);
    let owner = world.sign_in.expect("owner sign-in");
    world.send(root::Event::Core(core::Event::People(people::Event::Roles {
        project: 0,
        holdings: Box::new([people::Holding { person: 1, role: people::Role::Owner }]),
    })));
    world.send(root::Event::Core(core::Event::ProposeSettings {
        reply_to: ReplyTo::new(Token::new(700)),
        sign_in: owner,
        key: [7; 16],
        digest: [7; 32],
        change: Box::new(s::Proposed {
            against: 1,
            items: Box::new([s::Item { setting: s::Setting::Template(2), edit: s::Edit::Removed }]),
            reason: b"review the removal".as_slice().into(),
        }),
    }));
    let Some(people::Reply::Outcome(people::Outcome::Proposed { proposal, .. })) = world.people_answers.last() else {
        panic!("settings proposal")
    };
    let proposal = *proposal;
    world.reads = Some(reads::Parties::new(world.store.applied));
    reads::ask(&mut world, 1, core::reads::Read::Proposal { proposer: people::Proposer::Party(1), proposal });
    let answer = reads::latest(&world).clone();
    let limits = fixtures::limits().wire;
    let mut peer = Peer::started(&fixtures::settings());
    let bearer = fixtures::signed_in(&mut peer);
    let proposal = w::ProposalKey::new(
        &limits,
        w::ProposalKeyParts {
            proposer: w::Proposer::Party(
                w::ProposerParty::new(&limits, w::ProposerPartyParts { value: 1 }).expect("proposer"),
            ),
            number: proposal,
        },
    )
    .expect("proposal key");
    let read =
        w::Read::Proposal(w::ReadProposal::new(&limits, w::ReadProposalParts { proposal }).expect("proposal read"));
    let mut bytes = Writer::new(usize::try_from(read.measure()).expect("read size"));
    read.encode(&mut bytes).expect("read bytes");
    let envelope = w::Owned::new(&limits, w::OwnedParts { owner: w::Owner::Jig, version: 19, record: bytes.finish() })
        .expect("owned read");
    let mut bytes = Writer::new(usize::try_from(envelope.measure()).expect("owned size"));
    envelope.encode(&mut bytes).expect("owned bytes");
    let exchange = peer.post(b"/jig/v1/read", bytes.finish(), Some(&bearer));
    let Some(core::Event::Read { reply, .. }) = peer.take_core() else { panic!("core read") };
    peer.answer_read(reply, answer);
    let w::ReadAnswer::Value(value) =
        w::ReadAnswer::decode(&limits, &mut Reader::new(peer.answered(exchange).expect("read answer")))
            .expect("answer record")
    else {
        panic!("read value")
    };
    let link = jig_client_protocol_link::Limits::new(jig_client_world::test::limits().client.domain, 8)
        .expect("receiving limits");
    let read = jig_client_protocol_link::translate::reads::value(value.record(), &link)
        .expect("client decodes the real review");
    let jig_client_domain::contract::ReadValue::OwnerProposal(doc) = read else { panic!("proposal view") };
    let jig_client_domain::contract::OwnerProposalAction::Settings(action) = doc.action else {
        panic!("settings action")
    };
    assert_eq!(action.against, 1);
    assert_eq!(action.made, None);
    let review = action.review.expect("pending review");
    assert_eq!(review.newest, 1);
    let item = review.items.iter().next().expect("one review item");
    assert_eq!(item.edit, jig_client_domain::contract::RevisionItemEdit::Removed);
    assert_eq!(
        item.origin,
        jig_client_domain::contract::SettingOrigin::Default(jig_client_domain::contract::OriginRevision {
            revision: 1
        })
    );
    assert_eq!(item.would_be, jig_client_domain::contract::ReviewValue::None);
}
