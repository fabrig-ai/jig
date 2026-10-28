//! Read a held pasted address only where it is handed to the driver
//! (protocol/llm.md, 5.2 and 5.4). The clients component alone erases it.
use jig_domain_accounts as accounts;
use skein_lib::{Map, Secret, Token, bytes};
use skein_oauth_accounts as driver;

#[derive(Debug)]
struct Reading {
    source: u32,
    carried: u64,
}
/// A finish's next step. A driver request's address is never formatted.
#[expect(missing_debug_implementations, reason = "the driver request carries a pasted address")]
pub enum Next {
    Get { exchange: Token, carried: u64 },
    Driver(driver::Request),
    Child(accounts::Event),
}
#[derive(Debug)]
pub struct Finishes {
    reading: Map<Token, Reading>,
    next: u64,
}
impl Finishes {
    #[must_use]
    pub fn new(room: u32) -> Self {
        Self { reading: Map::with_capacity(room), next: 1 }
    }
    pub fn redirected(&mut self, source: u32, carried: u64) -> Next {
        if self.next > u64::from(u32::MAX) {
            return refused(source);
        }
        let exchange = Token::new(self.next);
        self.next = self.next.checked_add(1).expect("bounded read tokens");
        if self.reading.insert(exchange, Reading { source, carried }).is_err() {
            return refused(source);
        }
        Next::Get { exchange, carried }
    }
    pub fn read(&mut self, exchange: Token, value: Option<Secret>) -> Option<Next> {
        let Reading { source, carried: _carried } = self.reading.remove(&exchange)?;
        Some(match value {
            Some(value) => {
                Next::Driver(driver::Request::Redirected { account: source, uri: bytes::copy_of(value.expose()) })
            }
            None => refused(source),
        })
    }
    #[must_use]
    pub fn pending(&self) -> bool {
        !self.reading.is_empty()
    }
    pub fn abort(&mut self) {
        self.reading = Map::with_capacity(self.reading.capacity());
    }
}
fn refused(source: u32) -> Next {
    Next::Child(accounts::Event::FinishRefused { source, why: accounts::SignInRedirectWhy::NotWaiting })
}
