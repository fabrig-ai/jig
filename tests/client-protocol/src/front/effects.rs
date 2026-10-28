//! Fixture setup through the task-owner and worker doors.
use super::{Engine, Protocol, fixtures};
use jig_domain_core as core;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{ReplyTo, Token};
const DELEGATE_REPLY: u64 = 9801;
const RUN_ALLOWANCE: u64 = 5;
const DELEGATE_BUDGET: u64 = 10;
impl Protocol {
    #[must_use]
    pub fn two_run_slots(seed: u64) -> Self {
        let peers = jig_core_world::peers::Peers::new(
            vec![jig_fake_workers::Worker::new(
                1,
                2,
                vec![Box::new([jig_fake_workers::Script::Wait]), Box::new([jig_fake_workers::Script::Wait])],
            )],
            Vec::new(),
        );
        let mut config = jig_core_world::world::config(seed);
        set_run_allowance(&mut config);
        let mut limits = fixtures::owner_limits();
        limits.core.fleet.slots = 2;
        Self::with_engine(Engine::with_peers(seed, &limits, config, peers))
    }
    /// Admit one ordinary delegate of the first worker assignment.
    pub fn make_live_delegate(&mut self) {
        let owner = &self.engine.driver.world;
        let assignment = owner.peers.as_ref().expect("worker peer").workers[0].seen[0].clone();
        let key = root::Key::Core(core::Key::Tasks(tasks::Key::Live(assignment.task)));
        let Some(root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row)))) = owner.store.rows.get(&key) else {
            panic!("admitted task stored")
        };
        assert_eq!(row.run_reserved, RUN_ALLOWANCE);
        let mut authority = row.authority.clone();
        authority.budget.spend = DELEGATE_BUDGET;
        authority.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
        self.engine.driver.send(
            self.now,
            fixtures::wall(self.now),
            root::Event::Core(core::Event::DelegateValidated {
                to: ReplyTo::new(Token::new(DELEGATE_REPLY)),
                key: core::CallKey { task: assignment.task, attempt: assignment.attempt, completion: 1, position: 0 },
                batch: Box::new([core::Delegate {
                    deadline: jig_domain_tasks::Deadline::Named,
                    label: 0,
                    flags: tasks::Flags::PLAIN,
                    tracked: None,
                    budget: jig_domain_people::Budget::Named,
                    executor: tasks::Executor::Agent { charter: 1 },
                    spec: tasks::Spec {
                        asks: false,
                        words: b"live delegate".as_slice().into(),
                        parameters: Box::new([]),
                        inputs: Box::new([]),
                    },
                    contract: tasks::Contract::Report { words: 32 },
                    authority,
                    llm: None,
                    symbolic_grants: Box::new([]),
                    dependencies: Box::new([]),
                    wake: tasks::WakePolicy::DEFAULT,
                }]),
                stubs: Box::new([]),
            }),
        );
        let Some((_, _, core::CallPart::Delegated(numbers))) = self.engine.driver.world.answers.last() else {
            panic!("delegate admitted: {:?}", self.engine.driver.world.answers.last())
        };
        assert_eq!(numbers.len(), 1);
        self.advance(self.now, fixtures::wall(self.now));
    }
}

fn set_run_allowance(config: &mut root::Config) {
    use jig_domain_authority as authority;
    let current = &config.core.defaults.rules;
    let mut rules = current.clone();
    rules.maximum_run_spend = RUN_ALLOWANCE;
    let policy = config.core.seed_projects[0].policy.policy.clone();
    let mut domain =
        authority::Domain::new(rules, fixtures::owner_limits().core.authority).expect("small run allowance");
    let mut out = skein_lib::Queue::with_capacity(authority::POLICY_MAX_OUT);
    authority::step(&mut domain, authority::Event::Policy { project: 1, policy }, &mut out);
    assert_eq!(out.pop(), Some(authority::PolicyFact::Added { project: 1 }));
    config.core.seed_projects[0].policy.policy = domain.policy(1).expect("checked seed policy").clone();
    config.core.defaults.rules = (jig_domain_authority::Domain::new(domain.rules().clone(), *domain.limits())
        .expect("checked rules"))
    .rules()
    .clone();
}

#[cfg(test)]
mod tests {
    use super::{Protocol, fixtures};
    use skein_lib::Time;
    #[test]
    fn starting_a_chat_assigns_the_chat_templates_label() {
        let templates = jig_core_world::fixtures::templates();
        let label = templates
            .iter()
            .find(|template| template.number == jig_client::shapes::CHAT)
            .expect("fixture template")
            .label;
        let mut protocol = Protocol::new(fixtures::SEED);
        protocol.engine.driver.scripted_party(fixtures::starter(), Time::ZERO, fixtures::wall(Time::ZERO));
        protocol.advance(Time::ZERO, fixtures::wall(Time::ZERO));
        let labels: Vec<_> = protocol
            .engine
            .driver
            .world
            .store
            .rows
            .values()
            .filter_map(|record| match record {
                jig_test_domain::Record::Core(jig_domain_core::Record::Tasks(jig_domain_tasks::Stored::Live(row))) => {
                    Some((row.label, row.flags.conversation))
                }
                jig_test_domain::Record::Core(_) | jig_test_domain::Record::Connector { .. } => None,
            })
            .collect();
        assert_eq!(labels, vec![(label, true)]);
    }
    #[test]
    fn setting_a_goal_assigns_the_goal_templates_label() {
        use jig_fake_parties::{Act, Ask, Party, Role};
        let templates = jig_core_world::fixtures::templates();
        let label = templates
            .iter()
            .find(|template| template.number == jig_client::shapes::GOAL)
            .expect("fixture template")
            .label;
        let mut protocol = Protocol::new(fixtures::SEED);
        let party = Party::new(
            1,
            Role::Owner,
            1_000_000,
            Box::new([
                Act::SignIn { provider: 0, subject: fixtures::OWNER_SUBJECT.as_slice().into() },
                Act::Request {
                    key: fixtures::START_KEY,
                    ask: Ask::Goal {
                        words: b"tracked goal".as_slice().into(),
                        budget: 10,
                        priority: 1,
                        deadline: None,
                    },
                },
            ]),
        );
        protocol.engine.driver.scripted_party(party, Time::ZERO, fixtures::wall(Time::ZERO));
        protocol.advance(Time::ZERO, fixtures::wall(Time::ZERO));
        let labels: Vec<_> = protocol
            .engine
            .driver
            .world
            .store
            .rows
            .values()
            .filter_map(|record| match record {
                jig_test_domain::Record::Core(jig_domain_core::Record::Tasks(jig_domain_tasks::Stored::Live(row))) => {
                    Some((row.label, row.flags.listed))
                }
                jig_test_domain::Record::Core(_) | jig_test_domain::Record::Connector { .. } => None,
            })
            .collect();
        assert_eq!(labels, vec![(label, true)]);
    }
    #[test]
    fn the_two_slot_fixture_admits_a_running_delegate() {
        let mut protocol = Protocol::two_run_slots(fixtures::SEED);
        protocol.engine.driver.scripted_party(fixtures::starter(), Time::ZERO, fixtures::wall(Time::ZERO));
        protocol.advance(Time::ZERO, fixtures::wall(Time::ZERO));
        protocol.make_live_delegate();
        assert_eq!(protocol.runs_started(), 2);
    }
}
