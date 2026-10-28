//! Party note edits and run note requests translated through the notes child
//! (`domain/engine.md`, section 4.4; `domain/people.md`, section 5.1).

use super::refusals::note_person_refusal;
use super::runs::call_decision;
use super::{NoteBrief, NoteRoute};
use crate::{CallPart, Core, Event, Held, Key, Limits, Now, Record, Request, Requests, Write};
use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_notes as notes;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{Env, List, Queue, Token};

pub(super) fn route_notes(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    event: notes::Event,
) -> Requests {
    let mut child = Queue::with_capacity(notes::max_out(&env.limits.notes));
    notes::step(&mut core.notes, &Env { now: env.now, wall: env.wall, limits: env.limits.notes }, event, &mut child);
    tag_notes(core, env, work, child)
}

pub(super) fn person_note_scope(project: u32, scope: people::NoteScope) -> notes::Scope {
    match scope {
        people::NoteScope::Deployment => notes::Scope::Deployment,
        people::NoteScope::Project => notes::Scope::Project { project },
        people::NoteScope::Goal { goal } => notes::Scope::Goal { project, goal },
        people::NoteScope::Resources { connector, pattern } => {
            let mut segments = List::with_capacity(u32::try_from(pattern.segments.len()).expect("bounded pattern"));
            for segment in pattern.segments {
                segments.push(segment).expect("pattern segment room");
            }
            let last = match pattern.last {
                people::Last::Exact(value) => notes::Last::Exact(value),
                people::Last::Open(value) => notes::Last::Open(value),
            };
            notes::Scope::Resources {
                project,
                connector,
                pattern: notes::Pattern { segments: segments.into_boxed(), last },
            }
        }
    }
}

pub(super) fn person_note_change(change: people::NoteChange) -> notes::Change {
    match change {
        people::NoteChange::Correct { description, body, references, recalled } => {
            let mut kept = List::with_capacity(u32::try_from(references.len()).expect("bounded references"));
            for reference in references {
                kept.push(reference).expect("reference room");
            }
            notes::Change::Correct { description, body, references: kept, recalled }
        }
        people::NoteChange::Delete { recalled } => notes::Change::Delete { recalled },
    }
}

pub(super) fn tag_notes(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    mut child: Queue<notes::Request>,
) -> Requests {
    let room = notes::max_out(&env.limits.notes).checked_add(3).expect("bounded note route");
    let mut out = Queue::with_capacity(room);
    for _ in 0..child.len() {
        match child.pop().expect("note output count") {
            notes::Request::Save { record } => out.push(Request::Write(Write::Save(Record::Notes(record)))),
            notes::Request::Erase { key } => out.push(Request::Write(Write::Erase(Key::Notes(key)))),
            notes::Request::Load { owner, range } => {
                out.push(Request::Held(Box::new(Held::NotesLoad { owner, range })));
            }
            notes::Request::Written { owner, name, revision } => match core.note_routes.remove(&owner) {
                Some(NoteRoute::Call { to, key }) => {
                    call_decision(core, &mut out, to, key, CallPart::NoteWritten { name, revision });
                }
                Some(NoteRoute::Person { request }) => work.push(Event::People(people::Event::Decided {
                    request,
                    outcome: people::Outcome::NoteEdited { name },
                })),
                Some(NoteRoute::Brief { .. }) => unreachable!("an index cannot write an entry"),
                None => unreachable!("a note write has a waiting caller"),
            },
            notes::Request::Deleted { owner, name } => match core.note_routes.remove(&owner) {
                Some(NoteRoute::Person { request }) => work.push(Event::People(people::Event::Decided {
                    request,
                    outcome: people::Outcome::NoteEdited { name },
                })),
                Some(NoteRoute::Call { .. } | NoteRoute::Brief { .. }) | None => {
                    unreachable!("only a party deletes a note")
                }
            },
            notes::Request::Recalled { owner, entries, more } => match core.note_routes.remove(&owner) {
                Some(NoteRoute::Call { to, key }) => {
                    let part = recalled_part(core, key.task, entries, more);
                    call_decision(core, &mut out, to, key, part);
                }
                Some(NoteRoute::Person { .. } | NoteRoute::Brief { .. }) | None => {
                    unreachable!("a run owns this recall")
                }
            },
            notes::Request::Indexed { owner, lines, more } => match core.note_routes.remove(&owner) {
                Some(NoteRoute::Brief { task }) => work.push(Event::BriefNotes { task, lines, more }),
                Some(NoteRoute::Call { .. } | NoteRoute::Person { .. }) | None => {
                    unreachable!("brief preparation owns note index")
                }
            },
            notes::Request::Refused { owner, why, limit } => match core.note_routes.remove(&owner) {
                Some(NoteRoute::Call { to, key }) => match why {
                    notes::Refusal::Busy => {
                        let _pending = core.pending_calls.remove(&key);
                        out.push(Request::Now(Box::new(Now::NoteBusy { to, key })));
                    }
                    notes::Refusal::Oversized
                    | notes::Refusal::Full
                    | notes::Refusal::Exists
                    | notes::Refusal::Missing
                    | notes::Refusal::Moved { .. }
                    | notes::Refusal::RevisionExhausted => {
                        call_decision(core, &mut out, to, key, CallPart::NoteRefused(why));
                    }
                },
                Some(NoteRoute::Person { request }) => {
                    work.push(Event::People(people::Event::Decided {
                        request,
                        outcome: note_person_refusal(why, limit),
                    }));
                }
                Some(NoteRoute::Brief { task }) => match why {
                    notes::Refusal::Busy => {
                        if core.notes.busy() {
                            core.pending_note_briefs
                                .insert(task, NoteBrief::Operation)
                                .expect("brief count bounds waiting notes");
                        } else {
                            defer_brief(core, work, task);
                        }
                    }
                    notes::Refusal::Oversized
                    | notes::Refusal::Full
                    | notes::Refusal::Exists
                    | notes::Refusal::Missing
                    | notes::Refusal::Moved { .. }
                    | notes::Refusal::RevisionExhausted => {
                        work.push(Event::Tasks(tasks::Event::PreparationFailed { task }));
                    }
                },
                None => out.push(Request::Now(Box::new(Now::NotesRefused { owner, why }))),
            },
        }
    }
    retry_brief(core, work);
    out.push(Request::Decided);
    Requests::Out(out)
}

fn defer_brief(core: &mut Core, work: &mut Queue<Event>, task: u64) {
    super::result_reads::clear(core, task);
    drop(core.contexts.remove(&task));
    drop(core.dependency_results.remove(&task));
    drop(core.transcripts.remove(&task));
    core.pending_note_briefs.insert(task, NoteBrief::Activation).expect("brief count bounds deferred activations");
    work.push(Event::Tasks(tasks::Event::PreparationWaiting { task }));
}

pub(super) fn recalled_part(core: &Core, task: u64, entries: List<notes::Entry>, more: bool) -> CallPart {
    for entry in &entries {
        if !core.note_readable(task, &entry.scope) {
            return CallPart::ToolDenied {
                answer: authority::Answer::Refuse,
                findings: Box::new([authority::Finding::Scope { source: authority::Source::Task }]),
            };
        }
    }
    CallPart::NoteRecalled { entries: entries.into_boxed(), more }
}

#[expect(clippy::too_many_arguments, reason = "the route retains the admitted party and request fields")]
pub(super) fn edit_note_request(
    core: &mut Core,
    work: &mut Queue<Event>,
    request: Token,
    person: u64,
    project: u32,
    _role: Option<people::Role>,
    name: u64,
    scope: Box<people::NoteScope>,
    change: people::NoteChange,
) {
    let act = match &change {
        people::NoteChange::Correct { .. } => crate::acts::Act::EditNote,
        people::NoteChange::Delete { .. } => crate::acts::Act::DeleteNote,
    };
    if let Err(why) = crate::acts::note(core, person, project, &scope, act) {
        work.push(Event::People(people::Event::Decided { request, outcome: people::Outcome::Refused(why) }));
    } else {
        let owner = core.begin_note_route(NoteRoute::Person { request });
        let scope = person_note_scope(project, *scope);
        let change = person_note_change(change);
        work.push(Event::Notes(notes::Event::Edit { owner, party: person, name, scope, change }));
    }
}

fn retry_brief(core: &mut Core, work: &mut Queue<Event>) {
    if !core.notes.busy() {
        let mut waiting = None;
        for (&task, state) in &core.pending_note_briefs {
            if *state == NoteBrief::Operation {
                waiting = Some(task);
                break;
            }
        }
        if let Some(task) = waiting {
            let _pending = core.pending_note_briefs.remove(&task);
            work.push(Event::StartBrief { task });
        }
    }
}
