//! Application subjects use core admission and the existing pacing entry points.
use super::*;
use jig_domain_people::ApplicationScope;

fn application_subject(scope: ApplicationScope) -> Watched {
    Watched::Application { scope, connector: 1, subject: Token::new(500) }
}
fn document_asks(out: &[Request]) -> usize {
    let mut count = 0_usize;
    for request in out {
        if let Request::Ask { ask: Ask::Document { .. }, .. } = request {
            count = count.checked_add(1).unwrap();
        }
    }
    count
}
fn mark(core: &mut Core, limits: &Limits, connector: u16) {
    let mut subjects = List::with_capacity(2);
    subjects.push(Token::new(500)).unwrap();
    subjects.push(Token::new(500)).unwrap();
    let out = watch_outputs(core, limits, Event::Marked { connector, subjects });
    assert_eq!(document_asks(&out), 0);
    let Requests::Out(mut out) =
        finish_decision(core, &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: *limits }, true);
    while let Some(request) = out.pop() {
        if let Request::Ask { ask: Ask::Document { .. }, .. } = request {
            panic!("a mark waits for the delivery acknowledgement");
        }
    }
}

#[test]
fn application_watch_and_read_admission_checks_the_scope_before_any_document_handoff() {
    for scope in [ApplicationScope::Project(1), ApplicationScope::Deployment] {
        let (mut core, limits) = watch_core();
        let out = watch_outputs(
            &mut core,
            &limits,
            Event::Watch {
                watcher: Token::new(30),
                sign_in: 10,
                subject: application_subject(scope),
                window: window(),
            },
        );
        assert_eq!(document_asks(&out), 1);
        let out = watch_outputs(
            &mut core,
            &limits,
            Event::Read {
                reply: ReplyTo::new(Token::new(31)),
                sign_in: 10,
                read: reads::Read::Application { scope, connector: 1, read: Token::new(501), window: window() },
            },
        );
        let Request::Ask { connector: 1, ask: Ask::Read { party: 1, .. } } = &out[0] else {
            panic!("admitted application read reaches its owner");
        };
    }
    let (mut core, limits) = watch_core();
    let scope = ApplicationScope::Project(2);
    let out = watch_outputs(
        &mut core,
        &limits,
        Event::Watch { watcher: Token::new(30), sign_in: 10, subject: application_subject(scope), window: window() },
    );
    assert_eq!(document_asks(&out), 0);
    let Request::Ask { ask: Ask::DropDocument { subject }, .. } = &out[0] else {
        panic!("refusal drops the prepared name")
    };
    assert_eq!(*subject, Token::new(500));
    let Request::Now(value) = &out[1] else { panic!("watch refused before a document handoff") };
    let Now::WatchRefused { why, .. } = value.as_ref() else { panic!("watch refusal") };
    assert_eq!(*why, WatchRefusal::NotAllowed);
    let out = watch_outputs(
        &mut core,
        &limits,
        Event::Read {
            reply: ReplyTo::new(Token::new(31)),
            sign_in: 10,
            read: reads::Read::Application { scope, connector: 1, read: Token::new(501), window: window() },
        },
    );
    let Request::Ask { ask: Ask::DropDocument { .. }, .. } = &out[0] else {
        panic!("read refusal drops its prepared name")
    };
    let Request::Held(value) = &out[1] else { panic!("read refusal is held") };
    let Held::Read { answer, .. } = value.as_ref() else { panic!("one read terminal") };
    assert_eq!(*answer, reads::Answer::None(reads::Why::Unknown));
}

#[test]
fn connector_marks_twice_in_flight_make_one_more_whole_document() {
    let (mut core, limits) = watch_core();
    let watcher = Token::new(30);
    let out = watch_outputs(
        &mut core,
        &limits,
        Event::Watch {
            watcher,
            sign_in: 10,
            subject: application_subject(ApplicationScope::Project(1)),
            window: window(),
        },
    );
    assert_eq!(document_asks(&out), 1);
    let out =
        watch_outputs(&mut core, &limits, Event::View(views::Event::Made { watcher, outcome: views::Made::Ready }));
    let Request::Held(value) = &out[0] else { panic!("delivery is held") };
    let Held::ApplicationDelivery { subject, .. } = value.as_ref() else { panic!("connector construction") };
    assert_eq!(*subject, Token::new(500));
    mark(&mut core, &limits, 1);
    mark(&mut core, &limits, 1);
    mark(&mut core, &limits, 2);
    let out = watch_outputs(&mut core, &limits, Event::Delivered { watcher });
    assert_eq!(document_asks(&out), 1);
    let out =
        watch_outputs(&mut core, &limits, Event::View(views::Event::Made { watcher, outcome: views::Made::Ready }));
    let Request::Held(value) = &out[0] else { panic!("replacement delivery is held") };
    let Held::ApplicationDelivery { .. } = value.as_ref() else { panic!("one replacement whole document") };
    let out = watch_outputs(&mut core, &limits, Event::Delivered { watcher });
    assert_eq!(document_asks(&out), 0);
}

#[test]
fn a_connector_refusal_ends_its_watch_and_drops_the_owner_binding() {
    let (mut core, limits) = watch_core();
    let watcher = Token::new(30);
    watch_outputs(
        &mut core,
        &limits,
        Event::Watch {
            watcher,
            sign_in: 10,
            subject: application_subject(ApplicationScope::Project(1)),
            window: window(),
        },
    );
    let out = watch_outputs(
        &mut core,
        &limits,
        Event::View(views::Event::Made { watcher, outcome: views::Made::Gone { why: views::End::NotAllowed } }),
    );
    let Request::Held(value) = &out[0] else { panic!("refused construction ends its watch") };
    let Held::ApplicationWatchEnded { watcher: ended, connector, subject, why } = value.as_ref() else {
        panic!("owner binding accompanies the ending")
    };
    assert_eq!((*ended, *connector, *subject, *why), (watcher, 1, Token::new(500), views::End::NotAllowed));
}

#[test]
fn a_connector_delivery_waits_for_the_last_commit_the_root_made() {
    use skein_lib::{Journal, JournalLimits, JournalRoom, Released};
    let (mut core, limits) = watch_core();
    let watcher = Token::new(30);
    watch_outputs(
        &mut core,
        &limits,
        Event::Watch {
            watcher,
            sign_in: 10,
            subject: application_subject(ApplicationScope::Project(1)),
            window: window(),
        },
    );
    let out =
        watch_outputs(&mut core, &limits, Event::View(views::Event::Made { watcher, outcome: views::Made::Ready }));
    let mut delivery = None;
    for request in out {
        match request {
            Request::Held(held) => {
                assert!(delivery.is_none());
                delivery = Some(*held);
            }
            Request::ProjectMade { .. }
            | Request::Write(_)
            | Request::Ask { .. }
            | Request::Now(_)
            | Request::Decided => {}
        }
    }
    let delivery = delivery.unwrap();
    let mut journal = Journal::<u64, Held>::new(&JournalLimits { commits: 2, writes: 1, held: 4, now: 1, release: 4 });
    let room = JournalRoom { writes: 1, held: 1 };
    let mut prior = journal.decision(&room).unwrap();
    prior.write(123).unwrap();
    journal.accept(prior);
    let commit = journal.commit().unwrap().number;
    let mut decision = journal.decision(&room).unwrap();
    decision.hold(delivery).unwrap();
    journal.accept(decision);
    assert!(journal.commit().is_none(), "construction makes no commit");
    let mut released = Queue::with_capacity(4);
    assert_eq!(journal.release(&mut released), Released::None);
    assert!(released.is_empty());
    journal.committed(commit);
    assert_eq!(journal.release(&mut released), Released::Some);
    let Some(Held::ApplicationDelivery { watcher: due, .. }) = released.pop() else {
        panic!("durable connector delivery")
    };
    assert_eq!(due, watcher);
    assert!(released.is_empty());
}
