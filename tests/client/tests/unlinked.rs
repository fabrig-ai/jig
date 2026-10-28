use jig_client::{Client, Event, Link, Request};
use jig_client_world::{Front, Test, harness::ClientId, joint::Joint};
use skein_lib::{Duration, List, Queue, Time, Wall};
const PARTY: u64 = 1;
const CLIENT: ClientId = ClientId(0);
const NOW: Time = Time::ZERO;
const WALL: Wall = Wall::from_nanos(1_000_000_000_000);
const FIRST_RETRY: Duration = Duration::from_millis(10);
const MOST_RETRY: Duration = Duration::from_millis(80);
fn front() -> Joint {
    let mut owner = jig_core_world::world::limits();
    owner.core.documents.rows = 2;
    owner.core.documents.lines = 2;
    owner.core.documents.chips = 2;
    owner.core.documents.text_bytes = 32;
    jig_client_world::joint::front::fixture(&owner)
}
fn limits() -> jig_client::Limits {
    let world = jig_client_world::test::limits();
    let l = world.jig;
    jig_client::Limits {
        domain: jig_client::DomainLimits {
            owners: &[],
            requests: world.client.domain.requests,
            ask_bytes: 16384,
            rows: l.rows,
            segments: l.segments,
            text: l.text,
            turn_text: l.text,
            projects: l.projects,
            fields: l.fields,
            delegates: l.delegates,
            records: l.records,
            kept: l.kept,
            parts: l.parts,
            authority: 8,
            accounts: jig_client::AccountCounts { accounts: 8, sources: 8, sign_ins: 8 },
            heartbeat: world.client.domain.heartbeat,
            handles: 8,
            turn_bytes: 32,
            reads: l.reads,
            watches: l.watches,
            document_bytes: l.document_bytes,
            backoff: jig_client::Backoff { first: FIRST_RETRY, most: MOST_RETRY },
        },
        exchanges: 8,
    }
}
fn drain(client: &mut Client, front: &mut Joint, out: &mut Queue<Request>) {
    while let Some(request) = out.pop() {
        match request {
            Request::Down { down } => front.down(CLIENT, NOW, WALL, down).expect("Jig transport operation"),
            Request::Told { .. } | Request::Authorized(_) | Request::ServiceCredential { .. } => {}
            Request::Exchange { .. }
            | Request::OpenStream { .. }
            | Request::CloseStream
            | Request::Save { .. }
            | Request::Erase { .. }
            | Request::SignOut => panic!("unlinked client emits domain operations"),
        }
    }
    client.reclaim();
}
#[test]
fn an_unlinked_client_sees_me_live_against_the_real_core() {
    let limits = limits();
    let mut client = Client::unlinked(&limits, 8512).expect("bounded client");
    let mut out = Queue::uncapped();
    let mut front = front();
    Front::<Test>::bind(&mut front, CLIENT, PARTY);
    client.step(NOW, WALL, Event::StartUnlinked { kept: List::with_capacity(0) }, &mut out);
    client.step(NOW, WALL, Event::SignIn { with: jig_client::With::Transport }, &mut out);
    drain(&mut client, &mut front, &mut out);
    for _ in 0..20 {
        for (owner, up) in front.up(NOW, WALL, limits.domain.owners) {
            assert_eq!(owner, CLIENT);
            client.step(NOW, WALL, Event::Up { up }, &mut out);
            drain(&mut client, &mut front, &mut out);
        }
        if client.link().state == Link::Live {
            break;
        }
    }
    assert_eq!((client.link().state, client.link().party), (Link::Live, Some(1)));
}

#[test]
fn a_key_constructor_sends_the_value_once_without_keeping_it() {
    use jig_client::{Asker, Down, Key, Outcome, Told, contract as c};
    use skein_lib::{RequestEnvelope, Secret, sha256};
    let limits = limits();
    let mut client = Client::unlinked(&limits, 8513).expect("bounded client");
    let mut out = Queue::uncapped();
    let mut front = front();
    Front::<Test>::bind(&mut front, CLIENT, PARTY);
    client.step(NOW, WALL, Event::StartUnlinked { kept: List::with_capacity(0) }, &mut out);
    client.step(NOW, WALL, Event::SignIn { with: jig_client::With::Transport }, &mut out);
    drain(&mut client, &mut front, &mut out);
    for _ in 0..20 {
        for (_, up) in front.up(NOW, WALL, limits.domain.owners) {
            client.step(NOW, WALL, Event::Up { up }, &mut out);
            drain(&mut client, &mut front, &mut out);
        }
        if client.link().state == Link::Live {
            break;
        }
    }
    assert_eq!(client.link().state, Link::Live);
    let key = Key { asker: Asker(7), number: 31 };
    let bytes = b"a key supplied only for this sending";
    for (name, event) in [
        (
            [31; 16],
            jig_client::accounts::add_key(
                key,
                [31; 16],
                c::AccountHolder::Own,
                3,
                None,
                Secret::new(Box::from(bytes.as_slice())),
            ),
        ),
        (
            [32; 16],
            jig_client::accounts::replace_key(
                key,
                [32; 16],
                c::AccountHolder::Own,
                9,
                Secret::new(Box::from(bytes.as_slice())),
            ),
        ),
    ] {
        client.step(NOW, WALL, event, &mut out);
        let mut sent = None;
        while let Some(request) = out.pop() {
            match request {
                Request::Down { down: Down::Send { attempt, key: actual, request, value: Some(value) } } => {
                    assert_eq!(actual, name);
                    assert_eq!(value.expose(), bytes);
                    let jig_client::Ask::Jig(ask) = request else { panic!("jig ask") };
                    let c::Ask::AccountChange {
                        change: c::AccountChange::AddKey { digest, .. } | c::AccountChange::ReplaceKey { digest, .. },
                        ..
                    } = *ask
                    else {
                        panic!("typed key ask")
                    };
                    assert_eq!(digest, sha256::digest(bytes));
                    assert!(sent.replace(attempt).is_none());
                }
                Request::Told { told: Told::Pending { .. } } => {}
                other @ (Request::Exchange { .. }
                | Request::OpenStream { .. }
                | Request::CloseStream
                | Request::Save { .. }
                | Request::Erase { .. }
                | Request::Told { .. }
                | Request::Down { .. }
                | Request::SignOut
                | Request::Authorized(_)
                | Request::ServiceCredential { .. }) => panic!("carried account key must not be saved: {other:?}"),
            }
        }
        let attempt = sent.expect("sent once");
        client.step(
            NOW,
            WALL,
            Event::Up {
                up: jig_client::Up::Answer { attempt, envelope: RequestEnvelope::Lost, answer: None, why: None },
            },
            &mut out,
        );
        let mut lost = false;
        while let Some(request) = out.pop() {
            if matches!(request, Request::Told { told: Told::Answered { outcome: Outcome::Lost, .. } }) {
                lost = true;
            }
        }
        assert!(lost);
        assert!(client.pending(key).is_none());
        client.fire(NOW.saturating_add(Duration::from_secs(1)), WALL, &mut out);
        while let Some(request) = out.pop() {
            assert!(!matches!(request, Request::Down { down: Down::Send { .. } | Down::Save { .. } }));
        }
    }
}
