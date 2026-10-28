//! Read back party documents after the cold start has made every link.
use super::{World, core, fleet, root};
use core::documents::{self, Missing, Subject, Window};

mod durable;
use durable::{DurableView, Transients};
type Answer = (u64, Subject, Result<DurableView, Missing>);

impl World {
    /// End a story by restoring its actual store and reading the same public documents.
    #[expect(clippy::too_many_lines, reason = "one cold restart retains surviving peer facts and compares both sides")]
    pub fn restart_oracle(&mut self, configuration: root::Config) {
        self.oracle_skip = Some("the oracle already owns the story's teardown");
        self.cut = None;
        self.peers = None;
        self.hold_restore_pages = false;
        self.store.pause_pages(false);
        self.hold_restart_reads = false;
        self.hold_switch = false;
        self.hold_sweep = false;
        self.release_commits();
        let transients = Transients::from_core(self.domain.core(), configuration.core.workers_in_process);
        let stored = self.store.readback_rows();
        self.oracle_child_views(&stored);
        let before = self.oracle_documents(&transients);
        let hosts = self.domain.core().fleet.host_views();
        let hellos: Vec<_> = self
            .oracle_hellos
            .iter()
            .filter(|(_, channel, _)| {
                hosts
                    .as_slice()
                    .iter()
                    .any(|host| host.channel == *channel && host.connected && host.kind == fleet::HostKind::Worker)
            })
            .map(|(worker, channel, hello)| {
                let mut hello = copy_hello(hello);
                hello.hosting = self
                    .domain
                    .core()
                    .proofs
                    .iter()
                    .filter_map(|(task, proof)| {
                        let row = self.domain.core().tasks.task(*task)?;
                        if proof.placement != Some(*channel) {
                            return None;
                        }
                        let live = matches!(
                            &row.phase,
                            jig_domain_tasks::Phase::Active(
                                jig_domain_tasks::Active::Claimed { .. } | jig_domain_tasks::Active::Running { .. },
                            ) | jig_domain_tasks::Phase::Closing(jig_domain_tasks::Closing {
                                stage: jig_domain_tasks::Stage::Run { .. },
                                ..
                            }) | jig_domain_tasks::Phase::Held {
                                was: jig_domain_tasks::Was::Active(
                                    jig_domain_tasks::Active::Claimed { .. } | jig_domain_tasks::Active::Running { .. },
                                ),
                                ..
                            }
                        );
                        live.then_some(fleet::Hosted {
                            run: skein_lib::Token::new(*task),
                            attempt: skein_lib::Token::new(proof.attempt),
                            phase: fleet::Phase::Active,
                        })
                    })
                    .collect();
                (*worker, *channel, hello)
            })
            .collect();
        // Peers survive the engine, but do not take new scripted actions in teardown.
        let _peers = self.peers.take();

        let grants: Vec<_> = self
            .domain
            .accounts()
            .source_views()
            .as_slice()
            .iter()
            .filter_map(|source| self.domain.accounts().source_grant(source.source, self.env().now))
            .filter(|grant| matches!(grant.form, jig_domain_accounts::Form::Credential(_)))
            .collect();
        // accounts.md 3.5: the component reports its kept record's failure
        // again, just as it reports a surviving grant. No new fake refresh is
        // performed merely to read the story's durable facts back.
        let failures: Vec<_> = self
            .domain
            .accounts()
            .source_views()
            .as_slice()
            .iter()
            .filter_map(|source| {
                use jig_domain_accounts::{Failure, FromComponent, State};
                let failure = match source.state {
                    State::NeedsSignIn => Failure::SignInAgain,
                    State::Misconfigured => Failure::Misconfigured,
                    State::Unloaded { at, why } => Failure::Unloaded { at, why },
                    State::Asking
                    | State::Granted
                    | State::Later { .. }
                    | State::Limited { .. }
                    | State::Refused
                    | State::Cancelled
                    | State::Closing
                    | State::Moving => return None,
                };
                Some(FromComponent::Failed { account: source.source, failure })
            })
            .collect();
        self.hold_accounts = true;
        self.supplied_wall = Some(self.env().wall);
        self.hold_switch = true;
        self.hold_sweep = true;
        self.restart_with(configuration);
        assert_eq!(self.domain.restart_step(), Some(core::RestartStep::Sweep), "the cold restart makes every link");
        assert_eq!(self.store.readback_rows(), stored, "loading preserves complete stored child views");
        self.oracle_child_views(&stored);
        self.finish_sweep();
        self.finish_switch();
        assert!(self.domain.ready(), "cold restart finishes before readback");
        // The account component is outside the engine's process. Its surviving
        // grants are heard again after the new start's durable mark.
        for grant in grants {
            self.send(root::Event::Core(core::Event::Account(jig_domain_accounts::Event::Component(
                jig_domain_accounts::FromComponent::Granted {
                    form: grant.form,
                    account: grant.account,
                    generation: grant.generation,
                    valid: grant.valid,
                },
            ))));
        }
        for failure in failures {
            self.send(root::Event::Core(core::Event::Account(jig_domain_accounts::Event::Component(failure))));
        }
        for (worker, channel, hello) in hellos {
            self.send(root::Event::Core(core::Event::Fleet(fleet::Event::Hello { worker, channel, hello })));
        }
        let after = self.oracle_documents(&transients);
        assert_eq!(after.len(), before.len(), "cold restart preserves every document subject");
        for (actual, expected) in after.iter().zip(&before) {
            assert_eq!(actual, expected, "cold restart preserves {:?} for party {}", expected.1, expected.0);
        }
    }

    fn oracle_child_views(&self, stored: &std::collections::BTreeMap<root::Key, root::Record>) {
        use jig_domain_people::Stored as People;
        use jig_domain_tasks::Stored as Tasks;
        let core = self.domain.core();
        for (key, row) in stored {
            match row {
                root::Record::Core(core::Record::Tasks(Tasks::Live(row))) => {
                    assert_eq!(core.tasks.task(row.number), Some(row.as_ref()), "tasks stored view {key:?}");
                }
                root::Record::Core(core::Record::Tasks(Tasks::Ledger(row))) => {
                    assert_eq!(core.tasks.funding_records().get(&row.funder), Some(row), "ledger stored view {key:?}");
                }
                root::Record::Core(core::Record::People(People::Person { number, identity })) => {
                    assert_eq!(core.people.identity(*number), Some(identity), "people stored view {key:?}");
                }
                root::Record::Core(core::Record::People(People::Roles { project, holdings })) => assert_eq!(
                    core.people.project_members(*project),
                    Some(holdings.as_ref()),
                    "roles stored view {key:?}"
                ),
                root::Record::Core(core::Record::Core(core::CoreRecord::RunProof(row))) => {
                    assert_eq!(core.proofs.get(&row.task), Some(row), "proof stored view {key:?}");
                }
                root::Record::Core(_) | root::Record::Connector { .. } => {}
            }
        }
    }

    pub(crate) fn remember_oracle_hello(&mut self, event: &root::Event) {
        if let root::Event::Core(core::Event::Fleet(fleet::Event::Hello { worker, channel, hello })) = event {
            self.oracle_hellos.retain(|(_, kept, _)| kept != channel);
            self.oracle_hellos.push((*worker, *channel, copy_hello(hello)));
        }
    }

    fn oracle_documents(&self, transients: &Transients) -> Vec<Answer> {
        let core = self.domain.core();
        let window = Window { rows: self.limits.core.documents.rows, after: None };
        let mut answers = Vec::new();
        for party in core.people.parties().as_slice() {
            let mut subjects = vec![
                Subject::Frame { party: *party },
                Subject::Inbox { party: *party },
                Subject::Accounts { party: *party },
            ];
            for task in core.tasks.view_tasks() {
                subjects.push(Subject::Task { task: task.number });
                subjects.push(Subject::Tree { task: task.number });
            }
            for row in self.store.rows.values() {
                if let root::Record::Core(core::Record::People(jig_domain_people::Stored::Policy { project, .. })) = row
                {
                    subjects.push(Subject::Project { project: *project });
                }
            }
            for subject in subjects {
                let answer = documents::make(core, &self.limits.core, *party, &subject, &window, self.env().wall);
                answers.push((*party, subject, answer.map(|document| transients.durable(&document))));
            }
        }
        answers
    }
}

fn copy_hello(hello: &fleet::Hello) -> fleet::Hello {
    fleet::Hello {
        bounds: hello.bounds.clone(),
        stop_bound: hello.stop_bound,
        slots: hello.slots,
        workstreams: hello.workstreams.clone(),
        hosting: hello.hosting.clone(),
    }
}

impl Drop for World {
    fn drop(&mut self) {
        if !std::thread::panicking() && self.oracle_skip.is_none() {
            let configuration = self.oracle_configuration.take().expect("story retains its startup configuration");
            self.restart_oracle(configuration);
        }
    }
}

impl World {
    /// Inspect wires after the same cold-restart proof as every other story.
    #[must_use]
    pub fn into_wires(mut self) -> Box<[u8]> {
        let configuration = self.oracle_configuration.take().expect("story configuration");
        self.restart_oracle(configuration);
        let models = &self.oracle_configuration.as_ref().expect("restarted configuration").core.settings.models;
        let replacement = super::Store::new(&self.limits, models);
        std::mem::replace(&mut self.store, replacement).into_wires()
    }

    /// A deliberately unrestartable fixture states why it opts out, at its caller.
    pub fn skip_restart_oracle(&mut self, reason: &'static str) {
        assert!(!reason.is_empty(), "an oracle opt-out states its reason");
        self.oracle_skip = Some(reason);
    }
}

pub(super) fn copy_configuration(configuration: &root::Config) -> root::Config {
    let given = &configuration.core;
    let mut policies = skein_lib::Map::with_capacity(given.defaults.policies.capacity());
    for (number, policy) in &given.defaults.policies {
        policies.insert(*number, policy.clone()).expect("copy retains the admitted policy count");
    }
    root::Config {
        core: core::Config {
            worker_names: given.worker_names.clone(),
            deployment: given.deployment,
            seed: given.seed,
            workers_in_process: given.workers_in_process,
            hub_turns: given.hub_turns,
            owners: given.owners.clone(),
            grants: given.grants.clone(),
            defaults: core::settings::Defaults {
                charters: given.defaults.charters.clone(),
                templates: given.defaults.templates.clone(),
                policies,
                rules: given.defaults.rules.clone(),
            },
            settings: core::Settings {
                deployment_provider: given.settings.deployment_provider,
                models: given.settings.models.clone(),
                resume_bytes: given.settings.resume_bytes,
                tail_bytes: given.settings.tail_bytes,
                tools: given.settings.tools,
                llm_providers: given.settings.llm_providers.clone(),
                key_bytes: given.settings.key_bytes,
                redirect_bytes: given.settings.redirect_bytes,
            },
            seed_projects: given
                .seed_projects
                .iter()
                .map(|project| core::SeedProject {
                    number: project.number,
                    name: project.name.clone(),
                    policy: project.policy.clone(),
                })
                .collect(),
            connectors: given.connectors.clone(),
        },
        first: configuration.first.clone(),
        second: configuration.second.clone(),
    }
}
