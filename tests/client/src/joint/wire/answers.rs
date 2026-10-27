//! Owner answers and protocol dispositions (client/domain/contract.md, 4).
//! The caller retains the original reply route and releases held answers only
//! after the engine's actual commit. Translation itself attests no durability.
use super::{outcomes, refusals};
use jig_client_domain::contract as client;
use jig_domain_people as people;
use skein_lib::{RequestEnvelope, Token, Wall};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    SignedIn {
        person: u64,
        sign_in: u64,
        expires: Wall,
    },
    Final(client::Answer),
    MovingOut {
        account: u32,
        number: u32,
        source: u32,
    },
    Again(people::Again),
    SignedOut,
    /// The connector's separate whole application answer supplies its body.
    Applied {
        connector: u16,
        token: Token,
    },
}

/// # Errors
/// Refuses an unsupported refusal or a named LLM fact outside the receiving bounds.
pub fn reply(
    value: people::Reply,
    models: &[jig_domain_core::Model],
    limits: &jig_client::DomainLimits,
) -> Result<Reply, super::owner::Invalid> {
    Ok(match value {
        people::Reply::SignedIn { person, sign_in, expires } => Reply::SignedIn { person, sign_in, expires },
        people::Reply::Signer(_) => panic!("Whose is a separate provider read"),
        people::Reply::SignedOut => Reply::SignedOut,
        people::Reply::Again(why) => Reply::Again(why),
        people::Reply::Full { .. } => Reply::Again(people::Again::Busy),
        people::Reply::Refused(why) => {
            Reply::Final(client::Answer::Refused { refusal: refusal(why, models, limits)?, at: None })
        }
        people::Reply::Outcome(value) => match outcomes::outcome(&value, limits)? {
            outcomes::Outcome::MovingOut { account, number, source } => Reply::MovingOut { account, number, source },
            outcomes::Outcome::Restored(value) => Reply::Final(client::Answer::Done(client::Outcome::Restored(value))),
            outcomes::Outcome::MovedToNewest { newest, moved, stayed } => {
                Reply::Final(client::Answer::Done(client::Outcome::MovedToNewest(client::OutcomeMovedToNewest {
                    newest,
                    moved,
                    stayed,
                })))
            }
            outcomes::Outcome::SettingsChanged { revision } => {
                Reply::Final(client::Answer::Done(client::Outcome::SettingsChanged(client::OutcomeSettingsChanged {
                    revision,
                })))
            }
            outcomes::Outcome::Owner(value) => Reply::Final(client::Answer::Done(client::Outcome::Owner(value))),
            outcomes::Outcome::Applied { connector, token } => Reply::Applied { connector, token },
            outcomes::Outcome::Again(why) => Reply::Again(why),
            outcomes::Outcome::SignedOut => Reply::SignedOut,
            outcomes::Outcome::CompoundRefused { at, why } => {
                Reply::Final(client::Answer::Refused { refusal: refusal(why, models, limits)?, at: Some(at) })
            }
            outcomes::Outcome::Refused(why) => {
                Reply::Final(client::Answer::Refused { refusal: refusal(why, models, limits)?, at: None })
            }
        },
    })
}

fn refusal(
    value: people::Refusal,
    models: &[jig_domain_core::Model],
    limits: &jig_client::DomainLimits,
) -> Result<client::Refusal, super::owner::Invalid> {
    match value {
        people::Refusal::Llm { model, why } => super::llm::refusal(model, why, models, limits),
        people::Refusal::Forgotten { .. }
        | people::Refusal::Stale { .. }
        | people::Refusal::Full { .. }
        | people::Refusal::Circle { .. }
        | people::Refusal::Dependency { .. }
        | people::Refusal::Kept { .. }
        | people::Refusal::Role { .. }
        | people::Refusal::Authority { .. }
        | people::Refusal::Limit { .. }
        | people::Refusal::Moved { .. }
        | people::Refusal::Ended { .. }
        | people::Refusal::Taken { .. }
        | people::Refusal::NotWellFormed { .. }
        | people::Refusal::NotWaiting
        | people::Refusal::NoFurther
        | people::Refusal::Unknown
        | people::Refusal::Account { .. }
        | people::Refusal::SignInRedirect { .. }
        | people::Refusal::AccountMove { .. }
        | people::Refusal::Provider
        | people::Refusal::NameTaken
        | people::Refusal::NotOffered
        | people::Refusal::NoEscalationHolder
        | people::Refusal::KeyConflict => refusals::refusal(value),
    }
}

/// Only complete keyed protocol terminals become request-table inputs. A
/// sign-in or connector reference must use its separately retained owner route.
/// # Errors
/// Refuses a terminal whose supplied owner payload cannot be represented or admitted whole.
pub fn keyed(value: Reply) -> Result<(RequestEnvelope, Option<client::Answer>), Reply> {
    match value {
        Reply::Final(answer) => Ok((RequestEnvelope::Final, Some(answer))),
        Reply::Again(_) => Ok((RequestEnvelope::Again, None)),
        Reply::SignedOut => Ok((RequestEnvelope::SignedOut, None)),
        other @ (Reply::SignedIn { .. } | Reply::Applied { .. } | Reply::MovingOut { .. }) => Err(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_sparse_entrance_refusal_and_retained_refusal_keep_the_same_exact_original_child_terminal() {
        let why = people::Refusal::Moved { now: people::Moved::Person { revision: u64::MAX } };
        let entrance = reply(people::Reply::Refused(why.clone()), &[], &crate::test::limits().client.domain)
            .expect("representable owner value");
        let retained =
            reply(people::Reply::Outcome(people::Outcome::Refused(why)), &[], &crate::test::limits().client.domain)
                .expect("representable owner value");
        assert_eq!(entrance, retained);
        assert_eq!(
            keyed(entrance),
            Ok((
                RequestEnvelope::Final,
                Some(client::Answer::Refused {
                    refusal: client::Refusal::Moved {
                        now: client::Revision { value: u64::MAX },
                        what: client::Moved::PersonTask,
                    },
                    at: None
                })
            ))
        );
    }

    #[test]
    fn unsaved_retry_and_signed_out_never_manufacture_a_final_answer_or_drop_the_source_retry_kind() {
        for why in [people::Again::Busy, people::Again::NotReady] {
            let direct = reply(people::Reply::Again(why), &[], &crate::test::limits().client.domain)
                .expect("representable owner value");
            let outcome =
                reply(people::Reply::Outcome(people::Outcome::Again(why)), &[], &crate::test::limits().client.domain)
                    .expect("representable owner value");
            assert_eq!(direct, Reply::Again(why));
            assert_eq!(direct, outcome);
            assert_eq!(keyed(direct), Ok((RequestEnvelope::Again, None)));
        }
        assert_eq!(
            keyed(
                reply(people::Reply::SignedOut, &[], &crate::test::limits().client.domain)
                    .expect("representable owner value")
            ),
            Ok((RequestEnvelope::SignedOut, None))
        );
        assert_eq!(
            keyed(
                reply(people::Reply::Outcome(people::Outcome::SignedOut), &[], &crate::test::limits().client.domain)
                    .expect("representable owner value")
            ),
            Ok((RequestEnvelope::SignedOut, None))
        );
    }

    #[test]
    fn actual_sign_in_and_connector_reference_require_their_owner_routes_and_keep_full_width_values() {
        let authenticated = reply(
            people::Reply::SignedIn { person: u64::MAX, sign_in: 0, expires: Wall::EPOCH },
            &[],
            &crate::test::limits().client.domain,
        )
        .expect("representable owner value");
        assert_eq!(authenticated, Reply::SignedIn { person: u64::MAX, sign_in: 0, expires: Wall::EPOCH });
        assert_eq!(keyed(authenticated.clone()), Err(authenticated));
        let applied = reply(
            people::Reply::Outcome(people::Outcome::Applied { connector: u16::MAX, token: Token::new(u64::MAX) }),
            &[],
            &crate::test::limits().client.domain,
        )
        .expect("representable owner value");
        assert_eq!(applied, Reply::Applied { connector: u16::MAX, token: Token::new(u64::MAX) });
        assert_eq!(keyed(applied.clone()), Err(applied));
        assert_eq!(
            keyed(
                reply(
                    people::Reply::Outcome(people::Outcome::Said { task: u64::MAX, message: u64::MAX }),
                    &[],
                    &crate::test::limits().client.domain
                )
                .expect("representable owner value")
            ),
            Ok((
                RequestEnvelope::Final,
                Some(client::Answer::Done(client::Outcome::Owner(client::OwnerOutcome::Said {
                    task: u64::MAX,
                    message: u64::MAX,
                })))
            ))
        );
    }
}
