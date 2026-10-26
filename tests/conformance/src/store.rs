//! Restart flights routed through the real store (store.md 7.2 and 14).
use super::{Neighbours, Store, StoreOut, connector, core, root};
use jig_conformance::Input;
impl Neighbours {
    pub(super) fn restart(
        &mut self,
        store: &mut Store,
        step: core::RestartStep,
        census: Option<core::Census>,
    ) -> Vec<Input<root::Event, root::Record>> {
        self.restart_steps.push(step);
        match step {
            core::RestartStep::LoadCore | core::RestartStep::LoadInbox | core::RestartStep::RestoreConnector { .. } => {
                store.ask(StoreOut::Restart { step, census });
                Vec::new()
            }
            core::RestartStep::Switch => {
                if store.generation_pending {
                    store.ask(StoreOut::Restart { step, census: None });
                    Vec::new()
                } else {
                    vec![Input::Event(root::Event::Switched)]
                }
            }
            core::RestartStep::ReadAfresh { connector: number } => {
                self.restart_reads.insert(number, step);
                vec![Input::Event(root::Event::Connector {
                    number,
                    event: connector::Event::Restart(connector::RestartStep::FreshRead {
                        resource: jig_test_connector_world::path(1, 1),
                    }),
                })]
            }
            core::RestartStep::SettleOutbox { connector: number } => vec![Input::Event(root::Event::Connector {
                number,
                event: connector::Event::Restart(connector::RestartStep::SettleOutbox),
            })],
            core::RestartStep::AdoptRuns | core::RestartStep::Open => panic!("root performs synchronous restart step"),
        }
    }
}
