//! A party's batch (domain/tasks.md, sections 3 and 4).
use crate::{Authority, Grant, Parameter, PersonAddress, WakePolicy};
use alloc::boxed::Box;
/// A party's explicitly proposed action.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Action {
    Batch(Box<[Member]>),
    /// A held deployment settings set, identified by its request digest.
    Settings {
        against: u32,
        digest: [u8; 32],
    },
    /// Amend a task at the displayed specification revision.
    Amend {
        task: u64,
        revision: u64,
        amendment: Box<crate::Amendment>,
    },
}

pub(crate) fn invalid(limits: &crate::Limits, batch: &[Member]) -> crate::LimitKind {
    if batch.is_empty() {
        crate::LimitKind::Empty
    } else if batch.len() > usize::try_from(limits.batch).expect("u32 fits usize") {
        crate::LimitKind::Batch
    } else {
        crate::LimitKind::Words
    }
}

/// One root requested by a party, mirroring the tasks child's New.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Member {
    pub executor: TaskExecutor,
    pub words: Box<[u8]>,
    pub parameters: Box<[Parameter]>,
    pub inputs: Box<[u64]>,
    pub contract: Contract,
    pub authority: Authority,
    pub budget: Budget,
    pub deadline: Deadline,
    pub wake: WakePolicy,
    pub dependencies: Box<[Dependency]>,
    pub named: Box<[Grant]>,
    pub tracked: Option<u32>,
    pub label: u32,
    pub flags: Flags,
    /// The agent task's LLM choice; `None` takes its charter's default (domain/tasks.md, 3).
    pub llm: Option<crate::LlmChoice>,
}
/// Who carries out a member.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TaskExecutor {
    Agent { charter: u32 },
    Procedure { connector: u16, code: u32 },
    Person(PersonAddress),
}
/// The member's result contract, mirroring tasks' contracts.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Contract {
    Report { words: u32 },
    Change { connector: u16, kind: u16, words: u32 },
    Verdict { choices: Box<[Verdict]> },
}
/// How much of the party's available budget to give a member.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Budget {
    Named,
    AsAllowed { cap: Option<u64> },
}
/// The request deadline retained until the task-making decision.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Deadline {
    Named,
    Span(skein_lib::Duration),
}
/// A dependency within the batch or the party's reach.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Dependency {
    Batch(u32),
    Existing(u64),
}
/// The resolved amount of a member whose budget was `AsAllowed`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Given {
    pub task: u64,
    pub amount: u64,
}
/// One allowed verdict.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Verdict {
    pub code: u32,
    pub words: u32,
    pub followups: u32,
    pub fields: Box<[FieldRule]>,
}
/// A required machine field.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct FieldRule {
    pub name: Box<[u8]>,
    pub max: u32,
}
/// Task flags, mirroring the tasks child's flags.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Flags {
    pub conversation: bool,
    pub listed: bool,
    pub retention: Retention,
    pub ended_by: EndedBy,
}
impl Flags {
    pub const PLAIN: Flags =
        Flags { ended_by: EndedBy::Result, conversation: false, listed: false, retention: Retention::Kept };
}
/// A task's retention class.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Retention {
    Kept,
    ForgottenOnceRead,
}
/// Checked owned payload, excluding the inline member.
#[must_use]
pub fn member_bytes(member: &Member) -> Option<u64> {
    let mut total = crate::authority_bytes(&member.authority)?.checked_add(u64::try_from(member.words.len()).ok()?)?;
    total = total.checked_add(
        u64::try_from(member.parameters.len()).ok()?.checked_mul(u64::try_from(size_of::<Parameter>()).ok()?)?,
    )?;
    total = total.checked_add(
        u64::try_from(member.dependencies.len()).ok()?.checked_mul(u64::try_from(size_of::<Dependency>()).ok()?)?,
    )?;
    total = total.checked_add(u64::try_from(member.inputs.len()).ok()?.checked_mul(8)?)?;
    let mut named = member.authority.clone();
    named.grants.clone_from(&member.named);
    named.delegation.kinds = Box::new([]);
    named.note_resources = Box::new([]);
    total = total.checked_add(crate::authority_bytes(&named)?)?;
    total = total.checked_add(crate::llm_bytes(member.llm.as_ref())?)?;
    for parameter in &member.parameters {
        match parameter {
            Parameter::Bytes { value, .. } => total = total.checked_add(u64::try_from(value.len()).ok()?)?,
            Parameter::Number { .. } | Parameter::Resource { .. } => {}
        }
    }
    match &member.contract {
        Contract::Report { .. } | Contract::Change { .. } => {}
        Contract::Verdict { choices } => {
            total = total.checked_add(
                u64::try_from(choices.len()).ok()?.checked_mul(u64::try_from(size_of::<Verdict>()).ok()?)?,
            )?;
            for choice in choices {
                total = total.checked_add(
                    u64::try_from(choice.fields.len())
                        .ok()?
                        .checked_mul(u64::try_from(size_of::<FieldRule>()).ok()?)?,
                )?;
                for field in &choice.fields {
                    total = total.checked_add(u64::try_from(field.name.len()).ok()?)?;
                }
            }
        }
    }
    Some(total)
}
pub(crate) fn valid(limits: &crate::Limits, batch: &[Member]) -> bool {
    if batch.is_empty() || batch.len() > usize::try_from(limits.batch).expect("u32 fits usize") {
        return false;
    }
    for member in batch {
        if member.words.len() > usize::try_from(limits.words).expect("u32 fits usize")
            || !crate::llm::valid(member.llm.as_ref())
        {
            return false;
        }
        match member_bytes(member) {
            Some(bytes) if bytes <= u64::from(limits.amendment_bytes) => {}
            Some(_) | None => return false,
        }
    }
    true
}

/// The immutable authority that may end a task (domain/tasks.md, 5.8).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EndedBy {
    Result,
    Requester,
}

/// An open task is an agent's conversation (5.8).
#[must_use]
pub fn shaped(member: &Member) -> bool {
    if member.flags.ended_by == EndedBy::Result {
        return true;
    }
    let agent = match member.executor {
        TaskExecutor::Agent { .. } => true,
        TaskExecutor::Person(_) | TaskExecutor::Procedure { .. } => false,
    };
    member.flags.conversation && agent
}

pub(crate) fn restore_shaped(ask: &crate::Ask) -> bool {
    let batch = match ask {
        crate::Ask::Create { batch, .. } | crate::Ask::Propose { action: Action::Batch(batch), .. } => batch,
        crate::Ask::Propose { action: Action::Amend { .. } | Action::Settings { .. }, .. }
        | crate::Ask::Compound { .. }
        | crate::Ask::MarkSeen { .. }
        | crate::Ask::Application { .. }
        | crate::Ask::EditNote { .. }
        | crate::Ask::MakeService { .. }
        | crate::Ask::Adopt { .. }
        | crate::Ask::Withdraw { .. }
        | crate::Ask::Stop { .. }
        | crate::Ask::Cancel { .. }
        | crate::Ask::Release { .. }
        | crate::Ask::TakePerson { .. }
        | crate::Ask::HandBackPerson { .. }
        | crate::Ask::AnswerPerson { .. }
        | crate::Ask::Move { .. }
        | crate::Ask::DecideProposal { .. }
        | crate::Ask::Say { .. }
        | crate::Ask::AnswerQuestion { .. }
        | crate::Ask::Amend { .. }
        | crate::Ask::SetRoles { .. }
        | crate::Ask::ChangePolicy { .. }
        | crate::Ask::Rename { .. }
        | crate::Ask::MakeProject { .. }
        | crate::Ask::ChangeSettings { .. }
        | crate::Ask::MoveToNewest { .. }
        | crate::Ask::SetPool { .. }
        | crate::Ask::DecideEscalation { .. }
        | crate::Ask::AccountSignIn { .. }
        | crate::Ask::AccountSignOut { .. }
        | crate::Ask::AccountChange { .. } => return true,
    };
    for member in batch {
        if !shaped(member) {
            return false;
        }
    }
    true
}
