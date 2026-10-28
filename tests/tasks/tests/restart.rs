use jig_domain_tasks::{Active, End, Event, Key, Party, Phase, Refusal, Stored, TaskResult};
use jig_tasks_world::{LIMITS, Reply, World, task};
use skein_lib::Wall;

const BEFORE_CLAIM_ACCOUNT: u32 = 71;
const DURABLE_CLAIM_ACCOUNT: u32 = 72;

#[test]
fn make_and_claim_have_independent_before_and_after_durable_cuts() {
    let mut w = World::new(20, LIMITS);
    let reply_to = w.to();
    w.stage(Event::Make { reply_to, creator: Party::Person(1), batch: Box::new([task(1, &[])]) });
    assert_eq!(w.live(), 0);
    assert!(w.activations.is_empty());
    w.restart();
    assert_eq!(w.live(), 0);
    let reply_to = w.to();
    w.stage(Event::Make { reply_to, creator: Party::Person(1), batch: Box::new([task(2, &[])]) });
    w.durable();
    assert!(w.activations.is_empty());
    w.restart();
    assert!(w.activations.contains(&2));
    let reply_to = w.to();
    w.send(Event::Prepare { reply_to, task: 2 });
    let reply_to = w.to();
    w.stage(Event::Claim {
        account: Some(BEFORE_CLAIM_ACCOUNT),
        reply_to,
        task: 2,
        attempt: 10,
        resumed: false,
        budget: 100,
        writes: Box::new([]),
    });
    assert!(w.runs.is_empty());
    w.restart();
    assert_eq!(w.record(2).attempt, 0);
    assert_eq!(w.record(2).kept_account, None);
    assert!(w.activations.contains(&2));
    let reply_to = w.to();
    w.send(Event::Prepare { reply_to, task: 2 });
    let reply_to = w.to();
    w.stage(Event::Claim {
        account: Some(DURABLE_CLAIM_ACCOUNT),
        reply_to,
        task: 2,
        attempt: 11,
        resumed: false,
        budget: 100,
        writes: Box::new([]),
    });
    w.durable();
    assert!(w.runs.is_empty());
    w.restart();
    assert_eq!(w.runs.get(&2), Some(&11));
    assert_eq!(w.record(2).kept_account, Some(DURABLE_CLAIM_ACCOUNT));
    assert_eq!(w.record(2).phase, Phase::Active(Active::Claimed { attempt: 11 }));
    w.finish(2);
    w.settle(2);
}

#[test]
fn stale_claims_and_replayed_terminal_are_and_do_not_mutate() {
    let mut w = World::new(22, LIMITS);
    w.make(Party::Person(1), vec![task(1, &[])]);
    w.claim(1, 4);
    let reply_to = w.to();
    w.send(Event::Claim {
        account: None,
        reply_to,
        task: 1,
        attempt: 4,
        resumed: false,
        budget: 100,
        writes: Box::new([]),
    });
    assert!(
        matches!(w.replies.last_key_value().expect("reply").1, Reply::Refused(problem) if problem.why == Refusal::Attempt)
    );
    w.terminal(1, End::Refused);
    let record = w.record(1).clone();
    let reply_to = w.to();
    w.send(Event::Activation {
        saved: None,
        reply_to,
        task: 1,
        attempt: 4,
        end: End::Failed(jig_domain_tasks::Class::Lost),
        cause: jig_domain_tasks::Cause::Unpriced,
    });
    assert_eq!(w.record(1), &record);
    assert!(matches!(
        w.replies.last_key_value().expect("reply").1,
        Reply::Acknowledged(jig_domain_tasks::Accepted::Already)
    ));
}

#[test]
fn a_crash_before_finished_decision_resumes_current_attempt() {
    let mut w = World::new(24, LIMITS);
    w.make(Party::Person(1), vec![task(1, &[])]);
    w.claim(1, 1);
    let started = w.record(1).run_since;
    assert!(started.is_some());
    let reply_to = w.to();
    w.stage(Event::Activation {
        saved: None,
        reply_to,
        task: 1,
        attempt: 1,
        end: End::Finished { result: TaskResult::Report { words: Box::new([1]) }, cancel_delegates: false },
        cause: jig_domain_tasks::Cause::Unpriced,
    });
    assert!(w.results.is_empty());
    assert!(w.closing.is_empty());
    w.restart();
    assert_eq!(w.runs.get(&1), Some(&1));
    assert_eq!(w.record(1).run_since, started);
    w.finish(1);
    w.settle(1);
}

#[test]
fn restore_asserts_absent_links_and_loads_historical_shapes() {
    use jig_domain_tasks::{max_out, step};
    use skein_lib::{Env, Queue, Time, Wall};
    let mut source = World::new(25, LIMITS);
    source.make(Party::Person(1), vec![task(1, &[]), task(2, &[])]);
    for corrupt in 0..13 {
        let mut domain = jig_tasks_world::domain(&LIMITS, 25);
        let env = Env { limits: LIMITS, now: Time::ZERO, wall: Wall::EPOCH };
        let mut out = Queue::with_capacity(max_out(&LIMITS));
        for number in [1, 2] {
            let mut record = source.record(number).clone();
            match corrupt {
                0 => record.root = 99,
                1 => {
                    record.phase = Phase::Waiting;
                    record.dependencies = Box::new([3 - number]);
                    record.waiting_on = Box::new([3 - number]);
                }
                2 => record.contract = jig_domain_tasks::Contract::Report { words: 33 },
                3 => {
                    if number == 1 {
                        record.delegates = Box::new([2]);
                    } else {
                        record.requester = Party::Task(1);
                        record.root = 1;
                        record.depth = 1;
                    }
                }
                4 => record.result_followups = Box::new([3]),
                5..=7 => {
                    record.contract = jig_domain_tasks::Contract::Verdict {
                        choices: Box::new([jig_domain_tasks::Verdict {
                            fields: Box::new([]),
                            code: 7,
                            words: 32,
                            followups: 2,
                        }]),
                    };
                    record.phase = Phase::Closing(jig_domain_tasks::Closing {
                        stage: jig_domain_tasks::Stage::Delegates,
                        ending: jig_domain_tasks::Ending::Done(jig_domain_tasks::TaskResult::Verdict {
                            fields: Box::new([]),
                            code: 7,
                            words: Box::new([]),
                        }),
                    });
                    record.result_followups = match corrupt {
                        5 => Box::new([3, 3]),
                        6 => Box::new([3, 4, 5]),
                        7 => Box::new([0, number]),
                        _ => unreachable!(),
                    };
                }
                8 => record.run_since = Some(Wall::EPOCH),
                9 => {
                    record.attempt = 1;
                    record.phase = Phase::Active(Active::Running { attempt: 1 });
                }
                10 => record.phase = Phase::Active(Active::BackingOff { until: Wall::EPOCH }),
                11 => {
                    record.phase = Phase::Active(Active::BackingOff { until: Wall::EPOCH });
                    record.retry_notice = Some(jig_domain_tasks::RetryNotice::Refused { count: 1 });
                }
                12 => {
                    record.attempt = 1;
                    record.phase = Phase::Active(Active::Claimed { attempt: 1 });
                    record.run_since = Some(Wall::EPOCH);
                }
                _ => unreachable!(),
            }
            let record = Stored::Live(Box::new(record));
            step(&mut domain, &env, Event::Restore { record: record.clone() }, &mut out);
            assert_eq!(domain.stored_record(&record.key()), Some(record));
        }
        for record in source.records.values().filter(|row| matches!(row, Stored::Ledger(_))) {
            step(&mut domain, &env, Event::Restore { record: record.clone() }, &mut out);
        }
        if corrupt == 0 {
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                step(&mut domain, &env, Event::Check, &mut out);
            }))
            .expect_err("a missing root is a writer bug");
            let message = panic.downcast_ref::<String>().expect("formatted link assertion");
            assert_eq!(message, "tasks: Live(1) links to root 99, which is absent");
        } else {
            assert!(out.is_empty(), "bounded historical shapes load without a refusal");
        }
    }
}

#[test]
fn terminal_and_settlement_each_have_a_durable_cut_before_delivery() {
    let mut w = World::new(26, LIMITS);
    w.make(Party::Person(1), vec![task(1, &[])]);
    w.claim(1, 1);
    let reply_to = w.to();
    w.stage(Event::Activation {
        saved: None,
        reply_to,
        task: 1,
        attempt: 1,
        end: End::Finished { result: TaskResult::Report { words: Box::new([1]) }, cancel_delegates: false },
        cause: jig_domain_tasks::Cause::Unpriced,
    });
    w.durable();
    assert!(w.closing.is_empty());
    assert!(w.results.is_empty());
    w.restart();
    assert!(w.closing.contains(&1));
    assert!(!w.runs.contains_key(&1));
    w.stage(Event::EffectsSettled { task: 1 });
    w.durable();
    assert!(w.results.is_empty());
    w.restart();
    w.observe(jig_tasks_world::referee::Seen::Settled { task: 1 });
    w.stage(Event::Settled { task: 1 });
    w.durable();
    assert_eq!(w.results.len(), 1);
    assert!(!w.records.contains_key(&Key::Live(1)));
    w.restart();
    assert_eq!(w.results.len(), 1);
    assert!(w.closing.is_empty());
}

#[test]
fn wall_correction_does_not_move_a_live_backoff_but_restore_reprojects_it() {
    use skein_lib::{Duration, Wall};
    let mut w = World::new(28, LIMITS);
    w.make(Party::Person(1), vec![task(1, &[])]);
    w.claim(1, 1);
    w.terminal(1, End::Refused);
    w.env.wall = Wall::from_nanos(Duration::from_secs(100).as_nanos());
    w.advance();
    assert!(w.activations.contains(&1));
    w.claim(1, 2);
    w.terminal(1, End::Refused);
    w.env.wall = Wall::from_nanos(Duration::from_secs(200).as_nanos());
    w.restart();
    w.advance();
    assert!(w.activations.contains(&1));
}

#[test]
fn delegate_cancel_cut_after_durability_stops_adopted_runs_then_closes_deepest() {
    let mut w = World::new(27, LIMITS);
    w.make(Party::Person(1), vec![task(1, &[])]);
    w.claim(1, 1);
    w.make(Party::Task(1), vec![task(2, &[])]);
    w.claim(2, 2);
    w.make(Party::Task(2), vec![task(3, &[])]);
    w.claim(3, 3);
    w.observe(jig_tasks_world::referee::Seen::Cancelled { task: 2 });
    let reply_to = w.to();
    w.stage(Event::Activation {
        saved: None,
        reply_to,
        task: 1,
        attempt: 1,
        end: End::Finished { result: TaskResult::Report { words: Box::new([1]) }, cancel_delegates: true },
        cause: jig_domain_tasks::Cause::Unpriced,
    });
    w.durable();
    assert!(w.stops.is_empty());
    w.restart();
    assert_eq!(w.stops.len(), 2);
    w.complete_cancel();
    assert_eq!(w.results.len(), 3);
}

#[test]
fn dependency_progress_survives_restore_without_loading_historical_ends() {
    let mut w = World::new(29, LIMITS);
    w.make(Party::Person(1), vec![task(1, &[]), task(2, &[]), task(3, &[1, 2])]);
    w.claim(1, 1);
    w.finish(1);
    w.settle(1);
    assert_eq!(w.record(3).waiting_on.as_ref(), [2]);
    let before = w.records.clone();
    w.restart();
    assert_eq!(w.records, before);
    assert!(!w.activations.contains(&3));
    w.claim(2, 2);
    w.finish(2);
    w.settle(2);
    assert!(w.record(3).waiting_on.is_empty());
    w.restart();
    w.claim(3, 3);
    w.finish(3);
    w.settle(3);
    assert_eq!(w.results.len(), 3);
}

fn load_without_judgment(seed: u64, rows: &[Stored], env: &skein_lib::Env<jig_domain_tasks::Limits>) {
    let mut domain = jig_tasks_world::domain(&env.limits, seed);
    let mut out = skein_lib::Queue::with_capacity(jig_domain_tasks::max_out(&env.limits));
    for record in rows {
        jig_domain_tasks::step(&mut domain, env, Event::Restore { record: record.clone() }, &mut out);
        assert_eq!(domain.stored_record(&record.key()), Some(record.clone()));
    }
    jig_domain_tasks::step(&mut domain, env, Event::Check, &mut out);
    assert!(domain.checked());
    assert!(out.is_empty(), "linking neither judges historical rules nor activates work: {out:?}");
}

#[test]
fn restore_loads_bounded_funding_without_judging_eventual_postings() {
    use jig_domain_tasks::{Cause, Funder};
    let mut source = World::new(30, LIMITS);
    source.make(Party::Person(1), vec![task(1, &[])]);
    source.claim(1, 1);
    source.terminal_cause(
        1,
        End::Finished { result: TaskResult::Report { words: Box::new([1]) }, cancel_delegates: false },
        Cause::Priced { cumulative: 1 },
    );
    let mut rows = source.records.values().filter(|row| row.key().live()).cloned().collect::<Vec<_>>();
    for row in &mut rows {
        if let Stored::Ledger(ledger) = row
            && ledger.funder == (Funder::Period { project: 1, period: 0 })
        {
            // Keep the row individually representable; only the eventual
            // aggregate with the live task overflows at Check.
            ledger.numbers.budget = u64::MAX;
            ledger.numbers.reserved = 0;
            ledger.numbers.spent_below = u64::MAX;
        }
    }
    load_without_judgment(30, &rows, &source.env);
}

#[test]
fn bounded_unfinished_dependencies_load_without_subset_judgments() {
    use jig_domain_tasks::step;
    use skein_lib::Queue;
    let mut source = World::new(31, LIMITS);
    source.make(Party::Person(1), vec![task(1, &[])]);
    let mut record = source.record(1).clone();
    record.waiting_on = Box::new([99]);
    let record = Stored::Live(Box::new(record));
    let mut domain = jig_tasks_world::domain(&LIMITS, 31);
    let mut out = Queue::with_capacity(jig_domain_tasks::max_out(&LIMITS));
    step(&mut domain, &source.env, Event::Restore { record: record.clone() }, &mut out);
    assert!(out.is_empty());
    assert_eq!(domain.stored_record(&record.key()), Some(record));
}

#[test]
fn restore_loads_the_committed_unfinished_subset_before_activation() {
    let mut source = World::new(32, LIMITS);
    source.make(Party::Person(1), vec![task(1, &[]), task(2, &[1])]);
    for phase in [Phase::Waiting, Phase::Active(Active::Due), Phase::Active(Active::Preparing)] {
        let mut rows = source.records.values().filter(|row| row.key().live()).cloned().collect::<Vec<_>>();
        for row in &mut rows {
            if let Stored::Live(task) = row
                && task.number == 2
            {
                task.waiting_on = Box::new([]);
                task.phase = phase.clone();
            }
        }
        load_without_judgment(32, &rows, &source.env);
    }
}

#[test]
fn historical_settled_live_and_ledger_states_load_without_judgment() {
    use jig_domain_tasks::{Funder, Stage, step};
    use skein_lib::Queue;
    let mut source = World::new(33, LIMITS);
    source.open_period(1, 300);
    source.carve_pool(1, 200);
    let mut new = task(1, &[]);
    new.funder = Funder::Period { project: 1, period: 1 };
    source.make(Party::Person(1), vec![new]);
    source.claim(1, 1);
    source.finish(1);
    let mut task = source.record(1).clone();
    if let Phase::Closing(closing) = &mut task.phase {
        closing.stage = Stage::Settled;
    } else {
        panic!("closing fixture");
    }
    let mut invalid = vec![Stored::Live(Box::new(task))];
    for row in source.records.values() {
        if let Stored::Ledger(ledger) = row {
            let mut closed = *ledger;
            closed.closed = true;
            closed.numbers.reserved = closed.numbers.reserved.saturating_add(1);
            invalid.push(Stored::Ledger(closed));
            let mut spent = *ledger;
            spent.numbers.spent = 1;
            invalid.push(Stored::Ledger(spent));
        }
    }
    for record in invalid {
        let mut domain = jig_tasks_world::domain(&LIMITS, 33);
        let mut out = Queue::with_capacity(jig_domain_tasks::max_out(&LIMITS));
        step(&mut domain, &source.env, Event::Restore { record: record.clone() }, &mut out);
        assert!(out.is_empty());
        assert_eq!(domain.stored_record(&record.key()), Some(record));
    }
}

#[test]
fn restore_links_task_funding_without_judging_requester_ancestry() {
    use jig_domain_tasks::Funder;
    let mut source = World::new(34, LIMITS);
    source.make(Party::Person(1), vec![task(1, &[]), task(2, &[])]);
    let mut rows = source.records.values().filter(|row| row.key().live()).cloned().collect::<Vec<_>>();
    for row in &mut rows {
        match row {
            Stored::Live(task) if task.number == 1 => task.numbers.reserved = 100,
            Stored::Live(task) if task.number == 2 => task.funder = Funder::Task(1),
            Stored::Ledger(ledger) => ledger.numbers.reserved = 100,
            Stored::Live(_)
            | Stored::Ended(_)
            | Stored::Writer(_)
            | Stored::Result(_)
            | Stored::EndedByParty(_)
            | Stored::EndedByProject(_)
            | Stored::EndedDelegate(_)
            | Stored::Delegate(_)
            | Stored::Update(_)
            | Stored::Pool(_)
            | Stored::ClosedLedger(_)
            | Stored::DecidedProposal(_)
            | Stored::Stub(_)
            | Stored::Activity(_)
            | Stored::Milestone(_)
            | Stored::History(_)
            | Stored::PlacedHold(_)
            | Stored::PersonProposal(_) => {}
        }
    }
    load_without_judgment(34, &rows, &source.env);
}

#[test]
fn bounded_escalation_rows_load_without_recipient_or_revision_judgments() {
    use jig_domain_tasks::{Escalation, EscalationHolder, Hold, Was, step};
    use skein_lib::Queue;
    let mut source = World::new(36, LIMITS);
    source.make(Party::Person(1), vec![task(1, &[])]);
    for corrupt in 0..6 {
        let mut task = source.record(1).clone();
        task.phase = Phase::Held { was: Was::Active(Active::Due), why: Hold::Deadline, source: None };
        task.escalation = match corrupt {
            0 => Escalation::Routing { revision: 1 },
            1 => Escalation::Unheld { revision: 0 },
            2 => Escalation::Waiting { revision: 0, holder: EscalationHolder::Person(1), entry: 1, since: Wall::EPOCH },
            3 => Escalation::Waiting { revision: 1, holder: EscalationHolder::Person(2), entry: 1, since: Wall::EPOCH },
            4 => Escalation::Waiting {
                revision: 1,
                holder: EscalationHolder::Role { project: 2, role: 0 },
                entry: 1,
                since: Wall::EPOCH,
            },
            5 => Escalation::Rejected {
                revision: 1,
                by: 1,
                reason: vec![b'x'; usize::try_from(LIMITS.result_bytes).expect("configured bounded task result")]
                    .into_boxed_slice(),
            },
            _ => unreachable!(),
        };
        let mut domain = jig_tasks_world::domain(&LIMITS, 36);
        let mut out = Queue::with_capacity(jig_domain_tasks::max_out(&LIMITS));
        let record = Stored::Live(Box::new(task));
        step(&mut domain, &source.env, Event::Restore { record: record.clone() }, &mut out);
        assert!(out.is_empty());
        assert_eq!(domain.stored_record(&record.key()), Some(record));
    }
}

#[test]
fn waiting_restore_requests_recheck_once_and_identical_recipient_changes_nothing() {
    use jig_domain_tasks::{Escalation, EscalationHolder, Hold, Request, Was, step};
    use skein_lib::Queue;
    let mut source = World::new(37, LIMITS);
    source.make(Party::Person(1), vec![task(1, &[])]);
    for rejected in [false, true] {
        let mut domain = jig_tasks_world::domain(&LIMITS, 37);
        let mut out = Queue::with_capacity(jig_domain_tasks::max_out(&LIMITS));
        for record in source.records.values() {
            if matches!(record, Stored::Milestone(_) | Stored::Activity(_) | Stored::History(_) | Stored::Ended(_)) {
                continue;
            }
            let mut record = record.clone();
            if let Stored::Live(task) = &mut record {
                task.phase = Phase::Held { was: Was::Active(Active::Due), why: Hold::Deadline, source: None };
                task.revisions.hold = 4;
                task.escalation = if rejected {
                    Escalation::Rejected { revision: 4, by: 1, reason: b"no".as_slice().into() }
                } else {
                    Escalation::Waiting {
                        revision: 4,
                        holder: EscalationHolder::Person(1),
                        entry: 1,
                        since: source.env.wall,
                    }
                };
            }
            step(&mut domain, &source.env, Event::Restore { record }, &mut out);
        }
        assert!(out.is_empty());
        step(&mut domain, &source.env, Event::Check, &mut out);
        step(&mut domain, &source.env, Event::Restored, &mut out);
        let requests = std::iter::from_fn(|| out.pop()).collect::<Vec<_>>();
        assert_eq!(
            requests.iter().filter(|row| matches!(row, Request::EscalationNeeded { .. })).count(),
            usize::from(!rejected)
        );
        assert!(!requests.iter().any(|row| matches!(row, Request::Save { .. } | Request::Activate { .. })));
        let before = format!("{domain:?}");
        step(
            &mut domain,
            &source.env,
            Event::RoutedEscalation { task: 1, revision: 4, holder: EscalationHolder::Person(1), entry: 1 },
            &mut out,
        );
        assert!(out.is_empty(), "same selected recipient emits no save");
        assert_eq!(format!("{domain:?}"), before);
    }
}
