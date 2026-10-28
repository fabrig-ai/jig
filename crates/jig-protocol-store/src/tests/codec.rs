//! The notes family's translation both ways (protocol/store.md 5.1).
use super::fixtures;
use crate::codec::{accounts, component, notes};
use jig_domain_notes as domain;

const PROJECT: u32 = 1;
const CONNECTOR: u16 = 7;

fn a_notes_entry_reads_back_as_it_was_saved() {
    notes_round_trip(domain::Record::Entry(fixtures::note_entry()));
}
fn a_resources_line_reads_back_with_its_pattern() {
    notes_round_trip(domain::Record::Line(fixtures::note_line()));
}
fn a_notes_row_of_another_version_does_not_decode() {
    let limits = notes::limits(&fixtures::NOTES_LIMITS).expect("within the schema's ceilings");
    let entry = domain::Record::Entry(fixtures::note_entry());
    let mut bytes = notes::encode(&entry, &limits).expect("within its limits");
    *bytes.get_mut(1).expect("version prefix") = 2;
    assert_eq!(
        notes::decode(&bytes, &limits),
        Err(crate::notes_v1::Problem { path: crate::notes_v1::Path::RowVersion, reason: skein_codec::Reason::Version })
    );
}
fn notes_limits_past_the_ceilings_are_refused() {
    for field in 0_u8..4 {
        let mut limits = fixtures::NOTES_LIMITS;
        match field {
            0 => limits.pattern_bytes = u32::MAX,
            1 => limits.description_bytes = u32::MAX,
            2 => limits.body_bytes = u32::MAX,
            _ => limits.references = u32::MAX,
        }
        assert_eq!(notes::limits(&limits), None);
    }
}
fn the_format_row_reads_back_with_its_versions_and_deployment() {
    let row = component::Format {
        component: 1,
        core: 2,
        tasks: 3,
        people: 4,
        notes: 5,
        accounts: 8,
        settings: 9,
        application: 6,
        deployment: [7; 16],
        levels: crate::format::recorded(crate::format::NONE),
    };
    let bytes = row.encode().expect("fixed width fields fit");
    assert_eq!(component::Format::decode(&bytes), Ok(row));
    assert_eq!(component::Format::version(&bytes), Some(3));
}
fn the_installation_reads_back_and_another_version_does_not() {
    let row = accounts::Installation { id: [9; 16] };
    let mut bytes = row.encode();
    assert_eq!(bytes.len(), 18);
    assert_eq!(accounts::Installation::decode(&bytes), Ok(row));
    *bytes.get_mut(1).expect("version prefix") = 1;
    accounts::Installation::decode(&bytes).expect_err("another accounts version");
}
fn accounts_records() -> Box<[jig_domain_accounts::Stored]> {
    use jig_domain_accounts::{Held, Holder, Kind, Stored};
    let source = |kind: Kind, name: Box<[u8]>, held: Option<Held>| Stored::Source {
        account: 3,
        number: u32::MAX - 1,
        place: 7,
        kind,
        name,
        held,
    };
    Box::new([
        Stored::Issued {
            number: u32::MAX,
            holder: Holder::Party(u64::MAX),
            name: Box::new([b'p'; 32]),
            signed_out: false,
        },
        Stored::Next { number: 0 },
        Stored::Next { number: u32::MAX },
        Stored::Account {
            place: 0,
            uses: None,
            number: 3,
            provider: 2,
            holder: Holder::Deployment,
            subject: Box::new([]),
            display: Box::new([]),
        },
        Stored::Account {
            place: 0,
            uses: None,
            number: u32::MAX,
            provider: u32::MAX,
            holder: Holder::Party(u64::MAX),
            // At the child's SUBJECT and DISPLAY bounds.
            subject: Box::new([0xa5; 255]),
            display: Box::new([0x5a; 64]),
        },
        source(Kind::SignIn, Box::new([]), None),
        source(Kind::SignIn, Box::new([]), Some(Held::Moving)),
        source(Kind::Key, Box::new([]), Some(Held::Refused)),
        source(
            Kind::Program,
            Box::new([b'p'; 32]),
            Some(Held::Limited { reset: Some(skein_lib::Wall::from_nanos(u64::MAX)) }),
        ),
        source(Kind::None, Box::new([]), Some(Held::Limited { reset: None })),
    ])
}
fn each_accounts_record_reads_back_at_its_bounds() {
    for record in accounts_records() {
        let bytes = accounts::encode(&record).expect("within the child's bounds");
        assert_eq!(accounts::decode(&bytes), Ok(record));
    }
}
fn the_accounts_ceilings_are_the_childs_bounds() {
    use jig_domain_accounts::{DISPLAY, NAME, SUBJECT};
    let ceilings = crate::accounts_v2::CEILINGS;
    assert_eq!((ceilings.account_subject, ceilings.account_display, ceilings.source_name), (SUBJECT, DISPLAY, NAME));
    assert_eq!((SUBJECT, DISPLAY, NAME), (255, 64, 32), "the lengths the records at their bounds take");
    let subject = Box::new([0; 256]);
    let record = jig_domain_accounts::Stored::Account {
        place: 0,
        uses: None,
        number: 1,
        provider: 1,
        holder: jig_domain_accounts::Holder::Deployment,
        subject,
        display: Box::new([]),
    };
    accounts::encode(&record).expect_err("a subject past the child's bound");
}
fn an_accounts_record_of_another_version_or_the_installation_does_not_decode() {
    let mut bytes = accounts::encode(&jig_domain_accounts::Stored::Next { number: 9 }).expect("a counter");
    *bytes.get_mut(1).expect("version prefix") = 1;
    accounts::decode(&bytes).expect_err("another accounts version");
    accounts::decode(&accounts::Installation { id: [9; 16] }.encode()).expect_err("the installation is no record");
}
fn a_transcript_place_reads_back_with_its_window() {
    let row = component::Transcript { length: u64::MAX, window: 42 };
    assert_eq!(component::Transcript::decode(&row.encode().expect("fixed width fields fit")), Ok(row));
}
fn an_attempt_place_reads_back_with_its_first_turn() {
    let row = component::Attempt { first: u64::MAX };
    assert_eq!(component::Attempt::decode(&row.encode().expect("fixed width fields fit")), Ok(row));
}
fn a_notes_pattern_past_its_aggregate_limit_is_refused() {
    let limits = notes::limits(&fixtures::NOTES_LIMITS).expect("within ceilings");
    let mut line = fixtures::note_line();
    line.scope = domain::Scope::Resources {
        project: 3,
        connector: 2,
        pattern: domain::Pattern { segments: Box::new([Box::new([0; 16])]), last: domain::Last::Exact(Box::new([1])) },
    };
    assert_eq!(
        notes::encode(&domain::Record::Line(line), &limits),
        Err(crate::notes_v1::Problem {
            path: crate::notes_v1::Path::PatternSegments,
            reason: skein_codec::Reason::Bound,
        })
    );
}
fn a_party_authored_note_reads_back_in_each_scope() {
    let limits = notes::limits(&fixtures::NOTES_LIMITS).expect("within ceilings");
    for scope in [
        domain::Scope::Deployment,
        domain::Scope::Project { project: u32::MAX },
        domain::Scope::Resources {
            project: 3,
            connector: 2,
            pattern: domain::Pattern { segments: Box::new([]), last: domain::Last::Exact(Box::new([0, 255])) },
        },
    ] {
        let mut entry = fixtures::note_entry();
        entry.scope = scope;
        entry.author = domain::Author::Party { party: u64::MAX };
        let row = domain::Record::Entry(entry);
        assert_eq!(notes::decode(&notes::encode(&row, &limits).expect("within bounds"), &limits), Ok(row));
    }
}
fn a_decoded_pattern_past_its_aggregate_limit_is_refused() {
    let limits = notes::limits(&fixtures::NOTES_LIMITS).expect("within ceilings");
    let mut line = fixtures::note_line();
    line.scope = domain::Scope::Resources {
        project: 3,
        connector: 2,
        pattern: domain::Pattern { segments: Box::new([Box::new([0; 16])]), last: domain::Last::Exact(Box::new([1])) },
    };
    let mut wider = limits;
    wider.segment_bytes = 17;
    wider.pattern_segments = 17;
    let bytes = notes::encode(&domain::Record::Line(line), &wider).expect("within wider bound");
    assert_eq!(
        notes::decode(&bytes, &limits),
        Err(crate::notes_v1::Problem {
            path: crate::notes_v1::Path::PatternSegments,
            reason: skein_codec::Reason::Bound,
        })
    );
}

fn tasks_limits() -> crate::tasks_v9::Limits {
    crate::codec::tasks::limits(&fixtures::TASKS_LIMITS).expect("within the schema's ceilings")
}
fn saved_tasks(row: &jig_domain_tasks::Stored) -> Result<jig_domain_tasks::Stored, crate::tasks_v9::Problem> {
    let limits = tasks_limits();
    let bytes = crate::codec::tasks::encode(row, &limits, &fixtures::model_names(&[b"first", b"second", b"third"]))
        .expect("within its limits");
    crate::codec::tasks::decode(&bytes, &limits, &fixtures::model_names(&[b"first", b"second", b"third"]))
}
fn a_period_ledger_reads_back_as_it_was_saved() {
    let row = jig_domain_tasks::Stored::Ledger(fixtures::task_funding());
    assert_eq!(saved_tasks(&row), Ok(row));
}
fn a_pool_ledger_with_its_parent_reads_back() {
    let mut ledger = fixtures::task_funding();
    ledger.parent = Some(ledger.funder);
    ledger.funder = jig_domain_tasks::Funder::Pool { project: 3, person: 7, period: 5 };
    ledger.period = None;
    let row = jig_domain_tasks::Stored::Ledger(ledger);
    assert_eq!(saved_tasks(&row), Ok(row));
}
fn a_writer_slot_reads_back() {
    let row = jig_domain_tasks::Stored::Writer(fixtures::task_writer());
    assert_eq!(saved_tasks(&row), Ok(row));
}
fn an_effect_writer_slot_reads_back() {
    let mut slot = fixtures::task_writer();
    slot.writer = jig_domain_tasks::Writer::Effect { entry: u64::MAX };
    let row = jig_domain_tasks::Stored::Writer(slot);
    assert_eq!(saved_tasks(&row), Ok(row));
}
fn a_closed_ledger_reads_back() {
    let mut ledger = fixtures::task_funding();
    ledger.closed = true;
    ledger.funder = jig_domain_tasks::Funder::Recurring { project: 3, task: 17, period: 5 };
    let row = jig_domain_tasks::Stored::ClosedLedger(ledger);
    assert_eq!(saved_tasks(&row), Ok(row));
}
fn a_pool_slot_report_reads_back() {
    let row = jig_domain_tasks::Stored::Pool(jig_domain_tasks::PoolSlots {
        number: 9,
        pool: fixtures::task_name(),
        slots: 3,
    });
    assert_eq!(saved_tasks(&row), Ok(row));
}

fn saved_wire<T>(
    value: T,
    measure: fn(&T) -> u32,
    encode: fn(&T, &mut skein_lib::Writer) -> Result<(), skein_lib::Overflow>,
    decode: fn(&crate::tasks_v9::Limits, &mut skein_lib::Reader<'_>) -> Result<T, crate::tasks_v9::Problem>,
) -> T {
    let mut writer = skein_lib::Writer::new(usize::try_from(measure(&value)).expect("measured length"));
    encode(&value, &mut writer).expect("measured room");
    decode(&tasks_limits(), &mut skein_lib::Reader::new(&writer.finish())).expect("within bounds")
}
fn saved_authority(
    value: &jig_domain_tasks::Authority,
) -> Result<jig_domain_tasks::Authority, crate::tasks_v9::Problem> {
    use crate::{codec::tasks::value as translate, tasks_v9::Authority};
    let value = translate::authority(value, &tasks_limits(), None)?;
    let value = saved_wire(value, Authority::measure, Authority::encode, Authority::decode);
    translate::read_authority(value, &tasks_limits())
}
fn an_authority_with_every_grant_kind_reads_back() {
    let value = fixtures::task_authority();
    assert_eq!(saved_authority(&value), Ok(value));
}
fn an_authority_without_a_deadline_reads_back() {
    let mut value = fixtures::task_authority();
    value.budget.deadline = None;
    assert_eq!(saved_authority(&value), Ok(value));
}
fn a_specification_with_every_parameter_kind_reads_back() {
    use crate::{codec::tasks::value as translate, tasks_v9::Spec};
    let value = fixtures::task_spec();
    let limits = tasks_limits();
    let encoded = translate::spec(&value, &limits).expect("within bounds");
    let encoded = saved_wire(encoded, Spec::measure, Spec::encode, Spec::decode);
    assert_eq!(translate::read_spec(encoded, &limits), Ok(value));
}
fn each_party_kind_reads_back() {
    use crate::{codec::tasks::value as translate, tasks_v9::Party};
    for value in [
        jig_domain_tasks::Party::Task(u64::MAX),
        jig_domain_tasks::Party::Person(13),
        jig_domain_tasks::Party::Deployment { project: u32::MAX },
    ] {
        let encoded = translate::party(&value, &tasks_limits()).expect("within bounds");
        let encoded = saved_wire(encoded, Party::measure, Party::encode, Party::decode);
        assert_eq!(translate::read_party(encoded), value);
    }
}
fn each_executor_kind_reads_back() {
    use crate::{codec::tasks::value as translate, tasks_v9::Executor};
    use jig_domain_tasks::PersonAddress;
    for value in [
        jig_domain_tasks::Executor::Agent { charter: 7 },
        jig_domain_tasks::Executor::Procedure { connector: 2, code: 11 },
        jig_domain_tasks::Executor::Person(PersonAddress::Person(u64::MAX)),
        jig_domain_tasks::Executor::Person(PersonAddress::Role(u32::MAX)),
    ] {
        let encoded = translate::executor(value, &tasks_limits()).expect("within bounds");
        let encoded = saved_wire(encoded, Executor::measure, Executor::encode, Executor::decode);
        assert_eq!(translate::read_executor(encoded), value);
    }
}
fn each_failure_class_reads_back() {
    use crate::{codec::tasks::value as translate, tasks_v9::Class};
    use jig_domain_tasks::Class as Domain;
    for value in [Domain::Transient, Domain::Permanent, Domain::Run, Domain::Agent, Domain::Lost, Domain::Invalid] {
        let encoded = saved_wire(translate::class(value), Class::measure, Class::encode, Class::decode);
        assert_eq!(translate::read_class(encoded), value);
    }
}
fn each_activation_cause_reads_back() {
    use crate::{codec::tasks::value as translate, tasks_v9::Why};
    use jig_domain_tasks::{Tries, Why as Domain};
    for value in [
        Domain::Made,
        Domain::Ready,
        Domain::Message { message: u64::MAX },
        Domain::Released,
        Domain::Retry { tries: Tries { transient: 1, permanent: 2, run: 3, agent: 4, lost: 5, invalid: 6 } },
        Domain::Narrowed,
        Domain::Changed,
    ] {
        let encoded = translate::why(value, &tasks_limits()).expect("within bounds");
        let encoded = saved_wire(encoded, Why::measure, Why::encode, Why::decode);
        assert_eq!(translate::read_why(encoded), value);
    }
}
fn saved_contract(value: &jig_domain_tasks::Contract) -> Result<jig_domain_tasks::Contract, crate::tasks_v9::Problem> {
    use crate::{codec::tasks::value as translate, tasks_v9::Contract};
    let value = translate::contract(value, &tasks_limits())?;
    let value = saved_wire(value, Contract::measure, Contract::encode, Contract::decode);
    translate::read_contract(value, &tasks_limits())
}
fn each_result_contract_kind_reads_back() {
    use jig_domain_tasks::{Contract, Part, PartKey, PartKind};
    for value in [
        fixtures::task_contract(),
        Contract::Report { words: 13 },
        Contract::Change { connector: 2, kind: 3, words: 17 },
        Contract::Approval {
            state: Box::from(b"approval".as_slice()),
            part: Part { connector: 7, kind: PartKind::Effect, key: PartKey::Number(u64::MAX) },
        },
    ] {
        assert_eq!(saved_contract(&value), Ok(value));
    }
}
fn each_connector_part_kind_reads_back() {
    use crate::{codec::tasks::value as translate, tasks_v9::Part};
    use jig_domain_tasks::{PartKey, PartKind};
    for kind in [
        PartKind::Resource,
        PartKind::Pool,
        PartKind::Result,
        PartKind::Procedure,
        PartKind::Effect,
        PartKind::Outbox,
        PartKind::Hold,
    ] {
        let mut path = skein_lib::List::with_capacity(2);
        path.push(Box::from(b"a\0".as_slice())).expect("room for two segments");
        path.push(Box::from(b"b".as_slice())).expect("room for two segments");
        let value = jig_domain_tasks::Part { connector: 2, kind, key: PartKey::Path(path) };
        let encoded = translate::part(&value, &tasks_limits()).expect("within bounds");
        let encoded = saved_wire(encoded, Part::measure, Part::encode, Part::decode);
        assert_eq!(translate::read_part(encoded, &tasks_limits()), Ok(value));
    }
}

fn bound_problem(path: crate::tasks_v9::Path) -> crate::tasks_v9::Problem {
    crate::tasks_v9::Problem { path, reason: skein_codec::Reason::Bound }
}
fn spec_with_bytes(bytes: usize) -> jig_domain_tasks::Spec {
    jig_domain_tasks::Spec {
        words: Box::new([]),
        parameters: Box::new([jig_domain_tasks::Parameter::Bytes { name: 1, value: skein_lib::bytes::zeroed(bytes) }]),
        inputs: Box::new([]),
    }
}
fn a_specification_at_its_aggregate_byte_bound_reads_back() {
    use crate::{codec::tasks::value as translate, tasks_v9::Spec};
    let value = spec_with_bytes(32);
    let encoded = translate::spec(&value, &tasks_limits()).expect("at its byte bound");
    let encoded = saved_wire(encoded, Spec::measure, Spec::encode, Spec::decode);
    assert_eq!(translate::read_spec(encoded, &tasks_limits()), Ok(value));
}
fn a_specification_past_its_aggregate_byte_bound_is_refused() {
    use crate::codec::tasks::value as translate;
    let mut value = spec_with_bytes(32);
    value.words = Box::new([1]);
    assert_eq!(translate::spec(&value, &tasks_limits()), Err(bound_problem(crate::tasks_v9::Path::SpecWords)));
}
fn a_decoded_specification_past_its_aggregate_byte_bound_is_refused() {
    use crate::codec::tasks::value as translate;
    let mut value = spec_with_bytes(32);
    value.words = Box::new([1]);
    let mut wider = tasks_limits();
    wider.spec_words = 33;
    let encoded = translate::spec(&value, &wider).expect("within wider limit");
    assert_eq!(translate::read_spec(encoded, &tasks_limits()), Err(bound_problem(crate::tasks_v9::Path::SpecWords)));
}
fn authority_with_bytes(bytes: usize) -> jig_domain_tasks::Authority {
    let mut value = fixtures::task_authority();
    value.grants = Box::new([]);
    value.note_resources = Box::new([jig_domain_tasks::ResourceScope {
        connector: 1,
        pattern: jig_domain_tasks::Pattern {
            segments: Box::new([skein_lib::bytes::zeroed(bytes)]),
            last: jig_domain_tasks::Last::Exact(Box::new([])),
        },
    }]);
    value
}
fn an_authority_at_its_shared_byte_bound_reads_back() {
    let value = authority_with_bytes(32);
    assert_eq!(saved_authority(&value), Ok(value));
}
fn authority_past_shared_bytes() -> jig_domain_tasks::Authority {
    let mut value = authority_with_bytes(32);
    value.grants = Box::new([jig_domain_tasks::Grant {
        connector: 2,
        kind: 3,
        pattern: jig_domain_tasks::Pattern {
            segments: Box::new([]),
            last: jig_domain_tasks::Last::Open(Box::new([1])),
        },
    }]);
    value
}
fn grants_and_note_resources_past_their_shared_byte_bound_are_refused() {
    assert_eq!(
        crate::codec::tasks::value::authority(&authority_past_shared_bytes(), &tasks_limits(), None),
        Err(bound_problem(crate::tasks_v9::Path::AuthorityGrants)),
    );
}
fn decoded_grants_and_note_resources_past_their_shared_byte_bound_are_refused() {
    use crate::codec::tasks::value as translate;
    let mut wider = tasks_limits();
    wider.authority_segment_bytes = 33;
    let encoded = translate::authority(&authority_past_shared_bytes(), &wider, None).expect("within wider limit");
    assert_eq!(
        translate::read_authority(encoded, &tasks_limits()),
        Err(bound_problem(crate::tasks_v9::Path::AuthorityGrants))
    );
}
fn writer_with_path_bytes(bytes: usize) -> jig_domain_tasks::Stored {
    let mut slot = fixtures::task_writer();
    slot.resource.path = Box::new([skein_lib::bytes::zeroed(bytes), Box::new([1])]);
    jig_domain_tasks::Stored::Writer(slot)
}
fn a_writer_resource_at_its_aggregate_byte_bound_reads_back() {
    let row = writer_with_path_bytes(15);
    assert_eq!(saved_tasks(&row), Ok(row));
}
fn a_writer_resource_past_its_aggregate_byte_bound_is_refused() {
    assert_eq!(
        crate::codec::tasks::encode(
            &writer_with_path_bytes(16),
            &tasks_limits(),
            &fixtures::model_names(&[b"first", b"second", b"third"])
        ),
        Err(bound_problem(crate::tasks_v9::Path::NamePath))
    );
}
fn a_decoded_writer_resource_past_its_aggregate_byte_bound_is_refused() {
    let mut wider = tasks_limits();
    wider.segment_bytes = 17;
    let bytes = crate::codec::tasks::encode(
        &writer_with_path_bytes(16),
        &wider,
        &fixtures::model_names(&[b"first", b"second", b"third"]),
    )
    .expect("within wider limit");
    assert_eq!(
        crate::codec::tasks::decode(&bytes, &tasks_limits(), &fixtures::model_names(&[b"first", b"second", b"third"])),
        Err(bound_problem(crate::tasks_v9::Path::NamePath))
    );
}
fn a_tasks_row_of_another_version_does_not_decode() {
    let limits = tasks_limits();
    let row = jig_domain_tasks::Stored::Ledger(fixtures::task_funding());
    let mut bytes =
        crate::codec::tasks::encode(&row, &limits, &fixtures::model_names(&[b"first", b"second", b"third"]))
            .expect("within bounds");
    *bytes.get_mut(1).expect("version prefix") = 5;
    assert_eq!(
        crate::codec::tasks::decode(&bytes, &limits, &fixtures::model_names(&[b"first", b"second", b"third"])),
        Err(crate::tasks_v9::Problem { path: crate::tasks_v9::Path::RowVersion, reason: skein_codec::Reason::Version })
    );
}
fn task_funded_accounting_reads_back() {
    let mut value = fixtures::task_funding();
    value.funder = jig_domain_tasks::Funder::Task(u64::MAX);
    let row = jig_domain_tasks::Stored::Ledger(value);
    assert_eq!(saved_tasks(&row), Ok(row));
}

fn tasks_limits_past_the_ceilings_are_refused() {
    for field in 0_u8..11 {
        let mut limits = fixtures::TASKS_LIMITS;
        match field {
            0 => limits.hold_bytes = u32::MAX,
            1 => limits.hold_segments = u32::MAX,
            2 => limits.spec_bytes = u32::MAX,
            3 => limits.parameters = u32::MAX,
            4 => limits.inputs = u32::MAX,
            5 => limits.result_bytes = u32::MAX,
            6 => limits.contract_choices = u32::MAX,
            7 => limits.authority_bytes = u32::MAX,
            8 => limits.authority_segments = u32::MAX,
            9 => limits.executor_kinds = u32::MAX,
            _ => limits.authority_grants = u32::MAX,
        }
        assert_eq!(crate::codec::tasks::limits(&limits), None);
    }
}
fn verdict_rules_at_their_aggregate_byte_bound_read_back() {
    let value = fixtures::task_contract();
    assert_eq!(saved_contract(&value), Ok(value));
}
fn contract_past_shared_bytes() -> jig_domain_tasks::Contract {
    jig_domain_tasks::Contract::Verdict {
        choices: Box::new([jig_domain_tasks::Verdict {
            code: 7,
            words: 17,
            followups: 3,
            fields: Box::new([jig_domain_tasks::FieldRule { name: Box::from(b"field".as_slice()), max: 11 }]),
        }]),
    }
}
fn verdict_rules_past_their_aggregate_byte_bound_are_refused() {
    assert_eq!(
        crate::codec::tasks::value::contract(&contract_past_shared_bytes(), &tasks_limits()),
        Err(bound_problem(crate::tasks_v9::Path::VerdictFields))
    );
}
fn decoded_verdict_rules_past_their_aggregate_byte_bound_are_refused() {
    use crate::codec::tasks::value as translate;
    let mut wider = tasks_limits();
    wider.field_rule_name = 33;
    let encoded = translate::contract(&contract_past_shared_bytes(), &wider).expect("within wider limit");
    assert_eq!(
        translate::read_contract(encoded, &tasks_limits()),
        Err(bound_problem(crate::tasks_v9::Path::VerdictFields))
    );
}

mod phase;

fn core_limits() -> jig_domain_core::Limits {
    fixtures::CORE_LIMITS
}

fn people_limits() -> crate::people_v13::Limits {
    crate::codec::people::limits(&core_limits()).expect("within people's ceilings")
}

fn people_round_trip(row: jig_domain_people::Stored) {
    let limits = people_limits();
    let names = fixtures::model_names(&[b"first", b"second", b"third"]);
    let bytes = crate::codec::people::encode(&row, &limits, &names).expect("within people's bounds");
    assert_eq!(crate::codec::people::decode(&bytes, &limits, &names), Ok(row));
}

fn ask_round_trip(ask: jig_domain_people::Ask) {
    let limits = people_limits();
    let value = crate::codec::people::encode_ask(&ask, &limits).expect("bounded ask");
    let mut writer = skein_lib::Writer::new(usize::try_from(value.measure()).expect("bounded length"));
    value.encode(&mut writer).expect("measured room");
    let bytes = writer.finish();
    let read = crate::people_v13::Ask::decode(&limits, &mut skein_lib::Reader::new(&bytes)).expect("saved ask");
    assert_eq!(crate::codec::people::decode_ask(read, &limits), Ok(ask));
}

fn a_person_reads_back_with_its_identity() {
    people_round_trip(jig_domain_people::Stored::Person { number: u64::MAX, identity: fixtures::identity() });
}

fn a_projects_policy_reads_back() {
    people_round_trip(jig_domain_people::Stored::Policy {
        project: 3,
        name: Box::from("Tally"),
        value: {
            let mut value = fixtures::people_policy();
            value.ceiling.models = Some(jig_domain_people::Models::Named(1));
            value
        },
    });
}

fn a_roles_models_follow_their_names_to_todays_configuration() {
    let limits = people_limits();
    let mut value = fixtures::people_policy();
    value.roles[0].authority.models = Some(jig_domain_people::Models::Named(0b111));
    value.ceiling.models = Some(jig_domain_people::Models::Named(0b011));
    let row = jig_domain_people::Stored::Policy { project: 3, name: Box::from("Tally"), value: value.clone() };
    let then = fixtures::model_names(&[b"first", b"second", b"third"]);
    let bytes = crate::codec::people::encode(&row, &limits, &then).expect("within people's bounds");
    // "third" moved to the front, "second" is no longer configured, "fourth" is new.
    let now = fixtures::model_names(&[b"third", b"fourth", b"first"]);
    value.roles[0].authority.models = Some(jig_domain_people::Models::Named(0b101));
    value.ceiling.models = Some(jig_domain_people::Models::Named(0b100));
    assert_eq!(
        crate::codec::people::decode(&bytes, &limits, &now),
        Ok(jig_domain_people::Stored::Policy { project: 3, name: Box::from("Tally"), value })
    );
}

fn a_role_naming_every_model_keeps_every_model() {
    let limits = people_limits();
    let mut value = fixtures::people_policy();
    value.roles[0].authority.models = Some(jig_domain_people::Models::ALL);
    let row = jig_domain_people::Stored::Policy { project: 3, name: Box::from("Tally"), value };
    let bytes = crate::codec::people::encode(&row, &limits, &fixtures::model_names(&[b"first"])).expect("bounded");
    let now = fixtures::model_names(&[b"second", b"first"]);
    assert_eq!(crate::codec::people::decode(&bytes, &limits, &now), Ok(row));
}

fn a_policy_larger_than_one_amendment_reads_back() {
    let mut value = fixtures::people_policy();
    let first = value.roles[0].clone();
    let mut second = first.clone();
    second.number = 5;
    value.roles = Box::new([first, second]);
    people_round_trip(jig_domain_people::Stored::Policy { project: 3, name: Box::from("Tally"), value });
}

#[test]
fn shared_people_fields_take_the_larger_policy_bound() {
    let mut domain = core_limits();
    domain.people.amendment_bytes = 16;
    domain.authority.segment_bytes = 32;
    domain.authority.segments = 3;
    let limits = crate::codec::people::limits(&domain).expect("within ceilings");
    assert_eq!(limits.segment_bytes, 32);
    assert_eq!(limits.pattern_segments, 8184);
}

#[test]
fn each_kind_of_ask_reads_back() {
    for ask in fixtures::people_asks() {
        ask_round_trip(ask);
    }
}

fn a_read_conversation_position_reads_back() {
    people_round_trip(jig_domain_people::Stored::ReadConversation { party: 3, task: 4, through: u64::MAX });
}

fn a_sign_in_reads_back_with_its_wall_expiry() {
    people_round_trip(jig_domain_people::Stored::SignIn {
        number: 2,
        person: 3,
        expires: skein_lib::Wall::from_nanos(u64::MAX),
    });
}

fn every_role_reads_back_in_a_roster() {
    use jig_domain_people::{Holding, Role, Stored};
    for role in [Role::Owner, Role::Maintainer, Role::Member, Role::Observer, Role::Policy { role: u32::MAX }] {
        people_round_trip(Stored::Roles { project: 3, holdings: Box::new([Holding { person: 4, role }]) });
    }
}

fn a_read_results_position_reads_back() {
    people_round_trip(jig_domain_people::Stored::ReadResults { party: 3, through: u64::MAX });
}

fn people_limits_past_their_ceilings_are_refused() {
    let mut limits = core_limits();
    limits.people.amendment_bytes = u32::MAX;
    assert_eq!(crate::codec::people::limits(&limits), None);
}

#[test]
fn a_person_identity_field_past_its_limit_is_refused() {
    let mut identity = fixtures::identity();
    identity.key.subject = Box::new([0; 33]);
    assert_eq!(
        crate::codec::people::encode(
            &jig_domain_people::Stored::Person { number: 1, identity },
            &people_limits(),
            &fixtures::model_names(&[])
        ),
        Err(crate::people_v13::Problem {
            path: crate::people_v13::Path::IdentityKeySubject,
            reason: skein_codec::Reason::Bound
        })
    );
}

fn a_people_row_of_another_version_does_not_decode() {
    let limits = people_limits();
    let row = jig_domain_people::Stored::ReadResults { party: 1, through: 2 };
    let names = fixtures::model_names(&[]);
    let mut bytes = crate::codec::people::encode(&row, &limits, &names).expect("fixed row");
    *bytes.get_mut(1).expect("version prefix") = 9;
    assert_eq!(
        crate::codec::people::decode(&bytes, &limits, &names),
        Err(crate::people_v13::Problem {
            path: crate::people_v13::Path::RowVersion,
            reason: skein_codec::Reason::Version
        })
    );
}

mod record;
fn call_limits() -> crate::codec::core::Limits {
    crate::codec::core::limits(&core_limits()).expect("within core ceilings")
}
fn core_round_trip(row: jig_domain_core::CoreRecord) {
    let limits = call_limits();
    let bytes = crate::codec::core::encode(&row, &limits).expect("within core bounds");
    assert_eq!(crate::codec::core::decode(&bytes, &limits), Ok(row));
}
fn a_call_record_of_each_part_reads_back() {
    for part in fixtures::calls::call_parts() {
        core_round_trip(fixtures::calls::call(part));
    }
}
fn a_refused_delegation_reads_back_with_its_problem() {
    core_round_trip(fixtures::calls::call(jig_domain_core::CallPart::DelegationRefused(fixtures::calls::problem())));
}
fn a_refused_llm_choice_reads_back_with_its_model_and_fault() {
    use jig_domain_tasks::LlmFault;
    for why in [
        LlmFault::Unconfigured,
        LlmFault::Effort,
        LlmFault::Charter,
        LlmFault::Authority,
        LlmFault::Procedure,
        LlmFault::Repeated,
    ] {
        let mut problem = fixtures::calls::problem();
        problem.why = jig_domain_tasks::Refusal::Llm { model: 63, why };
        core_round_trip(fixtures::calls::call(jig_domain_core::CallPart::DelegationRefused(problem)));
    }
}
fn a_recalled_notes_page_reads_back() {
    core_round_trip(fixtures::calls::call(jig_domain_core::CallPart::NoteRecalled {
        entries: Box::new([fixtures::note_entry()]),
        more: true,
    }));
}
fn a_settled_call_reads_back() {
    use jig_domain_core as core;
    for answer in [
        core::SettledAnswer::Host { error: true, body: Box::from(b"denied".as_slice()) },
        core::SettledAnswer::Delivery {
            outcome: core::DeliveryOutcome::Stale,
            evidence: Box::from(b"receipt".as_slice()),
        },
    ] {
        let core::CoreRecord::Call(mut row) = fixtures::calls::call(core::CallPart::Unavailable) else {
            panic!("call fixture")
        };
        row.settled = Some(core::SettledCall { serial: 1, tool: Box::from(b"tool".as_slice()), answer });
        core_round_trip(core::CoreRecord::Call(row));
    }
}
fn a_finished_terminal_reads_back_with_its_followup_batch() {
    use jig_domain_tasks as tasks;
    let end = tasks::End::FinishedWithFollowups {
        result: tasks::TaskResult::Report { words: Box::from(b"finished".as_slice()) },
        followups: tasks::ResultFollowups::Delegates(Box::new([fixtures::task_creation(2)])),
    };
    end_round_trip(end);
}

fn a_finished_terminal_reads_back_with_its_followup_proposal() {
    use jig_domain_tasks as tasks;
    let end = tasks::End::FinishedWithFollowups {
        result: tasks::TaskResult::Report { words: Box::from(b"finished".as_slice()) },
        followups: tasks::ResultFollowups::Proposal(Box::new(fixtures::task_proposal())),
    };
    end_round_trip(end);
}

fn a_carried_phase_reads_back_as_its_own_family() {
    let phase = fixtures::task_phase();
    let limits = call_limits();
    let bytes = crate::codec::tasks::carried::phase(&phase, &limits.tasks).expect("phase within bounds");
    assert_eq!(crate::codec::tasks::carried::read_phase(&bytes, &limits.tasks), Ok(phase));
}

fn a_carried_change_reads_back_as_its_own_family() {
    let change = jig_domain_tasks::Change::Amended;
    let limits = call_limits();
    let bytes = crate::codec::tasks::carried::change(&change, &limits.tasks).expect("change within bounds");
    assert_eq!(crate::codec::tasks::carried::read_change(&bytes, &limits.tasks), Ok(change));
}

fn core_limits_past_the_settled_answer_ceiling_are_refused() {
    let mut domain = core_limits();
    domain.call_answer_bytes = crate::core_v6::CEILINGS.settled_answer_host_body + 1;
    assert!(crate::codec::core::limits(&domain).is_none());
}

fn end_round_trip(end: jig_domain_tasks::End) {
    let limits = call_limits();
    let bytes = crate::codec::tasks::carried::end(&end, &limits.tasks).expect("terminal within bounds");
    assert_eq!(crate::codec::tasks::carried::read_end(&bytes, &limits.tasks), Ok(end));
}

mod core_rows;

mod people_outcomes;

fn task_ledgers_read_back() {
    table([
        ("a_period_ledger_reads_back_as_it_was_saved", a_period_ledger_reads_back_as_it_was_saved),
        ("a_pool_ledger_with_its_parent_reads_back", a_pool_ledger_with_its_parent_reads_back),
        ("a_closed_ledger_reads_back", a_closed_ledger_reads_back),
        ("task_funded_accounting_reads_back", task_funded_accounting_reads_back),
    ]);
}

fn writer_slots_read_back() {
    table([
        ("a_writer_slot_reads_back", a_writer_slot_reads_back),
        ("an_effect_writer_slot_reads_back", an_effect_writer_slot_reads_back),
    ]);
}

fn authorities_read_back() {
    table([
        ("an_authority_with_every_grant_kind_reads_back", an_authority_with_every_grant_kind_reads_back),
        ("an_authority_without_a_deadline_reads_back", an_authority_without_a_deadline_reads_back),
        ("an_authority_at_its_shared_byte_bound_reads_back", an_authority_at_its_shared_byte_bound_reads_back),
    ]);
}

fn notes_rows_read_back_as_saved() {
    table([
        ("a_notes_entry_reads_back_as_it_was_saved", a_notes_entry_reads_back_as_it_was_saved),
        ("a_resources_line_reads_back_with_its_pattern", a_resources_line_reads_back_with_its_pattern),
    ]);
}

type Case = (&'static str, fn());

fn table<const N: usize>(cases: [Case; N]) {
    let mut failures = [None; N];
    let mut passed = true;
    for ((name, case), failure) in cases.into_iter().zip(&mut failures) {
        if std::panic::catch_unwind(case).is_err() {
            *failure = Some(name);
            passed = false;
        }
    }
    assert!(passed, "{failures:?}");
}

fn notes_round_trip(row: domain::Record) {
    let limits = notes::limits(&fixtures::NOTES_LIMITS).expect("within the schema's ceilings");
    let bytes = notes::encode(&row, &limits).expect("within its limits");
    assert_eq!(notes::decode(&bytes, &limits), Ok(row));
}

mod pieces;

mod cases;

#[test]
fn a_moved_call_part_round_trips_through_the_core_family() {
    use jig_domain_people::{HoldReason, Moved};
    for now in [Moved::Spec { revision: 2 }, Moved::Hold { revision: 3, reason: HoldReason::Budget }] {
        core_round_trip(fixtures::calls::call(jig_domain_core::CallPart::Moved { task: 1, now }));
    }
}

#[test]
fn a_revised_call_part_round_trips_through_the_core_family() {
    core_round_trip(fixtures::calls::call(jig_domain_core::CallPart::Revised { task: 1, revision: 2 }));
}

#[test]
fn an_application_ask_round_trips_each_scope_through_the_people_family() {
    use jig_domain_people::{ApplicationScope, Ask};
    for scope in [ApplicationScope::Project(PROJECT), ApplicationScope::Deployment, ApplicationScope::Own] {
        ask_round_trip(Ask::Application { scope, connector: CONNECTOR, digest: [8; 32] });
    }
}

#[test]
fn an_account_with_place_and_uses_round_trips() {
    let record = jig_domain_accounts::Stored::Account {
        number: 17,
        provider: 2,
        holder: jig_domain_accounts::Holder::Party(3),
        place: 4,
        uses: Some(Box::new([jig_domain_accounts::Use::Party(3), jig_domain_accounts::Use::Project(9)])),
        subject: Box::new([]),
        display: Box::new([]),
    };
    let bytes = accounts::encode(&record).expect("bounded account");
    assert_eq!(accounts::decode(&bytes), Ok(record));
}

#[test]
fn every_account_change_and_answer_round_trips() {
    use jig_domain_people::{AccountChange as C, AccountHolder, Ask, Outcome, RequestKey, Stored, Use};
    let changes = [
        C::AddKey { provider: 3, join: Some(2), digest: [7; 32] },
        C::ReplaceKey { source: 1, digest: [8; 32] },
        C::RemoveKey { source: 1 },
        C::Resume { source: 1 },
        C::Remove { account: 2 },
        C::MoveAccount { account: 2, place: 1 },
        C::MoveSource { account: 2, source: 1, place: 0 },
        C::SetUses { account: 2, uses: Some(Box::new([Use::Party(1), Use::Project(1)])) },
        C::SetUses { account: 2, uses: None },
        C::SetUses { account: 2, uses: Some(Box::new([])) },
    ];
    for holder in [AccountHolder::Own, AccountHolder::Deployment] {
        for change in &changes {
            let ask = Ask::AccountChange { holder, change: change.clone() };
            ask_round_trip(ask.clone());
            for outcome in [
                Outcome::AccountKeyKept { source: 1 },
                Outcome::AccountChanged,
                Outcome::AccountRemoved { confirmed: true, failed_hosts: Box::new([]) },
                Outcome::AccountRemoved { confirmed: false, failed_hosts: Box::new([]) },
            ] {
                people_round_trip(Stored::Answer {
                    models: Box::new([]),
                    key: RequestKey { person: 1, key: [2; 16] },
                    ask: Box::new(ask.clone()),
                    outcome,
                    at: skein_lib::Wall::from_nanos(3),
                });
            }
        }
    }
}

#[test]
fn the_providers_rule_refusal_round_trips() {
    people_round_trip(fixtures::people_outcomes::answered(jig_domain_people::Outcome::Refused(
        jig_domain_people::Refusal::Provider,
    )));
}

#[test]
fn an_llm_key_answer_keeps_its_digest_without_the_value() {
    use jig_domain_people::{AccountChange, AccountHolder, Ask, Outcome, RequestKey, Stored};
    const VALUE: &[u8] = b"raw-llm-key-value-never-committed";
    let row = Stored::Answer {
        models: Box::new([]),
        key: RequestKey { person: 1, key: [2; 16] },
        ask: Box::new(Ask::AccountChange {
            holder: AccountHolder::Own,
            change: AccountChange::AddKey { provider: 3, join: None, digest: [7; 32] },
        }),
        outcome: Outcome::AccountKeyKept { source: 2 },
        at: skein_lib::Wall::EPOCH,
    };
    let limits = people_limits();
    let names = fixtures::model_names(&[b"first", b"second", b"third"]);
    let bytes = crate::codec::people::encode(&row, &limits, &names).expect("bounded people row");
    for window in bytes.windows(VALUE.len()) {
        assert_ne!(window, VALUE);
    }
    assert_eq!(crate::codec::people::decode(&bytes, &limits, &names), Ok(row));
}

#[test]
fn run_provider_bindings_round_trip_at_the_bound_and_refuse_one_more() {
    use jig_domain_authority::AccountBinding;
    use jig_domain_core::CoreRecord;
    const BINDINGS: u32 = jig_domain_tasks::MODELS;
    let limits = call_limits();
    let mut proof = fixtures::core_rows::run();
    let mut bindings = skein_lib::List::with_capacity(BINDINGS + 1);
    for provider in 0..BINDINGS {
        bindings.push(AccountBinding { provider, account: provider, source: provider + BINDINGS }).unwrap();
    }
    proof.accounts = bindings.as_slice().into();
    core_round_trip(CoreRecord::RunProof(proof.clone()));
    bindings.push(AccountBinding { provider: BINDINGS, account: 0, source: 0 }).unwrap();
    proof.accounts = bindings.into_boxed();
    crate::codec::core::encode(&CoreRecord::RunProof(proof), &limits).expect_err("one past the provider bound");
}

#[test]
fn nested_retained_requests_keep_their_original_names_after_reconfiguration() {
    use jig_domain_people as p;
    let limits = people_limits();
    let mut authority = fixtures::people_authority();
    authority.models = Some(p::Models::Named(1));
    let mut member = fixtures::people_member();
    member.authority = authority.clone();
    let mut amendment = fixtures::people_amendment();
    amendment.authority = Some(authority.clone());
    let mut operations = skein_lib::List::with_capacity(1);
    operations
        .push(p::Operation::Amend { task: 4, revision: 1, amendment: Box::new(amendment.clone()), release: None })
        .unwrap();
    let asks = [
        p::Ask::Create { project: 3, batch: Box::new([member.clone()]) },
        p::Ask::Amend { project: 3, task: 4, revision: 1, amendment: amendment.clone(), release: None },
        p::Ask::Propose { project: 3, action: p::Action::Batch(Box::new([member])), reason: Box::new([]) },
        p::Ask::Propose {
            project: 3,
            action: p::Action::Amend { task: 4, revision: 1, amendment: Box::new(amendment.clone()) },
            reason: Box::new([]),
        },
        p::Ask::Compound { project: 3, operations },
        p::Ask::ChangePolicy {
            project: 3,
            change: p::PolicyChange::Role(p::PolicyRole {
                number: 4,
                authority,
                period_spend: 1,
                requests: 0,
                decides: 0,
            }),
        },
    ];
    let then = fixtures::model_names(&[b"first", b"second"]);
    for ask in asks {
        let row = p::Stored::Answer {
            key: p::RequestKey { person: 1, key: [2; 16] },
            ask: Box::new(ask),
            models: fixtures::model_names(&[b"first"]),
            outcome: p::Outcome::Refused(p::Refusal::Unknown),
            at: skein_lib::Wall::EPOCH,
        };
        let bytes = crate::codec::people::encode(&row, &limits, &then).unwrap();
        for now in [fixtures::model_names(&[b"second", b"first"]), fixtures::model_names(&[b"second", b"new"])] {
            assert_eq!(crate::codec::people::decode(&bytes, &limits, &now), Ok(row.clone()));
        }
    }
}
#[test]
fn a_named_full_mask_drops_removed_models_without_becoming_all() {
    let limits = people_limits();
    let mut value = fixtures::people_policy();
    value.roles[0].authority.models = Some(jig_domain_people::Models::Named(u64::MAX));
    let mut names = skein_lib::List::with_capacity(64);
    for at in 0_u8..64 {
        names.push(Box::<[u8]>::from([b'0'.checked_add(at).expect("ASCII name")])).expect("64 names");
    }
    let names = names.into_boxed();
    let row = jig_domain_people::Stored::Policy { project: 3, name: Box::from("Tally"), value: value.clone() };
    let bytes = crate::codec::people::encode(&row, &limits, &names).unwrap();
    let now = fixtures::model_names(&[b"new", b"o", b"0"]);
    value.roles[0].authority.models = Some(jig_domain_people::Models::Named(0b110));
    assert_eq!(
        crate::codec::people::decode(&bytes, &limits, &now),
        Ok(jig_domain_people::Stored::Policy { project: 3, name: Box::from("Tally"), value })
    );
}

#[test]
fn pending_policy_proposal_members_follow_names_on_restore() {
    use jig_domain_tasks as t;
    let limits = tasks_limits();
    let mut proposal = fixtures::task_person_proposal();
    let t::PersonAction::Batch(batch) = &mut proposal.action else { panic!("batch") };
    batch[0].authority.models = t::Models::Named(0b101);
    let row = t::Stored::PersonProposal(Box::new(proposal.clone()));
    let then = fixtures::model_names(&[b"first", b"second", b"third"]);
    let now = fixtures::model_names(&[b"third", b"first", b"new"]);
    let bytes = crate::codec::tasks::encode(&row, &limits, &then).unwrap();
    let mut reordered = proposal.clone();
    let t::PersonAction::Batch(batch) = &mut reordered.action else { panic!("batch") };
    batch[0].authority.models = t::Models::Named(0b011);
    assert_eq!(crate::codec::tasks::decode(&bytes, &limits, &now), Ok(t::Stored::PersonProposal(Box::new(reordered))));
    let removed = fixtures::model_names(&[b"new", b"third"]);
    let t::PersonAction::Batch(batch) = &mut proposal.action else { panic!("batch") };
    batch[0].authority.models = t::Models::Named(0b10);
    assert_eq!(
        crate::codec::tasks::decode(&bytes, &limits, &removed),
        Ok(t::Stored::PersonProposal(Box::new(proposal)))
    );
}

#[test]
fn unrelated_tasks_rows_keep_no_model_names_or_configuration_bytes() {
    use jig_domain_tasks as t;
    let limits = tasks_limits();
    let names = fixtures::model_names(&[b"unreferenced-model", b"another-model"]);
    for row in [
        t::Stored::Ledger(fixtures::task_funding()),
        t::Stored::ClosedLedger(fixtures::task_funding()),
        t::Stored::Writer(fixtures::task_writer()),
        t::Stored::Pool(t::PoolSlots { number: 1, pool: fixtures::task_name(), slots: 1 }),
        t::Stored::Delegate(t::Edge { requester: 1, task: 2 }),
    ] {
        let bytes = crate::codec::tasks::encode(&row, &limits, &names).unwrap();
        assert_eq!(bytes, crate::codec::tasks::encode(&row, &limits, &[]).unwrap());
        let wire = crate::tasks_v9::Row::decode(&limits, &mut skein_lib::Reader::new(&bytes)).unwrap();
        assert!(wire.models().is_empty());
        assert_eq!(crate::codec::tasks::decode(&bytes, &limits, &names), Ok(row));
    }
}
#[test]
fn tasks_context_contains_only_canonical_referenced_names_with_reindexed_selectors() {
    use jig_domain_tasks as t;
    let limits = tasks_limits();
    let mut same = None;
    for (names, mask) in [
        (fixtures::model_names(&[b"z", b"unused", b"a"]), 0b101),
        (fixtures::model_names(&[b"a", b"z", b"unused"]), 0b011),
        (fixtures::model_names(&[b"unused", b"a", b"z"]), 0b110),
    ] {
        let mut proposal = fixtures::task_person_proposal();
        let t::PersonAction::Batch(batch) = &mut proposal.action else { panic!("batch") };
        batch[0].authority.models = t::Models::Named(mask);
        let row = t::Stored::PersonProposal(Box::new(proposal));
        let bytes = crate::codec::tasks::encode(&row, &limits, &names).unwrap();
        let wire = crate::tasks_v9::Row::decode(&limits, &mut skein_lib::Reader::new(&bytes)).unwrap();
        assert_eq!(wire.models().len(), 2);
        assert_eq!(wire.models().get(0).unwrap().value(), b"a");
        assert_eq!(wire.models().get(1).unwrap().value(), b"z");
        if let Some(same) = &same {
            assert_eq!(&bytes, same);
        } else {
            same = Some(bytes.clone());
        }
        assert_eq!(crate::codec::tasks::decode(&bytes, &limits, &names), Ok(row));
    }
    let mut proposal = fixtures::task_person_proposal();
    for selector in [t::Models::ALL, t::Models::NONE] {
        let t::PersonAction::Batch(batch) = &mut proposal.action else { panic!("batch") };
        batch[0].authority.models = selector;
        let row = t::Stored::PersonProposal(Box::new(proposal.clone()));
        let names = fixtures::model_names(&[b"z", b"unused", b"a"]);
        let bytes = crate::codec::tasks::encode(&row, &limits, &names).unwrap();
        let wire = crate::tasks_v9::Row::decode(&limits, &mut skein_lib::Reader::new(&bytes)).unwrap();
        assert!(wire.models().is_empty());
        assert_eq!(crate::codec::tasks::decode(&bytes, &limits, &names), Ok(row));
    }
}

#[test]
fn tasks_named_all_64_keeps_its_names_and_literal_all_keeps_none() {
    use jig_domain_tasks as t;
    let limits = tasks_limits();
    let mut names = skein_lib::List::with_capacity(64);
    for at in 0_u8..64 {
        let mut name = skein_lib::bytes::zeroed(usize::try_from(jig_domain_core::MODEL_NAME_BYTES).unwrap());
        name.fill(b'0'.checked_add(at).unwrap());
        names.push(name).unwrap();
    }
    let names = names.into_boxed();
    let mut encoded = skein_lib::List::with_capacity(2);
    for selector in [t::Models::ALL, t::Models::Named(u64::MAX)] {
        let mut proposal = fixtures::task_person_proposal();
        let t::PersonAction::Batch(batch) = &mut proposal.action else { panic!("batch") };
        batch[0].authority.models = selector;
        let row = t::Stored::PersonProposal(Box::new(proposal));
        let bytes = crate::codec::tasks::encode(&row, &limits, &names).unwrap();
        let wire = crate::tasks_v9::Row::decode(&limits, &mut skein_lib::Reader::new(&bytes)).unwrap();
        assert_eq!(wire.models().len(), if selector.is_all() { 0 } else { 64 });
        assert!(u64::try_from(bytes.len()).unwrap() <= crate::tasks_v9::Row::worst_case_bytes(&limits).unwrap());
        assert_eq!(crate::codec::tasks::decode(&bytes, &limits, &names), Ok(row));
        let now = Box::new([names[0].clone(), Box::<[u8]>::from(b"new".as_slice())]);
        let decoded = crate::codec::tasks::decode(&bytes, &limits, now.as_ref()).unwrap();
        let t::Stored::PersonProposal(proposal) = decoded else { panic!("proposal") };
        let t::PersonAction::Batch(batch) = proposal.action else { panic!("batch") };
        assert_eq!(batch[0].authority.models, if selector.is_all() { t::Models::ALL } else { t::Models::Named(1) });
        encoded.push(bytes.len()).unwrap();
    }
    assert_eq!(
        *encoded.get(1).unwrap() - *encoded.get(0).unwrap(),
        64 * (4 + usize::try_from(jig_domain_core::MODEL_NAME_BYTES).unwrap())
    );
}

#[test]
fn canonical_tasks_context_reindexes_live_recurring_proposals_and_history() {
    use jig_domain_tasks as t;
    let limits = tasks_limits();
    let names = fixtures::model_names(&[b"z", b"unused", b"a"]);
    let mut task = fixtures::task_record();
    task.authority.models = t::Models::Named(1);
    task.recurring.as_mut().unwrap().template.batch[0].authority.models = t::Models::Named(4);
    task.proposal.as_mut().unwrap().action =
        t::ProposalAction::Widen { task: 17, authority: fixtures::task_authority() };
    let t::ProposalAction::Widen { authority, .. } = &mut task.proposal.as_mut().unwrap().action else {
        panic!("widen")
    };
    authority.models = t::Models::Named(5);
    let mut history = fixtures::task_history();
    history.amendment.as_mut().unwrap().before.authority.models = t::Models::Named(1);
    history.amendment.as_mut().unwrap().after.authority.models = t::Models::Named(4);
    let mut proposal = fixtures::task_person_proposal();
    let mut amendment = fixtures::task_amendment();
    amendment.authority.as_mut().unwrap().models = t::Models::Named(5);
    proposal.action = t::PersonAction::Amend { task: 17, revision: 3, amendment: Box::new(amendment) };
    for row in [
        t::Stored::Live(Box::new(task.clone())),
        t::Stored::Ended(Box::new(task)),
        t::Stored::History(history),
        t::Stored::PersonProposal(Box::new(proposal.clone())),
        t::Stored::DecidedProposal(Box::new(proposal)),
    ] {
        let bytes = crate::codec::tasks::encode(&row, &limits, &names).unwrap();
        let wire = crate::tasks_v9::Row::decode(&limits, &mut skein_lib::Reader::new(&bytes)).unwrap();
        assert_eq!(wire.models().len(), 2);
        assert_eq!(wire.models().get(0).unwrap().value(), b"a");
        assert_eq!(wire.models().get(1).unwrap().value(), b"z");
        assert_eq!(crate::codec::tasks::decode(&bytes, &limits, &names), Ok(row));
    }
}

#[test]
fn a_people_choice_keeps_its_explicit_account() {
    use jig_domain_people as p;
    let mut member = fixtures::people_member();
    member.llm = Some(p::LlmChoice {
        account: Some(42),
        main: p::ModelEffort { model: 0, effort: None },
        subagents: Box::new([]),
    });
    let ask = p::Ask::Create { project: 1, batch: Box::new([member]) };
    ask_round_trip(ask.clone());
    people_round_trip(p::Stored::Answer {
        models: Box::new([]),
        key: p::RequestKey { person: 2, key: [9; 16] },
        ask: Box::new(ask),
        outcome: p::Outcome::Created { first: 3, last: 3, given: None },
        at: skein_lib::Wall::EPOCH,
    });
}

#[test]
fn account_refusals_keep_their_reasons_in_the_store() {
    use jig_domain_people as p;
    use jig_domain_tasks as t;
    for why in [p::AccountWhy::Unknown, p::AccountWhy::Provider, p::AccountWhy::Uses, p::AccountWhy::Limited] {
        people_round_trip(fixtures::people_outcomes::answered(p::Outcome::Refused(p::Refusal::Account {
            account: 42,
            why,
        })));
    }
    for why in [t::AccountWhy::Unknown, t::AccountWhy::Provider, t::AccountWhy::Uses, t::AccountWhy::Limited] {
        let mut problem = fixtures::calls::problem();
        problem.why = t::Refusal::Account { account: 42, why };
        core_round_trip(fixtures::calls::call(jig_domain_core::CallPart::DelegationRefused(problem)));
    }
}

#[test]
fn a_program_declaration_retains_its_public_name() {
    use jig_domain_people::{AccountChange, AccountHolder, Ask, Outcome, RequestKey, Stored};
    let ask = Ask::AccountChange {
        holder: AccountHolder::Own,
        change: AccountChange::DeclareProgram { provider: 3, join: Some(2), name: b"personal".as_slice().into() },
    };
    people_round_trip(Stored::Answer {
        models: Box::new([]),
        key: RequestKey { person: 1, key: [7; 16] },
        ask: Box::new(ask),
        outcome: Outcome::AccountProgramDeclared { source: 0, name: b"personal".as_slice().into() },
        at: skein_lib::Wall::EPOCH,
    });
}
#[test]
fn program_sign_out_answers_retain_host_failures() {
    use jig_domain_people::{AccountHolder, AccountHostFailure, AccountHostWhy, Ask, Outcome, RequestKey, Stored};
    for why in [AccountHostWhy::NotInContact, AccountHostWhy::Lost, AccountHostWhy::Failed] {
        people_round_trip(Stored::Answer {
            models: Box::new([]),
            key: RequestKey { person: 1, key: [7; 16] },
            ask: Box::new(Ask::AccountSignOut { holder: AccountHolder::Own, source: 0 }),
            outcome: Outcome::AccountSignedOut {
                confirmed: false,
                failed_hosts: Box::new([AccountHostFailure { name: b"configured-worker".as_slice().into(), why }]),
            },
            at: skein_lib::Wall::EPOCH,
        });
    }
}
#[test]
fn program_checks_retain_the_source_number() {
    ask_round_trip(jig_domain_people::Ask::AccountChange {
        holder: jig_domain_people::AccountHolder::Own,
        change: jig_domain_people::AccountChange::CheckProgram { source: 0 },
    });
}

#[test]
fn retained_people_answers_keep_dependency_refusal_facts() {
    use jig_domain_people as p;
    for why in [
        p::Refusal::Circle { tasks: Box::new([7, 5, 9]), cut: true },
        p::Refusal::Kept { pending: Box::new([7, 9]) },
        p::Refusal::Dependency { dependency: 7, how: p::DependencyStatus::Failed },
        p::Refusal::Dependency { dependency: 9, how: p::DependencyStatus::Cancelled },
    ] {
        people_round_trip(fixtures::people_outcomes::answered(p::Outcome::Refused(why)));
    }
}

#[test]
#[expect(clippy::disallowed_macros, reason = "ordinary tests build boundary text")]
fn a_stored_project_name_is_bounded_and_utf8() {
    use crate::people_v13 as wire;
    use skein_lib::{Reader, Writer};
    // store.md, 4.4: the name is UTF-8 text, at most 64 bytes.
    const NAME_BOUND: usize = 64;
    let limits = people_limits();
    let at = "a".repeat(NAME_BOUND);
    people_round_trip(jig_domain_people::Stored::Policy {
        project: 3,
        name: Box::from(at.as_str()),
        value: fixtures::people_policy(),
    });
    for (name, reason) in
        [(vec![b'a'; NAME_BOUND + 1], skein_codec::Reason::Bound), (vec![0xff], skein_codec::Reason::Utf8)]
    {
        // A StoredPolicy's leading fields, written as records, stop at its bad name.
        let mut writer = Writer::new(size_of::<u32>() * 2 + name.len());
        writer.put(&3_u32.to_be_bytes()).expect("project");
        writer.put(&u32::try_from(name.len()).expect("test length").to_be_bytes()).expect("length");
        writer.put(&name).expect("name");
        assert_eq!(
            wire::StoredPolicy::decode(&limits, &mut Reader::new(&writer.finish())),
            Err(wire::Problem { path: wire::Path::StoredPolicyName, reason }),
        );
    }
}

#[test]
fn acted_rows_round_trip_every_deployment_act() {
    use jig_domain_people::{Act, ActKey, Adoption, Outcome, Refusal, ResourceName, ResourceRole, Stored};
    for act in [
        Act::Close {
            project: 1,
            task: u64::MAX,
            grace: Some(skein_lib::Duration::from_secs(5)),
            reason: b"closed".as_slice().into(),
        },
        Act::AddKey { provider: u32::MAX, join: Some(7), digest: [255; 32] },
        Act::RemoveAccount { account: u32::MAX },
        Act::MakeProject { maker: 1, name: "Tally".into(), policy: 1 },
        Act::Adopt {
            project: 1,
            adoption: Adoption {
                resource: ResourceName { connector: 1, path: Box::new([Box::from(&b"resource"[..])]) },
                role: ResourceRole::Owned,
                options: Box::new([Box::from(&b"home"[..])]),
            },
        },
    ] {
        people_round_trip(Stored::Acted {
            key: ActKey { connector: u16::MAX, key: [255; 16] },
            act,
            outcome: Outcome::Refused(Refusal::NotOffered),
            at: skein_lib::Wall::from_nanos(u64::MAX),
        });
    }
}

#[test]
fn project_asks_answers_and_name_refusals_round_trip() {
    use jig_domain_people::{Ask, LimitKind, Malformed, NameRule, Outcome, Refusal, RequestKey, Stored};
    for (ask, outcome) in [
        (Ask::Rename { project: 1, name: "Tally".into() }, Outcome::Renamed { project: 1 }),
        (Ask::MakeProject { name: "Tally".into(), policy: 1 }, Outcome::ProjectMade { project: 2 }),
    ] {
        people_round_trip(Stored::Answer {
            key: RequestKey { person: 1, key: [1; 16] },
            ask: Box::new(ask),
            models: Box::new([]),
            outcome,
            at: skein_lib::Wall::from_nanos(19),
        });
    }
    for why in [Refusal::NameTaken, Refusal::Limit { which: LimitKind::Projects, bound: None }] {
        people_round_trip(fixtures::people_outcomes::answered(Outcome::Refused(why)));
    }
    for rule in [NameRule::Empty, NameRule::Long, NameRule::Control, NameRule::Space, NameRule::Digits] {
        let why = Refusal::NotWellFormed { at: 0, why: Malformed::Name(rule) };
        people_round_trip(fixtures::people_outcomes::answered(Outcome::Refused(why)));
    }
}

mod holds;
mod settings_proposals;
