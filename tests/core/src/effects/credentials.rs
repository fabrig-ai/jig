//! The credential story's resource-bearing agent delegate.
use super::{World, connector, core, root, tasks};
use skein_lib::{ReplyTo, Token};

impl World {
    pub fn delegate_on_resource(&mut self, resource: u64) {
        self.delegate_on_path(
            resource,
            jig_test_connector_world::path(1, u8::try_from(resource).expect("fixture resource")),
        );
    }
    pub fn delegate_on_path(&mut self, resource: u64, path: connector::Path) {
        self.configure_holds(
            1,
            Box::new([tasks::Kind {
                connector: 1,
                kind: 1,
                hold: tasks::HoldKind::Exclusive { taken: tasks::Taken::Waits },
            }]),
        );
        self.send(root::Event::Connector {
            number: 1,
            event: connector::Event::Adopt { project: 1, resource: path, role: connector::ResourceRole::Owned },
        });
        let mut authority = self.assigned.as_ref().expect("assigned caller").run.authority.clone();
        authority.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
        authority.budget.spend = 20;
        self.send(root::Event::Core(core::Event::DelegateValidated {
            to: ReplyTo::new(Token::new(201)),
            key: self.call_key(1),
            batch: Box::new([core::Delegate {
                deadline: jig_domain_tasks::Deadline::Named,
                label: 0,
                flags: jig_domain_tasks::Flags::PLAIN,
                tracked: None,
                budget: jig_domain_people::Budget::Named,
                executor: tasks::Executor::Agent { charter: 1 },
                spec: tasks::Spec {
                    asks: false,
                    words: b"work on the resource".as_slice().into(),
                    parameters: Box::new([tasks::Parameter::Resource { name: 1, connector: 1, resource }]),
                    inputs: Box::new([]),
                },
                contract: tasks::Contract::Report { words: 128 },
                authority,
                llm: None,
                symbolic_grants: Box::new([]),
                dependencies: Box::new([]),
                wake: tasks::WakePolicy::DEFAULT,
            }]),
            stubs: Box::new([]),
        }));
    }
}
