use super::fixtures::{self, CREDENTIAL, Client};
use crate::*;
use skein_lib::List;
const OWNERS: &[Spoken] = &[Spoken { family: 19, version: 1 }];

#[test]
fn a_declared_owner_the_hello_does_not_name_is_not_served_before_any_send() {
    let mut limits = fixtures::limits();
    limits.domain.owners = OWNERS;
    let mut client = Client::with_limits(limits, List::with_capacity(0));
    assert_eq!(client.machine.link().not_served, &[]);
    client.step(Event::SignIn { with: With::Held(fixtures::credential(&CREDENTIAL)) });
    client.stream(fixtures::hello(jig_client_protocol_wire::VERSION));
    assert_eq!(client.machine.link().not_served, &[19]);
    assert_eq!(fixtures::keyed_count(client.requests()), 0);
}
