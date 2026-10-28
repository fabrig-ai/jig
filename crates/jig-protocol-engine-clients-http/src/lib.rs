//! The default HTTP mapping's engine end (protocol/client.md, 17.2 and 17.4).
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
use alloc::boxed::Box;
use jig_protocol_engine_clients as kit;
use skein_http::server as http;
use skein_lib::{Env, List, Map, Queue, Secret, Time, Token};
mod call;
mod credential;
pub use credential::{cleared, cookie};
mod dispatch;
mod guards;
mod stream;
#[derive(Debug)]
pub struct Settings {
    pub prefix: Box<[u8]>,
    /// The public name a TLS proxy serves the application at, as `Host` names it (protocol/client.md, 17.3).
    pub public: Option<Box<[u8]>>,
}
/// How an exchange came, decided by its `Host` alone (protocol/client.md, 17.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Via {
    Public,
    Listener,
}
/// `Public` when the call's `Host` is byte-equal to the configured public name,
/// with no case folding or port normalising; `Listener` otherwise, including a missing `Host`.
#[must_use]
pub fn via(settings: &Settings, call: &http::Call) -> Via {
    if let Some(public) = &settings.public
        && call.header(b"host") == Some(public.as_ref())
    {
        return Via::Public;
    }
    Via::Listener
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub calls: u32,
    pub streams: u32,
    pub body: u32,
    pub read: u32,
    pub send: u32,
    pub event: u32,
}
#[derive(Debug)]
pub enum Event {
    Listening { hosts: List<Box<[u8]>> },
    Call { call: Token, head: http::Call },
    Http { call: Token, event: http::Event },
    Component(kit::ToTransport),
}
#[derive(Debug)]
pub enum Request {
    Http { call: Token, request: http::Request },
    Component(kit::FromTransport),
}
#[derive(Debug)]
pub struct Mount {
    settings: Settings,
    hosts: List<Box<[u8]>>,
    calls: Map<Token, call::Call>,
}
impl Mount {
    #[must_use]
    pub fn new(settings: Settings, limits: &Limits) -> Self {
        Self { settings, hosts: List::with_capacity(0), calls: Map::with_capacity(limits.calls) }
    }
    /// Calls the application may hand over, including no call whose end is still queued.
    #[must_use]
    pub fn room(&self) -> u32 {
        self.calls.capacity().checked_sub(self.calls.len()).expect("bounded mounted calls")
    }
    pub fn down(&mut self, env: &Env<Limits>, event: Event, out: &mut Queue<Request>) {
        match event {
            Event::Listening { hosts } => self.hosts = hosts,
            Event::Call { call, head } => {
                let mut value = call::Call::new(call, &env.limits);
                value.via = via(&self.settings, &head);
                value.route = dispatch::route(&self.settings.prefix, &head);
                value.body_kind = head.body;
                value.browser = head.header(b"jig-client") == Some(b"browser");
                let mut forbidden = false;
                value.credential = match credential::read(&head) {
                    credential::Read::Bearer(secret) => Some(secret),
                    credential::Read::Cookie(secret) => {
                        forbidden = !value.browser;
                        Some(secret)
                    }
                    credential::Read::None => None,
                };
                let mut streams = 0_u32;
                for (_, call) in &self.calls {
                    if call.streaming {
                        streams = streams.saturating_add(1);
                    }
                }
                if !guards::host(&self.settings, &self.hosts, &head) {
                    value.route = dispatch::Route::Missing;
                    value.respond(421, Box::new([]), &env.limits);
                } else if value.route == dispatch::Route::Missing {
                    value.respond(404, Box::new([]), &env.limits);
                } else if forbidden {
                    value.route = dispatch::Route::Missing;
                    value.respond(403, Box::new([]), &env.limits);
                } else if !guards::content(&head) {
                    value.route = dispatch::Route::Missing;
                    value.respond(415, Box::new([]), &env.limits);
                } else if let skein_http::server::Body::Length(length) = head.body
                    && length > u64::from(dispatch::body_bound(value.route, env.limits.body))
                {
                    value.route = dispatch::Route::Missing;
                    value.respond(413, Box::new([]), &env.limits);
                } else if value.route == dispatch::Route::Stream && streams >= env.limits.streams {
                    value.route = dispatch::Route::Missing;
                    value.respond(409, Box::new([]), &env.limits);
                } else {
                    value.read(&env.limits);
                }
                self.calls.insert(call, value).expect("server bounds mounted calls");
            }
            Event::Http { call, event } => {
                if let Some(value) = self.calls.get_mut(&call) {
                    value.events.push(event);
                }
            }
            Event::Component(message) => self.component(env, message, out),
        }
    }
    pub fn up(&mut self, env: &Env<Limits>, out: &mut Queue<Request>) -> bool {
        let mut ready = None;
        for (name, call) in &self.calls {
            if !call.requests.is_empty() || !call.events.is_empty() || call.stream_ready() {
                ready = Some(*name);
                break;
            }
        }
        let Some(name) = ready else {
            return false;
        };
        let mut call = self.calls.remove(&name).expect("ready call");
        if let Some(request) = call.requests.pop() {
            out.push(Request::Http { call: name, request });
        } else if let Some(event) = call.events.pop() {
            call.http(env, event, out);
        } else {
            call.stream_up(env, out);
        }
        if !call.closed {
            self.calls.insert(name, call).expect("same call slot");
        }
        true
    }
    fn component(&mut self, env: &Env<Limits>, message: kit::ToTransport, _out: &mut Queue<Request>) {
        let name = match &message {
            kit::ToTransport::Reply { exchange, .. } => *exchange,
            kit::ToTransport::Streamed { stream, .. } | kit::ToTransport::StreamEnded { stream, .. } => *stream,
        };
        let Some(call) = self.calls.get_mut(&name) else {
            return;
        };
        match message {
            kit::ToTransport::Reply { reply, .. } => {
                let (status, body): (u16, Box<[u8]>) = match reply {
                    kit::Reply::Record(body) => (200, body),
                    kit::Reply::NotARecord => (400, Box::new([])),
                    kit::Reply::Conflict => (409, Box::new([])),
                };
                let (body, cookie) =
                    if status == 200 { dispatch::reply(&self.settings, call, env.wall, body) } else { (body, None) };
                call.respond_with_cookie(status, body, cookie, &env.limits);
            }
            kit::ToTransport::Streamed { message, .. } => call.stream(env, message),
            kit::ToTransport::StreamEnded { why, .. } => call.end(env, why),
        }
    }
    #[must_use]
    pub fn next_deadline(&self) -> Option<Time> {
        None
    }
    pub fn fire(&mut self, _env: &Env<Limits>, _out: &mut Queue<Request>) {}
}
#[must_use]
pub fn claims(settings: &Settings, call: &http::Call) -> bool {
    let Some(path) = call.target.strip_prefix(settings.prefix.as_ref()) else {
        return false;
    };
    path.starts_with(b"/")
        && !(call.method == skein_http::Method::Get
            && (path == b"/v1/sign-in/link" || path.starts_with(b"/v1/sign-in/link?")))
}
#[must_use]
pub fn link_path(settings: &Settings, secret: &Secret) -> Box<[u8]> {
    let encoded = skein_lib::base64::encoded_len(
        skein_lib::base64::Alphabet::Url,
        u32::try_from(secret.expose().len()).expect("bounded secret"),
    )
    .expect("bounded base64");
    let route = b"/v1/sign-in/link?secret=";
    let mut path = skein_lib::Writer::new(
        settings
            .prefix
            .len()
            .checked_add(route.len())
            .expect("path")
            .checked_add(usize::try_from(encoded).expect("u32 fits"))
            .expect("path"),
    );
    path.put(&settings.prefix).expect("prefix");
    path.put(route).expect("route");
    skein_lib::base64::encode(skein_lib::base64::Alphabet::Url, secret.expose(), &mut path).expect("secret");
    path.finish()
}
#[cfg(test)]
mod tests;

/// The application's printed-link page adds this policy (protocol/browser.md, 12.2).
#[must_use]
pub fn link_page() -> skein_http::Header {
    skein_http::Header { name: Box::from(b"referrer-policy".as_slice()), value: Box::from(b"no-referrer".as_slice()) }
}
