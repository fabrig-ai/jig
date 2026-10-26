//! Whole-batch structural and financial admission (domain/tasks.md, sections 4, 6.7 and 11).
//! Dependencies, delegation and threshold queue waits are checked together
//! before mutation; tasks does not judge root-authorized authority.
#![expect(
    clippy::result_large_err,
    reason = "bounded owner refusal keeps exact revision and capacity facts inline instead of allocating on refusal"
)]
use crate::domain::{Domain, record};
use crate::{Authority, Contract, Executor, Last, Limits, New, Parameter, Party, Phase, Problem, Refusal, Spec, Was};

fn problem(task: Option<u64>, why: Refusal) -> Problem {
    Problem { task, why, blocked_by: None, capacity: None }
}

pub(crate) fn check(domain: &Domain, limits: &Limits, creator: Party, batch: &[New]) -> Result<(), Problem> {
    check_with_result_proposal(domain, limits, creator, batch, None, None)
}

/// Preflight an agent verdict's own delegates after its current run reservation and charge settle.
pub(crate) fn check_direct_followups(
    domain: &Domain,
    limits: &Limits,
    proposer: u64,
    charge: u64,
    batch: &[New],
) -> Result<(), Problem> {
    check_with_result_proposal(domain, limits, Party::Task(proposer), batch, None, Some((proposer, charge)))
}

/// A result proposal may admit its accepted batch while the proposer closes.
pub(crate) fn check_result_followups(
    domain: &Domain,
    limits: &Limits,
    creator: Party,
    proposal: u64,
    batch: &[New],
) -> Result<(), Problem> {
    check_with_result_proposal(domain, limits, creator, batch, Some(proposal), None)
}

fn check_with_result_proposal(
    domain: &Domain,
    limits: &Limits,
    creator: Party,
    batch: &[New],
    result_proposal: Option<u64>,
    direct_finish: Option<(u64, u64)>,
) -> Result<(), Problem> {
    let size = u32::try_from(batch.len()).unwrap_or(u32::MAX);
    if size == 0 {
        return Err(problem(None, Refusal::Empty));
    }
    if size > limits.batch {
        return Err(problem(None, Refusal::Batch));
    }
    if !live_room(domain, limits, size) {
        return Err(Problem::capacity(
            None,
            Refusal::Live,
            crate::Capacity::batch(creator, batch, None, crate::CapacityCause::Tasks { limit: limits.tasks }),
        ));
    }
    check_placed(domain, creator)?;
    let parent = match creator {
        Party::Task(number) => {
            let Some(parent) = record(domain, number) else {
                return Err(problem(Some(number), Refusal::Unknown));
            };
            if let Some(proposal) = result_proposal
                && !matches_result_action(parent, proposal, batch)
            {
                return Err(problem(Some(number), Refusal::Reference));
            }
            match &parent.phase {
                Phase::Closing(_)
                    if result_proposal.is_some()
                        && parent.result_proposal
                        && result_proposal_number(parent) == result_proposal => {}
                Phase::Closing(_) | Phase::Ended(_) => return Err(problem(Some(number), Refusal::State)),
                Phase::Held { was, .. } => match was {
                    Was::Closing(_)
                        if result_proposal.is_some()
                            && parent.result_proposal
                            && result_proposal_number(parent) == result_proposal => {}
                    Was::Closing(_) => return Err(problem(Some(number), Refusal::State)),
                    Was::Waiting | Was::Active(_) => {}
                },
                Phase::Waiting | Phase::Active(_) => {}
            }
            if u32::try_from(parent.delegates.len()).unwrap_or(u32::MAX).saturating_add(size) > limits.delegates {
                return Err(crate::capacity::batch_problem(
                    Some(number),
                    Refusal::Delegates,
                    creator,
                    batch,
                    crate::CapacityCause::Delegates { limit: limits.delegates },
                ));
            }
            let Some(result_bytes) =
                usize::try_from(limits.result_bytes).expect("u32 fits usize").checked_mul(batch.len())
            else {
                return Err(problem(Some(number), Refusal::Busy));
            };
            let mut news = 0_u32;
            for member in batch {
                match member.executor {
                    Executor::Person(_) => news = news.checked_add(1).expect("bounded person batch"),
                    Executor::Agent { .. } | Executor::Procedure { .. } => {}
                }
            }
            let Some(messages) = size.checked_add(news) else { return Err(problem(Some(number), Refusal::Busy)) };
            if !crate::inbox::room(domain, limits, number, messages, result_bytes) {
                return Err(Problem::capacity(
                    Some(number),
                    Refusal::Busy,
                    crate::Capacity::batch(creator, batch, Some(number), crate::capacity::inbox(number, limits)),
                ));
            }
            if parent.depth.saturating_add(1) > limits.depth {
                return Err(problem(Some(number), Refusal::Depth));
            }
            let Some(made) = crate::funders::tree_made(domain, number, parent.root) else {
                return Err(problem(Some(number), Refusal::Funding));
            };
            if match made.checked_add(size) {
                Some(total) => total > limits.tree_tasks,
                None => true,
            } {
                return Err(crate::capacity::batch_problem(
                    Some(number),
                    Refusal::Tree,
                    creator,
                    batch,
                    crate::CapacityCause::Tree { limit: limits.tree_tasks },
                ));
            }
            Some(parent)
        }
        Party::Person(_) | Party::Deployment { .. } => None,
    };
    check_members(domain, limits, creator, batch, parent)?;
    crate::holds::check_batch(domain, limits, creator, batch)?;
    if !crate::funders::can_reserve(domain, creator, batch, result_proposal, direct_finish) {
        return Err(problem(None, Refusal::Funding));
    }
    crate::waits::check_batch(domain, limits, creator, batch)?;
    Ok(())
}

fn matches_result_action(parent: &crate::TaskRecord, number: u64, batch: &[New]) -> bool {
    let proposed = match &parent.proposal {
        Some(proposal) if parent.result_proposal && proposal.number == number => match &proposal.action {
            crate::ProposalAction::Batch(members) => members,
            crate::ProposalAction::Settings { .. }
            | crate::ProposalAction::Effect { .. }
            | crate::ProposalAction::ProcedureEffect { .. }
            | crate::ProposalAction::Amend { .. }
            | crate::ProposalAction::Widen { .. }
            | crate::ProposalAction::Release { .. }
            | crate::ProposalAction::ReleasePlaced { .. } => return false,
        },
        Some(_) | None => return false,
    };
    if proposed.len() != batch.len() {
        return false;
    }
    for (old, new) in proposed.iter().zip(batch) {
        let mut authority = old.authority.clone();
        if let crate::Deadline::Span(_) = old.deadline {
            authority.budget.deadline = new.authority.budget.deadline;
        }
        if old.number != new.number
            || old.project != new.project
            || old.executor != new.executor
            || old.spec != new.spec
            || old.contract != new.contract
            || authority != new.authority
            || old.numbers != new.numbers
            || old.dependencies != new.dependencies
            || old.holdings != new.holdings
            || old.wake != new.wake
            || old.recurring != new.recurring
            || old.tracked != new.tracked
        {
            return false;
        }
    }
    true
}

#[expect(clippy::manual_map, reason = "step code uses a closed match rather than a closure")]
fn result_proposal_number(parent: &crate::TaskRecord) -> Option<u64> {
    if !parent.result_proposal {
        return None;
    }
    match &parent.proposal {
        Some(proposal) => Some(proposal.number),
        None => None,
    }
}

#[expect(clippy::too_many_lines, reason = "one batch member validates its complete creation shape")]
fn check_members(
    domain: &Domain,
    limits: &Limits,
    creator: Party,
    batch: &[New],
    parent: Option<&crate::TaskRecord>,
) -> Result<(), Problem> {
    for (at, new) in batch.iter().enumerate() {
        assert!(new.deadline == crate::Deadline::Named, "root resolves every made deadline");
        let number = Some(new.number);
        if !crate::flags::valid(new.flags, new.executor, creator) {
            return Err(problem(number, Refusal::Contract));
        }
        if domain.names.contains_key(&new.number) {
            return Err(problem(number, Refusal::Duplicate));
        }
        for earlier in batch.iter().take(at) {
            if earlier.number == new.number {
                return Err(problem(number, Refusal::Duplicate));
            }
        }
        if let Some(parent) = parent
            && new.project != parent.project
        {
            return Err(problem(number, Refusal::Project));
        }
        match creator {
            Party::Deployment { project } if project != new.project => return Err(problem(number, Refusal::Project)),
            Party::Deployment { .. } | Party::Task(_) | Party::Person(_) => {}
        }
        let mut in_project = 0_u32;
        for (_, id) in &domain.names {
            if domain.tasks.get(*id).expect("name indexes live task").record.project == new.project {
                in_project = in_project.saturating_add(1);
            }
        }
        for sibling in batch {
            if sibling.project == new.project {
                in_project = in_project.saturating_add(1);
            }
        }
        if in_project > limits.project_tasks {
            return Err(crate::capacity::batch_problem(
                number,
                Refusal::Project,
                creator,
                batch,
                crate::CapacityCause::ProjectTasks { limit: limits.project_tasks },
            ));
        }
        let mut adding = 0_u32;
        for row in batch {
            if row.project == new.project && row.tracked.is_some() {
                adding = adding.checked_add(1).expect("bounded batch");
            }
        }
        if let Err(why) = crate::goals::room(domain, new.project, adding, limits) {
            return Err(problem(number, why));
        }
        match new.executor {
            Executor::Agent { charter } => {
                if domain.charter_models(charter, domain.newest()).is_none() {
                    return Err(problem(number, Refusal::Executor));
                }
            }
            Executor::Procedure { code, .. } => {
                if code == 0 {
                    return Err(problem(number, Refusal::Executor));
                }
            }
            Executor::Person(crate::PersonAddress::Person(0)) => {
                return Err(problem(number, Refusal::Executor));
            }
            Executor::Person(crate::PersonAddress::Person(_) | crate::PersonAddress::Role(_)) => {}
        }
        if !valid_recurring(limits, creator, new) {
            return Err(problem(number, Refusal::Spec));
        }
        if !valid_spec(limits, &new.spec) {
            return Err(problem(number, Refusal::Spec));
        }
        if !crate::wake::valid(&new.wake) {
            return Err(problem(number, Refusal::Spec));
        }
        if !valid_contract(limits, &new.contract) {
            return Err(problem(number, Refusal::Contract));
        }
        if !valid_authority(limits, &new.authority) {
            return Err(problem(number, Refusal::AuthorityShape));
        }
        if let Some(choice) = &new.llm
            && let Err(why) = crate::llm::check(domain, new.executor, domain.newest(), &new.authority, choice)
        {
            return Err(problem(number, why));
        }
        if new.dependencies.len() > usize::try_from(limits.dependencies).expect("u32 fits usize") {
            return Err(problem(number, Refusal::Dependencies));
        }
        for (index, dependency) in new.dependencies.iter().enumerate() {
            for earlier in new.dependencies.iter().take(index) {
                if earlier == dependency {
                    return Err(problem(number, Refusal::Dependencies));
                }
            }
            let mut known = false;
            for sibling in batch {
                if sibling.number == *dependency {
                    known = true;
                }
            }
            if let Party::Person(_) = creator
                && let Some(row) = record(domain, *dependency)
            {
                known = row.project == new.project;
            }
            if !known && let Some(parent) = parent {
                for delegate in &parent.delegates {
                    if delegate == dependency && domain.names.contains_key(dependency) {
                        known = true;
                    }
                }
            }
            if !known && let Some(parent) = parent {
                known = contains(&parent.references, *dependency) && domain.names.contains_key(dependency);
            }
            if !known {
                return Err(problem(number, Refusal::Dependencies));
            }
        }
    }
    Ok(())
}

fn valid_recurring(limits: &Limits, creator: Party, new: &New) -> bool {
    match &new.recurring {
        Some(template) => {
            let source = match new.funder {
                crate::Funder::Period { project, .. } => project == new.project,
                crate::Funder::Task(_) | crate::Funder::Pool { .. } | crate::Funder::Recurring { .. } => false,
            };
            let procedure = match new.executor {
                Executor::Procedure { code, .. } => code != 0,
                Executor::Agent { .. } | Executor::Person(_) => false,
            };
            procedure
                && creator == (Party::Deployment { project: new.project })
                && source
                && template.key != 0
                && new.numbers == (crate::Numbers { budget: 0, spent: 0, spent_below: 0, reserved: 0 })
                && crate::recurring::valid_template(limits, new.project, new.authority.budget.spend, &template.batch)
        }
        None => true,
    }
}

pub(crate) fn contains(numbers: &[u64], number: u64) -> bool {
    for candidate in numbers {
        if *candidate == number {
            return true;
        }
    }
    false
}

/// Pure bounded shape check used by the root before retaining a call body.
#[must_use]
pub fn valid_spec(limits: &Limits, spec: &Spec) -> bool {
    if spec.parameters.len() > usize::try_from(limits.parameters).expect("u32 fits usize")
        || spec.inputs.len() > usize::try_from(limits.inputs).expect("u32 fits usize")
        || !text(&spec.words)
    {
        return false;
    }
    for (at, input) in spec.inputs.iter().enumerate() {
        for earlier in spec.inputs.iter().take(at) {
            if earlier == input {
                return false;
            }
        }
    }
    let mut bytes = spec.words.len();
    for parameter in &spec.parameters {
        match parameter {
            Parameter::Bytes { value, .. } => {
                if !text(value) {
                    return false;
                }
                let Some(total) = bytes.checked_add(value.len()) else {
                    return false;
                };
                bytes = total;
            }
            Parameter::Number { .. } | Parameter::Resource { .. } => {}
        }
    }
    bytes <= usize::try_from(limits.spec_bytes).expect("u32 fits usize")
}

#[expect(clippy::disallowed_methods, reason = "client/domain/contract.md, 5.3 requires UTF-8 text validation")]
fn text(bytes: &[u8]) -> bool {
    core::str::from_utf8(bytes).is_ok()
}

/// Pure bounded result-contract shape check before retaining a call body.
#[must_use]
pub fn valid_contract(limits: &Limits, contract: &Contract) -> bool {
    match contract {
        Contract::Approval { state, part } => {
            if state.len() > usize::try_from(limits.result_bytes).expect("u32 fits usize") {
                return false;
            }
            match &part.key {
                crate::PartKey::Number(_) => true,
                crate::PartKey::Path(segments) => {
                    if segments.len() > limits.hold_segments || segments.capacity() > limits.hold_segments {
                        return false;
                    }
                    let mut bytes = 0_usize;
                    for segment in segments {
                        let Some(total) = bytes.checked_add(segment.len()) else { return false };
                        bytes = total;
                    }
                    bytes <= usize::try_from(limits.hold_bytes).expect("u32 fits usize")
                }
            }
        }
        Contract::Report { words } | Contract::Change { words, .. } => *words <= limits.result_bytes,
        Contract::Verdict { choices } => {
            if choices.is_empty() || choices.len() > usize::try_from(limits.contract_choices).expect("u32 fits usize") {
                return false;
            }
            for (at, choice) in choices.iter().enumerate() {
                if !crate::verdict::rules_valid(choice, limits.result_bytes) || choice.followups > limits.batch {
                    return false;
                }
                for earlier in choices.iter().take(at) {
                    if earlier.code == choice.code {
                        return false;
                    }
                }
            }
            true
        }
    }
}

/// Pure bounded permission-value shape check before retaining a call body.
#[must_use]
pub fn valid_authority(limits: &Limits, value: &Authority) -> bool {
    if value.grants.len() > usize::try_from(limits.authority_grants).expect("u32 fits usize")
        || value.note_resources.len() > usize::try_from(limits.authority_grants).expect("u32 fits usize")
        || value.delegation.kinds.len() > usize::try_from(limits.executor_kinds).expect("u32 fits usize")
        || value.notes.0 & !7 != 0
    {
        return false;
    }
    let mut bytes = 0_usize;
    for grant in &value.grants {
        if grant.pattern.segments.len() > usize::try_from(limits.authority_segments).expect("u32 fits usize") {
            return false;
        }
        for segment in &grant.pattern.segments {
            let Some(total) = bytes.checked_add(segment.len()) else {
                return false;
            };
            bytes = total;
        }
        match &grant.pattern.last {
            Last::Exact(last) | Last::Open(last) => {
                let Some(total) = bytes.checked_add(last.len()) else {
                    return false;
                };
                bytes = total;
            }
        }
    }
    for scope in &value.note_resources {
        if scope.pattern.segments.len() > usize::try_from(limits.authority_segments).expect("u32 fits usize") {
            return false;
        }
        for segment in &scope.pattern.segments {
            let Some(total) = bytes.checked_add(segment.len()) else { return false };
            bytes = total;
        }
        match &scope.pattern.last {
            Last::Exact(last) | Last::Open(last) => {
                let Some(total) = bytes.checked_add(last.len()) else { return false };
                bytes = total;
            }
        }
    }
    bytes <= usize::try_from(limits.authority_bytes).expect("u32 fits usize")
}

/// Shape bound for a batch retained before its resources have been reported.
/// Structural and financial admission is still checked when the batch resumes.
#[must_use]
pub fn bounded_batch(limits: &Limits, batch: &[New]) -> bool {
    if batch.is_empty() || batch.len() > usize::try_from(limits.batch).expect("u32 fits usize") {
        return false;
    }
    for member in batch {
        if !valid_spec(limits, &member.spec)
            || !valid_contract(limits, &member.contract)
            || !valid_authority(limits, &member.authority)
            || member.dependencies.len() > usize::try_from(limits.dependencies).expect("u32 fits usize")
            || member.holdings.len() > usize::try_from(limits.holdings).expect("u32 fits usize")
        {
            return false;
        }
        for holding in &member.holdings {
            let name = match holding {
                crate::Holding::Write { resource, .. } => resource,
                crate::Holding::Slot { pool, .. } => pool,
            };
            if !crate::holds::valid_name(limits, name) {
                return false;
            }
        }
    }
    true
}

fn live_room(domain: &Domain, limits: &Limits, size: u32) -> bool {
    domain.names.len().saturating_add(size) <= limits.tasks
        && domain.tasks.capacity().saturating_sub(domain.tasks.len()) >= size
}

fn check_placed(domain: &Domain, creator: Party) -> Result<(), Problem> {
    let number = match creator {
        Party::Task(number) => number,
        Party::Person(_) | Party::Deployment { .. } => return Ok(()),
    };
    let Some(parent) = record(domain, number) else {
        return Err(problem(Some(number), Refusal::Unknown));
    };
    match parent.executor {
        Executor::Agent { .. } => {}
        Executor::Procedure { .. } | Executor::Person(_) => return Ok(()),
    }
    if let Some(hold) = crate::placed::delegation_hold(domain, number) {
        return Err(problem(Some(number), Refusal::Placed { hold }));
    }
    Ok(())
}
