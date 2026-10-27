use super::*;
const PROJECT: u32 = 1;
const FIRST_MESSAGE: u64 = 19;
fn bounded_release() -> (Script, Ask, Answer) {
    let mut setup = live_script();
    let mut row = task();
    row.hold = Some(TaskHold::Document(Box::new(Hold {
        placed: None,
        revision: Revision { value: 7 },
        reason: HoldReason::Stopped { by: party(1), at: Wall::EPOCH },
        before: Phase::Running,
        since: Wall::EPOCH,
        result: None,
        spend: List::with_capacity(0),
    })));
    row.acts = list(vec![Offered {
        effect: None,
        act: ActKind::Amend,
        admission: Admission::Bounded {
            within: jig_client_domain::contract::Bounds::Amend {
                budget_left: 10,
                implications: List::with_capacity(0),
                grants: List::with_capacity(0),
                authority: Box::new(owner_authority()),
                ceiling: Box::new(owner_authority()),
            },
            beyond: Beyond::Refused(Refusal::NotOffered),
        },
    }]);
    let ask = releasing(
        Ask::Amend {
            project: PROJECT,
            task: row.chip.task,
            revision: Some(row.spec_revision),
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
            reason: text(b""),
        },
        row.hold.as_ref().unwrap().revision(),
    );
    setup.truth.objects.insert(ObjectKey::Task(row.chip.task), ReadValue::Task(Box::new(row)));
    let answer = Answer::Done(Outcome::Owner(OwnerOutcome::Compound {
        project: PROJECT,
        operations: 2,
        first_message: FIRST_MESSAGE,
    }));
    setup.rules.push(Rule { party: Some(1), ask: ask.clone(), answer: answer.clone(), changes: vec![] });
    (setup, ask, answer)
}
#[test]
fn a_compound_amend_and_release_obeys_the_same_bounded_offer() {
    let (setup, ask, answer) = bounded_release();
    let mut engine = Engine::new(setup, 1).unwrap();
    send(&mut engine, 0, 11, 1, 1, ask);
    assert_eq!(final_answer(&engine.poll(at(10_000_000))[0]), &answer);
}

pub(super) fn releasing(ask: Ask, hold: Revision) -> Ask {
    use jig_client_domain::contract::Operation;
    let Ask::Amend { project, task, revision, change, reason } = ask else {
        panic!("amendment");
    };
    Ask::Compound {
        project,
        operations: list(vec![
            Operation::Amend { project, task, revision, change, reason },
            Operation::Release { project, task, revision: hold, again: None },
        ]),
    }
}
