//! A backend application uses only the public client and scripted peer.
use jig_client::{self as client, contract as c};
use jig_client_scripted_engine::{self as peer, Engine};
use jig_client_test_records::settings as values;
use skein_lib::{Duration, List, Queue, RequestEnvelope, Time, Wall};
use std::collections::VecDeque;

struct Story {
    client: client::Client,
    engine: Engine,
    told: Vec<client::Told>,
    now: Time,
}
impl Story {
    fn event(&mut self, event: client::Event) {
        let mut events = VecDeque::from([event]);
        for _ in 0..128 {
            if let Some(event) = events.pop_front() {
                let mut out = Queue::uncapped();
                self.client.step(self.now, Wall::EPOCH, event, &mut out);
                while let Some(request) = out.pop() {
                    match request {
                        client::Request::Told { told } => self.told.push(told),
                        client::Request::Down { down } => match down {
                            client::Down::Open { watch, subject: client::Watchable::Jig(subject) } => {
                                self.engine.open(self.now, 11, watch, subject).expect("valid public story fixture");
                            }
                            client::Down::Continue { watch, .. } => self.engine.taken(watch),
                            client::Down::Close { watch } => self.engine.close(self.now, watch),
                            client::Down::Read { read, what: client::Readable::Jig(what) } => {
                                self.engine.read(self.now, 11, read, what).expect("valid public story fixture");
                            }
                            client::Down::Send { attempt, key, request: client::Ask::Jig(request), value: None } => {
                                self.engine
                                    .send(self.now, Wall::EPOCH, 11, key, attempt, *request)
                                    .expect("valid public story fixture");
                            }
                            client::Down::Save { .. } | client::Down::Erase { .. } => {}
                            other @ (client::Down::Send { .. }
                            | client::Down::Open { .. }
                            | client::Down::Read { .. }) => panic!("unexpected public down: {other:?}"),
                        },
                        other @ (client::Request::Exchange { .. }
                        | client::Request::OpenStream { .. }
                        | client::Request::CloseStream
                        | client::Request::Save { .. }
                        | client::Request::Erase { .. }
                        | client::Request::SignOut
                        | client::Request::Authorized(_)
                        | client::Request::ServiceCredential { .. }) => panic!("unexpected public request: {other:?}"),
                    }
                }
            }
            for up in self.engine.poll(self.now) {
                let up = match up {
                    peer::Up::Watch { token, event } => {
                        match &event {
                            c::WatchEvent::Delivered(_) => self.engine.taken(token),
                            c::WatchEvent::Opened | c::WatchEvent::Alive | c::WatchEvent::Behind => {}
                        }
                        client::Up::Watched { watch: token, event }
                    }
                    peer::Up::Read { token, terminal } => {
                        client::Up::ReadDone { read: token, result: client::Readout::Jig(terminal) }
                    }
                    peer::Up::Answer { attempt, envelope: peer::Envelope::Final, answer } => client::Up::Answer {
                        attempt,
                        envelope: RequestEnvelope::Final,
                        answer: answer.map(client::Answered::Jig),
                        why: None,
                    },
                    other @ (peer::Up::Answer { .. }
                    | peer::Up::ApplicationAnswer { .. }
                    | peer::Up::ApplicationRead { .. }
                    | peer::Up::App { .. }
                    | peer::Up::Ended { .. }) => panic!("unexpected scripted up: {other:?}"),
                };
                events.push_back(client::Event::Up { up });
            }
            if events.is_empty() {
                return;
            }
        }
        panic!("public exchange did not settle");
    }
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one public client story reads settings and history then observes both keyed answers"
)]
fn an_owner_reads_settings_and_history_and_changes_them_under_a_key() {
    let mut limits = jig_client_world::test::limits().client;
    limits.domain.rows = 8;
    let mut frame = jig_client_world::stories::frame(7);
    frame.deployment_role = Some(c::HeldRole::Owner);
    let mut setup = peer::Script::default();
    setup.bounds.rows = 8;
    setup.bounds.document_bytes = 65536;
    setup.bounds.commit_delay = Duration::ZERO;
    setup.truth.frames.insert(7, frame);
    setup.truth.sign_ins.insert(11, 7);
    setup.truth.settings = Some(values::settings_document());
    let mut settings_values = values::settings_values().into_boxed().into_vec();
    setup.truth.setting_values.push(c::SettingDocument {
        setting: c::Setting::Rules,
        revision: 6,
        value: settings_values.pop().expect("valid public story fixture"),
    });
    setup.truth.revisions.insert(6, c::RevisionAnswer::Revision(values::settings_revision()));
    setup.truth.revision_pages.insert(None, values::settings_page());
    let mut next = values::settings_document();
    next.newest = 7;
    let mut change = values::settings_change();
    setup.rules.push(peer::Rule {
        party: Some(7),
        ask: c::Ask::ChangeSettings(change.clone()),
        answer: c::Answer::Done(c::Outcome::SettingsChanged(c::OutcomeSettingsChanged { revision: 7 })),
        changes: vec![peer::Change::Settings(next)],
    });
    change.against = 4;
    setup.rules.push(peer::Rule {
        party: Some(7),
        ask: c::Ask::ChangeSettings(change.clone()),
        answer: c::Answer::Refused { refusal: c::Refusal::Stale(c::RefusalStale { newest: 7 }), at: None },
        changes: vec![],
    });
    let mut story = Story {
        client: client::Client::unlinked(&limits, 1).expect("valid public story fixture"),
        engine: Engine::new(setup, 1).expect("valid public story fixture"),
        told: vec![],
        now: Time::ZERO,
    };
    story.event(client::Event::StartUnlinked { kept: List::with_capacity(0) });
    story.event(client::Event::Up {
        up: client::Up::Served { served: true, versions: Some(c::VersionRange { first: 19, last: 19 }) },
    });
    story.event(client::Event::SignIn { with: client::With::Transport });
    let watch = client::Handle { asker: client::Asker::JIG, number: 8 };
    story.event(client::Event::Watch { handle: watch, subject: client::Watchable::Jig(c::Subject::Settings) });
    let queries = [
        c::Read::Settings,
        c::Read::Setting(c::ReadSetting { setting: c::Setting::Rules, revision: 6, proposal: None }),
        c::Read::Revision(c::ReadRevision { number: 6 }),
        c::Read::Revisions(c::ReadRevisions { before: None }),
    ];
    for (number, what) in queries.into_iter().enumerate() {
        let handle = client::Handle {
            asker: client::Asker::JIG,
            number: u64::try_from(number)
                .expect("valid public story fixture")
                .checked_add(1)
                .expect("valid public story fixture"),
        };
        story.event(client::Event::Read { handle, read: client::Readable::Jig(what) });
    }
    assert!(story.told.iter().any(|t| matches!(t, client::Told::Page { page: client::Readout::Jig(c::ReadResult::Value(c::ReadValue::Settings(v))), .. } if **v == values::settings_document())));
    assert!(story.told.iter().any(|t| matches!(t, client::Told::Page { page: client::Readout::Jig(c::ReadResult::Value(c::ReadValue::Revision(v))), .. } if **v == c::RevisionAnswer::Revision(values::settings_revision()))));
    for (number, request) in [values::settings_change(), change].into_iter().enumerate() {
        let key = client::Key {
            asker: client::Asker::JIG,
            number: u64::try_from(number)
                .expect("valid public story fixture")
                .checked_add(1)
                .expect("valid public story fixture"),
        };
        story.event(client::Event::Send {
            key,
            name: None,
            request: client::Ask::Jig(Box::new(c::Ask::ChangeSettings(request))),
        });
    }
    assert!(story.told.iter().any(|t| matches!(
        t,
        client::Told::Answered {
            outcome: client::Outcome::Answered(client::Answered::Jig(c::Answer::Done(c::Outcome::SettingsChanged(
                c::OutcomeSettingsChanged { revision: 7 }
            )))),
            ..
        }
    )));
    assert!(story.told.iter().any(|t| matches!(
        t,
        client::Told::Answered {
            outcome: client::Outcome::Answered(client::Answered::Jig(c::Answer::Refused {
                refusal: c::Refusal::Stale(c::RefusalStale { newest: 7 }),
                ..
            })),
            ..
        }
    )));
    assert!(
        matches!(story.client.document(watch).expect("valid public story fixture").latest, Some(client::Latest::Jig(c::Delivery::Settings(value))) if value.newest == 7)
    );
    assert!(story.told.iter().any(|t| matches!(t, client::Told::Page { page: client::Readout::Jig(c::ReadResult::Value(c::ReadValue::Setting(value))), .. } if value.setting == c::Setting::Rules && value.revision == 6)));
}
