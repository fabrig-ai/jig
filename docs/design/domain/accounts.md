# LLM accounts

Provisional, 2026-10-19, revised 2026-10-24. How an application on jig pays for its runs'
LLMs: who holds an account, the sources by which a run may use it, the
states the accounts child keeps of each, whose work it pays for, which
account each run gets, what people do with their accounts, and how an
account moves. It applies providers.md, the LLM providers' terms. The
model a task runs on is the task's (tasks.md, section 3), within the
models its authority names (authority.md, section 3); the accounts
child's place in the engine is engine.md, section 12; where records and
keys live is `../protocol/store.md`, section 9, and how the protocol
layer signs in, refreshes and lends is `../protocol/llm.md`, sections 5
and 6. Running a program, such as Claude Code, as the agent's backend is
smith's (smith's `protocol/programs.md`). What is still open is listed
in section 7.

## 1. In one page

- **Each application is its own client, with its own state.** It signs
  in under its own name, keeps its records in its own store, and
  refreshes them itself. No application reads another program's login
  or shares a file with one: not Codex's, not a person's `~/.claude`,
  not another application's (section 2).
- **An account is a holder, an LLM provider, its sources and its
  uses.** The holder is a person or the deployment; the sources, in the
  holder's order, say how a run may use the account; the uses say whose
  work it pays for (section 3).
- **A source is how a run is authorized:** *signed in*, with the
  application's own registration; a *key*; a *program*'s own login on a
  host, such as Claude Code's (`claude_code`); or *none*, for a local
  model. A new LLM provider is configuration, and a dialect or a
  program's adapter where it needs one, never a change to the domain
  (3.4).
- **A person may hold several accounts with one LLM provider,** each its
  own account, told apart by the provider's subject, never by an email;
  a subject has one holder in a deployment (3.3).
- **Each source has a state:** usable, limited, refused, needs sign-in,
  misconfigured or unloaded. The accounts child keeps them, from what
  the accounts' component and the runs' agents tell it, and starts no
  run on a source that is not usable (3.5).
- **One person, no friction.** A plan pays, by default, for its holder's
  own work and for the automatic work of each project in which its
  holder is the only party, so a person alone on a deployment signs in
  once and every run uses their subscription (section 4).
- **Who pays is decided with the run's check** and recorded in its
  claim: the account that serves the work's payer, and its first source
  usable on a host with room. A task keeps its account from run to run,
  chooses again when its payer changes, and waits for it at a limit,
  never moving to another (4.3).
- **A host that keeps a person's program logins takes only runs that
  person's accounts pay for,** until a run's process tree is contained
  from them, since a run there can read a login and a subscription
  serves one person. A deployment of one person is unaffected (3.4).
- **Values are never committed.** Records and keys are in the secrets'
  store, kept before they are lent; a request carrying a key names it by
  number once the protocol layer has kept it. The engine alone
  refreshes, so no two processes race a rotating token (section 5;
  `../protocol/store.md`, section 9).
- **Moving an account:** a signed-in source, between deployments of one
  application, never copied: out into a private file, then erased; in
  from it, refreshed once before it is lent, so that a copy left behind
  is dead (section 6).

## 2. Each application is its own client

- **Its own registration.** An application signs a person in under its
  own name: Sign in with ChatGPT's `agent_name_hint` is the
  application's name, used consistently (providers.md, 2.2). The
  installation's identifier that OpenAI asks for, `ext_agent_host_id`,
  is drawn once per deployment, at its first start, as `urn:uuid:…`,
  and kept in its store; it is never a person's identifier.
- **Its own records, refreshed by itself.** A sign-in's record and a
  key are kept in the application's store (`../protocol/store.md`,
  section 9), and its engine's accounts' component alone refreshes a
  record (skein's `oauth.md`, section 6), so refreshes of one record are
  serialized by construction (providers.md, 2.3).
- **No borrowed login.** No application reads another program's login
  file, a Codex login or another application's records. A program the
  agent runs (3.4, *program*) keeps its own login in a configuration
  directory of the application's own, one per source and per host
  (smith's `protocol/programs.md`, section 4), never the person's: a
  person's own Claude Code settings, hooks and plugins never load into a
  run.
- **smith's own product does likewise:** it signs in under its own name,
  keeps its records in its own state, and borrows nothing (smith's
  `shell.md`, section 7).

## 3. Accounts

### 3.1 What an account is

```
Account
  number      the domain's name for it, never reused
  provider    an LLM provider of the configuration's, by number
  holder      a person, or the deployment
  subject     the LLM provider's stable identifier for the account, where
              a signed-in source gives one; beside it, a display name
              its holder gives
  sources     in the holder's order: each a kind and a state, and for a
              program's, its name and its login on each host (3.4)
  uses        the payers it pays for (4.2)
```

- **An LLM provider** is configuration: its number, its name for
  showing, its endpoints and models, the sources it offers and what each
  needs (a sign-in's registration, a program), and the facts its terms
  ask an application to show, such as a manage-usage address
  (providers.md, 2.6; `../protocol/configuration.md`, section 3). jig's
  code names no LLM provider. It is not an identity provider
  (people.md, section 3), and the two are numbered apart.
- **Accounts are made at runtime,** by a person's sign-in, a key added
  or a program's source declared (section 5), and kept in the store
  (engine.md, 5.4). An LLM provider whose source is *none* has one
  account, the deployment's, made at the first start that configures
  it. An application's configuration names LLM providers, never
  accounts, and an endpoint names no account. On restart the accounts
  child restores those durable rows and names every signed-in or key
  source for the component, including deployment-held sources. The source
  sweep retains exactly those named secrets, leaving application-carried
  values to their own owner.
- **The domain sees names and states,** never a value: an account by
  number, a source by its account and its place, a program's also by
  its name, and a credential by its source and a generation, with how
  long it is valid (3.5).
- **The subject** is the LLM provider's validated identifier (an ID
  token's `sub`), never an email. It is the account's own, and no
  person's subject in people.md, section 3: an LLM provider is no
  identity provider, and a subject here opens no session. A key or a
  program's login gives none:
  an account with no signed-in source has no subject, and is told apart
  by its number and the display name its holder gives it. jig checks no
  subject for a program's source, since the program names no account it
  is signed in to (smith's `protocol/programs.md`, section 4).

### 3.2 Holders

- **Only a person holds a plan:** a subscription a person pays for, such
  as ChatGPT's or Claude's, used through a signed-in source or a
  program's login. A subscription serves one person (providers.md,
  section 1). An account with a plan among its sources is a *plan's
  account*.
- **A person or the deployment holds a key:** a person's own, or the
  deployment's, billed to the organization that owns it (providers.md,
  2.7 and 3.1). The deployment holds a *none* provider's account.
- **A service holds no account.** A plan's uses may name one (4.2), and
  a key's may.
- **A holder decides about their account:** the order of its sources
  and its place among their accounts, its uses, signing it out,
  removing it, and moving it (section 6). For the deployment's
  accounts, its owners decide: the first owners its configuration names (people.md,
  section 3; `../protocol/configuration.md`, section 3), and, in their
  name, a connector the configuration grants the deployment's `AddKey`
  or `RemoveAccount` (people.md, 5.3), where no client is there to ask.

### 3.3 Several accounts with one LLM provider

- **Each its own account.** A person may hold several subscriptions
  with one LLM provider, under several emails, and each sign-in to a new
  subject makes a new account. A sign-in to a subject the person already
  holds renews that account's record; where the LLM provider issues the
  renewal another client id, the replaced refresh token is revoked.
- **Told apart by the subject,** never the email: two sign-ins stay
  apart even when their emails match (providers.md, 2.2).
- **A subject has one holder.** A sign-in whose subject another holder
  holds is refused, naming the LLM provider's rule, and its new record is
  revoked: two holders would share one subscription, and race one
  rotating token.
- **A program's logins** are one configuration directory per source on
  each host, named by the source's name (3.4), each signed in on its
  own, with its own email, so one host may carry several Claude Code
  logins of one person.

### 3.4 Sources

| Kind | What the application holds | Who runs the agent's loop | Usable where | Renewed by |
|---|---|---|---|---|
| *signed in* (`sign_in`) | a record from its own OAuth registration, in the secrets' store | smith, over skein's LLM client: the direct backend | on any host: its access token goes with the run as a grant | the accounts' component, before it lapses |
| *key* (`key`) | a key, in the secrets' store | smith, the direct backend | on any host, the same way | nobody: its holder replaces it |
| *program* (`program`) | nothing: a program's own login, in the application's configuration directory for the source on each host | the program, adapted by smith's agent: the program backend (smith's `protocol/programs.md`, section 2) | on a worker whose host's half reports its login usable, and that the host rule (below) allows | the program |
| *none* (`none`) | nothing | smith, the direct backend | where its endpoint is reachable, such as a local model on loopback | — |

- **Later kinds fit the same table,** such as an exchange of a cloud
  provider's credentials for short-lived tokens.
- **The holder orders an account's sources.** A run takes the first
  that is usable on the host it is placed on, and passes to the next
  only when the earlier one is unusable there alone: a program's login
  needing sign-in on that host, or kept on another. A source unusable on
  every host (limited, refused, needs sign-in, misconfigured, unloaded:
  3.5) holds the account's runs until its holder acts on it, resumes it
  or reorders the sources, since the order decides who pays
  (providers.md, 2.5 and 3.4). An Anthropic account lists Claude Code,
  then a key, by default.
- **A grant names its source.** A value's grant carries its value and
  how it is sent, a bearer or a key (`../protocol/llm.md`, section 6). A
  program's grant carries none: it names the source, by whose name the
  host's half finds the program's configuration directory, and the agent
  checks the login again as the run starts (smith's
  `protocol/programs.md`, section 3).
- **A program is named** in the LLM provider's configuration, and in the
  agent's `programs`: `claude_code`, Claude Code, whose executable is
  `claude` (smith's `protocol/agent.md`, section 4).
- **A program's source on each host.** A worker's logins follow its
  hello: the engine sends it the program sources it may be given, by
  name, after the hello (`../protocol/channel.md`, 4.3), and places no
  run on it until the first `logins` that answers them. The host's half
  then checks each login when told of a new source, on its holder's
  check (section 5) and at a configured interval
  (`../protocol/configuration.md`, section 4), and reports what changed.
  A check answers usable, not signed in, signed in otherwise (another
  way of paying than the holder's plan) or failed (smith's
  `protocol/programs.md`, section 3), and the deployment's document
  shows each host's answer. A holder's check withdraws the source's grant;
  a source held Refused is not granted again while so held (Resume clears
  only Limited holds). Withdrawal records no fact: facts are expendable. Every notice names the worker's whole current
  source list, without login contents; the fleet places a run on a
  program's source only where its login is usable (engine.md, section 8). The engine's
  own slots run `smith-inline-agent`, which has no program backend, so
  they never take a program's source.
- **A program source's name** is 1 to 32 characters of `a-z`, `0-9` and
  `-`, given by its holder as they declare it, unique in the deployment,
  never changed and never reused once removed. A durable reservation keeps
  its source number, name and original holder after removal. Both shipped
  profiles admit 12 lifetime names. Restoring an existing live source reserves
  its name before admitting changes; a conflicting reservation is refused.
  Exhaustion refuses a declaration before
  reserving its numbers or writing any row. It, not the account's
  number, names the configuration directory, so that a directory a
  person signed in on a host keeps meaning that source, and a
  developer's own fixed directory serves each fresh deployment a live
  layer's test makes.
- **The host rule.** A host keeps a person's program logins when its
  half reports a login for a program source of theirs, in any state but
  needs sign-in. Runs, and a program's own tools, run as the host's
  user, so a run can read every login kept on its host until a run's
  process tree is contained from the programs directory (hosts.md, 5.1;
  smith's `domain/tools.md`, sections 1 and 5), and a subscription
  serves one person. So a host that keeps a person's program logins
  takes only runs that person's accounts pay for, until a run's tree is
  contained from them: the fleet places no other run there (engine.md,
  section 8), and the deployment's document says why. One person's
  several logins on one host change nothing, and a deployment of one
  person is unaffected. Deployment-held logins restrict no run; only runs
  bound to their account can use their source.
- **Signing out or removing a program's source** has each worker run the
  program's own sign-out in the source's configuration directory (smith's
  `protocol/hosts.md`, section 2); the answer names any host that did
  not. The answer covers the configured workers, named by their configured
  names: one already offline is not signed out (not in contact), and a channel
  lost during the operation is lost. A worker removed from configuration is
  not reportable and cannot reconnect or run work; its operator clears its
  machine's logins. When removing an account with several program sources,
  its one row per worker keeps the most severe result, independent of arrival
  order: not in contact, then failed, then lost. Lost means the admitted
  operation ended without a worker terminal.
  The issued-name record keeps desired sign-out, independently of observed
  login state. A reconnect or engine restart checks all issued names, including
  retired names; a retained login requiring cleanup is signed out before that
  worker becomes ready for its holder's runs. Cleanup has its own result and
  never changes or retries the original answer. A failed cleanup keeps the host
  restriction until absence is established; a holder's explicit check of a
  live declaration permits a fresh sign-in.
- **Concurrency is the host's slots:** a run on a program is a smith
  run, and counts as one run, as any other does.

### 3.5 States

A source's state is one of:

- **usable:** a signed-in source while the accounts child holds a
  granted generation still valid; a key while it is kept; a program's on
  each host whose half reports its login usable; a *none* source
  always; each unless one of the states below holds;
- **limited:** the LLM provider says it is spent, with when it resets
  where the provider says, and never a guess (providers.md, 2.5). No new
  run starts on it until then, or, with no time given, until its holder
  resumes it, replaces its LLM key or signs in again. An absent reset remains
  absent across new grants and restarts; it is never converted to zero or
  a guessed retry deadline. An explicit zero is a supplied reset. This
  retires ruling 130's absent-reset-to-zero mapping. Text a person reads
  says "limited";
- **refused:** the LLM provider refuses the account for what is asked:
  not eligible, a scope not granted, plan usage declined at sign-in, a
  key it does not take. It stays so until its holder acts: signs in
  again, with the scope, replaces the key, or chooses another source;
- **needs sign-in:** no record, or one revoked or past refreshing; for
  a program, on which host;
- **misconfigured:** the LLM provider rejected the application's client;
- **unloaded:** its record could not be read, saying where and why;
- **moving:** a signed-in source whose record is out with a move, or
  back from one and not yet refreshed (section 6). No run takes it, and
  nothing of the record's generation is lent.

Each source's mechanism maps its LLM provider's failures onto these,
skein for what a provider's API answers (skein's `llm.md`, section 2.5)
and smith for what a program reports (smith's `protocol/programs.md`,
section 5). A failure a run meets is the run's: unavailable is retried
by smith's session; unsupported, a field the LLM provider will not
take, is the request's error, never sent again unchanged (providers.md,
2.5).

How the accounts child keeps them, from what the accounts' component
says (`../protocol/llm.md`, section 5) and what runs' agents notice
(hosts.md, section 2):

- **Every signed-in source is wanted,** once the restart has restored
  the core's records, its accounts' among them (engine.md, section 6,
  item 1), to the close, so the component refreshes each before its
  access token lapses whether or not a run holds it, and answers each
  new generation, granted; an idle account keeps its
  refresh token (providers.md, 2.3; skein's `oauth.md`, section 6.3). A
  key is granted once it is kept, a replaced key as its next generation.
- **Every live attempt hears each new generation** of its source, on
  whatever host, the engine's own included; and the fleet sends each
  attempt it keeps its current grant when its worker says hello, and
  when it adopts the attempt (engine.md, section 8).
- **Unauthorized:** a run's LLM provider refused a generation. For a
  signed-in source the child passes it to the component, which refreshes
  once; a second unauthorized of the same generation fails it, and the
  source needs sign-in. A key refused so is refused. For a program the
  host's half checks the login again, and the source needs sign-in on
  that host when it fails.
- **Limited:** a run's agent says its source is spent (skein's
  `exhausted`, or the program's usage limit), with how long until it
  resets where it gives that; the core makes it a time on its clock as
  it hears the notice. The child marks the source limited until then,
  or until its holder resumes it, and asks nothing of the component.
- **Refused:** a run's agent says the LLM provider refused the account
  for what it asked, or a sign-in's answer does (section 5). The child
  marks the source refused, and asks nothing of the component.
- **Failed,** in one of five classes the component gives:
  - **later,** with how long: not a state. The source keeps its state,
    the child asks again after it, while the source is still wanted, and
    a run that needs a valid generation waits for the next one, never
    passing to the next source;
  - **sign in again:** its grant was revoked, or it has no record. The
    source needs sign-in, the deployment's document says its holder must
    sign it in, and a new sign-in (section 5) makes it usable again;
  - **misconfigured:** the LLM provider rejected the application's
    client. The source is misconfigured until a restart with its
    configuration fixed, and the document says so;
  - **unloaded:** its record could not be read. The source is unloaded,
    the document says where and why, and the child asks again at its
    retry interval;
  - **cancelled,** at the close.
- **Close** releases every source, and ends once the component has
  closed.
- **Deadlines belong to their source.** At creation of a limit or a
  retry, the accounts child retains `Deadline { at: Time, wall: Wall }`
  from that step's injected `env.now` and `env.wall`. Its `Limited` and
  `Later` state and spent-until facts expose that pair to the core.
  Timers use `at`; every task wait, account and deployment document,
  and stored reset uses the retained `wall`, exactly. Later clock
  samples and wall corrections do not change an existing deadline.
  The existing event and step environment suffice; no new `Event` is
  needed.
- **Limit merging keeps the same precedence.** A later reset replaces
  both `at` and `wall`; an earlier reset keeps the old pair, and a tie
  keeps the old pair too. A limit with no reset overrides a timed
  limit to `None`; an indefinite limit stays indefinite until resumed.
  No wall from a losing reset replaces the winner's wall.
- **What the store keeps** of each source is its kind, its name, its
  hold (limited, refused or moving), and a limit's reset as a wall
  time (engine.md, 5.4): a restart preserves that saved wall exactly,
  maps it once to a fresh monotonic
  `at`, lifts a limit whose reset passed while the engine was down,
  and holds a later one until it. The existing optional wall reset
  represents both timed and indefinite limits; no store or wire layout
  changes. A source held moving is stored so, and a restart brings it
  back as a cancelled move does (section 6). A program's login on a
  worker is not stored: its
  half reports it as `logins`, which follow the worker's hello
  (`../protocol/channel.md`, 4.3).

### 3.6 Bounds

Accounts in the deployment, accounts per holder, sources per account,
sign-ins at once, lifetime program source names (retired names included),
and program sources per worker are limits
(`../protocol/configuration.md`, section 6), within which the accounts'
records, skein's driver, the hub's grants, a worker's hello and the
secrets' store are sized. A request that would pass one is refused,
naming it. A move out or in holds one of the sign-ins at once from its
ask to its end (section 6), as a sign-in and a key being kept do, so
that the child's tables for them share that bound. Client receiving
counts for accounts, sources and sign-ins
are at least the engine's document lines, so every list cut there fits
(`../client/domain/client.md`, section 14).

## 4. Uses: whose work an account pays for

### 4.1 The payer

A run's task has one funder, and the chain of funders ends at a party's
pool or a project's period (authority.md, section 7). That end is the
run's **payer**: that pool's party, or that period's project. A recurring
allotment pays as its project. Financial links decide it, independently of
the requester and delegation tree; a Move or refunding changes it for every
descendant funded through the changed task. Prospective checks follow
staged financial links before live ones, within the tasks child's bounds,
and refuse unknown or cyclic task chains without writing anything.

### 4.2 Uses

- **A plan's account, by default,** pays for its holder's own work and
  for the automatic work of each project in which its holder is the only
  party with a role. A person alone on a deployment is the only party of
  every project, so everything runs on their subscription, with nothing
  to set.
- **A plan's account's uses may name more,** in its holder's word: a
  service, as an agent of theirs that connects with its own identity
  does (people.md, section 3), whose work then runs on its holder's
  subscription; or less than the default. A person other than its
  holder, and a project with another party, are refused in a plan's
  account's uses, naming the LLM provider's rule (providers.md, 2.6 and
  3.4).
- **A key's account's uses** are its holder's own work by default, a
  person's; the deployment's key pays for every payer. A key's holder
  may name any party or project in its uses: a key is the customer's to
  provision for its own authorized users, billed to the key's owner
  (providers.md, 3.1). A key that should pay for more than a plan's
  account may is an account of its own.
- **A shared project's automatic work** is so paid by a key: the
  deployment's, or a person's whose uses name the project. No plan pays
  for it (above).
- **An agent that brings its own LLM** pays for its own completions;
  jig's accounts are not involved.

### 4.3 Choosing the account

Selection uses runtime accounts and their kept source identities (3.1).
Configuration supplies provider membership and model choices; it gives no
account a preferred number or an endpoint default.

- **With the run's check.** As the core checks that a task's run may
  start (authority.md, 8.3; engine.md, 7.1, step 1), it takes, for its
  main model's LLM provider, the accounts whose uses cover the payer:
  the account its task keeps, or its creator named, if it has one; else
  the payer's own, in their order, then those that name it, then the
  deployment's. It takes the first with a source usable (3.4) on a host
  with a free slot of a kind the charter allows, and that the host rule
  allows: a host that keeps a person's program logins takes only runs
  that person's accounts pay for (3.4). The claim's commit records the
  account and the source (engine.md, 7.1, step 3), and the run is placed
  there.
- **The task keeps it,** from run to run, so a conversation stays on one
  account's cache and replay. Its creator names an account when it is
  created or amended (tasks.md, section 3) only to want one in
  particular, within the account's uses.
- **A change of payer chooses again.** When a commit changes a task's
  payer (a move to another requester, a widening or an acceptance
  funded by another party: tasks.md, section 7; authority.md, section
  7) or its main model's LLM provider, its next run's check takes the
  kept account only if its uses still cover the payer and its LLM
  provider is still the main model's, and chooses again otherwise.
- **A task whose account is unusable waits** for it, saying why
  (authority.md, 8.3). A limited task never moves to another of its
  holder's accounts: it waits for its own, as its LLM provider's limit
  asks, and a choice made again takes no other account of that holder
  and LLM provider while the kept one is limited. An amendment naming
  another account of that LLM provider is refused while its account is
  limited (providers.md, 2.5 and 3.4).
- **Its sub-agents,** on the direct backend, use the run's account for
  its main model's LLM provider, and for each other LLM provider a
  sub-agent's model uses, one account taken the same way at the run's
  check, which the claim records too. A sub-agent's model whose LLM
  provider has no usable account serving the payer is left out of that
  run's models (smith's `domain/run.md`, 5.3). A run on a program
  backend has no sub-agents: the program's own are withheld and smith's
  are not served (smith's `protocol/programs.md`, section 2).
- **Replay stays with its account.** A provider's opaque replay
  (reasoning, signatures and item ids) is kept only for the account that
  made it: a task whose account changes resumes its conversation carried
  over as for another LLM provider (smith's `protocol/transcript.md`,
  section 4).

The task's creator preference and the account kept by a successful claim are
separate. Clearing the former retains the latter. Current payer, membership,
uses and provider facts are joined at every check; a context retained during
preparation is not authority for a later claim. Account/source waits retain the
source's typed reason and optional reset, separately from host and application
credential waits. With no reset, no polling deadline is invented.

Recurring templates check explicit hints against the project's payer at start,
regardless of their members' declared funders, and again before each occurrence.
There is no per-occurrence refusal outcome yet. If an account has disappeared,
changed provider or ceased to serve the project, the occurrence is created and
its next run waits, naming the account and its typed admission reason in the
Task document. This exceptional wait is rechecked from current facts; it does
not silently choose a different account. A later occurrence-refusal outcome
will replace this fallback.

The grant number passed as smith's account is the selected **source** number.
Changing sources within one billing account therefore conservatively causes
smith's carry-over too: it preserves plain history, discards opaque replay and
compacts on the new LLM before opening its new window (smith's
`protocol/transcript.md`, section 4). Jig keeps the committed conversation and
passes the actual new grant. A separate stable billing-account replay identity
is a later decision for smith.

## 5. What people do

Each is a party's request through the client (people.md, 5.1), checked
against the account's holder (3.2) and decided by the accounts child,
or, for the deployment's key added and account removed, the
deployment's act a connector makes in its owners' name (people.md, 5.3);
each answer and each state is a fact the application's pages show
(`../client/domain/contract.md`, section 5), which a party watches
among their own (`../client/domain/contract.md`, 7.1).

| Request | What it does |
|---|---|
| sign in | starts an LLM provider's sign-in, cancelling first the party's own still waiting with that provider, which ends *cancelled*, so that starting again gets a port the provider fixes (`../protocol/llm.md`, 5.4); the visit goes to the client that asked, and the redirect to a loopback listener the engine binds for this sign-in; makes an account, renews one of the same subject, or is refused for a subject another holder holds (3.3); refused before it starts for the deployment where the provider's sign-in brings a plan, since only a person holds one (3.2) |
| finish a sign-in | the address the sign-in's browser was sent to, which its person pasted, for a browser on another machine than the engine's: carried as a key is, and handed to the sign-in as its redirect (`../protocol/llm.md`, 5.4); only from the party who started that sign-in, while it waits; answered *finishing* once the sign-in takes it, the sign-in then ending as any does; refused, the sign-in still waiting, as another address, another sign-in's state, or *too many* once it has refused a few for their state; refused *not waiting* when no sign-in of the party's under that number waits, whoever's it is, once it has taken a redirect, ended, or the engine restarted, and while another finish of it is out; a sign-in that ends with a finish out answers it *not waiting* before its end |
| sign out | revokes the record at the LLM provider, or has each worker sign a program's login out, and removes the source; when revocation is not confirmed, the answer says to disconnect the application in the provider's settings (providers.md, 2.3) |
| add a key, replace it, remove it | adding names the account it joins, a new key's account by default; a person's, or the deployment's by its owners, or added by the deployment's `AddKey` (people.md, 5.3) |
| declare a program's login | makes the account, or adds the source to one, under the name its holder gives (3.4); each host then reports whether its login is usable, and the answer names the application's command to run there, which hands the terminal to the program's own sign-in (smith's `protocol/programs.md`, section 4) |
| check a program's login | has each host's half check the source's login now (3.4) |
| order, uses, resume | the holder's: an account's place among theirs and its sources' order, its uses, a limited source resumed |
| remove an account | signs each of its sources out, as sign out does, and erases its keys; its number is never reused, and a task that kept it chooses again (4.3); the deployment's also by the deployment's `RemoveAccount` (people.md, 5.3) |
| move out | withdraws the account's signed-in source and hands its record to the client that asked, once, never in a committed answer, naming the move; sent once, never kept by the client (`../client/domain/contract.md`, 3.1); refused "not signed in" without a signed-in source holding a record (section 6) |
| written | the client wrote the moved record: erases the source, and the account with it when no source is left; refused "cancelled" for a move not under way |
| move in | takes a moved record carried as a key is: refused "not ours" for another application's, "held" for a subject another holder holds; makes the account or renews the holder's of its subject, refreshing the record once before it is usable (section 6) |

- **A value is never committed.** A key, or a moved record, is
  taken out by the clients' protocol layer as it decodes the request,
  kept in the secrets' store first, and handed to the core by a drawn
  number and its digest. An LLM key, or a moved record, is then adopted
  into its source's secret row before it is granted (`../protocol/store.md`, section 9).
  Requests and their answers are committed and kept (engine.md, 5.4);
  a value is not. A client sends a request carrying a value once, and
  never keeps nor resends it
  (`../protocol/client.md`, sections 5 and 10). A moved-out record goes
  the other way once, with the answer to the client that asked, and is
  never in the answer kept (section 6): the request is sent once, and a
  second answer under its key, without the record, is *lost* to the
  client.
- **The sign-in's redirect is on the engine's machine.** The engine
  binds a loopback listener on an ephemeral port for each sign-in, or
  on the port its configuration fixes (`../protocol/llm.md`, section 5),
  as an LLM provider that requires a loopback redirect allows
  (providers.md, 2.2). A person whose browser is on another computer
  finishes the sign-in with the address that browser was sent to
  (above), or signs in on a computer of their own whose record a move
  then brings in (section 6).
- **A pasted address is a value too,** held, never kept: in memory,
  for as long as its request takes, and erased whatever its answer
  (`../protocol/store.md`, section 9). It holds the sign-in's state and
  its code, but no token or verifier: the code is spent once, and only
  with the verifier the engine holds. Only its starter may hand it
  in, since a state or a sign-in's number alone grants nothing.
- **Plan usage declined** keeps the sign-in and marks the source
  refused, and the answer offers its two ways on: sign in again with
  the scope, or another source (providers.md, 2.2).
- **The terms' disclosures are facts:** whether an account is a plan's,
  its LLM provider's name and manage-usage address, the first use of a
  plan, so that the application can show them where its pages need them
  (providers.md, 2.6). The words and screens are the application's.

## 6. Moving an account

A signed-in source moves between deployments of one application, each
side by its holder's request: out of one deployment into a private
file, and from the file into another. A rotating refresh token is valid
in one place (providers.md, 2.3), so the move keeps one holder by
construction, never by a person deleting a copy.

- **Signed-in sources only.** A key is added again; a program's login is
  per host. Moving out an account with no signed-in source, or whose
  signed-in source holds no record that reads (it needs sign-in, or is
  unloaded), is refused "not signed in".
- **Out, in two steps.** Its holder asks to move the account out. The
  child takes the move's number from its counter, and withdraws the
  signed-in source: it releases its grant, and the component takes it
  out of the driver, so that no run takes it and no refresh follows; an
  exchange already sent finishes and keeps its record first, and live
  runs keep their access token until it lapses. The source is held
  *moving*, stored. Once the protocol layer has read the source's
  latest record and made the file's bytes (`../protocol/llm.md`, 5.6),
  the request is answered *moving out*, naming the move's number; the
  bytes go to the client that asked with that answer, once, never in a
  committed one: an answer lost on its way, or the request's key sent
  again, leaves the move with no file, and its deadline cancels it.
  The client writes the file new and private, and says
  *written*, naming the move: the child then erases the source and its
  record, and the account with them when it has no other source; its
  number is never reused. The file is then the only holder.
- **Cancelled.** A move with no *written* within the move's deadline (a
  limit, `../protocol/configuration.md`, section 6), or under way at a
  restart, is cancelled: the source comes back to the driver with its
  kept record, which is **refreshed once before it is lent again**, so
  that a file written meanwhile holds a dead refresh token. The source
  stays held moving until its first new generation is granted, so a
  restart before then refreshes it again; a refresh refused as sign in
  again leaves it needing sign-in. A *written* of a move that is not
  under way is refused "cancelled", and the client removes the file it
  wrote. A move whose record cannot be read is refused "not signed in",
  and its source comes back the same way.
- **In.** Its holder sends the file's bytes as a carried value, taken
  into the secrets' store as a key is (section 5). The protocol layer
  reads them and checks that they are this application's: its name, and
  the issuer and client id of a configured LLM provider's registration;
  otherwise the request is refused "not ours". The child then decides
  as for a sign-in (3.2, 3.3): it refuses the deployment where the LLM
  provider's sign-in brings a plan, and a subject another holder holds
  ("held"), else makes the account, or renews the holder's account of
  that subject. A move in counts as a sign-in at once (3.6). The record
  is adopted into the new source's row, the carried value erased, and
  the source added to the driver, held moving: **refreshed once before
  it is usable**, so that a file left behind is dead at once. The
  request is answered once the record is durable, naming the account;
  the client then removes the file.
- **While a source moves,** its holder may order it and set its
  account's uses, but its sign-out, its account's removal, another move
  out, and a renewal of its account by a sign-in or a move in are
  refused "moving", until the move is written or cancelled.
- **One holder at a time.** A rotating refresh token is valid in one
  place: a copy that refreshes signs the other out (providers.md, 2.3),
  so a record is moved, never copied.
- **It serves three needs:** a host with no browser, signed in on a
  computer with one (providers.md, 2.2); a deployment moved to another
  machine; and unattended tests of the live layer on a signed-in
  subscription, which move one account in and out of each run's fresh
  deployment. A test on a program's login finds it signed in on its
  host (3.4).

## 7. Open questions

- **Replay across accounts:** whether a provider's opaque replay
  (encrypted reasoning, thinking signatures) is accepted from another
  account of the same LLM provider. Default: dropped (4.3), until a
  capture shows it is accepted.
- **A redirect relayed by a client,** for an engine on another machine:
  the native client binds the loopback listener and hands the code to
  the engine. A remotely hosted application asks OpenAI first
  (providers.md, 2.1). Default: the engine's machine (section 5).
- **Who decides for the deployment's accounts** beyond its first owners:
  a deployment-wide role, which jig's roles, all of them a project's,
  do not give. Default: the first owners (3.2).
- **Program logins on a shared host,** once a run's process tree is
  contained from the programs directory: whether the rule of 3.4 then
  goes, or stays for a program's own tools. Default: the rule holds.
