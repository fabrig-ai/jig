use jig_domain_accounts::{Failure, FromComponent, Place, Unread};
use jig_protocol_accounts::{
    keys::Keys,
    party,
    records::Records,
    restart::{Inventory, Sweep},
    table::Table,
    translate,
};
use jig_protocol_accounts_world::{fixtures, key_world::World, world::Story};
use jig_protocol_store::{SecretOwner, Secrets};
use skein_lib::{CryptoRng, Env, List, Queue, Secret, Time, Wall};
use skein_oauth_accounts as driver;

#[test]
fn a_restart_loads_a_kept_key() {
    let mut world = World::new();
    let carried = world.carry(&[7; 32]);
    world.keep(7, carried, false);
    let row = world.get(SecretOwner::Source, 7);
    let mut keys = Keys::new(2, 32);
    let mut table = Table::new(2, 32);
    keys.restore(7, row, &mut table).expect("valid protocol world fixture");
    assert!(matches!(keys.want(7, Time::ZERO), Some(FromComponent::Granted { account: 7, generation: 1, .. })));
    assert_eq!(table.value(7, 1, Time::ZERO).expect("valid protocol world fixture").token.expose(), &[7; 32]);
    let row = world.get(SecretOwner::Source, 7);
    assert_eq!(row, Some(Secret::new(Box::new([7; 32]))));
}
#[test]
fn a_restart_re_adds_a_party_signed_in_source() {
    let limits = fixtures::limits();
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let mut records = Records::new(limits.accounts);
    let mut saved = fixtures::kept();
    saved.key = 7;
    saved.issued_client_id = Some(b"issued-client".as_slice().into());
    saved.granted_scopes = Some(b"openid read".as_slice().into());
    saved.subject = Some(b"subject".as_slice().into());
    let value = skein_oauth::encode_record(&saved, &limits.client.document).expect("valid protocol world fixture");
    let registration = fixtures::party_registration(Story::SignIn);
    let template = party::party_account(
        &registration,
        driver::Endpoint {
            address: std::net::SocketAddr::from(([127, 0, 0, 1], 31000)),
            transport: driver::Transport::Plaintext,
        },
        List::with_capacity(0),
        &[5; 16],
    );
    let restored = party::restore(template, 7, Some(Secret::new(value)), &mut records, &limits.client.document);
    let crypto = CryptoRng::new(&skein_world::crypto_seed(17), 0).expect("valid protocol world fixture");
    let mut component =
        driver::Component::new(List::with_capacity(0), &limits, crypto).expect("valid protocol world fixture");
    let mut above = Queue::with_capacity(64);
    let mut io = Queue::with_capacity(64);
    let mut files = Queue::with_capacity(64);
    component.add(&env, 7, restored, &mut above);
    assert!(above.is_empty(), "party source re-added");
    component.down(&env, driver::Request::Grant { account: 7 }, &mut above, &mut io, &mut files);
    let event = records.up(above.pop().expect("restored source grant"), Time::ZERO, b"");
    assert!(matches!(event, translate::Up::Granted { account: 7, generation: 1, .. }), "{event:?}");
}
#[test]
fn the_restart_erases_only_unnamed_source_rows() {
    let mut world = World::new();
    for number in [1, 7, 8, 9, 10] {
        world.secret(Secrets::Keep {
            owner: SecretOwner::Source,
            number,
            value: jig_protocol_store::SecretValue::Key(Secret::new(Box::new([7; 32]))),
        });
    }
    let carried = world.carry(&[9; 32]);
    let mut inventory = Inventory::new(2, 2);
    inventory.name(1); // restored deployment-held key source
    inventory.name(7); // restored party-held signed-in source
    assert!(inventory.step().is_none(), "nothing erased before Restored");
    inventory.restored();
    inventory.restored(); // ends the same pass, never starts another
    let mut loads = 0;
    let mut erased = Vec::new();
    for _ in 0..32 {
        match inventory.step() {
            Some(Sweep::Load { after }) => {
                loads += 1;
                let event = world.secret(Secrets::Load { owner: SecretOwner::Source, after });
                let jig_protocol_store::Event::SecretsLoaded { rows, next, .. } = event else { panic!("source page") };
                for row in rows {
                    inventory.row(row.number);
                }
                inventory.loaded(next);
            }
            Some(Sweep::Erase { number }) => {
                erased.push(number);
                world.secret(Secrets::Erase { owner: SecretOwner::Source, number });
                inventory.erased();
            }
            None if !inventory.work_pending() => break,
            None => (),
        }
    }
    assert_eq!(erased, [8, 9, 10]);
    assert_eq!(loads, 3, "one bounded page at a time");
    for number in [1, 7] {
        assert!(world.get(SecretOwner::Source, number).is_some());
    }
    for number in [8, 9, 10] {
        assert!(world.get(SecretOwner::Source, number).is_none());
    }
    assert!(world.get(SecretOwner::Application, carried).is_some(), "carried application rows survive");
    world.secret(Secrets::Keep {
        owner: SecretOwner::Source,
        number: 11,
        value: jig_protocol_store::SecretValue::Key(Secret::new(Box::new([7; 32]))),
    });
    inventory.restored();
    assert!(inventory.step().is_none(), "the restart sweeps only once");
    assert!(world.get(SecretOwner::Source, 11).is_some());
}
#[test]
fn a_restart_reports_missing_and_oversized_llm_keys_as_unloaded() {
    for value in [None, Some(Secret::new(Box::new([7; 33])))] {
        let mut keys = Keys::new(2, 32);
        let mut table = Table::new(2, 32);
        keys.restore(7, value, &mut table).expect("valid protocol world fixture");
        assert_eq!(
            keys.want(7, Time::ZERO),
            Some(FromComponent::Failed {
                account: 7,
                failure: Failure::Unloaded { at: Place::File, why: Unread::Unreadable }
            })
        );
        assert!(table.value(7, 1, Time::ZERO).is_none());
    }
}
