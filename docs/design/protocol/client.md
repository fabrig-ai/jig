# The client's protocol

Provisional, 2026-10-10, revised 2026-10-24. What carries jig's API
(`../client/domain/contract.md`) as messages and bytes, whatever the
transport: jig's messages; the clients' component's messages toward the
application's transport and server, and the client's link toward its
transport; the wire's records and their versions; exchanges and the one
stream a client keeps; how a keyed request's answer becomes the request
table's envelope; reads, watches and a run's text; signing in, by jig's
built-ins or by the application's sign-in and a `Mint`; the small
store's encoding; an application's documents and requests on the same
messages; the shells; and jig's default HTTP mapping, an option of
jig's (section 17). Its two sides are a component of the engine's
protocol layer and a part of `jig-client` (`README.md`, section 5). What
is still open is listed in section 15.

## 1. In one page

- **jig's messages, transport-agnostic.** A keyed request, a read,
  opening and closing a watch, a stream gone, signing in and signing
  out: each an exchange, one record each way, answered once. And one
  stream per client, for every delivery, tagged with its watch. Every
  message names its owner and its version, and comes with its credential
  as opaque bytes, or none (section 4).
- **The clients' component knows no HTTP.** `jig-protocol-engine-clients`
  takes jig's messages from the application's transport, and a `Mint`
  from the application's server, and answers with jig's messages, the
  stream's, and credentials. Paths, cookies, hosts, cross-origin rules
  and server-sent events are the transport's (4.2).
- **Nor does the client's link.** `jig-client-protocol-link`, inside
  `jig-client`, encodes and decodes jig's messages, makes the request
  table's envelopes, judges the stream's hello, and encodes the small
  store's records; `jig-client` hands its transport each message as
  bytes (4.3).
- **The transport is the application's,** with jig's default HTTP
  mapping to use, wrap or replace: its client end,
  `jig-client-protocol-http`, and its engine end,
  `jig-protocol-engine-clients-http`, with example guards; its routes
  under the prefix the application mounts it at, its stream as
  server-sent events (section 17).
- **Records from schemas, per owner, in general form.**
  `jig-client-protocol-wire` holds jig's records for both ends: the
  plumbing operations and the compound request, their answers, reads,
  task queries, subjects, `me`, the task line and the other documents.
  No record names a shape of work. Each application's wire crate holds
  its own (section 3).
- **One attempt per exchange.** The link makes one attempt per request,
  read or opening, and says how it ended. For a keyed request it makes
  the request table's envelope: *final* with the engine's answer decoded
  into its owner's vocabulary, *again*, *lost*, or *signed out*
  (`../client/domain/client.md`, section 6). Retrying is the table's.
- **The stream paces the watches.** The component hands the transport
  one message per stream at a time, the next once the transport has
  taken it; a client that reads slowly holds its stream back, and the
  engine coalesces its watches' changes meanwhile. Nothing is
  acknowledged by hand, and the engine keeps no backlog (section 7).
- **The stream keeps the link.** It opens with a hello (the engine's
  versions, its heartbeat), carries a heartbeat while nothing else is
  sent, and its loss drops every watch on it, which the client reopens.
- **jig owns credentials; sign-in flows are the application's.** jig's
  built-ins are the printed link, on loopback, and services'
  credentials; every other way is the application's own flow on its own
  routes, then a `Mint`. A credential is a sign-in's number beside
  random bytes; the engine keeps its digest, and hands the domain only
  the number (section 8).
- **Versions per owner:** the ranges served in the hello, and a version
  not served answered so, typed, naming the owner and the ranges, in an
  envelope every version reads (section 9).
- **The small store is the client's records,** the request table's and
  its own, per owner, versioned, each kept request's stored before it
  leaves. New top-level `Seen` stays in memory, and a request carrying a
  value is sent once and never stored (section 5). The application keeps
  its own (section 10).
- **Shells are the application's.** jig ships a native shell,
  `jig-client-shell-native`, and its browser parts,
  `jig-client-shell-web` (section 12).
- **On one person's machine** (`README.md`, section 4) the client reaches
  the engine on a loopback address, in plaintext, and the person signs
  in by a link the application's startup prints at start (section 8).

## 2. Where it sits

```
client                                                     engine
the application: its presentation, jig's helpers           the engine's root (core, connectors)
  ▲ jig_client::Client's surface (../client/domain/)         ▲ the core's vocabulary, and connectors'
jig-client: jig-client-domain, jig-client-protocol-link    jig-protocol-engine-clients, the clients' component
  ▲ jig's messages, as bytes                                 ▲ jig's messages, as bytes; Mint
the application's transport ◄── exchanges, one stream ──► the application's transport and server
  jig-client-protocol-http, the default mapping's            jig-protocol-engine-clients-http, the default mapping's
the shell: fetch, skein's HTTP client, a socket            skein's HTTP server, TLS or plaintext on loopback; io
```

- **`jig-protocol-engine-clients`,** the clients' component, is a
  component of the engine's protocol layer: jig's messages decoded and
  answered; each credential checked against its digest, and the domain
  handed the sign-in's number; the clients' streams and their pacing;
  the printed link and services' credentials; `Mint`; the translation
  between the wire's records and the core's vocabulary. An application's
  requests, reads and subjects are decoded by the application's wire,
  through the protocol layer's routing, and reach the engine's root
  typed. The application's server mounts it wherever it chooses; jig
  reserves no path.
- **`jig-client-protocol-link`,** inside `jig-client`, is the client's
  half toward the transport: each attempt's message, the stream's
  messages and their watches, the envelopes, the stream's hello judged
  against the client's versions, the small store's records encoded and
  decoded, turns decoded by `jig-protocol-transcript`, and the
  translation between the wire's records and `jig-client`'s vocabulary.
  It is step code, built for the host and for wasm32
  (`../README.md`, section 10).
- **`jig-client-protocol-wire`** holds jig's records, for both ends
  (section 3).
- **The transport** is the application's: what carries a message's bytes
  and its credential to the engine and back, and the stream's messages
  as they come. HTTP by jig's default mapping (section 17), a local
  socket for a command line, or an in-process channel for one process
  and the worlds.
- **The shell** is the platform's: its HTTP stack or socket, its
  storage, its clock and its entropy, around the application's loop
  (section 12).

## 3. The wire

- **`jig-client-protocol-wire`,** a codec crate generated from schemas
  (skein's `codec.md`), holds jig's records for both ends:
  - the envelope of every message: its owner and its version (section
    9);
  - the operations of `../client/domain/contract.md`, 3.2, one record
    each, by their names: `Create`, `Propose`, `Withdraw`, `Message`,
    `Amend`, `Cancel`, `Release`, `Stop`, `Move`, `Decide`, `Answer`,
    `Take`, `HandBack`, `Seen`, `MakeProject`, `ChangeSettings`,
    `MoveToNewest`, and the project's own; and
    `Compound { project, operations }` (3.4 of it). A change of settings
    carries each value as the configuration's member describes it, field
    for field, a charter's instructions as text and an application's
    kind's value as its bytes (`../domain/engine.md`, 17.1 and 17.5);
  - their answers, with refusals' facts (section 4 of it), *version not
    served* among them;
  - the reads of its section 6 and their pages, the task query among
    them (6.1); the subjects of 7.1 and their windows;
  - the documents of 5.1: `me`, the task line, and the rest;
  - a run's text (7.3); the stream's hello, heartbeat and endings;
  - signing in and out (section 8), and the small store's records
    (section 10).
- **In general form.** No record names a shape of work: a chat is a
  `Create` filled from a template; closing it, a compound request; a
  list of conversations or of goals, a task query whose rows are task
  lines; the charters offered for chats, templates in `me`. A new shape
  is data in an application's configuration, never a change to the
  wire.
- **The records are the contract's,** field for field: the joint world's
  translation (`../client/domain/testing.md`, section 5) is the map each
  end's translation follows, made bytes. Where the joint world finds the
  contract short, the contract changes first, and the wire follows.
- **Each application's wire crate** holds its own: its families' asks and
  their outcomes, its subjects, reads and documents. It is generated the
  same way, and depends on no crate of jig's but `skein-codec`.
- **Translation at each end:** `jig-protocol-engine-clients` between the
  wire and the core's vocabulary; `jig-client-protocol-link` between the
  wire and `jig-client`'s; the application's protocol layer between its
  wire and its connectors' vocabularies, and the application between
  its wire and its own. Each is a module of small total functions,
  exhaustive both ways, as the channel's are (`channel.md`, section 3).
- **Documents are decoded whole,** into the client's types, and checked
  against the client's limits before they reach `jig-client-domain`: a
  document past its bytes, rows, a path's segments, an authority's
  entries or texts is refused at decoding, as too large, never cut
  silently (`../client/domain/client.md`, section 16). An authority
  carries no cut mark (`../client/domain/contract.md`, 5.3). The wire's
  generated worst cases price each document at the client's limits.
- **The schemas' ceilings bound the client's limits.** Each bounded
  field has a fixed ceiling in its schema, and the client's limits stay
  within them. A list the client bounds by its rows (a task's
  dependencies, a role's holdings, a pattern's segments) takes at most
  32 items, so 32 is the most the client's `rows` may be, and with it
  the rows of its pages and windows. More changes the wire's schema;
  until the first release its version stays fixed (section 9; skein's
  `codec.md`, section 5). A client whose rows pass it is unusable: the link
  refuses its limits at construction, and nothing is cut to fit.
- **A document's nested collections cross as pieces.** The client bounds
  a document by its bytes in all, not each list by its count times its
  largest item. So a list whose items hold lists of their own (a page's
  rows; the rows of a table in which a contract type that nests itself
  is flattened, each row with its parent's index) crosses as one piece
  bounded by the client's document limit (skein's `codec.md`, section 3,
  "A sum crosses as one piece"). The client decodes each row as it
  splits the piece, counting it against that limit, so a page of many
  small rows and one of a few large ones cost what they hold. Lists of
  flat items stay lists.
- **Turns cross as their bytes,** smith's, inside the documents that
  carry them, and are decoded by `jig-protocol-transcript` in the
  client's link (`../client/domain/client.md`, section 10); a turn that
  does not decode crosses up as such, never dropped. An activity line's
  turn crosses beside the line's references: its bytes; its size, where
  the turn alone passes the read's byte bound; or unread (`store.md`,
  7.3).
- **No time crosses, only walls.** Documents carry wall times, which the
  store keeps (`../client/domain/contract.md`, 5.3); durations (a
  retention, a heartbeat, a grace, a deadline a request gives as a span,
  `../client/domain/contract.md`, 3.2) cross as durations. A deadline the
  engine keeps on its own clock (an account's next ask, a grace's end)
  crosses as the wall it falls at, which the engine's translation makes
  from its clock and its wall when it makes the document, so a client
  shows it as it shows every other moment.

Authority selectors in client v11 preserve omission, ALL and named sets
(including empty), at most 64 names of 4,096 UTF-8 bytes each. Both public
authority forms carry them. Authority documents carry `cut` independently
from the whole authority payload used for edits (`../client/domain/contract.md`,
3.2); decoding retains the flag, and `whole()` refuses a cut projection.

Client v11 appends the public provider catalogue and startup key bound to `me`,
and provider/payer readiness to `accounts`. Deployment includes the same
catalogue and viewer readiness for the reference fragment. Each deployment host's
`OwnerHostRule` is `open`, `person` with an `OwnerHostPerson` party number,
`several`, or `unknown`, recording its current kept-login placement rule.
The `position`
hello refusal names a worker position that is unconfigured or held by a live
channel. Existing tags stay fixed. These are bounded secret-free
configuration/derived facts
(`../client/domain/contract.md`, 5.1 and 7.1); people v5 and accounts v1 do not
change.

Client v11 carries added dependencies (`../domain/tasks.md`, 5.7, 6.7 and
section 7): an `Amend` names a task's dependencies as one whole list; the
refusals *circle*, naming its tasks in order, cut at its limit, which a
batch's cycle becomes, *dependency* and *kept*; a task document's
dependencies, each with its phase, its wall and maker when it was added,
and whether they keep the task; a kept task's pending dependencies in what
it waits on; and a hold for a circle of waits, naming it
(`../client/domain/contract.md`, sections 3 to 5). The owner's activity
also carries the parked run's kept line, its attempt and pending
dependencies, and the owner's message kinds the met notice, the dependency
and whether it was removed (`../domain/tasks.md`, 5.7 and 8.1;
`../domain/engine.md`, section 7). Existing tags stay fixed.

Client v11 also carries `TaskRunWait.refused`, as either
`TaskWaitRefused.hello` (`TaskWaitHello`: the existing `OwnerHostKind`, wall
receipt time and `OwnerHelloRefusal`) or `TaskWaitRefused.program`
(`TaskWaitProgram.model` or `.source`, with account and source numbers).
The Program arm has no producer until the Open compatibility slice.
The server and client reuse the deployment's hello-refusal translations;
shortfall dimensions that already omit charter numbers keep that wire shape.
The existing `TaskWaitSource { state }` stays unchanged.
`TaskWaitProgramSource { account, source }` names the Program source pair.
`TaskWaitRefusedProgram { value: TaskWaitProgram }` and
`TaskRunWaitRefused { value: TaskWaitRefused }` wrap enum payloads for the
generator; these unversioned records add no tag or header bytes. Shared admission
types precede their task-wait consumers in the schema, as the generator
requires. Existing variant tags, client version 11, worker version 4 and
all store families stay fixed. Nothing in this data slice changes status
or person-facing words (`../client/domain/contract.md`, 5.1).

Open uses client v12 with worker wire v6, tasks store v7 and people store
v7; core store remains v4. Its ending authority and refusal status travel
in these families; the worker's GrantValue retains tags 0 to 2.

Client v12 carries `resolved_default: Option<LlmChoice>` beside each
effective template's configured LLM intent in `me`
(`../client/domain/contract.md`, 5.1). The schema, generated codecs,
translations, golden records and served-version declarations move
with that field (sections 9 and 14). It is a preview of Create's
once-at-creation resolver, never task intent or a reservation. This
changes no core or task store version or layout, requires no migration,
and leaves application-family versions separate. Implementation status
is in `../STATUS.md`.

The account's move (`../domain/accounts.md`, section 6) adds to jig's
wire, in its current schema: the account changes `move_out { account
}`, `written { number }` and `move_in { digest }`, the last carrying the
moved file as the keyed request's value (section 5), whose bound grows
to 128 KiB; the answer `Made::moving_out { account, number, record }`,
the record a secret of at most 128 KiB filled for the asking exchange
alone (section 5), and the outcome `account_moved_in { account,
renewed }` (*written* is answered `account_changed`); the refusal
`account_move { why }`, `why` one of `not_signed_in`, `not_ours`,
`held`, `cancelled` and `moving`; and `moving` in each source state an
account document or a task's wait shows. Existing tags stay fixed; new
variants go last.

## 4. The connection

A client keeps one connection to the engine, whatever carries it, of
two parts: exchanges, each one message and one answer; and one stream,
the engine's messages to the client as they come.

- **Exchanges.** Every one of jig's messages but the stream's crosses as
  an exchange: one record each way, answered once. Each carries its
  credential (section 8) and names its owner and its version (section
  9).
- **The stream** is one per client, for as long as the client keeps it.
  Its first message is the hello: the stream's name, the versions the
  engine serves of jig's wire and of each application's, and the
  heartbeat. Every watch the client opens names its stream.
- **A client is one run of `jig-client`,** from its start to its end: a
  native process; a browser page from its load until it is left. A link
  followed, or back and forward, within the page's history keeps it
  (`browser.md`, section 9); a reload, an address typed or opened, or
  back and forward to an entry another load made, starts a new client.
  Each client holds a stream of its own, so two clients of one sign-in,
  two tabs or a page and the page that follows it, hold two. The engine
  never refuses or ends a stream for another stream of its sign-in:
  streams are bounded in all, by the component and by the transport
  (section 13), and an opening past either bound is refused, which the
  client takes as a dropped stream.
- **The predecessor's stream is sent gone.** A page that was left, or a
  process that died, may hold its stream, and its watches their places
  in the party's room, until the engine sees the stream's connection
  end: seconds later for a page whose browser drains it, and later
  still for a connection cut half-open. So the client keeps its
  stream's name in the small store, saved at each hello (section 10).
  The first time it would open a stream, a client whose kept records
  name one sends that stream gone (4.1), and opens its own once that
  exchange has its terminal, a reply or `Lost`. The engine ends a stream
  named gone only when it is the sign-in's and still open, so a name
  kept by another sign-in, or of a stream already ended, does nothing.
  A store copied (a browser tab duplicated) ends the original's stream
  once, which the original takes as any drop.
- **Heartbeats.** The engine sends a heartbeat on the stream whenever it
  has sent nothing for the heartbeat's span. The link tells each of the
  stream's watches it is alive at each heartbeat and each message; past
  twice the span with nothing read, it takes the stream as dropped.
- **A dropped stream** drops every watch on it, each told so, and the
  engine ends them as the client left; until then, each holds its place
  in the party's room (`../client/domain/contract.md`, 7.2). The client
  reopens them, and the stream with them, after its backoffs
  (`../client/domain/client.md`, 8.2). A watch's opening or closing
  that gets no answer drops the stream too (section 7).

### 4.1 jig's messages

| Message | Carries | Answered with |
|---|---|---|
| a keyed request | its key; the request's owner, version and record: one of jig's operations, a compound request, or an application's request naming its scope | an answer (section 5) |
| a read | its owner, version and record: one of jig's reads, or an application's naming its scope and window | the document, a page, or why there is none (section 6) |
| opening a watch | the stream's name; the client's name for the watch; the subject, with its owner, version and window | admitted, or refused (section 7) |
| closing a watch | the stream's name and the watch's | closed, once the engine has ended it |
| a stream gone | the stream's name | nothing more: the engine ends the stream and its watches, when it is the sign-in's and still open (section 4) |
| signing in | the printed link's secret; or, as a service, nothing more than the service's credential, as the message's credential | a sign-in's credential, refused, or not ready (section 8) |
| signing out | nothing more than the credential of the sign-in it ends | signed out, or not ready |

On the stream, in its owner's version: the hello; a delivery (a watch's
document, whole, in its owner's wire); a run's text; a heartbeat; a
watch ended, with why.

### 4.2 The clients' component's messages

```
in, from the transport:
  Exchange { exchange, credential, message }   one of jig's messages (4.1), an application's record inside it where its owner is the application; the credential as opaque bytes, or none
  StreamOpen { stream, credential }            a client opened its stream
  StreamTaken { stream }                       the transport took the last message streamed
  StreamGone { stream }                        the stream's connection ended
in, from the application's server code (never from a client):
  Mint { exchange, identity }                  an identity the application verified: a provider's number, a subject, a login, a name
out, to the transport:
  Reply { exchange, message }                  one record: an answer, a page, a watch admitted or refused, a sign-in's credential
  Streamed { stream, message }                 the hello, a delivery, a run's text, a heartbeat, a watch ended
  StreamEnded { stream, why }                  signed out, or the engine stopping
  Minted { exchange, credential, expires } | MintRefused { exchange, why }
out, to the application's startup:
  Link { secret }                              the printed link's one-time secret, each time one is drawn (section 8)
```

- **`Minted` carries the sign-in's expiry,** its wall time, beside the
  credential, so the server gives the credential the sign-in's life, as
  the default mapping's cookie's `Max-Age` (17.3).
- **Names.** `exchange` and `stream` are the transport's, which the
  component gives back and never reads. The stream's name that watches
  carry is the component's, drawn when the stream opens and sent in its
  hello.
- **Its side toward the engine's root** is the protocol layer's routing,
  as for every component: the core's events and outputs, an
  application's records decoded and encoded through its wire, and the
  secrets' store's records (`service.md`, section 2).
- **One reply per exchange,** made once the core has answered and, for a
  keyed request or a read, once that answer is durable. A transport that
  has lost its client drops the reply.
- **One streamed message in flight per stream.** The component hands the
  transport the next only after `StreamTaken`, which the transport says
  once it has written the message to its connection, or into a bounded
  buffer of its own (section 7).
- **The credential is checked** on every exchange and every stream
  opened (section 8). A stream opened with none, or with one the engine
  does not know, ends at once, `StreamEnded` signed out.
- **Bounded:** exchanges in flight per sign-in and in all; streams in
  all, never per sign-in (section 4); a message's bytes, what a stream
  may have queued, mints in flight (section 13).

### 4.3 The link

`jig-client-protocol-link` is `jig-client`'s half toward its transport.
`jig-client`'s surface, as the application holds it, is
`../client/domain/client.md`'s; toward the transport it is these
messages:

```
emitted:  Exchange { exchange, credential, message } | OpenStream { credential } | CloseStream | Save { records } | Erase { records }
heard:    Reply { exchange, message } | Lost { exchange } | Streamed { message } | StreamGone | Stored | NotStored
```

- **Messages as bytes.** The link encodes each of jig's messages, and
  decodes each reply and each streamed message, with
  `jig-client-protocol-wire`; an application's records travel inside
  them as bytes under their owner's number, which the application's wire
  encodes and decodes. There is no method, path or header here: those
  are the transport's.
- **One attempt per exchange,** whose terminal is a `Reply`, or `Lost`:
  the exchange failed, or ended without a whole reply within the
  exchange's deadline, which the transport keeps
  (`../client/domain/client.md`, section 16). Retrying is the client's.
- **The credential** on `Exchange` and `OpenStream` is the one the
  client holds, or none when the transport carries one the client never
  sees (section 8).
- **The stream:** its hello judged against the client's versions
  (section 9); each delivery and run's text routed to its watch by name;
  heartbeats keeping the watches alive; `StreamGone`, or silence past
  twice the heartbeat, dropping it; the stream refused signed out, a
  `Streamed` message the transport makes, ending the sign-in.
- **The predecessor's stream:** the stream's name saved at each hello;
  the name kept at start sent gone before the first `OpenStream`, which
  waits for that exchange's terminal (section 4).
- **The small store's records** encoded and decoded (section 10).

## 5. Keyed requests

An owner proposal outcome carries its proposal number and the original admitted role (`OwnerOutcomeProposed.to`), in the project of the request. Replays retain that original role; the proposal document gives its current recipient. Active client-v19 records change together before first release, without a family/version bump.


- **One exchange per attempt.** The request table's send (its attempt,
  its key, its request) becomes one keyed request: the key, the
  request's owner, version and record, with the credential. Its reply,
  or `Lost`, is the attempt's one terminal.
- **A compound request** is one keyed request: one record, `Compound`,
  holding its operations in order, answered by one answer
  (`../client/domain/contract.md`, 3.4).
- **The engine's side** checks the credential (section 8); hands the core
  the sign-in's number, the key and the request, decoded into the core's
  vocabulary; and, for an application's request, the digest of the
  record's bytes as they came (SHA-256), which tells a key sent again
  with another request apart (`../client/domain/contract.md`, 3.3). The
  record is canonical (skein's `codec.md`, section 1), so the same
  request always has the same digest. A request carrying a value
  (below) has a digest of its own.
- **A request carrying a value,** an LLM account's key added or
  replaced, a sign-in's pasted address, or a moved account's file moved in
  (`../client/domain/contract.md`, 3.2), or an application's
  request that carries one, is sent once and never kept: the client
  never stores it in the small store, and never sends it again by
  itself (`../client/domain/client.md`, section 6). Its key is its
  caller's name, never made from the value
  (`../client/domain/contract.md`, 3.1).
  - *The engine's side* takes the value out of the record as it decodes
    it, keeps it in the secrets' store before the core sees the request
    (`store.md`, section 9), and hands the core the request with the
    value named by a number jig draws for it, never one the request
    names, and the value's digest. An LLM key's value is then adopted
    into its source's secret row in one commit, which erases the
    carried row (`store.md`, section 9). A sign-in's pasted address is
    held, not kept: in the secrets' store's memory alone, never written,
    and erased once its request is answered, the core leaving every
    finish unused (`store.md`, section 9; `llm.md`, 5.4). The request's own digest covers
    its record, the value left out, and its value, each after its
    length, in place of the record's bytes as they came, so that a key
    sent again with another value is told apart. Those digests are the
    only trace of the bytes the core keeps, and no value can be read
    from them.
  - *The client's side* sends it once: an answer of busy or not ready, a
    link down, or a party signed out or not yet confirmed ends it as
    *unsent*, the engine having made nothing; an exchange that ends with
    no whole answer ends it as *lost*, the engine possibly having made
    it. Nothing is sent again by the client. Its caller asks again under
    the same key, with the value; the engine answers a key it has
    answered with its first answer, and erases unused a value kept for
    it again.
  - *No value is in a record kept:* not in the small store, the core's
    commits, an answer kept for its key, a log or a progress report.
  - *Its bound* is the wire's: a keyed request's value is at most
    128 KiB, which holds a moved file under each profile's record bound
    (`llm.md`, 5.6); startup refuses a configuration whose moved file's
    bound would pass it. The engine refuses a key past the bytes `me`
    states, and a moved file past its form's bound, as a limit.
- **A request whose answer carries a value,** an account moved out
  (`../client/domain/contract.md`, 3.2), goes the same way: sent once,
  never kept, never sent again by the client, since its value is given
  once and a second answer would not carry it.
  - *The engine's side* fills the value as it encodes the answer for
    the exchange that asked, never before the core's commit: the core's
    outcome names what holds it (the moving source), and the clients'
    component asks the application's root for it, which takes it from
    the accounts' protocol layer (`llm.md`, 5.6), once. The value is in
    no commit, no answer kept for the key, no log and no progress
    report, as a service's minted credential is not.
  - *A second answer under the key,* to the request sent again or to a
    join, is the kept answer, its value empty: the value went with the
    first. The client answers its caller *lost* for it (as for an
    exchange that ended with no whole answer): the engine made the
    request, the value is gone with the exchange that carried it, and
    the account's move is cancelled at its deadline
    (`../domain/accounts.md`, section 6). Its caller asks again under a
    new name once it is.

## 6. Reads

- **One exchange per read,** answered once: the document, a page with its
  cursors, or why there is none (unknown, busy, signed out, version not
  served) (`../client/domain/contract.md`, section 6). A page's cursor
  is the engine's, opaque bytes the client gives back.
- **Credential boundaries, on the client,** visibly end pending reads
  in the domain's existing `Page` forms: jig's `ReadNone::SignedOut` or
  a family's `None`, once each, on sign-out and unconditionally on
  successful sign-in, before the first `me` too
  (`../client/domain/client.md`, sections 7 and 12). The link erases
  old exchange names; the domain retains pending token and optional
  family and fences late completions with its existing token match.
  This adds no link output, public `Readout` variant or wire record,
  and never replays a read. Supersession and `Unwatch` remain abandonment.
- **A task query's page** carries its two groups, live then ended, each
  with its cursor, and says whether the ended group was cut by a filter
  beyond its index's prefix, or has none for the query's keys
  (`../client/domain/contract.md`, 6.1).
- **An application's read** names its scope and its window, and its
  connector's read, in the application's wire, and is answered with the
  application's document, or unknown, or not allowed.
- **A turn read** (`turn{n, attempt, turn}`) is answered with the
  turn's bytes, or its size where the turn alone passes the read's byte
  bound, or gone past its transcript's horizon, or why there is none, as
  above (`../client/domain/contract.md`, section 6; `store.md`, 8.3).
- **Held until durable** on the engine's side, as every read's answer is
  (`../domain/engine.md`, 5.2); the exchange waits for it.

## 7. Watches

- **Opening a watch** is an exchange: the stream's name, the client's name
  for the watch, its subject and window. It is answered admitted, or
  refused (unknown, not allowed, busy, signed out, version not served);
  the first delivery follows on the stream. A subject an application's
  connector judges is admitted at the end of the step that takes it, and
  the connector's refusal, if it comes later, ends the watch with its
  reason (`../client/domain/contract.md`, 7.2). A subject of the party's
  own (`me`, their inbox, an application's subject in the party's own
  scope) names no party, on the wire or in the core's events: the
  protocol layer hands the core the sign-in's number, and the core names
  the subject's party from it, as it does a read's, from the sign-ins its
  people child keeps (`../domain/engine.md`, 5.4). The protocol layer
  never keeps a sign-in's party, which a restart would lose.
- **A delivery** is one streamed message: the watch's name, and the
  document, whole, in its owner's wire. The component hands it to the
  transport when the core releases it (held until durable,
  `../domain/engine.md`, section 11), and tells the core it was taken
  once the transport says `StreamTaken`. Until then nothing more is
  streamed to that client, and the views coalesce what changes
  meanwhile. A client is never told it missed anything.
- **Ending:** a streamed message naming the watch and why
  (`../client/domain/contract.md`, 7.2); or closing it by an exchange,
  answered once the engine has ended it. A stream that ends ends its
  watches.
- **A closing follows its opening's answer.** Exchanges cross
  independently, and a closing that overtook its opening would find no
  watch, be answered closed, and leave the watch admitted after it, which
  nothing closes but its stream's end. So the client sends a watch's
  closing only once its opening is answered admitted, and none once it
  is refused. An opening or a closing that gets no answer leaves the
  client unsure what the engine holds: it drops the stream (section 4),
  whose end ends every watch on it.
- **Each delivery is settled once, while the engine runs.** The core
  makes a watch's next delivery, or ends the watch, only once the
  component has told it of the last one: taken, once the transport took
  its message; or dropped, when its stream ended first. So a watch's
  end, by the client, by the engine or with its stream, never excuses
  telling the core of a delivery the client took. A restart does: the
  core restarted cold holds no watch (`../domain/engine.md`, section
  11), and is told nothing of deliveries made before it; the client
  watches again (section 4), and its first delivery is the document
  whole.
- **A run's text** is a watch of the run: its deliveries carry the turn's
  name, the content's kind, the content after the offset the client has,
  and the offset it ends at (`../client/domain/contract.md`, 7.3). A watch opened, or opened
  again after its stream dropped, starts from the beginning of the turn
  being written, within its bound; the client keeps what it holds and
  takes what goes past it (`../client/domain/client.md`, 8.3).
  These messages leave without waiting for a commit, marked provisional.
  **Three kinds,** `text`, `reasoning` and `input` (a tool call's input
  as it streams, named by the call's provider id and its tool), are in
  the wire as a run's text is, each from its own offset, so that adding
  one needs no new version. The engine sends `text` only; a client ignores
  a kind it has no use for. Only a run asked for its
  text, a conversation's, has any, and only a run whose host gives the
  call it is making has that (`channel.md`, 6.4).

## 8. Signing in

- **jig owns credentials; the flows that ask for one are the
  application's,** but for jig's two built-ins
  (`../client/domain/contract.md`, section 2): the printed link, for one
  person's machine, and a service's credential.
- **A credential** is a sign-in's number beside 32 random bytes, so the
  engine finds its digest by number (`README.md`, section 8; `store.md`,
  section 9). Credentials come from a cryptographic generator of the
  component's, seeded from the kernel at start.
- **A sign-in, made:** the core hears the identity and commits the
  sign-in, which gives it its number; the component keeps the digest of
  a fresh credential under that number, in the store's secrets' records;
  and only then hands the credential out. The core says each sign-in
  that ends or expires, and the component erases its digest then. A
  sign-in kept by the printed link (below) is no sign-in made: nothing
  is committed, drawn or kept.
- **The application's sign-in.** An application whose people sign in
  another way (an OAuth or OpenID server, a company's directory, a chat
  platform's identity) runs that flow on its own routes, redirects
  included, in its own server, verifies the identity, and gives the
  component `Mint { exchange, identity }`: a provider's number, as its
  configuration numbers it (`configuration.md`, section 3, providers),
  the subject there, and a login and a name for display. The component
  makes a sign-in for it, as above, and answers `Minted` with its
  credential and the sign-in's expiry; or `MintRefused`, saying why: a
  provider the configuration does not give as one of the application's
  (of kind application), the printed link's and the deployment's being
  jig's own; a login or a name not UTF-8 or past its bound; not ready,
  while the engine stops; busy, at the people child's limits.
  - *From the server only.* `Mint` is a message of the application's
    server code; no message a client sends becomes one, and a transport
    never makes one from a client's bytes.
  - *How the credential travels* is the application's: a cookie its
    server sets, as the default mapping's (17.3), or a token its client
    holds.
  - *jig keeps nothing of the flow:* what it returned is the
    application's, kept or dropped as its own (`../domain/people.md`,
    section 3).
- **The printed link,** on one person's machine: at start the component
  draws a one-time secret, valid for a short while, and gives it to the
  application's startup, `Link { secret }`, which prints a URL carrying
  it. The URL is the application's; the default mapping's is its link
  route (17.1). Signing in with the secret signs that person in as the
  deployment's first owner it names, and spends it. One link is live at
  a time: a sign-in with a secret spent, expired or unknown is refused,
  and the component draws a fresh one, which the startup prints again,
  unless the live one is reserved (below).
  - *The secret is the link's, or its exchange's.* A sign-in that finds
    the secret live, before its expiry, reserves it for its exchange
    until people answers. While it is reserved, a sign-in with the same
    secret is answered *busy* and changes nothing, and one with another
    secret is refused and draws no fresh link: an attempt that crosses
    the exchange never replaces its secret. A sign-in the component has
    no room for among its exchanges is answered busy, and the secret
    stays the link's.
  - *Not ready, busy and full spend nothing.* The reserved secret is the
    link's again, with the expiry it was drawn with, so the same link
    signs in when sent again, until that expiry, checked again then.
    Signed in, refused and signed out spend it, as do the core's refusal
    before people answers and the engine's close or abort, which end the
    exchange; a spent secret is the link's no more, and the next sign-in
    by any other draws a fresh one.
  - *Full* (`../domain/people.md`, section 3) carries the deployment's
    room for sign-ins, the earliest saved expiry among them, and the
    link's own expiry, both wall times: the person reads when the next
    room comes, and until when this link serves, past which a fresh one
    is drawn when it is tried. The link's expiry is kept on the
    monotonic clock, and full says it at the answer's clocks. Full is the
    printed link's answer alone: a `Mint` and a service's sign-in are
    answered busy for it. On the wire, a sign-in's answer is signed in,
    refused, not ready, busy, or full with those three.
  - *A sign-in held is kept* (`../domain/people.md`, section 3). The
    link's exchange carries a credential as every exchange may, a
    browser's cookie or a native client's bearer (17.3). Once the secret
    is found live and reserved, the component checks that credential as
    any exchange's, its digest under its number and its saved expiry,
    and names its sign-in to the core as held; one that fails the check
    is no refusal, and the link signs in afresh. When people answers
    with the held sign-in, the component draws no credential and keeps
    no digest: its answer is signed in, with that sign-in's party and
    saved expiry, carrying no credential, and sets no cookie, since the
    client already holds it. The secret is spent, as by any sign-in.
  - *Reuse extends nothing:* the kept sign-in's expiry, the cookie's
    `Max-Age` and the bearer's file stay as they were.

  It is enabled by configuration (`configuration.md`, section 3,
  clients), and only where the application's clients' listener is on
  loopback, which its startup checks (`service.md`, section 3).
- **A service's credential** is issued when an owner makes the service:
  the component draws it, keeps its digest in the secrets' store under
  the service's number (`store.md`, section 9), and answers the owner's
  request with it once, never again.
  - *Its shape* is a sign-in's credential's: the service's number beside
    32 random bytes.
  - *To its owner:* the answer carries it beside the service's party; the
    client hands it to the application once, as a secret, and never
    keeps it in its small store; the application shows it once.
  - *Signing in with it:* the service's client signs in as a service,
    the service's credential as the message's credential, and takes
    back a sign-in's credential, with which it makes every exchange. The
    core commits that sign-in as any other, the provider the deployment
    and the subject the service (`../domain/people.md`, section 3); it
    expires as any, and the client signs in again. So exchanges are
    checked one way, by sign-in, and the service's credential only at
    sign-in.
  - *Once:* a request made again by its key is answered as it was, its
    party, and no credential, since the engine keeps only its digest. An
    owner who lost it reissues it (`../domain/people.md`, section 3).
  - *Replaced* when an owner reissues it: a new credential is drawn, its
    digest replaces the old, durable before the answer, which carries it
    once; the sign-ins made with the old end.
  - *Ended* when an owner removes the service: its digest is erased and
    its sign-ins end.
- **Where a sign-in's credential is held:** by the client, as a secret,
  when its transport needs it in hand; the application keeps it across a
  restart, where its platform keeps secrets, and gives it back as held
  (`../client/domain/client.md`, section 12). A transport may keep it
  itself, as a browser's cookie the client never sees: a sign-in's
  answer then carries none, and every exchange leaves with none, the
  transport adding it (17.3). A link's answer that carries none to a
  client that sent the sign-in with a credential in hand names the
  sign-in kept: the client goes on holding that credential.
- **Signed out:** a credential the engine does not know, or a sign-in
  ended or expired, is answered signed out on every exchange and ends
  the stream, and the client reports signed out to the application
  (`../client/domain/client.md`, section 12). Signing out is a message
  that ends the sign-in whose credential it carries, its digest erased;
  a transport that keeps the credential clears it then. Sent with a
  credential whose sign-in has ended, it is answered signed out like
  any exchange, so a client may send it again after it ended lost; a
  lost one clears nothing on the client's side. Its answer, signed out
  or lost, is the sign-in's whose credential it carried: a client that
  has since authenticated a newer sign-in drops it, and keeps the newer
  credential (`../client/domain/client.md`, section 12).
- **Installing a credential** ends every pending read visibly in the
  client domain (section 6), including `Held` or `Transport` before
  the first `me`. A `Link` or `Service` exchange that fails, is refused,
  is lost, is not ready, busy or full and installs no credential leaves old reads
  pending; reads made during it still belong to the old credential.
  A read made under that credential cannot answer under its replacement.
- **While the engine stops** (`../domain/engine.md`, 6.1), a sign-in, a
  sign-out and a `Mint` are answered not ready, as a keyed request is:
  nothing is committed, kept or erased, and a link's secret is not
  spent. Not ready is *again* (section 5): a service's client signs in
  again after its backoff, reaching the next engine; a person's sign-out
  shows as one that did not reach the engine
  (`../client/domain/client.md`, section 12). A refusal would tell the
  person their sign-in failed, and an answer with no record would be
  *lost*, which cannot tell a sign-in not taken from one taken whose
  answer was lost.

## 9. Versions

- **Per owner:** jig's wire and each application's carry their own
  version; every message names its owner and the version of its record,
  and a streamed message's record is in the version its watch was opened
  in.
- **The envelope is fixed.** A message's owner and version, and the
  answer *version not served*, are encoded the same in every version of
  jig's wire, so any engine reads them from any client, and any client
  reads that answer from any engine. *Version not served* is the first
  variant, tag 0, of each answer that carries it (a keyed request's, a
  read's, a watch's opening's, and a watch's end), and its bytes never
  change: a later version adds its variants after it.
- **The engine serves a range** of each owner's versions, which its
  stream's hello states (`../client/domain/contract.md`, section 8). To
  serve a version is to keep its generated codecs and their translations
  beside the current ones. Pre-release, each range is one version
  (`README.md`, section 7).
- **A version not served** is answered so, typed, before the record is
  read: *version not served*, naming the owner and the ranges the engine
  serves. A keyed request, a read and a watch's opening are each
  answered so; nothing is done.
- **Until the first release, versions stay fixed** (skein's `codec.md`,
  section 5). A change to a record, the template preview's among them
  (section 3), changes its schema, codec, translations and one set of
  golden bytes in place. Versions move at a release; family owners keep
  their own versions.
- **The client's versions:** jig's is its wire's; each family owner's,
  the application declares as it makes the client, and the client
  judges each hello against them, whole (`../client/domain/client.md`,
  8.2). A hello names at most its wire's bound of ranges, jig's among
  them: an engine serves no more owners than that, and a client
  declares no more family owners than that bound less jig's.
- **A client outside a range,** from the hello or from such an answer,
  sends nothing more of that owner: its requests stay kept and unsent,
  its reads are answered at once, unsent, and its watches end, as
  version not served. Outside jig's range, the link is out of date, and
  the client sends nothing at all (`../client/domain/client.md`, 8.2).
  The application says how to update; updated, the client sends what it
  kept in a version it speaks, under its keys (section 10).
- **One jig pin:** an application's `jig-client` and its engine come from
  one jig pin, so a client the application's server serves is in range
  unless the engine was updated under a client still running; reloading
  or restarting the client updates it. A client that does not come from
  the engine's application (a phone's, a command line's) may lag it.
- **How jig is released and pinned** is an open question
  (`../README.md`, section 14).

## 10. The small store

- **The client's records** (`../client/domain/client.md`, section 11):
  the request table's, each a whole request, jig's or an application's,
  with the key the engine knows it by, its party, the application's key
  for it, its owner and version, and when it was first sent; and the
  name of the client's stream (section 4), under a key of its own, which
  no request's key can be (a request's is 16 bytes); nothing else of the
  client's, and never a credential, an answer, a request carrying a
  value, or one whose answer carries one (section 5). New top-level `Seen` has no stored record. Each
  kept record is encoded with jig's wire, with its store version.
- **The stream's name** is saved at each hello, in a write of its own,
  and replaces the last; nothing waits for it, and a write of it not
  stored retires nothing. It is not a credential: ending a stream takes
  its sign-in's credential as well (section 4). It is not one of the
  request table's records, and the records read at start are bounded by
  what the table holds and this one.
- **The application's own records are its own.** What its presentation
  keeps (where the person was, their drafts; the reference UI's places
  and drafts among them) it keeps itself, where it likes. The client's
  `Save` carries only the client's records.
- **Kept requests stored before sent.** The transport answers each `Save`
  and `Erase` `Stored` or `NotStored`. A kept request's exchange never
  leaves before its record is stored: the client sends it only once its
  `Save` is answered `Stored`. `NotStored` (a quota full, a disk failing)
  is each request
  whose record that write was the last to carry not sent, parked or not,
  answered *not kept* (`../client/domain/client.md`, section 11). A
  request carrying a value, or whose answer carries one, is never
  stored, and leaves at once, once (section 5). An account's move
  *written* is an ordinary request: kept, and sent again under its key.
- **`Seen` has no save of its own.** The client domain consumes its
  table save before the composed client or link registers a store write
  (`../client/domain/client.md`, section 6). Its exchange still waits
  behind writes already given, and is eligible only for the confirmed
  party on the current link. A failed mixed write reports not kept only
  for the ordinary requests whose records it carried; a held `Seen`
  exchange is lost and retried normally. Consuming the save creates no
  tracked write; store acknowledgements are unchanged. Every terminal
  still emits `Erase`, including an absent new `Seen` key, with the usual
  acknowledgement.
- **Read at start,** the records the application's shell loaded handed
  to `jig-client` as it starts, each request's to the request table, and
  the stream's name to the link (section 4); one of a
  version the client does not read is dropped, and the application told,
  so a request kept across an update that changed its record's version
  is lost, not sent wrong. A family's request whose owner and version
  are not among those the application declares is one
  (`../client/domain/client.md`, 11). Pre-release, nothing is
  translated.
- **Legacy pending `Seen` records** remain readable and restore as
  `Kept`, are never saved again, and erase at their normal terminal.
  They are not eagerly dropped or migrated. These client records are
  separate from the engine's retained completed answers
  (`../client/domain/contract.md`, 3.1). A new unanswered in-memory
  `Seen` is silently lost on closing, reload or crash; the next opening
  checks and sends the watermark again, with no notice from a record of
  its own (`../client/domain/client.md`, section 11).
- **Where** is the shell's: a browser tab's session storage, as text
  skein's small store part encodes (skein's `ui-web.md`, section 10); a
  file a native client writes whole and syncs; a service's file, synced
  before its requests leave; a world's memory.

## 11. An application's documents and requests

- **On the same messages and stream:** a keyed request names its owner,
  jig or the application's family; a read and a subject name theirs; a
  streamed message's record is its watch's owner's.
- **Scopes:** an application's request, read or subject names its
  scope, a project, the deployment, or the party's own, and a read or
  subject its window (`../client/domain/contract.md`, 3.3 and 7.5).
  The party's own names no party: the core names it from the sign-in
  (section 7), and admits it on the credential alone.
- **The engine's side** decodes an application's record with the
  application's wire, through the protocol layer's routing, before the
  engine's root sees it; on the way out it encodes the connector's
  document and outcome the same way.
- **The client's side:** `jig-client` carries an application's records as
  bytes under their owner's number (`../client/domain/root.md`, section
  15), and the application decodes them with its wire.
- **An application's wire,** `ops`'s for one: `../examples.md`,
  section 8.

## 12. Shells

- **A shell is the application's:** its loop around `jig-client` and its
  presentation, its transport, its storage, its clock and its entropy,
  on its platform. `jig-client` runs no loop of its own, and never calls
  the application (`../client/domain/client.md`, section 1).
- **jig's native shell,** `jig-client-shell-native`: skein's HTTP client
  on skein's io, with the default mapping's client end,
  `jig-client-protocol-http`, which cuts the stream's body into
  messages; a file for the small store, and a private file for the
  credential, which it writes when its host gives it one to keep, or
  none, and reads back at start for its host to give as held (section
  8); for Linux and for the worlds that make bytes real. With no
  presentation, it is a service's shell, and a one-shot command's.
  Its public `Transport` owns the socket loop and admission, including
  sockets still closing, for both this shell and a caller's shared io,
  with a caller-chosen owner tag.
- **jig's browser parts,** `jig-client-shell-web`: the client's
  adapter, which carries the default mapping's calls over skein's
  fetch, the printed link's sign-in, and jig's look, over skein's
  `skein-ui-web` (`browser.md`, 5.1), for `ops` and jig's test
  application, whose browser shell is `jig-client-test-shell-web`
  (`browser.md`, 3.1). An application's own browser shell uses them,
  skein's parts alone, or neither.
- **Its pages are served by the application's server,** through skein's
  serving contract (skein's `ui-web.md`, section 4), under its own
  policy: jig serves no page, no bundle, and reserves no path
  (`service.md`, section 2; `browser.md`, section 12).

## 13. Limits and the worst case

- **The client's:** exchanges in flight, the stream's heartbeat
  tolerance, a record's bytes on each side (from the wire's worst cases
  at the client's limits), the stream's message bytes; account receiving
  counts for accounts per list, sources per account and sign-ins per
  list, checked by the link within the held document's decoded room
  (`../client/domain/client.md`, sections 14 and 16).
- **The transport's:** an exchange's deadline, and what it buffers of the
  stream and of the small store's writes.
- **The component's:** streams in all, whatever their sign-ins (section
  4); the watches one stream holds, each until the engine has ended it,
  at least the engine's watches one client opens at its limit, as a
  party's room is (`../client/domain/contract.md`, 7.2), and past them
  an opening is refused busy, within the views' limits on a party's
  watches and on all (`../domain/engine.md`, section 13); exchanges in
  flight per sign-in, which its clients share, since an exchange names
  no stream, and in all; a message's bytes, which bound a change of
  settings (`../domain/engine.md`, 17.8), what a stream may have
  queued before nothing more is handed out, the heartbeat's span, mints
  in flight.
- **The default mapping's,** at each end: its HTTP machines', TLS's, and
  the stream's buffer, at those limits.
- **The worst case** of each side is its components' and its transport's,
  at those limits. The client prices pending-read token and optional
  family metadata, and up to one cancellation `Page` per independently
  bounded read on a credential boundary, in `max_out` and composed
  queues (`../client/domain/client.md`, section 14).

## 14. Testing

Each within jig's suites' budgets (`../../development/workflow.md`,
section 2).

- **Step tests** in `jig-client-protocol-wire` and each application's
  wire: generated, as skein's `codec.md`, section 6 has them; the fixed
  envelope read across versions among them.
- **The client's wire world,** `jig-client-wire-world`: the client
  side's generated codecs (jig's wire, the reference UI's records,
  `ops`'s wire and jig's test family's) checked for drift against their
  schemas, and jig's wire's decoders run over arbitrary and mutated
  bytes in the fuzzy suite (skein's `codec.md`, section 6).
- **The template preview** uses the existing client-protocol carriage
  tests and wire goldens: v12 preserves both configured intent and
  `resolved_default`, including `None`. The existing shapes tests keep
  omission, and the reference view tests consume the supplied preview;
  core charters/templates stories own resolution order and invalidation
  (`../domain/testing.md`, section 4).
- **The component's world,** `jig-engine-clients-world`: the component
  against a scripted transport of messages and a scripted root, with no
  HTTP: answers of every kind and none, deliveries paced by a slow
  `StreamTaken`, a stream gone mid-delivery, two streams of one sign-in
  each with its own name, a stream sent gone by the client that followed
  it, signed out, records that do not decode, a version not served, a
  `Mint` answered and refused, the printed link spent and drawn again, a
  link's sign-in carrying a live credential kept (no credential drawn,
  no digest kept), and one carrying a credential bad, expired or ended
  while its check is pending signed in afresh and refused nothing; the
  link's sign-in answered not ready, busy and full, its secret the
  link's again with its expiry as drawn, and the same link signing in
  once there is room, or refused past that expiry; the same secret sent
  again while reserved answered busy, another refused with no fresh link
  drawn; the reservation dropped by the core's refusal, a close and an
  abort; the exchanges' admission full answered busy; full carrying the
  link's expiry as drawn; a `Mint` and a service's sign-in answered busy
  for full; a
  service signing in, a key added, sent again under its key with the
  same value and with another; an account moved out, its record in the
  first answer alone and empty in the kept answer sent again, and a
  moved file moved in under its key. Its referee: one reply per exchange; one
  streamed message in flight per stream; no credential reaches a
  domain; no key and no moved record is in the small store, a
  committed request or an answer kept.
- **The default mapping's ends,** each against a scripted peer of bytes:
  every route, the stream's events cut at random, the credential as a
  cookie and as a bearer, the printed link's route (a browser's link
  sign-in with its live cookie answered with no `Set-Cookie`, and one
  answered busy or full with none), and the
  example guards' stories (`browser.md`, section 15).
- **The protocol world,** `jig-client-protocol-world`: the client's
  world's stories (`../client/domain/testing.md`, section 4) run through
  bytes: `jig-client` over the native shell's machines and the default
  mapping's client end, against the engine's root and core over
  `jig-protocol-engine-clients` and the mapping's engine end on skein's
  HTTP server, joined by in-memory streams cut at random; and the same
  stories over an in-process transport with no HTTP. One story puts
  skein's text shell's terminal in front of the native client, on
  skein's simulated machine: lines in on a pipe, pages out on another
  (`../client/domain/view.md`, 15.6). Its referee is the joint world's,
  and adds: every record crossed decodes to what was encoded; no secret,
  nor any value a request carried, reaches a domain; a carried value is
  sent once, never resent nor in the small store (section 5); a slow
  stream never holds the engine up.
- **The browser tier:** the reference UI's browser shell in Chromium,
  against jig's test application, `jig-test` (`browser.md`, section
  15).

## 15. Open questions

- **Several tabs:** each tab a client with its own stream, which a hidden
  tab lets go (`browser.md`, section 10); whether tabs of one person
  share one stream through the platform's means.
- **A page that is left sending its stream gone as it goes** (a
  `keepalive` request at `pagehide`), so that a tab whose next page
  never opens a stream frees the room before the browser ends the
  connection. The next client's gone (section 4) frees it today.
- **A bound on one sign-in's streams,** for an engine serving many
  people between hosts. On one person's machine, the browser's
  connections to one origin and the engine's streams in all bound them
  (section 4).
- **Bundling parts** in a jig read's answer (`../client/domain/contract.md`,
  section 11).
- **A binary stream** in the default mapping: a streamed response body
  of the wire's bytes in place of server-sent events and base64, a third
  smaller, when browsers' streaming fetch serves every platform the
  client targets.
- **HTTP/2:** one connection for the stream and the exchanges, when
  skein's HTTP has it.
- **JSON records:** the same schemas written as JSON, for clients not
  built on skein, when the generator writes them (skein's `codec.md`,
  section 7).
- **A sign-in helper for OAuth or OpenID:** a generic flow, configured
  by its issuer and its client, which an application's server would run
  on its own routes before its `Mint`, so an application whose people
  sign in through a standard server writes no flow of its own (section
  8). It is sugar over `Mint`, which jig adopts once a second
  application needs it (`../client/domain/client.md`, section 19).

## 16. What it owes and is owed

- **`jig-client`** is owed by its design (`../client/domain/client.md`):
  the link's states, out of date among them (sections 8 and 9 here); a
  kept request's exchange sent only once its record is stored, with
  `Seen` memory-only (section 10); a credential held, or carried by the transport (section 8); reads and
  watches answered signed out, a reopened run's watch taken from its
  turn's beginning, and a run's document with no text and no call, for
  a run whose host gives neither (`../client/domain/contract.md`, 7.3);
  a request carrying a value, sent once, never stored, its value taken
  out and kept before the core sees it (section 5).
- **The application owes:** its transport, or jig's default mapping; its
  sign-in flows, with `Mint` sent from its server alone (section 8); the
  printed link's URL, printed from the secret the component gives; its
  small store's storage, answering each write; and its server's guards
  (`browser.md`, 12.2).
- **The engine's domain** is owed nothing beyond
  `../client/domain/contract.md`, section 9: sign-ins numbered by their
  commit, made for an identity the application verified, and a party
  ending theirs (`../domain/people.md`, section 3); watches and reads as
  `../domain/engine.md`, section 11 has them.
- **skein owes,** for one person's machine: skein-lib, skein-codec and
  skein-http, with its server-sent events reader and writer, built for
  `wasm32-unknown-unknown` (skein's `lib.md`, section 2; skein's
  `testing.md`, section 6); for the reference UI, its UI kit's browser
  parts, its bundler, and its face and harness for the browser tier
  (skein's `ui-web.md`, 5.1, 4.2 and 14.2), over its browser kit
  (skein's `browser.md`, sections 5 and 6); a client compiled to
  WebAssembly in its model (`programming-model.md`, 2.1). Its HTTP
  server and client serve as they are. Between hosts, TLS's server side
  (`README.md`, section 10).

## 17. The default HTTP mapping

jig's option for the transport: the component's messages and the
link's, onto HTTP, both ends. An application uses it, wraps it, or
writes its own transport; neither end decides anything of jig's model,
and the component below knows none of it.

### 17.1 Routes

Under the prefix the application mounts it at, the same paths at every
prefix; jig's test application mounts it at `/jig`.

| Route | Message |
|---|---|
| `GET <prefix>/v1/stream` | opening the stream, answered with server-sent events (17.2) |
| `POST <prefix>/v1/request` | a keyed request |
| `POST <prefix>/v1/read` | a read |
| `POST <prefix>/v1/watch` | opening a watch |
| `POST <prefix>/v1/unwatch` | closing a watch |
| `POST <prefix>/v1/stream-gone` | a stream gone |
| `POST <prefix>/v1/sign-in/link` | signing in with the printed link's secret |
| `POST <prefix>/v1/sign-in/deployment` | signing in as a service, its credential as the bearer |
| `POST <prefix>/v1/sign-out` | signing out |
| `POST <prefix>/v1/authorize` | an OpenID authorization (section 18) |

- **A POST's body** is its message's record, `application/octet-stream`.
  Its answer is `200` with the reply's record and `Cache-Control:
  no-store`, whatever jig's answer says: a refusal, signed out and a
  version not served are records. A POST whose `Content-Type` is not
  `application/octet-stream` is `415`, a body that is not a record of
  its route `400`, and one past the route's bound `413`, each with
  nothing done; any other method or path under the prefix is `404`, but
  a `GET` of the link's route (below). The guards' answers are the
  example guards' (`browser.md`, 12.2).
- **The printed link's URL** is `<prefix>/v1/sign-in/link?secret=…` on
  the loopback address the application's server listens on, which the
  application's startup prints. A `GET` of it, a browser's opening the
  link, spends nothing and draws nothing, since any other site can make
  a person's browser send a `GET`: the engine end leaves it to the
  application's page, which the server answers there, with
  `Referrer-Policy: no-referrer`; the shell takes the secret out of the
  address and `POST`s it, with the browser header, which signs in
  (`browser.md`, section 11). A native client sends the secret by the
  same `POST`.
- **No provider route.** The application's sign-in runs on its own
  routes; after a `Minted`, its server sets the credential's cookie on
  its own answer with the mapping's function for it (17.3). So do the
  OpenID provider's (section 18): its discovery document, key set, token
  and userinfo routes are the application's, and only `Authorize`, a
  client's message, is the mapping's.

### 17.2 The stream

- **Server-sent events,** `text/event-stream`, `Cache-Control: no-store`:
  the hello first; each streamed message one event, whose data is its
  record's bytes in base64; a heartbeat a comment line. No event names
  and no ids: a record says what it is, and a client reopens by its own
  backoffs, never by the event stream's retry.
- **Paced by its connection.** The engine end says `StreamTaken` once an
  event's bytes are in the connection's send buffer, bounded by the
  mapping's limits; a client that reads slowly fills it, and nothing
  more is handed out until it drains.
- **The client end** reads the body as it comes, a piece at a time;
  skein's server-sent events reader cuts it into events, each decoded
  from base64 into a `Streamed` message. A `401` to its `GET` is the
  stream refused signed out, the engine end's `StreamEnded` signed out
  before the stream opened (4.2); any other status but `200`, the
  body's end, or a failure, is `StreamGone`.

### 17.3 The credential

- **As a cookie,** for a browser: set by the engine end on a sign-in's
  answer, which then carries no credential in its body, or by the
  application's server after a `Minted`, through the engine end's
  function that sets the same cookie: `jig-sign-in=<the credential in
  base64url>; Path=<the prefix>; HttpOnly; SameSite=Strict;
  Max-Age=<the sign-in's remaining life>`, and `Secure` on an exchange
  that came by the public name (below), never on one that came by the
  listener's own address (why each: `browser.md`, section 19). Its life is
  the sign-in's, from the sign-in's expiry, which a `Minted` carries
  (4.2); signing out clears it, with `Max-Age=0`. The engine end reads
  it as each exchange's credential, the link's sign-in's among them: a
  sign-in kept by the link answers with no credential, so it sets no
  cookie, and the browser's stays as it was (section 8); nor does a
  sign-in refused, not ready, busy or full.
- **The public name,** for an application behind a TLS proxy: its
  HTTPS origin's host, with its port where it is not 443, such as
  `app.example.com`, as a browser sends it in `Host`. The
  application gives it the mapping when it mounts it, beside the
  prefix, or gives none; it is no member of jig's configuration, as
  the listener is not (`configuration.md`, section 3). The `Host`
  allow-list holds it beside the listener's own addresses
  (`browser.md`, 12.2), and each exchange is decided by its `Host`
  alone, byte for byte as the request sent it, with no case folded and
  no port normalised: one that came by the public name came through the
  proxy, over TLS, and its cookie is `Secure`; one that came by the
  listener's address came in plaintext, and its cookie is not. An
  exchange that names the public name to the loopback listener
  directly, with no proxy, gets `Secure` too, which is harmless: the
  browser keeps no such cookie over plaintext, or never sends it back
  there. Every answer that
  sets or clears the cookie takes that decision: the engine end's at a
  sign-in and a sign-out, and the application's after a `Minted`,
  whose function takes the decision the engine end reads from the
  application's own exchange.
- **As `Authorization: Bearer`,** everywhere else: the credential in
  base64url, and a service's credential on its sign-in route the same
  way.
- **The browser header,** `Jig-Client: browser`, on every request a
  browser makes under the prefix, the stream's `GET` among them, added
  by the browser's transport: in jig's browser parts, the client's
  adapter (`browser.md`, 5.1 and section 11). The engine end checks it
  (`browser.md`, 12.2): an exchange carrying it is a browser's, its
  sign-in answered with the cookie; any other, with the credential in
  its answer's body.

### 17.4 Its two ends

- **`jig-client-protocol-http`,** the client end, step code for the host
  and for wasm32: an `Exchange` as a method, a path under the prefix,
  headers (its content type, and the bearer where the client holds a
  credential) and a body, with no browser header, which a browser's
  transport adds (17.3); a response's status and body back as a
  `Reply`, or `Lost`; `OpenStream` as the stream's `GET`, its body cut
  into messages (17.2). The native shell and the reference's browser
  client host use it (`browser.md`, 3.2); an application's own shell
  may.
- **`jig-protocol-engine-clients-http`,** the engine end, a component on
  skein's HTTP server: each call under its prefix routed to one of the
  component's messages, but a `GET` of the link's route, left to the
  application's page with the referrer policy (17.1); its credential
  read from the cookie or the bearer; each `Reply` written as `200` with
  its record; the stream written as events, `StreamTaken` said as their
  bytes are written. The application's server mounts it at a prefix of
  its listener and hands it the calls under that prefix.
- **The engine end's room** is its calls in flight, streams among them
  (section 13), and it owns that bound alone. It holds a call from its
  head until it has handled the call's end, even once the server has
  let the call's connection go: until then the call takes room, and
  nothing tells the server it has let the call go. The application asks
  for its room before it hands the engine end a call, and answers a call
  under the prefix that finds none itself, as it answers a call no
  route claims: `503`, with no body, closing the connection, with
  nothing of the engine end's set aside. That is skein's server's busy
  status (skein's `http.md`, section 5: "at its entrance it refuses by
  not asking, or by answering a busy status"). The client takes it as a
  lost exchange or a dropped stream, and sends it again after its
  backoffs. The application's server bounds its own connections, never
  by the engine end's room, and a call handed with no room is the
  application's bug, asserted.
- **A peer's end is the call's end.** skein's server lets a response go
  on after its peer has ended, once the request is all read ("the
  response still goes, room may still come": skein's `http.md`, 5.5),
  and says nothing of that end above. No client of the mapping ends its
  side to read on: jig's client end and a browser end a connection only
  to leave it, and a process's kernel ends it when the process exits.
  So the application's server, which reads each connection's events
  below skein's server, ends a connection whose peer ends while a call
  it handed the engine end is unfinished, and tells the engine end the
  call's connection closed, as it does when a connection fails. The
  engine end takes that as the call's end, and a stream's as its
  connection's end, `StreamGone` (4.2): section 4's "until the engine
  sees the stream's connection end" is then the moment the peer's end
  is read, not a heartbeat's failed write later.
- **Example guards,** in the engine end, which an application uses,
  wraps, or replaces with its own: a `Host` allow-list; the browser's
  required header and the POSTs' content type, which force a preflight,
  with no CORS; the cookie's attributes; a referrer policy on the
  printed link's route. What each stops is `browser.md`'s (section
  19), and so are jig's attack stories an application may run against
  its own server (`browser.md`, section 15). A page's policy
  (its content security policy, its frames) is the application's
  page's.

## 18. The OpenID provider

The clients' component may be an OpenID Connect provider for the
deployment's own web applications: a forge, a wiki, whatever signs its
people in by OpenID. A person signed in to the application is signed in
there by the code flow, with no consent page: the clients are the
deployment's, configured by its owners. skein's issuer
(`skein-oauth-issuer`, skein's `oauth-issuer.md`) checks the requests
and writes the documents and tokens; the component holds what lives,
its codes and access tokens, and decides who is signed in. The routes
and their addresses are the application's (17.1: no provider route).

- **Configured** by `clients.provider` (`configuration.md`, section 3):
  its signing key (a 3072-bit RSA key, PKCS#8, a file of the
  service's own user, mode 0600, made on its host and never copied,
  printed or logged; any other form, size, mode or owner refuses the
  start, each with its reason), its clients (each an id, a secret and
  one redirect URI) and the providers whose subjects are verified
  addresses. Its tests, from the key file's reader up, use a 3072-bit
  key generated in their scratch directory, one per test binary. With
  none, the component is no provider: `Authorize` is refused
  `no_provider`, and a `Redeem` or a `Claims` is answered as an unknown
  client.
- **Built by the application's startup,** which alone knows its routes:
  it gives jig's helper, in `jig-protocol-config::clients` beside the
  key's reader, the configuration's member and its own issuer
  identifier, its public origin, and the URLs of its authorization,
  token, userinfo and key set routes; the helper builds skein's
  `Issuer`, which the startup hands the component in its settings,
  after taking from it the discovery document and the key set, which
  the application serves itself (skein's `oauth-issuer.md`, section 3).
- **`Authorize`,** a client's message: the authorization request's
  query, as the browser's address carried it to the application's
  page, which the client hands on (the application's client decides
  that the address is an authorization). The component answers, in
  this order:
  1. the query checked by the issuer: refused *malformed*, *unknown
     client* or *unknown redirect*, which the page shows and follows
     nowhere; or an error the issuer sends back to the client's
     redirect URI, answered *go* with that address;
  2. the exchange's credential, checked as any exchange's (section 8):
     none, unknown, ended or expired is *signed out*, and the page signs
     the person in, then sends the same `Authorize` again;
  3. the signer, read from people (`../domain/people.md`, section 3): a
     service is refused *not a person*, as is a subject that is not
     UTF-8;
  4. a code drawn, kept with the authorization and the person: *go*,
     with the client's redirect URI carrying it and the client's state.
     The page replaces its address with it.

  *Not ready* while the engine stops, and *busy* when the codes are
  full (below), each changing nothing.
- **The person** the claims name, from the signer's identity:
  `sub` its provider's number in decimal, `:`, then its subject. A
  party is one identity (`../domain/people.md`, section 3), and its
  identity outlives the store: a deployment started afresh gives the same
  person the same `sub`, so a client's account linked to it stays theirs,
  where a party's number would pass to whoever the fresh store numbers
  so. It is stable for as long as the configuration numbers the provider
  so; `preferred_username` its login; `name` its name, where
  not empty; and, where its provider is one of `addresses`, `email` its
  subject, verified. The same for the ID token and the userinfo answer.
- **The code's exchange and the claims** are messages of the
  application's server, as `Mint` is (section 8), from its token and
  userinfo routes: `Redeem` with the request's `Authorization` and
  `Content-Type` headers and its body, answered `Redeemed`; `Claims`
  with its `Authorization` header, answered `Claimed`. Each answer is a
  status, an optional challenge for `WWW-Authenticate`, and a body,
  which the server sends as it is, `application/json` and
  `Cache-Control: no-store` (skein's `oauth-issuer.md`, sections 5 and
  7). A code is spent at its first presentation, whatever follows;
  `Redeemed` with tokens draws an access token, kept with the person.
- **In the component's memory,** never the store: a code lives one
  minute, an access token five; each 32 bytes of the component's
  generator (section 8) in base64url, found by its digest. Their counts
  are limits (`configuration.md`, section 6): with every code's room
  taken, `Authorize` is busy; with every access token's, a code is not
  drawn either, so a code presented always finds a token's room. A
  restart drops them all: a sign-in under way fails, and the person
  signs in again.
- **On the default mapping,** `POST <prefix>/v1/authorize` (17.1).
- **Testing:** the component's world (`tests/engine-clients`) plays the
  application's page and server and a client of the provider's: a
  sign-in through `Authorize`, `Redeem` and `Claims` with the same
  `sub`; a code spent once; a code past its minute; a wrong redirect
  URI, at the authorization and at the exchange; a wrong secret; a
  wrong verifier; signed out, then signed in; a service refused; the
  codes full. skein's issuer world checks the protocol against skein's
  client; the application's process tier, against a real client.
