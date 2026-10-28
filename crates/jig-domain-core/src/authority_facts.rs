//! Authority check findings translated with the request's accounting snapshot
//! (client/domain/contract.md, section 4; domain/authority.md, section 8.4).
//! This module keeps no state. The pure check supplies which source and component
//! lacks; the core supplies the requested values and that source's current caps.
use crate::Core;
use jig_domain_authority as authority;
use jig_domain_people as people;
use skein_lib::Queue;

impl Core {
    #[expect(clippy::too_many_lines, reason = "one exhaustive finding translation retains each authority fact")]
    pub(crate) fn request_refusal(
        &self,
        ask: &authority::PersonAsk,
        answer: authority::Answer,
        findings: &mut Queue<authority::Finding>,
    ) -> people::Refusal {
        let finding = findings.pop().expect("a refused authority check explains its reason");
        let held = Some(people::Role::from_number(ask.role));
        let proposable = match answer {
            authority::Answer::Propose | authority::Answer::Refuse => Some(self.request_accepter(ask, answer)),
            authority::Answer::Allow | authority::Answer::Wait => None,
        };
        match finding {
            authority::Finding::UnknownProject => people::Refusal::Unknown,
            authority::Finding::UnknownRole | authority::Finding::Unpermitted | authority::Finding::Undecidable => {
                people::Refusal::Role { held }
            }
            authority::Finding::Oversized => self.limit_refusal(people::LimitKind::Authority),
            authority::Finding::Arithmetic => self.limit_refusal(people::LimitKind::Arithmetic),
            authority::Finding::ResourceUnavailable => {
                people::Refusal::Authority { proposable, lacking: people::Lack::ResourceUnavailable }
            }
            authority::Finding::Authority { source, lacks } => {
                let cap = self.source_cap(ask, source);
                let lacking = if lacks.spend {
                    people::Lack::Budget { needs: request_spend(&ask.request), left: source_spend(ask, source, cap) }
                } else if lacks.tasks {
                    people::Lack::Tasks { left: source_tasks(ask, source, cap.delegation.tasks) }
                } else if lacks.depth {
                    people::Lack::Depth { most: request_depth(&ask.request, source, cap.delegation.depth) }
                } else if lacks.deadline {
                    people::Lack::Deadline
                } else if lacks.grants {
                    people::Lack::Grant { kind: request_grant(&ask.request, cap, &self.authority.rules().implies) }
                } else if lacks.executors {
                    people::Lack::TaskKind { executor: None }
                } else if lacks.tools {
                    people::Lack::Tool
                } else if lacks.models {
                    people::Lack::Models
                } else if lacks.notes {
                    people::Lack::Scope
                } else {
                    unreachable!("authority deficit has a component")
                };
                people::Refusal::Authority { proposable, lacking }
            }
            authority::Finding::Spend { source } | authority::Finding::Price { source } => {
                let cap = self.source_cap(ask, source);
                people::Refusal::Authority {
                    proposable,
                    lacking: people::Lack::Budget {
                        needs: request_spend(&ask.request),
                        left: source_spend(ask, source, cap),
                    },
                }
            }
            authority::Finding::Tasks { source } => people::Refusal::Authority {
                proposable,
                lacking: people::Lack::Tasks {
                    left: source_tasks(ask, source, self.source_cap(ask, source).delegation.tasks),
                },
            },
            authority::Finding::Executor { .. } => {
                people::Refusal::Authority { proposable, lacking: people::Lack::TaskKind { executor: None } }
            }
            authority::Finding::Grant { source } => people::Refusal::Authority {
                proposable,
                lacking: people::Lack::Grant {
                    kind: request_grant(&ask.request, self.source_cap(ask, source), &self.authority.rules().implies),
                },
            },
            authority::Finding::RunBudget | authority::Finding::RunCap => people::Refusal::Authority {
                proposable,
                lacking: people::Lack::Budget {
                    needs: request_spend(&ask.request),
                    left: available(ask.pool).min(self.authority.rules().maximum_run_spend),
                },
            },
            authority::Finding::Deadline => people::Refusal::Authority { proposable, lacking: people::Lack::Deadline },
            authority::Finding::Account => people::Refusal::Authority { proposable, lacking: people::Lack::Account },
            authority::Finding::Writer => people::Refusal::Authority { proposable, lacking: people::Lack::Writer },
            authority::Finding::Tool => people::Refusal::Authority { proposable, lacking: people::Lack::Tool },
            authority::Finding::Scope { .. } => people::Refusal::Authority { proposable, lacking: people::Lack::Scope },
            authority::Finding::ResourceAccess => {
                people::Refusal::Authority { proposable, lacking: people::Lack::ResourceAccess }
            }
            authority::Finding::Reference => {
                people::Refusal::Authority { proposable, lacking: people::Lack::Reference }
            }
            authority::Finding::Required { judge } => people::Refusal::Authority {
                proposable,
                lacking: people::Lack::Required { connector: judge.connector, kind: judge.requirement },
            },
            authority::Finding::Failed { judge } => people::Refusal::Authority {
                proposable,
                lacking: people::Lack::Failed { connector: judge.connector, kind: judge.requirement },
            },
            authority::Finding::Unguarded { judge } => people::Refusal::Authority {
                proposable,
                lacking: people::Lack::Unguarded { connector: judge.connector, kind: judge.requirement },
            },
            authority::Finding::PeriodSpend => people::Refusal::Authority {
                proposable,
                lacking: people::Lack::Budget {
                    needs: ask.pool.budget,
                    left: self.authority.role(ask.project, ask.role).expect("checked role").period_spend,
                },
            },
        }
    }

    fn request_accepter(&self, ask: &authority::PersonAsk, answer: authority::Answer) -> people::Accepter {
        if answer != authority::Answer::Propose {
            return people::Accepter::Nobody;
        }
        let kind = match &ask.request {
            authority::PersonRequest::Create(_) => authority::ProposalKind::Batch,
            authority::PersonRequest::Amend(_) => authority::ProposalKind::Amend,
            authority::PersonRequest::Allot(_) | authority::PersonRequest::Move(_) => authority::ProposalKind::Widen,
            authority::PersonRequest::Accept(action) => match action {
                authority::Action::Batch(_) => authority::ProposalKind::Batch,
                authority::Action::Effect(_) => authority::ProposalKind::Effect,
                authority::Action::Widen(_) => authority::ProposalKind::Widen,
                authority::Action::Amend(_) => authority::ProposalKind::Amend,
                authority::Action::Escalate { .. } => authority::ProposalKind::Escalation,
            },
            authority::PersonRequest::Cancel
            | authority::PersonRequest::Release
            | authority::PersonRequest::Watch
            | authority::PersonRequest::Policy => return people::Accepter::Nobody,
        };
        match crate::acts::bounds::recipient(self, ask.project, kind) {
            Some(crate::acts::Recipient::Roles { roles, .. }) => match roles.iter().copied().min() {
                Some(role) => people::Accepter::Role(role),
                None => people::Accepter::Nobody,
            },
            Some(
                crate::acts::Recipient::Task(_)
                | crate::acts::Recipient::Party(_)
                | crate::acts::Recipient::Role { .. },
            )
            | None => people::Accepter::Nobody,
        }
    }

    fn source_cap(&self, ask: &authority::PersonAsk, source: authority::Source) -> &authority::Authority {
        match source {
            authority::Source::Task | authority::Source::Role => {
                &self.authority.role(ask.project, ask.role).expect("checked role").authority
            }
            authority::Source::Project => &self.authority.policy(ask.project).expect("checked policy").ceiling,
            authority::Source::Deployment => &self.authority.rules().ceiling,
        }
    }
}

fn available(numbers: authority::Numbers) -> u64 {
    numbers.budget.saturating_sub(numbers.spent).saturating_sub(numbers.spent_below).saturating_sub(numbers.reserved)
}

fn request_spend(request: &authority::PersonRequest) -> u64 {
    match request {
        authority::PersonRequest::Create(batch) => batch_spend(batch),
        authority::PersonRequest::Allot(value)
        | authority::PersonRequest::Amend(value)
        | authority::PersonRequest::Move(value) => value.budget.spend,
        authority::PersonRequest::Accept(action) => match action {
            authority::Action::Batch(batch) => batch_spend(batch),
            authority::Action::Widen(value) | authority::Action::Amend(value) => value.budget.spend,
            authority::Action::Effect(effect) => effect.price.unwrap_or(0),
            authority::Action::Escalate { release } => match release {
                Some(value) => value.budget.spend,
                None => 0,
            },
        },
        authority::PersonRequest::Cancel
        | authority::PersonRequest::Release
        | authority::PersonRequest::Watch
        | authority::PersonRequest::Policy => 0,
    }
}

fn batch_spend(batch: &[authority::Delegate]) -> u64 {
    let mut spend = 0_u64;
    for member in batch {
        spend = spend.saturating_add(member.authority.budget.spend);
    }
    spend
}

fn missing_grant(
    value: &authority::Authority,
    cap: &authority::Authority,
    implies: &authority::Implies,
) -> Option<u16> {
    for grant in &value.grants {
        let mut covered = false;
        for covering in &cap.grants {
            if authority::grant_at_most(grant, covering, implies) {
                covered = true;
                break;
            }
        }
        if !covered {
            return Some(grant.kind);
        }
    }
    None
}

fn request_grant(request: &authority::PersonRequest, cap: &authority::Authority, implies: &authority::Implies) -> u16 {
    let kind = match request {
        authority::PersonRequest::Create(batch) => batch_grant(batch, cap, implies),
        authority::PersonRequest::Allot(value)
        | authority::PersonRequest::Amend(value)
        | authority::PersonRequest::Move(value) => missing_grant(value, cap, implies),
        authority::PersonRequest::Accept(action) => match action {
            authority::Action::Batch(batch) => batch_grant(batch, cap, implies),
            authority::Action::Widen(value) | authority::Action::Amend(value) => missing_grant(value, cap, implies),
            authority::Action::Effect(effect) => Some(effect.kind),
            authority::Action::Escalate { release } => match release {
                Some(value) => missing_grant(value, cap, implies),
                None => None,
            },
        },
        authority::PersonRequest::Cancel
        | authority::PersonRequest::Release
        | authority::PersonRequest::Watch
        | authority::PersonRequest::Policy => None,
    };
    kind.expect("a grant finding identifies an uncovered requested grant")
}

fn batch_grant(batch: &[authority::Delegate], cap: &authority::Authority, implies: &authority::Implies) -> Option<u16> {
    for member in batch {
        if let Some(kind) = missing_grant(&member.authority, cap, implies) {
            return Some(kind);
        }
        for symbolic in &member.symbolic {
            let mut covered = false;
            for covering in &cap.grants {
                if authority::grant_at_most(symbolic, covering, implies) {
                    covered = true;
                    break;
                }
            }
            if !covered {
                return Some(symbolic.kind);
            }
        }
    }
    None
}

fn source_spend(ask: &authority::PersonAsk, source: authority::Source, cap: &authority::Authority) -> u64 {
    match source {
        authority::Source::Task | authority::Source::Role => cap.budget.spend.min(available(ask.pool)),
        authority::Source::Project | authority::Source::Deployment => cap.budget.spend,
    }
}

fn source_tasks(ask: &authority::PersonAsk, source: authority::Source, cap: u32) -> u32 {
    match source {
        authority::Source::Task | authority::Source::Role => cap.min(ask.tasks_left),
        authority::Source::Project | authority::Source::Deployment => cap,
    }
}

fn request_depth(request: &authority::PersonRequest, source: authority::Source, cap: u32) -> u32 {
    let batch = match request {
        authority::PersonRequest::Create(_) => true,
        authority::PersonRequest::Accept(action) => match action {
            authority::Action::Batch(_) => true,
            authority::Action::Widen(_)
            | authority::Action::Amend(_)
            | authority::Action::Effect(_)
            | authority::Action::Escalate { .. } => false,
        },
        authority::PersonRequest::Allot(_)
        | authority::PersonRequest::Amend(_)
        | authority::PersonRequest::Move(_)
        | authority::PersonRequest::Cancel
        | authority::PersonRequest::Release
        | authority::PersonRequest::Watch
        | authority::PersonRequest::Policy => false,
    };
    match source {
        authority::Source::Task | authority::Source::Role => {
            if batch {
                cap.saturating_sub(1)
            } else {
                cap
            }
        }
        authority::Source::Project | authority::Source::Deployment => cap,
    }
}
