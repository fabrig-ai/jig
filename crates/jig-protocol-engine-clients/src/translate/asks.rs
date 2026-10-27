//! Wire asks into the people vocabulary (client/domain/contract.md, 3).
use super::Untranslatable;
use alloc::boxed::Box;
use jig_client_protocol_wire::v19 as w;
use jig_domain_people as p;
use skein_lib::{List, Reader, Wall};
#[derive(Debug)]
pub(crate) struct NamedAsk {
    pub ask: p::Ask,
    pub models: Box<[Box<[u8]>]>,
}

pub(crate) fn named(record: w::Ask, models: &[crate::Model], limits: &w::Limits) -> Result<NamedAsk, Untranslatable> {
    let mut names = List::with_capacity(p::MODELS);
    let mut ask = decode(record, models, limits, &mut names)?;
    let original = names.into_boxed();
    let mut canonical = original.clone();
    canonical.sort_unstable();
    let mut moved = [0_u64; 64];
    for (at, name) in original.iter().enumerate() {
        for (to, other) in canonical.iter().enumerate() {
            if name == other {
                *moved.get_mut(at).expect("bounded names") =
                    1_u64.checked_shl(u32::try_from(to).expect("bounded model index")).unwrap_or(0);
                break;
            }
        }
    }
    p::remap_models(&mut ask, &moved);
    Ok(NamedAsk { ask, models: canonical })
}

#[cfg(test)]
pub(crate) fn ask(record: w::Ask, models: &[crate::Model], limits: &w::Limits) -> Result<p::Ask, Untranslatable> {
    Ok(named(record, models, limits)?.ask)
}

#[expect(clippy::too_many_lines, reason = "one total match names every wire ask")]
fn decode(
    record: w::Ask,
    models: &[crate::Model],
    limits: &w::Limits,
    names: &mut List<Box<[u8]>>,
) -> Result<p::Ask, Untranslatable> {
    Ok(match record {
        w::Ask::Restore(value) => {
            let value = value.into_parts();
            p::Ask::Restore { revision: value.revision, reason: value.reason }
        }
        w::Ask::MoveToNewest(value) => p::Ask::MoveToNewest { revision: value.revision() },
        w::Ask::ChangeSettings(value) => {
            let against = value.against();
            let record = w::Ask::ChangeSettings(value);
            let mut bytes = skein_lib::Writer::new(usize::try_from(record.measure()).expect("wire measure"));
            record.encode(&mut bytes).expect("measured ask");
            p::Ask::ChangeSettings {
                against,
                digest: crate::sign_in::digest(&bytes.finish()).expose().try_into().expect("32-byte digest"),
            }
        }
        w::Ask::Propose(value) => super::operations::propose(value, models, limits, names)?,
        w::Ask::Withdraw(value) => p::Ask::Withdraw { project: value.project(), proposal: value.proposal() },
        w::Ask::Create(value) => super::operations::create(value, models, limits, names)?,
        w::Ask::Compound(value) => super::operations::compound(value, models, limits, names)?,
        w::Ask::Message(value) => {
            let v = value.into_parts();
            p::Ask::Say { project: v.project, task: v.task, words: v.words }
        }
        w::Ask::Answer(value) => {
            let v = value.into_parts();
            super::operations::answer(v.project, v.on)
        }
        w::Ask::Take(value) => {
            let v = value.into_parts();
            p::Ask::TakePerson { project: v.project, task: v.task }
        }
        w::Ask::HandBack(value) => {
            let v = value.into_parts();
            p::Ask::HandBackPerson { project: v.project, task: v.task }
        }
        w::Ask::Decide(value) => {
            let v = value.into_parts();
            super::operations::decide(v.project, v.on)?
        }
        w::Ask::Release(value) => {
            let v = value.into_parts();
            p::Ask::Release { project: v.project, task: v.task, revision: v.revision.value(), again: v.again }
        }
        w::Ask::Amend(value) => {
            let v = value.into_parts();
            p::Ask::Amend {
                release: None,
                project: v.project,
                task: v.task,
                revision: match v.revision {
                    Some(value) => revision(value),
                    None => 0,
                },
                amendment: amendment(v.change, v.reason, models, limits, names)?,
            }
        }
        w::Ask::Stop(value) => {
            let v = value.into_parts();
            p::Ask::Stop { project: v.project, task: v.task }
        }
        w::Ask::Cancel(value) => {
            let v = value.into_parts();
            p::Ask::Cancel { project: v.project, task: v.task, reason: v.reason, grace: v.grace }
        }
        w::Ask::MoveTask(value) => {
            let v = value.into_parts();
            p::Ask::Move { project: v.project, task: v.task, reason: v.reason }
        }
        w::Ask::Seen(value) => {
            let v = value.into_parts();
            p::Ask::MarkSeen { seen: seen(v.through) }
        }
        w::Ask::MakeService(value) => {
            let v = value.into_parts();
            p::Ask::MakeService { project: v.project, name: v.name, role: role(v.role) }
        }
        w::Ask::AccountChange(value) => {
            p::Ask::AccountChange { holder: account_holder(value.holder()), change: account_change(value.change())? }
        }
        w::Ask::SignIn(value) => {
            p::Ask::AccountSignIn { holder: account_holder(value.holder()), provider: value.provider() }
        }
        w::Ask::SignOut(value) => {
            p::Ask::AccountSignOut { holder: account_holder(value.holder()), source: value.source() }
        }
        w::Ask::Rename(value) => {
            let v = value.into_parts();
            p::Ask::Rename { project: v.project, name: project_name(&v.name, w::Path::RenameName)? }
        }
        w::Ask::MakeProject(value) => {
            let v = value.into_parts();
            p::Ask::MakeProject { name: project_name(&v.name, w::Path::MakeProjectName)?, policy: v.policy }
        }
        w::Ask::SetPool(value) => {
            let v = value.into_parts();
            p::Ask::SetPool { project: v.project, person: v.party, budget: v.budget }
        }
        w::Ask::EditNote(value) => {
            let v = value.into_parts();
            p::Ask::EditNote {
                project: v.project,
                name: v.note,
                scope: Box::new(note_scope(v.scope, v.project, w::Path::EditNoteScope)?),
                change: Box::new(p::NoteChange::Correct {
                    description: v.description,
                    body: v.body,
                    references: v.references.into_boxed(),
                    recalled: recalled(v.revision, w::Path::EditNoteRevision)?,
                }),
            }
        }
        w::Ask::DeleteNote(value) => {
            let v = value.into_parts();
            p::Ask::EditNote {
                project: v.project,
                name: v.note,
                scope: Box::new(note_scope(v.scope, v.project, w::Path::DeleteNoteScope)?),
                change: Box::new(p::NoteChange::Delete {
                    recalled: recalled(v.revision, w::Path::DeleteNoteRevision)?,
                }),
            }
        }
        w::Ask::ChangePolicy(value) => {
            let v = value.into_parts();
            p::Ask::ChangePolicy { project: v.project, change: policy(v.edit, limits, names)? }
        }
        w::Ask::SetRoles(value) => {
            let v = value.into_parts();
            let mut holdings = List::with_capacity(v.holdings.capacity());
            for item in v.holdings.into_boxed() {
                let item = item.into_parts();
                holdings.push(p::Holding { person: item.party, role: role(item.role) }).expect("same capacity");
            }
            p::Ask::SetRoles { project: v.project, holdings: holdings.into_boxed() }
        }
    })
}
fn revision(value: w::Revision) -> u64 {
    value.value()
}
fn recalled(value: w::Revision, path: w::Path) -> Result<u32, Untranslatable> {
    match u32::try_from(value.value()) {
        Ok(value) => Ok(value),
        Err(_) => Err(Untranslatable { path }),
    }
}
pub(super) fn proposer(value: &w::Proposer) -> p::Proposer {
    match value {
        w::Proposer::Task(v) => p::Proposer::Task(v.value()),
        w::Proposer::Party(v) => p::Proposer::Party(v.value()),
    }
}
pub(super) fn proposal_decision(value: w::Decision) -> Result<p::ProposalDecision, Untranslatable> {
    match value {
        w::Decision::Accepted => Ok(p::ProposalDecision::Accept),
        w::Decision::Rejected(v) => Ok(p::ProposalDecision::Reject { reason: v.into_parts().reason }),
        w::Decision::PassedUp => Ok(p::ProposalDecision::Pass),
        w::Decision::Released(_) | w::Decision::LeftHeld(_) => {
            Err(Untranslatable { path: w::Path::DecideProposalDecision })
        }
    }
}
pub(super) fn escalation_decision(value: w::Decision) -> Result<p::EscalationDecision, Untranslatable> {
    match value {
        w::Decision::Released(v) => match v.again() {
            None => Ok(p::EscalationDecision::Release),
            Some(_) => Err(Untranslatable { path: w::Path::DecideEscalationDecision }),
        },
        w::Decision::Rejected(v) => Ok(p::EscalationDecision::Reject { reason: v.into_parts().reason }),
        w::Decision::LeftHeld(v) => Ok(p::EscalationDecision::Reject { reason: v.into_parts().reason }),
        w::Decision::PassedUp => Ok(p::EscalationDecision::Pass),
        w::Decision::Accepted => Err(Untranslatable { path: w::Path::DecideEscalationDecision }),
    }
}
pub(super) fn person_answer(value: w::PersonAnswer) -> p::PersonResult {
    match value {
        w::PersonAnswer::Choice(v) => {
            let v = v.into_parts();
            p::PersonResult::Verdict { code: v.option, words: v.reason }
        }
        w::PersonAnswer::Approved(v) => {
            let v = v.into_parts();
            p::PersonResult::Approved { state: v.state, words: v.words }
        }
        w::PersonAnswer::Words(v) => p::PersonResult::Report { words: v.into_parts().value },
        w::PersonAnswer::Cannot(v) => p::PersonResult::Failure { reason: v.into_parts().reason },
    }
}
fn seen(value: w::SeenThrough) -> p::Seen {
    match value {
        w::SeenThrough::Results(v) => p::Seen::Results { through: v.through() },
        w::SeenThrough::Conversation(v) => p::Seen::Conversation { task: v.task(), through: v.through() },
    }
}
fn role(value: w::HeldRole) -> p::Role {
    match value {
        w::HeldRole::Owner => p::Role::Owner,
        w::HeldRole::Maintainer => p::Role::Maintainer,
        w::HeldRole::Member => p::Role::Member,
        w::HeldRole::Observer => p::Role::Observer,
        w::HeldRole::Policy(v) => p::Role::Policy { role: v.role() },
    }
}
fn pattern(value: w::OwnerPattern) -> p::Pattern {
    let v = value.into_parts();
    p::Pattern {
        segments: v.segments.into_boxed(),
        last: match v.last {
            w::OwnerPatternEnd::Exact(v) => p::Last::Exact(v.into_parts().value),
            w::OwnerPatternEnd::Open(v) => p::Last::Open(v.into_parts().value),
        },
    }
}
fn note_scope(value: w::NoteScope, project: u32, path: w::Path) -> Result<p::NoteScope, Untranslatable> {
    Ok(match value {
        w::NoteScope::Deployment => p::NoteScope::Deployment,
        w::NoteScope::Project(v) => {
            if v.value() != project {
                return Err(Untranslatable { path });
            }
            p::NoteScope::Project
        }
        w::NoteScope::Task(v) => {
            if v.project() != project {
                return Err(Untranslatable { path });
            }
            p::NoteScope::Goal { goal: v.task() }
        }
        w::NoteScope::Resource(v) => {
            let v = v.into_parts();
            if v.project != project {
                return Err(Untranslatable { path });
            }
            p::NoteScope::Resources { connector: v.connector, pattern: pattern(v.pattern) }
        }
    })
}
pub(super) fn spec(value: w::Specification) -> p::Spec {
    let v = value.into_parts();
    let mut parameters = List::with_capacity(v.parameters.capacity());
    for item in v.parameters.into_boxed() {
        let item = match item {
            w::Parameter::Number(v) => p::Parameter::Number { name: v.name(), value: v.value() },
            w::Parameter::Bytes(v) => {
                let v = v.into_parts();
                p::Parameter::Bytes { name: v.name, value: v.value }
            }
            w::Parameter::Resource(v) => {
                p::Parameter::Resource { name: v.name(), connector: v.connector(), resource: v.resource() }
            }
        };
        parameters.push(item).expect("same capacity");
    }
    p::Spec { words: v.words, parameters: parameters.into_boxed(), inputs: v.inputs.into_boxed() }
}
fn wake_rule(value: w::WakeRule) -> p::WakeRule {
    match value {
        w::WakeRule::Never => p::WakeRule::Never,
        w::WakeRule::Immediate => p::WakeRule::Immediate,
        w::WakeRule::Batch(v) => p::WakeRule::Batch { count: v.count(), age: v.age() },
    }
}
pub(super) fn wake(value: w::OwnerWakePolicy) -> p::WakePolicy {
    let v = value.into_parts();
    p::WakePolicy {
        words: wake_rule(v.words),
        notices: wake_rule(v.notices),
        news: wake_rule(v.news),
        results: match v.results {
            w::ResultsWake::Never => p::ResultsWake::Never,
            w::ResultsWake::Each => p::ResultsWake::Each,
            w::ResultsWake::LastOrFailure => p::ResultsWake::LastOrFailure,
        },
        questions: v.questions,
        answers: v.answers,
        timers: v.timers,
    }
}
fn address(value: w::AddressChange) -> p::AddressChange {
    let v = value.into_parts();
    p::AddressChange {
        revision: v.revision.value(),
        address: match v.address {
            w::PersonAddress::Party(v) => p::PersonAddress::Person(v.value()),
            w::PersonAddress::Role(v) => p::PersonAddress::Role(v.value()),
        },
    }
}
#[expect(clippy::manual_map, reason = "optional step fields use named total functions")]
pub(super) fn amendment(
    value: w::Amendment,
    reason: Box<[u8]>,
    models: &[crate::Model],
    limits: &w::Limits,
    names: &mut List<Box<[u8]>>,
) -> Result<p::Amendment, Untranslatable> {
    let v = value.into_parts();
    let authority = match v.authority {
        Some(value) => Some(authority(value, limits, names)?),
        None => None,
    };
    Ok(p::Amendment {
        tracked: match v.tracked {
            Some(value) => Some(super::operations::tracking(value)),
            None => None,
        },
        addressed: match v.addressed {
            Some(value) => Some(address(value)),
            None => None,
        },
        instructions: v.instructions,
        spec: match v.spec {
            Some(value) => Some(spec(value)),
            None => None,
        },
        wake: match v.wake {
            Some(value) => Some(wake(value)),
            None => None,
        },
        dependencies: match v.dependencies {
            Some(value) => Some(List::into_boxed(value)),
            None => None,
        },
        authority,
        llm: super::llm::people(v.llm, models, w::Path::AmendmentLlm)?,
        reason,
    })
}
pub(super) fn count(bytes: &[u8], path: w::Path) -> Result<u32, Untranslatable> {
    let mut reader = Reader::new(bytes);
    let mut count = 0_u32;
    for _ in 0..bytes.len() {
        if reader.is_empty() {
            break;
        }
        let length = reader.u32().ok_or(Untranslatable { path })?;
        reader.bytes(length).ok_or(Untranslatable { path })?;
        count = count.checked_add(1).ok_or(Untranslatable { path })?;
    }
    Ok(count)
}
pub(super) fn item(reader: &mut Reader<'_>, path: w::Path) -> Result<Box<[u8]>, Untranslatable> {
    match skein_codec::piece::split(reader) {
        Ok(Some(bytes)) => Ok(bytes),
        Ok(None) | Err(_) => Err(Untranslatable { path }),
    }
}
fn sum(total: &mut u64, bytes: Option<u64>, ceiling: u32, path: w::Path) -> Result<(), Untranslatable> {
    *total = total.checked_add(bytes.ok_or(Untranslatable { path })?).ok_or(Untranslatable { path })?;
    if *total > u64::from(ceiling) {
        return Err(Untranslatable { path });
    }
    Ok(())
}
pub(super) fn grant(value: w::OwnerGrant) -> p::Grant {
    let v = value.into_parts();
    p::Grant { connector: v.connector, kind: v.kind, pattern: pattern(v.pattern) }
}
pub(super) fn grants(
    bytes: &[u8],
    ceiling: u32,
    path: w::Path,
    limits: &w::Limits,
) -> Result<Box<[p::Grant]>, Untranslatable> {
    let count = count(bytes, path)?;
    let mut values = List::with_capacity(count);
    let mut reader = Reader::new(bytes);
    let mut total = 0;
    for _ in 0..count {
        let bytes = item(&mut reader, path)?;
        let value = match w::OwnerGrant::decode(limits, &mut Reader::new(&bytes)) {
            Ok(value) => grant(value),
            Err(_) => return Err(Untranslatable { path }),
        };
        sum(&mut total, pattern_bytes(&value.pattern, size_of::<p::Grant>()), ceiling, path)?;
        values.push(value).expect("counted pieces");
    }
    Ok(values.into_boxed())
}
#[expect(clippy::manual_map, reason = "step code matches optional fields")]
pub(super) fn authority(
    value: w::OwnerAuthority,
    limits: &w::Limits,
    names: &mut List<Box<[u8]>>,
) -> Result<p::Authority, Untranslatable> {
    let v = value.into_parts();
    let delegation = v.delegation.into_parts();
    let mut kinds = List::with_capacity(delegation.kinds.capacity());
    for kind in delegation.kinds.into_boxed() {
        kinds
            .push(match kind {
                w::OwnerAuthorityExecutor::Charter(v) => p::Executor::Charter(v.value()),
                w::OwnerAuthorityExecutor::Procedure(v) => p::Executor::Procedure(v.value()),
                w::OwnerAuthorityExecutor::Role(v) => p::Executor::Role(v.value()),
            })
            .expect("same capacity");
    }
    let path = w::Path::OwnerAuthorityNoteResources;
    let count = count(&v.note_resources, path)?;
    let mut resources = List::with_capacity(count);
    let mut reader = Reader::new(&v.note_resources);
    let mut total = 0;
    for _ in 0..count {
        let bytes = item(&mut reader, path)?;
        let value = match w::OwnerResourceScope::decode(limits, &mut Reader::new(&bytes)) {
            Ok(value) => value.into_parts(),
            Err(_) => return Err(Untranslatable { path }),
        };
        let pattern = pattern(value.pattern);
        let size = pattern_bytes(&pattern, size_of::<p::ResourceScope>());
        sum(&mut total, size, limits.owner_authority_note_resources, path)?;
        resources.push(p::ResourceScope { connector: value.connector, pattern }).expect("counted pieces");
    }
    Ok(p::Authority {
        tools: v.tools,
        models: super::llm::requested(v.models, names)?,
        grants: grants(&v.grants, limits.owner_authority_grants, w::Path::OwnerAuthorityGrants, limits)?,
        delegation: p::Delegation { kinds: kinds.into_boxed(), tasks: delegation.tasks, depth: delegation.depth },
        spend: v.spend,
        deadline: match v.deadline {
            Some(value) => Some(Wall::from_nanos(value)),
            None => None,
        },
        notes: v.notes,
        note_resources: resources.into_boxed(),
    })
}
pub(super) fn requirement(value: w::PolicyRequirement) -> p::Requirement {
    let v = value.into_parts();
    p::Requirement {
        connector: v.connector,
        kind: v.kind,
        pattern: pattern(v.pattern),
        judge: p::Judge {
            connector: v.judge.connector(),
            requirement: v.judge.requirement(),
            parameters: v.judge.parameters(),
        },
        guard: match v.guard {
            w::PolicyGuard::Guarded => p::Guard::Guarded,
            w::PolicyGuard::Observed(v) => p::Guard::Observed { freshness: v.freshness() },
        },
        must_be_guarded: v.must_be_guarded,
    }
}
fn policy(
    value: w::PolicyEdit,
    limits: &w::Limits,
    names: &mut List<Box<[u8]>>,
) -> Result<p::PolicyChange, Untranslatable> {
    Ok(match value {
        w::PolicyEdit::Templates(v) => super::operations::templates(v),
        w::PolicyEdit::ProjectSpend(v) => p::PolicyChange::ProjectSpend { period_spend: v.period_spend() },
        w::PolicyEdit::Role(v) => {
            let v = v.into_parts().value.into_parts();
            p::PolicyChange::Role(p::PolicyRole {
                number: v.number,
                authority: authority(v.authority, limits, names)?,
                period_spend: v.period_spend,
                requests: v.requests,
                decides: v.decides,
            })
        }
        w::PolicyEdit::Requirements(v) => {
            let path = w::Path::PolicyEditRequirementsRequirements;
            let count = count(v.requirements(), path)?;
            let mut values = List::with_capacity(count);
            let mut reader = Reader::new(v.requirements());
            let mut total = 0;
            for _ in 0..count {
                let bytes = item(&mut reader, path)?;
                let value = match w::PolicyRequirement::decode(limits, &mut Reader::new(&bytes)) {
                    Ok(value) => requirement(value),
                    Err(_) => return Err(Untranslatable { path }),
                };
                sum(
                    &mut total,
                    p::requirements_bytes(core::slice::from_ref(&value)),
                    limits.policy_edit_requirements_requirements,
                    path,
                )?;
                values.push(value).expect("counted pieces");
            }
            p::PolicyChange::Requirements { requirements: values.into_boxed() }
        }
        w::PolicyEdit::Projections(v) => p::PolicyChange::Projections {
            grants: grants(
                v.grants(),
                limits.policy_edit_projections_grants,
                w::Path::PolicyEditProjectionsGrants,
                limits,
            )?,
        },
        w::PolicyEdit::Permissions(v) => {
            let v = v.into_parts();
            let mut mappings = List::with_capacity(v.mappings.capacity());
            for value in v.mappings.into_boxed() {
                mappings
                    .push(p::PermissionRole {
                        connector: value.connector(),
                        permission: value.permission(),
                        role: value.role(),
                    })
                    .expect("same capacity");
            }
            p::PolicyChange::Permissions { mappings: mappings.into_boxed() }
        }
    })
}

fn pattern_bytes(value: &p::Pattern, fixed: usize) -> Option<u64> {
    let mut bytes = u64::try_from(fixed).ok()?;
    bytes = bytes.checked_add(
        u64::try_from(value.segments.len()).ok()?.checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?,
    )?;
    for segment in &value.segments {
        bytes = bytes.checked_add(u64::try_from(segment.len()).ok()?)?;
    }
    bytes.checked_add(match &value.last {
        p::Last::Exact(bytes) | p::Last::Open(bytes) => u64::try_from(bytes.len()).ok()?,
    })
}

fn account_holder(value: &w::AccountHolder) -> p::AccountHolder {
    match value {
        w::AccountHolder::Own => p::AccountHolder::Own,
        w::AccountHolder::Deployment => p::AccountHolder::Deployment,
    }
}

fn account_change(value: &w::AccountChange) -> Result<p::AccountChange, Untranslatable> {
    Ok(match value {
        w::AccountChange::DeclareProgram(row) => {
            if !p::program_name(row.name()) {
                return Err(Untranslatable { path: w::Path::AccountChangeDeclareProgramName });
            }
            p::AccountChange::DeclareProgram {
                provider: row.provider(),
                join: *row.join(),
                name: skein_lib::bytes::copy_of(row.name()),
            }
        }
        w::AccountChange::CheckProgram(row) => p::AccountChange::CheckProgram { source: row.source() },
        w::AccountChange::MoveOut(row) => p::AccountChange::MoveOut { account: row.account() },
        w::AccountChange::Written(row) => p::AccountChange::Written { number: row.number() },
        w::AccountChange::FinishSignIn(row) => {
            p::AccountChange::FinishSignIn { source: row.source(), digest: *row.digest() }
        }
        w::AccountChange::MoveIn(row) => p::AccountChange::MoveIn { digest: *row.digest() },
        w::AccountChange::AddKey(row) => {
            p::AccountChange::AddKey { provider: row.provider(), join: *row.join(), digest: *row.digest() }
        }
        w::AccountChange::ReplaceKey(row) => {
            p::AccountChange::ReplaceKey { source: row.source(), digest: *row.digest() }
        }
        w::AccountChange::RemoveKey(row) => p::AccountChange::RemoveKey { source: row.source() },
        w::AccountChange::Resume(row) => p::AccountChange::Resume { source: row.source() },
        w::AccountChange::Remove(row) => p::AccountChange::Remove { account: row.account() },
        w::AccountChange::MoveAccount(row) => {
            p::AccountChange::MoveAccount { account: row.account(), place: row.place() }
        }
        w::AccountChange::MoveSource(row) => {
            p::AccountChange::MoveSource { account: row.account(), source: row.source(), place: row.place() }
        }
        w::AccountChange::SetUses(row) => p::AccountChange::SetUses {
            account: row.account(),
            uses: match row.uses() {
                None => None,
                Some(uses) => {
                    let mut rows = List::with_capacity(uses.len());
                    for use_ in uses {
                        rows.push(match use_ {
                            w::OwnerAccountUse::Party(value) => p::Use::Party(value.party()),
                            w::OwnerAccountUse::Project(value) => p::Use::Project(value.project()),
                        })
                        .expect("reserved uses");
                    }
                    Some(rows.into_boxed())
                }
            },
        },
    })
}

#[expect(clippy::disallowed_methods, reason = "project names cross the wire as UTF-8 bytes")]
fn project_name(bytes: &[u8], path: w::Path) -> Result<Box<str>, Untranslatable> {
    match core::str::from_utf8(bytes) {
        Ok(name) => Ok(name.into()),
        Err(_) => Err(Untranslatable { path }),
    }
}
