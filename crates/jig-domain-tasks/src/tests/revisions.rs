//! Entry-point tests use ordinary Rust to script the parent and inspect outputs.
#![allow(
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::wildcard_enum_match_arm,
    clippy::match_like_matches_macro,
    reason = "unit-test parent scripts are ordinary Rust under programming-model.md section 10.2"
)]
use crate::{
    Amendment, Authority, Budget, Contract, Control, Delegation, Domain, EscalationDecision, EscalationHolder,
    EscalationOutcome, Event, Executor, Funder, Hold, Key, Limits, MessageKind, Models, New, Numbers, Parameter, Party,
    PersonAddress, Request, Retries, Retry, Revisions, Scopes, Spec, Stored, TaskResult, Tools, WakePolicy, max_out,
    step,
};
use alloc::{boxed::Box, collections::BTreeMap};
use skein_lib::{Duration, Env, Queue, ReplyTo, Time, Token, Wall};

const STOPPED_TASK: u64 = 1;
const REQUESTER: u64 = 9;
const CONTINUE_MESSAGE: u64 = 1;

const RETRY: Retry = Retry { retries: 2, base: Duration::from_millis(10), max: Duration::from_secs(1) };

pub(super) const LIMITS: Limits = Limits {
    tasks: 16,
    funders: 32,
    project_tasks: 16,
    project_goals: 2,
    tree_tasks: 16,
    depth: 4,
    delegates: 8,
    references: 8,
    subscriptions: 8,
    batch: 8,
    dependencies: 8,
    holdings: 4,
    hold_kinds: 4,
    resource_reports: 64,
    pools: 8,
    hold_segments: 4,
    hold_bytes: 64,
    hold_waiters: 8,
    grace: Duration::from_secs(600),
    hold_wait: Duration::from_secs(10),
    circle: 8,
    inputs: 4,
    spec_bytes: 64,
    parameters: 4,
    result_bytes: 32,
    run_inbox: 8,
    inbox_messages: 8,
    inbox_bytes: 128,
    message_bytes: 32,
    proposal_stall: Duration::from_millis(10),
    escalation_stall: Duration::from_secs(3600),
    person_stall: Duration::from_secs(3600),
    saved_resources: 2,
    contract_choices: 4,
    charters: 2,
    versions: 8,
    revisions: 4,
    authority_grants: 4,
    authority_segments: 4,
    authority_bytes: 64,
    executor_kinds: 3,
    retries: Retries { transient: RETRY, permanent: RETRY, run: RETRY, agent: RETRY, lost: RETRY, invalid: RETRY },
    facts: 16,
    room_refusals: 2,
};

#[must_use]
fn authority() -> Authority {
    Authority {
        tools: Tools(0),
        models: Models::Named(0),
        grants: Box::new([]),
        delegation: Delegation { kinds: Box::new([]), tasks: 16, depth: 4 },
        budget: Budget { spend: 100, deadline: None },
        notes: Scopes(0),
        note_resources: Box::new([]),
    }
}

#[must_use]
pub(super) fn task(number: u64, dependencies: &[u64]) -> New {
    New {
        deadline: crate::Deadline::Named,
        label: 0,
        flags: crate::Flags::PLAIN,
        number,
        project: 1,
        executor: Executor::Agent { charter: 1 },
        llm: None,
        spec: Spec { words: Box::new([1]), parameters: Box::new([]), inputs: Box::new([]) },
        contract: Contract::Report { words: 32 },
        authority: authority(),
        numbers: Numbers { budget: 100, spent: 0, spent_below: 0, reserved: 0 },
        funder: Funder::Period { project: 1, period: 0 },
        dependencies: dependencies.into(),
        holdings: Box::new([]),
        wake: WakePolicy::DEFAULT,
        recurring: None,
        tracked: None,
    }
}

pub(super) struct Harness {
    pub(super) domain: Domain,
    pub(super) env: Env<Limits>,
    pub(super) stored: BTreeMap<Key, Stored>,
}

pub(super) fn to() -> ReplyTo {
    ReplyTo::new(Token::new(1))
}

impl Harness {
    pub(super) fn new() -> Self {
        let mut test = Self {
            domain: Domain::new(&LIMITS, 91, 1, crate::tests::charters(), Box::new([0])),
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits: LIMITS },
            stored: BTreeMap::new(),
        };
        test.send(Event::Restored);
        test.send(Event::OpenPeriod { reply_to: to(), project: 1, period: 0, budget: 10_000 });
        test
    }

    pub(super) fn send(&mut self, event: Event) -> Vec<Request> {
        let mut out = Queue::with_capacity(max_out(&LIMITS));
        step(&mut self.domain, &self.env, event, &mut out);
        let mut requests = Vec::new();
        while let Some(request) = out.pop() {
            match &request {
                Request::Save { record } => {
                    self.stored.insert(record.key(), record.clone());
                }
                Request::Erase { key } => {
                    self.stored.remove(key);
                }
                _ => {}
            }
            requests.push(request);
        }
        requests
    }

    pub(super) fn make(&mut self, batch: Vec<New>) {
        let out = self.send(Event::Make { reply_to: to(), creator: Party::Person(9), batch: batch.into_boxed_slice() });
        assert!(
            out.iter().any(|request| match request {
                Request::Made { .. } => true,
                _ => false,
            }),
            "{out:?}"
        );
    }

    fn revisions(&self, number: u64) -> Revisions {
        self.domain.task(number).expect("task live").revisions
    }

    pub(super) fn amend(&mut self, number: u64, amendment: Amendment) {
        let message = self.domain.task(number).unwrap().last_message.checked_add(1).expect("test message room");
        let out = self.send(Event::Amend {
            places: Box::new([]),
            funder: None,
            reply_to: to(),
            by: Party::Person(9),
            task: number,
            message,
            stop_run: false,
            amendment,
        });
        assert!(
            out.iter().any(|request| match request {
                Request::Done { .. } => true,
                _ => false,
            }),
            "{out:?}"
        );
    }

    pub(super) fn route(&mut self, number: u64, holder: EscalationHolder, entry: u64) {
        let revision = self.revisions(number).hold;
        self.send(Event::RoutedEscalation { task: number, revision, holder, entry });
    }

    pub(super) fn restart(&mut self) {
        self.domain =
            Domain::new(&LIMITS, 91, self.domain.newest(), self.domain.charters().into(), self.domain.efforts.clone());
        let rows: Vec<_> = self
            .stored
            .values()
            .filter(|row| match row {
                Stored::Live(_) | Stored::Ledger(_) | Stored::Stub(_) => true,
                _ => false,
            })
            .cloned()
            .collect();
        for stored in rows {
            let out = self.send(Event::Restore { record: stored });
            assert!(
                !out.iter().any(|request| match request {
                    Request::RestoreRefused { .. } => true,
                    _ => false,
                }),
                "{out:?}"
            );
        }
        let out = self.send(Event::Restored);
        assert!(
            !out.iter().any(|request| match request {
                Request::RestoreRefused { .. } => true,
                _ => false,
            }),
            "{out:?}"
        );
    }
}

fn spec(words: &[u8]) -> Spec {
    Spec { words: words.into(), parameters: Box::new([]), inputs: Box::new([]) }
}

pub(super) fn amendment() -> Amendment {
    Amendment {
        tracked: None,
        addressed: None,
        release: None,
        instructions: None,
        spec: None,
        wake: None,
        dependencies: None,
        authority: None,
        llm: None,
        reason: Box::new([1]),
    }
}

#[test]
fn a_hold_changes_revision_for_a_new_hold_or_reason_but_not_a_pass() {
    let mut test = Harness::new();
    test.make(vec![task(1, &[])]);
    test.send(Event::Hold { task: 1, why: Hold::Stopped });
    let first = test.revisions(1);
    assert_eq!(first, Revisions { hold: 1, spec: 0, person: 0 });
    test.route(1, EscalationHolder::Person(9), 1);
    let out = test.send(Event::DecideEscalation {
        reply_to: to(),
        task: 1,
        revision: first.hold,
        by: 9,
        entry: Some(2),
        decision: EscalationDecision::Pass { holder: EscalationHolder::Role { project: 1, role: 7 } },
    });
    assert!(out.iter().any(|request| match request {
        Request::EscalationDecided { outcome: EscalationOutcome::Passed { revision: 1 }, .. } => true,
        _ => false,
    }));
    assert_eq!(test.revisions(1), first);
    test.restart();
    assert_eq!(test.revisions(1), first);
    test.send(Event::Hold { task: 1, why: Hold::Stopped });
    assert_eq!(test.revisions(1), first);
    let out = test.send(Event::Hold { task: 1, why: Hold::Budget });
    assert!(out.iter().any(|request| match request {
        Request::EscalationNeeded { context } if context.escalation.revision() == 2 => true,
        _ => false,
    }));
    assert_eq!(test.revisions(1), Revisions { hold: 2, ..first });
    test.route(1, EscalationHolder::Person(9), 3);
    test.send(Event::Control {
        reply_to: to(),
        by: Party::Person(9),
        task: 1,
        control: Control::Release { again: None, message: None },
    });
    assert_eq!(test.revisions(1), Revisions { hold: 2, ..first });
    test.send(Event::Hold { task: 1, why: Hold::Budget });
    assert_eq!(test.revisions(1), Revisions { hold: 3, ..first });
}

#[test]
fn amendments_move_only_the_spec_and_changed_person_question() {
    let mut test = Harness::new();
    let mut person = task(1, &[]);
    person.executor = Executor::Person(PersonAddress::Person(10));
    test.make(vec![person]);
    test.amend(1, amendment());
    assert_eq!(test.revisions(1), Revisions { hold: 0, spec: 0, person: 0 });
    let mut change = amendment();
    change.spec = Some(Spec { words: Box::new([2]), parameters: Box::new([]), inputs: Box::new([]) });
    test.amend(1, change.clone());
    assert_eq!(test.revisions(1), Revisions { hold: 0, spec: 1, person: 1 });
    test.amend(1, change);
    assert_eq!(test.revisions(1), Revisions { hold: 0, spec: 1, person: 1 });
    let mut change = amendment();
    let mut narrowed = authority();
    narrowed.tools = Tools(1);
    change.authority = Some(narrowed);
    test.amend(1, change);
    assert_eq!(test.revisions(1), Revisions { hold: 0, spec: 2, person: 1 });
    test.restart();
    assert_eq!(test.revisions(1), Revisions { hold: 0, spec: 2, person: 1 });
}

#[test]
fn a_dependency_result_moves_only_its_person_tasks_revision_and_ended_rows_keep_it() {
    let mut test = Harness::new();
    test.env.wall = Wall::from_nanos(1_000_000_000);
    let mut input = task(1, &[]);
    input.executor = Executor::Person(PersonAddress::Person(10));
    let mut dependent = task(2, &[1]);
    dependent.executor = Executor::Person(PersonAddress::Person(10));
    test.make(vec![input, dependent]);
    let out = test.send(Event::AnswerPerson {
        reply_to: to(),
        task: 1,
        person: 10,
        result: TaskResult::Report { words: Box::new([1]) },
    });
    assert!(
        out.iter().any(|request| match request {
            Request::Done { .. } => true,
            _ => false,
        }),
        "{out:?}"
    );
    test.send(Event::EffectsSettled { task: 1 });
    test.send(Event::Settled { task: 1 });
    assert_eq!(test.revisions(2), Revisions { hold: 0, spec: 0, person: 1 });
    test.restart();
    assert_eq!(test.revisions(2), Revisions { hold: 0, spec: 0, person: 1 });
    let out = test.send(Event::AnswerPerson {
        reply_to: to(),
        task: 2,
        person: 10,
        result: TaskResult::Report { words: Box::new([1]) },
    });
    assert!(
        out.iter().any(|request| match request {
            Request::Done { .. } => true,
            _ => false,
        }),
        "{out:?}"
    );
    test.send(Event::EffectsSettled { task: 2 });
    let out = test.send(Event::Settled { task: 2 });
    assert!(out.iter().any(|request| match request {
        Request::Save { record: Stored::Ended(row) }
            if row.revisions == Revisions { hold: 0, spec: 0, person: 1 }
                && row.person_answer == Some(crate::PersonAnswer { by: 10, at: test.env.wall }) =>
            true,
        _ => false,
    }));
}

#[test]
fn parameters_wake_policy_and_dependencies_each_move_only_the_spec() {
    let mut test = Harness::new();
    test.make(vec![task(1, &[]), task(2, &[1])]);
    let mut change = amendment();
    change.spec = Some(Spec {
        words: Box::new([1]),
        parameters: Box::new([Parameter::Number { name: 1, value: 2 }]),
        inputs: Box::new([]),
    });
    test.amend(2, change);
    assert_eq!(test.revisions(2), Revisions { hold: 0, spec: 1, person: 0 });
    let mut change = amendment();
    let mut wake = WakePolicy::DEFAULT;
    wake.answers = !wake.answers;
    change.wake = Some(wake);
    test.amend(2, change);
    assert_eq!(test.revisions(2), Revisions { hold: 0, spec: 2, person: 0 });
    let mut change = amendment();
    change.dependencies = Some(Box::new([]));
    test.amend(2, change);
    assert_eq!(test.revisions(2), Revisions { hold: 0, spec: 3, person: 0 });
}

fn activity(test: &Harness, task: u64) -> Vec<crate::ActivityLine> {
    test.stored
        .values()
        .filter_map(|row| match row {
            Stored::Activity(row) if row.task == task => Some(row.clone()),
            _ => None,
        })
        .collect()
}

fn write(test: &mut Harness, number: u64, message: u64) {
    test.send(Event::Message {
        reply_to: to(),
        project: 1,
        task: number,
        word: crate::Word {
            number: message,
            from: Party::Person(9),
            kind: MessageKind::Words,
            words: Box::from(&b"Whole words"[..]),
            at: test.env.wall,
            hits: 1,
            eligible: true,
        },
    });
}

#[test]
fn activity_keeps_whole_words_after_a_committed_turn_takes_them() {
    let mut test = Harness::new();
    test.make(vec![task(1, &[])]);
    write(&mut test, 1, 1);
    test.send(Event::Prepare { reply_to: to(), task: 1 });
    test.send(Event::Claim {
        account: None,
        reply_to: to(),
        task: 1,
        attempt: 1,
        resumed: false,
        budget: 10,
        writes: Box::new([]),
    });
    test.send(Event::Started { task: 1, attempt: 1 });
    test.send(Event::Turn {
        refused: Box::new([]),
        reply_to: to(),
        task: 1,
        attempt: 1,
        turn: 1,
        read: Some(1),
        offered: Some(1),
        outstanding: Box::new([]),
        cumulative: 1,
    });
    assert!(test.domain.task(1).unwrap().inbox.is_empty());
    let lines = activity(&test, 1);
    assert!(lines.iter().any(|row| match &row.line {
        crate::Line::Words { message: 1, by: Party::Person(9), words, .. } => words.as_ref() == b"Whole words",
        _ => false,
    }));
}

#[test]
fn activity_orders_an_amendments_message_and_decision_and_keeps_both_settings() {
    let mut test = Harness::new();
    test.make(vec![task(1, &[])]);
    let first = test.domain.task(1).unwrap().activity;
    let before = test.domain.task(1).unwrap().spec.clone();
    let mut change = amendment();
    let after = Spec { words: Box::from(&b"Changed"[..]), ..before.clone() };
    change.spec = Some(after.clone());
    test.env.wall = Wall::from_nanos(8);
    test.amend(1, change);
    let lines = activity(&test, 1);
    assert_eq!(lines[usize::try_from(first).unwrap()].position, first + 1);
    let crate::Line::Heard(word) = &lines[usize::try_from(first).unwrap()].line else { panic!() };
    assert_eq!(word.kind, MessageKind::Amendment { revision: 1 });
    assert_eq!(lines[usize::try_from(first).unwrap() + 1].line, crate::Line::Decided { revision: 1 });
    assert_eq!(lines[usize::try_from(first).unwrap() + 1].at, Wall::from_nanos(8));
    let Stored::History(row) = &test.stored[&Key::History { task: 1, revision: 1 }] else { panic!() };
    let change = row.amendment.as_ref().unwrap();
    assert_eq!(change.before.spec, before);
    assert_eq!(change.after.spec, after);
}

#[test]
fn activity_positions_continue_after_restart_and_the_ended_task_keeps_its_lines() {
    let mut test = Harness::new();
    test.make(vec![task(1, &[])]);
    write(&mut test, 1, 1);
    let before = activity(&test, 1);
    test.restart();
    test.env.wall = Wall::from_nanos(5);
    write(&mut test, 1, 2);
    test.send(Event::Control {
        reply_to: to(),
        by: Party::Person(9),
        task: 1,
        control: Control::Cancel { reason: Box::from(&b"Ended"[..]), grace: None, message: None },
    });
    test.send(Event::EffectsSettled { task: 1 });
    test.send(Event::Settled { task: 1 });
    assert!(test.domain.task(1).is_none());
    let after = activity(&test, 1);
    assert_eq!(&after[..before.len()], before.as_slice());
    for (index, row) in after.iter().enumerate() {
        assert_eq!(row.position, u64::try_from(index).unwrap() + 1);
    }
    let Stored::Ended(row) = &test.stored[&Key::Ended(1)] else { panic!() };
    assert_eq!(row.activity, after.last().unwrap().position);
}

#[test]
fn activity_keeps_each_news_admission_before_the_unread_hints_merge() {
    let mut test = Harness::new();
    test.make(vec![task(1, &[])]);
    let reply = test.send(Event::SubscribeTopic {
        reply_to: to(),
        task: 1,
        subscription: crate::Subscription {
            number: 10,
            kind: crate::SubscriptionKind::Topic { connector: 1, topic: 1 },
        },
    });
    assert!(reply.iter().any(|request| match request {
        Request::Done { .. } => true,
        _ => false,
    }));
    for number in [1, 2] {
        test.env.wall = Wall::from_nanos(number);
        test.send(Event::Notice {
            task: 1,
            word: crate::Word {
                number,
                from: Party::Deployment { project: 1 },
                kind: MessageKind::News { subscription: 10, class: crate::NewsClass::Kept },
                words: Box::from([u8::try_from(number).unwrap()]),
                at: test.env.wall,
                hits: 1,
                eligible: true,
            },
        });
    }
    let row = test.domain.task(1).unwrap();
    assert_eq!(row.inbox.len(), 1);
    assert_eq!(row.inbox[0].at, Wall::from_nanos(1));
    assert_eq!(row.inbox[0].hits, 2);
    let messages: Vec<_> = activity(&test, 1)
        .into_iter()
        .filter_map(|row| match row.line {
            crate::Line::Heard(word) => Some(word),
            _ => None,
        })
        .collect();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].at, Wall::from_nanos(1));
    assert_eq!(messages[1].at, Wall::from_nanos(2));
    assert_eq!(messages[1].hits, 1);
}

#[test]
fn an_uncertain_release_needs_exactly_one_repeat_choice() {
    let mut test = Harness::new();
    test.make(vec![task(1, &[])]);
    test.send(Event::Hold { task: 1, why: Hold::Uncertain { entry: 6 } });
    let before = test.domain.task(1).unwrap().clone();
    let refused = test.send(Event::Control {
        reply_to: to(),
        task: 1,
        by: Party::Person(9),
        control: Control::Release { again: None, message: None },
    });
    assert!(refused.iter().any(|request| match request {
        Request::Refused { .. } => true,
        _ => false,
    }));
    assert_eq!(test.domain.task(1).unwrap(), &before);
    let out = test.send(Event::Control {
        reply_to: to(),
        task: 1,
        by: Party::Person(9),
        control: Control::Release { again: Some(false), message: Some(2) },
    });
    assert!(out.iter().any(|request| match request {
        Request::ResolveUncertain { task: 1, entry: 6, again: false } => true,
        _ => false,
    }));
}

#[test]
fn words_release_a_stop_before_its_cancelled_run_answers() {
    let mut test = Harness::new();
    test.make(vec![task(1, &[])]);
    test.send(Event::Prepare { reply_to: to(), task: 1 });
    test.send(Event::Claim {
        account: None,
        reply_to: to(),
        task: 1,
        attempt: 1,
        resumed: false,
        budget: 10,
        writes: Box::new([]),
    });
    test.send(Event::Started { task: 1, attempt: 1 });
    test.send(Event::Hold { task: 1, why: Hold::StoppedBy { party: 9 } });
    let word = crate::Word {
        number: CONTINUE_MESSAGE,
        from: Party::Person(REQUESTER),
        kind: MessageKind::Words,
        words: Box::new([1]),
        at: Wall::EPOCH,
        hits: 1,
        eligible: false,
    };
    let out = compound::compound(
        &mut test,
        vec![
            crate::Operation::Message { task: STOPPED_TASK, word },
            crate::Operation::Release { task: STOPPED_TASK, again: None, message: None },
        ],
        10_000,
    );
    assert!(out.iter().any(|request| match request {
        Request::Done { .. } => true,
        _ => false,
    }));
    assert!(match test.domain.task(1).unwrap().phase {
        crate::Phase::Active(crate::Active::Running { attempt: 1 }) => true,
        _ => false,
    });
    assert!(test.domain.task(1).unwrap().narrowing, "retain the cancelled attempt until its terminal");
    let out = test.send(Event::Activation {
        reply_to: to(),
        task: 1,
        attempt: 1,
        end: crate::End::Parked,
        cause: crate::Cause::Unpriced,
        saved: None,
    });
    assert!(out.iter().any(|request| match request {
        Request::Activate { .. } => true,
        _ => false,
    }));
    assert_eq!(test.domain.task(1).unwrap().due_reason, crate::Why::Released);
    test.restart();
    assert_eq!(test.domain.task(1).unwrap().due_reason, crate::Why::Released);
    test.send(Event::Prepare { reply_to: to(), task: 1 });
    test.send(Event::Claim {
        account: None,
        reply_to: to(),
        task: 1,
        attempt: 2,
        resumed: false,
        budget: 10,
        writes: Box::new([]),
    });
    test.send(Event::Started { task: 1, attempt: 2 });
    let mut change = amendment();
    let mut authority = test.domain.task(1).unwrap().authority.clone();
    authority.budget.spend = 90;
    change.authority = Some(authority);
    test.send(Event::Amend {
        places: Box::new([]),
        funder: None,
        reply_to: to(),
        by: Party::Person(9),
        task: 1,
        message: 2,
        stop_run: true,
        amendment: change,
    });
    assert!(test.domain.task(1).unwrap().narrowing);
    assert_eq!(test.domain.task(1).unwrap().due_reason, crate::Why::Narrowed);
    test.send(Event::Activation {
        reply_to: to(),
        task: 1,
        attempt: 2,
        end: crate::End::Parked,
        cause: crate::Cause::Unpriced,
        saved: None,
    });
    assert_eq!(test.domain.task(1).unwrap().due_reason, crate::Why::Narrowed);
}

#[test]
fn an_amendment_releases_only_the_named_hold_and_preflights_both_changes() {
    let mut h = Harness::new();
    h.make(vec![task(1, &[])]);
    h.send(Event::Hold { task: 1, why: Hold::StoppedBy { party: 9 } });
    let revision = h.revisions(1).hold;
    let mut change = amendment();
    change.release = Some(revision);
    change.spec = Some(Spec { words: Box::new([8]), parameters: Box::new([]), inputs: Box::new([]) });
    h.amend(1, change.clone());
    assert_eq!(h.domain.task(1).unwrap().spec.words.as_ref(), &[8]);
    assert!(match h.domain.task(1).unwrap().phase {
        crate::Phase::Held { .. } => false,
        _ => true,
    });
    h.send(Event::Hold { task: 1, why: Hold::Budget });
    h.amend(1, change);
    assert!(match h.domain.task(1).unwrap().phase {
        crate::Phase::Held { why: Hold::Budget, .. } => true,
        _ => false,
    });
    let held = h.revisions(1).hold;
    let mut bad = amendment();
    bad.release = Some(held);
    bad.dependencies = Some(Box::new([99]));
    let before = h.domain.task(1).unwrap().clone();
    h.send(Event::Amend {
        places: Box::new([]),
        funder: None,
        reply_to: to(),
        by: Party::Person(9),
        task: 1,
        message: before.last_message + 1,
        stop_run: false,
        amendment: bad,
    });
    assert_eq!(h.domain.task(1), Some(&before));
    crate::domain::task_mut(&mut h.domain, 1).unwrap().record.revision = u64::MAX - 1;
    let before = h.domain.task(1).unwrap().clone();
    let mut change = amendment();
    change.release = Some(held);
    h.send(Event::Amend {
        places: Box::new([]),
        funder: None,
        reply_to: to(),
        by: Party::Person(9),
        task: 1,
        message: before.last_message + 1,
        stop_run: false,
        amendment: change,
    });
    assert_eq!(h.domain.task(1), Some(&before));
}

#[test]
fn capacity_history_distinguishes_project_shape_from_room_and_survives_diagnostic_draining() {
    let mut h = Harness::new();
    h.make(vec![task(1, &[])]);
    h.env.limits.project_tasks = 1;
    let mut wrong = task(2, &[]);
    wrong.project = 2;
    let denied = h.send(Event::Make { reply_to: to(), creator: Party::Task(1), batch: Box::new([wrong]) });
    assert!(denied.iter().any(|r| match r {
        Request::Refused { problem, .. } => problem.why == crate::Refusal::Project && problem.capacity.is_none(),
        _ => false,
    }));
    assert!(h.domain.capacity_refusals().is_empty());
    for number in 2..=4 {
        h.env.wall = Wall::from_nanos(number);
        let denied =
            h.send(Event::Make { reply_to: to(), creator: Party::Person(9), batch: Box::new([task(number, &[])]) });
        assert!(denied.iter().any(|r| match r {
            Request::Refused { problem, .. } => problem.capacity.is_some(),
            _ => false,
        }));
    }
    let rows = h.domain.capacity_refusals();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows.get(0).unwrap().at, Wall::from_nanos(3));
    assert_eq!(rows.get(1).unwrap().at, Wall::from_nanos(4));
    assert_eq!(rows.get(1).unwrap().request.project, Some(1));
    assert_eq!(rows.get(1).unwrap().request.by, Party::Person(9));
    assert_eq!(rows.get(1).unwrap().request.action, crate::CapacityAction::Batch { first: Some(4), members: 1 });
    assert_eq!(rows.get(1).unwrap().request.cause, crate::CapacityCause::ProjectTasks { limit: 1 });
    assert_eq!(h.domain.capacity_refusals_lost(), 1);
    while h.domain.pop_fact().is_some() {}
    assert_eq!(h.domain.capacity_refusals(), rows);
    assert!(h.domain.task(4).is_none());
    let mut cold = Harness::new();
    cold.domain = Domain::new(&LIMITS, 91, 1, crate::tests::charters(), Box::new([0]));
    for row in h.stored.values() {
        match row {
            Stored::Live(_) | Stored::Ledger(_) => {
                cold.send(Event::Restore { record: row.clone() });
            }
            _ => {}
        }
    }
    cold.send(Event::Restored);
    assert!(cold.domain.ready());
    assert!(cold.domain.capacity_refusals().is_empty(), "room history is live-only");
    assert_eq!(cold.domain.capacity_refusals_lost(), 0);
}

#[test]
fn pending_task_proposal_room_keeps_original_action_and_excludes_wrong_project() {
    let mut h = Harness::new();
    h.make(vec![task(1, &[])]);
    let proposal = crate::Proposal {
        number: 7,
        proposer: 1,
        project: 1,
        action: crate::ProposalAction::Release { task: 1 },
        reason: Box::new([]),
        as_holder: false,
        state: crate::ProposalState::Pending { holder: crate::ProposalHolder::Person(9), since: Wall::EPOCH },
    };
    let accepted = h.send(Event::Propose { reply_to: to(), proposal: proposal.clone() });
    assert!(accepted.iter().any(|r| match r {
        Request::Done { .. } => true,
        _ => false,
    }));
    let mut newer = proposal;
    newer.number = 8;
    newer.project = 2;
    h.send(Event::Propose { reply_to: to(), proposal: newer.clone() });
    assert!(h.domain.capacity_refusals().is_empty());
    newer.project = 1;
    h.env.wall = Wall::from_nanos(17);
    h.send(Event::Propose { reply_to: to(), proposal: newer });
    let rows = h.domain.capacity_refusals();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows.get(0).unwrap().at, Wall::from_nanos(17));
    assert_eq!(rows.get(0).unwrap().request.action, crate::CapacityAction::Proposal { number: 8 });
    assert_eq!(rows.get(0).unwrap().request.cause, crate::CapacityCause::Proposals { limit: 1 });
    assert_eq!(h.domain.proposal(1, 7).unwrap().number, 7);
    h.env.limits.inbox_messages = 1;
    h.env.limits.run_inbox = 1;
    h.env.wall = Wall::from_nanos(23);
    let prior = h.domain.task(1).unwrap().clone();
    let denied = h.send(Event::Message {
        reply_to: to(),
        project: 1,
        task: 1,
        word: crate::Word {
            number: 9,
            from: Party::Person(9),
            kind: MessageKind::Words,
            words: Box::new([1]),
            at: Wall::EPOCH,
            hits: 1,
            eligible: true,
        },
    });
    assert!(denied.iter().any(|r| match r {
        Request::Refused { problem, .. } => problem.capacity.is_some(),
        _ => false,
    }));
    let rows = h.domain.capacity_refusals();
    let message = rows.get(1).unwrap();
    assert_eq!(message.at, Wall::from_nanos(23));
    assert_eq!(message.request.action, crate::CapacityAction::Message { task: 1, number: 9 });
    assert_eq!(message.request.cause, crate::CapacityCause::Inbox { task: 1, messages: 1, bytes: LIMITS.inbox_bytes });
    assert_eq!(h.domain.task(1).unwrap(), &prior, "refused words change no admitted inbox or task row");
}

#[test]
fn zero_capacity_history_counts_real_source_room_without_retaining_payloads() {
    let limits = Limits { funders: 0, room_refusals: 0, ..LIMITS };
    let env = Env { now: Time::ZERO, wall: Wall::from_nanos(21), limits };
    let mut domain = Domain::new(&limits, 1, 1, crate::tests::charters(), Box::new([0]));
    let mut out = Queue::with_capacity(max_out(&limits));
    step(&mut domain, &env, Event::Restored, &mut out);
    while out.pop().is_some() {}
    step(&mut domain, &env, Event::OpenPeriod { reply_to: to(), project: 1, period: 4, budget: 100 }, &mut out);
    let Request::Refused { problem, .. } = out.pop().unwrap() else { panic!("source full") };
    let fact = problem.capacity.unwrap();
    assert_eq!(fact.action, crate::CapacityAction::Source { funder: Funder::Period { project: 1, period: 4 } });
    assert_eq!(fact.cause, crate::CapacityCause::Funders { limit: 0 });
    assert!(domain.capacity_refusals().is_empty());
    assert_eq!(domain.capacity_refusals_lost(), 1);
    assert_eq!(domain.capacity_mark(), 1);
}

#[test]
fn lifecycle_wall_times_keep_the_first_start_through_duplicate_reports_holds_and_restore() {
    let mut h = Harness::new();
    h.env.wall = Wall::from_nanos(3);
    h.make(vec![task(1, &[])]);
    assert_eq!(h.domain.task(1).unwrap().phase_since, Wall::from_nanos(3));
    h.env.wall = Wall::from_nanos(5);
    h.send(Event::Prepare { reply_to: to(), task: 1 });
    h.env.wall = Wall::from_nanos(7);
    h.send(Event::Claim {
        account: None,
        reply_to: to(),
        task: 1,
        attempt: 1,
        resumed: false,
        budget: 10,
        writes: Box::new([]),
    });
    assert_eq!(h.domain.task(1).unwrap().run_since, None);
    h.env.wall = Wall::from_nanos(11);
    h.send(Event::Started { task: 1, attempt: 1 });
    let started = Some(Wall::from_nanos(11));
    h.env.wall = Wall::from_nanos(13);
    h.send(Event::Started { task: 1, attempt: 1 });
    h.send(Event::Started { task: 1, attempt: 2 });
    assert_eq!(h.domain.task(1).unwrap().run_since, started);
    assert_eq!(h.domain.task(1).unwrap().phase_since, Wall::from_nanos(11));
    h.env.wall = Wall::from_nanos(17);
    h.send(Event::Hold { task: 1, why: Hold::Budget });
    h.route(1, EscalationHolder::Person(9), 1);
    h.restart();
    assert_eq!(h.domain.task(1).unwrap().phase_since, Wall::from_nanos(17));
    assert_eq!(h.domain.task(1).unwrap().run_since, started);
    h.env.wall = Wall::from_nanos(19);
    h.send(Event::Control {
        reply_to: to(),
        by: Party::Person(9),
        task: 1,
        control: Control::Release { again: None, message: None },
    });
    assert_eq!(h.domain.task(1).unwrap().run_since, started);
    assert_eq!(h.domain.task(1).unwrap().phase_since, Wall::from_nanos(19));
}

#[test]
fn retry_observations_distinguish_attempt_failures_from_preparation_refusals_and_survive_restore() {
    let mut h = Harness::new();
    h.make(vec![task(1, &[]), task(2, &[])]);
    h.send(Event::Prepare { reply_to: to(), task: 1 });
    h.send(Event::Claim {
        account: None,
        reply_to: to(),
        task: 1,
        attempt: 1,
        resumed: false,
        budget: 10,
        writes: Box::new([]),
    });
    h.env.wall = Wall::from_nanos(20);
    h.send(Event::Activation {
        reply_to: to(),
        task: 1,
        attempt: 1,
        end: crate::End::Failed(crate::Class::Lost),
        saved: None,
        cause: crate::Cause::Unpriced,
    });
    let failed = Some(crate::RetryNotice::Failed { class: crate::Class::Lost, count: 1, allowance: 2 });
    assert_eq!(h.domain.task(1).unwrap().retry_notice, failed);
    assert_eq!(h.domain.task(1).unwrap().phase_since, Wall::from_nanos(20));
    h.env.wall = Wall::from_nanos(23);
    h.send(Event::Prepare { reply_to: to(), task: 2 });
    h.send(Event::PreparationFailed { task: 2 });
    let refused = Some(crate::RetryNotice::Refused { count: 1 });
    assert_eq!(h.domain.task(2).unwrap().retry_notice, refused);
    h.restart();
    assert_eq!(h.domain.task(1).unwrap().retry_notice, failed);
    assert_eq!(h.domain.task(2).unwrap().retry_notice, refused);
    assert_eq!(h.domain.task(2).unwrap().phase_since, Wall::from_nanos(23));
}

mod chat;

mod positions;

mod tracking;

mod compound;

mod text;

mod dependencies;

#[test]
fn replacing_charter_models_replaces_the_table_and_keeps_live_tasks() {
    let mut h = Harness::new();
    h.make(vec![task(1, &[])]);
    let before = h.domain.task(1).unwrap().clone();
    let charters = versions(&[(7, 2, Some(&[1]))]);
    h.send(Event::Charters { newest: 2, charters: charters.clone(), efforts: Box::new([0, 0]) });
    assert_eq!(h.domain.charters(), charters.as_ref());
    assert_eq!(h.domain.task(1), Some(&before));
}

fn versions(rows: &[(u32, u32, Option<&[u8]>)]) -> Box<[crate::CharterModels]> {
    rows.iter()
        .map(|&(number, revision, models)| crate::CharterModels { number, revision, models: models.map(Box::from) })
        .collect()
}

fn settings_charters(test: &mut Harness, newest: u32, rows: &[(u32, u32, Option<&[u8]>)]) {
    test.send(Event::Charters { newest, charters: versions(rows), efforts: Box::new([0, 0]) });
    assert_eq!(test.domain.newest(), newest);
}

fn settings_claim(test: &mut Harness, number: u64) -> Vec<Request> {
    test.send(Event::Claim {
        reply_to: to(),
        task: number,
        attempt: 1,
        resumed: false,
        budget: 10,
        account: None,
        writes: Box::new([]),
    })
}

fn settings_end(test: &mut Harness, number: u64) {
    test.send(Event::Control {
        reply_to: to(),
        by: Party::Person(9),
        task: number,
        control: Control::Cancel { reason: Box::new([]), grace: None, message: None },
    });
    test.send(Event::EffectsSettled { task: number });
    test.send(Event::Settled { task: number });
    assert!(test.domain.task(number).is_none());
}

#[test]
fn a_task_records_the_newest_revision_whatever_its_parents() {
    let mut h = Harness::new();
    settings_charters(&mut h, 3, &[(1, 1, Some(&[0]))]);
    let mut goal = task(1, &[]);
    goal.tracked = Some(1);
    goal.numbers.budget = 1000;
    goal.authority.budget.spend = 1000;
    let mut procedure = goal.clone();
    procedure.number = 2;
    procedure.tracked = None;
    procedure.executor = Executor::Procedure { connector: 1, code: 1 };
    h.make(vec![goal, procedure]);
    settings_charters(&mut h, 4, &[(1, 1, Some(&[0]))]);
    let mut delegate = task(3, &[]);
    delegate.funder = Funder::Task(1);
    let out = h.send(Event::Make { reply_to: to(), creator: Party::Task(1), batch: Box::new([delegate]) });
    assert!(
        out.iter().any(|r| match r {
            Request::Made { .. } => true,
            _ => false,
        }),
        "{out:?}"
    );
    let mut step_task = task(4, &[]);
    step_task.funder = Funder::Task(2);
    let decision = crate::ProcedureDecision {
        batch: Box::new([step_task]),
        ..crate::ProcedureDecision::new(crate::ProcedureEnd::Wait)
    };
    let out = h.send(Event::Procedure { reply_to: to(), task: 2, step: 1, read: None, decision });
    assert!(
        out.iter().any(|r| match r {
            Request::Made { .. } => true,
            _ => false,
        }),
        "{out:?}"
    );
    let mut person = task(5, &[]);
    person.executor = Executor::Person(PersonAddress::Person(9));
    h.make(vec![person]);
    for number in [1, 2] {
        assert_eq!(h.domain.task(number).unwrap().settings, crate::SettingsRevision { task: 3, run: None });
    }
    for number in [3, 4, 5] {
        assert_eq!(h.domain.task(number).unwrap().settings, crate::SettingsRevision { task: 4, run: None });
    }
    assert_eq!(h.domain.holders(3), 1);
    assert_eq!(h.domain.holders(4), 2);
}

#[test]
fn a_run_records_its_tasks_revision_until_its_attempt_ends() {
    let mut h = Harness::new();
    settings_charters(&mut h, 3, &[(1, 1, Some(&[0]))]);
    h.make(vec![task(1, &[]), task(2, &[])]);
    settings_charters(&mut h, 4, &[(1, 1, Some(&[0]))]);
    h.send(Event::Prepare { reply_to: to(), task: 1 });
    assert_eq!(h.domain.task(1).unwrap().settings.run, Some(3));
    let out = settings_claim(&mut h, 1);
    assert!(
        out.iter().any(|request| match request {
            Request::Save { record: Stored::Live(row) } =>
                row.settings == crate::SettingsRevision { task: 3, run: Some(3) },
            _ => false,
        }),
        "{out:?}"
    );
    h.send(Event::Activation {
        reply_to: to(),
        task: 1,
        attempt: 1,
        end: crate::End::Parked,
        cause: crate::Cause::Unpriced,
        saved: None,
    });
    assert_eq!(h.domain.task(1).unwrap().settings.run, None);
    h.send(Event::Prepare { reply_to: to(), task: 2 });
    assert_eq!(h.domain.task(2).unwrap().settings.run, Some(3));
    h.send(Event::PreparationFailed { task: 2 });
    assert_eq!(h.domain.task(2).unwrap().settings.run, None);
}

fn settings_choice(model: u8) -> crate::LlmChoice {
    crate::LlmChoice { account: None, main: crate::ModelEffort { model, effort: None }, subagents: Box::new([]) }
}

fn settings_agent(number: u64, model: u8) -> New {
    let mut new = task(number, &[]);
    new.executor = Executor::Agent { charter: 7 };
    new.authority.models = Models::Named(3);
    new.llm = Some(settings_choice(model));
    new
}

fn settings_amend(test: &mut Harness, number: u64, model: u8) -> Vec<Request> {
    let mut change = amendment();
    change.llm = Some(settings_choice(model));
    test.send(Event::Amend {
        places: Box::new([]),
        funder: None,
        reply_to: to(),
        by: Party::Person(9),
        task: number,
        message: test.domain.task(number).unwrap().last_message.checked_add(1).unwrap(),
        stop_run: false,
        amendment: change,
    })
}

#[test]
fn an_amended_llm_choice_is_checked_against_its_own_charter_version() {
    let mut h = Harness::new();
    settings_charters(&mut h, 1, &[(7, 1, Some(&[0]))]);
    h.make(vec![settings_agent(1, 0)]);
    settings_charters(&mut h, 2, &[(7, 2, Some(&[1])), (7, 1, Some(&[0]))]);
    assert_eq!(h.domain.charter_models(7, 1), Some([0].as_slice()));
    assert_eq!(h.domain.charter_models(7, 2), Some([1].as_slice()));
    assert_eq!(h.domain.default_llm(7, Models::All).unwrap().main.model, 1);
    let before = h.domain.task(1).unwrap().clone();
    let out = settings_amend(&mut h, 1, 1);
    assert!(
        out.iter().any(|r| match r {
            Request::Refused { problem, .. } =>
                problem.why == crate::Refusal::Llm { model: 1, why: crate::LlmFault::Charter },
            _ => false,
        }),
        "{out:?}"
    );
    assert_eq!(h.domain.task(1), Some(&before));
    let mut change = amendment();
    change.llm = Some(settings_choice(0));
    h.amend(1, change);
    let out =
        h.send(Event::Make { reply_to: to(), creator: Party::Person(9), batch: Box::new([settings_agent(2, 0)]) });
    assert!(
        out.iter().any(|r| match r {
            Request::Refused { problem, .. } =>
                problem.why == crate::Refusal::Llm { model: 0, why: crate::LlmFault::Charter },
            _ => false,
        }),
        "{out:?}"
    );
    h.make(vec![settings_agent(2, 1)]);
    assert_eq!(h.domain.task(2).unwrap().settings.task, 2);
}

#[test]
fn a_removed_charters_older_version_still_serves_the_task_that_reads_it() {
    let mut h = Harness::new();
    settings_charters(&mut h, 1, &[(7, 1, Some(&[0]))]);
    h.make(vec![settings_agent(1, 0)]);
    settings_charters(&mut h, 2, &[(7, 1, Some(&[0])), (7, 2, None)]);
    assert_eq!(h.domain.charter_models(7, 0), None);
    assert_eq!(h.domain.charter_models(7, 1), Some([0].as_slice()));
    assert_eq!(h.domain.charter_models(7, 2), None);
    assert_eq!(h.domain.default_llm(7, Models::All), None);
    let out = settings_amend(&mut h, 1, 1);
    assert!(
        out.iter().any(|r| match r {
            Request::Refused { problem, .. } =>
                problem.why == crate::Refusal::Llm { model: 1, why: crate::LlmFault::Charter },
            _ => false,
        }),
        "{out:?}"
    );
    let mut change = amendment();
    change.llm = Some(settings_choice(0));
    h.amend(1, change);
    let out =
        h.send(Event::Make { reply_to: to(), creator: Party::Person(9), batch: Box::new([settings_agent(2, 0)]) });
    assert!(
        out.iter().any(|r| match r {
            Request::Refused { problem, .. } => problem.why == crate::Refusal::Executor,
            _ => false,
        }),
        "{out:?}"
    );
    h.restart();
    assert_eq!(h.domain.task(1).unwrap().settings.task, 1);
    assert_eq!(h.domain.holders(1), 1);
}

#[test]
fn live_holders_count_each_revision_once_and_reach_zero_as_the_last_ends() {
    let mut h = Harness::new();
    let mut procedure = task(4, &[]);
    procedure.executor = Executor::Procedure { connector: 1, code: 1 };
    h.make(vec![task(1, &[]), task(2, &[]), procedure]);
    settings_charters(&mut h, 2, &[(1, 1, Some(&[0]))]);
    h.make(vec![task(3, &[])]);
    h.send(Event::Prepare { reply_to: to(), task: 3 });
    settings_claim(&mut h, 3);
    assert_eq!(h.domain.holders(1), 2);
    assert_eq!(h.domain.holders(2), 1);
    settings_end(&mut h, 1);
    assert_eq!(h.domain.holders(1), 1);
    settings_end(&mut h, 2);
    assert_eq!(h.domain.holders(1), 0);
    h.send(Event::Activation {
        reply_to: to(),
        task: 3,
        attempt: 1,
        end: crate::End::Parked,
        cause: crate::Cause::Unpriced,
        saved: None,
    });
    assert_eq!(h.domain.holders(2), 1);
    settings_end(&mut h, 3);
    assert_eq!(h.domain.holders(2), 0);
}

#[test]
fn a_restart_keeps_each_tasks_revision_and_rebuilds_the_holders() {
    let mut h = Harness::new();
    h.make(vec![task(1, &[]), task(2, &[])]);
    settings_end(&mut h, 1);
    settings_charters(&mut h, 2, &[(1, 1, Some(&[0]))]);
    h.make(vec![task(3, &[])]);
    h.send(Event::Prepare { reply_to: to(), task: 3 });
    settings_claim(&mut h, 3);
    let live = [h.domain.task(2).unwrap().settings, h.domain.task(3).unwrap().settings];
    let ended = h.stored[&Key::Ended(1)].clone();
    let counts = [h.domain.holders(1), h.domain.holders(2)];
    h.restart();
    assert_eq!(h.domain.newest(), 2);
    assert_eq!(h.domain.task(2).unwrap().settings, live[0]);
    assert_eq!(h.domain.task(3).unwrap().settings, live[1]);
    assert_eq!(h.stored[&Key::Ended(1)], ended);
    assert_eq!([h.domain.holders(1), h.domain.holders(2)], counts);
}

#[test]
fn abandoned_preparations_clear_the_run_revision_but_held_attempts_keep_it() {
    let mut h = Harness::new();
    h.make(vec![task(1, &[]), task(2, &[]), task(3, &[])]);
    h.send(Event::Prepare { reply_to: to(), task: 1 });
    h.restart();
    assert_eq!(h.domain.task(1).unwrap().settings.run, None);
    h.send(Event::Prepare { reply_to: to(), task: 1 });
    h.send(Event::Hold { task: 1, why: Hold::Budget });
    assert_eq!(h.domain.task(1).unwrap().settings.run, None);
    h.send(Event::Prepare { reply_to: to(), task: 2 });
    settings_end(&mut h, 2);
    let Stored::Ended(ended) = &h.stored[&Key::Ended(2)] else { panic!("ended task") };
    assert_eq!(ended.settings.run, None);
    h.send(Event::Prepare { reply_to: to(), task: 3 });
    settings_claim(&mut h, 3);
    h.send(Event::Hold { task: 3, why: Hold::Budget });
    assert_eq!(h.domain.task(3).unwrap().settings.run, Some(1));
    h.route(1, EscalationHolder::Person(9), 1);
    h.route(3, EscalationHolder::Person(9), 3);
    h.restart();
    assert_eq!(h.domain.task(3).unwrap().settings.run, Some(1));
    h.send(Event::Activation {
        reply_to: to(),
        task: 3,
        attempt: 1,
        end: crate::End::Parked,
        cause: crate::Cause::Unpriced,
        saved: None,
    });
    assert_eq!(h.domain.task(3).unwrap().settings.run, None);
}

#[test]
fn restored_holders_count_a_distinct_run_revision_until_its_terminal() {
    let mut h = Harness::new();
    settings_charters(&mut h, 2, &[(1, 1, Some(&[0]))]);
    h.make(vec![task(1, &[])]);
    h.send(Event::Prepare { reply_to: to(), task: 1 });
    settings_claim(&mut h, 1);
    let Stored::Live(row) = h.stored.get_mut(&Key::Live(1)).unwrap() else { panic!("live claim") };
    row.settings.run = Some(1);
    h.restart();
    assert_eq!(h.domain.holders(1), 1);
    assert_eq!(h.domain.holders(2), 1);
    h.send(Event::Activation {
        reply_to: to(),
        task: 1,
        attempt: 1,
        end: crate::End::Parked,
        cause: crate::Cause::Unpriced,
        saved: None,
    });
    assert_eq!(h.domain.holders(1), 0);
    assert_eq!(h.domain.holders(2), 1);
    settings_end(&mut h, 1);
    assert_eq!(h.domain.holders(2), 0);
}
