//! Independent configured policies selected by incoming makes, applied only
//! when their durable answers name the made projects (people.md, 5.2–5.3).
use crate::Store;
use jig_domain_authority as authority;
use jig_domain_core as core;
use jig_domain_people as people;
use jig_test_domain as root;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub(super) struct Makes {
    configured: BTreeMap<u32, authority::Policy>,
    requests: BTreeMap<people::RequestKey, authority::Policy>,
    acts: BTreeMap<people::ActKey, authority::Policy>,
}
impl Makes {
    pub(super) fn new(config: &root::Config) -> Self {
        let mut makes = Self { configured: BTreeMap::new(), requests: BTreeMap::new(), acts: BTreeMap::new() };
        makes.configure(config);
        makes
    }
    pub(super) fn configure(&mut self, config: &root::Config) {
        self.configured =
            config.core.defaults.policies.iter().map(|(number, seed)| (*number, seed.policy.clone())).collect();
    }
    pub(super) fn inbound(&mut self, store: &Store, event: &root::Event) {
        match event {
            root::Event::Core(core::Event::People(people::Event::Ask {
                sign_in,
                key,
                ask: people::Ask::MakeProject { policy, .. },
                ..
            })) => {
                let record = store.rows.get(&root::Key::Core(core::Key::People(people::Key::SignIn(*sign_in))));
                if let Some(root::Record::Core(core::Record::People(people::Stored::SignIn { person, .. }))) = record
                    && let Some(policy) = self.configured.get(policy)
                {
                    self.requests.insert(people::RequestKey { person: *person, key: *key }, policy.clone());
                }
            }
            root::Event::Core(core::Event::Act { connector, key, act, .. }) => {
                if let people::Act::MakeProject { policy, .. } = &**act
                    && let Some(policy) = self.configured.get(policy)
                {
                    self.acts.insert(people::ActKey { connector: *connector, key: *key }, policy.clone());
                }
            }
            root::Event::Core(_)
            | root::Event::Stop
            | root::Event::Switched
            | root::Event::RestartBegin { .. }
            | root::Event::RestartDone(_)
            | root::Event::Timer(_)
            | root::Event::Link { .. }
            | root::Event::ResultRead { .. }
            | root::Event::WatchApplication { .. }
            | root::Event::ReadApplication { .. }
            | root::Event::Credential { .. }
            | root::Event::Connector { .. }
            | root::Event::Application { .. }
            | root::Event::ProjectionEffect { .. }
            | root::Event::CallAnswered { .. }
            | root::Event::TopicCall { .. }
            | root::Event::EffectCall { .. }
            | root::Event::TranscriptLoaded { .. }
            | root::Event::Committed { .. }
            | root::Event::WriterRead { .. }
            | root::Event::ConnectorTimer { .. }
            | root::Event::Text { .. }
            | root::Event::Failed { .. }
            | root::Event::Turn { .. }
            | root::Event::Answer { .. } => {}
        }
    }
    pub(super) fn durable(&mut self, store: &Store) -> Vec<(u32, authority::Policy)> {
        let mut projects = Vec::new();
        for row in store.rows.values() {
            let (policy, outcome) = match row {
                root::Record::Core(core::Record::People(people::Stored::Answer { key, outcome, .. })) => {
                    (self.requests.remove(key), outcome)
                }
                root::Record::Core(core::Record::People(people::Stored::Acted { key, outcome, .. })) => {
                    (self.acts.remove(key), outcome)
                }
                root::Record::Core(_) | root::Record::Connector { .. } => continue,
            };
            if let Some(policy) = policy
                && let people::Outcome::ProjectMade { project } = outcome
            {
                projects.push((*project, policy));
            }
        }
        projects
    }
}
