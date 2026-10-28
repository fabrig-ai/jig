//! Cold admission of bounded live rows and complete link validation
//! (domain/tasks.md, sections 2 and 5). Live/Ledger/Stub enter startup;
//! historical Ended rows stay in root storage. Successful restoration
//! reconstructs activations, claims, closing gates and projected retry timers;
//! failed restoration never becomes ready through more input.
use crate::domain::{Domain, Startup, Task, activate, publish, record, snapshot, task_mut};
use crate::{
    Active, Closing, Executor, Limits, Party, Phase, Problem, Refusal, Request, Stage, Stored, TaskRecord, Was,
};
use skein_lib::{Env, Queue};

pub(crate) fn restore(domain: &mut Domain, env: &Env<Limits>, stored: Stored, out: &mut Queue<Request>) {
    match domain.startup {
        Startup::Checked | Startup::Ready => return failed(domain, None, Refusal::NotReady, out),
        Startup::Failed => return,
        Startup::Restoring => {}
    }
    match stored {
        Stored::PlacedHold(hold) => {
            if domain.placed.contains_key(&hold.number) || domain.placed.len() >= env.limits.placed_holds {
                return failed(domain, None, Refusal::Restore, out);
            }
            domain.placed.insert(hold.number, *hold).expect("restored hold room");
        }
        Stored::PersonProposal(row) => restore_person_proposal(domain, env, *row, out),
        Stored::Live(task) => {
            let number = task.number;
            if domain.names.contains_key(&number) || domain.tasks.is_full() {
                return failed(domain, Some(number), Refusal::Restore, out);
            }
            let observed_hold = match task.phase {
                Phase::Held { why, .. } => Some(why),
                Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Ended(_) => None,
            };
            match task.executor {
                Executor::Agent { .. } => {
                    domain.live_agents = domain.live_agents.checked_add(1).expect("bounded restored agents");
                }
                Executor::Person(_) | Executor::Procedure { .. } => {}
            }
            crate::settings::hold(domain, &task);
            let id = domain
                .tasks
                .insert(Task {
                    published: Some(task.clone()),
                    procedure_offered: None,
                    observed_phase: Some(task.phase.clone()),
                    record: *task,
                    alarm: None,
                    grace_alarm: None,
                    observed_hold,
                })
                .expect("restored task admitted");
            let indexed = domain.names.insert(number, id);
            assert!(indexed == Ok(None), "restored name admitted");
            crate::waits::refresh(domain, number);
        }
        Stored::Stub(stub) => {
            if domain.stubs.contains_key(&stub.task) || domain.stubs.len() == domain.stubs.capacity() {
                return failed(domain, Some(stub.task), Refusal::Restore, out);
            }
            domain.stubs.insert(stub.task, stub).expect("bounded restored stub");
        }
        Stored::Writer(slot) => {
            if !crate::writers::restore(domain, &env.limits, slot) {
                failed(domain, None, Refusal::Restore, out);
            }
        }
        Stored::Pool(row) => {
            if !crate::holds::restore_pool(domain, &env.limits, row) {
                failed(domain, None, Refusal::Restore, out);
            }
        }
        // Historical ended rows stay outside the live arena;
        // they cannot accidentally return an ended task to the live arena.
        Stored::Update(row) => failed(domain, Some(row.task), Refusal::Restore, out),
        Stored::EndedByParty(row) => failed(domain, Some(row.task), Refusal::Restore, out),
        Stored::EndedByProject(row) => failed(domain, Some(row.task), Refusal::Restore, out),
        Stored::EndedDelegate(row) | Stored::Delegate(row) => failed(domain, Some(row.task), Refusal::Restore, out),
        Stored::Result(row) => failed(domain, Some(row.task), Refusal::Restore, out),
        Stored::DecidedProposal(_) | Stored::ClosedLedger(_) => failed(domain, None, Refusal::Restore, out),
        Stored::Ended(task) => failed(domain, Some(task.number), Refusal::Restore, out),
        Stored::Milestone(row) => failed(domain, Some(row.task), Refusal::Restore, out),
        Stored::History(row) => failed(domain, Some(row.task), Refusal::Restore, out),
        Stored::Activity(row) => failed(domain, Some(row.task), Refusal::Restore, out),
        Stored::Ledger(record) => {
            if !crate::funders::restore_funding(domain, record) {
                failed(domain, None, Refusal::Restore, out);
            }
        }
    }
}

fn restore_person_proposal(
    domain: &mut Domain,
    env: &Env<Limits>,
    row: crate::PersonProposal,
    out: &mut Queue<Request>,
) {
    if domain.person_proposals.len() >= env.limits.tasks || domain.person_proposals.contains_key(&row.number) {
        return failed(domain, None, Refusal::Restore, out);
    }
    let inserted = domain.person_proposals.insert(row.number, row);
    assert!(inserted == Ok(None), "restored person proposal admitted");
}

fn failed(domain: &mut Domain, task: Option<u64>, why: Refusal, out: &mut Queue<Request>) {
    domain.startup = Startup::Failed;
    out.push(Request::RestoreRefused { problem: Problem { task, why, blocked_by: None, capacity: None } });
}

// Only actual links are made here. Historical subjects (such as an ended
// dependency or an inbox sender) do not require a live target (engine.md, 6).
fn links(domain: &Domain, task: &TaskRecord) {
    if let Party::Task(parent) = task.requester {
        assert!(
            record(domain, parent).is_some(),
            "tasks: Live({}) links to parent {parent}, which is absent",
            task.number
        );
    }
    assert!(
        record(domain, task.root).is_some(),
        "tasks: Live({}) links to root {}, which is absent",
        task.number,
        task.root
    );
    for child in &task.delegates {
        assert!(
            record(domain, *child).is_some(),
            "tasks: Live({}) links to delegate {child}, which is absent",
            task.number
        );
    }
    for peer in &task.references {
        assert!(
            record(domain, *peer).is_some() || domain.stubs.contains_key(peer),
            "tasks: Live({}) links to reference {peer}, which is absent",
            task.number
        );
    }
    for dependency in &task.waiting_on {
        assert!(
            record(domain, *dependency).is_some(),
            "tasks: Live({}) links to waiting task {dependency}, which is absent",
            task.number
        );
    }
}

pub(crate) fn check(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    match domain.startup {
        Startup::Checked | Startup::Ready | Startup::Failed => return,
        Startup::Restoring => {}
    }
    for (number, _) in &domain.names {
        links(domain, record(domain, *number).expect("name indexes task"));
    }
    crate::placed::link(domain);
    crate::writers::link(domain);
    crate::funders::link(domain);
    assert!(crate::waits::restore(domain, env, out), "tasks: bounded restored waits can be rebuilt");
    domain.startup = Startup::Checked;
}

pub(crate) fn restored(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    match domain.startup {
        Startup::Restoring => return failed(domain, None, Refusal::NotReady, out),
        Startup::Ready | Startup::Failed => return,
        Startup::Checked => {}
    }
    domain.startup = Startup::Ready;
    crate::holds::restore(domain, env);
    crate::subscriptions::restore(domain, env);
    crate::wake::restore(domain, env);
    let numbers = snapshot(domain, env.limits.tasks);
    for number in numbers.into_boxed() {
        crate::grace::schedule(domain, env, number);
        let phase = record(domain, number).expect("restored name exists").phase.clone();
        match record(domain, number).expect("restored name exists").escalation {
            crate::Escalation::Waiting { .. } => out.push(Request::EscalationNeeded {
                context: crate::escalation::context(domain, number).expect("valid held person context"),
                restored: true,
            }),
            crate::Escalation::Unheld { .. }
            | crate::Escalation::Routing { .. }
            | crate::Escalation::Rejected { .. } => {}
        }
        match phase {
            Phase::Active(Active::Due | Active::Preparing) => {
                let row = &mut task_mut(domain, number).expect("restored name exists").record;
                let next = crate::run::ends::for_record(row).amended(row.executor, Phase::Active(Active::Due));
                let changed = row.phase != next;
                row.phase = next;
                let due = row.phase == Phase::Active(Active::Due);
                if changed {
                    publish(domain, env, number, out);
                }
                if due {
                    activate(domain, number, out);
                }
            }
            Phase::Active(Active::Claimed { attempt } | Active::Running { attempt }) => {
                out.push(Request::Adopt {
                    task: number,
                    attempt,
                    kept: record(domain, number).expect("restored live task").turn,
                });
            }
            Phase::Active(Active::Idle | Active::BackingOff { .. }) => publish(domain, env, number, out),
            Phase::Closing(closing) => match closing.stage {
                Stage::Run { attempt } => {
                    out.push(Request::Adopt {
                        task: number,
                        attempt,
                        kept: record(domain, number).expect("restored live task").turn,
                    });
                    out.push(Request::Stop { task: number, attempt });
                }
                Stage::Effects => out.push(Request::Close { task: number, ending: closing.ending }),
                Stage::Releases => out.push(Request::Release { task: number, ending: closing.ending }),
                Stage::Delegates | Stage::Settled => {}
            },
            Phase::Held { was, .. } => match was {
                Was::Active(Active::Claimed { attempt } | Active::Running { attempt })
                | Was::Closing(Closing { stage: Stage::Run { attempt }, .. }) => {
                    out.push(Request::Adopt {
                        task: number,
                        attempt,
                        kept: record(domain, number).expect("restored live task").turn,
                    });
                    out.push(Request::Stop { task: number, attempt });
                }
                Was::Closing(Closing { stage: Stage::Effects, ending }) => {
                    out.push(Request::Close { task: number, ending });
                }
                Was::Waiting
                | Was::Active(Active::Idle | Active::Due | Active::Preparing | Active::BackingOff { .. })
                | Was::Closing(Closing { stage: Stage::Delegates | Stage::Releases | Stage::Settled, .. }) => {}
            },
            Phase::Waiting => {}
            Phase::Ended(_) => unreachable!("ended record not restored into live arena"),
        }
    }
}

/// Bounded durable amendment payload shared by both proposal origins.
pub(crate) fn valid_amendment(limits: &Limits, amendment: &crate::Amendment) -> bool {
    (match amendment.addressed {
        Some(change) => crate::control::valid_address(change.address),
        None => true,
    }) && match &amendment.instructions {
        Some(instructions) => instructions.len() <= usize::try_from(limits.message_bytes).expect("u32 fits usize"),
        None => true,
    } && amendment.reason.len() <= usize::try_from(limits.message_bytes).expect("u32 fits usize")
        && match &amendment.spec {
            Some(spec) => crate::batch::valid_spec(limits, spec) && spec.inputs.is_empty(),
            None => true,
        }
        && match &amendment.authority {
            Some(authority) => crate::batch::valid_authority(limits, authority),
            None => true,
        }
        && match &amendment.dependencies {
            Some(dependencies) => dependencies.len() <= usize::try_from(limits.dependencies).expect("u32 fits usize"),
            None => true,
        }
        && match &amendment.wake {
            Some(wake) => crate::wake::valid(wake),
            None => true,
        }
}

impl Domain {
    /// Copy one live stored row without projecting lifecycle observations (tasks.md, 2).
    #[must_use]
    pub fn stored_record(&self, key: &crate::Key) -> Option<Stored> {
        Some(match *key {
            crate::Key::Live(number) => Stored::Live(alloc::boxed::Box::new(self.task(number)?.clone())),
            crate::Key::Stub(number) => Stored::Stub(*self.stubs.get(&number)?),
            crate::Key::Ledger(funder) => Stored::Ledger(*self.funding.get(&funder)?),
            crate::Key::PlacedHold(number) => {
                Stored::PlacedHold(alloc::boxed::Box::new(self.placed.get(&number)?.clone()))
            }
            crate::Key::PersonProposal(number) => {
                Stored::PersonProposal(alloc::boxed::Box::new(self.person_proposals.get(&number)?.clone()))
            }
            crate::Key::Writer(number) => {
                let mut found = None;
                for (_, row) in &self.writers {
                    if row.number == number {
                        found = Some(row.clone());
                    }
                }
                Stored::Writer(found?)
            }
            crate::Key::Pool(number) => {
                let mut found = None;
                for (_, row) in &self.pools {
                    if row.number == number {
                        found = Some(row.clone());
                    }
                }
                Stored::Pool(found?)
            }
            crate::Key::Ended(_)
            | crate::Key::ClosedLedger(_)
            | crate::Key::DecidedProposal(_)
            | crate::Key::History { .. }
            | crate::Key::Milestone { .. }
            | crate::Key::Activity { .. }
            | crate::Key::Update { .. }
            | crate::Key::Result { .. }
            | crate::Key::EndedDelegate { .. }
            | crate::Key::Delegate { .. }
            | crate::Key::EndedByParty { .. }
            | crate::Key::EndedByProject { .. } => return None,
        })
    }
}
