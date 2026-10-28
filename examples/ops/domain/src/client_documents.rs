//! Same-decision assembly of the application's client documents.
use crate::domain::{Work, held};
use crate::route::{core_out, infrastructure_out, observability_out};
use crate::{ClientDocument, ClientMissing, ClientSubject, Domain, Limits, Output, Write};
use alloc::boxed::Box;
use jig_domain_core as core;
use jig_domain_views as views;
use jig_ops_domain_connectors::Connector;
use jig_ops_domain_infrastructure as infrastructure;
use jig_ops_domain_observability as observability;
use skein_lib::{Decision, Env, Queue, ReplyTo, Token};

pub(crate) fn prepare(_domain: &Domain, token: Token, subject: ClientSubject, work: &mut Queue<Work>) -> u16 {
    match subject {
        ClientSubject::Infrastructure(subject) => {
            work.push(Work::Infrastructure(infrastructure::Event::DocumentPrepare { token, subject }));
            jig_ops_domain_connectors::number(Connector::Infrastructure)
        }
        ClientSubject::Observability(subject) => {
            work.push(Work::Observability(observability::Event::DocumentPrepare { token, subject }));
            jig_ops_domain_connectors::number(Connector::Observability)
        }
    }
}

pub(crate) fn forget(_domain: &Domain, number: u16, subject: Token, work: &mut Queue<Work>) {
    match jig_ops_domain_connectors::connector(number) {
        Some(Connector::Infrastructure) => {
            work.push(Work::Infrastructure(infrastructure::Event::DocumentForget { subject }));
        }
        Some(Connector::Observability) => {
            work.push(Work::Observability(observability::Event::DocumentForget { subject }));
        }
        None => {}
    }
}

fn infrastructure_scope(scope: jig_domain_people::ApplicationScope) -> infrastructure::ClientScope {
    match scope {
        jig_domain_people::ApplicationScope::Project(project) => infrastructure::ClientScope::Project(project),
        jig_domain_people::ApplicationScope::Deployment => infrastructure::ClientScope::Deployment,
        jig_domain_people::ApplicationScope::Own => unreachable!("own document refused before translation"),
    }
}
fn observability_scope(scope: jig_domain_people::ApplicationScope) -> observability::ClientScope {
    match scope {
        jig_domain_people::ApplicationScope::Project(project) => observability::ClientScope::Project(project),
        jig_domain_people::ApplicationScope::Deployment => observability::ClientScope::Deployment,
        jig_domain_people::ApplicationScope::Own => unreachable!("own document refused before translation"),
    }
}

pub(crate) fn ask_document(
    _domain: &Domain,
    number: u16,
    watcher: Token,
    scope: jig_domain_people::ApplicationScope,
    subject: Token,
    party: u64,
    work: &mut Queue<Work>,
) {
    if scope == jig_domain_people::ApplicationScope::Own {
        work.push(Work::Core(core::Event::View(jig_domain_views::Event::Made {
            watcher,
            outcome: jig_domain_views::Made::Gone { why: jig_domain_views::End::Gone },
        })));
        return;
    }
    match jig_ops_domain_connectors::connector(number) {
        Some(Connector::Infrastructure) => {
            work.push(Work::Infrastructure(infrastructure::Event::Document {
                watcher,
                scope: infrastructure_scope(scope),
                subject,
                party,
            }));
        }
        Some(Connector::Observability) => {
            work.push(Work::Observability(observability::Event::Document {
                watcher,
                scope: observability_scope(scope),
                subject,
                party,
            }));
        }
        None => {
            unreachable!("core admits configured connectors only");
        }
    }
}

#[expect(clippy::too_many_arguments, reason = "a connector completes its opaque construction envelope")]
pub(crate) fn document(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision<Write, Output>,
    number: u16,
    watcher: Token,
    subject: Token,
    document: Result<ClientDocument, views::End>,
    work: &mut Queue<Work>,
) {
    let (outcome, mut value) = match document {
        Ok(document) => (views::Made::Ready, Some(document)),
        Err(why) => (views::Made::Gone { why }, None),
    };
    let core::Requests::Out(mut requests) = core::step(
        &mut domain.core,
        &Env { now: env.now, wall: env.wall, limits: env.limits.core },
        core::Event::View(views::Event::Made { watcher, outcome }),
    );
    for _ in 0..requests.len() {
        let request = requests.pop().expect("document completion count");
        match request {
            core::Request::Held(output) => match *output {
                core::Held::ApplicationDelivery { watcher: due, connector: owner, subject: named } => {
                    assert!(
                        watcher == due && number == owner && subject == named,
                        "document keeps its owning connector"
                    );
                    let document = value.take().expect("ready document belongs to this construction");
                    held(decision, Output::ApplicationDocument { watcher, document });
                }
                output @ (core::Held::ApplicationWatchEnded { .. }
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
                | core::Held::ParkRun { .. }
                | core::Held::Load(..)
                | core::Held::NotesLoad { .. }
                | core::Held::NotesWritten { .. }
                | core::Held::Acted { .. }
                | core::Held::NotesDeleted { .. }) => {
                    forward(domain, env, decision, core::Request::Held(Box::new(output)), work);
                }
            },
            request @ (core::Request::Now(_)
            | core::Request::Write(_)
            | core::Request::Ask { .. }
            | core::Request::ProjectMade { .. }
            | core::Request::Decided) => forward(domain, env, decision, request, work),
        }
    }
    assert!(value.is_none(), "the core paced every ready document");
}
fn forward(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision<Write, Output>,
    request: core::Request,
    work: &mut Queue<Work>,
) {
    let mut requests = Queue::with_capacity(1);
    requests.push(request);
    core_out(domain, env, decision, core::Requests::Out(requests), work);
}

#[expect(clippy::too_many_arguments, reason = "the root owns the admitted reply only during this synchronous handoff")]
#[expect(clippy::too_many_lines, reason = "both connector-owned read vocabularies are routed exhaustively")]
pub(crate) fn read(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision<Write, Output>,
    number: u16,
    reply: ReplyTo,
    scope: jig_domain_people::ApplicationScope,
    read: Token,
    party: u64,
    work: &mut Queue<Work>,
) {
    if scope == jig_domain_people::ApplicationScope::Own {
        forget(domain, number, read, work);
        held(decision, Output::ApplicationRead { reply, answer: Err(ClientMissing::Unknown) });
        return;
    }
    let mut reply = Some(reply);
    match jig_ops_domain_connectors::connector(number) {
        Some(Connector::Infrastructure) => {
            let mut out = Queue::with_capacity(infrastructure::MAX_OUT);
            infrastructure::step(
                &mut domain.infrastructure,
                &Env { now: env.now, wall: env.wall, limits: env.limits.infrastructure },
                infrastructure::Event::ReadDocument { scope: infrastructure_scope(scope), read, party },
                &mut out,
            );
            for _ in 0..out.len() {
                let request = out.pop().expect("client read completion count");
                match request {
                    infrastructure::Request::ReadAnswered { read: named, answer } => {
                        assert!(read == named, "the connector echoes its read name");
                        let answer = match answer {
                            Ok(document) => Ok(ClientDocument::Infrastructure(Box::new(document))),
                            Err(why) => Err(match why {
                                infrastructure::DocumentMissing::Unknown => ClientMissing::Unknown,
                                infrastructure::DocumentMissing::NotAllowed => ClientMissing::NotAllowed,
                            }),
                        };
                        held(
                            decision,
                            Output::ApplicationRead { reply: reply.take().expect("one client read terminal"), answer },
                        );
                    }
                    request @ (infrastructure::Request::Document { .. }
                    | infrastructure::Request::Marked { .. }
                    | infrastructure::Request::ApplicationDone { .. }
                    | infrastructure::Request::ApplicationBusy { .. }
                    | infrastructure::Request::Close { .. }
                    | infrastructure::Request::Closed { .. }
                    | infrastructure::Request::Released { .. }
                    | infrastructure::Request::Restarted { .. }
                    | infrastructure::Request::Named { .. }
                    | infrastructure::Request::Slots { .. }
                    | infrastructure::Request::Wake { .. }
                    | infrastructure::Request::Drift { .. }
                    | infrastructure::Request::Changed { .. }
                    | infrastructure::Request::Described { .. }
                    | infrastructure::Request::Refused { .. }
                    | infrastructure::Request::Make { .. }
                    | infrastructure::Request::Outcome { .. }
                    | infrastructure::Request::Step { .. }
                    | infrastructure::Request::Save { .. }
                    | infrastructure::Request::Erase { .. }
                    | infrastructure::Request::System(_)) => {
                        let mut requests = Queue::with_capacity(1);
                        requests.push(request);
                        infrastructure_out(domain, env, decision, &mut requests, work);
                    }
                }
            }
        }
        Some(Connector::Observability) => {
            let mut out = Queue::with_capacity(observability::MAX_OUT);
            observability::step(
                &mut domain.observability,
                &Env { now: env.now, wall: env.wall, limits: env.limits.observability },
                observability::Event::ReadDocument { scope: observability_scope(scope), read, party },
                &mut out,
            );
            for _ in 0..out.len() {
                let request = out.pop().expect("client read completion count");
                match request {
                    observability::Request::ReadAnswered { read: named, answer } => {
                        assert!(read == named, "the connector echoes its read name");
                        let answer = match answer {
                            Ok(document) => Ok(ClientDocument::Observability(Box::new(document))),
                            Err(why) => Err(match why {
                                observability::DocumentMissing::Unknown => ClientMissing::Unknown,
                                observability::DocumentMissing::NotAllowed => ClientMissing::NotAllowed,
                            }),
                        };
                        held(
                            decision,
                            Output::ApplicationRead { reply: reply.take().expect("one client read terminal"), answer },
                        );
                    }
                    request @ (observability::Request::Document { .. }
                    | observability::Request::Marked { .. }
                    | observability::Request::WatchInterest { .. }
                    | observability::Request::Closed { .. }
                    | observability::Request::Released { .. }
                    | observability::Request::Restarted { .. }
                    | observability::Request::Verdict { .. }
                    | observability::Request::Answer { .. }
                    | observability::Request::News { .. }
                    | observability::Request::Health { .. }
                    | observability::Request::Triage { .. }
                    | observability::Request::WatchDecision { .. }
                    | observability::Request::WakeProcedure { .. }
                    | observability::Request::AssociationRefused { .. }
                    | observability::Request::Described { .. }
                    | observability::Request::Refused { .. }
                    | observability::Request::Make { .. }
                    | observability::Request::Outcome { .. }
                    | observability::Request::Save { .. }
                    | observability::Request::Erase { .. }
                    | observability::Request::System(_)
                    | observability::Request::Changed { .. }) => {
                        let mut requests = Queue::with_capacity(1);
                        requests.push(request);
                        observability_out(domain, env, decision, &mut requests, work);
                    }
                }
            }
        }
        None => {
            unreachable!("core admits configured read owners only");
        }
    }
    assert!(reply.is_none(), "the connector answers within the decision");
}
