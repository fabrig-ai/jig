//! Exact request translation (client/domain/contract.md, 3).
//! Whole supplied fields preserve independent revisions and optional edits.
use super::request_arguments;
mod compound;
mod create;
mod policy;
use jig_client_domain::contract as client;
use jig_domain_people as people;
use policy::policy_edit;
use skein_lib::List;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Invalid {
    Settings,
    Decision,
    NoteRevision,
    NoteProject,
    CompoundProject,
    /// A compound operation position cannot be represented in a refusal.
    OperationIndex,
    /// An unconfigured model/effort, or an invalid authority name context.
    Llm,
}

/// # Errors
/// Refuses an inapplicable closed decision, an overflowing recalled revision, a mismatched note project
/// or a model choice.
pub fn ask(value: &client::Ask, models: &[jig_domain_core::Model]) -> Result<people::Ask, Invalid> {
    Ok(request(value, models)?.ask)
}

/// The canonical original name context accompanies every explicit authority selector.
pub struct Request {
    pub ask: people::Ask,
    pub models: Box<[Box<[u8]>]>,
}

/// # Errors
/// Refuses unsupported values or a model-name context beyond people's bounds.
pub fn request(value: &client::Ask, models: &[jig_domain_core::Model]) -> Result<Request, Invalid> {
    let mut names = List::with_capacity(people::MODELS);
    let mut ask = translate(value, models, &mut names)?;
    let original = names.into_boxed();
    let mut canonical = original.clone();
    canonical.sort_unstable();
    let mut moved = [0_u64; 64];
    for (old, name) in original.iter().enumerate() {
        for (new, candidate) in canonical.iter().enumerate() {
            if name == candidate {
                let bit = 1_u64.checked_shl(u32::try_from(new).map_err(|_| Invalid::Llm)?).ok_or(Invalid::Llm)?;
                *moved.get_mut(old).ok_or(Invalid::Llm)? = bit;
                break;
            }
        }
    }
    people::remap_models(&mut ask, &moved);
    Ok(Request { ask, models: canonical })
}

#[expect(
    clippy::too_many_lines,
    reason = "one exhaustive closed request match keeps every supported and pending wire arm explicit"
)]
fn translate(
    value: &client::Ask,
    models: &[jig_domain_core::Model],
    names: &mut List<Box<[u8]>>,
) -> Result<people::Ask, Invalid> {
    Ok(match value {
        client::Ask::Restore(value) => {
            people::Ask::Restore { revision: value.revision, reason: value.reason.clone().into_bytes() }
        }
        client::Ask::MoveToNewest(value) => people::Ask::MoveToNewest { revision: value.revision },
        client::Ask::ChangeSettings(_) => return Err(Invalid::Settings),
        client::Ask::Propose { project, action, reason } => create::propose(*project, action, reason, models, names)?,
        client::Ask::Withdraw { project, proposal } => people::Ask::Withdraw { project: *project, proposal: *proposal },
        client::Ask::Create { project, tasks } => create::ask(*project, tasks, models, names)?,
        client::Ask::Compound { project, operations } => compound::ask(*project, operations, models, names)?,
        client::Ask::Message { project, task, words } => {
            people::Ask::Say { project: *project, task: *task, words: Box::from(words.bytes()) }
        }
        client::Ask::Answer { project, on: client::AnswerOn::Question { task, message, words } } => {
            people::Ask::AnswerQuestion {
                project: *project,
                task: *task,
                question: *message,
                words: Box::from(words.bytes()),
            }
        }
        client::Ask::Answer { project, on: client::AnswerOn::Person { task, revision, answer } } => {
            people::Ask::AnswerPerson {
                project: *project,
                task: *task,
                revision: revision.value,
                result: request_arguments::person_answer(answer),
            }
        }
        client::Ask::Take { project, task } => people::Ask::TakePerson { project: *project, task: *task },
        client::Ask::HandBack { project, task } => people::Ask::HandBackPerson { project: *project, task: *task },
        client::Ask::Decide { project, on: client::DecideOn::Proposal { proposal, decision, running } } => {
            people::Ask::DecideProposal {
                project: *project,
                proposer: match proposal.proposer {
                    client::Proposer::Task(number) => people::Proposer::Task(number),
                    client::Proposer::Party(number) => people::Proposer::Party(number),
                },
                proposal: proposal.number,
                decision: proposal_decision(decision)?,
                running: running.as_ref().map(|value| match value {
                    client::Running::Stays => people::Running::Stays,
                    client::Running::Moves => people::Running::Moves,
                }),
            }
        }
        client::Ask::Decide { project, on: client::DecideOn::Escalation { task, revision, decision } } => {
            people::Ask::DecideEscalation {
                project: *project,
                task: *task,
                revision: revision.value,
                decision: escalation_decision(decision)?,
            }
        }
        client::Ask::Release { project, task, revision, again } => {
            people::Ask::Release { project: *project, task: *task, revision: revision.value, again: *again }
        }
        client::Ask::Stop { project, task } => people::Ask::Stop { project: *project, task: *task },
        client::Ask::Cancel { project, task, reason, grace } => {
            people::Ask::Cancel { project: *project, task: *task, reason: Box::from(reason.bytes()), grace: *grace }
        }
        client::Ask::Move { project, task, reason } => {
            people::Ask::Move { project: *project, task: *task, reason: Box::from(reason.bytes()) }
        }
        client::Ask::Seen { through: seen } => people::Ask::MarkSeen { seen: request_arguments::seen(*seen) },
        client::Ask::AccountChange { holder, change } => {
            people::Ask::AccountChange { holder: account_holder(*holder), change: account_change(change) }
        }
        client::Ask::SignIn { holder, provider } => {
            people::Ask::AccountSignIn { holder: account_holder(*holder), provider: *provider }
        }
        client::Ask::SignOut { holder, source } => {
            people::Ask::AccountSignOut { holder: account_holder(*holder), source: *source }
        }
        client::Ask::Rename { project, name } => people::Ask::Rename {
            project: *project,
            name: core::str::from_utf8(name.bytes()).expect("client text is UTF-8").into(),
        },
        client::Ask::MakeProject { name, policy } => people::Ask::MakeProject {
            name: core::str::from_utf8(name.bytes()).expect("client text is UTF-8").into(),
            policy: *policy,
        },
        client::Ask::SetPool { project, party, budget } => {
            people::Ask::SetPool { project: *project, person: *party, budget: *budget }
        }
        client::Ask::EditNote { project, note, scope, revision, description, body, references } => {
            people::Ask::EditNote {
                project: *project,
                name: *note,
                scope: Box::new(note_scope(scope, *project)?),
                change: Box::new(people::NoteChange::Correct {
                    description: Box::from(description.bytes()),
                    body: Box::from(body.bytes()),
                    references: references.iter().copied().collect(),
                    recalled: u32::try_from(revision.value).map_err(|_| Invalid::NoteRevision)?,
                }),
            }
        }
        client::Ask::DeleteNote { project, note, scope, revision } => people::Ask::EditNote {
            project: *project,
            name: *note,
            scope: Box::new(note_scope(scope, *project)?),
            change: Box::new(people::NoteChange::Delete {
                recalled: u32::try_from(revision.value).map_err(|_| Invalid::NoteRevision)?,
            }),
        },
        client::Ask::Amend { project, task, revision, change, reason } => people::Ask::Amend {
            project: *project,
            task: *task,
            revision: revision.map_or(0, |value| value.value),
            amendment: amend(change, reason, models, names)?,
            release: None,
        },
        client::Ask::ChangePolicy { project, edit } => {
            people::Ask::ChangePolicy { project: *project, change: policy_edit(edit, names)? }
        }
        client::Ask::SetRoles { project, holdings } => people::Ask::SetRoles {
            project: *project,
            holdings: holdings
                .iter()
                .map(|value| people::Holding { person: value.party, role: role(value.role) })
                .collect(),
        },
        client::Ask::MakeService { project, name, role: held } => {
            people::Ask::MakeService { project: *project, name: Box::from(name.bytes()), role: role(*held) }
        }
    })
}

fn amend(
    change: &client::Amendment,
    reason: &client::Text,
    models: &[jig_domain_core::Model],
    names: &mut List<Box<[u8]>>,
) -> Result<people::Amendment, Invalid> {
    Ok(people::Amendment {
        tracked: change.tracked.map(request_arguments::tracking),
        addressed: change.addressed.map(|value| people::AddressChange {
            revision: value.revision.value,
            address: match value.address {
                client::PersonAddress::Party(number) => people::PersonAddress::Person(number),
                client::PersonAddress::Role(number) => people::PersonAddress::Role(number),
            },
        }),
        instructions: change.instructions.as_ref().map(|value| Box::from(value.bytes())),
        spec: change.spec.as_ref().map(specification),
        wake: change.wake.map(request_arguments::wake),
        dependencies: change.dependencies.as_ref().map(|values| values.iter().copied().collect()),
        authority: change.authority.as_ref().map(|value| authority(value, names)).transpose()?,
        llm: super::llm::requested(change.llm.as_ref(), models)?,
        reason: Box::from(reason.bytes()),
    })
}

/// A configured-name refusal precedes translating the request into people's indices.
/// # Errors
/// Refuses an operation position beyond the client refusal index range.
pub fn refused(value: &client::Ask, models: &[jig_domain_core::Model]) -> Result<Option<client::Answer>, Invalid> {
    super::llm::refused(value, models)
}

fn specification(value: &client::Specification) -> people::Spec {
    people::Spec {
        words: Box::from(value.words.bytes()),
        parameters: value
            .parameters
            .iter()
            .map(|value| match value {
                client::Parameter::Number { name, value } => people::Parameter::Number { name: *name, value: *value },
                client::Parameter::Bytes { name, value } => {
                    people::Parameter::Bytes { name: *name, value: Box::from(value.bytes()) }
                }
                client::Parameter::Resource { name, connector, resource } => {
                    people::Parameter::Resource { name: *name, connector: *connector, resource: *resource }
                }
            })
            .collect(),
        inputs: value.inputs.iter().copied().collect(),
    }
}

fn role(value: client::HeldRole) -> people::Role {
    match value {
        client::HeldRole::Owner => people::Role::Owner,
        client::HeldRole::Maintainer => people::Role::Maintainer,
        client::HeldRole::Member => people::Role::Member,
        client::HeldRole::Observer => people::Role::Observer,
        client::HeldRole::Policy { role } => people::Role::Policy { role },
    }
}

fn pattern(value: &client::OwnerPattern) -> people::Pattern {
    people::Pattern {
        segments: value.segments.iter().cloned().collect(),
        last: match &value.last {
            client::OwnerPatternEnd::Exact(bytes) => people::Last::Exact(bytes.clone()),
            client::OwnerPatternEnd::Open(bytes) => people::Last::Open(bytes.clone()),
        },
    }
}

fn grant(value: &client::OwnerGrant) -> people::Grant {
    people::Grant { connector: value.connector, kind: value.kind, pattern: pattern(&value.pattern) }
}

fn authority(value: &client::OwnerAuthority, names: &mut List<Box<[u8]>>) -> Result<people::Authority, Invalid> {
    let models = match &value.models {
        None => None,
        Some(client::AuthorityModels::All) => Some(people::Models::ALL),
        Some(client::AuthorityModels::Named(selected)) => {
            let mut bits = 0_u64;
            for name in selected {
                if name.is_empty()
                    || name.len() > usize::try_from(people::MODEL_NAME_BYTES).map_err(|_| Invalid::Llm)?
                {
                    return Err(Invalid::Llm);
                }
                let mut found = None;
                for (at, previous) in names.iter().enumerate() {
                    if name == previous {
                        found = Some(u32::try_from(at).map_err(|_| Invalid::Llm)?);
                        break;
                    }
                }
                let index = if let Some(at) = found {
                    at
                } else {
                    let at = names.len();
                    names.push(name.clone()).map_err(|_| Invalid::Llm)?;
                    at
                };
                let bit = 1_u64.checked_shl(index).ok_or(Invalid::Llm)?;
                if bits & bit != 0 {
                    return Err(Invalid::Llm);
                }
                bits |= bit;
            }
            Some(people::Models::Named(bits))
        }
    };
    Ok(people::Authority {
        tools: value.tools,
        models,
        grants: value.grants.iter().map(grant).collect(),
        delegation: people::Delegation {
            kinds: value
                .delegation
                .kinds
                .iter()
                .map(|kind| match kind {
                    client::OwnerAuthorityExecutor::Charter(number) => people::Executor::Charter(*number),
                    client::OwnerAuthorityExecutor::Procedure(number) => people::Executor::Procedure(*number),
                    client::OwnerAuthorityExecutor::Role(number) => people::Executor::Role(*number),
                })
                .collect(),
            tasks: value.delegation.tasks,
            depth: value.delegation.depth,
        },
        spend: value.spend,
        deadline: value.deadline,
        notes: value.notes,
        note_resources: value
            .note_resources
            .iter()
            .map(|scope| people::ResourceScope { connector: scope.connector, pattern: pattern(&scope.pattern) })
            .collect(),
    })
}

fn note_scope(value: &client::NoteScope, project: u32) -> Result<people::NoteScope, Invalid> {
    Ok(match value {
        client::NoteScope::Deployment => people::NoteScope::Deployment,
        client::NoteScope::Project(named) => {
            if *named != project {
                return Err(Invalid::NoteProject);
            }
            people::NoteScope::Project
        }
        client::NoteScope::Task { project: named, task: goal } => {
            if *named != project {
                return Err(Invalid::NoteProject);
            }
            people::NoteScope::Goal { goal: *goal }
        }
        client::NoteScope::Resource { project: named, connector, pattern: value } => {
            if *named != project {
                return Err(Invalid::NoteProject);
            }
            people::NoteScope::Resources { connector: *connector, pattern: pattern(value) }
        }
    })
}

fn proposal_decision(value: &client::Decision) -> Result<people::ProposalDecision, Invalid> {
    match value {
        client::Decision::Accepted => Ok(people::ProposalDecision::Accept),
        client::Decision::Rejected { reason } => {
            Ok(people::ProposalDecision::Reject { reason: Box::from(reason.bytes()) })
        }
        client::Decision::PassedUp => Ok(people::ProposalDecision::Pass),
        client::Decision::Released { .. } | client::Decision::LeftHeld { .. } => Err(Invalid::Decision),
    }
}

fn escalation_decision(value: &client::Decision) -> Result<people::EscalationDecision, Invalid> {
    match value {
        client::Decision::Released { again: None } => Ok(people::EscalationDecision::Release),
        client::Decision::LeftHeld { reason } | client::Decision::Rejected { reason } => {
            Ok(people::EscalationDecision::Reject { reason: Box::from(reason.bytes()) })
        }
        client::Decision::PassedUp => Ok(people::EscalationDecision::Pass),
        client::Decision::Accepted | client::Decision::Released { again: Some(_) } => Err(Invalid::Decision),
    }
}

fn account_holder(value: client::AccountHolder) -> people::AccountHolder {
    match value {
        client::AccountHolder::Own => people::AccountHolder::Own,
        client::AccountHolder::Deployment => people::AccountHolder::Deployment,
    }
}

fn account_change(value: &client::AccountChange) -> people::AccountChange {
    match value {
        client::AccountChange::MoveOut { account } => people::AccountChange::MoveOut { account: *account },
        client::AccountChange::Written { number } => people::AccountChange::Written { number: *number },
        client::AccountChange::FinishSignIn { source, digest } => {
            people::AccountChange::FinishSignIn { source: *source, digest: *digest }
        }
        client::AccountChange::MoveIn { digest } => people::AccountChange::MoveIn { digest: *digest },
        client::AccountChange::DeclareProgram { provider, join, name } => {
            people::AccountChange::DeclareProgram { provider: *provider, join: *join, name: name.clone() }
        }
        client::AccountChange::CheckProgram { source } => people::AccountChange::CheckProgram { source: *source },
        client::AccountChange::AddKey { provider, join, digest } => {
            people::AccountChange::AddKey { provider: *provider, join: *join, digest: *digest }
        }
        client::AccountChange::ReplaceKey { source, digest } => {
            people::AccountChange::ReplaceKey { source: *source, digest: *digest }
        }
        client::AccountChange::RemoveKey { source } => people::AccountChange::RemoveKey { source: *source },
        client::AccountChange::Resume { source } => people::AccountChange::Resume { source: *source },
        client::AccountChange::Remove { account } => people::AccountChange::Remove { account: *account },
        client::AccountChange::MoveAccount { account, place } => {
            people::AccountChange::MoveAccount { account: *account, place: *place }
        }
        client::AccountChange::MoveSource { account, source, place } => {
            people::AccountChange::MoveSource { account: *account, source: *source, place: *place }
        }
        client::AccountChange::SetUses { account, uses } => people::AccountChange::SetUses {
            account: *account,
            uses: uses.as_ref().map(|uses| {
                uses.iter()
                    .map(|use_| match use_ {
                        client::OwnerAccountUse::Party(n) => people::Use::Party(*n),
                        client::OwnerAccountUse::Project(n) => people::Use::Project(*n),
                    })
                    .collect()
            }),
        },
    }
}

#[cfg(test)]
mod tests {
    const UTF8_TEXTS: [&[u8]; 3] = [b"~/\0", b"~\0", b"~/"];

    use super::*;
    fn text(value: &[u8]) -> client::Text {
        client::Text::new(Box::from(value), 128).expect("the complete supplied fixture is admitted")
    }

    fn chosen() -> client::LlmChoice {
        client::LlmChoice {
            account: Some(17),
            main: client::ModelEffort { model: text(b"scripted"), effort: Some(text(b"high")) },
            subagents: crate::test::list([client::ModelEffort { model: text(b"scripted"), effort: None }]),
        }
    }

    fn expected_choice() -> people::LlmChoice {
        people::LlmChoice {
            account: Some(17),
            main: people::ModelEffort { model: 0, effort: Some(1) },
            subagents: Box::new([people::ModelEffort { model: 0, effort: None }]),
        }
    }

    fn change(llm: client::LlmChoice) -> Box<client::Amendment> {
        Box::new(client::Amendment {
            addressed: None,
            spec: None,
            wake: None,
            dependencies: None,
            authority: None,
            instructions: None,
            tracked: None,
            llm: Some(llm),
        })
    }

    fn member(llm: client::LlmChoice) -> client::NewTask {
        let source = super::super::owner::tests::amendment_source();
        let models = [jig_core_world::world::scripted_model()];
        let limits = super::super::rows::tests::limits();
        client::NewTask {
            executor: client::OwnerExecutor::Agent { charter: 1 },
            spec: client::Specification {
                words: text(b"task"),
                parameters: List::with_capacity(0),
                inputs: List::with_capacity(0),
            },
            contract: client::NewContract::Report { words: 16 },
            authority: super::super::owner::authority(&source.authority, &limits, &models)
                .expect("fixture authority")
                .authority,
            budget: client::NewBudget::Named,
            deadline: client::NewDeadline::Named,
            wake: None,
            dependencies: List::with_capacity(0),
            named: List::with_capacity(0),
            tracked: None,
            label: 0,
            flags: client::TaskFlags {
                ended_by: client::EndedBy::Result,
                conversation: false,
                listed: false,
                retention: client::Retention::Kept,
            },
            llm: Some(llm),
        }
    }

    #[test]
    fn amendment_carries_the_chosen_llm() {
        let request = client::Ask::Amend {
            project: 1,
            task: 2,
            revision: None,
            change: change(chosen()),
            reason: text(b"choice"),
        };
        let people::Ask::Amend { amendment, .. } =
            ask(&request, &[jig_core_world::world::scripted_model()]).expect("translated amendment")
        else {
            panic!("amendment")
        };
        assert_eq!(amendment.llm, Some(expected_choice()));
    }

    #[test]
    fn creation_carries_the_chosen_llm() {
        let request = client::Ask::Create { project: 1, tasks: crate::test::list([member(chosen())]) };
        let people::Ask::Create { batch, .. } =
            ask(&request, &[jig_core_world::world::scripted_model()]).expect("translated creation")
        else {
            panic!("creation")
        };
        assert_eq!(batch.first().expect("one task").llm, Some(expected_choice()));
    }

    #[test]
    fn missing_models_are_refused_at_the_request_entrance() {
        let mut llm = chosen();
        llm.main.model = text(b"removed");
        let tasks = crate::test::list([member(llm.clone())]);
        let amendment = client::Ask::Amend {
            project: 1,
            task: 2,
            revision: None,
            change: change(llm.clone()),
            reason: text(b"choice"),
        };
        let cases = [
            (client::Ask::Create { project: 1, tasks: tasks.clone() }, None),
            (
                client::Ask::Propose {
                    project: 1,
                    action: client::ProposeAction::Batch { tasks },
                    reason: text(b"choice"),
                },
                None,
            ),
            (
                client::Ask::Propose {
                    project: 1,
                    action: client::ProposeAction::Amendment {
                        task: 2,
                        revision: client::Revision { value: 3 },
                        change: change(llm.clone()),
                    },
                    reason: text(b"choice"),
                },
                None,
            ),
            (amendment, None),
            (
                client::Ask::Compound {
                    project: 1,
                    operations: crate::test::list([
                        client::Operation::Stop { project: 1, task: 2 },
                        client::Operation::Amend {
                            project: 1,
                            task: 2,
                            revision: None,
                            change: change(llm),
                            reason: text(b"choice"),
                        },
                    ]),
                },
                Some(1),
            ),
        ];
        for (request, at) in cases {
            assert_eq!(
                refused(&request, &[jig_core_world::world::scripted_model()]),
                Ok(Some(client::Answer::Refused {
                    refusal: client::Refusal::Llm { model: text(b"removed"), why: client::LlmWhy::Unconfigured },
                    at
                }))
            );
        }
    }

    #[test]
    fn amendment_requests_keep_supplied_empty_or_opaque_reason_and_independent_address_spec_and_release_revisions() {
        for reason in [text(b""), text(UTF8_TEXTS[0])] {
            for address in [client::PersonAddress::Party(u64::MAX), client::PersonAddress::Role(u32::MAX)] {
                let request = client::Ask::Amend {
                    project: u32::MAX,
                    task: 0,
                    revision: Some(client::Revision { value: 6 }),
                    change: Box::new(client::Amendment {
                        llm: None,
                        addressed: Some(client::AddressChange { revision: client::Revision { value: 12 }, address }),
                        instructions: Some(text(b"")),
                        spec: None,
                        wake: None,
                        dependencies: Some(List::with_capacity(7)),
                        authority: None,
                        tracked: None,
                    }),
                    reason: reason.clone(),
                };
                let client::Ask::Amend { project, task, revision, change, reason: supplied_reason } = request else {
                    panic!("amendment");
                };
                let mut operations = List::with_capacity(2);
                operations
                    .push(client::Operation::Amend { project, task, revision, change, reason: supplied_reason })
                    .expect("two operations fit");
                operations
                    .push(client::Operation::Release {
                        project,
                        task,
                        revision: client::Revision { value: 4 },
                        again: None,
                    })
                    .expect("two operations fit");
                let request = client::Ask::Compound { project, operations };
                let before = request.clone();
                let Ok(people::Ask::Compound { project, operations }) = ask(&request, &[]) else {
                    panic!("whole compound");
                };
                let [
                    people::Operation::Amend { task, revision, amendment, release: None },
                    people::Operation::Release { revision: release, .. },
                ] = operations.as_slice()
                else {
                    panic!("amend and release");
                };
                let amendment = amendment.clone();
                assert_eq!((project, *task, *revision, Some(*release)), (u32::MAX, 0, 6, Some(4)));
                let addressed = amendment.addressed.expect("selected address");
                assert_eq!(addressed.revision, 12);
                assert_eq!(
                    addressed.address,
                    match address {
                        client::PersonAddress::Party(n) => people::PersonAddress::Person(n),
                        client::PersonAddress::Role(n) => people::PersonAddress::Role(n),
                    }
                );
                assert_eq!(amendment.reason.as_ref(), reason.bytes());
                assert_eq!(amendment.instructions.as_deref(), Some([].as_slice()));
                assert_eq!(amendment.dependencies.as_deref(), Some([].as_slice()));
                assert_eq!((amendment.spec, amendment.wake, amendment.authority), (None, None, None));
                assert_eq!(request, before);
            }
        }
    }

    #[test]
    fn amendment_whole_selected_specification_authority_and_wake_keep_every_parameter_channel_and_optional_absence() {
        let mut parameters = List::with_capacity(7);
        parameters.push(client::Parameter::Number { name: u32::MAX, value: u64::MAX }).expect("room");
        parameters.push(client::Parameter::Bytes { name: 0, value: text(UTF8_TEXTS[1]) }).expect("room");
        parameters.push(client::Parameter::Resource { name: 3, connector: u16::MAX, resource: 0 }).expect("room");
        let mut inputs = List::with_capacity(5);
        inputs.push(u64::MAX).expect("room");
        inputs.push(0).expect("room");
        let selected = client::Specification { words: text(b""), parameters, inputs: inputs.clone() };
        let source = super::super::owner::tests::amendment_source();
        let limits =
            jig_client::DomainLimits { rows: 4, text: 128, turn_text: 128, ..crate::test::limits().client.domain };
        let authority =
            super::super::owner::authority(&source.authority, &limits, &[jig_core_world::world::scripted_model()])
                .expect("whole authority")
                .authority;
        let wake = client::OwnerWakePolicy {
            words: client::WakeRule::Batch { count: u32::MAX, age: skein_lib::Duration::ZERO },
            notices: client::WakeRule::Immediate,
            news: client::WakeRule::Never,
            results: client::ResultsWake::LastOrFailure,
            questions: true,
            answers: false,
            timers: true,
        };
        let request = client::Ask::Amend {
            project: 0,
            task: u64::MAX,
            revision: Some(client::Revision { value: u64::MAX }),
            change: Box::new(client::Amendment {
                llm: None,
                addressed: None,
                instructions: None,
                spec: Some(selected),
                wake: Some(wake),
                dependencies: Some(inputs),
                authority: Some(authority.clone()),
                tracked: None,
            }),
            reason: text(b"supplied"),
        };
        let Ok(people::Ask::Amend { project, task, revision, amendment, release }) = ask(&request, &[]) else {
            panic!("whole amendment")
        };
        assert_eq!((project, task, revision, release), (0, u64::MAX, u64::MAX, None));
        assert_eq!((amendment.addressed, amendment.instructions), (None, None));
        let spec = amendment.spec.expect("selected whole spec");
        assert_eq!(spec.words.as_ref(), b"");
        assert_eq!(spec.inputs.as_ref(), &[u64::MAX, 0]);
        assert_eq!(
            spec.parameters.as_ref(),
            &[
                people::Parameter::Number { name: u32::MAX, value: u64::MAX },
                people::Parameter::Bytes { name: 0, value: Box::from(UTF8_TEXTS[1]) },
                people::Parameter::Resource { name: 3, connector: u16::MAX, resource: 0 },
            ]
        );
        assert_eq!(amendment.dependencies.as_deref(), Some([u64::MAX, 0].as_slice()));
        assert_eq!(
            amendment.authority,
            Some(super::authority(&authority, &mut List::with_capacity(people::MODELS)).expect("supported authority"))
        );
        assert_eq!(
            amendment.wake,
            Some(people::WakePolicy {
                words: people::WakeRule::Batch { count: u32::MAX, age: skein_lib::Duration::ZERO },
                notices: people::WakeRule::Immediate,
                news: people::WakeRule::Never,
                results: people::ResultsWake::LastOrFailure,
                questions: true,
                answers: false,
                timers: true,
            })
        );
        assert_eq!(amendment.reason.as_ref(), b"supplied");
    }

    #[test]
    fn request_translation_keeps_named_question_person_hold_and_proposal_identities_and_empty_words() {
        assert_eq!(
            ask(
                &client::Ask::Answer {
                    project: u32::MAX,
                    on: client::AnswerOn::Question { task: u64::MAX, message: 42, words: text(b"") }
                },
                &[]
            ),
            Ok(people::Ask::AnswerQuestion { project: u32::MAX, task: u64::MAX, question: 42, words: Box::new([]) })
        );
        for again in [None, Some(false), Some(true)] {
            assert_eq!(
                ask(
                    &client::Ask::Release {
                        project: 4,
                        task: 99,
                        revision: client::Revision { value: u64::MAX },
                        again
                    },
                    &[]
                ),
                Ok(people::Ask::Release { project: 4, task: 99, revision: u64::MAX, again })
            );
        }
        for proposer in [client::Proposer::Task(u64::MAX), client::Proposer::Party(u64::MAX)] {
            let owner = match proposer {
                client::Proposer::Task(n) => people::Proposer::Task(n),
                client::Proposer::Party(n) => people::Proposer::Party(n),
            };
            assert_eq!(
                ask(
                    &client::Ask::Decide {
                        project: 4,
                        on: client::DecideOn::Proposal {
                            proposal: client::ProposalKey { proposer, number: 7 },
                            decision: client::Decision::Rejected { reason: text(UTF8_TEXTS[0]) },
                            running: None
                        }
                    },
                    &[]
                ),
                Ok(people::Ask::DecideProposal {
                    project: 4,
                    proposer: owner,
                    proposal: 7,
                    decision: people::ProposalDecision::Reject { reason: Box::from(UTF8_TEXTS[0]) },
                    running: None
                })
            );
        }
    }

    #[test]
    fn note_request_translation_checks_recalled_width_and_supplied_project_without_dropping_references() {
        let mut references = List::with_capacity(5);
        references.push(u64::MAX).expect("the complete supplied fixture is admitted");
        references.push(13).expect("the complete supplied fixture is admitted");
        let value = client::Ask::EditNote {
            project: 4,
            note: 99,
            scope: client::NoteScope::Project(4),
            revision: client::Revision { value: u64::from(u32::MAX) },
            description: text(b""),
            body: text(UTF8_TEXTS[2]),
            references,
        };
        assert_eq!(
            ask(&value, &[]),
            Ok(people::Ask::EditNote {
                project: 4,
                name: 99,
                scope: Box::new(people::NoteScope::Project),
                change: Box::new(people::NoteChange::Correct {
                    description: Box::new([]),
                    body: Box::from(UTF8_TEXTS[2]),
                    references: Box::from([u64::MAX, 13]),
                    recalled: u32::MAX
                })
            })
        );
        assert_eq!(
            ask(
                &client::Ask::DeleteNote {
                    project: 4,
                    note: 99,
                    scope: client::NoteScope::Project(5),
                    revision: client::Revision { value: 1 }
                },
                &[]
            ),
            Err(Invalid::NoteProject)
        );
        assert_eq!(
            ask(
                &client::Ask::DeleteNote {
                    project: 4,
                    note: 99,
                    scope: client::NoteScope::Task { project: 4, task: u64::MAX },
                    revision: client::Revision { value: u64::from(u32::MAX) + 1 }
                },
                &[]
            ),
            Err(Invalid::NoteRevision)
        );
    }

    #[test]
    fn decision_translation_refuses_an_inapplicable_kind_or_uncertain_choice_instead_of_reinterpreting_the_ask() {
        for again in [Some(false), Some(true)] {
            assert_eq!(escalation_decision(&client::Decision::Released { again }), Err(Invalid::Decision));
        }
        assert_eq!(
            escalation_decision(&client::Decision::Released { again: None }),
            Ok(people::EscalationDecision::Release)
        );
        assert_eq!(proposal_decision(&client::Decision::Released { again: None }), Err(Invalid::Decision));
        assert_eq!(proposal_decision(&client::Decision::LeftHeld { reason: text(b"") }), Err(Invalid::Decision));
        assert_eq!(escalation_decision(&client::Decision::Accepted), Err(Invalid::Decision));
        assert_eq!(
            escalation_decision(&client::Decision::LeftHeld { reason: text(b"") }),
            Ok(people::EscalationDecision::Reject { reason: Box::new([]) })
        );
    }

    fn literal_pattern(open: bool) -> client::OwnerPattern {
        let mut segments = List::with_capacity(4);
        segments.push(Box::from(&b"one/two"[..])).expect("the complete supplied fixture is admitted");
        segments.push(Box::from(&[255, 0][..])).expect("the complete supplied fixture is admitted");
        client::OwnerPattern {
            segments,
            last: if open {
                client::OwnerPatternEnd::Open(Box::new([]))
            } else {
                client::OwnerPatternEnd::Exact(Box::from(&[b'/', 254][..]))
            },
        }
    }

    #[test]
    fn policy_request_translation_keeps_literal_patterns_guard_judge_and_full_width_permission_fields() {
        for open in [false, true] {
            let expected = people::Pattern {
                segments: Box::from([Box::from(&b"one/two"[..]), Box::from(&[255, 0][..])]),
                last: if open {
                    people::Last::Open(Box::new([]))
                } else {
                    people::Last::Exact(Box::from(&[b'/', 254][..]))
                },
            };
            let mut requirements = List::with_capacity(5);
            for guard in [
                client::PolicyGuard::Guarded,
                client::PolicyGuard::Observed { freshness: skein_lib::Duration::from_nanos(u64::MAX) },
            ] {
                requirements
                    .push(client::PolicyRequirement {
                        connector: u16::MAX,
                        kind: 7,
                        pattern: literal_pattern(open),
                        judge: client::PolicyJudge { connector: 3, requirement: u16::MAX, parameters: u32::MAX },
                        guard,
                        must_be_guarded: true,
                    })
                    .expect("the complete supplied fixture is admitted");
            }
            let Ok(people::Ask::ChangePolicy { project, change: people::PolicyChange::Requirements { requirements } }) =
                ask(
                    &client::Ask::ChangePolicy {
                        project: u32::MAX,
                        edit: client::PolicyEdit::Requirements { requirements },
                    },
                    &[],
                )
            else {
                panic!("one whole policy request");
            };
            assert_eq!(project, u32::MAX);
            assert_eq!(requirements.len(), 2);
            for value in &requirements {
                assert_eq!(value.pattern, expected);
                assert_eq!((value.connector, value.kind), (u16::MAX, 7));
                assert_eq!(value.judge, people::Judge { connector: 3, requirement: u16::MAX, parameters: u32::MAX });
                assert!(value.must_be_guarded);
            }
            assert_eq!(requirements[0].guard, people::Guard::Guarded);
            assert_eq!(
                requirements[1].guard,
                people::Guard::Observed { freshness: skein_lib::Duration::from_nanos(u64::MAX) }
            );
            let mut mappings = List::with_capacity(3);
            mappings
                .push(client::PolicyPermission { connector: u16::MAX, permission: u16::MAX, role: u32::MAX })
                .expect("the complete supplied fixture is admitted");
            assert_eq!(
                ask(&client::Ask::ChangePolicy { project: 4, edit: client::PolicyEdit::Permissions { mappings } }, &[]),
                Ok(people::Ask::ChangePolicy {
                    project: 4,
                    change: people::PolicyChange::Permissions {
                        mappings: Box::from([people::PermissionRole {
                            connector: u16::MAX,
                            permission: u16::MAX,
                            role: u32::MAX
                        }])
                    }
                })
            );
        }
    }

    #[test]
    fn policy_role_and_whole_replacements_keep_opaque_authority_codes_flags_and_explicit_empty_lists() {
        let mut kinds = List::with_capacity(5);
        kinds
            .push(client::OwnerAuthorityExecutor::Charter(u32::MAX))
            .expect("the complete supplied fixture is admitted");
        kinds
            .push(client::OwnerAuthorityExecutor::Procedure(u32::MAX))
            .expect("the complete supplied fixture is admitted");
        kinds.push(client::OwnerAuthorityExecutor::Role(u32::MAX)).expect("the complete supplied fixture is admitted");
        let grant = client::OwnerGrant { connector: u16::MAX, kind: u16::MAX, pattern: literal_pattern(true) };
        let mut grants = List::with_capacity(3);
        grants.push(grant.clone()).expect("the complete supplied fixture is admitted");
        let mut resources = List::with_capacity(3);
        resources
            .push(client::OwnerResourceScope { connector: u16::MAX, pattern: literal_pattern(false) })
            .expect("the complete supplied fixture is admitted");
        let role = client::PolicyEditRole {
            number: u32::MAX,
            authority: client::OwnerAuthority {
                models: None,
                tools: u64::MAX,
                grants: grants.clone(),
                delegation: client::OwnerDelegation { kinds, tasks: u32::MAX, depth: u32::MAX },
                spend: u64::MAX,
                deadline: None,
                notes: u8::MAX,
                note_resources: resources,
            },
            period_spend: u64::MAX,
            requests: u16::MAX,
            decides: u8::MAX,
        };
        let Ok(people::Ask::ChangePolicy { project: 4, change: people::PolicyChange::Role(now) }) =
            ask(&client::Ask::ChangePolicy { project: 4, edit: client::PolicyEdit::Role(role) }, &[])
        else {
            panic!("closed role replacement");
        };
        assert_eq!((now.number, now.period_spend, now.requests, now.decides), (u32::MAX, u64::MAX, u16::MAX, u8::MAX));
        assert_eq!((now.authority.tools, now.authority.spend, now.authority.notes), (u64::MAX, u64::MAX, u8::MAX));
        assert_eq!(now.authority.deadline, None);
        assert_eq!((now.authority.delegation.tasks, now.authority.delegation.depth), (u32::MAX, u32::MAX));
        assert_eq!(
            &*now.authority.delegation.kinds,
            &[
                people::Executor::Charter(u32::MAX),
                people::Executor::Procedure(u32::MAX),
                people::Executor::Role(u32::MAX)
            ]
        );
        assert_eq!(now.authority.grants[0], super::grant(&grant));
        assert_eq!(now.authority.note_resources[0].pattern, pattern(&literal_pattern(false)));
        assert_eq!(
            ask(&client::Ask::ChangePolicy { project: 4, edit: client::PolicyEdit::Projections { grants } }, &[]),
            Ok(people::Ask::ChangePolicy {
                project: 4,
                change: people::PolicyChange::Projections { grants: Box::from([super::grant(&grant)]) }
            })
        );
        assert_eq!(
            ask(
                &client::Ask::ChangePolicy { project: 4, edit: client::PolicyEdit::ProjectSpend { period_spend: 0 } },
                &[]
            ),
            Ok(people::Ask::ChangePolicy {
                project: 4,
                change: people::PolicyChange::ProjectSpend { period_spend: 0 }
            })
        );
        for edit in [
            client::PolicyEdit::Requirements { requirements: List::with_capacity(5) },
            client::PolicyEdit::Projections { grants: List::with_capacity(5) },
            client::PolicyEdit::Permissions { mappings: List::with_capacity(5) },
        ] {
            let Ok(people::Ask::ChangePolicy { change, .. }) =
                ask(&client::Ask::ChangePolicy { project: 4, edit }, &[])
            else {
                panic!("explicit empty replacement");
            };
            match change {
                people::PolicyChange::Requirements { requirements } => assert!(requirements.is_empty()),
                people::PolicyChange::Projections { grants } => assert!(grants.is_empty()),
                people::PolicyChange::Permissions { mappings } => assert!(mappings.is_empty()),
                people::PolicyChange::Role(_)
                | people::PolicyChange::RoleSpend { .. }
                | people::PolicyChange::ProjectSpend { .. }
                | people::PolicyChange::Templates { .. } => {
                    panic!("replacement kind retained")
                }
            }
        }
    }

    #[test]
    fn role_spend_keeps_its_role_and_full_width_amount() {
        assert_eq!(
            ask(
                &client::Ask::ChangePolicy {
                    project: 4,
                    edit: client::PolicyEdit::RoleSpend { role: 4, period_spend: u64::MAX },
                },
                &[]
            ),
            Ok(people::Ask::ChangePolicy {
                project: 4,
                change: people::PolicyChange::RoleSpend { role: 4, period_spend: u64::MAX }
            })
        );
    }

    #[test]
    fn role_requests_preserve_all_builtins_full_policy_width_and_whole_named_holdings() {
        let cases = [
            (client::HeldRole::Owner, people::Role::Owner),
            (client::HeldRole::Maintainer, people::Role::Maintainer),
            (client::HeldRole::Member, people::Role::Member),
            (client::HeldRole::Observer, people::Role::Observer),
            (client::HeldRole::Policy { role: u32::MAX }, people::Role::Policy { role: u32::MAX }),
        ];
        let mut holdings = List::with_capacity(7);
        let mut expected = Vec::new();
        for (index, (supplied, actual)) in cases.into_iter().enumerate() {
            let party = u64::MAX - u64::try_from(index).expect("the complete supplied fixture is admitted");
            holdings
                .push(client::Holding { party, role: supplied })
                .expect("the complete supplied fixture is admitted");
            expected.push(people::Holding { person: party, role: actual });
            assert_eq!(
                ask(&client::Ask::MakeService { project: u32::MAX, name: text(UTF8_TEXTS[0]), role: supplied }, &[]),
                Ok(people::Ask::MakeService { project: u32::MAX, name: Box::from(UTF8_TEXTS[0]), role: actual })
            );
        }
        assert_eq!(
            ask(&client::Ask::SetRoles { project: u32::MAX, holdings }, &[]),
            Ok(people::Ask::SetRoles { project: u32::MAX, holdings: expected.into_boxed_slice() })
        );
        assert_eq!(
            ask(&client::Ask::SetRoles { project: 0, holdings: List::with_capacity(7) }, &[]),
            Ok(people::Ask::SetRoles { project: 0, holdings: Box::new([]) })
        );
    }
    #[test]
    fn note_mutation_scope_keeps_explicit_goal_project_and_whole_literal_pattern_without_scope_inference() {
        for open in [false, true] {
            let pattern = literal_pattern(open);
            let scope =
                client::NoteScope::Resource { project: u32::MAX, connector: u16::MAX, pattern: pattern.clone() };
            let request = client::Ask::DeleteNote {
                project: u32::MAX,
                note: u64::MAX,
                scope: scope.clone(),
                revision: client::Revision { value: u64::from(u32::MAX) },
            };
            let before = request.clone();
            assert_eq!(
                ask(&request, &[]),
                Ok(people::Ask::EditNote {
                    project: u32::MAX,
                    name: u64::MAX,
                    scope: Box::new(people::NoteScope::Resources {
                        connector: u16::MAX,
                        pattern: super::pattern(&pattern)
                    }),
                    change: Box::new(people::NoteChange::Delete { recalled: u32::MAX }),
                })
            );
            assert_eq!(request, before);
            assert_eq!(note_scope(&scope, 0), Err(Invalid::NoteProject));
        }
        let goal = client::NoteScope::Task { project: u32::MAX, task: u64::MAX };
        assert_eq!(note_scope(&goal, u32::MAX), Ok(people::NoteScope::Goal { goal: u64::MAX }));
        assert_eq!(note_scope(&goal, 0), Err(Invalid::NoteProject));
    }
}
