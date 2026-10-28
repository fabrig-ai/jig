//! The worker routes each child output before returning (hosts.md, section 4).
use crate::{
    Encodable, Encoded, Event, Invalid, Limits, Request,
    hand_out::{Finish, Pending},
    translate,
    workspace::Workspace,
};
use alloc::boxed::Box;
use jig_domain_host as host;
use skein_lib::{Env, Map, Queue, Time, Token};
use smith_host_domain as smith;

#[derive(Debug)]
pub(crate) struct Call {
    owner: Token,
    agent: Token,
    callback: Token,
    deadline: Time,
    ask: Option<smith::Ask>,
    name: Option<Box<[u8]>>,
    withdrawn: bool,
    delivered: Option<smith::Delivery>,
    delivery: bool,
}

impl Call {
    fn new(owner: Token, agent: Token, callback: Token, deadline: Time, ask: smith::Ask) -> Self {
        let delivery = match &ask {
            smith::Ask::Deliver { .. } => true,
            smith::Ask::Host { .. } => false,
        };
        Self {
            owner,
            agent,
            callback,
            deadline,
            ask: Some(ask),
            name: None,
            withdrawn: false,
            delivered: None,
            delivery,
        }
    }
}

#[derive(Debug)]
pub struct Domain {
    hub: host::Domain,
    agent: smith::Domain,
    agents: Map<Token, Token>,
    calls: Map<(Token, Token), Call>,
    pending: Queue<Pending>,
    gone: Map<Token, Box<[u8]>>,
    hub_out: Queue<host::Request>,
    agent_out: Queue<smith::Output>,
    workspace: Workspace,
    stopping: bool,
}

impl Domain {
    #[must_use]
    pub fn new(limits: &Limits) -> Self {
        assert!(crate::worst_case(limits).is_some(), "worker child bounds are representable");
        Self {
            hub: host::Domain::new(&limits.hub),
            agent: smith::Domain::new(&limits.agent),
            agents: Map::with_capacity(limits.hub.slots),
            calls: Map::with_capacity(limits.hub.slots.checked_mul(limits.agent.calls).expect("validated calls")),
            pending: Queue::uncapped(),
            gone: Map::with_capacity(limits.hub.slots),
            hub_out: Queue::uncapped(),
            agent_out: Queue::uncapped(),
            workspace: Workspace::new(limits.hub.slots),
            stopping: false,
        }
    }

    #[must_use]
    pub fn next_deadline(&self) -> Option<Time> {
        [self.agent.next_deadline(), self.hub.next_deadline()].into_iter().flatten().min()
    }

    pub fn reclaim(&mut self) {
        self.hub.reclaim();
        self.agent.reclaim();
    }

    /// Entries still being routed or awaiting a payload hand-out return.
    #[must_use]
    pub fn backlog(&self) -> u64 {
        u64::from(self.hub_out.len())
            .saturating_add(u64::from(self.agent_out.len()))
            .saturating_add(u64::from(self.pending.len()))
    }

    #[must_use]
    pub fn hosted(&self) -> u32 {
        self.hub.hosted()
    }

    #[must_use]
    pub fn work_pending(&self) -> bool {
        self.hub.is_ready() || !self.hub_out.is_empty() || !self.agent_out.is_empty() || self.stopping_ready()
    }

    fn stopping_ready(&self) -> bool {
        self.stopping
            && self.hub.sources_settled()
            && self.hub.hosted() == 0
            && self.agent.agents() == 0
            && self.pending.is_empty()
    }

    #[must_use]
    pub fn delivery(&self, workspace: Token) -> Option<&crate::Content> {
        self.workspace.contents.get(&workspace)?.as_ref()
    }
}

pub fn step(domain: &mut Domain, env: &Env<Limits>, event: Event, out: &mut Queue<Request>) {
    match event {
        Event::Source(event) => hub(domain, env, *event),
        Event::Link(link) => hub(domain, env, host::Event::Down(link)),
        Event::Ready => hub(
            domain,
            env,
            host::Event::Opened {
                stop_bound: env.limits.stop_bound,
                workstreams: env.limits.workstreams.clone(),
                bounds: env.limits.bounds.clone(),
            },
        ),
        Event::Lost => hub(domain, env, host::Event::SourceLost),
        Event::GracePassed => {
            hub(domain, env, host::Event::CancelAll { reason: host::Reason::Contact });
            if domain.stopping {
                domain.hub.give_up_turns();
            }
        }
        Event::Shutdown => {
            domain.stopping = true;
            hub(domain, env, host::Event::CancelAll { reason: host::Reason::Shutdown });
        }
        Event::Resend { run, attempt, turn } => {
            if let Some(up) = domain.hub.turn(run, attempt, turn) {
                out.push(Request::Link(host::LinkUp::Turn { run, attempt, turn: up }));
            }
        }
        Event::RelayCancelled { run, attempt, call } => {
            hub(domain, env, host::Event::RelayCancelled { run, attempt, call });
        }
        Event::Process(event) => agent(domain, env, smith::Input::Process(*event)),
        Event::Live { agent: source, piece } => {
            let mut owner = None;
            for (client, agent) in &domain.agents {
                if *agent == source {
                    owner = Some(*client);
                    break;
                }
            }
            if let Some(owner) = owner {
                agent(domain, env, smith::Input::Process(smith::process::Event::Progress { owner: source }));
                hub(domain, env, host::Event::Piece { owner, piece });
            }
        }
        Event::Decoded { agent: owner, start } => decoded(domain, env, owner, start),
        Event::Encoded { owner, encoded: value } => {
            encoded(domain, env, owner, value);
            if !hand_out_pending(domain, owner)
                && let Some(detail) = domain.gone.remove(&owner)
            {
                gone(domain, env, owner, detail);
            }
        }
        Event::Activity { owner, activity } => {
            if let Some(activity) = activity {
                hub(domain, env, host::Event::Activity { owner, activity });
            }
        }
        Event::Timer => {
            host::resume(&mut domain.hub, &hub_env(env), &mut domain.hub_out);
            smith::fire(&mut domain.agent, &agent_env(env), &mut domain.agent_out);
        }
    }
    route(domain, env, out);
}

pub fn fire(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    step(domain, env, Event::Timer, out);
}

fn hub_env(env: &Env<Limits>) -> Env<host::Limits> {
    Env { now: env.now, wall: env.wall, limits: env.limits.hub }
}
fn agent_env(env: &Env<Limits>) -> Env<smith::Limits> {
    Env { now: env.now, wall: env.wall, limits: env.limits.agent }
}
fn hub(domain: &mut Domain, env: &Env<Limits>, event: host::Event) {
    host::step(&mut domain.hub, &hub_env(env), event, &mut domain.hub_out);
}
fn agent(domain: &mut Domain, env: &Env<Limits>, event: smith::Input) {
    smith::step(&mut domain.agent, &agent_env(env), event, &mut domain.agent_out);
}
fn parent(domain: &mut Domain, env: &Env<Limits>, event: smith::parent::Event) {
    agent(domain, env, smith::Input::Parent(event));
}

fn decoded(domain: &mut Domain, env: &Env<Limits>, owner: Token, start: Result<Box<smith::Start>, Invalid>) {
    let Some(hosting) = domain.hub.hosting(owner) else { return };
    if hosting.phase != host::Phase::Starting {
        hub(domain, env, host::Event::Gone { owner, detail: Box::new([]) });
        return;
    }
    match start {
        Ok(start) => parent(domain, env, smith::parent::Event::Spawn { client: owner, start: *start }),
        Err(invalid) => {
            let why: &[u8] = match invalid {
                Invalid::Charter => b"charter",
                Invalid::Transcript => b"transcript",
                Invalid::CallName => b"call name",
            };
            hub(
                domain,
                env,
                host::Event::from_agent(host::FromAgent::StartRefused {
                    owner,
                    why: Box::from(why),
                    cause: host::StartCause::Other,
                }),
            );
        }
    }
}

fn route(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    loop {
        if let Some(request) = domain.hub_out.pop() {
            down(domain, env, request, out);
        } else if let Some(request) = domain.agent_out.pop() {
            match request {
                smith::Output::Process(request) => out.push(Request::Process(Box::new(request))),
                smith::Output::Parent(request) => up(domain, env, request, out),
            }
        } else {
            break;
        }
    }
    for _ in 0..env.limits.hub.told {
        let Some(activity) = domain.hub.pop_activity() else { break };
        out.push(Request::Link(activity));
    }
    assert!(
        domain.hub_out.is_empty() && domain.agent_out.is_empty(),
        "child hand-offs finish before the next entrance"
    );
    if domain.stopping_ready() {
        out.push(Request::Stop);
        domain.stopping = false;
    }
}

fn down(domain: &mut Domain, env: &Env<Limits>, request: host::Request, out: &mut Queue<Request>) {
    match request.to_agent() {
        Ok(request) => to_agent(domain, env, request, out),
        Err(request) => match request {
            host::Request::Source { owner, name, operation, deadline } => {
                out.push(Request::Source { owner, name, operation, deadline });
            }
            host::Request::SourceFault => out.push(Request::SourceFault),
            host::Request::Up(up) => out.push(Request::Link(up)),
            host::Request::Hello { hello } => {
                out.push(Request::Hello(hello));
                for index in 0..domain.hub.retained_turns() {
                    if let Some(turn) = domain.hub.turn_at(index) {
                        out.push(Request::Link(turn));
                    }
                }
                for index in 0..domain.hub.kept_answers() {
                    if let Some(answer) = domain.hub.answer_at(index) {
                        out.push(Request::Link(answer));
                    }
                }
            }
            host::Request::CancelRelay { run, attempt, call } => out.push(Request::CancelRelay { run, attempt, call }),
            host::Request::Prepare { owner, workspace: _ } => {
                let event = domain.workspace.prepare(owner);
                hub(domain, env, event);
            }
            host::Request::Abort { owner } => hub(
                domain,
                env,
                host::Event::Unprepared { owner, failure: host::Preparation::Transient, detail: Box::new([]) },
            ),
            host::Request::Deliver { owner, workspace, title, body } => {
                let delivered = domain.workspace.deliver(workspace, title, body);
                let mut key = None;
                for (at, call) in &domain.calls {
                    if call.delivery && call.delivered.is_none() && call.owner == workspace {
                        key = Some(*at);
                        break;
                    }
                }
                domain.calls.get_mut(&key.expect("workspace owns one delivery")).expect("owned callback").delivered =
                    Some(delivered);
                hub(
                    domain,
                    env,
                    host::Event::Delivered {
                        owner,
                        delivery: host::Delivery {
                            outcome: host::DeliveryOutcome::Delivered,
                            left: workspace,
                            changed: true,
                        },
                    },
                );
            }
            host::Request::Save { owner, workspace: _ } => hub(domain, env, host::Event::Saved { owner, at: None }),
            host::Request::Release { workspace } => domain.workspace.release(workspace),
            host::Request::Message { .. }
            | host::Request::Reply { .. }
            | host::Request::Start { .. }
            | host::Request::AcknowledgeAgentTurn { .. }
            | host::Request::Stop { .. }
            | host::Request::Grant { .. } => {
                unreachable!("to_agent returns every agent-directed request as Ok");
            }
        },
    }
}

fn to_agent(domain: &mut Domain, env: &Env<Limits>, request: host::ToAgent, out: &mut Queue<Request>) {
    let event = match request {
        host::ToAgent::Start { owner, start } => {
            let run = domain.hub.hosting(owner).expect("hosted start").run;
            out.push(Request::Decode { agent: owner, run, start });
            return;
        }
        host::ToAgent::Message { agent, name, sender, words, finish_requested } => {
            smith::parent::Event::Message { agent, name, label: sender, text: words, finish_requested }
        }
        host::ToAgent::Acknowledge { agent, turn } => smith::parent::Event::Acknowledge { agent, turn },
        host::ToAgent::Cancel { agent } => smith::parent::Event::Stop { agent },
        host::ToAgent::Grant { agent, grant } => {
            let Some(grant) = translate::grant(grant) else { return };
            smith::parent::Event::Grant { agent, grant }
        }
        host::ToAgent::Answer { agent, call: name, reply } => {
            let mut key = None;
            for (at, call) in &domain.calls {
                if call.agent == agent && call.name.as_deref() == Some(name.as_ref()) {
                    key = Some(*at);
                    break;
                }
            }
            let Some(key) = key else { return };
            let mut call = domain.calls.remove(&key).expect("owned callback");
            let reply = match reply {
                host::Reply::Relayed { error, text } => smith::Reply::Host { error, body: text },
                host::Reply::Delivered(_) => smith::Reply::Delivery(call.delivered.take().expect("workspace terminal")),
                host::Reply::Unavailable => smith::Reply::Unavailable,
                host::Reply::Withdrawn => smith::Reply::Withdrawn,
                host::Reply::Busy => smith::Reply::Busy,
            };
            smith::parent::Event::Answer { agent, call: call.callback, reply }
        }
    };
    parent(domain, env, event);
}

fn up(domain: &mut Domain, env: &Env<Limits>, request: smith::parent::Request, out: &mut Queue<Request>) {
    match request {
        smith::parent::Request::Started { client, agent } => {
            let old = domain.agents.insert(client, agent).expect("the hub bounds agents");
            assert!(old.is_none(), "one started agent per hosted owner");
        }
        smith::parent::Request::Admitted { client } => {
            let agent = *domain.agents.get(&client).expect("started before admitted");
            hub(domain, env, host::Event::Started { owner: client, agent });
        }
        smith::parent::Request::Turn { client, turn } => {
            domain.pending.push(Pending::Turn { owner: client });
            out.push(Request::Encode { owner: client, value: Encodable::Turn(turn) });
        }
        smith::parent::Request::Called { client, logical_run: _, call, name, deadline, ask } => {
            let agent = *domain.agents.get(&client).expect("admitted call");
            let old = domain
                .calls
                .insert((client, call), Call::new(client, agent, call, deadline, ask))
                .expect("Smith bounds callbacks");
            assert!(old.is_none(), "one callback right");
            domain.pending.push(Pending::Call { owner: client, call });
            out.push(Request::Encode { owner: client, value: Encodable::CallName(name) });
        }
        smith::parent::Request::Withdrawn { client, call } => {
            if let Some(call) = domain.calls.get_mut(&(client, call)) {
                call.withdrawn = true;
                if let Some(name) = &call.name {
                    let name = name.clone();
                    hub(domain, env, host::Event::Withdrawn { owner: client, call: name });
                }
            }
        }
        smith::parent::Request::Answered { client, answer, detail: _ } => {
            let finish = Finish::of(&answer.result);
            domain.pending.push(Pending::Result {
                owner: client,
                turns: answer.turns,
                spent: answer.spent,
                read: answer.read,
                finish,
            });
            out.push(Request::Encode { owner: client, value: Encodable::Result(answer.result) });
        }
        smith::parent::Request::Told { client, body } => out.push(Request::Fact { owner: client, body }),
        smith::parent::Request::Waiting { client, read: _ } => hub(domain, env, host::Event::Yielded { owner: client }),
        smith::parent::Request::Long { client, operation, span } => hub(
            domain,
            env,
            host::Event::Activity {
                owner: client,
                activity: host::Activity::Long { operation: translate::operation(operation), span },
            },
        ),
        smith::parent::Request::LongDone { client } => {
            hub(domain, env, host::Event::Activity { owner: client, activity: host::Activity::LongDone });
        }
        smith::parent::Request::Unauthorized { client, account, generation } => {
            hub(domain, env, host::Event::Unauthorized { owner: client, account, generation });
        }
        smith::parent::Request::Limited { client, account, reset } => {
            hub(domain, env, host::Event::Limited { owner: client, account, reset });
        }
        smith::parent::Request::Refused { client, account, code, reason } => {
            hub(domain, env, host::Event::AccountRefused { owner: client, account, code, reason });
        }
        smith::parent::Request::Faulted { client, fault } => {
            if let Some(hosting) = domain.hub.hosting(client)
                && hosting.phase == host::Phase::Starting
            {
                return;
            }
            hub(domain, env, host::Event::Faulted { owner: client, fault: translate::fault(fault) });
        }
        smith::parent::Request::MessageRefused { client, name, reason } => {
            hub(domain, env, host::Event::Refused { owner: client, name, reason: translate::refusal(reason) });
        }
        smith::parent::Request::Unread { client, names } => {
            if let Some(hosting) = domain.hub.hosting(client)
                && hosting.phase != host::Phase::Starting
            {
                for name in names {
                    hub(
                        domain,
                        env,
                        host::Event::Refused { owner: client, name, reason: host::MessageRefusal::Ending },
                    );
                }
            }
        }
        smith::parent::Request::Gone { client, end, detail } => ended(domain, env, client, end, detail),
    }
}

fn encoded(domain: &mut Domain, env: &Env<Limits>, owner: Token, encoded: Encoded) {
    let pending = domain.pending.pop().expect("each hand-out is answered once in order");
    match pending {
        Pending::Turn { owner: expected } => {
            assert_eq!(owner, expected, "ordered hand-out owner");
            match encoded {
                Encoded::Turn(turn) => hub(domain, env, host::Event::Turn { owner, turn }),
                Encoded::Undecodable => {
                    hub(domain, env, host::Event::Faulted { owner, fault: host::AgentFailure::Rules });
                }
                Encoded::CallName(_) | Encoded::Result(_) => unreachable!("routing echoes the hand-out kind"),
            }
        }
        Pending::Call { owner: expected, call } => {
            assert_eq!(owner, expected, "ordered hand-out owner");
            let Encoded::CallName(name) = encoded else {
                let entry = domain.calls.remove(&(owner, call)).expect("pending callback");
                parent(
                    domain,
                    env,
                    smith::parent::Event::Answer { agent: entry.agent, call, reply: smith::Reply::Unavailable },
                );
                hub(domain, env, host::Event::Faulted { owner, fault: host::AgentFailure::Rules });
                return;
            };
            let entry = domain.calls.get_mut(&(owner, call)).expect("pending call owns callback");
            entry.name = Some(name.clone());
            let ask = entry.ask.take().expect("call handed to hub once");
            let deadline = entry.deadline.saturating_since(env.now);
            let withdrawn = entry.withdrawn;
            let ask = match ask {
                smith::Ask::Host { tool, effect, body } => {
                    host::Ask::Relay { tool, writes: effect == smith::Effect::Write, input: body, deadline }
                }
                smith::Ask::Deliver { fields } => {
                    let mut title: Box<[u8]> = Box::new([]);
                    let mut body: Box<[u8]> = Box::new([]);
                    for field in fields.into_value().fields {
                        match field.name.as_ref() {
                            b"title" => title = field.value,
                            b"body" => body = field.value,
                            _ => {}
                        }
                    }
                    host::Ask::Deliver { title, body }
                }
            };
            hub(domain, env, host::Event::Called { owner, call: name.clone(), ask });
            if withdrawn {
                hub(domain, env, host::Event::Withdrawn { owner, call: name });
            }
        }
        Pending::Result { owner: expected, turns, spent, read, finish } => {
            assert_eq!(owner, expected, "ordered hand-out owner");
            let Encoded::Result(body) = encoded else {
                hub(domain, env, host::Event::Faulted { owner, fault: host::AgentFailure::Rules });
                return;
            };
            let finish = match finish {
                Finish::Ended => host::Finish::Ended { outcome: body },
                Finish::Parked => host::Finish::Parked,
                Finish::Failed(failure) => host::Finish::Failed { failure },
                Finish::Refused(cause) => {
                    hub(
                        domain,
                        env,
                        host::Event::from_agent(host::FromAgent::StartRefused { owner, why: body, cause }),
                    );
                    return;
                }
            };
            hub(domain, env, host::Event::Finished { owner, turns, spent, read, finish });
        }
    }
}

fn hand_out_pending(domain: &Domain, owner: Token) -> bool {
    for pending in &domain.pending {
        let expected = match pending {
            Pending::Turn { owner } | Pending::Call { owner, .. } | Pending::Result { owner, .. } => *owner,
        };
        if owner == expected {
            return true;
        }
    }
    false
}

fn gone(domain: &mut Domain, env: &Env<Limits>, owner: Token, detail: Box<[u8]>) {
    if let Some(hosting) = domain.hub.hosting(owner)
        && hosting.phase != host::Phase::Answered
    {
        hub(domain, env, host::Event::Gone { owner, detail });
    }
}

fn ended(domain: &mut Domain, env: &Env<Limits>, owner: Token, end: smith::End, detail: Box<[u8]>) {
    let started = domain.agents.remove(&owner).is_some();
    let unstarted = match end {
        smith::End::Busy | smith::End::Invalid(_) | smith::End::Unspawned => true,
        smith::End::Exited { .. } | smith::End::Settled => false,
    };
    assert!(started || unstarted, "a process terminal follows Started");
    if let Some(why) = translate::unstarted(end) {
        hub(
            domain,
            env,
            host::Event::from_agent(host::FromAgent::StartRefused {
                owner,
                why: Box::from(why),
                cause: host::StartCause::Other,
            }),
        );
    } else if hand_out_pending(domain, owner) {
        drop(domain.gone.insert(owner, detail).expect("one deferred gone per run"));
    } else {
        gone(domain, env, owner, detail);
    }
}
