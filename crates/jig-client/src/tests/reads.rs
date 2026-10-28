use super::fixtures::{self, Client, SUBJECT_TASK, TASK};
use crate::{Event, Handle, Message, Readable, Request, With, contract as c};
use jig_client_protocol_wire::v19 as w;
use skein_lib::{List, Reader, Secret, Time, Writer};

fn client() -> Client {
    let mut limits = fixtures::limits();
    limits.exchanges = 1;
    limits.domain.reads = 2;
    let mut client = Client::with_limits(limits, List::with_capacity(0), crate::max_out(&limits));
    client.step(Event::SignIn { with: With::Held(Secret::new(Box::new([7; 40]))) });
    client.stream(fixtures::hello(jig_client_protocol_wire::VERSION));
    let opening = client.exchange();
    let watch = client.opened_watch();
    client.stream(fixtures::frame(watch));
    let answer = w::OpenAnswer::Admitted;
    let mut bytes = Writer::new(usize::try_from(answer.measure()).unwrap());
    answer.encode(&mut bytes).unwrap();
    client.step(Event::Reply { exchange: opening, message: bytes.finish() });
    client.step(Event::Stored);
    client
}

#[test]
fn a_conversations_original_read_leaves_once_when_an_exchange_slot_is_released() {
    let mut client = client();
    let occupied = client.read(Handle { number: 100, ..TASK });
    let mut thread = fixtures::thread::model();
    let read = thread.moved(4).unwrap();
    client.step(Event::Read { handle: TASK, read: Readable::Jig(read) });
    assert!(client.requests().is_empty(), "local pressure keeps the original demand, without a terminal");
    assert!(thread.reading.is_some(), "the conversation still owns its read");
    let admission = client.machine.admission();
    assert_eq!(admission.attempts, 1);
    assert_eq!(admission.deferred_reads, 1);
    client.fire(Time::ZERO);
    assert!(client.requests().is_empty(), "no timer admits the read while the slot is occupied");

    client.step(Event::Reply { exchange: occupied, message: fixtures::unknown_read() });
    let mut admitted = None;
    for request in client.requests() {
        if let Request::Exchange { exchange, message: Message::Read(bytes), .. } = request {
            assert!(admitted.replace(*exchange).is_none(), "exactly one deferred read leaves");
            let owned = w::Owned::decode(&w::CEILINGS, &mut Reader::new(bytes)).unwrap();
            let w::Read::Activity(activity) = w::Read::decode(&w::CEILINGS, &mut Reader::new(owned.record())).unwrap()
            else {
                panic!("the original conversation demand")
            };
            assert_eq!(activity.task(), SUBJECT_TASK);
            assert!(activity.before().is_none());
        }
    }
    assert!(admitted.is_some(), "the release is the wake, without another document or fire");
    client.step(Event::Reply { exchange: occupied, message: fixtures::unknown_read() });
    assert!(client.requests().is_empty(), "a stale release does not send the read twice");
    client.fire(Time::ZERO);
    assert!(client.requests().is_empty());
    assert!(thread.loaded < thread.committed);
}

#[test]
fn replacing_a_queued_read_keeps_only_the_current_demand_within_the_read_bound() {
    let mut client = client();
    let occupied = client.read(Handle { number: 100, ..TASK });
    for project in 2..12 {
        client.step(Event::Read { handle: TASK, read: Readable::Jig(c::Read::Project { project }) });
        assert!(client.requests().is_empty());
    }
    client.step(Event::Read {
        handle: Handle { number: 101, ..TASK },
        read: Readable::Jig(c::Read::Project { project: 12 }),
    });
    let [Request::Told { told: crate::Told::Page { page: crate::Readout::Limit, .. } }] = client.requests() else {
        panic!("the unchanged logical read bound still refuses a third current read")
    };
    client.step(Event::Reply { exchange: occupied, message: fixtures::unknown_read() });
    let mut reads = List::with_capacity(2);
    for request in client.requests() {
        if let Request::Exchange { message: Message::Read(bytes), .. } = request {
            reads.push(bytes).unwrap();
        }
    }
    assert_eq!(reads.len(), 1);
    let owned = w::Owned::decode(&w::CEILINGS, &mut Reader::new(reads.get(0).unwrap())).unwrap();
    let w::Read::Project(project) = w::Read::decode(&w::CEILINGS, &mut Reader::new(owned.record())).unwrap() else {
        panic!("only the replacement project read leaves")
    };
    assert_eq!(project.project(), 11);
}

#[test]
fn unwatch_and_a_credential_boundary_discard_queued_reads_before_slots_are_reused() {
    for signing_in in [false, true] {
        let mut client = client();
        let occupied = client.read(Handle { number: 100, ..TASK });
        client.step(Event::Read {
            handle: TASK,
            read: Readable::Jig(c::Read::Activity { task: SUBJECT_TASK, before: None }),
        });
        assert!(client.requests().is_empty());
        if signing_in {
            client.step(Event::SignIn { with: With::Held(Secret::new(Box::new([9; 40]))) });
            let mut terminals = List::with_capacity(2);
            for request in client.requests() {
                if let Request::Told { told: crate::Told::Page { handle, page } } = request
                    && *handle == TASK
                {
                    terminals.push(page).unwrap();
                }
            }
            assert_eq!(terminals.as_slice(), &[&crate::Readout::Jig(c::ReadResult::None(c::ReadNone::SignedOut))]);
        } else {
            client.step(Event::Unwatch { handle: TASK });
        }
        for request in client.requests() {
            if let Request::Exchange { message: Message::Read(_), .. } = request {
                panic!("no cancelled read is admitted")
            }
        }
        client.step(Event::Reply { exchange: occupied, message: fixtures::unknown_read() });
        for request in client.requests() {
            if let Request::Exchange { message: Message::Read(_), .. } = request {
                panic!("no cancelled read is admitted")
            }
        }
        client.fire(Time::ZERO);
        for request in client.requests() {
            if let Request::Exchange { message: Message::Read(_), .. } = request {
                panic!("no cancelled read is admitted")
            }
        }
    }
}

#[test]
fn peer_refusals_and_a_real_loss_still_end_an_admitted_read_without_resending() {
    for (answer, expected) in [
        (Some(w::ReadNone::Busy), c::ReadNone::Busy),
        (Some(w::ReadNone::Unknown), c::ReadNone::Unknown),
        (None, c::ReadNone::Unreachable),
    ] {
        let mut client = client();
        let occupied = client.read(Handle { number: 100, ..TASK });
        client.step(Event::Read {
            handle: TASK,
            read: Readable::Jig(c::Read::Activity { task: SUBJECT_TASK, before: None }),
        });
        assert!(client.requests().is_empty());
        client.step(Event::Lost { exchange: occupied });
        let admitted = client.exchange();
        match answer {
            Some(answer) => client.step(Event::Reply { exchange: admitted, message: fixtures::read_none(answer) }),
            None => client.step(Event::Lost { exchange: admitted }),
        }
        let [Request::Told { told: crate::Told::Page { handle, page } }] = client.requests() else {
            panic!("one actual terminal: {:?}", client.requests())
        };
        assert_eq!(*handle, TASK);
        assert_eq!(*page, crate::Readout::Jig(c::ReadResult::None(expected)));
        client.fire(Time::ZERO);
        assert!(client.requests().is_empty(), "a peer result or actual loss is not retried");
    }
}

#[test]
fn a_peer_version_refusal_ends_queued_reads_instead_of_admitting_them() {
    let mut client = client();
    let occupied = client.read(Handle { number: 100, ..TASK });
    client.step(Event::Read {
        handle: TASK,
        read: Readable::Jig(c::Read::Activity { task: SUBJECT_TASK, before: None }),
    });
    assert!(client.requests().is_empty());
    let answer = w::ReadAnswer::NotServed(
        w::NotServed::new(&w::CEILINGS, w::NotServedParts { owner: w::Owner::Jig, served: List::with_capacity(0) })
            .unwrap(),
    );
    let mut bytes = Writer::new(usize::try_from(answer.measure()).unwrap());
    answer.encode(&mut bytes).unwrap();
    client.step(Event::Reply { exchange: occupied, message: bytes.finish() });
    let mut terminal = None;
    for request in client.requests() {
        if let Request::Told { told: crate::Told::Page { handle, page } } = request
            && *handle == TASK
        {
            assert!(terminal.replace(page).is_none());
        }
    }
    let Some(crate::Readout::NotServed { family: None, .. }) = terminal else {
        panic!("the peer's unserved-version result ends the queued read")
    };
    client.step(Event::Reply { exchange: occupied, message: fixtures::unknown_read() });
    for request in client.requests() {
        if let Request::Exchange { message: Message::Read(_), .. } = request {
            panic!("no refused read is admitted")
        }
    }
}

/// O2: the old link's consequences leave before the new domain's requests.
#[test]
fn link_consequences_precede_the_next_domain_request() {
    const ORDER_OUTPUTS: u32 = 32;
    let mut limits = fixtures::limits();
    limits.exchanges = 3;
    let mut client = Client::with_limits(limits, List::with_capacity(0), ORDER_OUTPUTS);
    client.sign_in();
    for task in [20, 21] {
        client.step(Event::Watch {
            handle: Handle { number: task, ..TASK },
            subject: crate::Watchable::Jig(c::Subject::Task { task }),
        });
    }
    for handle in [TASK, fixtures::ME] {
        client.step(Event::Read { handle, read: Readable::Jig(c::Read::Project { project: 1 }) });
    }
    client.step(Event::SignIn { with: With::Held(fixtures::credential(&fixtures::NEW_CREDENTIAL)) });
    let [
        Request::CloseStream,
        Request::Told { told: crate::Told::Page { handle: first, .. } },
        Request::Told { told: crate::Told::Page { handle: second, .. } },
        Request::Told { told: crate::Told::Ended { handle: watch_a, .. } },
        Request::Told { told: crate::Told::Ended { handle: watch_b, .. } },
        Request::Told { told: crate::Told::Link { link: crate::Link::SignedOut } },
        Request::Told { told: crate::Told::Link { link: crate::Link::Starting } },
        Request::OpenStream { .. },
    ] = client.requests()
    else {
        panic!("close the old stream before dispatching the credential boundary: {:?}", client.requests())
    };
    assert_eq!([*first, *second], [TASK, fixtures::ME]);
    assert_eq!([watch_a.number, watch_b.number], [20, 21]);

    // The new opening and two reads occupy the exchange edge; the next
    // two reads must wait, then leave in order as one slot is released.
    client.stream(fixtures::hello(jig_client_protocol_wire::VERSION));
    let opening = client.exchange();
    let first = client.read(TASK);
    let second = client.read(fixtures::ME);
    for number in [30, 31] {
        client.step(Event::Read {
            handle: Handle { number, ..TASK },
            read: Readable::Jig(c::Read::Project { project: u32::try_from(number).unwrap() }),
        });
        assert!(client.requests().is_empty(), "the edge is occupied");
    }
    for (exchange, project) in [(first, 30), (second, 31)] {
        client.step(Event::Reply { exchange, message: fixtures::unknown_read() });
        let mut sent = List::with_capacity(1);
        for request in client.requests() {
            if let Request::Exchange { message: Message::Read(bytes), .. } = request {
                let owned = w::Owned::decode(&w::CEILINGS, &mut Reader::new(bytes)).unwrap();
                let w::Read::Project(read) = w::Read::decode(&w::CEILINGS, &mut Reader::new(owned.record())).unwrap()
                else {
                    panic!("deferred project read")
                };
                sent.push(read.project()).unwrap();
            }
        }
        assert_eq!(sent.as_slice(), &[project]);
    }
    client.step(Event::Lost { exchange: opening });
    assert!(
        client.requests().iter().all(|request| !matches!(request, Request::Exchange { message: Message::Read(_), .. }))
    );
}
