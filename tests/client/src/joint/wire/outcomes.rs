//! Closed owner facts and separate transport dispositions (contract.md, 4).
//! Uses the actual whole owner carrier; successful owner translation is kept separate from transport-only dispositions.
use super::owner;
use jig_client::DomainLimits as Limits;
use jig_client_domain::contract as client;
use jig_domain_people as people;
use skein_lib::{List, Token};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    SettingsChanged { revision: u32 },
    MovedToNewest { newest: u32, moved: u32, stayed: u32 },
    Owner(client::OwnerOutcome),
    MovingOut { account: u32, number: u32, source: u32 },
    Applied { connector: u16, token: Token },
    Again(people::Again),
    SignedOut,
    Refused(people::Refusal),
    CompoundRefused { at: u32, why: people::Refusal },
}

/// # Errors
/// Refuses malformed program outcomes or host answers above the link's receiving bounds.
#[expect(clippy::too_many_lines, reason = "one exhaustive translation retains every owner outcome")]
pub fn outcome(value: &people::Outcome, limits: &Limits) -> Result<Outcome, owner::Invalid> {
    let owner = match value {
        people::Outcome::MovedToNewest { newest, moved, stayed } => {
            return Ok(Outcome::MovedToNewest { newest: *newest, moved: *moved, stayed: *stayed });
        }
        people::Outcome::SettingsChanged { revision } => return Ok(Outcome::SettingsChanged { revision: *revision }),
        people::Outcome::AccountMovingOut { account, number, source } => {
            return Ok(Outcome::MovingOut { account: *account, number: *number, source: *source });
        }
        people::Outcome::AccountFinishing { source } => client::OwnerOutcome::AccountFinishing { source: *source },
        people::Outcome::AccountMovedIn { account, renewed } => {
            client::OwnerOutcome::AccountMovedIn { account: *account, renewed: *renewed }
        }
        people::Outcome::NoteEdited { name } => client::OwnerOutcome::NoteEdited { name: *name },
        people::Outcome::ServiceMade { person } => client::OwnerOutcome::ServiceMade { person: *person },
        people::Outcome::Adopted { project } => client::OwnerOutcome::Adopted { project: *project },
        people::Outcome::Compound { project, operations, first_message } => {
            client::OwnerOutcome::Compound { project: *project, operations: *operations, first_message: *first_message }
        }
        people::Outcome::CompoundRefused { at, why } => {
            return Ok(Outcome::CompoundRefused { at: *at, why: why.clone() });
        }
        people::Outcome::Proposed { proposal, to } => client::OwnerOutcome::Proposed { proposal: *proposal, to: *to },
        people::Outcome::Withdrawn { proposal } => client::OwnerOutcome::Withdrawn { proposal: *proposal },
        people::Outcome::Created { first, last, given } => client::OwnerOutcome::Created {
            first: *first,
            last: *last,
            given: given.map(|value| client::CreatedGiven { task: value.task, amount: value.amount }),
        },
        people::Outcome::Stopped { task } => client::OwnerOutcome::Stopped { task: *task },
        people::Outcome::Cancelled { task } => client::OwnerOutcome::Cancelled { task: *task },
        people::Outcome::Released { task } => client::OwnerOutcome::Released { task: *task },
        people::Outcome::PersonTaken { task } => client::OwnerOutcome::PersonTaken { task: *task },
        people::Outcome::PersonHandedBack { task } => client::OwnerOutcome::PersonHandedBack { task: *task },
        people::Outcome::PersonAnswered { task } => client::OwnerOutcome::PersonAnswered { task: *task },
        people::Outcome::Moved { task } => client::OwnerOutcome::Moved { task: *task },
        people::Outcome::Said { task, message, .. } => client::OwnerOutcome::Said { task: *task, message: *message },
        people::Outcome::QuestionAnswered { task, question, message } => {
            client::OwnerOutcome::QuestionAnswered { task: *task, question: *question, message: *message }
        }
        people::Outcome::Amended { task, .. } => client::OwnerOutcome::Amended { task: *task },

        people::Outcome::RolesSet { project } => client::OwnerOutcome::RolesSet { project: *project },
        people::Outcome::Renamed { project } => client::OwnerOutcome::Renamed { project: *project },
        people::Outcome::ProjectMade { project } => client::OwnerOutcome::ProjectMade { project: *project },
        people::Outcome::PolicyChanged { project } => client::OwnerOutcome::PolicyChanged { project: *project },
        people::Outcome::PoolSet { project, person } => {
            client::OwnerOutcome::PoolSet { project: *project, person: *person }
        }
        people::Outcome::MarkedSeen { seen } => client::OwnerOutcome::MarkedSeen {
            seen: match seen {
                people::Seen::Results { through } => client::SeenThrough::Results { through: *through },
                people::Seen::Conversation { task, through } => {
                    client::SeenThrough::Conversation { task: *task, through: *through }
                }
            },
        },
        people::Outcome::ProposalDecided { proposer, proposal, by, at, choice } => {
            client::OwnerOutcome::ProposalDecided {
                proposal: client::ProposalKey {
                    proposer: match proposer {
                        people::Proposer::Task(number) => client::Proposer::Task(*number),
                        people::Proposer::Party(number) => client::Proposer::Party(*number),
                    },
                    number: *proposal,
                },
                by: *by,
                at: *at,
                choice: match choice {
                    people::ProposalChoice::Accepted => client::OwnerProposalChoice::Accepted,
                    people::ProposalChoice::Rejected => client::OwnerProposalChoice::Rejected,
                    people::ProposalChoice::Passed => client::OwnerProposalChoice::Passed,
                    people::ProposalChoice::Withdrawn => client::OwnerProposalChoice::Withdrawn,
                    people::ProposalChoice::Stale => client::OwnerProposalChoice::Stale,
                },
            }
        }
        people::Outcome::EscalationDecided { task, revision, by, at, choice } => {
            client::OwnerOutcome::EscalationDecided {
                task: *task,
                revision: client::Revision { value: *revision },
                by: *by,
                at: *at,
                choice: match choice {
                    people::EscalationChoice::Released => client::OwnerEscalationChoice::Released,
                    people::EscalationChoice::Rejected => client::OwnerEscalationChoice::Rejected,
                    people::EscalationChoice::Passed => client::OwnerEscalationChoice::Passed,
                },
            }
        }
        people::Outcome::Applied { connector, token } => {
            return Ok(Outcome::Applied { connector: *connector, token: *token });
        }
        people::Outcome::Again(why) => return Ok(Outcome::Again(*why)),
        people::Outcome::SignedOut => return Ok(Outcome::SignedOut),
        people::Outcome::Refused(why) => return Ok(Outcome::Refused(why.clone())),
        people::Outcome::AccountSignInStarted { source, address } => {
            client::OwnerOutcome::AccountSignInStarted { source: *source, address: address.clone() }
        }
        people::Outcome::AccountSignedOut { confirmed, failed_hosts } => client::OwnerOutcome::AccountSignedOut {
            confirmed: *confirmed,
            failed_hosts: hosts(failed_hosts, *confirmed, limits)?,
        },
        people::Outcome::AccountKeyKept { source } => client::OwnerOutcome::AccountKeyKept { source: *source },
        people::Outcome::AccountProgramDeclared { source, name } => {
            client::OwnerOutcome::AccountProgramDeclared { source: *source, name: super::programs::program_name(name)? }
        }
        people::Outcome::AccountChanged => client::OwnerOutcome::AccountChanged,
        people::Outcome::AccountRemoved { confirmed, failed_hosts } => client::OwnerOutcome::AccountRemoved {
            confirmed: *confirmed,
            failed_hosts: hosts(failed_hosts, *confirmed, limits)?,
        },
    };
    Ok(Outcome::Owner(owner))
}

fn hosts(
    rows: &[people::AccountHostFailure],
    confirmed: bool,
    limits: &Limits,
) -> Result<Box<[client::AccountHostFailure]>, owner::Invalid> {
    if confirmed && !rows.is_empty() {
        return Err(owner::Invalid::Program);
    }
    let count = u32::try_from(rows.len()).map_err(|_| owner::Invalid::TooLarge)?;
    if count > 128 {
        return Err(owner::Invalid::TooLarge);
    }
    let mut bytes = List::<client::AccountHostFailure>::worst_case(count).ok_or(owner::Invalid::TooLarge)?;
    for row in rows {
        bytes = bytes
            .checked_add(u64::try_from(row.name.len()).map_err(|_| owner::Invalid::TooLarge)?)
            .ok_or(owner::Invalid::TooLarge)?;
    }
    if bytes > limits.document_bytes.checked_mul(2).ok_or(owner::Invalid::TooLarge)? {
        return Err(owner::Invalid::TooLarge);
    }
    let mut result = List::with_capacity(count);
    for row in rows {
        result
            .push(client::AccountHostFailure {
                name: super::programs::configured_name(&row.name)?,
                why: match row.why {
                    people::AccountHostWhy::NotInContact => client::AccountHostWhy::NotInContact,
                    people::AccountHostWhy::Lost => client::AccountHostWhy::Lost,
                    people::AccountHostWhy::Failed => client::AccountHostWhy::Failed,
                },
            })
            .map_err(|_| owner::Invalid::TooLarge)?;
    }
    Ok(result.into_boxed())
}

#[cfg(test)]
mod tests {
    use super::*;
    use skein_lib::Wall;
    fn outcome(value: &people::Outcome) -> Outcome {
        super::outcome(value, &crate::test::limits().client.domain).expect("representable fixture")
    }

    const FIRST_CREATED: u64 = 17;
    const LAST_CREATED: u64 = 19;
    const GIVEN_AMOUNT: u64 = 23;
    const GIVEN_CASES: [(Option<people::Given>, Option<client::CreatedGiven>); 2] = [
        (None, None),
        (
            Some(people::Given { task: FIRST_CREATED, amount: GIVEN_AMOUNT }),
            Some(client::CreatedGiven { task: FIRST_CREATED, amount: GIVEN_AMOUNT }),
        ),
    ];
    #[test]
    fn created_answers_keep_the_whole_batch_range() {
        for (before, after) in GIVEN_CASES {
            assert_eq!(
                outcome(&people::Outcome::Created { first: FIRST_CREATED, last: LAST_CREATED, given: before }),
                Outcome::Owner(client::OwnerOutcome::Created {
                    first: FIRST_CREATED,
                    last: LAST_CREATED,
                    given: after,
                })
            );
        }
    }

    #[test]
    fn committed_choices_keep_equal_number_makers_full_winner_clocks_and_every_closed_result_kind() {
        let at = Wall::from_nanos(u64::MAX);
        for maker in [people::Proposer::Task(u64::MAX), people::Proposer::Party(u64::MAX)] {
            let proposer = match maker {
                people::Proposer::Task(n) => client::Proposer::Task(n),
                people::Proposer::Party(n) => client::Proposer::Party(n),
            };
            for (before, after) in [
                (people::ProposalChoice::Accepted, client::OwnerProposalChoice::Accepted),
                (people::ProposalChoice::Rejected, client::OwnerProposalChoice::Rejected),
                (people::ProposalChoice::Passed, client::OwnerProposalChoice::Passed),
                (people::ProposalChoice::Withdrawn, client::OwnerProposalChoice::Withdrawn),
                (people::ProposalChoice::Stale, client::OwnerProposalChoice::Stale),
            ] {
                assert_eq!(
                    outcome(&people::Outcome::ProposalDecided {
                        proposer: maker,
                        proposal: 41,
                        by: 37,
                        at,
                        choice: before
                    }),
                    Outcome::Owner(client::OwnerOutcome::ProposalDecided {
                        proposal: client::ProposalKey { proposer, number: 41 },
                        by: 37,
                        at,
                        choice: after
                    })
                );
            }
        }
        for (before, after) in [
            (people::EscalationChoice::Released, client::OwnerEscalationChoice::Released),
            (people::EscalationChoice::Rejected, client::OwnerEscalationChoice::Rejected),
            (people::EscalationChoice::Passed, client::OwnerEscalationChoice::Passed),
        ] {
            assert_eq!(
                outcome(&people::Outcome::EscalationDecided {
                    task: 99,
                    revision: u64::MAX,
                    by: 37,
                    at,
                    choice: before
                }),
                Outcome::Owner(client::OwnerOutcome::EscalationDecided {
                    task: 99,
                    revision: client::Revision { value: u64::MAX },
                    by: 37,
                    at,
                    choice: after
                })
            );
        }
    }

    #[test]
    fn owner_outcomes_keep_independent_messages_counts_seen_positions_and_released_flags_without_inventing_makers() {
        for released in [false, true] {
            assert_eq!(
                outcome(&people::Outcome::Said { task: u64::MAX, message: 17 }),
                Outcome::Owner(client::OwnerOutcome::Said { task: u64::MAX, message: 17 })
            );
            assert_eq!(
                outcome(&people::Outcome::Amended { task: 11, released }),
                Outcome::Owner(client::OwnerOutcome::Amended { task: 11 })
            );
        }
        assert_eq!(
            outcome(&people::Outcome::QuestionAnswered { task: 11, question: 19, message: u64::MAX }),
            Outcome::Owner(client::OwnerOutcome::QuestionAnswered { task: 11, question: 19, message: u64::MAX })
        );

        assert_eq!(
            outcome(&people::Outcome::MarkedSeen { seen: people::Seen::Conversation { task: 11, through: u64::MAX } }),
            Outcome::Owner(client::OwnerOutcome::MarkedSeen {
                seen: client::SeenThrough::Conversation { task: 11, through: u64::MAX }
            })
        );
        assert_eq!(
            outcome(&people::Outcome::MarkedSeen { seen: people::Seen::Results { through: 0 } }),
            Outcome::Owner(client::OwnerOutcome::MarkedSeen { seen: client::SeenThrough::Results { through: 0 } })
        );
        for why in [people::Again::Busy, people::Again::NotReady] {
            assert_eq!(outcome(&people::Outcome::Again(why)), Outcome::Again(why));
        }
        assert_eq!(outcome(&people::Outcome::SignedOut), Outcome::SignedOut);
        let token = Token::new(u64::MAX);
        assert_eq!(
            outcome(&people::Outcome::Applied { connector: u16::MAX, token }),
            Outcome::Applied { connector: u16::MAX, token }
        );
        assert_eq!(
            outcome(&people::Outcome::Refused(people::Refusal::Role { held: None })),
            Outcome::Refused(people::Refusal::Role { held: None })
        );
    }
}

#[cfg(test)]
mod programs {
    use super::*;
    fn limits() -> Limits {
        crate::test::limits().client.domain
    }
    fn failed() -> people::Outcome {
        people::Outcome::AccountSignedOut {
            confirmed: false,
            failed_hosts: Box::new([people::AccountHostFailure {
                name: b"configured-worker".as_slice().into(),
                why: people::AccountHostWhy::Lost,
            }]),
        }
    }
    #[test]
    fn malformed_declaration_names_are_refused() {
        assert_eq!(
            outcome(
                &people::Outcome::AccountProgramDeclared { source: 1, name: b"bad name".as_slice().into() },
                &limits()
            ),
            Err(owner::Invalid::Program)
        );
    }
    #[test]
    fn confirmed_answers_refuse_host_failures() {
        let people::Outcome::AccountSignedOut { failed_hosts, .. } = failed() else { panic!("sign-out fixture") };
        assert_eq!(
            outcome(&people::Outcome::AccountSignedOut { confirmed: true, failed_hosts }, &limits()),
            Err(owner::Invalid::Program)
        );
    }
    #[test]
    fn host_answers_hold_at_the_native_budget() {
        let bytes = List::<client::AccountHostFailure>::worst_case(1).expect("host list") + 17;
        let limits = Limits { document_bytes: bytes.div_ceil(2), turn_text: limits().turn_text, ..limits() };
        assert!(outcome(&failed(), &limits).is_ok());
        assert_eq!(
            outcome(
                &failed(),
                &Limits { document_bytes: limits.document_bytes - 1, turn_text: limits.turn_text, ..limits }
            ),
            Err(owner::Invalid::TooLarge)
        );
    }
    #[test]
    fn host_answers_keep_whole_configured_names() {
        let value = outcome(&failed(), &Limits { text: 1, turn_text: 1, ..limits() }).expect("independent name bound");
        let Outcome::Owner(client::OwnerOutcome::AccountSignedOut { failed_hosts, .. }) = value else {
            panic!("host answer")
        };
        assert_eq!(failed_hosts[0].name.as_ref(), b"configured-worker");
    }
}
