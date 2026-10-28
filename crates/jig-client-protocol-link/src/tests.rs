use crate::*;
use alloc::boxed::Box;
use jig_client_domain::contract as c;
use jig_client_domain::{Answered, Ask, Family, Readable, Readout, Watchable};
use jig_client_protocol_wire::v19 as w;
use skein_lib::{Queue, Reader, RequestEnvelope, Time, Token};
mod attempts;
mod carried;
mod documents;
mod fixtures;
mod llm_accounts;
mod owner_accounts;
mod pages;

mod reads;

mod stream;

#[test]
fn the_link_serves_the_wire_s_version_alone() {
    assert!(serves(c::VersionRange { first: 19, last: 19 }));
    assert!(!serves(c::VersionRange { first: 7, last: 7 }));
}

const TOO_MANY_OWNERS: [jig_client_domain::Spoken; 16] = {
    let mut owners = [jig_client_domain::Spoken { family: 0, version: 1 }; 16];
    let mut index = 0;
    let mut family = 0;
    while index < 16 {
        owners[index].family = family;
        index += 1;
        family += 1;
    }
    owners
};
const DUPLICATE_OWNERS: [jig_client_domain::Spoken; 2] = [jig_client_domain::Spoken { family: 19, version: 1 }; 2];
const INVALID_OWNERS: [&[jig_client_domain::Spoken]; 2] = [&TOO_MANY_OWNERS, &DUPLICATE_OWNERS];
#[test]
fn owner_declarations_fit_one_hello_without_duplicates() {
    let bounds = fixtures::limits();
    for owners in INVALID_OWNERS {
        let client = jig_client_domain::Limits { owners, turn_text: bounds.client.turn_text, ..bounds.client };
        assert_eq!(Limits::new(client, bounds.exchanges), Err(Unusable::Limits), "{owners:?}");
    }
    let client = jig_client_domain::Limits {
        owners: &TOO_MANY_OWNERS[..15],
        turn_text: bounds.client.turn_text,
        ..bounds.client
    };
    assert_eq!(Limits::new(client, bounds.exchanges).expect("fifteen distinct owners").client.owners.len(), 15);
}

mod store;

mod sign_in;

mod compound;
mod create;

mod accounts;

mod authority_models;

mod programs;
mod projects;
mod turn_text;

mod settings;

mod settings_proposals;

mod placed_holds;

#[test]
fn valid_refuses_each_unrepresentable_limit() {
    let base = fixtures::limits();
    assert!(valid(&base));
    let cases = [
        Limits { exchanges: 0, ..base },
        Limits { wire: w::Limits { authorize_go_location: 1, ..base.wire }, ..base },
        Limits { client: jig_client_domain::Limits { document_bytes: u64::MAX, ..base.client }, ..base },
        Limits { client: jig_client_domain::Limits { watches: 0, ..base.client }, ..base },
        Limits { client: jig_client_domain::Limits { heartbeat: skein_lib::Duration::ZERO, ..base.client }, ..base },
    ];
    for (case, limits) in cases.into_iter().enumerate() {
        assert!(!valid(&limits), "case {case}");
    }
}
