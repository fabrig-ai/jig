//! Accounts component and transcript fixture responses for admission stories.
use super::{World, accounts, core, root};
use skein_lib::Duration;

/// The subject and display name every sign-in at the fake accounts' component answers with.
const SIGNED_IN: &[u8] = b"signed-in";
impl World {
    pub(super) fn transcript_loaded(&mut self, now: core::Now) {
        if self.fail_account_before_workspace {
            self.fail_account_before_workspace = false;
            self.events.push_back(root::Event::Core(core::Event::Account(accounts::Event::Component(
                accounts::FromComponent::Failed { account: 1, failure: accounts::Failure::SignInAgain },
            ))));
        }
        if let core::Now::StartPreparation { task, from, .. } = &now {
            if from.is_none() {
                self.events.push_back(root::Event::TranscriptLoaded { task: *task, rows: Box::new([]), done: true });
            } else {
                self.store.ask(super::StoreOut::Now(now));
            }
        } else {
            panic!("preparation route");
        }
    }
}

impl World {
    pub(super) fn account_request(&mut self, request: accounts::ToComponent) {
        let response = match request {
            accounts::ToComponent::Want { account: 1 } if !self.hold_accounts => {
                Some(accounts::FromComponent::Granted {
                    form: accounts::Form::Credential(accounts::Scheme::Bearer),
                    account: 1,
                    generation: 1,
                    valid: Duration::from_secs(60),
                })
            }
            accounts::ToComponent::Rejected { account, generation } => Some(accounts::FromComponent::Granted {
                form: accounts::Form::Credential(accounts::Scheme::Bearer),
                account,
                generation: generation.checked_add(1).expect("fixture generation has a successor"),
                valid: Duration::from_secs(60),
            }),
            accounts::ToComponent::Close => Some(accounts::FromComponent::Closed),
            accounts::ToComponent::Want { .. } | accounts::ToComponent::Release { .. } => None,
        };
        if let Some(response) = response {
            if let accounts::FromComponent::Granted { account, generation, .. } = &response {
                self.account_generations.insert(*account, *generation);
            }
            self.events.push_back(root::Event::Core(core::Event::Account(accounts::Event::Component(response))));
        }
    }

    /// The fake component's account sign-in and sign-out: each
    /// provider's sign-in answers with [`SIGNED_IN`]'s subject, and each revocation is confirmed.
    #[expect(clippy::needless_pass_by_value, reason = "the scripted component consumes its owned request envelope")]
    pub(super) fn account_signing(&mut self, signing: accounts::Signing) {
        let event = match signing {
            accounts::Signing::Withdraw { source, .. } => {
                self.moved
                    .insert(source, skein_lib::Secret::new(format!("moved:{source}").into_bytes().into_boxed_slice()));
                accounts::Event::Withdrawn { source, readable: true }
            }
            accounts::Signing::Return { source } => {
                self.moved.remove(&source);
                // Return restores the driver's record; Refresh, which follows it, grants.
                return;
            }
            accounts::Signing::Refresh { source } => {
                if self.hold_accounts {
                    return;
                }
                return self.grant_move(source);
            }
            accounts::Signing::Open { source, .. } => {
                if std::mem::take(&mut self.fail_move_open) {
                    accounts::Event::NotOurs { source }
                } else {
                    accounts::Event::Opened {
                        source,
                        provider: self.move_provider.expect("a configured move provider"),
                        subject: Box::from(SIGNED_IN),
                        display: Box::from(SIGNED_IN),
                    }
                }
            }
            accounts::Signing::Adopt { source, .. } => accounts::Event::Adopted { source, kept: true },
            accounts::Signing::SignIn { source, .. } => {
                let visit = accounts::Event::Visited {
                    source,
                    address: format!("https://issuer.test/visit/{source}").into_bytes().into_boxed_slice(),
                };
                self.events.push_back(root::Event::Core(core::Event::Account(visit)));
                if self.wait_for_finish {
                    return;
                }
                accounts::Event::SignedIn {
                    source,
                    subject: Box::from(SIGNED_IN),
                    display: Box::from(SIGNED_IN),
                    declined: false,
                }
            }
            accounts::Signing::Redirected { source, carried } => {
                self.finishes.push((source, carried));
                if let Some(why) = self.finish_script.pop_front().expect("a scripted finish") {
                    accounts::Event::FinishRefused { source, why }
                } else {
                    self.events
                        .push_back(root::Event::Core(core::Event::Account(accounts::Event::Finishing { source })));
                    accounts::Event::SignedIn {
                        source,
                        subject: Box::from(SIGNED_IN),
                        display: Box::from(SIGNED_IN),
                        declined: false,
                    }
                }
            }
            accounts::Signing::CancelSignIn { source } => {
                accounts::Event::SignInFailed { source, failure: accounts::Failure::Cancelled }
            }
            accounts::Signing::SignOut { source } => accounts::Event::SignedOut { source, confirmed: true },
            accounts::Signing::Keep { source, .. } => {
                let response = if let Some(why) = self.fail_llm_key_keep.take() {
                    accounts::FromComponent::KeepFailed { account: source, why }
                } else {
                    let generation = self.next_account_generation(source);
                    accounts::FromComponent::Granted {
                        form: accounts::Form::Credential(accounts::Scheme::Key),
                        account: source,
                        generation,
                        valid: accounts::KEY_VALID,
                    }
                };
                accounts::Event::Component(response)
            }
            accounts::Signing::Forget { source } => {
                self.moved.remove(&source);
                self.account_generations.remove(&source);
                return;
            }
            accounts::Signing::Replaced { .. } | accounts::Signing::Restore { .. } | accounts::Signing::Restored => {
                return;
            }
        };
        self.events.push_back(root::Event::Core(core::Event::Account(event)));
    }
}

impl World {
    /// Take a scripted move's bytes once, after its answer has committed.
    pub fn take_moved(&mut self, source: u32) -> Option<skein_lib::Secret> {
        self.moved.remove(&source)
    }

    fn grant_move(&mut self, source: u32) {
        let generation = self.next_account_generation(source);
        self.events.push_back(root::Event::Core(core::Event::Account(accounts::Event::Component(
            accounts::FromComponent::Granted {
                account: source,
                generation,
                form: accounts::Form::Credential(accounts::Scheme::Bearer),
                valid: Duration::from_secs(60),
            },
        ))));
    }

    fn next_account_generation(&mut self, source: u32) -> u64 {
        let current = self.domain.accounts().source(source).and_then(|fact| fact.generation).unwrap_or(0);
        let generation = self
            .account_generations
            .get(&source)
            .copied()
            .unwrap_or(0)
            .max(current)
            .checked_add(1)
            .expect("fixture generation has a successor");
        self.account_generations.insert(source, generation);
        generation
    }
}
