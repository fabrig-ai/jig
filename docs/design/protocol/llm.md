# LLM calls and accounts in the engine

Provisional, 2026-10-10, revised 2026-10-24. How a host that runs agents
in its own process makes their LLM calls, and how every engine that runs
agents carries out its LLM accounts: the completions
`smith-inline-agent`'s runs ask for, in the engine or on a worker whose
root composes it, carried by smith's LLM component over skein's LLM
connections (`../domain/hosts.md`, 5.2 and section 10); and the accounts'
sources, signed in, refreshed and signed out by skein's OAuth accounts
driver with the secrets' store as its keeper, the credentials' table, and
the grants that reach runs in the engine and on workers. The accounts'
policy (holders, sources, states, uses) is `../domain/accounts.md`'s, and
the LLM providers' terms `../domain/providers.md`'s. An application
without agents has none of this. What is still open is listed in section
10.

## 1. In one page

- **smith's component makes the calls in a host's process.** A process
  whose runs are `smith-inline-agent`'s (smith's `domain/host.md`, 9.1)
  owns one `smith-protocol-llm` component, with skein's LLM connection
  component beneath it, as smith's own hosts do (smith's
  `protocol/llm.md`; skein's `llm-connection.md`): the engine, for its
  own slots, and a worker that composes `smith-inline-agent`. jig writes
  no prompt, and none of smith's own tools' schemas, decoding or
  rendering; its engine tools' are `jig-protocol-tools`' (`channel.md`,
  6.3). Agent processes make their own calls (section 6).
- **The root routes completions,** as `smith-inline-agent` asks them,
  to the protocol layer, which gives smith's component each with its
  slot's result contract, and their terminals back to the agent (section
  3; `../domain/root.md`, section 5). They do not pass through the hub.
- **Accounts: the policy is the core's, the mechanism the protocol
  layer's.** The accounts child decides which sources are usable, which
  account a run takes, and which runs hear of a new credential
  (`../domain/accounts.md`, sections 3 and 4). skein's OAuth accounts
  driver, a component of the engine's protocol layer, signs sources in
  as a party asks, refreshes each signed-in source before it lapses,
  signs it out with revocation, keeps each new record in the secrets'
  store before lending its access token, and lends tokens by generation
  (skein's `oauth.md`, section 6). `jig-protocol-accounts` translates
  between the two and holds the values lent.
- **Four sources, two kinds of grant.** A signed-in source's access
  token and a key are values, lent as credentials with their scheme,
  `bearer` or `key` (skein's `llm.md`, 2.6). A program source is
  lent by its name: its login is the program's own, in a configuration
  directory on each host, which jig never reads (5.5). A *none* source
  lends nothing.
- **Names in the domain, values beside it.** The domain names a grant by
  account, source and generation, with how long it is valid. Values move
  only between components, through the protocol layer's routing: from
  the secrets' store into the driver and the table, from the table into
  smith's LLM component for the engine's own runs, and into the
  channel's records for runs on workers (`channel.md`, section 8). A key
  reaches the secrets' store from a request that is never kept
  (`client.md`, section 5). No root sees a value, and
  `jig-protocol-accounts` links no smith crate.
- **Failures by class.** skein classes what a provider's API answers
  (exhausted, refused, unsupported, unauthorized, with the provider's
  error code and request id: skein's `llm.md`, 2.5), and smith what a
  program reports (smith's `protocol/programs.md`, section 5). A run's
  notices reach the accounts child as limited, refused or unauthorized
  (section 6); unsupported is the request's error.
- **LLM providers are configuration,** one list from which the charter's
  endpoint names, smith's component's endpoints and skein's connections
  are all made, so the three never disagree. Configuration names no
  account: accounts are made at runtime (`configuration.md`, section 3).
- **The budget is smith's to hold** within each run: it reserves each
  completion's most before making it and winds down on its reserve, so a
  run never spends past its budget (smith's `domain/run.md`, 9 and 9.1).
  The core places no run smith would refuse for its budget
  (`../domain/authority.md`, section 7), and sees spend in the turns it
  commits.

## 2. Where it sits

```
engine's root
├── core: accounts                  sign in, sign out, add, remove, want, release, notices; granted, failed, states   (names only)
├── jig-domain-host                 the engine's own hub: its runs' turns, activity and text
├── smith-inline-agent              completions asked and cancelled, and their terminals; facts and content drained
└── …
engine's protocol layer
├── jig-protocol-accounts           the child's vocabulary to the driver's and back; the credentials' table
│     └── skein-oauth-accounts      sign-in, refresh, sign-out, records kept in the secrets' store, tokens lent by generation
├── jig-protocol-store              the secrets' store: records and keys, beside credentials' digests (store.md, section 9)
├── smith-protocol-llm              prompts, tools' schemas, decoding, rendering; its grants' table; text as it comes
│     └── skein-llm-connection      connections, TLS, deadlines, the memory pool
├── the slots' contracts            each slot's result contract, from its start's charter, beside smith's component
└── jig-protocol-engine-workers     grants' values into assignments and grant records; program sources told to workers
```

- **One of each per engine,** shared by every run on its own slots, so
  connections and memory are pooled across runs, and each signed-in
  source refreshes once however many runs use it.
- **On a worker that composes `smith-inline-agent`,** the same, beside
  `smith-protocol-machine`, with no accounts: its grants come from jig's
  channel (`service.md`, section 6). A worker whose agent processes run
  programs keeps their configuration directories, and smith's host half
  checks their logins (5.5).
- **The protocol layer routes,** as any consumer of smith's components
  does (smith's `protocol/README.md`, section 6): io's tokens for the
  components' entities, their earliest deadline, their timers fired where
  its own are, and values from the secrets' store to the driver and the
  table, and from the accounts' table, or the worker's, to the
  components that fill them in.

## 3. Completions for `smith-inline-agent`'s runs

- **A completion asked:** `smith-inline-agent` hands up a run's lower
  request, `Complete`, scoped by the agent's token for the run, as
  smith's domain asks it: the completion's name (the turn it will be
  told as, and its retry), the grant it uses, the prompt's parts with
  its model and most output tokens, its deadlines and bounds (smith's
  `domain/host.md`, 9.1). The root routes it to the protocol layer,
  which hands it to smith's component with its slot's contract.
- **Each slot's contract, beside the component.** smith's component
  needs a completion's result contract (smith's outcome and delivery
  specifications) to write `finish`'s schema and decode its calls
  (smith's `protocol/llm.md`, sections 3 and 4), and smith's request
  carries none. So the protocol layer keeps each slot's, taken from its
  start's charter as its routing decodes it (`service.md`, section 2),
  as smith's local host does, until the run is gone. It is bounded by
  the slots and a charter's largest contract.
- **One terminal per completion:** completed, with its blocks, stop and
  usage; failed, with its class, evidence and detail; or cancelled. The
  protocol layer gives it to the root, which routes it to the agent's
  run. Retrying is smith's session's (smith's `domain/session.md`,
  section 4).
- **A cancel,** `Cancel`, follows a run's cancel or drop: the agent asks
  for it, and the completion still ends with its one terminal.
- **Only these reach the engine.** The engine's runs have no workspace, so
  `smith-inline-agent`'s machine requests (files, searches, commands,
  checks, guides) never come there; on a worker that composes it, they go
  to `smith-protocol-machine` (smith's `domain/host.md`, 9.1).
- **Activity and text, from the agent:** each pass the host's root
  drains `smith-inline-agent`'s facts and content (smith's
  `domain/host.md`, 9.1) and hands the hub a run's activity, a tool begun
  with its call's name and input among it (`../domain/hosts.md`, 6.5).
  The protocol layer asks smith's component for each completion's text
  as it arrives (smith's `protocol/events.md`, 3.5) and hands it up with
  its completion's name and offset: the run's text by turn, retry and
  offset (`../domain/hosts.md`, 6.5), best effort, never holding the
  run.
- **Unauthorized, refused and exhausted** come back to the run as
  smith's failures. The run's notices go up through the agent and the
  hub to the accounts child, as limited, refused or unauthorized, with
  the account and source, fenced by attempt, as a worker's run reports
  them on the channel (section 6). Unsupported, a field the provider
  will not take, is the request's error, never a notice.

## 4. LLM providers, endpoints and models

- **An LLM provider** is configuration: its name, its endpoints and
  their models, the sources it offers, and the facts its terms ask an
  application to show (`configuration.md`, section 3;
  `../domain/providers.md`). jig's code names no provider.
- **An endpoint** is configuration: its name, its dialect, its address and
  transport (TLS with its server name and trusted roots, or plaintext to
  loopback), the dialect's path and headers, and its options. It names
  no account: a run's charter names a grant for each of its LLMs
  (smith's `protocol/agent.md`, section 4).
- **One list, three views.** The configuration's list makes the
  charter's endpoint names (`jig-protocol-charter`), smith's component's
  endpoints and skein's connection endpoints, by index, in the
  application's startup. A run's charter names endpoints the engine
  knows; a worker's agents resolve the same names against their own
  configuration's list, against which the worker's routing decodes
  charters (`configuration.md`, section 4).
- **Models and their prices** are configuration too, and the charter
  carries them (smith's `protocol/charter.md`, section 2;
  `../domain/hosts.md`, section 8): each model's endpoint; its window, the
  most input tokens one prompt may hold, past which smith compacts the
  conversation into a new window; its output, the most output tokens one
  completion may have, thinking included, reserved before each
  completion; the efforts it takes; its prices in the deployment's unit,
  each per a number of tokens: input, cached input, output, and cache
  writes when priced apart, at the input's rate when not; and a
  description of what it is good at, which the brief's catalogue shows
  (`../domain/engine.md`, section 9). Only configuration's models are
  offered; none is taken from a provider's list.
- **What a host declares.** A charter's window and output are at most
  what the agent that runs it declares, which its host's hello states and
  the fleet checks: a hello is refused for a model it declares smaller
  than a charter needs, never for one it does not declare, and a run is
  placed only on a host that declares its task's models
  (`../domain/engine.md`, section 8).
- **The task chooses the model,** with its effort, within its charter's
  models and its authority's (`../domain/tasks.md`, section 3;
  `../domain/authority.md`, section 3); a task that names none takes its
  charter's default.

## 5. Accounts

### 5.1 skein's driver

- **One driver per engine,** `skein-oauth-accounts`, made at startup
  (skein's `oauth.md`, 6.2) with each signed-in source as one of its
  accounts: the source's record, read from the secrets' store before the
  loop (`store.md`, section 9); its LLM provider's registration, with the
  client id the provider issued at its first sign-in; its token
  endpoint's address and transport; and the owner's store as its keeper
  (skein's `oauth.md`, 6.4). A registration is public: the engine holds
  no client secret.
- **Sources come and go while it runs.** A sign-in adds an account to
  the driver, within its limit; a sign-out or a removal takes one out
  (skein's `oauth.md`, 6.2). Every signed-in source is wanted once the
  restart has restored the core's records (`../domain/engine.md`,
  section 6, item 1), and until the close, so the driver refreshes each
  before it lapses whether or not a run holds it, and an idle source
  keeps its refresh token (`../domain/providers.md`, 2.3).
- **What it does, and jig does not:** sign-in, with its loopback
  redirect and the ID token's checks; one exchange per account at a time;
  a held account refreshed its refresh lead before expiry, each new
  generation granted again; every new record kept before its token is
  lent, so a rotated refresh token is never lost to a crash; a rejected
  token refreshed once, a second rejection of the same generation failed;
  sign-out, revoking the record at the provider where it offers
  revocation; failures classed (skein's `oauth.md`, sections 4 and 6.3).
  jig writes no OAuth exchange.
- **The refresh token never leaves the driver** and the secrets' store.
  Only access tokens come out of the driver, each with its generation and
  how long it is valid.
- **A key needs no driver.** It goes from the secrets' store into the
  table (5.3) at start, and from a request as it is added or replaced. A
  *none* source holds nothing.

### 5.2 The translation

`jig-protocol-accounts` is small total functions between the accounts
child's vocabulary and the driver's, both ways, exhaustive on both:

| The accounts child asks | The driver hears |
|---|---|
| sign in a source, under the number the child gives it | sign in, with its redirect: an ephemeral loopback port, or the one configuration fixes |
| want a signed-in source | grant: hold it, and lend its access token |
| a generation unauthorized by its provider | rejected |
| release a source | release |
| sign out a source, or remove it | sign out: revoke its record, take the account out |
| cancel a sign-in | cancel |
| finish a sign-in, with the number of the address its person pasted | redirected, with the address read from the secrets' store (5.4); a value already gone, the finish not waiting |
| withdraw a source for a move | release, then remove: an exchange already sent finishes and keeps its record first (5.6) |
| bring a moved source back, or adopt a moved record | add, with its record and the owner's store as keeper (5.6) |
| refresh a source once before it is lent | nothing more: the record that add hands over has its access token disowned, so the driver's first grant refreshes it (5.6) |
| close | cancel each sign-in under way, then close: a sign-in waiting on a person never holds the close |

| The driver says | The accounts child hears |
|---|---|
| visit: the authorization address | visit: the address the party's sign-in request is answered with |
| keep: a record | nothing: the routing keeps it in the secrets' store and answers `kept` (`store.md`, section 9) |
| taken: a redirected the sign-in took | the finish taken: its answer, *finishing* |
| refused, a redirected: another address, another sign-in's state, full, or not waiting | the finish refused, saying which (full: *too many*), the sign-in still waiting: never a sign-in failed, nor its source signed out; not waiting for any other refusal of a redirected |
| signed in: the subject, the scopes granted | signed in, with the subject and display name: the child makes the account, renews the one of that subject, or refuses the subject another holder holds (`../domain/accounts.md`, 3.3) |
| signed in, without a scope the provider requires | refused, naming the scope |
| granted: the access token, its generation, how long it is valid | granted: the generation and how long it is valid; the token goes into the table (5.3) |
| failed: retry later (with how long), malformed, limit, not kept | failed, later, with how long (the driver's delay, or the child's backoff) |
| failed: sign in again, expired | failed, needs sign-in |
| failed: client rejected | failed, misconfigured |
| failed: unloaded | failed, unloaded, with where and why |
| failed: cancelled; a grant refused as closed | failed, cancelled |
| a grant refused as full | failed, later, after the child's backoff |
| signed out: revocation confirmed, or not | signed out, and whether revocation was confirmed |
| removed, for a withdrawn source | withdrawn, once its record is read and the file's bytes made; or, with no record that reads, withdrawn unread (5.6) |
| closed | closed |

- **Unreachable in the engine:** `hand in` and `expiring`, since no
  access-only record is handed in (5.4); a `redirected` refused for its
  source, since a confidential registration is refused at its start; a grant's refusals for an
  account the driver does not hold. The translation names them, and
  asserts they never come.
- **The visit is not a value to hide:** the address holds the sign-in's
  `state` and its challenge, never its verifier, and is spent when the
  sign-in ends, so the answer that carries it is committed as any
  answer is.

### 5.3 The table

- **A grant's value is a credential:** a signed-in source's access token
  or a key, with its scheme, `bearer` or `key`, which skein's LLM client
  takes and its dialect puts where its provider wants it (skein's
  `llm.md`, 2.6). It carries no account id. The table holds it secret,
  and so does jig's channel (`channel.md`, section 8).
- **Put in its taker's form where it is handed over.** The protocol
  layer that hands a value to an agent capability puts it in the form
  that capability takes: smith's LLM component takes the credential as
  skein's client does; smith's host half takes it in the form smith's
  `protocol/channel.md`, section 6 gives, for its agent processes. On a
  worker that is the worker's protocol layer, which knows the capability
  it composes; the engine does not, so it never sends a value in an
  agent's form.
- **The values lent,** by account and source, held by
  `jig-protocol-accounts`: a signed-in source's two newest generations,
  with their expiry on the engine's clock; a key's newest, with none.
  Replacing a key is a new generation, pushed as a refresh's is.
- **Sized at startup** from the accounts' bounds and a credential's
  bytes (`configuration.md`, section 6); it never grows.
- **An entry goes** when its token expires, when a newer generation
  pushes it out, or when its source is signed out or removed.

### 5.4 An account's first record

- **A signed-in source's first record comes from a sign-in through a
  client:** a party's sign-in request (`../domain/accounts.md`, section
  5) has the driver bind a loopback listener and draw the authorization
  address, and the request is answered with it. The person opens it in
  a browser on the engine's machine; the provider redirects to the
  listener; the driver exchanges the code, checks the ID token and keeps
  the record before it says signed in. The party sees the sign-in end on
  its `accounts` watch (`../client/domain/contract.md`, 7.1).
- **Only a public registration signs in.** A confidential one, whose
  redirect reaches a web server (skein's `oauth.md`, 6.5), is refused
  at its start, naming the LLM provider.
- **A browser on another machine** than the engine's is sent to the
  redirect on its own loopback, where nothing answers. Its person
  copies that page's whole address and finishes the sign-in with it
  through a client (`../domain/accounts.md`, section 5): the request
  carries the address as its value, which the clients' component holds
  in the secrets' store, in memory and never written (`store.md`,
  section 9), and the core hands the accounts child its number. The
  child admits it only from the party who started that sign-in, while
  it waits, one at a time; the routing reads the address and hands it
  to the driver as a `redirected` (skein's `oauth.md`, 6.5). The
  clients' component erases it once the request is answered, whatever
  the answer: the core leaves every finish unused, as it leaves a
  refused key's value. The driver checks it as its listener checks a
  redirect, against the address the listener bound, and takes it or
  refuses it with the sign-in still waiting; the listener stays up
  meanwhile, so whichever comes first is the sign-in's one exchange.
  The sign-in ends on the `accounts` watch as any does.
- **The registration's parts.** The client id the provider issues at
  the first sign-in is kept with the record and used after it; the
  installation's identifier (`store.md`, 4.4) goes where the provider
  takes it (`../domain/providers.md`, 2.2).
- **A key comes from a request** carrying it, which the clients'
  component keeps in the secrets' store before the core sees the request
  (`client.md`, section 5; `store.md`, section 9).
- **A record that cannot be read** fails its source as unloaded, saying
  where and why; the source stays unusable until its holder signs in
  again, which replaces it.
- **A revoked record** fails its source as needs sign-in; the driver
  drops it and the secrets' store erases it, so a new sign-in's record
  takes its place (skein's `oauth.md`, 6.4).
- **No record is handed in.** No account is read from a file another
  program's login fills, and none is the driver's handed-in, access-only
  kind: a live test runs on a key, on a sign-in made as it starts, on a
  record moved in (5.6), or on a program's login on its host
  (`service.md`, section 8). A moved record is this application's own,
  moved by its holder: it enters as a kept record, refreshed once before
  it is lent.

### 5.5 Program sources and *none*

- **The engine holds no value** for a program source; its grant carries
  the source's name in its form. A model whose endpoint names a program
  takes only a program source; one without it skips program sources. Its
  login is the program's own, in a
  configuration directory on each host, named by the source's name under
  the programs directory the worker's configuration
  names (`configuration.md`, section 4); jig never reads it
  (`../domain/accounts.md`, 3.4).
- **Each worker is told its program sources,** by name, after its hello
  and on each change (`channel.md`, 5.2). The worker has smith's host
  half check each login once told its sources, when told of a new
  source, on its holder's check, and at the configured interval
  (`configuration.md`, section 4), and reports what changed (smith's
  `protocol/hosts.md`, section 2; the check's answers are smith's
  `protocol/programs.md`, section 3). The fleet places a run on a program
  source only on a host that reports its login usable.
- **A person's logins keep a host to that person's runs.** A host that
  keeps a person's program logins takes only runs that person's accounts
  pay for, until a run's process tree is contained from them; one
  person's deployment is unaffected (`../domain/accounts.md`, 3.4).
- **Signing out or removing a program source** has each worker's host
  half run the program's own sign-out in the source's configuration
  directory (smith's `protocol/hosts.md`, section 2; the command is
  smith's `protocol/programs.md`, 4.1); the answer names each host that
  did not (`channel.md`, 5.1).
- **The engine's own slots never take a program source:**
  `smith-inline-agent` has no program backend (smith's
  `protocol/programs.md`, section 2).
- **A *none* source** lends no value: its endpoint takes none, such as a
  local model's server on loopback.

### 5.6 A moved record

- **The file's form,** one account a file, opaque, private and never
  printed: a magic, `JIGM`, and the form's version, then, each after its
  length, the application's name (its root gives it to the accounts'
  protocol layer), the issuer, the client id the record refreshes with
  (the one its LLM provider issued, kept in the record, or the
  registration's own where none was issued), the subject, the display
  name, and the record in skein's form (skein's `encode_record`, its own
  version inside). Each part has its bound, the whole at most their sum,
  and nothing follows the record. `jig-protocol-accounts`' `moved`
  module encodes and decodes it with total functions; a decode refuses a
  wrong magic, another version, a part past its bound, a short or
  trailing input and a record skein does not read.
- **Out.** The child releases the source and asks to withdraw it,
  giving its account's display name for the file. The
  routing removes it from the driver; at `removed`, it reads the
  source's record from the secrets' store, and the moves make the
  file's bytes and hold them for the client that asked, telling the
  child it is withdrawn. They go out once: the clients' protocol layer,
  encoding the answer *moving out* for the exchange that asked, after
  its commit, asks the root for the bytes of the source the answer
  names, and the root takes them from the moves (`Moves::take`); a
  second take gives none, and the answer goes with an empty record
  (`client.md`, section 5). They are dropped when the move is
  written (the record then erased from the secrets' store), cancelled,
  or the engine closes, and are never committed, logged or kept
  elsewhere. A source whose record cannot be read is withdrawn unread,
  and the child refuses the move "not signed in".
- **Back.** A cancelled move's source is added to the driver again with
  the record read for it; one held moving at a restart is added as
  every signed-in source is (5.1), except that the child asks it
  **refreshed once**: the copy the driver is handed has its access
  token disowned, its expiry past and no earliest refresh time, so the
  driver refreshes before it lends anything (a grant asked for when the
  token expires within the lead refreshes first, skein's `oauth.md`,
  6.3), or answers that its exchanges are full, and the grant is asked
  again later. Only the driver's copy is so marked, never the stored
  row: the source keeps its moving hold until the first grant after the
  refresh, so a restart before it refreshes again. A refresh refused as
  sign in again leaves the source needing sign-in.
- **In.** The carried value is read from the secrets' store and
  decoded. It is this application's when its name is the one the root
  gave, its issuer a configured LLM provider's registration's, and its
  client id the one that registration refreshes the record with; the
  child then hears the LLM provider, the subject and the display name,
  or "not ours". Accepted, the record, renamed to its new source's
  number, is adopted into the source's row in one secrets' commit that
  erases the carried value (`store.md`, section 9); the source is added
  to the driver with its access token disowned, refreshed once as
  above, and the child hears it adopted.
- **The file left behind is dead** once the receiving deployment's
  first refresh is kept: the issuer has rotated its refresh token
  (`../domain/providers.md`, 2.3), so a copy that refreshes is refused,
  and signs nobody out.

## 6. Grants

- **A grant is a credential or a program source's name** (smith's
  `protocol/channel.md`, section 6): a signed-in source's access token
  or a key, with its scheme and, for a token, how long it is valid; or
  a program source's name, with no value, by which smith's host half
  finds its configuration directory, and the agent checks the login
  again as the run starts (smith's `protocol/programs.md`, section 3).
  A *none* source's run takes no grant.
- **To runs in the engine's process:** when the child lends a
  credential, the protocol layer hands its value from the accounts'
  table to smith's LLM component's table, by account, source and
  generation, with when it lapses (smith's `protocol/llm.md`, section
  7). A completion names its grant; one whose value is missing or lapsed
  fails as unauthorized and unsent, and smith's session asks again once
  a newer grant has come.
- **To runs on workers:** `jig-protocol-engine-workers` fills a
  credential's value in from the accounts' table as it encodes an
  assignment or a grant record, and the worker's protocol layer hands it
  to smith's host half for its agent processes, or to its own LLM
  component's table when it composes `smith-inline-agent`, each in its
  form (5.3; `channel.md`, section 8). A program source's grant crosses
  as its name.
- **Pushed to every live attempt** whose account's source it is, when a
  new generation is lent, and sent again to each attempt a worker keeps
  after its hello, or that the fleet adopts (`../domain/engine.md`,
  section 8; `../domain/accounts.md`, 3.5).
- **Notices,** from either kind of run, reach the child with the account
  and source and, for a credential, its generation, fenced by attempt:
  - **limited:** skein's `exhausted`, or a program's usage limit (smith's
    `protocol/programs.md`, section 5), with when it resets only where
    the provider says;
  - **refused:** the provider refuses the account for what is asked,
    with its evidence; the source stays so until its holder acts;
  - **unauthorized:** for a signed-in source, a generation the driver
    refreshes once (5.1); for a key or a program's login, a source that
    needs its holder (`../domain/accounts.md`, 3.5).

  Unsupported is the request's error, which smith never sends again
  unchanged; misconfigured is the driver's state, never a notice.

## 7. Limits and the worst case

- **smith's component's,** from `smith-inline-agent`'s limits: the
  request's bytes, tools' schemas, a completion's bytes and blocks, a
  decoded call's bytes, a failure's detail, a contract's bytes (smith's
  `protocol/llm.md`, section 8).
- **skein's connection component's:** conversations, the agent's slots;
  its memory pool; connections per endpoint (skein's
  `llm-connection.md`, section 7).
- **The slots' contracts:** one per slot, each at most a charter's
  largest contract.
- **Text as it comes:** a piece's bytes, and the pieces a pass may hand
  up.
- **The driver's:** accounts, the signed-in sources the accounts' bounds
  allow; exchanges at once; listeners, the sign-ins at once; the sign-in
  deadline (skein's `oauth.md`, section 3); the refresh lead (skein's
  `oauth.md`, 6.7). The table's, from the accounts' bounds
  and a credential's bytes (`configuration.md`, section 6). The moves'
  (5.6): a file's bytes and its record for each source withdrawn, at
  most the accounts' sources, and one for each move in under way, at
  most the sign-ins at once, each at most the file's bound. The engine's
  only.
- **A host's worst case,** the engine's or a worker's that composes
  `smith-inline-agent`, includes all of them, beside the agent's slots'
  sum (`../domain/hosts.md`, 5.2).

## 8. Testing

Each within jig's suites' budgets (`../../development/workflow.md`,
section 2).

- **The driver is skein's,** tested in skein's worlds; jig tests what it
  adds.
- **The moved record's form:** its codec's step tests, each refusal of
  a decode, and a fuzzed decode, total and bounded.
- **The translation's step tests:** every request and event each way;
  every failure class mapped; the unreachable ones asserted; the table's
  generations, schemes, expiry and pushing out.
- **The engine's LLM path and accounts,** in the protocol world
  (`service.md`, section 8): the root, `smith-inline-agent`, smith's
  component with the slots' contracts beside it, skein's connection
  component, the driver and the secrets' store, against skein's fake LLM
  peer and fake issuer (skein's `fake-llm.md`; skein's `oauth.md`,
  section 5), over streams cut at random: completions, `finish` decoded
  under each slot's own contract, failures of each class, cancels
  mid-stream, a grant refreshed between two completions, a rotation, a
  rejected token, a sign-in through a client with the redirect the test
  drives, its subject renewed and refused for another holder, a key
  added and replaced from a request, a source limited and resumed, a
  sign-out with and without confirmed revocation, a record that cannot
  be read, a record moved from one owner to another, which refreshes it
  once, and the file left behind refused by the issuer, after a move
  written and after one cancelled, text reaching a watch by turn, retry and offset, and a tool
  begun shown with its call. Its referee: no token reaches a run before
  its record is kept; a name never means two values; no value is in a
  committed request, answer or record outside the secrets' store; every
  completion is given its own run's contract.
- **Program sources,** in the protocol world with a worker and smith's
  fake program: sources told, logins checked and reported, runs placed
  only where usable, a usage limit noticed as limited, and a sign-out
  that names the hosts that did not.
- **A worker that composes `smith-inline-agent`,** in the protocol world:
  grants from jig's channel in its component's table, and its runs'
  completions answered as the engine's are.
- **The domain worlds** keep the scripted LLM below `smith-inline-agent`
  and the accounts child's own world: none of this is in them.

## 9. What it owes and is owed

- **The domain** wants every signed-in source, and pushes each new
  generation to every live attempt (`../domain/accounts.md`, 3.5;
  `../domain/engine.md`, section 12).
- **What smith gives** serves as it is, beside what it is owed (below;
  `README.md`, section 11).
- **What jig asks of smith:** grants with a credential's scheme or a
  program source's name; limited, refused and unauthorized notices; the
  program backend and its host half's login checks and sign-out (smith's
  `protocol/channel.md`, section 6; smith's `protocol/programs.md`).
  Later: an abort and a closed terminal for its LLM component
  (`README.md`, section 11).
- **skein owes** the driver's accounts added and removed while it runs,
  its sign-out with revocation, the issued client id kept with the
  record, the scopes granted and the subject on signing in, and the
  owner's store as keeper (skein's `oauth.md`, section 6); the
  credential's scheme and the failure classes (skein's `llm.md`,
  sections 2 and 2.5).
- **The store owes** the secrets' records and keys (`store.md`, section
  9), and the accounts' family (`store.md`, 4.4).

## 10. Open questions

- **A redirect relayed by a client,** for an engine on another machine
  than the person's browser: the native client binds the loopback
  listener and hands the redirect to the engine, as a pasted one is
  (5.4), with no person copying it. A confidential
  registration's redirect, reaching the application's server on its own
  routes, which hands it to the driver (`service.md`, section 2), is
  open with it.
- **Fairness on a host's loop** (`../domain/hosts.md`, section 12):
  many `smith-inline-agent` runs' completions share one connection
  component and one loop; whether each run gets a share of each
  iteration.
