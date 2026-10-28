//! LLM keys move from carried values to Source rows before they are granted
//! (domain/accounts.md, 3.5 and 5; protocol/store.md, 9).
use crate::table;
use jig_domain_accounts as accounts;
use skein_lib::{Map, Secret, Time, Token, sha256};

#[derive(Debug)]
struct Source {
    digest: Option<[u8; 32]>,
    generation: u64,
    readable: bool,
    existing: bool,
}
#[derive(Debug)]
enum Waiting {
    Reading { source: u32, carried: u64, replaces: Option<[u8; 32]>, generation: u64 },
    Adopting { source: u32, value: Secret, generation: u64 },
}
/// A secrets exchange's next step; values remain in this protocol layer.
#[derive(Debug)]
pub enum Store {
    Get { carried: u64 },
    Adopt { carried: u64, source: u32, replaces: Option<[u8; 32]> },
}
/// The next step of an LLM key's secrets exchange.
#[derive(Debug)]
pub enum Next {
    Store { exchange: Token, request: Store },
    Child(accounts::FromComponent),
    Granted { event: accounts::FromComponent, source: u32, generation: u64, value: table::Value },
}
#[derive(Debug)]
pub struct Keys {
    sources: Map<u32, Source>,
    waiting: Map<Token, Waiting>,
    next: u64,
    token_bytes: u32,
}
impl Keys {
    #[must_use]
    pub fn new(accounts: u32, token_bytes: u32) -> Self {
        Self { sources: Map::with_capacity(accounts), waiting: Map::with_capacity(accounts), next: 1, token_bytes }
    }
    /// Read first, so an oversized LLM key stays carried for the unused erase.
    pub fn keep(&mut self, source: u32, carried: u64, replaces: bool) -> Next {
        if self.next > u64::from(u32::MAX) {
            return failed(source);
        }
        let held = self.sources.get(&source);
        let (digest, generation) = match held {
            Some(held) if replaces => {
                let Some(generation) = held.generation.checked_add(1) else { return failed(source) };
                (held.digest, generation)
            }
            Some(_) => return failed(source),
            None if replaces => return failed(source),
            None => (None, 1),
        };
        if !self.sources.contains_key(&source)
            && self
                .sources
                .insert(source, Source { digest: None, generation: 0, readable: false, existing: false })
                .is_err()
        {
            return failed(source);
        }
        let exchange = Token::new(self.next);
        let Some(next) = self.next.checked_add(1) else { return failed(source) };
        self.next = next;
        let pending = Waiting::Reading { source, carried, replaces: digest, generation };
        if self.waiting.insert(exchange, pending).is_err() {
            self.discard_unkept(source);
            return failed(source);
        }
        Next::Store { exchange, request: Store::Get { carried } }
    }
    pub fn read(&mut self, exchange: Token, value: Option<Secret>) -> Option<Next> {
        let pending = self.waiting.remove(&exchange)?;
        let (source, carried, replaces, generation) = match pending {
            Waiting::Reading { source, carried, replaces, generation } => (source, carried, replaces, generation),
            other @ Waiting::Adopting { .. } => {
                self.waiting.insert(exchange, other).expect("removed exchange's place");
                return None;
            }
        };
        let Some(value) = value else {
            self.discard_unkept(source);
            return Some(failed(source));
        };
        if value.len() > usize::try_from(self.token_bytes).expect("token bound fits usize") {
            self.discard_unkept(source);
            return Some(Next::Child(accounts::FromComponent::KeepFailed {
                account: source,
                why: accounts::KeyFailure::TooLong,
            }));
        }
        self.waiting
            .insert(exchange, Waiting::Adopting { source, value, generation })
            .expect("removed exchange's place");
        Some(Next::Store { exchange, request: Store::Adopt { carried, source, replaces } })
    }
    pub fn kept(&mut self, exchange: Token, kept: bool, now: Time) -> Option<Next> {
        let pending = self.waiting.remove(&exchange)?;
        let (source, value, generation) = match pending {
            Waiting::Adopting { source, value, generation } => (source, value, generation),
            Waiting::Reading { source, .. } => {
                self.discard_unkept(source);
                return Some(failed(source));
            }
        };
        if !kept {
            self.discard_unkept(source);
            return Some(failed(source));
        }
        let held = self.sources.get_mut(&source).expect("admitted LLM key source");
        held.digest = Some(sha256::digest(value.expose()));
        held.generation = generation;
        held.readable = true;
        held.existing = true;
        Some(granted(source, generation, value, now))
    }
    fn discard_unkept(&mut self, source: u32) {
        if let Some(held) = self.sources.get(&source)
            && !held.existing
        {
            let _removed = self.sources.remove(&source);
        }
    }
    #[must_use]
    pub fn contains(&self, source: u32) -> bool {
        self.sources.contains_key(&source)
    }
    #[must_use]
    pub fn want(&self, source: u32, now: Time) -> Option<accounts::FromComponent> {
        let held = self.sources.get(&source)?;
        if !held.readable {
            return Some(accounts::FromComponent::Failed {
                account: source,
                failure: accounts::Failure::Unloaded { at: accounts::Place::File, why: accounts::Unread::Unreadable },
            });
        }
        Some(event(source, held.generation, now))
    }
    /// Loads a named LLM key at generation one; only Want announces the grant.
    #[expect(clippy::manual_map, reason = "step code does not use closures")]
    pub fn restore(
        &mut self,
        source: u32,
        value: Option<Secret>,
        table: &mut table::Table,
    ) -> Result<(), table::Unfit> {
        let digest = match &value {
            Some(value) => Some(sha256::digest(value.expose())),
            None => None,
        };
        let readable = match &value {
            Some(value) => value.len() <= usize::try_from(self.token_bytes).expect("token bound"),
            None => false,
        };
        self.sources
            .insert(source, Source { digest, generation: u64::from(readable), readable, existing: true })
            .or(Err(table::Unfit))?;
        if readable {
            let token = value.expect("readable LLM key value");
            table.granted(source, 1, table::Value { token, scheme: table::Scheme::Key }, Time::from_nanos(u64::MAX))?;
        }
        Ok(())
    }
    pub fn forget(&mut self, source: u32) {
        let _removed = self.sources.remove(&source);
    }
    #[must_use]
    pub fn pending(&self) -> bool {
        !self.waiting.is_empty()
    }
}
fn failed(source: u32) -> Next {
    Next::Child(accounts::FromComponent::KeepFailed { account: source, why: accounts::KeyFailure::NotStored })
}
fn event(source: u32, generation: u64, now: Time) -> accounts::FromComponent {
    accounts::FromComponent::Granted {
        account: source,
        generation,
        form: accounts::Form::Credential(accounts::Scheme::Key),
        valid: Time::from_nanos(u64::MAX).saturating_since(now),
    }
}
fn granted(source: u32, generation: u64, value: Secret, now: Time) -> Next {
    Next::Granted {
        event: event(source, generation, now),
        source,
        generation,
        value: table::Value { token: value, scheme: table::Scheme::Key },
    }
}
