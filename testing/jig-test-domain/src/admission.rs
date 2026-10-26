//! Admission at the service boundary; store terminals keep their reserved rights (service.md, 4).
use crate::{Domain, Event, Limits, step};
use alloc::boxed::Box;
use jig_domain_core as core;
use skein_lib::{Env, JournalRoom};

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
            let room = room(env, &event);
            if !domain.journal.takes(&room) {
                return Err(event);
            }
            step(domain, env, event);
            Ok(())
        }
    }
}

fn room(env: &Env<Limits>, event: &Event) -> JournalRoom {
    let core = match event {
        Event::Core(event) => core::room(&env.limits.core, event),
        Event::Link { .. }
        | Event::ResultRead { .. }
        | Event::Stop
        | Event::Application { .. }
        | Event::ConnectorTimer { .. }
        | Event::WriterRead { .. }
        | Event::Switched
        | Event::RestartBegin { .. }
        | Event::RestartDone(_)
        | Event::TranscriptLoaded { .. }
        | Event::Committed { .. }
        | Event::Failed { .. }
        | Event::ProjectionEffect { .. }
        | Event::TopicCall { .. }
        | Event::EffectCall { .. }
        | Event::Timer(_)
        | Event::Credential { .. }
        | Event::WatchApplication { .. }
        | Event::ReadApplication { .. }
        | Event::CallAnswered { .. }
        | Event::Connector { .. }
        | Event::Text { .. }
        | Event::Turn { .. }
        | Event::Answer { .. } => core::room_max(&env.limits.core),
    }
    .expect("validated core room");
    let connectors = jig_test_connector::MAX_OUT.checked_mul(2).expect("two connectors");
    JournalRoom {
        writes: core.writes.checked_add(connectors).expect("root writes"),
        held: core.held.checked_add(connectors).expect("root deliveries"),
    }
}

pub(super) fn resume(domain: &mut Domain, env: &Env<Limits>) {
    for _ in 0..domain.store_answers.len() {
        let Some(event) = domain.store_answers.iter().next() else {
            break;
        };
        if !domain.journal.takes(&room(env, event)) {
            break;
        }
        let event = domain.store_answers.pop().expect("reserved store terminal");
        step(domain, env, *event);
    }
}

#[cfg(test)]
mod tests {
    use super::try_step;
    use crate::{Event, tests::fixtures};
    use jig_domain_core as core;
    use jig_domain_people as people;
    use skein_lib::{JournalRoom, ReplyTo, Token};

    #[test]
    fn a_full_journal_returns_its_input_and_takes_its_commit_answer() {
        let env = fixtures::env();
        let mut domain = fixtures::domain();
        fixtures::fill_journal(&mut domain, &env);
        let event = Event::Core(core::Event::People(people::Event::Ask {
            models: Box::new([]),
            reply_to: ReplyTo::new(Token::new(7)),
            sign_in: 1,
            key: [1; 16],
            ask: people::Ask::Say { project: 1, task: 1, words: b"kept".as_slice().into() },
        }));
        let Err(Event::Core(core::Event::People(people::Event::Ask { ask, .. }))) = try_step(&mut domain, &env, event)
        else {
            panic!("unadmitted input returns to its owner");
        };
        let people::Ask::Say { words, .. } = ask else {
            panic!("same party input");
        };
        assert_eq!(words.as_ref(), b"kept");
        try_step(&mut domain, &env, Event::Committed { number: 1, fill: 0 }).expect("commit answers are taken");
        assert!(domain.journal.takes(&JournalRoom { writes: 1, held: 0 }), "commit answer frees the journal");
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
        fixtures::fill_journal(&mut domain, &env);
        for _ in 0..env.limits.journal.commits {
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
