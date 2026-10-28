//! Declarations reserve a durable name before any host operation is possible.
use jig_accounts_world::{LIMITS, World, account, provider, providers, source};
use jig_domain_accounts::{Event, Holder, Kind, Refusal, Request, Stored, programs};
use skein_lib::{Env, Queue};
const HOLDER: Holder = Holder::Party(7);
const OTHER: Holder = Holder::Party(8);
const PROVIDER: u32 = 1;
fn configured() -> jig_domain_accounts::Providers {
    let mut offered = provider(PROVIDER, true, true);
    offered.offers.programs = Box::new([b"claude_code".as_slice().into()]);
    providers(vec![offered])
}
fn fresh() -> World {
    let mut world = World::with(configured(), &[]);
    world.step(Event::Want);
    world
}
fn declare(
    world: &mut World,
    holder: Holder,
    provider: u32,
    join: Option<u32>,
    name: &[u8],
) -> (Result<u32, programs::DeclarationRefusal>, Vec<Request>) {
    let mut out = Queue::with_capacity(jig_domain_accounts::max_out(&LIMITS));
    let env = Env { now: world.now, wall: world.wall, limits: LIMITS };
    let result = programs::declare(&mut world.domain, &env, holder, provider, join, name.into(), &mut out);
    let mut requests = Vec::new();
    while let Some(request) = out.pop() {
        requests.push(request);
    }
    (result, requests)
}
fn issued(number: u32, name: &[u8]) -> Stored {
    Stored::Issued { number, holder: HOLDER, name: name.into(), signed_out: false }
}
#[test]
fn a_declaration_saves_its_reservation_atomically() {
    let mut world = fresh();
    assert_eq!(
        declare(&mut world, HOLDER, PROVIDER, None, b"personal"),
        (
            Ok(1),
            vec![
                Request::Save { record: account(0, PROVIDER, HOLDER, b"") },
                Request::Save { record: issued(1, b"personal") },
                Request::Save {
                    record: Stored::Source {
                        account: 0,
                        number: 1,
                        place: 0,
                        kind: Kind::Program,
                        name: b"personal".as_slice().into(),
                        held: None
                    }
                },
                Request::Save { record: Stored::Next { number: 2 } },
                Request::Programs {
                    names: Box::new([jig_domain_accounts::Program {
                        name: jig_domain_accounts::SourceName::new(b"personal".as_slice())
                            .expect("admitted fixture name"),
                        cleanup: false
                    }])
                },
                Request::Granted { grant: named_grant(1, 2) },
            ]
        )
    );
    let account = world.domain.account(0).expect("kept account");
    assert!(account.plan());
    assert!(account.subject().is_empty());
}
#[test]
fn a_program_joins_at_the_last_source_place() {
    let mut world = World::with(configured(), &[account(20, PROVIDER, HOLDER, b""), source(20, 21, 0, Kind::Key)]);
    world.step(Event::Want);
    assert_eq!(declare(&mut world, HOLDER, PROVIDER, Some(20), b"personal").0, Ok(22));
    assert_eq!(
        world.domain.account(20).expect("kept account").sources().iter().map(|(p, s)| (*p, *s)).collect::<Vec<_>>(),
        vec![(0, 21), (1, 22)]
    );
}
#[test]
fn a_foreign_holder_cannot_join_an_account() {
    let mut world = World::with(configured(), &[account(20, PROVIDER, OTHER, b""), source(20, 21, 0, Kind::Key)]);
    world.step(Event::Want);
    assert_eq!(
        declare(&mut world, HOLDER, PROVIDER, Some(20), b"personal"),
        (Err(programs::DeclarationRefusal::Account(Refusal::Unknown)), vec![])
    );
    assert_eq!(world.domain.next_number(), 22);
}
#[test]
fn a_provider_must_offer_a_program() {
    let mut world = World::with(providers(vec![provider(PROVIDER, true, true)]), &[]);
    world.step(Event::Want);
    assert_eq!(
        declare(&mut world, HOLDER, PROVIDER, None, b"personal"),
        (Err(programs::DeclarationRefusal::Account(Refusal::Provider { provider: PROVIDER })), vec![])
    );
}
#[test]
fn invalid_names_allocate_nothing() {
    for name in [b"".as_slice(), b"Upper", b"a_b", &[b'a'; 33]] {
        let mut world = fresh();
        assert_eq!(
            declare(&mut world, HOLDER, PROVIDER, None, name),
            (Err(programs::DeclarationRefusal::Name), vec![])
        );
        assert_eq!(world.domain.next_number(), 0);
    }
}
#[test]
fn names_accept_the_exact_length_bound() {
    let mut world = fresh();
    assert_eq!(declare(&mut world, HOLDER, PROVIDER, None, &[b'a'; 32]).0, Ok(1));
}
#[test]
fn a_live_name_cannot_be_declared_twice() {
    let mut world = fresh();
    assert_eq!(declare(&mut world, HOLDER, PROVIDER, None, b"personal").0, Ok(1));
    assert_eq!(
        declare(&mut world, OTHER, PROVIDER, None, b"personal"),
        (Err(programs::DeclarationRefusal::Issued), vec![])
    );
    assert_eq!(world.domain.next_number(), 2);
}
#[test]
fn retired_names_survive_restart() {
    let mut world = World::with(configured(), &[issued(50_000, b"retired")]);
    world.step(Event::Want);
    assert_eq!(
        declare(&mut world, HOLDER, PROVIDER, None, b"retired"),
        (Err(programs::DeclarationRefusal::Issued), vec![])
    );
    assert_eq!(declare(&mut world, OTHER, PROVIDER, None, b"fresh").0, Ok(50_002));
}
#[test]
fn the_lifetime_name_bound_refuses_before_allocating() {
    let rows = (0..LIMITS.program_names).map(|n| issued(n, format!("old-{n}").as_bytes())).collect::<Vec<_>>();
    let mut world = World::with(configured(), &rows);
    world.step(Event::Want);
    assert_eq!(
        declare(&mut world, HOLDER, PROVIDER, None, b"fresh"),
        (Err(programs::DeclarationRefusal::Names), vec![])
    );
    assert_eq!(world.domain.accounts(), 0);
    assert_eq!(world.domain.next_number(), LIMITS.program_names);
}
#[test]
fn a_restored_source_must_match_its_reserved_holder() {
    let mut program = source(20, 21, 0, Kind::Program);
    if let Stored::Source { name, .. } = &mut program {
        *name = b"personal".as_slice().into();
    }
    let mut world = World::with(configured(), &[account(20, PROVIDER, OTHER, b""), program, issued(21, b"personal")]);
    world.step(Event::Want);
    assert!(world.domain.source(21).is_none());
    assert_eq!(world.domain.unrestored(), 2);
    assert_eq!(declare(&mut world, HOLDER, PROVIDER, None, b"personal").0, Err(programs::DeclarationRefusal::Issued));
}

#[test]
fn restoring_a_program_reserves_its_existing_directory_name() {
    let mut program = source(20, 21, 0, Kind::Program);
    if let Stored::Source { name, .. } = &mut program {
        *name = b"personal".as_slice().into();
    }
    let mut world = World::with(configured(), &[account(20, PROVIDER, HOLDER, b""), program]);
    assert!(world.step(Event::Want).contains(&Request::Save { record: issued(21, b"personal") }));
    assert!(world.domain.source(21).is_some());
    assert_eq!(
        declare(&mut world, OTHER, PROVIDER, None, b"personal"),
        (Err(programs::DeclarationRefusal::Issued), vec![])
    );
}

fn sign_out(world: &mut World, holder: Holder) -> Vec<Request> {
    world.step(Event::SignOut { request: skein_lib::Token::new(91), holder, source: 1 })
}
fn targets(world: &mut World, workers: &[u32]) -> Vec<Request> {
    world.step(Event::ProgramTargets {
        source: 1,
        hosts: workers.iter().map(|number| jig_accounts_world::worker(*number)).collect(),
    })
}
fn ended(world: &mut World, worker: u32, result: jig_domain_accounts::ProgramResult) -> Vec<Request> {
    world.step(Event::ProgramSignedOut { source: 1, host: jig_accounts_world::worker(worker), result })
}
#[test]
fn signing_out_a_program_retains_its_declaration() {
    let mut world = fresh();
    assert_eq!(declare(&mut world, HOLDER, PROVIDER, None, b"personal").0, Ok(1));
    sign_out(&mut world, HOLDER);
    assert_eq!(
        targets(&mut world, &[]),
        vec![Request::Answer {
            request: skein_lib::Token::new(91),
            answer: jig_domain_accounts::Answer::SignedOut { confirmed: true, failed_hosts: Box::new([]) },
        }]
    );
    assert!(matches!(world.domain.account(0), Some(account) if account.sources().len() == 1));
}
#[test]
fn a_foreign_program_sign_out_emits_no_host_command() {
    let mut world = fresh();
    assert_eq!(declare(&mut world, HOLDER, PROVIDER, None, b"personal").0, Ok(1));
    assert_eq!(
        sign_out(&mut world, OTHER),
        vec![Request::Answer {
            request: skein_lib::Token::new(91),
            answer: jig_domain_accounts::Answer::Refused { refusal: Refusal::Unknown }
        }]
    );
}
#[test]
fn a_program_answer_names_every_unconfirmed_configured_position() {
    use jig_domain_accounts::{ProgramHost, ProgramResult};
    let mut world = fresh();
    assert_eq!(declare(&mut world, HOLDER, PROVIDER, None, b"personal").0, Ok(1));
    sign_out(&mut world, HOLDER);
    assert!(targets(&mut world, &[0, 1, 2]).is_empty());
    assert!(ended(&mut world, 1, ProgramResult::SignedOut).is_empty());
    assert!(ended(&mut world, 0, ProgramResult::NotInContact).is_empty());
    assert_eq!(
        ended(&mut world, 2, ProgramResult::Lost),
        vec![Request::Answer {
            request: skein_lib::Token::new(91),
            answer: jig_domain_accounts::Answer::SignedOut {
                confirmed: false,
                failed_hosts: Box::new([
                    ProgramHost {
                        worker: jig_accounts_world::worker(0),
                        result: jig_domain_accounts::ProgramFailure::NotInContact
                    },
                    ProgramHost {
                        worker: jig_accounts_world::worker(2),
                        result: jig_domain_accounts::ProgramFailure::Lost
                    },
                ]),
            },
        }]
    );
}
#[test]
fn a_late_program_terminal_cannot_answer_twice() {
    let mut world = fresh();
    assert_eq!(declare(&mut world, HOLDER, PROVIDER, None, b"personal").0, Ok(1));
    sign_out(&mut world, HOLDER);
    targets(&mut world, &[0]);
    ended(&mut world, 0, jig_domain_accounts::ProgramResult::Lost);
    assert!(ended(&mut world, 0, jig_domain_accounts::ProgramResult::SignedOut).is_empty());
}
#[test]
fn restart_preserves_desired_program_absence() {
    let mut world = fresh();
    let (_, declared) = declare(&mut world, HOLDER, PROVIDER, None, b"personal");
    let mut records: Vec<_> = declared
        .into_iter()
        .filter_map(|request| match request {
            Request::Save { record } => Some(record),
            Request::Programs { .. }
            | Request::CheckProgram { .. }
            | Request::SignOutProgram { .. }
            | Request::Component(_)
            | Request::Signing(_)
            | Request::Answer { .. }
            | Request::Granted { .. }
            | Request::Erase { .. }
            | Request::Closed => None,
        })
        .collect();
    for request in sign_out(&mut world, HOLDER) {
        if let Request::Save { record } = request {
            records.retain(|old| old.key() != record.key());
            records.push(record);
        }
    }
    records.sort_by_key(|row| match row {
        Stored::Issued { .. } => 0,
        Stored::Account { .. } => 1,
        Stored::Source { .. } => 2,
        Stored::Next { .. } => 3,
    });
    let mut again = World::with(configured(), &records);
    assert!(again.step(Event::Want).iter().any(|request| matches!(request,
        Request::Programs { names } if names.len() == 1 && names[0].cleanup)));
}
#[test]
fn an_explicit_holder_check_permits_signing_in_again() {
    let mut world = fresh();
    assert_eq!(declare(&mut world, HOLDER, PROVIDER, None, b"personal").0, Ok(1));
    sign_out(&mut world, HOLDER);
    targets(&mut world, &[]);
    let mut out = Queue::with_capacity(jig_domain_accounts::max_out(&LIMITS));
    assert_eq!(programs::check(&mut world.domain, HOLDER, 1, &mut out), Ok(()));
    let mut requests = Vec::new();
    while let Some(request) = out.pop() {
        requests.push(request);
    }
    assert!(
        requests
            .iter()
            .any(|request| matches!(request, Request::Save { record: Stored::Issued { signed_out: false, .. } }))
    );
}
#[test]
fn removing_a_program_account_retains_cleanup_evidence() {
    let mut world = fresh();
    assert_eq!(declare(&mut world, HOLDER, PROVIDER, None, b"personal").0, Ok(1));
    let requests = world.step(Event::Change {
        request: skein_lib::Token::new(92),
        holder: HOLDER,
        change: jig_domain_accounts::Change::Remove { account: 0 },
    });
    assert!(world.domain.account(0).is_none());
    assert!(requests.iter().any(|request| matches!(request, Request::Programs { names } if names[0].cleanup)));
}

#[test]
fn a_change_answers_a_program_declaration() {
    let mut world = fresh();
    let request = skein_lib::Token::new(41);
    let rows = world.step(Event::Change {
        request,
        holder: HOLDER,
        change: jig_domain_accounts::Change::DeclareProgram {
            provider: PROVIDER,
            join: None,
            name: b"personal".as_slice().into(),
        },
    });
    assert_eq!(
        rows.last(),
        Some(&Request::Answer {
            request,
            answer: jig_domain_accounts::Answer::ProgramDeclared { source: 1, name: b"personal".as_slice().into() }
        })
    );
}
#[test]
fn a_foreign_check_emits_no_host_command() {
    let mut world = fresh();
    let (source, _) = declare(&mut world, HOLDER, PROVIDER, None, b"personal");
    let source = source.expect("declared source");
    let request = skein_lib::Token::new(42);
    assert_eq!(
        world.step(Event::Change {
            request,
            holder: OTHER,
            change: jig_domain_accounts::Change::CheckProgram { source }
        }),
        vec![Request::Answer { request, answer: jig_domain_accounts::Answer::Refused { refusal: Refusal::Unknown } }]
    );
}
#[test]
fn a_holder_check_is_answered_after_dispatch() {
    let mut world = fresh();
    let (source, _) = declare(&mut world, HOLDER, PROVIDER, None, b"personal");
    let source = source.expect("declared source");
    let request = skein_lib::Token::new(43);
    let rows = world.step(Event::Change {
        request,
        holder: HOLDER,
        change: jig_domain_accounts::Change::CheckProgram { source },
    });
    assert_eq!(
        rows[rows.len() - 2..],
        [
            Request::CheckProgram {
                name: jig_domain_accounts::SourceName::new(b"personal".as_slice()).expect("admitted fixture name")
            },
            Request::Answer { request, answer: jig_domain_accounts::Answer::Done }
        ]
    );
}

#[test]
fn an_early_terminal_waits_for_its_target_snapshot() {
    let mut world = fresh();
    declare(&mut world, HOLDER, PROVIDER, None, b"personal").0.expect("declaration");
    sign_out(&mut world, HOLDER);
    assert!(ended(&mut world, 0, jig_domain_accounts::ProgramResult::SignedOut).is_empty());
    assert_eq!(
        targets(&mut world, &[0]),
        vec![Request::Answer {
            request: skein_lib::Token::new(91),
            answer: jig_domain_accounts::Answer::SignedOut { confirmed: true, failed_hosts: Box::new([]) }
        }]
    );
}
#[test]
fn an_early_failure_survives_the_target_snapshot() {
    let mut world = fresh();
    declare(&mut world, HOLDER, PROVIDER, None, b"personal").0.expect("declaration");
    sign_out(&mut world, HOLDER);
    ended(&mut world, 0, jig_domain_accounts::ProgramResult::Failed);
    assert_eq!(
        targets(&mut world, &[0]),
        vec![Request::Answer {
            request: skein_lib::Token::new(91),
            answer: jig_domain_accounts::Answer::SignedOut {
                confirmed: false,
                failed_hosts: Box::new([jig_domain_accounts::ProgramHost {
                    worker: jig_accounts_world::worker(0),
                    result: jig_domain_accounts::ProgramFailure::Failed
                }])
            }
        }]
    );
}
#[test]
fn closing_answers_a_pending_program_sign_out() {
    let mut world = fresh();
    declare(&mut world, HOLDER, PROVIDER, None, b"personal").0.expect("declaration");
    sign_out(&mut world, HOLDER);
    let rows = world.step(Event::Close);
    let answers: Vec<_> = rows.into_iter().filter(|row| matches!(row, Request::Answer { .. })).collect();
    assert_eq!(
        answers,
        vec![Request::Answer {
            request: skein_lib::Token::new(91),
            answer: jig_domain_accounts::Answer::Refused { refusal: Refusal::Closing }
        }]
    );
}
#[test]
fn a_pending_program_sign_out_blocks_account_removal() {
    let mut world = fresh();
    declare(&mut world, HOLDER, PROVIDER, None, b"personal").0.expect("declaration");
    sign_out(&mut world, HOLDER);
    let request = skein_lib::Token::new(92);
    assert_eq!(
        world.step(Event::Change {
            request,
            holder: HOLDER,
            change: jig_domain_accounts::Change::Remove { account: 0 }
        }),
        vec![Request::Answer {
            request,
            answer: jig_domain_accounts::Answer::Refused {
                refusal: Refusal::Limit { bound: jig_domain_accounts::Bound::Sources, number: None }
            }
        }]
    );
}

#[test]
fn an_accepted_program_name_is_a_fleet_source_name() {
    let mut world = fresh();
    let name = b"a-0123456789";
    let (result, rows) = declare(&mut world, HOLDER, PROVIDER, None, name);
    assert!(result.is_ok());
    let Some(Request::Programs { names }) = rows.iter().find(|row| matches!(row, Request::Programs { .. })) else {
        panic!("published names")
    };
    assert_eq!(Some(names[0].name), jig_domain_workers::SourceName::new(name));
}
#[test]
fn a_refused_program_name_is_not_a_fleet_source_name() {
    let mut world = fresh();
    let name = b"not portable";
    assert_eq!(declare(&mut world, HOLDER, PROVIDER, None, name), (Err(programs::DeclarationRefusal::Name), vec![]));
    assert_eq!(jig_domain_workers::SourceName::new(name), None);
}

#[test]
fn a_pending_program_sign_out_blocks_a_holder_check() {
    let mut world = fresh();
    declare(&mut world, HOLDER, PROVIDER, None, b"personal").0.expect("declaration");
    sign_out(&mut world, HOLDER);
    let mut out = Queue::with_capacity(jig_domain_accounts::max_out(&LIMITS));
    assert_eq!(
        programs::check(&mut world.domain, HOLDER, 1, &mut out),
        Err(Refusal::Limit { bound: jig_domain_accounts::Bound::Sources, number: None })
    );
    assert!(out.is_empty());
}
#[test]
fn an_early_program_terminal_finishes_account_removal() {
    let mut world = fresh();
    declare(&mut world, HOLDER, PROVIDER, None, b"personal").0.expect("declaration");
    let request = skein_lib::Token::new(92);
    world.step(Event::Change { request, holder: HOLDER, change: jig_domain_accounts::Change::Remove { account: 0 } });
    ended(&mut world, 0, jig_domain_accounts::ProgramResult::SignedOut);
    assert_eq!(
        targets(&mut world, &[0]),
        vec![Request::Answer {
            request,
            answer: jig_domain_accounts::Answer::Removed { confirmed: true, failed_hosts: Box::new([]) }
        }]
    );
}

fn removal_failures(
    first: jig_domain_accounts::ProgramResult,
    last: jig_domain_accounts::ProgramResult,
) -> Vec<Request> {
    let mut world = fresh();
    declare(&mut world, HOLDER, PROVIDER, None, b"personal").0.expect("first declaration");
    declare(&mut world, HOLDER, PROVIDER, Some(0), b"work").0.expect("second declaration");
    let request = skein_lib::Token::new(92);
    world.step(Event::Change { request, holder: HOLDER, change: jig_domain_accounts::Change::Remove { account: 0 } });
    for source in [1, 2] {
        world.step(Event::ProgramTargets { source, hosts: Box::new([jig_accounts_world::worker(0)]) });
    }
    assert!(
        world
            .step(Event::ProgramSignedOut { source: 1, host: jig_accounts_world::worker(0), result: first })
            .is_empty()
    );
    world.step(Event::ProgramSignedOut { source: 2, host: jig_accounts_world::worker(0), result: last })
}
fn removal_answer(result: jig_domain_accounts::ProgramFailure) -> Vec<Request> {
    vec![Request::Answer {
        request: skein_lib::Token::new(92),
        answer: jig_domain_accounts::Answer::Removed {
            confirmed: false,
            failed_hosts: Box::new([jig_domain_accounts::ProgramHost {
                worker: jig_accounts_world::worker(0),
                result,
            }]),
        },
    }]
}
#[test]
fn account_removal_keeps_the_most_severe_worker_failure() {
    use jig_domain_accounts::{ProgramFailure as Failure, ProgramResult as Result};
    for (first, last) in [(Result::Lost, Result::Failed), (Result::Failed, Result::Lost)] {
        assert_eq!(removal_failures(first, last), removal_answer(Failure::Lost));
    }
}
#[test]
fn account_removal_prefers_failure_to_absent_contact() {
    use jig_domain_accounts::{ProgramFailure as Failure, ProgramResult as Result};
    for (first, last) in [(Result::NotInContact, Result::Failed), (Result::Failed, Result::NotInContact)] {
        assert_eq!(removal_failures(first, last), removal_answer(Failure::Failed));
    }
}

#[test]
fn an_issued_name_answers_its_source_holder() {
    for signed_out in [false, true] {
        let world = World::with(
            configured(),
            &[Stored::Issued { number: 21, holder: HOLDER, name: b"personal".as_slice().into(), signed_out }],
        );
        assert_eq!(programs::issued(&world.domain, b"personal"), Some((21, HOLDER)));
    }
}

fn named_grant(source: u32, generation: u64) -> jig_domain_accounts::Grant {
    jig_domain_accounts::Grant {
        form: jig_domain_accounts::Form::Program(
            jig_domain_accounts::SourceName::new(b"personal").expect("source name"),
        ),
        account: source,
        generation,
        valid: jig_domain_accounts::PROGRAM_VALID,
    }
}
#[test]
fn a_usable_program_source_is_granted_by_its_name() {
    let mut world = fresh();
    let (source, requests) = declare(&mut world, HOLDER, PROVIDER, None, b"personal");
    let source = source.expect("declared source");
    assert!(requests.contains(&Request::Granted { grant: named_grant(source, 2) }));
}
#[test]
fn a_program_grant_never_lapses() {
    let mut world = fresh();
    let (source, _) = declare(&mut world, HOLDER, PROVIDER, None, b"personal");
    let source = source.expect("declared source");
    assert_eq!(world.domain.source_grant(source, skein_lib::Time::from_nanos(u64::MAX)), Some(named_grant(source, 2)));
    assert_eq!(world.domain.next_deadline(), None);
}
#[test]
fn a_new_login_raises_the_program_grant_s_generation() {
    let mut world = fresh();
    let (source, _) = declare(&mut world, HOLDER, PROVIDER, None, b"personal");
    let source = source.expect("declared source");
    sign_out(&mut world, HOLDER);
    world.step(Event::ProgramTargets { source, hosts: Box::new([]) });
    let requests = world.step(Event::Change {
        request: skein_lib::Token::new(44),
        holder: HOLDER,
        change: jig_domain_accounts::Change::CheckProgram { source },
    });
    assert!(requests.contains(&Request::Save { record: Stored::Next { number: 3 } }));
    assert!(requests.contains(&Request::Granted { grant: named_grant(source, 3) }));
}
#[test]
fn a_restored_program_grant_uses_the_durable_counter_epoch() {
    for held in [None, Some(jig_domain_accounts::Held::Refused)] {
        let program = Stored::Source {
            account: 0,
            number: 1,
            place: 0,
            kind: Kind::Program,
            name: b"personal".as_slice().into(),
            held,
        };
        let mut world = World::with(
            configured(),
            &[account(0, PROVIDER, HOLDER, b""), program, issued(1, b"personal"), Stored::Next { number: 3 }],
        );
        let requests = world.step(Event::Want);
        assert_eq!(requests.contains(&Request::Granted { grant: named_grant(1, 3) }), held.is_none());
        assert!(!requests.iter().any(|request| matches!(request, Request::Save { record: Stored::Source { .. } })));
    }
}

#[test]
fn signing_out_withdraws_the_program_grant() {
    let mut world = fresh();
    declare(&mut world, HOLDER, PROVIDER, None, b"personal").0.expect("source");
    sign_out(&mut world, HOLDER);
    assert_eq!(world.domain.source_grant(1, world.now), None);
    assert_eq!(
        world.domain.select(
            jig_domain_accounts::Use::Party(7),
            PROVIDER,
            Some(b"claude_code"),
            Some(0),
            None,
            &[jig_domain_accounts::Coverage { account: 0, serves: true }],
            &[jig_domain_accounts::HostSource { source: 1, usable: true }],
            world.now,
        ),
        jig_domain_accounts::Selection::Wait { account: 0, source: Some(1), why: jig_domain_accounts::WaitWhy::Host }
    );
}
#[test]
fn a_login_check_with_an_exhausted_epoch_changes_nothing() {
    // Restoring the counter before Want gives the greatest representable epoch.
    let rows = [
        account(0, PROVIDER, HOLDER, b""),
        Stored::Source {
            account: 0,
            number: 1,
            place: 0,
            kind: Kind::Program,
            name: b"personal".as_slice().into(),
            held: None,
        },
        issued(1, b"personal"),
        Stored::Next { number: u32::MAX },
    ];
    let mut world = World::with(configured(), &rows);
    world.step(Event::Want);
    let mut out = Queue::with_capacity(jig_domain_accounts::max_out(&LIMITS));
    assert_eq!(programs::check(&mut world.domain, HOLDER, 1, &mut out), Err(Refusal::Numbers));
    assert!(out.is_empty());
    assert_eq!(world.domain.source_grant(1, world.now), Some(named_grant(1, u64::from(u32::MAX))));
}
