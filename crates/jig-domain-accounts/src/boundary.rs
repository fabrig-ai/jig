use crate::{Holder, Key, Kind, Refusal, SignInRedirectWhy, Stored, Use};
use alloc::boxed::Box;
use skein_lib::{Duration, Time, Token, Wall};

/// Maximum address bytes in an account sign-in answer.
pub const ADDRESS: u32 = 2_048;

/// The form in which a grant lends its value (protocol/channel.md, section 8).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Form {
    /// A credential with the scheme its provider takes.
    Credential(Scheme),
    /// An application's credential, held in its own table.
    Application,
    /// A program source lent by its configuration-directory name.
    Program(crate::SourceName),
}

/// A credential's scheme (protocol/channel.md, section 8).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Scheme {
    /// A bearer credential.
    Bearer,
    /// A key credential.
    Key,
}

impl Form {
    /// The protocol table that holds this form's value.
    #[must_use]
    pub const fn table(self) -> GrantTable {
        match self {
            Self::Credential(_) => GrantTable::Accounts,
            Self::Application => GrantTable::Application,
            Self::Program(_) => GrantTable::Name,
        }
    }
}

/// The protocol layer's table that holds a grant (protocol/channel.md, section 8).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum GrantTable {
    Accounts,
    Application,
    /// The value is the form's name; no table entry is held.
    Name,
}

/// A name and the time its token has left; never the token itself. An account's grant is
/// named by its source's number.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Grant {
    pub form: Form,
    pub account: u32,
    pub generation: u64,
    pub valid: Duration,
}

/// Accounts on the component's terms (protocol/llm.md, 5.2), and what runs' agents
/// notice (domain/accounts.md, 3.5): a generation unauthorized; a source limited until
/// its reset, or, with none, until its holder resumes it; a source refused, with the
/// provider's code and reason; a run failed because its source turned limited or
/// refused. The component and a notice name a source by its number, the grant's, in
/// the field named `account`; the child finds its account. The store's records are
/// restored before `Want`, which starts the child.
///
/// A party's sign-in at an LLM provider and sign-out of a signed-in source (accounts.md,
/// section 5) come for the holder people admitted them for (3.2), with the request's token,
/// which the child echoes in its answer and in a sign-in's end. The component's sign-in,
/// failure and sign-out name the source by the number the child gave it: `SignedIn` carries the
/// provider's subject and display name, `declined` when plan usage was declined (providers.md,
/// 2.2); `SignedOut` says the record is revoked and forgotten, `confirmed` when the provider
/// confirmed the revocation (2.3).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Event {
    Restore {
        record: Stored,
    },
    Want,
    ProgramTargets {
        source: u32,
        hosts: Box<[crate::Worker]>,
    },
    ProgramSignedOut {
        source: u32,
        host: crate::Worker,
        result: ProgramResult,
    },
    Component(FromComponent),
    Unauthorized {
        account: u32,
        generation: u64,
    },
    Limited {
        account: u32,
        reset: Option<Time>,
    },
    Refused {
        account: u32,
        code: Box<[u8]>,
        reason: Box<[u8]>,
    },
    RunFailed {
        account: u32,
        turned: Turned,
    },
    Change {
        request: Token,
        holder: Holder,
        change: Change,
    },
    Credential {
        number: u32,
        generation: u64,
        valid: Duration,
    },
    SignIn {
        request: Token,
        holder: Holder,
        by: u64,
        provider: u32,
    },
    SignOut {
        request: Token,
        holder: Holder,
        source: u32,
    },
    /// The account sign-in listener is ready; address is at most `ADDRESS` bytes.
    Visited {
        source: u32,
        address: Box<[u8]>,
    },
    /// The driver took the pasted address (protocol/llm.md, 5.2).
    Finishing {
        source: u32,
    },
    /// The address was refused; the sign-in still waits.
    FinishRefused {
        source: u32,
        why: SignInRedirectWhy,
    },
    SignedIn {
        source: u32,
        subject: Box<[u8]>,
        display: Box<[u8]>,
        declined: bool,
    },
    SignInFailed {
        source: u32,
        failure: Failure,
    },
    SignedOut {
        source: u32,
        confirmed: bool,
    },
    /// The withdrawn source's latest record was read into the move's file.
    Withdrawn {
        source: u32,
        readable: bool,
    },
    /// The carried file belongs to this application; its parts obey SUBJECT and DISPLAY.
    Opened {
        source: u32,
        provider: u32,
        subject: Box<[u8]>,
        display: Box<[u8]>,
    },
    /// The carried file does not read or belongs to another application.
    NotOurs {
        source: u32,
    },
    /// The opened record was adopted and its carried value erased in one secrets' commit.
    Adopted {
        source: u32,
        kept: bool,
    },
    Close,
}

/// What a failed run says its account turned.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Turned {
    Limited { reset: Option<Time> },
    Refused,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FromComponent {
    KeepFailed { account: u32, why: KeyFailure },
    Granted { account: u32, generation: u64, valid: Duration, form: Form },
    Expiring { account: u32, generation: u64 },
    Failed { account: u32, failure: Failure },
    Closed,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Failure {
    Later {
        after: Option<Duration>,
    },
    SignInAgain,
    /// An LLM account sign-in omitted this configured must-grant scope.
    MissingScope {
        scope: u32,
    },
    Misconfigured,
    Unloaded {
        at: Place,
        why: Unread,
    },
    Cancelled,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Place {
    Directory,
    File,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Unread {
    Unreadable,
    Refused,
    Failed,
    Stalled,
    TooLarge,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ToComponent {
    Want { account: u32 },
    Rejected { account: u32, generation: u64 },
    Release { account: u32 },
    Close,
}

/// What the child asks of the component for a party's sign-in and sign-out (protocol/llm.md,
/// 5.2), each naming a source by the number the child gives it.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Signing {
    /// Withdraw and read the latest record for a move (accounts.md, section 6).
    Withdraw {
        source: u32,
        display: Box<[u8]>,
    },
    /// Return a cancelled move's source to the driver with its kept record.
    Return {
        source: u32,
    },
    /// Refresh once before lending any generation of a returned or adopted record.
    Refresh {
        source: u32,
    },
    /// Read the carried move file for this new source number.
    Open {
        source: u32,
        carried: u64,
    },
    /// Adopt the opened record into the source row and erase the carried value.
    Adopt {
        source: u32,
        carried: u64,
    },
    Keep {
        source: u32,
        carried: u64,
        replaces: bool,
    },
    Forget {
        source: u32,
    },
    Restore {
        source: u32,
        provider: u32,
        kind: Kind,
    },
    Restored,
    /// Start the provider's sign-in; its visit carries the address the party goes to (JP6).
    SignIn {
        source: u32,
        provider: u32,
    },
    /// Hand the sign-in the address its person pasted, named by its held number (protocol/llm.md, 5.2).
    Redirected {
        source: u32,
        carried: u64,
    },
    /// Cancel a sign-in under way, including at close (protocol/llm.md, 5.2).
    CancelSignIn {
        source: u32,
    },
    /// Revoke the source's record at its provider and forget it: a party's sign-out, or a
    /// sign-in refused (accounts.md, 3.3).
    SignOut {
        source: u32,
    },
    /// A renewal replaced the source by another: forget its record, revoking its refresh token
    /// where the provider issued the renewal another client id (accounts.md, 3.3).
    Replaced {
        source: u32,
        by: u32,
    },
}

/// The child's answer to a holder's sign-in, sign-out or account change.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Answer {
    /// The sign-in took the address; its end is an accounts-watch fact.
    Finishing {
        source: u32,
    },
    /// Withdrawn and read; the protocol layer hands its bytes to the asking client beside
    /// this answer. Written names the move's own number (accounts.md, section 6).
    MovingOut {
        account: u32,
        number: u32,
        source: u32,
    },
    /// The adopted record is durable; renewed when the holder already held its subject.
    MovedIn {
        account: u32,
        renewed: bool,
    },
    ProgramDeclared {
        source: u32,
        name: Box<[u8]>,
    },
    KeyKept {
        source: u32,
    },
    Done,
    Removed {
        confirmed: bool,
        failed_hosts: Box<[ProgramHost]>,
    },
    /// The sign-in started under this source's number; its end is a fact (`Ended`).
    Started {
        source: u32,
        address: Box<[u8]>,
    },
    /// A signed-in source is removed; a program keeps its declaration. Unconfirmed, its holder disconnects the
    /// application in the provider's settings (providers.md, 2.3); failed program hosts are named.
    SignedOut {
        confirmed: bool,
        failed_hosts: Box<[ProgramHost]>,
    },
    Refused {
        refusal: Refusal,
    },
}

/// How a party's sign-in ended (accounts.md, 3.3), a fact for the accounts' watch.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Ended {
    pub request: Token,
    pub source: u32,
    pub by: u64,
    pub end: End,
}

/// A sign-in under way, including the person who asked for it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Pending {
    pub source: u32,
    pub provider: u32,
    pub holder: Holder,
    pub by: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum End {
    /// A new account, with the signed-in source; declined, the source is refused (section 5).
    Made {
        account: u32,
        declined: bool,
    },
    /// The holder's account of the subject, its signed-in source replaced by this one.
    Renewed {
        account: u32,
        declined: bool,
    },
    /// Refused, and its new record revoked.
    Refused {
        refusal: Refusal,
    },
    Failed {
        failure: Failure,
    },
}

/// What the child asks of its parent. A save or an erase is part of the decision that makes
/// the change (engine.md, 5.6): each account and source the child makes, removes or whose hold
/// changes, and the counter whenever it moves.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Request {
    /// All issued names, including retired cleanup names, to check after every reconnect.
    Programs {
        names: Box<[Program]>,
    },
    CheckProgram {
        name: crate::SourceName,
    },
    SignOutProgram {
        source: u32,
        name: crate::SourceName,
    },
    Component(ToComponent),
    Signing(Signing),
    /// Answer a party's request.
    Answer {
        request: Token,
        answer: Answer,
    },
    Granted {
        grant: Grant,
    },
    /// Keep the record under its own key in this decision.
    Save {
        record: Stored,
    },
    /// Remove the record under this key in this decision.
    Erase {
        key: Key,
    },
    Closed,
}

/// A source-owned deadline (domain/accounts.md, 3.5; domain/engine.md, 5.4).
/// Scheduling uses `at`; documents and stored resets retain `wall` exactly.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Deadline {
    pub at: Time,
    pub wall: Wall,
}

/// Secret-free source state (domain/engine.md, section 12; domain/accounts.md, 3.5):
/// limited until a time, or, with none, until its holder resumes it; refused until its
/// holder acts.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum State {
    Asking,
    Granted,
    Later { until: Deadline },
    Limited { until: Option<Deadline> },
    Refused,
    NeedsSignIn,
    Misconfigured,
    Unloaded { at: Place, why: Unread },
    Cancelled,
    Closing,
    Moving,
}

/// A source's state, with its account; an account's view is the source deciding it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Fact {
    pub account: u32,
    pub source: u32,
    pub kind: Kind,
    pub state: State,
    pub generation: Option<u64>,
    pub usable: bool,
    pub expiring: bool,
    pub spent_until: Option<Deadline>,
    pub attention: bool,
}

/// A holder's account change; an LLM key is named only by its carried number.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Change {
    /// Finish a sign-in with its person's pasted address, named by its held number (accounts.md, section 5).
    FinishSignIn {
        source: u32,
        carried: u64,
        by: u64,
    },
    /// Move the account's signed-in source out (accounts.md, section 6).
    MoveOut {
        account: u32,
    },
    /// The client wrote the move's file, naming the move's number.
    Written {
        number: u32,
    },
    /// Move in from a file's bytes, carried as an LLM key's value is.
    MoveIn {
        carried: u64,
    },
    DeclareProgram {
        provider: u32,
        join: Option<u32>,
        name: Box<[u8]>,
    },
    CheckProgram {
        source: u32,
    },
    AddKey {
        provider: u32,
        join: Option<u32>,
        carried: u64,
    },
    ReplaceKey {
        source: u32,
        carried: u64,
    },
    RemoveKey {
        source: u32,
    },
    Resume {
        source: u32,
    },
    Remove {
        account: u32,
    },
    MoveAccount {
        account: u32,
        place: u32,
    },
    MoveSource {
        account: u32,
        source: u32,
        place: u32,
    },
    SetUses {
        account: u32,
        uses: Option<Box<[Use]>>,
    },
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum KeyFailure {
    TooLong,
    NotStored,
}

#[cfg(test)]
mod tests {
    use super::{Form, GrantTable, Scheme};

    #[test]
    fn a_form_names_its_table() {
        for (form, table) in [
            (Form::Credential(Scheme::Bearer), GrantTable::Accounts),
            (Form::Credential(Scheme::Key), GrantTable::Accounts),
            (Form::Application, GrantTable::Application),
            (Form::Program(crate::SourceName::new(b"claude").expect("source name")), GrantTable::Name),
        ] {
            assert_eq!(form.table(), table);
        }
    }

    #[test]
    fn a_form_takes_34_bytes() {
        // tag 1 + length 1 + 32 name bytes.
        assert_eq!(size_of::<Form>(), 34);
    }
}

/// One configured worker's terminal sign-out result; no login bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ProgramResult {
    SignedOut,
    NotInContact,
    Lost,
    Failed,
}
/// Only unconfirmed terminal results can enter a failed-host list.
/// Account removal keeps the greatest severity: `NotInContact` < `Failed` < `Lost`.
/// Lost means an admitted in-flight operation ended without its worker's terminal.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ProgramFailure {
    NotInContact,
    Failed,
    Lost,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ProgramHost {
    pub worker: crate::Worker,
    pub result: ProgramFailure,
}
/// Desired cleanup is durable; observations remain per-channel and volatile.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Program {
    pub name: crate::SourceName,
    pub cleanup: bool,
}

/// The live restore set that does not fit the deployment's current limits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RestoreLimit {
    Accounts,
    Sources,
    SourcePlaces,
    ProgramNames,
}
