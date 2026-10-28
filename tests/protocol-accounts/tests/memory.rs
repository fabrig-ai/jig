use jig_protocol_accounts::table::{Scheme, Table, Value};
use skein_heap::{Counting, Meter};
use skein_lib::{Secret, Time, bytes};
#[global_allocator]
static HEAP: Counting = Counting;
#[test]
fn the_table_keeps_two_generations_at_its_account_and_token_limits() {
    let accounts = 2;
    let token_bytes = 32;
    let mut table = Table::new(accounts, token_bytes);
    let token_length = usize::try_from(token_bytes).expect("token bound");
    for account in 0..accounts {
        for generation in 1..=2 {
            table
                .granted(
                    account,
                    generation,
                    Value { token: Secret::new(bytes::zeroed(token_length)), scheme: Scheme::Bearer },
                    Time::from_nanos(1000),
                )
                .expect("full value fits");
        }
    }
    for account in 0..accounts {
        for generation in 1..=2 {
            assert_eq!(
                table.value(account, generation, Time::ZERO).expect("both generations retained").token.len(),
                token_length
            );
        }
    }
}

#[test]
fn llm_keys_at_the_source_limit_wait_for_adoption() {
    use jig_protocol_accounts::keys::{Keys, Next};
    let mut keys = Keys::new(2, 32);
    for source in 1..=2 {
        let Next::Store { exchange, .. } = keys.keep(source, u64::from(source), false) else { panic!("carried read") };
        let next = keys.read(exchange, Some(Secret::new(bytes::zeroed(32))));
        assert!(matches!(next, Some(Next::Store { .. })));
        assert!(keys.contains(source));
    }
}

#[test]
fn the_source_inventory_sweeps_unnamed_rows_at_its_limits() {
    use jig_protocol_accounts::restart::{Inventory, Sweep};
    let mut inventory = Inventory::new(2, 2);
    inventory.name(1);
    inventory.name(2);
    inventory.restored();
    assert!(matches!(inventory.step(), Some(Sweep::Load { after: None })));
    for number in 1..=4 {
        inventory.row(number);
    }
    inventory.loaded(None);
    for expected in 3..=4 {
        assert!(matches!(inventory.step(), Some(Sweep::Erase { number }) if number == expected));
        inventory.erased();
    }
    assert!(inventory.step().is_none());
    assert!(!inventory.active());
}

#[test]
fn the_moved_codec_at_its_ceilings_fits_its_heap_bound() {
    use jig_protocol_accounts::v1::{CEILINGS, Moved, MovedParts};
    use skein_lib::{Reader, Writer};

    let field = |length| bytes::zeroed(usize::try_from(length).expect("schema ceiling fits"));
    let value = Moved::new(
        &CEILINGS,
        MovedParts {
            application: field(CEILINGS.moved_application),
            issuer: field(CEILINGS.moved_issuer),
            client_id: field(CEILINGS.moved_client_id),
            subject: field(CEILINGS.moved_subject),
            display: field(CEILINGS.moved_display),
            record: Secret::new(field(CEILINGS.moved_record)),
        },
    )
    .expect("parts at their ceilings");
    let mut writer = Writer::new(usize::try_from(value.measure()).expect("wire size"));
    value.encode(&mut writer).expect("measured writer");
    let input = writer.finish();
    drop(value);
    let bound = Moved::worst_case_heap(&CEILINGS).expect("codec heap bound");
    let meter = Meter::new();
    meter.start();
    let decoded = Moved::decode(&CEILINGS, &mut Reader::new(&input)).expect("full codec value");
    let measured = meter.end();
    meter.check(measured, bound, &"moved codec");
    std::hint::black_box(&decoded);
}
