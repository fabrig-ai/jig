//! Typed facts said through a checked phrasebook (client/domain/view.md, section 7).
//! Keeps language data, a selected language and the person's UTC offset. It never
//! computes authority. Each saying must fit the phrasebook's checked shape.
mod held;
mod lacking;
mod refusals;
mod status;
use crate::intents::Progress;
use crate::phrases::jig::JigPhrase;
use alloc::boxed::Box;
use jig_client::contract::{Actor, Decision, Ending, Executor, Made, Outcome, OwnerExecutor, ProposalKey, Proposer};
use jig_client::{
    Link,
    contract::Phase,
    words::{DecisionWord as D, Word},
};
use skein_lib::{Duration, Wall, bytes::copy_of};
use skein_ui::phrases::{Hole, Phrasebook};
use skein_ui::{Builder, Role, Run};

/// Checked words and the person's time presentation for one tree build.
#[derive(Clone, Debug)]
pub struct Words {
    book: Phrasebook,
    language: u16,
    offset_minutes: i32,
    jig_capacity: Option<u32>,
}
impl Words {
    /// Keep the whole vocabulary envelope with its checked book.
    #[must_use]
    pub fn new(book: Phrasebook, language: u16, offset_minutes: i32) -> Self {
        let jig_capacity = crate::phrases::jig_capacity(&book);
        Self { book, language, offset_minutes, jig_capacity }
    }
    /// The checked book, shared so its envelope cannot become stale.
    #[must_use]
    pub fn book(&self) -> &Phrasebook {
        &self.book
    }
    /// Replace the book and its whole vocabulary envelope together.
    pub fn set_book(&mut self, book: Phrasebook) {
        let jig_capacity = crate::phrases::jig_capacity(&book);
        self.book = book;
        self.jig_capacity = jig_capacity;
    }
    /// The complete jig envelope across every loaded language.
    #[must_use]
    pub fn jig_capacity(&self) -> Option<u32> {
        self.jig_capacity
    }
    /// The language selected for sayings.
    #[must_use]
    pub fn language(&self) -> u16 {
        self.language
    }
    /// Select a language without changing the whole vocabulary envelope.
    pub fn set_language(&mut self, language: u16) {
        self.language = language;
    }
    /// The person's offset from UTC, in minutes.
    #[must_use]
    pub fn offset_minutes(&self) -> i32 {
        self.offset_minutes
    }
    /// Change the person's offset without changing the vocabulary envelope.
    pub fn set_offset_minutes(&mut self, offset_minutes: i32) {
        self.offset_minutes = offset_minutes;
    }
    /// Say one phrase in the selected language; mis-sized facts are a caller bug.
    #[must_use]
    pub fn say(&self, phrase: JigPhrase, holes: &[Hole]) -> Box<[u8]> {
        self.book.say(self.language, phrase.id(), holes).expect("checked phrase and bounded facts").text
    }
    /// Say jig's public enum phrase in the selected language.
    #[must_use]
    pub fn say_word(&self, word: Word, holes: &[Hole]) -> Box<[u8]> {
        self.book.say(self.language, word.id(), holes).expect("checked word and bounded facts").text
    }
    /// Emit one decision fact in the selected public vocabulary.
    pub fn decision_fact(&self, b: &mut Builder, phrase: D, holes: &[Hole]) {
        let text = self.say_word(Word::Decision(phrase), holes);
        b.text(Run::Plain, &text);
    }
    pub(crate) fn node_text(&self, text: u32) -> u32 {
        let phrase = match self.jig_capacity() {
            Some(phrase) => phrase,
            None => {
                let mut largest = 0;
                for phrase in crate::owner_authority_words::ALL {
                    largest = largest.max(self.book.capacity(phrase.id()).expect("authority words checked"));
                }
                largest
            }
        };
        crate::limits::node_text(text, phrase).expect("checked node text")
    }
    /// Emit a text leaf using a phrase's checked words.
    pub fn text(&self, b: &mut Builder, phrase: JigPhrase, holes: &[Hole]) {
        let text = self.say(phrase, holes);
        b.text(Run::Plain, &text);
    }
    /// Emit a named status, also exposing its words as text.
    pub fn status(&self, b: &mut Builder, phrase: JigPhrase, holes: &[Hole]) {
        let text = self.say(phrase, holes);
        b.open(Role::Status);
        b.name(&text);
        b.text(Run::Plain, &text);
        b.close();
    }
    /// Show a link's whole vocabulary.
    pub fn link(&self, b: &mut Builder, link: Link) {
        let phrase = match link {
            Link::Starting => JigPhrase::Starting,
            Link::Live => JigPhrase::Live,
            Link::Offline => JigPhrase::Offline,
            Link::SignedOut => JigPhrase::SignedOut,
            Link::OutOfDate => JigPhrase::UpdateClient,
        };
        self.status(b, phrase, &[]);
    }
    /// Say the request table's progress without owning its retry policy.
    pub fn progress(&self, b: &mut Builder, progress: Progress) {
        let phrase = match progress {
            Progress::Confirming => JigPhrase::Confirming,
            Progress::InFlight => JigPhrase::InFlight,
            Progress::Retrying => JigPhrase::Word(Word::Phase(Phase::BackingOff)),
            Progress::Full { held, room_at } => return self.answers_full(b, held, room_at),
            Progress::Parked => JigPhrase::Parked,
            Progress::Unknown => JigPhrase::Unknown,
        };
        self.status(b, phrase, &[]);
    }
    /// The engine's shared retained-answer table has no free place.
    pub fn answers_full(&self, b: &mut Builder, held: u32, room_at: Wall) {
        self.status(b, JigPhrase::AnswersFull, &[Hole::Number(u64::from(held)), Hole::Wall(room_at)]);
    }
    /// Say the executor kind; its connector content belongs in a slot.
    pub fn executor(&self, b: &mut Builder, executor: Executor) {
        let phrase = match executor {
            Executor::Agent { .. } => JigPhrase::Agent,
            Executor::Procedure { .. } => JigPhrase::Procedure,
            Executor::Party { .. } => JigPhrase::Person,
            Executor::Role { .. } => JigPhrase::RoleExecutor,
        };
        self.text(b, phrase, &[]);
    }
    /// Say the complete requested goal executor, with only supplied numbers.
    pub fn goal_executor(&self, b: &mut Builder, executor: OwnerExecutor) {
        match executor {
            OwnerExecutor::Agent { charter: number } => {
                self.text(b, JigPhrase::Charter, &[Hole::Number(u64::from(number))]);
            }
            OwnerExecutor::Procedure { connector, code } => self.text(
                b,
                JigPhrase::CreateProcedure,
                &[Hole::Number(u64::from(connector)), Hole::Number(u64::from(code))],
            ),
            OwnerExecutor::Person(_) => {}
        }
    }
    /// Show an amount in the engine's units.
    pub fn amount(&self, b: &mut Builder, amount: u64) {
        self.text(b, JigPhrase::Amount, &[Hole::Amount(amount)]);
    }
    /// Show the elapsed wall interval, clamped when the wall clock moved backward.
    pub fn waiting(&self, b: &mut Builder, now: Wall, since: Wall) {
        self.text(
            b,
            JigPhrase::WaitingSpan,
            &[Hole::Span(Duration::from_nanos(now.as_nanos().saturating_sub(since.as_nanos())))],
        );
    }
    /// Apply the person's offset using checked arithmetic, clamped at the epoch.
    pub fn wall(&self, b: &mut Builder, at: Wall) {
        b.open(Role::Time);
        self.text(b, JigPhrase::At, &[Hole::Wall(self.adjusted_wall(at))]);
        b.close();
    }
    /// Name the deadline as such, in the person's time presentation.
    pub fn deadline(&self, b: &mut Builder, at: Wall) {
        b.open(Role::Time);
        self.text(b, JigPhrase::Deadline, &[Hole::Wall(self.adjusted_wall(at))]);
        b.close();
    }
    /// A wall instant shifted to the person's UTC offset for a phrase hole.
    #[must_use]
    pub fn adjusted_wall(&self, at: Wall) -> Wall {
        let nanos = u64::from(self.offset_minutes.unsigned_abs()).saturating_mul(60_000_000_000);
        let adjusted = if self.offset_minutes < 0_i32 {
            at.as_nanos().saturating_sub(nanos)
        } else {
            at.as_nanos().saturating_add(nanos)
        };
        Wall::from_nanos(adjusted)
    }
    /// Say who acted; party names remain unchanged when supplied by the document.
    pub fn actor(&self, b: &mut Builder, by: Actor) {
        match by {
            Actor::Party(number) => self.text(b, JigPhrase::ByParty, &[Hole::Number(number)]),
            Actor::Task(number) => self.text(b, JigPhrase::ByTask, &[Hole::Number(number)]),
            Actor::Deployment => self.text(b, JigPhrase::ByDeployment, &[]),
            Actor::ProjectDeployment { project } => {
                self.text(b, JigPhrase::ByProjectDeployment, &[Hole::Number(u64::from(project))]);
            }
        }
    }
    /// Say each possible durable decision, including the uncertain-effect choice.
    pub fn decision(&self, b: &mut Builder, how: &Decision) {
        match how {
            Decision::Accepted => self.text(b, JigPhrase::Accepted, &[]),
            Decision::Rejected { reason } => self.text(b, JigPhrase::Rejected, &[name(reason.bytes())]),
            Decision::PassedUp => self.text(b, JigPhrase::PassedUp, &[]),
            Decision::Released { again } => {
                let phrase = match again {
                    Some(true) => JigPhrase::ReleasedAgain,
                    Some(false) => JigPhrase::ReleasedWithout,
                    None => JigPhrase::Released,
                };
                self.text(b, phrase, &[]);
            }
            Decision::LeftHeld { reason } => self.text(b, JigPhrase::LeftHeld, &[name(reason.bytes())]),
        }
    }
    /// Say a durable ending and every fact that ending carries.
    pub fn ending(&self, b: &mut Builder, how: &Ending) {
        match how {
            Ending::Done => {
                self.text(b, JigPhrase::Word(Word::Phase(Phase::Done)), &[]);
            }
            Ending::Failed { reason } => match reason {
                Some(reason) => self.text(b, JigPhrase::Failed, &[name(reason.bytes())]),
                None => self.text(b, JigPhrase::FailedBare, &[]),
            },
            Ending::Cancelled { by, reason, at } => {
                match reason {
                    Some(reason) => self.text(b, JigPhrase::Cancelled, &[name(reason.bytes())]),
                    None => self.text(b, JigPhrase::CancelledBare, &[]),
                }
                self.actor(b, *by);
                if let Some(at) = at {
                    self.wall(b, *at);
                }
            }
            Ending::Withdrawn { reason } => self.text(b, JigPhrase::Withdrawn, &[name(reason.bytes())]),
            Ending::Decided { by, party, how, at } => {
                if how.is_none() {
                    self.text(b, JigPhrase::DecidedBare, &[]);
                }
                match party {
                    Some(party) => self.text(b, JigPhrase::ByNamed, &[name(party.name.bytes())]),
                    None => self.actor(b, Actor::Party(*by)),
                }
                if let Some(how) = how {
                    self.decision(b, how);
                }
                self.wall(b, *at);
            }
            Ending::Deleted => self.text(b, JigPhrase::DeletedEnding, &[]),
            Ending::Task(value) => crate::owner_chip::ending_summary(b, value, self),
        }
    }
    /// The named maker, without inventing a party name or another actor kind.
    pub fn proposer(&self, b: &mut Builder, proposer: Proposer) {
        match proposer {
            Proposer::Task(task) => self.actor(b, Actor::Task(task)),
            Proposer::Party(party) => self.actor(b, Actor::Party(party)),
        }
    }
    /// A proposal's maker kind and both full-width numeric identities.
    pub fn proposal(&self, b: &mut Builder, proposal: ProposalKey) {
        let (phrase, maker) = match proposal.proposer {
            Proposer::Task(task) => (JigPhrase::Proposed, task),
            Proposer::Party(party) => (JigPhrase::ProposedByParty, party),
        };
        self.text(b, phrase, &[Hole::Number(maker), Hole::Number(proposal.number)]);
    }
    /// Say all durable outcomes, preserving who decided first and when.
    pub fn outcome(&self, b: &mut Builder, outcome: &Outcome) {
        match outcome {
            Outcome::Restored(value) => self.text(
                b,
                JigPhrase::SettingsRestored,
                &[Hole::Number(u64::from(value.to)), Hole::Number(value.proposal)],
            ),
            Outcome::MovedToNewest(value) => self.text(
                b,
                JigPhrase::MovedToNewest,
                &[
                    Hole::Number(u64::from(value.moved)),
                    Hole::Number(u64::from(value.newest)),
                    Hole::Number(u64::from(value.stayed)),
                ],
            ),
            Outcome::SettingsChanged(value) => {
                self.text(b, JigPhrase::SettingsChanged, &[Hole::Number(u64::from(value.revision))]);
            }
            Outcome::Owner(value) => crate::owner_outcome::render(b, value, self),
            Outcome::Made { what } => match what {
                Made::Tasks { tasks } => {
                    for task in tasks {
                        self.text(b, JigPhrase::MadeTask, &[Hole::Number(*task)]);
                    }
                }
                Made::MovingOut { account, number, .. } => self.text(
                    b,
                    JigPhrase::MovingOut,
                    &[Hole::Number(u64::from(*account)), Hole::Number(u64::from(*number))],
                ),
                Made::Service { party } => self.text(b, JigPhrase::MadeService, &[Hole::Number(*party)]),
                Made::PersonResult { task } => self.text(b, JigPhrase::MadeResult, &[Hole::Number(*task)]),
                Made::Note { note, .. } => self.text(b, JigPhrase::MadeNote, &[Hole::Number(*note)]),
            },
            Outcome::Proposed { proposal } => {
                self.proposal(b, *proposal);
            }
            Outcome::Decided { by, how, at } => {
                self.actor(b, *by);
                self.decision(b, how);
                self.wall(b, *at);
            }
            Outcome::DecidedFirst { by, how, at } => {
                self.text(b, JigPhrase::DecidedFirst, &[name(by.name.bytes())]);
                self.decision(b, how);
                self.wall(b, *at);
            }
            Outcome::Changed => self.text(b, JigPhrase::Changed, &[]),
        }
    }
}
/// Copy a document name into a phrase's owned hole.
#[must_use]
pub fn name(bytes: &[u8]) -> Hole {
    Hole::Name(copy_of(bytes))
}
