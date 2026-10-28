//! The transport owns methods, routes and headers.
use crate::Mapping;
use alloc::boxed::Box;
use jig_client_protocol_link::{Heard, Message};
use skein_http::{Header, Method};
use skein_lib::{
    List, Reader, Secret, Token, Writer,
    base64::{self, Alphabet},
    bytes,
};
#[derive(Debug, PartialEq, Eq)]
pub struct Call {
    pub method: Method,
    pub path: Box<[u8]>,
    pub headers: List<Header>,
    pub body: Box<[u8]>,
}
#[must_use]
pub fn exchange(mapping: &Mapping, credential: Option<&Secret>, message: Message) -> Call {
    let (route, body): (&[u8], Box<[u8]>) = match message {
        Message::Authorize(body) => (b"/v1/authorize".as_slice(), body),
        Message::Request(body) => (b"/v1/request".as_slice(), body),
        Message::Read(body) => (b"/v1/read".as_slice(), body),
        Message::Watch(body) => (b"/v1/watch".as_slice(), body),
        Message::Unwatch(body) => (b"/v1/unwatch".as_slice(), body),
        Message::StreamGone(body) => (b"/v1/stream-gone".as_slice(), body),
        Message::SignInLink(body) => (b"/v1/sign-in/link".as_slice(), body),
        Message::SignInService => (b"/v1/sign-in/deployment".as_slice(), Box::new([])),
        Message::SignOut => (b"/v1/sign-out".as_slice(), Box::new([])),
    };
    call(mapping, credential, Method::Post, route, body)
}
pub(crate) fn call(
    mapping: &Mapping,
    credential: Option<&Secret>,
    method: Method,
    route: &[u8],
    body: Box<[u8]>,
) -> Call {
    let mut path = Writer::new(mapping.prefix.len().checked_add(route.len()).expect("bounded route"));
    path.put(&mapping.prefix).expect("measured prefix");
    path.put(route).expect("measured route");
    let mut headers = List::with_capacity(2);
    if method == Method::Post {
        headers
            .push(Header { name: bytes::copy_of(b"Content-Type"), value: bytes::copy_of(b"application/octet-stream") })
            .expect("header capacity");
    }
    if let Some(credential) = credential {
        let size = base64::encoded_len(Alphabet::Url, u32::try_from(credential.len()).expect("bounded credential"))
            .expect("bounded bearer");
        let mut value = Writer::new(usize::try_from(size).expect("u32 fits").checked_add(7).expect("bounded bearer"));
        value.put(b"Bearer ").expect("measured bearer");
        base64::encode(Alphabet::Url, credential.expose(), &mut value).expect("measured bearer");
        headers
            .push(Header { name: bytes::copy_of(b"Authorization"), value: value.finish() })
            .expect("header capacity");
    }
    Call { method, path: path.finish(), headers, body }
}
#[must_use]
pub fn answered(exchange: Token, status: u16, body: Box<[u8]>) -> Heard {
    if status == 200 {
        return Heard::Reply { exchange, message: body };
    }
    if status == 409 {
        let mut reader = Reader::new(&body);
        if let Ok(full) =
            jig_client_protocol_wire::v19::StreamFull::decode(&jig_client_protocol_wire::v19::CEILINGS, &mut reader)
            && reader.remaining() == 0
        {
            let bound = match full.bound() {
                jig_client_protocol_wire::v19::StreamBound::Component => jig_client_domain::StreamBound::Component,
                jig_client_protocol_wire::v19::StreamBound::Transport => jig_client_domain::StreamBound::Transport,
                jig_client_protocol_wire::v19::StreamBound::Exchanges => jig_client_domain::StreamBound::Exchanges,
            };
            return Heard::Full { exchange, full: jig_client_domain::StreamFull { bound, size: full.size() } };
        }
    }
    Heard::Lost { exchange }
}

#[must_use]
pub fn stream(mapping: &Mapping, credential: Option<&Secret>) -> Call {
    call(mapping, credential, Method::Get, b"/v1/stream", Box::new([]))
}
