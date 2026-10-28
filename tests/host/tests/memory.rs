//! Memory stays within the worst case (programming-model.md, 6.3), measured by
//! a counting allocator: the host with every slot holding an assignment of
//! exactly its limits, then every run ending with as much as it may hold.

use jig_domain_host::{
    AgentFailure, Ask, Delivery, DeliveryOutcome, Domain, Ending, Event, Finish, Grant, Invalid, Limits, Preparation,
    Reason, Refusal, Request, RunAssignment, Workspace, max_out, resume, step, worst_case,
};
use jig_domain_host::{LinkDown, LinkUp};
use skein_lib::{Duration, Env, Queue, Time, Token, Wall};
use skein_world::domain::heap::{self, Meter};

#[global_allocator]
static HEAP: heap::Counting = heap::Counting;

const LIMITS: Limits = Limits {
    accounts: 4,
    slots: 2,
    program_sources: 12,
    source_interval: skein_lib::Duration::from_secs(60),
    source_timeout: skein_lib::Duration::from_secs(10),
    charter_bytes: 1024,
    transcript_bytes: 528,
    delivery_evidence_bytes: 0,
    turn_bytes: 512,
    outcome_bytes: 768,
    detail_bytes: 128,
    held: 2,
    inbox: 4,
    event_bytes: 256,
    run_calls: 2,
    facts: 16,
    told: 2,
    turns: 0,
    turn_queue_bytes: 0,
    piece_bytes: 4096,
};

fn bytes(len: u64) -> Box<[u8]> {
    vec![b'x'; usize::try_from(len).expect("a test length fits")].into_boxed_slice()
}

/// What a step asked for, without the payload.
#[derive(PartialEq, Eq, Debug)]
enum Asked {
    Prepare { owner: Token },
    Start,
    Delivery { owner: Token },
    Relay { call: Box<[u8]> },
    Save { owner: Token },
    RunEnding { answer: Ending },
    Answer { answer: jig_domain_host::Answer },
    Turn,
    Hosting { runs: usize },
    Other,
}

/// The host under `limits`, measured: each step's peak is checked against
/// the worst case, less what it handed out in requests, which their
/// receivers count.
struct Measured {
    domain: Domain,
    env: Env<Limits>,
    out: Queue<Request>,
    meter: Meter,
    bound: u64,
}

impl Measured {
    fn new(limits: Limits) -> Measured {
        let bound = worst_case(&limits).expect("the test limits fit");
        // The output queue belongs to the parent, not the measured child.
        let out = Queue::with_capacity(max_out(&limits));
        let meter = Meter::new();
        let domain = Domain::new(&limits);
        Measured { domain, env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits }, out, meter, bound }
    }

    fn step(&mut self, event: Event) -> Vec<Asked> {
        self.meter.start();
        step(&mut self.domain, &self.env, event, &mut self.out);
        self.drain()
    }

    fn resume(&mut self) -> Vec<Asked> {
        self.meter.start();
        resume(&mut self.domain, &self.env, &mut self.out);
        self.drain()
    }

    fn acknowledge_answers(&mut self) {
        for _ in 0..self.domain.kept_answers() {
            let Some(LinkUp::Answer { run, attempt, .. }) = self.domain.answer_at(0) else { panic!("kept answer") };
            assert!(self.step(Event::Down(LinkDown::Acknowledge { run, attempt })).is_empty());
        }
    }

    fn drain(&mut self) -> Vec<Asked> {
        let measured = self.meter.end();
        let mut asked = Vec::new();
        let mut cancelled = Vec::new();
        while let Some(request) = self.out.pop() {
            asked.push(match request {
                Request::Source { .. } | Request::SourceFault => unreachable!("source scheduling has its own world"),
                Request::Up(
                    LinkUp::Logins { .. }
                    | LinkUp::SignedOut { .. }
                    | LinkUp::Piece { .. }
                    | LinkUp::Activity { .. }
                    | LinkUp::Unauthorized { .. }
                    | LinkUp::Limited { .. }
                    | LinkUp::AccountRefused { .. },
                ) => continue,
                Request::Up(LinkUp::Turn { .. }) => Asked::Turn,
                Request::Deliver { owner, .. } => Asked::Delivery { owner },
                Request::Up(LinkUp::Call { call, .. }) => Asked::Relay { call },
                Request::Up(LinkUp::Answer { answer, .. }) => {
                    if answer.turns == 0 {
                        Asked::RunEnding { answer: answer.ending }
                    } else {
                        Asked::Answer { answer }
                    }
                }
                Request::Start { .. } => Asked::Start,
                Request::Prepare { owner, .. } => Asked::Prepare { owner },
                Request::Save { owner, .. } => Asked::Save { owner },
                Request::Hello { hello } => Asked::Hosting { runs: hello.hosting.len() },
                Request::CancelRelay { run, attempt, call } => {
                    cancelled.push(Event::RelayCancelled { run, attempt, call });
                    continue;
                }
                Request::Abort { .. }
                | Request::Message { .. }
                | Request::Reply { .. }
                | Request::Stop { .. }
                | Request::Release { .. }
                | Request::Grant { .. }
                | Request::AcknowledgeAgentTurn { .. } => Asked::Other,
            });
        }
        self.meter.check(measured, self.bound, &self.env.limits);
        // The iteration ends: the reclaim point.
        self.domain.reclaim();
        for event in cancelled {
            asked.extend(self.step(event));
        }
        asked
    }
}

/// An assignment for `run` of exactly the limits: a charter and
/// names of their limits, as many items as a workspace may list.
fn assignment(run: u64, limits: &Limits) -> RunAssignment {
    RunAssignment {
        identity: [0; 16],
        grants: (0..limits.accounts)
            .map(|account| Grant {
                form: jig_domain_host::Form::Credential(jig_domain_host::Scheme::Bearer),
                account,
                generation: 7,
                valid: Duration::from_secs(300),
            })
            .collect(),
        run: Token::new(run),
        attempt: Token::new(run + 1000),
        workspace: Some(Workspace { workstream: run, items: Token::new(run + 2000) }),
        save: true,
        live: jig_domain_host::LiveAsk::NONE,
        charter: bytes(limits.charter_bytes),
    }
}

/// Fills every slot with an assignment of exactly the limits and a full hold
/// of inbound events of exactly their limit; starts each run, has it land a
/// change in every item and make as many calls as it may, the last a
/// delivery still in flight as it ends with an outcome of exactly the limit; and
/// takes each through its tail. Every step's peak is checked against the
/// worst case.
fn fill(limits: Limits) {
    let delivered = Delivery { outcome: DeliveryOutcome::Delivered, left: Token::new(5000), changed: true };
    let mut host = Measured::new(limits);
    let mut owners = Vec::new();
    for run in 0..u64::from(limits.slots) {
        let [Asked::Prepare { owner }] = host.step(jig_host_world::fixtures::assign_with(
            assignment(run, &limits),
            jig_host_world::fixtures::messages(&limits),
        ))[..] else {
            panic!("an assignment of exactly the limits is admitted");
        };
        for _ in 0..limits.held {
            let event = jig_host_world::fixtures::inbound(
                Token::new(run),
                Token::new(run + 1000),
                Token::new(1),
                bytes(limits.event_bytes),
            );
            assert!(host.step(event).is_empty(), "held");
        }
        owners.push(owner);
    }
    let held = host.meter.held();
    let starting = limits.charter_bytes + u64::from(limits.held + limits.inbox) * limits.event_bytes;
    assert!(held >= u64::from(limits.slots) * starting, "{limits:?}: every slot holds what it may as it prepares");

    let mut pending = Vec::new();
    for (index, owner) in owners.iter().enumerate() {
        let index = u64::try_from(index).expect("fits");
        let workspace = Token::new(index);
        assert_eq!(host.step(Event::Prepared { owner: *owner, workspace }), [Asked::Start]);
        let agent = Token::new(index);
        assert_eq!(
            host.step(Event::Started { owner: *owner, agent }).len(),
            usize::try_from(limits.held).expect("fits")
        );
        assert!(host.step(Event::Yielded { owner: *owner }).is_empty(), "waiting");
        let [Asked::Delivery { owner: delivery }] = host.step(jig_host_world::fixtures::called(
            *owner,
            Token::new(1),
            jig_host_world::fixtures::deliver(bytes(64)),
        ))[..] else {
            panic!("delivered");
        };
        assert_eq!(
            host.step(Event::Delivered { owner: delivery, delivery: delivered }),
            [Asked::Other],
            "landed everywhere"
        );
        let mut calls = Vec::new();
        for call in 2..=u64::from(limits.run_calls) {
            let ask = jig_host_world::fixtures::relay(bytes(64));
            let requests = host.step(jig_host_world::fixtures::called(*owner, Token::new(call), ask));
            let [Asked::Relay { call }] = requests.as_slice() else {
                panic!("relayed");
            };
            calls.push(call.clone());
        }
        // The first relayed call is answered, and a delivery takes its place.
        if let Some(call) = calls.first() {
            let relayed = Event::Down(LinkDown::Relayed {
                run: Token::new(index),
                attempt: Token::new(index + 1000),
                call: call.clone(),
                answer: jig_domain_host::Relayed::Answer { error: false, text: bytes(64) },
            });
            assert_eq!(host.step(relayed), [Asked::Other]);
        }
        let delivery = if limits.run_calls > 1 {
            let ask = jig_host_world::fixtures::deliver(bytes(64));
            let [Asked::Delivery { owner: delivery }] =
                host.step(jig_host_world::fixtures::called(*owner, Token::new(99), ask))[..]
            else {
                panic!("delivered again");
            };
            Some(delivery)
        } else {
            None
        };
        let finish = Finish::Ended { outcome: bytes(limits.outcome_bytes) };
        assert!(!host.step(jig_host_world::fixtures::finished(*owner, finish)).is_empty(), "stopped");
        assert!(host.step(Event::Faulted { owner: *owner, fault: AgentFailure::WallTime }).is_empty(), "decided");
        let detail = bytes(u64::from(limits.detail_bytes) + 1);
        let gone = host.step(Event::Gone { owner: *owner, detail });
        if let Some(delivery) = delivery {
            assert!(gone.is_empty(), "the delivery in flight is waited for");
            pending.push(delivery);
        }
    }
    if limits.run_calls > 1 {
        let held = host.meter.held();
        assert!(held >= u64::from(limits.slots) * limits.outcome_bytes, "{limits:?}: every run holds its outcome");
    }
    for delivery in pending {
        owners_push(&mut host, delivery, delivered);
    }
    host.acknowledge_answers();
    assert_eq!(host.domain.hosted(), 0, "every slot came back");

    beyond(&mut host, &limits);
}

/// Entrance refusals consume the supplied payload without retaining it.
fn beyond(host: &mut Measured, limits: &Limits) {
    // Beyond the limits: refused, and nothing held.
    let mut beyond = assignment(0, limits);
    beyond.charter = bytes(limits.charter_bytes + 1);
    let refused = host.step(jig_host_world::fixtures::assign(beyond));
    assert_eq!(refused, [Asked::RunEnding { answer: Ending::Refused(Refusal::Invalid(Invalid::Charter)) }]);
    let [Asked::Prepare { owner: _ }] = host.step(jig_host_world::fixtures::assign(assignment(0, limits)))[..] else {
        panic!("admitted");
    };
    let large = jig_host_world::fixtures::inbound(
        Token::new(0),
        Token::new(1000),
        Token::new(1),
        bytes(limits.event_bytes + 1),
    );
    assert!(host.step(large).is_empty());
}

/// A delivery still in flight as its run ended settles, and the run answers.
fn owners_push(host: &mut Measured, call: Token, delivery: Delivery) {
    let settled = host.step(Event::Delivered { owner: call, delivery });
    let [Asked::Other, Asked::Other, Asked::RunEnding { answer: Ending::Ended { .. } }] = &settled[..] else {
        panic!("told how it went, released and answered: {settled:?}");
    };
}

/// Every other entry point, each measured: a report, cancels of every run and
/// of one, a prepare that fails, a save, and a fault.
fn paths(limits: Limits) {
    let mut host = Measured::new(limits);
    let mut owners = Vec::new();
    for run in 0..u64::from(limits.slots) {
        let [Asked::Prepare { owner }] = host.step(Event::Down(LinkDown::Assign {
            assignment: jig_domain_host::Assignment {
                messages: Box::new([]),
                assignment: assignment(run, &limits),
                turns: Box::new([]),
                answered: Box::new([]),
            },
        }))[..] else {
            panic!("admitted");
        };
        owners.push(owner);
    }
    let reported = host.step(jig_host_world::fixtures::opened());
    assert_eq!(reported, [Asked::Hosting { runs: usize::try_from(limits.slots).expect("fits") }]);
    let first = owners[0];
    let unprepared = Event::Unprepared {
        owner: first,
        failure: Preparation::Permanent { resource: Some(Token::new(7)) },
        detail: bytes(u64::from(limits.detail_bytes) + 1),
    };
    assert_eq!(host.step(unprepared).len(), 1, "answered");
    for owner in owners.iter().skip(1) {
        assert_eq!(host.step(Event::Prepared { owner: *owner, workspace: *owner }), [Asked::Start]);
        assert!(host.step(Event::Started { owner: *owner, agent: *owner }).is_empty());
    }
    assert!(host.step(Event::CancelAll { reason: Reason::Contact }).is_empty());
    while host.domain.is_ready() {
        host.resume();
    }
    for (index, owner) in owners.iter().enumerate().skip(1) {
        let index = u64::try_from(index).expect("fits");
        let cancel = Event::Down(LinkDown::Cancel { run: Token::new(index), attempt: Token::new(index + 1000) });
        assert!(host.step(cancel).is_empty(), "decided");
        let [Asked::Save { owner: saving }] = host.step(Event::Gone { owner: *owner, detail: bytes(10) })[..] else {
            panic!("saved");
        };
        let saved = host.step(Event::Saved { owner: saving, at: Some(Token::new(7)) });
        assert_eq!(saved.len(), 2, "released and answered");
    }
    host.acknowledge_answers();
    // A fault, from a run that had started.
    let [Asked::Prepare { owner }] = host.step(jig_host_world::fixtures::assign(assignment(0, &limits)))[..] else {
        panic!("admitted");
    };
    assert_eq!(host.step(Event::Prepared { owner, workspace: owner }), [Asked::Start]);
    assert!(host.step(Event::Started { owner, agent: owner }).is_empty());
    assert_eq!(host.step(Event::Faulted { owner, fault: AgentFailure::NoProgress }).len(), 1, "stopped");
    assert_eq!(host.step(Event::Gone { owner, detail: bytes(10) }).len(), 1, "saved");
}

#[test]
fn a_host_with_every_slot_full_stays_within_its_worst_case() {
    fill(LIMITS);
    fill(Limits { slots: 16, held: 8, run_calls: 4, ..LIMITS });
    fill(Limits { slots: 200, charter_bytes: 65_536, transcript_bytes: 16_384, event_bytes: 4096, ..LIMITS });
    fill(Limits { run_calls: 1, held: 0, ..LIMITS });
    // The outcome dominates what a run holds: the ending's side of the max.
    fill(Limits { charter_bytes: 16, transcript_bytes: 16, held: 0, outcome_bytes: 4096, run_calls: 4, ..LIMITS });
}

#[test]
fn every_entry_point_stays_within_the_worst_case() {
    paths(LIMITS);
    paths(Limits { slots: 8, ..LIMITS });
}

#[test]
fn ordered_transcripts_and_delivery_feedback_fit_the_hosts_bound() {
    use jig_domain_host::{Assignment, Ending, Finish, Turn};
    let limits = Limits { transcript_bytes: 2048, turn_bytes: 128, turns: 1, turn_queue_bytes: 128, ..LIMITS };
    let mut owners = Vec::with_capacity(usize::try_from(limits.slots).expect("bounded"));
    let mut host = Measured::new(limits);
    for run in 0..u64::from(limits.slots) {
        let assignment = assignment(run, &limits);
        let overhead = u64::try_from(size_of::<Box<[u8]>>()).expect("bounded");
        let turns = (0..16).map(|_| bytes(limits.turn_bytes - overhead)).collect::<Vec<_>>().into_boxed_slice();
        let next = Assignment { messages: Box::new([]), assignment, turns, answered: Box::new([]) };
        let [Asked::Prepare { owner }] = host.step(Event::Down(LinkDown::Assign { assignment: next }))[..] else {
            panic!("prepare")
        };
        owners.push(owner);
    }
    assert!(host.meter.held() >= u64::from(limits.slots) * (limits.charter_bytes + limits.transcript_bytes));
    for owner in owners {
        assert_eq!(host.step(Event::Prepared { owner, workspace: owner }), [Asked::Start]);
        assert!(host.step(Event::Started { owner, agent: owner }).is_empty());
        let [Asked::Delivery { owner: call }] = host.step(jig_host_world::fixtures::called(
            owner,
            Token::new(51),
            Ask::Deliver { title: bytes(9), body: bytes(23) },
        ))[..] else {
            panic!("delivery")
        };
        let delivery = Delivery { outcome: DeliveryOutcome::Refused, left: Token::new(7), changed: false };
        assert_eq!(host.step(Event::Delivered { owner: call, delivery }), [Asked::Other]);
        fill_refusals(&mut host, owner, limits.inbox);
        assert_eq!(
            host.step(Event::Turn {
                owner,
                turn: Turn {
                    opens_window: false,
                    refused: Box::new([]),
                    turn: 1,
                    spent: 23,
                    read: None,
                    body: bytes(limits.turn_bytes)
                }
            }),
            [Asked::Turn, Asked::Other]
        );
        host.step(Event::Finished { read: None, owner, turns: 1, spent: 29, finish: Finish::Parked });
        let [Asked::Save { owner: saving }] =
            host.step(Event::Gone { owner, detail: bytes(u64::from(limits.detail_bytes)) })[..]
        else {
            panic!("save ordinary unfinished work")
        };
        let asked = host.step(Event::Saved { owner: saving, at: Some(Token::new(7)) });
        let [Asked::Other, Asked::Answer { answer }] = &*asked else { panic!("host answer: {asked:?}") };
        assert_eq!((answer.turns, answer.spent), (1, 29));
        let Ending::Parked { work } = &answer.ending else { panic!("parked") };
        assert!(work.saved.is_some());
    }
    host.acknowledge_answers();
    assert_eq!(host.domain.hosted(), 0);
}

fn fill_refusals(host: &mut Measured, owner: Token, inbox: u32) {
    for name in 0..inbox {
        assert!(
            host.step(Event::Refused {
                owner,
                name: Token::new(u64::from(name)),
                reason: jig_domain_host::MessageRefusal::Full
            })
            .is_empty()
        );
    }
}

#[test]
fn source_replacement_at_capacity_stays_within_the_memory_bound() {
    use jig_domain_host::{LoginState, SourceFailure, SourceName, sources};
    let limits = Limits { slots: 0, ..LIMITS };
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let mut out = Queue::with_capacity(max_out(&limits));
    let meter = Meter::new();
    let mut domain = Domain::new(&limits);
    let names: Box<[_]> = (0..limits.program_sources)
        .map(|index| SourceName::new(format!("{index:032}").as_bytes()).expect("full source name"))
        .collect();
    meter.start();
    step(&mut domain, &env, Event::Down(LinkDown::Sources { sources: names.clone() }), &mut out);
    let measured = meter.end();
    let Some(Request::Source { mut owner, .. }) = out.pop() else { panic!("first check") };
    meter.check(measured, worst_case(&limits).expect("source bound"), &limits);
    for index in 0..limits.program_sources {
        let event = Event::SourceAnswered {
            owner,
            answer: sources::Answer::Checked(LoginState::Failed {
                reason: SourceFailure::Exited,
                detail: bytes(u64::from(limits.detail_bytes)),
            }),
        };
        meter.start();
        step(&mut domain, &env, event, &mut out);
        // Keep the full inventory at the measured step peak, then hand it out.
        let measured = meter.end();
        while let Some(request) = out.pop() {
            match request {
                Request::Source { owner: next, .. } => owner = next,
                Request::Up(LinkUp::Logins { logins }) => assert_eq!(logins.len(), names.len()),
                other @ (Request::SourceFault
                | Request::Up(_)
                | Request::Hello { .. }
                | Request::Message { .. }
                | Request::Reply { .. }
                | Request::Start { .. }
                | Request::AcknowledgeAgentTurn { .. }
                | Request::Deliver { .. }
                | Request::CancelRelay { .. }
                | Request::Prepare { .. }
                | Request::Abort { .. }
                | Request::Stop { .. }
                | Request::Grant { .. }
                | Request::Save { .. }
                | Request::Release { .. }) => panic!("unexpected source output: {other:?}"),
            }
        }
        meter.check(measured, worst_case(&limits).expect("source bound"), &limits);
        assert_eq!(domain.sources_settled(), index + 1 == limits.program_sources);
    }
    meter.start();
    step(&mut domain, &env, Event::Down(LinkDown::Sources { sources: names }), &mut out);
    let measured = meter.end();
    assert!(matches!(out.pop(), Some(Request::Source { .. })));
    meter.check(measured, worst_case(&limits).expect("replacement bound"), &limits);
}

#[test]
fn maximum_input_pieces_in_the_shared_queue_fit_the_hosts_bound() {
    use jig_domain_host::{Activity, LiveAsk, LiveKind, Piece};
    let limits = Limits { told: 64, ..LIMITS };
    let mut host = Measured::new(limits);
    let mut assignment = assignment(1, &limits);
    assignment.live = LiveAsk { text: false, reasoning: false, input: true };
    let [Asked::Prepare { owner }] = host.step(jig_host_world::fixtures::assign(assignment))[..] else {
        panic!("prepare")
    };
    assert_eq!(host.step(Event::Prepared { owner, workspace: owner }), [Asked::Start]);
    assert!(host.step(Event::Started { owner, agent: owner }).is_empty());
    while host.domain.pop_activity().is_some() {}
    let before = host.meter.held();
    for offset in 0..limits.told / 2 {
        let piece = Piece {
            turn: 1,
            retry: 1,
            kind: LiveKind::Input { call: bytes(256), tool: bytes(64) },
            offset: u64::from(offset),
            bytes: bytes(u64::from(limits.piece_bytes)),
        };
        assert!(host.step(Event::Piece { owner, piece }).is_empty());
    }
    let payloads = u64::from(limits.told / 2) * (u64::from(limits.piece_bytes) + 256 + 64);
    assert!(host.meter.held() >= before + payloads, "all maximum input payloads stay owned by the hub");
    for _ in limits.told / 2..limits.told {
        assert!(host.step(Event::Activity { owner, activity: Activity::LongDone }).is_empty());
    }
    let mut pieces = 0;
    let mut activity = 0;
    while let Some(record) = host.domain.pop_activity() {
        match record {
            LinkUp::Piece { piece, .. } => {
                assert_eq!(piece.bytes.len(), usize::try_from(limits.piece_bytes).expect("small bound"));
                pieces += 1;
            }
            LinkUp::Activity { .. } => activity += 1,
            other @ (LinkUp::Logins { .. }
            | LinkUp::SignedOut { .. }
            | LinkUp::Unauthorized { .. }
            | LinkUp::Limited { .. }
            | LinkUp::AccountRefused { .. }
            | LinkUp::Turn { .. }
            | LinkUp::Call { .. }
            | LinkUp::Answer { .. }) => panic!("unexpected queued record: {other:?}"),
        }
    }
    assert_eq!((pieces, activity), (limits.told / 2, limits.told / 2));
    assert_eq!((host.domain.pieces_fenced(), host.domain.pieces_lost(), host.domain.activity_lost()), (0, 0, 0));
}

#[test]
fn parking_every_slot_stays_within_the_hosts_memory_bound() {
    eprintln!("park memory price: {}", worst_case(&LIMITS).expect("bounded"));
    let mut host = Measured::new(LIMITS);
    let mut owners = Vec::new();
    for run in 0..u64::from(LIMITS.slots) {
        let [Asked::Prepare { owner }] = host.step(jig_host_world::fixtures::assign(assignment(run, &LIMITS)))[..] else {
            panic!("admitted");
        };
        owners.push(owner);
    }
    for (run, owner) in owners.into_iter().enumerate() {
        let run = u64::try_from(run).expect("small fixture");
        assert_eq!(host.step(Event::Down(LinkDown::Park { run: Token::new(run), attempt: Token::new(run + 1000) })).len(), 1);
        assert!(matches!(&host.step(Event::Unprepared {
            owner, failure: Preparation::Transient, detail: Box::new([]),
        })[..], [Asked::RunEnding { answer: Ending::Parked { .. } }]));
    }
    host.acknowledge_answers();
}
