use super::fixtures;
use crate::{Stream, worst_case};
use alloc::boxed::Box;
use jig_client_protocol_link::{Heard, Streamed};
use skein_lib::Queue;
const CUTS: [usize; 2] = [fixtures::WHOLE_STREAM.len(), 1];
const STATUSES: [u16; 2] = [201, 503];
#[test]
fn the_body_cut_at_each_byte_gives_the_same_records() {
    for cut in 0..=fixtures::WHOLE_STREAM.len() {
        let (mut stream, mut out) = fixtures::reader();
        stream.body(fixtures::WHOLE_STREAM.get(..cut).unwrap(), &mut out);
        stream.body(fixtures::WHOLE_STREAM.get(cut..).unwrap(), &mut out);
        for expected in fixtures::RECORDS {
            assert_eq!(
                out.pop(),
                Some(Heard::Streamed { message: Streamed::Record(Box::from(expected)) }),
                "cut {cut}"
            );
        }
        assert!(out.is_empty(), "cut {cut}");
    }
}
#[test]
fn an_event_s_data_read_as_it_arrives_gives_the_same_record() {
    for size in CUTS {
        let (mut stream, mut out) = fixtures::reader();
        for piece in fixtures::WHOLE_STREAM.chunks(size) {
            stream.body(piece, &mut out);
        }
        for expected in fixtures::RECORDS {
            assert_eq!(
                out.pop(),
                Some(Heard::Streamed { message: Streamed::Record(Box::from(expected)) }),
                "size {size}"
            );
        }
        assert!(out.is_empty(), "size {size}");
    }
}
#[test]
fn malformed_base64_ends_the_stream() {
    let (mut stream, mut out) = fixtures::reader();
    stream.body(b"data: !!!!\n\n", &mut out);
    assert_eq!(out.pop(), Some(Heard::StreamGone));
    assert!(out.is_empty());
}
#[test]
fn a_comment_line_is_a_heartbeat() {
    let (mut stream, mut out) = fixtures::reader();
    stream.body(b": heartbeat\n\n", &mut out);
    assert_eq!(out.pop(), Some(Heard::Streamed { message: Streamed::Heartbeat }));
    assert!(out.is_empty());
}
#[test]
fn a_401_is_the_stream_refused_signed_out() {
    let mut stream = Stream::new(&fixtures::stream_limits());
    let mut out = Queue::with_capacity(4);
    stream.opened(401, &mut out);
    assert_eq!(out.pop(), Some(Heard::Streamed { message: Streamed::SignedOut }));
    stream.body(fixtures::WHOLE_STREAM, &mut out);
    stream.ended(&mut out);
    assert!(out.is_empty());
}
#[test]
fn another_status_other_than_200_ends_the_stream() {
    for status in STATUSES {
        let mut stream = Stream::new(&fixtures::stream_limits());
        let mut out = Queue::with_capacity(4);
        stream.opened(status, &mut out);
        assert_eq!(out.pop(), Some(Heard::StreamGone), "status {status}");
        assert!(out.is_empty(), "status {status}");
    }
}
#[test]
fn a_piece_after_the_end_is_dropped() {
    let (mut stream, mut out) = fixtures::reader();
    stream.ended(&mut out);
    assert_eq!(out.pop(), Some(Heard::StreamGone));
    stream.body(fixtures::WHOLE_STREAM, &mut out);
    stream.ended(&mut out);
    assert!(out.is_empty());
}
#[test]
fn the_stream_limits_have_a_finite_worst_case() {
    assert!(worst_case(&fixtures::stream_limits()).is_some());
}

#[test]
fn a_409_preserves_full_across_every_body_cut() {
    for bound in [jig_client_domain::StreamBound::Component, jig_client_domain::StreamBound::Transport] {
        let tag = match bound {
            jig_client_domain::StreamBound::Component => 0,
            jig_client_domain::StreamBound::Transport => 1,
        };
        let body = [tag, 0, 0, 0, 2];
        for cut in 0..=body.len() {
            let (mut stream, mut out) = fixtures::reader();
            stream.opened(409, &mut out);
            assert!(out.is_empty());
            stream.body(&body[..cut], &mut out);
            stream.body(&body[cut..], &mut out);
            stream.ended(&mut out);
            assert_eq!(
                out.pop(),
                Some(Heard::Streamed { message: Streamed::Full(jig_client_domain::StreamFull { bound, size: 2 }) })
            );
            stream.ended(&mut out);
            assert!(out.is_empty());
        }
    }
}
#[test]
fn a_malformed_409_drops_once_without_claiming_full() {
    for body in [&[][..], &[0, 0, 0, 0][..], &[2, 0, 0, 0, 2][..], &[0, 0, 0, 0, 2, 0][..]] {
        let (mut stream, mut out) = fixtures::reader();
        stream.opened(409, &mut out);
        stream.body(body, &mut out);
        stream.ended(&mut out);
        stream.ended(&mut out);
        assert_eq!(out.pop(), Some(Heard::StreamGone));
        assert!(out.is_empty());
    }
}
