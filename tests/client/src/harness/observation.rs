//! What shells and people can observe, independent of a root's state
//! (client/domain/testing.md, 6). Applications translate their vocabulary here.
use super::ClientId;
use skein_lib::{Time, Token};

/// A card's stable identity in an application's public tree vocabulary.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CardKey(pub Vec<u8>);
/// A card's visible progress or departure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CardSeen {
    /// Still waiting for a person's decision.
    Waiting,
    /// A keyed decision is pending.
    Deciding,
    /// The decision's outcome is shown.
    Done,
    /// A departure is shown, with or without an explanation.
    Left { reason: bool },
}
/// Facts read from a visible accessibility tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Seen {
    /// A keyed draft judged against the offer shown before it left.
    Draft { key: jig_client::Key, offer: jig_client::shapes::Fall },
    /// The changed tree broke the page outline.
    Outline { fault: skein_ui::outline::Fault },
    /// One card; a printed revision overrides its last delivered document's.
    Card { key: CardKey, state: CardSeen, revision: Option<u64> },
    /// Words the person is told were sent.
    Words { task: u64, message: Option<u64>, words: Vec<u8> },
    /// A visible non-work page explains why the whole previous page is hidden.
    HiddenPage,
    /// The sign-in page says that the former sign-in has ended.
    SignedOut,
    /// The visible link status.
    Link { live: bool },
}
/// Metadata of a keyed request at the shell's wire boundary.
#[derive(Clone, Debug)]
pub struct Sent {
    pub attempt: Token,
    pub key: [u8; 16],
    pub card: Option<CardKey>,
    pub revision: Option<(u16, u64)>,
    pub words: Option<(u64, Vec<u8>)>,
}
/// Facts in a delivered shell callback, before the root consumes it.
#[derive(Clone, Debug)]
pub enum Receipt {
    /// A final draft outcome; proposal documents supply its recipient independently.
    DraftAnswer { attempt: Token, outcome: DraftOutcome },
    /// A complete jig document delivered before the application consumes it.
    Document { value: Box<jig_client::contract::ReadValue> },
    /// The document revision available to a card in the next build.
    Card { key: CardKey, revisions: Vec<(u16, u64)> },
    /// A wire attempt ended; only final answers meet the request deadline.
    Answer { attempt: Token, final_answer: bool, words: Option<(u64, u64)> },
    /// The frame confirms the party; a read has no watch identity.
    Party { party: u64, watch: Option<Token> },
    /// No later delivery on this watch can confirm a party.
    WatchEnded { watch: Token },
    /// A newly started client can receive a fresh protocol hello.
    Started,
    /// The stream hello requires updating this client; business Frames cannot lift it.
    OutOfDate,
    /// Requests park until the party can sign in again.
    Parked,
}
/// A draft's committed result, before its proposal document is delivered.
#[derive(Clone, Debug)]
pub enum DraftOutcome {
    Made,
    Proposed(jig_client::contract::ProposalKey),
    ProposedByParty { number: u64, to: u32 },
    Refused,
}
/// Durable truth reported by the engine boundary, drained once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observed {
    /// The engine ended this client's sign-in.
    SignInEnded { client: ClientId },
    /// A completed store commit and its clock.
    Durable { commit: u64, at: Time },
    /// One party's key answered at this durable commit.
    Made { party: u64, key: [u8; 16], commit: u64 },
    /// Words found in committed task activity.
    Kept { task: u64, message: u64, words: Vec<u8>, commit: u64 },
    /// A complete committed activity snapshot; omitted messages are no longer retained.
    Retained { task: u64, messages: Vec<u64>, commit: u64 },
    /// An object stopped waiting independently of this client.
    Stopped { card: CardKey, commit: u64 },
    /// The task ended, so no future word retention is promised.
    Ended { task: u64, commit: u64 },
    /// Truth's link status; compared with what each person was shown.
    Reachable { live: bool },
}
/// One boundary observation made by the loop.
#[derive(Clone, Debug)]
pub enum Observation {
    /// A jig decision leaving the shell, with the tree the person acted on.
    Decision {
        client: ClientId,
        key: jig_client::Key,
        ask: Box<jig_client::contract::Ask>,
        scene: Option<jig_client_test_kit::check::Scene>,
    },
    /// The shell is sending a person's sign-out to its front.
    SignOutAsked { client: ClientId },
    /// Admit a tab/device to link expectations.
    Client { client: ClientId },
    /// A keyed send is leaving a client's shell.
    Request { client: ClientId, party: u64, sent: Sent },
    /// A shell callback has arrived.
    Delivered { client: ClientId, receipt: Receipt },
    /// The complete visible observations after one build.
    Shown { client: ClientId, seen: Vec<Seen> },
    /// Navigation legitimately removes a page's cards.
    LeftPage { client: ClientId },
    /// A tab started again from its kept records: its page left, and its
    /// link is judged afresh from now.
    Reloaded { client: ClientId },
    /// The person's decoded display act changes which still-waiting cards are visible.
    WindowChanged { client: ClientId },
    /// A durable engine fact or reachability change.
    Truth(Observed),
    /// Measured client/view/model heap and its checked envelope.
    Bounded { heap: u64, bound: u64 },
}

impl CardKey {
    /// Copy a public tree key as identity, without interpreting owner tokens.
    #[must_use]
    pub fn of(key: skein_ui::Key) -> Self {
        let mut bytes = Vec::new();
        match jig_client_view::Owner::from_opaque(key.owner) {
            Some(jig_client_view::Owner::Jig) => bytes.extend_from_slice(&[0, 0, 0]),
            Some(jig_client_view::Owner::Application { number: connector }) => {
                bytes.push(1);
                bytes.extend_from_slice(&connector.to_le_bytes());
            }
            None => {
                bytes.push(2);
                bytes.extend_from_slice(&key.owner.number.to_le_bytes());
            }
        }
        bytes.extend_from_slice(&key.kind.to_le_bytes());
        bytes.extend_from_slice(&key.a.to_le_bytes());
        bytes.extend_from_slice(&key.b.to_le_bytes());
        Self(bytes)
    }
}
