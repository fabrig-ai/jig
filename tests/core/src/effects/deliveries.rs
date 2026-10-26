//! Scripted peers consume each root delivery in order.
use super::{Fault, ReplyTo, StoreOut, Token, World, bootstrap, connector, core, people, root};
impl World {
    #[expect(clippy::too_many_lines, reason = "one exhaustive delivery handler drives the script's independent peers")]
    pub(super) fn delivered(&mut self, mut delivery: root::Delivery) {
        self.observer.delivered(self.wall, &mut delivery);
        if let Some(offers) = &mut self.offers {
            offers.delivered(&mut delivery);
        }
        if let Some(watches) = &mut self.watches {
            watches.released(&delivery, self.store.applied);
        }
        if let Some(peer) = &mut self.client_peer {
            match peer.capture(self.store.applied, delivery) {
                Ok(()) => return,
                Err(unowned) => delivery = unowned,
            }
        }
        if let Some(peers) = &mut self.peers {
            peers.delivered(&delivery);
            if let root::Delivery::Core(core::Held::PeopleReply { to, reply }) = delivery {
                peers.reply(to, &reply);
                self.people_answers.push(reply);
                return;
            }
        }
        match delivery {
            root::Delivery::ProjectMade { project } => self.project_made.push((self.store.applied, project)),
            root::Delivery::Core(core::Held::Acted { to, outcome }) => {
                self.act_answers.push((to.into_token(), outcome));
            }
            root::Delivery::Core(core::Held::Sources { channel, sources }) => {
                self.events.push_back(root::Event::Core(core::Event::Fleet(jig_domain_fleet::Event::Logins {
                    channel,
                    logins: sources
                        .iter()
                        .map(|source| jig_domain_fleet::Login {
                            name: *source,
                            state: jig_domain_fleet::LoginState::Usable,
                        })
                        .collect(),
                })));
            }
            root::Delivery::ApplicationRead { reply, number, answer } => {
                if let Some(reads) = &mut self.reads {
                    reads.application_replied(reply, number, answer, &self.store);
                }
            }
            root::Delivery::Core(core::Held::Read { reply, answer }) => {
                if let Some(reads) = &mut self.reads {
                    reads.replied(reply, answer, &self.store);
                }
            }
            root::Delivery::Core(core::Held::ReadLoad { load, range }) => {
                let requests = if let Some(reads) = &mut self.reads {
                    reads.load(load, *range, &self.store)
                } else {
                    vec![StoreOut::ReadLoad { load, range }]
                };
                for request in requests {
                    self.store.ask(request);
                }
            }
            root::Delivery::Core(
                core::Held::Applied { .. }
                | core::Held::ApplicationDelivery { .. }
                | core::Held::ApplicationWatchEnded { .. },
            ) => {
                panic!("root assembles connector outcomes before journaling")
            }
            root::Delivery::Application { to, number, outcome } => {
                self.application_answers.push((to.into_token(), number, outcome));
            }
            root::Delivery::Core(core::Held::CarriedUnused { number }) => {
                let before = self
                    .people_answers
                    .len()
                    .checked_add(self.application_answers.len())
                    .and_then(|count| count.checked_add(self.act_answers.len()));
                self.carried_unused.push((before.expect("bounded story answers"), number));
            }
            root::Delivery::Projection { connector, feed } => {
                if feed.closing && !self.hold_projection_settlements {
                    self.events.push_back(root::Event::Core(core::Event::ProjectionSettled {
                        goal: feed.goal.number,
                        connector,
                    }));
                }
                self.projections.push((self.commits.len(), connector, *feed));
            }
            root::Delivery::Restart(step) => self.restart_step(step),
            root::Delivery::CallBusy { name, .. } => self.busy_calls.push(name),
            root::Delivery::Core(core::Held::TurnBusy { run, attempt, turn, .. }) => {
                self.busy_turns.push((run.raw(), attempt.raw(), turn));
            }
            root::Delivery::Core(
                held @ (core::Held::Drained | core::Held::WatchEnded { .. } | core::Held::Acknowledge { .. }),
            ) => {
                self.stop_deliveries.push(held);
            }
            root::Delivery::CallAnswer { name, call, .. } => self.host_answers.push((name, call)),
            root::Delivery::Message { word, .. } => self.inbound.push(word),
            root::Delivery::Assigned { assignment, .. } => {
                self.assigned_answers =
                    assignment.answered.iter().cloned().map(|call| self.protocol.replay(call)).collect();
                self.last_read = None;
                self.assignments.push((assignment.task, assignment.attempt));
                self.assigned = Some(assignment);
            }
            root::Delivery::Core(core::Held::Grant { channel, run, attempt, grant }) => {
                self.grant_delivered(channel, run, attempt, grant);
            }
            root::Delivery::Core(core::Held::Cancel { run, attempt, .. }) => {
                self.cancellations.push((run.raw(), attempt.raw()));
            }
            root::Delivery::Procedure { task, step, connector: number, code, context } => {
                self.procedure = Some(task);
                self.procedure_inputs.push((step, number, code, context.clone()));
                if self.hold_procedures {
                    return;
                }
                self.events.push_back(root::Event::Connector {
                    number,
                    event: root::procedure_input(&context, code, jig_test_connector_world::path(1, 1)),
                });
            }
            root::Delivery::LostRead { connector, resource, call, .. } => {
                if self.hold_lost_reads {
                    self.pending_lost_reads.push((connector, resource, call));
                    return;
                }
                let event = self.systems[usize::from(connector - 1)].answer(call, Fault::None);
                self.events.push_back(root::Event::WriterRead { connector, resource, event });
            }
            root::Delivery::System {
                connector: number,
                call: connector::SystemRequest::ReadFact { resource, observed },
            } => {
                let call = connector::SystemRequest::ReadFact { resource, observed };
                if self.restart_reads.contains_key(&number) && !self.hold_restart_reads {
                    let event = self.systems[usize::from(number - 1)].answer(call, Fault::None);
                    self.events.push_back(root::Event::Connector { number, event: connector::Event::System(event) });
                    let step = self.restart_reads.remove(&number).expect("restart read");
                    self.events.push_back(root::Event::RestartDone(step));
                } else {
                    self.pending_reads.push((number, call));
                }
            }
            root::Delivery::System { connector: number, call } => {
                if let connector::SystemRequest::Apply { entry, key, .. } = &call {
                    assert!(self.store.rows.values().any(|row| matches!(row, root::Record::Connector { number: owner, record: connector::Record::Outbox(row) } if *owner == number && row.number == *entry && row.key == *key)), "system sees only a committed outbox attempt");
                    assert_eq!(key.deployment, [31; 16]);
                }
                if self.hold_lookups && matches!(call, connector::SystemRequest::Look { .. }) {
                    self.pending_lookups.push((number, call));
                    return;
                }
                if self.hold_writes && matches!(call, connector::SystemRequest::Apply { .. }) {
                    self.pending_writes.push((number, call));
                    return;
                }
                let event = self.systems[usize::from(number - 1)].answer(call, Fault::None);
                self.events.push_back(root::Event::Connector { number, event: connector::Event::System(event) });
            }
            root::Delivery::Core(core::Held::PeopleReply {
                reply: reply @ people::Reply::SignedIn { person, sign_in, .. },
                ..
            }) => {
                let initial = self.sign_in.is_none();
                self.sign_in = Some(sign_in);
                self.people_answers.push(reply);
                bootstrap::signed_in(self, person);
                if !initial {
                    return;
                }
                self.events.push_back(root::Event::Core(core::Event::People(people::Event::Ask {
                    models: Box::new([]),
                    reply_to: ReplyTo::new(Token::new(102)),
                    sign_in,
                    key: [5; 16],
                    ask: self.initial_ask.clone(),
                })));
            }
            root::Delivery::Core(core::Held::Load(load)) => self.store.ask(jig_test_store::StoreOut::Now(*load)),
            root::Delivery::Core(core::Held::NotesLoad { owner, range }) => {
                self.store.ask(StoreOut::NotesLoad { owner, range });
            }
            root::Delivery::Core(core::Held::CallAnswer { to, key, part }) => {
                let token = to.into_token();
                assert!(
                    self.store.rows.contains_key(&root::Key::Core(core::Key::Core(core::CoreKey::Call(key)))),
                    "state-changing answer has its committed record"
                );
                if self.lose_answer {
                    self.lost_answers += 1;
                } else {
                    if let Some(event) = self
                        .peers
                        .as_mut()
                        .map_or(&mut self.protocol, |peers| &mut peers.protocol)
                        .answer(token, key, &part, true)
                    {
                        self.events.push_back(event);
                    }
                    self.answers.push((token, key, part));
                }
            }
            root::Delivery::Core(core::Held::PeopleReply { reply, .. }) => self.people_answers.push(reply),
            root::Delivery::ApplicationDocument { .. }
            | root::Delivery::Link { .. }
            | root::Delivery::Core(
                core::Held::Delivery { .. }
                | core::Held::SettledCall { .. }
                | core::Held::Assign { .. }
                | core::Held::Inbound { .. }
                | core::Held::Relayed { .. }
                | core::Held::Relay { .. }
                | core::Held::Result { .. }
                | core::Held::AcknowledgeTurn { .. }
                | core::Held::TaskTurnKept { .. }
                | core::Held::TaskTerminalAcknowledged { .. }
                | core::Held::NotesWritten { .. }
                | core::Held::NotesDeleted { .. }
                | core::Held::StopRun { .. }
                | core::Held::CheckSource { .. }
                | core::Held::SignOutSource { .. }
                | core::Held::SourceSignedOut { .. }
                | core::Held::Refuse { .. }
                | core::Held::MakeEffect { .. },
            ) => {}
            root::Delivery::Fleet(_) => panic!("root releases its fleet continuations internally"),
        }
    }
}
