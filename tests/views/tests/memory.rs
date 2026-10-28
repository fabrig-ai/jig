use jig_client_views_world::LIMITS;
use jig_domain_views::{self as views, Call, Domain, Event, Follow, Made, TurnName, Window};
use skein_lib::{Env, List, Queue, Time, Token, Wall};
fn step(domain: &mut Domain, out: &mut Queue<views::Request>, event: Event) {
    views::step(domain, &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: LIMITS }, event, out);
    while out.pop().is_some() {}
}
#[test]
fn retained_runs_survive_followers_and_release_on_end() {
    let mut domain = Domain::new(&LIMITS);
    let mut out = Queue::with_capacity(views::max_out(&LIMITS));
    for wave in 0..2 {
        for watcher in 0..LIMITS.watchers {
            let token = Token::new(u64::from(wave * LIMITS.watchers + watcher));
            step(
                &mut domain,
                &mut out,
                Event::Watch {
                    watcher: token,
                    party: Token::new(1),
                    subject: Token::new(u64::from(watcher)),
                    window: Window { rows: 10, after: None },
                    follow: Some(Follow { run: Token::new(u64::from(watcher % LIMITS.runs)), turn: None, offset: 0 }),
                },
            );
            step(&mut domain, &mut out, Event::Made { watcher: token, outcome: Made::Ready });
        }
        for run in 0..LIMITS.runs {
            for offset in [0, LIMITS.text_bytes] {
                step(
                    &mut domain,
                    &mut out,
                    Event::Text {
                        run: Token::new(u64::from(run)),
                        turn: TurnName { attempt: 1, turn: 1, retry: 0 },
                        offset,
                        text: vec![1; usize::try_from(LIMITS.text_bytes).expect("text bound")].into_boxed_slice(),
                        call: Some(Call {
                            identity: jig_domain_views::CallIdentity {
                                owner: Token::new(1),
                                sequence: 1,
                                retry: 0,
                                id: Box::new([]),
                            },
                            tool: vec![2; 16].into_boxed_slice(),
                            input: vec![3; 16].into_boxed_slice(),
                            cut: false,
                            began: Some(Wall::EPOCH),
                        }),
                    },
                );
            }
        }
        let mut subjects = List::with_capacity(LIMITS.subjects_per_change);
        for subject in 0..LIMITS.subjects_per_change {
            subjects.push(Token::new(u64::from(subject))).expect("bounded change");
        }
        step(&mut domain, &mut out, Event::Changed { subjects });
        if wave == 0 {
            for watcher in 0..LIMITS.watchers {
                let token = Token::new(u64::from(watcher));
                step(&mut domain, &mut out, Event::Unwatch { watcher: token });
                step(&mut domain, &mut out, Event::Delivered { watcher: token });
                step(&mut domain, &mut out, Event::StreamDelivered { watcher: token });
            }
            assert_eq!(domain.runs(), LIMITS.runs); // Retention survives the final follower.
            for run in 0..LIMITS.runs {
                step(&mut domain, &mut out, Event::RunEnded { run: Token::new(u64::from(run)) });
            }
            assert_eq!(domain.runs(), 0); // End releases retained runs; watch slots remain until reclaim.
        }
    }
}
