//! The public linked client, native client wire, real core and applied store.
use jig_client::{self as client, contract as c};
use jig_client_protocol_wire::v19 as w;
use jig_core_world::{effects::World, fixtures, reads};
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_engine_clients_world::fixtures::{self as wire, Peer};
use jig_test_domain as root;
use skein_lib::{List, Queue, Reader, Secret, Time, Wall, Writer};

fn encoded(value: &w::StreamEvent) -> Box<[u8]> {
    let mut out = Writer::new(value.measure() as usize);
    value.encode(&mut out).expect("stream record");
    out.finish()
}

struct Party {
    machine: client::Client,
    peer: Peer,
    bearer: Box<[u8]>,
    sign_in: u64,
    next: u64,
    out: Queue<client::Request>,
}
impl Party {
    fn step(&mut self, event: client::Event) -> Vec<client::Request> {
        self.machine.step(Time::ZERO, Wall::EPOCH, event, &mut self.out);
        let mut requests = Vec::new();
        while let Some(request) = self.out.pop() {
            if matches!(request, client::Request::Save { .. } | client::Request::Erase { .. }) {
                self.machine.step(Time::ZERO, Wall::EPOCH, client::Event::Stored, &mut self.out);
            } else {
                requests.push(request);
            }
        }
        requests
    }

    fn new(world: &mut World, sign_in: u64, person: u64) -> Self {
        let mut settings = wire::settings();
        settings.models[0].name = b"scripted".as_slice().into();
        let mut peer = Peer::started(&settings);
        let printed = peer.printed_link();
        let exchange = peer.post_link(&printed);
        peer.answer_signed_in(sign_in, person);
        peer.answer_kept(sign_in);
        let token = peer.sign_in_answer(exchange).into_parts().token.expect("credential");
        let mut bearer = Writer::new(61);
        bearer.put(b"Bearer ").expect("prefix");
        skein_lib::base64::encode(skein_lib::base64::Alphabet::Url, token.expose(), &mut bearer).expect("bearer");
        let mut limits = jig_client_world::test::limits().client;
        limits.domain.owners = &[];
        limits.domain.rows = 32;
        limits.domain.projects = 64;
        limits.domain.text = 4096;
        limits.domain.document_bytes = 1_048_576;
        let mut party = Self {
            machine: client::Client::new(&limits, person).expect("client bounds"),
            peer,
            bearer: bearer.finish(),
            sign_in,
            next: 1,
            out: Queue::with_capacity(client::max_out(&limits)),
        };
        party.step(client::Event::Start { kept: List::with_capacity(0) });
        party.step(client::Event::SignIn { with: client::With::Held(Secret::new(token.expose().into())) });
        let hello = w::Hello::new(
            &w::CEILINGS,
            w::HelloParts {
                stream: [7; 16],
                served: jig_client_world::test::list([w::Served::new(
                    &w::CEILINGS,
                    w::ServedParts { owner: w::Owner::Jig, first: 19, last: 19 },
                )
                .expect("served")]),
                heartbeat: skein_lib::Duration::from_secs(5),
            },
        )
        .expect("hello");
        let requests = party.step(client::Event::Streamed {
            message: client::Streamed::Record(encoded(&w::StreamEvent::Hello(hello))),
        });
        let watch = requests
            .iter()
            .find_map(|request| match request {
                client::Request::Exchange { message: client::Message::Watch(bytes), .. } => {
                    Some(w::Open::decode(&w::CEILINGS, &mut Reader::new(bytes)).expect("Me watch").watch())
                }
                _ => None,
            })
            .expect("client opens Me");
        world.sign_in = Some(sign_in);
        reads::ask(world, 90, core::reads::Read::Frame);
        let core::reads::Answer::Document(document) = reads::latest(world) else { panic!("real Me") };
        let core::documents::Document::Frame(frame) = document.as_ref() else { panic!("real frame") };
        let me = jig_protocol_engine_clients::translate::owner::frame(frame, &settings.models, &w::CEILINGS)
            .expect("Me codec");
        let value = w::DeliveryValue::Me(me);
        let mut bytes = Writer::new(value.measure() as usize);
        value.encode(&mut bytes).expect("Me delivery");
        let document =
            w::Owned::new(&w::CEILINGS, w::OwnedParts { owner: w::Owner::Jig, version: 19, record: bytes.finish() })
                .expect("owned");
        let delivery = w::Delivery::new(&w::CEILINGS, w::DeliveryParts { watch, document }).expect("delivery");
        party.step(client::Event::Streamed {
            message: client::Streamed::Record(encoded(&w::StreamEvent::Delivery(delivery))),
        });
        assert_eq!(party.machine.link().party, Some(person));
        party
    }

    fn exchange(&mut self, world: &mut World, requests: Vec<client::Request>) -> Vec<client::Request> {
        let (exchange, message) = requests
            .into_iter()
            .find_map(|request| match request {
                client::Request::Exchange { exchange, message, .. } => Some((exchange, message)),
                _ => None,
            })
            .expect("client exchange");
        let (path, bytes): (&[u8], _) = match message {
            client::Message::Request(bytes) => (b"/jig/v1/request", bytes),
            client::Message::Read(bytes) => (b"/jig/v1/read", bytes),
            _ => panic!("request or read"),
        };
        let socket = self.peer.post(path, bytes, Some(&self.bearer));
        let event = self.peer.take_core().expect("production wire reaches core");
        match event {
            core::Event::People(people::Event::Ask { ref reply_to, .. }) => {
                let reply_to = skein_lib::ReplyTo::new(reply_to.clone().into_token());
                world.send(root::Event::Core(event));
                self.peer.answer_request(reply_to, world.people_answers.last().expect("durable reply").clone());
            }
            core::Event::Read { reply, sign_in, read } => {
                assert_eq!(sign_in, self.sign_in);
                world.sign_in = Some(sign_in);
                reads::ask(world, 91, read);
                self.peer.answer_read(reply, reads::latest(world).clone());
            }
            _ => panic!("one authenticated client act"),
        }
        let message = self.peer.answered(socket).expect("encoded durable answer").into();
        self.step(client::Event::Reply { exchange, message })
    }

    fn ask(&mut self, world: &mut World, ask: c::Ask) -> c::Answer {
        let key = client::Key { asker: client::Asker::JIG, number: self.next };
        self.next += 1;
        let requests = self.step(client::Event::Send { key, name: None, request: client::Ask::Jig(Box::new(ask)) });
        self.exchange(world, requests).into_iter().find_map(|request| match request {
            client::Request::Told { told: client::Told::Answered { outcome: client::Outcome::Answered(client::Answered::Jig(answer)), .. } } => Some(answer),
            _ => None,
        }).expect("decoded final answer")
    }

    fn task(&mut self, world: &mut World, task: u64) -> Box<c::Task> {
        let requests = self.step(client::Event::Read {
            handle: client::Handle { asker: client::Asker::JIG, number: 9 },
            read: client::Readable::Jig(c::Read::Task { task }),
        });
        self.exchange(world, requests)
            .into_iter()
            .find_map(|request| match request {
                client::Request::Told {
                    told:
                        client::Told::Page {
                            page: client::Readout::Jig(c::ReadResult::Value(c::ReadValue::Task(task))),
                            ..
                        },
                } => Some(task),
                _ => None,
            })
            .expect("decoded task document")
    }
}

#[test]
fn a_client_reads_a_stopped_goals_hold_and_only_its_owner_lifts_it() {
    let (mut config, mut limits) = jig_core_world::peers::fixture(960, 0);
    config.core.seed_projects[0].policy.policy.roles[0].authority.budget.spend = 100;
    limits.core.fleet.slots = 3;
    limits.core.tasks.inbox_messages = 8;
    limits.core.tasks.inbox_bytes = 512;
    jig_core_world::peers::journal(&mut limits);
    let mut world = World::configured(960, false, config, limits);
    world.reads = Some(reads::Parties::new(world.store.applied));
    let owner_sign_in = world.sign_in.expect("owner");
    let coordinator = world.assignments[0];
    let tasks::Party::Person(owner) = fixtures::live_row(&world, coordinator.0).requester else { panic!("owner") };
    let maintainer_sign_in = fixtures::second_party(&mut world, b"maintainer", people::Role::Maintainer);
    let Some(people::Reply::SignedIn { person: maintainer, .. }) = world.people_answers.last() else {
        panic!("maintainer")
    };
    let maintainer = *maintainer;
    let mut goal = fixtures::member(&world, tasks::Executor::Agent { charter: 1 }, b"goal");
    goal.tracked = Some(1);
    goal.authority.budget.spend = 40;
    goal.authority.delegation.tasks = 2;
    let goal = fixtures::delegate(&mut world, coordinator, 1, goal);
    world.turn(1, 0, None, b"goal turn");
    let run = (goal, fixtures::live_row(&world, goal).attempt);
    let mut child = fixtures::member(&world, tasks::Executor::Agent { charter: 1 }, b"delegate");
    child.authority.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
    let child = fixtures::delegate(&mut world, run, 1, child);
    world.turn(1, 0, None, b"delegate turn");
    let mut owner_client = Party::new(&mut world, owner_sign_in, owner);
    let mut maintainer_client = Party::new(&mut world, maintainer_sign_in, maintainer);
    assert!(matches!(owner_client.ask(&mut world, c::Ask::Stop { project: 1, task: goal }), c::Answer::Done(_)));
    let document = owner_client.task(&mut world, goal);
    let hold = document.hold.as_ref().expect("held");
    let placed = hold.placed().expect("placed hold details");
    let number = placed.number;
    assert_eq!(placed.owner, c::HoldOwner::Party { person: owner });
    assert_eq!(placed.mode, c::HoldMode::Stop);
    assert_eq!(placed.scope, c::HoldScope::Subtree { task: goal });
    assert_eq!(placed.revision, c::Revision { value: 1 });
    assert!(placed.may_lift);
    assert!(!placed.lifted);
    assert!(placed.reason.is_empty());
    assert!(placed.placed_at > Wall::EPOCH);
    assert!(matches!(hold, c::TaskHold::Owner(value) if value.reason == c::HoldCategory::Placed { hold: number }));
    assert!(
        document.acts.iter().any(|offer| offer.act == c::ActKind::Release && offer.admission == c::Admission::Allowed)
    );
    let document = maintainer_client.task(&mut world, goal);
    let hold = document.hold.as_ref().expect("held");
    assert!(!hold.placed().expect("same placed hold").may_lift);
    let why = c::Refusal::Authority {
        lacking: c::Lacking::HoldOwner { hold: number, owner: c::HoldOwner::Party { person: owner } },
        proposable: None,
    };
    assert!(
        document
            .acts
            .iter()
            .any(|offer| offer.act == c::ActKind::Release && offer.admission == c::Admission::Refused(why.clone()))
    );
    assert_eq!(
        maintainer_client
            .ask(&mut world, c::Ask::Release { project: 1, task: goal, revision: hold.revision(), again: None }),
        c::Answer::Refused { refusal: why, at: None }
    );
    assert_eq!(
        owner_client
            .ask(&mut world, c::Ask::Lift { hold: number, revision: c::Revision { value: 1 }, with_tasks: true }),
        c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::Lifted { hold: number, released: 2 }))
    );
    assert!(owner_client.task(&mut world, goal).hold.is_none());
    assert!(owner_client.task(&mut world, child).hold.is_none());
}
