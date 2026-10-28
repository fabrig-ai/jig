//! Party proposal shape and durable withdrawal (domain/tasks.md, section 9).
use crate::{
    Domain, Limits, PersonAction, PersonProposal, PersonProposalState, ProposalOutcome, Refusal, Request, Stored,
};
use alloc::boxed::Box;
use skein_lib::{Env, List, Queue, ReplyTo};

pub(crate) fn valid_action(proposal: &PersonProposal, limits: &Limits) -> bool {
    let batch = match &proposal.action {
        PersonAction::ReleasePlaced { task, hold, revision } => {
            return proposal.project != 0 && *task != 0 && *hold != 0 && *revision != 0 && !proposal.reason.is_empty();
        }
        PersonAction::Settings { against, items } => {
            return proposal.project == 0 && *against > 0 && valid_settings(items);
        }
        PersonAction::Batch(batch) => batch,
        PersonAction::Amend { task, amendment, .. } => {
            return proposal.project != 0 && *task != 0 && crate::stored::valid_amendment(limits, amendment);
        }
    };
    if proposal.project == 0 || !crate::batch::bounded_batch(limits, batch) {
        return false;
    }
    for member in batch {
        if member.number == 0
            || member.project != proposal.project
            || member.recurring.is_some()
            || !crate::wake::valid(&member.wake)
            || member.numbers.spent != 0
            || member.numbers.spent_below != 0
            || member.numbers.reserved != 0
        {
            return false;
        }
        match member.executor {
            crate::Executor::Agent { charter: 0 }
            | crate::Executor::Procedure { connector: 0, .. }
            | crate::Executor::Procedure { code: 0, .. }
            | crate::Executor::Person(crate::PersonAddress::Person(0)) => return false,
            crate::Executor::Agent { .. } | crate::Executor::Procedure { .. } | crate::Executor::Person(_) => {}
        }
        match member.funder {
            crate::Funder::Pool { project, person, .. }
                if project == proposal.project && person == proposal.proposer => {}
            crate::Funder::Pool { .. }
            | crate::Funder::Task(_)
            | crate::Funder::Period { .. }
            | crate::Funder::Recurring { .. } => return false,
        }
    }
    true
}

pub(crate) fn made(domain: &Domain, action: &PersonAction) -> bool {
    let batch = match action {
        PersonAction::Settings { .. } | PersonAction::ReleasePlaced { .. } => return true,
        PersonAction::Batch(batch) => batch,
        PersonAction::Amend { task, .. } => return crate::domain::record(domain, *task).is_some(),
    };
    for member in batch {
        if crate::domain::record(domain, member.number).is_none() {
            return false;
        }
    }
    true
}

pub(crate) fn withdraw(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: Option<ReplyTo>,
    proposer: u64,
    number: u64,
    out: &mut Queue<Request>,
) {
    let own = match domain.person_proposals.get(&number) {
        Some(row) => row.proposer == proposer,
        None => false,
    };
    // A release withdraws restored settings before task adoption (engine.md, 17.7).
    let restoring_settings = to.is_none()
        && domain.startup == crate::domain::Startup::Restoring
        && match domain.person_proposals.get(&number) {
            Some(row) => row.action.is_settings(),
            None => false,
        };
    let refusal = if !domain.ready() && !restoring_settings {
        Some(Refusal::NotReady)
    } else if own {
        None
    } else {
        Some(Refusal::Unknown)
    };
    if let Some(why) = refusal {
        if let Some(to) = to {
            crate::domain::refused(to, None, why, out);
        }
        return;
    }
    let mut proposal = domain.person_proposals.remove(&number).expect("pending proposal checked");
    let settings = match proposal.action {
        PersonAction::Settings { .. } => true,
        PersonAction::Batch(_) | PersonAction::Amend { .. } | PersonAction::ReleasePlaced { .. } => false,
    };
    proposal.state = PersonProposalState::Withdrawn { at: env.wall };
    out.push(Request::Erase { key: crate::Key::PersonProposal(number) });
    out.push(Request::Save {
        record: Stored::Update(crate::Update {
            party: proposal.proposer,
            at: env.wall,
            task: proposal.first(),
            object: crate::UpdateObject::Proposal { proposal: crate::Newest::of(proposal.number) },
        }),
    });
    out.push(Request::Save { record: Stored::DecidedProposal(Box::new(proposal)) });
    if !settings {
        crate::closing::prune_stubs(domain, env, out);
    }
    out.push(Request::PersonProposalDecided { reply_to: to, proposer, number, outcome: ProposalOutcome::Withdrawn });
}

pub(crate) fn ended(domain: &mut Domain, env: &Env<Limits>, task: u64, out: &mut Queue<Request>) {
    let mut pending = List::with_capacity(env.limits.tasks);
    for (&number, proposal) in &domain.person_proposals {
        let names = match &proposal.action {
            PersonAction::Batch(batch) => {
                let mut names = false;
                for member in batch {
                    if member.dependencies.contains(&task) {
                        names = true;
                        break;
                    }
                }
                names
            }
            PersonAction::Amend { task: target, .. } | PersonAction::ReleasePlaced { task: target, .. } => {
                *target == task
            }
            PersonAction::Settings { .. } => false,
        };
        if names {
            pending.push((number, proposal.proposer)).expect("bounded pending proposals");
        }
    }
    for (number, proposer) in pending.into_boxed() {
        withdraw(domain, env, None, proposer, number, out);
    }
}

pub(crate) fn check(domain: &Domain, env: &Env<Limits>, proposal: &PersonProposal) -> Result<(), Box<crate::Problem>> {
    if let PersonAction::ReleasePlaced { .. } = proposal.action {
        return match release_basis(domain, proposal) {
            Ok(_) => Ok(()),
            Err(why) => Err(Box::new(crate::Problem::new(Some(proposal.first()), why))),
        };
    }
    let PersonAction::Amend { task, amendment, .. } = &proposal.action else { return Ok(()) };
    let row =
        crate::domain::record(domain, *task).ok_or(Box::new(crate::Problem::new(Some(*task), Refusal::Unknown)))?;
    if row.project != proposal.project {
        return Err(Box::new(crate::Problem::new(Some(*task), Refusal::Unknown)));
    }
    let mut amendment = *amendment.clone();
    // Its authority is bounded above; its source is checked at acceptance.
    amendment.authority = None;
    let message = row.last_message.checked_add(1).ok_or(Box::new(crate::Problem::new(Some(*task), Refusal::Read)))?;
    crate::control::check_amend(
        domain,
        &crate::compound::Staged::new(0),
        env,
        crate::Party::Person(proposal.proposer),
        crate::control::Amend {
            places: Box::new([]),
            number: *task,
            message,
            stop_run: false,
            funder: None,
            amendment,
        },
    )?;
    Ok(())
}

pub(crate) fn valid_settings(items: &[crate::SettingHead]) -> bool {
    if items.is_empty() || items.len() > usize::try_from(crate::SETTINGS_ITEMS).expect("settings bound") {
        return false;
    }
    for (at, item) in items.iter().enumerate() {
        match item.setting {
            crate::Setting::Charter(0) | crate::Setting::Template(0) | crate::Setting::Policy(0) => {
                return false;
            }
            crate::Setting::Charter(_)
            | crate::Setting::Template(_)
            | crate::Setting::Policy(_)
            | crate::Setting::Rules => {}
        }
        for earlier in items.iter().take(at) {
            if earlier.setting == item.setting {
                return false;
            }
        }
    }
    true
}

impl Domain {
    /// Release proposals derive their sole recipient from the retained hold (tasks.md, 5.9).
    #[must_use]
    pub fn person_proposal_owner(&self, proposal: &PersonProposal) -> Option<crate::Owner> {
        let PersonAction::ReleasePlaced { hold, .. } = proposal.action else { return None };
        Some(self.placed_hold(hold)?.owner)
    }
}

pub(crate) fn release_plan(
    domain: &Domain,
    env: &Env<Limits>,
    proposal: &PersonProposal,
    by: crate::Party,
) -> Result<Option<crate::control::ReleasePlan>, Refusal> {
    let PersonAction::ReleasePlaced { task, .. } = proposal.action else { return Ok(None) };
    release_basis(domain, proposal)?;
    if !crate::control::allowed(domain, task, by, env.limits.tasks) {
        return Err(Refusal::Reference);
    }
    let plan = crate::control::check_release(domain, &crate::compound::Staged::new(0), env, task, by, None, None)?;
    Ok(Some(plan))
}

fn release_basis(domain: &Domain, proposal: &PersonProposal) -> Result<(), Refusal> {
    let PersonAction::ReleasePlaced { task, hold, revision } = proposal.action else { return Ok(()) };
    let current = domain.placed_hold(hold).ok_or(Refusal::Unknown)?;
    if current.lifted || current.revision != revision {
        return Err(Refusal::HoldRevision { revision: current.revision });
    }
    let target = crate::domain::record(domain, task).ok_or(Refusal::Unknown)?;
    if target.project != proposal.project || target.placed != Some(hold) {
        return Err(Refusal::State);
    }
    if !matches!(target.phase, crate::Phase::Held { .. }) && !crate::placed::draining(domain, target) {
        return Err(Refusal::State);
    }
    Ok(())
}
