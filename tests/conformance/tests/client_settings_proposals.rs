//! The settings proposal vocabulary meets at wire 19; task E supplies its domain producers.
use jig_client_domain::contract as c;
use jig_client_protocol_link::{Limits, translate};
use jig_client_protocol_wire::v19 as w;
use jig_client_test_records::settings_proposals as values;
use skein_lib::{Reader, Writer};
macro_rules! encoded {
    ($value:expr) => {{
        let value = $value;
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        out.finish()
    }};
}
fn limits() -> Limits {
    let mut client = jig_client_world::test::limits().client.domain;
    client.rows = 8;
    Limits::new(client, 4).expect("client limits")
}
fn submitted(
    ask: &w::Ask,
    limits: &Limits,
) -> (jig_engine_clients_world::Peer, skein_lib::Token, jig_domain_core::Event) {
    let mut peer = jig_engine_clients_world::Peer::started(&jig_engine_clients_world::fixtures::settings());
    let bearer = jig_engine_clients_world::fixtures::signed_in(&mut peer);
    let keyed = w::Keyed::new(
        &limits.wire,
        w::KeyedParts { key: [7; 16], owner: w::Owner::Jig, version: 19, request: encoded!(ask), value: None },
    )
    .expect("keyed request");
    let socket = peer.post(b"/jig/v1/request", encoded!(&keyed), Some(&bearer));
    let event = peer.take_core().expect("authenticated core request");
    (peer, socket, event)
}
#[test]
fn settings_proposals_and_running_decisions_keep_the_client_contract() {
    let limits = limits();
    let mut items = skein_lib::List::with_capacity(1);
    items
        .push(c::SettingItem {
            setting: c::Setting::Template(c::SettingNumber { number: 1 }),
            edit: c::SettingEdit::Removed,
        })
        .expect("one removal");
    let proposal = c::ProposeSettings { against: 6, items };
    let ask = c::Ask::Propose {
        project: 0,
        action: c::ProposeAction::Settings(proposal.clone()),
        reason: c::Text::new(Box::from(b"review".as_slice()), limits.client.text).expect("reason"),
    };
    let wire_ask = translate::asks::ask(&ask, &limits.wire).expect("ask");
    let (_, _, engine) = submitted(&wire_ask, &limits);
    assert!(matches!(
        engine,
        jig_domain_core::Event::ProposeSettings { change, .. } if change.against == 6 && change.items.len() == 1
    ));
    let w::Ask::Propose(wire) = wire_ask else { panic!("proposal") };
    assert_eq!(wire.project(), 0);
    let w::ProposeAction::Settings(wire) = wire.action() else { panic!("settings") };
    assert_eq!(translate::settings_proposals::restore_propose_settings(wire, &limits), Ok(proposal));
    for running in [c::Running::Stays, c::Running::Moves] {
        let ask = c::Ask::Decide {
            project: 0,
            on: c::DecideOn::Proposal {
                proposal: c::ProposalKey { proposer: c::Proposer::Party(3), number: 7 },
                decision: c::Decision::Accepted,
                running: Some(running.clone()),
            },
        };
        let wire_ask = translate::asks::ask(&ask, &limits.wire).expect("decision");
        let (_, _, engine) = submitted(&wire_ask, &limits);
        let expected = match running {
            c::Running::Stays => jig_domain_people::Running::Stays,
            c::Running::Moves => jig_domain_people::Running::Moves,
        };
        assert!(
            matches!(engine, jig_domain_core::Event::People(jig_domain_people::Event::Ask { ask: jig_domain_people::Ask::DecideProposal { running: Some(value), .. }, .. }) if value == expected)
        );
        let w::Ask::Decide(wire) = wire_ask else { panic!("decide") };
        let w::DecideOn::Proposal(wire) = wire.on() else { panic!("proposal decision") };
        assert_eq!(
            translate::settings::restore_running(wire.running().as_ref().expect("running choice"), &limits),
            Ok(running)
        );
    }
}
#[test]
fn pending_reviews_keep_origins_values_sizes_and_the_cut_fact() {
    let limits = limits();
    let pending = values::pending();
    let wire = translate::settings_proposals::wire_owner_proposal_action_settings(&pending, &limits.wire)
        .expect("settings action");
    let mut parts = w::OwnerProposal::decode(
        &limits.wire,
        &mut Reader::new(include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/record_owner_proposal_smallest.bin"
        )),
    )
    .expect("proposal envelope")
    .into_parts();
    parts.action = w::OwnerProposalAction::Settings(Box::new(wire));
    parts.waiting = w::OwnerProposalWaiting::new(
        &limits.wire,
        w::OwnerProposalWaitingParts {
            holder: w::OwnerHistoryHolder::Policy(
                w::OwnerHistoryHolderPolicy::new(
                    &limits.wire,
                    w::OwnerHistoryHolderPolicyParts { project: 0, kind: w::OwnerMessageProposalKind::Settings },
                )
                .expect("settings holder"),
            ),
            since: 0,
        },
    )
    .expect("waiting");

    let wire = w::ReadValue::OwnerProposal(w::OwnerProposal::new(&limits.wire, parts).expect("proposal"));
    let c::ReadValue::OwnerProposal(client) =
        translate::reads::value(&encoded!(&wire), &limits).expect("client proposal")
    else {
        panic!("owner proposal")
    };
    assert_eq!(client.action, c::OwnerProposalAction::Settings(Box::new(pending)));
    assert_eq!(
        client.waiting.holder,
        c::OwnerHistoryHolder::Policy { project: 0, kind: c::OwnerMessageProposalKind::Settings }
    );
    let review = values::review();
    let c::ReviewValue::TooLarge(size) = &review.items.get(0).expect("first review item").would_be else {
        panic!("oversized")
    };
    assert_eq!(size.bytes, values::OVERSIZED_VALUE);
}
#[test]
fn retained_settings_actions_keep_the_revision_they_made() {
    let limits = limits();
    let retained = values::retained();
    let wire = translate::settings_proposals::wire_owner_retained_action_settings(&retained, &limits.wire)
        .expect("retained settings");
    let mut proposal = w::OwnerProposalHistory::decode(
        &limits.wire,
        &mut Reader::new(include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/record_owner_proposal_history_smallest.bin"
        )),
    )
    .expect("history envelope")
    .into_parts();
    proposal.action = w::OwnerRetainedAction::Settings(wire);
    let key = w::ProposalKey::new(
        &limits.wire,
        w::ProposalKeyParts {
            proposer: w::Proposer::Task(
                w::ProposerTask::new(&limits.wire, w::ProposerTaskParts { value: proposal.proposer })
                    .expect("task proposer"),
            ),
            number: proposal.number,
        },
    )
    .expect("history key");
    proposal.state = w::OwnerProposalHistoryState::Accepted(
        w::OwnerProposalHistoryStateAccepted::new(
            &limits.wire,
            w::OwnerProposalHistoryStateAcceptedParts {
                by: w::OwnerRequester::Person(
                    w::OwnerRequesterPerson::new(&limits.wire, w::OwnerRequesterPersonParts { value: 3 })
                        .expect("accepting party"),
                ),
            },
        )
        .expect("accepted"),
    );

    let value = w::OwnerHistoricalValue::Proposal(
        w::OwnerHistoricalValueProposal::new(
            &limits.wire,
            w::OwnerHistoricalValueProposalParts {
                value: w::OwnerProposalHistory::new(&limits.wire, proposal).expect("history"),
            },
        )
        .expect("historical proposal"),
    );
    let mut historical = w::OwnerHistorical::decode(
        &limits.wire,
        &mut Reader::new(include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/record_owner_historical_smallest.bin"
        )),
    )
    .expect("historical envelope")
    .into_parts();
    historical.key = w::ObjectKey::Proposal(
        w::ObjectKeyProposal::new(&limits.wire, w::ObjectKeyProposalParts { value: key }).expect("proposal subject"),
    );
    historical.value = value;
    let wire =
        w::ReadValue::Historical(Box::new(w::OwnerHistorical::new(&limits.wire, historical).expect("historical")));
    let c::ReadValue::Historical(client) = translate::reads::value(&encoded!(&wire), &limits).expect("client history")
    else {
        panic!("historical")
    };
    let c::OwnerHistoricalValue::Proposal(proposal) = client.value else { panic!("retained proposal") };
    assert_eq!(proposal.action, c::OwnerRetainedAction::Settings(retained));
}

#[test]
fn a_running_choice_refusal_keeps_its_reason() {
    let limits = limits();
    let refusal = w::Refusal::Malformed(
        w::RefusalMalformed::new(&limits.wire, w::RefusalMalformedParts { at: 0, why: w::Malformed::Running })
            .expect("malformed running"),
    );
    let answer =
        w::Final::Refused(w::Refused::new(&limits.wire, w::RefusedParts { refusal, at: None }).expect("refused"));
    let owned =
        w::Owned::new(&limits.wire, w::OwnedParts { owner: w::Owner::Jig, version: 19, record: encoded!(&answer) })
            .expect("owned refusal");
    assert_eq!(
        translate::answers::answer(w::KeyedAnswer::FinalAnswer(owned), &limits),
        Ok(c::Answer::Refused { refusal: c::Refusal::Malformed { at: 0, why: c::Malformed::Running }, at: None })
    );
}

fn engine_proposal() -> jig_domain_core::documents::Proposal {
    use jig_domain_core::{documents as d, reads, settings as s};
    use jig_domain_tasks as t;
    let template = jig_domain_core::Template {
        number: 1,
        name: b"chat".as_slice().into(),
        executor: jig_domain_people::TemplateExecutor::Chosen,
        contract: t::Contract::Report { words: 1 },
        shallower: 0,
        budget: jig_domain_people::TemplateBudget::Given,
        deadline: jig_domain_people::TemplateDeadline::None,
        tracked: None,
        flags: t::Flags::PLAIN,
        label: 0,
        grace: None,
        llm: None,
    };
    let mut next = template.clone();
    next.grace = Some(skein_lib::Duration::from_secs(4));
    let mut items = skein_lib::List::with_capacity(1);
    items
        .push(d::ReviewItem {
            setting: s::Setting::Template(1),
            edit: reads::RevisionEdit::Set,
            now: Some(s::Value::Template(Box::new(template))),
            origin: Some((false, 1)),
            would_be: Some(s::Value::Template(Box::new(next))),
        })
        .expect("item");
    let mut heads = skein_lib::List::with_capacity(1);
    heads
        .push(reads::RevisionItemHead { setting: s::Setting::Template(1), edit: reads::RevisionEdit::Set })
        .expect("head");
    d::Proposal {
        proposer: jig_domain_people::Proposer::Party(9),
        number: 7,
        project: 0,
        reason: d::Text { bytes: b"review".as_slice().into(), cut: false },
        action: d::ProposalAction::Settings {
            against: 1,
            items: heads,
            review: Some(Box::new(d::SettingsReview { newest: 1, items, value_bytes: 65_536 })),
            made: None,
        },
        as_holder: false,
        waiting: d::ProposalWaiting {
            holder: t::ProposalHolder::Policy { project: 0, kind: t::ProposalKind::Settings },
            since: skein_lib::Wall::EPOCH,
        },
        funding: skein_lib::List::with_capacity(0),
        acts: skein_lib::List::with_capacity(0),
    }
}
#[test]
fn engine_pending_settings_review_reaches_the_client_and_encodes_back_without_loss() {
    let limits = limits();
    let source = engine_proposal();
    let wire =
        jig_protocol_engine_clients::translate::objects::proposal(&[], &source, &limits.wire).expect("engine document");
    let c::ReadValue::OwnerProposal(client) =
        translate::reads::value(&encoded!(&w::ReadValue::OwnerProposal(wire.clone())), &limits).expect("client review")
    else {
        panic!("proposal document")
    };
    let c::OwnerProposalAction::Settings(action) = client.action else { panic!("settings action") };
    assert_eq!((action.against, action.made), (1, None));
    let review = action.review.as_ref().expect("pending review");
    assert_eq!(review.newest, 1);
    let item = review.items.get(0).expect("one item");
    assert_eq!(item.origin, c::SettingOrigin::Default(c::OriginRevision { revision: 1 }));
    let c::ReviewValue::Value(value) = &item.would_be else { panic!("proposed value") };
    let c::SettingValue::Template(template) = &value.value else { panic!("template") };
    assert_eq!(template.grace, Some(skein_lib::Duration::from_secs(4)));
    let w::OwnerProposalAction::Settings(original) = wire.action() else { panic!("settings wire action") };
    assert_eq!(
        translate::settings_proposals::wire_owner_proposal_action_settings(&action, &limits.wire)
            .expect("client encoder"),
        **original
    );
}
#[test]
fn engine_accepted_party_settings_history_keeps_its_party_key_and_revision() {
    use jig_domain_core::reads;
    use jig_domain_tasks as t;
    let limits = limits();
    let proposal = t::PersonProposal {
        number: 7,
        proposer: 9,
        project: 0,
        reason: b"review".as_slice().into(),
        action: t::PersonAction::Settings {
            against: 1,
            items: Box::new([t::SettingHead { setting: t::Setting::Template(1), edit: t::SettingEdit::Set }]),
        },
        state: t::PersonProposalState::SettingsAccepted { by: t::Party::Person(10), revision: 2 },
    };
    let historical = reads::Historical {
        value: reads::Past::PersonProposal { proposal: Box::new(proposal), decision: None },
        at: Some(skein_lib::Wall::EPOCH),
        by: Some(t::Party::Person(10)),
    };
    let wire = jig_protocol_engine_clients::translate::objects::historical(&historical, &[], &limits.wire)
        .expect("engine history");
    let c::ReadValue::Historical(client) =
        translate::reads::value(&encoded!(&w::ReadValue::Historical(Box::new(wire))), &limits).expect("client history")
    else {
        panic!("historical proposal")
    };
    let key = c::ObjectKey::Proposal(c::ProposalKey { proposer: c::Proposer::Party(9), number: 7 });
    assert!(client.matches(key));
    let c::OwnerHistoricalValue::Proposal(proposal) = client.value else { panic!("retained action history") };
    let c::OwnerRetainedAction::Settings(action) = proposal.action else { panic!("settings heads") };
    assert_eq!((action.against, action.made), (1, Some(2)));
    assert_eq!(action.items.get(0).expect("head").setting, c::Setting::Template(c::SettingNumber { number: 1 }));
}
#[test]
fn engine_running_and_pending_settings_limit_refusals_keep_their_client_names() {
    use jig_domain_people as p;
    let limits = limits();
    for (source, expected) in [
        (
            p::Refusal::NotWellFormed { at: 0, why: p::Malformed::Running },
            c::Refusal::Malformed { at: 0, why: c::Malformed::Running },
        ),
        (
            p::Refusal::Limit { which: p::LimitKind::SettingsProposals },
            c::Refusal::Limit { which: c::Limit::Admission { which: c::AdmissionLimit::SettingsProposals } },
        ),
    ] {
        let wire = w::Ask::Message(
            w::Message::new(&limits.wire, w::MessageParts { project: 1, task: 1, words: Box::new([]) })
                .expect("message"),
        );
        let (mut peer, socket, event) = submitted(&wire, &limits);
        let jig_domain_core::Event::People(p::Event::Ask { reply_to, .. }) = event else { panic!("people ask") };
        peer.answer_request(reply_to, p::Reply::Outcome(p::Outcome::Refused(source)));
        let answer = w::KeyedAnswer::decode(&limits.wire, &mut Reader::new(peer.answered(socket).expect("answer")))
            .expect("keyed answer");
        assert_eq!(translate::answers::answer(answer, &limits), Ok(c::Answer::Refused { refusal: expected, at: None }));
    }
}
