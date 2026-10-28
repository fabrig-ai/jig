//! Admission at the service boundary by backlog watermark (skein's draft `dynamic-memory.md`, 4).
use crate::{Domain, Event, Limits, step};
use alloc::boxed::Box;
use jig_domain_core as core;
use skein_lib::Env;

/// Return an unadmitted input to its owner; a store terminal always enters its root-owned queue.
#[expect(clippy::result_large_err, reason = "the service admission boundary returns the original event by value")]
pub fn try_step(domain: &mut Domain, env: &Env<Limits>, event: Event) -> Result<(), Event> {
    match event {
        event @ (Event::Committed { .. } | Event::Failed { .. }) => {
            step(domain, env, event);
            Ok(())
        }
        event @ (Event::Switched
        | Event::RestartDone(_)
        | Event::TranscriptLoaded { .. }
        | Event::Core(
            core::Event::ReadLoaded { .. }
            | core::Event::EndedReadLoaded { .. }
            | core::Event::SeenConversationLoaded { .. }
            | core::Event::HistoricalPersonTask { .. }
            | core::Event::PartyInputsLoaded { .. }
            | core::Event::HistoricalProposal { .. }
            | core::Event::DecidedProposal { .. }
            | core::Event::HistoricalEscalation { .. }
            | core::Event::Notes(_),
        )) => {
            domain.store_answers.push(Box::new(event));
            Ok(())
        }
        event @ (Event::Link { .. }
        | Event::ResultRead { .. }
        | Event::Stop
        | Event::Application { .. }
        | Event::ConnectorTimer { .. }
        | Event::WriterRead { .. }
        | Event::RestartBegin { .. }
        | Event::ProjectionEffect { .. }
        | Event::TopicCall { .. }
        | Event::EffectCall { .. }
        | Event::Timer(_)
        | Event::Core(_)
        | Event::Credential { .. }
        | Event::WatchApplication { .. }
        | Event::ReadApplication { .. }
        | Event::CallAnswered { .. }
        | Event::Connector { .. }
        | Event::Text { .. }
        | Event::Turn { .. }
        | Event::Answer { .. }) => {
            if !admits(domain, &env.limits) {
                return Err(event);
            }
            step(domain, env, event);
            Ok(())
        }
    }
}

/// What the root holds of admitted work, in entries (the draft's 4.1):
/// the journal's backlog, the queued store answers, and its own `now`
/// and `busy` outputs not yet released.
pub(crate) fn backlog(domain: &Domain) -> u64 {
    domain
        .journal
        .backlog()
        .checked_add(u64::from(domain.store_answers.len()))
        .expect("four u32 counts fit in u64")
        .checked_add(u64::from(domain.now.len()))
        .expect("five u32 counts fit in u64")
        .checked_add(u64::from(domain.busy.len()))
        .expect("six u32 counts fit in u64")
}

/// New external work enters only while the journal runs and the backlog
/// is under the watermark (the draft's 4.2).
pub(crate) fn admits(domain: &Domain, limits: &Limits) -> bool {
    !domain.journal.stopped() && backlog(domain) < u64::from(limits.watermark)
}

pub(super) fn resume(domain: &mut Domain, env: &Env<Limits>) {
    for _ in 0..domain.store_answers.len() {
        if domain.journal.stopped() {
            break;
        }
        let event = domain.store_answers.pop().expect("queued store terminal");
        step(domain, env, *event);
    }
}

#[cfg(test)]
mod tests {
    use super::try_step;
    use crate::{Event, tests::fixtures};
    use jig_domain_core as core;
    use jig_domain_people as people;
    use skein_lib::{ReplyTo, Token};

    #[test]
    fn new_work_waits_at_the_watermark_and_its_commit_answer_is_taken() {
        let env = fixtures::env();
        let mut domain = fixtures::domain();
        fixtures::fill_to_watermark(&mut domain, &env);
        let event = Event::Core(core::Event::People(people::Event::Ask {
            models: Box::new([]),
            reply_to: ReplyTo::new(Token::new(7)),
            sign_in: 1,
            key: [1; 16],
            ask: people::Ask::Say { project: 1, task: 1, words: b"kept".as_slice().into() },
        }));
        let event = try_step(&mut domain, &env, event).expect_err("new work waits at the watermark");
        let Event::Core(core::Event::People(people::Event::Ask { ask, .. })) = &event else {
            panic!("unadmitted input returns to its owner");
        };
        let people::Ask::Say { words, .. } = ask else {
            panic!("same party input");
        };
        assert_eq!(words.as_ref(), b"kept");
        try_step(&mut domain, &env, Event::Committed { number: 1, fill: 0 }).expect("commit answers are taken");
        try_step(&mut domain, &env, event).expect("the same input enters below the watermark");
    }
}

#[cfg(test)]
mod expiry_tests {
    use crate::{Event, tests::fixtures};
    use jig_domain_core as core;
    use jig_domain_people as people;
    use skein_lib::{Env, Queue, Time, Wall};

    #[test]
    fn refused_wall_expiry_timer_keeps_membership_and_both_deadlines_due() {
        let mut env = fixtures::env();
        let mut domain = fixtures::domain();
        let expiry = Wall::from_nanos(10);
        let mut ignored = Queue::with_capacity(people::max_out(&env.limits.core.people));
        for record in [
            people::Stored::Person {
                number: 1,
                identity: people::Identity {
                    key: people::IdentityKey { provider: 0, subject: Box::new([1]) },
                    login: Box::new([]),
                    name: Box::new([]),
                },
            },
            people::Stored::SignIn { number: 1, person: 1, expires: expiry },
        ] {
            people::step(
                &mut domain.core.people,
                &Env { now: env.now, wall: env.wall, limits: env.limits.core.people },
                people::Event::Restore { record },
                &mut ignored,
            );
        }
        people::step(
            &mut domain.core.people,
            &Env { now: env.now, wall: env.wall, limits: env.limits.core.people },
            people::Event::Restored,
            &mut ignored,
        );
        let elapsed = domain.core.people.next_deadline();
        assert!(domain.core.people.ready());
        let commits = fixtures::fill_to_watermark(&mut domain, &env);
        for _ in 0..commits {
            domain.core.counters.next_commit();
        }
        env.now = Time::from_nanos(2);
        env.wall = expiry;
        let projected = domain.core_deadlines(env.now, env.wall);
        assert!(projected.contains(&(core::Timer::People, Some(env.now))));
        assert!(super::try_step(&mut domain, &env, Event::Timer(core::Timer::People)).is_err());
        assert_eq!(domain.core_deadlines(env.now, env.wall), projected, "refusal retains the service deadline");
        assert_eq!(domain.core.people.sign_in_person(1), Some(1));
        assert_eq!(domain.core.people.next_wall_expiry(), Some(expiry));
        assert_eq!(domain.core.people.next_deadline(), elapsed);
        assert!(crate::timers_due(&domain, &env));
        super::try_step(&mut domain, &env, Event::Committed { number: 1, fill: 0 })
            .expect("admitted timer or durable terminal");
        super::try_step(&mut domain, &env, Event::Timer(core::Timer::People))
            .expect("admitted timer or durable terminal");
        assert_eq!(domain.core.people.sign_in_person(1), None);
        assert_eq!(domain.core.people.next_wall_expiry(), None);
        assert_eq!(domain.core.people.next_deadline(), None);
    }
}
