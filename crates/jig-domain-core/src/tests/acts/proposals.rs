//! New proposals need an accepting role; parties' words fit a run message.
use super::*;

fn procedure_fixture() -> (Core, Limits, connector::EffectDescription) {
    let (mut core, limits) = tree_core();
    let grant = authority::Grant {
        connector: 1,
        kind: 5,
        pattern: authority::Pattern { segments: Box::new([]), last: authority::Last::Open(Box::new([])) },
    };
    let mut rules = core.authority.rules().clone();
    let mut value = core.authority.policy(1).unwrap().clone();
    rules.ceiling.grants = Box::new([grant.clone()]);
    value.ceiling.grants = Box::new([grant]);
    value.roles[0].decides = authority::Proposals(16);
    core.authority = authority::Domain::new(rules, limits.authority).unwrap();
    let mut out = Queue::with_capacity(authority::POLICY_MAX_OUT);
    authority::step(&mut core.authority, authority::Event::Policy { project: 1, policy: value }, &mut out);
    assert_eq!(out.pop(), Some(authority::PolicyFact::Added { project: 1 }));
    let mut row = new_row(&core, 1, tasks::Funder::Period { project: 1, period: 1 }, None);
    row.executor = tasks::Executor::Procedure { connector: 1, code: 1 };
    send_task(
        &mut core,
        &limits,
        tasks::Event::Make {
            reply_to: ReplyTo::new(Token::new(120)),
            creator: tasks::Party::Deployment { project: 1 },
            batch: Box::new([row]),
        },
    );
    let description = connector::EffectDescription {
        waits_for_dependencies: false,
        connector: 1,
        purpose: 19,
        form: connector::EffectForm::Set,
        recovery: connector::Recovery::Keyed,
        effect: authority::Effect {
            connector: 1,
            kind: 5,
            name: authority::Name { segments: Box::new([b"target".as_slice().into()]) },
            state: [13; 32],
            price: None,
            access: authority::EffectAccess::Owned,
            additional: Box::new([]),
            guards: Box::new([]),
        },
    };
    let needed = authority::needs(&authority::Action::Effect(description.effect.clone())).unwrap();
    let action = tasks::ProposalAction::ProcedureEffect {
        connector: 1,
        authority: task_authority(&needed),
        step: 1,
        purpose: 19,
    };
    drop(core.proposal_admit_action(1, &action, Wall::EPOCH).expect("the policy ceiling admits this proposal"));
    assert_eq!(
        core.proposal_holder(&limits, 1, &action, None, Wall::EPOCH),
        Some(tasks::ProposalHolder::Policy { project: 1, kind: tasks::ProposalKind::Effect })
    );
    (core, limits, description)
}

#[test]
fn a_procedure_step_proposing_what_nobody_may_accept_is_refused() {
    let (mut core, limits, description) = procedure_fixture();
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let mut work = Queue::with_capacity(4);
    let mut out = Queue::with_capacity(4);
    effects::procedure_propose(&mut core, &env, &mut work, &mut out, Token::new(121), 1, 1, 1, &description);
    let Some(Request::Ask {
        connector: 1,
        ask: Ask::Effect(connector::Ask::Drop { owner, answer: authority::Answer::Refuse, .. }),
    }) = out.pop()
    else {
        unreachable!("the proposal is refused to its connector")
    };
    assert_eq!(owner, Token::new(121));
    let Some(Event::Tasks(tasks::Event::Procedure {
        task: 1,
        step: 1,
        decision: tasks::ProcedureDecision { end: tasks::ProcedureEnd::Wait, .. },
        ..
    })) = work.pop()
    else {
        unreachable!("the refused procedure step waits");
    };
    assert!(core.procedure_proposals.is_empty());
}

#[test]
#[should_panic(expected = "party words fit one run message")]
fn core_new_refuses_party_words_beyond_a_runs_message() {
    let mut limits = limits();
    limits.people.words = limits.tasks.message_bytes + 1;
    drop(Core::new(config(41), &limits).expect("fixture core configuration"));
}
