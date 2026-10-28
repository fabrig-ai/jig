//! The person's browser, cut down to what a sign-in's visit needs here: one
//! GET of the URL the driver asks to be visited, its answer's status kept.
//! It follows the issuer's loopback redirect and knows no OAuth.
use crate::fixtures::{self, ROOM};
use skein_http::{Header, Method, client as http};
use skein_io::{self as io, kernel};
use skein_lib::stream::{Down, Read, Up};
use skein_lib::{Env, Queue, Time, Token, Wall, bytes};
use skein_world::Host;
use std::net::SocketAddr;

const WEB: Token = Token::new(1);
/// Room for a response body; the issuer's redirect has none.
const BODY: u32 = 256;
/// Room for the URL handed over, the driver's `url_bytes`.
const URL: usize = 1024;

pub struct Visitor {
    io: io::Io,
    http: http::Client,
    events: Queue<io::Event>,
    requests: Queue<io::Request>,
    submissions: Queue<kernel::Submit>,
    completions: Queue<kernel::Complete>,
    answers: Queue<http::Event>,
    calls: Queue<http::Request>,
    below: Queue<Down>,
    socket: Option<Token>,
    connected: bool,
    /// The URL handed over, held in place until the visitor's own turn
    /// copies it, so its heap is the visitor's.
    inbox: [u8; URL],
    inbox_len: usize,
    handed: bool,
    status: Option<u16>,
    account_redirect: Option<Box<[u8]>>,
    stop_at_redirect: bool,
}
impl std::fmt::Debug for Visitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Visitor").field("status", &self.status).finish_non_exhaustive()
    }
}
impl Visitor {
    #[must_use]
    pub fn new() -> Self {
        let limits = fixtures::limits();
        Self {
            io: io::Io::new(&limits.io),
            http: http::Client::new(&limits.http),
            events: Queue::with_capacity(ROOM),
            requests: Queue::with_capacity(ROOM),
            submissions: Queue::with_capacity(ROOM),
            completions: Queue::with_capacity(ROOM),
            answers: Queue::with_capacity(ROOM),
            calls: Queue::with_capacity(ROOM),
            below: Queue::with_capacity(ROOM),
            socket: None,
            connected: false,
            inbox: [0; URL],
            inbox_len: 0,
            handed: false,
            status: None,
            account_redirect: None,
            stop_at_redirect: false,
        }
    }
    /// Hands over `url` to visit, once: a plain `http://` URL with a
    /// literal address.
    pub(crate) fn hand(&mut self, url: &[u8]) {
        assert!(!self.handed, "one visit");
        self.handed = true;
        self.inbox.get_mut(..url.len()).expect("the URL fits the inbox").copy_from_slice(url);
        self.inbox_len = url.len();
    }
    pub fn stop_at_redirect(&mut self) {
        self.stop_at_redirect = true;
    }
    #[must_use]
    pub fn redirect(&self) -> Option<&[u8]> {
        self.account_redirect.as_deref()
    }
    pub(crate) fn handed(&self) -> bool {
        self.handed
    }
    fn visit(&mut self, url: &[u8]) {
        let rest = url.strip_prefix(b"http://").expect("a plain HTTP URL");
        let at = rest.iter().position(|byte| *byte == b'/').expect("a path");
        let (authority, target) = rest.split_at(at);
        let address: SocketAddr = std::str::from_utf8(authority).expect("ascii").parse().expect("a literal address");
        self.calls.push(http::Request::Call(http::Call {
            method: Method::Get,
            target: bytes::copy_of(target),
            headers: Box::new([Header { name: bytes::copy_of(b"host"), value: bytes::copy_of(authority) }]),
            body: http::Body::None,
            close: true,
        }));
        self.requests.push(io::Request::Connect { owner: WEB, addr: io::Addr::Ip(address) });
    }
    /// The visit's answer's status, once its exchange is done.
    #[must_use]
    pub fn status(&self) -> Option<u16> {
        self.status
    }
    fn progress(&mut self, now: Time, wall: Wall) {
        let env = Env { now, wall, limits: fixtures::limits().http };
        let Some(socket) = self.socket.filter(|_| self.connected) else { return };
        while let Some(event) = self.answers.pop() {
            match event {
                http::Event::Response(response) => {
                    if self.status.is_none() {
                        self.status = Some(response.status);
                    }
                    if response.status == 302 {
                        self.account_redirect = response
                            .headers
                            .iter()
                            .find(|header| header.name.eq_ignore_ascii_case(b"location"))
                            .map(|header| header.value.clone());
                        assert!(self.account_redirect.is_some(), "account sign-in issuer redirect");
                    }
                    self.calls.push(http::Request::Body(Down::Demand { read: Read::Arrived { max: BODY }, room: 0 }));
                }
                http::Event::Body(Up::Bytes(_)) => {
                    self.calls.push(http::Request::Body(Down::Demand { read: Read::Arrived { max: BODY }, room: 0 }));
                }
                http::Event::Body(Up::End) | http::Event::Closed => {}
                http::Event::Done(_) => {
                    self.calls.push(http::Request::Close);
                    self.requests.push(io::Request::Close { entity: socket });
                }
                http::Event::Failed(_)
                | http::Event::Upload(Up::Room | Up::Bytes(_) | Up::End | Up::Failed(_))
                | http::Event::Body(Up::Room | Up::Failed(_)) => panic!("the visit failed: {event:?}"),
            }
        }
        while self.answers.room() >= 2
            && self.below.room() >= 2
            && let Some(call) = self.calls.pop()
        {
            http::down(&mut self.http, &env, call, &mut self.answers, &mut self.below);
        }
        while let Some(down) = self.below.pop() {
            self.requests.push(io::Request::Stream { stream: socket, down });
        }
    }
}
impl Default for Visitor {
    fn default() -> Self {
        Self::new()
    }
}
impl Host for Visitor {
    fn iterate(&mut self, now: Time, wall: Wall) {
        let limits = fixtures::limits();
        let env = Env { now, wall, limits: limits.io };
        if self.inbox_len > 0 {
            let url = bytes::copy_of(&self.inbox[..self.inbox_len]);
            self.inbox_len = 0;
            self.visit(&url);
        }
        for _ in 0..ROOM {
            if self.events.room() < 3 || self.submissions.room() < 2 || !self.io.is_ready() {
                break;
            }
            io::resume(&mut self.io, &env, &mut self.events, &mut self.submissions);
        }
        for _ in 0..ROOM {
            if self.events.room() < 3 || self.submissions.room() < 2 {
                break;
            }
            let Some(complete) = self.completions.pop() else { break };
            io::up(&mut self.io, &env, complete, &mut self.events, &mut self.submissions);
        }
        for _ in 0..ROOM {
            if self.events.room() < 3 || self.submissions.room() < 2 || !self.io.is_due(now) {
                break;
            }
            io::fire(&mut self.io, &env, &mut self.events, &mut self.submissions);
        }
        while let Some(event) = self.events.pop() {
            match event {
                io::Event::Connecting { socket, .. } => self.socket = Some(socket),
                io::Event::Connected { .. } => self.connected = true,
                io::Event::Stream { up, .. } => http::up(
                    &mut self.http,
                    &Env { now, wall, limits: limits.http },
                    up,
                    &mut self.answers,
                    &mut self.below,
                ),
                io::Event::Closed { .. } => {
                    self.socket = None;
                    self.connected = false;
                    if !self.stop_at_redirect
                        && let Some(url) = self.account_redirect.take()
                    {
                        self.http = http::Client::new(&limits.http);
                        self.visit(&url);
                    }
                }
                other @ (io::Event::Listening { .. }
                | io::Event::Accepted { .. }
                | io::Event::Output { .. }
                | io::Event::Spawned { .. }
                | io::Event::Exited { .. }
                | io::Event::Usage { .. }
                | io::Event::Resized
                | io::Event::Window { .. }
                | io::Event::Shutdown { .. }
                | io::Event::Failed { .. }) => panic!("visitor event {other:?}"),
            }
        }
        self.progress(now, wall);
        for _ in 0..ROOM {
            if !self.io.takes() || self.submissions.room() < 2 {
                break;
            }
            let Some(request) = self.requests.pop() else { break };
            io::down(&mut self.io, &env, request, &mut self.submissions);
        }
        self.io.reclaim();
    }
    fn completions(&mut self) -> &mut Queue<kernel::Complete> {
        &mut self.completions
    }
    fn submissions(&mut self) -> &mut Queue<kernel::Submit> {
        &mut self.submissions
    }
    fn work_pending(&self, _: Time) -> bool {
        self.io.is_ready()
            || self.inbox_len > 0
            || !self.events.is_empty()
            || !self.requests.is_empty()
            || !self.completions.is_empty()
            || (self.connected && (!self.answers.is_empty() || !self.calls.is_empty() || !self.below.is_empty()))
    }
    fn next_deadline(&self) -> Option<Time> {
        self.io.next_deadline()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        None
    }
    fn is_empty(&self) -> bool {
        self.io.is_empty()
            && self.inbox_len == 0
            && self.events.is_empty()
            && self.requests.is_empty()
            && self.submissions.is_empty()
            && self.completions.is_empty()
            && self.answers.is_empty()
            && self.calls.is_empty()
            && self.below.is_empty()
            && self.socket.is_none()
    }
    fn operations(&self) -> u32 {
        io::operations(&fixtures::limits().io).expect("io operation bound")
    }
}
