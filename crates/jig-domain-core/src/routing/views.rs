//! Watch admission and construction in the core's vocabulary.
//! `client/domain/contract.md`, sections 3.2 and 7. Documents are made when
//! views asks and moved into held outputs; no document remains in the core.
//! The child owns pacing and provisional text, and knows no concrete subject.
//!
//! | Event | State change | Emits |
//! | --- | --- | --- |
//! | Watch | retain authenticated construction inputs | first held delivery or refusal |
//! | Delivered | child clears its in-flight mark | next current held delivery if changed |
//! | Unwatch | child closes after outstanding terminals | held watch ending |

use crate::documents::{self, Missing, Subject, Watched, Window};
use crate::{Core, Held, Limits, Now, Request, Requests, WatchBinding, WatchRefusal, WatchedSubject};
use alloc::boxed::Box;
use jig_domain_views as views;
use skein_lib::{Env, List, Map, Queue, Token};

pub(super) fn route_view(core: &mut Core, env: &Env<Limits>, event: views::Event) -> Requests {
    let mut out = Queue::uncapped();
    views::step(&mut core.views, &Env { now: env.now, wall: env.wall, limits: env.limits.views }, event, &mut out);
    made_outputs(core, env, out)
}
pub(super) use super::watch_marks::{touch_number, touch_people_event, touch_people_row, touch_task};

// Construct and move each document into the decision that asked for it.
pub(super) fn made_outputs(core: &mut Core, env: &Env<Limits>, mut child: Queue<views::Request>) -> Requests {
    let mut ready = Queue::uncapped();
    let mut documents = Map::with_capacity(env.limits.views.watchers);
    let mut out = Queue::uncapped();
    for _ in 0..child.len() {
        match child.pop().expect("view output count") {
            views::Request::Make { watcher, .. } => {
                if let Some((connector, subject, scope, party, window)) = application_subject(core, watcher) {
                    out.push(Request::Ask {
                        connector,
                        ask: crate::Ask::Document { watcher, scope, subject, party, window },
                    });
                    continue;
                }
                let outcome = match watch_document(core, &env.limits, watcher, env.now, env.wall) {
                    Ok(document) => {
                        let previous = documents.insert(watcher, document).expect("bounded construction scratch");
                        assert!(previous.is_none(), "one construction per watcher");
                        views::Made::Ready
                    }
                    Err(views::End::Gone) if retained_task(core, watcher).is_some() => {
                        append_stream(crate::reads::start_watch(core, env, watcher), &mut out);
                        continue;
                    }
                    Err(why) => views::Made::Gone { why },
                };
                views::step(
                    &mut core.views,
                    &Env { now: env.now, wall: env.wall, limits: env.limits.views },
                    views::Event::Made { watcher, outcome },
                    &mut ready,
                );
            }
            request @ (views::Request::Watching { .. }
            | views::Request::Refused { .. }
            | views::Request::Stream { .. }
            | views::Request::Deliver { .. }
            | views::Request::Ended { .. }) => ready.push(request),
        }
    }
    for _ in 0..ready.len() {
        match ready.pop().expect("made output count") {
            views::Request::Watching { .. } => {}
            views::Request::Make { .. } => unreachable!("construction answered within the step"),
            views::Request::Deliver { watcher, .. } => match application_subject(core, watcher) {
                Some((connector, subject, _, _, _)) => {
                    out.push(Request::Held(Box::new(Held::ApplicationDelivery { watcher, connector, subject })));
                }
                None => {
                    let document = documents.remove(&watcher).expect("delivery follows construction");
                    out.push(Request::Held(Box::new(Held::Delivery { watcher, document: Box::new(document) })));
                }
            },
            views::Request::Ended { watcher, end: why } => {
                let held = match application_subject(core, watcher) {
                    Some((connector, subject, _, _, _)) => {
                        Held::ApplicationWatchEnded { watcher, connector, subject, why }
                    }
                    None => Held::WatchEnded { watcher, why },
                };
                forget_watch(core, watcher);
                out.push(Request::Held(Box::new(held)));
            }
            views::Request::Refused { watcher, refusal } => {
                forget_watch(core, watcher);
                let why = match refusal {
                    views::Refusal::Busy => WatchRefusal::Busy,
                };
                out.push(Request::Now(Box::new(Now::WatchRefused { watcher, why })));
            }
            views::Request::Stream { watcher, turn, from, to, text, cut, call, writing } => {
                out.push(Request::Now(Box::new(Now::RunText { watcher, turn, from, to, text, cut, call, writing })));
            }
        }
    }
    out.push(Request::Decided);
    Requests::Out(out)
}

fn refused(watcher: Token, why: WatchRefusal) -> Requests {
    let mut out = Queue::with_capacity(2);
    out.push(Request::Now(Box::new(Now::WatchRefused { watcher, why })));
    out.push(Request::Decided);
    Requests::Out(out)
}

fn refused_subject(watcher: Token, subject: &Subject, why: WatchRefusal) -> Requests {
    match subject {
        Subject::Application { connector, subject, .. } => {
            let mut out = Queue::with_capacity(3);
            out.push(Request::Ask { connector: *connector, ask: crate::Ask::DropDocument { subject: *subject } });
            out.push(Request::Now(Box::new(Now::WatchRefused { watcher, why })));
            out.push(Request::Decided);
            Requests::Out(out)
        }
        Subject::Frame { .. }
        | Subject::Task { .. }
        | Subject::Tree { .. }
        | Subject::Chips { .. }
        | Subject::Inbox { .. }
        | Subject::Accounts { .. }
        | Subject::Query { .. }
        | Subject::Run { .. }
        | Subject::Settings
        | Subject::Deployment
        | Subject::Proposal { .. }
        | Subject::Escalation { .. }
        | Subject::Question { .. }
        | Subject::PersonTask { .. }
        | Subject::Result { .. }
        | Subject::Project { .. }
        | Subject::Note { .. } => refused(watcher, why),
    }
}

pub(super) fn open(
    core: &mut Core,
    env: &Env<Limits>,
    watcher: Token,
    sign_in: u64,
    subject: Watched,
    window: Window,
) -> Requests {
    let follow = match &subject {
        Watched::Run { task } => Some(views::Follow { run: Token::new(*task), turn: None, offset: 0 }),
        Watched::Frame
        | Watched::Task { .. }
        | Watched::Tree { .. }
        | Watched::Chips { .. }
        | Watched::Accounts
        | Watched::Inbox
        | Watched::Query { .. }
        | Watched::Settings
        | Watched::Deployment
        | Watched::Application { .. } => None,
    };
    open_follow(core, env, watcher, sign_in, subject, window, follow)
}

pub(super) fn open_run(
    core: &mut Core,
    env: &Env<Limits>,
    watcher: Token,
    sign_in: u64,
    task: u64,
    turn: Option<views::TurnName>,
    offset: u32,
) -> Requests {
    open_follow(
        core,
        env,
        watcher,
        sign_in,
        Watched::Run { task },
        Window { rows: env.limits.documents.rows, after: None },
        Some(views::Follow { run: Token::new(task), turn, offset }),
    )
}

// Retain one clamped window for every construction (`client/domain/contract.md`, 7.2).
fn open_follow(
    core: &mut Core,
    env: &Env<Limits>,
    watcher: Token,
    sign_in: u64,
    subject: Watched,
    window: Window,
    follow: Option<views::Follow>,
) -> Requests {
    let Some(party) = core.people.person(sign_in, env.now, env.wall) else {
        return match subject {
            Watched::Application { scope, connector, subject } => {
                refused_subject(watcher, &Subject::Application { scope, connector, subject }, WatchRefusal::SignedOut)
            }
            Watched::Frame
            | Watched::Task { .. }
            | Watched::Tree { .. }
            | Watched::Chips { .. }
            | Watched::Accounts
            | Watched::Inbox
            | Watched::Query { .. }
            | Watched::Run { .. }
            | Watched::Settings
            | Watched::Deployment => refused(watcher, WatchRefusal::SignedOut),
        };
    };
    let subject = named_subject(subject, party);
    let window = Window { rows: window.rows.min(env.limits.documents.rows), ..window };
    if core.view_subjects.contains_key(&watcher) || core.view_subjects.len() >= env.limits.views.watchers {
        return refused_subject(watcher, &subject, WatchRefusal::Busy);
    }
    if let Some(why) = admission(core, env, party, &subject, &window) {
        return refused_subject(watcher, &subject, why);
    }
    let subject = match subject {
        Subject::Chips { tasks: named } => {
            let mut tasks = List::with_capacity(named.len());
            for number in named.into_boxed() {
                tasks.push(number).expect("admitted chip count");
            }
            Subject::Chips { tasks }
        }
        subject @ (Subject::Frame { .. }
        | Subject::Task { .. }
        | Subject::Tree { .. }
        | Subject::Inbox { .. }
        | Subject::Accounts { .. }
        | Subject::Query { .. }
        | Subject::Run { .. }
        | Subject::Settings
        | Subject::Deployment
        | Subject::Proposal { .. }
        | Subject::Escalation { .. }
        | Subject::Question { .. }
        | Subject::PersonTask { .. }
        | Subject::Result { .. }
        | Subject::Project { .. }
        | Subject::Note { .. }
        | Subject::Application { .. }) => subject,
    };
    let mut existing = None;
    for (_, binding) in &core.view_subjects {
        if core.view_names.get(binding.subject).expect("bound subject").subject == subject {
            existing = Some(binding.subject);
            break;
        }
    }
    let name = match existing {
        Some(name) => {
            let shared = core.view_names.get_mut(name).expect("shared watched subject");
            shared.watchers = shared.watchers.checked_add(1).expect("bounded subject watchers");
            name
        }
        None => {
            if core.view_names.is_full() {
                return refused_subject(watcher, &subject, WatchRefusal::Busy);
            }
            core.view_names.insert(WatchedSubject { subject, watchers: 1 }).expect("admitted watched subject")
        }
    };
    let view_window = views::Window { rows: window.rows, after: None };
    let previous = core
        .view_subjects
        .insert(watcher, WatchBinding { party, sign_in, subject: name, window })
        .expect("bounded watch binding");
    assert!(previous.is_none(), "one binding per watcher");
    route_view(
        core,
        env,
        views::Event::Watch { watcher, party: Token::new(party), subject: name.token(), window: view_window, follow },
    )
}

// Construction authenticates jig's named values; connector values remain opaque.
// Unmakeable documents refuse Unknown (`client/domain/contract.md`, 7.2).
fn admission(core: &Core, env: &Env<Limits>, party: u64, subject: &Subject, window: &Window) -> Option<WatchRefusal> {
    match subject {
        Subject::Application { scope, connector, .. } => {
            if !core.connectors.as_ref().contains(connector) {
                return Some(WatchRefusal::Unknown);
            }
            if *scope != jig_domain_people::ApplicationScope::Own && core.people.role(party, scope.project()).is_none()
            {
                return Some(WatchRefusal::NotAllowed);
            }
            None
        }
        Subject::Frame { .. }
        | Subject::Task { .. }
        | Subject::Tree { .. }
        | Subject::Chips { .. }
        | Subject::Inbox { .. }
        | Subject::Accounts { .. }
        | Subject::Query { .. }
        | Subject::Run { .. }
        | Subject::Settings
        | Subject::Deployment
        | Subject::Proposal { .. }
        | Subject::Escalation { .. }
        | Subject::Question { .. }
        | Subject::PersonTask { .. }
        | Subject::Result { .. }
        | Subject::Project { .. }
        | Subject::Note { .. } => match documents::make(core, &env.limits, party, subject, window, env.wall) {
            Ok(_) => None,
            Err(Missing::NotAllowed) => Some(WatchRefusal::NotAllowed),
            Err(Missing::Unknown | Missing::Ended) if retained_subject(subject).is_some() => {
                if core.reads.len() >= env.limits.load_slots { Some(WatchRefusal::Busy) } else { None }
            }
            Err(Missing::Unknown | Missing::Ended | Missing::TooLarge) => Some(WatchRefusal::Unknown),
        },
    }
}

fn named_subject(subject: Watched, party: u64) -> Subject {
    match subject {
        Watched::Application { scope, connector, subject } => Subject::Application { scope, connector, subject },
        Watched::Frame => Subject::Frame { party },
        Watched::Task { task } => Subject::Task { task },
        Watched::Tree { task } => Subject::Tree { task },
        Watched::Chips { tasks } => Subject::Chips { tasks },
        Watched::Inbox => Subject::Inbox { party },
        Watched::Accounts => Subject::Accounts { party },
        Watched::Query { query } => Subject::Query { query },
        Watched::Run { task } => Subject::Run { task },
        Watched::Settings => Subject::Settings,
        Watched::Deployment => Subject::Deployment,
    }
}

fn application_subject(
    core: &Core,
    watcher: Token,
) -> Option<(u16, Token, jig_domain_people::ApplicationScope, u64, Window)> {
    let binding = core.view_subjects.get(&watcher)?;
    let subject = &core.view_names.get(binding.subject)?.subject;
    match subject {
        Subject::Application { scope, connector, subject } => {
            Some((*connector, *subject, *scope, binding.party, binding.window))
        }
        Subject::Frame { .. }
        | Subject::Task { .. }
        | Subject::Tree { .. }
        | Subject::Chips { .. }
        | Subject::Inbox { .. }
        | Subject::Accounts { .. }
        | Subject::Query { .. }
        | Subject::Run { .. }
        | Subject::Settings
        | Subject::Deployment
        | Subject::Proposal { .. }
        | Subject::Escalation { .. }
        | Subject::Question { .. }
        | Subject::PersonTask { .. }
        | Subject::Result { .. }
        | Subject::Project { .. }
        | Subject::Note { .. } => None,
    }
}

/// Marks are gathered until all of the decision's connector handoffs finish.
pub(super) fn marked(core: &mut Core, connector: u16, subjects: List<Token>) -> Requests {
    for (_, binding) in &core.view_subjects {
        let subject = &core.view_names.get(binding.subject).expect("bound application subject").subject;
        let touched = match subject {
            Subject::Application { connector: owner, subject, .. } => {
                *owner == connector && subjects.as_slice().contains(subject)
            }
            Subject::Frame { .. }
            | Subject::Task { .. }
            | Subject::Tree { .. }
            | Subject::Chips { .. }
            | Subject::Inbox { .. }
            | Subject::Accounts { .. }
            | Subject::Query { .. }
            | Subject::Run { .. }
            | Subject::Settings
            | Subject::Deployment
            | Subject::Proposal { .. }
            | Subject::Escalation { .. }
            | Subject::Question { .. }
            | Subject::PersonTask { .. }
            | Subject::Result { .. }
            | Subject::Project { .. }
            | Subject::Note { .. } => false,
        };
        if touched {
            let _new = core.changed_views.insert(binding.subject).expect("bounded connector marks");
        }
    }
    let mut out = Queue::with_capacity(1);
    out.push(Request::Decided);
    Requests::Out(out)
}

fn forget_watch(core: &mut Core, watcher: Token) {
    let Some(binding) = core.view_subjects.remove(&watcher) else { return };
    let subject = core.view_names.get_mut(binding.subject).expect("bound watched subject");
    subject.watchers = subject.watchers.checked_sub(1).expect("an ending watch owns its subject");
    if subject.watchers == 0 {
        let _changed = core.changed_views.remove(&binding.subject);
        core.view_names.retire(binding.subject);
    }
}

fn watch_document(
    core: &Core,
    limits: &Limits,
    watcher: Token,
    now: skein_lib::Time,
    wall: skein_lib::Wall,
) -> Result<documents::Document, views::End> {
    let binding = core.view_subjects.get(&watcher).ok_or(views::End::Gone)?;
    if core.people.person(binding.sign_in, now, wall) != Some(binding.party) {
        return Err(views::End::SignedOut);
    }
    let subject = &core.view_names.get(binding.subject).expect("bound watched subject").subject;
    let filtered = match subject {
        Subject::Chips { tasks: named } => {
            let mut tasks = List::with_capacity(named.len());
            for number in named {
                let Some(row) = core.tasks.task(*number) else { continue };
                match row.phase {
                    jig_domain_tasks::Phase::Ended(_) => {}
                    jig_domain_tasks::Phase::Waiting
                    | jig_domain_tasks::Phase::Active(_)
                    | jig_domain_tasks::Phase::Closing(_)
                    | jig_domain_tasks::Phase::Held { .. } => tasks.push(*number).expect("bounded live chips"),
                }
            }
            Some(Subject::Chips { tasks })
        }
        Subject::Frame { .. }
        | Subject::Task { .. }
        | Subject::Tree { .. }
        | Subject::Inbox { .. }
        | Subject::Accounts { .. }
        | Subject::Query { .. }
        | Subject::Run { .. }
        | Subject::Settings
        | Subject::Deployment
        | Subject::Proposal { .. }
        | Subject::Escalation { .. }
        | Subject::Question { .. }
        | Subject::PersonTask { .. }
        | Subject::Result { .. }
        | Subject::Project { .. }
        | Subject::Note { .. }
        | Subject::Application { .. } => None,
    };
    let subject = match &filtered {
        Some(filtered) => filtered,
        None => subject,
    };
    match documents::make_with_clock(
        core,
        limits,
        binding.party,
        subject,
        &binding.window,
        documents::DocumentClock { now, wall },
    ) {
        Ok(document) => Ok(document),
        Err(Missing::NotAllowed) => Err(views::End::NotAllowed),
        Err(Missing::Unknown | Missing::Ended | Missing::TooLarge) => Err(views::End::Gone),
    }
}

#[expect(clippy::too_many_arguments, reason = "a live host reports its fenced turn, offset, text and call")]
pub(super) fn run_text(
    core: &mut Core,
    env: &Env<Limits>,
    run: Token,
    attempt: Token,
    turn: u32,
    retry: u32,
    offset: u32,
    text: Box<[u8]>,
    call: Option<views::Call>,
) -> Requests {
    let mut out = Queue::uncapped();
    append_stream(super::watch_marks::end_invalid(core, env), &mut out);
    let live = match core.proofs.get(&run.raw()) {
        Some(proof) => {
            proof.attempt == attempt.raw()
                && proof.terminal.is_none()
                && turn != 0
                && match proof.turn {
                    Some(kept) => turn > kept.turn,
                    None => true,
                }
        }
        None => false,
    };
    if live {
        let had_call = views::has_current_call(&core.views, run, attempt.raw());
        append_stream(
            route_view(
                core,
                env,
                views::Event::Text {
                    run,
                    turn: views::TurnName { attempt: attempt.raw(), turn, retry },
                    offset,
                    text,
                    call,
                },
            ),
            &mut out,
        );
        if had_call && !views::has_current_call(&core.views, run, attempt.raw()) {
            changed_run(core, env, run, &mut out);
        }
    }
    out.push(Request::Decided);
    Requests::Out(out)
}

#[expect(clippy::too_many_arguments, reason = "the host reports input with its fenced turn and provider identity")]
pub(super) fn run_input(
    core: &mut Core,
    env: &Env<Limits>,
    run: Token,
    attempt: Token,
    turn: u32,
    retry: u32,
    offset: u32,
    id: Box<[u8]>,
    tool: Box<[u8]>,
    input: Box<[u8]>,
) -> Requests {
    let mut out = Queue::uncapped();
    append_stream(super::watch_marks::end_invalid(core, env), &mut out);
    let live = match core.proofs.get(&run.raw()) {
        Some(proof) => {
            proof.attempt == attempt.raw()
                && proof.terminal.is_none()
                && turn != 0
                && match proof.turn {
                    Some(kept) => turn > kept.turn,
                    None => true,
                }
        }
        None => false,
    };
    if live {
        let had_call = views::has_current_call(&core.views, run, attempt.raw());
        append_stream(
            route_view(
                core,
                env,
                views::Event::Input {
                    run,
                    turn: views::TurnName { attempt: attempt.raw(), turn, retry },
                    offset,
                    id,
                    tool,
                    input,
                },
            ),
            &mut out,
        );
        if had_call && !views::has_current_call(&core.views, run, attempt.raw()) {
            changed_run(core, env, run, &mut out);
        }
    }
    out.push(Request::Decided);
    Requests::Out(out)
}

pub(super) fn run_call(core: &mut Core, env: &Env<Limits>, run: Token, attempt: Token, call: views::Call) -> Requests {
    let live = match core.proofs.get(&run.raw()) {
        Some(proof) => proof.attempt == attempt.raw() && proof.terminal.is_none(),
        None => false,
    };
    let mut out = Queue::uncapped();
    if live {
        append_stream(super::watch_marks::end_invalid(core, env), &mut out);
        let before = (views::retained_attempt(&core.views, run), views::revision(&core.views, run));
        append_stream(route_view(core, env, views::Event::Called { run, attempt: attempt.raw(), call }), &mut out);
        if before != (views::retained_attempt(&core.views, run), views::revision(&core.views, run)) {
            changed_run(core, env, run, &mut out);
        }
    }
    out.push(Request::Decided);
    Requests::Out(out)
}

fn changed_run(core: &mut Core, env: &Env<Limits>, run: Token, out: &mut Queue<Request>) {
    let mut named = None;
    for (_, binding) in &core.view_subjects {
        let subject = &core.view_names.get(binding.subject).expect("bound current-run subject").subject;
        match subject {
            Subject::Run { task } if *task == run.raw() => {
                named = Some(binding.subject.token());
                break;
            }
            Subject::Frame { .. }
            | Subject::Task { .. }
            | Subject::Tree { .. }
            | Subject::Chips { .. }
            | Subject::Inbox { .. }
            | Subject::Accounts { .. }
            | Subject::Query { .. }
            | Subject::Run { .. }
            | Subject::Settings
            | Subject::Deployment
            | Subject::Proposal { .. }
            | Subject::Escalation { .. }
            | Subject::Question { .. }
            | Subject::PersonTask { .. }
            | Subject::Result { .. }
            | Subject::Project { .. }
            | Subject::Note { .. }
            | Subject::Application { .. } => {}
        }
    }
    if let Some(name) = named {
        let mut subjects = List::with_capacity(1);
        subjects.push(name).expect("one watched run subject");
        append_stream(route_view(core, env, views::Event::Changed { subjects }), out);
    }
}

pub(super) fn stream_delivered(core: &mut Core, env: &Env<Limits>, watcher: Token) -> Requests {
    let mut out = Queue::uncapped();
    append_stream(super::watch_marks::end_invalid(core, env), &mut out);
    append_stream(route_view(core, env, views::Event::StreamDelivered { watcher }), &mut out);
    out.push(Request::Decided);
    Requests::Out(out)
}
fn append_stream(requests: Requests, out: &mut Queue<Request>) {
    let Requests::Out(mut requests) = requests;
    for _ in 0..requests.len() {
        match requests.pop().expect("stream output count") {
            Request::Decided => {}
            request @ (Request::Held(_)
            | Request::Now(_)
            | Request::ProjectMade { .. }
            | Request::Write(_)
            | Request::Ask { .. }) => {
                out.push(request);
            }
        }
    }
}

// Task facts serve both final task and final tree documents, through the same load.
pub(crate) fn retained_task(core: &Core, watcher: Token) -> Option<u64> {
    let binding = core.view_subjects.get(&watcher)?;
    retained_subject(&core.view_names.get(binding.subject)?.subject)
}

fn retained_subject(subject: &Subject) -> Option<u64> {
    match subject {
        Subject::Task { task } | Subject::Tree { task } => Some(*task),
        Subject::Frame { .. }
        | Subject::Chips { .. }
        | Subject::Inbox { .. }
        | Subject::Accounts { .. }
        | Subject::Query { .. }
        | Subject::Run { .. }
        | Subject::Settings
        | Subject::Deployment
        | Subject::Proposal { .. }
        | Subject::Escalation { .. }
        | Subject::Question { .. }
        | Subject::PersonTask { .. }
        | Subject::Result { .. }
        | Subject::Project { .. }
        | Subject::Note { .. }
        | Subject::Application { .. } => None,
    }
}

pub(crate) fn retained_made(
    core: &mut Core,
    env: &Env<Limits>,
    watcher: Token,
    document: Result<documents::Document, views::End>,
) -> Requests {
    let mut child = Queue::uncapped();
    let view_env = Env { now: env.now, wall: env.wall, limits: env.limits.views };
    let outcome = match &document {
        Ok(_) => views::Made::Ready,
        Err(why) => views::Made::Gone { why: *why },
    };
    views::step(&mut core.views, &view_env, views::Event::Made { watcher, outcome }, &mut child);
    if document.is_ok() {
        views::step(&mut core.views, &view_env, views::Event::EndWatch { watcher, end: views::End::Gone }, &mut child);
    }
    let mut out = Queue::uncapped();
    for _ in 0..child.len() {
        match child.pop().expect("made output count") {
            views::Request::Deliver { .. } => {
                let document = document.expect("ready retained document");
                out.push(Request::Held(Box::new(Held::Delivery { watcher, document: Box::new(document) })));
                out.push(Request::Decided);
                return Requests::Out(out);
            }
            views::Request::Ended { end, .. } => {
                forget_watch(core, watcher);
                out.push(Request::Held(Box::new(Held::WatchEnded { watcher, why: end })));
            }
            views::Request::Watching { .. }
            | views::Request::Refused { .. }
            | views::Request::Make { .. }
            | views::Request::Stream { .. } => unreachable!("construction emits delivery or ending"),
        }
    }
    out.push(Request::Decided);
    Requests::Out(out)
}
