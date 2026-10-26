//! Durable projection episodes and live news (domain/connectors.md, 8;
//! domain/engine.md, 5.4). Begin alone creates a receipt. Recovery applies a
//! staged connector reset and advances its fence in one synchronous decision.
use crate::{
    Ask, Core, CoreKey, CoreRecord, Family, Held, Key, Limits, ProjectionKey, ProjectionRecord, Record, Request,
    Requests, Write, fresh, routing::result_reads,
};
use alloc::boxed::Box;
use jig_domain_tasks as tasks;
use skein_lib::{Decimal, Env, Queue, Token, Wall, Writer};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ProjectionFailureSource {
    pub source: u64,
    pub at: Wall,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProjectionFailureState {
    Armed,
    Pending(ProjectionFailureSource),
    Reported(ProjectionFailureSource),
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ProjectionFailure {
    pub goal: u64,
    pub connector: u16,
    pub after: u64,
    pub state: ProjectionFailureState,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProjectionRecovery {
    Recovered { after: u64 },
    Busy,
    Missing,
}

pub(crate) fn member(core: &Core, goal: u64, connector: u16) -> bool {
    if !core.connectors.contains(&connector) {
        return false;
    }
    let Some(state) = core.projections.get(&goal) else { return false };
    state.closing_at.is_none() || state.pending.contains(&connector)
}
fn save(out: &mut Queue<Request>, row: ProjectionFailure) {
    out.push(Request::Write(Write::Save(Record::Core(CoreRecord::Projection(ProjectionRecord::Failure(row))))));
}
pub(crate) fn begin(core: &mut Core, goal: u64, connector: u16, owner: Token) -> Requests {
    let mut out = Queue::with_capacity(3);
    let key = (goal, connector);
    let source = if member(core, goal, connector)
        && (core.projection_failures.contains_key(&key)
            || core.projection_failures.len() < core.projection_failures.capacity())
        && core.counters.deployment().connector_rows != u64::MAX
    {
        if !core.projection_failures.contains_key(&key) {
            let row = ProjectionFailure {
                goal,
                connector,
                after: core.counters.deployment().connector_rows,
                state: ProjectionFailureState::Armed,
            };
            core.projection_failures.insert(key, row).expect("receipt pair room checked");
            save(&mut out, row);
        }
        fresh(&mut core.counters, Family::ConnectorRow)
    } else {
        None
    };
    out.push(Request::Ask { connector, ask: Ask::ProjectionBegun { owner, source } });
    out.push(Request::Decided);
    Requests::Out(out)
}
pub(crate) fn current(core: &Core, goal: u64, connector: u16, source: u64) -> bool {
    member(core, goal, connector)
        && source != 0
        && source <= core.counters.deployment().connector_rows
        && match core.projection_failures.get(&(goal, connector)) {
            Some(row) => source > row.after,
            None => false,
        }
}
pub(crate) fn failed(core: &mut Core, goal: u64, connector: u16, source: u64, at: Wall, out: &mut Queue<Request>) {
    if !current(core, goal, connector, source) {
        return;
    }
    let row = core.projection_failures.get_mut(&(goal, connector)).expect("current receipt");
    match row.state {
        ProjectionFailureState::Armed => {
            row.state = ProjectionFailureState::Pending(ProjectionFailureSource { source, at });
            save(out, *row);
        }
        ProjectionFailureState::Pending(_) | ProjectionFailureState::Reported(_) => {}
    }
}
pub(crate) fn recover(core: &mut Core, goal: u64, connector: u16, owner: Token) -> Requests {
    let mut out = Queue::with_capacity(4);
    let result = if member(core, goal, connector) {
        match core.projection_failures.get_mut(&(goal, connector)) {
            None => ProjectionRecovery::Missing,
            Some(row) => match row.state {
                ProjectionFailureState::Pending(_) => ProjectionRecovery::Busy,
                ProjectionFailureState::Armed => ProjectionRecovery::Recovered { after: row.after },
                ProjectionFailureState::Reported(_) => {
                    row.after = core.counters.deployment().connector_rows;
                    row.state = ProjectionFailureState::Armed;
                    let after = row.after;
                    save(&mut out, *row);
                    out.push(Request::Ask { connector, ask: Ask::CommitProjectionRecovery { goal, owner, after } });
                    ProjectionRecovery::Recovered { after }
                }
            },
        }
    } else {
        ProjectionRecovery::Missing
    };
    out.push(Request::Held(Box::new(Held::ProjectionRecovery { connector, owner, result })));
    out.push(Request::Decided);
    Requests::Out(out)
}
fn can_deliver(core: &Core) -> bool {
    !core.stopping
        && core.restart == crate::restart::Restart::Open
        && core.counters.deployment().messages != u64::MAX
        && core.projection_news_at != Some(core.counters.deployment().commits)
}
fn news_bytes(connector: u16) -> u32 {
    18_u32
        .checked_add(u32::try_from(Decimal::of(u64::from(connector)).as_bytes().len()).expect("connector digits"))
        .expect("news fits")
}
pub(crate) fn owed(core: &Core) -> bool {
    if !can_deliver(core) {
        return false;
    }
    for (_, row) in &core.projection_failures {
        if match row.state {
            ProjectionFailureState::Pending(_) => true,
            ProjectionFailureState::Armed | ProjectionFailureState::Reported(_) => false,
        } && (core.tasks.projection_news_room(&core.projection_task_limits, row.goal, news_bytes(row.connector))
            || (core.tasks.task(row.goal).is_none() && result_reads::news_room(core)))
        {
            return true;
        }
    }
    false
}
pub(crate) fn deliver(core: &mut Core, env: &Env<Limits>, out: &mut Queue<Request>) {
    if !can_deliver(core) {
        return;
    }
    let mut chosen = None;
    for (key, row) in &core.projection_failures {
        match row.state {
            ProjectionFailureState::Pending(source)
                if core.tasks.projection_news_room(&env.limits.tasks, row.goal, news_bytes(row.connector)) =>
            {
                chosen = Some((*key, source));
                break;
            }
            ProjectionFailureState::Armed
            | ProjectionFailureState::Pending(_)
            | ProjectionFailureState::Reported(_) => {}
        }
    }
    let Some(((goal, connector), source)) = chosen else {
        if result_reads::news_room(core) {
            let mut archived = None;
            for (_, row) in &core.projection_failures {
                match row.state {
                    ProjectionFailureState::Pending(_) if core.tasks.task(row.goal).is_none() => {
                        archived = Some((row.goal, row.connector));
                        break;
                    }
                    ProjectionFailureState::Armed
                    | ProjectionFailureState::Pending(_)
                    | ProjectionFailureState::Reported(_) => {}
                }
            }
            if let Some((goal, connector)) = archived {
                result_reads::news(core, goal, connector, out);
            }
        }
        return;
    };
    let number = fresh(&mut core.counters, Family::Message).expect("news counter checked");
    let word = news_word(goal, connector, number, source);
    let room = tasks::max_out(&env.limits.tasks);
    let mut child = Queue::with_capacity(room);
    let result = tasks::projection_news(
        &mut core.tasks,
        &Env { now: env.now, wall: env.wall, limits: env.limits.tasks },
        goal,
        word,
        &mut child,
    );
    assert!(result == tasks::ProjectionNewsResult::Delivered, "room and delivery share admission");
    route_news(core, env, child, room, out);
    reported(core, goal, connector, source, out);
}

fn route_news(core: &mut Core, env: &Env<Limits>, child: Queue<tasks::Request>, room: u32, out: &mut Queue<Request>) {
    let Requests::Out(mut tagged) = crate::routing::task_routes::tag_tasks(core, env, child, room, false, false, false);
    for _ in 0..tagged.len() {
        match tagged.pop().expect("tagged news") {
            Request::Decided => {}
            request @ (Request::Write(_)
            | Request::Held(_)
            | Request::Now(_)
            | Request::Ask { .. }
            | Request::ProjectMade { .. }) => out.push(request),
        }
    }
}

fn news_word(goal: u64, connector: u16, number: u64, source: ProjectionFailureSource) -> tasks::Word {
    let digits = Decimal::of(u64::from(connector));
    let mut words = Writer::new(usize::try_from(news_bytes(connector)).expect("small news"));
    words.put(b"Projection ").expect("measured prefix");
    words.put(digits.as_bytes()).expect("measured connector");
    words.put(b" failed").expect("measured suffix");
    tasks::Word {
        number,
        from: tasks::Party::Task(goal),
        kind: tasks::MessageKind::News { subscription: 0, class: tasks::NewsClass::Kept },
        words: words.finish(),
        at: source.at,
        hits: 1,
        eligible: false,
    }
}

fn reported(core: &mut Core, goal: u64, connector: u16, source: ProjectionFailureSource, out: &mut Queue<Request>) {
    let row = core.projection_failures.get_mut(&(goal, connector)).expect("pending receipt retained");
    row.state = ProjectionFailureState::Reported(source);
    save(out, *row);
    core.projection_news_at = Some(core.counters.deployment().commits);
    crate::projections::finish_forget(core, goal, out);
}

pub(crate) fn archived(
    core: &mut Core,
    env: &Env<Limits>,
    goal: u64,
    connector: u16,
    row: Option<Box<tasks::TaskRecord>>,
    busy: bool,
    out: &mut Queue<Request>,
) {
    // Even a busy answer must not start another read in its answering decision.
    let deliverable = can_deliver(core);
    core.projection_news_at = Some(core.counters.deployment().commits);
    if busy || !deliverable {
        return;
    }
    let Some(receipt) = core.projection_failures.get(&(goal, connector)) else { return };
    let source = match receipt.state {
        ProjectionFailureState::Pending(source) => source,
        ProjectionFailureState::Armed | ProjectionFailureState::Reported(_) => return,
    };
    if let Some(mut task) = row {
        assert!(task.number == goal, "ended load names its requested owner");
        let number = fresh(&mut core.counters, Family::Message).expect("news counter checked");
        let word = news_word(goal, connector, number, source);
        let mut child = Queue::with_capacity(1);
        match tasks::archived_projection_news(&mut task, &env.limits.tasks, word, &mut child) {
            tasks::ProjectionNewsResult::Delivered => {
                route_news(core, env, child, 1, out);
                crate::routing::watch_marks::touch_task(core, &env.limits, &task);
                // This is an in-place header update, not the ending route that
                // allocates a result position and publishes its indexes.
                out.push(Request::Write(Write::Save(Record::Tasks(tasks::Stored::Ended(task)))));
            }
            tasks::ProjectionNewsResult::Full | tasks::ProjectionNewsResult::Missing => {}
            tasks::ProjectionNewsResult::Invalid => unreachable!("core constructs valid projection news"),
        }
    }
    reported(core, goal, connector, source, out);
}

pub(crate) fn forgotten(core: &mut Core, goal: u64, out: &mut Queue<Request>) {
    result_reads::forget_news(core, goal);
    for &connector in core.connectors.as_ref() {
        if let Some(row) = core.projection_failures.get_mut(&(goal, connector)) {
            match row.state {
                ProjectionFailureState::Pending(source) => {
                    row.state = ProjectionFailureState::Reported(source);
                    save(out, *row);
                }
                ProjectionFailureState::Armed | ProjectionFailureState::Reported(_) => {}
            }
        }
    }
    crate::projections::finish_forget(core, goal, out);
}
pub(crate) fn pending(core: &Core, goal: u64) -> bool {
    for (_, row) in &core.projection_failures {
        if row.goal == goal
            && match row.state {
                ProjectionFailureState::Pending(_) => true,
                ProjectionFailureState::Armed | ProjectionFailureState::Reported(_) => false,
            }
        {
            return true;
        }
    }
    false
}
pub(crate) fn erase(core: &mut Core, goal: u64, out: &mut Queue<Request>) {
    for &connector in core.connectors.as_ref() {
        if core.projection_failures.remove(&(goal, connector)).is_some() {
            out.push(Request::Write(Write::Erase(Key::Core(CoreKey::Projection(ProjectionKey::Failure {
                goal,
                connector,
            })))));
        }
    }
}
pub(crate) fn restore(core: &mut Core, row: ProjectionFailure) -> bool {
    let maximum = core.counters.deployment().connector_rows;
    let valid = match row.state {
        ProjectionFailureState::Armed => true,
        ProjectionFailureState::Pending(source) | ProjectionFailureState::Reported(source) => {
            source.source > row.after && source.source <= maximum
        }
    };
    let key = (row.goal, row.connector);
    valid
        && row.after <= maximum
        && core.connectors.contains(&row.connector)
        && core.projections.contains_key(&row.goal)
        && !core.projection_failures.contains_key(&key)
        && core.projection_failures.insert(key, row).is_ok()
}
pub(crate) fn consistent(core: &Core) -> bool {
    for (goal, state) in &core.projections {
        if state.closing_at.is_some()
            && (!crate::projections::final_owner(core.tasks.task(*goal))
                || !state.milestones.is_empty()
                || (state.pending.is_empty() && !pending(core, *goal)))
        {
            return false;
        }
    }
    true
}
