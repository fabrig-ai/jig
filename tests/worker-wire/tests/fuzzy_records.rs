use jig_protocol_worker_wire as wire;
use skein_lib::{Reader, Rng, Writer};

macro_rules! check {
    ($bytes:expr, $record:ident) => {
        if let Ok(value) = wire::$record::decode(&wire::CEILINGS, &mut Reader::new($bytes)) {
            let mut writer = Writer::new(usize::try_from(value.measure()).expect("measured length"));
            value.encode(&mut writer).expect("measured room");
            assert_eq!(writer.finish().as_ref(), $bytes);
        }
    };
}

fn random_bytes(rng: &mut Rng) -> Vec<u8> {
    let len = usize::try_from(rng.below(256)).expect("small length");
    (0..len).map(|_| u8::try_from(rng.below(256)).expect("a byte")).collect()
}

#[test]
fn fuzzy_up_decoders_never_panic_and_reencode_what_they_read() {
    let mut rng = Rng::new(0x22_0001);
    for _ in 0..20_000 {
        let bytes = random_bytes(&mut rng);
        check!(&bytes, Credential);
        check!(&bytes, Hello);
        check!(&bytes, Turn);
        check!(&bytes, Call);
        check!(&bytes, Answer);
        check!(&bytes, Fact);
        check!(&bytes, Piece);
        check!(&bytes, InputCall);
        check!(&bytes, LiveAsk);
        check!(&bytes, Unauthorized);
        check!(&bytes, Limited);
        check!(&bytes, AccountRefused);
        check!(&bytes, Logins);
        check!(&bytes, SignedOut);
    }
}

#[test]
fn fuzzy_down_decoders_never_panic_and_reencode_what_they_read() {
    let mut rng = Rng::new(0x22_0002);
    for _ in 0..20_000 {
        let bytes = random_bytes(&mut rng);
        check!(&bytes, Assignment);
        check!(&bytes, Inbound);
        check!(&bytes, Relayed);
        check!(&bytes, TurnKept);
        check!(&bytes, TurnBusy);
        check!(&bytes, Acknowledge);
        check!(&bytes, Cancel);
        check!(&bytes, Park);
        check!(&bytes, Granted);
        check!(&bytes, Sources);
        check!(&bytes, Check);
        check!(&bytes, SignOut);
    }
}
