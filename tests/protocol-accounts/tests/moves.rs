use jig_protocol_accounts_world::{
    move_world::{MoveStory, Wrong, run_move},
    world,
};
use skein_fake_peers::oauth::Observation;
#[global_allocator]
static HEAP: skein_heap::Counting = skein_heap::Counting;

fn assert_exchanges(observed: &[Observation], statuses: &[u16]) {
    let posts = observed.iter().filter(|event| matches!(event, Observation::Post { .. })).count();
    let answered = observed
        .iter()
        .filter_map(|event| match event {
            Observation::Answered { status, .. } => Some(*status),
            Observation::Accepted { .. }
            | Observation::Authorize { .. }
            | Observation::Post { .. }
            | Observation::Document { .. }
            | Observation::Revoke { .. }
            | Observation::Redirect { .. }
            | Observation::Closed { .. } => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(posts, statuses.len(), "one exchange per owner, each a refresh");
    assert_eq!(answered, statuses);
    assert!(
        !observed.iter().any(|event| matches!(event, Observation::Authorize { .. } | Observation::Revoke { .. })),
        "only refreshes, no sign-in or revocation"
    );
}
#[test]
fn a_moved_record_leaves_one_owner_and_is_refreshed_once_by_the_other() {
    let outcome = run_move(1, MoveStory::Written);
    assert_exchanges(world::issuer_saw(&outcome), &[200]);
}
#[test]
fn the_file_left_behind_is_dead() {
    let outcome = run_move(1, MoveStory::Dead);
    assert_exchanges(world::issuer_saw(&outcome), &[200, 400]);
}
#[test]
fn the_file_left_behind_is_dead_after_a_cancelled_move() {
    let outcome = run_move(1, MoveStory::Cancelled);
    assert_exchanges(world::issuer_saw(&outcome), &[200, 400]);
}
#[test]
fn a_move_in_of_another_applications_file_is_not_ours() {
    for wrong in [Wrong::Application, Wrong::Issuer, Wrong::Client] {
        let outcome = run_move(1, MoveStory::NotOurs(wrong));
        assert_exchanges(world::issuer_saw(&outcome), &[]);
    }
}
#[test]
fn a_restart_while_moving_refreshes_before_lending() {
    let outcome = run_move(1, MoveStory::Restart);
    assert_exchanges(world::issuer_saw(&outcome), &[200]);
}

#[test]
fn adoption_preserves_the_stored_record_and_expires_only_the_drivers_copy() {
    use jig_domain_accounts::Event;
    use jig_protocol_accounts::{
        moved,
        moves::{Moves, Next, Store},
        party,
        records::Records,
    };
    use jig_protocol_accounts_world::fixtures;
    use skein_lib::{Duration, List, Map, Wall, bytes, sha256};
    use skein_oauth_accounts as driver;

    let limits = fixtures::limits().client.document;
    let mut record = fixtures::kept();
    record.subject = Some(bytes::copy_of(b"subject"));
    record.issued_client_id = Some(bytes::copy_of(b"issued-client"));
    record.earliest_refresh_at = Some(Wall::from_nanos(Duration::from_secs(200).as_nanos()));
    let file = moved::encode(
        &moved::Moved {
            application: bytes::copy_of(fixtures::MOVE_APPLICATION),
            issuer: bytes::copy_of(b"http://127.0.0.1:31000"),
            client_id: bytes::copy_of(b"issued-client"),
            subject: bytes::copy_of(b"subject"),
            display: bytes::copy_of(b"Moved"),
            record: record.clone(),
        },
        &limits,
    )
    .expect("moved fixture");
    let mut providers = Map::with_capacity(1);
    assert!(
        providers
            .insert(
                1,
                party::party_account(
                    &fixtures::party_registration(world::Story::SignIn),
                    driver::Endpoint {
                        address: "127.0.0.1:31000".parse().expect("issuer socket"),
                        transport: driver::Transport::Plaintext
                    },
                    List::with_capacity(0),
                    &[5; 16],
                )
            )
            .is_ok()
    );
    let mut moves = Moves::new(bytes::copy_of(fixtures::MOVE_APPLICATION), 1);
    let source = 17;
    let carried = jig_protocol_store::CARRIED;
    let _read = moves.open(source, carried);
    assert!(matches!(
        moves.opened(source, Some(file.clone()), &providers, &limits),
        Next::Child(Event::Opened { provider: 1, .. })
    ));
    let Next::Store { request: Store::AdoptValue { value, holds, .. }, .. } = moves.adopt(source, carried, &limits)
    else {
        panic!("decoded adoption")
    };
    assert_eq!(holds, sha256::digest(file.expose()));
    record.key = source;
    assert!(
        skein_oauth::decode_record(value.expose(), &limits).expect("adopted record") == record,
        "only the key changes in storage"
    );
    let adopted = moves.adopted(source, true).expect("durable adoption");
    assert_eq!(adopted.value, value);
    let mut records = Records::new(1);
    let account = party::restore(
        providers.remove(&adopted.provider).expect("configured provider"),
        source,
        Some(adopted.value),
        &mut records,
        &limits,
    );
    moves.stage(source, account);
    let driver::Account::SignIn { keeper: driver::Keeper::Owner { kept: Some(copy) }, .. } =
        moves.ready(source, true).expect("staged source")
    else {
        panic!("owner's driver copy")
    };
    record.expires_at = Wall::from_nanos(0);
    record.earliest_refresh_at = None;
    assert!(copy == record, "only the driver's copy is disowned");
    let jig_protocol_accounts::records::Next::Store(keep) = records.keep(source, copy, &limits) else {
        panic!("next keep")
    };
    assert_eq!(
        keep.replaces,
        Some(sha256::digest(value.expose())),
        "the next keep replaces the unmodified durable row"
    );
}

#[test]
fn the_small_profile_file_bound_loses_the_magic_bytes() {
    // protocol/llm.md, 5.6: 26 fixed bytes, outer caps totaling 2,686,
    // and the small profile's 65,536-byte record allowance.
    const FILE_BYTES: u64 = 68_248;
    let profile = jig_protocol_config::profile(jig_protocol_config::Profile::Small);
    assert_eq!(jig_protocol_accounts::moved::bound(&profile.driver.client.document), FILE_BYTES);
}
