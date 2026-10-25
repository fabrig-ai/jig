# Testing the domain

Provisional, 2026-10-07, revised 2026-10-23. How jig's domain is tested,
and how it tests an application's: the worlds of jig's own parts, the
core's world, and the conformance world every application runs its
domain on, with the referee that holds it to jig's promises. It applies
testing-strategy.md to jig's domain layer; the tiers below the domain
(protocol, simulated, the real loop) are `../protocol/service.md`'s,
section 8. What is still open is listed in section 10.

## 1. In one page

- **Domain worlds first,** as testing-strategy.md, 2.2 has them: every
  child domain of the core has its world, the core has its own, and so
  does the hub, none of them needing a protocol layer.
- **The conformance world is jig's gift to applications.** It runs an
  application's whole engine domain, its root over jig's core and its
  connectors, against jig's fakes for the store, the hosts and the
  parties and the application's fakes for its systems; it crashes the
  engine at every commit, holds and fails commits, loses hosts; and
  jig's referee checks, from outside, every promise of core.md,
  section 11.
- **Generic in the world, concrete in the domain.** Worlds are ordinary
  Rust (programming-model.md, 10.2), so the conformance world is generic
  over the application, through a trait of the test kit's. No step code
  is.
- **A test connector** stands in for an application in jig's own worlds:
  resources, holds and pools, priced effects, requirements, procedures,
  topics and faults, all drawn from a seed.
- **Faults everywhere, limits tiny,** replayed from a seed, on skein's
  world harness, within the suites' time budgets.

## 2. jig's tiers

| Tier | Real | Faked |
|---|---|---|
| step tests | one step function; authority's checks | its events, by hand |
| a child's world | one child domain of the core, or the hub | its parent, scripted; its neighbours |
| the core's world | the core with every child, under a root of jig's own | the test connector, scripted hosts, scripted parties, the store |
| the conformance world | an application's engine domain, root and all | jig's fakes, the application's fakes, the faults |
| an application's system worlds | its engine, its workers and its agents' domains together | its systems' fakes, at their domain faces |

Every world runs on skein's world harness: one loop, one seed, the
referee in the loop, the heap counted at every iteration
(testing-strategy.md, sections 6 and 7).

## 3. The children's worlds

- **tasks:** batches, lifecycle, holds and queues, inboxes, wakes,
  proposals; a task's label, flags and last activity, cancels with a
  grace across a restart, goals per project, the ended indexes saved as
  tasks end, what parties and agents ask alike, and each task's
  revision, its moves and its charter version's models (tasks.md,
  section 12).
- **authority:** step tests only, as a decision over data; one batch
  under a task and under a party answering alike, and `AsAllowed`
  resolved over generated pools, roles and caps (authority.md,
  section 11).
- **people:** sign-ins and credentials minted, roles, requests,
  inboxes; operations alone and in compound requests, made whole or not
  at all; read positions over conversations, live and ended, across a
  restart (people.md, section 11). Existing request step tests and the
  people world fill completed-answer capacity: new `Seen` progresses
  without saving an `Answer` on success or any routed refusal, while a
  new ordinary key is busy. Ordinary reservations exclude `Seen` flights
  exactly, but the existing pending, flight and waiter bounds still
  apply. Duplicate waiters each get one terminal; pending conflicts,
  retained replay and conflicts, legacy `Seen` answers restored through
  original expiry, and reprocessing to a maximum watermark are checked
  without a new harness, tier or budget (people.md, 5.1 and section 6).
- **fleet:** hosts of both kinds, placement, fencing, graces, adoption;
  host needs per charter version, a run of an older version placed on a
  host its version allows and the newest's does not (engine.md, 17.3).
- **brief:** sections planned, gathered, cut by size, a required one
  missing; connectors' sections as sizes and tokens only.
- **views:** watches of jig's subjects marked by generated commits: a
  task query by the tasks it matches before a decision or after it, `me`
  and an inbox by their party's changes; changes coalesced, one delivery
  in flight per watch, and the watches' limits refused at the entrance
  (engine.md, section 11).
- **notes:** its own concern.
- **accounts:** the existing world checks source deadline creation,
  merge and restore with injected monotonic/wall clock pairs: later,
  earlier and equal resets, the old pair kept on a tie, a no-reset
  limit overriding a timed one to `None`, and indefinite limits kept
  until resumed. Restore preserves the exact saved wall; timers are
  checked just before and at the monotonic deadline (accounts.md, 3.5).
- **the hub,** with agent capabilities of both kinds, smith's: agent
  processes, `smith-host-domain` over smith's agent processes over skein's
  fake LLM;
  and, as the in-process kind, `smith-inline-agent`'s runs over skein's
  fake LLM; with turns that open windows, and messages refused and left
  unread: hosts.md, section 11.

Each child's world plays its parent and the store as engine.md, 5.6
says: it keeps saved records, commits at the end of each step, and
restarts by restoring them, so restart is tested in each child before the
core exists.

## 4. The core's world

The core with every child beneath it, under a root of jig's own for
testing, against fakes that share none of its types:

- **jig's test connector** (section 7);
- **one to three scripted workers and the engine's own slots,** whose
  runs play scripts: turns, calls, waits, parks, answers, crashes and
  silence;
- **scripted parties,** making requests and answering what waits for
  them;
- **the store,** skein-kv's in-memory mode, slow, holding commits, or
  failing one, which stops the engine;
- **restarts,** cold, at drawn moments or at one a scenario chooses, from
  the store's last durable commit.

Its stories are engine.md, section 15's; its referee is section 6's.
Beside the referee, the documents the core delivers are held to a
reference made from outside: a task query's rows against every task the
scenario made, filtered and ordered as the query asks, its ended part
within what its index serves; what each task waits on against the
tree and the entries the scenario made.

The existing `joint_clock` and documents fixtures compare a source's
deadline in task waits, accounts and deployment documents and in its
stored reset. Different injected clock pairs between reads leave those
wall facts exactly equal, including after restore: literal equality,
never a real-clock sample or tolerance (accounts.md, 3.5; engine.md, 5.4).

The existing charters and templates stories check `me`'s
`resolved_default` with catalogue and charter orders opposed, explicit
and omitted intent, a chosen executor, no eligible model, and account,
policy, membership and effective-executor changes that refresh the
preview. The resolver and payer are Create's own (engine.md, 11.1).

Settings (engine.md, section 17) are tested where each part lives, most
of it below this world: the current settings from defaults and changes,
the check, a charter's version for a revision, the held revisions'
accounting and the bounds' arithmetic as step tests of the core's
settings functions, pure and fast; the records' round trip and
goldens in the store's tests (`../protocol/store.md`, section 14); the
document's members read as defaults, and the new limits, in the
configuration's tests; the change's and the move's records in the
client wire's. This world holds what needs the decision and the
children together: the stories of engine.md, section 15. None needs a
process: the world's cold restart shows what a restart keeps.

First-start and restart stories use people roster capacities equal to
and above the actual configured project count; they seed the whole set
and keep it across restart. Invalid programmatic configuration is
`Core::new`'s precondition, never an invalid world constructor or a
catch-panic scenario (engine.md, section 14).

Existing inbox and ended-mark fixtures control the current subject,
role and requester and the interleaving of ended-record and position
loads with overlapping marks and forget. A completed `Seen` retried
after mark and forget answers `Unknown`, writes nothing and never
recreates history; lost authority can answer `Role`. Holding the journal
commit proves no external success leaves early. Crashing and restoring
recovers only committed read positions or committed absence, with the
mark and any consequent forget atomic (people.md, section 6). These
proofs stay in the existing step and world tiers and budgets.

## 5. The conformance world

What every application runs, and what makes jig's promises its own.

- **What jig gives:**
  - the harness, generic over the application: it builds the
    application's engine domain from its configuration, feeds it events,
    takes what it asks for from the journal, and plays its peers;
  - the fakes for jig's peers (section 7);
  - the faults: a crash after any commit, the engine restarted cold from
    the last durable one; commits held, so the engine runs ahead; a
    commit failed; hosts lost, slow, or back after their grace; parties
    sending a request twice;
  - the adversarial scenarios every application runs, with tiny limits:
    - an effect's copy arriving late, after its retry deadline and after
      a retry; a target state another hand reached too;
    - the engine restarting again and again while an effect is
      uncertain, before its deadline passes;
    - a judge's facts changing between its verdict and the effect;
    - a pool shrinking while its slots are held and a task waits;
    - the policy narrowing while a proposal waits, an entry is committed
      and a run is live;
    - a standing task running across many periods;
    - many sessions and sub-agents of one run near the end of its
      budget;
    - a compound request, the engine crashing before and after its
      commit and the request sent again: made whole once, or not at all,
      and answered once;
    - a cancel with a grace across restarts: the task finishing within
      it, or cancelled once when it ends, from the grace kept with it;
    - a party's and a run's decisions about one task racing: the one
      named with a revision that has moved refused, never made over the
      other;
    - a conversation ending while its party reads it, the engine
      restarting between: no turn read comes back unread, and a task
      forgotten once read leaves nothing a page shows;
    - a task query watched while its tasks end, and watched again after
      each restart: every delivery after its commit, the ended ones
      found through their indexes;

    Each runs with the application's own kinds. One the application has
    no kind for (a pool, where its connectors take no pooled holds) is
    reported as not applicable, never faked with a different mechanism;
    jig's own worlds still cover it;
  - the referee (section 6).
- **What the application gives,** through the harness's trait:
  - its engine domain, built from configuration;
  - its systems' fakes, as step machines, each reporting what it saw in
    the referee's terms: which effects reached it, under which keys, on
    which resources, of which kinds, at what condition. The fake reports
    what it observed in its own system's terms, mapped by the fake, never
    by the root, so a root that mistranslates an effect is caught when
    the observed effect differs from the one the core decided;
  - each kind of effect's recovery class, which the referee holds the
    fake system's observations to;
  - its scenarios: the parties' scripts, its systems' starting state and
    faults, its agents' scripts;
  - the policy each scenario runs under, which the referee reads as its
    independent statement of what is allowed.
- **Crashing at every commit.** For a scenario, the focused suite crashes
  after a few chosen commits and checks the story still ends as it
  should; the fuzzy suite sweeps every commit of the scenario, and draws
  cuts at random across random worlds.
- **What it catches,** where its scenarios reach: a root that sends a
  held output early, splits a decision across commits, admits after a
  child changed, restarts out of the core's order, lets an effect reach
  a connector unchecked, or translates one wrongly (root.md,
  section 11); and a connector that repeats an effect beyond its
  recovery class, gives a verdict on facts older than their freshness,
  forgets a retry deadline across a restart, or decides differently when
  stepped twice on the same facts.

## 6. The referee

jig's referee watches from outside (testing-strategy.md, section 7): what
the fakes saw, what the store made durable, and the facts the domain
emits, never a domain's state. Each promise of core.md, section 11
becomes observations:

- **Authority holds:** every effect a fake system saw is covered by the
  scenario's policy as it stood when the effect was decided, for the
  task that asked, or by a proposal whose acceptance the store made
  durable first; every effect whose kind has requirements saw, before
  it, a met verdict from each judge, each observed one within its
  freshness when decided, each guarded one still true when applied; the
  effect a fake observed is the effect the core decided, in kind and
  resources; nothing waits as a proposal that its proposer did not ask
  for: a run's `propose`, a step's proposal, a party's `Propose`, or a
  result's follow-ups.
- **Spend is bounded:** recorded spend never passes a task's budget,
  and actual spend, the fake LLM's own count, passes it by no more than
  the turns lost with workers allow.
- **Once, as each kind allows:** a keyed or conditional effect takes
  effect on its fake system at most once, late copies included; an
  idempotent one ends in the decided state; an unrecoverable one whose
  outcome was uncertain is never repeated before a person decides; every
  batch, and every compound request, is in the store whole or not at
  all; every uncertain entry is resolved, or held, within a bound
  however often the engine restarts.
- **Nothing ahead of its commit:** every effect a fake system saw, every
  assignment a host saw, every answer a party or a run saw, and every
  document a party's watch or read was given followed a durable commit
  that holds it; after a crash, nothing seen is missing from the store.
- **Order:** no task started before its dependencies were done and
  closed; no two runs held one resource's writer slot at once; no pool
  admitted a holder beyond the slots it was known to have; no task ended
  before its delegates.
- **Nothing lost:** liveness deadlines for every result to reach its
  requester, every person's words to reach their task or the task to end,
  every proposal to be decided, withdrawn, or waiting where a holder sees
  it; no turn a party has seen comes back unread, across its task's end
  and restarts (people.md, section 6).
- **Nothing written over:** no effect on an owned object that the
  application did not make it, but as a participant or through its
  connector's mechanics; no amendment or release made against a
  revision that had moved since the one its party, run or procedure
  named.
- **A run reads its revision:** every assignment a host saw carries the
  charter version of the revision its run was prepared under, and every
  task's revision in the store was the newest when it was made or
  moved; every change of settings is in the store whole or not at all,
  and a refused one left nothing.
- **Bounded:** the heap within the worst case at every iteration, and
  every story ending within its bound.

## 7. Fakes and kits

What jig ships for tests, as ordinary Rust or as step machines where
more than one world needs them (testing-strategy.md, section 4):

- **the store:** jig's store component over skein-kv's in-memory mode,
  with an in-memory stand-in for transcripts' files, the world holding
  commits, failing one, and serving pages slowly
  (`../protocol/store.md`, section 14);
- **scripted workers:** each a host that speaks the contract of
  hosts.md, section 2, playing a run's script, keeping turns and answers
  until acknowledged, refusing messages and leaving them unread, losing
  its channel and coming back, or vanishing;
- **`smith-inline-agent`'s runs:** smith's real domain over skein's fake
  LLM, with a script per charter, for the hub's world and for an
  application's worlds that run smith's own domain;
- **scripted parties:** sign-ins, operations alone and in compound
  requests, reads and watches, by script or at random within their role;
  a template filled into a `Create` as a client fills it from `me`;
- **the test connector:** resources named by paths, kinds of hold
  (exclusive and pooled, refusing and waiting, pools that shrink),
  priced effects of every form and every recovery class, with keys,
  conditions, late copies and uncertainty, requirements guarded and
  observed, judged on its facts, procedures that step, wait, delegate,
  amend, cancel with a grace, message and finish, topics with classified
  news, some offered to agents, drift, kinds of settings of its own,
  judged on its facts, accepting or refusing as the seed draws, and a
  fake system beneath it that reports what it saw. The core's world runs two of them, so that
  judges, resources and kinds cross connectors, with names and kinds
  generated from the seed;
- **the referee and the harness** of the conformance world.

They stay in jig, and an application's worlds reuse them. What jig finds
missing in skein's world harness goes into skein first.

## 8. What an application adds

- **its connectors' worlds,** each with its system's fake below and the
  root scripted above (connectors.md, section 15);
- **its conformance scenarios,** which are its stories: `ops`'s are
  `../examples.md`, section 7;
- **its system worlds,** if it has workers: its engine, its workers with
  their workspaces, and smith's agents, together;
- **its own tiers below the domain,** with its protocol layers.

## 9. Budgets

jig's tests fit the suites' budgets (testing-strategy.md, section 8):
the focused suite runs a few cuts and seeds per scenario, the fuzzy
suite sweeps. jig's tests count against jig's budgets
(`../../development/workflow.md`, section 2); an application's
conformance scenarios count against the application's. The stories of
the plumbing (operations alone and compound, a grace, task queries,
read positions, revisions named by runs and procedures) join the
scenarios of the worlds they belong to, a few cuts each in the focused
suite and swept with the rest in the fuzzy one: none needs a suite of
its own.

Settings' tests are priced so (engine.md, section 17). Their step tests
are pure functions over a few settings, each well under a millisecond.
The core's world gains about a dozen focused scenarios (half with the
change, half with revisions and moves), the tasks child's and the
fleet's a few each, each as long as the scenarios beside it, a few cuts
each: about what the runtime projects' stories added, a small part of
the focused suite's 30 seconds. The fuzzy suite gains no sweep: a change
of settings, and a move, become two more operations its random worlds
draw, at the cost of the steps they take. Each milestone that adds them
measures both suites before and after on the gate; past a budget, it
finds the cost first, and if the cover needs more time than the budget
gives, it says so to the user rather than cutting the cover.

## 10. Open questions

- **How many cuts the focused suite takes** per scenario, and which: the
  commits around an effect, a claim and an answer are the likeliest to
  show a bug.
- **Coverage of the root's arms:** whether the harness should report
  which of the root's routes a suite never exercised, as transition
  coverage does for state machines.

Settings acceptance interim (engine.md,17.2; configuring E-QA-2 retargeted): Until held charter versions are implemented, a settings change is refused as full when a live agent task selects a model not present in the prospective charters, or its charter is removed. This newest-only refusal names the current revision and incompatible tasks; Stays and Moves do not avoid it. It is not the retained-version capacity rule of 17.8. A refused acceptance keeps the proposal pending. E-QA-2 covers newest-only compatibility, restart and no-partial-commit; retained-version capacity remains pending for F.
