# The store

Provisional, 2026-10-12, revised 2026-10-24. What lies beneath the
engine's store vocabulary: skein's key-value store, embedded in the
engine's process, holds every record jig's core and the application
keep, transcripts are files beside it, and secrets are in a second
store. jig ships the component that does this, `jig-protocol-store`; the
application hands it its own records as bytes it encodes. The domain's
half (commits, loads, what is kept) is `../domain/engine.md`, section 5,
and what the rest of the protocol layer relies on is `README.md`,
section 9. Read it with skein's `kv.md`. What is still open is listed in
section 16.

## 1. In one page

- **skein-kv, in the engine's process.** The engine's protocol layer owns
  `jig-protocol-store`, a component that owns two skein-kv stores (skein's
  `kv.md`): the store, with every record jig and the application keep,
  and the secrets' store. Keys are ordered bytes, values are bytes, both
  in memory, made durable by an append-only log and snapshots. There is
  no store process, no wire and no SQL: the engine is the store's only
  client.
- **One component, two owners of records.** jig's records (the core's
  and its children's) reach the component typed, in the core's
  vocabulary, and the component encodes them. The application's records
  (its connectors' and its own children's) reach it as key and value
  bytes the application encodes, under the application's tag. Both go
  into one commit (section 3).
- **Keys sort as the domain's keys do,** written with skein-kv's key
  writer, with a tag per owner, then per family. Every range the domain
  asks for maps to a short, fixed list of intervals (section 4).
- **Records from schemas,** with skein's generator: a codec per family,
  versioned, golden bytes and fuzzing. A format row names the versions,
  each owner's patch level and the deployment; a store of another
  version or deployment refuses to open, and before the first release
  nothing is translated (section 5).
- **Generations.** The maps live in numbered generations under the
  deployment's directory, `active` naming the open one. A store behind
  the binary's patch levels is patched at open in a copy, checked, and
  switched to by one durable rename; a crash anywhere leaves one whole
  generation active (section 2, 10.2 to 10.4).
- **One decision, one commit, the same number.** Each of the root's
  commits is one skein-kv commit with the same number. Group commit folds
  bursts into one sync. A failed or refused commit stops the engine
  (section 6).
- **Loads are answered in the step that asks,** from memory, in bounded
  pages: the restart's live ranges, parties' reads with their joins, a
  child's pages, single records (section 7). Only a turn's bytes, read
  from its transcript, take io.
- **Ended tasks are found through indexes,** the tasks child's: by their
  requester party, and by their project for those flagged listed, each
  by label and newest ended first. Live tasks are the tasks child's
  memory, never the store's to list (7.3, 7.4).
- **Transcripts are files, one per task,** each turn a frame written and
  synced before the commit that accepts it. The map keeps where each
  task's current window starts, so a resumed run reads only its window
  (section 8).
- **Secrets in a second store,** in a directory only the engine's user
  may read, written by components alone: minted credentials' digests,
  and LLM accounts' records and keys, each kept before it is used
  (section 9).
- **Growth is bounded by retention and fill:** the domain forgets an
  ended task once read, or shrinks it past a horizon on its timers, by
  the task's retention class, and refuses new work above a high-water
  mark of the map (section 11).
- **The real store in every world:** the domain worlds run the component
  over skein-kv's in-memory mode, with an in-memory stand-in for
  transcripts' files, and the store's own world crashes it on skein's
  simulated disk (section 14).

## 2. Where it sits

```
the application's root          commits (jig's writes typed, its own wrapped), loads, restart steps
        │ the root's store vocabulary, through the protocol layer's routing
        │   (the application's records encoded by its own codecs on the way down, decoded on the way up)
jig-protocol-store              keys and records to bytes; ranges to intervals; parties' reads and joins;
        │                       transcripts' files; secrets
        │ skein-kv's vocabulary (skein's kv.md, section 2)      │ io's file requests (skein's io.md, section 5)
skein-kv ×2                     the store; the secrets' store
        │ io's file requests
skein-io                        files: the ring, or the simulator's disk
```

- **A component of the engine's protocol layer,** step code under the
  subset (programming-model.md, section 10), with its limits, worst case,
  deadlines and most outputs per entry point. It depends on skein-lib,
  skein-kv, io's file vocabulary, skein-codec and its generated codecs,
  and the core's vocabulary (its keys, records, ranges and reads). It
  depends on no application's crate, and links no smith crate: turns are
  bytes to it.
- **On disk,** in the deployment's directory (`configuration.md`,
  section 3), which nothing else writes. The two maps live in numbered
  generations; `active` names the one a start opens, and an upgrade
  patches a copy into the next and switches to it (10.2). Transcripts
  are shared by every generation:

```
<deployment>/
├── active          the active generation's number: its decimal digits and a newline
├── generations/
│   ├── 7/          the previous generation, kept until the next switch
│   │   ├── store/      skein-kv: snapshot, log segments
│   │   ├── secrets/    the secrets' store: the directory 0o700, its files 0o600
│   │   └── manifest    what made this generation (below)
│   └── 8/          the active one, as 7
└── transcripts/    one file per task, in directories by the task's number
```

- **A first start** is a directory with no `active`, and none of
  `store/`, `secrets/` and `transcripts/` at its top (today's layout,
  below). A `generations/` that holds only a `1/` whose `store/` and
  `secrets/` hold no file, beside at most its manifest or the manifest's temporary
  (`manifest.skein-<16 hex>`), is a first start a cut stopped before `active`
  named it (the stores' first open, which makes their files,
  comes after `active`): it is removed, with any temporary io's durable replace
  left beside `active` (`active.skein-<16 hex>`; skein's `io.md`, 5.2).
  A `generations/` that holds anything else, with no `active`, is
  refused, `Generations::Unnamed`, and left as it is: a generation
  whose `active` was lost, which the operator names again from a
  backup. The start then makes `generations/1/` with its empty
  `store/` and `secrets/` (each directory durable as io makes it), and
  generation 1's manifest; then writes `active`, `1`, by io's durable
  replace expecting no file there (`Store` with `Absent`); then makes
  `transcripts/`. The maps are empty: the root's first commit writes
  the format row, as it does today (5.3).
- **Strays.** `active` names a generation only once it is whole
  (10.4), and at most it and its previous (`active` less one) stand:
  every other entry of `generations/` is a stray, a copy a crash left
  before its switch or a generation a crash left in its drop, and every
  open removes them once the active generation has opened and nothing
  refuses (10.1). Nothing reads the previous generation; it is what an
  operator goes back to, until the active one's first commit of its
  own (temper's `draft/upgrades.md`, 3.2).
- **The manifest,** a file per generation, written by io's durable
  replace into the generation's directory before `active` names it,
  and never changed: a record of the component's own schema
  (`Manifest`, beside the format row in the component's family, 5.1,
  added to `component-v3.schema` in place, as the levels are: 5.3),
  encoded and versioned as every record. It names:
  - `generation`, the generation's number, and `from`, the one it was
    patched from, none for a first start's;
  - `before` and `after`, each owner's levels (5.3), none before its
    baseline; a first start's before levels are all none, and its after
    levels the binary's;
  - `counts`: each kept family's count of records before its patches
    and after, one per family the binary's sets keep, in their order (a
    first start's each 0 and 0; none with no sets). Only kept families
    are walked: every other record is copied byte for byte, and no patch
    adds or removes a record, so the check before the switch refuses a
    count that moved (10.2, step 5);
  - `store` and `secrets`, the last commit of each map when patching
    ended: the store's, which the deployment's header names (section 6),
    and the secrets' store's; for a first start, 0 and none respectively;
  - `written`, the wall-clock time it was written, `env.wall` in its own
    unit.

  Nothing at open reads it: it is the operator's record of what an
  upgrade did, and what temper's `upgrade --check` and `versions`
  print (temper's `draft/upgrades.md`, 3.1).
- **A directory in today's layout** (`store/`, `secrets/` or
  `transcripts/` at its top, and no `active`) is a store from an older
  build, made before generations. Generations never have the first
  two at the top, and a first start makes `transcripts/` last, after
  `active`, so no cut leaves it alone; a new store beside it would
  number its tasks again over its files. Open refuses it, `OlderBuild`
  with `Older::Directory`, before anything is written, and leaves it
  untouched (5.3). Before the first release nothing is translated:
  the operator stops the service, moves `store/`, `secrets/` and
  `transcripts/` out of the deployment's directory together (its
  transcripts belong to its tasks), and starts again, which is a first
  start. The refusal's text says so, naming
  the directory:

  > `<deployment>` holds a store from an older build, in the layout
  > before generations (`store/`, `secrets/` or `transcripts/` at its
  > top), and this start has changed nothing in it. Before the first
  > release a store is not translated, so every start refuses this one
  > until it is moved. To start afresh, stop the service, move
  > `store/`, `secrets/` and `transcripts/` out of `<deployment>`
  > together, and start again: nothing is deleted, and the next start
  > makes a new, empty store. The new store has no tasks, sign-ins,
  > accounts or keys, and every client signs in again.

- **`OpenRefusal::Format`** refuses another version of a family (5.3),
  putting the family's lower-case name in place of <family>:

  > `<deployment>` holds a store whose <family> family is at another
  > version than this build's, and this start has changed nothing in it.
  > Before the first release a store is not translated, so every start
  > refuses this one until it is moved. To start afresh, stop the service,
  > move `active`, `generations/` and `transcripts/` out of `<deployment>`
  > together, and start again: nothing is deleted, and the next start
  > makes a new, empty store. The new store has no tasks, sign-ins,
  > accounts or keys, and every client signs in again.

- **`OpenRefusal::Deployment`** refuses a format row naming another
  deployment than its header (5.3):

  > The store in `<deployment>` names another deployment than its header
  > does, so its files come from two deployments, and this start has
  > changed nothing in it. Restore `<deployment>` whole from one backup,
  > or, to start afresh, stop the service, move `active`, `generations/`
  > and `transcripts/` out of `<deployment>` together, and start again:
  > nothing is deleted, and the next start makes a new, empty store.

- **`OpenRefusal::Numbers`** refuses a recovered last commit other than
  the one the header records (section 6):

  > The store in `<deployment>` does not end at the commit its header
  > records, so a file was lost, cut or restored apart from the others,
  > and this start has changed nothing in it. Restore `<deployment>` whole
  > from one backup, or, to start afresh, stop the service, move `active`,
  > `generations/` and `transcripts/` out of `<deployment>` together, and
  > start again: nothing is deleted, and the next start makes a new,
  > empty store.

- **`OpenRefusal::Failed`** stops an open when a file fails or a store
  cannot recover (section 10):

  > A file in `<deployment>` could not be read or written, or its store
  > could not be recovered, while this start opened it, and the start
  > stopped there. Check the disk's space and the directory's permissions,
  > and start again. If the next start fails the same way, restore
  > `<deployment>` from a backup.

- **Each `Older` names its own layout's fix,** in a sentence its
  refusal's text carries whole:
  - `Directory`: "To start afresh, stop the service, move `store/`,
    `secrets/` and `transcripts/` out of `<deployment>` together, and
    start again: nothing is deleted, and the next start makes a new,
    empty store."
  - `Unrecorded`, `Layout` and `Unleveled`: "To start afresh, stop the
    service, move `active`, `generations/` and `transcripts/` out of
    `<deployment>` together, and start again: nothing is deleted, and
    the next start makes a new, empty store."
- **A backup** copies, with the engine stopped, the deployment's
  directory whole, as before. Online, which stays open (section 16),
  it would copy `active`, then the generation `active` names, then
  `transcripts/`, in that order, and rest on recovery's rule: no switch
  happens while the engine serves (patching is open's, 10.2), and a
  transcript's frames past the committed length the copied map names
  are cut at the next open (8.4). The previous generation needs no
  copy for a backup of the deployment as it runs.
- **Opened before the root starts** (section 10; `service.md`,
  section 3).

## 3. The boundary

What the component takes and answers, as the engine's protocol layer
routes it. The core's side is `../domain/engine.md`, sections 5 and 6;
the root wraps it, and the protocol layer's routing unwraps it, encoding
the application's part with the application's codecs.

| Down, to the component | Up, from it |
|---|---|
| **commit** n: jig's writes, typed (the core's saves and erases of its records and its children's, and a task's transcript dropped whole); the application's writes, each a key's bytes and a value's bytes, or an erase | **committed** n, with the store's fill (11.2); **failed** n; or **FrameTooLarge** n, with task, full encoded bytes and page limit (section 6) |
| **restore** a restart step's range: jig's (the core's live families, 7.2), or the application's, as byte intervals under its tag | jig's records, typed, then done; the application's rows as bytes, then done |
| **load** a page of a child's range: jig's, or the application's intervals | the page's rows, typed or as bytes, and a cursor if more remain |
| **read** a party's read: its scope, position, direction, rows and bytes (`../domain/engine.md`, 5.3) | the page, each row with its joins, and its cursor; or busy, or gone |
| **get** a record by key: jig's or the application's | the record, or none |
| **transcript**: a task's current window, from the turn the core names as its start; a turn by its task, attempt and number; a page of a task's turns from either end | the turns, with their fields and bytes, and a cursor; or unread |
| **secrets**: keep, replace, erase, get and load, by owner and number; carry a value under a drawn number; adopt a carried value into its source's row (section 9) | kept, stale, erased, the value or none, a page, or the carried number and digest |

- **Turns are records of the core's** (`../domain/engine.md`, 5.4): the
  core saves each accepted turn, keyed by its task, attempt and number,
  with its spend, the last message it read, its time, whether it opens a
  window, and its bytes. The component keeps the bytes and those fields
  in the task's transcript file, and the transcript's places in the map
  (section 8). The core never erases a single turn: a task's transcript
  goes whole, by retention (11.1).
- **Reads see what is durable.** A load, a read, a get or a transcript
  sees every commit answered before it was asked, and nothing that is
  not durable (skein's `kv.md`, section 2). Each load of the core's is
  held until every commit made before it is answered
  (`../domain/engine.md`, 4.2 and 5.3), so it sees what the decisions
  before it wrote.
- **One terminal per request,** tagged with the asker's token
  (programming-model.md, 4.4).

## 4. Keys

### 4.1 The encoding

- **Bytes sort as the domain's keys do.** Every key is written with
  skein-kv's key writer (skein's `kv.md`, 2.1) so that byte order equals
  the order Rust derives for it: a variant's index as a tag, then its
  fields in order; numbers big-endian at fixed width, narrower ones
  widened; an absent option before a present one; a byte string escaped
  and terminated, so a shorter one sorts before its extensions; a list of
  byte strings as each string marked, then an end mark.
- **Keys never hold payload:** numbers, small fixed identities, and
  names within their limits.
- **Written out, not generated.** skein's generator writes values, whose
  length-prefixed bytes do not sort (skein's `codec.md`, section 3), so
  each family's key has a small encoder and decoder of its own, beside
  its codec. A property test holds each encoder to the derived order on
  generated keys, and each decoder to the encoder.

### 4.2 Tags

The first byte of every key names its owner:

| Tag | Owner | Then |
|---|---|---|
| 0 | the component's own rows | its family: the format row (5.3), transcripts' places (8.1) |
| 1 | jig | the child's tag, in the order of the core's key: the core's own, then each child's; then the family's key; and one row of the component's, the installation's identifier, at its reserved key (4.4) |
| 2 | the application | its own key bytes, which it encodes by the same rule, so that bytes sort as its keys do |

- **The application's bytes stay under its tag.** The component prefixes
  them, and refuses an application's key or interval it would have to
  write outside it, which is a bug, as a contract break.
- **A child's keys are one interval,** and each family one interval
  within it, since the encoding follows the keys' nesting.

### 4.3 Ranges to intervals

The domain asks for ranges as predicates over its keys; the store reads
intervals. So:

- **each range maps to a short, fixed, ordered list of intervals,** in the
  component, and a page that exhausts one interval with room left goes on
  into the next in the same step, since skein-kv answers loads at once;
- **families loaded together are adjacent:** within each child, the
  families a restart loads come first, and history after them
  (`../domain/engine.md`, 5.5), so each child's live part is one interval;
- **a test checks each mapping** against the range's predicate over
  generated keys: every key the predicate admits lies in an interval, and
  every key in an interval is admitted.

### 4.4 jig's families

What the store keeps of jig, by owner. "At start" families are read by
the restart (section 10), the others by a load, a read or a get when
wanted. The families' keys are the children's; the order within each
child puts live first.

| Owner | Family | Key | Loaded |
|---|---|---|---|
| component | the format row | none: one row | at open |
| component | the installation's identifier, at its reserved key in the accounts' family (01 04 03): drawn by the shell at each start (`configuration.md`, section 8) and kept by the store's first commit, beside the format row (5.3), so that each later start's draw is ignored | none: one row | at open, handed up as the store opens, never by the restart's ranges (7.2) |
| component | transcripts' places: each task's committed length and its current window's start; each attempt's first turn | task; task, attempt | at open, the run proofs' tasks' lengths (8.4); the rest when read |
| core | the deployment: its id, its numbers (the next project's among them, past every project seeded or made), the last commit | none: one row | at start |
| core | settings (`../domain/engine.md`, 17.7): the defaults the newest revision was made with, a record per setting; the changes over them, a record per setting changed (its value whole, or its removal, with the revision that made it); the held revisions' records; the charter versions held revisions read and the newest does not; pending proposals' sets | under the core's family tag 8: kind, then scope (0 for the deployment, a project's number), then number, for defaults and changes; revision; charter, then the revision that made it; proposal | at start, after the deployment's row, before every other family of the core's |
| core | settings' history: the records of revisions no longer held, each with its number, the revision it was made against, when, who decided it, the proposal it came from, whether it moved running work, and its items with the values they set; within the history limit, the oldest erased by the decision that makes a newer one | under the core's family tag 9: revision | on demand |
| core | goals' projections (feed, plan rows, pending milestones), run proofs, calls committed since a live task's last turn, live agent tasks' conversations (their first attempt, their current window's start) | a goal's, then its own; task; task, attempt, completion, position; task | at start |
| core | turns (in transcripts' files, section 8), terminals, decisions on proposals and escalations | task, attempt, turn; task, attempt; proposer, proposal; task, revision | on demand |
| tasks | live tasks, each with its revision and its claim's (`../domain/engine.md`, 17.3), stubs naming ended results, the funders' live periods, writer slots, pools' slots, pending proposals | task; task; funder; slot; proposal | at start |
| tasks | ended tasks, past periods, decided proposals, history, milestones, activity | task, then a position or revision; funder, period; proposal | on demand |
| people | parties, sign-ins (no secret), roles, projects' names and whole policies (their ceilings, escalation roles and overrides of templates among them), seeded and made alike, read positions over results and over live conversations, answered keys within the retention, a party's or, for the deployment's acts, a connector's (`../domain/people.md`, 5.3), including program declarations and checks, their declared names, and sign-out/removal answers naming unconfirmed configured workers with their original failure reasons | as the people child keys them; a live conversation's position by party, then task | at start |
| people | ended conversations' read positions (7.4) | task, then party | at start, by the census, every row (7.2); otherwise on demand |
| accounts | issued program names, including retired ones, with their source number and original holder and desired sign-out (key tag 4, after the installation's reserved tag); the next number the accounts child gives, below which it gives none again; LLM accounts: each one's holder, LLM provider, subject and display name, and its uses; their sources, each at its place in its holder's order, with its kind, its name for a program's, and its hold: limited, with its reset as a wall time where it has one, refused, or moving (a signed-in source whose record is out with a move, or back and not yet refreshed: `../domain/accounts.md`, section 6). The rest of a source's state is heard again at each start, and none is kept per host (`../domain/accounts.md`, 3.5) | source for an issued name; none: one row; account; account, source | at start, issued names before live account/source records |
| notes | entries, and an index line per entry per scope | name; scope, name | when a scope is in use; entries on demand |
| tasks | the results' index, the inbox's census's (7.2): a row per result of a task a party requested, at its position among that party's results, past the tasks child's live families | party, position | at start, by the census, past each party's read position over results |
| tasks, people | indexes for parties' reads (7.4): each party's inbox by time; ended tasks by requester party and by project; ended delegates; a subtree's delegates | the index's own, newest first where it reads so: an ended index's by party or project, then label, ended at reversed and task | on demand |

Children that keep nothing durable (the views, the fleet, authority,
briefs) have no family. What a run reports as it goes (its text, its
activity) is not stored: each committed turn holds what the run said
and did. Nor is a program's login on a host, which its host reports
(`llm.md`, 5.5), or a sign-in under way, which a restart ends
(`../domain/accounts.md`, section 5). An account's values are in the
secrets' store (section 9).

## 5. Records

Core-v6's projection records include the final header's
`closing_at: Option<u64>` and a failure receipt keyed by goal and connector
under projection tag 4. The receipt holds its source fence and Armed,
Pending or Reported state, with source and original receipt wall time
for Pending and Reported (../domain/connectors.md, 8). These shapes change
in place in core-v6; their generated Core layout fingerprint changes.
An old Core layout is refused by name before row decode and without store
mutation. Versions are unchanged; no migration or automatic wipe is added.

The current tasks family is version 9: its durable `MessageKind` adds
`FinishRequested { revision }` for a cancel grace. Tasks version 8 stores
refuse to open under the existing format check; no migration is added.
Component2/core5/people8/notes1 remain unchanged. The accounts family
is version 2: a source's hold adds *moving* (`../domain/accounts.md`,
section 6); accounts version 1 stores refuse to open likewise.

Settings (`../domain/engine.md`, section 17) add the core's two
settings families (4.4), under a schema of their own, `settings`, whose
version the format row names beside the others' (5.3), so that their
records change independently of the core's; and a revision on each task's
record and claim changes the tasks family's schema. Until the first
release, each family's version keeps its current number, and its schema,
generated code and one set of golden bytes change in place. Versions
move at a release, not when a change lands (skein's `codec.md`,
section 5). The settings' first version defines every record of 17.7,
charter versions
and pending proposals' sets among them, whichever change first
writes them. As every family's: each record stored at the top is
versioned; a bytes field that holds a record or pieces of one says so;
the keys' root type is jig's own and explicit. A store of an earlier
version refuses to open under the format check (5.3). Before the baseline,
a store from an older build is started afresh when its families change:
its version does not distinguish the changed layout, and nothing is
translated. A deployment made again from the same document,
with no change stored, behaves as it did. Each record of a setting
holds its value whole, a charter's instructions as text; their sizes
follow from the limits that bound each kind (5.4).

### 5.1 Codecs

- **One schema per family,** generated by skein's generator (skein's
  `codec.md`), used by the component only: the core's records, each
  child's, the component's own. The domain's records are translated to
  and from the generated ones with small total functions, exhaustive both
  ways, so a domain change to a durable field breaks a translation's
  build the day it happens.
Open's ending authority is carried in tasks v7 and people v7. Core's
schema remains v4. These are the versions the format row requires.

- **Each record carries its family's version,** as every generated record
  does (skein's `codec.md`, section 5), which is what an upgrade will
  need.
- **A collection the domain bounds by its bytes in all is one piece**
  (skein's `codec.md`, 3), its limit the domain's sum: a verdict's
  fields by the result's bytes, an amendment's lists, an authority's and
  a policy's by the amendment's and the policy's bytes. Only a
  collection the domain bounds by a count of its own is a counted list.
  So a family's limits, made from the domain's (5.4), are never past
  its ceilings for a count the domain does not have, and a record's
  worst case is the sum, not a count times its largest item.
- **Account preferences are separate from claims.** People's retained asks
  keep a choice's optional account number, including through amendments and
  proposed batches. Tasks keep that preference and their last claimed account
  separately; the run proof keeps every provider/account/source binding.
- **Authority models retain their selector** (`../domain/authority.md`,
  section 3). Literal ALL is distinct from a named set, including an
  explicit empty set and a set of all 64 configured models. The people
  family (version 5) keeps one bounded name context per policy row or retained
  ask. Tagged authority masks reference that context, including nested Create members, Amend, compound operations,
  proposals and ChangePolicy roles. A retained ask's names are unique and sorted by their
  bytes (policy snapshot contexts follow configuration order), at most 64, each at most 4,096 UTF-8 bytes; these retained names
  are priced separately from an authority's other bounded payload.
  Omission is distinct too: Create defaults to the creator's models,
  Amend keeps the target's, an edited role keeps its models and a new
  role takes the project's ceiling. Explicit selectors replace models
  under that ceiling; literal ALL never stands for a frozen named set.
- **Retained request identity survives removal.** A retained ask keeps
  its original canonical selector, never remapped for equality. An exact
  retry returns its retained answer before current model-availability
  admission; another selector on that key is KeyConflict. A new key
  naming an unknown model is refused. Restoring effective policy maps
  names to today's configuration, dropping removed names without an
  error; only literal ALL gains a newly configured model. Answers and
  pending proposals retain their identity and outcome. A policy row
  holds no ceiling: each start restores it under configuration's.
  The tasks family's version 5 rows retain only names their own named
  selectors reference, in canonical byte order, with masks reindexed to
  that context. Literal ALL and unrelated variants retain no names.
  Live/ended authorities, pending or decided proposals, recurring members
  and amendment history remap effective masks on load. These are effective
  snapshots; the original keyed ask remains people's.
- **Added dependencies keep when and by whom.** The tasks family's
  version 6 rows keep each live task's dependency with whether it was
  named when the task was made or added by an amendment, and for an
  added one its wall and its maker, a party or a task
  (`../domain/tasks.md`, section 7); a hold for a circle of waits, with
  the circle's tasks, cut at their limit, and when the held task began
  to wait for holds (`../domain/tasks.md`, 6.7); and, for a task that left
  its queues and came back, when its wait bound counts from
  (`../domain/tasks.md`, 6.2). The tasks family's activity also keeps a
  parked run's `Kept` line, its attempt and pending dependencies
  (`../domain/tasks.md`, 5.7; `../domain/engine.md`, section 7), and the
  `Met` notice, the dependency and whether it was removed
  (`../domain/tasks.md`, 8.1). The index of holders,
  waiters and dependents is not stored: the restart rebuilds it from
  the live tasks, and judges their waits whole (`../domain/engine.md`,
  section 6). People's retained answers keep the refusals for added
  dependencies as the client's contract has them
  (`../client/domain/contract.md`, section 4).
- **Golden bytes** in one set per family, for its current schema until
  the first release (skein's `codec.md`, section 5); a fuzz target per
  family's decoder; bounds at their limit and one past it.

### 5.2 The application's records

- **Its own codecs,** generated the same way, in its protocol layer: it
  encodes each record and key before the component sees it, and decodes
  each row the component hands back, before its root sees it.
- **Its store version,** one number the application gives the component
  when it makes it. Until the first release it stays fixed while its
  encodings change in place; it moves at a release (skein's `codec.md`,
  section 5), as its payload version does for the channel
  (`channel.md`, 4.1).

### 5.3 The format row

- **What it records:** the version of the component's format (its keys'
  encoding, its frames, its own rows), the version of each of jig's
  families (the accounts' and the settings' among them), the application's store version,
  and the deployment's id.
- **Written in the first commit,** with the deployment's header, by the
  component, and the installation's identifier with it (4.4), so that a
  crash keeps both rows or neither, and the identifier is durable before
  any sign-in sends it.
- **Checked at open:** a store whose row names another version of
  anything, or another deployment than its header, refuses to open,
  saying which; so does one whose installation's identifier is missing
  or unreadable. Pre-release, nothing is translated (`README.md`,
  section 7).
- **The levels** (temper's `draft/upgrades.md`, 4.2): beside the
  versions, the row names each owner's patch level, an optional
  number per owner whose records the store keeps: jig's, the
  application's, skein's (its OAuth records in the secrets' store) and
  smith's (its turns' fields, 8.1). They are fields of the component's
  `Format`, added to `component-v3.schema` in place, its generated code
  and goldens made again: no version moves before the first release
  (temper's `draft/upgrades.md`, 4.1), and the layouts tell an older
  build's store apart (`Older::Layout`). None is an owner before its
  baseline, which every owner is before the first release; a level is
  its owner's count of landed patches from its baseline. The row is the
  table of applied patches: the levels and the versions they imply.
  - **Written** by the first commit, with the binary's levels, and by
    patching's last commit (10.2), with the levels patched to; by no
    other commit.
  - **Read first,** by the kept text of the component's version the row
    names (skein's `codec.md`, 11), so a store behind reads its own
    row; before the component's baseline, by its current codec, as
    today.
  - **Compared at open,** per owner, after the component's version and
    with the families' versions, before the header and the deployment
    (section 10, step 2):
    - both none: the versions and the layouts decide, as before the
      baseline;
    - first, any owner's store level past the binary's: `Ahead { owner,
      found: Some(level), wanted }`, since no patch runs backwards (a
      level is past none); the first in `Owner`'s order, upstream first;
    - then any owner's store level none where the binary's is set:
      `OlderBuild` with `Older::Unleveled`, including another owner's
      none in a row whose jig level is set (temper's `draft/upgrades.md`,
      4.1; the baseline layouts remain open, section 16);
    - then any owner's store level behind: the patch path (10.2).
      No jig patch moves a family of jig's own yet, so jig's families'
      versions and layouts are held to the binary's. The application's
      version is not compared: its patches rewrite it to the binary's
      in the sealing commit. The deployment and the header's commits
      are checked as before; both version checks, the row's own and
      the header's deployment's, follow this rule;
    - equal: the versions and the layouts must be the binary's, or
      `OlderBuild`.

    Every refusal comes before anything is written but skein-kv's
    recovery (10.1). A row at a component version past the binary's
    is a later jig's, since from the baseline the component's version
    moves only by jig's patches: `Ahead { owner: Jig, found: None,
    wanted }`, its levels unread.
- **The layouts,** in a row of their own beside it (`Layouts`, the
  component's): each of jig's families' fingerprint, the digest of its
  layout, computed from the schemas built in and committed beside their
  generated codecs, which the drift test checks against them, so a
  build holds them as constants. Written with the
  levels, by the first commit and patching's last; a store with no such
  row is `Older::Unrecorded`, and one whose fingerprint of a family is
  not the binary's at its version `Older::Layout(family)`, the first
  in `Family`'s order. Both refuse before the root starts, and the
  store is left as it is (section 2); the refusal's text names the
  directory. `Unrecorded`'s:

  > `<deployment>` holds a store from an older build, made before its
  > families' layouts were recorded, and this start has changed nothing
  > in it. Before the first release a store is not translated, so every
  > start refuses this one until it is moved. To start afresh, stop the
  > service, move `active`, `generations/` and `transcripts/` out of
  > `<deployment>` together, and start again: nothing is deleted, and
  > the next start makes a new, empty store. The new store has no
  > tasks, sign-ins, accounts or keys, and every client signs in again.

  `Layout`'s, for the settings family (the others name theirs):

  > `<deployment>` holds a store from an older build, whose settings
  > family's layout differs from this build's, and this start has
  > changed nothing in it. Before the first release a store is not
  > translated, so every start refuses this one until it is moved. To
  > start afresh, stop the service, move `active`, `generations/` and
  > `transcripts/` out of `<deployment>` together, and start again:
  > nothing is deleted, and the next start makes a new, empty store.
  > The new store has no tasks, sign-ins, accounts or keys, and every
  > client signs in again.

### 5.4 Sizes

`jig_protocol_store::sizes::frame(domain, transcript_bytes) -> Option<u32>`
returns the maximum complete current TurnRecord frame for an admitted
ordinary transcript byte bound. It copies the current codec limits,
changes only that field's bound, uses the generated current record sizing
and adds the actual frame wrapper shared with the encoder. A bound past
the existing codec ceiling or an arithmetic overflow is refused, never
clamped. Applications deriving page room combine it with the checked
map key-plus-value bound; row, queue, read-slot and decoder ceilings are
separate and need not grow with this admission counter.

The component computes, from the domain's limits and the application's,
the largest encoded key, value and commit, and refuses to start when
skein-kv's limits are smaller (its key, value, operations and commit;
skein's `kv.md`, section 7), so a commit the root admitted is never
refused as too large.

- **The largest commit is what it can leave in the map,** not its
  writes times the largest row: each key a commit puts is in the map
  once it is applied, at most skein-kv's budget in all, since a commit
  writes each key once (section 6) and the fill keeps the map within
  its budget (11.2). So the largest commit is the map's budget, plus,
  for each operation the journal's writes allow, a key and its framing.
  A deployment's commit then fits skein-kv's `u32` whenever its map's
  budget does.

## 6. Commits

- **Numbers are shared.** The journal numbers the root's commits, and
  skein-kv numbers its own from one, across restarts. Every root commit
  is one skein-kv commit, and nothing else writes the store but
  patching, before the root starts, whose last commit writes the header
  with its own number (10.2), so the numbers are equal whenever the
  root runs. The component checks this as it submits, and stops on a
  mismatch. At start, skein-kv's recovered last commit must equal
  the commit the deployment's header names, which every writing decision
  saves; a store that disagrees refuses the start.
- **Writes become operations** in order: a save a put of its encoded key
  and record, an erase an erase, the application's bytes under its tag.
  A key written more than once in one commit becomes one operation, its
  last write, which leaves the map as the writes in order would:
  skein-kv applies a commit whole (skein's `kv.md`, section 7), so
  nothing sees the writes between.
  A turn's save becomes a frame in its task's file, written first
  (8.2), and puts of its place in the map.
- **In order, whatever waits.** A commit with turns goes to skein-kv once
  their files are synced, and the commits after it queue behind it, so
  numbers reach skein-kv in order. The wait is one sync, and only for the
  commits behind a turn.
- **Group commit.** While a sync is in flight, commits queue, and the next
  write carries them all (skein's `kv.md`, section 4). The root's bound
  on commits in flight is checked at start against skein-kv's queue and
  queued bytes, and the component's queue behind transcripts' syncs, so
  the store never answers a commit busy.
- **Committed n** goes up when skein-kv answers that commit, which makes
  every earlier one durable too, with the store's fill (11.2).
- **A failure stops the engine.** skein-kv answers failed for every commit
  in flight when a write or a sync fails or stalls, and a failed write or
  sync of a transcript fails its commit the same way. The component
  answers failed, the root stops (`../domain/engine.md`, 5.1), and the
  process ends; the next opens the store, which recovers. The engine
  never carries on over a store that recovered beneath it.
- **A refusal is a failure.** Past the checks at start, a commit skein-kv
  refuses (too large, the map full) would be a decision already made and
  not kept, so it stops the engine as a failure does. Section 11 keeps
  the map from filling.
- **An oversized transcript frame refuses the whole commit before data
  IO.** Each actual encoded current frame, including all metadata, payload
  and its wrapper, must fit the store's page byte bound `P`; equality is
  admitted. All frames and map operations are prepared before any of that
  commit's data is written, so a later oversized turn leaves none of the
  refused batch on disk. Its one terminal is
  `FrameTooLarge { number, task, bytes, limit }`, where `bytes` is the full
  encoded frame length and `limit` is `P`. There is no additional `Failed`
  for that number. This terminal comes first, followed by the existing
  failures of earlier pending commits, and the component stops as for an
  ordinary commit failure. Previously durable data remains recoverable.
  The event is strictly in-process: no journal record, stored family or
  wire message carries it. An application feeds the rejected number to its
  existing uncommitted-commit policy and may report the named cause.
- **A commit's operations are bounded,** as skein-kv applies each commit
  in one step (skein's `kv.md`, section 7). Where removing a record would
  erase an unbounded number of rows (a transcript's attempts' places,
  8.5), its commit erases the row that names the rest, so nothing reads
  them from then on, and the rest, orphans, go in the commits that
  follow: the component adds a bounded number of their erases to each
  commit it makes for the root, within that commit's operations, until
  none is left. At open it finds any a restart left by the rows they
  name being gone. Every commit stays one decision's, numbered as the
  root numbers it.

## 7. Loads

### 7.1 Pages

- **Same step.** skein-kv serves a page from memory in the step that asks
  (skein's `kv.md`, section 2), and the component answers in that step
  too, through its outputs, never by recursion. Only transcripts' bytes
  take io (section 8).
- **The cursor is exclusive.** A page's cursor is the last key it
  returned; the next page starts just past it, and an absent cursor at
  the interval's beginning. skein-kv's ranges start inclusively, and its
  next key is the first it did not return, so the component asks from
  the cursor followed by a zero byte, the least key above it.
- **Two bounds.** skein-kv bounds a page by encoded bytes, the asker by
  rows and decoded bytes. The component keeps the longest prefix that
  fits both.
- **A row that does not decode** stops the engine, as a failed commit
  does: the store holds bytes it did not write, or that broke since.

### 7.2 The restart's ranges

The core's restart script (`../domain/engine.md`, section 6) asks the
root for one step at a time, and the root asks the store:

- **the core's live ranges,** in the order the core restores them: the
  deployment's header first, then the settings' live ranges (defaults,
  changes, held revisions, charter versions, pending proposals' sets),
  which every later record is judged against, then the accounts' family (its counter,
  accounts and sources: the interval stops before the installation's
  identifier, which the component keeps), then people's families (live
  conversations' read positions among them, ended ones never), then the
  tasks child's live families, then the core's own live rows (goals'
  projections, run proofs, calls, agents' conversations). It is a fixed
  list of intervals, in that order, paged and handed up typed. The root
  has the accounts child want its sources only once these are restored
  (`../domain/accounts.md`, 3.5);
- **the inbox's census,** a step of its own once people's read positions
  are restored: the family of ended read positions, one row per ended
  conversation not yet erased (7.4), then the results' index from each
  party's read position over results (4.4), parties in the census's
  order; each row joined to its ended task's record by a point read;
  never every ended task (section 15). It then reads each unanswered
  question's activity line by the position its live task keeps (15.7),
  to rebuild the core's bounded, volatile question facts: the original
  time and words, never another durable record. The census:
  - hands up an ended read position, then its task's record, and a
    result's row only as its task's record;
  - never parts a position from its record: a page counts both against
    the asker's rows and bytes, and ends before a pair that does not
    fit, so a root asks the census for at least two rows;
  - goes on from the last index row's key. The census places each key
    it admits in the walk's order (`Census::interval`), so that a root
    that keeps its own cursors finds the interval to go on from (4.3);
  - restores each ended task once per party, whichever part names it:
    neither part's progress skips a row of the other;
- **each connector's live ranges,** as the application's byte intervals,
  handed up as bytes for its codecs;
- **what the core never restores** (turns, terminals, history, ended
  tasks, notes' entries) is never read at start.

### 7.3 Parties' reads

A party's read (`../client/domain/contract.md`, section 6) names a
scope, a position, a direction and its bounds. The component serves each
scope as intervals over jig's families, and joins each row in the same
step by point reads, those its scope's rows carry: its owner task, live
or ended; its requester ancestry, within the domain's depth limit; its
links; the record it relates to. A task's activity joins its owner
alone, once a page, and each turn's line its turn (below).

| Scope | Served from |
|---|---|
| a task, a person task, a result | the task's record, by key, with its last attempt's terminal |
| a proposal | a party's: its record, by key, with its decision; a task's: the row of its proposer's history that keeps it |
| an escalation | its decision, by the revision its task's record names, with the hold's milestone |
| a question, a reply | the task's activity line, by its position, with the line that answers a question, where its answer was heard (below), or the turn a reply was (from the transcript, 8.3) |
| a turn | the task's transcript (8.3) |
| a task's activity, newest first | the activity family, read backwards (7.4); each turn's line with its turn, from the transcript (8.3); its owner task joined once for the page, and no row's ancestry or links, since an activity line names its task alone |
| a task's history, or a subtree's | the history and milestone families; a subtree's by the index of its delegates, task by task: the task's own rows, then each delegate's in that index's order, each task's rows in its own history's order |
| a task's ended delegates | the index of ended delegates by requester |
| an inbox group | the index of the party's inbox, by time: what needs them oldest first, updates newest first; a group, and the filter, select within it |
| a task query's live tasks | not the store's: the tasks child's live tasks, scanned in memory |
| a task query's ended tasks, naming a requester party | the index of ended tasks by requester, from the party, and the label where the query names one |
| a task query's ended tasks, naming a requester task or a parent | the index of ended delegates by requester |
| a task query's ended tasks, naming a project and none of the above | the index of ended tasks by project, from the project, and the label where the query names one: listed tasks only |
| a project's notes, a scope's notes | the notes' lines of the project's scopes, or of the one scope, filtered by description, each with its entry: by scope, then by name within it, as the lines are keyed |
| a project's spend | the funders' periods of the project |

- **A task query's ended part** (`../client/domain/contract.md`, 6.1)
  is served from one index, chosen by the query's keys in the table's
  order: a requester party; a requester task or a parent; a project. A
  query naming none of these has no ended part, and its page says so.
  The keys beyond the index's prefix (the label, where it is not in it;
  the root; the phase class; the executor kind; tracked) filter its
  rows. Its rows come in the index's order: within a label, newest
  ended first from the two ended indexes; by task from the ended
  delegates', whatever order the query asks of its live part.
- **A task query's row joins** its task's record, by key: the ended
  record, or its summary once retention has shrunk it (11.1); and, for
  a task flagged conversation, its requester's ended read position, by
  a point read (7.4).
- **A page goes on from the last row it examined,** in its scope's
  order, not from the last it returned. A page that skips rows (an index
  row whose owner is gone, 7.4) or leaves them out (an inbox group, a
  description's filter, a task query's filter beyond its index's
  prefix) goes on past them; a page that reached its bound
  and returned none is cut, not ended, whichever its scope and whichever
  bound it reached (rows, bytes or joins), and carries the position it
  reached, so the client reads on from the row it did not take. A
  subtree's history is in its scope's order, task by task,
  its position the task and the row.
- **Joins are point reads** in the same step, priced within the read's
  bounds: a page's rows times the joins each may need. An owner lookup
  is charged one join, whether the owner is found live, found ended
  after its live record is missed, or not found: a miss decodes
  nothing, so the second probe needs no join of its own.
  A turn's is read from its transcript, and answers once its frame is read.
  An activity page's joins are its owner's, once, and one for each turn, so its
  first row needs at most the owner's and a turn's.
- **A page of a task's activity carries its turns.** Each turn's line
  comes with its turn: its spend, the last message it read, its time,
  and smith's bytes. A turn and its line commit together, so a page's
  turn lines are the transcript's frames in the same order, one each,
  and the component reads them in one walk (8.3). A turn counts its
  bytes with its line's against the read's byte bound, and one join:
  - a turn whose line and bytes together pass the read's whole byte
    bound comes with its size in place of its bytes, and counts without
    them, so no page comes back empty for a turn;
  - one that fits the bound but not what is left of the page ends the
    page before its line, and the next page starts with it;
  - a frame that does not read whole (8.3) comes as unread, and ends
    the page.
- **Single reads go by key.** Where a task's record does not name the
  key a single read needs (a question's line and its answer's, an
  escalation's hold, a task's proposal in its proposer's history), the
  domain names it there, or keeps an index (section 15).
- **An answer is read where it was heard** (`../domain/engine.md`, 7.2;
  who may read it, `../client/domain/contract.md`, section 6): the
  question's record names the answer by its task and position, and the
  read joins it by a point read, as the record it relates to.
- **A read past its bounds** is cut and says so, as every load is
  (`../domain/engine.md`, 5.3).

### 7.4 Indexes

skein-kv has no secondary index (skein's `kv.md`, section 1). Where a
scope's order is not a family's key order, the domain keeps an index: a
family of small rows its owner saves and erases in the same decision as
the records they point at, with keys reversed in time where a page reads
newest first. Each index is its owner's, tested in its world. Section
15 lists those the domain owes.

**The ended indexes,** the tasks child's, each saved in the decision
that ends a task (`../domain/tasks.md`, section 3):

- *ended by requester:* key `(party, label, ended at reversed, task)`, a
  row for every ended task whose requester is a party;
- *ended by project:* key `(project, label, ended at reversed, task)`, a
  row only for an ended task flagged *listed*. The flag is the task's,
  set from its template when it was made, so the engine reads no
  template to index.

Each serves a task query's ended part from its prefix, the party or the
project, then the label (7.3). A filter beyond the prefix scans the
index's pages, which may come back cut. A task moved to a party
(`../domain/tasks.md`, section 7) is indexed by the requester it has
when it ends.

**Read positions over conversations,** the people child's: one per task
flagged *conversation*, its requester's, while that requester is a
party (`../domain/people.md`, section 6):

- *a live conversation's,* in a family keyed `(party, task)`, loaded at
  start, so their bound in memory stays the live conversations, one
  each;
- *an ended one's:* the decision that ends the task moves its position
  to the family of ended read positions, keyed `(task, party)`, read on
  demand. A read of the conversation and a task query's row join it by
  a point read (7.3); the inbox's census reads the family through
  (7.2);
- *erased with the task:* by retention, as it shrinks a kept task
  (11.1); or in the decision that forgets a task forgotten once read
  (below).

**An owner erased first.** When a task's record is erased before the
records its index rows name, as a task forgotten once read is
(`../domain/people.md`, section 6; 11.1), its decision erases the rows
its record names whole: its result's update, its rows in the ended
indexes, and its ended read position. Its replies' updates stay, since a
conversation's replies are unbounded, a decision is bounded, and the
task's record names only its newest; they go when retention erases the
reply lines they name (11.1). Until then a page skips a row whose owner
is gone, counting it within its bounds, so nothing of a forgotten task
shows and no reply read comes back unread.

**Reading backwards:** a family whose positions are dense (activity)
reads newest first by computing forward intervals from its last
position, which its owner task's record names; a family read newest
first by time is keyed reversed. skein-kv's load runs forwards only
(skein's `kv.md`, section 12).

## 8. Transcripts

### 8.1 A file per task

- **One file per task,** named by the task's number, in directories by
  the number's high bits, so each directory holds a bounded number of
  files. The name is derived: nothing lists the files.
- **A turn per frame,** appended in commit order: the turn's attempt and
  number, its spend, the last message it read, its time, whether it opens
  a window, then smith's bytes, with a checksum, and the frame's length
  at both ends, so the file can be walked from either end.
- **A frame keeps the versions it was written in,** with no field of its
  own. Its body is the core's `TurnRecord`, a versioned record, so its
  first two bytes are the core family's version; its `transcript` is
  smith's turn, versioned on its own (smith's `protocol/transcript.md`,
  section 6), so its first two bytes are smith-transcript's version, and
  the core's schema says so: `transcript: bytes 8388608 holds
  smith-transcript Turn` (skein's `codec.md`, section 3). No frame is
  rewritten: one behind the binary's versions is patched each time it is
  read (8.6). The word is core-v6's in place, before the baseline: it
  moves the family's layout and no byte (skein's `codec.md`, section 9),
  so a store of the build before it is older (5.3) and starts afresh, as
  every store before the baseline does (temper's `draft/upgrades.md`,
  4.1).
- **The map keeps each transcript's places,** in the component's own
  family: its committed length and the offset of its current window's
  first turn, one row per task with a transcript; and each attempt's
  first turn's offset, one row per attempt. A turn is found from its
  attempt's first turn, by walking forwards.
- **Windows.** A frame whose turn opens a window moves the task's
  current window to it, in the same commit. A run that starts fresh opens
  a new conversation, whose first turn opens a window (smith's
  `protocol/transcript.md`, section 3). A patch marked fresh closes
  every live task's window, in the patched copy (10.2, step 4): the run
  proof's window says so (core-v6's `record Window { attempt: u64 turn:
  u32 closed: bool }`, the flag in place before the baseline, beside the
  holds word and with it).

### 8.2 Written before its commit

The core commits a turn with its spend, its run proof and the messages it
read, in one decision, and acknowledges the host once that commit is
durable (`../domain/engine.md`, 7.2). For a commit with turns, the
component takes the following steps.

Before step 1, it encodes and checks every frame against `P` as described
in section 6, retaining each accepted encoding once for the pending batch.
No frame from a refused commit is placed on the file IO queue.

1. writes each turn's frame at its file's end, opening the file
   (`OpenWrite`), or creating it where it is missing. The end is the
   committed length, plus the frames queued ahead and not yet durable,
   since a task's next turn may be decided before its last one commits;
2. syncs each file it wrote, and, for a file it created, the directory
   that holds it, and that directory's own, for a directory it made;
3. submits the commit to skein-kv, with each transcript's new places among
   its operations;
4. answers committed once skein-kv has.

A crash after step 2 and before the commit is durable leaves a frame past
the committed length. The next open cuts the file to its committed length
(8.4), and the host, never acknowledged, sends the turn again.

### 8.3 Read back

- **Resuming:** a run that resumes is given its current window's turns,
  from the window's first turn to the end, within the resume limit
  (`../domain/engine.md`, 7.2), read forwards a page at a time while the
  task prepares. The core asks only when its conversation has a window,
  and names the turn that opens it, as it keeps it (`../domain/engine.md`,
  5.4); a window that starts at another turn fails the read as unread.
- **A turn,** for a party's read: from its attempt's first turn, walking
  frames forwards to its number. A turn read alone is bounded by the
  read's byte bound (7.3): past it, the read answers the turn's size in
  place of its bytes (`../client/domain/contract.md`, section 6).
- **A page's turns,** for a party's page of a task's activity (7.3):
  backwards, one frame for each turn line, from the frame of the page's
  newest turn: the transcript's last frame when the page reads from the
  activity's end, otherwise found as a turn is. A turn past the page's
  bound has its fields and length read, not its bytes.
- **A page of a task's turns,** forwards or backwards, from a cursor that
  is an offset the component gives and takes back.
- **Direct complete-turn requests** that cannot produce a turn under the
  read's room terminate `Unread`, never a retryable empty page at an
  unchanged offset; a nonempty prefix may give a continuation. A party's
  single-turn `TooLarge` answer is unchanged.
- **A tail,** for a fresh run's brief: the window's last turns as text,
  each rendered by the host's protocol layer when loaded, and each turn's
  text at most the tail's own budget, as the core's memory prices it.
- **Never past what is committed.**
- **Current bytes only:** each kind hands on a frame at the binary's
  versions, one behind patched as it is read (8.6).
- **A frame that fails its checksum** fails the read as unread, and so
  does one its machines refuse (8.6). The core
  treats the transcript as one the agent cannot use: the run fails as
  transient and the next starts fresh (`../domain/engine.md`, 7.2); a
  closed window's tail it cannot read, it leaves out.
- **Open files are bounded:** the component keeps a bounded set open,
  live tasks' first, and opens others to read them.

### 8.4 After a crash

At open, before the root starts, the component reads the core's run
proofs, one for each live task ever claimed (4.4), and cuts each such
task's file to its committed length, syncing it: zero for a task with no
places yet, whose first turn's commit was lost; a missing file is left
missing. Only a crash leaves bytes past a committed length, since a
failed commit stops the engine, and only a claimed task has frames, its
claim durable before its run was placed (`../domain/engine.md`, 5.2).
After the cut every file holds exactly its committed turns. An ended
task's file was cut while its task lived, and is not touched again.

### 8.5 Dropped

Retention drops a task's transcript whole (11.1): the commit erases the
task's row of places, and the component removes its file and syncs the
directory before it submits the commit. The task's attempts' places,
as many as its attempts, are orphans from that commit on, read by
nothing, and go in the commits that follow, a bounded number each
(section 6, "A commit's operations are bounded"). A crash
before the commit leaves places whose file is gone: a read of it answers
as for a dropped transcript, and retention's next pass drops it again.
Removing a missing file is not a failure.

### 8.6 Patched as read

Patching never writes a transcript (10.2): the transcripts are most of
what a deployment holds, and a frame is only ever read. So a frame
behind the binary's versions is patched each time it is read, and
nothing is written.

- **The machines** are the frames' families' (10.3,
  `KeptFamily::frames`): one `Patcher` each, the body's family in jig's
  set and the turn's in smith's, made at every open that serves (10.1)
  from the sets it was handed, after the frames' checks of 10.3, each
  kept text kept and each of its owner's patches planned, and held by
  the transcripts' part for the whole run. They are not patching's
  (10.3), which go with the copy's open. A family with no kept text has
  none: every real set is empty until the baseline, so no machine is
  made and every read runs as it does today.
- **A frame's versions** are read with its head, past the frame's
  length: the body's first two bytes, and the turn's first two, past the
  `transcript` field's length, after the fields the head reads already.
  With smith's machine, a turn of fewer than two bytes is unread.
- **A body behind** (its version kept by jig's machine, before the
  binary's) runs jig's plans from its version to the binary's, from the
  frames' root (found in the frame's version by the kept texts' renames,
  as a held root is, `patching/walk.rs`), `next_pass` after the first.
  At its held turn the machine stops (`Step::Held`): at the first pass,
  smith's machine patches the turn from its version to smith's last
  kept, and its bytes go back with `put_content`; the passes after carry
  them (`carry`). A body at the binary's version is decoded as before,
  by the binary's codec, and its turn, if behind, is patched by smith's
  machine alone, from smith's frames' root.
- **One rule for versions.** Where a family has a machine, a version
  among its kept texts and before its last is patched, its last goes on
  as it is, and any other, older than its first or past its last, is a
  halt (`Decode` or `Order`), so the frame is unread. Where a family has
  none, its bytes go on as they are, as today: a body its codec cannot
  read is unread (8.3), and a turn's reader refuses a version not its
  own (smith's `protocol/transcript.md`, section 4).
- **Owners' order.** The copy runs owners' patches in their pins' order
  (10.3); a frame runs jig's first and smith's at the held turn. The two
  agree because neither reads the other's bytes: jig's written functions
  on a frame's body never read its `transcript`, and smith's read their
  turn alone (smith's `protocol/transcript.md`, section 12).
- **Nothing is written.** The read hands on the patched bytes and the
  file keeps the frame's own, so its lengths, its checksum and the map's
  places stay true, and the frame is patched again at its next read. A
  cut anywhere in a read leaves every file as it was.
- **Every read kind** of 8.3 (resuming, a turn, a page's turns, a page
  of turns and a fresh run's tail, the last read as resuming's is) reads
  its frames through one step, the frame's decode, and the machines run
  there. The core, a party's read and a host's protocol layer are handed
  current bytes alone: `jig-protocol-transcript` reads smith's current
  version only, as smith's agent does.
- **Bounds on patched bytes.** A frame behind is read whole and patched
  before anything is judged: its size, against a page's bound, a turn's
  (a party's charge, the page's row and the turn's bytes) and the resume
  limit, is its patched body's and its 12 bytes of framing. Where either
  machine is made, every frame is charged so, behind or not, so a frame's
  charge does not hang on its version, and a turn refused past a bound
  (`TurnFailure::TooLarge`) names that size. A direct Window/Page read
  charges the whole frame, including metadata, payload and framing,
  whether or not a machine is made. With no machine, a party's read
  charges the turn's bytes. The head alone (a turn past a
  page's bound, 8.3) serves a frame at the binary's versions only: a
  head whose body or turn is behind is read on, whole, from the bytes
  already read. A patched record past its machine's bytes or nodes
  halts (`Room`).
- **A halt is an unread frame.** A frame the machines refuse (any
  `Halt` of skein's `codec.md`, 11.4, a site's `NoSite` among them) is
  read as one that fails its checksum (8.3), in every read kind.
- **A written site goes up,** as patching's do (10.3): at
  `Step::Written` the component answers `Site` to the read's own token,
  `of` the site's owner, its `key` the frame's name (the task's key in
  the component's places, 4.4, then the frame's offset in its file,
  eight bytes big-endian), the site, the node and the machine, moved;
  the read waits for `Request::Site` with the result. A shell answers a
  site at once, by its owner's generated `written`, with a `Default`
  `Context`, its key set: reads come in any order and a frame is patched
  at each, so a frame's patch reads that frame alone (skein's
  `codec.md`, 11.5). The component's routing of `Request::Site` follows
  its phase: a walk's while patching, as today, and the read waiting at
  a site while serving, its token the read's; any other is refused, as
  a site no walk waits for is today. No new variant: a shell that hands
  sets answers `Site` while serving as while patching, and one that
  hands none never sees it.
- **One read at a time,** as today (`transcripts/read.rs`): a read
  waiting at a site holds the component's read, commits wait behind it
  as behind any read (`transcripts/write.rs`), and a machine serves one
  frame from `start` to `finish`.
- **Memory:** each machine's `Patcher::worst_case_heap` of its capacity,
  in serving's worst case (section 13), priced at startup; none for sets
  with no kept text. A capacity's versions, patches and text are its
  owner's kept versions, patches and largest text. Its bytes and nodes
  are its frames' bounds, never the map's: jig's, the frames' largest
  body and the most nodes any kept body takes, its held turn one node;
  smith's, the `transcript` field's bytes and the most nodes any kept
  turn takes. Both are counted from the frames' root in each kept text
  (`transcripts/patches.rs`, `node_bound`), so every frame the kept
  texts allow is read and none is refused for its nodes. A turn's bytes
  do not bound its nodes (a list of empty records takes none), so
  smith's real turn, 16 messages of 64 blocks, takes 2,109,520 nodes,
  and its machine is priced at that capacity from the baseline on, about
  298 MB with what its tables hold while they grow (skein's `codec.md`,
  11.7); a machine holds only what the largest record it has met needed
  (section 16).
- **Windows across a mark.** A patched turn is the same conversation in
  current bytes, as smith's patch makes it; one of smith's patches may
  say instead that a window across it starts fresh (`mark
  fresh-windows`). Its plan closes the live tasks' windows in the copy
  (10.2, step 4), so no run resumes from turns patched across it; a tail,
  a party's read and a page read them patched as any others. No attempt
  is live across such a patch: its upgrade is a full one, the workers
  stopped first (temper's `draft/upgrades.md`, 7.2), which temper's
  section 9 is to say of a pending patch marked fresh (section 16).

## 9. Secrets

- **A second skein-kv store,** in `secrets/`, opened as a private
  directory (skein's `io.md`, 5.3): the directory `0o700`, its files
  `0o600`, owned by the engine's user. The store, its snapshots and its
  logs hold no secret.
- **What it keeps,** each under its owner's tag and the number the domain
  gives it:
  - minted credentials' digests, by their sign-in's number, with their
    expiry: each credential the clients' component mints, for a printed
    link's sign-in, a service's, or an identity the application's server
    verified (`client.md`, section 8);
  - services' credentials' digests, by the service's number;
  - each signed-in source's record, by the source's number: its tokens,
    the client id the provider issued, the scopes granted and the ID
    token, as skein's accounts driver hands it to keep (skein's
    `oauth.md`, 6.4; `llm.md`, 5.1), or as a moved record brought it
    (`llm.md`, 5.6);
  - each LLM key, by its source's number, adopted from the value its
    request carried (`client.md`, section 5);
  - an application's own records, under its tag: its connectors'
    credentials, which it decodes itself, by numbers below 2^63;
  - each value a request carried (`client.md`, section 5), an LLM key
    or an application's value, under the application's tag, by a number
    jig draws itself from 2^63 up, never one the request names: the next
    number is a row of its own beside the values (key `[2, 1]`), written
    in the commit that keeps the value, so a number is never drawn
    twice, an erased one included, and a number already holding a value
    is never written over: the carry is refused and nothing is kept.

  A program source's login is not here: it is the program's own, on
  each host (`llm.md`, 5.5).
- **Written by components alone,** through the protocol layer's routing.
  The domain names a secret by number, never holds one, and never sees
  the secrets' store's commits, which have numbers of their own.
- **Kept before used.** A sign-in is committed first, which gives it its
  number; its credential's digest is then kept, and durable; only then
  does the clients' component answer the credential, to the client that
  signed in or to the application's server that asked it to mint
  (`README.md`, section 8). A crash between leaves a sign-in no client
  holds a credential for, which expires with its time.
- **A record kept before lent.** skein's driver takes this store as its
  keeper (skein's `oauth.md`, 6.4): each new record goes out as a keep,
  which the routing writes here under its source's number, stating the
  digest of the record it replaces, and answers `kept` once it is
  durable; the driver lends the record's token only then. A crash
  between leaves at worst a record its driver never lent, which the
  next start lends from.
- **A carried value kept before the core sees it.** The clients'
  component takes the value out of the request carrying it as it
  decodes it, and keeps it here before the core gets the request, which
  names it by its number and digest only (`client.md`, section 5).
  Where the core leaves the request unused (refused, a key sent again
  under a key answered or in flight, or the engine stopping) it says
  so, naming the number, and the component erases that value, if its
  own decode kept it; replacing or erasing a kept value goes through
  the core's decisions alone.
- **A pasted address held, never kept.** A sign-in's pasted address
  (`llm.md`, 5.4) is carried as a key is, with one difference: the
  store holds it in memory and writes nothing, under a number of a
  range of its own, drawn in memory, which names a value of this run
  alone. The core names it by number and digest as it names a carried
  value, and leaves its request unused whatever the answer, so the
  clients' component erases it once the request is answered, its one
  eraser; the accounts component only reads it, and a value already
  gone answers its finish not waiting. The store has no clock and
  drops a value by no time of its own: the accounts child answers
  every finish by its sign-in's end at the latest, and the clients'
  component then erases it. The store drops what it holds at its
  close; a crash or a restart loses it with the sign-in it was for,
  which never survives one. Held values are bounded by the clients'
  component's carried values at once, each by the driver's redirect
  bound, in the worst case; past them the store holds nothing and the
  request is answered busy.
- **An LLM key adopted before granted.** The accounts component checks
  the carried value against its byte bound, then asks the store to
  adopt it into the source's row, naming the digest it replaces, or
  none for a new source. Adoption writes the source's row and erases
  the carried row in one secrets commit; a missing carried value or a
  changed destination digest answers stale and changes neither row. The
  component grants the key only once adoption is durable, a replaced
  key as its next generation (`llm.md`, 5.1). A later erase of the
  carried number is a no-op. A source's record or LLM key occupies one
  secret row under a fixed-size store key.
- **A moved record adopted the same way,** in one secrets commit
  (`AdoptValue`, beside the key's `Adopt`) that writes the source's row
  and erases the carried row: the row holds
  the record the carried file holds, in skein's form, not the file's
  bytes as they came (`llm.md`, 5.6), and the adoption states the
  carried value's digest, answering stale when it changed. The record
  is added to the driver only once adoption is durable, and lent only
  after its first refresh is kept.
- **A moved-out record is read, never copied in the store:** the
  routing reads the source's row once the driver has removed it, and
  the file's bytes made from it live only in the protocol layer until
  the client takes them (`llm.md`, 5.6). The row is erased when the
  move is written.
- **A presented credential names its sign-in:** a credential is the
  sign-in's number and its random bytes, opaque to the transport that
  carries it. The component gets the digest by number, and the clients'
  component compares in constant time.
- **Erased** when the core says a sign-in ended (signed out or revoked)
  or expired, or a service was removed; and when the accounts child
  says a source was signed out or removed, a key replaced, a record
  revoked, or a move written. A crash between that commit and the erase leaves a value
  nobody names, which the restart sweeps: once people's families are
  restored, the component erases each of jig's digests whose sign-in or
  service it did not hand up. For sources, the accounts child names
  each party source to restore before wanting it, then says the list
  is complete. The accounts component re-adds signed-in sources and
  loads keys by number; it then pages the source records once and
  erases each neither configured nor named, including orphaned keys.
  Configured sources are spared even without an accounts-family row.
  Every application row is spared: a carried value an application used
  may still live at its drawn number. A value orphaned by a crash
  before the core says used or unused remains there, including an LLM
  key not yet adopted; collecting those values is later work.
- **A failure stops the engine,** as the store's does (section 6): what a
  failed commit of the secrets' store was keeping is never handed out.
- **Read at start,** within its own limits and worst case: named sources
  by number, the sources' sweep in bounded pages.
- **Patched with the store** (10.2), in the copy's own secrets' store
  and its own commits, by the patch sets of the families it holds:
  jig's digests and records, the application's records under its tag,
  and skein's OAuth records by skein's set (temper's
  `draft/upgrades.md`, 6.3). Its numbers are kept as they are: the
  carried values' next number (`[2, 1]`, its own row) and every number
  a row is kept under, the sources' and accounts' drawn by the accounts'
  counter in the store (`[1, 4, 0]`), so that the start's sweep erases
  nothing it should keep and no number is drawn twice. Its last commit
  is named in the manifest (section 2). As built, a kept family names
  its map (`KeptFamily::map`, 10.3): a secrets family is walked in the
  copy's secrets' store, each page one commit of that store's own,
  within that map's limits and record room, and the seal stays the
  store's (the secrets' store has no format row or header). A fixture
  family of the application's records proves it (`jig-test-patches`'
  `fixture-token`), both numbers unchanged across the switch. Still
  open: the sign-in and service secrets are the shell's own 40-byte
  encoding and skein's OAuth records a hand codec of skein-oauth's (its
  `RECORD_VERSION`), neither from a schema, so neither has a kept text
  a patch could name, and no set keeps them (section 16).

## 10. Opening and restart

1. **The shell opens the deployment's directory,** and the component
   finds the active generation (10.1) and opens both its stores;
   skein-kv recovers each: the snapshot, then the log's longest valid
   prefix (skein's `kv.md`, section 3). A store behind the binary's
   patch levels is patched in a copy that becomes the active generation
   (10.2), and the open begins again over it.
2. **The component reads its own rows,** and the core's that it checks:
   the format row, refusing another version, another level or another
   deployment (5.3); the
   deployment's header, refusing a last commit other than skein-kv's
   recovered one (section 6); the run proofs' tasks' committed lengths,
   cutting each file to its length (8.4); and the installation's
   identifier, which replaces the one the shell drew (4.4). An empty
   store gets its format row and the installation's identifier in the
   root's first commit, with the deployment's header.
3. **The root starts** (`service.md`, section 3): the core's restart
   script asks for one step at a time, and the store serves each step's
   ranges (7.2). The accounts child's want saves its never-reused
   counter when it advances, and the ordinary account/source rows it
   makes for providers whose source is none. A first start with no
   accounts needs no such commit. Restarts retain those same numbers.
4. **Secrets are served** to the protocol layer's other components once
   the secrets' store is open: each signed-in source's record to make
   skein's driver with, and each key to the accounts' table, when the
   accounts child names the restored sources. After all are named, the
   component sweeps unnamed source secrets (`llm.md`, 5.1).

### 10.1 Generations at open

Before a map opens, the component reads the deployment's directory
(section 2). Nothing is removed or made until the active generation has
opened and nothing refuses. The shell seeds each of the store's file
drivers before the open, through `seed`, whose `Seeded` witness
`on_directory` takes: io names a `Store`'s temporaries from a driver's
randomness (skein's `io.md`, 5), and an unseeded driver refuses every
`Store`, a first start's write of `active` among them, so a shell that
seeds none does not build. The witness proves that three drivers were
seeded, not that they are the three the shell serves the store with.

1. **The lock:** io's `Lock` on the deployment's root, held as long as
   the process (skein's `io.md`, 5). `Busy` refuses,
   `Generations::Busy`: another process has the deployment open, and
   two starts never copy, remove or switch beside each other.
2. **`active`,** a whole-file load of at most 21 bytes: a generation's
   number in decimal, with no leading zero, and a newline; anything
   else refuses, `Generations::Unreadable`. With none, the directory is
   a first start's, or refused as today's layout (`Older::Directory`)
   or as generations no `active` names (`Generations::Unnamed`), as
   section 2 says.
3. **The active generation's maps:** its `store/` and `secrets/` must
   stand, or the start refuses, `Generations::Missing { generation }`.
4. **The stores open** (step 1), and the format row, the layouts and
   the deployment's header are read and checked (step 2, 5.3). Every
   refusal of the levels, versions, layouts and deployment comes here,
   and nothing has been written but skein-kv's recovery of the active
   generation's maps (its next segment, a leftover snapshot's removal;
   skein's `kv.md`, section 3).
5. **The strays removed:** every entry of `generations/` but the
   active generation and its previous, and any temporary io's durable
   replace left beside `active`. io has no recursive removal: each directory is
   listed from its first page, each file in it removed, and listed
   again until it is empty (a removal moves what a later page would
   show), then removed itself, deepest first; then `generations/` is
   synced. A start never removes the active generation or its
   previous.
6. **`transcripts/`** is made where it is missing.
7. **At the binary's levels,** the open goes on with the rest of step 2
   (the run proofs' cuts, the installation's identifier), as it always
   has. Behind them, it patches a copy (10.2).

Each refusal names the deployment's directory and what to do: another
process holding the lock is stopped first; an unreadable or missing
generation is restored from a backup, or `active` written again to name
one that stands; an older build's is moved aside (section 2). Each
`Generations` refusal's text, whole, as `OpenRefusal::words` gives it
(the deployment's directory where `<deployment>` stands):

- `Busy`: "Another process has `<deployment>` open, and this start has
  changed nothing in it. Stop that process first, then start again."
- `Unreadable`: "The `active` file in `<deployment>` does not name a
  generation, and this start has changed nothing in it. Restore
  `<deployment>` from a backup, or write `active` again to name a
  generation under `generations/` that stands, and start again."
- `Missing`: "The generation `active` names in `<deployment>` has lost
  its `store/` or `secrets/`, and this start has changed nothing in it.
  Restore `<deployment>` from a backup, or write `active` again to name
  a generation under `generations/` that stands, and start again."
- `Unnamed`: "`<deployment>` holds generations that no `active` names,
  and this start has changed nothing in it. Name one again from a
  backup of `<deployment>`, or write `active` to name the generation
  under `generations/` to open, and start again."

- `OlderBuild { why: Unleveled }`: "`<deployment>` holds a store from before
  the first release, which records no patch levels, and this start has
  changed nothing in it. A store from before the first release is not
  translated, so every start refuses this one until it is moved. To start
  afresh, stop the service, move `active`, `generations/` and `transcripts/`
  out of `<deployment>` together, and start again: nothing is deleted, and
  the next start makes a new, empty store. The new store has no tasks,
  sign-ins, accounts or keys, and every client signs in again."
- `Ahead`: "`<deployment>` holds a store from a later build, at patch levels
  past this build's, and this start has changed nothing in it. No patch runs
  backwards: start the later build again, or restore `<deployment>` from a
  backup this build made."
- `Patch`: "A patch refused a record while this start patched a copy of the
  store in `<deployment>`, and the generation it patched from stays active,
  unchanged. Every start of this build refuses the same record: start the
  build before it again, which opens that generation as it was."
- `Set`: "This build's patches do not plan, and this start has changed
  nothing in `<deployment>`. The build is wrong: start the build before it
  again."
- `Check { why: Large }`: "The check before the switch found a patched record
  too large for a commit alone in the copy of the store this start patched in
  `<deployment>`, and the generation it patched from stays active, unchanged.
  Every start of this build checks a fresh copy the same way: start the build
  before it again, which opens that generation as it was."
- `Check { why: Count }`: "The check before the switch found a family whose
  count of records the patches changed in the copy of the store this start
  patched in `<deployment>`, and the generation it patched from stays active,
  unchanged. Every start of this build checks a fresh copy the same way: start
  the build before it again, which opens that generation as it was."
- `Check { why: Load }`: "The check before the switch found a record the
  restart could not load within the deployment's limits in the copy of the
  store this start patched in `<deployment>`, and the generation it patched
  from stays active, unchanged. Every start of this build checks a fresh copy
  the same way: start the build before it again, which opens that generation
  as it was."
- `Check { why: Join }`: "The check before the switch found a record naming an
  ended task the store does not hold in the copy of the store this start
  patched in `<deployment>`, and the generation it patched from stays active,
  unchanged. Every start of this build checks a fresh copy the same way: start
  the build before it again, which opens that generation as it was."
- `Check { why: Private }`: "The check before the switch found a file in the
  secrets' store of the copy this start patched in `<deployment>` that is not
  private to the service's user, and the generation it patched from stays
  active, unchanged. A start makes its copy's files private, so another
  process or user changed one: make sure nothing else writes in
  `<deployment>`, and start again."

The refusals at open, beside section 3's others:

```rust
pub enum OpenRefusal {
    Format { family: Family, found: u32, wanted: u32 },
    Deployment,
    Numbers { store: u64, header: u64 },
    Failed,
    OlderBuild { why: Older },                                 // nothing translated before the baseline
    Ahead { owner: Owner, found: Option<u32>, wanted: Option<u32> }, // no patch runs backwards
    Generations { why: Generations },
    Patch { owner: Owner, patch: u32, key: Box<[u8]>, halt: Halt }, // a record the patch refused (10.2)
    Set { owner: Owner, patch: u32 },                          // a patch set that does not plan (10.3)
    Check { why: Check },                                      // the check before the switch (10.2, step 5)
}
pub enum Older { Unrecorded, Layout(Family), Directory, Unleveled }
pub enum Generations { Busy, Unreadable, Unnamed, Missing { generation: u64 } }
pub enum Check { Large { key }, Count { family, before, after }, Load { key }, Join { key }, Private { name } }
```

`Older`'s first two compare the layouts' row (5.3); `Check` names what
failed: a record too large for a commit alone, or a row that does not
load, by its key; a family whose count moved, with both counts; an ended
read position whose ended task is missing, by its key; a secrets' file
that is not private, by its name.

### 10.2 Patching a copy

The component's own phase, before the root starts, when the format row
names an owner's level behind the binary's (5.3). From the active
generation `A` to `A + 1`:

1. **Plan,** over `A` as 10.1 opened it: the levels, the patches
   between them and their order across owners (10.3), a set that does
   not plan refused; the header decoded by the kept text of its own
   level and checked, as is the deployment; one machine per kept family
   made (10.3); whether a pending patch is marked fresh (10.3); the room
   (below) against the limits. Every refusal of
   5.3, and an effect in flight that a pending patch changes (temper's
   `draft/upgrades.md`, section 8), comes here. Then `A`'s stores
   close: no transcript has been cut, which the open after the switch
   does. Then `A - 1`, where one stands, is dropped, since once `A + 1`
   exists going back is to `A`, which nothing here changes: renamed out
   of the numbers (`generations/dropped`), `generations/` synced, then
   removed as 10.1 removes a stray, so that a cut never leaves part of
   a generation where the previous stands.
2. **Copy** `A`'s `store/` and `secrets/` into `generations/A + 1/`,
   file by file, each read and written in bounded chunks through the
   file driver (io has no copy), each file synced and each directory
   made durable as io makes it, then `generations/` synced. The copy's
   `secrets/` is opened private (`OpenPrivate`: 0o700) and its files
   made 0o600, as section 9's are. `A` is only read.
3. **Open the copy:** both its stores recovered, and its format row,
   layouts and header read and checked at the copy's own levels, as
   10.1 (step 4) reads `A`'s; no transcript is cut.
4. **Patch,** one walk per patch, in the plan's order (10.3): its
   families' keys (`KeptFamily::keys`) walked in key order by pages;
   each record, at its block's `from`, decoded by that version's kept
   text, run through the patch, encoded by its `to`'s and put back at
   its key; a record at another version refuses, `Patch` with its key.
   Each page is one commit through the component's own path, within
   skein-kv's bounds (5.4); a record a page cannot hold alone refuses,
   `Check { why: Large }` with its key. The secrets' store is patched the
   same way, in its own commits, within its own limits (section 9).
   **Windows closed:** where a pending patch is marked fresh, a walk of
   its own after the plan's last patch, over the core's run proofs (4.4),
   one for each live task ever claimed, by pages, each page a commit as
   a patch's are: each proof is decoded by the binary's codec, since
   every core record is at the binary's version by then, and one with a
   window not yet closed is put back at its key with its window closed
   (core-v6's `Window.closed`), every other field as it was. A proof
   with no window, or one already closed, is not written, and a page
   with none to close makes no commit; a proof that does not decode, or
   decodes as another key's record, refuses `Check { why: Load }` with
   its key, as step 5 would. Nothing else is written, and no transcript:
   the frames are read patched (8.6), and the closed window is the
   core's to act on (`../domain/engine.md`, 7.2). The walk answers no
   `Patched`, since it is no patch, and adds or removes no record.
   The store's
   last commit, one of its own after the last page's, writes the format
   row's levels and versions and the layouts, the binary's (5.3), and
   the deployment's header as patched, naming that commit's number
   (section 6). The commits before it leave the header naming `A`'s
   last, which nothing reads: a cut removes `A + 1` (10.4), and the
   check runs after the last. The numbering carries on from `A`'s, and
   the journal's next commit follows patching's last. As each patch
   ends, the component answers `Patched` with its owner, number and
   counts, for the shell's line (temper's `draft/upgrades.md`, 3.1).
5. **Check,** before anything switches (temper's `draft/upgrades.md`,
   6.4), over the copy as patched and still open (skein-kv has no
   read-only open), writing nothing; the first failure refuses, `Check`:
   each kept family's count after is its count before, taken as the copy
   opened, since no patch adds or removes a record; every row of the
   core's live ranges and of the census's ended read positions (7.2),
   which the restart reads whole, loads by the binary's codecs within
   the deployment's limits, its key its record's; each ended read
   position's ended task is held, joined as the restart joins it; and
   each file of the copy's secrets' store is a regular file, private to
   the service's user, with one link. What the core's census chooses
   (the results after each party's read position, the unanswered
   questions' activity), the children's rules, strictly, and their live
   sets' limits are not checked here yet (section 16).
6. **The manifest** written into `A + 1` (section 2), by io's durable
   replace expecting no file there.
7. **Switch:** `active` replaced by io's durable replace, expecting the
   digest of the `active` 10.1 read, so that only the start that
   planned from `A` switches from it. One rename, never two standing
   for one.
8. **Open again** from 10.1: the root starts over the switched
   generation, opened as any later start opens it.

- **A refusal or a failure** at any step (a patch's `Halt`, a check
  failed, a file's fault) stops the start, answering `OpenRefused`
  with why: the record's key and the patch, or the check and what
  failed. Before the switch's rename, `A` stays active and unchanged,
  and the next start removes `A + 1` and begins again from a fresh
  copy. A switch answered `Failed` with its rename made (`committed`:
  skein's `io.md`, 5.2) may leave `A + 1` active, whole and checked,
  which the next start opens.
- **Room.** On disk, the maps twice (temper's `draft/upgrades.md`,
  6.1): `A` and `A + 1`, never the transcripts; `A + 1` is `A`'s maps and,
  beside them until skein-kv's next snapshot, the log of the patch
  commits, each record a walk rewrites once. A disk that cannot hold
  them fails the copy with a file's fault, and every start fails again
  until there is room: checking the free space before the copy is open
  (section 16). In memory: one map at a time (`A`'s closed before the
  copy's opens) and one machine per kept family (10.3), each
  `Patcher::worst_case_heap` of its capacity. Section 13's worst case
  counts the larger of patching's peak and serving's, never both.
- **The domain sees nothing of it** but the check's restore (step 5),
  and, after the switch, the windows a fresh mark closed (step 4).

### 10.3 Patch sets, the plan and written sites

- **Patch sets are data,** handed to the component's open by its shell
  beside the limits: for each owner whose records it keeps, the
  generated module's constants (skein's `codec.md`, 12.3), no function:

```rust
// jig_protocol_store
pub enum Owner { Skein, Smith, Jig, Application }   // upstream first
pub struct PatchSet {
    pub owner: Owner,
    pub name: &'static str,               // the owner as its patches' headers spell it
    pub families: &'static [KeptFamily],
    pub patches: &'static [&'static str], // `patches::PATCHES`, in number order
    pub fresh: Option<&'static str>,      // the mark across which windows start fresh, as its owner spells it
}
pub enum KeptMap { Store, Secrets }       // the map its records live in (section 9)
pub struct KeptFamily {
    pub name: &'static str,               // the family, as its texts spell it
    pub texts: &'static [&'static str],   // `patches::kept::<family>::TEXTS`, in version order
    pub map: KeptMap,
    pub keys: &'static [Keys],            // its records' keys, and no other's
    pub frames: Option<&'static str>,     // the declaration it is in transcripts' frames, where their machine starts
}
pub struct Keys {
    pub from: &'static [u8],              // inclusive
    pub to: &'static [u8],                // exclusive
    pub root: &'static str,               // the declaration its records are, where its machine starts
}
pub struct PatchSets {
    pub sets: &'static [PatchSet],        // at most one per owner
}
impl PatchSets {
    pub const NONE: PatchSets = PatchSets { sets: &[] };
}
```

  A family's records are its `map`'s, walked and committed there
  (section 9). A family's ranges hold records of different declarations (the
  core's tasks beside their turns), and a machine starts each record at a
  declaration (skein's `codec.md`, 11.4: `start(plan, root, record)`), so
  each range names its records' own. Keys' bytes: the application's are
  its own keys, relative to its tag, as its rows are (4.2: the component
  adds the tag), and in the secrets' map relative to its secrets' prefix
  `[2, 0]` (section 9); every other owner's are the map's whole keys.

  **The frames' families** (8.6) are the two whose `frames` names a
  root: in jig's set, the core's, whose `TurnRecord` is a frame's body;
  in smith's, smith-transcript, whose `Turn` the body's `transcript`
  holds, with no `keys`, since none of its records is a map's. Every
  other family's is `None`. A set with a frames' family in another slot,
  or two in one, is refused, `Set` with patch 0; so is a body whose held
  turn names a family other than smith's frames' one, judged from the
  kept texts, at the plan and at every open that serves, before its
  machines are made (8.6). Smith's set is smith-transcript's constants
  in the `Smith` slot (smith's `protocol/transcript.md`, section 12),
  its `map` `Store` and unused, its `keys` empty.

  A family's `keys` hold its own records and leave out what lies among
  them but is another's (the installation's identifier at `01 04 03`
  among the accounts', the settings' records among the core's): jig's
  from the component's own table (4.4), the application's from its
  keys' encoders (section 12). A set with no kept text is an owner
  before its baseline, level none; otherwise its level is its patches'
  count. Every real set is empty until the baseline (temper's
  `draft/upgrades.md`, 4.1), so no store is behind and open runs as it
  always has; the component's tests hand it fixture owners' sets:
  `jig-test-patches` in the application's slot and `jig-test-patches-jig`
  in jig's, so that the order across owners is shown, and
  `jig-test-patches-smith` in smith's, a fixture turn family whose frames
  are patched as read, with a patch marked fresh. No test owner names a
  production family. A fixture body family's last text is the core's
  `TurnRecord` at the binary's version under its own name, its
  `transcript` holding the fixture turn, since a patched body is decoded
  by the binary's codec; the fixture turn's last is its own, since the
  component hands a turn's bytes on undecoded. The names, texts and keys are `&'static`, the binary's own constants,
  never state: the only references in fields the component holds
  (programming-model.md, 10.3), read by its plan and by each serving
  open, which makes the frames' machines from them (8.6), since `keep`
  and `plan` copy what they need into each machine's tables.
- **The plan** reads each patch's header (skein's `codec.md`, 11.6:
  `header`, `level`, `after`): a patch's owner must be its set's `name`
  and its number its place there, or the set is refused, naming it.
  The manifest counts every kept family by its name (section 2), so the
  sets fit its bounds, at most 64 families in all and each name at most
  64 bytes, or the set where they pass them is refused, `Set` with
  patch 0, as two sets for one owner are: judged before the plan, and
  at a first start before anything is written.
  The order across owners follows the pins (temper's
  `draft/upgrades.md`, 4.4), downstream first. From the most
  downstream owner with patches pending, its patches run in number
  order; before a patch that names `after U L`, `U`'s pending patches
  up to level `L` run, each planned the same way, and none past it;
  after an owner's last, its nearest upstream's remaining ones, then
  that one's upstream's. A patch whose `after` names a level its
  upstream has already passed in the store, or one past the binary's,
  refuses the set: under the pins that is a binary built wrongly
  (section 16). The plan is a list of (owner, patch), its length the
  patches' count, made by a loop bounded by it.
- **The fresh mark** is a set's word (`PatchSet::fresh`), its owner's to
  spell and to mean: smith's `fresh-windows` (skein's `codec.md`, 11.1;
  smith's `protocol/transcript.md`, section 12), handed in by the shell
  as data, so the component names no owner's constant. The plan reads
  it with each pending patch's header (`Header::marked`): a set's
  patches past its level in the store and up to the binary's, one marked
  by its set's word enough. Then the copy's windows are closed (10.2,
  step 4); a set whose `fresh` is `None` marks nothing, and a patch's
  other marks are its owner's alone. A patch may carry a mark and change
  no family.
- **One machine per kept family,** a `Patcher` (skein's `codec.md`,
  11.4 and 11.6), made at the plan, on the patch path alone, with its
  capacity from the family's limits and texts, each kept text kept and
  each of its owner's patches planned. In a patch's walk a record is
  one block behind; a held value `k` versions behind runs `k` plans
  to the version its family has after the patch, `next_pass` after the
  first. In a patch's walk, its families' records run its plan,
  and a family the patch leaves alone whose records hold one it moves runs
  its identity plan (codec.md 11.3), which reaches the held fields alone.
  The frames' machines are others, made at each open that serves
  (8.6).
- **A written site goes up.** Step code holds no function (codec.md,
  11.4; programming-model.md, 10.3), so at `Step::Written` the
  component answers `Site` with the owner, the record's key, the site,
  the node and the family's `Patcher` itself, moved, never shared. The
  shell sets the key in that owner's `Context` (codec.md, 11.5), which
  it keeps for the whole patching (`Default` at its start), runs the
  owner's generated `written` over `parts()`, and hands the `Patcher`
  back in a `Site` request with the result; the component then calls
  `next()`. One record's walk waits on at most one site at a time. A
  frame read while serving sends its sites up the same way, to the
  read's token, with a `Default` `Context` for each frame (8.6).

```rust
// up                                      // down
Event::Site {                              Request::Site {
    owner: Token,                              owner: Token,
    of: Owner,                                 patcher: Box<Patcher>,
    key: Box<[u8]>,                            result: Result<(), Halt>,
    site: Site,                            }
    at: Node,
    patcher: Box<Patcher>,
}
Event::Patched { owner: Token, of: Owner, patch: u32, counts: Counts }
pub struct Counts { pub records: u64, pub commits: u64 }
```

- **The shell that answers** is the store's shell, the one that names
  the owners' crates and drives the store's open: the application's
  (temper's `temper-service`, its `Store`); jig has no production
  shell of its own. In jig's tests the store world's (`tests/store`) answers
  the fixture owners', while patching and while serving. `jig-test-service`'s
  hands no sets. `jig-test-store-files`' hands none either, unless a core
  world gives it fixture sets (`Store::with_patch_sets`, and `patch_sets`
  for the next open): then it opens over a fake machine's directory,
  so that the copy, the walk and the switch run, and it reads the
  patched proofs back through the component. Its fixture sets write no
  site, and both shells would answer one `Halt::NoSite` (skein's
  `codec.md`, 11.4: a site its caller has no function for), as their
  routes already do for any token.
- **What a shell downstream sees** (temper's `temper-service`, its
  domain world's driver, `temper-github-app`): open's signature stays
  as it is. The component holds `PatchSets::NONE` until a shell hands
  it sets, by a setter beside the constructors (`Store::patch_sets`,
  additive); with none it never answers `Site` or `Patched` and never
  wants `Request::Site`. The new refusals (10.1) reach shells whose
  refusal matches already end in a wildcard (temper's: `Format` its
  version stop, the rest unavailable), so these build as before, and
  nothing downstream matches `Older` or `Generations`. The new `Event`
  and `Request` variants do not build as before: a shell that matches
  either exhaustively gains their arms with the build that adds them.
  These shells are temper's store shell, engine, store route, accounts
  and clients, and its domain world's driver; and jig's test service,
  test stores and accounts worlds. Sets' new fields (`PatchSet::fresh`,
  `KeptFamily::frames`) reach no shell downstream, since none builds a
  set before the baseline; reads patched as they are read add no
  variant. The closed window is the core's (`../domain/engine.md`,
  section 6), and reaches temper's sources as no change.
- **Held fields** are the component's: at `Step::Held`, the held
  family's own machine patches the held bytes, its own sites going up
  as above, and its bytes are put into the holder's (`put_content`),
  or carried where that family does not move; the holder's machine
  waits meanwhile. No family holds itself through another, so one machine
  per family suffices: the plan refuses sets whose kept texts' holds
  close a cycle, `Set` with patch 0, before anything is written, and a
  walk that meets one still halts `Order`, refused as a patch's halt
  (10.2).
- **Keys do not move** by patch: a key is its owner's code (4.1), and a
  patch's text has no line that moves one (skein's `codec.md`, 11.1). A
  moved key would need its function's answer up and back, and a check
  that it lands on no record its walk has yet to meet. A family whose
  keys change waits for that (section 16); before the baseline none
  does.

### 10.4 Power loss

Every write, sync and rename above is a cut point. After each, the
next start opens one whole generation: the old, untouched, with the
partial one removed, or the new.

| Cut during | What the next start finds | What it does |
|---|---|---|
| a first start, before `active`'s rename | no `active`; at most `generations/1/` and a temporary | removes both; a first start |
| a first start, after `active`'s rename | `active` lost with its directory's sync, or `1` | as above, or opens 1 and makes `transcripts/` |
| removing strays | part of a stray | removes the rest |
| the drop of `A - 1`, before its rename | `active` is `A`; `A - 1` whole | patches again, its plan dropping `A - 1` |
| the drop, after its rename | `active` is `A`; part of `generations/dropped` | removes the rest as a stray; patches again |
| the copy, a patch commit, a closing commit, the check, the manifest | `active` is `A`; `A + 1` partial or whole | removes `A + 1`; patches again from a fresh copy |
| the switch, before its rename | `active` is `A` | as above |
| the switch, after its rename | `active` is `A`, or `A + 1` whole | as above, or opens `A + 1` |

- **A cut per write, sync and rename,** each run from its own start in
  the store's world (section 14), not their product.
- **No `active` names a generation before it is whole:** its maps,
  their directories and its manifest are synced before the rename that
  names it, and nothing writes a generation once a later one is
  active.
- **Windows closed with the copy:** the closing commits are `A + 1`'s,
  cut as a patch commit is, so the generation that the switch makes
  active holds every live window closed, and a crash after it keeps
  them so; `A`, which going back opens (temper's `draft/upgrades.md`,
  3.2), holds them as they were.
- **A read writes nothing,** patched or not (8.6): a cut anywhere in a
  read leaves every file as it was, and the next read patches the same
  frame the same way.


## 11. Growth

### 11.1 Retention

Retention is the domain's: mechanisms on its timers, tested in its
worlds, with horizons the deployment's configuration gives. What becomes
of an ended task is its retention class, set from its template when it
was made (`../domain/tasks.md`, section 3):

- **A task kept** stays to the deployment's retention horizon, and whole
  while none is set.
- **Ended tasks shrink** past a horizon: one commit erases a task's full
  records (its record, history, activity, terminals, its ended read
  position) and saves a summary, so what parties read of old work comes
  from summaries. Its rows in the ended indexes stay, and a task query
  joins its summary (7.3).
- **Transcripts go** past a longer horizon, whole, by their task (8.5).
  A live task keeps its transcript, whatever its age.
- **Answered keys expire** past the people child's retention.
- **A task forgotten once read** (`../domain/people.md`, section 6) goes
  in one decision: its record, its rows in the ended indexes, its
  result's update and its ended read position (7.4).
- **A forgotten task's remains go** at retention's next pass, whatever
  their age: its activity, history, terminals and transcript, and its
  replies' rows in its requester's updates (7.4). Nothing reads them,
  and the requester's pages of updates skip those rows until then.

### 11.2 Fill

- **The store reports its fill** with each commit's answer: the map's
  bytes against its budget (skein's `kv.md`, section 7).
- **Above a high-water mark** of that budget (both are limits,
  `configuration.md`, section 6), the domain refuses new work at its
  entrances: new tasks, new sign-ins. Work under way goes on,
  and a refusal is never a failed commit. The deployment's document
  shows the fill.
- **Transcripts' disk** is the deployment's: retention bounds it, and its
  directory's free space is the operator's.

## 12. The application's part

- **Its families,** under its tag (4.2), keyed by its own encoder by the
  same rule, encoded by its own codecs, with its store version (5.2).
- **Its restart ranges** as byte intervals under its tag, each live
  interval of each connector, served in the restart's step for that
  connector (`../domain/engine.md`, section 6).
- **Its loads and gets** by byte intervals and keys, answered with bytes.
- **Its indexes,** as records of its own, by the same rule (7.4).
- **Its secrets,** under its tag in the secrets' store (section 9).
- **What it writes is in the same commits** as jig's: one decision, one
  commit, whoever's records it holds.

## 13. Limits and the worst case

- **The component's limits:** both stores' (skein-kv's, their maps'
  budgets among them), the fill's high-water mark, the open files,
  the frames' largest bytes (a turn's, from the domain's limits), the
  queue of commits behind transcripts' syncs, the reads in flight and the
  joins a page may make.
- **Three file drivers,** one for each store (skein's `kv.md`, section
  12) and one for transcripts, so a turn's sync never queues behind a
  store's write, nor a store's behind a turn's.
- **The worst case** is both stores' (skein's `kv.md`, section 7: about
  three times each map's budget), the component's buffers and tables, and
  the three drivers' (skein's `io.md`, section 5). Patching's peak (10.2:
  one map open, one machine per kept family within its own map's record
  room, the walk's page and commit buffers at the larger of the two
  maps' bounds, since one walk serves both, the copy's chunk, and the
  closing walk's run proof decoded by the binary's codec beside its
  bytes) is the
  larger of it and serving's, since the root never serves while it
  patches. The sets are the shell's, so `worst_case_with_patches(limits, sets)`
  prices their machines, and `worst_case(limits)` prices `PatchSets::NONE`.
  Serving's holds the frames' machines (8.6), at most two, each its
  `Patcher::worst_case_heap`, and none for sets with no kept text: their
  bytes the frames' bounds and their nodes the most their kept texts
  allow (8.6). The tests price smith's real turn's machine against
  serving's (`tests/frame_memory.rs`), from a copy of smith's schema
  checked against smith-transcript's generated bounds.
- **Whole files:** the manifest (section 2) is read and written whole,
  and at its bounds, 64 kept families each named in 64 bytes, it is 5464
  bytes (88, and 84 for each family). The store's file driver's
  whole-file bound is at least that, checked at startup with the worst
  case, or the start is refused naming it; a test world's drivers are
  made with such a bound (skein-io's `FileIo::with_whole_limit`).
- **A page read whole:** a transcript frame of up to the page is read
  back in one read, so each file driver reads at least the page, checked
  at startup, or the start is refused naming it; a test world that
  raises its page raises its drivers' read with it.
- **Checked whole at startup** with the root's and every component's
  (`configuration.md`, section 6).

## 14. Testing

Each within jig's suites' budgets.

- **The real store in the domain worlds.** The worlds run the component
  over skein-kv's in-memory mode, in place of a fake of the map, so every
  story runs the real keys, codecs, intervals, paging and joins. No
  world fakes the store's component: the real one runs in each
  (testing-strategy.md, section 4).
- **Faults come from the world.** The world holds commits between the
  root and the component, in order: it delays them to make the store
  slow, and fails one to stop the engine. A crash keeps what reached the
  store and loses what the world held.
- **Transcripts' files have a stand-in** in the domain worlds: an
  in-memory file system answering the component's file requests, with
  the same rule after a crash. A crash keeps both in-memory stores and
  the stand-in, and the next engine's component is made over them and
  opens as section 10 says.
- **The store's world,** `tests/store`: the component over skein-kv and
  files on skein's simulated, crashing disk, a scripted root committing,
  loading, reading and appending turns, crashed at every file operation
  and sync. Its referee: after recovery, the last acknowledged commit, or
  that and one whole uncertain one, never part of one; no turn readable
  past its commit and none missing before it; numbers that match the
  header; pages equal to a model's.
- **Generations and patching,** in the store's world: a fixture owner's
  patch sets over a store at an older level, patched, checked and
  switched, then opened as at its levels; each refusal before any
  write; a second start refused `Busy` while one holds the lock; the
  copy's secrets private; a cut at every write, sync and rename of the
  first start, the copy, the patch commits, the manifest, the switch
  and the drop, each
  run from its own start, the referee finding one whole generation
  active, the other strays removed, and its records the patched or the
  old, never a mix.
- **Transcripts patched as read,** most of it at the lowest layer. Step
  tests in `jig-protocol-store` over `jig-test-patches-smith` (and a
  fixture body family in `jig-test-patches-jig`): frames written at each
  fixture version, read by every kind of 8.3, each handing on the bytes
  its patches' goldens give (skein's `codec.md`, 12.3) and the file's
  bytes unchanged; bounds judged on patched sizes; each halt, a
  `NoSite` and a turn past its kept texts among them, an unread frame; a
  version before a machine's first or past its last unread; a site up
  and back on a read, a commit waiting behind it, and a stray `Site`
  refused in each phase; the frames' checks refusing at a serving open;
  no machine and today's path with sets that keep no text; each
  machine's worst case against its measured peak; over skein-io's fault
  points, a cut at each step of a read changes no file. Fuzzed: a frame's decode with
  machines, over random fixture frames. One story in the store's world:
  a fixture store behind, patched with a fresh-marked patch, its live
  windows closed in the copy, a cut at each closing commit run from its
  own start, and after the switch a window read patched and a closed one
  found closed, going back finding them as they were. The core's side
  (a closed window starting fresh with its tail, counted as a
  transition) is the core's tests' (`../domain/engine.md`, 7.2), in its
  world, whose store closes the window by the walk itself: smith's
  fixture turn kept at one text, and a patch marked fresh that changes
  no family. The independent testing of each change is an independent tester's from this
  design, committed as `qa_*` tests the gate skips.
- **Step tests:** each key encoder against the derived order; each
  range's intervals against its predicate; each codec both ways, with
  golden bytes; refusals at open (format, deployment, sizes, numbers).
  Direct transcript pages either advance through a nonempty prefix or
  terminate `Unread` when no turn fits. Private phase tests refuse a
  closing run proof outside its decode limit, a mismatched key and proof,
  and a malformed run-proof key with `Check::Load` naming the offending
  key: the active generation and source stay unchanged, and the next
  start sweeps the abandoned copy. The existing closing-write fault and
  crash stories retain the switch-order evidence.
- **Fuzz targets** for each family's decoder and a frame's.
- **On the real ring,** the reference binary (`service.md`, section 7)
  killed around each phase of a turn's commit and started again. A killed
  process keeps the page cache, so this checks integration, not power
  loss (skein's `kv.md`, section 9).

## 15. What it owes and is owed

- **The domain owes:**
  1. **live before history** in each child's keys: the tasks child's
     history and ended tasks after its live families (stubs among
     these), decided proposals apart from pending ones, and past funding
     periods apart from live ones (4.3);
  2. **the restart reads live work only:** the inbox's census reads
     unread results from the results' index and ended conversations from
     the family of ended read positions, not every ended task, joining
     each to its record by a point read; the census hands each ended read
     position up with its record, and the people child keeps none (7.2;
     `../domain/people.md`, section 6);
  3. **indexes for parties' reads:** each party's inbox by time; ended
     tasks by requester party, and by project for those flagged listed,
     each keyed by label and ended at reversed, the flag read from the
     task, never from a template; each task's ended delegates; a
     subtree's delegates for its history (7.3, 7.4);
  4. **turns as the store keeps them:** saved with whether each opens a
     window, never erased one by one, a task's transcript dropped whole;
     a resume reads the current window, from the turn the core keeps as
     its start (8.3; `../domain/engine.md`, 5.4 and 7.2);
  5. **fill:** with each commit's answer, and new work refused above the
     high-water mark (11.2);
  6. **retention:** by each task's retention class, summaries of ended
     tasks kept, transcripts dropped, answered keys expired, and the
     remains of tasks forgotten once read collected, each on the
     domain's timer (11.1); a task forgotten once read erased in one
     decision (11.1; `../domain/people.md`, section 6);
  7. **keys for single reads:** a question's line and its answer's, an
     escalation's hold, and a task's proposal's row in its proposer's
     history, each named by its position in its task's record, as a
     reply's is, an answer's with the task whose activity holds it, so a
     read finds it by key, never by a scan (7.3);
  8. **read positions over conversations:** a live conversation's in the
     people child's family, loaded at start; moved, by the decision that
     ends its task, to the family of ended read positions, read on
     demand; erased with the task (7.4, 11.1).
  9. **the accounts' family:** the next number; each account with its
     holder, LLM provider, subject, display name and uses; each source
     with its place in its holder's order, kind, name for a program's,
     and its hold, limited with its reset or refused; restored at start,
     a record breaking the child's rules counted and left in the store;
     a source's record or key named by the source's number, and
     erased when the source goes (4.4, section 9;
     `../domain/accounts.md`, 3.5). The installation's identifier is the
     component's (4.4, 5.3).
- **skein owes:** io's `OpenWrite` and `Cut` (skein's `io.md`, section
  5), for 8.2 and 8.4. skein-kv serves as it is: its in-memory
  mode, its key writer, its fill. Its accounts driver takes the owner's
  store as keeper, made with the records it holds (skein's `oauth.md`,
  6.4).
- **The application owes:** its keys' encoders, its records' codecs, its
  store version, its live intervals, and its indexes, by the rules of
  section 12.

## 16. Open questions

- **Upgrades** are a pass at opening that patches a copy and switches
  to it (section 2, 5.3, 10.1 to 10.4). Still open:
  - values nested in another owner's family as they run across owners'
    levels (skein's `codec.md`, section 13);
  - effects in flight that a patch changes (temper's
    `draft/upgrades.md`, section 8). Transcripts are patched as they
    are read (8.6), and a fresh mark closes the live windows (10.2,
    step 4). A pending fresh mark requires a full upgrade, with workers
    stopped and every live attempt ended before the switch even when
    their wires match (temper's `draft/upgrades.md`, section 9). The
    application's stop must enforce this before switching;
  - pieces held by a frame's turn (smith's `Found.hits`, `pieces Hit`)
    are copied, not opened, so a smith patch that changes `Hit` is
    refused `Pieces` until they open (skein's `codec.md`, 11.3 and
    section 13);
  - the channel's records the core keeps as bytes, a settled call's
    `name` and a delivery's `evidence` (core-v6), which from the
    baseline say what they hold (skein's `codec.md`, section 3): smith's
    channel records are versioned with the channel whole, not each, so
    they take a word only once smith keeps them as a family of its own,
    or jig keeps them in its own vocabulary (smith's
    `protocol/transcript.md`, section 12);
  - the core's records walked by a smith patch: the core's `Row` reaches
    `TurnRecord`, which holds smith's turn, so each smith patch runs the
    core's identity plan over every core record, though no row in the
    map holds a turn; a frames' root kept out of `Row`'s `Value` would
    spare that walk;
  - smith's machine is priced for the most nodes its kept turn allows
    (8.6): from the baseline on, about 298 MB per serving store
    (`Patcher::worst_case_heap` at 2,109,520 nodes), though a typical
    turn takes a few hundred nodes. Its tables grow in bounded steps
    (skein's `codec.md`, 11.7), so a store holds what its largest turn
    needed, not the price; but it keeps that room until the machine
    goes, so one large turn keeps it large (skein's `codec.md`, 13,
    "Shrinking");
  - the children's rules, strictly, their live sets' limits, and the
    rows the core's census chooses, in the check before the switch
    (10.2, step 5; `../domain/engine.md`, section 6). The rules live in
    each child's restore, which refuses the start on a broken record,
    but for the accounts child, which counts such records and leaves
    them; no child has a check over decoded rows the store could call
    with nothing committed, and a live set's limit is the child's own
    arena's, counted by its own members. The census's results after each
    party's read position and its unanswered questions' activity are
    chosen by the core from its restored state (7.2), which the store
    does not hold before the switch. Until then a patched record a child
    refuses, or a result row naming an ended task the store does not
    hold (the store's own join, which stops the start), makes the start
    after the switch refuse, as any restart's does, and the generation patched from stays
    on disk as `active` − 1, spared by the strays' sweep (section 2), for
    the operator to go back to before the active generation's first
    commit of its own. Writing `active` back to A gives up A + 1, the
    patched copy, which the next start sweeps as a stray: a start of the
    build before opens A as it was, and a start of this build patches A
    again and refuses the same way, until the binary or the record
    changes. One the accounts child counts and leaves stays in the
    patched store;
  - schemas for the secrets' records, so that a patch can name them
    (section 9);
  - keys a patch moves (10.3), left to later work: a line for them
    in skein's patch text, and the key function's answer up and back;
  - the free space checked before the copy (10.2);
  - the layouts of skein's and smith's families recorded, so that
    their owners' none can be read as level 0 at a later baseline
    (5.3);
  - a gate holding each patch's `after` to at least the pin it lands
    on (10.3);
  - a copy made by the file system (a reflink, or btrfs's snapshot:
    skein's `io.md`, section 5) in place of one written file by file.
- **The header by its own level's text.** A jig behind decodes the header
  by the binary's core text, which every store holds until jig's first patch
  moves the core; that patch brings the kept text's decode with it.
- **Backups:** copying the deployment's directory whole while the engine
  is stopped works. An online backup would copy `active`, the generation
  it names and `transcripts/`, in that order (section 2), and rest on
  recovery's rule, and on skein-kv keeping each segment the copy has
  yet to read.
- **Finding a turn faster:** a row per turn in the map, if walking an
  attempt's frames proves slow for parties' reads.
- **Values on disk,** with only keys in memory, if a deployment outgrows
  its map's budget after retention (skein's `kv.md`, section 12).
- **Search** over tasks, notes and transcripts: an index of the domain's,
  or a file-based one beside the store.
- **Several engines:** the store is one process's; the log of commits is
  what would be replicated.
