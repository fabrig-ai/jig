# The reusable client world

`jig-client-world` is an ordinary Rust test kit for an application's client
root and view. Joint and application worlds depend on it and implement the
public `App` and `Front<A>` traits in `src/harness/mod.rs`; they reuse `World`,
its shell, independent model tree, fake people and referee.

`App` adapts the root's own events, requests, records and addresses. Its
`duty` is the shell boundary: save/erase, wire, show, sign-in or fact. A failed
store calls `unsent` for that step's sends. `record_key` identifies records;
`patch_limit` and `worst_case` provide the owner's bounds. `seen`,
`sent`, `received` and `card` translate public trees, wire requests and
callback values into the referee's vocabulary, without reading root state.

`window_changed` decodes the person's own filter, fold, read-more and
navigation acts (QUESTIONS 34). It defaults to false; applications with those
acts implement it. `WindowChanged` and `LeftPage` clear only that client's
prior card visibility checks. Request, durable-write, words, revision and
link obligations remain. `Seen::HiddenPage` explains a whole privacy,
sign-in, missing or incompatible-version page and must contain no cards.
`Receipt::Parked` suspends request and online expectations while sign-in or
version compatibility prevents work; a confirmed `Receipt::Party` resumes
online expectations when the front is reachable. Network loss must still
produce the visible offline status.

`Front` owns transport tokens, callbacks, terminal accounting and the route
changes made by `bind`, `reload`, `received` and `sign_in`. `observed` drains
only new durable commits, made keys, kept/retained words, stopped objects,
ended tasks and reachability changes. The joint world supplies these from
its engine/store; the scripted adapter supplies them from its durable
ledger. Include task endings before retention changes from the same atomic
commit. Pending deliveries held while offline must advance the clock to
recovery, and sign-ins during a restart must survive until recovery.

`World::client`, `tab`, `person` and `random_person` admit devices, copied-store
tabs and people. `go`, `back`, `forward`, `reload`, `act`, `event`, `fault`,
`fault_at`, `sign_as` and `fail_store` expose the shell's boundary operations.
`run` yields the referee verdict, judged checks, virtual time, callbacks,
faults, dropped old-epoch callbacks, trace and heap evidence. Configure
`Settings` at construction; story fixtures may call `randomness` and
`measure_heap` before admitting clients. Random people wait between actions
using the configured latency, so they act on delivered pages. Heap-counted
binaries install `skein_world::domain::heap::Counting` as their allocator.

`stories` exposes the bounded truth fixtures; `random` replays six starting
states and random whole task/family documents. The focused binaries retain
all scripted stories, fourteen separate faults, both incident variants,
negative referee cases, shell/store and memory cases. `fuzzy_client` keeps
six independent 16-seed story sweeps and one 32-seed random-truth sweep.

The test application's root holds `jig_client::Client`, passes its transport
requests out in order, and routes tells by asker. The reference UI's bridge
counts kept requests per party; until jig 21.4, foreign work leaves when that
party signs in. The client keeps only its own table records; the UI and family
keep their records separately under their own keys.

The scripted test family carries its complete requests, subjects, documents
and answers as bytes through its wire crate. Admission refusals stay on jig's
entrance wire and reach the original family child as the exact refusal.
