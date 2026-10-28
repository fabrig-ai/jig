//! Account moves through the public client against the real root (testing.md, 5).
use super::{engine::Driver, fixture::clock, session::Session};
use crate::harness::ClientId;
use jig_client::{self as client, contract as c};
use jig_core_world::peers::Peers;
use jig_domain_people as people;
use jig_test_domain as root;
use skein_lib::{Duration, List, Queue, Secret, Time};

const OWNER: ClientId = ClientId(0);
const MOVE_DEADLINE: Duration = Duration::from_secs(1);
const WATCH: client::Handle = client::Handle { asker: client::Asker::JIG, number: 91 };
fn key(number: u64) -> client::Key {
    client::Key { asker: client::Asker::JIG, number }
}
fn configuration() -> root::Config {
    let mut config = jig_core_world::world::config(8510);
    config.core.settings.llm_providers.providers[0].offers.sign_in = true;
    config
}
struct Story {
    session: Session,
    client: client::Client,
    out: Queue<client::Request>,
    now: Time,
    told: Vec<client::Told>,
    sent: Vec<[u8; 16]>,
    account: u32,
}
impl Story {
    fn new() -> Self {
        let mut owner_limits = jig_core_world::world::limits();
        owner_limits.core.accounts.accounts = 2;
        owner_limits.core.accounts.move_deadline = MOVE_DEADLINE;
        jig_core_world::peers::journal(&mut owner_limits);
        let now = Time::ZERO;
        let driver = Driver::new(configuration(), owner_limits, Peers::new(Vec::new(), Vec::new()), now, clock(now));
        let mut limits = crate::test::limits();
        limits.client.domain.text = 4096;
        limits.client.domain.turn_text = 4096;
        limits.client.domain.turn_bytes = 4096;
        limits.client.domain.rows = 32;
        limits.client.domain.projects = 64;
        limits.client.domain.document_bytes = 1_048_576;
        let codec = jig_protocol_transcript::Limits {
            smith: smith_transcript::v6::CEILINGS,
            bytes: 1_048_576,
            turn: c::TurnLimits { messages: 64, blocks: 16, text: 65_536 },
        };
        let mut story = Self {
            session: Session::new(driver, limits, codec),
            client: client::Client::unlinked(&limits.client, 8510).expect("bounded client"),
            out: Queue::uncapped(),
            now,
            told: Vec::new(),
            sent: Vec::new(),
            account: 0,
        };
        story.session.stream_heartbeat(Duration::from_millis(50));
        story.session.bind(
            OWNER,
            1,
            people::Identity {
                key: people::IdentityKey { provider: 0, subject: 7_u64.to_be_bytes().into() },
                login: Box::new([]),
                name: Box::new([]),
            },
            now,
            clock(now),
        );
        story.event(client::Event::StartUnlinked { kept: List::with_capacity(0) });
        let provider = story.session.driver.transport.providers.providers[0].number;
        story.event(client::Event::Send {
            key: key(1),
            name: Some([1; 16]),
            request: client::Ask::Jig(Box::new(c::Ask::SignIn { holder: c::AccountHolder::Own, provider })),
        });
        story.event(client::Event::Watch { handle: WATCH, subject: client::Watchable::Jig(c::Subject::Accounts) });
        story.account = story.document().accounts.get(0).expect("signed-in account").number;
        story.told.clear();
        story.sent.clear();
        story
    }
    fn event(&mut self, event: client::Event) {
        self.client.step(self.now, clock(self.now), event, &mut self.out);
        self.pump();
    }
    fn pump(&mut self) {
        for _ in 0..64 {
            while let Some(request) = self.out.pop() {
                match request {
                    client::Request::Down { down: client::Down::Save { record } } => {
                        if let client::Ask::Jig(ask) = &record.request.request {
                            assert!(
                                !matches!(
                                    ask.as_ref(),
                                    c::Ask::AccountChange {
                                        change: c::AccountChange::MoveOut { .. } | c::AccountChange::MoveIn { .. },
                                        ..
                                    }
                                ),
                                "a move and its record never reach Save"
                            );
                        }
                        self.client.step(self.now, clock(self.now), client::Event::Stored, &mut self.out);
                    }
                    client::Request::Down { down: client::Down::Erase { .. } } => {
                        self.client.step(self.now, clock(self.now), client::Event::Stored, &mut self.out);
                    }
                    client::Request::Down { down } => {
                        if let client::Down::Send { key, .. } = &down {
                            self.sent.push(*key);
                        }
                        self.session
                            .send(
                                OWNER,
                                self.now,
                                clock(self.now),
                                jig_client_test_domain::Request::Client { request: client::Request::Down { down } },
                            )
                            .expect("real joint request");
                    }
                    client::Request::Told { told } => self.told.push(told),
                    client::Request::Exchange { .. }
                    | client::Request::OpenStream { .. }
                    | client::Request::CloseStream
                    | client::Request::Save { .. }
                    | client::Request::Erase { .. }
                    | client::Request::SignOut
                    | client::Request::Authorized(_)
                    | client::Request::ServiceCredential { .. } => panic!("unlinked client doors only"),
                }
            }
            let callbacks = self.session.poll(self.now, clock(self.now)).expect("real root replies");
            if callbacks.is_empty() {
                return;
            }
            for (owner, event) in callbacks {
                assert_eq!(owner, OWNER);
                self.session.received(owner, self.now, clock(self.now), &event);
                let jig_client_test_domain::Event::Client { event } = event else { panic!("client callback") };
                self.client.step(self.now, clock(self.now), event, &mut self.out);
            }
        }
        panic!("joint session did not settle");
    }
    fn document(&self) -> &c::OwnerAccounts {
        let Some(client::Latest::Jig(c::Delivery::OwnerAccounts(document))) =
            self.client.document(WATCH).and_then(|d| d.latest)
        else {
            panic!("accounts document")
        };
        document
    }
    fn outcome(&mut self, number: u64) -> client::Outcome {
        let at = self
            .told
            .iter()
            .position(|t| matches!(t, client::Told::Answered { key: k, .. } if *k == key(number)))
            .expect("answered caller");
        let client::Told::Answered { outcome, .. } = self.told.remove(at) else { unreachable!() };
        outcome
    }
    fn moving(&mut self) -> (u32, Secret) {
        self.event(client::accounts::move_out(key(2), [2; 16], c::AccountHolder::Own, self.account));
        let client::Outcome::Answered(client::Answered::Jig(c::Answer::Done(c::Outcome::Made {
            what: c::Made::MovingOut { account, number, record },
        }))) = self.outcome(2)
        else {
            panic!("moving out answered with its value")
        };
        assert_eq!(account, self.account);
        assert!(!record.is_empty());
        assert_eq!(self.sent, [[2; 16]], "one send of the move");
        (number, record)
    }
    fn written(&mut self, number: u32) -> client::Outcome {
        self.event(client::accounts::written(key(3), [3; 16], c::AccountHolder::Own, number));
        self.outcome(3)
    }
    fn advance(&mut self, span: Duration) {
        self.now = self.now.saturating_add(span);
        self.pump();
        self.client.fire(self.now, clock(self.now), &mut self.out);
        self.pump();
    }
}
fn done() -> client::Outcome {
    client::Outcome::Answered(client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(
        c::OwnerOutcome::AccountChanged,
    ))))
}
fn cancelled() -> client::Outcome {
    client::Outcome::Answered(client::Answered::Jig(c::Answer::Refused {
        at: None,
        refusal: c::Refusal::AccountMove { why: c::AccountMoveWhy::Cancelled },
    }))
}
#[test]
fn moved_out_is_moving_until_written_then_gone() {
    let mut story = Story::new();
    let (number, _) = story.moving();
    let source = story.document().accounts.get(0).expect("own account").sources.get(0).expect("signed-in source");
    assert_eq!((source.state, source.usable), (c::OwnerAccountState::Moving, false));
    assert_eq!(story.written(number), done());
    assert!(story.document().accounts.is_empty());
}
#[test]
fn the_record_moves_in_and_the_account_returns() {
    let mut story = Story::new();
    let (number, record) = story.moving();
    assert_eq!(story.written(number), done());
    story.event(client::accounts::move_in(key(4), [4; 16], c::AccountHolder::Own, record));
    let client::Outcome::Answered(client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(
        c::OwnerOutcome::AccountMovedIn { account, renewed: false },
    )))) = story.outcome(4)
    else {
        panic!("moved in")
    };
    assert_eq!(story.document().accounts.get(0).expect("own account").number, account);
    assert_eq!(
        story.document().accounts.get(0).expect("own account").sources.get(0).expect("signed-in source").state,
        c::OwnerAccountState::Granted
    );
    story.advance(Duration::from_secs(1));
    assert_eq!(story.sent.iter().filter(|key| **key == [4; 16]).count(), 1);
}
#[test]
fn the_move_out_name_sent_again_is_lost_without_a_record() {
    let mut story = Story::new();
    story.moving();
    story.event(client::accounts::move_out(key(4), [2; 16], c::AccountHolder::Own, story.account));
    assert_eq!(story.outcome(4), client::Outcome::Lost);
    story.advance(Duration::from_secs(1));
    assert_eq!(story.sent, [[2; 16], [2; 16]], "one send per explicit caller request");
}
#[test]
fn late_written_is_cancelled_and_the_source_is_refreshed() {
    let mut story = Story::new();
    let (number, _) = story.moving();
    let deadline = MOVE_DEADLINE;
    story.advance(Duration::from_nanos(deadline.as_nanos().checked_add(1).expect("past move deadline")));
    assert_eq!(story.written(number), cancelled());
    assert_eq!(
        story.document().accounts.get(0).expect("own account").sources.get(0).expect("signed-in source").state,
        c::OwnerAccountState::Granted
    );
    assert_eq!(story.sent.iter().filter(|key| **key == [2; 16]).count(), 1);
}
#[test]
fn a_cold_engine_restart_cancels_the_move() {
    let mut story = Story::new();
    let (number, _) = story.moving();
    let until = story.now.saturating_add(Duration::from_millis(50));
    story.session.restart(configuration(), story.now, clock(story.now), until).expect("cold restart");
    story.advance(Duration::from_millis(50));
    story.advance(Duration::from_millis(100));
    assert_eq!(
        story.document().accounts.get(0).expect("own account").sources.get(0).expect("signed-in source").state,
        c::OwnerAccountState::Granted
    );
    assert_eq!(story.written(number), cancelled());
    assert_eq!(story.sent.iter().filter(|key| **key == [2; 16]).count(), 1);
}
