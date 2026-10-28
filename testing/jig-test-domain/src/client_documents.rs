//! Assemble connector-owned documents in the same decision as the core's pacing.
use crate::routes::{route_connector, route_core};
use crate::{Delivery, Domain, Limits, Work, Write};
use alloc::boxed::Box;
use jig_domain_core as core;
use jig_domain_views as views;
use jig_test_connector as connector;
use skein_lib::{Decision, Env, Queue, ReplyTo, Token};

pub(crate) fn window(value: core::documents::Window) -> connector::ClientWindow {
    connector::ClientWindow {
        rows: value.rows,
        after: match value.after {
            Some(core::documents::Cursor::Number { number }) => Some(connector::ClientCursor::Number(number)),
            Some(core::documents::Cursor::Inbox { .. }) => Some(connector::ClientCursor::Unknown),
            None => None,
        },
    }
}

pub(crate) fn scope(value: jig_domain_people::ApplicationScope) -> connector::ClientScope {
    match value {
        jig_domain_people::ApplicationScope::Project(project) => connector::ClientScope::Project(project),
        jig_domain_people::ApplicationScope::Deployment => connector::ClientScope::Deployment,
        jig_domain_people::ApplicationScope::Own => connector::ClientScope::Own,
    }
}

#[expect(clippy::too_many_arguments, reason = "the construction envelope is routed unchanged")]
pub(crate) fn document(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision<Write, Delivery>,
    number: u16,
    watcher: Token,
    subject: Token,
    document: Result<connector::ClientDocument, connector::DocumentMissing>,
    work: &mut Queue<Work>,
) {
    let (outcome, mut value) = match document {
        Ok(document) => (views::Made::Ready, Some(document)),
        Err(why) => (
            views::Made::Gone {
                why: match why {
                    connector::DocumentMissing::Unknown => views::End::Gone,
                    connector::DocumentMissing::NotAllowed => views::End::NotAllowed,
                },
            },
            None,
        ),
    };
    let core::Requests::Out(mut requests) = core::step(
        &mut domain.core,
        &Env { now: env.now, wall: env.wall, limits: env.limits.core },
        core::Event::View(views::Event::Made { watcher, outcome }),
    );
    for _ in 0..requests.len() {
        let request = requests.pop().expect("document completion requests");
        match request {
            core::Request::Held(held) => match *held {
                core::Held::ApplicationDelivery { watcher: due, connector: owner, subject: named } => {
                    assert!(due == watcher && owner == number && named == subject, "construction keeps its owner");
                    let document = value.take().expect("ready construction owns its whole document");
                    crate::hold(
                        decision,
                        Delivery::ApplicationDocument { watcher, number, document: Box::new(document) },
                    );
                }
                held @ (core::Held::ApplicationWatchEnded { .. }
                | core::Held::ReadLoad { .. }
                | core::Held::Read { .. }
                | core::Held::Applied { .. }
                | core::Held::Delivery { .. }
                | core::Held::WatchEnded { .. }
                | core::Held::MakeEffect { .. }
                | core::Held::SettledCall { .. }
                | core::Held::Assign { .. }
                | core::Held::Grant { .. }
                | core::Held::Relayed { .. }
                | core::Held::Inbound { .. }
                | core::Held::Relay { .. }
                | core::Held::PeopleReply { .. }
                | core::Held::CarriedUnused { .. }
                | core::Held::CallAnswer { .. }
                | core::Held::Result { .. }
                | core::Held::Acknowledge { .. }
                | core::Held::TaskTerminalAcknowledged { .. }
                | core::Held::TaskTurnKept { .. }
                | core::Held::AcknowledgeTurn { .. }
                | core::Held::Drained
                | core::Held::Cancel { .. }
                | core::Held::Park { .. }
                | core::Held::Sources { .. }
                | core::Held::CheckSource { .. }
                | core::Held::SignOutSource { .. }
                | core::Held::SourceSignedOut { .. }
                | core::Held::Refuse { .. }
                | core::Held::TurnBusy { .. }
                | core::Held::StopRun { .. }
                | core::Held::Load(..)
                | core::Held::NotesLoad { .. }
                | core::Held::NotesWritten { .. }
                | core::Held::Acted { .. }
                | core::Held::NotesDeleted { .. }) => {
                    forward(domain, env, decision, core::Request::Held(Box::new(held)), work);
                }
            },
            request @ (core::Request::Write(_)
            | core::Request::Ask { .. }
            | core::Request::Now(_)
            | core::Request::ProjectMade { .. }
            | core::Request::Decided) => forward(domain, env, decision, request, work),
        }
    }
    assert!(value.is_none(), "every ready document was assembled");
}

fn forward(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision<Write, Delivery>,
    request: core::Request,
    work: &mut Queue<Work>,
) {
    let mut requests = Queue::with_capacity(1);
    requests.push(request);
    route_core(domain, env, decision, core::Requests::Out(requests), work);
}

#[expect(clippy::too_many_arguments, reason = "an admitted read completes synchronously with its reply owned here")]
pub(crate) fn read(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision<Write, Delivery>,
    number: u16,
    reply: ReplyTo,
    scope: connector::ClientScope,
    read: Token,
    party: u64,
    window: core::documents::Window,
    work: &mut Queue<Work>,
) {
    let mut reply = Some(reply);
    let mut out = Queue::with_capacity(connector::MAX_OUT);
    connector::step(
        crate::numbered(domain, number),
        &Env { now: env.now, wall: env.wall, limits: env.limits.connector },
        connector::Event::ReadDocument { scope, read, party, window: self::window(window) },
        &mut out,
    );
    for _ in 0..out.len() {
        let request = out.pop().expect("read completion count");
        match request {
            connector::Request::ReadAnswered { read: named, answer } => {
                assert!(read == named, "the connector echoes its read name");
                crate::hold(
                    decision,
                    Delivery::ApplicationRead { reply: reply.take().expect("one read terminal"), number, answer },
                );
            }
            request @ (connector::Request::Document { .. }
            | connector::Request::Marked { .. }
            | connector::Request::ApplicationDone { .. }
            | connector::Request::ApplicationAgain { .. }
            | connector::Request::ApplicationRelease { .. }
            | connector::Request::Resource { .. }
            | connector::Request::Closed { .. }
            | connector::Request::Verdict { .. }
            | connector::Request::Step { .. }
            | connector::Request::Answer { .. }
            | connector::Request::Ready { .. }
            | connector::Request::Section { .. }
            | connector::Request::Workspace { .. }
            | connector::Request::DriftResource { .. }
            | connector::Request::Changed { .. }
            | connector::Request::RestartDone
            | connector::Request::Described { .. }
            | connector::Request::EffectRefused { .. }
            | connector::Request::EffectBusy { .. }
            | connector::Request::Make { .. }
            | connector::Request::Outcome { .. }
            | connector::Request::System(_)
            | connector::Request::Named { .. }
            | connector::Request::Unknown { .. }
            | connector::Request::Refused { .. }
            | connector::Request::Adopted { .. }
            | connector::Request::Slots { .. }
            | connector::Request::Drift { .. }
            | connector::Request::Topic { .. }
            | connector::Request::TopicRefused { .. }
            | connector::Request::News { .. }
            | connector::Request::Save { .. }
            | connector::Request::Erase { .. }) => {
                let mut requests = Queue::with_capacity(1);
                requests.push(request);
                route_connector(domain, env, decision, number, &mut requests, work);
            }
        }
    }
    assert!(reply.is_none(), "the connector answers within the decision");
}

pub(crate) fn role_number(role: Option<jig_domain_people::Role>) -> Option<u32> {
    let role = role?;
    Some(role.number())
}
