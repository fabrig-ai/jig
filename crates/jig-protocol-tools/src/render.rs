//! Text answers from the core's parts (protocol/channel.md, section 6.3).
use crate::{Problem, Rendered, Why};
use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_core::{self as core, CallPart};
use skein_lib::{Decimal, List, Writer};

fn text(parts: &[&[u8]], error: bool) -> Rendered {
    let mut size = 0_usize;
    for part in parts {
        size = size.checked_add(part.len()).expect("bounded text parts");
    }
    let mut writer = Writer::new(size);
    for part in parts {
        writer.put(part).expect("measured text parts");
    }
    Rendered { error, text: writer.finish() }
}
#[must_use]
pub fn problem(tool: &[u8], problem: &Problem) -> Rendered {
    let why: &[u8] = match problem.why {
        Why::Malformed => b"is malformed",
        Why::Missing => b"is missing",
        Why::Type => b"has the wrong type",
        Why::Range => b"is outside its range",
        Why::TooLarge => b"is too large",
    };
    if problem.field.is_empty() {
        text(&[tool, b": the input ", why], true)
    } else {
        text(&[tool, b": the field ", problem.field, b" ", why], true)
    }
}
#[must_use]
pub fn render(part: &CallPart) -> Rendered {
    match part {
        CallPart::SettingsRefused(why) => settings_refused(why),
        CallPart::Effect { entry, outcome, .. } => {
            let status: &[u8] = match outcome {
                None => b"pending",
                Some(core::connector::OutboxOutcome::Made) => b"made",
                Some(core::connector::OutboxOutcome::Failed) => b"failed",
                Some(core::connector::OutboxOutcome::Withdrawn) => b"withdrawn",
                Some(core::connector::OutboxOutcome::NotMade) => b"not made",
                Some(core::connector::OutboxOutcome::Uncertain) => b"uncertain",
                Some(core::connector::OutboxOutcome::Held { .. }) => b"held",
            };
            text(&[b"Effect ", Decimal::of(*entry).as_bytes(), b": ", status], false)
        }
        CallPart::Connector { .. } => text(&[b"Connector answer"], false),
        CallPart::Proposed { proposal, holder } => proposed(*proposal, *holder),
        CallPart::ProposalDecided { proposal, outcome } => {
            let outcome: &[u8] = match outcome {
                jig_domain_tasks::ProposalOutcome::Accepted => b"accepted",
                jig_domain_tasks::ProposalOutcome::Rejected => b"rejected",
                jig_domain_tasks::ProposalOutcome::Withdrawn => b"withdrawn",
                jig_domain_tasks::ProposalOutcome::Stale => b"stale",
                jig_domain_tasks::ProposalOutcome::Passed => b"passed",
            };
            text(&[b"Already decided: proposal ", Decimal::of(*proposal).as_bytes(), b" was ", outcome], false)
        }
        CallPart::Sent { message } => text(&[b"Sent message ", Decimal::of(*message).as_bytes()], false),
        CallPart::Subscribed { subscription } => text(&[b"Subscribed ", Decimal::of(*subscription).as_bytes()], false),
        CallPart::Delegated(numbers) => delegated(numbers),
        CallPart::NoteWritten { name, revision } => text(
            &[
                b"Wrote note ",
                Decimal::of(*name).as_bytes(),
                b" revision ",
                Decimal::of(u64::from(*revision)).as_bytes(),
            ],
            false,
        ),
        CallPart::Controlled | CallPart::Introduced | CallPart::Unsubscribed | CallPart::EscalationDecided { .. } => {
            text(&[b"Done"], false)
        }
        CallPart::EffectDenied { answer, findings }
        | CallPart::ToolDenied { answer, findings }
        | CallPart::DelegationDenied { answer, findings } => denied(*answer, findings),
        CallPart::ControlDenied { answer } => denied(*answer, &[]),
        CallPart::EffectRefused(problem)
        | CallPart::EscalationRefused(problem)
        | CallPart::ProposalRefused(problem)
        | CallPart::ControlRefused(problem)
        | CallPart::MessageRefused(problem)
        | CallPart::SubscriptionRefused(problem)
        | CallPart::DelegationRefused(problem) => task_problem(problem),
        CallPart::NoteRecalled { entries, more } => Rendered { error: false, text: notes(entries, *more) },
        CallPart::NoteRefused(why) => {
            let why: &[u8] = match why {
                jig_domain_notes::Refusal::Busy => b"busy",
                jig_domain_notes::Refusal::Oversized => b"too large",
                jig_domain_notes::Refusal::Full => b"scope full",
                jig_domain_notes::Refusal::Exists => b"name exists",
                jig_domain_notes::Refusal::Missing => b"missing",
                jig_domain_notes::Refusal::Moved { .. } => b"revision changed",
                jig_domain_notes::Refusal::RevisionExhausted => b"revision exhausted",
            };
            text(&[b"Note refused: ", why], true)
        }
        CallPart::Revised { task, revision } => text(
            &[
                b"Done: task ",
                Decimal::of(*task).as_bytes(),
                b" is at spec revision ",
                Decimal::of(*revision).as_bytes(),
            ],
            false,
        ),
        CallPart::Moved { task, now } => moved(*task, now),
        CallPart::Again { task, required } => text(
            &[
                b"Refused: task ",
                Decimal::of(*task).as_bytes(),
                if *required {
                    b" holds an effect whose outcome is uncertain; say again: true or false"
                } else {
                    b" holds no uncertain effect; leave again out"
                },
            ],
            true,
        ),
        CallPart::PassedUp { task } => text(
            &[b"Beyond your authority: task ", Decimal::of(*task).as_bytes(), b"'s escalation was passed up"],
            false,
        ),
        CallPart::Unavailable => text(&[b"Unavailable"], true),
    }
}
fn delegated(members: &[core::Made]) -> Rendered {
    let mut parts = List::with_capacity(u32::try_from(members.len()).expect("bounded batch"));
    let mut size = b"Delegated tasks: ".len();
    for (at, member) in members.iter().enumerate() {
        let given = match member.given {
            Some(given) => text(&[b", given ", Decimal::of(given).as_bytes()], false).text,
            None => Box::new([]),
        };
        let line = text(
            &[
                if at == 0 { b"" } else { b"; " },
                Decimal::of(member.task).as_bytes(),
                b" at spec revision ",
                Decimal::of(member.revision).as_bytes(),
                &given,
            ],
            false,
        )
        .text;
        size = size.checked_add(line.len()).expect("bounded delegate answer");
        parts.push(line).expect("one line per delegate");
    }
    let mut writer = Writer::new(size);
    writer.put(b"Delegated tasks: ").expect("heading");
    for part in &parts {
        writer.put(part).expect("delegate");
    }
    Rendered { error: false, text: writer.finish() }
}

fn denied(answer: authority::Answer, findings: &[authority::Finding]) -> Rendered {
    let status: &[u8] = match answer {
        authority::Answer::Allow => b"Allowed",
        authority::Answer::Wait => b"Waiting",
        authority::Answer::Propose => b"Beyond authority",
        authority::Answer::Refuse => b"Refused",
    };
    let count = u32::try_from(findings.len())
        .expect("bounded findings")
        .checked_mul(9)
        .expect("nine authority components")
        .checked_add(1)
        .expect("status");
    let mut parts = List::with_capacity(count);
    parts.push(Box::from(status)).expect("status slot");
    for finding in findings {
        match finding {
            authority::Finding::Authority { lacks, .. } => {
                for (missing, name) in [
                    (lacks.tools, b"tools".as_slice()),
                    (lacks.grants, b"grants"),
                    (lacks.executors, b"executors"),
                    (lacks.tasks, b"tasks"),
                    (lacks.depth, b"depth"),
                    (lacks.spend, b"spend"),
                    (lacks.deadline, b"deadline"),
                    (lacks.notes, b"notes"),
                    (lacks.models, b"models"),
                ] {
                    if missing {
                        parts.push(text(&[b"; missing ", name], true).text).expect("component slot");
                    }
                }
            }
            other @ (authority::Finding::Oversized
            | authority::Finding::UnknownProject
            | authority::Finding::UnknownRole
            | authority::Finding::Executor { .. }
            | authority::Finding::Tasks { .. }
            | authority::Finding::Spend { .. }
            | authority::Finding::Arithmetic
            | authority::Finding::RunBudget
            | authority::Finding::RunCap
            | authority::Finding::Deadline
            | authority::Finding::Account
            | authority::Finding::Writer
            | authority::Finding::Tool
            | authority::Finding::Grant { .. }
            | authority::Finding::Price { .. }
            | authority::Finding::ResourceAccess
            | authority::Finding::ResourceUnavailable
            | authority::Finding::Reference
            | authority::Finding::Scope { .. }
            | authority::Finding::Required { .. }
            | authority::Finding::Failed { .. }
            | authority::Finding::Unguarded { .. }
            | authority::Finding::Unpermitted
            | authority::Finding::Undecidable
            | authority::Finding::PeriodSpend) => {
                parts.push(text(&[b"; ", finding_text(other)], true).text).expect("finding slot");
            }
        }
    }
    let mut bytes = 0_usize;
    for part in &parts {
        bytes = bytes.checked_add(part.len()).expect("bounded findings text");
    }
    let mut writer = Writer::new(bytes);
    for part in &parts {
        writer.put(part).expect("measured findings");
    }
    Rendered { error: true, text: writer.finish() }
}
fn finding_text(finding: &authority::Finding) -> &'static [u8] {
    match finding {
        authority::Finding::Oversized => b"input too large",
        authority::Finding::UnknownProject => b"unknown project",
        authority::Finding::UnknownRole => b"unknown role",
        authority::Finding::Authority { .. } => b"missing authority",
        authority::Finding::Executor { .. } => b"executor not permitted",
        authority::Finding::Tasks { .. } => b"task capacity",
        authority::Finding::Spend { .. } => b"spend capacity",
        authority::Finding::Arithmetic => b"unrepresentable accounting",
        authority::Finding::RunBudget => b"run budget too small",
        authority::Finding::RunCap => b"run budget exceeds cap",
        authority::Finding::Deadline => b"deadline passed",
        authority::Finding::Account => b"account unavailable",
        authority::Finding::Writer => b"writer unavailable",
        authority::Finding::Tool => b"missing tool authority",
        authority::Finding::Grant { .. } => b"missing grant",
        authority::Finding::Price { .. } => b"price exceeds funding",
        authority::Finding::ResourceAccess => b"resource access denied",
        authority::Finding::ResourceUnavailable => b"resource unavailable",
        authority::Finding::Reference => b"missing reference",
        authority::Finding::Scope { .. } => b"missing note scope",
        authority::Finding::Required { .. } => b"required fact missing",
        authority::Finding::Failed { .. } => b"required fact failed",
        authority::Finding::Unguarded { .. } => b"required guard missing",
        authority::Finding::Unpermitted => b"permission missing",
        authority::Finding::Undecidable => b"decision permission missing",
        authority::Finding::PeriodSpend => b"period spend exceeds ceiling",
    }
}

fn task_problem(problem: &jig_domain_tasks::Problem) -> Rendered {
    let why: &[u8] = match problem.why {
        jig_domain_tasks::Refusal::Placed { .. } => b"held by a placed hold",
        jig_domain_tasks::Refusal::HoldOwner { .. } => b"only the placed hold's owner may release it",
        jig_domain_tasks::Refusal::HoldRevision { .. } => b"placed hold revision changed",
        jig_domain_tasks::Refusal::Unproposable => b"no proposal could be accepted",
        jig_domain_tasks::Refusal::PersonRevision { .. } => b"person revision",
        jig_domain_tasks::Refusal::ResourceUnavailable => b"resource unavailable",
        jig_domain_tasks::Refusal::NotReady => b"not ready",
        jig_domain_tasks::Refusal::Unknown => b"unknown",
        jig_domain_tasks::Refusal::Busy => b"busy",
        jig_domain_tasks::Refusal::Duplicate => b"duplicate",
        jig_domain_tasks::Refusal::Empty => b"empty",
        jig_domain_tasks::Refusal::Batch => b"batch",
        jig_domain_tasks::Refusal::Live => b"live",
        jig_domain_tasks::Refusal::Project => b"project",
        jig_domain_tasks::Refusal::Tree => b"tree",
        jig_domain_tasks::Refusal::Depth => b"depth",
        jig_domain_tasks::Refusal::Delegates => b"delegates",
        jig_domain_tasks::Refusal::Reference => b"reference",
        jig_domain_tasks::Refusal::Subscription => b"subscription",
        jig_domain_tasks::Refusal::Dependencies => b"dependencies",
        jig_domain_tasks::Refusal::Circle { cut } => {
            return dependency_list(
                problem,
                b"these tasks would wait for one another",
                b"these tasks would wait for one another: T",
                b", T",
                b"",
                if cut { b"; some task lists are incomplete" } else { b"" },
            );
        }
        jig_domain_tasks::Refusal::DependencyEnded { how } => {
            let tasks = problem.blocked_by.as_deref().unwrap_or(&[]);
            let Some(task) = tasks.first() else {
                return text(&[b"Refused: a dependency has ended"], true);
            };
            let task = Decimal::of(*task);
            return match how {
                jig_domain_tasks::Status::Failed => {
                    text(&[b"Refused: dependency T", task.as_bytes(), b" failed"], true)
                }
                jig_domain_tasks::Status::Cancelled => {
                    text(&[b"Refused: dependency T", task.as_bytes(), b" was cancelled"], true)
                }
                jig_domain_tasks::Status::Done => text(&[b"Refused: T", task.as_bytes(), b" is done"], true),
            };
        }
        jig_domain_tasks::Refusal::Kept => {
            return dependency_list(
                problem,
                b"kept by its dependencies",
                b"kept until T",
                b", kept until T",
                b" is done",
                b"",
            );
        }
        jig_domain_tasks::Refusal::Executor => b"executor",
        jig_domain_tasks::Refusal::Spec => b"spec",
        jig_domain_tasks::Refusal::Contract => b"contract",
        jig_domain_tasks::Refusal::AuthorityShape => b"authority shape",
        jig_domain_tasks::Refusal::Inputs => b"inputs",
        jig_domain_tasks::Refusal::State => b"state",
        jig_domain_tasks::Refusal::Goals => b"goals",
        jig_domain_tasks::Refusal::Grace => b"grace",
        jig_domain_tasks::Refusal::Attempt => b"attempt",
        jig_domain_tasks::Refusal::LiveDelegates => b"live delegates",
        jig_domain_tasks::Refusal::Restore => b"restore",
        jig_domain_tasks::Refusal::Read => b"read",
        jig_domain_tasks::Refusal::Turn => b"turn",
        jig_domain_tasks::Refusal::Funding => b"funding",
        jig_domain_tasks::Refusal::HoldKind => b"hold kind",
        jig_domain_tasks::Refusal::HoldTaken => b"hold taken",
        jig_domain_tasks::Refusal::Holds => b"holds",
        jig_domain_tasks::Refusal::Account { why, .. } => match why {
            jig_domain_tasks::AccountWhy::Unknown => b"account not found",
            jig_domain_tasks::AccountWhy::Provider => b"account belongs to another provider",
            jig_domain_tasks::AccountWhy::Uses => b"account does not serve the payer",
            jig_domain_tasks::AccountWhy::Limited => b"kept account is limited",
        },
        jig_domain_tasks::Refusal::Llm { why, .. } => match why {
            jig_domain_tasks::LlmFault::Unconfigured => b"model not configured",
            jig_domain_tasks::LlmFault::Effort => b"effort not declared",
            jig_domain_tasks::LlmFault::Charter => b"model outside the charter",
            jig_domain_tasks::LlmFault::Authority => b"model outside authority",
            jig_domain_tasks::LlmFault::Procedure => b"no model runs this task",
            jig_domain_tasks::LlmFault::Repeated => b"model repeated",
        },
    };
    if let jig_domain_tasks::Refusal::Account { account, .. } = problem.why {
        let account = Decimal::of(u64::from(account));
        return text(&[b"Refused account ", account.as_bytes(), b": ", why], true);
    }
    text(&[b"Refused: ", why], true)
}

// The client's dependency phrases in one line (domain/tasks.md, sections 5.7 and 6.7).
fn dependency_list(
    problem: &jig_domain_tasks::Problem,
    plain: &[u8],
    first: &[u8],
    next: &[u8],
    each_tail: &[u8],
    tail: &[u8],
) -> Rendered {
    let tasks = problem.blocked_by.as_deref().unwrap_or(&[]);
    if tasks.is_empty() {
        return text(&[b"Refused: ", plain], true);
    }
    let mut size = b"Refused: ".len();
    size = size.checked_add(tail.len()).expect("dependency tail");
    for (at, task) in tasks.iter().enumerate() {
        let prefix = if at == 0 { first } else { next };
        size = size
            .checked_add(prefix.len())
            .expect("dependency prefix")
            .checked_add(Decimal::of(*task).as_bytes().len())
            .expect("dependency number")
            .checked_add(each_tail.len())
            .expect("bounded dependency text");
    }
    let mut writer = Writer::new(size);
    writer.put(b"Refused: ").expect("refusal heading");
    for (at, task) in tasks.iter().enumerate() {
        writer.put(if at == 0 { first } else { next }).expect("dependency prefix");
        writer.put(Decimal::of(*task).as_bytes()).expect("dependency number");
        writer.put(each_tail).expect("dependency status");
    }
    writer.put(tail).expect("dependency tail");
    Rendered { error: true, text: writer.finish() }
}

fn notes(entries: &[jig_domain_notes::Entry], more: bool) -> Box<[u8]> {
    let tail = if more { b"\nMore notes remain.".as_slice() } else { b"\nEnd of notes.".as_slice() };
    let mut bytes = b"Notes".len().checked_add(tail.len()).expect("page framing");
    for entry in entries {
        bytes = bytes
            .checked_add(b"\nNote ".len())
            .expect("note heading")
            .checked_add(Decimal::of(entry.name).as_bytes().len())
            .expect("note identity")
            .checked_add(b" revision ".len())
            .expect("revision heading")
            .checked_add(Decimal::of(u64::from(entry.revision)).as_bytes().len())
            .expect("revision")
            .checked_add(2)
            .expect("line breaks")
            .checked_add(entry.description.len())
            .expect("description")
            .checked_add(entry.body.len())
            .expect("body")
            .checked_add(b"\nReferences:".len())
            .expect("references heading");
        for reference in &entry.references {
            bytes = bytes
                .checked_add(1)
                .expect("reference separator")
                .checked_add(Decimal::of(*reference).as_bytes().len())
                .expect("reference");
        }
    }
    let mut writer = Writer::new(bytes);
    writer.put(b"Notes").expect("page heading");
    for entry in entries {
        writer.put(b"\nNote ").expect("note heading");
        writer.put(Decimal::of(entry.name).as_bytes()).expect("note identity");
        writer.put(b" revision ").expect("revision heading");
        writer.put(Decimal::of(u64::from(entry.revision)).as_bytes()).expect("revision");
        writer.put(b"\n").expect("line break");
        writer.put(&entry.description).expect("description");
        writer.put(b"\n").expect("line break");
        writer.put(&entry.body).expect("body");
        writer.put(b"\nReferences:").expect("references heading");
        for reference in &entry.references {
            writer.put(b" ").expect("reference separator");
            writer.put(Decimal::of(*reference).as_bytes()).expect("reference");
        }
    }
    writer.put(tail).expect("page tail");
    writer.finish()
}

fn moved(task: u64, now: &jig_domain_people::Moved) -> Rendered {
    use jig_domain_people::Moved;
    match now {
        Moved::Spec { revision } => text(
            &[
                b"Moved: task ",
                Decimal::of(task).as_bytes(),
                b" is at spec revision ",
                Decimal::of(*revision).as_bytes(),
                b" now",
            ],
            true,
        ),
        Moved::Person { revision } => text(
            &[
                b"Moved: task ",
                Decimal::of(task).as_bytes(),
                b" is at person revision ",
                Decimal::of(*revision).as_bytes(),
                b" now",
            ],
            true,
        ),
        Moved::Note { revision } => text(
            &[
                b"Moved: note ",
                Decimal::of(task).as_bytes(),
                b" is at revision ",
                Decimal::of(u64::from(*revision)).as_bytes(),
                b" now",
            ],
            true,
        ),
        Moved::Hold { revision, reason } => text(
            &[
                b"Moved: task ",
                Decimal::of(task).as_bytes(),
                b" is held at hold revision ",
                Decimal::of(*revision).as_bytes(),
                b" now, ",
                hold_reason(reason),
            ],
            true,
        ),
    }
}
fn hold_reason(reason: &jig_domain_people::HoldReason) -> &'static [u8] {
    use jig_domain_people::{FailureClass, HoldReason};
    match reason {
        HoldReason::Exceeded(value) => match value.bound {
            jig_domain_people::Bound::Reasoning => b"for its configured reasoning bound",
            jig_domain_people::Bound::Metadata => b"for its configured metadata bound",
            jig_domain_people::Bound::Items => b"for its configured answer items bound",
            jig_domain_people::Bound::Answer => b"for its configured answer bytes bound",
            jig_domain_people::Bound::Tools => b"for its configured tools bound",
            jig_domain_people::Bound::HistoryItems => b"for its configured request items bound",
            jig_domain_people::Bound::History => b"for its configured history bound",
            jig_domain_people::Bound::Window => b"for its configured window bound",
            jig_domain_people::Bound::Output => b"for its configured output bound",
        },
        HoldReason::Failures { class } => match class {
            FailureClass::Transient => b"for transient failures",
            FailureClass::Permanent => b"for permanent failures",
            FailureClass::Run => b"for run failures",
            FailureClass::Agent => b"for agent failures",
            FailureClass::Lost => b"for lost runs",
            FailureClass::Invalid => b"for invalid results",
        },
        HoldReason::Stalled => b"because it stalled",
        HoldReason::Dependency { .. } => b"for its dependency",
        HoldReason::Stopped => b"because it was stopped",
        HoldReason::Placed { .. } => b"Held by a placed hold",
        HoldReason::Drift => b"for drift",
        HoldReason::Effects => b"for its effects",
        HoldReason::EffectFailed => b"for a failed effect",
        HoldReason::Uncertain { .. } => b"for an uncertain effect",
        HoldReason::Budget => b"for its budget",
        HoldReason::Deadline => b"for its deadline",
        HoldReason::HoldsWaited => b"for waiting on holds",
        HoldReason::ResourceUnavailable => b"for an unavailable resource",
        HoldReason::ResourceAccess => b"for resource access",
        HoldReason::PoolLost { .. } => b"for a lost pool",
        HoldReason::Procedure => b"for its procedure",
        HoldReason::Circle => b"for a circle of waits",
    }
}

fn proposed(proposal: u64, holder: jig_domain_tasks::ProposalHolder) -> Rendered {
    use jig_domain_tasks::{ProposalHolder, ProposalKind};
    let to = match holder {
        ProposalHolder::Task(task) => text(&[b"task ", Decimal::of(task).as_bytes()], false).text,
        ProposalHolder::Person(person) => text(&[b"person ", Decimal::of(person).as_bytes()], false).text,
        ProposalHolder::Policy { kind, .. } => Box::from(match kind {
            ProposalKind::Batch => b"the project's role for batches".as_slice(),
            ProposalKind::Effect => b"the project's role for effects",
            ProposalKind::Amend => b"the project's role for amendments",
            ProposalKind::Widen => b"the project's role for widenings",
            ProposalKind::Release => b"the project's role for releases",
            ProposalKind::Settings => b"the deployment's owners",
        }),
    };
    text(&[b"Proposed ", Decimal::of(proposal).as_bytes(), b", to ", &to], false)
}

fn settings_refused(why: &jig_domain_people::Refusal) -> Rendered {
    use jig_domain_people::{Lack, LimitKind, Refusal};
    if let Refusal::Stale { newest } = why {
        return text(&[b"Settings refused: stale; newest ", Decimal::of(u64::from(*newest)).as_bytes()], true);
    }
    let words: &[u8] = if let Refusal::NotWellFormed { .. } = why {
        b"malformed set"
    } else if let Refusal::Limit { which: LimitKind::SettingsProposals } = why {
        b"pending settings proposals limit"
    } else if let Refusal::Authority { lacking: Lack::Approver, .. } = why {
        b"no owner to accept"
    } else {
        b"refused"
    };
    text(&[b"Settings refused: ", words], true)
}

/// A short protocol-stand summary; applications render the complete documents.
#[must_use]
pub fn settings_answer(answer: &core::SettingsAnswer) -> Rendered {
    match answer {
        core::SettingsAnswer::Inventory(value) => text(
            &[
                b"Settings newest ",
                Decimal::of(u64::from(value.newest)).as_bytes(),
                b"; pending ",
                Decimal::of(u64::from(value.pending.len())).as_bytes(),
            ],
            false,
        ),
        core::SettingsAnswer::Setting(value) => {
            text(&[b"Setting at revision ", Decimal::of(u64::from(value.revision)).as_bytes()], false)
        }
        core::SettingsAnswer::Revision(core::reads::RevisionAnswer::Revision { revision, .. }) => {
            text(&[b"Revision ", Decimal::of(u64::from(revision.head.number)).as_bytes()], false)
        }
        core::SettingsAnswer::Revision(core::reads::RevisionAnswer::Forgotten { oldest }) => {
            text(&[b"Forgotten; oldest ", Decimal::of(u64::from(*oldest)).as_bytes()], true)
        }
        core::SettingsAnswer::Revision(core::reads::RevisionAnswer::Unknown) | core::SettingsAnswer::Unknown => {
            text(&[b"Unknown settings"], true)
        }
        core::SettingsAnswer::Revisions(value) => {
            text(&[b"Revisions: ", Decimal::of(u64::from(value.rows.len())).as_bytes(), b" rows"], false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jig_domain_tasks::{Problem, Refusal, Status};

    const CIRCLE: &[u64] = &[3, 5, 7];
    const ENDED: &[u64] = &[4];
    const KEPT: &[u64] = &[4, 6];

    fn refusal(why: Refusal, tasks: &[u64], expected: &[u8]) {
        let problem = Problem::naming(None, why, tasks);
        assert_eq!(render(&CallPart::ControlRefused(problem)), Rendered { error: true, text: expected.into() },);
    }

    #[test]
    fn a_circle_names_its_tasks_in_order() {
        refusal(Refusal::Circle { cut: false }, CIRCLE, b"Refused: these tasks would wait for one another: T3, T5, T7");
    }

    #[test]
    fn a_cut_circle_says_some_lists_are_incomplete() {
        refusal(
            Refusal::Circle { cut: true },
            CIRCLE,
            b"Refused: these tasks would wait for one another: T3, T5, T7; some task lists are incomplete",
        );
    }

    #[test]
    fn a_failed_dependency_is_named() {
        refusal(Refusal::DependencyEnded { how: Status::Failed }, ENDED, b"Refused: dependency T4 failed");
    }

    #[test]
    fn a_cancelled_dependency_is_named() {
        refusal(Refusal::DependencyEnded { how: Status::Cancelled }, ENDED, b"Refused: dependency T4 was cancelled");
    }

    #[test]
    fn a_done_dependency_is_named() {
        refusal(Refusal::DependencyEnded { how: Status::Done }, ENDED, b"Refused: T4 is done");
    }

    #[test]
    fn a_kept_task_names_each_pending_dependency() {
        refusal(Refusal::Kept, KEPT, b"Refused: kept until T4 is done, kept until T6 is done");
    }

    #[test]
    fn dependency_refusals_without_names_keep_their_plain_lines() {
        for (why, expected) in [
            (Refusal::Circle { cut: false }, b"Refused: these tasks would wait for one another".as_slice()),
            (Refusal::Circle { cut: true }, b"Refused: these tasks would wait for one another"),
            (Refusal::DependencyEnded { how: Status::Failed }, b"Refused: a dependency has ended"),
            (Refusal::Kept, b"Refused: kept by its dependencies"),
        ] {
            for blocked_by in [None, Some(Box::from([]))] {
                let problem = Problem { blocked_by, ..Problem::new(None, why) };
                assert_eq!(render(&CallPart::ControlRefused(problem)), Rendered { error: true, text: expected.into() },);
            }
        }
    }
}
