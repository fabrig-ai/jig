//! The English fallback for the public enum phrases.
use super::{ALL, HoldWord, RefusalWord, StatusWord, WaitingWord, Word};
use crate::contract::{Bound, Phase};
use alloc::boxed::Box;
use skein_lib::{List, bytes::copy_of};
use skein_ui::phrases::HoleKind;
use skein_ui::phrases::{Form, Language, Phrase, PluralClass, PluralRule, Segment, SpanUnit, WallFormat};
pub(super) fn language() -> Language {
    let mut phrases = List::with_capacity(u32::try_from(ALL.len()).expect("closed word count"));
    for word in ALL {
        let segments = match word {
            Word::Decision(value) => value.segments(),
            Word::Phase(_) | Word::Hold(_) | Word::Refusal(_) | Word::Waiting(_) | Word::Status(_) => {
                let (_, label, holes) = word.data();
                let mut segments = List::with_capacity(3);
                segments.push(Segment::Text(copy_of(label))).expect("word label fits");
                for kind in holes {
                    segments.push(Segment::Hole { index: 0, kind: *kind }).expect("word hole fits");
                }
                if let Word::Hold(HoldWord::Exceeded(bound)) = word {
                    let suffix: &[u8] = match bound {
                        Bound::Reasoning | Bound::Metadata | Bound::Answer => b" bytes",
                        Bound::Items => b" items",
                        Bound::Tools => b" tools",
                        Bound::HistoryItems => b" messages and blocks",
                        Bound::History => b"",
                        Bound::Window | Bound::Output => b" tokens",
                    };
                    if !suffix.is_empty() {
                        segments.push(Segment::Text(copy_of(suffix))).expect("unit suffix fits");
                    }
                }
                segments.into_boxed()
            }
        };
        phrases
            .push(Phrase { id: word.id(), forms: Box::new([Form { class: PluralClass::Other, segments }]) })
            .expect("all word phrases fit");
    }
    Language {
        plural: PluralRule::English,
        span_unit: SpanUnit::Seconds,
        wall: WallFormat::Clock,
        phrases: phrases.into_boxed(),
    }
}

impl Word {
    pub(super) const fn data(self) -> (u16, &'static [u8], &'static [HoleKind]) {
        match self {
            Self::Decision(value) => value.data(),
            Self::Refusal(RefusalWord::Forgotten) => (118, b"Forgotten; oldest kept revision ", &[HoleKind::Number]),
            Self::Refusal(RefusalWord::Stale) => (112, b"Settings changed; newest revision ", &[HoleKind::Number]),
            Self::Refusal(RefusalWord::Full) => {
                (113, b"Settings history is full; oldest held revision ", &[HoleKind::Number])
            }
            Self::Hold(HoldWord::Circle) => (91, b"Held for a circle of waits", &[]),
            Self::Refusal(RefusalWord::Circle) => (92, b"These tasks would wait for one another", &[]),
            Self::Refusal(RefusalWord::Dependency) => (93, b"The added dependency has ended", &[]),
            Self::Refusal(RefusalWord::Kept) => (94, b"This answer waits for pending dependencies", &[]),
            Self::Waiting(WaitingWord::Dependencies) => (95, b"Kept by its dependencies", &[]),
            Self::Phase(Phase::Waiting) => (0, b"Waiting", &[]),
            Self::Phase(Phase::Idle) => (1, b"Waiting for news", &[]),
            Self::Phase(Phase::Due) => (2, b"Due to run", &[]),
            Self::Phase(Phase::Preparing) => (3, b"Preparing a host", &[]),
            Self::Phase(Phase::Claimed) => (4, b"Claimed by a host", &[]),
            Self::Phase(Phase::Running) => (5, b"Running", &[]),
            Self::Phase(Phase::BackingOff) => (6, b"Retrying", &[]),
            Self::Phase(Phase::Active) => (7, b"Executor active", &[]),
            Self::Phase(Phase::Closing) => (8, b"Closing", &[]),
            Self::Phase(Phase::Held) => (9, b"Held", &[]),
            Self::Phase(Phase::Ended) => (10, b"Ended", &[]),
            Self::Phase(Phase::Done) => (11, b"Completed", &[]),
            Self::Phase(Phase::Failed) => (12, b"Failed", &[]),
            Self::Phase(Phase::Cancelled) => (13, b"Cancelled", &[]),
            Self::Hold(HoldWord::Exceeded(bound)) => exceeded(bound),
            Self::Hold(HoldWord::Failures) => (14, b"Held: out of tries", &[]),
            Self::Hold(HoldWord::Stalled) => (15, b"The executor stopped progressing", &[]),
            Self::Hold(HoldWord::Dependency) => (16, b"Dependency task ", &[HoleKind::Number]),
            Self::Hold(HoldWord::Stopped) => (17, b"Task stopped", &[]),
            Self::Hold(HoldWord::Placed) => (18, b"Held by a placed hold", &[]),
            Self::Hold(HoldWord::Drift) => (19, b"Required resource drifted", &[]),
            Self::Hold(HoldWord::Effects) => (20, b"Effects need a decision", &[]),
            Self::Hold(HoldWord::EffectFailed) => (21, b"An effect failed permanently", &[]),
            Self::Hold(HoldWord::Uncertain) => (22, b"Uncertain outbox entry ", &[HoleKind::Number]),
            Self::Hold(HoldWord::Budget) => (23, b"Insufficient activation budget", &[]),
            Self::Hold(HoldWord::Deadline) => (24, b"Deadline passed", &[]),
            Self::Hold(HoldWord::HoldsWaited) => (25, b"Resource holds waited too long", &[]),
            Self::Hold(HoldWord::ResourceUnavailable) => (26, b"A required connector resource is unavailable", &[]),
            Self::Hold(HoldWord::ResourceAccess) => (27, b"Current resource access forbids this action", &[]),
            Self::Hold(HoldWord::PoolLost) => (28, b"Allocation lost from pool ", &[HoleKind::Number]),
            Self::Hold(HoldWord::Procedure) => (29, b"Procedure needs a decision", &[]),
            Self::Refusal(RefusalWord::NoEscalationHolder) => (30, b"No one to escalate to", &[]),
            Self::Refusal(RefusalWord::Malformed) => (31, b"Malformed operation", &[]),
            Self::Refusal(RefusalWord::Role) => (32, b"The held role does not permit this action", &[]),
            Self::Refusal(RefusalWord::Taken) => (34, b"Held by party ", &[HoleKind::Number]),
            Self::Refusal(RefusalWord::NotWaiting) => (35, b"This object waits for someone else", &[]),
            Self::Refusal(RefusalWord::NoFurther) => (36, b"This holder cannot pass further", &[]),
            Self::Refusal(RefusalWord::Authority) => (37, b"Authority missing", &[]),
            Self::Refusal(RefusalWord::Limit) => (38, b"Limit reached", &[]),
            Self::Refusal(RefusalWord::Moved) => (39, b"The object changed", &[]),
            Self::Refusal(RefusalWord::Ended) => (40, b"The object ended", &[]),
            Self::Refusal(RefusalWord::Unknown) => (41, b"No visible object with that name", &[]),
            Self::Refusal(RefusalWord::NameTaken) => (106, b"Another project has that name", &[]),
            Self::Refusal(RefusalWord::NotOffered) => (42, b"This act is not offered", &[]),
            Self::Refusal(RefusalWord::KeyConflict) => (43, b"The request key names another act", &[]),
            Self::Refusal(RefusalWord::AccountMoveNotSignedIn) => (107, b"not signed in", &[]),
            Self::Refusal(RefusalWord::AccountMoveNotOurs) => (108, b"not ours", &[]),
            Self::Refusal(RefusalWord::AccountMoveHeld) => (109, b"held by another holder", &[]),
            Self::Refusal(RefusalWord::AccountMoveCancelled) => (110, b"cancelled", &[]),
            Self::Refusal(RefusalWord::SignInRedirectNotWaiting) => (114, b"no sign-in of yours is waiting", &[]),
            Self::Refusal(RefusalWord::SignInRedirectAddress) => (115, b"not this sign-in's address", &[]),
            Self::Refusal(RefusalWord::SignInRedirectState) => (116, b"from another sign-in", &[]),
            Self::Refusal(RefusalWord::SignInRedirectTooMany) => (117, b"too many addresses from other sign-ins", &[]),
            Self::Refusal(RefusalWord::AccountMoveMoving) => (111, b"moving", &[]),
            Self::Refusal(RefusalWord::Provider) => (86, b"The provider does not permit these account uses", &[]),
            Self::Refusal(RefusalWord::Llm) => (85, b"This model cannot be chosen: ", &[HoleKind::Name]),
            Self::Refusal(RefusalWord::AccountUnknown) => (87, b"Account not found: ", &[HoleKind::Number]),
            Self::Refusal(RefusalWord::AccountProvider) => {
                (88, b"Account belongs to another provider: ", &[HoleKind::Number])
            }
            Self::Refusal(RefusalWord::AccountUses) => (89, b"Account does not serve the payer: ", &[HoleKind::Number]),
            Self::Refusal(RefusalWord::AccountLimited) => {
                (90, b"Kept account is limited; cannot switch to account ", &[HoleKind::Number])
            }
            Self::Waiting(WaitingWord::Party) => (44, b"Waiting for a party", &[]),
            Self::Waiting(WaitingWord::Delegates) => (45, b"Waiting for delegates", &[]),
            Self::Waiting(WaitingWord::News) => (46, b"Waiting for news", &[]),
            Self::Waiting(WaitingWord::Nothing) => (47, b"Nothing awaited", &[]),
            Self::Status(StatusWord::HostRefused) => (96, b"Cannot start yet", &[]),
            Self::Status(StatusWord::Starting) => (48, b"Starting", &[]),
            Self::Status(StatusWord::Writing) => (49, b"Writing", &[]),
            Self::Status(StatusWord::Tool) => (50, b"Using tool ", &[HoleKind::Name]),
            Self::Status(StatusWord::Working) => (51, b"Working", &[]),
            Self::Status(StatusWord::WaitingForYou) => (52, b"Waiting for your words", &[]),
            Self::Status(StatusWord::WaitingFor) => (53, b"Waiting for another party", &[]),
            Self::Status(StatusWord::Parked) => (54, b"Parked; your words resume it", &[]),
            Self::Status(StatusWord::Retrying) => (55, b"Retrying", &[]),
            Self::Status(StatusWord::Held) => (56, b"Held", &[]),
            Self::Status(StatusWord::Closing) => (57, b"Closing", &[]),
            Self::Status(StatusWord::Ended) => (58, b"Ended", &[]),
        }
    }
}

const fn exceeded(bound: Bound) -> (u16, &'static [u8], &'static [HoleKind]) {
    match bound {
        Bound::Reasoning => {
            (97, b"Held: the model's reasoning went past its configured allowance of ", &[HoleKind::Number])
        }
        Bound::Metadata => {
            (98, b"Held: the model's metadata went past its configured allowance of ", &[HoleKind::Number])
        }
        Bound::Items => (99, b"Held: the model's answer went past its configured allowance of ", &[HoleKind::Number]),
        Bound::Answer => (100, b"Held: the model's answer went past its configured allowance of ", &[HoleKind::Number]),
        Bound::Tools => (101, b"Held: the run's tools went past their configured allowance of ", &[HoleKind::Number]),
        Bound::HistoryItems => {
            (102, b"Held: the run's request went past its configured allowance of ", &[HoleKind::Number])
        }
        Bound::History => {
            (103, b"Held: the run's history no longer fits its configured allowance, even compacted", &[])
        }
        Bound::Window => (104, b"Held: the run no longer fits the model's window of ", &[HoleKind::Number]),
        Bound::Output => (105, b"Held: the request went past the model's output ceiling of ", &[HoleKind::Number]),
    }
}
