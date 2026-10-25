//! The walking world's scripted delivery responses.
use super::{ReplyTo, Scenario, Token, World, core, people, root, tasks};
impl World {
    #[expect(clippy::too_many_lines, reason = "one script handles each externally observed delivery")]
    pub(super) fn delivered(&mut self, mut delivery: root::Delivery) {
        self.observer.delivered(self.iteration * 1_000_000, &mut delivery);
        match delivery {
            root::Delivery::Core(core::Held::Sources { channel, sources }) => {
                assert!(sources.is_empty(), "walking worker source fixture");
                self.events.push_back(root::Event::Core(core::Event::Fleet(jig_domain_fleet::Event::Logins {
                    channel,
                    logins: Box::new([]),
                })));
            }
            root::Delivery::Projection { connector, feed } => {
                if feed.closing {
                    self.events.push_back(root::Event::Core(core::Event::ProjectionSettled {
                        goal: feed.goal.number,
                        generation: feed.generation,
                        connector,
                    }));
                }
            }
            root::Delivery::Restart(_) => panic!("walking fixture uses the restored-marker path"),
            root::Delivery::Core(core::Held::Load(load)) => self.store.ask(jig_test_store::StoreOut::Now(*load)),
            root::Delivery::Core(core::Held::NotesLoad { owner, range }) => {
                self.store.ask(jig_test_store::StoreOut::NotesLoad { owner, range });
            }
            root::Delivery::Core(core::Held::PeopleReply {
                reply: people::Reply::SignedIn { sign_in, .. }, ..
            }) => {
                self.sign_in = Some(sign_in);
                let people::Ask::Create { project, mut batch } =
                    crate::fixtures::create_chat(&self.creating, 1, 0, b"hello".as_slice().into())
                else {
                    unreachable!()
                };
                if matches!(self.scenario, Scenario::Goal | Scenario::Batch) {
                    let member = &mut batch[0];
                    member.words = b"complete procedure".as_slice().into();
                    member.flags.conversation = false;
                    member.flags.listed = true;
                    member.label = jig_client_test_records::GOAL;
                    member.tracked = Some(1);
                    member.budget = people::Budget::Named;
                    member.authority.spend = 100;
                }
                let ask = people::Ask::Create { project, batch };
                self.events.push_back(root::Event::Core(core::Event::People(people::Event::Ask {
                    models: Box::new([]),
                    reply_to: ReplyTo::new(Token::new(102)),
                    sign_in,
                    key: [5; 16],
                    ask,
                })));
            }
            root::Delivery::Core(core::Held::PeopleReply {
                reply: people::Reply::Outcome(people::Outcome::Created { first: task, .. }),
                ..
            }) => {
                self.chat = Some(task);
                assert!(
                    self.store.rows.contains_key(&root::Key::Core(core::Key::Tasks(tasks::Key::Live(task)))),
                    "party saw task only after commit"
                );
            }
            root::Delivery::Assigned { channel, assignment } => {
                assert_eq!(channel, Token::new(7));
                assert!(
                    self.store
                        .rows
                        .contains_key(&root::Key::Core(core::Key::Core(core::CoreKey::RunProof(assignment.task))))
                );
                let task = assignment.task;
                let attempt = assignment.attempt;
                self.assignment = Some(assignment);
                match self.scenario {
                    Scenario::Chat => self.events.push_back(root::Event::Turn {
                        opens_window: false,
                        refused: Box::new([]),
                        channel,
                        task,
                        attempt,
                        turn: 1,
                        read: None,
                        cumulative: 3,
                        transcript: b"answer".as_slice().into(),
                    }),
                    Scenario::Goal | Scenario::Batch => {
                        let authority = tasks::Authority {
                            tools: tasks::Tools(0),
                            models: tasks::Models::NONE,
                            grants: Box::new([]),
                            delegation: tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 },
                            budget: tasks::Budget { spend: 0, deadline: None },
                            notes: tasks::Scopes(0),
                            note_resources: Box::new([]),
                        };
                        self.delegated = true;
                        let delegate = core::Delegate {
                            label: 0,
                            flags: jig_domain_tasks::Flags::PLAIN,
                            tracked: None,
                            budget: jig_domain_people::Budget::Named,
                            executor: tasks::Executor::Procedure { connector: 1, code: 1 },
                            spec: tasks::Spec {
                                words: b"run the check".as_slice().into(),
                                parameters: Box::new([]),
                                inputs: Box::new([]),
                            },
                            contract: tasks::Contract::Report { words: 0 },
                            authority,
                            llm: None,
                            symbolic_grants: Box::new([]),
                            dependencies: Box::new([]),
                            wake: tasks::WakePolicy::DEFAULT,
                        };
                        let batch = match self.scenario {
                            Scenario::Goal => vec![delegate].into_boxed_slice(),
                            Scenario::Batch => vec![delegate.clone(), delegate].into_boxed_slice(),
                            Scenario::Chat => unreachable!("chat does not delegate"),
                        };
                        self.events.push_back(root::Event::Core(core::Event::DelegateValidated {
                            to: ReplyTo::new(Token::new(104)),
                            key: core::CallKey { task, attempt, completion: 1, position: 0 },
                            batch,
                            stubs: Box::new([]),
                        }));
                    }
                }
            }
            root::Delivery::Procedure { task, connector, code, context, .. } => {
                self.procedure = Some(task);
                self.events.push_back(root::Event::Connector {
                    number: connector,
                    event: root::procedure_input(&context, code, jig_test_connector_world::path(1, 1)),
                });
            }
            root::Delivery::Core(core::Held::CallAnswer { part: core::CallPart::Delegated(tasks), .. }) => {
                assert_eq!(tasks.len(), 1);
                self.procedure = Some(tasks[0].task);
            }
            root::Delivery::Core(core::Held::CallAnswer {
                part: core::CallPart::DelegationDenied { findings, .. },
                ..
            }) => {
                assert_eq!(self.scenario, Scenario::Batch);
                assert!(!findings.is_empty(), "worker receives a reason for the atomic refusal");
                self.batch_refusals += 1;
            }
            root::Delivery::Core(core::Held::CallAnswer {
                part: core::CallPart::DelegationRefused(problem), ..
            }) => {
                assert_eq!(self.scenario, Scenario::Batch);
                assert_eq!(problem.why, tasks::Refusal::Batch);
                self.batch_refusals += 1;
            }
            root::Delivery::Message { channel, task: run, attempt, word } => {
                assert_eq!(word.from, tasks::Party::Task(self.procedure.expect("delegated child")));
                self.events.push_back(root::Event::Answer {
                    read: None,
                    refused: Box::new([]),
                    channel,
                    task: run,
                    attempt,
                    cumulative: 3,
                    end: tasks::End::Finished {
                        result: tasks::TaskResult::Report { words: b"done".as_slice().into() },
                        cancel_delegates: false,
                    },
                });
            }
            root::Delivery::Core(core::Held::AcknowledgeTurn { channel, run, attempt, turn }) => {
                assert_eq!(channel, Token::new(7));
                assert!(
                    self.store.rows.contains_key(&root::Key::Core(core::Key::Core(core::CoreKey::Turn {
                        task: run.raw(),
                        attempt: attempt.raw(),
                        turn,
                    }))),
                    "worker saw ACK only after turn committed"
                );
                self.turn_acks += 1;
                self.events.push_back(root::Event::Answer {
                    read: None,
                    refused: Box::new([]),
                    channel,
                    task: run.raw(),
                    attempt: attempt.raw(),
                    cumulative: 3,
                    end: tasks::End::Finished {
                        result: tasks::TaskResult::Report { words: b"done".as_slice().into() },
                        cancel_delegates: false,
                    },
                });
            }
            root::Delivery::Core(core::Held::Acknowledge { .. }) => self.answer_acks += 1,
            root::Delivery::Core(core::Held::Result { task, words, .. }) => {
                assert!(self.store.rows.contains_key(&root::Key::Core(core::Key::Tasks(tasks::Key::Ended(task)))));
                assert_eq!(words.as_ref(), b"done");
                self.results += 1;
            }
            root::Delivery::ApplicationDocument { .. }
            | root::Delivery::ApplicationRead { .. }
            | root::Delivery::Core(
                core::Held::Delivery { .. } | core::Held::WatchEnded { .. } | core::Held::Grant { .. },
            ) => {}
            other @ (root::Delivery::Link { .. }
            | root::Delivery::ProjectMade { .. }
            | root::Delivery::CallBusy { .. }
            | root::Delivery::CallAnswer { .. }
            | root::Delivery::Core(_)
            | root::Delivery::Fleet(_)
            | root::Delivery::System { .. }
            | root::Delivery::Application { .. }
            | root::Delivery::LostRead { .. }) => {
                panic!("unhandled walking delivery {other:?}: {:?}", self.trace)
            }
        }
    }
}
