//! Operation fixtures shared by the scripted engine tests.
use super::*;
pub(super) fn proposed_amendment(bounded: bool, changed: bool) -> (Script, Ask, Answer) {
    let mut setup = live_script();
    let mut row = task();
    row.acts = list(vec![Offered {
        effect: None,
        act: ActKind::Amend,
        admission: if bounded {
            Admission::Bounded {
                within: jig_client_domain::contract::Bounds::Amend {
                    budget_left: 10,
                    grants: List::with_capacity(0),
                    implications: List::with_capacity(0),
                    authority: Box::new(row.authority.clone()),
                    ceiling: Box::new(row.authority.clone()),
                },
                beyond: Beyond::Proposed { to: Recipient::Party(9) },
            }
        } else {
            Admission::Proposed { to: Recipient::Party(9) }
        },
    }]);
    let ask = Ask::Propose {
        project: 1,
        action: ProposeAction::Amendment {
            task: row.chip.task,
            revision: row.spec_revision,
            change: Box::new(Amendment {
                llm: None,
                addressed: None,
                spec: None,
                wake: None,
                dependencies: None,
                authority: None,
                instructions: Some(text(b"new")),
                tracked: None,
            }),
        },
        reason: text(b"more work"),
    };
    let answer = Answer::Done(Outcome::Owner(if changed {
        OwnerOutcome::Amended { task: row.chip.task }
    } else {
        OwnerOutcome::Proposed { proposal: 1, to: 0 }
    }));
    setup.truth.objects.insert(ObjectKey::Task(row.chip.task), ReadValue::Task(Box::new(row)));
    setup.rules.push(Rule { party: Some(1), ask: ask.clone(), answer: answer.clone(), changes: vec![] });
    (setup, ask, answer)
}

pub(super) fn query(project: u32) -> TaskQuery {
    TaskQuery {
        project: Some(project),
        requester: None,
        root: None,
        parent: None,
        label: None,
        phase: None,
        executor: None,
        tracked: None,
        order: QueryOrder::Changed,
    }
}

/// One root using the shared task's specification and authority.
pub(super) fn new_task(words: Text) -> NewTask {
    NewTask {
        llm: None,
        deadline: jig_client_domain::contract::NewDeadline::Named,
        executor: OwnerExecutor::Agent { charter: 1 },
        spec: Specification { words, parameters: list(vec![]), inputs: list(vec![]) },
        contract: NewContract::Report { words: 128 },
        authority: owner_authority().authority,
        budget: NewBudget::Named,
        wake: None,
        dependencies: List::with_capacity(0),
        named: List::with_capacity(0),
        tracked: None,
        label: 0,
        flags: TaskFlags { ended_by: EndedBy::Result, conversation: true, listed: false, retention: Retention::Kept },
    }
}
pub(super) fn create(project: u32, words: Text) -> Ask {
    Ask::Create { project, tasks: list(vec![new_task(words)]) }
}
pub(super) fn creating(frame: &mut Me) {
    frame.projects.get_mut(0).unwrap().creating = Some(Creating {
        within: CreateBounds {
            authority: Box::new(owner_authority()),
            pool_left: 100,
            charters: list(vec![1]),
            procedures: List::with_capacity(0),
            latest: None,
            budget: 100,
            maximum_budget: 1000,
            maximum_deadline: None,
        },
        beyond: Beyond::Proposed { to: Recipient::Party(2) },
    });
}

pub(super) fn span_proposal() -> (Script, Ask) {
    let mut setup = script();
    let mut value = proposal();
    let member = new_task(text(b"span work"));
    let task = ProposedTask {
        number: 9,
        spec: SpecificationDocument::complete(member.spec),
        executor: member.executor,
        contract: TaskContractDocument::Report { words: 128 },
        authority: OwnerAuthorityDocument { authority: member.authority, cut: false },
        deadline: NewDeadline::Span(Duration::from_secs(5)),
        budget: 20,
        funder: Funder::Pool { project: 1, person: 1, period: 1 },
        dependencies: List::with_capacity(0),
        resources: List::with_capacity(0),
        wake: OwnerWakePolicy {
            markers: 0,
            words: WakeRule::Immediate,
            notices: WakeRule::Never,
            news: WakeRule::Never,
            results: ResultsWake::Each,
            questions: false,
            answers: false,
            timers: false,
        },
        priority: None,
        cut: false,
    };
    value.action = ProposalAction::Batch { tasks: list(vec![task]), own: false };
    let ask = Ask::Decide {
        project: 1,
        on: DecideOn::Proposal { proposal: value.key, decision: Decision::Accepted, running: None },
    };
    setup.truth.objects.insert(ObjectKey::Proposal(value.key), ReadValue::Proposal(Box::new(value)));
    setup.truth.frames.get_mut(&1).unwrap().projects.get_mut(0).unwrap().creating.as_mut().unwrap().within.latest =
        Some(Wall::from_nanos(20_000_000_000));
    setup.rules.push(Rule {
        party: Some(1),
        ask: ask.clone(),
        answer: Answer::Done(Outcome::Changed),
        changes: vec![],
    });
    (setup, ask)
}
