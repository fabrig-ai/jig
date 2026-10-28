//! The checks a sign-in and a new source make (accounts.md, 3.3 and 3.6), as functions of the
//! child's state alone, shared by sign-ins and LLM-key keeps.
use crate::records::within;
use crate::{Domain, Holder, Kind, Limits, SUBJECT};

/// What a sign-in's subject does with its provider's accounts (accounts.md, 3.3).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SignIn {
    /// A subject no account of the provider holds: a new account, with its signed-in source.
    New,
    /// A subject its holder already holds: that account's record is renewed.
    Renew { account: u32 },
}

/// Why a sign-in or a new source is refused, naming what refuses it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Refusal {
    /// A finish refused while the sign-in may still wait (accounts.md, section 5).
    SignInRedirect {
        why: SignInRedirectWhy,
    },
    /// No signed-in source holding a record that reads (accounts.md, section 6).
    NotSignedIn,
    /// The carried file does not belong to this application.
    NotOurs,
    /// Written named no answered move of this holder's still under way.
    Cancelled,
    /// This source is withdrawn for a move.
    Moving,
    NotLimited,
    Uses,
    NotKept {
        why: crate::KeyFailure,
    },
    /// A bound of the deployment's (accounts.md, 3.6) would be passed.
    Limit {
        bound: Bound,
        number: Option<NumericLimit>,
    },
    /// Another holder holds this provider's account of the subject: the provider's rule that a
    /// subscription serves one person.
    Held {
        provider: u32,
    },
    /// No configured provider of this number offers this source to this holder; a plan's
    /// sign-in belongs to a person (accounts.md, 3.2).
    Provider {
        provider: u32,
    },
    /// A subject empty, or longer than `SUBJECT`.
    Subject,
    /// No account or source of this number, or none of the holder's.
    Unknown,
    /// The counter has no numbers left for it.
    Numbers,
    /// A source of a kind this request does not serve: a sign-out serves a signed-in source.
    Kind {
        kind: Kind,
    },
    /// The child takes no request before its start, nor once closing.
    Closing,
    /// The account sign-in ended before its visit.
    NotStarted,
}

/// The bounds of accounts.md, 3.6 that the child keeps.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Bound {
    Accounts,
    HolderAccounts,
    Sources,
    SignIns,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LimitKey {
    Accounts,
    HolderAccounts,
    Sources,
    SignIns,
    ProgramNames,
    ProgramHosts,
    Credentials,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NumericLimit {
    pub key: Option<LimitKey>,
    pub value: u32,
}
impl Refusal {
    #[must_use]
    pub const fn limited(bound: Bound, key: LimitKey, value: u32) -> Self {
        Self::Limit { bound, number: Some(NumericLimit { key: Some(key), value }) }
    }
    #[must_use]
    pub const fn fixed(bound: Bound, value: u32) -> Self {
        Self::Limit { bound, number: Some(NumericLimit { key: None, value }) }
    }
}

impl Domain {
    /// A sign-in's subject, for a holder at a provider: a new subject makes an account, the
    /// holder's own renews it, and another holder's is refused (accounts.md, 3.3).
    ///
    /// # Errors
    ///
    /// The refusal names the provider's rule, the provider, the subject or the bound passed.
    pub fn sign_in(&self, limits: &Limits, holder: Holder, provider: u32, subject: &[u8]) -> Result<SignIn, Refusal> {
        if subject.is_empty() || !within(subject, SUBJECT) {
            return Err(Refusal::Subject);
        }
        match self.providers.get(provider) {
            Some(offered) if offered.offers.sign_in => {}
            Some(_) | None => return Err(Refusal::Provider { provider }),
        }
        for (number, account) in &self.accounts {
            if account.provider != provider || *account.subject != *subject {
                continue;
            }
            if account.holder != holder {
                return Err(Refusal::Held { provider });
            }
            for (_, source) in &account.sources {
                if self.sources.get(source).expect("an account's sources are kept").kind == Kind::SignIn {
                    if self.moves.contains_key(source) {
                        return Err(Refusal::Moving);
                    }
                    return Ok(SignIn::Renew { account: *number });
                }
            }
            self.room_for_source(limits, *number)?;
            return Ok(SignIn::Renew { account: *number });
        }
        self.room_for_account(limits, holder)?;
        Ok(SignIn::New)
    }
    /// Room for a holder's new account and its first source.
    ///
    /// # Errors
    ///
    /// The deployment's accounts, the holder's, or the counter's numbers.
    pub fn room_for_account(&self, limits: &Limits, holder: Holder) -> Result<(), Refusal> {
        let mut making: u32 = 0;
        let mut holders: u32 = 0;
        for (_, keeping) in &self.keeping {
            if keeping.join.is_none() {
                making = making.saturating_add(1);
                if keeping.holder == holder {
                    holders = holders.saturating_add(1);
                }
            }
        }
        if self.accounts.len().saturating_add(making) >= limits.accounts.min(self.accounts.capacity()) {
            return Err(Refusal::limited(Bound::Accounts, LimitKey::Accounts, limits.accounts));
        }
        let mut held: u32 = 0;
        for (_, account) in &self.accounts {
            if account.holder == holder {
                held = held.saturating_add(1);
            }
        }
        if held.saturating_add(holders) >= limits.holder_accounts {
            return Err(Refusal::limited(Bound::HolderAccounts, LimitKey::HolderAccounts, limits.holder_accounts));
        }
        self.room_for_live_source()?;
        if self.numbers_left() < making.saturating_add(2) {
            return Err(Refusal::Numbers);
        }
        Ok(())
    }
    fn room_for_live_source(&self) -> Result<(), Refusal> {
        let mut keeping: u32 = 0;
        for (_, entry) in &self.keeping {
            if !entry.replacing {
                keeping = keeping.saturating_add(1);
            }
        }
        if self.sources.len().saturating_add(keeping) >= self.sources.capacity() {
            return Err(Refusal::fixed(Bound::Sources, self.sources.capacity()));
        }
        Ok(())
    }
    /// Room for one more source in an account.
    ///
    /// # Errors
    ///
    /// No such account, its sources' bound, or the counter's numbers.
    pub fn room_for_source(&self, limits: &Limits, account: u32) -> Result<(), Refusal> {
        let Some(entry) = self.accounts.get(&account) else { return Err(Refusal::Unknown) };
        let mut joining: u32 = 0;
        let mut making: u32 = 0;
        for (_, keeping) in &self.keeping {
            if !keeping.replacing && keeping.join == Some(account) {
                joining = joining.saturating_add(1);
            }
            if keeping.join.is_none() {
                making = making.saturating_add(1);
            }
        }
        if entry.sources.len().saturating_add(joining) >= limits.sources.min(entry.sources.capacity()) {
            return Err(Refusal::limited(Bound::Sources, LimitKey::Sources, limits.sources));
        }
        self.room_for_live_source()?;
        if self.numbers_left() <= making {
            return Err(Refusal::Numbers);
        }
        Ok(())
    }
}

/// Room for one more sign-in beside those under way.
///
/// # Errors
///
/// The sign-ins' bound.
pub const fn room_for_sign_in(limits: &Limits, under_way: u32) -> Result<(), Refusal> {
    if under_way >= limits.sign_ins {
        return Err(Refusal::limited(Bound::SignIns, LimitKey::SignIns, limits.sign_ins));
    }
    Ok(())
}

/// Why a finish was refused (client/domain/contract.md, section 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SignInRedirectWhy {
    /// No sign-in of this party's waits for its redirect under that number.
    NotWaiting,
    /// The address does not match the sign-in's redirect or its required fields.
    Address,
    /// The state belongs to another sign-in; this sign-in still waits.
    State,
    /// Too many states from other sign-ins; it still waits for its browser.
    TooMany,
}
