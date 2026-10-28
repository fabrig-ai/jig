use crate::*;
use alloc::boxed::Box;
use skein_lib::{Duration, Env, List, Queue, Time, Token, Wall};
pub(crate) const ME: Handle = Handle { asker: Asker::JIG, number: 1 };
pub(crate) const TASK: Handle = Handle { asker: Asker::JIG, number: 2 };
pub(crate) const RUN: Handle = Handle { asker: Asker::JIG, number: 4 };
pub(crate) const RUN_ATTEMPT: u64 = 1;
pub(crate) const FIRST_TURN: u32 = 1;
pub(crate) const NEXT_TURN: u32 = 2;
pub(crate) const FIRST_RETRY: u32 = 0;
pub(crate) const NEXT_RETRY: u32 = 1;
pub(crate) const PAGE: Handle = Handle { asker: Asker::JIG, number: 3 };
pub(crate) const PROJECT: u32 = 1;
pub(crate) const OTHER: Handle = Handle { asker: Asker::JIG, number: 5 };
pub(crate) const SUBJECT_TASK: u64 = 9;
pub(crate) const OTHER_SUBJECT: u64 = 10;
pub(crate) const INITIAL_TASK_WATCH: Token = Token::new(2);
// The receiving fixture has four request places; a full engine reports this count and expiry.
pub(crate) const FULL_HELD: u32 = 4;
pub(crate) const FULL_ROOM: Wall = Wall::from_nanos(120_000_000_000);
pub(crate) fn limits() -> Limits {
    Limits {
        owners: &[Spoken { family: 19, version: 1 }, Spoken { family: 1, version: 1 }],
        // tests/client/src/test.rs:70-77, the concrete world's table profile.
        requests: skein_lib::RequestLimits {
            requests: 4,
            bytes: 65536,
            out: 16,
            first: Duration::from_millis(10),
            most: Duration::from_millis(80),
            margin: Duration::from_secs(1),
        },
        ask_bytes: 16384,
        rows: 2,
        segments: 4,
        text: 32,
        turn_text: 32,
        projects: 2,
        fields: 4,
        delegates: 2,
        records: 2,
        kept: 4,
        parts: 8,
        authority: 8,
        accounts: AccountCounts { accounts: 8, sources: 8, sign_ins: 8 },
        heartbeat: Duration::from_millis(100),
        handles: 8,
        turn_bytes: 32,        // tests/client/src/test.rs:42
        reads: 8,              // tests/client/src/test.rs:32
        watches: 8,            // tests/client/src/test.rs:23
        document_bytes: 65536, // tests/client/src/test.rs:38
        backoff: Backoff {
            // tests/client/src/test.rs:51-54
            first: Duration::from_millis(10),
            most: Duration::from_millis(80),
        },
    }
}
pub(crate) fn task(task: u64) -> Watchable {
    Watchable::Jig(contract::Subject::Task { task })
}
pub(crate) fn delivery(task: u64) -> Up {
    Up::Watched {
        watch: INITIAL_TASK_WATCH,
        event: contract::WatchEvent::Delivered(contract::Delivery::Task(Box::new(document(task)))),
    }
}
#[derive(Debug)]
pub(crate) struct Client {
    pub domain: Domain,
    pub env: Env<Limits>,
    pub out: Queue<Request>,
    pub downs: List<Down>,
    pub told: List<Told>,
}
impl Client {
    pub(crate) fn new(limits: Limits) -> Self {
        let mut client = Self::unsigned(limits);
        client.step(Event::Up { up: Up::SignedIn });
        client.told();
        client.downs();
        client
    }
    pub(crate) fn unsigned(limits: Limits) -> Self {
        Self {
            domain: Domain::new(&limits, 7),
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            out: Queue::with_capacity(max_out(&limits)),
            downs: List::with_capacity(max_out(&limits)),
            told: List::with_capacity(max_out(&limits)),
        }
    }
    pub(crate) fn started() -> Self {
        let mut client = Self::new(limits());
        client.step(Event::Start { kept: List::with_capacity(0) });
        client.downs();
        client
    }
    pub(crate) fn step(&mut self, event: Event) {
        step(&mut self.domain, &self.env, event, &mut self.out);
        self.drain();
        reclaim(&mut self.domain);
    }
    pub(crate) fn at(&mut self, time: Time) {
        self.env.now = time;
        fire(&mut self.domain, &self.env, &mut self.out);
        self.drain();
        reclaim(&mut self.domain);
    }
    fn drain(&mut self) {
        while let Some(request) = self.out.pop() {
            match request {
                Request::Down { down } => self.downs.push(down).unwrap(),
                Request::Told { told } => self.told.push(told).unwrap(),
            }
        }
    }
    pub(crate) fn downs(&mut self) -> Box<[Down]> {
        core::mem::replace(&mut self.downs, List::with_capacity(max_out(&self.env.limits))).into_boxed()
    }
    pub(crate) fn told(&mut self) -> Box<[Told]> {
        core::mem::replace(&mut self.told, List::with_capacity(max_out(&self.env.limits))).into_boxed()
    }
    pub(crate) fn name(&self, subject: Watchable) -> Token {
        for (_, w) in &self.domain.watches {
            if w.subject == subject && !w.closing {
                return w.name.unwrap();
            }
        }
        unreachable!("fixture watch exists")
    }
    pub(crate) fn deliver(&mut self, task: u64) {
        let watch = self.name(self::task(task));
        self.step(Event::Up {
            up: Up::Watched {
                watch,
                event: contract::WatchEvent::Delivered(contract::Delivery::Task(Box::new(document(task)))),
            },
        });
    }
    pub(crate) fn end(&mut self, watch: Token, end: contract::WatchEnd) {
        self.step(Event::Up { up: Up::WatchEnded { watch, end } });
    }
}

fn text() -> contract::Text {
    contract::Text::new(Box::from([]), 0).unwrap()
}
fn document(number: u64) -> contract::Task {
    contract::Task {
        dependency_facts: List::with_capacity(0),
        kept_account: None,
        run_wait: None,
        settings: Some(contract::TaskSettings { task: 1, run: None, newest: 1 }),
        label: 0,
        flags: contract::TaskFlags {
            ended_by: contract::EndedBy::Result,
            conversation: false,
            listed: true,
            retention: contract::Retention::Kept,
        },
        last_activity: Wall::from_nanos(0),
        waits_on: contract::WaitsOn::Nothing,
        chip: contract::Chip {
            task: number,
            title: contract::DocumentText::complete(text()),
            executor: contract::Executor::Agent { charter: 1 },
            phase: contract::Phase::Waiting,
            details: Box::new(contract::ChipPhase::Starting),
        },
        goal: None,
        priority: None,
        lineage_chips: List::with_capacity(0),
        inputs: List::with_capacity(0),
        project: 7,
        requester: contract::OwnerRequester::Task(number),
        lineage: List::with_capacity(0),
        spec: contract::SpecificationDocument {
            specification: contract::DocumentSpecification {
                asks: false,
                words: contract::DocumentText { bytes: text(), cut: false },
                parameters: List::with_capacity(0),
                inputs: List::with_capacity(0),
            },
            cut: false,
        },
        spec_revision: contract::Revision { value: 7 },
        person_revision: None,
        taken_by: None,
        instructions: None,
        wake: contract::OwnerWakePolicy {
            words: contract::WakeRule::Never,
            notices: contract::WakeRule::Never,
            news: contract::WakeRule::Never,
            results: contract::ResultsWake::Never,
            questions: false,
            answers: false,
            timers: false,
        },
        subscriptions: contract::Subscriptions { entries: List::with_capacity(0) },
        result_contract: contract::TaskContractDocument::Approval {
            state: Box::from([]),
            part: contract::OwnerPart {
                connector: 7,
                kind: contract::OwnerPartKind::Resource,
                key: contract::OwnerPartKey::Path(List::with_capacity(0)),
            },
        },
        dependencies: List::with_capacity(0),
        references: List::with_capacity(0),
        cut: false,
        resources: List::with_capacity(0),
        budget: contract::OwnerBudget {
            numbers: contract::OwnerNumbers { budget: 7, spent: 7, spent_below: 7, reserved: 7 },
            left: 7,
            funder: contract::Funder::Task(number),
            deadline: None,
        },
        authority: contract::OwnerAuthorityDocument {
            authority: contract::OwnerAuthority {
                models: None,
                tools: 7,
                grants: List::with_capacity(0),
                delegation: contract::OwnerDelegation { kinds: List::with_capacity(0), tasks: 7, depth: 7 },
                spend: 7,
                deadline: None,
                notes: 7,
                note_resources: List::with_capacity(0),
            },
            cut: false,
        },
        hold: None,
        proposals: List::with_capacity(0),
        activity: 7,
        changed: Wall::from_nanos(7),
        acts: List::with_capacity(0),
        result: None,
        delegates: List::with_capacity(0),
        context: None,
        llm: None,
    }
}

pub(crate) const KEY: Key = Key { asker: Asker::JIG, number: 1 };
pub(crate) const OTHER_KEY: Key = Key { asker: Asker(1), number: 2 };
pub(crate) const PARTY: u64 = 1;
pub(crate) const OTHER_PARTY: u64 = 2;
pub(crate) const ENGINE_KEY: skein_lib::RequestKey = [1; 16];
pub(crate) const OTHER_ENGINE_KEY: skein_lib::RequestKey = [2; 16];
/// Given names distinct from the two drawn-key fixtures.
pub(crate) const NAME: skein_lib::RequestKey = [3; 16];
pub(crate) const OTHER_NAME: skein_lib::RequestKey = [4; 16];
pub(crate) const ME_WATCH: Token = Token::new(1);
pub(crate) fn stop() -> Ask {
    Ask::Jig(Box::new(contract::Ask::Stop { project: 1, task: SUBJECT_TASK }))
}
pub(crate) fn seen() -> Ask {
    Ask::Jig(Box::new(contract::Ask::Seen {
        through: contract::SeenThrough::Conversation { task: SUBJECT_TASK, through: 1 },
    }))
}
pub(crate) fn me(party: u64, retention: Duration) -> Up {
    Up::Watched {
        watch: ME_WATCH,
        event: contract::WatchEvent::Delivered(contract::Delivery::Me(Box::new(contract::Me {
            providers: List::with_capacity(0),
            providers_cut: false,
            key_bytes: 256,
            redirect_bytes: 4096,
            party: contract::MeParty {
                number: party,
                kind: contract::PartyKind::Person,
                login: contract::DocumentText::complete(text()),
                name: contract::DocumentText::complete(text()),
            },
            projects: List::with_capacity(0),
            needs: contract::Needs { questions: 0, proposals: 0, escalations: 0, person_tasks: 0 },
            needs_total: 0,
            updates: contract::Count { value: 0, limited: false },
            results_seen: 0,
            cut: false,
            retention,
            deployment_role: None,
        }))),
    }
}
pub(crate) fn record(key: Key, engine: skein_lib::RequestKey, party: u64) -> skein_lib::RequestRecord<Held> {
    skein_lib::RequestRecord {
        key: engine,
        scope: party,
        first_sent: Some(Wall::EPOCH),
        request: Held { key, request: stop() },
    }
}
impl Client {
    pub(crate) fn live() -> Self {
        let mut c = Self::started();
        c.step(Event::Up { up: me(PARTY, Duration::from_secs(1)) });
        c.told();
        c
    }
    pub(crate) fn send(&mut self) -> Token {
        self.send_as(KEY, None)
    }
    pub(crate) fn send_as(&mut self, key: Key, name: Option<skein_lib::RequestKey>) -> Token {
        self.step(Event::Send { key, name, request: stop() });
        let mut attempt = None;
        for down in self.downs() {
            if let Down::Send { attempt: sent, .. } = down {
                attempt = Some(sent);
            }
        }
        self.told();
        attempt.unwrap()
    }
}

pub(crate) fn project(project: u32) -> Readable {
    Readable::Jig(contract::Read::Project { project })
}
impl Client {
    pub(crate) fn read(&mut self, handle: Handle, read: Readable) -> Token {
        self.step(Event::Read { handle, read });
        let downs = self.downs();
        let [Down::Read { read, .. }] = downs.as_ref() else {
            panic!("one read");
        };
        *read
    }
}

pub(crate) fn run(task: u64, turn: u32, retry: u32, from: u32, bytes: &[u8]) -> Up {
    assert_eq!(task, SUBJECT_TASK);
    Up::Watched {
        watch: INITIAL_TASK_WATCH,
        event: contract::WatchEvent::Delivered(contract::Delivery::Run(Box::new(contract::RunText {
            writing: None,
            attempt: RUN_ATTEMPT,
            began: Some(Wall::EPOCH),
            turn: contract::ProvisionalTurnName { attempt: RUN_ATTEMPT, number: turn, retry },
            from,
            through: from.checked_add(u32::try_from(bytes.len()).unwrap()).unwrap(),
            text: contract::Text::new(Box::from(bytes), u32::MAX).unwrap(),
            cut: false,
            call: None,
        }))),
    }
}

pub(crate) fn run_subject() -> Watchable {
    Watchable::Jig(contract::Subject::Run { task: SUBJECT_TASK, turn: None, offset: 0 })
}
impl Client {
    pub(crate) fn running() -> Self {
        let mut c = Self::started();
        c.step(Event::Watch { handle: RUN, subject: run_subject() });
        c.downs();
        c
    }
    pub(crate) fn run(&mut self, turn: u32, retry: u32, from: u32, bytes: &[u8]) {
        let mut up = run(SUBJECT_TASK, turn, retry, from, bytes);
        let Up::Watched { watch, .. } = &mut up else {
            unreachable!("fixture run delivery");
        };
        *watch = self.name(run_subject());
        self.step(Event::Up { up });
    }
}
