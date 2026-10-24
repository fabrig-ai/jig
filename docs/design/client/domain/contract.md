# The client and the engine

Provisional, 2026-10-09, revised 2026-10-24. jig's API: what crosses
between a party's client and the engine, at the level of the two
domains. It says who the party is; the plumbing operations a party asks,
alone or several in one compound request, each keyed and answered once
durable; the templates a client fills them from; the answers; the
documents the engine gives, `me` and the task line among them, with the
acts they offer and what a decision will do, as data; the reads, task
queries and watches that bring them; an application's traffic on the
same terms; and versions. It is to clients what `../../domain/hosts.md`,
section 2 is to hosts: the contract both sides keep, whatever carries
it. Its messages and their bytes are `../../protocol/client.md`'s.
Section 9 lists what the engine's domain owes the client, and where
`../../domain/` designs it. What is still open is listed in section 11.

## 1. In one page

- **Plumbing, in general form.** The API is jig's primitives as
  operations: create a batch of tasks, propose, withdraw, message,
  amend, cancel (with a grace), release, stop, move, decide, answer,
  take and hand back, mark seen, and the project's own (3.2). No
  operation, document or subject names a shape of work: a chat and a
  goal are tasks with fields (`../../domain/core.md`, section 9).
- **Keyed requests, answered once durable.** Everything a party does that
  changes something is a request, keyed by its client, checked against
  the party's role, committed, and answered only once that commit is
  durable. Ordinary deciding acts sent twice under a retained key are
  made once and answered alike; `Seen` advances a read position only
  forward and retains no new answer (3.1; `../../domain/people.md`, 5.1).
- **Several operations, one decision.** A compound request holds a short
  list of operations, admitted in order and committed together or not
  at all, under one key with one answer (3.4).
- **Proposing is explicit.** A party's create or amendment beyond their
  authority is refused, naming what it lacked and whether a proposal of
  it could be accepted; `Propose` asks. Nobody is asked by accident
  (3.5).
- **Templates are sugar the client fills in.** The application's
  configuration defines them, a project's policy may override their
  defaults, and `me` serves them merged. The engine never expands or
  checks one: it checks the request a template becomes as it checks any
  other (3.6).
- **Decisions name what they saw.** A request deciding about something
  that changes names the revision of that thing as the client was given
  it: a hold's, a task's spec's, a person task's, a note's (5.2). The
  engine refuses one whose revision has moved, saying what it is now.
- **Refusals carry facts:** what was lacking and by how much, which limit,
  what the object is now, how it ended, whether a proposal could be
  accepted (section 4).
- **Documents per owner.** The engine gives the client typed documents
  about one subject each: jig's carry jig's values, and name an
  application's values as parts, which the application's own documents
  carry (section 5).
- **`me` says who the party is:** their projects and roles, what waits
  for them, the templates each project offers, and the bounds a draft is
  compared with (5.1).
- **Documents say what the party may do, and what a decision will do.**
  Each object's document carries the acts the engine would admit from
  this party now: made, proposable (and to whom), or refused, with
  bounds where the answer depends on what the party writes. What a
  decision will do (what accepting grants and costs, what cancelling
  ends) is data, which the application shows wherever it confirms (5.3,
  5.4). A client never computes authority; the engine checks again when
  the request comes.
- **Lists are task queries.** One record, the task line, for every list
  of tasks; a query by a fixed set of keys and orders, within a window;
  live tasks scanned, ended ones found through the store's indexes
  (6.1).
- **Reads decide nothing, and show only what is durable.** A read is
  answered from the engine's live state or a page of its store, needs no
  key, and its answer waits until every commit it reflects is durable, so
  a party never sees state a crash would lose.
- **Watches are level-triggered.** A watch delivers its subject's current
  document whenever it has changed since the last delivery, one delivery
  in flight. Nothing is queued, so nothing is missed: a slow client sees
  fewer deliveries. A run's live text is the one stream, read from the
  offset the client has (section 7).
- **An application's requests and documents** cross on the same terms and
  the same messages, kept by the people child by key like jig's, each
  naming its owner by its number (root.md, section 15), routed by the
  engine's root to that owner, and each read and subject naming its
  scope (a project, the deployment, or the party's own) and its window
  (7.5).
- **Versions per owner,** a version not served refused with a typed
  answer (section 8).
- **jig's promises stay below the API,** whatever the presentation: a
  decision names the revision it saw (5.2); an ordinary deciding request
  is made once, and every request is answered once durable (3.1); a
  document reflects only committed state (section 6, 7.2); admission and
  authority are the engine's (3.2, 3.5);
  a credential is its party's (section 2). Confirming a deciding act is
  the application's, with the facts jig gives as data (5.4); keeping a
  credential from being used across sites is the application's server's
  (section 10).

## 2. Parties and signing in

- **Who:** a party is known by an identity: a provider's number and a
  subject there, with a login and a name for display
  (`../../domain/people.md`, section 3). The domain sees an
  authenticated identity, and the kind of party it is.
- **jig owns credentials, not sign-in flows.** It mints a credential for
  an identity, verifies a presented one to its party, expires it, and
  revokes it, a party's sign-out included. A credential is its party's:
  a request presented with a valid one is that party's, and everything
  after is checked by their role and authority.
- **Its built-in ways of signing in:** the printed link's one-time
  secret, on a loopback listener, for one person's machine; and a
  service's credential, issued when an owner makes the service
  (`../../protocol/client.md`, section 8).
- **Every other way is the application's sign-in:** its own flow, on its
  own routes, redirects included (an OAuth server, a company's
  directory, a chat platform's identity), which verifies the identity
  and then asks the engine, from the server's side, to mint a credential
  for it: a `Mint`, refused unless the identity's provider is of kind
  application (`../../protocol/configuration.md`, section 3). The
  application decides how the credential travels to its client.
- **jig adds no "acting for."** A program relaying a person's decision
  (a bot on a chat platform) holds that person's own credential, from
  the application's sign-in, and its request is the person's, naming the
  revision it showed them.
- **A sign-in is numbered,** and every request, read and watch carries
  its credential below the domain, which hands the domain the sign-in's
  number; the engine refuses one whose sign-in has ended, and the client
  reports signed out to the application (client.md, section 12).
- **`me`** (5.1) says who the party is, so a client never decides it
  from what it kept.

## 3. Requests

### 3.1 The keyed request

- **The key** is sixteen bytes, scoped to the party: the same key from
  two of their sign-ins is one request (`../../domain/people.md`, 5.1).
  The client draws it, or the application gives it as a name (below).
- **Answered once durable:** the answer waits for the commit of the
  decision it reports, as every output that someone outside could act on
  does (`../../domain/engine.md`, 5.2).
- **Ordinary answers kept for a retention:** an ordinary deciding
  request's durable original answer is saved under its key, success and
  permanent routed refusal alike, and a request sent again within the
  retention gets it again. `me` says how long the retention is, so a
  client never sends a request again past it
  (client.md, section 6). Busy and not ready are never kept: the
  client sends the request again, under its key, but a request carrying
  a value (below).
- **Capacity is engine-wide:** retained answers plus pending ordinary
  requests' reserved answer places, never per party. Retained replay or
  conflict and pending joins or conflicts precede new admission. No
  unexpired ordinary answer is evicted to admit another key.
- **`Seen` keeps no new answer:** a new top-level `Seen` bypasses
  completed-answer capacity, using the existing bounded pending, flight,
  waiter, load, journal and output room without reserving an answer
  place. It saves no `Answer` on success or routed refusal. While pending,
  an identical request joins and a different ask conflicts as usual.
  After completion its key is free: a retry is processed against current
  subject and authority, and a different ask under that key is a new
  request. Legacy retained `Seen` answers replay or conflict until their
  original expiry, still occupying their old places without early
  erasure or reclassification (`../../domain/people.md`, 5.1).
- **One held key, one ask:** a key sent again with a different ask is
  refused as a conflict while pending or retained.
- **A name** is a key the application gives in place of one the client
  draws, for a request that comes from outside the application
  (sixteen bytes of a digest of an agent's key, say). The engine treats
  it as any key: a name sent again is answered as above, within the
  retention for an ordinary deciding request, and one sent with another
  ask under a pending or retained key is refused as a conflict,
  which for a name is no bug of the client's. The application builds a
  named request from its own inputs and `me` alone, never from a clock,
  so that the name sent again makes the same ask: a template's deadline
  goes as a span (3.2). A client holds one request under a name at a
  time: a `Send` under a name its request table holds is answered held
  at once, and nothing is kept or sent (client.md, section 6).
- **A compound request is one keyed request** (3.4): one key, one ask,
  one answer.
- **A request carrying a value,** such as an LLM account's key added or
  replaced (3.2), or an application's request's credential, is never
  kept by the client (client.md, section 6): it carries a name, goes
  once, and its caller asks again under that name. The
  engine keeps the value in its secrets' store before the domain sees
  the request, handing the domain only a reference to the value and
  its digest (`../../protocol/client.md`, section 5).
  Its name is its caller's, never made from the value, so that a name
  carries no secret. It is never in a compound request.
- **A request whose answer carries a value,** an LLM account moved out
  (3.2), goes as one carrying a value does: never kept by the client,
  sent once under a name, never in a compound request. The value is in
  the first answer alone, to the exchange that asked; the answer kept
  for the key holds none, so a second answer under the name carries
  none, and the client answers it *lost* (`../../protocol/client.md`,
  section 5).

### 3.2 jig's requests

The plumbing operations, by their names on the wire
(`../../protocol/client.md`, section 3): what each names, and what it
becomes. Their rules are the domain's (`../../domain/people.md`, 5.1
and 5.2; `../../domain/tasks.md`, sections 4, 7 and 9). A party and an
agent ask the same operations and pass the same check, from different
sources: the party's role and pool, or the task's authority and what it
has left (`../../domain/authority.md`, 8.1 and 8.4).

| Operation | Names | Becomes |
|---|---|---|
| `Create` | a project; a batch: for each task its executor (an agent's charter, a procedure and its parameters, a person, or a role, which is the project's: roles are per project, `../../domain/people.md`, section 1), spec, result contract (a report, a change or a verdict: an approval is a procedure's, `../../domain/people.md`, section 8), authority, whose spend and deadline are the task's budget and deadline (`../../domain/authority.md`, section 3), that budget as named there or `AsAllowed`, that deadline a wall or a span (below), wake policy, or none for its executor's default (`../../domain/tasks.md`, 8.3), as an agent's delegate may leave it, dependencies and inputs, the grants named after it (`../../domain/authority.md`, section 4), tracked and priority, label and flags, and its LLM choice (its main model with its effort, the models its run's sub-agents may use with theirs, and an account, any of them left out for its charter's default: `../../domain/tasks.md`, section 3); and the dependencies among its tasks. The other resources a task will write are those its connectors name from its executor and spec (`../../domain/tasks.md`, section 4) | the batch checked whole and made at once (`../../domain/tasks.md`, section 4), its tasks numbered consecutively in the batch's order, the first and the last answered, and the `AsAllowed` task, if any, with what it was given |
| `Propose` | a project; the action: a batch, or an amendment (a widening of a task's authority among them); a party makes no effect (`../../domain/people.md`, 5.1); or, in the deployment's scope, a change of settings, as `ChangeSettings` names it but for whether running work moves (`../../domain/engine.md`, 17.6); a reason | a proposal to whoever may accept it, its number answered (3.5) |
| `Withdraw` | the party's own pending proposal | withdrawn |
| `Message` | a task; the words | words in its inbox, their message number answered |
| `Amend` | a task; its spec's revision, or a person task's for its address; the change: spec and parameters, instructions, authority, wake policy, its dependencies as a whole list (those it names anew added, those it leaves out removed: `../../domain/tasks.md`, section 7), a person task's address, tracked and priority, its LLM choice, which its next run takes; a reason | amended; an active task given a pending dependency kept until it is done (`../../domain/tasks.md`, 5.7). A change of priority alone names no revision: the first commit decides |
| `Cancel` | a task; a reason; a grace, or none | it and its tree closing; with a grace, the task first hears that it is asked to finish, as it hears an amendment, and is cancelled when the grace ends unless it ended first |
| `Release` | a held task; its hold's revision; for an uncertain effect that cannot be recovered, whether to make it again | its hold lifted, judged afresh |
| `Stop` | a task | its run cancelled, the task held for the party |
| `Move` | a task the party may amend; a reason | the party its requester, its reservation moved to their pool (`../../domain/tasks.md`, section 7) |
| `Decide` | a proposal, by its proposer and number: accept, for a proposal of settings with whether running work moves, reject with a reason, or pass up; or an escalation, by its held task and its hold's revision: release, leave held with a reason, or pass up | as `../../domain/tasks.md`, section 9 has it; or decided first, by whom and how |
| `Answer` | a question, by its task and message, with the words; or a person task, by its revision, with the answer its contract asks (a choice, words, or the state an approval approves) or that it cannot be done | an answer to the question; the person task's result |
| `Take`, `HandBack` | a person task addressed to a role the party holds | held by them, or back to the role |
| `Seen` | how far the party has read: their results, or one conversation's turns | their durable read position advanced to `max(old, through)`; after completion a retry checks current subject and authority and may return `Unknown` after forget or `Role` after authority loss, never recreating history or replaying a new completed answer (3.1; `../../domain/people.md`, section 6) |
| `ChangeSettings` | the deployment, its scope (7.5); the revision the party saw; the items, at most one per setting, each a setting (a charter, a template or a policy by number, the rules, or an application's kind, scope and number) and what becomes of it: set to a value whole, a charter's instructions as text, removed, or reset to its default; whether running work stays or moves (`../../domain/engine.md`, 17.2) | the settings changed in one decision, the new revision's number answered; the deployment's `settings` and every `me` whose templates changed marked. Not among a compound request's operations (3.4) |
| `MoveToNewest` | the deployment, its scope (7.5); a revision held | every live task on it that may move set to the newest revision in one decision, the count of those that could not answered (`../../domain/engine.md`, 17.4) |
| `MakeProject` | the deployment, its scope (7.5); the new project's name (`../../domain/core.md`, 6.1); the number of a policy among the settings, the newest's (`../../domain/engine.md`, 17.1) | a project, its policy a copy of that one, its maker its owner and only party (`../../domain/people.md`, 5.2), its number answered; the maker's `me` gains it. Not among a compound request's operations (3.4) |
| the project's own | notes, by the revision recalled; its name (`../../domain/core.md`, 6.1); the policy, its templates' overrides among them (3.6); roles; a party's pool; services, a service's credential answered once; as `../../domain/people.md`, 5.1 and 5.2 | the note or the project changed |
| adopting a resource | the application's own request (3.3), its form the application's; the engine's root makes the people request on the party's behalf | the project changed |
| the party's LLM accounts | as `../../domain/accounts.md`, section 5 has them: signing in with an LLM provider; finishing the party's sign-in under way with the address its browser was sent to, carrying it as its value (3.1), of at most the bytes `me` states (5.1); signing a source out; adding or replacing a key, carrying its value (3.1), of at most the bytes `me` states (5.1), and removing one; declaring a program's login under a name; checking a program source's login; an account's place among the holder's and its sources' order; its uses; resuming a limited source; removing an account, which signs each of its sources out and erases its keys; moving an account out, saying it *written* by the move's number, and moving one in, carrying the moved file as its value (3.1; `../../domain/accounts.md`, section 6). The party's own accounts, or, for an owner, the deployment's | an account made, renewed or changed. A sign-in is answered done with the address to open in a browser, and ends on the `accounts` watch (7.1), made, renewed or refused; a finish is answered *finishing* once the sign-in has taken the address, and the sign-in ends on the watch; a sign-out's answer says whether the provider confirmed revocation, and names each host that did not sign a program's login out; a program's login declared is answered with the source's name, by which the application's own command signs the program in on each host; a move out is answered *moving out*, naming the account and the move's number, with the moved record to write, once (3.1); *written* is answered done; a move in is answered *moved in*, naming the account and whether it renewed one |

- **A task's fields cross as data** (`../../domain/tasks.md`,
  section 3): *tracked* and *priority*, which the engine acts on; a
  *label*, an opaque 32-bit number the application gives, to which jig
  gives no meaning and no rule of the engine's keys; and flags, each
  with engine semantics: *conversation* (its turns are entries for its
  requester, with their read position and an unread count), *listed*
  (once ended, it is found by its project and label, 6.1), and its
  *retention class*, *kept* or *forgotten once read*. A `Create` names them, and a
  task keeps them; documents carry them, with the two the engine keeps
  on every task: its *last activity* and what it is *waiting on* (5.1).
- **`AsAllowed`** is a budget given as "as much as the creator's role and
  pool allow at admission, capped by the role's spend." The pool's
  amount left is a live join no client can compute without racing, so
  the engine resolves it, and the answer says what was given. At most one
  task of a batch gives it; it is resolved after the batch's named
  amounts are counted, and refused as lacking budget where it would give
  nothing (`../../domain/tasks.md`, section 4). It and a deadline given
  as a span are the plumbing templates need (3.6).
- **A deadline given as a span** is "this long after the task is
  made". The engine makes it a wall from the wall time of the decision
  that makes the task, a `Create`'s admission or the acceptance of the
  proposal that asks for it, as it checks the batch
  (`../../domain/authority.md`, section 7), and the task keeps that
  wall. A request so carries no clock of its sender's, and is the same
  ask each time it is sent, under its key or its name (3.1).
- **Where a party may reach.** A party's `Create` may depend on a live
  task, or take an ended task's result as an input, in their tree, or
  any task of the project for a maintainer; its tasks' requester is the
  party. An agent reaches its
  delegates and the tasks it was introduced to (`../../domain/tasks.md`,
  8.5). An `Amend` that adds a dependency reaches as far as a `Create`
  would, and below the task it amends (`../../domain/tasks.md`,
  section 7).
- **A party's root has someone to escalate to.** A party may create a
  root task only when its escalations have a holder: their role covers
  escalations, or the project's policy names an escalation role. It is
  refused otherwise, saying so (section 4), as a proposal nobody may
  accept is (3.5).
- **What stays asymmetric:** procedures decide nothing
  (`../../domain/tasks.md`, section 9); only parties execute person
  tasks, stop runs, move tasks and run the project.
- **An approval names its state.** A person task's contract may be an
  approval of a state its procedure names, which its connector shows by
  a part (5.3). The answer names the state it approves; declining it is
  "cannot do it", with a reason. It is admitted naming the person task's
  revision, as every answer is, and refused as moved unless the state it
  names equals the contract's, byte for byte: the engine compares the
  state and never reads it. The result reaches the procedure that asked,
  whose connector reads it.
- **A watch is not a keyed request.** It changes nothing and needs no
  answer kept: it is admitted on the party's sign-in and role, and ends
  when its client leaves (section 7).

Authority payloads carry an optional model selector: ALL or an explicit
list of at most 64 unique, nonempty UTF-8 names, each at most 4,096 bytes.
The client may receive fewer bytes per name under its whole text bound;
an identity beyond that bound is refused whole. An empty list is explicit denial. Omission follows the Create, Amend and
role defaults in `../../domain/authority.md`, 3. Both public authority
forms carry this selector. Authority documents also carry `cut`; only a
whole document can supply a replacement. Names are copied literally in
both directions, and current availability is checked after retained-key
lookup (`../../protocol/store.md`, 5.1).

The program operations use the existing account-change request: declaration
carries the provider, optional joined account, and portable source name; check
carries the source number. Declaration returns that number and whole name. Check
answers when dispatched, while inventory facts arrive on the existing watches.
Sign-out and account removal retain the configured names of workers that failed,
were not in contact, or lost contact; replay preserves those original names.
Neither request carries a secret or digest.

### 3.3 An application's requests

A request about an application's own object (silence this alert, extend
this environment) is the application's (`../../domain/people.md`, 5.1):

- **keyed alike,** in the same key space, by the same request table in
  the client (client.md, section 6);
- **admitted by the people child** (`../../domain/people.md`, 5.1) by
  key, as jig's are: duplicates joined, the party's sign-in checked,
  and, by the scope the request names as a read or a subject does
  (7.5), their role in its project, their role in the deployment, or
  nothing more for the party's own; a key sent again with another
  request is told apart by a digest of the request's encoding, which
  the protocol layer gives the domain with it;
- **refused at admission with jig's refusal:** a request the people
  child refuses there (its party's role in the request's scope, a key
  sent again with another request, a limit), or one whose sign-in has
  ended, is answered as jig's are (section 4) and never reaches its
  connector. The client hands that answer to the application under the
  request's key and owner, which shows it as a refusal, never as its
  object gone;
- **routed by the engine's root,** by the owner it names, to the
  connector it concerns, which decides it within the same decision.
  What it asks of the core on the party's behalf (an amendment, a task,
  a release) is judged as the party's own request would be, against
  their role and authority; what it changes of its own is the
  connector's to allow, by its policy's roles;
- **answered once durable,** once admitted, with an outcome of the
  connector's own: only an admitted request is the connector's to
  decide. The people child keeps the key and a token; the connector
  keeps the outcome under that token, durably, for the retention, and
  gives it again when the request is sent again, as the core asks a
  connector for any value it does not own (`../../domain/core.md`,
  section 5; `../../domain/connectors.md`, 5.5);
- **never its own acts:** a connector acts on the core on a party's
  behalf only within that party's request. What it asks of the core
  with no request, where no client is there (a person's conversation
  closed at an end no client saw, an act a restart left undone), is the
  deployment's act (`../../domain/people.md`, 5.3): granted to it by
  configuration, keyed in its own key space, and answered to it, never
  to a client.

### 3.4 A compound request

- **Some decisions are made of several operations,** committed together
  or not at all: closing a conversation whose delegates are live, each
  delegate cancelled or moved to the party and the conversation then
  cancelled with a grace; reordering several tracked tasks at once;
  amending a held task and releasing it; words to a task the party
  stopped, and its release.
- **One keyed request, `Compound { project, operations }`:** a list of
  operations, admitted in order and committed in one decision, all or
  none. It has one key and one answer: done, with each operation's
  outcome in order (below); or refused, naming the first operation that
  failed, by its place in the list, and why.
- **What it may hold:** `Cancel`, with or without a grace; `Move`;
  `Amend`, priority included; `Release`; `Message`; `Stop`. Creating,
  proposing, withdrawing, deciding, answering, taking, handing back,
  marking seen and the project's own are requests of their own.
- **Admitted as the people child admits it** (`../../domain/people.md`,
  5.1): in order, each operation as it would be alone after those
  before it, with one pool counted across them; every operation's task
  in the request's project; at most *operations per request*, refused
  at admission past it as a limit (section 4), enough to close a
  conversation with every delegate live, or reorder every goal of a
  project, in one request (`../../protocol/configuration.md`, section
  6).
- **Revisions, as alone:** each operation names its revision where it
  would alone (5.2), so a `Release` names its hold's beside words or an
  amendment; a priority-only `Amend` names none, the first commit
  deciding; a `Message` names a task and words, and releases nothing.
- **Each operation's outcome: made, with a message number.** Its
  operations take consecutive message numbers, one each, in the
  request's order, and the answer names the first: the operation at
  place *i* took the first plus *i*. A `Message`'s words have that
  number in its task's inbox, as a lone `Message`'s answer names
  theirs. What another operation adds to its task's inbox, if anything,
  has its number too; one that adds nothing leaves its number unused.
  So a client finds each operation's number by its place alone, without
  knowing what each kind adds.
- **Nothing more of an operation is answered,** as nothing more is when
  it is alone. What it changed (a grace's end, a spec's revision, a
  requester and the reservation moved, a hold lifted) is in its task's
  documents, as after the operation alone, and a later decision names
  the revision a document gave (5.2).
- **Its worst case** is `../../domain/engine.md`, section 13's.
- **A client never chains requests** for what the person sees as one
  act, so it never has to undo the first when the second is refused.

### 3.5 Proposing is explicit

- **A request beyond authority is refused,** for a party as for an
  agent. A `Create` or an `Amend` whose authority, budget, deadline or
  grants pass what the party may give alone is refused as lacking
  authority: what it lacked and by how much, and whether a proposal of
  it could be accepted (section 4); to whom it would go, the offer says
  (5.3). Nothing is proposed on the party's behalf.
- **`Propose` asks.** The party proposes the action, which waits for the
  roles the project's policy names for its kind
  (`../../domain/tasks.md`, section 9). Where the policy names none that
  may accept it, it is refused at admission as lacking authority, saying
  that no proposal could be accepted: never a proposal that waits for
  nobody (`../../domain/people.md`, 5.1). A proposal once made, or one a
  task makes, waits as `../../domain/tasks.md`, section 9 says.
- **A party's proposal is the party's:** its proposer is the party, an
  amendment's as a batch's, so its decision, and a rejection's reason,
  reach them.
- **Why:** a person is never asked by accident, the reason
  `../../domain/engine.md`, 7.3 gives for agents, and a service is a
  program. Proposing when beyond is sugar: a helper that sends
  `Propose` where the offer says a draft is beyond the party (5.3).

### 3.6 Templates

- **A template is a recipe for a task of one shape:** the defaults that
  turn "a chat with these words" into a complete `Create`. It is data,
  numbered and never renumbered, in the application's configuration.
  What each carries, jig's two defaults (chat, template 1; goal,
  template 2), and what a project's policy may override, are
  `../../protocol/configuration.md`'s, 3.1, with their JSON in its
  section 10.
- **The engine never expands or checks one.** It keeps the
  configuration's templates and each policy's overrides only to serve
  each project's merged templates in `me` (5.1). The client fills a
  template into an ordinary `Create` or `Propose`, a deadline's span
  sent as a span (3.2), and the engine checks that request as it checks
  one written by hand: a template grants nothing, and a client that
  ignores one only makes its own lists inconsistent, while authority and
  limits still hold.
- **Permissions stay primitive,** with no "may use template", and a
  task keeps the values it was made with
  (`../../protocol/configuration.md`, 3.1).

## 4. Answers

Every request has one answer, of one of these kinds:

- **done,** with its outcome: what was made (the first and the last of
  the tasks' numbers, consecutive in the batch's order, and the
  `AsAllowed` task, if any, with what it was given; a word's message
  number; a proposal's number, whom it waits for being in its
  document; an account moved out, with its record, once: 3.1), or what
  was decided; for a compound request, its
  operations made, in order, and the first of the message numbers they
  took, one each (3.4); and, when another party decided first,
  who, how and when;
- **refused,** with its facts, and for a compound request the place of
  the first operation that failed:
  - *role:* the party's role here does not allow it;
  - *authority:* what was lacking, typed (a budget and how much is left,
    a grant, a kind of task, a depth, a deadline, tasks left); and, where
    the request was judged against the party's role and pool and found
    beyond them (`../../domain/authority.md`, 8.4), whether a proposal of
    it could be accepted (3.5). A refusal that another check decided
    (the request's shape, a reference, an uncertain effect's choice, its
    funding, the project's spend per period, an `AsAllowed` that would
    give nothing) makes no claim either way. Who would accept a proposal
    is the offer's to say (5.3), and the proposal's document's once it
    is made, not the refusal's;
  - *no one to escalate to:* a party's root task whose escalations would
    have no holder (3.2);
  - *limit:* which one (the deployment's projects, the project's live
    tasks, its goals, the
    operations per request, a grace past its bound, the inbox full, the
    words too long, a key or a pasted address past the bytes `me` states (5.1), the LLM
    accounts' bounds: accounts in the deployment, accounts per holder,
    sources per account, sign-ins at once, program sources per worker);
  - *provider's rule:* an LLM provider's terms forbid it, naming the
    rule (a plan's uses naming another person, or a project with
    another party; another account for a task its account limits:
    `../../domain/accounts.md`, sections 4.2 and 4.3);
  - *account preference:* the account's number and why it cannot fund the
    prospective task: unknown, another provider, uses that do not serve the
    financial payer, or a different account while the kept account of that
    provider is limited. An otherwise valid unavailable account is accepted
    and waits; acceptance checks a proposal against its accepter's funding;
  - *moved:* the object's revision now, and what changed;
  - *ended:* the object has ended, and how;
  - *unknown:* nothing by that name the party may see;
  - *name taken:* another project holds the name a `MakeProject` or a
    rename gives, under simple case folding (`../../domain/core.md`,
    6.1); it names no project, which the party may not see;
  - *not offered:* the deployment does not offer it (an executor it does
    not have, a charter its newest revision removed);
  - *stale:* a change or a proposal of settings names a revision other
    than the newest, which it names (`../../domain/engine.md`, 17.2);
  - *judged:* an application's connector refused an item of its kind of
    settings, naming the item and with its reason, in its own words
    (`../../domain/engine.md`, 17.5);
  - *full:* the settings' revisions or charter versions held would pass
    their limits, naming the oldest revision held and its live tasks,
    cut at their limit, which a `MoveToNewest` may move
    (`../../domain/engine.md`, 17.8);
  - *circle:* the batch or the dependencies an amendment adds would close
    a circle of waits, with the live tasks or among its own: the circle's
    tasks, in order, each waiting for the next, cut at its limit
    (`../../domain/tasks.md`, 6.7);
  - *dependency:* a dependency an amendment adds has failed or was
    cancelled, naming it and how it ended (`../../domain/tasks.md`,
    section 7);
  - *kept:* a person task's answer while it is kept, naming its pending
    dependencies (`../../domain/tasks.md`, 5.7);
  - *not well formed:* which task or operation, and why (two tasks of a
    batch giving `AsAllowed`; the conversation flag on a task that is not
    an agent's; a proposed batch giving `AsAllowed`; a project's name out
    of shape, saying which rule; a change of settings whose settings
    would fail their check, naming the setting and its fault, one of
    the check's closed list (`../../domain/engine.md`, 17.1), or that
    changes nothing). A client words each refusal of a change from
    these facts, checking no setting itself;
  - *sign-in redirect:* a finish refused, saying why: *not waiting*
    (no sign-in of the party's under that number waits for its
    redirect: none, another party's, one that has taken one, ended,
    or begun before the engine's restart), *address* (not the
    sign-in's redirect: another scheme, host, port or path, a
    fragment, a field twice, past its bounds, or without its code or
    issued client id), *state* (another sign-in's: the sign-in still
    waits), or *too many* (it has refused a few for their state, and
    takes no more pastes, still waiting for its browser: skein's
    `oauth.md`, 6.5; `../../domain/accounts.md`, section 5);
  - *key conflict:* the key names another ask, which is a client's bug
    where the client drew the key (3.1);
  - *account move:* an account's move refused, saying why: *not signed
    in* (no signed-in source holding a record that reads), *not ours*
    (a moved file of another application, or of no configured LLM
    provider's registration), *held* (its subject held by another
    holder), *cancelled* (a *written* for a move no longer under way:
    the client removes the file it wrote), or *moving* (the source is
    out with a move, or back from one and not yet refreshed:
    `../../domain/accounts.md`, section 6);
- **busy** or **not ready:** nothing was made and nothing kept; send it
  again, under its key, after a backoff;
- **signed out:** the sign-in has ended; sign in, then send it again;
- **version not served:** the engine does not serve the version the
  request's owner is encoded in. Nothing was read, made or kept. It
  names the owner and the ranges the engine serves (section 8).

A client hears one more, which no engine sends: **no answer**, when the
connection failed before one came; whether the engine has the request is
unknown, and the key makes sending it again safe.

How the client's link hands each answer to its request table is
`../../protocol/client.md`'s, section 5; a request answered version not
served is held, kept and unsent, for its owner's version (client.md,
8.2).

## 5. Documents

### 5.1 What they are

- **A document** is a typed value about one subject, from the engine's
  committed state, as the party may see it. Its owner is jig, or the
  application: jig's documents carry jig's values only.
- **jig's documents:**
  - *`me`:* the signed-in party's own document, always about whoever the
    credential belongs to: the party (number, kind, login and name);
    their projects, each with its number and its name, which every
    project has and no other project of the deployment shares
    (`../../domain/core.md`, 6.1), their role there and their pool;
    what waits for them, counted exactly (questions, proposals,
    escalations, person tasks), and their updates unread, counted up to
    a limit; their read position over results; each project's merged
    templates (3.6), each with `resolved_default: Option<LlmChoice>`
    beside its configured LLM intent. The preview uses Create's own
    resolver, charter order, effective creating authority and current
    receiving-party payer (`../../domain/engine.md`, 11.1). An explicit
    choice previews itself; an omitted choice with no eligible model
    or an executor still *chosen* gives `None`. It grants and reserves
    nothing. Shapes retain configured intent, never the preview, and
    a clipped catalogue gives no order from which to guess it. Relevant
    account/source, policy, membership and effective-executor changes
    invalidate it (7.2). The document also gives what they may create
    in each project, as the bounds a draft is compared with (5.3);
    their role in the deployment, if
    any; the models they may choose in each project, as the brief's
    catalogue gives them (`../../domain/engine.md`, section 9): each
    with its LLM provider, window and output, the efforts it takes, its
    prices (amounts per `unit` tokens in the deployment's spend unit;
    cache writes are priced at input when no separate price is given)
    and its optional description as configured, so that a task's LLM
    choice (3.2) is drafted from it; the most bytes a key may have,
    which bounds a request carrying one (3.1), and the most a pasted
    sign-in address may have, the driver's redirect bound; and the retention of the
    engine's answers (3.1). It carries no version: the stream's hello does
    (section 8);
  - *a task:* its header, overview, budget and authority; its fields
    (3.2): tracked and priority, label, flags, last activity, and what
    it waits on; its LLM choice with the creator's optional account preference,
    the separate account kept by the last successful claim, and its next-run
    account/source wait with the source's typed state and optional wall reset
    (unknown stays unknown), or a credential or host wait. `Task.run_wait`
    also carries `Refused(TaskWaitRefused)`: `Hello { kind, at, why }` is
    the fleet's latest observed allowed-kind refusal, suppressed by any
    admitted connected non-draining allowed-kind host; `Program(OpenProgram)`
    names `Model` or `Source { account, source }`. The latter is carried
    without a producer until the Open compatibility slice. The client uses
    its own `OwnerHostKind` and `OwnerHelloRefusal` vocabulary; the canonical
    deployment translation retains the existing shortfall context limits.
    These are derived document facts, never a new hold or stored state.
    Client wire 11 carries both arms; status and person-facing words are a
    separate follow-up. Task summary rows do not gain a wait field. Its hold
    and its escalation if held; its dependencies, each a task's number
    and phase, pending or done, with when and by whom each was added
    after the task was made, and whether they keep it
    (`../../domain/tasks.md`, 5.7); the proposals waiting in its subtree; how
    far its activity has committed; and the acts the party may make on
    it, with what each would do (5.3, 5.4);
  - *a task line:* one record for every list of tasks (6.1): the task's
    chip; its project and its requester; tracked and priority, label
    and flags; when it last changed, its last activity, and when it
    ended, if it has; its deadline; what it waits on; its budget spent
    and left; its live subtree counted by phase class, with how many are
    held below; and, for a task with a conversation, its requester's
    read position over its turns and their unread count;
  - *a chip:* a task's number, title, executor's kind, phase; a closing
    phase says what the task waits for: its delegates still closing,
    counted, and the connectors it waits on to settle, by number. Their
    effects are the connectors' own, and jig's documents never count
    them;
  - *a tree:* a task's live subtree, as chips, with the proposals waiting
    on each, folded beyond the watch's window into counts by phase, with
    counts of what is held below;
  - *an inbox:* the party's entries, each naming its object and its
    project, in its groups, within a window, with the party's read
    positions: one over results, one per conversation over its turns;
  - *a proposal, an escalation, a question, a person task, a result:* each
    object whole, with the acts the party may make on it and what each
    would do; a pending proposal of settings with its review
    (`../../domain/engine.md`, 17.6): the revision it was made against,
    the newest, and for each item the setting, what becomes of it, what
    it is now (its value, following its default or the change of which
    revision, or not offered) and what it would be, each value within
    the document's bound or with its size, as a revision's items are
    (section 6);
  - *an activity line:* one of a task's words, run boundaries, turns,
    results heard, news heard, decisions, in commit order; a turn as its
    bytes, within the page's byte bound, which the client's link
    decodes, with no read of its own, into the client's turn or into a
    turn that does not decode, with why (client.md, section 10): a turn
    that alone passes the page's bound comes with its size instead, and
    does not decode, as too large; one the engine could not read whole,
    as malformed;
  - *a project's,* its policy (its templates' overrides among it), roles
    and spend, a note; a project's document carries its recurring
    tasks' lines, each saying when it next runs: when the next period
    begins, where the period's source said so; otherwise at the next
    period, with the current period and when it began
    (`../../domain/authority.md`, section 7);
  - *the deployment:* hosts, each with its program sources' logins;
    the deployment's LLM accounts, as the party's own are (below), with
    the runs using each and the tasks waiting on each; capacity;
  - *the party's accounts:* the party's own LLM accounts, each with its
    LLM provider, display name and holder; its sources in order, each
    with its state: usable; limited, until when where the provider said;
    refused, and why; needs sign-in, and for a program on which hosts;
    misconfigured; unloaded, where and why; a program source's state on
    each host; and its uses. With them, the facts the providers' terms
    ask an application to show: whether each is a plan, its LLM
    provider's name and manage-usage address, and a plan's first use;
    the party's sign-ins under way, and how the last ended: made,
    renewed, or refused for a subject another holder holds; the checks
    they asked of a program's login that a host has yet to answer, with
    those hosts; and two facts a client waits on: *usable*, that an
    account which serves the party's work (their own, those whose uses
    name them, and the deployment's: `../../domain/accounts.md`, 4.3)
    has a source usable on a host, and *settled*, that no sign-in or
    check the party asked is in progress
    (`../../domain/accounts.md`, sections 3 and 5). It holds no value,
    and the words and screens are the application's.
  - *the settings:* the newest revision's number; each setting, by kind,
    scope and number, with its name where it has one, whether it
    follows its default or a change, and the revision that last changed
    it; each revision held, with who decided it, when, the proposal it
    came from, and how many live tasks hold it; the pending proposals of
    settings, by number (`../../domain/engine.md`, section 17). It
    carries no value: one setting is read whole on its own, from the
    newest or a held revision (section 6). Seen by a party with a
    deployment role, and a project's items by a party with a role there.
Program host facts use each worker's whole configured name. Inventory still awaited
is unknown, including after a reconnect; a disconnected worker's earlier inventory
is not a current login fact. The deployment lists all issued names, including
retired names awaiting cleanup; a party's accounts watch lists only that holder's
names. Each row distinguishes the login observation from the last sign-out's
result and the desired cleanup state. Bounded diagnostics say when cut.

- **What a task waits on** is the engine's to compute, since only it
  holds the tree and the inbox together: a party (who), for a question,
  a proposal, an escalation or a person task in its tree; its
  dependencies, for a kept task: the pending ones, the oldest first, and
  how many; its delegates; news; or nothing (`../../domain/tasks.md`,
  section 3).
- **Its last activity** is when it last heard words, replied, or had a
  delegate end: a wall time, kept on every task, by which lists order.

An account row in the party's `accounts` or owners' deployment document
also carries its provider's configured name and optional manage-usage
address. The name uses the document's cut-aware text bound; the address
is whole, at most 2,048 bytes. The client link rejects addresses outside
`valid_manage_usage`'s ASCII HTTPS subset before publishing either account
document form. This subset is safe for the view's external-link builder;
configuration applies stricter host-label and port checks.
Provider and account numbers remain whole.
The account's plan flag follows its current sources and configured
signed-in-plan flag; program sources remain plans. These facts require no
read of another holder's accounts. Deployment capacity history labels
account sign-in, sign-out and change without their provider, holder,
subject, source, key or digest payloads, and includes the accounts',
holder's accounts', sources' and sign-ins' room limits under the same
keyed retention and bounded entrance-history rules as other actions.

The public provider catalogue in `me` carries the configured provider number,
cut-aware name, sign-in and key offers, whole program names (at most 32 bytes
each), the no-credential offer, signed-in-plan flag and whole manage-usage
address. Each offer carries the actual key ceiling; `me` also carries that shared
ceiling for a common key input. It is the smaller of startup's accounts token
limit and the client wire's carried-value limit. Offers describe configuration,
not a running backend. Provider and program lists use the document line bound;
each has its own cut flag. Absence from a cut catalogue means nothing.

`accounts` also carries volatile readiness rows keyed by provider and payer:
the party itself, then every project it may read, including deployment-role
membership where it is a readable project. Rows expose no serving holder,
account or source identity. Coverage is the same current people/uses join as
run selection. A row is usable when a serving source can run on an eligible
connected host; unavailable when the complete known evidence rules that out;
unknown when program login evidence is still missing. Slot and budget
admission, task charter restrictions and account pins remain the concrete run's
check. Settled is optional: false for this party's pending sign-in at that
provider, unknown when program checks cannot yet be known, true otherwise.
The readiness list has its own cut flag; it never makes a negative statement
about an omitted provider/payer. Decisions use full domain state, independently
of displayed account/source/host prefixes. The existing aggregate usable and
settled fields remain available.

### 5.2 Revisions

- **One for each thing a party decides about,** which moves only when
  that thing changes, never with a priority, spend, a delegate's phase
  or a run's progress:
  - *a hold's,* on a held task: what holds it, and why. It moves when
    the task is held anew or its reason changes, never when its
    escalation passes up. A release and a decision about its escalation
    name it;
  - *a spec's,* on a task: its words, parameters, wake policy,
    dependencies, authority, instructions and whether it is tracked; an
    amendment names it;
  - *a person task's:* its question, options and the results it shows;
    an answer names it, and an amendment of its address;
  - *a note's,* as the notes child keeps it; a correction or a deletion
    names it.

  A proposal never changes once made, so deciding it names its number;
  where it waits may move, and a party it no longer waits for is refused
  as no longer the one it waits for. Stopping, cancelling, moving and a
  change of priority alone name no revision.
- **Where an escalation waits is checked apart from its hold.** Deciding
  an escalation (releasing the task from it, leaving it held, passing it
  up) needs the escalation to wait for the party, and is refused as not
  waiting for them otherwise. Releasing, amending or cancelling the held
  task needs the task in the party's tree, or their role in the project
  to allow it, wherever its escalation waits
  (`../../domain/people.md`, 5.1): a release that names the hold's
  revision from before a pass up is made, not refused as moved.
- **The revision is the promise below the presentation.** Whatever
  showed the person a decision, the request names the revision of a
  document the engine gave, and is refused if it moved, for every
  client and every presentation alike.

### 5.3 What else they carry

- **Acts offered.** Each object's document says which acts the party may
  make on it now, as the engine judges it, and what each would be:
  *made*; *proposable*, and to whom, where it is beyond the party and a
  proposal of it could be accepted; or *refused*, and why (role,
  authority, nobody who could accept a proposal of it). An act whose
  answer depends on what the party writes is offered *bounded* (below).
  A presentation offers what the document says, and the engine judges
  again when the request comes.
- **Offered with bounds.** Creating and amending are made, proposable or
  refused by what the party writes: a budget, a deadline, a grant. A
  document cannot say which for every draft, so it gives the engine's
  facts a draft is compared with, in two bounds, and what happens beyond
  the first:
  - *creating,* in `me`, per project: the executors the party may create
    with, and their pool left; within, the largest budget and the latest
    deadline they may give a task alone, and the authority they may give
    it, from their role and their pool: whole, their role's, one level
    shallower in delegation and its tasks within a tree's; and, past
    that, the largest
    budget and the latest deadline any root task of the project may
    have, from its spend per period, its policy's ceiling and the
    deployment's rules;
  - *amending,* in a task's document: within, the whole authority the
    party may give alone (their role's, its budget no more than their
    pool has left, its tasks no more than the task has left), the budget
    they may add to the task, and the grants they hold, listed; and,
    past that, the policy's ceiling, its budget no more than their pool
    has left;
  - *beyond the first bound:* proposable, to the roles the project's
    policy names for that kind of proposal, in their order; or, where it
    names none, refused as lacking authority, saying that no proposal
    could be accepted.

  A presentation, or the helper `shapes` (client.md, section 19),
  compares the draft with those facts before the request is sent, and
  says which it will be. A task is within the first bound when its
  executor is listed, its budget no larger and its deadline no later
  than the bound's (a task with no deadline being beyond any bound that
  has one), and its authority within what the party may give; a budget
  `AsAllowed` is within it, and its answer says what it gave; a
  deadline given as a span (3.2) is within both bounds, since no
  presentation reads a clock: the engine judges it at the decision,
  as the wall it then makes (`../../domain/authority.md`, section 7),
  and answers a `Create` it puts past the first bound as beyond
  authority, saying whether a proposal of it could be accepted. Past the
  second bound, or with an executor not listed, it is refused as not
  offered, and nothing is sent. An amendment that widens no authority is
  made. One that widens anything, even while it narrows something else,
  is judged whole: the authority it leaves the task must fit the first
  bound, the budget it adds be no more than the party may add, and each
  grant it adds be among those listed; past the ceiling it is refused as
  not offered, and nothing is sent. Between the two bounds, a
  presentation sends `Propose` where the offer says proposable, or says
  the refusal in place, sending nothing. That is the engine's word read,
  not authority computed: the engine judges again when the request
  comes, by the rule that made the offer. No read assesses a draft.
- **Reordering,** in a project's document: whether the party may change
  its tracked tasks' priorities, in one compound request of
  priority-only amendments (3.4). It is judged by the party's role in
  the project alone, as a change of priority is ("maintainer, for
  tracked or priority", `../../domain/people.md`, 5.1), and the policy's
  word on that role's amending: made or refused, never proposable, and
  never bounded, since a priority asks no authority and names no
  revision (5.2). Amending's offer in a task's document bounds what the
  party may write of the task's authority and budget, not its priority,
  so a client that moves tasks reads the project's offer, not each
  task's: a party who may amend a task of their tree may still not
  reorder it.
- **Parts.** Where a jig document names an application's value, it
  carries the part's reference: its kind, its owner, and jig's key (a
  resource's path, a kind's number, a task's or a proposal's number).
  Its owner is the connector whose value it is: every part jig's
  documents name is a connector's, so the number a part carries is its
  owner number (root.md, section 15). The value is in the application's
  own documents.
- **Times are wall times,** which the store keeps, so a document means the
  same after a restart. Accounts' source limits and retries carry the
  wall fact retained when they were created (`../../domain/accounts.md`,
  3.5): a task's wait, the account's and the deployment's projections
  all use that exact fact, as the store does, never convert again at
  read time. Their timers remain monotonic.
- **Bounded:** each document within its limits, in bytes and windows,
  never in rows sized for one client's screen. A path and an authority
  are bounded by the engine's names and authorities, not by a window's
  rows (below).
- **A path crosses whole.** A resource's path and a pattern are names,
  compared segment by segment (`../../domain/authority.md`, section 4),
  and the client acts on them as they are (a filter by a resource, a
  draft's pattern). A document carries each whole, with as many
  segments as the engine admits in a name (the most of its authority's
  and its tasks' bounds on a name's segments): the engine never cuts
  one, nor refuses a document for one. A document's bound on its short
  lists is not a path's.
- **An authority crosses whole.** A draft is compared with an authority,
  and an amendment's authority (3.2, `Amend`) is drafted from the one a
  document carries, so the client holds each as the engine does: a
  task's authority, and each authority an offer's
  bounds, a proposal or an amendment carries, with its grants, its
  executors and its notes' scopes; and an amending offer's grants
  listed and the implications it names. A document carries each list
  whole, with as many entries as the engine admits there: grants, a
  grants listing and notes' scopes, the most of its authority's and its
  tasks' bounds on an authority's grants; executors, the most of its
  authority's bound on executors and its tasks' on executor kinds;
  implications, its authority's bound on them. The engine never cuts
  one, nor refuses a document or an offer for one, nor marks one cut. A
  document's bound on its short lists is not an authority's.
- **Texts are UTF-8,** words for showing. A source's text is UTF-8 where
  it enters the engine: a party's words, refused at their request's
  entrance when they are not; a task's spec, its words and its text
  parameters, refused with a spec's other faults wherever the tasks
  child admits a spec, a batch's, an amendment's or a proposal's,
  whoever made it: a party, an agent's tool, a procedure's step, a
  recurring task (`../../domain/tasks.md`, sections 3 and 4); a task's
  result's texts, such as its words or a failure's reason, refused as a
  result its contract does not admit wherever the tasks child admits a
  result: a run's end, a procedure's step, a restore
  (`../../domain/tasks.md`, section 3); an identity's login and name,
  refused where they are minted; a run's output; an application's words
  through its own documents. A value that is not text, such as a
  commit's id, is a task's parameter or a result's words only as text
  its application spells, such as its hexadecimal digits. A text cut to
  its bound is cut at a character's boundary, never inside a character,
  and says it was cut. So every text a document carries is UTF-8, as the
  wire's text fields check (`../../protocol/client.md`, section 3), and
  the engine never makes a document its wire refuses.

### 5.4 A decision's effect

- **What a decision will do is data,** in the documents that offer it,
  from the engine's state as the document was made:
  - *accepting a proposal:* what it grants and what it costs, and the
    pool that funds it, after;
  - *releasing a held task:* what lifting its hold does: the task judged
    afresh, a run started again, or an uncertain effect made again;
  - *cancelling:* what ends: the live tasks below the task, counted by
    phase; and, of the task and those below it, the live runs (each
    attempt claimed whose answer its task has not heard) and the pending
    proposals (each a task made that no holder has decided). Its effects
    are its connectors' (5.1): each withdraws the entries not yet sent and
    settles those in flight, and the document counts none of them;
  - *moving a task:* the reservation that moves to the party's pool.
- **The confirmation is the application's.** jig holds the revision,
  the "once" and admission below the API; whether and how a person
  confirms a deciding act, and which of these facts it shows, is the
  application's choice, which it makes wherever it confirms. jig's
  reference UI keeps jig's confirmation (view.md, 18.5); jig's check of
  it is testing.md's, 6.2.

### 5.5 Configured-bound holds

- **A typed category.** `HoldCategory::Exceeded(Exceeded)` carries the
  client's own `Exceeded { bound: Bound, value: u64 }` and the nine
  `Bound` tags of `../../domain/hosts.md`, 6.3. The payload is `Copy` and
  inline. `value` is the configured allowance in the bound's unit,
  never a measured size. `History` does not identify the ceiling's unit;
  its value is still carried and stored. People's `HoldReason::Exceeded(Exceeded)`
  carries the same cause in revision refusals, with people's own types.
  Translations between the domains' copies are exhaustive, without a
  wildcard.
- **Words name the allowance.** Jig's hold phrases, including the
  reference view's, take `N` as a Number parameter except for `History`,
  whose phrase takes no number because its unit is not known:

| Bound | Phrase |
|---|---|
| `Reasoning` | Held: the model's reasoning went past its configured allowance of N bytes |
| `Metadata` | Held: the model's metadata went past its configured allowance of N bytes |
| `Items` | Held: the model's answer went past its configured allowance of N items |
| `Answer` | Held: the model's answer went past its configured allowance of N bytes |
| `Tools` | Held: the run's tools went past their configured allowance of N tools |
| `HistoryItems` | Held: the run's request went past its configured allowance of N messages and blocks |
| `History` | Held: the run's history no longer fits its configured allowance, even compacted |
| `Window` | Held: the run no longer fits the model's window of N tokens |
| `Output` | Held: the request went past the model's output ceiling of N tokens |

- **Release keeps its authority.** The hold is lifted by an authorized
  `Release`, not by words. The task's immediate hold and release rules
  are `../../domain/tasks.md`, 5.5 and 6.

## 6. Reads

A read asks for one document, or one page of a list, and is answered
once, with the document or why there is none. It decides nothing and
needs no key. Its answer is made from the engine's state as it is, which
may run ahead of the store by the commits in flight, so it is held as an
output of a decision that made no commit: tagged with the last commit
made, and released once that is durable (`../../README.md`, 6.2). What a
party reads is therefore never lost by a crash.

| Read | From |
|---|---|
| `me` | live state |
| `task{n}`; `proposal{key}`; an escalation; a question; a person task | live state, or the store once ended or decided |
| a result | the store, or live state while a stub names it |
| a task's history, a page, for it or its subtree | the store |
| `activity{n}`: a task's activity, a page, from its end | the store: words, run boundaries, turns, what it heard |
| `turn{n, attempt, turn}`: one committed turn of a task's activity | the store: the task's transcript (`../../protocol/store.md`, 8.3) |
| `inbox`: a group of it, a page, filtered by project, kind or root | live state, then the store past the engine's limit |
| `tasks{query}`: a task query's page (6.1) | live state, then the store's indexes of ended tasks |
| a project; its spend, a page; its notes, a page | live state; the store; the notes child |
| the deployment | live state |
| `settings`; `setting{kind, scope, number, revision}`: one setting's value whole, as the newest reads it, or, for a charter, as a held revision reads it; `revision{n}`: a revision's record and its items, as it set them; `revisions`: its records, a page, newest first | live state; the store's history, for a revision no longer held (`../../domain/engine.md`, 17.7) |
| an application's read, naming its owner, its scope and its window (7.5) | its owner, the connector, through the engine's root |

- **An ended task is a finished read.** Opening its task or tree watch
  delivers its retained document once, then ends the watch (7.2). A
  client waiting for a condition tests this first document too: a condition
  already true answers at once, and an ending alone proves no condition.
  A task read returns its live state or retained terminal history under
  the party's current visibility. A retained load
  waits for the preceding commits to be durable and checks current
  sign-in and visibility when it completes; it never restores a retired
  row into the live set (`../../domain/engine.md`, 5.2 and 5.3).
- **Pages** are within a row and a byte limit, and carry a cursor for the
  next, or say there is none (`../../domain/engine.md`, 5.3).
- **Refused** for what the party may not see, as unknown; when the
  engine's loads are full, as busy; when the sign-in has ended, as
  signed out (client.md, section 12); in a version the engine does not
  serve, as version not served (section 8).
- **A question's answer** is read with the question, by whoever may read
  the question: a delegate is in its requester's project, and the
  answer's words are those of the task the question was put to
  (`../../domain/engine.md`, 7.2; `../../protocol/store.md`, 7.3).
- **A turn** is named by its task, its attempt and its number within it
  (7.3). It is read once and never watched: a committed turn does not
  change. It is read with the access of `activity{n}`. Its answer is:
  - *the turn:* smith's bytes, which the client decodes as it decodes
    activity's turns (client.md, section 10);
  - *unknown,* when it is not there or the party may not see it;
  - *gone,* past its transcript's horizon, as a dropped transcript is
    (`../../protocol/store.md`, 8.5 and 11.1);
  - *busy,* as any read;
  - *too large,* with its size in bytes: from the engine, past the
    single read's bound, the store's per-read byte budget
    (`../../protocol/store.md`, 7.3); from the client, past its limit
    for one document (client.md, section 14). On activity's pages the
    client keeps the size the wire carries for a turn too large for a
    page (5.3);
  - *malformed,* when it does not decode, as a page's turn is (5.3).
- **A revision** is read by its number, by a party who may read the
  `settings` (5.1), a project's items by a party with a role there.
  `revision{n}`'s answer is:
  - *the revision:* its record (the revision it was made against, when,
    who decided it, a party, the release or the first start, the
    proposal it came from, whether it moved running work) and its
    items, each a setting by kind, scope and number and what became of
    it, set with the value whole, removed, or reset; an item's value
    past the single read's bound comes with its size instead, as too
    large, read whole by `setting{…}` while its revision is held;
  - *forgotten,* a revision made but erased past the history
    (`../../domain/engine.md`, 17.7), naming the oldest kept;
  - *unknown,* past the newest;
  - *busy,* as any read.
- **`revisions`** is the history a page at a time, newest first, from
  the newest or a cursor: each row a record without values, its items
  as settings and what became of each, cut at the page's bound with how
  many more. The last page says whether older revisions were forgotten.
  It is read, never watched: the `settings`' mark says a revision came.

### 6.1 Task queries

- **A task query** selects tasks by a fixed set of keys, any conjunction
  of them: project; requester (a party, or a task); root; parent;
  label; phase class (working, waiting, held, closing, ended); executor
  kind; tracked. It orders by number, ascending; last change or last
  activity, latest first; priority, ascending, tracked tasks before
  untracked; or ended at, latest first, its live group, which has no
  end, by number ascending. Ties go by number, ascending. It reads
  within a window, by cursor. Its rows are task lines (5.1). It is read
  a page at a time, and watched as `tasks{query}` (7.1). A list of a
  party's conversations, of a project's goals by priority, of a task's
  ended delegates, are each a task query.
- **Live first, then ended,** in two groups, each with its own cursor.
- **Live tasks are scanned,** in the order asked: the tasks child keeps
  no secondary index, and its live tasks are bounded by its limits, so
  any conjunction of the keys costs no more than one scan. The fixed set
  bounds what marks a watch (7.2) and what crosses the wire, not the
  filtering.
- **Ended tasks are found through the store's indexes**
  (`../../protocol/store.md`, 4.4, 7.3 and 7.4), in each index's order,
  within a label the latest ended first:
  - *by requester:* every ended task whose requester is a party, by that
    party and label;
  - *by project:* only ended tasks flagged *listed*, by project and
    label;
  - *by requester task:* a task's ended delegates, in their family.

  A query's ended group is served by one index, chosen by its keys in
  this order: a requester party; a requester task or a parent; a
  project. Its other keys filter the index's pages as they are scanned,
  and a page so filtered may come back cut, saying so. A query naming
  none of these has no ended group, and its page says so.
- **What it does not serve:** full-text search
  (`../../protocol/store.md`, section 16); ended tasks filtered by
  outcome, resource or spend; keys valued by a procedure's parameters,
  which belong in the application's own records naming the tasks; ended
  tracked tasks across projects.
- **Retention:** an ended task forgotten once read leaves the indexes,
  with its read position, once its requester, a party, has read its
  result and its turns, and no live task names it
  (`../../domain/people.md`, section 6); a kept one stays to the
  deployment's retention horizon, and whole while none is set
  (`../../protocol/store.md`, 11.1).

## 7. Watches

### 7.1 Subjects

| Subject | Its document | Who may watch |
|---|---|---|
| `me` | `me` | the party, always |
| `task{n}` | the task's document | a party who may see it |
| `tree{root}` | a task's live subtree, within a window | likewise |
| `chips` | a bounded set of tasks the client names, as chips | likewise, each |
| `inbox` | the party's own, across projects, within a window | the party |
| `tasks{query}` | the live task lines a query selects (6.1), in its order, within the watch's window, saying whether there are more | a party with a role in the project it names; a query naming no project, only the party, for tasks whose requester they are |
| `run{n}` | a task's live run: its attempt, its provisional text and current call (7.3) | a party who may see the task |
| `deployment` | hosts, accounts, capacity | a party with a deployment role |
| `settings` | the settings (5.1) | a party with a deployment role; a project's items, a party with a role there |
| `accounts` | the party's accounts (5.1): their own LLM accounts, their sources and states, their sign-ins and checks under way, and whether an account serving their work is *usable* and everything they asked is *settled* | the party, always |
| an application's subject, naming its owner, its scope and its window | the application's document | a party the scope admits (7.5); what of it they may see, as its owner, the application's connector, says |

The objects a client shows apart from these (a proposal, an escalation,
a question, a person task, a result, a task's activity, a project, a
note) are read (section 6), and read again when a watch that names them
says they moved. A turn is a read and not a subject: a committed turn
does not change, so it is read once (section 6).

Account source/uses and fleet changes invalidate accounts watches; membership
changes invalidate them too, including another member joining a sole-holder
project. The catalogue is immutable startup configuration and arrives on every
new `me` watch, including after restart.

### 7.2 Level-triggered

- **A delivery is a whole document,** the subject's current state within
  the watch's window, computed when the delivery is made, not when the
  change happened, and held until every commit it reflects is durable,
  as a read's answer is (section 6).
- **One delivery in flight per watch.** A change while one is in flight
  marks the watch changed; when the client has taken that delivery, the
  next is computed from the state as it is then. Changes in between are
  coalesced, never queued.
- **Nothing is missed:** the next delivery has every change. A slow
  client sees fewer deliveries, a fast one more; neither holds the engine
  up, and what the engine keeps per watch is a mark and a window, never a
  backlog.
- **The first delivery** is the subject's document when the watch is
  admitted. For a task or tree whose root has ended, the core loads its
  retained task facts through the bounded durable read path, checks the
  current sign-in and visibility, and delivers once. A tree still means
  the live subtree (5.1): its retained ended root has no live descendants,
  folded counts or continuation, whether it was a root or a delegate.
  The watch ends with the existing subject-gone reason after that delivery
  is taken or dropped. Missing or hidden retained facts end it without a
  delivery; an ending alone is never terminal success. Other ended
  subjects are refused `Unknown`. No retired task is restored to serve it.
- **Ended,** each saying why: the client left; its subject ended and left
  the engine; the party may no longer see it; the sign-in ended; the
  engine is stopping.
- **Admitted** against the party's sign-in and what they may see, and the
  watches the engine allows per party, per stream
  (`../../protocol/client.md`, section 13) and in all; past them, refused
  as busy. A watch holds its place from its admission until the engine has
  ended it: while its closing is in flight, and while its stream's end
  or its last delivery is (`../../domain/engine.md`, section 11). A
  deployment allows each party, and each stream, at least the engine's
  watches one client opens at its limit, `me`'s included (client.md,
  section 14), so that a party's one client within its limit never finds
  its room full. Busy is the room full for now, and the client opens the watch
  again after a backoff (client.md, 8.1), as a request refused busy is
  sent again (section 4).
- **A window past the engine's bounds is clamped,** at admission, never
  refused: the watch keeps the window the engine's rows admit, and each
  delivery is its document within it, saying there is more as its
  subject's document does (a tree's folded counts and its cursor; a task
  query's and an inbox's cursor; 5.3). An application's window is
  clamped likewise before its connector is asked, and so is a live
  read's window. So busy is never a window's answer.
- **A document the engine cannot make within its bounds** for any window
  (a whole value past them, which its bounds are derived to rule out,
  5.3) is refused as unknown at admission, never as busy, since opening
  again would not mend it: the client ends the handle (client.md, 8.1).
  A delivery that cannot be made ends the watch, as its subject gone.
  An answer of its own is open (section 11).
- **An application's subject names its owner, its scope and its
  window,** as an application's read does: its owner by its number, the
  scope a project, the deployment, or the party's own (7.5). The core
  cannot read a connector's subject, so it admits on what it holds: the
  party's sign-in, their role in that scope, and the views' limits, as
  for jig's subjects. The connector, asked for the document, judges that
  the subject is in that scope and what the party may see of it, and
  answers unknown or not allowed otherwise. The engine's root only
  routes.
- **What marks a watch changed:** each decision's commit names the
  tasks, projects and parties it touched. A task touched marks the
  watches of that task, of every tree along its lineage, of chips naming
  it, and of every task query whose keys it matched before the decision
  or matches after it, so a task that leaves a query's set changes it
  too; a party's entries, role or pool touched mark their inbox and
  their `me`; a project's name or policy changed marks the `me` of every
  party with a role in it; an account's source, state, order or uses
  touched, a program's login reported on a host, or a sign-in or a check
  begun or ended, marks the `accounts` of the parties it concerns and
  the `deployment`'s, and the affected parties' `me` when template
  default eligibility changes. Membership and effective-executor changes
  also invalidate affected `resolved_default` previews (5.1). Matching a
  task against the open queries is bounded by the watches the engine
  allows, each a fixed set of keys. A connector marks its own subjects,
  through the root, as its state changes.

### 7.3 A run's live text

The one stream: what a run writes before its turn commits is not state
the engine keeps (`../../domain/engine.md`, section 11), but a person
reads it as it comes. Its deliveries decide nothing and follow no
commit, so they leave through the journal's door (`../../README.md`,
6.3), marked provisional.

- **The run's document** is its attempt, the turn being written, named by
  its attempt, its number within it and its completion's retry as the
  host reports them, that retry's text so far, and the call it is making,
  within a bound on bytes.
- **Read from an offset:** each delivery carries the turn's name, the text
  after the offset the client has, and the offset it ends at; the client
  takes it and is sent the rest when there is more. A new turn's text,
  or a new retry's, starts at zero under its own name, so it never resets
  another turn's; a later retry replaces the earlier retries of its turn.
- **Committed turns are not in it.** A turn's committed form reaches the
  client only through its task's activity, read once the task's document
  says the activity has moved, and held until durable (section 6).
  `jig_client::thread` keeps a turn's provisional text until then
  (client.md, section 10); the client's watch holds only the turn being
  written (client.md, 8.3).
- **Past its bound,** the text is cut from its start, at a character's
  boundary, and the delivery says so; the turn, once committed, has it
  all.
- **The watch follows the task,** not one attempt: a new run's text
  follows the last, and its beginning is in the document.
- **Only from a host that gives it.** A run's text and the call it is
  making come from hosts that hand them up (`../../domain/hosts.md`,
  6.5): a run in an agent process gives neither, and its document holds
  no text and no call, while its turns reach the client as they commit.

### 7.4 An application's streams

An application's data that its people follow as it comes (a log
followed, a metric's latest points) is a stream of the application's:
a subject naming its owner, its scope and its window, read from an
offset, as a run's text is (7.3). Its deliveries decide nothing and are
never a document of the engine's committed state: they leave through
the journal's door (`../../README.md`, 6.3), marked provisional, and an
application shows them marked so. It adds a kind of subject to the
application's own and a use of the journal's door, with no change to
7.1's subjects or 7.2's rules, and crosses on the client's one stream as
any watch's deliveries do (`../../protocol/client.md`, section 7).

### 7.5 An application's scopes and windows

- **Scopes.** An application's request, read or subject names one:
  - *a project,* admitted on the party's role in it;
  - *the deployment,* admitted on their role in the deployment
    (`../../domain/people.md`, section 3), as jig's `MakeProject` is
    (3.2);
  - *the party's own,* what is the signed-in party's across projects (an
    application's settings of theirs, the incidents they are paged
    for), admitted on the credential alone, and naming no party: the
    core names the party from the sign-in, as for `me`.

  The connector judges what any scope shows (7.2).
- **Windows.** An application's watches and reads carry a window, as
  jig's subjects do: bounded rows and bytes, at the cursor its owner
  gave, so that what the views keep per watch is a window and a mark,
  never a backlog, and every document fits the client's limits.
- **Streams** are 7.4's; **versions** are per owner (section 8).

## 8. Versions

- **Each owner's documents and requests are versioned,** jig's by jig and
  each application's by the application, and every message names its
  owner and its version (`../../protocol/client.md`, section 9).
- **The engine serves a range of each owner's versions,** which the hello
  of its stream states; `me` carries none.
- **A version not served is refused with a typed answer,** *version not
  served* (section 4), naming the owner and the ranges the engine
  serves, with nothing read, made or kept; a read and a watch in it
  likewise. What the client then does with that owner's requests, reads
  and watches, and when it sends what it kept, is client.md's, 8.2;
  what the application says of it is its own (the reference UI's:
  pages.md, section 8).
- **One jig pin.** In an application, `jig-client` and the engine come
  from one jig pin, so they agree on jig's records unless the engine was
  updated under a client still running. A change to the plumbing moves
  jig's wire version, and before a release each range is one version
  (`../../protocol/README.md`, section 7).
- **How jig is released and pinned,** and how a change of jig's reaches
  every application, is `../../README.md`'s, section 14.

## 9. What the engine's domain owes the client

What `../../domain/` owes this contract, each designed where named:

1. **The plumbing operations of 3.2 for parties:** a `Create` of a
   batch, person tasks among its executors; `Propose` of every kind of
   action, and `Withdraw`; `Cancel` with a grace, bounded by a limit of
   the tasks child's; `Move`; `Seen` over a conversation's turns; a
   release, an amendment and a person task's answer naming the revision
   they decide, a priority-only amendment naming none; an approval
   naming the state it approves; an uncertain effect's release saying
   whether to make it again (`../../domain/people.md`, 5.1;
   `../../domain/tasks.md`, sections 4, 7 and 9).
2. **The compound request** (3.4): the people child's limit on
   operations per request; operations admitted in order, one pool
   counted across them, committed in one decision or refused naming the
   first that failed; done, answered with the first of the message
   numbers its operations took, one each in order
   (`../../domain/people.md`, 5.1); and the tasks child's limit of
   goals per project, the live tracked tasks of one project, refused at
   the entrance as live tasks per project are (`../../domain/tasks.md`,
   section 11).
3. **Proposing explicit** (3.5): a party's create or amendment beyond
   their authority refused, saying what it lacked and whether a proposal
   of it could be accepted (section 4); a party's proposal naming the
   party as its proposer (`../../domain/people.md`, 5.1;
   `../../domain/tasks.md`, section 9).
4. **A party's root has someone to escalate to** (3.2;
   `../../domain/people.md`, 5.1).
5. **A task's fields** (3.2): its label, its conversation and listed
   flags, its retention class, its last activity, and what it waits on,
   computed; and `AsAllowed`, resolved at admission and answered
   (`../../domain/tasks.md`, section 3).
6. **Read positions over conversations,** kept by the people child only
   for tasks with a conversation, and those of ended ones in the store
   (`../../domain/people.md`, section 6; `../../protocol/store.md`, 4.4).
7. **Templates served, never expanded** (3.6): the configuration's
   templates and each policy's overrides, kept only to serve each
   project's merged templates in `me`; an override naming no template,
   or one another override of the same policy names, refused when the
   policy is changed; a retained LLM choice incompatible with the
   effective executor refused at startup, policy change and restore
   (`../../domain/engine.md`, 11.1;
   `../../protocol/configuration.md`, 3.1).
8. **Credentials minted on the server's word:** a sign-in committed for
   an identity the application's server verified, its provider of kind
   application (section 2; `../../domain/people.md`, section 3).
9. **An application's keyed requests,** admitted and kept by the people
   child as jig's are, told apart by a digest, judged as the party's when
   they ask the core for something, their outcomes kept by their
   connector for the retention (3.3; `../../domain/people.md`, 5.1;
   `../../domain/connectors.md`, 5.5).
10. **Refusals with facts,** as section 4 lists them
    (`../../domain/people.md`, 5.1).
11. **Revisions** of holds, specs and person tasks, in their documents
    and in the decisions that name them (5.2 lists them;
    `../../domain/people.md`, 5.1).
12. **Acts offered** in each object's document, made, proposable or
    refused, creating and amending with bounds, reordering in a
    project's document by the party's role alone, and a decision's
    effect as data (5.3, 5.4; `../../domain/engine.md`, 11.5).
13. **`me`, and the inbox across projects:** the party's entries in
    every project they hold a role in, needs counted exactly and updates
    up to a limit (`../../domain/people.md`, section 6).
14. **A task's activity kept in commit order,** including each party's
    words as written, so that a conversation reads whole after its words
    are taken (`../../domain/engine.md`, 7.2).
15. **The reads of section 6,** task queries among them (6.1), their
    answers held until the last commit made is durable
    (`../../domain/engine.md`, 5.2 and 5.3).
16. **Level-triggered views** with the subjects of 7.1, what marks them
    changed, task queries by their keys among them (7.2), a run's text
    by turn, retry and offset, and an application's subjects through
    the same watches, each naming its scope, the party's own among them,
    and its window (7.5; `../../domain/engine.md`, section 11). The
    views child keeps, per watch, its subject, its window, whether a
    delivery is in flight and whether its subject changed since; the
    subject's owner (the core, or a connector through the root) computes
    each document when a delivery is made, and the delivery is held as a
    read's answer is.
17. **A watch is not a keyed request** (3.2; `../../domain/people.md`,
    5.1).
18. **The party's LLM accounts** (3.2, 5.1): the accounts' requests
    admitted from parties, a sign-in answered with its address and
    ended on the `accounts` watch, a key named by the request that
    carried it, the accounts' bounds and the providers' rules refused
    with their facts, and each account's sources, states, uses and the
    terms' facts in the party's accounts and the deployment's documents,
    with the *usable* and *settled* facts of the first, marked as 7.2
    says (`../../domain/accounts.md`, sections 3 to 5;
    `../../domain/people.md`, 5.1).
19. **A task's LLM choice** (3.2): given at `Create`, changed by
    `Amend` for the next run, its charter's default where it gives
    none, within the authority's models; the models a party may choose
    in `me` (`../../domain/tasks.md`, sections 3, 4 and 7;
    `../../domain/authority.md`, section 3).

## 10. Below the domain

What the protocol layers owe this contract, which the client's protocol
designs (`../../protocol/client.md`):

- **jig's messages, transport-agnostic.** The API crosses as jig's
  messages: a keyed request, a read, opening and closing a watch, a
  stream gone, signing in and signing out, each answered once, each
  naming its owner and version, each with the credential it came with
  as opaque bytes; and one stream per client, carrying every delivery
  tagged with its watch (`../../protocol/client.md`, section 4). The
  clients' component, `jig-protocol-engine-clients`, takes them and
  answers them, and knows nothing of HTTP, paths, cookies, hosts or
  server-sent events.
- **The transport is the application's:** HTTP, a local socket for a
  command line, or an in-process channel for one process or a world.
  jig ships a default HTTP mapping, both ends, with example guards,
  which an application uses if it suits it, wraps, or replaces
  (`../../protocol/client.md`, section 17).
- **The credential is checked below the domain:** the component verifies
  it against its digest and hands the domain the sign-in's number.
  Signing in is jig's built-ins and `Mint`, the application's server's
  word (section 2; `../../protocol/client.md`, section 8).
- **A value is taken out below the domain:** a key a request carries is
  kept in the secrets' store by the clients' component before the
  domain sees the request, which names it by the request alone
  (`../../protocol/client.md`, section 5).
- **`jig-client-protocol-wire`** encodes jig's messages and documents
  for both ends, from one schema, with their versions; each
  application's own wire crate does the same for its own.
- **Turns** cross as their bytes, inside activity lines, and are decoded
  by `jig-protocol-transcript` in the client's link.
- **What the application's server keeps:** a credential from being used
  across sites, with jig's example guards and attack stories, as it
  keeps it out of its logs (`../../protocol/browser.md`); its own pages
  and bundle, which jig never serves (`../../protocol/service.md`).

## 11. Open questions

- **Bundling parts:** whether a jig read's answer should carry the
  application's parts it names, put together by the engine's root, to
  save a client the second round trip.
- **Windows of a large tree:** how a tree watch's window follows what a
  person opened, so that a tree of hundreds of tasks costs what is shown.
- **Who may see what** among a project's parties
  (`../../domain/people.md`, section 12).
- **Search** over tasks, results, notes and transcripts, which the engine
  has only over notes' descriptions.
- **A document past its bounds:** what a read or a watch is answered
  when the engine cannot make its document within the client's bounds:
  bounds derived so that it cannot happen, as a path's segments are, or
  an answer of its own. Until it is settled, a watch's is refused as
  unknown and a delivery's ends its watch (7.2).
