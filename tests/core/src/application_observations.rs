//! Translate only authenticated peer input and transactions received by the store.
use crate::Store;
use jig_conformance::referee as r;
use jig_domain_core as core;
use jig_domain_people as people;
use jig_test_connector as connector;
use jig_test_domain as root;
use jig_test_domain::Write;

pub(crate) fn requested(store: &Store, event: &mut root::Event) -> Option<r::Observed> {
    let root::Event::Application { reply_to, sign_in, key, number, digest, .. } = event else { return None };
    let person = store.rows.values().find_map(|row| match row {
        root::Record::Core(core::Record::People(people::Stored::SignIn { number, person, .. }))
            if number == sign_in =>
        {
            Some(*person)
        }
        root::Record::Core(_) | root::Record::Connector { .. } => None,
    })?;
    Some(r::Observed::ApplicationRequested {
        reply: crate::observations::destination(reply_to),
        key: r::ApplicationKey { person, key: *key },
        connector: *number,
        digest: *digest,
    })
}

fn marker(row: &root::Record, connector: u16, token: u64) -> Option<(r::ApplicationKey, [u8; 32])> {
    if let root::Record::Core(core::Record::People(people::Stored::Answer {
        key,
        ask,
        outcome: people::Outcome::Applied { connector: answered, token: saved },
        ..
    })) = row
        && let people::Ask::Application { connector: owner, digest, .. } = ask.as_ref()
        && *owner == connector
        && *answered == connector
        && saved.raw() == token
    {
        Some((r::ApplicationKey { person: key.person, key: key.key }, *digest))
    } else {
        None
    }
}

pub(crate) fn submitted(store: &Store) -> Vec<r::Observed> {
    let Some(pending) = store.pending.back() else { return Vec::new() };
    let mut observed = Vec::new();
    for write in &pending.writes {
        if let Write::Save(root::Record::Connector {
            number,
            record: connector::Record::ApplicationAnswer { token, until, outcome },
        }) = write
        {
            let (key, digest) = pending
                .writes
                .iter()
                .find_map(|write| match write {
                    Write::Save(row) => marker(row, *number, token.raw()),
                    Write::Wires | Write::Erase(_) | Write::CopySettingsRevision(_) => None,
                })
                .or_else(|| store.rows.values().find_map(|row| marker(row, *number, token.raw())))
                .expect("owner terminal has its shared key metadata");
            observed.push(r::Observed::ApplicationSubmitted {
                commit: pending.number,
                answer: r::ApplicationAnswer {
                    key,
                    connector: *number,
                    digest,
                    token: token.raw(),
                    until: until.as_nanos(),
                    outcome: encoded(outcome),
                },
            });
        }
    }
    observed
}

/// The testing world owns this opaque encoding of its connector's complete outcome.
pub(crate) fn encoded(outcome: &connector::ApplicationOutcome) -> Box<[u8]> {
    format!("{outcome:?}").into_bytes().into_boxed_slice()
}
