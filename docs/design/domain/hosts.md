# Agents and their hosts

Provisional, 2026-10-07, revised 2026-10-23. Where an application's
agents run, and what hosts them: the contract between the core and a
host, the three shapes an application may take, the agent as a
capability of two kinds, both smith's, jig's one host for runs wherever
their agents run, smith's payloads it carries, the application's
workspaces, and how a run's charter, tools, contract, messages and
delivery become smith's. The agent is smith, whose design is its own
(smith's `docs/design/`, cited by path: smith's `domain/run.md`, smith's
`protocol/channel.md`). The core's side of runs is engine.md, sections 7
and 8, and the LLM accounts that pay for them are accounts.md's. What is
still open is listed in section 12.

## 1. In one page

- **smith is the agent.** Nothing in smith assumes code, repositories or
  a workspace, so the same agent serves every application, told apart by
  its charters. jig builds no code into it.
- **The core needs a host for each run, not a worker** (section 2). A
  host starts a run with an assignment, relays messages and calls, keeps
  turns and the answer until the core acknowledges them, and hears how
  the run ends. smith's host contract carries the same (smith's
  `domain/host.md`, section 2).
- **Three shapes** (section 3): no agents; agents inside the engine;
  agents on workers. An application may mix the last two, a charter
  saying which hosts may run it.
- **One host** (section 6): `jig-domain-host`, the hub of hosted runs,
  composed by every root that hosts runs: the engine's, when the
  application runs agents there, and a worker's. It knows a hosted run's
  lifecycle (admission, fencing, relays, turns and the answer kept until
  acknowledged, cancel) and nothing of how the agent runs, where its
  workspace is, or how it reaches the core. Those are its root's and its
  capabilities'.
- **The agent is a capability of two kinds, both smith's** (section 5):
  agent processes, `smith-host-domain`, one process per run; and
  `smith-inline-agent`, smith's domains in the host's own process. Both
  speak smith's parent face. The engine's hub has `smith-inline-agent`; a
  worker's has agent processes, or `smith-inline-agent` by its root's
  choice. jig ships no agent of its own.
- **The link to the core is the root's:** over a channel the worker
  dials, through its protocol layer; or, in the engine, straight into the
  core's fleet, linked from construction and lost only with the engine.
- **Workspaces are the application's** (section 7): a capability of the
  root that prepares what the connectors named, saves unfinished work,
  delivers what a run hands over, and says what the run left. jig defines
  the vocabulary; what a workspace holds is the application's. The engine
  has none, by policy: it runs no commands for its agents.
- **Neither the core nor the hub depends on smith** (6.8). The core
  keeps turns as bytes and its charter in its own terms, which
  `jig-protocol-charter` puts into smith's (section 8); the hub carries
  smith's payloads as bytes, which its root hands out to its protocol
  layer to be decoded for the agent, or encoded from it.
- **A run does what a party does** (section 8). The engine's host tools
  that act on tasks, `introduce` aside, speak a party's operations: a
  run's amendment and release name the revision it was shown, it
  proposes an action beyond its authority, explicitly, and it subscribes
  to the topics a connector offers to agents.
- **Every message ends once** (section 9): read, refused or unread. The
  start carries the messages waiting in the task's inbox; a refusal
  travels inside the run's next turn, or its answer; a message the run
  refused or left unread stays in the inbox and wakes its task again.
- **A run's text comes from a host whose agent gives it** (6.5): every
  host tells a run's activity, without content; a run's text, when its
  assignment asks for it, comes from any host, an agent process or
  `smith-inline-agent`; the calls a run makes, named as they begin, only
  from the second.
- **Committed turns survive.** Turns and answers are kept until the core
  acknowledges them, once committed, and sent again after every hello.
  What a host loses before it was committed (its turns, when a worker
  dies) is done again by the task's next attempt, which starts from the
  last committed turn and is told of the calls committed after it
  (engine.md, 7.1 and 7.3).

## 2. The contract

Between the core's fleet and a host, whatever its shape:

- **Down, from the core:**
  - an **assignment:** the run and its attempt; the workspace's items and
    its workstream key; the charter; when it resumes, the transcript's
    current window and the calls committed since its last turn (6.4);
    the start's messages (section 9); the run's identity; its grants:
    for each LLM account its check chose, its source's credential or a
    program source's name, and the application's credentials its
    workspace needs (engine.md, 7.1); and the **ask** for live content:
    one boolean per kind, `text`, `reasoning` and `input`, which the core
    sets for a run whose task is a conversation (engine.md, 7.1) and the
    hub passes into smith's start (6.2, 6.5);
  - **messages** for the run, named, in order (tasks.md, 8.2);
  - **answers** to the run's relayed calls;
  - **grants** renewed while the run lives: each new generation of a
    credential it holds;
  - to a worker, the **program sources** it may be given, by name, after
    its hello and as one is added or removed; a holder's **check** of a
    program source's login; and a **sign-out** of one, which the worker
    runs (accounts.md, 3.4 and section 5);
  - **acknowledgements** of turns and of the answer, once committed;
  - at most one **cancel.**
- **Up, from the host:**
  - a **hello:** the host's slots, how long it may take to stop a run it
    lost contact for, the workstreams it holds, the runs it hosts and
    those whose turns or answers wait for acknowledgement; and its
    agent's bounds, by which the fleet admits it (engine.md, section 8):
    the limits a run's start must meet (smith's `protocol/limits.md`):
    the models it declares, each with its window and output, among which
    a run's models must be; the most turns, spend, time and waiting a run
    may have; its checks' timeout; the largest message a run takes, and
    how many its inbox holds; and how many turns it keeps unacknowledged.
    A worker's program logins are not in its hello: they follow it, as
    `logins` (below);
  - **turns,** numbered within their attempt, each with its spend, the
    last message it read, whether it opens a window, and the messages
    refused since the run's previous turn, each by name with smith's
    reason (6.4);
  - **calls,** named by the run, each answered once;
  - **facts,** best effort: the run's activity; a run's live content,
    each piece named by its turn, retry, kind and offset, when the
    assignment asked for it and the host's agent gives it; and, from a
    host that has them, a tool begun with its call (6.5);
  - one **answer:** finished with its result, parked, or failed with a
    typed failure; with the turns it took, what it spent, the last
    message it read (the unread fence, section 9), the messages refused
    since its last turn, what it left on resources (the application's
    part), and how its save went;
  - **notices,** relayed from the run's agent: a source *limited*,
    with how long until its limit resets where its LLM provider gave
    that, or *refused*, and a generation *unauthorized* (accounts.md,
    3.5; smith's `protocol/channel.md`, section 4);
  - **logins,** from a worker, after its hello: the state of each
    program source's login on it, as the check of the sources the engine
    sent answered (the fleet places no run on the worker until the
    first), then each one whose check answers otherwise than before, and
    a sign-out's outcome (accounts.md, 3.4; `../protocol/channel.md`,
    4.3);
  - **refusals:** busy, a duplicate among them (6.1), or invalid.
- **Fenced.** Every message names the run's task and attempt; the core
  drops what a stale attempt sends, and a host drops an attempt the core
  has finished with.

## 3. Three shapes

```
no agents           procedures and people only             no host, no smith, no LLM accounts
agents in-engine    the hub and smith-inline-agent         no workspace; tools are host tools only
agents on workers   the hub and agent processes            workspaces, commands, many machines
```

1. **No agents.** An approval pipeline, an operations runbook of
   procedures. The deployment configures no charters; a batch naming an
   agent executor is refused, and the fleet and accounts stay idle.
   Nothing links smith.
2. **Agents in the engine.** A triager, a researcher, a chat assistant,
   whose tools are the engine's and its connectors' reads. The engine's
   root composes the hub and `smith-inline-agent`, with no workspace
   (sections 5 and 6). The costs:
   - no process containment, which is fine when nothing runs commands;
   - the engine's worst case includes its runs;
   - LLM traffic goes through the engine's protocol layer, on its loop.
3. **Agents on workers.** When runs execute commands, hold workspaces, or
   need many machines. Each run is its own agent process, so stopping a
   run ends its process tree. A worker's root may compose
   `smith-inline-agent` instead, for one person's machine: its runs are
   then in the worker's process, with no containment (5.2). Only an
   agent process runs a program backend, such as Claude Code on a
   person's plan, so a program's source pays only for runs on workers
   (5.1).

A charter says which kinds of host may run it, and the fleet places by it
(engine.md, section 8). `ops` is the second shape; an application whose
agents change code in workspaces is the third.

One process may run the engine and its workers, for one person's
machine, development and tests, from the same parts
(`../protocol/service.md`, section 10). Its workers' agents still run as
child processes, as between hosts, spawned by smith's host half; the
agent may be the application's own binary, running smith's agent shell
as a command of its own (section 10).

## 4. Structure

In the engine, when it hosts runs:

```
<application>-domain           the application's root
├── core                       jig's core
├── jig-domain-host            the hub: hosted runs on the engine's own slots
├── smith-inline-agent         the agent, smith's: its domains in this process, one per run in flight
└── …
```

On a worker:

```
<application>-worker           the worker's root: the application's; routes among its children
├── jig-domain-host            the hub: hosted runs, admit, prepare, start, relay, park or end
├── smith-host-domain          the agent, smith's: processes, spawn, channel, watchdog, cancel then kill
└── <workspace>                the application's: prepare, save, deliver, release (section 7)
```

A worker's root may compose `smith-inline-agent` in place of
`smith-host-domain` (5.2); nothing else in its tree changes.

Each tree follows programming-model.md, 4.5. The hub knows a hosted run's
lifecycle and nothing of the application's workspaces, of processes, of
channels or of the core, and links no smith crate; the agent and the
workspace are capabilities, which the root translates to and from with
small total functions, the same functions for both kinds of agent, since
both speak smith's parent face, smith's payloads among them by hand-out
(6.8). A hand-off goes from a capability to the hub, from the hub to a
capability, and at most once more back and forth, so each entry point
completes them before it returns.

A worker's root has the one shape of root.md, section 3, with no store: a
worker keeps nothing durable. In the engine, the hub and
`smith-inline-agent` are children of the engine's root, beside the core:
what the hub sends up goes to the core's fleet, and what the fleet sends
down for the engine's slots goes to the hub (root.md, section 5).

## 5. The agent

The hub's agent capability is one of smith's two crates. Both speak
smith's parent face (`smith_host_domain::parent`; smith's
`domain/host.md`, sections 4 and 9.4), its payloads typed (6.8): down, a
start (what the assignment gives the agent, with the workspace's
directories), messages, answers to calls, acknowledgements of turns,
grants, a stop and a drop; up, in order, message refusals, calls and
withdrawals, turns, waiting, long operations, notices, the unread names,
one answer, and the agent's failures and end, typed. Both refuse a
message at the entrance, typed, with smith's reasons (section 9).

### 5.1 Agent processes

`smith-host-domain` runs each run as an agent process (smith's
`domain/host.md`, section 4). It is a worker's agent capability unless
its root chooses otherwise:

- **one process per run,** its process tree the run's containment
  boundary, its environment free of credentials, which come as grants on
  its channel. A program backend is the agent's child, inside that tree,
  started with its source's configuration directory and no credential
  variable (smith's `protocol/programs.md`, sections 2 and 4.2);
- **one channel** over the process's pipes, whose rules smith's host
  domain enforces, its host half in the worker's protocol layer
  (section 10);
- **a watchdog on progress,** paused while the run waits for a message,
  for a call's answer, or for room to send a turn, and stretched to the
  span of a long operation the run reports; a separate bound on wall
  time;
- **cancel, then kill:** a cancel down the channel first; past the host
  domain's grace, the tree terminated; past its `kill_after`, killed.
  Only an exited process and an empty tree say the agent is gone;
- **LLM completions** are the agent process's own io, with the
  credentials it is granted, or the program's, for a grant that names a
  program source (smith's `protocol/programs.md`, section 3);
- **the programs directory** is the worker's, private to its user, with
  one configuration directory per program source, named by the source,
  holding the program's own login and never a value of jig's
  (accounts.md, 3.4). The worker's host half checks each login, at the
  engine's asking or its own interval, runs a sign-out, and reports each
  answer (section 2). A run's process tree, and a program's own tools,
  run as the worker's user and can read every login there, so until a
  run's tree is contained from the programs directory a worker that
  keeps a person's logins takes only runs that person's accounts pay
  for (engine.md, section 8; accounts.md, 3.4).

The hub's source machine owns when to check: on a new list, an explicit
check and a positive interval. An explicit check queues a fresh command;
requests for that queued command coalesce, while a command already running
finishes independently. Sign-out precedes a pending check.
Commands are serialized across the worker so one command owns the injected
resources and settles before reuse. Its whole first inventory waits for
every source's answer. The protocol layer drives the real host half's
command I/O and deadlines, with an explicit executable, private directory
and bounded environment below the root. Without a configured program, the check fails as unconfigured.
These commands take no run slot. An unconfigured program counts as an
absent login: it cannot sign in or use a program login. A failed check after
an exit or timeout keeps withholding new placement until a later check
establishes absence, because a lost reply does not prove the login absent.
The joint holder check will narrow this restriction to the person's runs.
Shutdown reports an active sign-out's actual result once it settles; only a
queued sign-out that never started fails as closed. Channel loss suppresses
its old-channel reply, while the replacement channel starts with a fresh
whole inventory.

### 5.2 `smith-inline-agent`

`smith-inline-agent` runs smith's domains in the host's own process
(smith's `domain/host.md`, 9.1 to 9.4). It is the engine's hub's agent,
with no workspace, and a worker's by its root's choice:

- **a run is a composed smith domain,** one per run in flight, on the
  hub's slots, not a process: the contract crosses as typed values, with
  no pipe and no frames;
- **it hands its parent** what smith's host domain would, in the same
  order, through the same parent face, so the root translates both the
  same way;
- **a stop** is smith's cancel: the run winds down and answers. Past the
  inline agent's cancel grace, the run is dropped: its relayed calls are
  withdrawn, and what it holds below it is ended. The agent is gone once
  each has its terminal (smith's `domain/host.md`, 9.2);
- **requests below the run go up:** its LLM completions, and with a
  workspace the machine's operations, leave as requests that the root
  routes to its protocol layer, which runs them on smith's components
  with the grants' values (section 10), the run holding only their
  names. Their terminals come back to the run that asked, never through
  the hub;
- **content and facts** are drained from each run in the host's process
  (smith's `domain/host.md`, 9.1), and reach the hub, through the root,
  as a run's text and activity (6.5);
- **no watchdog on progress:** a composed domain's waits are bounded by
  its own deadlines and those of the requests it hands up; a bound on
  wall time stays;
- **bounded:** each run's worst case is smith's, and the inline agent's
  is its slots' sum, part of the root's;
- **no program backend:** it has no child of its own, so it runs the
  direct backend only, and the fleet gives it no grant naming a program
  source (engine.md, section 8; smith's `protocol/programs.md`,
  section 2).

What it gives up is said in smith's `domain/host.md`, 9.3: no process as
the run's containment boundary. That is why the engine runs no commands
for its agents, and why a worker composes it only by its root's choice.

## 6. The hub

`jig-domain-host` hosts runs on a fixed number of slots: a worker's, or
the engine's own, a configured number. It is the same hub in both
places; what differs is its root and its capabilities. One hub, rather
than one per shape, buys:

- **one lifecycle:** admission, fencing, relays, turns and answers kept
  until acknowledged, and cancel are written, reviewed and tested once,
  in one world, with smith's agent processes over skein's fake LLM and
  with smith's real domain; the open questions of section 12 are answered once for
  both shapes;
- **one root shape:** the engine's root and a worker's compose the same
  three kinds of child: the hub, an agent capability, and a workspace
  where there is one;
- **one process when wanted:** the engine and its workers share a
  process (section 3), the workers' agents still child processes, with
  no code of their own.

### 6.1 The link to the core

- **On a worker, the worker dials the engine** and keeps one channel
  open, through its protocol layer. The engine never connects to a
  worker, so a worker can run wherever it can reach the engine
  (`../protocol/channel.md`, 10.1).
- **In the engine, the link is direct:** the engine's root connects the
  hub to the core's fleet from construction, with one hello, and it is
  never lost. A restart settles its claims at once (engine.md,
  section 6); its slots never dial in.
- **Hello first,** as section 2 says, on every link. Facts go only after
  it.
- **One vocabulary toward the link,** section 2's whole, on a worker as
  in the engine: the hello as one value, which a worker's root assembles
  from the hub, its workspace, its agent capability's bounds and its
  link's graces; notices relayed from the agent; facts as outputs, best
  effort, as well as by query. The engine's root translates its own
  hub's link as it translates a worker's (`../protocol/channel.md`,
  section 1), its hello stating the bounds of its `smith-inline-agent`'s
  configuration.
- **The link vocabulary names only what crosses.** It names a run and an
  attempt by their numbers, and a call, a message or a turn by the name
  or number the run or the engine gave it, which mean the same on both
  ends. A handle one side mints for itself (a delivery's token, a
  request's reply handle) is not in it: an assignment is answered by its
  run and attempt, and a root that wants a handle of its own keeps it
  beside them.
- **A relayed call is named by its bytes both ways,** the run's name for
  it as its root had it encoded (`../protocol/channel.md`, section 7),
  its answer and its cancel included. A delivery's token is the hub's own
  handle and never leaves it, so neither a worker's link nor a root that
  composes a hub, the engine's own included, keeps a table from names to
  tokens.
- **Turn busy is not in it.** The engine's answer of busy to a turn is
  the fleet's word to what keeps the backoff below the hub, which arms no
  timers (6.2): a worker's link, or the engine's root for its own hub, on
  its protocol layer's timers. At the backoff's deadline it asks its root
  for the hub's kept copy of the turn, which goes up again as any turn
  (`../protocol/channel.md`, 5.2).
- **The engine holds the claim.** It commits a claim before assigning a
  run, and decides what follows: commit its result, run it again on any
  host, or hold its task. The hub never retries a run and never decides
  that work is done.
- **Attempts are fenced.** Once the engine has cancelled or reassigned an
  attempt, whatever it still sends is dropped; on the hub, a cancelled
  run's calls are answered as unavailable, and an attempt assigned again
  while the hub hosts it is dropped. A fresh assignment for an attempt
  already hosted is refused as busy.

### 6.2 Hosted runs

```
admit ─► prepare ─► start ─► active ⇄ waiting
                               │  turns: kept until acknowledged
                               │  calls: delivery served here; the rest relayed to the engine
                               ▼
                         park or end ─► stop ─► save ─► release ─► answer
cancel, from any state ──────────────► stop ─► save ─► release ─► answer
```

1. **Admit** the assignment, or refuse it at the entrance: busy (every
   slot taken, an answer not yet acknowledged keeping its own; the run
   hosted under another attempt; the host shutting down) or invalid
   (beyond the limits).
2. **Prepare** the workspace from its items (section 7), if the run has
   any and the root has a workspace.
3. **Start** the agent through the agent capability (section 5), with
   what the assignment gives it, smith's payloads decoded by hand-out on
   the way (6.8), and its ask for live content, which goes into smith's
   start as it is (6.5).
4. **Relay** while the run is live:
   - **messages** that arrive after the assignment go down as they
     arrive, each named, in order, a bounded few held until the run is
     live; one the hub cannot hold, or that comes as the run ends, it
     refuses, full or ending (6.4). One that arrives once the answer is
     sent is past its fence, and so unread;
   - **turns** come up as the run ends each, kept until acknowledged
     (6.4);
   - **calls** come up, a bounded number per run and one delivery at a
     time; deliveries are served by the workspace (section 9); every
     other call goes to the engine and its answer comes back. A call the
     run withdraws past its own deadline is answered as withdrawn, and
     may be asked again by the same name once its relay has ended;
   - a run that **waits** holds its slot until its next message, naming
     the last message it has read.
5. **Park or end,** as the run decides. A run parks only once every turn
   it took is sent; its transcript is its state.
6. **Stop:** answer the run's relayed calls as unavailable, and wait until
   the agent capability says the agent is gone (section 5), and until a
   delivery in flight has settled: a delivery cannot be abandoned half
   way, and what it made counts. Nothing touches the workspace while
   anything of the run is still running.
7. **Save** unfinished work, if the assignment asks: at park, at an
   unfinished end and at cancel, where the workspace's items say
   (section 7).
8. **Release** the workspace to the cache.
9. **Answer** the engine, exactly once (section 2). The engine decides
   what happens next, and its acknowledgement frees the slot. A start
   refused, at the entrance (step 1) or by the agent capability (6.3),
   is answered once as refused: the hub frees what it took for it at
   once, keeps no copy and lists it at no hello, and the fleet closes it
   as it hears it, never acknowledging it (engine.md, section 8).

A run with no workspace skips preparing, saving and releasing. A cancel
from the engine, a lost link or shutdown, or a fault of the agent, takes
the same tail: stop, save, release, answer. A run that says how it
finishes as it winds down has that as its answer; only a run that says
nothing is answered as the host stopped it. The hub arms no timers: each
wait is bounded below it.

### 6.3 Failures

Typed, so the engine can act on them without reading prose:

- **refused** at the entrance: busy; or invalid, beyond the hub's
  limits, with smith's parts that do not decode (6.8), or as a start
  smith refuses, naming what (section 8);
- **preparation failed:** transient (the system could not be reached,
  the worker's side failed or ran out of time, another run of the
  workstream holds its workspace), or permanent until something changes
  on a system, naming the resource the workspace says;
- **the run failed,** as it reports it (smith's `domain/run.md`,
  section 10), or because the source of an LLM account it needs turned
  limited, refused or in need of a sign-in, naming the account, which
  is no failure of its task's (tasks.md, 5.5; accounts.md, 3.5);
- **the agent failed:** it could not be started, it exited or was dropped
  without answering, it broke the channel's rules, or the watchdog or its
  bound on wall time stopped it;
- **cancelled:** by the engine, for lost contact, or for shutdown.

Each carries bounded detail for operators; none of it is shown to an LLM.

**A configured bound** is `RunFailure::Exceeded(Exceeded)`, where
`Exceeded { bound: Bound, value: u64 }` names the bound and its configured
value in that bound's unit, never a measured size. The payload is `Copy`
and inline. For `History`, the value is the ceiling met, whose unit the
tag does not identify. Its bounds are:

| Bound | What is bounded | Unit |
|---|---|---|
| `Reasoning` | the model's reasoning, per item or in all | bytes |
| `Metadata` | the model's other replay metadata | bytes |
| `Items` | the items of the model's answer | count |
| `Answer` | the model's answer | bytes |
| `Tools` | the tools offered to the model | count |
| `HistoryItems` | the messages and blocks one request carries | count of both |
| `History` | retained history that compaction could not bring under a configured ceiling | messages, blocks or bytes; the tag does not identify which |
| `Window` | the model's window | tokens |
| `Output` | the model's declared output ceiling | tokens |

The tasks child holds this failure at once (tasks.md, 5.5). It is not an
account wait and is not rewritten to an account Recheck. The host, tasks,
people and client domains each own their `Exceeded` and `Bound`; every
translation between them is exhaustive, without a wildcard.

### 6.4 Turns

- **A turn** is the bytes of one turn of the run's conversation, in
  smith's vocabulary (smith's `domain/session.md`, section 3; smith's
  `protocol/transcript.md`), encoded by hand-out (6.8), numbered within
  its attempt, with what it spent, the last message it read, and whether
  it opens a window: a window turn, or a conversation's first. The hub
  reads only its number, its size, its spend, that name and that flag,
  which come beside the bytes. A resumed run is given its current
  window's turns only (engine.md, 7.2; smith's `domain/host.md`,
  section 6).
- **Refusals ride with turns.** A turn's fence covers every message
  before it, so the core must know of a refused message before it
  commits a turn whose fence passes it, across an engine restart too. No
  refusal crosses on its own: the hub keeps each, its name and smith's
  reason, with the run's next turn, or its answer if none follows, and
  the core records them in the commit that takes the turn's read
  messages (engine.md, 7.2). An agent refuses a message only before a
  fence covers it (smith's `protocol/channel.md`, section 5), and the hub
  makes its own refusals before it relays a later message.
- **Kept until acknowledged,** like an answer: sent at once, sent again
  after every hello, and after a backoff when the engine answers it busy.
  A run may have a bounded number of turns, and bytes, unacknowledged;
  past it, the hub stops taking the run's turns, and the run waits, its
  watchdog paused.
- **The hub holds them, not the agent.** The hub acknowledges a turn to
  its agent capability as soon as it has taken it, and keeps it until the
  engine acknowledges it: for either kind of agent capability, the hub is
  the host that says a turn is safe (smith's `domain/host.md`,
  section 6). Its room is what it lets the agent send: past it, the hub
  takes no more turns and withholds the agent's acknowledgements, so the
  run waits.
- **Stopping never waits for the engine.** Once a run is stopping, the
  hub takes every turn the agent has sent, past its room if need be,
  since the turns the agent may leave unacknowledged are bounded by its
  start, so the agent can be gone (section 5) while the engine is out of
  reach, and the stop bound holds (6.6).
- **Before the answer.** The answer says how many turns the run took,
  and is sent after them.
- **Lost with the worker.** A turn not acknowledged when a worker dies is
  lost with the run; the transcript ends at the last turn committed, and
  the next run starts from there. In the engine, turns die only with the
  engine, and a restart starts from the store.

### 6.5 Facts

What the run reports as it goes (smith's `domain/run.md`, section 11)
goes up to the engine for live views, and, from agent processes, feeds
their watchdog. Forwarding is best effort: a bounded queue, what does not
fit dropped and counted, sent on an open link only, after the hello.
Nothing decides on a fact.

- **Activity, from every host,** carries no content: the run's phase;
  its long operations, each begun with what it is (its checks, an LLM
  completion, a command) and its span, and ended (smith's
  `domain/host.md`, section 2); and counts from smith's channel facts,
  each a kind, its time and a count (smith's `protocol/channel.md`,
  section 8), which the root decodes by hand-out before the hub sees them
  (6.8). Activity is the host's observation, fenced like everything a
  host sends (section 2). The engine shows a run as writing, or as
  running a tool, only on such a fact, never inferring it from an
  attempt being placed.
- **A run's text, at the assignment's ask.** A run whose assignment
  asks for text hands up, as facts, its main conversation's text as the
  model writes it, from any host: pieces, each naming its turn, its
  retry, its kind and its offset (below). In an agent process the pieces
  come up smith's channel, which the host's half decodes (smith's
  `protocol/channel.md`, sections 3 and 4); in `smith-inline-agent`'s
  host they are drained in the process (5.2). A run whose assignment does
  not ask gives none, as every host did before: its words appear as each
  turn commits. A sub-agent's or a compaction's completion gives no text.
- **A tool begun with its call, only from a host that has it:** a hub
  whose agent is `smith-inline-agent` hands up a tool begun naming its
  call and its tool, with the call's input within the views' bound, so a
  run's document shows the call it is making. smith's channel carries no
  such record, so runs in agent processes give none; whether it is asked
  of smith later is open (section 12).
- **A piece names its turn, its retry, its kind and its offset,** whatever
  its source: the turn it belongs to, by its attempt and its number
  within it (an attempt is one activation of smith's run, whose turns
  smith numbers); the retry of the turn's completion, which smith
  retries within the turn, each retry's content starting again at zero;
  its kind: `text`, `reasoning`, or `input`, a tool call's input as it
  streams, named by the call's provider id and its tool; and the offset
  of its first byte within that retry's stream of that kind
  (`../protocol/llm.md`, section 3), counted as smith counts it, in 64
  bits, in the hub's vocabulary and on jig's channel alike. Only `text`
  is produced and kept now: smith builds no other kind, so a host hands
  up none. A piece of another kind that reaches the engine, and one
  whose offset is past the 32 bits the views count, is dropped by the
  engine's root as it hands the piece to the core, counted, until the
  views keep it (engine.md, section 11).
- **No piece after its turn.** A piece of a turn goes up before that
  turn's record; once the turn is sent, none of it follows, and a retry's
  content starts again at offset 0. The engine's views rely on it, and
  keep a run's text by turn, retry and offset (engine.md, section 11).
  The hub holds to it, whatever its agent sends: it forwards a piece
  only from the agent of its run's current attempt, while that attempt
  is live (active or waiting), of a kind its
  assignment asked for, within its bound on a piece's bytes, and of a
  turn it has not yet taken from the agent; and it checks the last
  again as the piece leaves its queue, since its root drains facts at
  its own pace. It drops any other piece, counted apart from the pieces
  it had no room for.
- **Best effort, and gaps.** A piece goes up only while the link leaves
  room for everything else, as every fact does. In the hub, it waits
  with the run's activity in the same bounded queue, and takes room
  there only while half of it is free, so that a run's text never
  crowds out its activity; it is dropped and counted otherwise. smith
  holds a bounded amount of unsent content and drops the rest, counted
  (smith's
  `protocol/channel.md`, section 7). A drop shows as a gap, a piece
  whose offset is past the last one's end, which the views mark as cut.
- **One text, whatever its host.** Above the hub, the core's views, the
  client and the application see a run's text the same way from an agent
  process and from `smith-inline-agent`.
- **What jig still asks of smith, later:** the LLM component's abort and
  closed terminal (`../protocol/README.md`, section 11).
- **A conversation's delegated tasks give no text.** Only the run of the
  conversation's own task is asked; a task it delegates streams nothing
  (section 12).

### 6.6 Losing contact

On a worker; in the engine the link is never lost, and none of this
happens.

- **Runs go on for a grace** if the channel drops, and what they send
  waits, never dropped: turns, with the refusals they carry, and relays,
  unless their call has closed meanwhile. On reconnecting, the worker
  says what it hosts and the engine keeps or cancels each run.
- **Past the grace,** the hub cancels them itself, saving their work
  first, and keeps their turns and answers for the next channel.
- **The stop bound** the worker says at its hello is the longest it may
  take to stop a run it lost contact for, counted from the last moment
  the engine may have heard from it: the time its link takes to notice
  the loss, then its grace, then the longer of the time its agent
  capability may take to be gone after a stop (5.1 and 5.2) and a
  delivery's deadline, then a save. The engine's grace, counted from when
  the engine notices the loss, is strictly longer (engine.md, section 8),
  so no next attempt starts while this one may still write, however each
  side's link notices a partition (`../protocol/channel.md`, 10.1).

### 6.7 Shutting down

Shutting down cancels every run and admits no more. The host is done
once the engine has every turn and answer. A worker gives up what it
keeps, counted, only once no run is left and the engine is still out of
reach past the grace.

### 6.8 smith's payloads

- **Bytes in the hub.** The hub links no smith crate. It carries as
  bytes it never reads the charter, a resumed run's transcript, each
  turn's body, call names and a run's result, and the run's identity,
  reading only what section 2 puts beside them. A host tool's input
  stays the bytes the LLM wrote, for the engine's protocol layer to
  decode (section 8).
- **Typed in the agent.** smith's agent capabilities take and give them
  typed, through smith's parent face, never encoding or decoding them
  (smith's `domain/host.md`, sections 3 and 9.4).
- **Translated by hand-out.** The root that composes the hub with an
  agent capability, a worker's or the engine's for its own hub, hands
  each payload out to its protocol layer, whose routing decodes or
  encodes it with smith's codecs (`smith-protocol-channel`'s functions,
  and `smith-channel`'s records for a call's name and a fact), keeping
  no state,
  and hands it back as an event of the root, in the order it went out:
  the path of host tools' calls (`../protocol/channel.md`, 6.3).
  - **Down,** the charter, the transcript and the committed calls'
    names are decoded, with the endpoints of the agent's configuration,
    which resolve those the charter names, and smith's start goes to the
    agent typed. One that does not decode fails the start as invalid
    (6.3).
  - **Up,** each turn, call name and result is encoded for the hub, and
    each of smith's channel facts is decoded into jig's typed activity
    (6.5), so the hub and jig's channel carry jig's activity, never
    smith's facts. Each live piece from smith's channel goes to the hub
    as jig's typed piece, and to the agent process's watchdog as its
    progress, as before (5.1).
- **Who links smith:** `jig-protocol-charter` (section 8), a client's
  decoding of turns (`../client/domain/client.md`, section 10), and an
  application's roots that compose smith's agent capability, with their
  protocol layers' routing, which also runs smith's components where it
  composes them. Never the hub, the core, or jig's channel's components,
  which pass smith's bytes through (`../protocol/channel.md`, 6.1).

## 7. Workspaces

A workspace is what a run works in, prepared from the items the
connectors named (connectors.md, section 10). Its kinds are the
application's: a capability of the root that composes the hub, speaking
jig's workspace vocabulary with the hub, through the root:

- **prepare** these items under this workstream key: answered ready,
  with what the agent is to be told of it (directories, names, what may
  be written, a merge to resolve), or failed, transient or permanent,
  naming the resource;
- **deliver** what the run hands over, from its `deliver` or its
  `finish` (section 9): answered with the delivery's outcome in smith's
  terms, and, for the engine, what it left on the resources;
- **save** unfinished work where the items say: answered saved, with
  where, or failed;
- **release** the workspace to its cache, or **discard** it when it may
  be damaged.

- **A cache, keyed by workstream:** the runs of one piece of work find
  their workspace ready. A workspace is reused only if it holds exactly
  the items named, and is reset to their starting point: nothing local is
  authoritative.
- **One operation at a time** per workspace, each bounded by deadlines
  given as data; the workspace arms no timers.
- **No credentials on disk:** what reaches a system names an identity,
  which the protocol layer maps to credentials per operation.
- **A run with no items** needs no workspace, and its host prepares
  nothing; a worker may still run it, for containment.
- **None in the engine,** by policy: the engine's root composes no
  workspace, and a charter whose runs need one is placed on workers only.

## 8. Charters, tools and contracts

The core decides a run's charter in its own terms (engine.md, 7.1). Its
translation into smith's charter (smith's `domain/run.md`, 3.1; smith's
`protocol/charter.md`, section 2; the version,
`../protocol/README.md`, section 11), is in one place,
`jig-protocol-charter`, a function of jig's that links smith, which the
engine's protocol layer calls for every run, on workers and in the
engine alike, and which reads every run's result back by the path of
host tools' calls (6.8).

- **Instructions and brief:** the charter's instructions, and the brief's
  sections rendered as smith's titled sections, in order, the
  connectors' sections among them as their connectors render them.
- **Tools:** the task's tool families become smith's:

  | jig's family | In smith |
  |---|---|
  | inspecting, modifying, the shell | smith's inspect, modify and shell, when the run has a workspace; on a program backend, the program's built-in tools of these families, each call asked of smith's policy through the program's permission prompt (smith's `protocol/programs.md`, 5.3 and 5.4) |
  | sub-agents | smith's sub-agents, on the direct backend; none on a program backend (smith's `protocol/programs.md`, section 2) |
  | the engine's tools (engine.md, 7.3) | host tools, each declared with its schema, its description and whether it writes |
  | each connector's reads, and the effects and topics it offers to agents | host tools, declared by the application; its effects through the engine's `effect`, its topics through `subscribe` |
  | waiting, by its wake policy | smith's `wait` |
  | a delivery, by its result contract or mid-run | smith's `deliver`, when the run has a workspace that delivers |

  Each host tool's schema and description are written once, with the
  function that decodes its calls and the one that renders its answers;
  a call's input is decoded into the core's or the connector's typed
  request, or answered as an error naming what is wrong. The engine's
  protocol layer decodes every call the fleet has admitted, from workers
  and from the engine's own hub alike, so no domain parses a call's input
  (`../protocol/channel.md`, 6.3).
- **The engine's tools that act on tasks, `introduce` aside, speak a
  party's operations** (engine.md, 7.3; people.md, 5.1). Each host
  tool's input names what its operation names, as the run was shown it:
  - `delegate`: a batch, each task with what a party's `Create` names
    (tasks.md, section 4);
  - `amend`: the task, its spec's revision, and the change; a change of
    priority alone names none;
  - `release`: the task, its hold's revision, and, for an effect held
    uncertain, whether to make it again;
  - `cancel`: the task, a reason, and a grace, if the run gives one;
  - `decide`: a proposal by its number, or an escalation with its hold's
    revision, and the choice, with a reason;
  - `propose`: the action, with a reason: a batch; an amendment, naming
    the revision as `amend` does; an effect of a kind a connector offers
    to agents; or a widening of the task's own authority;
  - `withdraw`: the proposal;
  - `subscribe`, `unsubscribe`, a task's own: a topic a connector offers
    to agents, a referenced task's state, or a timer.
- **A revision is shown before it is named** (engine.md, 7.3): this
  translation renders each revision where the run sees it, and a call
  answered moved with what the task is now.
- **A connector's topics offered to agents** are declared by the
  application with its connector's host tools, each with its schema and
  description, as its offered effects are. A `subscribe` call naming one
  is decoded by the engine's protocol layer into the connector's typed
  topic, and checked as a read is: the task's tools hold `subscribe`, and
  a read grant covers the resources the connector says the topic
  concerns (authority.md, 8.5; connectors.md, 5.3).
- **Result contracts** (tasks.md, section 3) become smith's (smith's
  `domain/run.md`, 7.1): a report; a verdict, with its labels, fields and
  follow-ups; a connector's result, as a delivery the run must make
  through its workspace before it finishes, with the fields its workspace
  asks for; a failure. The core judges a result again as it commits it
  (tasks.md, 5.6).
- **Budget, models, waiting:** the run's budget, its turns, spend and
  time, its spend reserved from its task at its claim, in the
  deployment's unit, and its reserve of turns and time for finishing
  (smith's `domain/run.md`, 9.1); its models, from its task's LLM choice
  (tasks.md, section 3), the main session's and those its sub-agents may
  use, each with its effort, on an endpoint named as the agent's
  configuration names it, with its window, its output and its prices
  (engine.md, 7.1); for each model's LLM provider, the account its check
  chose, whose grant says the source, and so the backend, since an
  endpoint names no account (accounts.md, 3.4; smith's
  `protocol/programs.md`, section 3); the charter's waiting time, and
  whether a run resumes its transcript or starts fresh.
- **What jig asks of smith's budget** (smith's `domain/run.md`, 9 and
  9.1): before each completion, its sub-agents' included, smith reserves
  that completion's most (its input at the highest input rate, its
  output at the output rate) from the run's budget, and returns the
  difference when the completion ends. When the ordinary part of the
  budget is spent, the run winds down on its reserve and finishes; a
  completion that does not fit what is left at all ends the run for its
  budget. All of it is inside the run, with no call to its host, so a
  run never spends past its budget (authority.md, section 7). On a
  program backend, which makes its own completions, smith cannot reserve
  one first: the program may pass the ordinary part by one completion of
  its main model, which the reserve takes, and the run stops at its next
  turn (smith's `protocol/programs.md`, 5.7 and section 7). Such a run's
  reserve holds one more of those completions (authority.md, section 7),
  so it never spends past its budget either.
- **The core places no run smith would refuse.** A task whose budget
  cannot hold its run's reserve and one of its main model's most costly
  completions, and on a program backend one more again, is held for its
  budget (authority.md, 7 and 8.3); the fleet admits no host whose agent
  cannot run the engine's charters, and places a run only on a host that
  declares its models and where its sources are usable (engine.md,
  section 8). When a change of settings changes the charters (engine.md,
  17.3), the fleet judges its connected hosts afresh from what each
  declared at its hello, kept for the deployment's configured models
  only: a host now short of a charter stays connected, its running
  attempts with it, and takes none of that charter's new runs; its next
  hello is admitted or refused as any is. The same admission decision is public as
  `jig_domain_fleet::shortfall`, using the core's owned
  `fleet_needs::configured` requirements, so an application's startup can
  diagnose its prospective host before it says hello. A start smith refuses
  all the same fails its
  attempt as invalid, naming why (6.3).

### 8.1 Open compatibility

A task's ending (tasks.md, 5.8) is smith's mode for its run: a task its
result ends runs `Finite`, and one its requester ends runs `Open`. An
Open run is granted the wait and never delivery.

An Open task's ordinary program path is refused before preparation, at
selection and again at the claim's recheck, alternative models and
grants judged by the same view. The refusal chooses no other source or
account and changes no task's ending. A wait for an account or a source
comes before it.

Each admitted hello declares the program endpoint numbers its channel
serves, the engine host's among them, fixed for that channel. Unknown
evidence fails closed: an Open start is refused on it. At most 64 distinct
numbers are admitted, whatever the number of models: the cap is kept
with the hello's other bounds, and the worker wire's ceiling is the
same. The wire and the domain each refuse a longer list, or one naming
a number twice, before keeping it, and a malformed list refuses the
channel without counting an agent short.

A refused agent start carries its typed cause, `StartCause::{Other,
Grants}`, apart from the operator's bytes. A run's end is rechecked,
its attempt released and no try spent, if and only if all of these
hold:

- its start was refused with the Grants cause;
- its saved work and its result passed their fences;
- it spent nothing and committed no turn;
- its task's ordinary program source, on its main binding or an
  alternative, was offered.

Whatever class the host's refusal was given, the recheck is the same;
the selection then refuses an unsupported start again. A structural
invalid from the host, a program endpoint not proven, and a run that
committed keep their ordinary handling.

## 9. Messages and delivery

- **Messages** go to smith as messages, rendered by the same translation
  (section 8): a sender's label (its requester, a delegate by number, a
  party by name, a subscription's topic) and the message's words, whole.
  Proposals and escalations waiting for the task are relayed the same
  way. What a decision about a task names travels with it: a notice of
  a delegate held, and an escalation, carry the hold's revision
  (section 8). The run names the last it read with each turn and when it
  waits. A proposal or an escalation relayed to a run and decided before
  its turn commits, often by a call in that very turn, still ends read
  when the turn names it (tasks.md, section 9).
- **A grace asks the run to finish.** Jig keeps `FinishRequested { revision }`
  as a durable inbox kind, separately reserved from merged amendments. Its
  live message and its carried Start message translate to smith's
  `finish_requested: true`; every other kind translates to false. The
  owned kind travels through held deliveries, without querying later task
  state. In an Open run, admission enters smith's existing wind-down, so
  its next text parks. Finite runs hear an ordinary amendment; cancellation,
  refusals and read fences keep their existing meaning. The hub carries the
  flag without linking smith; application roots only translate it.
- **The start carries the inbox,** as many of its messages as the run's
  inbox takes, oldest first; the rest are relayed as the run reads
  (tasks.md, 8.2). The run opens on them (smith's `domain/host.md`,
  section 2).
- **Every message fits** a run's message bound (tasks.md, 8.2), and the
  fleet admits no host whose agent takes less (engine.md, section 8), so
  no run refuses a message as too large.
- **Each message ends once** (smith's `domain/host.md`, section 2):
  - **read:** a turn's last message read names it or a later one;
  - **refused:** by the hub or by the agent, by name, with smith's
    reason: too large, the run's inbox full, a name in use, or the run
    ending; the refusal travels inside the run's next turn, or its
    answer if no turn follows (6.4);
  - **unread:** the run answered without reading it, which the answer's
    last message read, the unread fence, says: every message relayed
    after it, and not refused, went unread.
- **Refused or unread, it stays** in the task's inbox, untaken, and
  wakes the task again when the run ends, as it would had it arrived
  then (tasks.md, 8.2 and 8.3). Nothing a host does drops a message.
- **Delivery** is the workspace's: smith's `deliver` is served by the hub
  through the application's workspace (section 7), which makes it. Its
  outcome is smith's (smith's `domain/run.md`, 8.2): delivered, nothing,
  stale, refused, failed. A delivery that was made is the run's result
  even if the run was winding down; what it left on resources reaches
  their connectors with the answer.

## 10. Below the domain

- **The engine's channel,** jig's: framed, dialled by the worker and
  authenticated, carrying the contract of section 2, versioned.
- **Agent processes:** spawning smith's agent in a contained process
  tree, with proof that the tree is empty after it stops; smith's channel
  over its pipes, `smith-channel` its codec, its host half
  `smith-host-protocol`, a component of the worker's protocol layer
  (smith's `protocol/hosts.md`, section 2). The agent may be a command of
  the application's own binary that runs smith's agent shell
  (`smith-agent-shell`).
- **Workspaces' io:** the application's.
- **In a host's process:** `smith-inline-agent`'s LLM completions and,
  with a workspace, the machine's operations, on smith's components
  (`smith-protocol-llm`, `smith-protocol-machine`) in the protocol layer
  of the process that runs it, each slot's result contract kept beside
  them (`../protocol/llm.md`, section 3).
- **smith's payloads, by hand-out:** the protocol layer's routing under
  each root that composes an agent capability (6.8).

## 11. The world

- **The hub's world:** its root scripted; an engine that assigns,
  relays, cancels and acknowledges once its fake store commits; a link
  that drops for less and for more than the grace, and one that never
  drops, as in the engine; agent capabilities of both kinds, smith's:
  `smith-host-domain` over smith's agent processes over skein's fake LLM,
  which
  progress, hand up a conversation's text in pieces at their
  assignment's ask, call, take turns, some opening a window, wait, park
  and end,
  refuse messages and leave them unread, but also hang, crash, break
  rules and ignore cancels; and `smith-inline-agent`'s runs, smith's real
  domain over skein's fake LLM, which answer, wait, park and resume from
  their current window, stream text, are cancelled mid-turn, and ignore
  a cancel past the grace, to be dropped; and a scripted workspace that
  prepares, delivers, saves and fails, or none. It checks that every
  turn and answer reaches the engine once and is acknowledged once,
  every relayed call is answered once, every message relayed ends once,
  read, refused or unread, each refusal inside a turn or answer no later
  than the first whose fence passes it, nothing touches a workspace
  while a run's agent may, no piece goes up after its turn or for a kind
  its assignment did not ask for, and nothing is live once it settles.
- **The application's worlds** run its hosts, with its own workspace or
  none, against jig's core on the conformance world (testing.md,
  section 5).

## 12. Open questions

- **A delegated task's text.** A task a conversation delegates streams no
  text for now (6.5). Later the conversation's run could ask for it from
  its parent, so the person sees the delegate write; whether that asks
  of the assignment, or of the views, is open.
- **A tool's input and reasoning, as they stream.** Their kinds are in
  the wire (6.5) and have no producer: each waits for smith to build it
  and, for reasoning, for the LLM providers to be asked for readable
  reasoning.

- **Fairness on the engine's loop:** whether runs in the engine need a
  share of each iteration, so many chats never slow the engine's
  decisions.
- **Saving periodically,** so a dying worker loses less than the work
  since the run's last save.
- **Before the first channel:** whether a worker's failed dials, before
  it ever reached the engine, want a grace of their own.
- **The hand-out's cost in the engine.** smith's payloads cross the hub
  as bytes (6.8), so for runs in the engine some are encoded and decoded
  once more than they need to be: the charter, encoded by
  `jig-protocol-charter` and decoded again for `smith-inline-agent`; a
  result, encoded for the hub and read back by `jig-protocol-charter`.
  Each turn is encoded once, as the core keeps it anyway. Whether that
  ever shows, and if so whether the hub carries tokens its root
  resolves.
- **Workspaces in the engine.** Whether a deployment may give in-engine
  runs a workspace, for one person's machine, knowing it has no
  containment; the mechanism would allow it.
- **Containing a run from the programs directory,** so that a worker
  keeping several people's program logins may take each one's runs:
  accounts.md, section 7.
- **Agents other than smith:** the contract of section 2 is jig's; an
  agent that spoke it through a capability of its own could run beside
  smith. Not wanted yet.
