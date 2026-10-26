//! The pin belongs to the successful claim, separately from the creator's preference.
#![allow(
    clippy::disallowed_macros,
    clippy::disallowed_types,
    reason = "test scripts are ordinary Rust under programming-model.md section 10.2"
)]
use super::revisions::{Harness, task, to};
use crate::{Event, LlmChoice, ModelEffort, Models};
const TASK: u64 = 1;
const PREFERENCE: u32 = 41;
const SELECTED: u32 = 42;
const REJECTED: u32 = 43;

fn preparing() -> Harness {
    let mut h = Harness::new();
    let mut new = task(TASK, &[]);
    new.authority.models = Models::Named(1);
    new.llm = Some(LlmChoice {
        account: Some(PREFERENCE),
        main: ModelEffort { model: 0, effort: None },
        subagents: Box::new([]),
    });
    h.make(vec![new]);
    h.send(Event::Prepare { reply_to: to(), task: TASK });
    h
}
fn claim(h: &mut Harness, attempt: u64, account: u32, budget: u64) {
    h.send(Event::Claim {
        reply_to: to(),
        task: TASK,
        attempt,
        resumed: false,
        budget,
        account: Some(account),
        writes: Box::new([]),
    });
}

#[test]
fn only_a_successful_claim_keeps_its_account() {
    let mut h = preparing();
    claim(&mut h, 1, REJECTED, 0);
    assert_eq!(h.domain.task(TASK).unwrap().kept_account, None);
    claim(&mut h, 1, SELECTED, 10);
    let claimed = h.domain.task(TASK).unwrap().clone();
    claim(&mut h, 1, REJECTED, 10);
    assert_eq!(h.domain.task(TASK).unwrap(), &claimed);
    assert_eq!(claimed.kept_account, Some(SELECTED));
    assert_eq!(claimed.llm.as_ref().unwrap().account, Some(PREFERENCE));
}

#[test]
fn kept_account_survives_restore_separately_from_preference() {
    let mut h = preparing();
    claim(&mut h, 1, SELECTED, 10);
    h.restart();
    let restored = h.domain.task(TASK).unwrap();
    assert_eq!(restored.kept_account, Some(SELECTED));
    assert_eq!(restored.llm.as_ref().unwrap().account, Some(PREFERENCE));
    assert_eq!((restored.attempt, restored.run_reserved, restored.numbers.reserved), (1, 10, 10));
}

#[test]
fn an_unclaimed_task_cannot_restore_a_kept_account() {
    let h = preparing();
    let mut row = h.domain.task(TASK).unwrap().clone();
    row.kept_account = Some(SELECTED);
    let mut fresh = crate::Domain::new(&h.env.limits, 91, 1, crate::tests::charters(), Box::new([0]));
    let mut out = skein_lib::Queue::with_capacity(crate::max_out(&h.env.limits));
    crate::step(&mut fresh, &h.env, Event::Restore { record: crate::Stored::Live(Box::new(row)) }, &mut out);
    let Some(crate::Request::RestoreRefused { .. }) = out.pop() else {
        panic!("an unclaimed account pin must be refused");
    };
}

#[test]
fn an_account_wait_releases_reserve_and_charges_once_without_a_try() {
    let mut h = preparing();
    claim(&mut h, 1, SELECTED, 10);
    let terminal = || Event::Activation {
        reply_to: to(),
        task: TASK,
        attempt: 1,
        cause: crate::Cause::Priced { cumulative: 3 },
        end: crate::End::Recheck,
        saved: None,
    };
    h.send(terminal());
    h.send(terminal());
    let row = h.domain.task(TASK).unwrap();
    assert_eq!((row.run_reserved, row.numbers.reserved, row.numbers.spent), (0, 0, 3));
    assert_eq!(row.tries, crate::Tries::NONE);
    assert_eq!(row.phase, crate::Phase::Active(crate::Active::Due));
    assert_eq!(row.kept_account, Some(SELECTED));
    assert_eq!(h.domain.next_deadline(), None);
}

fn amend_choice(h: &mut Harness, account: Option<u32>) {
    let row = h.domain.task(TASK).unwrap();
    let message = row.last_message.checked_add(1).expect("test message room");
    let mut choice = row.llm.clone().unwrap();
    choice.account = account;
    h.send(Event::Amend {
        places: Box::new([]),
        funder: None,
        reply_to: to(),
        by: crate::Party::Person(9),
        task: TASK,
        message,
        stop_run: false,
        amendment: crate::Amendment {
            tracked: None,
            addressed: None,
            instructions: None,
            spec: None,
            wake: None,
            dependencies: None,
            authority: None,
            llm: Some(choice),
            release: None,
            reason: Box::new([]),
        },
    });
}

fn amended() -> (Harness, u64) {
    let mut h = preparing();
    claim(&mut h, 1, SELECTED, 10);
    let revision = h.domain.task(TASK).unwrap().revisions.spec;
    // The preference already names PREFERENCE; replacing the pin alone is a real edit.
    amend_choice(&mut h, Some(PREFERENCE));
    (h, revision)
}

#[test]
fn an_explicit_different_account_clears_the_pin_in_the_amendment() {
    let (h, _) = amended();
    assert_eq!(h.domain.task(TASK).unwrap().kept_account, None);
}

#[test]
fn replacing_a_pin_advances_the_spec_revision() {
    let (h, revision) = amended();
    assert_eq!(h.domain.task(TASK).unwrap().revisions.spec, revision.checked_add(1).expect("one amendment"));
}

#[test]
fn replacing_a_pin_retains_the_live_reservation() {
    let (h, _) = amended();
    assert_eq!(h.domain.task(TASK).unwrap().run_reserved, 10);
}

#[test]
fn a_cleared_account_pin_survives_restore() {
    let (mut h, _) = amended();
    h.restart();
    assert_eq!(h.domain.task(TASK).unwrap().kept_account, None);
}

#[test]
fn the_next_claim_keeps_the_replacement_account() {
    let (mut h, _) = amended();
    h.send(Event::Activation {
        reply_to: to(),
        task: TASK,
        attempt: 1,
        cause: crate::Cause::Priced { cumulative: 0 },
        end: crate::End::Recheck,
        saved: None,
    });
    h.send(Event::Prepare { reply_to: to(), task: TASK });
    claim(&mut h, 2, PREFERENCE, 10);
    assert_eq!(h.domain.task(TASK).unwrap().kept_account, Some(PREFERENCE));
}

#[test]
fn clearing_the_preference_keeps_the_claimed_account() {
    let mut h = preparing();
    claim(&mut h, 1, SELECTED, 10);
    amend_choice(&mut h, None);
    assert_eq!(h.domain.task(TASK).unwrap().kept_account, Some(SELECTED));
}
