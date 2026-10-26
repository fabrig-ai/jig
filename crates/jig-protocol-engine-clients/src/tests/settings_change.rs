//! Settings cross the clients' component as one value and one keyed digest.
use super::fixtures::{self as f, Exchange};
use crate::{Request, translate};
use jig_client_protocol_wire::v19 as w;
use jig_domain_core::{self as core, settings as s};
use jig_domain_people as p;
use skein_lib::List;

fn ask() -> w::Ask {
    w::Ask::ChangeSettings(
        jig_client_protocol_link::translate::settings::wire_change_settings(
            &jig_client_test_records::settings::settings_change(),
            &f::wire(),
        )
        .expect("settings fixture"),
    )
}
#[test]
fn all_four_whole_values_translate_and_the_component_passes_the_exact_asks_digest() {
    let mut env = f::env();
    env.limits.message_bytes = 8192;
    env.limits.wire.keyed_request = 8192;
    let mut exchange = Exchange::with_env(env);
    exchange.component.settings.models[0].name = Box::from(b"model".as_slice());
    let wire = ask();
    let encoded = f::encode_ask(&wire);
    let expected_digest = crate::sign_in::digest(&encoded).expose().try_into().expect("settings fixture");
    let expected = p::Ask::ChangeSettings { against: 5, digest: expected_digest };
    assert_eq!(translate::asks::ask(wire, &exchange.component.settings.models, &f::wire()), Ok(expected));
    let keyed = w::Keyed::new(
        &exchange.env.limits.wire,
        w::KeyedParts { key: [5; 16], owner: w::Owner::Jig, version: 19, request: encoded, value: None },
    )
    .expect("settings envelope");
    let mut writer = skein_lib::Writer::new(usize::try_from(keyed.measure()).expect("settings measure"));
    keyed.encode(&mut writer).expect("settings envelope");
    exchange.send(crate::Message::Request(writer.finish()));
    exchange.authenticate();
    let Some(Request::Core(event)) = exchange.out.pop() else { panic!("core entrance") };
    let core::Event::ChangeSettings { sign_in, key, digest, change, .. } = *event else { panic!("settings entrance") };
    assert_eq!((sign_in, key, digest), (41, [5; 16], expected_digest));
    assert_eq!(change.against, 5);
    assert_eq!(change.running, s::Running::Moves);
    assert_eq!(change.items.len(), 4);
    let s::Edit::Set(s::Value::Charter(charter)) = &change.items[0].edit else { panic!("charter") };
    assert_eq!(change.items[0].setting, s::Setting::Charter(2));
    assert_eq!(charter.policy.instructions.as_ref(), "instructions λ".as_bytes());
    assert_eq!(charter.policy.model.effort.as_deref(), Some(b"high".as_slice()));
    assert_eq!(charter.policy.model.cache_write_price, Some(5));
    assert_eq!(charter.policy.alternatives[0].name.as_ref(), b"other");
    let s::Edit::Set(s::Value::Template(template)) = &change.items[1].edit else { panic!("template") };
    assert_eq!(template.grace, Some(skein_lib::Duration::from_secs(18)));
    assert_eq!(
        template.llm.as_ref().expect("settings fixture").main,
        jig_domain_tasks::ModelEffort { model: 0, effort: Some(1) }
    );
    assert_eq!(template.llm.as_ref().expect("settings fixture").account, Some(21));
    assert!(template.flags.conversation);
    let s::Edit::Set(s::Value::Policy(policy)) = &change.items[2].edit else { panic!("policy") };
    assert_eq!(policy.policy.period_spend, 101);
    assert_eq!(policy.policy.requirements[0].judge.parameters, 6);
    assert_eq!(policy.permissions[0].permission, 11);
    assert_eq!(policy.templates[0].priority, Some(5));
    let s::Edit::Set(s::Value::Rules(rules)) = &change.items[3].edit else { panic!("rules") };
    assert_eq!(rules.implies.entries()[0].implies, 3);
    assert_eq!(rules.maximum_run_spend, 90);
    assert!(exchange.out.is_empty());
}
#[test]
fn resets_and_removals_translate_and_bad_pieces_reach_no_core() {
    let limits = f::wire();
    let mut rows = List::with_capacity(2);
    for edit in [w::SettingEdit::Reset, w::SettingEdit::Removed] {
        let item = w::SettingItem::new(&limits, w::SettingItemParts { setting: w::Setting::Rules, edit })
            .expect("settings fixture");
        let mut writer = skein_lib::Writer::new(usize::try_from(item.measure()).expect("settings fixture"));
        item.encode(&mut writer).expect("settings fixture");
        rows.push(writer.finish()).expect("settings fixture");
    }
    let request = w::ChangeSettings::new(
        &limits,
        w::ChangeSettingsParts { against: 3, items: f::pieces(rows.as_slice()), running: w::Running::Stays },
    )
    .expect("settings fixture");
    let change = translate::settings::change(request, &f::models(), &limits).expect("settings fixture");
    assert_eq!(
        change.items.as_ref(),
        &[
            s::Item { setting: s::Setting::Rules, edit: s::Edit::Reset },
            s::Item { setting: s::Setting::Rules, edit: s::Edit::Removed }
        ]
    );
    for items in [Box::<[u8]>::from([0]), f::pieces(&[Box::new([255])])] {
        let wire = w::Ask::ChangeSettings(
            w::ChangeSettings::new(&limits, w::ChangeSettingsParts { against: 1, items, running: w::Running::Stays })
                .expect("settings fixture"),
        );
        let mut exchange = Exchange::new();
        let owner = exchange.send(crate::Message::Request(f::keyed(w::Owner::Jig, 19, f::encode_ask(&wire))));
        exchange.authenticate();
        assert!(exchange.out.is_empty());
        assert_eq!(exchange.response(owner).0, 400);
    }
}
#[test]
fn settings_answers_and_each_fault_keep_their_facts() {
    let limits = f::wire();
    let answer = f::translated(&p::Reply::Outcome(p::Outcome::SettingsChanged { revision: 8 }), &limits);
    let w::Final::Done(value) = answer else { panic!("done settings") };
    let w::Outcome::SettingsChanged(value) = value.outcome() else { panic!("settings changed") };
    assert_eq!(value.revision(), 8);
    for why in [
        p::Refusal::Stale { newest: 8 },
        p::Refusal::Full { revision: 8, tasks: Box::new([3, 5]), more: 2 },
        p::Refusal::NotWellFormed { at: 0, why: p::Malformed::SettingsUnchanged },
    ] {
        let value = translate::refusals::refusal(why.clone(), &limits).expect("settings fixture");
        match (why, value) {
            (p::Refusal::Stale { newest }, w::Refusal::Stale(value)) => assert_eq!(value.newest(), newest),
            (p::Refusal::Full { revision, tasks, more }, w::Refusal::Full(value)) => {
                assert_eq!(value.revision(), revision);
                assert_eq!(value.tasks().as_slice(), tasks.as_ref());
                assert_eq!(value.more(), more);
            }
            (p::Refusal::NotWellFormed { .. }, w::Refusal::Malformed(value)) => {
                assert_eq!(value.why(), &w::Malformed::SettingsUnchanged);
            }
            value => panic!("settings refusal {value:?}"),
        }
    }
    for setting in [p::Setting::Charter(1), p::Setting::Template(2), p::Setting::Policy(3), p::Setting::Rules] {
        for fault in [
            p::SettingFault::Shape,
            p::SettingFault::Count,
            p::SettingFault::Reference,
            p::SettingFault::Rules,
            p::SettingFault::Budget,
            p::SettingFault::Grace,
            p::SettingFault::Owner,
        ] {
            let value = translate::refusals::refusal(
                p::Refusal::NotWellFormed { at: 0, why: p::Malformed::Settings { setting, fault } },
                &limits,
            )
            .expect("settings fixture");
            let w::Refusal::Malformed(value) = value else { panic!("malformed settings") };
            let w::Malformed::Settings(facts) = value.why() else { panic!("settings facts") };
            assert_eq!(
                facts.setting(),
                &match setting {
                    p::Setting::Charter(number) => w::Setting::Charter(
                        w::SettingNumber::new(&limits, w::SettingNumberParts { number }).expect("number")
                    ),
                    p::Setting::Template(number) => w::Setting::Template(
                        w::SettingNumber::new(&limits, w::SettingNumberParts { number }).expect("number")
                    ),
                    p::Setting::Policy(number) => w::Setting::Policy(
                        w::SettingNumber::new(&limits, w::SettingNumberParts { number }).expect("number")
                    ),
                    p::Setting::Rules => w::Setting::Rules,
                }
            );
        }
    }
}

fn proposal_ask() -> w::Ask {
    let w::Ask::ChangeSettings(value) = ask() else { panic!("settings fixture") };
    let value = value.into_parts();
    let limits = f::wire();
    w::Ask::Propose(
        w::Propose::new(
            &limits,
            w::ProposeParts {
                project: 0,
                action: w::ProposeAction::Settings(
                    w::ProposeSettings::new(
                        &limits,
                        w::ProposeSettingsParts { against: value.against, items: value.items },
                    )
                    .expect("proposed set"),
                ),
                reason: b"review this set".as_slice().into(),
            },
        )
        .expect("proposal ask"),
    )
}
#[test]
fn settings_proposals_reach_the_core_with_the_whole_set_and_exact_digest() {
    let mut env = f::env();
    env.limits.message_bytes = 8192;
    env.limits.wire.keyed_request = 8192;
    let mut exchange = Exchange::with_env(env);
    exchange.component.settings.models[0].name = b"model".as_slice().into();
    exchange.component.settings.models[1].name = b"other".as_slice().into();
    let ask = proposal_ask();
    let bytes = f::encode_ask(&ask);
    let digest: [u8; 32] = crate::sign_in::digest(&bytes).expose().try_into().expect("digest");
    assert_eq!(
        translate::asks::ask(ask, &exchange.component.settings.models, &f::wire()),
        Ok(p::Ask::Propose {
            project: 0,
            action: p::Action::Settings { against: 5, digest },
            reason: b"review this set".as_slice().into()
        })
    );
    exchange.send(crate::Message::Request(proposal_envelope(bytes, &exchange.env.limits.wire)));
    exchange.authenticate();
    let Some(Request::Core(event)) = exchange.out.pop() else { panic!("core proposal") };
    let core::Event::ProposeSettings { sign_in, digest: held, change, .. } = *event else {
        panic!("settings proposal entrance")
    };
    assert_eq!((sign_in, held, change.against, change.items.len()), (41, digest, 5, 4));
    assert_eq!(change.reason.as_ref(), b"review this set");
}
#[test]
fn accepting_settings_translates_both_running_choices() {
    let limits = f::wire();
    for (wire, expected) in [(w::Running::Stays, p::Running::Stays), (w::Running::Moves, p::Running::Moves)] {
        let value = w::Ask::Decide(
            w::Decide::new(
                &limits,
                w::DecideParts {
                    project: 0,
                    on: w::DecideOn::Proposal(
                        w::DecideProposal::new(
                            &limits,
                            w::DecideProposalParts {
                                proposal: w::ProposalKey::new(
                                    &limits,
                                    w::ProposalKeyParts {
                                        proposer: w::Proposer::Party(
                                            w::ProposerParty::new(&limits, w::ProposerPartyParts { value: 9 })
                                                .expect("proposer"),
                                        ),
                                        number: 7,
                                    },
                                )
                                .expect("proposal key"),
                                decision: w::Decision::Accepted,
                                running: Some(wire),
                            },
                        )
                        .expect("decision"),
                    ),
                },
            )
            .expect("ask"),
        );
        assert_eq!(
            translate::asks::ask(value, &f::models(), &limits),
            Ok(p::Ask::DecideProposal {
                project: 0,
                proposer: p::Proposer::Party(9),
                proposal: 7,
                decision: p::ProposalDecision::Accept,
                running: Some(expected)
            })
        );
    }
}
#[test]
fn unknown_models_in_settings_proposals_are_named_before_decoding_the_set() {
    let mut env = f::env();
    env.limits.message_bytes = 8192;
    env.limits.wire.keyed_request = 8192;
    let mut exchange = Exchange::with_env(env);
    let bytes = proposal_envelope(f::encode_ask(&proposal_ask()), &exchange.env.limits.wire);
    let owner = exchange.send(crate::Message::Request(bytes));
    exchange.authenticate();
    assert!(exchange.out.is_empty());
    let w::Final::Refused(value) = exchange.final_answer(owner) else { panic!("named model refusal") };
    let w::Refusal::Llm(refusal) = value.refusal() else { panic!("LLM refusal") };
    assert_eq!(refusal.model(), b"model");
    assert_eq!(*refusal.why(), w::LlmWhy::Unconfigured);
}

fn proposal_envelope(request: Box<[u8]>, limits: &w::Limits) -> Box<[u8]> {
    let keyed =
        w::Keyed::new(limits, w::KeyedParts { key: [5; 16], owner: w::Owner::Jig, version: 19, request, value: None })
            .expect("whole proposal envelope");
    let mut writer = skein_lib::Writer::new(usize::try_from(keyed.measure()).expect("wire size"));
    keyed.encode(&mut writer).expect("measured envelope");
    writer.finish()
}

#[test]
fn move_to_newest_translates_the_revision_and_both_counts() {
    let limits = f::wire();
    let ask = w::Ask::MoveToNewest(w::MoveToNewest::new(&limits, w::MoveToNewestParts { revision: 7 }).expect("move"));
    assert_eq!(translate::asks::ask(ask, &[], &limits), Ok(p::Ask::MoveToNewest { revision: 7 }));
    let answer = translate::answers::answer(
        &p::Reply::Outcome(p::Outcome::MovedToNewest { newest: 9, moved: 3, stayed: 1 }),
        &[],
        &limits,
    )
    .expect("answer");
    let w::KeyedAnswer::FinalAnswer(answer) = answer else { panic!("final answer") };
    let answer = w::Final::decode(&limits, &mut skein_lib::Reader::new(answer.record())).expect("final");
    let w::Final::Done(answer) = answer else { panic!("done") };
    let w::Outcome::MovedToNewest(value) = answer.outcome() else { panic!("counts") };
    assert_eq!((value.newest(), value.moved(), value.stayed()), (9, 3, 1));
}
