//! The root keeps these orders whatever admits work (skein's draft
//! `dynamic-memory.md`, 4.6): complete application routes, completion arrival
//! order within an output lane, and older held notices before later ones.
use jig_domain_core as core;
use jig_domain_host as host;
use jig_domain_people as people;
use jig_ops_domain as root;
use jig_ops_domain_infrastructure as infrastructure;
use jig_ops_domain_observability as observability;
use jig_ops_world::direct::Driver;
use skein_lib::{ReplyTo, Token};

const SEED: u64 = 9205;
const PROJECT: u32 = 1;
const FIRST: Token = Token::new(960);
const SECOND: Token = Token::new(961);
const TURN: u32 = 1;
const TURN_TEXT: &[u8] = b"kept turn";
// One retained turn and one run terminal, each with its withheld commit.
const RUN_COMMITS: usize = 2;
const ONCE: usize = 1;
const ONE_REFUSAL: u64 = 1;
// One pressure probe plus the deliberately refused second application request.
const APPLICATION_REFUSALS: u64 = 2;
// Initial pressure and one refill after each run commit.
const TURN_REFUSALS: u64 = 3;

fn application(sign_in: u64, reply: Token, key: u8) -> root::Event {
    root::Event::Application {
        reply_to: ReplyTo::new(reply),
        sign_in,
        key: [key; 16],
        project: PROJECT,
        digest: [key; 32],
        value: infrastructure::ApplicationRequest {
            environment: infrastructure::Environment::new(b"pool".as_slice().into(), b"missing".as_slice().into()),
            revision: 0,
            until: 3600,
        },
    }
}

fn answers(driver: &Driver) -> Vec<(Token, &infrastructure::ApplicationOutcome)> {
    driver
        .released
        .iter()
        .filter_map(|output| {
            if let root::Output::Application { to, outcome } = output {
                [FIRST, SECOND].into_iter().find(|token| *to == ReplyTo::new(*token)).map(|token| (token, outcome))
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn an_application_route_finishes_before_the_next_entrance() {
    let mut driver = Driver::started(SEED);
    driver.send(application(driver.sign_in, FIRST, 3));
    driver.fill_to_watermark();
    let returned = driver.returned;
    driver.send(application(driver.sign_in, SECOND, 4));
    assert_eq!(driver.returned, returned + ONE_REFUSAL, "the second operation is handed back under pressure");
    assert!(answers(&driver).is_empty(), "the whole first answer waits for its commit");
    let first_writes = driver
        .transactions
        .iter()
        .filter(|(_, writes)| {
            writes.iter().any(|write| {
                matches!(
                    write,
                    root::Write::Save(root::Record::Infrastructure(infrastructure::Record::ApplicationAnswer { .. }))
                )
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(first_writes.len(), ONCE, "the complete connector route writes in one commit");
    let (_, writes) = first_writes[0];
    assert_eq!(
        writes
            .iter()
            .filter(|write| matches!(write, root::Write::Save(root::Record::Core(
        core::Record::People(people::Stored::Answer { key, .. })
    )) if key.key == [3; 16]))
            .count(),
        ONCE,
        "the caller's retained answer shares the connector transaction"
    );
    driver.settle();
    assert_eq!(
        answers(&driver),
        [(FIRST, &infrastructure::ApplicationOutcome::Unknown)],
        "the first answer leaves whole before the second operation"
    );
    // Phase 1's Event is not Clone, so the refused operation is reconstructed
    // with the same key and payload after drain; phase 2 exercises the same retry.
    driver.send(application(driver.sign_in, SECOND, 4));
    driver.settle();
    assert_eq!(
        answers(&driver),
        [(FIRST, &infrastructure::ApplicationOutcome::Unknown), (SECOND, &infrastructure::ApplicationOutcome::Unknown)]
    );
    assert_eq!(driver.returned, APPLICATION_REFUSALS);
    assert_eq!(
        driver
            .transactions
            .iter()
            .filter(|(_, writes)| writes.iter().any(|write| matches!(
                write,
                root::Write::Save(root::Record::Infrastructure(infrastructure::Record::ApplicationAnswer { .. }))
            )))
            .count(),
        RUN_COMMITS,
        "each admitted operation contributes one whole transaction"
    );
}

fn read(token: Token) -> root::Event {
    root::Event::Observability(observability::Event::Read {
        token,
        read: observability::Read::Logs {
            service: jig_ops_world::service(),
            window: observability::Window { from: 0, through: 1 },
            filter: Box::new([]),
            max_bytes: 128,
        },
    })
}

fn read_done(token: Token, bytes: &[u8]) -> root::Event {
    root::Event::Observability(observability::Event::System(observability::SystemEvent::ReadDone {
        token,
        bytes: bytes.into(),
    }))
}

#[test]
fn read_answers_keep_completion_arrival_order_under_load() {
    let mut driver = Driver::started(SEED);
    driver.send(read(FIRST));
    driver.send(read(SECOND));
    driver.fill_to_watermark();
    let returned = driver.returned;
    driver.send(read_done(SECOND, b"second"));
    driver.send(read_done(FIRST, b"first"));
    assert_eq!(driver.returned, returned, "both read terminals enter at the watermark");
    driver.settle();
    let replies = driver
        .released
        .iter()
        .filter_map(|output| {
            if let root::Output::Read { owner, bytes } = output { Some((*owner, bytes.as_ref())) } else { None }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        replies,
        [(SECOND, b"second".as_slice()), (FIRST, b"first".as_slice())],
        "answers leave once each in completion arrival order"
    );
    assert_eq!(driver.returned, ONE_REFUSAL);
}

#[test]
#[ignore = "passes once the watermark takes completions (phase 2)"]
fn turn_kept_precedes_terminal_acknowledgement_under_load() {
    let mut driver = Driver::turn_story(SEED);
    driver.send(root::Event::Host(host::Event::Turn {
        owner: driver.owner,
        turn: host::Turn {
            turn: TURN,
            spent: 1,
            read: None,
            opens_window: true,
            refused: Box::new([]),
            body: TURN_TEXT.into(),
        },
    }));
    driver.send(root::Event::Agent(host::FromAgent::Finished {
        owner: driver.owner,
        turns: TURN,
        spent: 1,
        read: None,
        finish: host::Finish::Parked,
    }));
    // Finished stops the capability; Gone completes that stop and lets the
    // hub submit its retained terminal, as the real capability would.
    driver.send(root::Event::Agent(host::FromAgent::Gone { owner: driver.owner, detail: Box::new([]) }));
    // The first two submissions are the run commits; pressure adds newer ones.
    assert_eq!(driver.pending.len(), RUN_COMMITS, "the turn and terminal each have a withheld commit");
    driver.fill_to_watermark();
    for _ in 0..RUN_COMMITS {
        driver.commit_one();
        driver.fill_to_watermark();
    }
    let returned = driver.returned;
    while let Some(event) = driver.events.pop_front() {
        driver.send_retaining(event);
        assert_eq!(
            driver.returned, returned,
            "no admitted continuation is handed back; total refusals: {}",
            driver.returned
        );
    }
    driver.settle();
    let turns = driver
        .transactions
        .iter()
        .flat_map(|(_, writes)| writes)
        .filter_map(|write| {
            if let root::Write::Save(root::Record::Core(core::Record::Core(core::CoreRecord::Turn(row)))) = write {
                Some(row)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(turns.len(), ONCE, "one retained turn record");
    assert_eq!(turns[0].transcript.as_ref(), TURN_TEXT);
    let terminals = driver
        .transactions
        .iter()
        .flat_map(|(_, writes)| writes)
        .filter_map(|write| {
            if let root::Write::Save(root::Record::Core(core::Record::Core(core::CoreRecord::Terminal(row)))) = write {
                Some(row)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(terminals.len(), ONCE, "one run terminal record");
    let run = Token::new(turns[0].task);
    let attempt = Token::new(turns[0].attempt);
    assert_eq!(
        driver.notices,
        [host::LinkDown::TurnKept { run, attempt, turn: TURN }, host::LinkDown::Acknowledge { run, attempt }],
        "turn kept precedes terminal acknowledgement, each once"
    );
    assert_eq!(driver.returned, TURN_REFUSALS);
}
