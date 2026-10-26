//! Each holds/pieces word reads the bytes its owner wrote (protocol/store.md, 5.1).
use super::fixtures;
use crate::{codec, core_v6 as core, people_v13 as people, tasks_v9 as tasks};
use jig_domain_core as c;
use jig_domain_people as p;
use jig_domain_tasks as t;
use skein_lib::Reader;

// Ordinary test code: one framing loop serves every declaration's generated decoder.
fn pieces<L, T>(bytes: &[u8], limits: &L, decode: fn(&L, &mut Reader<'_>) -> T) -> Box<[T]> {
    let mut reader = Reader::new(bytes);
    let mut values = skein_lib::List::with_capacity(
        reader
            .remaining()
            .checked_div(u32::try_from(size_of::<u32>()).expect("piece length size"))
            .expect("nonzero length size"),
    );
    while let Some(bytes) = skein_codec::piece::split(&mut reader).expect("whole piece framing") {
        let mut item = Reader::new(&bytes);
        assert!(values.push(decode(limits, &mut item)).is_ok(), "framing bounds piece count");
        assert!(item.is_empty(), "the declaration consumes its whole piece");
    }
    assert!(reader.is_empty(), "pieces consume their whole holder");
    values.into_boxed()
}

#[derive(Debug, PartialEq, Eq)]
enum Held {
    Problem(t::Problem),
    Phase(t::Phase),
    Change(t::Change),
    End(t::End),
    Note(jig_domain_notes::Record),
}

impl Held {
    fn read(&self, bytes: &[u8], limits: &codec::core::Limits) -> Self {
        let mut reader = Reader::new(bytes);
        let value = match self {
            Self::Problem(_) => {
                tasks::TaskProblem::decode(&limits.tasks, &mut reader).unwrap();
                Self::Problem(codec::tasks::carried::read_problem(bytes, &limits.tasks).unwrap())
            }
            Self::Phase(_) => {
                let value = tasks::PhaseRecord::decode(&limits.tasks, &mut reader).unwrap();
                Self::Phase(codec::tasks::phase::read_phase(value.into_parts().value, &limits.tasks).unwrap())
            }
            Self::Change(_) => {
                tasks::ChangeRecord::decode(&limits.tasks, &mut reader).unwrap();
                Self::Change(codec::tasks::carried::read_change(bytes, &limits.tasks).unwrap())
            }
            Self::End(_) => {
                tasks::EndRecord::decode(&limits.tasks, &mut reader).unwrap();
                Self::End(codec::tasks::carried::read_end(bytes, &limits.tasks).unwrap())
            }
            Self::Note(_) => {
                crate::notes_v1::Row::decode(&limits.notes, &mut reader).unwrap();
                Self::Note(codec::notes::decode(bytes, &limits.notes).unwrap())
            }
        };
        assert!(reader.is_empty(), "held declaration consumes its whole holder");
        value
    }
}

fn core_row(row: &c::CoreRecord, limits: &codec::core::Limits) -> core::Value {
    let bytes = codec::core::encode(row, limits).unwrap();
    core::Row::decode(&limits.core, &mut Reader::new(&bytes)).unwrap().into_parts().value
}

struct HeldRow {
    path: &'static str,
    bytes: Box<[u8]>,
    expected: Held,
}

#[test]
fn cores_held_words_read_their_fixtures_whole() {
    let limits = codec::core::limits(&fixtures::CORE_LIMITS).unwrap();
    let problem = fixtures::calls::problem();
    let core::Value::Call(call) =
        core_row(&fixtures::calls::call(c::CallPart::DelegationRefused(problem.clone())), &limits)
    else {
        panic!("call fixture")
    };
    let core::CallPart::DelegationRefused(problem_bytes) = call.into_parts().part else { panic!("refused fixture") };
    let projection = fixtures::core_rows::milestone(c::MilestoneId::Lifecycle { task: 11, position: 13 });
    let core::Value::Projection(projection_bytes) = core_row(&c::CoreRecord::Projection(projection.clone()), &limits)
    else {
        panic!("projection fixture")
    };
    let core::ProjectionRecord::Milestone(milestone_bytes) = projection_bytes.into_parts().value else {
        panic!("milestone fixture")
    };
    let milestone_bytes = milestone_bytes.into_parts().milestone.into_parts();
    let c::ProjectionRecord::Milestone { milestone, .. } = projection else { panic!("milestone fixture") };
    let terminal = fixtures::core_rows::terminal();
    let core::Value::Terminal(terminal_bytes) = core_row(&c::CoreRecord::Terminal(terminal.clone()), &limits) else {
        panic!("terminal fixture")
    };
    let entry = fixtures::note_entry();
    let core::Value::Call(notes_bytes) = core_row(
        &fixtures::calls::call(c::CallPart::NoteRecalled { entries: Box::new([entry.clone()]), more: false }),
        &limits,
    ) else {
        panic!("notes call fixture")
    };
    let core::CallPart::NoteRecalled(notes_bytes) = notes_bytes.into_parts().part else { panic!("recalled fixture") };
    let rows = [
        HeldRow {
            path: "TaskBytes.value",
            bytes: problem_bytes.into_parts().value.into_parts().value,
            expected: Held::Problem(problem),
        },
        HeldRow {
            path: "PhaseBytes.value",
            bytes: milestone_bytes.phase.unwrap().into_parts().value,
            expected: Held::Phase(milestone.phase.unwrap()),
        },
        HeldRow {
            path: "ChangeBytes.value",
            bytes: milestone_bytes.change.unwrap().into_parts().value,
            expected: Held::Change(milestone.change.unwrap()),
        },
        HeldRow {
            path: "EndBytes.value",
            bytes: terminal_bytes.into_parts().end.into_parts().value,
            expected: Held::End(terminal.end),
        },
        HeldRow {
            path: "NoteBytes.value",
            bytes: notes_bytes.entries().get(0).unwrap().value().into(),
            expected: Held::Note(jig_domain_notes::Record::Entry(entry)),
        },
    ];
    for row in rows {
        assert_eq!(row.expected.read(&row.bytes, &limits), row.expected, "{}", row.path);
    }
}

#[derive(Debug, PartialEq, Eq)]
enum PeopleItems {
    Segments(Box<[Box<[u8]>]>),
    Executors(Box<[p::Executor]>),
    Grants(Box<[p::Grant]>),
    Scopes(Box<[p::ResourceScope]>),
    Requirements(Box<[p::Requirement]>),
    Permissions(Box<[p::PermissionRole]>),
    Parameters(Box<[p::Parameter]>),
    Numbers(Box<[u64]>),
    Dependencies(Box<[p::Dependency]>),
    Verdicts(Box<[p::Verdict]>),
    Rules(Box<[p::FieldRule]>),
}

fn segments(bytes: &[u8], limits: &people::Limits) -> Box<[Box<[u8]>]> {
    pieces(bytes, limits, |limits, reader| people::Segment::decode(limits, reader).unwrap().into_parts().bytes)
}

fn pattern(value: people::Pattern, limits: &people::Limits) -> p::Pattern {
    let value = value.into_parts();
    p::Pattern {
        segments: segments(&value.segments, limits),
        last: match value.last {
            people::Last::Exact(value) => p::Last::Exact(value.into_parts().value),
            people::Last::Open(value) => p::Last::Open(value.into_parts().value),
        },
    }
}

fn rules(bytes: &[u8], limits: &people::Limits) -> Box<[p::FieldRule]> {
    pieces(bytes, limits, |limits, reader| {
        let value = people::FieldRule::decode(limits, reader).unwrap().into_parts();
        p::FieldRule { name: value.name, max: value.max }
    })
}

impl PeopleItems {
    fn read(&self, bytes: &[u8], limits: &people::Limits) -> Self {
        match self {
            Self::Segments(_) => Self::Segments(segments(bytes, limits)),
            Self::Executors(_) => {
                Self::Executors(pieces(bytes, limits, |limits, reader| {
                    match people::Executor::decode(limits, reader).unwrap() {
                        people::Executor::Charter(value) => p::Executor::Charter(value.into_parts().value),
                        people::Executor::Procedure(value) => p::Executor::Procedure(value.into_parts().value),
                        people::Executor::Role(value) => p::Executor::Role(value.into_parts().value),
                    }
                }))
            }
            Self::Grants(_) => Self::Grants(pieces(bytes, limits, |limits, reader| {
                let value = people::Grant::decode(limits, reader).unwrap().into_parts();
                p::Grant { connector: value.connector, kind: value.kind, pattern: pattern(value.pattern, limits) }
            })),
            Self::Scopes(_) => Self::Scopes(pieces(bytes, limits, |limits, reader| {
                let value = people::ResourceScope::decode(limits, reader).unwrap().into_parts();
                p::ResourceScope { connector: value.connector, pattern: pattern(value.pattern, limits) }
            })),
            Self::Requirements(_) => Self::Requirements(pieces(bytes, limits, |limits, reader| {
                let value = people::Requirement::decode(limits, reader).unwrap().into_parts();
                let judge = value.judge.into_parts();
                p::Requirement {
                    connector: value.connector,
                    kind: value.kind,
                    pattern: pattern(value.pattern, limits),
                    judge: p::Judge {
                        connector: judge.connector,
                        requirement: judge.requirement,
                        parameters: judge.parameters,
                    },
                    guard: match value.guard {
                        people::Guard::Guarded => p::Guard::Guarded,
                        people::Guard::Observed(value) => p::Guard::Observed {
                            freshness: skein_lib::Duration::from_nanos(value.into_parts().freshness),
                        },
                    },
                    must_be_guarded: value.must_be_guarded,
                }
            })),
            Self::Permissions(_) => Self::Permissions(pieces(bytes, limits, |limits, reader| {
                let value = people::PermissionRole::decode(limits, reader).unwrap().into_parts();
                p::PermissionRole { connector: value.connector, permission: value.permission, role: value.role }
            })),
            Self::Parameters(_) => Self::Parameters(pieces(bytes, limits, |limits, reader| {
                match people::Parameter::decode(limits, reader).unwrap() {
                    people::Parameter::Number(value) => {
                        let value = value.into_parts();
                        p::Parameter::Number { name: value.name, value: value.value }
                    }
                    people::Parameter::Bytes(value) => {
                        let value = value.into_parts();
                        p::Parameter::Bytes { name: value.name, value: value.value }
                    }
                    people::Parameter::Resource(value) => {
                        let value = value.into_parts();
                        p::Parameter::Resource {
                            name: value.name,
                            connector: value.connector,
                            resource: value.resource,
                        }
                    }
                }
            })),
            Self::Numbers(_) => Self::Numbers(pieces(bytes, limits, |limits, reader| {
                people::Number64::decode(limits, reader).unwrap().value()
            })),
            Self::Dependencies(_) => Self::Dependencies(pieces(bytes, limits, |limits, reader| {
                match people::Dependency::decode(limits, reader).unwrap() {
                    people::Dependency::Batch(value) => p::Dependency::Batch(value.into_parts().at),
                    people::Dependency::Existing(value) => p::Dependency::Existing(value.into_parts().task),
                }
            })),
            Self::Verdicts(_) => Self::Verdicts(pieces(bytes, limits, |limits, reader| {
                let value = people::Verdict::decode(limits, reader).unwrap().into_parts();
                p::Verdict {
                    code: value.code,
                    words: value.words,
                    followups: value.followups,
                    fields: rules(&value.fields, limits),
                }
            })),
            Self::Rules(_) => Self::Rules(rules(bytes, limits)),
        }
    }
}

struct PeopleRow {
    path: &'static str,
    bytes: Box<[u8]>,
    expected: PeopleItems,
}

// Read the holder back from a complete stored answer, rather than manufacturing pieces in the test.
fn people_ask(ask: &p::Ask, limits: &people::Limits) -> people::Ask {
    let row = fixtures::people_outcomes::answered(p::Outcome::Refused(p::Refusal::Unknown));
    let p::Stored::Answer { key, models, outcome, at, .. } = row else { panic!("answer fixture") };
    let row = p::Stored::Answer { key, models, outcome, at, ask: Box::new(ask.clone()) };
    let bytes = codec::people::encode(&row, limits, &[]).unwrap();
    let people::Value::Answer(value) =
        people::Row::decode(limits, &mut Reader::new(&bytes)).unwrap().into_parts().value
    else {
        panic!("answer fixture")
    };
    value.into_parts().ask
}

fn member_rows(limits: &people::Limits) -> [PeopleRow; 10] {
    let member = fixtures::holds::member();
    let people::Ask::Create(value) =
        people_ask(&p::Ask::Create { project: 1, batch: Box::new([member.clone()]) }, limits)
    else {
        panic!("create fixture")
    };
    let encoded = value.into_parts().batch.into_boxed().into_iter().next().unwrap().into_parts();
    let authority = encoded.authority.into_parts();
    let people::Contract::Verdict(contract) = encoded.contract else { panic!("verdict fixture") };
    let p::Contract::Verdict { choices } = &member.contract else { panic!("verdict fixture") };
    let choices_bytes = contract.into_parts().choices;
    let mut choices_reader = Reader::new(&choices_bytes);
    let verdict_piece = skein_codec::piece::split(&mut choices_reader).unwrap().unwrap();
    let verdict = people::Verdict::decode(limits, &mut Reader::new(&verdict_piece)).unwrap();
    let grant_piece = skein_codec::piece::split(&mut Reader::new(&authority.grants)).unwrap().unwrap();
    let grant = people::Grant::decode(limits, &mut Reader::new(&grant_piece)).unwrap();
    [
        PeopleRow {
            path: "Pattern.segments",
            bytes: grant.pattern().segments().into(),
            expected: PeopleItems::Segments(member.authority.grants[0].pattern.segments.clone()),
        },
        PeopleRow {
            path: "Delegation.kinds",
            bytes: authority.delegation.into_parts().kinds,
            expected: PeopleItems::Executors(member.authority.delegation.kinds.clone()),
        },
        PeopleRow {
            path: "Authority.grants",
            bytes: authority.grants,
            expected: PeopleItems::Grants(member.authority.grants.clone()),
        },
        PeopleRow {
            path: "Authority.note_resources",
            bytes: authority.note_resources,
            expected: PeopleItems::Scopes(member.authority.note_resources.clone()),
        },
        PeopleRow {
            path: "Verdict.fields",
            bytes: verdict.into_parts().fields,
            expected: PeopleItems::Rules(choices[0].fields.clone()),
        },
        PeopleRow {
            path: "ContractVerdict.choices",
            bytes: choices_bytes,
            expected: PeopleItems::Verdicts(choices.clone()),
        },
        PeopleRow {
            path: "Member.parameters",
            bytes: encoded.parameters,
            expected: PeopleItems::Parameters(member.parameters.clone()),
        },
        PeopleRow {
            path: "Member.inputs",
            bytes: encoded.inputs,
            expected: PeopleItems::Numbers(member.inputs.clone()),
        },
        PeopleRow {
            path: "Member.dependencies",
            bytes: encoded.dependencies,
            expected: PeopleItems::Dependencies(member.dependencies.clone()),
        },
        PeopleRow { path: "Member.named", bytes: encoded.named, expected: PeopleItems::Grants(member.named.clone()) },
    ]
}

fn amendment_rows(limits: &people::Limits) -> [PeopleRow; 3] {
    let amendment = fixtures::people_amendment();
    let people::Ask::Amend(value) = people_ask(
        &p::Ask::Amend { project: 1, task: 2, revision: 3, amendment: amendment.clone(), release: Some(4) },
        limits,
    ) else {
        panic!("amend fixture")
    };
    let amendment_bytes = value.into_parts().amendment.into_parts();
    let spec = amendment.spec.as_ref().unwrap();
    let spec_bytes = amendment_bytes.spec.unwrap().into_parts();
    [
        PeopleRow {
            path: "Spec.parameters",
            bytes: spec_bytes.parameters,
            expected: PeopleItems::Parameters(spec.parameters.clone()),
        },
        PeopleRow {
            path: "Spec.inputs",
            bytes: spec_bytes.inputs,
            expected: PeopleItems::Numbers(spec.inputs.clone()),
        },
        PeopleRow {
            path: "Amendment.dependencies",
            bytes: amendment_bytes.dependencies.unwrap(),
            expected: PeopleItems::Numbers(amendment.dependencies.unwrap()),
        },
    ]
}

fn note_rows(limits: &people::Limits) -> [PeopleRow; 3] {
    let note = fixtures::holds::edit_note();
    let people::Ask::EditNote(note_bytes) = people_ask(&note, limits) else { panic!("note fixture") };
    let people::NoteChange::Correct(note_bytes) = note_bytes.into_parts().change else { panic!("correction fixture") };
    let p::Ask::EditNote { change, .. } = note else { panic!("note fixture") };
    let p::NoteChange::Correct { references, .. } = *change else { panic!("correction fixture") };
    let adoption = fixtures::holds::adopt();
    let people::Ask::Adopt(adoption_bytes) = people_ask(&adoption, limits) else { panic!("adoption fixture") };
    let adoption_bytes = adoption_bytes.into_parts().adoption.into_parts();
    let p::Ask::Adopt { adoption, .. } = adoption else { panic!("adoption fixture") };
    [
        PeopleRow {
            path: "NoteChangeCorrect.references",
            bytes: note_bytes.into_parts().references,
            expected: PeopleItems::Numbers(references),
        },
        PeopleRow {
            path: "ResourceName.path",
            bytes: adoption_bytes.resource.into_parts().path,
            expected: PeopleItems::Segments(adoption.resource.path),
        },
        PeopleRow {
            path: "Adoption.options",
            bytes: adoption_bytes.options,
            expected: PeopleItems::Segments(adoption.options),
        },
    ]
}

fn policy_rows(limits: &people::Limits) -> [PeopleRow; 4] {
    let policy = fixtures::people_policy();
    let grants = policy.ceiling.grants.clone();
    let policy_row = p::Stored::Policy { project: 3, name: "Tally".into(), value: policy.clone() };
    let bytes = codec::people::encode(&policy_row, limits, &[]).unwrap();
    let people::Value::Policy(policy_bytes) =
        people::Row::decode(limits, &mut Reader::new(&bytes)).unwrap().into_parts().value
    else {
        panic!("policy fixture")
    };
    let permissions_row = PeopleRow {
        path: "PolicyValue.permissions",
        bytes: policy_bytes.value().permissions().into(),
        expected: PeopleItems::Permissions(policy.permissions.clone()),
    };
    let [requirements, projections, permissions] = [
        (
            p::Ask::ChangePolicy {
                project: 1,
                change: p::PolicyChange::Requirements { requirements: policy.requirements.clone() },
            },
            PeopleItems::Requirements(policy.requirements),
        ),
        (
            p::Ask::ChangePolicy { project: 1, change: p::PolicyChange::Projections { grants: grants.clone() } },
            PeopleItems::Grants(grants),
        ),
        (
            p::Ask::ChangePolicy {
                project: 1,
                change: p::PolicyChange::Permissions { mappings: policy.permissions.clone() },
            },
            PeopleItems::Permissions(policy.permissions),
        ),
    ]
    .map(|(ask, expected)| {
        let people::Ask::ChangePolicy(value) = people_ask(&ask, limits) else { panic!("policy change fixture") };
        let (path, bytes) = match value.into_parts().change {
            people::PolicyChange::Requirements(value) => {
                ("PolicyChangeRequirements.requirements", value.into_parts().requirements)
            }
            people::PolicyChange::Projections(value) => ("PolicyChangeProjections.grants", value.into_parts().grants),
            people::PolicyChange::Permissions(value) => {
                ("PolicyChangePermissions.mappings", value.into_parts().mappings)
            }
            people::PolicyChange::ProjectSpend(_)
            | people::PolicyChange::Role(_)
            | people::PolicyChange::Ceiling(_)
            | people::PolicyChange::Templates(_) => panic!("packed policy change fixture"),
        };
        PeopleRow { path, bytes, expected }
    });
    [permissions_row, requirements, projections, permissions]
}

#[test]
fn peoples_piece_words_read_their_fixtures_whole() {
    let limits = super::people_limits();
    let rows = member_rows(&limits)
        .into_iter()
        .chain(amendment_rows(&limits))
        .chain(policy_rows(&limits))
        .chain(note_rows(&limits));
    for row in rows {
        assert_eq!(row.expected.read(&row.bytes, &limits), row.expected, "{}", row.path);
    }
}

#[derive(Debug, PartialEq, Eq)]
enum TaskItems {
    Rules(Box<[t::FieldRule]>),
    Fields(Box<[t::ResultField]>),
}

impl TaskItems {
    fn read(&self, bytes: &[u8], limits: &tasks::Limits) -> Self {
        match self {
            Self::Rules(_) => Self::Rules(pieces(bytes, limits, |limits, reader| {
                let value = tasks::FieldRule::decode(limits, reader).unwrap().into_parts();
                t::FieldRule { name: value.name, max: value.max }
            })),
            Self::Fields(_) => Self::Fields(pieces(bytes, limits, |limits, reader| {
                let value = tasks::ResultField::decode(limits, reader).unwrap().into_parts();
                t::ResultField { name: value.name, value: value.value }
            })),
        }
    }
}

struct TaskRow {
    path: &'static str,
    bytes: Box<[u8]>,
    expected: TaskItems,
}

#[test]
fn tasks_piece_words_read_their_fixtures_whole() {
    let limits = super::tasks_limits();
    let task = fixtures::holds::task();
    let bytes = codec::tasks::encode(&t::Stored::Ended(Box::new(task.clone())), &limits, &[]).unwrap();
    let tasks::Value::Ended(value) = tasks::Row::decode(&limits, &mut Reader::new(&bytes)).unwrap().into_parts().value
    else {
        panic!("ended fixture")
    };
    let value = value.into_parts();
    let tasks::Contract::Verdict(contract) = value.definition.contract().clone() else { panic!("contract fixture") };
    let t::Contract::Verdict { choices } = task.contract else { panic!("contract fixture") };
    let tasks::Phase::Ended(ended) = value.outcome.into_parts().phase else { panic!("ended fixture") };
    let tasks::Ending::Done(done) = ended.into_parts().ending else { panic!("done fixture") };
    let tasks::TaskResult::Verdict(result) = done.into_parts().result else { panic!("verdict fixture") };
    let tasks::MessageKind::Result(message) = value.interests.inbox().get(0).unwrap().kind().clone() else {
        panic!("message fixture")
    };
    let tasks::ResultKind::Verdict(message) = message.into_parts().kind else { panic!("verdict message fixture") };
    let rows = [
        TaskRow {
            path: "Verdict.fields",
            bytes: contract.choices().get(0).unwrap().fields().into(),
            expected: TaskItems::Rules(choices[0].fields.clone()),
        },
        TaskRow {
            path: "VerdictResult.fields",
            bytes: result.into_parts().fields,
            expected: TaskItems::Fields(fixtures::holds::result_fields()),
        },
        TaskRow {
            path: "VerdictMessage.fields",
            bytes: message.into_parts().fields,
            expected: TaskItems::Fields(fixtures::holds::result_fields()),
        },
    ];
    for row in rows {
        assert_eq!(row.expected.read(&row.bytes, &limits), row.expected, "{}", row.path);
    }
}
