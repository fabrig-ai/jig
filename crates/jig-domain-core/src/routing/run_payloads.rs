//! Turn and answer admission fences (domain/engine.md, 7.2 and 7.4).
use crate::{Core, Event, Request, Requests};
use alloc::boxed::Box;
use jig_domain_tasks as tasks;
use skein_lib::{Queue, ReplyTo, Token};

#[expect(clippy::too_many_arguments, reason = "the route retains its event fields")]
pub(super) fn route_answer_payload(
    core: &mut Core,
    work: &mut Queue<Event>,
    run: Token,
    attempt: Token,
    payload: Token,
    cumulative: u64,
    end: tasks::End,
    saved: Option<Box<[tasks::SavedResource]>>,
    invalid_saved: bool,
    read: Option<u64>,
    refused: &[u64],
) -> Requests {
    if !core.current_proof(run.raw(), attempt.raw()) {
        let mut out = Queue::with_capacity(2);
        out.push(Request::Now(Box::new(crate::Now::DropPayload { payload })));
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    let (cumulative, end, saved) = match core.committed_terminal(run.raw(), attempt.raw()) {
        Some(terminal) => (terminal.cumulative, terminal.end, None),
        None => {
            let proof = core.proofs.get(&run.raw()).expect("current proof");
            let last_read = match proof.turn {
                Some(turn) => turn.read,
                None => None,
            };
            let mut invalid = read > last_read;
            for number in refused {
                invalid |= *number == 0 || Some(*number) > proof.offered;
            }
            if invalid_saved || invalid {
                (cumulative, tasks::End::Failed(tasks::Class::Invalid), None)
            } else {
                if !refused.is_empty() {
                    work.push(Event::Tasks(tasks::Event::Refusals {
                        task: run.raw(),
                        attempt: attempt.raw(),
                        refused: Box::from(refused),
                        offered: proof.offered,
                    }));
                }
                (cumulative, end, saved)
            }
        }
    };
    let proof = core.proofs.get(&run.raw()).expect("current proof");
    let account_wait = proof.account_terminal
        || match proof.account_failure {
            Some(source) => match core.accounts.source(source) {
                Some(fact) => !fact.usable,
                None => true,
            },
            None => false,
        };
    let proof = core.proofs.get_mut(&run.raw()).expect("terminal proof pre-reserved");
    let end = match end {
        tasks::End::Failed(tasks::Class::Transient | tasks::Class::Permanent | tasks::Class::Run) if account_wait => {
            tasks::End::Recheck
        }
        tasks::End::Finished { .. }
        | tasks::End::FinishedWithFollowups { .. }
        | tasks::End::Exceeded(_)
        | tasks::End::Parked
        | tasks::End::Failed(_)
        | tasks::End::Refused
        | tasks::End::Recheck => end,
    };
    proof.terminal =
        Some(crate::TerminalRecord { task: run.raw(), attempt: attempt.raw(), cumulative, end: end.clone() });
    work.push(Event::Tasks(tasks::Event::Activation {
        reply_to: ReplyTo::new(payload),
        task: run.raw(),
        attempt: attempt.raw(),
        cause: tasks::Cause::Priced { cumulative },
        end,
        saved,
    }));
    let mut out = Queue::with_capacity(1);
    out.push(Request::Decided);
    Requests::Out(out)
}

#[expect(clippy::too_many_arguments, reason = "the route retains its event fields")]
pub(super) fn route_turn_payload(
    core: &mut Core,
    work: &mut Queue<Event>,
    run: Token,
    attempt: Token,
    turn: u32,
    body: Token,
    read: Option<u64>,
    refused: Box<[u64]>,
    cumulative: u64,
) -> Requests {
    assert!(core.current_proof(run.raw(), attempt.raw()), "current turn has reserved proof");
    let proof = core.proofs.get(&run.raw()).expect("current proof");
    let offered = proof.offered;
    let last_read = match proof.turn {
        Some(turn) => turn.read,
        None => None,
    };
    if read < last_read || read > last_read && !proof.outstanding.contains(&read.expect("new read is present")) {
        let mut out = Queue::with_capacity(2);
        out.push(Request::Now(Box::new(crate::Now::RefusedPayload {
            request: body,
            problem: tasks::Problem {
                task: Some(run.raw()),
                why: tasks::Refusal::Read,
                blocked_by: None,
                capacity: None,
            },
        })));
        out.push(Request::Decided);
        return Requests::Out(out);
    }
    // The parent retains the original fence with the accepted-turn payload.
    let read = if read == last_read { None } else { read };
    work.push(Event::Tasks(tasks::Event::Turn {
        reply_to: ReplyTo::new(body),
        task: run.raw(),
        attempt: attempt.raw(),
        turn,
        read,
        refused,
        offered,
        outstanding: proof.outstanding.clone(),
        cumulative,
    }));
    let mut out = Queue::with_capacity(1);
    out.push(Request::Decided);
    Requests::Out(out)
}
