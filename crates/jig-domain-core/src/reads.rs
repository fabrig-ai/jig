//! Authenticated unkeyed reads (`client/domain/contract.md`, section 6).
//! Live documents are copied at admission; past rows load in bounded owner
//! ranges. The core keeps only each load's reply, sign-in and query. A terminal
//! consumes it, checks current visibility and moves the answer to the journal.
//! Neither reading nor loading advances a party's read position.
//!
//! | State | Event | Next | Emits |
//! | --- | --- | --- | --- |
//! | absent | visible live read | absent | held document or live page |
//! | absent | stored read, room | loading | held owner-range load |
//! | absent | full load slab | absent | held busy |
//! | loading | bounded visible page | retired | held answer |
//! | loading | signed out or invisible | retired | held unknown |
//! | loading | missing transport range | retired | held gone |
//! | retired | late duplicate terminal | retired | nothing |

pub mod settings;
pub use settings::{SettingsAnswer, SettingsRead};
mod vocabulary;
pub use vocabulary::{
    Answer, Cursor, EndedGroup, Filter, Group, Historical, Loaded, Page, PageRow, PageTurn, Past, Position, Range,
    Read, RevisionAnswer, RevisionEdit, RevisionItemHead, RevisionRow, RevisionsPage, Row, Scope, SettingDocument,
    TaskPage, TurnFailure, UnjoinedTurn, Why,
};
pub(crate) mod activity;
mod answers;
mod watches;
pub(crate) use watches::start_watch;
use watches::watch_loaded;
pub mod ended;
pub mod indexed;
mod rows;
mod turn;
pub use rows::row_key;

use crate::documents::{self, DocumentClock, Missing, Subject, Window};
use crate::{Core, Held, Limits, Request, Requests};
use alloc::boxed::Box;
use core::mem::size_of;
use skein_lib::{Env, Id, List, Queue, ReplyTo, Token};

/// Which physical rows a read scope loads (protocol/store.md, 4.3 and 7.3).
/// Cursor admission is separate: notes lines are joined to entry rows before paging.
#[must_use]
pub fn admits(party: u64, scope: &Scope, key: &crate::Key) -> bool {
    use crate::{Key, notes};
    use jig_domain_people::Proposer;
    use jig_domain_tasks::{Funder, Key as TaskKey};
    match scope {
        Scope::SettingsHistory { first, last, newest, .. } => match key {
            Key::Core(crate::CoreKey::Settings(crate::settings::Key::Held(n))) => n >= first && n <= last,
            Key::Core(crate::CoreKey::Settings(crate::settings::Key::History(n))) => {
                n != newest && n >= first && n <= last
            }
            Key::Core(_) | Key::Tasks(_) | Key::People(_) | Key::Notes(_) | Key::Accounts(_) => false,
        },
        Scope::Task { task } | Scope::PersonTask { task } | Scope::Result { task } => {
            key == &Key::Tasks(TaskKey::Live(*task)) || key == &Key::Tasks(TaskKey::Ended(*task))
        }
        Scope::Reply { task, activity } => key == &Key::Tasks(TaskKey::Activity { task: *task, position: *activity }),
        Scope::History { task, subtree: false } => match key {
            Key::Tasks(TaskKey::History { task: owner, .. } | TaskKey::Milestone { task: owner, .. }) => owner == task,
            Key::Tasks(_) | Key::Core(_) | Key::People(_) | Key::Notes(_) | Key::Accounts(_) => false,
        },
        Scope::Proposal { proposer: Proposer::Party(owner), proposal } => {
            *owner == party
                && (key == &Key::Tasks(TaskKey::PersonProposal(*proposal))
                    || key == &Key::Tasks(TaskKey::DecidedProposal(*proposal)))
        }
        Scope::EndedDelegates { task } => match key {
            Key::Tasks(TaskKey::EndedDelegate { requester, .. }) => requester == task,
            Key::Tasks(_) | Key::Core(_) | Key::People(_) | Key::Notes(_) | Key::Accounts(_) => false,
        },
        Scope::Spend { project } => match key {
            Key::Tasks(
                TaskKey::Ledger(Funder::Period { project: owner, .. })
                | TaskKey::ClosedLedger(Funder::Period { project: owner, .. }),
            ) => owner == project,
            Key::Tasks(_) | Key::Core(_) | Key::People(_) | Key::Notes(_) | Key::Accounts(_) => false,
        },
        Scope::Notes { project } => match key {
            Key::Notes(notes::Key::Line { scope, .. }) => match scope {
                notes::Scope::Deployment => *project == 0,
                notes::Scope::Project { project: owner }
                | notes::Scope::Goal { project: owner, .. }
                | notes::Scope::Resources { project: owner, .. } => owner == project,
            },
            Key::Notes(_) | Key::Tasks(_) | Key::Core(_) | Key::People(_) | Key::Accounts(_) => false,
        },
        Scope::ScopedNotes { scope: wanted, .. } => match key {
            Key::Notes(notes::Key::Line { scope, .. }) => scope == wanted,
            Key::Notes(_) | Key::Tasks(_) | Key::Core(_) | Key::People(_) | Key::Accounts(_) => false,
        },
        Scope::Inbox { group: Group::Updates | Group::Results | Group::Replies, .. } => match key {
            Key::Tasks(TaskKey::Update { party: owner, .. }) => *owner == party,
            Key::Tasks(_) | Key::Core(_) | Key::People(_) | Key::Notes(_) | Key::Accounts(_) => false,
        },
        Scope::Tasks { .. }
        | Scope::Proposal { proposer: Proposer::Task(_), .. }
        | Scope::Escalation { .. }
        | Scope::Question { .. }
        | Scope::Turn { .. }
        | Scope::History { subtree: true, .. }
        | Scope::Activity { .. }
        | Scope::Inbox {
            group: Group::Needs | Group::Proposals | Group::Escalations | Group::Questions | Group::PersonTasks,
            ..
        } => false,
    }
}

#[derive(Debug)]
pub(crate) struct Pending {
    destination: Destination,
    sign_in: u64,
    party: u64,
    range: Range,
    tasks: Option<Box<TaskPage>>,
}

#[derive(Debug)]
enum Destination {
    Read(ReplyTo),
    Watch(Token),
    Settings { to: ReplyTo, key: crate::CallKey },
    Done,
}

impl Pending {
    fn take_reply(&mut self) -> ReplyTo {
        match core::mem::replace(&mut self.destination, Destination::Done) {
            Destination::Read(reply) => reply,
            Destination::Watch(_) | Destination::Settings { .. } | Destination::Done => {
                unreachable!("a pending read owns its reply")
            }
        }
    }
}

fn terminal(reply: ReplyTo, answer: Answer) -> Requests {
    let mut out = Queue::with_capacity(2);
    out.push(Request::Held(Box::new(Held::Read { reply, answer })));
    out.push(Request::Decided);
    Requests::Out(out)
}

pub(crate) fn scope(read: &Read) -> Option<Scope> {
    match read {
        Read::Task { task } => Some(Scope::Task { task: *task }),
        Read::Proposal { proposer, proposal } => Some(Scope::Proposal { proposer: *proposer, proposal: *proposal }),
        Read::Escalation { task } => Some(Scope::Escalation { task: *task }),
        Read::Question { task, message } => Some(Scope::Question { task: *task, message: *message }),
        Read::PersonTask { task } => Some(Scope::PersonTask { task: *task }),
        Read::Result { task } => Some(Scope::Result { task: *task }),
        Read::Reply { task, activity } => Some(Scope::Reply { task: *task, activity: *activity }),
        Read::Turn { task, attempt, turn } => Some(Scope::Turn { task: *task, attempt: *attempt, turn: *turn }),
        Read::EndedDelegates { task, .. } => Some(Scope::EndedDelegates { task: *task }),
        Read::History { task, subtree, .. } => Some(Scope::History { task: *task, subtree: *subtree }),
        Read::Activity { task, .. } => Some(Scope::Activity { task: *task }),
        Read::Inbox { group, filter, .. } => Some(Scope::Inbox { group: *group, filter: *filter }),
        Read::Tasks { query, .. } => Some(Scope::Tasks { query: *query }),
        Read::Notes { project, .. } => Some(Scope::Notes { project: *project }),
        Read::ScopedNotes { scope, description, .. } => {
            Some(Scope::ScopedNotes { scope: scope.clone(), description: description.clone() })
        }
        Read::Spend { project, .. } => Some(Scope::Spend { project: *project }),
        Read::Frame
        | Read::Project { .. }
        | Read::Settings
        | Read::Setting { .. }
        | Read::Revision { .. }
        | Read::Revisions { .. }
        | Read::Deployment
        | Read::Application { .. } => None,
    }
}

fn cursor(read: &Read) -> Option<&Cursor> {
    match read {
        Read::EndedDelegates { from, .. }
        | Read::History { from, .. }
        | Read::Inbox { from, .. }
        | Read::Tasks { ended: from, .. }
        | Read::Notes { from, .. }
        | Read::ScopedNotes { from, .. }
        | Read::Spend { from, .. } => from.as_ref(),
        Read::Activity { before, .. } => before.as_ref(),
        Read::Frame
        | Read::Task { .. }
        | Read::Proposal { .. }
        | Read::Escalation { .. }
        | Read::Question { .. }
        | Read::PersonTask { .. }
        | Read::Result { .. }
        | Read::Reply { .. }
        | Read::Turn { .. }
        | Read::Project { .. }
        | Read::Settings
        | Read::Setting { .. }
        | Read::Revision { .. }
        | Read::Revisions { .. }
        | Read::Deployment
        | Read::Application { .. } => None,
    }
}

fn subject(read: &Read, party: u64) -> Option<Subject> {
    match read {
        Read::Settings => Some(Subject::Settings),
        Read::Frame => Some(Subject::Frame { party }),
        Read::Task { task } => Some(Subject::Task { task: *task }),
        Read::Proposal { proposer, proposal } => Some(Subject::Proposal { proposer: *proposer, number: *proposal }),
        Read::Escalation { task } => Some(Subject::Escalation { task: *task }),
        Read::Question { task, message } => Some(Subject::Question { task: *task, message: *message }),
        Read::PersonTask { task } => Some(Subject::PersonTask { task: *task }),
        Read::Result { task } => Some(Subject::Result { task: *task }),
        Read::Project { project } => Some(Subject::Project { project: *project }),
        Read::Deployment => Some(Subject::Deployment),
        Read::Inbox { .. } => Some(Subject::Inbox { party }),
        Read::Tasks { query, .. } => Some(Subject::Query { query: *query }),
        Read::Setting { .. }
        | Read::Revision { .. }
        | Read::Revisions { .. }
        | Read::Application { .. }
        | Read::EndedDelegates { .. }
        | Read::History { .. }
        | Read::Activity { .. }
        | Read::Notes { .. }
        | Read::ScopedNotes { .. }
        | Read::Spend { .. }
        | Read::Reply { .. }
        | Read::Turn { .. } => None,
    }
}

fn backwards(scope: &Scope) -> bool {
    match scope {
        Scope::Activity { .. } | Scope::Escalation { .. } | Scope::Proposal { .. } => true,
        Scope::SettingsHistory { .. }
        | Scope::Task { .. }
        | Scope::Question { .. }
        | Scope::PersonTask { .. }
        | Scope::Result { .. }
        | Scope::Reply { .. }
        | Scope::Turn { .. }
        | Scope::EndedDelegates { .. }
        | Scope::History { .. }
        | Scope::Inbox {
            group: Group::Needs | Group::Proposals | Group::Escalations | Group::Questions | Group::PersonTasks,
            ..
        }
        | Scope::Inbox { group: Group::Updates | Group::Results | Group::Replies, .. }
        | Scope::Tasks { .. }
        | Scope::Notes { .. }
        | Scope::ScopedNotes { .. }
        | Scope::Spend { .. } => false,
    }
}

fn refused_application(reply: ReplyTo, read: &Read, why: Why) -> Requests {
    match read {
        Read::Application { connector, read, .. } => {
            let mut out = Queue::with_capacity(3);
            out.push(Request::Ask { connector: *connector, ask: crate::Ask::DropDocument { subject: *read } });
            out.push(Request::Held(Box::new(Held::Read { reply, answer: Answer::None(why) })));
            out.push(Request::Decided);
            Requests::Out(out)
        }
        Read::Frame
        | Read::Task { .. }
        | Read::Proposal { .. }
        | Read::Escalation { .. }
        | Read::Question { .. }
        | Read::PersonTask { .. }
        | Read::Result { .. }
        | Read::Reply { .. }
        | Read::Turn { .. }
        | Read::EndedDelegates { .. }
        | Read::History { .. }
        | Read::Activity { .. }
        | Read::Inbox { .. }
        | Read::Tasks { .. }
        | Read::Notes { .. }
        | Read::ScopedNotes { .. }
        | Read::Spend { .. }
        | Read::Project { .. }
        | Read::Settings
        | Read::Setting { .. }
        | Read::Revision { .. }
        | Read::Revisions { .. }
        | Read::Deployment => terminal(reply, Answer::None(why)),
    }
}

pub(crate) fn start(core: &mut Core, env: &Env<Limits>, reply: ReplyTo, sign_in: u64, read: Read) -> Requests {
    let Some(party) = core.people.person(sign_in, env.now, env.wall) else {
        let why = if core.people.ready() { Why::SignedOut } else { Why::Unknown };
        return refused_application(reply, &read, why);
    };
    match &read {
        Read::Application { scope, connector, read: name, window } => {
            if (*scope != jig_domain_people::ApplicationScope::Own
                && core.people.role(party, scope.project()).is_none())
                || !core.connectors.as_ref().contains(connector)
            {
                return refused_application(reply, &read, Why::Unknown);
            }
            let mut out = Queue::with_capacity(2);
            out.push(Request::Ask {
                connector: *connector,
                ask: crate::Ask::Read { reply, scope: *scope, read: *name, party, window: *window },
            });
            out.push(Request::Decided);
            return Requests::Out(out);
        }
        Read::Frame
        | Read::Task { .. }
        | Read::Proposal { .. }
        | Read::Escalation { .. }
        | Read::Question { .. }
        | Read::PersonTask { .. }
        | Read::Result { .. }
        | Read::Reply { .. }
        | Read::Turn { .. }
        | Read::EndedDelegates { .. }
        | Read::History { .. }
        | Read::Activity { .. }
        | Read::Inbox { .. }
        | Read::Tasks { .. }
        | Read::Notes { .. }
        | Read::ScopedNotes { .. }
        | Read::Spend { .. }
        | Read::Project { .. }
        | Read::Settings
        | Read::Setting { .. }
        | Read::Revision { .. }
        | Read::Revisions { .. }
        | Read::Deployment => {}
    }
    match read {
        Read::Setting { setting, revision, proposal } => match proposal {
            Some(proposal) if revision == 0 => settings::proposed_setting(core, reply, party, setting, proposal),
            Some(_) => terminal(reply, Answer::None(Why::Malformed)),
            None => settings::setting(core, reply, party, setting, revision),
        },
        Read::Revision { number } => settings::revision(core, env, reply, sign_in, party, number),
        Read::Revisions { before } => settings::revisions(core, env, reply, sign_in, party, before),
        read @ (Read::Settings
        | Read::Tasks { .. }
        | Read::Application { .. }
        | Read::Frame
        | Read::Task { .. }
        | Read::Proposal { .. }
        | Read::Escalation { .. }
        | Read::Question { .. }
        | Read::PersonTask { .. }
        | Read::Result { .. }
        | Read::Reply { .. }
        | Read::Turn { .. }
        | Read::EndedDelegates { .. }
        | Read::History { .. }
        | Read::Activity { .. }
        | Read::Inbox { .. }
        | Read::Project { .. }
        | Read::Notes { .. }
        | Read::ScopedNotes { .. }
        | Read::Spend { .. }
        | Read::Deployment) => start_jig(core, env, reply, sign_in, party, read),
    }
}

fn bounded_query(read: &Read, limits: &Limits) -> bool {
    match read {
        Read::ScopedNotes { scope, description, .. } => {
            jig_domain_notes::valid_scope(scope, &limits.notes)
                && match description {
                    Some(query) => {
                        query.len() <= usize::try_from(limits.notes.description_bytes).expect("description bound fits")
                    }
                    None => true,
                }
        }
        Read::Frame
        | Read::Task { .. }
        | Read::Proposal { .. }
        | Read::Escalation { .. }
        | Read::Question { .. }
        | Read::PersonTask { .. }
        | Read::Result { .. }
        | Read::Reply { .. }
        | Read::Turn { .. }
        | Read::EndedDelegates { .. }
        | Read::History { .. }
        | Read::Activity { .. }
        | Read::Inbox { .. }
        | Read::Tasks { .. }
        | Read::Project { .. }
        | Read::Notes { .. }
        | Read::Spend { .. }
        | Read::Settings
        | Read::Setting { .. }
        | Read::Revision { .. }
        | Read::Revisions { .. }
        | Read::Deployment
        | Read::Application { .. } => true,
    }
}

struct ReadStart {
    after_live: Option<documents::Cursor>,
    after_store: Option<Position>,
    stored: bool,
}

fn read_start(read: &Read, party: u64) -> Option<ReadStart> {
    if let Read::Tasks { ended: Some(cursor), .. } = read {
        match cursor {
            Cursor::Store { .. } => {}
            Cursor::Live { .. } => return None,
        }
    }
    let target = scope(read);
    let mut after_live = None;
    let mut after_store = None;
    let mut stored = false;
    match cursor(read) {
        Some(Cursor::Live { scope, after, store_after }) => {
            if Some(scope) != target.as_ref() {
                return None;
            }
            subject(read, party)?;
            after_live = Some(*after);
            after_store.clone_from(store_after);
        }
        Some(Cursor::Store { scope, after }) => {
            if Some(scope) != target.as_ref() {
                return None;
            }
            stored = true;
            after_store.clone_from(after);
        }
        None => {}
    }
    Some(ReadStart { after_live, after_store, stored })
}

fn start_jig(core: &mut Core, env: &Env<Limits>, reply: ReplyTo, sign_in: u64, party: u64, read: Read) -> Requests {
    if !bounded_query(&read, &env.limits) {
        return terminal(reply, Answer::None(Why::Busy));
    }
    let target = scope(&read);
    let Some(start) = read_start(&read, party) else {
        return terminal(reply, Answer::None(Why::Unknown));
    };
    let mut tasks = if let Read::Tasks { query, live, .. } = &read {
        match task_live(core, env, party, *query, live.as_ref()) {
            Ok(page) => Some(Box::new(page)),
            Err(why) => return terminal(reply, Answer::None(why)),
        }
    } else {
        None
    };
    if let Some(page) = &tasks
        && page.ended == EndedGroup::None
    {
        return terminal(reply, Answer::Tasks(tasks.take().expect("task page")));
    }
    let ReadStart { after_live, after_store, stored } = start;
    if let Some(position) = &after_store
        && match &target {
            Some(scope) => !rows::position_allowed(scope, position),
            None => true,
        }
    {
        return terminal(reply, Answer::None(Why::Unknown));
    }
    if tasks.is_none()
        && !stored
        && let Some(subject) = subject(&read, party)
    {
        let window = Window { rows: env.limits.documents.rows, after: after_live };
        match documents::make_with_clock(
            core,
            &env.limits,
            party,
            &subject,
            &window,
            DocumentClock { now: env.now, wall: env.wall },
        ) {
            Ok(document) => {
                if !rows::needs_store(&read, &document) {
                    let answer = rows::live_page(core, &env.limits, party, &read, document);
                    return terminal(reply, answer);
                }
            }
            Err(Missing::NotAllowed) => return terminal(reply, Answer::None(Why::Unknown)),
            Err(Missing::TooLarge) => return terminal(reply, Answer::None(Why::Busy)),
            Err(Missing::Unknown | Missing::Ended) => {}
        }
    }
    let Some(scope) = target else {
        return terminal(reply, Answer::None(Why::Unknown));
    };
    if !rows::scope_allowed(core, party, &scope) {
        return terminal(reply, Answer::None(Why::Unknown));
    }
    if core.reads.len() >= env.limits.load_slots {
        return terminal(reply, Answer::None(Why::Busy));
    }
    let range = Range {
        party,
        backwards: match cursor(&read) {
            Some(Cursor::Live { .. } | Cursor::Store { .. }) | None => backwards(&scope),
        },
        scope,
        after: after_store,
        rows: env.limits.documents.rows,
        bytes: env.limits.load_bytes,
    };
    let emitted = range.clone();
    let pending = Pending { destination: Destination::Read(reply), sign_in, party, range, tasks };
    let load = match core.reads.insert(pending) {
        Ok(load) => load.token(),
        Err(mut pending) => return terminal(pending.take_reply(), Answer::None(Why::Busy)),
    };
    let mut out = Queue::with_capacity(2);
    out.push(Request::Held(Box::new(Held::ReadLoad { load, range: Box::new(emitted) })));
    out.push(Request::Decided);
    Requests::Out(out)
}

pub(crate) fn loaded(core: &mut Core, env: &Env<Limits>, load: Token, result: Loaded) -> Requests {
    let name = Id::<Pending>::from_token(load);
    let Some(pending) = core.reads.get(name) else {
        return Requests::Out(Queue::with_capacity(0));
    };
    match &pending.destination {
        Destination::Watch(watcher) => return watch_loaded(core, env, name, *watcher, result),
        Destination::Settings { .. } => return settings::run_loaded(core, env, name, result),
        Destination::Read(_) => {}
        Destination::Done => return Requests::Out(Queue::with_capacity(0)),
    }
    let answer = if core.people.person(pending.sign_in, env.now, env.wall) != Some(pending.party) {
        Answer::None(Why::SignedOut)
    } else if !rows::scope_allowed(core, pending.party, &pending.range.scope) {
        Answer::None(Why::Unknown)
    } else {
        match result {
            Loaded::TurnProblem { task, attempt, turn: number, owner, why } => {
                turn::failure(core, &pending.range, task, attempt, number, &owner, why)
            }
            Loaded::TooLinked { joins } => Answer::None(Why::TooLinked { joins }),
            Loaded::Busy => Answer::None(Why::Busy),
            Loaded::Gone => Answer::None(Why::Gone),
            Loaded::Page { rows, next } => rows::from_store(core, &env.limits, &pending.range, rows, next, env.wall),
        }
    };
    let answer = match (core.reads.get_mut(name).expect("live read").tasks.take(), answer) {
        (Some(mut tasks), Answer::Page(page)) => {
            let mut lines = List::with_capacity(env.limits.documents.rows);
            for row in page.rows.into_boxed() {
                let PageRow::Line(line) = row else { unreachable!("task index supplies lines") };
                lines.push(line).expect("bounded ended group");
            }
            tasks.ended = EndedGroup::Rows { rows: lines, next: page.next, cut: page.cut };
            Answer::Tasks(tasks)
        }
        (_, answer) => answer,
    };
    let reply = core.reads.get_mut(name).expect("live read").take_reply();
    core.reads.retire(name);
    terminal(reply, answer)
}

#[expect(clippy::manual_map, reason = "step code matches the optional live continuation")]
fn task_live(
    core: &Core,
    env: &Env<Limits>,
    party: u64,
    query: documents::TaskQuery,
    cursor: Option<&Cursor>,
) -> Result<TaskPage, Why> {
    let after = match cursor {
        None => None,
        Some(Cursor::Live { scope: Scope::Tasks { query: owner }, after, store_after: None }) if *owner == query => {
            Some(*after)
        }
        Some(_) => return Err(Why::Unknown),
    };
    let document = match documents::make_with_clock(
        core,
        &env.limits,
        party,
        &Subject::Query { query },
        &Window { rows: env.limits.documents.rows, after },
        DocumentClock { now: env.now, wall: env.wall },
    ) {
        Ok(document) => document,
        Err(Missing::TooLarge) => return Err(Why::Busy),
        Err(Missing::Unknown | Missing::NotAllowed | Missing::Ended) => return Err(Why::Unknown),
    };
    let documents::Document::Lines(lines) = document else { unreachable!("query makes task lines") };
    let live_next = match lines.next {
        Some(after) => Some(Cursor::Live { scope: Scope::Tasks { query }, after, store_after: None }),
        None => None,
    };
    let ended = match ended::ended_part(&query) {
        ended::EndedPart::None => EndedGroup::None,
        ended::EndedPart::ByParty { .. } | ended::EndedPart::ByProject { .. } | ended::EndedPart::Delegates { .. } => {
            EndedGroup::Rows { rows: List::with_capacity(0), next: None, cut: false }
        }
    };
    Ok(TaskPage { live: lines.lines, live_next, ended })
}

/// Maximum answer payload retained in a root's held-output slot.
#[must_use]
pub fn answer_worst_case(limits: &Limits) -> Option<u64> {
    let document = documents::document_with_context_worst_case(limits)?;
    // Run settings answers own the same documents, in an inline sum carrier.
    u64::try_from(size_of::<Answer>().max(size_of::<SettingsAnswer>())).ok()?.checked_add(
        document
            .checked_mul(u64::from(limits.documents.rows).checked_add(3)?)?
            .checked_add(limits.load_bytes.checked_mul(2)?)?
            .checked_add(List::<PageRow>::worst_case(limits.documents.rows)?)?
            .checked_add(u64::try_from(size_of::<Page>()).ok()?)?
            .checked_add(query_worst_case(limits)?)?
            .checked_add(settings::load_bytes(limits, limits.documents.rows)?.checked_mul(2)?)?,
    )
}

// Resource pattern segments and literal bytes, plus one description query.
pub(crate) fn query_worst_case(limits: &Limits) -> Option<u64> {
    u64::from(limits.notes.pattern_bytes)
        .checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?.checked_add(1)?)?
        .checked_add(u64::from(limits.notes.description_bytes))
}

pub(super) fn load_settings(core: &mut Core, env: &Env<Limits>, pending: Pending) -> Requests {
    if core.reads.len() >= env.limits.load_slots {
        return settings::answer(pending.destination, Answer::None(Why::Busy));
    }
    let emitted = pending.range.clone();
    let load = match core.reads.insert(pending) {
        Ok(load) => load.token(),
        Err(pending) => return settings::answer(pending.destination, Answer::None(Why::Busy)),
    };
    let mut out = Queue::with_capacity(2);
    out.push(Request::Held(Box::new(Held::ReadLoad { load, range: Box::new(emitted) })));
    out.push(Request::Decided);
    Requests::Out(out)
}
