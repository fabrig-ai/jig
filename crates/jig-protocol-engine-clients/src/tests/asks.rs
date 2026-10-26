use super::fixtures::{self as f, Exchange};
use crate::{
    Request, ToApplication,
    translate::{Untranslatable, asks::ask},
};
use jig_client_protocol_wire::v19 as w;
use jig_domain_people as p;
#[test]
fn a_message_record_becomes_the_cores_say() {
    let limits = f::wire();
    assert_eq!(
        ask(f::start(&limits), &f::models(), &limits),
        Ok(p::Ask::Say { project: 3, task: 8, words: Box::from(b"hello".as_slice()) })
    );
}
#[test]
fn a_decision_record_keeps_its_revision() {
    let limits = f::wire();
    let cases = [
        (w::Decision::PassedUp, p::EscalationDecision::Pass),
        (
            w::Decision::Released(
                w::DecisionReleased::new(&limits, w::DecisionReleasedParts { again: None }).expect("release"),
            ),
            p::EscalationDecision::Release,
        ),
    ];
    for (decision, expected) in cases {
        let value = f::decide(w::DecideOn::Escalation(
            w::DecideEscalation::new(
                &limits,
                w::DecideEscalationParts { task: 8, revision: f::revision(u64::MAX), decision },
            )
            .expect("decision"),
        ));
        assert_eq!(
            ask(value, &f::models(), &limits),
            Ok(p::Ask::DecideEscalation { project: 3, task: 8, revision: u64::MAX, decision: expected })
        );
    }
}
#[test]
fn an_amendment_record_keeps_its_release_and_reason() {
    for (release, expected_release, reason) in
        [(None, None, b"".as_slice()), (Some(f::revision(4)), Some(4), b"supplied".as_slice())]
    {
        let amendment = p::Amendment {
            instructions: Some(Box::new([])),
            dependencies: Some(Box::new([])),
            ..f::person_unchanged(reason)
        };
        let expected = match expected_release {
            Some(revision) => {
                let mut operations = skein_lib::List::with_capacity(2);
                operations
                    .push(p::Operation::Amend { task: 8, revision: 6, amendment: Box::new(amendment), release: None })
                    .unwrap();
                operations.push(p::Operation::Release { task: 8, revision, again: None }).unwrap();
                p::Ask::Compound { project: 3, operations }
            }
            None => p::Ask::Amend { project: 3, task: 8, revision: 6, amendment, release: None },
        };
        assert_eq!(ask(f::amendment(release, reason), &f::models(), &f::wire()), Ok(expected));
    }
}
fn invalid() -> w::Ask {
    let limits = f::wire();
    f::decide(w::DecideOn::Escalation(
        w::DecideEscalation::new(
            &limits,
            w::DecideEscalationParts { task: 8, revision: f::revision(6), decision: w::Decision::Accepted },
        )
        .expect("valid record with inapplicable decision"),
    ))
}
#[test]
fn an_ask_that_does_not_translate_reaches_no_core() {
    let value = invalid();
    assert_eq!(
        ask(value.clone(), &f::models(), &f::wire()),
        Err(Untranslatable { path: w::Path::DecideEscalationDecision })
    );
    let mut exchange = Exchange::new();
    for bytes in [f::encode_ask(&value), f::encode_ask(&f::malformed_piece()), Box::new([255]), {
        let bytes = f::encode_ask(&f::start(&f::wire()));
        let mut out = skein_lib::Writer::new(bytes.len().checked_add(1).expect("tail size"));
        out.put(&bytes).expect("ask");
        out.put(&[0]).expect("tail");
        out.finish()
    }] {
        let owner = exchange.send(crate::Message::Request(f::keyed(w::Owner::Jig, 19, bytes)));
        exchange.authenticate();
        assert!(exchange.out.is_empty());
        assert_eq!(exchange.response(owner), (400, b"".as_slice()));
    }
}
#[test]
fn an_application_record_is_routed_with_its_digest() {
    let mut exchange = Exchange::new();
    let family = w::Owner::Family(w::Family::new(&f::wire(), w::FamilyParts { number: 9 }).expect("family"));
    exchange.send(crate::Message::Request(f::keyed(family, 2, Box::from(b"abc".as_slice()))));
    exchange.authenticate();
    let Some(Request::Application(ToApplication::Request { sign_in, key, family, version, record, digest, .. })) =
        exchange.out.pop()
    else {
        panic!("application request");
    };
    assert_eq!((sign_in, key, family, version, record.as_ref()), (41, [5; 16], 9, 2, b"abc".as_slice()));
    assert_eq!(
        digest,
        [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22, 0x23, 0xb0, 0x03,
            0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00, 0x15, 0xad
        ]
    );
}
#[test]
fn a_third_exchange_past_the_client_s_bound_is_busy() {
    let mut exchange = Exchange::new();
    let body = f::keyed(w::Owner::Jig, 19, f::encode_ask(&f::start(&f::wire())));
    for _ in 0..2_u32 {
        exchange.send(crate::Message::Request(body.clone()));
        exchange.authenticate();
        let Some(Request::Core(_)) = exchange.out.pop() else {
            panic!("admitted ask");
        };
    }
    let owner = exchange.send(crate::Message::Request(body));
    exchange.authenticate();
    assert!(exchange.out.is_empty());
    assert_eq!(exchange.response(owner), (200, b"\x02".as_slice()));
}

#[test]
fn every_other_ask_family_keeps_its_supplied_fields() {
    let limits = f::wire();
    let cases = f::other_asks();
    for (wire, expected) in cases {
        assert_eq!(ask(wire, &f::models(), &limits), Ok(expected));
    }
}

#[test]
fn policy_authority_keeps_every_length_prefixed_grant_and_resource() {
    let limits = f::wire();
    let (wire, expected) = f::authority_ask();
    assert_eq!(ask(wire.clone(), &f::models(), &limits), Ok(expected));
}
#[test]
fn a_piece_past_its_decoded_sum_is_refused() {
    let limits = f::wire();
    let (wire, _) = f::authority_ask();
    let small = w::Limits { owner_authority_grants: 19, ..limits };
    assert_eq!(ask(wire, &f::models(), &small), Err(Untranslatable { path: w::Path::OwnerAuthorityGrants }));
}

#[test]
fn an_undecodable_application_record_is_answered_bad_request() {
    let mut exchange = Exchange::new();
    let family = w::Owner::Family(w::Family::new(&f::wire(), w::FamilyParts { number: 9 }).expect("family"));
    let owner = exchange.send(crate::Message::Request(f::keyed(family, 2, Box::new([255]))));
    exchange.authenticate();
    let Some(Request::Application(ToApplication::Request { reply_to, .. })) = exchange.out.pop() else {
        panic!("application request");
    };
    exchange.component.down(
        &exchange.env,
        crate::Event::Application(crate::FromApplication::Undecodable { to: reply_to }),
        &mut exchange.out,
    );
    assert_eq!(exchange.response(owner), (400, b"".as_slice()));
    assert!(exchange.out.is_empty());
}

#[test]
fn a_decide_on_an_escalation_reaches_the_people_child_with_its_revision() {
    let limits = f::wire();
    let target = w::DecideEscalation::new(
        &limits,
        w::DecideEscalationParts { task: 8, revision: f::revision(u64::MAX), decision: w::Decision::PassedUp },
    )
    .expect("escalation");
    assert_eq!(
        ask(f::decide(w::DecideOn::Escalation(target)), &f::models(), &limits),
        Ok(p::Ask::DecideEscalation { project: 3, task: 8, revision: u64::MAX, decision: p::EscalationDecision::Pass })
    );
}
#[test]
fn a_cancel_with_a_grace_reaches_the_people_child_with_its_span() {
    const GRACE: skein_lib::Duration = skein_lib::Duration::from_millis(200);
    let limits = f::wire();
    let cancel = w::Cancel::new(
        &limits,
        w::CancelParts { project: 3, task: 8, reason: b"enough".as_slice().into(), grace: Some(GRACE) },
    )
    .expect("cancel");
    assert_eq!(
        ask(w::Ask::Cancel(cancel), &f::models(), &limits),
        Ok(p::Ask::Cancel { project: 3, task: 8, reason: b"enough".as_slice().into(), grace: Some(GRACE) })
    );
}
#[test]
fn a_priority_only_amend_names_no_revision() {
    assert_eq!(
        ask(f::priority_amend(), &f::models(), &f::wire()),
        Ok(p::Ask::Amend {
            project: 3,
            task: 8,
            revision: 0,
            release: None,
            amendment: p::Amendment {
                tracked: Some(p::Tracking::Tracked { priority: u32::MAX }),
                ..f::person_unchanged(b"")
            },
        })
    );
}
#[test]
fn a_policy_change_of_template_overrides_reaches_the_people_child() {
    assert_eq!(
        ask(f::template_change(), &f::models(), &f::wire()),
        Ok(p::Ask::ChangePolicy {
            project: 3,
            change: p::PolicyChange::Templates {
                overrides: Box::new([p::TemplateOverride {
                    template: 2,
                    executor: Some(p::TemplateExecutor::Chosen),
                    budget: Some(p::TemplateBudget::Given),
                    deadline: Some(p::TemplateDeadline::None),
                    priority: Some(u32::MAX)
                }])
            }
        })
    );
}

#[test]
fn a_propose_of_an_amendment_reaches_the_people_child_with_its_reason() {
    let limits = f::wire();
    let action = w::ProposeAction::Amendment(Box::new(
        w::ProposeAmendment::new(
            &limits,
            w::ProposeAmendmentParts { task: 8, revision: f::revision(9), change: f::empty_amendment(None) },
        )
        .expect("amendment"),
    ));
    let value = w::Ask::Propose(
        w::Propose::new(&limits, w::ProposeParts { project: 3, action, reason: b"more work".as_slice().into() })
            .expect("proposal"),
    );
    assert_eq!(
        ask(value, &f::models(), &limits),
        Ok(p::Ask::Propose {
            project: 3,
            action: p::Action::Amend { task: 8, revision: 9, amendment: Box::new(f::person_unchanged(b"more work")) },
            reason: b"more work".as_slice().into(),
        })
    );
}

#[test]
fn a_sign_in_ask_is_made_a_people_ask_for_its_holder() {
    let limits = f::wire();
    for (holder, expected) in
        [(w::AccountHolder::Own, p::AccountHolder::Own), (w::AccountHolder::Deployment, p::AccountHolder::Deployment)]
    {
        let wire = w::Ask::SignIn(
            w::AskSignIn::new(&limits, w::AskSignInParts { holder: holder.clone(), provider: u32::MAX }).unwrap(),
        );
        assert_eq!(
            ask(wire, &f::models(), &limits),
            Ok(p::Ask::AccountSignIn { holder: expected, provider: u32::MAX })
        );
        let wire =
            w::Ask::SignOut(w::AskSignOut::new(&limits, w::AskSignOutParts { holder, source: u32::MAX }).unwrap());
        assert_eq!(ask(wire, &f::models(), &limits), Ok(p::Ask::AccountSignOut { holder: expected, source: u32::MAX }));
    }
}

#[test]
fn project_records_reach_people_with_their_whole_name_and_policy() {
    let limits = f::wire();
    for (record, expected) in [
        (
            w::Ask::Rename(
                w::Rename::new(&limits, w::RenameParts { project: 3, name: b"Tally".as_slice().into() })
                    .expect("rename"),
            ),
            p::Ask::Rename { project: 3, name: "Tally".into() },
        ),
        (
            w::Ask::MakeProject(
                w::MakeProject::new(&limits, w::MakeProjectParts { name: b"Tally".as_slice().into(), policy: 7 })
                    .expect("make"),
            ),
            p::Ask::MakeProject { name: "Tally".into(), policy: 7 },
        ),
    ] {
        assert_eq!(ask(record, &f::models(), &limits), Ok(expected));
    }
}

#[test]
fn a_ceiling_edit_translates_the_whole_authority() {
    let (wire, expected) = f::authority_ask();
    let w::Ask::ChangePolicy(wire) = wire else { panic!("policy fixture") };
    let w::PolicyEdit::Role(role) = wire.into_parts().edit else { panic!("role fixture") };
    let p::Ask::ChangePolicy { project, change: p::PolicyChange::Role(expected) } = expected else {
        panic!("role fixture")
    };
    let limits = f::wire();
    let wire = w::Ask::ChangePolicy(
        w::ChangePolicy::new(
            &limits,
            w::ChangePolicyParts {
                project,
                edit: w::PolicyEdit::Ceiling(
                    w::PolicyEditCeiling::new(
                        &limits,
                        w::PolicyEditCeilingParts { ceiling: role.into_parts().value.into_parts().authority },
                    )
                    .unwrap(),
                ),
            },
        )
        .unwrap(),
    );
    assert_eq!(
        ask(wire, &f::models(), &limits),
        Ok(p::Ask::ChangePolicy { project, change: p::PolicyChange::Ceiling { ceiling: expected.authority } })
    );
}
