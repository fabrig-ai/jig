use jig_protocol_accounts::table::{self, Scheme, Table, Value};
use skein_heap::{Counting, Meter};
use skein_lib::{Secret, Time, bytes};
#[global_allocator]
static HEAP: Counting = Counting;
#[test]
fn the_table_at_its_limits_stays_within_its_worst_case() {
    let accounts = 2;
    let token_bytes = 32;
    let bound = table::worst_case(accounts, token_bytes).expect("table bound");
    let meter = Meter::new();
    meter.start();
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
    let measured = meter.end();
    meter.check(measured, bound, &"accounts table");
}

#[test]
fn llm_keys_being_adopted_stay_within_their_worst_case() {
    use jig_protocol_accounts::keys::{self, Keys, Next};
    let bound = keys::worst_case(2, 32).expect("LLM key state bound");
    let meter = Meter::new();
    meter.start();
    let mut keys = Keys::new(2, 32);
    for source in 1..=2 {
        let Next::Store { exchange, .. } = keys.keep(source, u64::from(source), false) else { panic!("carried read") };
        let next = keys.read(exchange, Some(Secret::new(bytes::zeroed(32))));
        assert!(matches!(next, Some(Next::Store { .. })));
    }
    let measured = meter.end();
    meter.check(measured, bound, &"LLM key state");
}

#[test]
fn the_source_inventory_stays_within_its_worst_case() {
    use jig_protocol_accounts::restart::{self, Inventory};
    let bound = restart::worst_case(2, 2).expect("source inventory bound");
    let meter = Meter::new();
    meter.start();
    let mut inventory = Inventory::new(2, 2);
    inventory.name(1);
    inventory.name(2);
    inventory.row(3);
    inventory.row(4);
    let measured = meter.end();
    meter.check(measured, bound, &"source inventory");
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
