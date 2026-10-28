use super::*;

const RESTORED_KEY: RequestKey = RequestKey { person: 1, key: [1; 16] };

fn compound(count: u32) -> Ask {
    let mut operations = List::with_capacity(count);
    for task in 1..=count {
        operations.push(Operation::Stop { task: u64::from(task) }).expect("test operation room");
    }
    Ask::Compound { project: 1, operations }
}

#[test]
fn a_compound_past_operations_per_request_is_refused() {
    let mut test = Test::new(LIMITS);
    test.signin(1, 10, identity(0, 1));
    let refused = test.request(10, 1, compound(3));
    assert_eq!(
        reply(&refused),
        Reply::Refused(Refusal::configured(
            LimitKind::Operations,
            u64::from(LIMITS.operations),
            LimitKey::PeopleOperations,
            LimitUnit::Items
        ))
    );
}

#[test]
fn an_empty_compound_is_refused() {
    let mut test = Test::new(LIMITS);
    test.signin(1, 10, identity(0, 1));
    let refused = test.request(10, 1, compound(0));
    assert_eq!(reply(&refused), Reply::Refused(Refusal::Limit { which: LimitKind::Operations, bound: None }));
}

#[test]
fn a_compound_sent_twice_under_one_key_is_routed_once() {
    let mut test = Test::new(LIMITS);
    test.signin(1, 10, identity(0, 1));
    let request = route(&test.request(10, 1, compound(2)));
    assert!(test.request(10, 1, compound(2)).is_empty());
    let outcome = Outcome::Compound { project: 1, operations: 2, first_message: 1 };
    let replies = test.send(Event::Decided { request, outcome: outcome.clone() });
    let mut answers = 0_u32;
    for row in replies {
        if let Request::Reply { reply, .. } = row {
            assert_eq!(reply, Reply::Outcome(outcome.clone()));
            answers += 1;
        }
    }
    assert_eq!(answers, 2);
    assert_eq!(reply(&test.request(10, 1, compound(2))), Reply::Outcome(outcome));
}

#[test]
fn a_made_compound_with_a_zero_first_message_cannot_restore() {
    let mut test = Test::new(LIMITS);
    test.d = Domain::new(&LIMITS, Box::new([]), 2, Box::new([]));
    let outcome = Outcome::Compound { project: 1, operations: 2, first_message: 0 };
    assert_eq!(
        test.send(Event::Restore {
            record: Stored::Answer {
                models: Box::new([]),
                key: RESTORED_KEY,
                ask: Box::new(compound(2)),
                outcome,
                at: Wall::EPOCH
            }
        }),
        [Request::RestoreRefused {
            key: Key::Answer(RESTORED_KEY),
            refusal: Refusal::Limit { which: LimitKind::Restore, bound: None }
        }],
    );
    assert!(!test.d.ready());
}

const INVALID: &[(u64, &[u8], Refusal)] = &[
    (0, b"words", Refusal::Unknown),
    (1, b"", Refusal::Limit { which: LimitKind::Empty, bound: None }),
    (1, b"too many words", Refusal::configured(LimitKind::Words, 8, LimitKey::PeopleWords, LimitUnit::Bytes)),
];
#[test]
fn a_compounds_invalid_operation_keeps_its_standalone_refusal() {
    let mut test = Test::new(LIMITS);
    test.signin(1, 10, identity(0, 1));
    for &(task, words, ref refusal) in INVALID {
        let mut operations = List::with_capacity(1);
        operations.push(Operation::Message { task, words: words.into() }).expect("one operation");
        let rows = test.request(10, 1, Ask::Compound { project: 1, operations });
        assert_eq!(reply(&rows), Reply::Refused(refusal.clone()), "task {task}, words {words:?}");
    }
}
