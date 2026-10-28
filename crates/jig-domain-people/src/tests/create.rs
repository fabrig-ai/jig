use super::*;
const SIGN_IN: u64 = 10;
const PARTY: u64 = 1;
#[test]
fn a_members_create_routes_once_to_the_core() {
    let mut test = Test::new(LIMITS);
    test.member();
    let ask = Ask::Create { project: 1, batch: Box::new([fixtures::member(b"one")]) };
    let rows = test.request(SIGN_IN, 1, ask.clone());
    let request = route(&rows);
    assert_eq!(rows.len(), 1);
    assert!(test.request(SIGN_IN, 1, ask.clone()).is_empty());
    test.send(Event::Decided { request, outcome: Outcome::Created { first: PARTY, last: PARTY, given: None } });
    assert_eq!(
        reply(&test.request(SIGN_IN, 1, ask)),
        Reply::Outcome(Outcome::Created { first: PARTY, last: PARTY, given: None })
    );
}
#[test]
fn an_observers_create_is_refused_by_role() {
    let mut test = Test::new(LIMITS);
    test.member();
    test.send(Event::Roles { project: 1, holdings: Box::new([Holding { person: PARTY, role: Role::Observer }]) });
    let rows = test.request(SIGN_IN, 1, Ask::Create { project: 1, batch: Box::new([fixtures::member(b"one")]) });
    let request = route(&rows);
    let outcome = Outcome::Refused(Refusal::Role { held: Some(Role::Observer) });
    assert_eq!(reply(&test.send(Event::Decided { request, outcome: outcome.clone() })), Reply::Outcome(outcome));
}
#[test]
fn an_empty_create_is_refused_as_empty() {
    let mut test = Test::new(LIMITS);
    test.member();
    assert_eq!(
        reply(&test.request(SIGN_IN, 1, Ask::Create { project: 1, batch: Box::new([]) })),
        Reply::Refused(Refusal::Limit { which: LimitKind::Empty, bound: None })
    );
}
#[test]
fn a_create_past_its_batch_limit_is_refused() {
    let mut test = Test::new(LIMITS);
    test.member();
    let batch = Box::new([fixtures::member(b"one"), fixtures::member(b"two"), fixtures::member(b"three")]);
    assert_eq!(
        reply(&test.request(SIGN_IN, 1, Ask::Create { project: 1, batch })),
        Reply::Refused(Refusal::configured(
            LimitKind::Batch,
            u64::from(LIMITS.batch),
            LimitKey::PeopleBatch,
            LimitUnit::Items
        ))
    );
}
#[test]
fn a_create_whose_words_pass_their_bound_is_refused() {
    let mut test = Test::new(LIMITS);
    test.member();
    assert_eq!(
        reply(&test.request(
            SIGN_IN,
            1,
            Ask::Create { project: 1, batch: Box::new([fixtures::member(b"too many words")]) }
        )),
        Reply::Refused(Refusal::configured(
            LimitKind::Words,
            u64::from(LIMITS.words),
            LimitKey::PeopleWords,
            LimitUnit::Bytes
        ))
    );
}

#[test]
fn a_limit_refusal_carries_the_exact_words_amendment_or_fixed_name_ceiling() {
    use crate::{LimitBound as B, LimitKey as K, LimitUnit as U};
    let member = fixtures::member(b"hello");
    let member_bytes = u32::try_from(member_bytes(&member).expect("fixture measure")).expect("fixture bytes fit");
    for (words, amendment_bytes, ask, expected) in [
        (
            4,
            member_bytes,
            Ask::Create { project: 1, batch: Box::new([member.clone()]) },
            Refusal::Limit { which: LimitKind::Words, bound: Some(B::configured(4, K::PeopleWords, U::Bytes)) },
        ),
        (
            5,
            member_bytes - 1,
            Ask::Create { project: 1, batch: Box::new([member.clone()]) },
            Refusal::Limit {
                which: LimitKind::Amendment,
                bound: Some(B::configured(u64::from(member_bytes - 1), K::PeopleAmendmentBytes, U::Bytes)),
            },
        ),
        (
            5,
            member_bytes,
            Ask::MakeProject { name: "x".repeat(NAME_BYTES + 1).into_boxed_str(), policy: 1 },
            Refusal::Limit {
                which: LimitKind::Name,
                bound: Some(B::fixed(u64::try_from(NAME_BYTES).expect("name bound fits"), U::Bytes)),
            },
        ),
        (
            5,
            member_bytes,
            Ask::Create { project: 1, batch: Box::new([]) },
            Refusal::Limit { which: LimitKind::Empty, bound: None },
        ),
    ] {
        let mut limits = LIMITS;
        limits.words = words;
        limits.amendment_bytes = amendment_bytes;
        let mut test = Test::new(limits);
        test.member();
        assert_eq!(reply(&test.request(SIGN_IN, 1, ask)), Reply::Refused(expected));
    }
    let mut limits = LIMITS;
    limits.words = 5;
    limits.amendment_bytes = member_bytes;
    let mut test = Test::new(limits);
    test.member();
    assert_eq!(
        test.request(SIGN_IN, 1, Ask::Create { project: 1, batch: Box::new([member]) }).len(),
        1,
        "both byte ceilings admit equality"
    );
}
