use super::*;
use skein_lib::List;
pub const PROJECT: u32 = 7;
pub const CHARTER: u32 = 7;
pub const ROLE: u32 = 2;
pub fn create_ask() -> c::Ask {
    let mut tasks = List::with_capacity(2);
    tasks.push(new_task(c::OwnerExecutor::Agent { charter: CHARTER }, c::NewContract::Report { words: 17 })).unwrap();
    tasks
        .push(new_task(
            c::OwnerExecutor::Person(c::PersonAddress::Role(ROLE)),
            c::NewContract::Change { connector: 5, kind: 6, words: 18 },
        ))
        .unwrap();
    c::Ask::Create { project: PROJECT, tasks }
}
pub fn new_task(executor: c::OwnerExecutor, contract: c::NewContract) -> c::NewTask {
    c::NewTask {
        llm: None,
        deadline: jig_client_domain::contract::NewDeadline::Named,
        executor,
        spec: c::Specification {
            asks: false,
            words: store_text(),
            parameters: list([c::Parameter::Number { name: 5, value: 6 }]),
            inputs: list([u64::MAX]),
        },
        contract,
        authority: read_value_owner_authority(),
        budget: c::NewBudget::AsAllowed { cap: Some(99) },
        wake: Some(read_value_owner_wake_policy()),
        dependencies: list([c::TaskRef::Member(1), c::TaskRef::Task(u64::MAX)]),
        named: list([c::OwnerGrant {
            connector: 5,
            kind: 6,
            pattern: c::OwnerPattern {
                segments: list([b"raw".as_slice().into()]),
                last: c::OwnerPatternEnd::Open(b"tail".as_slice().into()),
            },
        }]),
        tracked: Some(13),
        label: 14,
        flags: c::TaskFlags {
            ended_by: c::EndedBy::Result,
            conversation: false,
            listed: true,
            retention: c::Retention::ForgottenOnceRead,
        },
    }
}

// A finite wall distinct from the fixture's other clocks.
pub const DEADLINE: skein_lib::Wall = skein_lib::Wall::from_nanos(101);
pub fn task_with_deadline(wall: Option<skein_lib::Wall>) -> c::NewTask {
    let mut task = new_task(c::OwnerExecutor::Agent { charter: CHARTER }, c::NewContract::Report { words: 17 });
    task.authority.deadline = wall;
    task
}

pub fn deadline_asks(task: c::NewTask) -> [(&'static str, c::Ask); 2] {
    [
        ("create", c::Ask::Create { project: PROJECT, tasks: list([task.clone()]) }),
        (
            "propose",
            c::Ask::Propose {
                project: PROJECT,
                action: c::ProposeAction::Batch { tasks: list([task]) },
                reason: store_text(),
            },
        ),
    ]
}
