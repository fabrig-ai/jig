use crate::{relays::Relays, tests::fixtures};
use skein_channel::{Frame, frame_writer};
use skein_lib::Token;
fn record(name: &[u8]) -> Frame {
    let mut frame = frame_writer(0x0102, u32::try_from(name.len()).expect("small name")).expect("frame");
    frame.put(name).expect("body");
    frame.finish().expect("complete")
}
fn add(relays: &mut Relays, run: u64, name: &[u8]) -> bool {
    relays.add(Token::new(run), Token::new(2), name.into(), &record(name)).is_ok()
}
#[test]
fn a_relay_answered_ends_once_and_a_late_cancel_finds_nothing() {
    let mut relays = Relays::new(&fixtures::LIMITS);
    assert!(add(&mut relays, 7, b"first"));
    assert!(relays.end(Token::new(7), Token::new(2), b"first"));
    assert!(!relays.end(Token::new(7), Token::new(2), b"first"));
    assert!(relays.replay(0).is_none());
}
#[test]
fn relays_go_again_in_the_order_first_sent() {
    let mut limits = fixtures::LIMITS;
    limits.sizes.calls = 3;
    let mut relays = Relays::new(&limits);
    for name in [b"third".as_slice(), b"first", b"second"] {
        assert!(add(&mut relays, 7, name));
    }
    assert!(relays.end(Token::new(7), Token::new(2), b"first"));
    for (index, name) in [(0, b"third".as_slice()), (1, b"second")] {
        assert_eq!(relays.replay(index).expect("record").bytes().get(8..), Some(name));
    }
    assert!(relays.replay(2).is_none());
}
#[test]
fn the_relay_table_holds_every_slot_and_call() {
    let mut limits = fixtures::LIMITS;
    limits.sizes.slots = 2;
    limits.sizes.calls = 2;
    let mut relays = Relays::new(&limits);
    for run in [7, 8] {
        for name in [b"first".as_slice(), b"second"] {
            assert!(add(&mut relays, run, name));
        }
    }
    assert!(!add(&mut relays, 9, b"extra"));
    assert!(relays.end(Token::new(7), Token::new(2), b"first"));
    assert!(add(&mut relays, 9, b"extra"));
}
#[test]
fn a_repeated_call_keeps_its_first_record_and_position() {
    let mut relays = Relays::new(&fixtures::LIMITS);
    assert!(add(&mut relays, 7, b"first"));
    relays.add(Token::new(7), Token::new(2), b"first".as_slice().into(), &record(b"new")).expect("existing relay");
    assert_eq!(relays.replay(0).expect("record").bytes().get(8..), Some(b"first".as_slice()));
    assert!(relays.replay(1).is_none());
}
