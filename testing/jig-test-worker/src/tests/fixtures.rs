use crate::{Domain, Encodable, Event, Limits, Request};
use alloc::boxed::Box;
use jig_domain_host as host;
use skein_lib::{Duration, Env, List, Queue, Reader, Time, Token, Wall, Writer};
use smith_host_domain as smith;

pub const RUN: Token = Token::new(41);
pub const ATTEMPT: Token = Token::new(7);
pub const AGENT: Token = Token::new(0);
pub const CALL: Token = Token::new(19);

pub fn limits() -> Limits {
    let hub = jig_protocol_config::profile::profile(jig_protocol_config::profile::Profile::Small).hub;
    let agent = smith::Limits {
        agents: 2,
        directories: 1,
        conflicts: 0,
        path_bytes: 64,
        name_bytes: 64,
        accounts: 8,
        charter_bytes: 65_536,
        transcript_bytes: 262_144,
        answered_bytes: 65_536,
        message_bytes: 65_536,
        messages: 8,
        calls: 8,
        call_bytes: 65_536,
        answer_bytes: smith::Delivered::worst_case(),
        turns: 8,
        turn_bytes: 65_536,
        unacknowledged_bytes: 524_288,
        fact_bytes: 65_536,
        outcome_bytes: 4_096,
        detail_bytes: 4_096,
        spawn_timeout: Duration::from_secs(1),
        no_progress: Duration::from_secs(120),
        silence: Duration::from_secs(60),
        long_span: Duration::from_secs(60),
        long_open: 8,
        wall_time: Duration::from_secs(300),
        cancel_grace: Duration::from_secs(5),
        exit_grace: Duration::from_secs(5),
        teardown: Duration::from_secs(1),
        kill_after: Duration::from_secs(2),
        facts: 8,
    };
    let bounds = host::Bounds {
        program_endpoints: Box::new([]),
        models: Box::new([]),
        turns: 8,
        spend: 100,
        time: Duration::from_secs(60),
        waiting: Duration::from_secs(60),
        checks: Duration::from_secs(60),
        message_bytes: 65_536,
        inbox: 8,
        unacknowledged: 8,
    };
    Limits {
        watermark: crate::WATERMARK,
        hub,
        agent,
        bounds,
        stop_bound: Duration::from_secs(20),
        workstreams: Box::new([1]),
    }
}

// Smith's smallest v4 golden, used by its agent-process world's charter fixture.
const SMALLEST: [u8; 121] = [
    0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0,
];

pub fn charter_bytes() -> Box<[u8]> {
    let source =
        smith_charter::Charter::decode(&smith_charter::CEILINGS, &mut Reader::new(&SMALLEST)).expect("golden charter");
    let mut parts = source.into_parts();
    parts.mode = smith_charter::Mode::Finite;
    parts.budget = smith_charter::Budget::new(
        &smith_charter::CEILINGS,
        smith_charter::BudgetParts {
            turns: 1,
            spend: 2,
            time: Duration::from_secs(60),
            reserve: smith_charter::Reserve::new(
                &smith_charter::CEILINGS,
                smith_charter::ReserveParts { turns: 0, time: Duration::ZERO },
            )
            .expect("reserve"),
        },
    )
    .expect("budget");
    parts.contract = smith_charter::Contract::new(
        &smith_charter::CEILINGS,
        smith_charter::ContractParts {
            report: Some(
                smith_charter::TextRule::new(
                    &smith_charter::CEILINGS,
                    smith_charter::TextRuleParts { max: 128, fields: List::with_capacity(0) },
                )
                .expect("report"),
            ),
            verdicts: List::with_capacity(0),
            change: None,
            failure: None,
        },
    )
    .expect("contract");
    let mut llm = parts.main.into_parts();
    llm.account = Some(0);
    llm.model = Box::from(&b"fake"[..]);
    llm.window = 1_048_576;
    llm.output = 512;
    llm.prices = smith_charter::Prices::new(
        &smith_charter::CEILINGS,
        smith_charter::PricesParts { input: 1, cached: 1, cache_write: None, output: 1, unit: 1_000_000 },
    )
    .expect("prices");
    parts.main = smith_charter::Llm::new(&smith_charter::CEILINGS, llm).expect("model");
    let record = smith_charter::Charter::new(&smith_charter::CEILINGS, parts).expect("charter");
    let mut writer = Writer::new(usize::try_from(record.measure()).expect("charter size"));
    record.encode(&mut writer).expect("measured charter");
    writer.finish()
}

pub fn charter_value() -> smith::Charter {
    let mut endpoints = List::with_capacity(1);
    endpoints
        .push(smith_protocol_channel::Endpoint { name: Box::new([]), number: 0, dialect: 1 })
        .expect("one endpoint");
    let endpoints = smith_protocol_channel::Endpoints::new(endpoints);
    let charter = smith_protocol_channel::decode_charter(&charter_bytes(), &smith_charter::CEILINGS, &endpoints)
        .expect("fixture charter");
    smith::Charter::new(charter, 65_536).expect("bounded charter")
}

pub fn assignment(charter: Box<[u8]>) -> host::Assignment {
    host::Assignment {
        messages: Box::new([]),
        turns: Box::new([]),
        answered: Box::new([]),
        assignment: host::RunAssignment {
            identity: [3; 16],
            run: RUN,
            attempt: ATTEMPT,
            workspace: Some(host::Workspace { workstream: 1, items: Token::new(9) }),
            save: false,
            live: jig_domain_host::LiveAsk::NONE,
            charter,
            grants: Box::new([]),
        },
    }
}

pub fn start(source: host::Start) -> smith::Start {
    smith::Start {
        identity: Some(smith_domain_run::Identity::new(source.identity)),
        live: None,
        logical_run: RUN,
        activation: source.activation,
        messages: Box::new([]),
        workspace: source.workspace,
        charter: charter_value(),
        transcript: None,
        answered: Box::new([]),
        directories: Box::new([smith::Directory {
            root: Token::new(9),
            name: Box::from(&b"workspace"[..]),
            host: Box::from(&b"/test"[..]),
            writable: true,
            git: false,
            conflicts: Box::new([]),
        }]),
        grants: Box::new([]),
    }
}

pub fn turn() -> smith::Turn {
    use smith_domain_session::{llm, record};
    let body = record::Turn {
        model: b"model".as_slice().into(),
        effort: None,
        version: record::VERSION,
        kind: record::Kind::Ordinary,
        endpoint: Some(llm::Endpoint(0)),
        dialect: 0,
        sequence: 1,
        usage: llm::Usage {
            input_tokens: Some(1),
            output_tokens: Some(1),
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
        },
        spent: 2,
        jobs: 0,
        messages: Box::new([]),
    };
    smith::Turn {
        opens_window: false,
        number: 1,
        spent: 2,
        read: None,
        body: smith::TurnValue::new(body, 65_536).expect("bounded turn"),
    }
}

pub fn result() -> smith::RunResult {
    smith::RunResult::Accepted {
        outcome: smith::Declared::new(
            smith_domain_run::outcome::Declared::Report(smith_domain_run::outcome::Report {
                text: Box::from(&b"done"[..]),
                fields: Box::new([]),
            }),
            4_096,
        )
        .expect("bounded result"),
    }
}

pub fn delivery() -> smith::Ask {
    let value = smith_domain_run::outcome::Change {
        fields: Box::new([
            smith_domain_run::outcome::Field { name: Box::from(&b"title"[..]), value: Box::from(&b"a title"[..]) },
            smith_domain_run::outcome::Field { name: Box::from(&b"body"[..]), value: Box::from(&b"a body"[..]) },
        ]),
    };
    smith::Ask::Deliver { fields: smith::Fields::new(value, 65_536).expect("bounded fields") }
}

pub struct Worker {
    pub domain: Domain,
    pub out: Queue<Request>,
    pub env: Env<Limits>,
    pub process: Token,
}

impl Worker {
    pub fn new(limits: Limits) -> Self {
        let out = Queue::uncapped();
        Self {
            domain: Domain::new(&limits),
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            out,
            process: AGENT,
        }
    }

    pub fn step(&mut self, event: Event) {
        crate::step(&mut self.domain, &self.env, event, &mut self.out);
    }

    pub fn assign(&mut self) {
        self.step(Event::Link(host::LinkDown::Assign { assignment: assignment(charter_bytes()) }));
    }

    pub fn take_decode(&mut self) -> (Token, host::Start) {
        let mut found = None;
        for _ in 0..self.out.len() {
            if let Some(Request::Decode { agent, start, .. }) = self.out.pop() {
                found = Some((agent, *start));
                break;
            }
        }
        found.expect("a start is handed out")
    }

    pub fn take_encode(&mut self) -> (Token, Encodable) {
        let mut found = None;
        for _ in 0..self.out.len() {
            if let Some(Request::Encode { owner, value }) = self.out.pop() {
                found = Some((owner, value));
                break;
            }
        }
        found.expect("a value is handed out")
    }

    pub fn hand_outs(&self) -> u32 {
        let mut count = 0_u32;
        for request in &self.out {
            if let Request::Encode { .. } = request {
                count = count.checked_add(1).expect("bounded hand-outs");
            }
        }
        count
    }

    pub fn spawns(&self) -> u32 {
        let mut spawns = 0_u32;
        for request in &self.out {
            if let Request::Process(request) = request
                && let smith::process::Request::Spawn { .. } = request.as_ref()
            {
                spawns = spawns.checked_add(1).expect("bounded spawns");
            }
        }
        spawns
    }

    pub fn started() -> Self {
        Self::lending(Box::new([]))
    }

    /// A started agent lent `grants`, so its notices name known accounts.
    pub fn lending(grants: Box<[smith::Grant]>) -> Self {
        Self::asking(grants, host::LiveAsk::NONE)
    }

    pub fn asking(grants: Box<[smith::Grant]>, live: host::LiveAsk) -> Self {
        let mut worker = Self::new(limits());
        let mut assignment = assignment(charter_bytes());
        assignment.assignment.live = live;
        worker.step(Event::Link(host::LinkDown::Assign { assignment }));
        let (owner, source) = worker.take_decode();
        let mut start = start(source);
        start.grants = grants;
        worker.step(Event::Decoded { agent: owner, start: Ok(Box::new(start)) });
        let mut process = None;
        for _ in 0..worker.out.len() {
            if let Some(Request::Process(request)) = worker.out.pop()
                && let smith::process::Request::Spawn { owner, .. } = *request
            {
                process = Some(owner);
            }
        }
        worker.process = process.expect("one spawned capability");
        worker.step(Event::Process(Box::new(smith::process::Event::Spawned {
            owner: worker.process,
            process: Token::new(99),
        })));
        worker.sent();
        worker.receive(smith::Up::Admitted);
        worker.clear();
        worker
    }

    pub fn receive(&mut self, message: smith::Up) {
        self.step(Event::Process(Box::new(smith::process::Event::Received { owner: self.process, message })));
    }

    pub fn sent(&mut self) {
        let mut sends = 0_u32;
        for request in &self.out {
            if let Request::Process(request) = request
                && let smith::process::Request::Send { .. } = request.as_ref()
            {
                sends = sends.checked_add(1).expect("bounded sends");
            }
        }
        self.clear();
        for _ in 0..sends {
            self.step(Event::Process(Box::new(smith::process::Event::Sent { owner: self.process })));
        }
    }

    pub fn clear(&mut self) {
        for _ in 0..self.out.len() {
            drop(self.out.pop());
        }
    }

    pub fn gone(&mut self) {
        self.sent();
        self.step(Event::Process(Box::new(smith::process::Event::Hangup { owner: self.process })));
        self.step(Event::Process(Box::new(smith::process::Event::Exited {
            owner: self.process,
            exit: Some(smith::Exit::Code(0)),
        })));
        self.step(Event::Process(Box::new(smith::process::Event::Reaped {
            owner: self.process,
            detail: Box::new([]),
        })));
    }

    pub fn take_answer(&mut self) -> host::Answer {
        let mut found = None;
        for _ in 0..self.out.len() {
            if let Some(Request::Link(host::LinkUp::Answer { answer, .. })) = self.out.pop() {
                found = Some(answer);
                break;
            }
        }
        found.expect("one answer reaches the engine")
    }
}

pub fn encoded_turn() -> host::Turn {
    host::Turn {
        turn: 1,
        spent: 2,
        read: None,
        opens_window: false,
        refused: Box::new([]),
        body: Box::from(&b"encoded turn"[..]),
    }
}
