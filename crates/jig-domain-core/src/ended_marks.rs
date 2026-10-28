//! Held loads serialize marks over one ended conversation (people.md, 6).
use crate::{Core, Event, Limits, Request, Requests};
use jig_domain_people as people;
use skein_lib::{Env, Queue, Token};

#[derive(Debug)]
pub(crate) struct Loaded {
    pub task: u64,
    pub party: u64,
    pub project: u32,
    pub last_reply: u64,
    pub owner: Option<crate::forget::Owner>,
}

pub(crate) fn loaded(core: &mut Core, env: &Env<Limits>, work: &mut Queue<Event>, answer: Answer) -> Requests {
    let Answer { request, task, party, through, busy } = answer;
    let Some(mark) = core.ended_marks.remove(&request) else { return Requests::Out(Queue::with_capacity(0)) };
    let Some((person, people::Seen::Conversation { task: number, through: wanted })) =
        core.people.seen_context(request)
    else {
        return Requests::Out(Queue::with_capacity(0));
    };
    assert!(person == party && number == task && mark.task == task && mark.party == party, "correlated ended position");
    if busy {
        work.push(Event::People(people::Event::Decided {
            request,
            outcome: people::Outcome::Again(people::Again::Busy),
        }));
        let mut out = Queue::with_capacity(1);
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    let mut child = Queue::uncapped();
    core.people.mark_ended(
        &Env { now: env.now, wall: env.wall, limits: env.limits.people },
        people::EndedMark { request, project: mark.project, last_reply: mark.last_reply, seen: through },
        &mut child,
    );
    core.touch_party(party);
    if let Some(owner) = mark.owner
        && crate::forget::forgets(
            &owner,
            crate::forget::Reading {
                results: core.people.results_seen(party).unwrap_or(0),
                turns: Some(through.max(wanted)),
                named: core.tasks.stub(task).is_some(),
            },
        )
        .is_some()
    {
        let Requests::Out(mut erase) = crate::forget::forget(core, env, work, &owner);
        // Route the mark first, then its erase, in this decision.
        let Requests::Out(mut out) = crate::routing::tag_ended_mark(core, env, child, work);
        while let Some(request) = erase.pop() {
            out.push(request);
        }
        return Requests::Out(out);
    }
    crate::routing::tag_ended_mark(core, env, child, work)
}

pub(crate) struct Answer {
    pub request: Token,
    pub task: u64,
    pub party: u64,
    pub through: u64,
    pub busy: bool,
}
