//! Generations follow the fleet's live claims and their worker's hello.
use super::{A1, A2, C1, Harness, LIMITS, R1, R2, fixtures, hosted, references, to};
use crate::{Event, Grant, GrantName, HostKind, Kinds, Phase, Request};
use fixtures::{ANALYST_FIRST, ANALYST_SECOND, OTHER_ACCOUNT};
use skein_lib::Token;

impl Harness {
    fn start_granted(&mut self, run: Token, attempt: Token, workstream: u64, grant: Grant) {
        assert!(
            self.step(Event::Start {
                placement: None,
                version: crate::Version { charter: 1, revision: 1 },
                reply_to: to(attempt),
                run,
                attempt,
                workstream,
                kinds: Kinds::Workers,
                assignment: references(),
                grants: Box::new([grant]),
            })
            .is_empty()
        );
        assert!(self.settle().contains(&Request::Placed { run, attempt }));
    }

    fn grant(&mut self, grant: Grant) -> Box<[Request]> {
        self.step(Event::Grant { grant })
    }

    fn adopt_granted(&mut self, run: Token, attempt: Token, grant: Grant) {
        assert!(
            self.step(Event::Adopt {
                reply_to: to(attempt),
                run,
                attempt,
                kept: 0,
                kind: HostKind::Worker,
                worked: false,
                holds: Box::new([GrantName { table: grant.form.table(), account: grant.account }]),
                grants: Box::new([grant]),
            })
            .is_empty()
        );
    }
}

fn placed(grant: Grant) -> Harness {
    let mut h = Harness::new(LIMITS);
    h.hello(C1, 2, &[], &[]);
    h.start_granted(R1, A1, 7, grant);
    h
}

fn sent(run: Token, attempt: Token, grant: Grant) -> Request {
    Request::Grant { channel: C1, run, attempt, grant }
}

#[test]
fn a_new_generation_reaches_each_live_attempt_of_its_account() {
    let mut h = placed(ANALYST_FIRST);
    h.start_granted(R2, A2, 8, ANALYST_FIRST);
    assert_eq!(&*h.grant(ANALYST_SECOND), &[sent(R1, A1, ANALYST_SECOND), sent(R2, A2, ANALYST_SECOND)]);
}

#[test]
fn a_new_generation_skips_attempts_of_another_account() {
    let mut h = placed(ANALYST_FIRST);
    h.start_granted(R2, A2, 8, OTHER_ACCOUNT);
    assert_eq!(&*h.grant(ANALYST_SECOND), &[sent(R1, A1, ANALYST_SECOND)]);
}

#[test]
fn an_adrift_attempts_grant_waits_for_its_workers_hello() {
    let mut h = placed(ANALYST_FIRST);
    h.step(Event::Lost { channel: C1 });
    assert!(h.grant(ANALYST_SECOND).is_empty());
    assert_eq!(&*h.hello(C1, 2, &[], &[hosted(R1, A1, Phase::Active)]), &[sent(R1, A1, ANALYST_SECOND)]);
}

#[test]
fn an_adopted_attempt_is_sent_its_current_grant_when_listed() {
    let mut h = Harness::new(LIMITS);
    h.adopt_granted(R1, A1, ANALYST_SECOND);
    assert_eq!(
        &*h.hello(C1, 2, &[], &[hosted(R1, A1, Phase::Active)]),
        &[Request::Placed { run: R1, attempt: A1 }, sent(R1, A1, ANALYST_SECOND)],
    );
}

#[test]
fn a_lapsed_grant_is_not_sent_at_a_hello() {
    let mut h = placed(ANALYST_FIRST);
    h.at(61);
    assert!(h.hello(C1, 2, &[], &[hosted(R1, A1, Phase::Active)]).is_empty());
}

#[test]
fn a_resent_grant_keeps_the_form_of_its_newest_generation() {
    let mut h = placed(ANALYST_FIRST);
    h.step(Event::Lost { channel: C1 });
    let newest = Grant { form: crate::Form::Credential(crate::Scheme::Key), ..ANALYST_SECOND };
    h.grant(newest);
    assert_eq!(&*h.hello(C1, 2, &[], &[hosted(R1, A1, Phase::Active)]), &[sent(R1, A1, newest)]);
}
