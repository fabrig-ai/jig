use jig_domain_tasks::{
    self as tasks, Contract, Event, Executor, FieldRule, Party, PersonAddress, ResultField, TaskResult, Verdict,
};
use jig_tasks_world::{LIMITS, Reply, World, task};

fn rules() -> Box<[FieldRule]> {
    Box::new([FieldRule { name: b"a".as_slice().into(), max: 4 }, FieldRule { name: b"b".as_slice().into(), max: 4 }])
}

fn field(name: &[u8], value: &[u8]) -> ResultField {
    ResultField { name: name.into(), value: value.into() }
}

fn answer(world: &mut World, fields: Vec<ResultField>) -> Reply {
    let reply_to = world.to();
    world.send(Event::AnswerPerson {
        reply_to,
        task: 1,
        person: 9,
        result: TaskResult::Verdict { code: 2, words: Box::new([]), fields: fields.into_boxed_slice() },
    });
    world.replies.last_key_value().expect("person answer replied").1.clone()
}

#[test]
fn verdict_contracts_refuse_empty_duplicate_and_oversized_machine_field_rules() {
    for fields in [
        vec![FieldRule { name: Box::new([]), max: 1 }],
        vec![FieldRule { name: b"a".as_slice().into(), max: 1 }, FieldRule { name: b"a".as_slice().into(), max: 1 }],
        vec![FieldRule { name: b"a".as_slice().into(), max: LIMITS.result_bytes }],
        vec![FieldRule { name: b"a".as_slice().into(), max: u32::MAX }],
    ] {
        let mut world = World::new(459, LIMITS);
        let mut new = task(1, &[]);
        new.contract = Contract::Verdict {
            choices: Box::new([Verdict { code: 2, words: 0, followups: 0, fields: fields.into_boxed_slice() }]),
        };
        assert!(matches!(world.make(Party::Person(9), vec![new]), Reply::Refused(_)));
        assert!(!world.records.contains_key(&tasks::Key::Live(1)), "nothing retained from invalid rules");
    }
}

#[test]
fn verdict_answers_require_every_closed_machine_field_once_within_its_cap() {
    let mut world = World::new(460, LIMITS);
    let mut new = task(1, &[]);
    new.executor = Executor::Person(PersonAddress::Person(9));
    new.contract =
        Contract::Verdict { choices: Box::new([Verdict { code: 2, words: 0, followups: 0, fields: rules() }]) };
    assert_eq!(world.make(Party::Person(9), vec![new]), Reply::Made(vec![1]));
    for fields in [
        vec![field(b"a", b"1")],
        vec![field(b"a", b"1"), field(b"b", b"2"), field(b"c", b"3")],
        vec![field(b"a", b"1"), field(b"a", b"2")],
        vec![field(b"a", b"1"), field(b"c", b"2")],
        vec![field(b"a", b"12345"), field(b"b", b"2")],
        vec![field(b"", b"1"), field(b"b", b"2")],
    ] {
        assert!(matches!(answer(&mut world, fields), Reply::Refused(_)));
        assert_eq!(world.record(1).person_answer, None);
    }
    let fields = vec![field(b"b", b"\x00\xff"), field(b"a", b"2")];
    assert_eq!(answer(&mut world, fields.clone()), Reply::Done);
    world.settle(1);
    assert_eq!(
        world.results[&1],
        tasks::Ending::Done(TaskResult::Verdict { code: 2, words: Box::new([]), fields: fields.into_boxed_slice() })
    );
}

// tasks.md, 11: 512 fields, independently of a larger result byte allowance.
const FIELD_CEILING: u32 = tasks::VERDICT_FIELDS;
const LARGE_RESULT_BYTES: u32 = 2048;

fn counted_rules(count: u32) -> Box<[FieldRule]> {
    (0..count).map(|name| FieldRule { name: name.to_be_bytes()[2..].into(), max: 0 }).collect()
}

fn counted_verdict(count: u32) -> tasks::New {
    let mut new = task(1, &[]);
    new.executor = Executor::Person(PersonAddress::Person(9));
    new.contract = Contract::Verdict {
        choices: Box::new([Verdict { code: 2, words: 0, followups: 0, fields: counted_rules(count) }]),
    };
    new
}

fn counted_world() -> World {
    World::new(461, tasks::Limits { result_bytes: LARGE_RESULT_BYTES, ..LIMITS })
}

#[test]
fn a_verdict_contract_admits_the_schema_field_ceiling() {
    let mut world = counted_world();
    assert_eq!(world.make(Party::Person(9), vec![counted_verdict(FIELD_CEILING)]), Reply::Made(vec![1]));
}

#[test]
fn a_verdict_contract_refuses_one_field_past_the_schema_ceiling() {
    let mut world = counted_world();
    assert!(matches!(
        world.make(Party::Person(9), vec![counted_verdict(FIELD_CEILING + 1)]),
        Reply::Refused(tasks::Problem { why: tasks::Refusal::Contract, .. })
    ));
    assert!(!world.records.contains_key(&tasks::Key::Live(1)));
}

fn counted_answer(count: u32) -> (World, Reply) {
    let mut world = counted_world();
    world.make(Party::Person(9), vec![counted_verdict(FIELD_CEILING)]);
    let fields = counted_rules(count).iter().map(|rule| field(&rule.name, &[])).collect();
    let reply = answer(&mut world, fields);
    (world, reply)
}

#[test]
fn a_verdict_result_admits_the_schema_field_ceiling() {
    let (_, reply) = counted_answer(FIELD_CEILING);
    assert_eq!(reply, Reply::Done);
}

#[test]
fn a_verdict_result_refuses_one_field_past_the_schema_ceiling() {
    let (world, reply) = counted_answer(FIELD_CEILING + 1);
    assert!(matches!(reply, Reply::Refused(tasks::Problem { why: tasks::Refusal::Contract, .. })));
    assert_eq!(world.record(1).person_answer, None);
}
