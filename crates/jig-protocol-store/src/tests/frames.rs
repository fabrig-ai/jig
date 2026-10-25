//! Turn frame integrity (protocol/store.md, section 8.1).
use super::fixtures;
use crate::transcripts::frame::{self, Unread};
use alloc::boxed::Box;
use skein_lib::Reader;
fn frame() -> Box<[u8]> {
    let turn = fixtures::window_turn(fixtures::LIVE, 2, 3, b"smith's bytes");
    frame::encode(&turn, &fixtures::codec_limits()).expect("bounded frame")
}
#[test]
fn a_frame_decodes_to_the_turn_it_encoded() {
    let limits = fixtures::codec_limits();
    let turn = fixtures::window_turn(fixtures::LIVE, 2, 3, b"smith's bytes");
    let bytes = frame::encode(&turn, &limits).expect("within the turn's limits");
    assert_eq!(frame::decode(&bytes, &limits), Ok(turn));
}

#[test]
fn current_frame_sizing_includes_metadata_and_wrapper_and_rejects_codec_excess() {
    let domain = fixtures::CORE_LIMITS;
    let mut turn = fixtures::turn(fixtures::LIVE, 1, 1, b"tiny");
    turn.read = Some(1);
    let bytes = frame::encode(&turn, &fixtures::codec_limits()).expect("tiny current frame");
    assert_eq!(crate::sizes::frame(&domain, 4), Some(u32::try_from(bytes.len()).expect("tiny frame length")));
    assert_eq!(bytes.len(), 68);
    let ceiling = crate::core_v6::CEILINGS.turn_record_transcript;
    assert_eq!(crate::sizes::frame(&domain, ceiling), ceiling.checked_add(64));
    assert_eq!(crate::sizes::frame(&domain, ceiling.checked_add(1).expect("one past codec ceiling")), None);
    assert_eq!(crate::sizes::frame(&domain, u32::MAX), None);
}
#[test]
fn the_trailing_length_matches_the_leading_one() {
    let bytes = frame();
    let leading = Reader::new(&bytes).u32().expect("leading length");
    let start = bytes.len().checked_sub(4).expect("a whole frame");
    let trailing = Reader::new(bytes.get(start..).expect("trailing bytes")).u32().expect("trailing length");
    assert_eq!(trailing, leading);
    assert_eq!(leading, u32::try_from(bytes.len()).expect("bounded frame"));
}
#[test]
fn a_flipped_byte_fails_the_frames_checksum() {
    let mut bytes = frame();
    *bytes.get_mut(4).expect("first body byte") ^= 1;
    assert_eq!(frame::decode(&bytes, &fixtures::codec_limits()), Err(Unread::Checksum));
}
#[test]
fn a_frame_cut_short_reads_as_short() {
    let bytes = frame();
    let end = bytes.len().checked_sub(1).expect("whole frame");
    assert_eq!(frame::decode(bytes.get(..end).expect("short frame"), &fixtures::codec_limits()), Err(Unread::Short));
}
#[test]
fn crc32c_matches_its_check_value() {
    assert_eq!(frame::crc32c(b"123456789"), 0xe306_9283);
}
#[test]
fn a_turn_past_the_frame_body_bound_is_refused() {
    let mut limits = fixtures::codec_limits();
    limits.core.turn_record_transcript = 4;
    let turn = fixtures::turn(fixtures::LIVE, 1, 1, b"12345");
    assert_eq!(frame::encode(&turn, &limits), None);
}
#[test]
fn a_frame_with_disagreeing_lengths_is_unread() {
    let mut bytes = frame();
    let last = bytes.last_mut().expect("trailing length");
    *last ^= 1;
    assert_eq!(frame::decode(&bytes, &fixtures::codec_limits()), Err(Unread::Lengths));
}
#[test]
fn a_checked_frame_with_no_record_is_unread() {
    let bytes = [0, 0, 0, 12, 0, 0, 0, 0, 0, 0, 0, 12];
    assert_eq!(frame::decode(&bytes, &fixtures::codec_limits()), Err(Unread::Record));
}

fn bitwise_crc32c(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0_u8..8 {
            crc = if crc & 1 == 0 { crc >> 1_u32 } else { (crc >> 1_u32) ^ 0x82f6_3b78 };
        }
    }
    !crc
}

#[test]
fn crc32c_by_table_matches_the_bitwise_loop() {
    for byte in 0_u8..=255 {
        assert_eq!(frame::crc32c(&[byte]), bitwise_crc32c(&[byte]), "one-byte input {byte}");
    }
    let mut rng = skein_lib::Rng::new(0x5701_e016);
    let mut bytes = [0_u8; 1_000];
    for case in 0_u8..64 {
        let length = usize::try_from(rng.below(1_001)).expect("input length fits usize");
        let input = bytes.get_mut(..length).expect("at most 1,000 bytes");
        for byte in &mut *input {
            *byte = u8::try_from(rng.below(256)).expect("one byte");
        }
        assert_eq!(frame::crc32c(input), bitwise_crc32c(input), "seeded input {case}, {length} bytes");
    }
}
