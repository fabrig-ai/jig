use super::fixtures::{self as f, Exchange};
use crate::{Event, Request, SecretsRequest};
use jig_client_protocol_wire::v19 as w;
use jig_domain_people as p;
use skein_lib::{Reader, ReplyTo, Token};
#[test]
fn a_family_request_the_people_child_refuses_is_answered_under_jig_s_owner() {
    let mut exchange = Exchange::new();
    let (owner, to) = exchange.application();
    exchange.reply(to, p::Reply::Refused(p::Refusal::KeyConflict));
    assert_eq!(
        exchange.final_answer(owner),
        w::Final::Refused(
            w::Refused::new(&f::wire(), w::RefusedParts { at: None, refusal: w::Refusal::KeyConflict })
                .expect("refused")
        )
    );
    assert!(exchange.component.pending.exchanges.is_empty());
    assert!(exchange.out.is_empty());
}
#[test]
fn a_made_chat_is_answered_done_with_its_task() {
    let mut exchange = Exchange::new();
    let (owner, to) = exchange.ask();
    assert!(unanswered(&exchange, owner));
    exchange.reply(to, p::Reply::Outcome(p::Outcome::Said { task: 8, message: 9 }));
    assert_eq!(exchange.final_answer(owner), f::started());
    assert!(exchange.component.pending.exchanges.is_empty());
    assert!(exchange.out.is_empty());
}
#[test]
fn a_moved_hold_is_refused_with_its_facts() {
    let mut exchange = Exchange::new();
    let (owner, to) = exchange.ask();
    exchange.reply(
        to,
        p::Reply::Outcome(p::Outcome::Refused(p::Refusal::Moved {
            now: p::Moved::Hold { revision: 31, reason: p::HoldReason::Dependency { task: 9 } },
        })),
    );
    assert_eq!(exchange.final_answer(owner), f::moved_hold());
}
#[test]
fn again_busy_is_answered_busy_and_not_kept() {
    let mut exchange = Exchange::new();
    for (reply, expected) in [
        (p::Reply::Again(p::Again::Busy), w::KeyedAnswer::Busy),
        (p::Reply::Outcome(p::Outcome::Again(p::Again::NotReady)), w::KeyedAnswer::NotReady),
    ] {
        let (owner, to) = exchange.ask();
        exchange.reply(to, reply);
        assert_eq!(exchange.answer(owner), expected);
        assert!(exchange.component.pending.exchanges.is_empty());
        assert!(exchange.out.is_empty());
        exchange.component.connections.remove(&owner);
    }
}
#[test]
fn an_ended_sign_in_is_answered_signed_out() {
    let mut exchange = Exchange::new();
    let (owner, to) = exchange.ask();
    exchange.component.down(&exchange.env, Event::SignInEnded { sign_in: f::NUMBER }, &mut exchange.out);
    assert_eq!(exchange.answer(owner), w::KeyedAnswer::SignedOut);
    let Some(Request::Secrets(SecretsRequest::Erase { owner: secret_owner })) = exchange.out.pop() else {
        panic!("erase digest");
    };
    assert_eq!(secret_owner, crate::SecretOwner::SignIn(41));
    assert!(exchange.component.pending.exchanges.is_empty());
    exchange.reply(to, p::Reply::Outcome(p::Outcome::Said { task: 8, message: 9 }));
    assert_eq!(exchange.answer(owner), w::KeyedAnswer::SignedOut);
    assert!(exchange.out.is_empty());
}
#[test]
fn an_application_outcome_is_carried_under_its_family() {
    let mut exchange = Exchange::new();
    let (owner, to) = exchange.application();
    let number = to.into_token();
    exchange
        .reply(ReplyTo::new(number), p::Reply::Outcome(p::Outcome::Applied { connector: 9, token: Token::new(91) }));
    assert!(unanswered(&exchange, owner));
    answer_application(&mut exchange, ReplyTo::new(number), Box::from(b"outcome".as_slice()));
    assert_eq!(exchange.answer(owner), f::application_answer());
    assert!(exchange.component.pending.exchanges.is_empty());
    assert!(exchange.out.is_empty());
}
#[test]
fn owner_outcomes_keep_every_supplied_fact() {
    for to in [7, 91] {
        let final_answer = f::translated(&p::Reply::Outcome(p::Outcome::Proposed { proposal: 31, to }), &f::wire());
        let w::Final::Done(done) = final_answer else { panic!("done") };
        let w::Outcome::Owner(owner) = done.outcome() else { panic!("owner outcome") };
        let w::OwnerOutcome::Proposed(row) = owner.value() else { panic!("proposal") };
        assert_eq!((row.proposal(), row.to()), (31, to));
    }
    let limits = f::wire();
    for cases in [f::owner_outcomes().as_slice(), f::decision_outcomes().as_slice(), f::seen_outcomes().as_slice()] {
        for (reply, expected) in cases {
            assert_eq!(f::translated(reply, &limits), *expected);
        }
    }
}

#[test]
fn a_service_result_contains_its_party_and_no_credential() {
    let answer = crate::translate::answers::answer(
        &p::Reply::Outcome(p::Outcome::ServiceMade { person: 7 }),
        &f::models(),
        &f::wire(),
    )
    .expect("service answer");
    let mut out = skein_lib::Writer::new(25);
    answer.encode(&mut out).expect("25-byte party-only answer");
    assert_eq!(out.finish().as_ref(), &f::SERVICE_ANSWER);
}
#[test]
fn an_account_sign_in_is_answered_with_its_source_and_address() {
    let limits = w::Limits { owned_record: 4096, ..f::wire() };
    for length in [0, 1, usize::try_from(p::ADDRESS).unwrap()] {
        let address: Box<[u8]> = skein_lib::bytes::zeroed(length);
        let outcome = p::Outcome::AccountSignInStarted { source: u32::MAX, address: address.clone() };
        let expected = w::OwnerOutcome::AccountSignInStarted(
            w::OwnerOutcomeAccountSignInStarted::new(
                &limits,
                w::OwnerOutcomeAccountSignInStartedParts { source: u32::MAX, address },
            )
            .unwrap(),
        );
        assert_eq!(f::translated(&p::Reply::Outcome(outcome), &limits), account_sign_in_done(expected, &limits));
    }
}
#[test]
fn an_account_sign_out_is_answered_with_whether_it_was_confirmed() {
    let limits = f::wire();
    for confirmed in [false, true] {
        let expected = w::OwnerOutcome::AccountSignedOut(
            w::OwnerOutcomeAccountSignedOut::new(
                &limits,
                w::OwnerOutcomeAccountSignedOutParts { confirmed, failed_hosts: skein_lib::List::with_capacity(0) },
            )
            .unwrap(),
        );
        assert_eq!(
            f::translated(
                &p::Reply::Outcome(p::Outcome::AccountSignedOut { confirmed, failed_hosts: Box::new([]) }),
                &limits
            ),
            account_sign_in_done(expected, &limits)
        );
    }
}
#[test]
fn the_account_bounds_are_admission_limits() {
    for (source, expected) in [
        (p::LimitKind::Accounts, w::AdmissionLimit::Accounts),
        (p::LimitKind::HolderAccounts, w::AdmissionLimit::HolderAccounts),
        (p::LimitKind::Sources, w::AdmissionLimit::Sources),
        (p::LimitKind::SignIns, w::AdmissionLimit::SignIns),
    ] {
        assert_eq!(
            crate::translate::refusals::refusal(p::Refusal::Limit { which: source }, &f::wire()).unwrap(),
            f::admission_refusal(expected)
        );
    }
}
fn unanswered(exchange: &Exchange, owner: Token) -> bool {
    exchange.component.connections.get(&owner).expect("owner").response.is_none()
}

#[test]
fn refusal_facts_keep_every_sparse_entrance_category() {
    let limits = f::wire();
    for cases in [f::refusals().as_slice(), f::extra_refusals().as_slice()] {
        for (source, expected) in cases {
            assert_eq!(crate::translate::refusals::refusal(source.clone(), &limits).expect("refusal fits"), *expected);
        }
    }
}
#[test]
fn moved_holds_keep_every_cause_and_revision() {
    let limits = f::wire();
    for (source, expected) in f::holds() {
        let source = p::Refusal::Moved { now: p::Moved::Hold { revision: 31, reason: source } };
        assert_eq!(crate::translate::refusals::refusal(source, &limits).expect("hold fits"), f::hold_refusal(expected));
    }
}
#[test]
fn ended_refusals_keep_one_sparse_row_and_its_wall_time() {
    let limits = f::wire();
    for (source, expected) in f::endings() {
        let actual =
            crate::translate::refusals::refusal(p::Refusal::Ended { how: source }, &limits).expect("ending fits");
        assert_eq!(actual, expected);
        if source == p::Ending::Done {
            done_ending_round_trips(&limits);
        }
    }
}
fn done_ending_round_trips(limits: &w::Limits) {
    let refusal =
        crate::translate::refusals::refusal(p::Refusal::Ended { how: p::Ending::Done }, limits).expect("ending fits");
    let value = w::Final::Refused(w::Refused::new(limits, w::RefusedParts { at: None, refusal }).expect("refusal"));
    let mut out = skein_lib::Writer::new(usize::try_from(value.measure()).expect("short final"));
    value.encode(&mut out).expect("final answer");
    let owned = w::Owned::new(
        limits,
        w::OwnedParts { owner: w::Owner::Jig, version: jig_client_protocol_wire::VERSION, record: out.finish() },
    )
    .expect("owned final");
    assert_eq!(
        jig_client_protocol_link::translate::answers::answer(w::KeyedAnswer::FinalAnswer(owned), &f::client_limits()),
        Ok(jig_client_domain::contract::Answer::Refused {
            refusal: jig_client_domain::contract::Refusal::Ended { how: jig_client_domain::contract::Ending::Done },
            at: None
        })
    );
}
#[test]
fn an_answer_past_its_bound_is_lost_without_a_partial_response() {
    let mut exchange = Exchange::new();
    for (bound, expected) in f::response_bounds() {
        let (owner, to) = exchange.application();
        exchange.env.limits.wire.owned_record = bound;
        answer_application(&mut exchange, to, Box::new([3; 257]));
        assert!(exchange.component.pending.exchanges.is_empty());
        match expected {
            Some(expected) => assert_eq!(exchange.answer(owner), expected),
            None => {
                assert!(unanswered(&exchange, owner));
            }
        }
        assert!(exchange.out.is_empty());
        exchange.component.connections.remove(&owner);
    }
}
fn answer_application(exchange: &mut Exchange, to: ReplyTo, record: Box<[u8]>) {
    exchange.component.down(
        &exchange.env,
        Event::Application(crate::FromApplication::Answered { to, family: 9, record }),
        &mut exchange.out,
    );
}

#[test]
fn admission_refusals_keep_the_category_without_a_guessed_bound() {
    let limits = f::wire();
    for cases in [f::admission_0().as_slice(), f::admission_1().as_slice()] {
        for (source, expected) in cases {
            assert_eq!(
                crate::translate::refusals::refusal(p::Refusal::Limit { which: *source }, &limits).expect("limit fits"),
                f::admission_refusal(expected.clone())
            );
        }
    }
}
#[test]
fn authority_refusals_keep_the_known_facts_and_optional_proposal_verdict() {
    let limits = f::wire();
    let cases = f::lacks();
    for (proposable, boolean) in
        [(None, None), (Some(p::Accepter::Nobody), Some(false)), (Some(p::Accepter::Role(0)), Some(true))]
    {
        for (source, expected) in &cases {
            let source = p::Refusal::Authority { lacking: *source, proposable };
            let expected = w::Refusal::Authority(
                w::RefusalAuthority::new(
                    &limits,
                    w::RefusalAuthorityParts { lacking: expected.clone(), proposable: boolean },
                )
                .expect("authority"),
            );
            assert_eq!(crate::translate::refusals::refusal(source, &limits).expect("authority fits"), expected);
        }
    }
}
#[test]
fn an_absent_sign_in_is_answered_signed_out_without_a_saved_answer() {
    let mut exchange = Exchange::new();
    for reply in [p::Reply::SignedOut, p::Reply::Outcome(p::Outcome::SignedOut)] {
        let (owner, to) = exchange.ask();
        exchange.reply(to, reply);
        assert_eq!(exchange.answer(owner), w::KeyedAnswer::SignedOut);
        assert!(exchange.component.pending.exchanges.is_empty());
        assert!(exchange.out.is_empty());
        exchange.component.connections.remove(&owner);
    }
}
#[test]
fn a_sparse_ending_past_its_piece_bound_is_refused() {
    let limits = w::Limits { ending_rows: 5, ..f::wire() };
    assert_eq!(
        crate::translate::refusals::refusal(p::Refusal::Ended { how: p::Ending::Done }, &limits),
        Err(w::Problem { path: w::Path::EndingRows, reason: skein_codec::Reason::Bound })
    );
}

const UNSERVED_VERSION: u32 = jig_client_protocol_wire::VERSION + 1;
const REFUSED_WATCH: u64 = 71;
#[test]
fn a_keyed_request_outside_its_owners_range_is_answered_not_served() {
    let mut exchange = Exchange::new();
    let owner = exchange.send(crate::Message::Request(f::keyed(w::Owner::Jig, UNSERVED_VERSION, Box::new([255]))));
    exchange.authenticate();
    assert_eq!(exchange.answer(owner), w::KeyedAnswer::NotServed(f::not_served(w::Owner::Jig)));
    assert!(exchange.out.is_empty());
}
#[test]
fn a_read_outside_its_owners_range_names_the_served_ranges() {
    let mut exchange = Exchange::new();
    let owner = exchange.send(crate::Message::Read(f::owned_read(w::Owner::Jig, UNSERVED_VERSION, Box::new([255]))));
    exchange.authenticate();
    assert_eq!(exchange.read_answer(owner), w::ReadAnswer::NotServed(f::not_served(w::Owner::Jig)));
    assert!(exchange.out.is_empty());
}
#[test]
fn an_opening_outside_its_owners_range_is_answered_not_served() {
    let mut exchange = Exchange::new();
    let stream = exchange.open_stream();
    let subject = w::Owned::new(
        &f::wire(),
        w::OwnedParts { owner: w::Owner::Jig, version: UNSERVED_VERSION, record: Box::new([255]) },
    )
    .expect("opaque subject");
    let owner = exchange.open_record(stream, REFUSED_WATCH, subject);
    assert_eq!(
        w::OpenAnswer::decode(&f::wire(), &mut Reader::new(exchange.response(owner).1)),
        Ok(w::OpenAnswer::NotServed(f::not_served(w::Owner::Jig)))
    );
    assert!(exchange.out.is_empty());
}

fn account_sign_in_done(value: w::OwnerOutcome, limits: &w::Limits) -> w::Final {
    let outcome = w::Outcome::Owner(w::OutcomeOwner::new(limits, w::OutcomeOwnerParts { value }).unwrap());
    w::Final::Done(w::Done::new(limits, w::DoneParts { outcome }).unwrap())
}

#[test]
fn an_account_sign_in_answer_over_the_address_bound_is_refused_at_its_address() {
    let outcome = p::Outcome::AccountSignInStarted { source: 7, address: skein_lib::bytes::zeroed(2049) };
    let result = crate::translate::answers::answer(&p::Reply::Outcome(outcome), &f::models(), &f::wire());
    assert_eq!(
        result.err(),
        Some(w::Problem { path: w::Path::OwnerOutcomeAccountSignInStartedAddress, reason: skein_codec::Reason::Bound })
    );
}

#[test]
fn dependency_refusals_keep_all_names_and_outcomes_through_the_client() {
    use jig_client_domain::contract as c;
    use skein_lib::List;
    let mut names = List::with_capacity(2);
    names.push(7).unwrap();
    names.push(5).unwrap();
    for (why, expected) in [
        (
            p::Refusal::Circle { tasks: Box::new([7, 5]), cut: true },
            c::Refusal::Circle { tasks: names.clone(), cut: true },
        ),
        (p::Refusal::Kept { pending: Box::new([7, 5]) }, c::Refusal::Kept { pending: names }),
        (
            p::Refusal::Dependency { dependency: 7, how: p::DependencyStatus::Failed },
            c::Refusal::Dependency { dependency: 7, how: c::OwnerTaskStatus::Failed },
        ),
        (
            p::Refusal::Dependency { dependency: 5, how: p::DependencyStatus::Cancelled },
            c::Refusal::Dependency { dependency: 5, how: c::OwnerTaskStatus::Cancelled },
        ),
    ] {
        let reply = p::Reply::Outcome(p::Outcome::Refused(why));
        let record = crate::translate::answers::answer(&reply, &[], &f::wire()).unwrap();
        assert_eq!(
            jig_client_protocol_link::translate::answers::answer(record, &f::client_limits()),
            Ok(c::Answer::Refused { at: None, refusal: expected })
        );
    }
}

#[test]
fn project_outcomes_keep_the_project_number() {
    let limits = f::wire();
    for (outcome, expected) in [
        (
            p::Outcome::Renamed { project: 3 },
            w::OwnerOutcome::Renamed(
                w::OwnerOutcomeRenamed::new(&limits, w::OwnerOutcomeRenamedParts { project: 3 }).expect("renamed"),
            ),
        ),
        (
            p::Outcome::ProjectMade { project: 4 },
            w::OwnerOutcome::ProjectMade(
                w::OwnerOutcomeProjectMade::new(&limits, w::OwnerOutcomeProjectMadeParts { project: 4 }).expect("made"),
            ),
        ),
    ] {
        let expected = w::Final::Done(
            w::Done::new(
                &limits,
                w::DoneParts {
                    outcome: w::Outcome::Owner(
                        w::OutcomeOwner::new(&limits, w::OutcomeOwnerParts { value: expected }).expect("owner"),
                    ),
                },
            )
            .expect("done"),
        );
        assert_eq!(f::translated(&p::Reply::Outcome(outcome), &limits), expected);
    }
}

#[test]
#[expect(clippy::disallowed_macros, reason = "ordinary test builds typed refusal cases")]
fn project_refusals_keep_their_facts() {
    let limits = f::wire();
    let mut cases = vec![
        (p::Refusal::NameTaken, w::Refusal::NameTaken),
        (
            p::Refusal::Limit { which: p::LimitKind::Projects },
            w::Refusal::Limit(
                w::RefusalLimit::new(
                    &limits,
                    w::RefusalLimitParts {
                        which: w::Limit::Admission(
                            w::LimitAdmission::new(
                                &limits,
                                w::LimitAdmissionParts { which: w::AdmissionLimit::Projects },
                            )
                            .expect("admission limit"),
                        ),
                    },
                )
                .expect("limit"),
            ),
        ),
    ];
    for (rule, expected) in [
        (p::NameRule::Empty, w::NameRule::Empty),
        (p::NameRule::Long, w::NameRule::Long),
        (p::NameRule::Control, w::NameRule::Control),
        (p::NameRule::Space, w::NameRule::Space),
        (p::NameRule::Digits, w::NameRule::Digits),
    ] {
        cases.push((
            p::Refusal::NotWellFormed { at: 0, why: p::Malformed::Name(rule) },
            w::Refusal::Malformed(
                w::RefusalMalformed::new(
                    &limits,
                    w::RefusalMalformedParts {
                        at: 0,
                        why: w::Malformed::Name(
                            w::MalformedName::new(&limits, w::MalformedNameParts { rule: expected })
                                .expect("name rule"),
                        ),
                    },
                )
                .expect("malformed"),
            ),
        ));
    }
    for (why, expected) in cases {
        let w::Final::Refused(value) = f::translated(&p::Reply::Refused(why), &limits) else { panic!("refused") };
        assert_eq!(value.refusal(), &expected);
    }
}
