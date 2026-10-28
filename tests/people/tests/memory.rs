use jig_domain_people::{
    Ask, Domain, Entry, EntryKind, Event, Holding, Identity, IdentityKey, Limits, Outcome, Request, Role, Whom, fire,
    max_out, step,
};
use jig_people_world::LIMITS;
use skein_lib::{Env, Queue, ReplyTo, Time, Token, Wall};

struct Measured {
    domain: Domain,
    env: Env<Limits>,
    out: Queue<Request>,
    call: u64,
}

impl Measured {
    fn new(limits: Limits) -> Measured {
        Self::configured(limits, false)
    }
    fn configured(limits: Limits, deployment: bool) -> Measured {
        let out = Queue::with_capacity(max_out(&limits));
        let (owners, grants): (Box<[jig_domain_people::InitialOwner]>, Box<[jig_domain_people::ActGrant]>) =
            if deployment {
                let owners = Box::new([jig_domain_people::InitialOwner {
                    project: 1,
                    identity: jig_people_world::identity(1).key,
                }]);
                let grants = (0..limits.act_grants)
                    .map(|connector| jig_domain_people::ActGrant {
                        connector: u16::try_from(connector).expect("bounded connector fixture"),
                        may: jig_domain_people::ActKind::ALL.into(),
                    })
                    .collect();
                (owners, grants)
            } else {
                (Box::new([]), Box::new([]))
            };
        let domain = Domain::new(&limits, owners, 1, grants);
        Measured { domain, env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits }, out, call: 0 }
    }

    fn to(&mut self) -> ReplyTo {
        self.call += 1;
        ReplyTo::new(Token::new(self.call))
    }

    fn event(&mut self, event: Event) -> Option<Token> {
        step(&mut self.domain, &self.env, event, &mut self.out);
        self.drain()
    }

    fn drain(&mut self) -> Option<Token> {
        let _capacity = self.domain.capacity_refusals();
        let mut routed = None;
        while let Some(request) = self.out.pop() {
            if let Request::Route { request, .. } | Request::Act { request, .. } = request {
                routed = Some(request);
            }
        }

        self.domain.reclaim();
        routed
    }

    fn ask(&mut self, key: u8) -> Option<Token> {
        let reply_to = self.to();
        let words = vec![1; usize::try_from(self.env.limits.words).expect("test values are admitted and fit")]
            .into_boxed_slice();
        self.event(Event::Ask {
            models: Box::new([]),
            reply_to,
            sign_in: 1,
            key: [key; 16],
            ask: Ask::Create { project: 1, batch: Box::new([jig_people_world::fixtures::people_member(&words, 1)]) },
        })
    }
}

#[test]
#[expect(clippy::too_many_lines, reason = "one allocation family fills every owner and scratch path")]
fn full_people_roles_signins_pending_waiters_answers_and_expiry_fit() {
    for limits in [
        LIMITS,
        Limits {
            people: 1,
            sign_ins: 1,
            projects: 1,
            holdings: 1,
            requests: 2,
            pending: 1,
            waiters: 1,
            facts: 0,
            ..LIMITS
        },
    ] {
        let mut test = Measured::new(limits);
        test.event(Event::Restored);
        for number in 1..=u64::from(limits.people) {
            let reply_to = test.to();
            let identity = Identity {
                key: IdentityKey { provider: 0, subject: (number).to_be_bytes().into() },
                login: vec![
                    1;
                    usize::try_from(limits.identity_bytes).expect("test values are admitted and fit")
                        - size_of::<u64>()
                ]
                .into_boxed_slice(),
                name: Box::new([]),
            };
            test.event(Event::SignedIn {
                held: None,
                reply_to,
                person: number,
                sign_in: number,
                identity,
                kind: jig_domain_people::Kind::Person,
            });
        }
        for project in 1..=limits.projects {
            let holdings = (1..=u64::from(limits.holdings.min(limits.people)))
                .map(|person| Holding { person, role: Role::Member })
                .collect::<Vec<_>>()
                .into_boxed_slice();
            test.event(Event::Roles { project, holdings });
        }
        for task in 1..=u64::from(limits.inbox_tasks) {
            let entries = (0..limits.inbox_recipients)
                .map(|message| Entry {
                    task,
                    project: 1,
                    whom: Whom::Person(1),
                    kind: EntryKind::Question { message: u64::from(message) },
                    at: Wall::EPOCH,
                })
                .collect::<Vec<_>>()
                .into_boxed_slice();
            test.event(Event::Waiting { task, entries });
        }
        assert_eq!(test.domain.needs(1), Some(limits.inbox_tasks * limits.inbox_recipients));
        for party in 1..=u64::from(limits.people) {
            for position in 1..=u64::from(limits.inbox_entries) + 1 {
                test.domain.remember_result(
                    &limits,
                    party,
                    jig_domain_people::ResultRef {
                        task: position,
                        project: 1,
                        at: Wall::EPOCH,
                        position,
                        forgettable: None,
                    },
                );
                test.drain();
            }
        }
        for key in
            1..=u8::try_from(limits.requests.saturating_sub(limits.pending)).expect("test values are admitted and fit")
        {
            let token = test.ask(key).expect("test values are admitted and fit");
            let outcome = if key == 1 {
                Outcome::Refused(jig_domain_people::Refusal::Limit { which: jig_domain_people::LimitKind::LiveTasks })
            } else {
                Outcome::Created { first: u64::from(key), last: u64::from(key), given: None }
            };
            test.event(Event::Decided { request: token, outcome });
        }
        let mut tokens = [None; 8];
        for (slot, key) in
            tokens.iter_mut().zip(100..100 + u8::try_from(limits.pending).expect("test values are admitted and fit"))
        {
            *slot = test.ask(key);
            for _ in 1..limits.waiters {
                assert!(test.ask(key).is_none());
            }
        }
        for _ in 0..=limits.room_refusals {
            assert!(test.ask(250).is_none());
        }
        for token in tokens.into_iter().flatten() {
            test.event(Event::Decided {
                request: token,
                outcome: Outcome::Created { first: 90, last: 90, given: None },
            });
        }
        test.env.now = Time::from_nanos(limits.sign_in_lifetime.as_nanos());
        while test.domain.is_due(test.env.now) {
            fire(&mut test.domain, &test.env, &mut test.out);
            test.drain();
        }

        full_conversation_positions_and_replies_fit(limits);
    }
}

fn full_conversation_positions_and_replies_fit(limits: Limits) {
    // Every per-conversation position and the complete reply census fit with restore inputs and outputs.
    let mut restored = Measured::new(limits);
    for party in 1..=u64::from(limits.people) {
        for task in 1..=u64::from(limits.inbox_tasks) {
            restored.event(Event::Restore {
                record: jig_domain_people::Stored::ReadConversation { party, task, through: 1 },
            });
        }
        restored.event(Event::Restore {
            record: jig_domain_people::Stored::Person {
                number: party,
                identity: Identity {
                    key: IdentityKey { provider: 0, subject: party.to_be_bytes().into() },
                    login: Box::new([]),
                    name: Box::new([]),
                },
            },
        });
    }
    restored.event(Event::Restored);
    for project in 1..=limits.projects {
        restored.event(Event::Roles { project, holdings: Box::new([]) });
        for party in 1..=u64::from(limits.people) {
            for position in 1..=u64::from(limits.inbox_entries) + 1 {
                let number = (party - 1) * u64::from(limits.projects) * (u64::from(limits.inbox_entries) + 1)
                    + u64::from(project - 1) * (u64::from(limits.inbox_entries) + 1)
                    + position;
                restored.domain.remember_result(
                    &limits,
                    party,
                    jig_domain_people::ResultRef {
                        task: number,
                        project,
                        at: Wall::EPOCH,
                        position: number,
                        forgettable: None,
                    },
                );
                restored.drain();
            }
            restored.event(Event::ChatState {
                party,
                project,
                task: u64::from(limits.inbox_tasks) + (party - 1) * u64::from(limits.projects) + u64::from(project),
                last_reply: 1,
                live: false,
                seen: None,
                replies: Box::new([]),
            });
        }
    }
    for task in 1..=u64::from(limits.inbox_tasks) {
        let entries = (0..limits.inbox_recipients)
            .map(|message| Entry {
                task,
                project: 1,
                whom: Whom::Person(1),
                kind: EntryKind::ChatReply { message: u64::from(message) + 2 },
                at: Wall::EPOCH,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let party = (task - 1) % u64::from(limits.people) + 1;
        let replies = (0..limits.inbox_recipients)
            .map(|message| jig_domain_people::ReplyRef {
                entry: Entry {
                    task,
                    project: 1,
                    whom: Whom::Person(party),
                    kind: EntryKind::ChatReply { message: u64::from(message) + 2 },
                    at: Wall::EPOCH,
                },
                activity: u64::from(message) + 2,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        restored.event(Event::ChatState {
            party,
            project: 1,
            task,
            last_reply: u64::from(limits.inbox_recipients) + 1,
            live: true,
            seen: None,
            replies,
        });
        restored.event(Event::Waiting { task, entries });
    }
    for task in 1..=u64::from(limits.inbox_tasks) {
        restored.event(Event::Forget { task, project: 1, party: None, last_reply: 0 });
    }
}

#[test]
fn retained_account_sign_in_answers_keep_their_longest_addresses() {
    let mut test = Measured::new(LIMITS);
    test.event(Event::Restored);
    let reply_to = test.to();
    test.event(Event::SignedIn {
        held: None,
        reply_to,
        person: 1,
        sign_in: 1,
        identity: Identity {
            key: IdentityKey { provider: 0, subject: Box::new([1]) },
            login: Box::new([]),
            name: Box::new([]),
        },
        kind: jig_domain_people::Kind::Person,
    });
    for key in 0..u8::try_from(LIMITS.requests).expect("tiny request bound") {
        let reply_to = test.to();
        let request = test
            .event(Event::Ask {
                models: Box::new([]),
                reply_to,
                sign_in: 1,
                key: [key; 16],
                ask: Ask::AccountSignIn { holder: jig_domain_people::AccountHolder::Own, provider: 3 },
            })
            .expect("account sign-in admitted");
        let address =
            vec![b'a'; usize::try_from(jig_domain_people::ADDRESS).expect("small address bound")].into_boxed_slice();
        test.event(Event::Decided {
            request,
            outcome: Outcome::AccountSignInStarted { source: u32::from(key), address },
        });
    }
    let reply_to = test.to();
    assert!(
        test.event(Event::Ask {
            models: Box::new([]),
            reply_to,
            sign_in: 1,
            key: [0; 16],
            ask: Ask::AccountSignIn { holder: jig_domain_people::AccountHolder::Own, provider: 3 }
        })
        .is_none()
    );
}
