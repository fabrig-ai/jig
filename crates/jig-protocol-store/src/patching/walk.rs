//! One record's stopped conversion, including held records (protocol/store.md 10.3).
use super::machines::{FamilyIx, Machines};
use super::plan::Planned;
use alloc::boxed::Box;
use skein_codec::patch::{Halt, Patcher, PlanIx, Site, Step};
use skein_codec::schema::{DeclIx, Span};
use skein_codec::value::{Node, Part, Path, Problem, Refusal};
use skein_codec::{Reason, version_of};
use skein_lib::{List, bytes};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Stop {
    Site { family: FamilyIx, site: Site, at: Node },
    Done,
}
#[derive(Clone, Copy, Debug)]
struct Frame {
    family: FamilyIx,
    plan: PlanIx,
    root: DeclIx,
    /// Held records run until the family's version after the step; the outer record
    /// runs just this step, including its identity when it has no block.
    target: u16,
    /// At most one stop per old node, then Done, per pass across all resumes.
    calls: u64,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    Running,
    Waiting,
    Done,
    Halted,
}
#[derive(Debug)]
pub(crate) struct Walk {
    holder: FamilyIx,
    step: Planned,
    frames: List<Frame>,
    state: State,
}

pub(crate) fn start(
    machines: &mut Machines,
    step: Planned,
    family: FamilyIx,
    root: &str,
    record: Box<[u8]>,
) -> Result<(Walk, Stop), Halt> {
    let machine = machines.get(family);
    let version = machine.reached;
    let plan = match machine.block(step) {
        Some(plan) => plan,
        None => machine.identity(version).ok_or(Halt::Order)?,
    };
    let patcher = machines.patcher(family);
    let schema = patcher.schema(version).ok_or(Halt::Order)?;
    let root = schema.find(root.as_bytes()).ok_or(Halt::Order)?;
    let versions = patcher.versions(plan);
    if versions.from != version {
        return Err(Halt::Order);
    }
    let target = versions.to;
    patcher.start(plan, root, record)?;
    let mut frames = List::with_capacity(machines.len());
    frames.push(Frame { family, plan, root, target, calls: 0 }).expect("one holder frame");
    let mut walk = Walk { holder: family, step, frames, state: State::Running };
    let stopped = settle(machines, &mut walk)?;
    Ok((walk, stopped))
}

pub(crate) fn resume(machines: &mut Machines, walk: &mut Walk, result: Result<(), Halt>) -> Result<Stop, Halt> {
    if walk.state != State::Waiting {
        return Err(Halt::Order);
    }
    if let Err(halt) = result {
        walk.state = State::Halted;
        return Err(halt);
    }
    walk.state = State::Running;
    settle(machines, walk)
}
fn settle(machines: &mut Machines, walk: &mut Walk) -> Result<Stop, Halt> {
    let result = drive(machines, walk);
    if result.is_err() {
        walk.state = State::Halted;
    }
    result
}
fn drive(machines: &mut Machines, walk: &mut Walk) -> Result<Stop, Halt> {
    // No active family repeats. Each family's nodes bound its calls in a
    // pass, and each held record runs at most its family's kept versions.
    // Sum the families' own bounds before taking the bounded call tree's
    // product. Saturation still leaves a finite bound, as before; the sum
    // is checked and an unrepresentable family bound halts Room.
    let mut calls = 0_u64;
    for ix in 0..machines.len() {
        let family = FamilyIx(ix);
        let nodes = u64::from(machines.room(family).nodes).checked_add(1).ok_or(Halt::Room)?;
        let passes = u64::from(machines.get(family).identities.len());
        calls = calls.checked_add(nodes.checked_mul(passes).ok_or(Halt::Room)?).ok_or(Halt::Room)?;
    }
    let mut turns = calls;
    for _ in 0..machines.len() {
        turns = turns.saturating_mul(calls.saturating_add(1));
    }
    for _ in 0..turns {
        let top = walk.frames.len().checked_sub(1).ok_or(Halt::Order)?;
        let frame = walk.frames.get_mut(top).expect("top frame exists");
        if frame.calls > u64::from(machines.room(frame.family).nodes) {
            return Err(Halt::Room);
        }
        frame.calls = frame.calls.checked_add(1).expect("u32 node bound plus Done fits u64");
        let frame = *frame;
        match machines.patcher(frame.family).next()? {
            Step::Written { site, at } => {
                walk.state = State::Waiting;
                return Ok(Stop::Site { family: frame.family, site, at });
            }
            Step::Held { at, family, name } => held(machines, walk, frame.family, at, family, name)?,
            Step::Done => {
                let patcher = machines.patcher(frame.family);
                // next_pass requires finish even though the caller doesn't yet
                // need the bytes of an intermediate held pass.
                let _: &[u8] = patcher.finish()?;
                let version = patcher.versions(frame.plan).to;
                if version < frame.target {
                    let root = patcher.counterpart(frame.plan, frame.root).ok_or(Halt::Order)?;
                    let plan = patcher.plan_from(version).ok_or(Halt::Order)?;
                    patcher.next_pass(plan, root)?;
                    *walk.frames.get_mut(top).expect("top frame") = Frame { plan, root, calls: 0, ..frame };
                    continue;
                }
                if top == 0 {
                    walk.state = State::Done;
                    return Ok(Stop::Done);
                }
                // Move the finished machine out temporarily to borrow its bytes
                // while the waiting holder puts them. No extra output box.
                let mut patcher = take(machines, frame.family);
                let holder_ix = top.checked_sub(1).expect("held frame has a holder");
                let holder = walk.frames.get(holder_ix).expect("holder frame").family;
                let result = machines.patcher(holder).parts().1.put_content(patcher.finish()?);
                give(machines, frame.family, patcher);
                result?;
                walk.frames.truncate(top);
            }
        }
    }
    Err(Halt::Room)
}

fn held(
    machines: &mut Machines,
    walk: &mut Walk,
    holder: FamilyIx,
    at: Node,
    family: Span,
    name: Span,
) -> Result<(), Halt> {
    let held = {
        let patcher = machines.get(holder).patcher.as_ref().expect("holder waits in the component");
        machines.named(patcher.spelled(family)).ok_or(Halt::Misread(at))?
    };
    for frame in &walk.frames {
        if frame.family == held {
            // The same machine cannot hold two active records. This includes
            // holding itself indirectly, and halts with the library's Order.
            return Err(Halt::Order);
        }
    }
    let reached = machines.get(held).after(walk.step);
    let version = {
        let (old, _) = machines.patcher(holder).parts();
        match version_of(content(old, at)?) {
            Ok(version) => version,
            Err(_) => return Err(Halt::Misread(at)),
        }
    };
    if version == reached {
        let (old, new) = machines.patcher(holder).parts();
        return new.carry(old, at);
    }
    // The holder's spans remain borrowed from its machine while the held
    // machine resolves the root and plans. Move it out for the split borrow.
    let holder_patcher = take(machines, holder);
    let root = held_root(machines.patcher(held), version, reached, holder_patcher.spelled(name));
    give(machines, holder, holder_patcher);
    let root = root?;
    if version > reached {
        return Err(Halt::Decode(Refusal::Problem(Problem {
            path: Path { decl: root, part: Part::Version },
            reason: Reason::Version,
        })));
    }
    let record = {
        let (old, _) = machines.patcher(holder).parts();
        bytes::copy_of(content(old, at)?)
    };
    let patcher = machines.patcher(held);
    let plan = patcher.plan_from(version).ok_or(Halt::Order)?;
    patcher.start(plan, root, record)?;
    walk.frames
        .push(Frame { family: held, plan, root, target: reached, calls: 0 })
        .expect("no active family twice: stack bounded by machine count");
    Ok(())
}
pub(crate) fn held_root(patcher: &Patcher, version: u16, reached: u16, name: &[u8]) -> Result<DeclIx, Halt> {
    if let Some(schema) = patcher.schema(version)
        && let Some(root) = schema.find(name)
    {
        return Ok(root);
    }
    let mut root = patcher.schema(reached).ok_or(Halt::Order)?.find(name).ok_or(Halt::Order)?;
    // A held name may spell the reached declaration after a rename. Walk
    // sources back to the input version; forward passes use counterparts.
    let passes = reached.checked_sub(version).ok_or(Halt::Order)?;
    for offset in 0..passes {
        let from = reached.checked_sub(offset).expect("offset below reached").checked_sub(1).expect("previous version");
        let plan = patcher.plan_from(from).ok_or(Halt::Order)?;
        root = patcher.source(plan, root).ok_or(Halt::Order)?;
    }
    Ok(root)
}

pub(crate) fn finished<'a>(machines: &'a mut Machines, walk: &Walk) -> Result<&'a [u8], Halt> {
    if walk.state != State::Done {
        return Err(Halt::Order);
    }
    machines.patcher(walk.holder).finish()
}
pub(crate) fn take(machines: &mut Machines, family: FamilyIx) -> Box<Patcher> {
    machines.get_mut(family).patcher.take().expect("patcher leaves only once at a site")
}
pub(crate) fn give(machines: &mut Machines, family: FamilyIx, patcher: Box<Patcher>) {
    let slot = &mut machines.get_mut(family).patcher;
    assert!(slot.is_none(), "the returning patcher was out at a site");
    *slot = Some(patcher);
}

/// Held bytes and held secrets share the walk, but use their own read accessors.
fn content(old: &skein_codec::patch::Old, at: Node) -> Result<&[u8], Halt> {
    match old.content(at) {
        Ok(bytes) => Ok(bytes),
        Err(_) => old.expose(at),
    }
}
