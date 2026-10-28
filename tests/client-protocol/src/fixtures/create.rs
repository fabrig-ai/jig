use jig_client::contract as c;
use skein_lib::List;
/// Small party roots with one engine-resolved budget.
#[must_use]
pub fn create_tasks() -> List<c::NewTask> {
    let mut tasks = List::with_capacity(2);
    for allowed in [true, false] {
        let mut dependencies = List::with_capacity(1);
        if !allowed {
            dependencies.push(c::TaskRef::Member(0)).expect("one dependency");
        }
        tasks
            .push(c::NewTask {
                llm: None,
                deadline: jig_client_domain::contract::NewDeadline::Named,
                executor: c::OwnerExecutor::Agent { charter: 1 },
                spec: c::Specification {
                    words: c::Text::new(b"Work".as_slice().into(), 32).expect("bounded spec"),
                    parameters: List::with_capacity(0),
                    inputs: List::with_capacity(0),
                },
                contract: c::NewContract::Report { words: 8 },
                authority: c::OwnerAuthority {
                    models: None,
                    tools: 0,
                    grants: List::with_capacity(0),
                    delegation: c::OwnerDelegation { kinds: List::with_capacity(0), tasks: 0, depth: 0 },
                    spend: if allowed { 0 } else { 10 },
                    deadline: None,
                    notes: 0,
                    note_resources: List::with_capacity(0),
                },
                budget: if allowed { c::NewBudget::AsAllowed { cap: Some(20) } } else { c::NewBudget::Named },
                wake: Some(c::OwnerWakePolicy {
                    words: c::WakeRule::Immediate,
                    notices: c::WakeRule::Never,
                    news: c::WakeRule::Never,
                    results: c::ResultsWake::Never,
                    questions: false,
                    answers: false,
                    timers: false,
                }),
                dependencies,
                named: List::with_capacity(0),
                tracked: None,
                label: 1,
                flags: c::TaskFlags {
                    ended_by: c::EndedBy::Result,
                    conversation: false,
                    listed: true,
                    retention: c::Retention::Kept,
                },
            })
            .expect("two roots");
    }
    tasks
}

#[must_use]
pub fn batch_config(seed: u64) -> jig_test_domain::Config {
    use jig_domain_authority as a;
    let mut config = jig_core_world::world::config(seed);
    let current = &config.core.defaults.rules;
    let policy = config.core.seed_projects[0].policy.policy.clone();
    let mut bounds = jig_core_world::world::limits().core.authority;
    bounds.batch = 2;
    let mut authority = a::Domain::new(current.clone(), bounds).expect("two-member authority");
    let mut out = skein_lib::Queue::uncapped();
    a::step(&mut authority, a::Event::Policy { project: 1, policy }, &mut out);
    assert_eq!(out.pop(), Some(a::PolicyFact::Added { project: 1 }));
    config.core.seed_projects[0].policy.policy = authority.policy(1).expect("checked seed policy").clone();
    config.core.defaults.rules =
        (a::Domain::new(authority.rules().clone(), *authority.limits()).expect("checked rules")).rules().clone();
    config
}
#[must_use]
pub fn proposed_tasks() -> List<c::NewTask> {
    let mut tasks = create_tasks();
    tasks.get_mut(0).expect("first root").budget = c::NewBudget::Named;
    tasks.get_mut(0).expect("first root").authority.spend = 150;
    tasks
}

#[cfg(test)]
mod tests {
    use jig_domain_authority as a;
    #[test]
    fn the_create_story_authority_admits_both_members() {
        let config = super::batch_config(super::super::SEED);
        let mut bounds = jig_core_world::world::limits().core.authority;
        bounds.batch = 2;
        let mut authority =
            a::Domain::new(config.core.defaults.rules.clone(), bounds).expect("fixture authority bounds");
        let given = |spend| a::Delegate {
            executor: a::Executor::Charter(1),
            symbolic: Box::new([]),
            authority: a::Authority {
                tools: a::Tools(0),
                models: a::Models::NONE,
                grants: Box::new([]),
                delegation: a::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 },
                budget: a::Budget { spend, deadline: None },
                notes: a::Scopes(0),
                note_resources: Box::new([]),
            },
        };
        let mut admitted = skein_lib::Queue::uncapped();
        a::step(
            &mut authority,
            a::Event::Policy { project: 1, policy: config.core.seed_projects[0].policy.policy.clone() },
            &mut admitted,
        );
        let ask = a::PersonAsk {
            project: 1,
            role: 0,
            pool: a::Numbers { budget: 500, spent: 0, spent_below: 0, reserved: 0 },
            tasks_left: 2,
            request: a::PersonRequest::Create(Box::new([given(20), given(10)])),
        };
        let mut out = skein_lib::Queue::uncapped();
        assert_eq!(a::check_request(&authority, &ask, &mut out).answer, a::Answer::Allow);
    }
}
