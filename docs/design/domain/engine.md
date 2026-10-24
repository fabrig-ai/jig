# The core

Provisional, 2026-10-07, revised 2026-10-24. What jig's core is, as a
domain layer inside an application's engine: its parts and how they fit,
what crosses between it and the application's root, how a decision
reaches the store before anything leaves the engine, how it restarts,
how it runs agents, the capabilities it keeps (the fleet, briefs,
notes, views, accounts), and its settings, changed at runtime. The model is core.md; tasks are tasks.md,
authority authority.md, connectors connectors.md, parties people.md,
hosts hosts.md, LLM accounts accounts.md; what the application's root
does with all of it is root.md. The mechanics are those of skein's
`programming-model.md`. What is still open is listed in section 16.

## 1. In one page

- **The core is where work is decided.** It keeps the tasks, checks
  every action against authority, starts agents' runs on hosts, asks
  parties, and tells the application's connectors what to make, judge,
  step, project and release. It is a child of the application's root,
  and the parent of its own children.
- **A parent and its children.** The tasks child domain is the center
  of the core, keeping every live task; authority is policy; the fleet,
  people, accounts, briefs, notes and views are capabilities. The core
  routes between them and faces the root (section 3).
- **It speaks one vocabulary to the root** (section 4): the store's
  records, the hosts', the parties', the accounts', and the connectors'
  in jig's terms. It knows connectors by number.
- **A decision commits once,** and nothing leaves before it does
  (section 5). The core says, of every output, whether it waits for the
  decision's commit; the root's journal holds it until it is durable.
- **A restart is the core's script** (section 6): its own records, then
  each connector's, then the runs adopted, the connectors reading afresh
  and settling their outboxes, and only then decisions.
- **Agents run from their task** (section 7). A run is given its task's
  charter, a brief built for why it runs, the messages waiting in its
  task's inbox, its transcript's current window if it resumes, and the
  engine's tools its authority allows. Each turn it takes is
  committed as it ends; every call that changed something is committed
  and answered once, however often it is asked.
- **It computes what only it can** (section 11): the documents parties
  watch and read, from committed state: `me`, the signed-in party's own;
  task queries, over the live tasks it holds and the ended tasks' indexes
  in the store; what each task waits on, from its tree and the entries
  waiting in it; what a decision would do, as data. A conversation is
  known by its flag, never by its charter, and a template is served,
  never used.
- **Bounded.** The core holds live work only: live tasks, their inboxes
  and proposals, the runs in flight. Ended tasks, transcripts, history
  and notes are in the store, loaded when wanted.
- **Settings are the binary's defaults with the store's changes over
  them** (section 17): charters, templates, the policies projects are
  made from, the rules, and an application's own kinds. An owner's
  change is one decision and makes a revision; a task records the
  revision it was made in, and its runs read their charter from it.
- **The domain is complete** (programming-model.md, section 4): a world
  of domains and fakes runs everything the core does, with no protocol
  and no io (section 15).

## 2. The core in the engine

```
application root        the journal, routing, translation; faces the protocol layer
├── core                tasks, authority, people, fleet, accounts, brief, notes, views
├── <connectors>        the application's
├── jig-domain-host     jig's hub, for runs on the engine's own slots, if the application runs agents there (hosts.md)
├── smith-inline-agent  the hub's agent capability in the engine: smith's domains in this process, with no workspace
└── …                   the application's own children
```

- **The root faces the protocol layer,** and the core faces the root.
  Everything the core hears arrives through the root, already in the
  core's vocabulary; everything it asks for leaves through the root,
  which translates it and puts it into the journal (root.md).
- **Hosts are reached through the root:** workers over the channel, which
  the protocol layer carries; the engine's own hub as a sibling the root
  routes to (hosts.md, 6.1), its agent smith's `smith-inline-agent`
  (hosts.md, section 5). Both speak the same vocabulary. The core's
  fleet does not tell them apart beyond what placement needs.
- **The engine never reads a system** but through its connectors; what a
  workspace holds is for the run to find.
- **The store is the engine's alone.** Nothing else reads or writes it,
  so the engine needs no compare-and-swap against anyone: its own
  ordering of commits is the truth.

## 3. Structure

```
jig-domain-core             the core: routing among its children, runs, calls, the restart script, numbers
├── jig-domain-tasks        the center of the core: tasks, batches, lifecycle, holds, inboxes, wakes, proposals
├── jig-domain-authority    authority's order and checks; the rules and the projects' policies
├── jig-domain-people       parties: identities, sign-ins, roles, requests, inboxes
├── jig-domain-fleet        hosts: slots, placement, attempts, relaying
├── jig-domain-accounts     LLM accounts and an application's credentials: sources, states, uses, granted
├── jig-domain-brief        a run's brief, from typed sections and connectors' sized ones, within a budget
├── jig-domain-notes        notes: scopes, entries, indexes
└── jig-domain-views        parties' watches, level-triggered

jig-domain-workers         shared checked names and worker membership values (root.md, 2)
```

The tree follows programming-model.md, 4.5: each child domain has its
own vocabulary, limits and world; a parent owns its children's state and
routes between them; siblings share no domain types, and the core
translates between them with small total functions.

- **`tasks` is the center of the core:** it keeps every live task, and
  knows a task's lifecycle and nothing of runs, connectors or clients.
- **`authority` is policy:** decisions over data, keeping only the rules
  and policies in force.
- **The others are capabilities,** each knowing its own concern: the
  fleet the hosts, people the parties, accounts the LLM accounts that pay
  for runs and the credentials runs are lent, the brief a run's context,
  notes what agents learn, views who watches.
- **The core keeps little of its own:**
  - the deployment's numbers: the next task number and every other number
    the core gives, committed as they are used;
  - the runs in flight: for each, its task's number, its attempt, and
    what its assignment was made from, so an answer can be routed and a
    brief rebuilt (section 7);
  - for each live agent task, its conversation's first attempt and where
    its current window starts (7.1 and 7.2);
  - the calls committed for each live task since its last committed turn,
    by attempt and name, with the core's part of their answers (7.3);
  - the decisions in progress, between the hand-offs the root completes
    within one step (4.3);
  - the restart script's state (section 6);
  - the settings: the defaults, the changes over them, the held
    revisions and their charter versions (section 17).

## 4. Its vocabulary toward the root

### 4.1 What it hears

- **from the store,** through the root: the answers to its loads, in
  pages; the records restored at a restart; that the loading it asked for
  is done. It does not hear commits' answers: the journal does (root.md,
  section 4), and feeds back what the core asked to resume after one;
- **from hosts:** a host's hello with its slots, what it hosts and its
  agent's bounds; turns, with the messages a run refused, calls, answers
  and facts of runs; notices relayed from a run's agent, a source
  limited (with how long until its limit resets, where its LLM provider
  gives that, which the core makes a time on its clock as it hears it)
  or refused, or a generation unauthorized (accounts.md, 3.5); each
  program source's login on a worker, which follows its hello and which
  its half checks (section 8); a host lost or back;
- **from parties:** sign-ins and their ends, a credential to mint for an
  identity the application's server verified (people.md, section 3);
  keyed requests, each one operation or a compound request of several
  (people.md, 5.1); reads, and watches opened and closed (section 11);
- **from accounts' protocol:** a generation granted, a failure, a
  sign-in's address for the client that asked and its subject once
  signed in, a revocation's outcome, the component closed
  (accounts.md, 3.5 and section 5);
- **from the application,** through its root: each generation of an
  application's credential granted (12.1); a project's period begun,
  saying when the next begins if its source knows. The core keeps no
  calendar, infers no cadence and schedules nothing of its own
  (authority.md, section 7);
- **from connectors,** each by number, in jig's terms (connectors.md,
  section 2): resources' roles and kinds of hold; pools' slots; effects'
  descriptions; verdicts; procedures' step decisions; outbox outcomes;
  classified news for subscribers; sections and workspace items ready,
  with their sizes; adoptions; drift; a restart step done;
- **from itself:** an output it asked to resume once released (4.2), and
  its children's timers, through `fire`.

### 4.2 What it asks for

Each request the core makes says which of three it is:

- **part of the decision:** a record saved or erased, which goes into
  the decision's commit; and the hand-offs the root completes before the
  step returns: describe this effect, judge this requirement, keep or
  drop this effect, take this procedure's parameters, gather or cut this
  section;
- **held:** what must wait for the decision's commit to be durable: an
  assignment, a message relayed to a run, a cancel, an acknowledgement,
  an answer to a call or to a party, a load (so that it sees every commit
  before it), an outbox entry to make, a resumption of the core's own; and
  a party's read's answer and a watch's delivery, behind the last commit
  made (5.2);
- **now:** what decides nothing and shows nothing durable: a run's text
  streamed to watchers, the answer to a run's read, a run's admitted call
  handed out to be decoded (4.4), a host's refusal before anything was
  admitted.

The root keeps each as the core said (root.md, section 3); the journal
releases held ones in order, and the conformance world checks that
nothing held left early (testing.md, section 5).

### 4.3 Decisions and hand-offs

A decision is a route through several children and connectors, completed
within one step of the root's (programming-model.md, 4.5):

- **One event in,** from the root.
- **Hand-offs out and back:** the core asks the root for what only a
  connector can say (a description, a verdict, a section's size), and
  the root asks the connector and gives the answer back to the core,
  before its step returns. Hand-offs are bounded per decision and
  acyclic: the core never asks a connector something whose answer needs
  another of the core's answers.
- **One end:** the core says the decision is complete, and the root
  accepts it into the journal (root.md, section 3).

What must first wait for a load, a read or a host is a state of its own
(a task preparing, a brief gathering), and its decision is made in the
step its last answer arrives.

### 4.4 The usual routes

| What arrives | Route | Then |
|---|---|---|
| a run's turn | fleet (fenced) → tasks (the inbox taken) → authority (spend) → views | commit; the turn acknowledged |
| a run's call to an engine tool | fleet → authority (the check) → tasks or notes | commit; the call answered |
| a run's call for an effect | fleet → the connector describes it → authority, its judges asked → the connector keeps it | commit; answered when made, or past its deadline |
| a run's call for a read | fleet → authority (the grant) → the connector serves it | answered now; nothing committed |
| a run's answer | fleet → tasks (result checked, task closing or idle; the messages it refused or left unread stay, and wake it by its policy) → authority (spend); what it left → its connectors | commit; acknowledged; effects made |
| a procedure's step | its connector → authority (each effect, its judges; each batch) → tasks | commit; effects made; tasks started |
| a connector's news | classified by the connector → tasks (inboxes, wakes) | commit; due tasks run, procedures step |
| an outbox outcome | its connector → tasks (settled; closing advances) | commit |
| a party's request | people → authority (their role and pool; a budget given `AsAllowed` resolved; a deadline given as a span made a wall) → tasks | commit; the party answered, with what an `AsAllowed` budget was given |
| a party's compound request | people (one project, within operations per request) → for each operation in order: authority, one pool counted across them → tasks | one commit, or none; one answer: all made, or the first operation refused and why |
| a project made (a party's `MakeProject`, or the deployment's act) | people (the maker an owner of the deployment) → the core (the name in shape and free, a configured policy, room; the next number) → authority (the policy copied) → people (the maker its owner) → tasks (its first period, with no next start) | commit; the maker answered; the root hears the project made |
| a task due | tasks → authority (the run may start) → brief, with its connectors gathering, and its transcript's current window loading → tasks (claim, with its inbox's messages for the start) → fleet (place) | commit the claim; assign |
| a task closing | tasks → its connectors (release) | commit; releases made |
| a goal changing | tasks → its projecting connectors | commit; projections made |
| a project's period | authority (its spend within the policy) → tasks (the period's funding opened; recurring tasks' batches; standing allotments renewed) | commit; tasks started |
| a timer | its child | as its child says |

- **A run's call is fenced, then decoded.** The fleet admits a call only
  from its task's live attempt; the core hands the admitted call out, and
  the engine's protocol layer decodes its input and gives the typed
  request back to the root, which routes it as the rows above say
  (`../protocol/channel.md`, 6.3; root.md, section 5). No domain parses a
  call's input.
- **A budget given `AsAllowed`** is resolved by authority as the batch is
  admitted (authority.md, section 7). A pool's amount left is a live
  number only the core holds, which no client could read and send back
  without racing. The task keeps the amount, and the answer, a party's or
  a run's, says what was given.
- **A compound request is one decision** (people.md, 5.1): the route
  above runs once per operation, in order, within one step of the root's,
  and its hand-offs are bounded by its operations (section 13).

## 5. Decisions and the store

### 5.1 Commits

- **One decision, one commit.** What a decision changes in every child
  and connector it touched is one commit: records written or removed, as
  one.
- **Numbered and in order.** The journal numbers commits and the store
  applies them in that order. Its answer is that a commit is durable,
  which makes every earlier one durable too.
- **The state runs ahead.** A child's state changes as the decision is
  made, not when it commits, so the next decision sees it.
- **Commits in flight are bounded,** and the store's answers are always
  taken, so the bound never stops the answer that frees it. Past the
  bound, a decision that would need a commit is not taken, and what asked
  for it is told so: a party's request is answered busy; a run's tool
  call is answered busy, and the run asks it again after a backoff
  (smith's `domain/run.md`, 5.2); a turn or an answer is answered busy,
  and its host sends it again after a backoff; a connector keeps its
  events and its outbox's outcomes until there is room. The journal says
  whether there is room before any child changes (root.md, section 4).
- **A failed commit stops the engine** (core.md, section 4). Nothing
  waiting for that commit, or any later one, goes out. A restart starts
  from the last durable commit.

### 5.2 What waits for a commit

Every output that someone outside the engine could act on waits for the
commit it follows from:

- **effects:** an outbox entry is handed to its connector to make
  (connectors.md, section 4);
- **assignments:** a run is assigned only once its claim is durable, so
  an attempt never runs that a restart would not know of;
- **acknowledgements:** a host's answer, and a run's turn, are
  acknowledged once committed; until then the host keeps them;
- **answers to tool calls** that changed state, and to parties'
  requests;
- **messages relayed to a live run,** so a run never reads what a
  restart would not have;
- **loads,** so a load sees every commit before it;
- **what parties read and watch:** a read's answer and a watch's
  delivery decide nothing, but are made from state that may run ahead of
  the store, so each waits for the last commit made before it, as an
  output of a decision with no commit of its own (section 11);
- **grants:** a generation sent to an attempt (section 12) decides
  nothing, but goes to the attempts the fleet keeps, which run ahead of
  the store: one whose assignment waits for its commit is an attempt a
  restart would not know of, and its host does not know it yet either.
  So a grant waits for the last commit made before it, and leaves after
  its attempt's assignment, since held outputs leave in order
  (`../README.md`, 6.2).

What decides nothing and shows nothing durable waits for nothing: a
run's text streamed to watchers as it is written (section 11), and the
answers to a run's reads.

### 5.3 Loads

- **What is loaded, not held:** ended tasks and their results;
  transcripts; history; notes' entries; anything a client pages through.
- **A load is a request** with a token and one terminal event, rows in
  bounded pages (programming-model.md, 4.4). What waits on it is in a
  state that says so: a brief gathering, a run being prepared to resume,
  a party's page, an amendment adding an ended task (tasks.md, section
  7). That amendment's request stays unanswered while the rows load,
  one at a time: each ended task it adds, then the ended tasks above it
  that its reach is read through, at most one per level of the tree. Once
  they are in, it is admitted again from the start, on the rows and on
  live state as it then is: the pass that found them missing answered
  nothing and committed nothing. If what waits goes first (its run ends,
  its party leaves, the engine restarts), its load goes with it, and its
  request is left as any unanswered one is.
- **What a decision loads stays as loaded until it is decided.** Its
  loads are held (4.2): each is sent once every commit made before it is
  durable, so it sees them. While they are in flight, a request whose
  decision would change what they load is answered busy (people.md,
  section 6, a mark on an ended conversation). Otherwise two decisions
  that each loaded one record before either changed it would both decide
  on the old record, and the later commit would undo the earlier.
- **Bounded:** the loads in flight, and the bytes a load brings, are
  limits; a load past its bytes is cut, and says how much.
- **Parties' reads** are documents and pages the client's contract lists
  (`../client/domain/contract.md`, section 6): from live state where the
  engine holds it, loaded otherwise, and answered once the last commit
  made is durable (5.2).
- **Ended tasks are read through the store's indexes** (11.2): an ended
  task by its key, a list of them through one ended index, each row
  joined to its task's record, and an ended conversation's read position,
  by point reads in the same step (`../protocol/store.md`, 7.3).

### 5.4 What the store keeps of the core

| Record | Written by | Loaded |
|---|---|---|
| the deployment: its id, made at its first start, and its numbers | the core | at start |
| settings: the defaults the newest revision was made with, the changes over them, the held revisions' records, the charter versions they read and pending proposals' sets (17.7) | the core | at start, after the deployment, before every other record |
| settings' history: the records of revisions no longer held, within the history limit (17.7) | the core | on demand |
| projects: resources and roles, homes, policy, parties' roles | people | at start |
| parties: identities, sign-ins | people | those with a live sign-in or a role, at start; others as they sign in |
| parties' requests' keys, with their answers, for a retention | people | at start, those within it |
| tasks: every field of tasks.md, section 3 (its label, flags, last activity, LLM choice, kept account and revision among them), holds and their queues, writer slots, the last turn committed for a live attempt, and the revision a claim was prepared under (17.3) | tasks | live ones at start; ended ones on demand |
| LLM accounts: each account's holder, LLM provider, subject and display name, uses, and its sources in their order, each with its kind, a program's name, its state and a limit's reset; never a program's login on a host, which hosts report (accounts.md, 3.1 and 3.5) | accounts | at start |
| the indexes of ended tasks: by requester, every ended task a party requested; by project, the ended tasks flagged listed; each saved in the decision that ends its task (11.2; `../protocol/store.md`, 7.4) | tasks | on demand |
| parties' read positions over conversations: a live one's; an ended one's, moved by the decision that ends the task and erased with it (11.4) | people | live ones at start; ended ones on demand |
| calls committed since each live task's last turn, by attempt and name, with the core's part of their answers | the core | with their task |
| each live agent task's conversation: its first attempt, where its current window starts, and whether an upgrade closed it (7.2) | the core | with their task |
| funders' numbers: each party's pool and each project's spend, per period; a period's beginning, and when the next begins if its source said | tasks | at start |
| a task's history: made, amended, held, released, cancelled, by whom, why; and its activity: words written to it, runs begun and ended, what it heard (7.2) | tasks | on demand |
| inboxes: messages not yet taken | tasks | with their task |
| proposals: pending, and decided ones' outcome | tasks | pending ones at start |
| transcripts: turns, with their spend and whether each opens a window | the core | to resume, from the current window's start; on demand |
| notes | notes | indexes per scope in use; entries on demand |
| secrets: minted credentials' digests, by their sign-in's number, and services' credentials' digests; LLM accounts' records and keys, by their account and source, each kept before it is lent (`../protocol/store.md`, section 9; `../protocol/llm.md`, sections 5 and 6) | the protocol layer, never the domain, which names them by account, source and generation, or by a sign-in's or a service's number | at start, by the protocol layer |

Each connector keeps its own records beside these (connectors.md,
section 2): its outbox, procedures' states and parameters, what it made
by key, its projections' states, and its own state. The application's
root wraps both in its store vocabulary (root.md, section 6).

**Times** the store keeps are wall times (`env.wall`), so a record means
the same after a restart; a wait the core arms is a deadline on its
monotonic clock (`env.now`), computed from them (programming-model.md,
section 9). Accounts' source deadlines retain both clocks where the
limit or retry is created (`accounts.md`, 3.5): timers use `Deadline.at`,
and every projection and stored reset uses its source-owned
`Deadline.wall`, never a conversion at read or save time. Restore keeps
the exact wall and maps it to the new monotonic clock once. The existing
optional wall reset and client fields keep their layouts. Fleet grace
and adrift times remain the fleet's separate concern.

### 5.5 What the core asks of the store

The store is assumed (core.md, section 2); this is what the core relies
on, and all it relies on:

- a commit of several records is applied whole or not at all;
- commits are applied in the order sent, and durable once answered;
- a load sees every commit answered before it was sent;
- records are keyed by the domain's own keys, and listed by key ranges,
  in bounded pages; what is live (live tasks, pending proposals, the
  outboxes, live resources) is kept in key ranges of its own, so a
  restart reads it without reading history;
- what one engine wrote, the next reads: records carry the version of
  their shape;
- nothing else writes it.

### 5.6 How a child's records reach a commit

Each child that keeps durable state, and each connector, defines its own
records and ordered keys in its own vocabulary. Neither a sibling nor
the store chooses their shapes. The parent translates their keys and
carries their records in its own store vocabulary; the store's protocol
layer encodes each shape with a version (5.5).

- **`Save { record }`** keeps the record under its own key, and
  **`Erase { key }`** removes that key. These requests are part of the
  decision the parent is making. They have no terminal event and do not
  ask the child to wait: its state changes as it decides (5.1).
- **`Restore { record }`** supplies one record at restart, and
  **`Restored`** says every record in the child's live ranges has been
  supplied. The child then decides from that restored state.
- **`Load { owner, range }`** asks for one bounded page of the child's
  own key range. Its one terminal event is **`Loaded { owner, rows,
  more }`**. The token is echoed, and `more` says another page remains,
  which the child asks for with a load of its own.

A child never waits for a commit and never holds an output back for one.
Its outputs leave it as it decides, each saying which of the three of
4.2 it is, and its parent passes them up. A child's world plays its
parent and the store: it keeps saved records, applies erases, commits at
the end of each step, and restarts by feeding the durable records as
`Restore` followed by `Restored`. Restart is therefore tested before the
root is built.

## 6. Restart

A restart is a cold start that rebuilds everything a decision reads from
the store and the connectors, and makes nothing twice. The core runs it
as a script: it asks the root for one step at a time, and asks for the
next only once the root says this one is done (root.md, section 8).

1. **The core's records:** the deployment's numbers; the settings,
   checked whole (17.7), before every other record; projects, their
   policies and parties' roles; parties with a live sign-in or a role,
   and their requests' keys within the retention; parties' read
   positions over live conversations, never ended ones' (11.4); live
   tasks with their inboxes, subscriptions, holds and queues, and their
   dependencies with when and by whom each was added, the waits among
   them judged whole, so that a queue's waiter left on a circle of waits
   is held (tasks.md, 6.7); pending proposals; the calls committed since each live task's last turn;
   each live agent task's conversation; the funders' numbers; LLM
   accounts. The core's limits bound what is live (tasks.md,
   section 11), so what is loaded fits; a live set past limits lowered
   since refuses the start, saying which. A live task names its
   revision, which is held (17.3). Once all of it is restored, new
   defaults make the release's revision, checked against the limits
   with the live tasks' revisions held (17.7).
2. **Each connector's records,** connector by connector: the root hands
   it its kinds' current settings (17.5), then loads that connector's
   live ranges and restores them into it (connectors.md, section 14).
3. **The runs adopted.** Every task claimed or running names its attempt
   and the kind of host it was placed on. Claims placed in the engine
   died with it, and are settled at once as lost, or as not started if
   they committed nothing (tasks.md, 5.2); so are claims on workers in
   the engine's own process, which no worker outlives
   (`../protocol/service.md`, section 10), when its configuration says
   its workers are in process. Claims on other workers reach the
   fleet, which adopts each attempt a worker reports as it dials in,
   keeps waiting for one within its grace, and cancels any it does not
   name (section 8). Nothing new is assigned before every claim loaded has
   reached the fleet.
4. **Read afresh.** Each connector reads, for the live tasks that name
   them, the resources its system owns.
5. **Settle the outboxes.** Each connector resolves every entry not
   committed as made, as its recovery class says (connectors.md, 4.3):
   looked for by its key or the state it leads to, written again if it
   is a set; one not found waits for the retry deadline committed with
   its last attempt, which the restart keeps; one of a kind that cannot
   be recovered holds its task for a person.
6. **Decide afresh.** Every procedure steps once on the facts read;
   every agent task due gets a run; every inbox's messages are relayed or
   wake as their policy says.

Workers keep the answers and turns they were not acknowledged for, and
send them again after their hello; the core commits any it does not
have, and acknowledges each.

**After an upgrade.** jig's store patches a copy of a store behind the
binary's patch levels before the root starts, and switches to it
(`../protocol/store.md`, 10.2), so a restart always runs over the
generation the deployment's `active` names, at the binary's levels,
and sees nothing of the patching. The check before that switch
(temper's `draft/upgrades.md`, 6.4) runs what the store holds of this
section's rules: every record of the live ranges and of the census's
ended read positions loads within its codec's limits, and each ended
read position's task is held (`../protocol/store.md`, 10.2, step 5).
This section's rules, strictly, the live sets' limits and the rows the
census chooses are not in it yet (store.md, section 16): a
patched record a child refuses makes the restart after the switch
refuse, as any restart's does, with the generation patched from kept on
disk as `active` − 1 for the operator to go back to; one the accounts
child counts and leaves stays. The restart then runs over the switched
generation as any restart does. A patch marked fresh has closed every
live task's window in the copy before the switch (store.md, 10.2, step
4), so the restart restores them closed, and each task's next attempt
starts fresh (7.2).

`RestartRequest::ProjectPolicy { project: u32 }` names a saved project
policy whose byte size exceeds `limits.policy_bytes` or cannot be
accounted. `Core::restore_policy` keeps its boolean result, records this
refusal and stops the restart. `Core::restore_failure(&self) -> Option<RestartRequest>` reports either
this refusal or the existing
`Settings { failure, changed }` detail from one private slot. A later
`restart_refuse` retains it, and `restart_done` returns it without advancing.

### 6.1 Stopping

The engine stops when its service is told to (a signal, an operator), and
the core hears it from its root before anything closes
(`../protocol/service.md`, section 5). From then on it takes nothing that
would make a commit:

- **a party's request** is answered not ready, and an application's
  request likewise, and so are a sign-in, a credential to mint and a
  sign-out, which commit too (`../protocol/client.md`, section 8);
- **a run's turn or call** is answered busy, and its answer left
  unacknowledged, so its host keeps them and sends them again, to this
  engine or the next;
- **a connector's events and outcomes** stay with the connector, as when
  the journal is full;
- **no run is placed, and no procedure stepped.**

It ends every watch saying the engine is stopping
(`../client/domain/contract.md`, 7.2), and says once when nothing it holds
waits for a commit, so the service can close its components. A restart
then starts from the store as section 6 says.

Finishing continues while stopping. After routing previously accepted work,
a Resume at which the core owes finishing closes without taking a new input,
placing a run, stepping a procedure or resuming a connector. Follow-up watch
endings and the one drain notification remain held behind the last commit.
The continuation makes no new commit (root.md, section 3).

## 7. Agent executors

### 7.1 From a task to a run

When an agent task is due (tasks.md, 5.2), the core does as below. A
kept task is never due (tasks.md, 5.7), and one kept while it prepares
has its claim refused, its preparation abandoned with no try spent; it
is due again once its dependencies are done.

1. **Checks** that the run may start, and with what budget
   (authority.md, 8.3), and chooses the accounts that pay for it: one
   for its main model's LLM provider, kept by its task, and one for each
   other LLM provider its sub-agents' models use, each with its first
   source usable on a host with room (accounts.md, 4.3).
2. **Prepares it,** the task preparing over as many steps as it takes:
   gathers its brief (section 9), for why it runs: its first activation,
   or what woke it; has each connector that owns a resource the task
   names gather its workspace items, and name the application's
   credentials they need (connectors.md, section 10); and loads its
   transcript's current window, if its charter resumes and it has one
   within the resume limit (7.2). Either way it is given the calls
   committed after its task's last committed turn: a resumed run as a
   note after the transcript, a fresh one as a section of its brief.
3. **Commits the claim:** the task claimed, with its new attempt number,
   the revision its preparation began under (17.3), the host kind it is placed on, the accounts and sources its check
   chose, the task's kept account among them (accounts.md, 4.3), the
   writer slots of the holds its workspace writes (tasks.md, 6.3), and
   the messages its start carries (tasks.md, 8.2).
4. **Places it** through the fleet, once the claim is durable, on a host
   its charter allows, where its sources are usable and the host rule
   allows it (section 8), with an assignment the root puts together
   (root.md, section 7):
   - its run and attempt, the core's names for them, and its
     conversation's first attempt, from which the engine's protocol layer
     derives the run's identity, for the LLM provider's prompt cache,
     the same for every attempt of a conversation and never shared by
     two (`../protocol/channel.md`, section 7). No domain holds it;
   - its workspace: the connectors' items, and a workstream key, the
     number of the task that holds the resources written, so that the
     runs of one piece of work find their workspace cached;
   - its start's messages, named, oldest first; the rest are relayed as
     the run reads (7.4);
   - its **ask** for live content (`hosts.md`, section 2): `text` set when
     its task is flagged conversation (tasks.md, section 3), `reasoning`
     and `input` never yet. The core sets it with the run's prepared
     parts (step 2), from which the root puts the assignment together;
     the root copies it, kind by kind, and no host decides it. A task a
     conversation delegates is not flagged, so its run is not asked
     (`hosts.md`, section 12);
   - its charter, in the core's terms: the task's instructions, the
     brief, the tools its authority gives, the outcome its result
     contract asks for; its budget for the run: turns, spend and time,
     with a reserve of turns and time for finishing, its spend reserved
     from what its task has left in the claim's commit (authority.md,
     section 7); and its models, from its task's LLM choice (tasks.md,
     section 3): the main model with its effort, and those its
     sub-agents may use, each with its effort, its window (the input
     tokens a conversation's window may hold), its output (the most
     output tokens a completion may have) and its prices (authority.md,
     section 7).
     The engine's protocol layer translates it into smith's (hosts.md,
     section 8);
   - its transcript's current window, when it resumes, and the calls
     committed since its last turn;
   - its credential grants: for each account its check chose, its
     source's (accounts.md, 3.4), a value's current generation with how
     it is sent, or a program source's name, which has its agent run the
     program backend (smith's `protocol/programs.md`, section 3); and
     the application's credentials its connectors named (12.1).
     Each names its form; the form selects the table the protocol layer
     fills its value from (`../protocol/channel.md`, section 8).

The check's allowance carries its checked spend, retained models, a concrete
channel and kind, and one provider/account/source binding per retained provider.
Main and same-provider sub-agents share their binding. A provider with no usable
serving account contributes no optional models; a usable optional provider's
retained models constrain the same host as main. Fleet preserves affinity and
free-slot preference among those hosts. An undeclared unused alternative does
not block the run; a declared model with inadequate bounds still refuses hello.

Preparation facts are read again after asynchronous work and before the claim.
The successful task claim keeps main's account in the same commit as the core's
complete bindings. A changed selection abandons preparation without a retry
timer. Start and queued placement retain the selected channel; loss of room
returns the claim for a fresh check, releasing its reservation without a failed
try. Source failures are authenticated against that claim, and their ordinary
terminal charges actual spend once and releases writers. A recovered refresh
cannot turn an unrelated later failure into an account wait.

Restoration refuses duplicate provider bindings and a surviving source whose
account or provider disagrees. Removed rows remain legitimate history. Adoption
offers grants only from the recorded surviving sources, never replacements;
an unavailable recorded grant can renew on its original source, and an absent
worker settles through fleet's existing grace. A new assignment rechecks the
recorded bindings and current coverage.

The check joins each candidate host's kept program logins to the accounts' issued
names and their original holders. Every bound account must satisfy that host's
rule, including retained providers. Deployment-held logins restrict no run;
a deployment with at most one person is unaffected. A program source is chosen
only where its login is usable; the engine never takes one. A program source's
run is granted its source's name and runs on smith's program backend, for the
program its endpoint names (`../protocol/configuration.md`, section 3). Until
configuration's providers-only conversion, legacy
model account fields seed deployment-held accounts only; neither admission nor
the per-run charter routes by them.

### 7.2 Transcripts

- **A transcript belongs to its task,** whichever runs added to it. Its
  conversation is the turns from the last run that started fresh, and
  its current window the conversation's turns from the last that opens a
  window: a window turn, which a compaction made, or the conversation's
  first (smith's `protocol/transcript.md`, section 3). That conversation
  is the transcript's, which every agent task has; a task flagged
  *conversation* is another matter, whose turns are entries for its
  requester (11.4).
- **Beside it, the task's activity:** the words parties wrote to it as
  they wrote them, its runs' beginnings and ends, the results and news it
  heard and the decisions made about it, kept in the order they
  committed, so a client reads a conversation whole after its words have
  been taken from the inbox (`../client/domain/contract.md`, section 6).
  A message is a line in the activity of the task that hears it
  (tasks.md, 8.1): a party's answer to a question a task heard is that
  task's own line; a task's answer to the delegate that asked is the
  delegate's line, never repeated in the answering task's activity,
  whose turn holds the call that sent it; a read of the question joins
  its answer by a point read (`../protocol/store.md`, 7.3). Who may read
  an answer is the client's contract's (`../client/domain/contract.md`,
  section 6).
- **Committed turn by turn.** A run sends each turn up as it ends: what
  the LLM said and the calls it made, their results, what it spent, the
  last inbox message it read, and whether it opens a window. The core
  commits the turn with its spend, with the messages up to that one
  taken but those it says the run refused (hosts.md, 6.4), and, when it
  opens a window, with its task's current window starting there; then
  it acknowledges it. A host keeps each turn until it is acknowledged,
  and a run with too many unacknowledged turns waits (hosts.md, 6.4).
  Admission is whole: the tasks child validates the attempt, turn and
  offered read-through before changing its spend: the read-through names
  a message offered to the attempt, still waiting or, for a proposal or
  an escalation, decided since it was offered (tasks.md, section 9), and
  none past the attempt's highest offer; a refused turn changes nothing,
  and a duplicate accepted turn changes nothing again. The core keeps
  the ids it offered an attempt with the run's proof until a turn's
  fence reaches them or the run refuses them, so such a fence survives
  the entry's decision and the core's restart, and they hold their room
  in the run's inbox until then (7.4).
- **The core never looks inside.** A turn is bytes of the agent's own
  conversation vocabulary, bounded, which a client's protocol layer
  renders. The core reads only its size, its spend, what it took and
  whether it opens a window, which the turn carries beside its bytes.
- **What a turn's calls to the engine made is the core's to say.** As
  it commits a turn, the core writes beside it, from the parts of the
  answers it committed for the turn's calls (7.3), what each call made,
  under the call's name (its completion and its position,
  `../protocol/channel.md`, section 7), as
  `../client/domain/objects.md`, section 10, shows them: the tasks a
  delegation made, the proposal a `propose` made, a decision's object,
  what an effect made, a note. A call it holds no part for names
  nothing. No layer reads what a call made from its answer's words,
  which its protocol layer renders for the LLM (hosts.md, section 8):
  smith's transcript keeps a host tool's answer as text alone (smith's
  `protocol/transcript.md`, section 2).
- **Resuming.** A run that resumes is given its current window only,
  within the resume limit (the deployment's bound on the current
  window's bytes, across the attempts it spans, as the window crosses to
  a host: `../protocol/channel.md`, 6.1), and, as a note after
  it, the calls committed after its last turn, with their answers. One
  past the limit starts fresh from a brief that carries the transcript's
  tail. Earlier windows' turns stay readable by parties
  (`../client/domain/contract.md`, section 6), and are never sent to a
  run. A transcript the agent cannot use fails its run as transient,
  saying so, and the next run is prepared fresh.
- **A window closed by an upgrade.** A turn is handed to a run in its
  current version, patched as read when it was written in an older one
  (`../protocol/store.md`, 8.6), and a run resumes from it. A patch
  marked fresh says instead that windows across it start fresh: smith's,
  when what its turns mean to an agent changes. Patching then closes
  each live task's current window in the copy, before the switch
  (`../protocol/store.md`, 10.2, step 4): its run proof's window keeps
  its attempt and turn, and is closed. The core restores it closed, and
  the task's next attempt starts fresh from a brief carrying the
  transcript's tail, as one past the resume limit does: the closed
  window's turns are loaded for their text alone, none is resumed, and
  the new attempt is its conversation's first. It is a transition, never
  a failure: the attempt counts as any attempt does, its brief's tail
  within the tail's budget, and no transient try, failure, refusal or
  unread transcript is counted; its activity shows a run begun fresh,
  as one past the limit. A closed window's tail the store cannot read
  (a frame unread, `../protocol/store.md`, 8.3) is left out: the attempt
  starts fresh without it, and nothing is counted. However the upgrade's
  stop classes the attempt it ends (temper's `draft/upgrades.md`, 7.2),
  a closed window is kept for its tail's load, where a transient try
  drops an open one. No attempt is to be live across the switch: an
  upgrade across a fresh mark is to be a full one, its workers stopped
  first, so that every window the next attempt opens is its own, and
  open. That is temper's to make, in its `draft/upgrades.md`, section 9
  (`../protocol/store.md`, section 16); the core assumes it and checks
  nothing of it. The run proof
  keeps the window as a `CurrentWindow` (its attempt, its turn, whether
  it is closed: core-v6's `Window`); the transcript a run prepares keeps
  the pair it loads from, and whether it is closed beside it.
- **Turns are numbered within their attempt,** from one. The task keeps
  the last turn committed for its live attempt, restored with the fleet's
  adoption of the claim, so a copy already committed is acknowledged
  again and dropped, after a restart too.

### 7.3 The engine's tools

A run reaches the engine through smith's host tools, declared in its
charter, which its host relays; their schemas, decoding and rendering
are the protocol layer's (hosts.md, section 8). Each is checked against
its task's authority (authority.md, section 8), decided, committed, and
answered after the commit:

`delegate` and `amend` may name an `llm` choice: a main model and optional
effort, sub-agent model/effort pairs, and an optional account number. Host-tool
JSON names models and efforts by their configured indices; account numbers
are durable names. Omitting the choice keeps the delegate's charter default
or the amended task's existing choice. A choice grants no additional models:
the core checks inherited authority and the prospective financial payer.
These per-tool declarations change within the existing call envelope.

| Tool | What it does | Answers |
|---|---|---|
| `delegate` | makes a batch of tasks as its delegates, each with what a party's `Create` names, its label, flags and LLM choice among them (tasks.md, section 4) | their numbers and their specs' revisions, and what a budget given `AsAllowed` was given; refused, and why; beyond authority, naming what it lacked and whether a proposal could be accepted |
| `message` | words, a question or an answer to a task it references | sent; no reference; the inbox full; too large for a run to read |
| `amend`, `release` | a task below it: an amendment naming its spec's revision as the run last read it, but for a change of priority alone, and adding or removing dependencies within its reach (tasks.md, section 7); a release naming its hold's, and, for an effect held uncertain, whether to make it again (tasks.md, 5.5 and section 7) | done, with the task's spec revision after the commit, the one its next amendment names (a release moves none); refused, and why; moved, with what it is now |
| `cancel` | a task below it and its tree, with a reason, and a grace if the run gives one, within the tasks child's bound: the task is asked to finish first (tasks.md, section 7) | done; refused, and why |
| `decide` | accepts, rejects or passes up a proposal or an escalation waiting for it, an escalation naming its hold's revision (tasks.md, section 9) | done; beyond its authority, and passed up; moved, with what it is now |
| `propose` | an action beyond its authority, with a reason: a batch, an amendment, an effect, or a widening of its own authority (authority.md, section 9) | proposed, and to whom; refused, when nobody may accept it (tasks.md, section 9) |
| `withdraw` | its own pending proposal (tasks.md, section 9) | withdrawn; already decided, and how |
| `settings`, when its charter gives the family | reads the settings, or one setting whole, as a party's read does (`../client/domain/contract.md`, 5.1); proposes a change of settings against the revision it read, with a reason (17.6) | the settings; proposed, and to whom; stale, naming the newest; refused, when nobody may accept it |
| `introduce` | two tasks it references, each given a reference to the other, within their limits (tasks.md, 8.5) | done; no reference; refused, and why |
| `subscribe`, `unsubscribe` | a topic a connector offers to agents (connectors.md, 5.3), under a read grant on the resources the connector says it concerns (authority.md, 8.5); a referenced task's state; a timer (tasks.md, 8.4) | done; refused, and why |
| `effect` | one of a connector's kinds of effect offered to agents (connectors.md, 4.5) | made, with what it made; committed and pending, past the call's deadline; waiting for a verdict; failed; refused, naming them, for a kind that waits for dependencies while its task is kept (tasks.md, 5.7) |
| `note`, `recall` | writes a note in a scope; reads entries by name or by a search (section 10) | written; the entries |
| a connector's reads | reads its system (connectors.md, section 9) | the answer, within the read's budget |

**Calls are named, and decided once.** Every call names its task's
attempt and the run's own name for it. The core commits a call that
changes state together with its part of the answer, and answers a call
it has seen before from that record, never deciding it twice; what a
connector adds to an answer (what an effect made) it keeps under the
call's key, and the root puts the answer together again (root.md,
section 7). The core keeps a task's records, by attempt and name, until a
turn of the task that carries their answers commits, or the task ends;
so calls an attempt made in a turn it lost are still there for the next
attempt to be told of (7.1). A run whose call was withdrawn, lost with
its channel, or answered busy asks it again with the same name while it
lives, so the LLM is told the outcome rather than trying anew; an
effect's key is derived from the call that asked for it (connectors.md,
4.4).

Three tools a run has are not the engine's: `wait`, which the run and its
host serve, holding the slot until a message arrives (7.4); `finish`,
which the run serves, checking its result against the contract (smith's
`domain/run.md`, section 7), and whose result reaches the core as the
run's answer (tasks.md, 5.6); and `deliver`, which its host serves
through the application's workspace (hosts.md, section 9).

Beyond authority, a call is refused naming what it lacked; the run may
`propose` the same action with a reason. Calls never become proposals on
their own, so a person is never asked by accident.

**The tools are a party's operations.** Each engine tool that acts on
tasks, `introduce` aside, is one of the operations a party's requests
make (people.md, 5.1), checked by the same rules (authority.md, 8.1 and
8.4), so a run and a party do the same things to the same tasks:
`delegate` is `Create`;
`message` is `Message`, and `Answer` for a question; `propose`,
`withdraw`, `amend`, `cancel`, `release` and `decide` are theirs by
name. What only parties do stays theirs (people.md, 5.1): carrying out
person tasks, stopping runs, moving tasks, marking what they have seen,
and running the project. `introduce` is a task's alone: only a task
holds references (tasks.md, 8.5).

**A call names what it saw.** An amendment, a release and a decision
about an escalation name the revision the run was shown, as a party's
do: its brief shows its delegates' revisions (section 9), the messages
relayed to it show a hold's (hosts.md, section 9), and `delegate`'s,
`amend`'s and `release`'s answers show the spec's revisions they leave.
One whose object has moved since is answered moved, with what it is
now, and the run decides again on that; a change of priority alone
names none, and the first commit decides (tasks.md, section 7). So no
call writes over a change its run was not shown.

### 7.4 Answers, waiting and parking

- **A run answers once:** finished, with its result; parked; or failed,
  with a typed failure. The core commits it, once it has every turn the
  answer counts, and acknowledges it; a finished task closes, a parked
  one is idle, a failed one retries or is held (tasks.md, 5.5). What the
  answer says the run left on resources goes to their connectors
  (connectors.md, section 10).
- **Finished done while kept, it parks.** A run whose task is kept
  (tasks.md, 5.7) and finishes done, with a result its contract takes,
  is committed as parked: its turns first, then its saved state, kept,
  with no try spent, and a line in its task's activity naming the
  dependencies it waits for. Woken once they are done, its next run goes
  on from what it left, its brief or its start saying what it waited
  for (section 9), and finishes again. The tasks child decides it, as it
  decides every reason a task may not go on, so the run and its host
  know nothing of it.
- **Waiting holds the slot; parking frees it.** A run that calls `wait`
  keeps its host's slot while it waits, so a run that is talked with
  answers at once. Idle
  past its charter's threshold, it parks: every turn it took is committed
  first, and it ends. A coordinator's threshold is zero: it parks as soon
  as it waits.
- **Woken again,** a parked task is due. Its charter says whether the
  next run resumes the transcript or starts fresh from a brief; by
  default a charter for conversations resumes and a coordinator's starts
  fresh, its brief
  carrying its goal, its plan's state and its notes, and its start the
  messages that woke it (7.1).
- **Messages to a live run** are relayed as the run reads, named, in
  order, never more relayed and unread than the run's inbox takes; a
  turn that reads one takes it (7.2; tasks.md, 8.2).
- **Refused or unread, a message stays** in the task's inbox, untaken,
  and wakes the task again when the run ends (hosts.md, section 9;
  tasks.md, 8.2 and 8.3). A turn's commit never takes a message the run
  refused, whatever its fence covers (7.2).
- **A cancel** ends the run: the host cancels it and answers; what it
  committed before stays committed.
- **Saved state.** A run that parks, fails or is cancelled with unfinished
  work has it saved by its host, where its connectors say, and the next
  run of its task starts from it (connectors.md, section 10).

## 8. The fleet

The fleet child domain knows the hosts and the runs they host:

- **Hosts of two kinds** (hosts.md, section 3): workers, which dial in
  with their slots and the workstreams their workspaces hold; and the
  engine's own slots, a configured number, hosted by the engine's own hub
  (hosts.md, section 6), its agent `smith-inline-agent` with no
  workspace, present from construction. A charter says which kinds may
  run it. A run goes to a host with a free slot of a kind its charter
  allows, that declares its models, where its sources are usable and
  that the host rule allows (below), preferring a worker that holds its
  workstream. A refusal as busy is no failure.
- **Models at the hello, program logins after it.** A host's hello
  states the models its agent declares (hosts.md, section 2). A worker's
  logins follow its hello: the engine sends it, by name, every program
  source it may be given, and places no run on it until the first
  `logins` that answers them (`../protocol/channel.md`, 4.3). Its host's
  half checks each login when told of a new source, on its holder's
  check (accounts.md, section 5) and at a configured interval
  (`../protocol/configuration.md`, section 4), and reports what changed.
  The fleet waits for the inventory matching its desired source list,
  including an empty list, and resets that readiness on each channel.
  Hello listings and adoption recovery still run while new placement waits.
  A run is placed only on a host that declares its task's models and,
  for a program source, where that source's login is usable
  (accounts.md, 3.4). The engine's own slots never take a program
  source: `smith-inline-agent` has no program backend.
- **The host rule:** a host that keeps a person's program logins takes
  only runs that person's accounts pay for, until a run's process tree is
  contained from them (accounts.md, 3.4; hosts.md, 5.1). One person's
  deployment is unaffected. The deployment's document names each host
  the rule keeps a run from, and why. The rule governs placement: a queued
  assignment rechecks it, while assigned and adopted runs continue.
- **Attempts are fenced.** Everything a host sends is dropped unless its
  attempt is its task's live one, but a cancelled attempt's answer. No
  attempt of a task is placed while the fleet knows a host may still run
  another.
- **Answers and turns are acknowledged once committed** (5.2), or once
  fenced off; a copy before then is dropped unacknowledged, one for an
  attempt the core has finished with is acknowledged again.
- **Losing contact.** A lost worker's runs are kept for a grace; one that
  reconnects says what it hosts, and the core keeps or cancels each. Past
  the grace they are lost, and their tasks retry. The engine's own slots
  are lost only with the engine (section 6).
- **Relaying:** messages and cancels down, tool calls up and their
  answers back, each call answered once; a call the fleet cannot pass up
  is answered at once, unserved, or busy while the engine stops
  (6.1), never left without an answer.
- **The graces are ordered.** The engine's grace is longer than the
  longest a worker may take to stop a run it lost contact for, which the
  worker says at its hello (hosts.md, 6.6); a worker whose bound is equal
  to or longer than the engine's grace is refused. So the core places no
  next attempt while a worker may still host the last, and one run at
  most writes a resource at a time.
- **A host's agent must run the engine's charters.** The fleet refuses
  a hello, as not admitted and naming why, whose agent's bounds
  (hosts.md, section 2) do not hold every run the engine may place there:
  a model it declares held with a smaller window or output than the
  configuration's for it, never a model it does not declare, which only
  narrows what is placed there (above); turns, spend, time or waiting per
  run smaller than the deployment's most for a run, or a charter's; a checks' timeout longer
  than the reserve's time of a charter that allows a change; a largest
  message smaller than the deployment's message bound (tasks.md, 8.2), or
  an inbox that holds fewer than a start carries; fewer turns kept
  unacknowledged than the hub's bound (hosts.md, 6.4). So no run is
  placed where smith would refuse its start, and no run refuses a
  message as too large or its start's messages as full.
  The core exposes these owned requirements as
  `jig_domain_core::fleet_needs::configured(&Config, &Limits)`, for a
  validated configuration with the same limits used to construct the core.
  Its charter count must fit `limits.tasks.charters`, and each charter's
  main model plus alternatives must fit a `u32`. This projection does not
  validate configuration. A root may check a prospective host at startup
  with `jig_domain_fleet::shortfall(&Needs, &Bounds, HostKind)`, the same
  checker used by hello admission. It returns the first shortfall for
  applicable charters in configuration order, then checks deployment-wide
  bounds if any charter applies. Turns, time, waiting and checks name the
  failing charter's number; model window and output also name the model's
  position. Callers can then read required and offered amounts without
  repeating admission policy. The current client wire still reports turns,
  time, waiting and checks as dimensions without a charter number.
- **Standing hello refusals are volatile facts.** The fleet keeps the latest
  actual refusal for each host kind, even with no diagnostic history room.
  Admitting a hello clears only that kind's fact; duplicate hellos neither
  replace nor clear it, and channel zero's reserved-channel refusal is excluded.
  `standing_refusal(kinds)` returns the newest allowed kind's fact (engine
  wins equal wall times), only while no admitted, connected, non-draining
  host of any allowed kind exists. Busy slots and model eligibility do not
  weaken that suppression. The engine's construction placeholder is not an
  admitted hello. History eviction and fact drains do not affect this query;
  a new fleet starts with neither fact.
  The core uses it only to refine an agent's Active Due or Preparing
  `RunWait::Host` into `TaskRunWait::Refused(TaskWaitRefused::Hello { kind,
  at, why })`. Account and credential waits keep their precedence. No task
  state, placement decision, attempt, retry or stored record changes.
- **Grants follow their attempts:** each new generation of an LLM
  account's source or an application's credential goes to every live
  attempt that holds one (section 12), and each attempt the fleet keeps
  at a worker's hello, or adopts, is sent its current grants, so no run
  outlives its token for want of a resend. A grant never overtakes its
  attempt's assignment (5.2), so a generation renewed while the
  assignment waits for its commit reaches the attempt after it.
- **Every start and adoption ends once:** answered, lost, withdrawn (the
  core cancelled it), or refused (busy, a duplicate, or invalid).
- **A refused start keeps nothing.** A host's answer that refuses its
  start, at its entrance or as a start its agent refuses (hosts.md, 6.2
  and 6.3), ends a run the host never hosted. The fleet closes the
  attempt as it hears it, and never acknowledges it: one refused as busy
  is placed again, as no failure; one refused as invalid ends its start,
  refused. One lost with its channel leaves its attempt unlisted at the
  next hello, and the start ends lost; the next attempt is refused again,
  or placed.
- **Restarting** (section 6): the fleet adopts the claims the store
  names, then hears that the loading is done; a run a worker lists that
  no claim adopts is cancelled once a grace has passed.
- **Its history is diagnostic:** the deployment's recent lost workers,
  refused hellos and capacity refusals are kept in memory, within their
  limits, for the deployment's document, and start empty after a
  restart.

Workers are known by their delivery channel and by the configured worker
position that channel authenticated as. A hello naming an unconfigured
position or one held by a live channel is refused as `Position`, naming that
position separately from capacity refusals. Program login answers and
deployment facts name the configured worker, whose name is supplied at startup, never
from a worker message. A sign-out targets every configured worker; one not in
contact is unconfirmed at once. An old channel's result cannot change a new
channel's inventory or complete a later operation. Cleanup of issued names
after reconnect is a separate operation, and its login restrictions remain
until the host establishes absence.

## 9. Briefs

The brief child domain builds every run's brief from sections, each
within a budget, the brief within its own: sections follow why the run
runs, those it runs for are required, the rest optional; what does not
fit is cut in its kind's order and says how much; sections are gathered
within one deadline, while the task is preparing, and a required one
missing sends the task back to due after a backoff, no attempt spent
(tasks.md, 5.2).

Its sections come from two places:

- **The core's,** which it holds typed: the task (its spec and contract,
  and its lineage: its requesters' specs, cut); its dependencies' and
  inputs' results in jig's terms, and, for a run after its task was kept
(tasks.md, 5.7), what it waited for: each dependency added, when and by
whom; its delegates, their states, and the
  revisions a decision about them names, each one's spec's and a held
  one's hold's (7.3), and the results that came since (the plan, for a
  coordinator); its earlier
  attempts and why they failed; the calls its task committed since its
  last turn, when it does not resume; the proposals and escalations
  waiting for its decision; the index of its notes; a transcript's tail,
  when it does not resume, the window's last turns as text rendered by the
  host's protocol layer when loaded, within the tail's own budget; and,
  for a run whose tools hold `delegate`,
  the catalogue: each model its authority allows (authority.md, section
  3), with its LLM provider, its window and output, its efforts, its
  prices, and what it is good at, a description its configuration gives
  (`../protocol/configuration.md`, section 3), and whether an account
  that serves the run's payer is usable now, limited and until when, or
  needs sign-in (accounts.md, 3.5), so that a planning agent gives each
  task it delegates the LLM choice its work needs, by capability and
  cost (tasks.md, section 4).
- **Its connectors',** which it plans by kind, priority and budget but
  never holds (core.md, section 5): for each resource the task names, and
  for each dependency or input whose result is a connector's, the
  connector gathers the section, keeps it, and says its size; the brief
  child decides what fits, asks for cuts by size, and, when the brief is
  complete, the root takes each section from its connector to put the
  assignment together (connectors.md, section 9).

A section is loaded or read as it is cut, asking for no more than it
keeps. The host's protocol layer renders the whole as text, in order.

A brief has no inbox section. The messages waiting in the task's inbox
go with the assignment as its start's messages, and the run reads them
as messages, whole (7.1; tasks.md, 8.2).

## 10. Notes

Notes are what agents learn and pass on: a service that is slow to warm
up, why an approach was dropped, which check is flaky, what a person
prefers. The notes child domain knows scopes, entries and indexes:

- **An entry** has a one-line description, a body, a scope, references to
  tasks, and who wrote it: a party, or a task.
- **Scopes:** the deployment; a project; a goal; a resource pattern of a
  connector's (a service, a repository), within a project.
- **In the store,** where parties read, correct and delete them through a
  client. Notes are hints: where a note and the facts disagree, the facts
  are right.
- **Written by a tool,** checked against the task's note scopes
  (authority.md, section 3), and committed like any decision. A revision
  names the revision its run recalled, and is refused as moved if the
  entry changed since.
- **Read in two steps.** A brief carries the index of the notes in its
  scope, a line per entry, saying how many more there are when they do
  not all fit; a run asks for the entries it wants with `recall`, by name
  or by a search of their descriptions, answered a page at a time.
  Indexes are kept per scope in use, and evicted least recently used;
  entries per scope are bounded.
- **Knowledge that belongs to a system** belongs there, such as a
  repository's guide: a note that matures may be proposed as an effect
  on it.

## 11. Views

The views child domain keeps parties' watches, and paces what each is
delivered; the core makes the documents they deliver, and answers
parties' reads, from what its children hold. What a watch shows, and
what a client reads, is the client's contract
(`../client/domain/contract.md`, sections 5 to 7); what the core
computes for them is said below (11.1 to 11.5).

- **Watches:** jig's subjects (`../client/domain/contract.md`, 7.1):
  `me`, the signed-in party's own document (11.1); a task; a task's
  tree; chips, a bounded set of tasks; the party's inbox, across
  projects; a task query's live lines (11.2); a task's live run; the
  party's LLM accounts (accounts.md, section 5); the deployment; and an application's own subjects, through the root to the
  connector that owns them. The other objects a client shows (a
  proposal, an escalation, a question, a person task, a result, a task's
  activity, a task query's ended part, a project, a note) are read, and
  read again when a watch that names them moves.
- **An application's subjects name their scope,** as its reads do, since
  the core cannot read a connector's subject: a project, the deployment,
  or the party's own, what is the signed-in party's across projects. The
  core admits a watch or a read of one on the party's sign-in and their
  role in that scope, the party's own on the sign-in alone, and on the
  views' limits, as it admits jig's; whether the subject is in that
  scope, and what the party may see of it, is the connector's to judge
  (connectors.md, 5.5). Its watches and reads carry a window, as jig's
  do (`../client/domain/contract.md`, 7.5). Only the document, or why
  there is none, crosses back; the root only routes.
- **Level-triggered,** as facts are (core.md, section 4). A delivery is
  the subject's whole current document, within the watch's window, made
  by the subject's owner when the delivery is made: the core, or a
  connector through the root. A watch has one delivery in flight; a
  change meanwhile marks it, and the next delivery, once the watcher has
  taken the last, is made from the state as it is then. Changes in
  between are coalesced, never queued. A watch ended with a delivery in
  flight ends once that delivery is taken or dropped
  (`../protocol/client.md`, section 7).
- **Finishing belongs to the core** (root.md, section 3). Gathered subject
  changes, retained writing's proof reconciliation and unreported watch
  endings remain owed after a projection pass defers them. A closing watch
  awaiting its delivery or stream acknowledgement waits on its child;
  repeating its ending is not finishing work. Sign-in expiry reaches the
  core through the people's timer (people.md, section 3); stopping still
  completes watch endings and the drain notification (section 6.1).
- **An ended task or tree opens as a finished read**
  (`../client/domain/contract.md`, 7.2). Construction uses one of the
  existing bounded task loads, held behind preceding commits. The core
  checks the sign-in and retained row's project on completion, makes the
  final document without restoring the row, and closes the watch after
  its delivery terminal. A close during the load drains construction
  without delivering. Live watches keep their existing pacing.
- **Watches are never stored.** A restart (section 6) starts with none,
  and is told nothing of the deliveries made before it; each client
  opens its watches again (`../protocol/client.md`, section 4).
- **A slow watcher never holds the engine up,** and is never told it
  missed anything: it sees fewer deliveries. What the views child keeps
  per watch is its subject, its window, and two marks: a delivery in
  flight, and a change since. A task query's subject is its keys and its
  order, of a fixed size. Nothing grows.
- **Marked by commits:** each decision's commit names the tasks, projects
  and parties it touched, and the views mark the watches they concern:
  - *a task touched:* its own, the trees along its lineage, chips
    naming it, and every task query it matches before the decision or
    after it (11.2);
  - *what a task waits on moving:* the same watches of the tasks along
    its lineage (11.3);
  - *a party touched,* their entries, a turn of a conversation they
    requested, their read positions, their roles or pools, or a policy's
    templates in a project of theirs: their `me` and their inbox.
  - *an LLM account touched,* its source, state, order or uses, a
    program's login reported on a host, or a sign-in or a check begun or
    ended: the `accounts` of the parties it concerns, and the
    deployment's (accounts.md, section 5).

  A connector marks its own subjects (connectors.md, 5.5).
- **Durable before shown:** a delivery waits for the last commit made
  before it, as every read's answer does (5.2).
- **A run's text as it is written** is the one stream: kept per live run
  being watched, within a bound, named by the turn and the retry the host
  reports it for (hosts.md, 6.5), and delivered from the offset each
  watcher has; never stored, and marked provisional. The core keeps kind
  `text` only: a piece of another kind is dropped where it enters the
  core, by the engine's root as it translates the hub's piece into the
  core's text, and counted, until the views keep it. So is a piece whose
  offset is past the 32 bits the views count, where a host counts
  smith's 64 (`hosts.md`, 6.5). The protocol layer carries every kind
  and offset as it is (`../protocol/channel.md`, 6.4), and the same root
  route serves the engine's own hub and its workers'. A later retry
  replaces an earlier one's text; text that starts past what the views
  hold of its retry, because facts were dropped between, marks that
  turn's text cut, as text past its bound does. It decides nothing and
  leaves at once (5.2). A committed turn reaches a client through the
  task's activity, read (7.2).
- **History is the store's.** What a client shows of the past (a task's
  results, its activity, a plan's revisions and why, the ended tasks a
  query finds) is read (5.3), paged, never held.

Nothing the core decides depends on a view arriving.

### 11.1 `me`

- **The signed-in party's own document,** about whoever the credential
  belongs to; no party watches or reads another's. The core makes it
  from:
  - the people child: the party, their kind, login and name; their
    projects and their role in each; what waits for them, counted
    exactly, and their updates unread, counted up to a limit; their read
    position over results; their role in the deployment, if any;
  - authority: their pool in each project, and what they may create,
    the bounds a draft is compared with, their allotment left among
    them;
  - the templates: each of their projects' merged templates.

  What it carries, field by field, is the client's contract's
  (`../client/domain/contract.md`, 5.1).
- **Templates are served, never used.** The core keeps the
  current templates, the newest revision's (section 17;
  `../protocol/configuration.md`, 3.1), and
  each project's policy's overrides of them (authority.md, section 6)
  only to merge them, per project, into `me`. No decision reads one: the
  client fills a template into an ordinary request, which the core
  checks as one written by hand, so a template grants nothing, and a task
  keeps the values it was made with, never which template gave them.
- **A resolved default beside intent.** Each effective template in `me`
  carries `resolved_default: Option<LlmChoice>` beside its configured
  LLM choice. An explicit template choice previews itself. For an
  omitted choice, the core uses Create's own resolver, in charter
  order, under the effective creating authority and with the current
  receiving party as payer. No eligible model, or an executor still
  *chosen*, gives `None`. This is an observation, never admission or
  an account reservation. The client's shapes keep the configured
  intent, never copy the preview into a request. Create resolves an
  omitted choice once and the task retains it (tasks.md, section 3);
  the preview changes no task or core store layout.
- **Marked** as any of it moves. A pool's amount left moves with spend;
  account and source changes affecting eligibility, policy, membership
  and effective-executor changes invalidate the affected previews.
  Its marks are coalesced, as every watch's are.

### 11.2 Task queries

- **One query for every list of tasks.** A task query names keys, an
  order and a window: its keys among project, requester (a party or a
  task), root, parent, label, phase class, executor kind and tracked;
  its order one of number, last change, last activity, priority and
  ended at; its window by cursor (`../client/domain/contract.md`, 6.1).
  Its rows are task lines, the one record every list of tasks is made of
  (`../client/domain/contract.md`, 5.1): its live tasks first, in its
  order, then its ended ones, each group with its own cursor. Its live
  lines are watched; its ended ones are read, a page at a time. The core
  admits it on the party's role in the project it names, or, naming
  none, for tasks whose requester the party is
  (`../client/domain/contract.md`, 7.1).
- **Live tasks are scanned.** The tasks child keeps no secondary index,
  and its live tasks are bounded by its limits (tasks.md, section 11), so
  a query's live part is one pass over them, whatever its keys, keeping
  the window's rows in order. The fixed set of keys bounds what marking
  compares and what a query carries, never the filtering.
- **Ended tasks are found through indexes** in the store, which the tasks
  child saves in the decision that ends a task (`../protocol/store.md`,
  7.3 and 7.4):
  - *by requester:* every ended task whose requester is a party, by that
    party and the label;
  - *the ended delegates',* for a query naming a requester task or a
    parent: that task's ended delegates;
  - *by project:* only the ended tasks flagged *listed*, by their project
    and the label. The flag is the task's, set when it was made, so the
    core reads no template to index.

  A query's keys choose one index, in that order: a requester party; a
  requester task or a parent; a project (`../protocol/store.md`, 7.3).
  Its other keys, the root among them, filter that index's pages, which
  may come back cut, and the next page goes on from the last row
  examined. A query whose keys choose none has no ended part, and its
  answer says so. The decision that forgets a task forgotten once read
  erases its index rows (11.4); retention erases the rest with their
  tasks (`../protocol/store.md`, 11.1).
- **What a query never finds:** words in a text (`../protocol/store.md`,
  section 16); ended tasks by outcome, resource or spend; tasks by a
  procedure's parameters, which belong in the application's own records
  naming the tasks; ended tracked tasks across projects.
- **Each line** comes from its live task, or from an ended task's index
  row joined to its record, with what only the core can give: what the
  task waits on (11.3), and, for a conversation, its unread count
  (11.4).
- **Marked by a task it matches,** before the decision or after it, so a
  task entering a query, leaving it, moving within its order or ending
  marks it. Matching compares the task's fields, and its lineage's root
  and parent, with the query's keys: a bounded comparison per watch. The
  core gives the views each touched task's keys as they were and as they
  are.

### 11.3 What a task waits on, and its last activity

Two values each task line and each task's document carry (tasks.md,
section 3):

- **What it waits on is computed,** since only the core holds a task's
  tree and the entries waiting in it together. It is never stored, so no
  restart finds it stale. It is the first of:
  - *a party,* when an entry in the task's live subtree, itself
    included, waits for a party: a question put to one, a proposal or an
    escalation waiting at parties, a person task addressed to a party or
    a role. It names whom the oldest such entry waits for, a party or a
    role, and how many wait;
  - *nothing,* when it is working: its run live and not waiting;
  - *its dependencies,* when it is kept (tasks.md, 5.7): the pending
    ones, the oldest first, and how many;
  - *its delegates,* when it has live delegates;
  - *news,* when it is idle, its run parked or waiting, or its procedure
    waiting on its facts: what moves it next is a message its wake
    policy lets through, or a fact its connector reads (tasks.md, 8.3;
    connectors.md, 6.2);
  - *nothing,* otherwise: its phase says what it waits for (its
    dependencies, its holds, its closing), or it has ended.

  The core computes it as it makes a line or a document, by a walk of
  the task's live subtree, within tasks per tree. An entry for a party
  made or ended, and a delegate made or ended, touch every task along
  its lineage, within the depth limit, so their watches are marked
  (section 11).
- **Its last activity is kept** on every task: a wall time, set by the
  decision that commits its cause, words the task heard, a turn its run
  committed, or a delegate's end (tasks.md, section 3). It orders lists,
  moves no revision, and stays on the ended task, so a query's ended
  rows show it.

### 11.4 Conversations

- **Known by its flag,** never by its charter, its label, or the
  template that made it (tasks.md, section 3). Only an agent's task
  carries it, so its turns are its transcript's (7.2).
- **Its turns are entries for its requester,** while that requester is a
  party: a turn committed is an update for them, and marks their `me`,
  their inbox, and the queries the task matches.
- **Read positions are the people child's** (people.md, section 6),
  moved only by the party's `Seen`. A line's unread count is the task's
  turns past its requester's position. There is one per task
  flagged conversation, its requester's, so they number at most the
  tasks flagged conversation:
  - *a live task's,* in the people child, restored at start (section 6);
  - *an ended task's,* moved to the store by the decision that ends it,
    never held, and joined by a point read where a party reads the
    conversation or a query's ended row (5.3), so an ended conversation
    reads with what its party had read.
- **A task's retention class** says what becomes of it once ended
  (people.md, section 6; tasks.md, section 3 defines the flag); the
  decision that forgets a task forgotten once read erases its read
  position and its index rows with it.

### 11.5 What a decision would do

- **Offers are the core's judgement.** Each object's document says which
  acts the party may make on it now, and for each whether it would be
  made, proposed and to whom, or refused and why, with its bounds. The
  core judges them from the authority, policies and pools only it holds,
  and judges again when the act comes (`../client/domain/contract.md`,
  5.3).
- **A decision's effect is data,** made from the core's state as the
  document is made (`../client/domain/contract.md`, 5.4): what accepting
  a proposal grants and costs, and the pool after; what a release does;
  what a cancel ends, its tasks by phase, its live runs and its pending
  proposals, never its connectors' effects; the reservation a move
  takes. Confirming a decision is the application's;
  the core gives the facts it shows, and takes the decision naming the
  revision its party was shown (`../client/domain/contract.md`, 5.2).

## 12. Accounts

The accounts child domain keeps the deployment's LLM accounts, and an
application's credentials beside them (12.1). accounts.md says what its
LLM accounts are: their holders, sources and uses (its section 3), each
source's state and how the child keeps it, from the restart's first step
to the close (3.5), and the accounts chosen with a run's check (4.3). The
protocol layer's accounts' component signs signed-in sources in,
refreshes them, keeps their records and keys before lending, and holds
their values (`../protocol/llm.md`, section 5). The domain names a
source by its account and its place, and a credential by its source and
a generation, with how long it is valid, and never sees a value or a
record.

### 12.1 An application's credentials

A connector's workspace may need credentials of the application's own: a
token to read from a system and write to it. The accounts child keeps
them beside LLM accounts, and lends them as it lends a source's value
(accounts.md, 3.5):

- **Named, never seen:** each by the application's number for it and a
  generation, with how long it is valid. Their values are the
  application's protocol layer's, in a table of its own, read from a
  file or from an exchange it refreshes.
- **Granted through the root:** the application's root tells the child
  each generation granted, as the accounts' component does for an
  account (4.1). One is usable while a granted generation is still
  valid (authority.md, 8.3).
- **Lent to runs:** a connector names those a run's workspace needs as
  it gathers its items (connectors.md, section 10); the assignment
  carries them as grants, each naming the application form, which
  selects the application's table (7.1;
  `../protocol/channel.md`, section 8); and they follow their attempts
  as an account's do (section 8).
- **Unauthorized, limited and refused are LLM accounts' only.** A
  system that refuses a credential is the workspace's failure (hosts.md,
  6.3), and the application's to refresh where it keeps it.

## 13. Limits and working sets

What the core holds, each bounded by its limits and summed into its
worst case (programming-model.md, 6.3):

- live tasks, with their inboxes, subscriptions, references, holds and
  queues, and their dependencies, with the index of each resource's
  holders and waiters and each task's dependents (tasks.md, 6.7);
  pending proposals (tasks.md, section 11);
- the rules and the live projects' policies, within limits on projects,
  resources per project and parties holding roles;
- parties signed in, and their pending requests; their read positions
  over live conversations, one per live task flagged conversation
  (11.4);
- the settings (17.8): the defaults and the changes over them, the
  held revisions and the charter versions they read;
- each live project's overrides of the templates, which the core only
  serves (11.1);
- the calls committed since each live task's last turn, and the funders'
  numbers;
- hosts, their runs, and the calls and messages relayed;
- LLM accounts, with their sources and uses, within the accounts'
  bounds (accounts.md, 3.6), and an application's credentials;
- decisions in progress and their hand-offs; loads in flight and their
  rows;
- briefs gathering, with the sizes of the connectors' sections, and the
  notes' indexes in use;
- watches, each its subject, a task query's keys and order among them,
  its window and two marks (section 11).

None of it grows with the store's history. A new task, a party, a run or
a watch past its limit is refused at its entrance, and the asker told
why. The root adds the core's worst case to its connectors' and its own
(root.md, section 10).

- **A compound request's decision** is priced as operations per request,
  the people child's limit (people.md, 5.1), times the costliest
  operation it may hold: a cancel of a whole tree, within tasks per tree,
  with the hand-offs to the connectors its tasks name. It is summed into
  the worst case with the rest, as every decision is (`../README.md`,
  6.2), so one request holding a whole conversation's close, or a whole
  project's goals reordered, fits.
- **The check for circles of waits** is priced as a walk of every live
  task, with a mark and a counter per task and per resource, within the
  live tasks' and holds' limits; its scratch is sized at start, as the
  index is, and the tasks a circle names within their limit (tasks.md,
  6.7 and section 11). A restart's judgement of every wait at once
  takes the same.
- **What reads and deliveries compute** is priced with them: a task
  query's live part is one pass over the live tasks, and what each of
  its lines waits on a walk of a tree, within tasks per tree (11.2 and
  11.3); its ended part is a load, within the loads' limits (5.3). None
  of it is kept between deliveries.

## 14. Below the domain

What the protocol and io layers owe the core, through the application's
protocol layer, which composes jig's pieces with its own:

- **The store:** commits of typed records, numbered, answered once
  durable, and loads by key range in pages, over skein-kv; jig encodes
  the core's records, the application its connectors', each with a
  version per shape.
- **Workers:** jig's channel, the engine's side: assignments, messages,
  grants, cancels, answers to relayed calls, acknowledgements down;
  hellos, turns and answers with the messages refused, calls, facts and
  notices up (hosts.md, section 10).
- **Connectors:** each its own protocol, the application's.
- **Parties:** jig's messages, carried by the application's transport to
  the clients' component, which knows no transport
  (`../protocol/client.md`): jig's documents and the application's, its
  requests and reads, and credentials. jig mints, verifies, expires and
  revokes credentials; the printed link and a service's credential are
  its own ways of signing in, and any other is the application's, which
  asks the component to mint a credential for the identity it verified
  (people.md, sections 3 and 10; `../client/domain/contract.md`,
  sections 2 and 10).
- **Accounts:** sign-in, refresh, revocation, records and keys kept in
  the secrets' store before they are lent, and tokens lent, by skein's
  OAuth accounts driver (`../protocol/llm.md`, sections 5 and 6;
  `../protocol/store.md`, section 9); a value a party's request carries
  taken out and kept before the request reaches the core (accounts.md,
  section 5); an application's credentials by its own protocol layer,
  each generation granted told to the core through the root (12.1).
- **Configuration:** the deployment's rules, connectors and their
  systems' addresses and credentials, LLM providers with their
  endpoints, sources and terms' facts, and models with their windows,
  outputs, efforts, prices and descriptions, but no account (accounts.md,
  3.1), workers' secrets, the engine's
  own slots, the charters the deployment offers with their runs' budgets
  and reserves, the application's templates, which the core only serves
  (11.1), procedures' settings, limits. Projects and their policies are
  the store's, seeded from configuration on a first start. The
  whole-config check after overrides admits only an actual configured
  project count within both authority and people roster capacities,
  ownerless projects included (`../protocol/configuration.md`, section
  6). `Core::new` asserts the matching
  `projects.len() + 1 <= limits.people.projects` programmer
  precondition, the one being the deployment's roles (people.md,
  section 3), alongside its other configuration assertions. Operator
  input reaches that constructor only after validation; the core seeds
  the whole set and the deployment's roster, with no other implicit
  roster or partial seed.

## 15. The world

Each child domain has a world of its own, its parent and neighbours
scripted in it, its expectations in a referee; authority, which keeps no
state, is tested by its step tests.

The core's world runs the core with every child beneath it, under a root
of jig's own for testing, against fakes that share none of its types
(testing.md, section 4): jig's test connector, with resources, holds,
pools, priced effects, requirements, procedures and faults drawn from
the seed; one to three scripted workers and the engine's own slots,
whose runs play scripts; scripted parties; the store on skein-kv's
in-memory mode, slow or failing a commit; and the engine restarting
cold, at drawn moments or at one a scenario chooses.

Its stories: a conversation that answers, parks and resumes; a plan of
reports, a choice and procedure tasks, revised mid-way, and done; a
procedure's effect waiting for another connector's verdict, then made; a
delegate held past its tries, escalated to a person and released; a goal
cancelled with runs live and effects in flight, closing deepest first; a
tool call whose answer is lost with the channel and the engine, asked
again after a restart and answered from its record, made once; a run
handing a held resource to a procedure while it still holds the writer
slot; two tasks queued for a pool's last slot, and the pool shrinking
under them; an uncertain effect across restarts made before its retry
deadline; an effect of a kind that cannot be recovered, uncertain and
held for a person; a judge's facts changing between its verdict and the
effect; a policy narrowed while a proposal waits, an entry is committed
and a run is live; a note written, corrected by a person and recalled; a
recurring task across a period's reset; a person's words to a running
run; a conversation resumed from its current window after a compaction;
a message a run left unread, waking its task again as the run ends;
drift holding a task, and the client told why.

Its stories of what the core computes and serves: a task query watched
while tasks enter it, leave it, change its order and end, each delivery
after its commit, the ended ones then read through the ended indexes,
by requester and, for a listed task, by project, a filter past the
prefix coming back cut; what a task waits on moving from its delegates
to a party as a delegate asks its person a question, and back once it
is answered; a conversation's turns counted unread until its person
marks them seen, the conversation ended and read after a restart with
the position its person left, and one forgotten once read leaving
nothing that shows; `me` marked as a pool is spent from, deliveries
coalesced; a budget given `AsAllowed` answered with what was given.

Its stories of added dependencies (tasks.md, 5.7): an agent task given
a dependency while its run is live, the run finishing done and parked,
its activity naming what it waits for, woken when the dependency is
done and finishing again, after a restart too; an idle agent task kept,
a message to it waiting in its inbox and never waking it, then
delivered as it goes on; a kept task's run asking an effect that waits
for dependencies, refused; a run amending a delegate to wait for
another, and one whose add would close a circle, refused naming it; the
reordering of two tasks queued for one write hold, ended done in the
new order.

Its stories of agents and procedures doing what parties do: a run
amending a delegate a party amended since it was shown, answered moved
with what it is now, then amending again; a run releasing a held
delegate, naming its hold's revision, and making an uncertain effect
again; a run proposing an amendment and an effect beyond its authority,
each accepted by a person; a run subscribing to a connector's topic and
woken by its news; a run cancelling a delegate with a grace it finishes
within; a procedure amending, cancelling and messaging its delegates,
one amendment refused as moved and the procedure stepping again on what
it is told.

Its stories of settings (section 17): an owner's change of a charter,
a template and a policy in one set, committed in one decision, then
`me` re-read with the template, the next `MakeProject` copying the
policy, a narrowed rule refusing the next effect of a task made before
it, and a restart restoring the same settings; a change refused as
stale, as out of shape, and by a connector judging an application's
kind, each committing nothing; a restart with other defaults, recorded
as the release's, a changed setting kept over them and a reset
returning one to the new default; a charter changed while its task's
run is live, the run finishing on its version and its delegates made on
the newest; a conversation moved at its next run, and one whose charter
was removed staying where it is; a change moving running work, and a
removed charter living while a task holds it, a delegation to it
refused as not offered; a restart with old revisions held and a run
adopted on its version; the bound reached, the change refused naming
the oldest revision's tasks, those moved with `MoveToNewest`, and the
change then made; a revision leaving memory as its last holder ends.
Its stories of a proposal of settings (17.6): one proposed by a run and
accepted by an owner, moving running work; one made stale by another
change and withdrawn, its proposer told.

Its referee holds the core to core.md, section 11, seen from outside;
applications' worlds reuse it on jig's conformance world (testing.md,
section 5).

## 16. Open questions

- **Commits in flight:** how many, and whether the store should take
  several decisions as one commit when they wait together.
- **Briefs:** whether a brief's store loads and connectors' gathering
  share one budget; cancelling a brief while it gathers; how a missing
  section shows in what the agent reads.
- **Transcripts' limits:** how the resume limit is sized against a
  charter's models' windows, so that a conversation smith compacts
  (smith's `domain/session.md`, section 8) resumes rather than starts
  fresh; and how long the store keeps earlier windows' turns, which no
  run is sent.
- **Views:** a limit on watchers per subject; how a watch of a task's
  tree is bounded when the tree is large; whether, should a pass over the
  live tasks per delivery, or a walk of a tree per line, prove slow, the
  tasks child keeps what a task waits on along its lineage as entries
  come and go (11.2 and 11.3).
- **Scaling out:** one engine per deployment until it binds; the store
  being the engine's alone is what would change first.
- **Models an LLM provider lists** for an account (providers.md, 2.4),
  in the catalogue beside configuration's (section 9). Default:
  configuration's only, since a run's budget needs each model's window,
  output and prices, which a provider's list does not give.
- **What a model is good at:** a description in configuration, or
  structured capabilities a planning agent could filter on. Default: a
  description (section 9).
- **A release's removed default that a change names** (a template
  changed to run a charter the release drops): the start is refused,
  naming both (17.7), or the default is kept as a change of its own,
  recorded in the release's revision. Default: refused, as a document
  naming a charter it lacks is. The refusal is the backstop: a release
  that drops such a default carries its own patch for the stored
  changes that name it, as every release that changes what is stored
  does from the baseline on (temper's draft `upgrades.md`, section 4),
  so only a release that forgot its patch meets the refusal.
- **An application's kinds pinned,** a value read from a task's
  revision as a charter is, should an application's setting shape what a
  running task is given. Default: never pinned (17.5).
- **Delegates on their parent's revision,** so that a goal's subtree
  keeps the settings its plan was made under, as a change's option.
  Default: later; delegates take the newest (17.3).

## 17. Settings

A deployment's charters, templates, the policies its projects are made
from, its rules, and an application's settings of its own are its
**settings**: the binary's defaults, with the changes the store keeps
over them. A change is one decision; each makes a revision; a task's
runs read their charter from the revision the task was made in. This
section is the mechanics; the model is core.md, section 13; who asks
is people.md, 5.1; what crosses to a connector is connectors.md,
section 17; the records are `../protocol/store.md`, 4.4.

### 17.1 What a setting is

- **A setting is named by its kind and its number,** and, for an
  application's kind kept per project, its project:

  | Kind | Number | Value | Read |
  |---|---|---|---|
  | charter | the charter's, from 1 | a charter whole: its instructions as text, its models, tool families, hosts, waiting, resume and default budget (`../protocol/configuration.md`, section 3) | at each run's preparation, from the run's revision (17.3) |
  | template | the template's, from 1 | a template whole (`../protocol/configuration.md`, 3.1) | when `me` is served (11.1): the newest |
  | policy | the policy's, from 1 | a whole policy in a seed's format, a project made from it copying it (core.md, 6.1) | at each `MakeProject`: the newest |
  | rules | none: one setting | the deployment's rules, whole (authority.md, section 6) | at every decision: the newest |
  | an application's kind | the application's | bytes jig never reads (17.5) | by its connector: the newest |

- **The defaults are the binary's.** The application passes them as
  values when it builds the core (`Config`'s defaults): its charters,
  policies and rules, its templates, jig's chat and goal among them
  where it wants them, which jig gives as values made for the charter
  they run (`../protocol/configuration.md`, 3.1), and its own kinds'
  (17.5). An application that passes none has `jig-protocol-config`
  read them from the document (17.7).
- **The current settings are the defaults with the stored changes over
  them.** A change sets a setting's value whole, or removes a default,
  so that it is no longer offered. A setting no change names follows its
  default, whatever the binary makes it. A reset erases a setting's
  change: it follows its default again. A setting no default has is
  added by a change that sets it, and its removal, like its reset,
  erases that change. The rules are set or reset, never removed.
- **Checked whole, by one check.** The core checks settings as a start
  has always checked the document (`../protocol/configuration.md`,
  sections 3, 3.1 and 6): each value in its shape and within its bound
  (a charter's instructions within the journal's run bytes, its budgets
  above its reserve and holding one of its default model's costliest
  completions, its models configured; a template's charter offered and
  its grace within the tasks child's span; a policy within the rules,
  its roles well formed, its overrides' templates offered, its owner
  role able to be a root's requester), the counts within their limits
  (charters within `tasks.charters`, templates within `templates`,
  policies within theirs), and the references among them, across the
  set. One function of the core's runs it: at each start over the
  defaults and the current settings, and for a change over the settings
  it would leave. Its refusal names a setting by kind and number, which
  the parser maps to the document's path when the document gave it,
  and its fault, one of a closed list a client words without checking
  anything itself: *shape* (a member out of its form or past its
  bound), *count* (its kind past its limit), *reference* (it names a
  charter, template, model or host the set does not offer), *rules* (a
  policy above them), *budget* (a charter's below its reserve or short
  of one costliest completion), *grace* (a template's past the tasks
  child's span), *owner* (a policy's owner role unable to be a root's
  requester).
- **What is not a setting:** the limits, which size memory at the
  start; the hosts, the LLM providers and models, the connectors, the
  clients and the first owners, the document's; and each project's own
  policy, which its owners change (people.md, 5.2).

### 17.2 A change

- **A set, against a revision.** An owner's `ChangeSettings` (people.md,
  5.1) names the revision it was made against and its items, at most
  one per setting, each a setting and what becomes of it: *set* to a
  value, whole; *removed*; or *reset* to its default. It says, too,
  whether running work *stays* on its revision or *moves* (17.4).
- **Refused whole,** nothing of it committed, when it is:
  - *stale:* another revision is the newest, which the refusal names;
  - *not theirs:* an item of the deployment's (jig's kinds, and an
    application's kept for the deployment) from a party who is not an
    owner in the deployment (people.md, section 3), or an item of a
    project's from one who is not that project's owner (people.md,
    section 4). A set touching both needs one party who is both;
  - *out of shape:* the settings it would leave fail 17.1's check,
    which names the item and what fails;
  - *judged:* a connector refuses an item of its kinds (17.5), saying
    why;
  - *full:* the revisions or the charter versions held once the
    decision is done, its moves included, would pass their limits
    (17.8). The refusal names the oldest revision held and its
    live tasks, within the limit on the tasks a refusal names (tasks.md,
    section 11), so the owner may move them (17.4) and send it again;
  - *empty:* it changes nothing of the current settings.
- **One decision.** Its commit holds the changes saved and erased, the
  revision it makes with its record (17.3 and 17.7), the charter
  versions it makes and those no held revision reads any longer, the
  answer under the request's key, naming the new revision, and what each
  part hears of it, in the same decision: the tasks child and the fleet
  the charter versions added and dropped; authority the rules; the
  views a mark on every `me`, when a template changed, and on every
  watch of the settings; the connectors their kinds' values (17.5); and, when
  it moves running work, the tasks moved (17.4). After a crash the old
  revision holds or the new one does, never a mix (core.md, section 4).
- **What follows it.** A narrowed rule reaches every later decision,
  tasks already made included, and a widened one widens no existing
  task (authority.md, section 6); a live project's policy is not checked
  against new rules, only the policies projects are made from are. The
  next `MakeProject` copies the policy as changed; a project made before
  keeps its own copy (people.md, 5.2). Clients re-read `me`'s templates,
  and a template removed leaves its number unused (`../protocol/configuration.md`,
  3.1). A task made after the change reads its
  charter from the new revision; running work, 17.4.
- **Asking is deciding.** A change is an owner's direct request,
  decided as it is admitted. One proposed by someone who may not make it
  is a proposal of settings (17.6), and accepting it makes this same
  decision.

### 17.3 Revisions

- **A revision is the settings as a change left them,** numbered from
  1, the first start's, and each change makes the next. A start whose
  defaults change the current settings makes one too, recorded as the
  release's (17.7). Everything made reads the newest.
- **A task records its revision:** the newest, when it is made, by
  whatever makes it (a party's batch, a run's delegation, a procedure's
  step, a recurring task, an accepted proposal), whatever revision its
  parent runs under. It keeps it, live and ended, until it moves
  (17.4). A delegation to a charter the newest does not offer is refused
  as not offered, as any create naming one is, and the agent adapts. A
  procedure's or a person's task reads nothing of a charter: its
  revision is its history alone, and it holds none.
- **A run records the revision it is prepared under:** its task's, as
  its preparation begins (7.1, after a conversation's move, 17.4),
  committed with its claim. What the run is given of its charter is
  that revision's: its instructions, models, tool families, hosts,
  waiting, resume and budget; and so is what the core decides from the
  charter while the run lives: its calls' checks, its placement, its
  finish. A change made during its preparation or its run changes
  neither.
- **Charter versions.** A charter's version is its value as one
  revision set it, named by the charter's number and the revision that
  made it, and kept once, however many revisions read it. A revision
  reads, for each charter, the latest version made at or before it; a
  removal is a version too, from which on the charter is not offered.
  So a revision costs only the charters it changed. Only charters are
  kept by version: every other kind is read at the newest (17.1), and of
  an older revision only its record says what it set of them.
- **Two lookups.** `charter_of(task)` is the version of the task's live
  run, or, with none live, of the task's revision; `charter(number)` is
  the newest's, for what is about to be made. Every site of the core
  that reads a charter reads it through one of them. An application
  that reads a task's charter through `charter_of` reads the task's own
  version with no change of its own: temper's forge routing, which
  takes an effect call's timeout from `charter_of(task)`, needs none.
  An amendment of a task's LLM choice is checked against its own
  version's models, a delegate's against the newest's.
- **Held revisions.** The newest is held, and so is every revision a
  live agent task records or a run in flight was prepared under (its
  preparation, its claim, its placement). A revision nothing holds,
  other than the newest, leaves memory in the decision that ends its
  last holder: the charter versions no held revision reads go with it,
  from the core, the children and the store, and its record stays as
  history (17.7).
- **The children keep versions, not revisions.** The core tells the
  tasks child and the fleet each charter version a held revision reads
  as it is made (to the tasks child, the models its tasks may choose; to
  the fleet, the host needs of its runs), and each one dropped. The
  tasks child finds a task's version by its charter and its revision,
  the latest at or before it, from its own table; the fleet is given the
  version with each run it places. No other child reads a charter.
- **What documents show.** A document that shows a task's charter (its
  tree, its waits, its LLM choice) reads the version its revision reads
  while that revision is held; once it is not, as for a task ended long
  since, it names the charter and the revision, and nothing read from
  the charter.

### 17.4 Moving running work

- **A move** sets a live agent task's revision to the newest. A task moves
  only when the newest offers its charter and allows its LLM choice
  (tasks.md, section 3); one that cannot stays on its revision and
  finishes there. A move takes effect at the task's next run: a live
  run goes on with the version it was prepared under, until it ends or
  an owner cancels it, and holds that revision until then.
- **A conversation moves at its next run.** A task flagged
  conversation, which its requester ends and which never ends on its
  own (tasks.md, 5.8), moves as each of its runs begins to be prepared,
  when it may. So a fix to its charter reaches its next turn.
- **A change may move running work:** committed with *moves*, it moves,
  in its decision, every live agent task on an older revision that may
  move.
- **An owner moves running work later** with `MoveToNewest` (people.md,
  5.1), naming a revision: every live agent task on it that may move moves, in
  one decision, and the answer counts those that could not. It is what
  a change refused as full offers.
- **What a task may do is never pinned.** Its authority is the rules',
  its project's policy's and its chain's as they stand (authority.md,
  section 6); only what its runs are given is its charter version's.

### 17.5 An application's settings

An application's own settings ride the same mechanism: defaulted,
stored, changed, recorded in revisions and decided as jig's are,
without jig reading them. No trait and no generic: a kind is a number,
a value is bytes, and its connector judges it within the change's
decision.

- **Declared with the defaults.** For each of its kinds the application
  gives its number, in a space of its own beside jig's kinds; its scope,
  the deployment or each project; the connector that owns it, by number
  (core.md, 3.5), which the root may route to any child of its own; its
  bounds, a value's largest bytes and the values per scope, which the
  worst case prices; and its defaults, each a number and bytes.
- **Values are bytes.** The core keeps them, the store keeps their
  changes, and a revision records them as it records jig's; the core
  checks only their bounds. They are never pinned: whatever reads one
  reads the newest.
- **Judged in the change's decision.** A change naming items of a kind
  hands them, each as the value it would leave (a reset as the default
  it returns to, or as a removal where there is none), with the
  revision they would make, to that kind's
  connector through the root, within the decision's step (4.3), as a
  requirement's verdict is asked (connectors.md, section 7). The
  connector accepts, or refuses with its reason, which the change's
  refusal carries. A set commits only when every connector it names
  accepts.
- **Told in the same decision.** Once committed, each connector hears
  its kinds' new values, its state running ahead of the commit as every
  child's does (5.1). At each start, each connector hears its kinds'
  current values before its records are restored (section 6, step 2),
  judged again by nobody: a stored value was judged when its change was
  made, and the defaults are the application's own. A release reads the
  values its kinds took before, as it reads its records; one it cannot
  read refuses the start, naming it, as a record it cannot read does.
- **Read by the application.** What a connector keeps of them is its
  own. The root, and through it the application's own children, read a
  value through the core by kind, scope and number, as they read a
  charter.
- **The root routes, never judges** (root.md, section 11): the judging
  is the connector's, in the connector's own terms.

### 17.6 A proposal of settings

Its code comes after the rest (`../STATUS.md`).

- **Proposed by a member or a run.** A party with a role in the
  deployment or in a project proposes a set with `Propose` (people.md,
  5.1); a run with its `propose` call, when its charter gives it the
  settings family (7.3), which reads the settings and proposes a set;
  never a procedure. A proposal names its revision, its items and its
  reason, as a change does, and leaves to its accepter whether running
  work moves.
- **In the deployment's scope.** No task's authority covers settings,
  so a proposal of settings waits for parties from the start: the
  deployment's owners for the deployment's items, and a project's owners
  for that project's; a set touching both waits for parties holding all
  of them. One no party may accept is refused at its entrance, as every
  proposal nobody may accept is (tasks.md, section 9).
- **The owner keeps the value.** The tasks child keeps the proposal as
  it keeps every proposal: its proposer, reason, where it waits and its
  state. The core keeps its set, by the proposal's number, in memory and
  in the store, restored with it.
- **Accepting it is the change** of 17.2, with the accepter's choice of
  whether running work moves, checked again then. An acceptance refused
  (out of shape, judged, full) leaves the proposal pending, and the
  accepter is told why. Its revision records the proposal and its
  accepter.
- **Stale is withdrawn.** The decision that makes a revision withdraws
  each pending proposal of settings made against an older one, as no
  longer applying (tasks.md, section 9), and its proposer hears so; a
  fresh proposal names the newest.
- **Reviewed against its own revision.** A pending proposal is always
  against the newest, the stale ones withdrawn, so the core serves its
  review from the current settings: for each item, the setting, what
  becomes of it (set, removed, reset), what it is now (its value, and
  whether it follows its default or the change of which revision, or
  that it is not offered) and what it would be (the value set, none, or
  its default's). A reviewer never compares against a later revision
  than the proposal's; once decided or withdrawn, a proposal carries its
  items alone, and an accepted one names the revision it made
  (`../client/domain/contract.md`, 5.1).
- **Bounded:** pending proposals of settings in the deployment are a
  limit of their own, each priced at a whole set (17.8), beside the
  tasks child's limits on pending proposals.

### 17.7 In the store, and at a start

- **What the store keeps** (the core's settings family,
  `../protocol/store.md`, 4.4): the defaults the newest revision was
  made with, setting by setting; the changes, each a setting's value whole or its
  removal, with the revision that made it; a record per revision, with
  its number, the revision it was made against, when, who decided it (a
  party, the release, the first start), the proposal it came from,
  whether it moved running work, and its items with the values they
  set; each charter version a held revision reads and the newest does
  not; and pending proposals' sets. The tasks family keeps each
  task's revision, live and ended, and each claim's.
- **History** is the revision records, kept for every revision held
  and for the latest ones within the history limit (17.8); the decision
  that makes a revision, or that ends the last holder of one, erases
  those that fall out. Restoring a version
  proposes its items again, against the newest. History is read from
  the store, a revision by its number and the records a page at a
  time, newest first, each with what it changed; a revision erased is
  answered as forgotten, naming the oldest kept
  (`../client/domain/contract.md`, section 6). The store keeps the
  history's extent, the oldest revision it holds, among the live
  settings. Each commit that makes a revision, and a start whose history
  limit was lowered, erases from the oldest up to the limit's cutoff,
  within the room the journal gives one commit, and records the new
  oldest. A revision below the cutoff is answered as forgotten even while
  a row that commit had no room for remains; later revisions' commits
  erase the rest.
- **A start** reads the settings right after the deployment's
  numbers, before any other record of the core's (section 6, step 1):
  the stored defaults, the changes, the held revisions' records and the
  charter versions. It then compares the
  defaults it is given with those stored:
  - *a first start* makes revision 1 from its defaults, recorded as the
    first start's;
  - *defaults that differ* are stored, and when they change the current
    settings (a setting no change is over), they make a revision
    recorded as the release's, with no proposal, once the core's
    records are restored (section 6, step 1); like any revision, it
    withdraws stale proposals of settings (17.6); new work takes it,
    running work keeps its own;
  - *the current settings are checked whole* (17.1): a start whose
    settings fail is refused, naming the setting, the change over it if
    any, and what fails, as a document failing its check always was;
  - *past limits:* held revisions or charter versions past limits
    lowered since, or a release's revision that would pass them, refuse
    the start, saying which, as a live set past lowered limits does.

  A saved project policy above the current policy byte budget refuses the
  start naming its project and policy; the named restore failure is retained
  until the shell reports it.

  All of it, the release's revision included, is committed before the
  core admits anything.
- **The document's members as defaults.** An application that passes
  no defaults gives the document's `charters`, `templates`, `policies`
  and `rules` (`../protocol/configuration.md`, section 3), and the
  store's changes lie over them as over any defaults. A deployment that
  restarts with the same document and no change behaves as it did; a
  document edited between starts gives the release's new defaults
  (above); a stored change wins over the document's member; a reset
  returns the setting to the document's value.

### 17.8 Bounds

- **Two limits of the core's, sized at the start**
  (`../protocol/configuration.md`, section 6): the *revisions held*
  besides the newest, and the *charter versions held* besides the
  newest's. A held revision costs its record's head, never its items'
  values; a charter version costs a charter at its bound (the journal's
  run bytes and the core's part of a charter), the tasks child's models
  for it and the fleet's needs. The core's worst case prices both, and
  the tasks child's and the fleet's tables are sized for the charters
  and the versions together.
- **The current settings** cost the defaults and the changes, each
  within its kind's bounds (17.1): at most two whole sets, priced so.
  The fleet keeps, for each connected host, what its hello declared for
  the configured models and its agent's bounds, priced by the models'
  bound, so that it can judge the host against changed needs.
- **History** is a third number, of revision records the store keeps
  beyond those held, within the store's fill (`../protocol/store.md`,
  11.2). The core holds none of it.
- **Proposals of settings** bring a fourth limit, of pending ones, each
  priced at a whole set (17.6).
- **A change fits one request:** its items within the clients'
  component's message bytes (`../protocol/client.md`, section 13). A
  set larger is several changes, each against the revision the last
  made.
- **A full bound names its holders:** a change refused as full names
  the oldest held revision's live tasks, and moving them is the way on
  (17.4).
