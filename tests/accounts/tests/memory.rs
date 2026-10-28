use jig_accounts_world::{LIMITS, provider, providers};
use jig_domain_accounts::{
    ADDRESS, DISPLAY, Domain, Event, Failure, Form, FromComponent, Held, Holder, Kind, Limits, NAME, Request, SUBJECT,
    Scheme, Signing, Stored, max_out, step,
};
use skein_lib::{Duration, Env, Queue, Time, Token, Wall};
const KINDS: [Kind; 3] = [Kind::SignIn, Kind::Key, Kind::Program];
/// Bytes at their longest, distinct for each account.
fn longest(length: u32, account: u32) -> Box<[u8]> {
    let mut bytes = vec![b'a'; usize::try_from(length).expect("a small length")];
    bytes[0] = b'a' + u8::try_from(account).expect("a small number");
    bytes.into_boxed_slice()
}
/// Each account's number, its sources' following it.
fn number(account: u32, place: Option<u32>) -> u32 {
    let first = account * (LIMITS.sources + 1);
    match place {
        Some(place) => first + 1 + place,
        None => first,
    }
}
fn kind(place: u32) -> Kind {
    KINDS[usize::try_from(place).expect("a small place") % KINDS.len()]
}
fn granted(source: u32) -> FromComponent {
    FromComponent::Granted {
        form: Form::Credential(Scheme::Bearer),
        account: source,
        generation: 9,
        valid: Duration::from_secs(100),
    }
}
/// Every account with its longest names, every source of each, a program's name at its longest,
/// a limit on each key, a grant for each signed-in source, and every credential. Each record is
/// made inside its step, as the store's decoding would make it. Then a sign-out pending for as
/// many sources as are kept, each account's signed-in source renewed and signed out in turn, every
/// sign-in's end kept, and a sign-in under way.
#[test]
#[expect(clippy::too_many_lines, reason = "one saturation story exercises every retained account state")]
fn full_accounts_restore_sign_in_and_sign_out() {
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    let providers = providers(vec![provider(1, true, true)]);
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: LIMITS };
    let mut domain = Domain::new(&LIMITS, providers);
    for account in 0..LIMITS.accounts {
        let record = Stored::Account {
            place: 0,
            uses: Some((0..jig_domain_accounts::USES).map(|n| jig_domain_accounts::Use::Party(u64::from(n))).collect()),
            number: number(account, None),
            provider: 1,
            holder: Holder::Party(u64::from(account)),
            subject: longest(SUBJECT, account),
            display: longest(DISPLAY, account),
        };
        step(&mut domain, &env, Event::Restore { record }, &mut out);
        for place in 0..LIMITS.sources {
            let name = match kind(place) {
                Kind::Program => longest(NAME, account),
                Kind::SignIn | Kind::Key | Kind::None => Box::new([]),
            };
            let held = match kind(place) {
                Kind::Key => Some(Held::Limited { reset: Some(Wall::from_nanos(60_000_000_000)) }),
                Kind::SignIn | Kind::Program | Kind::None => None,
            };
            if kind(place) == Kind::Program {
                step(
                    &mut domain,
                    &env,
                    Event::Restore {
                        record: Stored::Issued {
                            number: number(account, Some(place)),
                            holder: Holder::Party(u64::from(account)),
                            name: name.clone(),
                            signed_out: false,
                        },
                    },
                    &mut out,
                );
            }
            let record = Stored::Source {
                account: number(account, None),
                number: number(account, Some(place)),
                place,
                kind: kind(place),
                name,
                held,
            };
            step(&mut domain, &env, Event::Restore { record }, &mut out);
        }
    }
    step(&mut domain, &env, Event::Want, &mut out);
    while out.pop().is_some() {}
    for account in 0..LIMITS.accounts {
        for place in 0..LIMITS.sources {
            if kind(place) != Kind::SignIn {
                continue;
            }
            let source = number(account, Some(place));
            let event = granted(source);
            step(&mut domain, &env, Event::Component(event), &mut out);
            while out.pop().is_some() {}
        }
    }
    for number in 0..LIMITS.credentials {
        let event = Event::Credential { number, generation: 1, valid: Duration::from_secs(100) };
        step(&mut domain, &env, event, &mut out);
        while out.pop().is_some() {}
    }
    assert_eq!(domain.unrestored(), 0);
    assert_eq!(domain.accounts(), LIMITS.accounts);
    assert_eq!(domain.source_views().len(), LIMITS.accounts * LIMITS.sources);
    assert!(domain.usable(number(0, None)));
    let mut run = Run { env, out, request: 0 };
    for account in 0..LIMITS.accounts {
        run.sign_out(&mut domain, account, number(account, Some(0)));
    }
    let mut ends = 0;
    for _ in 1..LIMITS.sources {
        for account in 0..LIMITS.accounts {
            let source = run.sign_in(&mut domain, account);
            let event = Event::SignedIn {
                source,
                subject: longest(SUBJECT, account),
                display: longest(DISPLAY, account),
                declined: false,
            };
            run.step(&mut domain, event, "renewed");
            ends += 1;
            run.sign_out(&mut domain, account, source);
        }
    }
    while ends < LIMITS.facts {
        let source = run.sign_in(&mut domain, 0);
        run.step(&mut domain, Event::SignInFailed { source, failure: Failure::Cancelled }, "failed");
        ends += 1;
    }
    let source = run.sign_in(&mut domain, 0);
    run.visit(&mut domain, source);
    assert_eq!(domain.facts_lost(), 0, "every end kept");
}

/// Each step takes its asks.
struct Run {
    env: Env<Limits>,
    out: Queue<Request>,
    request: u64,
}
impl Run {
    /// Retain the account sign-in answer in the output queue.
    fn visit(&mut self, domain: &mut Domain, source: u32) {
        let address = longest(ADDRESS, 0);
        step(domain, &self.env, Event::Visited { source, address }, &mut self.out);
    }

    /// Take the step's last ask of the component.
    fn step(&mut self, domain: &mut Domain, event: Event, _label: &str) -> Option<Signing> {
        step(domain, &self.env, event, &mut self.out);
        let mut signing = None;
        while let Some(request) = self.out.pop() {
            if let Request::Signing(asked) = request {
                signing = Some(asked);
            }
        }

        signing
    }
    fn token(&mut self) -> Token {
        self.request += 1;
        Token::new(self.request)
    }
    /// The account's holder signs in at provider 1: the started source's number.
    fn sign_in(&mut self, domain: &mut Domain, account: u32) -> u32 {
        let event = Event::SignIn {
            request: self.token(),
            holder: Holder::Party(u64::from(account)),
            by: u64::from(account),
            provider: 1,
        };
        let Some(Signing::SignIn { source, .. }) = self.step(domain, event, "sign in") else {
            panic!("a sign-in started");
        };
        source
    }
    fn sign_out(&mut self, domain: &mut Domain, account: u32, source: u32) {
        let event = Event::SignOut { request: self.token(), holder: Holder::Party(u64::from(account)), source };
        assert_eq!(self.step(domain, event, "sign out"), Some(Signing::SignOut { source }));
    }
}

#[test]
fn pending_keys_and_account_removals_complete() {
    use jig_domain_accounts::{Change, USES, Use};
    let providers = providers(vec![provider(1, true, true)]);
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: LIMITS };
    let mut run = Run { env, out: Queue::with_capacity(max_out(&LIMITS)), request: 0 };
    let mut domain = Domain::new(&LIMITS, providers);
    for number in 0..LIMITS.accounts {
        let account = jig_accounts_world::account(number * 2, 1, Holder::Party(u64::from(number)), b"subject");
        // Each subscription has its own subject.
        let Stored::Account { number: account, provider, holder, place, mut subject, display, .. } = account else {
            unreachable!()
        };
        subject[0] = u8::try_from(number).expect("tiny number");
        run.step(
            &mut domain,
            Event::Restore {
                record: Stored::Account {
                    number: account,
                    provider,
                    holder,
                    place,
                    subject,
                    display,
                    uses: Some((0..USES).map(|n| Use::Party(u64::from(n))).collect()),
                },
            },
            "uses restored",
        );
        run.step(
            &mut domain,
            Event::Restore { record: jig_accounts_world::source(account, account + 1, 0, Kind::SignIn) },
            "source restored",
        );
    }
    run.step(&mut domain, Event::Want, "restored");
    let change = Change::AddKey { provider: 1, join: Some(0), carried: 1 << 63 };
    assert!(matches!(
        run.step(
            &mut domain,
            Event::Change { request: Token::new(50), holder: Holder::Party(0), change },
            "LLM key being kept"
        ),
        Some(Signing::Keep { .. })
    ));
    for number in 1..LIMITS.accounts {
        run.step(
            &mut domain,
            Event::Change {
                request: Token::new(u64::from(number)),
                holder: Holder::Party(u64::from(number)),
                change: Change::Remove { account: number * 2 },
            },
            "account awaiting sign-out",
        );
    }
    let pending = domain.next_number() - 1;
    run.step(
        &mut domain,
        Event::Component(FromComponent::KeepFailed {
            account: pending,
            why: jig_domain_accounts::KeyFailure::NotStored,
        }),
        "LLM key keep failed",
    );
    run.step(
        &mut domain,
        Event::Change { request: Token::new(0), holder: Holder::Party(0), change: Change::Remove { account: 0 } },
        "all removals pending",
    );
    for number in 0..LIMITS.accounts {
        run.step(&mut domain, Event::SignedOut { source: number * 2 + 1, confirmed: true }, "removal confirmed");
    }
}

#[test]
fn full_program_operations_stay_below_the_accounts_bound() {
    let mut offered = provider(1, false, false);
    offered.offers.programs = Box::new([b"claude_code".as_slice().into()]);
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: LIMITS };
    let out = Queue::with_capacity(max_out(&LIMITS));
    let providers = providers(vec![offered]);
    let mut domain = Domain::new(&LIMITS, providers);
    let mut run = Run { env, out, request: 0 };
    for account in 0..LIMITS.accounts {
        let holder = Holder::Party(u64::from(account));
        let _last = run.step(
            &mut domain,
            Event::Restore {
                record: Stored::Account {
                    number: number(account, None),
                    provider: 1,
                    holder,
                    place: 0,
                    uses: None,
                    subject: Box::new([]),
                    display: Box::new([]),
                },
            },
            "program account",
        );
        for place in 0..LIMITS.sources {
            let source = number(account, Some(place));
            let name: Box<[u8]> = format!("{source:032}").into_bytes().into_boxed_slice();
            let _last = run.step(
                &mut domain,
                Event::Restore {
                    record: Stored::Issued { number: source, holder, name: name.clone(), signed_out: false },
                },
                "issued name",
            );
            let _last = run.step(
                &mut domain,
                Event::Restore {
                    record: Stored::Source {
                        account: number(account, None),
                        number: source,
                        place,
                        kind: Kind::Program,
                        name,
                        held: None,
                    },
                },
                "program source",
            );
        }
    }
    for number in 100..103 {
        let _last = run.step(
            &mut domain,
            Event::Restore {
                record: Stored::Issued {
                    number,
                    holder: Holder::Deployment,
                    name: format!("{number:032}").into_bytes().into_boxed_slice(),
                    signed_out: true,
                },
            },
            "retired name",
        );
    }
    let _last = run.step(&mut domain, Event::Want, "issued snapshot");
    for account in 0..LIMITS.accounts {
        for place in 0..LIMITS.sources {
            let source = number(account, Some(place));
            let request = run.token();
            let _last = run.step(
                &mut domain,
                Event::SignOut { request, holder: Holder::Party(u64::from(account)), source },
                "pending program",
            );
            let _last = run.step(
                &mut domain,
                Event::ProgramTargets {
                    source,
                    hosts: (0..LIMITS.program_hosts).map(jig_accounts_world::worker).collect(),
                },
                "full target set",
            );
        }
    }
    let _last = run.step(&mut domain, Event::Close, "close pending programs");
}

/// Full account rows and source alarms stay resident while the move tables saturate.
fn move_tables() -> (Domain, Run) {
    let limits = Limits { sign_ins: LIMITS.accounts, ..LIMITS };
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let offered = providers(vec![provider(1, true, true)]);
    let domain = Domain::new(&limits, offered);
    let out = Queue::with_capacity(max_out(&limits));
    (domain, Run { env, out, request: 0 })
}
fn restore_move_accounts(domain: &mut Domain, run: &mut Run) {
    for account in 0..run.env.limits.accounts {
        run.step(
            domain,
            Event::Restore {
                record: Stored::Account {
                    number: number(account, None),
                    provider: 1,
                    holder: Holder::Party(u64::from(account)),
                    place: 0,
                    subject: longest(SUBJECT, account),
                    display: longest(DISPLAY, account),
                    uses: Some(
                        (0..jig_domain_accounts::USES).map(|n| jig_domain_accounts::Use::Party(u64::from(n))).collect(),
                    ),
                },
            },
            "full move account",
        );
        for place in 0..run.env.limits.sources {
            run.step(
                domain,
                Event::Restore {
                    record: Stored::Source {
                        account: number(account, None),
                        number: number(account, Some(place)),
                        place,
                        kind: if place == 0 { Kind::SignIn } else { Kind::Key },
                        name: Box::new([]),
                        held: Some(Held::Limited { reset: Some(Wall::from_nanos(120_000_000_000)) }),
                    },
                },
                "full move sources and alarms",
            );
        }
    }
    run.step(domain, Event::Want, "move accounts restored");
    for number in 0..run.env.limits.credentials {
        run.step(
            domain,
            Event::Credential { number, generation: 1, valid: Duration::from_secs(100) },
            "move credentials",
        );
    }
    assert_eq!(domain.unrestored(), 0);
    assert_eq!(domain.accounts(), run.env.limits.accounts);
}
#[test]
fn full_moves_follow_their_deadlines() {
    use jig_domain_accounts::Change;
    let (mut domain, mut run) = move_tables();
    restore_move_accounts(&mut domain, &mut run);
    for account in 0..run.env.limits.sign_ins {
        let source = number(account, Some(0));
        let request = run.token();
        assert_eq!(
            run.step(
                &mut domain,
                Event::Change {
                    request,
                    holder: Holder::Party(u64::from(account)),
                    change: Change::MoveOut { account: number(account, None) },
                },
                "move withdrawn with longest display"
            ),
            Some(Signing::Withdraw { source, display: longest(DISPLAY, account) })
        );
        run.step(&mut domain, Event::Withdrawn { source, readable: true }, "move deadline armed");
        assert_eq!(domain.source(source).expect("moving source").state, jig_domain_accounts::State::Moving);
    }
    assert_eq!(domain.next_deadline(), Some(Time::ZERO.saturating_add(run.env.limits.move_deadline)));
    run.step(&mut domain, Event::Close, "full moves close");
}
#[test]
fn full_opened_arrivals_keep_their_values() {
    use jig_domain_accounts::Change;
    let (mut domain, mut run) = move_tables();
    restore_move_accounts(&mut domain, &mut run);
    for account in 0..run.env.limits.sign_ins {
        let request = run.token();
        let Some(Signing::Open { source, carried }) = run.step(
            &mut domain,
            Event::Change {
                request,
                holder: Holder::Party(u64::from(account)),
                change: Change::MoveIn { carried: 1 << 63 },
            },
            "arrival opened",
        ) else {
            panic!("the arrival starts");
        };
        step(
            &mut domain,
            &run.env,
            Event::Opened {
                source,
                provider: 1,
                subject: longest(SUBJECT, account),
                display: longest(DISPLAY, account),
            },
            &mut run.out,
        );
        assert_eq!(run.out.pop(), Some(Request::Signing(Signing::Adopt { source, carried })));
        assert!(run.out.pop().is_none());
    }
    run.step(&mut domain, Event::Close, "full arrivals close");
}

#[test]
fn valid_refuses_each_unrepresentable_limit() {
    for (name, limits, accepted) in [
        ("fixture", LIMITS, true),
        ("accounts", Limits { accounts: 0, ..LIMITS }, false),
        ("output", Limits { accounts: u32::MAX, ..LIMITS }, false),
        ("holder zero", Limits { holder_accounts: 0, ..LIMITS }, false),
        ("holder range", Limits { holder_accounts: LIMITS.accounts + 1, ..LIMITS }, false),
        ("hosts zero", Limits { program_hosts: 0, ..LIMITS }, false),
        ("hosts range", Limits { program_hosts: 129, ..LIMITS }, false),
        ("program names", Limits { program_names: 0, ..LIMITS }, false),
        ("sources", Limits { sources: 0, ..LIMITS }, false),
        ("sign ins", Limits { sign_ins: 0, ..LIMITS }, false),
        ("retry", Limits { retry: Duration::ZERO, ..LIMITS }, false),
        ("move deadline", Limits { move_deadline: Duration::ZERO, ..LIMITS }, false),
        ("backoff base", Limits { backoff_base: Duration::ZERO, ..LIMITS }, false),
        ("backoff order", Limits { backoff_max: Duration::ZERO, ..LIMITS }, false),
    ] {
        assert_eq!(jig_domain_accounts::valid(&limits), accepted, "{name}");
    }
}
