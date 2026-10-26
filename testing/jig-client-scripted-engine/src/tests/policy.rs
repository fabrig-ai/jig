use super::*;
fn policy_project() -> Project {
    Project {
        number: 1,
        name: text(b"supplied fixture"),
        period: 0,
        policy: Policy {
            ceiling: authority(),
            period: 0,
            spend: 0,
            roles: list(vec![]),
            requirements: list(vec![]),
            deciders: list(vec![]),
        },
        roles: list(vec![]),
        spend: budget(),
        pools: list(vec![]),
        resources: list(vec![]),
        services: list(vec![]),
        recurring: list(vec![]),
        parts: list(vec![]),
        spend_lines: list(vec![]),
        acts: list(vec![Offered { effect: None, act: ActKind::ChangePolicy, admission: Admission::Allowed }]),
    }
}
#[test]
fn each_closed_policy_edit_is_one_durable_keyed_request_and_replays_without_splitting() {
    let edits = [
        PolicyEdit::Ceiling {
            ceiling: OwnerAuthority {
                models: None,
                tools: u64::MAX,
                grants: list(vec![]),
                delegation: OwnerDelegation { kinds: list(vec![]), tasks: u32::MAX, depth: u32::MAX },
                spend: u64::MAX,
                deadline: None,
                notes: u8::MAX,
                note_resources: list(vec![]),
            },
        },
        PolicyEdit::ProjectSpend { period_spend: u64::MAX },
        PolicyEdit::Role(PolicyEditRole {
            number: u32::MAX,
            authority: OwnerAuthority {
                models: None,
                tools: u64::MAX,
                grants: list(vec![]),
                delegation: OwnerDelegation { kinds: list(vec![]), tasks: u32::MAX, depth: u32::MAX },
                spend: u64::MAX,
                deadline: None,
                notes: u8::MAX,
                note_resources: list(vec![]),
            },
            period_spend: u64::MAX,
            requests: u16::MAX,
            decides: u8::MAX,
        }),
        PolicyEdit::Requirements { requirements: list(vec![]) },
        PolicyEdit::Projections { grants: list(vec![]) },
        PolicyEdit::Permissions {
            mappings: list(vec![PolicyPermission { connector: u16::MAX, permission: u16::MAX, role: u32::MAX }]),
        },
    ];
    for (position, edit) in edits.into_iter().enumerate() {
        let ask = Ask::ChangePolicy { project: 1, edit };
        let mut setup = script();
        setup.truth.projects.insert(1, policy_project());
        setup.rules.push(Rule {
            party: Some(1),
            ask: ask.clone(),
            answer: Answer::Done(Outcome::Changed),
            changes: vec![],
        });
        let mut engine = Engine::new(setup, position as u64 + 300).unwrap();
        send(&mut engine, 0, 11, 1, 1, ask.clone());
        send(&mut engine, 1, 12, 1, 2, ask.clone());
        assert!(engine.commits().is_empty());
        let answers = engine.poll(Time::from_nanos(10_000_001));
        assert_eq!(answers.len(), 2);
        assert_eq!(final_answer(&answers[0]), &Answer::Done(Outcome::Changed));
        assert_eq!(final_answer(&answers[1]), final_answer(&answers[0]));
        assert_eq!(engine.commits().len(), 1);
        assert_eq!(engine.commits()[0].ask, Some(ask.clone()));
        send(
            &mut engine,
            20_000_000,
            11,
            1,
            3,
            Ask::ChangePolicy { project: 1, edit: PolicyEdit::ProjectSpend { period_spend: 0 } },
        );
        assert_eq!(
            final_answer(&engine.poll(Time::from_nanos(20_000_000))[0]),
            &Answer::Refused { refusal: Refusal::KeyConflict, at: None }
        );
        send(&mut engine, 20_000_000, 11, 1, 4, ask);
        assert_eq!(final_answer(&engine.poll(Time::from_nanos(20_000_000))[0]), &Answer::Done(Outcome::Changed));
        assert_eq!(engine.commits().len(), 1);
    }
}
