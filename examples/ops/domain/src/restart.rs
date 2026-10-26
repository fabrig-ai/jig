//! Copy the core-selected script. A connector reports when it has completed
//! reads or settlement; the root never advances it by guessing (domain/root.md, 8).
use crate::domain::{Work, held};
use crate::{Domain, Limits, Output, Range, Record, Store, Write};
use jig_domain_accounts as accounts;
use jig_domain_core as core;
use jig_domain_notes as notes;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_ops_domain_connectors::Connector;
use jig_ops_domain_infrastructure as infrastructure;
use jig_ops_domain_observability as observability;
use skein_lib::{Decision, Env, Queue};

pub(crate) fn route(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision<Write, Output>,
    request: core::RestartRequest,
    work: &mut Queue<Work>,
) {
    match request {
        core::RestartRequest::Idle
        | core::RestartRequest::Settings { .. }
        | core::RestartRequest::ProjectPolicy { .. }
        | core::RestartRequest::Unresolved { .. }
        | core::RestartRequest::Refused { .. } => {}
        core::RestartRequest::Step(step) => match step {
            core::RestartStep::LoadCore => held(decision, Output::Load { step, range: Range::Core }),
            core::RestartStep::LoadInbox => {
                // The accounts' records came with the core's ranges: want
                // each real source once its account rows are restored.
                let requests = core::want_accounts(
                    &mut domain.core,
                    &Env { now: env.now, wall: env.wall, limits: env.limits.core },
                );
                crate::route::core_out(domain, env, decision, requests, work);
                let census = domain.core.census(&env.limits.core);
                held(decision, Output::Load { step, range: Range::Census(census) });
            }
            core::RestartStep::RestoreConnector { connector } => {
                let range = match jig_ops_domain_connectors::connector(connector) {
                    Some(Connector::Infrastructure) => Range::Infrastructure,
                    Some(Connector::Observability) => Range::Observability,
                    None => {
                        let _request = domain.core.restart_refuse();
                        return;
                    }
                };
                held(decision, Output::Load { step, range });
            }
            core::RestartStep::Switch => {
                work.push(Work::Core(core::Event::Tasks(tasks::Event::Check)));
                work.push(Work::SwitchChecked);
            }
            core::RestartStep::AdoptRuns => {
                work.push(Work::Core(core::Event::Tasks(tasks::Event::Restored)));
                work.push(Work::AdoptClaims);
            }
            core::RestartStep::ReadAfresh { connector } => match jig_ops_domain_connectors::connector(connector) {
                Some(Connector::Infrastructure) => {
                    work.push(Work::Infrastructure(infrastructure::Event::ReadAfresh));
                }
                Some(Connector::Observability) => {
                    work.push(Work::Observability(observability::Event::ReadAfresh));
                }
                None => {
                    let _request = domain.core.restart_refuse();
                }
            },
            core::RestartStep::SettleOutbox { connector } => match jig_ops_domain_connectors::connector(connector) {
                Some(Connector::Infrastructure) => {
                    work.push(Work::Infrastructure(infrastructure::Event::Restart));
                }
                Some(Connector::Observability) => {
                    work.push(Work::Observability(observability::Event::Restart));
                }
                None => {
                    let _request = domain.core.restart_refuse();
                }
            },
            core::RestartStep::Open => {
                let _request = domain.core.restart_done(step);
                work.push(Work::Infrastructure(infrastructure::Event::SendCloses));
            }
        },
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the store adapter preserves the core restart order across each store terminal"
)]
pub(crate) fn store(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision<Write, Output>,
    store: Store,
    work: &mut Queue<Work>,
) {
    match store {
        Store::Switched => {
            if domain.core.stopping() {
                return;
            }
            crate::domain::write(domain, decision, Write::Wires);
            held(decision, Output::ToChild(crate::Released::RestartDone(core::RestartStep::Switch)));
        }
        Store::Committed { .. } | Store::Failed { .. } => unreachable!("store terminals belong to the journal"),
        Store::Loaded(event) => work.push(Work::Core(event)),
        Store::Restored(step) => {
            if step == core::RestartStep::LoadCore {
                if !domain.core.settings_restored(&Env { now: env.now, wall: env.wall, limits: env.limits.core }) {
                    return;
                }
                work.push(Work::Core(core::Event::SettingsCommit {
                    room: core::room_max(&env.limits.core).expect("validated core room").writes,
                }));
                // The completed core range is also the people's restoration
                // terminal. Retain it when copying this cold-load adapter.
                work.push(Work::Core(core::Event::People(people::Event::Restored)));
            }
            let request = domain.core.restart_done(step);
            work.push(Work::Restart(request));
        }
        Store::Transcript { task, rows, done } => {
            // A message can invalidate preparation while this store read is
            // in flight. Its old answer belongs to no current preparation.
            if !domain.core.transcripts.contains_key(&task) {
                return;
            }
            for (row, text) in rows {
                if !domain.core.append_transcript(task, row, text) {
                    work.push(Work::Core(core::Event::PreparationFailed { task }));
                    return;
                }
            }
            if done {
                let event = if domain.core.transcript_readable(task) {
                    core::Event::StartBrief { task }
                } else {
                    core::Event::TranscriptUnread { task }
                };
                work.push(Work::Core(event));
            }
        }
        Store::Restore(record) => {
            let after_settings = match &record {
                Record::Core(core::Record::Core(core::CoreRecord::Settings(_) | core::CoreRecord::Deployment(_))) => {
                    false
                }
                Record::Core(_) | Record::Infrastructure(_) | Record::Observability(_) => true,
            };
            if domain.core.restart_step() == Some(core::RestartStep::LoadCore)
                && after_settings
                && !domain.core.settings_restored(&Env { now: env.now, wall: env.wall, limits: env.limits.core })
            {
                return;
            }
            let valid = if domain.core.restart_step() == Some(core::RestartStep::LoadInbox) {
                if let Record::Core(row) = record {
                    domain.core.restore_inbox_row(&env.limits.core, &row)
                } else {
                    false
                }
            } else {
                match record {
                    Record::Core(core::Record::Core(row)) => match domain.core.restore_core(row, &env.limits.core) {
                        core::Restored::Deployment { commits } => {
                            domain.restored_commits = Some(commits);
                            true
                        }
                        core::Restored::Live | core::Restored::Archive => true,
                        core::Restored::Rejected => false,
                    },
                    Record::Core(core::Record::Tasks(row)) => {
                        if domain.core.restore_task_row(&row) {
                            let mut out = Queue::with_capacity(tasks::max_out(&env.limits.core.tasks));
                            tasks::step(
                                &mut domain.core.tasks,
                                &Env { now: env.now, wall: env.wall, limits: env.limits.core.tasks },
                                tasks::Event::Restore { record: row },
                                &mut out,
                            );
                            out.is_empty()
                        } else {
                            false
                        }
                    }
                    Record::Core(core::Record::People(people::Stored::Policy { project, name, value })) => {
                        domain.core.restore_policy(&env.limits.core, project, name, value)
                    }
                    Record::Core(core::Record::People(row)) => {
                        if domain.core.restore_people_row(&row) {
                            let mut out = Queue::with_capacity(people::max_out(&env.limits.core.people));
                            people::step(
                                &mut domain.core.people,
                                &Env { now: env.now, wall: env.wall, limits: env.limits.core.people },
                                people::Event::Restore { record: row },
                                &mut out,
                            );
                            out.is_empty()
                        } else {
                            false
                        }
                    }
                    Record::Core(core::Record::Notes(row)) => {
                        work.push(Work::Core(core::Event::Notes(notes::Event::Restore { record: row })));
                        true
                    }
                    Record::Core(core::Record::Accounts(record)) => restore_accounts(domain, env, record),
                    Record::Infrastructure(record) => {
                        let valid = restore_roles(domain, env, &record);
                        if valid {
                            work.push(Work::Infrastructure(infrastructure::Event::Restore { record }));
                        }
                        valid
                    }
                    Record::Observability(record) => {
                        work.push(Work::Observability(observability::Event::Restore { record }));
                        true
                    }
                }
            };
            if !valid {
                let _request = domain.core.restart_refuse();
            }
        }
    }
}

/// A record breaking the accounts child's rules is counted there and dropped;
/// it never refuses the start.
fn restore_accounts(domain: &mut Domain, env: &Env<Limits>, record: accounts::Stored) -> bool {
    let mut out = Queue::with_capacity(accounts::max_out(&env.limits.core.accounts));
    accounts::step(
        &mut domain.core.accounts,
        &Env { now: env.now, wall: env.wall, limits: env.limits.core.accounts },
        accounts::Event::Restore { record },
        &mut out,
    );
    out.is_empty()
}

/// Preserve connector-owned project adoption before the core admits live claims.
fn restore_roles(domain: &mut Domain, env: &Env<Limits>, record: &infrastructure::Record) -> bool {
    let mut valid = true;
    match record {
        infrastructure::Record::Rely { project, resources, .. } => {
            for named in resources {
                valid &= domain.core.restore_resource_role(
                    &Env { now: env.now, wall: env.wall, limits: env.limits.core },
                    *project,
                    tasks::Name {
                        connector: jig_ops_domain_connectors::number(Connector::Infrastructure),
                        path: named.resource.segments(),
                    },
                    crate::translate::resource_role(named.role),
                );
            }
        }
        infrastructure::Record::Closing { .. }
        | infrastructure::Record::Drift(_)
        | infrastructure::Record::Procedure(_)
        | infrastructure::Record::Proposal { .. }
        | infrastructure::Record::Outbox(_)
        | infrastructure::Record::EnvironmentEnd { .. }
        | infrastructure::Record::ApplicationAnswer { .. }
        | infrastructure::Record::Made { .. } => {}
    }
    valid
}
