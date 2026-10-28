//! Incremental SSE and canonical base64 framing (protocol/client.md, 4 and 7).
use crate::Limits;
use jig_client_protocol_link::{Heard, Streamed};
use jig_client_protocol_wire::v19 as w;
use skein_http::sse;
use skein_lib::base64::{Alphabet, Decoder};
use skein_lib::stream::{Down, Read, Up};
use skein_lib::{Env, Queue, Time, Wall, bytes};

// StreamBound is one tag byte, followed by the four-byte size.
const FULL_BYTES: u32 = 5;

#[derive(Debug)]
pub struct Stream {
    reader: sse::Reader,
    decoder: Decoder,
    active: bool,
    refusal: Option<skein_lib::Writer>,
    env: Env<sse::Limits>,
    above: Queue<sse::Event>,
    below: Queue<Down>,
    decoded: u32,
    line_start: bool,
    comment: bool,
}
impl Stream {
    #[must_use]
    pub fn new(limits: &Limits) -> Self {
        let (decoded, framing) = bounds(*limits).expect("validated stream bounds");
        let mut stream = Self {
            reader: sse::Reader::new(&framing),
            decoder: Decoder::new(Alphabet::Standard, decoded),
            active: true,
            refusal: None,
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits: framing },
            above: Queue::with_capacity(2),
            below: Queue::with_capacity(2),
            decoded,
            line_start: true,
            comment: false,
        };
        stream.demand(sse::Request::Next);
        stream
    }
    /// A 409 carries a bounded stream-full record; 401 refuses signed out.
    pub fn opened(&mut self, status: u16, out: &mut Queue<Heard>) {
        if status == 401 && self.active {
            self.active = false;
            out.push(Heard::Streamed { message: Streamed::SignedOut });
        } else if status == 409 && self.active {
            self.refusal = Some(skein_lib::Writer::new(usize::try_from(FULL_BYTES).expect("u32 fits")));
        } else if status != 200 {
            self.ended(out);
        }
    }
    pub fn ended(&mut self, out: &mut Queue<Heard>) {
        if self.active {
            self.active = false;
            if let Some(writer) = self.refusal.take()
                && writer.room() == 0
            {
                let bytes = writer.finish();
                let mut reader = skein_lib::Reader::new(&bytes);
                if let Ok(full) = w::StreamFull::decode(&w::CEILINGS, &mut reader)
                    && reader.remaining() == 0
                {
                    let bound = match full.bound() {
                        w::StreamBound::Component => jig_client_domain::StreamBound::Component,
                        w::StreamBound::Transport => jig_client_domain::StreamBound::Transport,
                        w::StreamBound::Exchanges => jig_client_domain::StreamBound::Exchanges,
                    };
                    out.push(Heard::Streamed {
                        message: Streamed::Full(jig_client_domain::StreamFull { bound, size: full.size() }),
                    });
                    return;
                }
            }
            out.push(Heard::StreamGone);
        }
    }
    pub fn body(&mut self, piece: &[u8], out: &mut Queue<Heard>) {
        if !self.active {
            return;
        }
        if piece.len() > usize::try_from(self.env.limits.chunk).expect("u32 fits") {
            self.ended(out);
            return;
        }
        if let Some(writer) = &mut self.refusal {
            if writer.put(piece).is_err() {
                self.refusal = None;
                self.ended(out);
            }
            return;
        }
        let mut at = 0_usize;
        for _ in 0..piece.len() {
            if at == piece.len() || !self.active {
                break;
            }
            let line = line_piece(piece.get(at..).expect("within body"), self.env.limits.chunk);
            at = at.checked_add(line.len()).expect("bounded body");
            if self.piece(line, out).is_err() {
                self.ended(out);
            }
        }
    }
    /// The reader's line scans are split at their first ending, within its chunk bound.
    fn piece(&mut self, piece: &[u8], out: &mut Queue<Heard>) -> Result<(), ()> {
        if self.line_start {
            self.comment = piece.first() == Some(&b':');
            self.line_start = false;
        }
        let ended = piece.last() == Some(&b'\r') || piece.last() == Some(&b'\n');
        let heartbeat = ended && self.comment;
        if ended {
            self.line_start = true;
            self.comment = false;
        }
        sse::up(&mut self.reader, &self.env, Up::Bytes(bytes::copy_of(piece)), &mut self.above, &mut self.below);
        for _ in 0..self.env.limits.chunk.saturating_mul(3).saturating_add(8) {
            let Some(event) = self.above.pop() else { break };
            match event {
                sse::Event::Opened => self.data(),
                sse::Event::Data(Up::Bytes(piece)) => {
                    self.decoder.push(&piece).or(Err(()))?;
                    self.data();
                }
                sse::Event::Data(Up::End) => {}
                sse::Event::Dispatched(_) => {
                    let decoder = core::mem::replace(&mut self.decoder, Decoder::new(Alphabet::Standard, self.decoded));
                    let body = decoder.finish().or(Err(()))?;
                    out.push(Heard::Streamed { message: Streamed::Record(body) });
                    self.demand(sse::Request::Next);
                }
                sse::Event::Data(Up::Failed(_)) | sse::Event::Failed(_) | sse::Event::Ended => {
                    return Err(());
                }
                sse::Event::Data(Up::Room) | sse::Event::Closed => unreachable!("reader is read-only and active"),
            }
        }
        self.clear_demands();
        if heartbeat {
            out.push(Heard::Streamed { message: Streamed::Heartbeat });
        }
        Ok(())
    }
    fn data(&mut self) {
        self.demand(sse::Request::Data(Down::Demand { read: Read::Arrived { max: self.env.limits.chunk }, room: 0 }));
    }
    fn demand(&mut self, request: sse::Request) {
        self.clear_demands();
        sse::down(&mut self.reader, &self.env, request, &mut self.above, &mut self.below);
    }
    fn clear_demands(&mut self) {
        for _ in 0_u32..2 {
            let Some(request) = self.below.pop() else { break };
            match request {
                Down::Demand { read: Read::Line { .. }, room: 0 } => {}
                Down::Demand { .. } | Down::Send(_) | Down::Finish => unreachable!("SSE only reads lines"),
            }
        }
    }
}
fn bounds(limits: Limits) -> Option<(u32, sse::Limits)> {
    if limits.record == 0 || limits.chunk == 0 {
        return None;
    }
    let decoded = limits.record;
    let encoded = skein_lib::base64::encoded_len(Alphabet::Standard, decoded)?;
    let event = encoded.checked_add(256)?;
    Some((decoded, sse::Limits { line: event, event, field: 64, chunk: limits.chunk }))
}
#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    let (decoded, framing) = bounds(*limits)?;
    sse::worst_case(&framing)?
        .checked_add(u64::from(framing.chunk))?
        .checked_add(u64::from(decoded).checked_mul(2)?)?
        .checked_add(u64::from(FULL_BYTES))?
        .checked_add(Queue::<sse::Event>::worst_case(2)?)?
        .checked_add(Queue::<Down>::worst_case(2)?)
}

fn line_piece(bytes: &[u8], chunk: u32) -> &[u8] {
    let maximum = core::cmp::min(bytes.len(), usize::try_from(chunk).expect("u32 fits usize"));
    let piece = bytes.get(..maximum).expect("bounded scan");
    for (at, byte) in piece.iter().enumerate() {
        if *byte == b'\r' || *byte == b'\n' {
            return piece.get(..at.saturating_add(1)).expect("scan stops at ending");
        }
    }
    piece
}
