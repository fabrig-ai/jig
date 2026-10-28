//! Root admission by variant and backlog (programming-model.md, 7.1–7.2).
use crate::{Domain, Event, Limits, step};
use alloc::boxed::Box;
use jig_domain_core as core;
use jig_domain_host as host;
use jig_test_connector as connector;
use skein_lib::Env;

/// How the root takes an event (programming-model.md, 7.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    /// Finishes or advances admitted work, taken at any backlog.
    Admitted,
    /// New work, handed back unchanged at the watermark.
    New,
    /// Its admitted part is stepped; its new part waits at the watermark.
    Mixed,
}

/// Class an event by its variant and payload alone.
#[must_use]
pub fn class(event: &Event) -> Class {
    match event {
        Event::Core(event) => {
            if event.new_work() {
                Class::New
            } else {
                Class::Admitted
            }
        }
        Event::Timer(timer) => {
            if timer.new_work() {
                Class::New
            } else {
                Class::Admitted
            }
        }
        Event::Link { link, .. } => match link {
            crate::link::Event::Hello { .. } | crate::link::Event::Lost => Class::Admitted,
            crate::link::Event::Up(record) => match record {
                host::LinkUp::Call { .. } => Class::New,
                host::LinkUp::Piece { .. }
                | host::LinkUp::Logins { .. }
                | host::LinkUp::SignedOut { .. }
                | host::LinkUp::Activity { .. }
                | host::LinkUp::Unauthorized { .. }
                | host::LinkUp::Limited { .. }
                | host::LinkUp::AccountRefused { .. }
                | host::LinkUp::Turn { .. }
                | host::LinkUp::Answer { .. } => Class::Admitted,
            },
        },
        Event::Connector { event, .. } => connector_class(event),
        Event::WriterRead { event, .. } => match event {
            connector::SystemEvent::Fact { origin: connector::Origin::Other, .. } => Class::Mixed,
            connector::SystemEvent::Fact { origin: connector::Origin::Own, .. }
            | connector::SystemEvent::Settle { .. }
            | connector::SystemEvent::Drift { .. }
            | connector::SystemEvent::Applied { .. }
            | connector::SystemEvent::Looked { .. }
            | connector::SystemEvent::News { .. }
            | connector::SystemEvent::Unavailable { .. }
            | connector::SystemEvent::Pool { .. } => Class::Admitted,
        },
        Event::WatchApplication { .. }
        | Event::ReadApplication { .. }
        | Event::Application { .. }
        | Event::RestartBegin { .. }
        | Event::Credential { .. }
        | Event::ProjectionEffect { .. }
        | Event::TopicCall { .. }
        | Event::EffectCall { .. } => Class::New,
        Event::ResultRead { .. }
        | Event::Stop
        | Event::ConnectorTimer { .. }
        | Event::Switched
        | Event::RestartDone(..)
        | Event::CallAnswered { .. }
        | Event::Text { .. }
        | Event::Turn { .. }
        | Event::Answer { .. }
        | Event::TranscriptLoaded { .. }
        | Event::Committed { .. }
        | Event::Failed { .. } => Class::Admitted,
    }
}

fn connector_class(event: &connector::Event) -> Class {
    match event {
        connector::Event::System(event) => match event {
            connector::SystemEvent::Fact { origin: connector::Origin::Other, .. } => Class::Mixed,
            connector::SystemEvent::Fact { origin: connector::Origin::Own, .. }
            | connector::SystemEvent::Settle { .. }
            | connector::SystemEvent::Applied { .. }
            | connector::SystemEvent::Looked { .. } => Class::Admitted,
            connector::SystemEvent::Drift { .. }
            | connector::SystemEvent::News { .. }
            | connector::SystemEvent::Unavailable { .. }
            | connector::SystemEvent::Pool { .. } => Class::New,
        },
        connector::Event::DocumentPrepare { .. }
        | connector::Event::DocumentForget { .. }
        | connector::Event::Document { .. }
        | connector::Event::ReadDocument { .. }
        | connector::Event::ApplicationPrepare { .. }
        | connector::Event::ApplicationDrop { .. }
        | connector::Event::Application { .. }
        | connector::Event::ApplicationCore { .. }
        | connector::Event::Close { .. }
        | connector::Event::EffectDecision { .. }
        | connector::Event::Judge { .. }
        | connector::Event::Procedure { .. }
        | connector::Event::StepMade { .. }
        | connector::Event::StepRefused { .. }
        | connector::Event::Read { .. }
        | connector::Event::Gather { .. }
        | connector::Event::Cut { .. }
        | connector::Event::Items { .. }
        | connector::Event::HandOver { .. }
        | connector::Event::Restart(..)
        | connector::Event::Names { .. }
        | connector::Event::Unnamed { .. }
        | connector::Event::Adopt { .. }
        | connector::Event::DescribeTopic { .. }
        | connector::Event::Subscribe { .. }
        | connector::Event::Unsubscribe { .. }
        | connector::Event::Describe { .. }
        | connector::Event::Keep { .. }
        | connector::Event::DescribeProposal { .. }
        | connector::Event::KeepProposal { .. }
        | connector::Event::DropProposal { .. }
        | connector::Event::Drop { .. }
        | connector::Event::Make { .. }
        | connector::Event::Resolve { .. }
        | connector::Event::Withdraw { .. }
        | connector::Event::Restore { .. } => Class::Admitted,
    }
}

/// Return new work to its owner at the watermark; store answers keep arrival order.
#[expect(clippy::result_large_err, reason = "the admission boundary returns the original owned event")]
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
            | core::Event::EndedTaskLoaded { .. }
            | core::Event::SeenConversationLoaded { .. }
            | core::Event::HistoricalPersonTask { .. }
            | core::Event::PartyInputsLoaded { .. }
            | core::Event::HistoricalProposal { .. }
            | core::Event::DecidedProposal { .. }
            | core::Event::HistoricalEscalation { .. },
        )) => {
            domain.store_answers.push(Box::new(event));
            Ok(())
        }
        event @ Event::Core(core::Event::Notes(_)) if class(&event) == Class::Admitted => {
            domain.store_answers.push(Box::new(event));
            Ok(())
        }
        event @ (Event::Link { .. }
        | Event::ResultRead { .. }
        | Event::Stop
        | Event::WatchApplication { .. }
        | Event::ReadApplication { .. }
        | Event::Application { .. }
        | Event::WriterRead { .. }
        | Event::ConnectorTimer { .. }
        | Event::RestartBegin { .. }
        | Event::Timer(..)
        | Event::Core(..)
        | Event::Credential { .. }
        | Event::Connector { .. }
        | Event::ProjectionEffect { .. }
        | Event::TopicCall { .. }
        | Event::EffectCall { .. }
        | Event::CallAnswered { .. }
        | Event::Text { .. }
        | Event::Turn { .. }
        | Event::Answer { .. }) => match class(&event) {
            Class::Admitted => {
                step(domain, env, event);
                Ok(())
            }
            Class::New => {
                if !admits(domain, &env.limits) {
                    return Err(event);
                }
                step(domain, env, event);
                Ok(())
            }
            Class::Mixed => {
                if admits(domain, &env.limits) {
                    step(domain, env, event);
                    return Ok(());
                }
                let (admitted, rest) = split(domain, event);
                step(domain, env, admitted);
                match rest {
                    Some(rest) => Err(rest),
                    None => Ok(()),
                }
            }
        },
    }
}

#[expect(clippy::manual_map, reason = "step code wraps owned remainders without closures")]
fn split(domain: &mut Domain, event: Event) -> (Event, Option<Event>) {
    match event {
        Event::Connector {
            number,
            event: connector::Event::System(connector::SystemEvent::Fact { resource, fact, origin }),
        } => {
            let (settle, drift) = connector::split(crate::numbered(domain, number), resource, fact, origin);
            let rest = match drift {
                Some(event) => Some(Event::Connector { number, event: connector::Event::System(event) }),
                None => None,
            };
            (Event::Connector { number, event: connector::Event::System(settle) }, rest)
        }
        Event::WriterRead {
            connector: number,
            resource,
            event: connector::SystemEvent::Fact { resource: observed, fact, origin },
        } => {
            // A read the existing route would ignore has no work to defer either.
            if fact.pending || observed.segments() != resource.path.as_ref() {
                return (
                    Event::WriterRead {
                        connector: number,
                        resource,
                        event: connector::SystemEvent::Fact { resource: observed, fact, origin },
                    },
                    None,
                );
            }
            let (settle, drift) = connector::split(crate::numbered(domain, number), observed, fact, origin);
            let rest = match drift {
                Some(event) => Some(Event::Connector { number, event: connector::Event::System(event) }),
                None => None,
            };
            (Event::WriterRead { connector: number, resource, event: settle }, rest)
        }
        Event::Link { .. }
        | Event::ResultRead { .. }
        | Event::Stop
        | Event::WatchApplication { .. }
        | Event::ReadApplication { .. }
        | Event::Application { .. }
        | Event::WriterRead { .. }
        | Event::ConnectorTimer { .. }
        | Event::RestartBegin { .. }
        | Event::Switched
        | Event::RestartDone(..)
        | Event::Timer(..)
        | Event::Core(..)
        | Event::Credential { .. }
        | Event::Connector { .. }
        | Event::ProjectionEffect { .. }
        | Event::TopicCall { .. }
        | Event::EffectCall { .. }
        | Event::CallAnswered { .. }
        | Event::Text { .. }
        | Event::Turn { .. }
        | Event::Answer { .. }
        | Event::TranscriptLoaded { .. }
        | Event::Committed { .. }
        | Event::Failed { .. } => unreachable!("only a mixed fact is split"),
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
    fn wall_expiry_is_taken_at_the_watermark() {
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
        assert!(domain.core.people.ready());
        let commits = fixtures::fill_to_watermark(&mut domain, &env);
        for _ in 0..commits {
            domain.core.counters.next_commit();
        }
        env.now = Time::from_nanos(2);
        env.wall = expiry;
        super::try_step(&mut domain, &env, Event::Timer(core::Timer::People))
            .expect("sign-in expiry advances admitted work even at the watermark");
        assert_eq!(domain.core.people.sign_in_person(1), None);
        assert_eq!(domain.core.people.next_wall_expiry(), None);
        assert_eq!(domain.core.people.next_deadline(), None);
    }
}
