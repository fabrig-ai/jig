//! A valid person task would put its specification in the person's inbox.
use jig_client_domain::contract as c;
use jig_client_protocol_wire::v19 as w;
use skein_lib::{List, Writer};

const PROJECT: u32 = 1;
const PERSON_ROLE: u32 = 0;
const SPEC_WORDS: u32 = 32;
const RESULT_WORDS: u32 = 8;
const LABEL: u32 = 1;
const REQUEST_KEY: [u8; 16] = [0x46; 16];

fn task() -> c::NewTask {
    c::NewTask {
        llm: None,
        deadline: jig_client_domain::contract::NewDeadline::Named,
        executor: c::OwnerExecutor::Person(c::PersonAddress::Role(PERSON_ROLE)),
        spec: c::Specification {
            asks: false,
            words: c::Text::new(b"Forged".as_slice().into(), SPEC_WORDS).expect("bounded specification"),
            parameters: List::with_capacity(0),
            inputs: List::with_capacity(0),
        },
        contract: c::NewContract::Report { words: RESULT_WORDS },
        authority: c::OwnerAuthority {
            models: None,
            tools: 0,
            grants: List::with_capacity(0),
            delegation: c::OwnerDelegation { kinds: List::with_capacity(0), tasks: 0, depth: 0 },
            spend: 0,
            deadline: None,
            notes: 0,
            note_resources: List::with_capacity(0),
        },
        budget: c::NewBudget::Named,
        wake: None,
        dependencies: List::with_capacity(0),
        named: List::with_capacity(0),
        tracked: None,
        label: LABEL,
        flags: c::TaskFlags {
            ended_by: c::EndedBy::Result,
            conversation: false,
            listed: true,
            retention: c::Retention::Kept,
        },
    }
}

pub(super) fn request() -> Box<[u8]> {
    let mut tasks = List::with_capacity(1);
    tasks.push(task()).expect("one forged task");
    let ask = jig_client_protocol_link::translate::asks::ask(&c::Ask::Create { project: PROJECT, tasks }, &w::CEILINGS)
        .expect("forged request fits wire");
    let mut writer = Writer::new(usize::try_from(ask.measure()).expect("request size fits"));
    ask.encode(&mut writer).expect("measured request");
    let keyed = w::Keyed::new(
        &w::CEILINGS,
        w::KeyedParts {
            key: REQUEST_KEY,
            owner: w::Owner::Jig,
            version: jig_client_protocol_wire::VERSION,
            request: writer.finish(),
            value: None,
        },
    )
    .expect("bounded keyed request");
    let mut writer = Writer::new(usize::try_from(keyed.measure()).expect("keyed size fits"));
    keyed.encode(&mut writer).expect("measured keyed request");
    writer.finish()
}
