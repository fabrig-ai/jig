//! The client-facing seam of the core world (client/domain/testing.md, 5).
//! Values reach this peer only after the existing store and core referee saw
//! their release. Workers and systems remain the world's independent fakes.
use jig_domain_core as core;
use jig_test_domain as root;
use skein_lib::{ReplyTo, Token};
use std::collections::BTreeSet;

/// Untranslated outputs for a client-backed peer. The protocol adapter owns
/// sign-ins, request attempts and watch tokens, and takes these owned values.
#[derive(Debug, Default)]
pub struct ClientPeer {
    pub delivered: Vec<(u64, root::Delivery)>,
    pub now: Vec<core::Now>,
    replies: Option<BTreeSet<Token>>,
}

impl ClientPeer {
    /// Capture held people replies only at explicitly registered destinations.
    #[must_use]
    pub fn routed() -> Self {
        Self { replies: Some(BTreeSet::new()), ..Self::default() }
    }

    /// Retain a destination actually issued by the client's protocol adapter.
    /// Taking or capturing replies never retires this registration.
    pub fn register_reply(&mut self, token: Token) {
        if let Some(replies) = &mut self.replies {
            replies.insert(token);
        }
    }

    /// Explicitly retire a protocol destination, after its waiters have ended.
    #[must_use]
    pub fn unregister_reply(&mut self, token: Token) -> bool {
        self.replies.as_mut().is_some_and(|replies| replies.remove(&token))
    }

    /// Reset explicit reply ownership without changing the capture mode.
    pub fn clear_replies(&mut self) {
        if let Some(replies) = &mut self.replies {
            replies.clear();
        }
    }

    /// Consume a client delivery or return its complete unowned envelope.
    /// The caller has already observed its durable release exactly once.
    ///
    /// # Errors
    /// Returns the same delivery when its external consumer is another peer.
    #[expect(
        clippy::result_large_err,
        reason = "return the whole owned envelope without allocating or changing its boundary"
    )]
    pub fn capture(&mut self, commit: u64, delivery: root::Delivery) -> Result<(), root::Delivery> {
        let delivery = if let Some(replies) = &self.replies {
            if let root::Delivery::Core(core::Held::PeopleReply { to, reply }) = delivery {
                let token = to.into_token();
                let delivery = root::Delivery::Core(core::Held::PeopleReply { to: ReplyTo::new(token), reply });
                if !replies.contains(&token) {
                    return Err(delivery);
                }
                delivery
            } else {
                delivery
            }
        } else {
            delivery
        };
        if Self::accepts(&delivery) {
            self.delivered.push((commit, delivery));
            Ok(())
        } else {
            Err(delivery)
        }
    }

    /// Whether this held output crosses the boundary to a party's client.
    #[must_use]
    pub fn accepts(delivery: &root::Delivery) -> bool {
        match delivery {
            root::Delivery::ApplicationDocument { .. }
            | root::Delivery::ApplicationRead { .. }
            | root::Delivery::Application { .. } => true,
            root::Delivery::Core(held) => match held {
                core::Held::PeopleReply { .. }
                | core::Held::Read { .. }
                | core::Held::Delivery { .. }
                | core::Held::WatchEnded { .. }
                | core::Held::CarriedUnused { .. } => true,
                core::Held::Acted { .. }
                | core::Held::ApplicationDelivery { .. }
                | core::Held::ApplicationWatchEnded { .. }
                | core::Held::Applied { .. }
                | core::Held::ReadLoad { .. }
                | core::Held::MakeEffect { .. }
                | core::Held::ProjectionRecovery { .. }
                | core::Held::SettledCall { .. }
                | core::Held::Assign { .. }
                | core::Held::Grant { .. }
                | core::Held::Relayed { .. }
                | core::Held::Inbound { .. }
                | core::Held::Relay { .. }
                | core::Held::CallAnswer { .. }
                | core::Held::Result { .. }
                | core::Held::Acknowledge { .. }
                | core::Held::TaskTerminalAcknowledged { .. }
                | core::Held::TaskTurnKept { .. }
                | core::Held::AcknowledgeTurn { .. }
                | core::Held::Drained
                | core::Held::Cancel { .. }
                | core::Held::Sources { .. }
                | core::Held::CheckSource { .. }
                | core::Held::SignOutSource { .. }
                | core::Held::SourceSignedOut { .. }
                | core::Held::Refuse { .. }
                | core::Held::TurnBusy { .. }
                | core::Held::StopRun { .. }
                | core::Held::Load(..)
                | core::Held::NotesLoad { .. }
                | core::Held::NotesWritten { .. }
                | core::Held::NotesDeleted { .. } => false,
            },
            root::Delivery::Projection { .. }
            | root::Delivery::Message { .. }
            | root::Delivery::Restart(_)
            | root::Delivery::Link { .. }
            | root::Delivery::ProjectMade { .. }
            | root::Delivery::CallBusy { .. }
            | root::Delivery::CallAnswer { .. }
            | root::Delivery::Assigned { .. }
            | root::Delivery::Fleet(_)
            | root::Delivery::System { .. }
            | root::Delivery::LostRead { .. }
            | root::Delivery::Procedure { .. } => false,
        }
    }

    /// Whether this journal-door output belongs to the client's protocol.
    #[must_use]
    pub fn immediate(value: &core::Now) -> bool {
        match value {
            core::Now::SignInRefused { .. } | core::Now::WatchRefused { .. } | core::Now::RunText { .. } => true,
            core::Now::Relayed { .. }
            | core::Now::EffectAnswer { .. }
            | core::Now::SettledCallRefused { .. }
            | core::Now::Call { .. }
            | core::Now::RelayBusy { .. }
            | core::Now::CallBusy { .. }
            | core::Now::DropCall { .. }
            | core::Now::Undelivered { .. }
            | core::Now::Account(_)
            | core::Now::NotesIndexed { .. }
            | core::Now::NotesRecalled { .. }
            | core::Now::NotesRefused { .. }
            | core::Now::NoteBusy { .. }
            | core::Now::DropPayload { .. }
            | core::Now::DropAssignment { .. }
            | core::Now::TurnPayload { .. }
            | core::Now::AnswerPayload { .. }
            | core::Now::AcceptedTurn { .. }
            | core::Now::RefusedPayload { .. }
            | core::Now::Activate { .. }
            | core::Now::PrepareAgent { .. }
            | core::Now::StartPreparation { .. }
            | core::Now::SeenConversation { .. }
            | core::Now::LoadEndedRead { .. }
            | core::Now::HistoricalProposal { .. }
            | core::Now::DecidedProposal { .. }
            | core::Now::HistoricalPersonTask { .. }
            | core::Now::EndedTask { .. }
            | core::Now::PartyInputs { .. }
            | core::Now::CompleteBrief { .. }
            | core::Now::BriefCorePlanned { .. }
            | core::Now::WorkspaceRequest { .. }
            | core::Now::RunPreparationFailed { .. }
            | core::Now::RunPrepared { .. }
            | core::Now::HistoricalEscalation { .. }
            | core::Now::RestoreRefused => false,
        }
    }
}
