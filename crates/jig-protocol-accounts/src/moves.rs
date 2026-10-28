//! Moved records stay here until taken or adopted (protocol/llm.md, 5.6;
//! protocol/store.md, 9). Owners route the steps without inspecting values.
use crate::moved;
use alloc::boxed::Box;
use jig_domain_accounts as child;
use skein_lib::{Map, Secret, Wall, sha256};
use skein_oauth as oauth;
use skein_oauth_accounts as driver;

#[derive(Debug)]
enum Out {
    Removing { display: Box<[u8]> },
    Reading { display: Box<[u8]> },
    Held { value: Option<Secret> },
}
enum In {
    Reading { carried: u64 },
    Opened { carried: u64, holds: [u8; 32], provider: u32, record: oauth::SavedToken },
    Adopting { provider: u32, value: Secret },
}
/// Store requests are distinguished by their operation and source number.
#[derive(Debug)]
pub enum Store {
    ReadOut,
    ReadBack,
    ReadIn { carried: u64 },
    AdoptValue { carried: u64, holds: [u8; 32], value: Secret },
}
#[expect(missing_debug_implementations, reason = "driver requests may carry records")]
pub enum Next {
    Driver(driver::Request),
    Store { source: u32, request: Store },
    Child(child::Event),
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Forget {
    Pending,
    Withdrawn,
    Source,
}
#[derive(Debug)]
pub struct Adoption {
    pub provider: u32,
    pub value: Secret,
}
#[expect(missing_debug_implementations, reason = "staged driver accounts carry records")]
pub struct Moves {
    application: Box<[u8]>,
    outgoing: Map<u32, Out>,
    incoming: Map<u32, In>,
    /// Restored sources are added at their first use. Refresh precedes Want
    /// for a source held moving at restart, so its driver copy is marked then.
    restored: Map<u32, driver::Account>,
}
impl Moves {
    #[must_use]
    pub fn new(application: Box<[u8]>, room: u32) -> Self {
        assert!(
            !application.is_empty() && application.len() <= usize::try_from(moved::APPLICATION).expect("bound"),
            "the root supplies a nonempty application name within the moved-file bound"
        );
        Self {
            application,
            outgoing: Map::with_capacity(room),
            incoming: Map::with_capacity(room),
            restored: Map::with_capacity(room),
        }
    }
    pub fn stage(&mut self, source: u32, account: driver::Account) {
        assert!(self.restored.insert(source, account).is_ok(), "restored sources fit the driver bound");
    }
    pub fn ready(&mut self, source: u32, refresh: bool) -> Option<driver::Account> {
        let Some(mut account) = self.restored.remove(&source) else {
            assert!(!refresh, "a move's Refresh precedes its first driver add");
            return None;
        };
        match &mut account {
            driver::Account::SignIn { keeper: driver::Keeper::Owner { kept: Some(record) }, .. } if refresh => {
                // llm.md 5.6: a copy left the owner, so disown its access token.
                // Mark only the driver's copy: Grant refreshes before lending,
                // without the stored record's earliest-refresh floor deferring it.
                record.expires_at = Wall::from_nanos(0);
                record.earliest_refresh_at = None;
            }
            driver::Account::SignIn { .. } | driver::Account::HandedIn => (),
        }
        Some(account)
    }
    pub fn withdraw(&mut self, source: u32, display: Box<[u8]>) -> Next {
        self.outgoing.insert(source, Out::Removing { display }).expect("moves fit the child's sources bound");
        Next::Driver(driver::Request::Remove { account: source })
    }
    pub fn removed(&mut self, source: u32) -> Option<Next> {
        let state = self.outgoing.get_mut(&source)?;
        match state {
            Out::Removing { display } => {
                let display = core::mem::replace(display, Box::new([]));
                *state = Out::Reading { display };
                Some(Next::Store { source, request: Store::ReadOut })
            }
            Out::Reading { .. } | Out::Held { .. } => None,
        }
    }
    pub fn read(
        &mut self,
        source: u32,
        value: Option<Secret>,
        registration: &driver::Account,
        limits: &oauth::Limits,
    ) -> Next {
        let display = match self.outgoing.remove(&source) {
            Some(Out::Reading { display }) => display,
            Some(Out::Removing { .. } | Out::Held { .. }) | None => return withdrawn(source, false),
        };
        let value = self.file(source, value, display, registration, limits);
        let readable = value.is_some();
        self.outgoing.insert(source, Out::Held { value }).expect("removed move's place");
        withdrawn(source, readable)
    }
    fn file(
        &self,
        source: u32,
        value: Option<Secret>,
        display: Box<[u8]>,
        account: &driver::Account,
        limits: &oauth::Limits,
    ) -> Option<Secret> {
        let value = value?;
        let record = oauth::decode_record(value.expose(), limits).ok()?;
        if record.key != source || record.refresh_token.is_none() {
            return None;
        }
        let registration = match account {
            driver::Account::SignIn { registration, .. } => registration,
            driver::Account::HandedIn => return None,
        };
        let issuer = registration.terms.as_ref()?.openid.as_ref()?.issuer.clone();
        let client_id = record.issued_client_id.as_ref().unwrap_or(&registration.client_id).clone();
        let subject = record.subject.as_ref()?.clone();
        moved::encode(
            &moved::Moved { application: self.application.clone(), issuer, client_id, subject, display, record },
            limits,
        )
        .ok()
    }
    pub fn take(&mut self, source: u32) -> Option<Secret> {
        match self.outgoing.get_mut(&source)? {
            Out::Held { value } => value.take(),
            Out::Removing { .. } | Out::Reading { .. } => None,
        }
    }
    pub fn drop_out(&mut self, source: u32) {
        let _dropped = self.outgoing.remove(&source);
    }
    pub fn returning(&mut self, source: u32) -> Next {
        self.drop_out(source);
        Next::Store { source, request: Store::ReadBack }
    }
    pub fn open(&mut self, source: u32, carried: u64) -> Next {
        if self.incoming.insert(source, In::Reading { carried }).is_err() {
            return not_ours(source);
        }
        Next::Store { source, request: Store::ReadIn { carried } }
    }
    pub fn opened(
        &mut self,
        source: u32,
        value: Option<Secret>,
        providers: &Map<u32, driver::Account>,
        limits: &oauth::Limits,
    ) -> Next {
        let carried = match self.incoming.remove(&source) {
            Some(In::Reading { carried }) => carried,
            Some(In::Opened { .. } | In::Adopting { .. }) | None => return not_ours(source),
        };
        let Some(value) = value else { return not_ours(source) };
        let Ok(file) = moved::decode(value.expose(), limits) else { return not_ours(source) };
        if file.application != self.application
            || file.record.subject.as_deref() != Some(file.subject.as_ref())
            || file.record.refresh_token.is_none()
        {
            return not_ours(source);
        }
        for (&provider, account) in providers {
            let registration = match account {
                driver::Account::SignIn { registration, .. } => registration,
                driver::Account::HandedIn => continue,
            };
            let Some(terms) = &registration.terms else { continue };
            let Some(openid) = &terms.openid else { continue };
            let client_id = file.record.issued_client_id.as_ref().unwrap_or(&registration.client_id);
            if file.issuer == openid.issuer && file.client_id == *client_id {
                let holds = sha256::digest(value.expose());
                assert!(
                    self.incoming.insert(source, In::Opened { carried, holds, provider, record: file.record }).is_ok(),
                    "removed arrival's place"
                );
                return Next::Child(child::Event::Opened {
                    source,
                    provider,
                    subject: file.subject,
                    display: file.display,
                });
            }
        }
        not_ours(source)
    }
    pub fn adopt(&mut self, source: u32, carried: u64, limits: &oauth::Limits) -> Next {
        let Some(In::Opened { carried: original, holds, provider, mut record }) = self.incoming.remove(&source) else {
            return adopted(source, false);
        };
        if original != carried {
            return adopted(source, false);
        }
        record.key = source;
        let Ok(value) = oauth::encode_record(&record, limits) else { return adopted(source, false) };
        let value = Secret::new(value);
        assert!(
            self.incoming.insert(source, In::Adopting { provider, value: value.clone() }).is_ok(),
            "removed arrival's place"
        );
        Next::Store { source, request: Store::AdoptValue { carried, holds, value } }
    }
    pub fn adopted(&mut self, source: u32, kept: bool) -> Option<Adoption> {
        match self.incoming.remove(&source)? {
            In::Adopting { provider, value } if kept => Some(Adoption { provider, value }),
            In::Adopting { .. } | In::Reading { .. } | In::Opened { .. } => None,
        }
    }
    pub fn forget(&mut self, source: u32) -> Forget {
        let _restored = self.restored.remove(&source);
        if self.incoming.remove(&source).is_some() {
            return Forget::Pending;
        }
        if self.outgoing.contains_key(&source) {
            self.drop_out(source);
            return Forget::Withdrawn;
        }
        Forget::Source
    }
    pub fn close(&mut self) {
        self.outgoing = Map::with_capacity(self.outgoing.capacity());
        self.incoming = Map::with_capacity(self.incoming.capacity());
        self.restored = Map::with_capacity(self.restored.capacity());
    }
}
fn withdrawn(source: u32, readable: bool) -> Next {
    Next::Child(child::Event::Withdrawn { source, readable })
}
fn not_ours(source: u32) -> Next {
    Next::Child(child::Event::NotOurs { source })
}
fn adopted(source: u32, kept: bool) -> Next {
    Next::Child(child::Event::Adopted { source, kept })
}
