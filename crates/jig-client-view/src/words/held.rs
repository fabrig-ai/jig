//! Complete moved-hold facts without requiring an object slot; contract.md, 4.
use super::{Words, name};
use crate::phrases::jig::JigPhrase;
use jig_client::contract::{HoldCategory, HoldFailure, HoldReason, Part};
use jig_client::words::{HoldWord, Word};
use skein_ui::Builder;
use skein_ui::phrases::Hole;
impl Words {
    /// Admission reason facts, without invented amounts, actors, clocks or parts.
    pub(crate) fn hold_category(&self, b: &mut Builder, reason: HoldCategory) {
        match reason {
            HoldCategory::Circle => {
                self.text(b, JigPhrase::CircleHold, &[]);
                self.text(b, JigPhrase::CircleLifts, &[]);
            }
            HoldCategory::Exceeded(value) => {
                let holes: &[Hole] = match value.bound {
                    jig_client::contract::Bound::History => &[],
                    jig_client::contract::Bound::Reasoning
                    | jig_client::contract::Bound::Metadata
                    | jig_client::contract::Bound::Items
                    | jig_client::contract::Bound::Answer
                    | jig_client::contract::Bound::Tools
                    | jig_client::contract::Bound::HistoryItems
                    | jig_client::contract::Bound::Window
                    | jig_client::contract::Bound::Output => &[Hole::Number(value.value)],
                };
                self.text(b, JigPhrase::Word(Word::hold(&reason)), holes);
            }
            HoldCategory::Failures { class } => {
                self.text(b, JigPhrase::Word(Word::Hold(HoldWord::Failures)), &[]);
                let phrase = match class {
                    HoldFailure::Transient => JigPhrase::Temporary,
                    HoldFailure::Permanent => JigPhrase::Permanent,
                    HoldFailure::Run => JigPhrase::FailureRun,
                    HoldFailure::Agent => JigPhrase::FailureAgent,
                    HoldFailure::Lost => JigPhrase::HostLost,
                    HoldFailure::Invalid => JigPhrase::FailureInvalid,
                };
                self.text(b, phrase, &[]);
            }
            HoldCategory::Stalled => self.text(b, JigPhrase::Word(Word::Hold(HoldWord::Stalled)), &[]),
            HoldCategory::Dependency { task } => {
                self.text(b, JigPhrase::Word(Word::Hold(HoldWord::Dependency)), &[Hole::Number(task)]);
            }
            HoldCategory::Stopped => self.text(b, JigPhrase::Word(Word::Hold(HoldWord::Stopped)), &[]),
            HoldCategory::Placed { .. } => self.text(b, JigPhrase::Word(Word::hold(&reason)), &[]),
            HoldCategory::Drift => self.text(b, JigPhrase::Word(Word::Hold(HoldWord::Drift)), &[]),
            HoldCategory::Effects => self.text(b, JigPhrase::Word(Word::Hold(HoldWord::Effects)), &[]),
            HoldCategory::EffectFailed => self.text(b, JigPhrase::Word(Word::Hold(HoldWord::EffectFailed)), &[]),
            HoldCategory::Uncertain { entry } => {
                self.text(b, JigPhrase::Word(Word::Hold(HoldWord::Uncertain)), &[Hole::Number(entry)]);
            }
            HoldCategory::Budget => self.text(b, JigPhrase::Word(Word::Hold(HoldWord::Budget)), &[]),
            HoldCategory::Deadline => self.text(b, JigPhrase::Word(Word::Hold(HoldWord::Deadline)), &[]),
            HoldCategory::HoldsWaited => self.text(b, JigPhrase::Word(Word::Hold(HoldWord::HoldsWaited)), &[]),
            HoldCategory::ResourceUnavailable | HoldCategory::ResourceAccess => {
                self.text(b, JigPhrase::Word(Word::hold(&reason)), &[]);
            }
            HoldCategory::PoolLost { pool } => {
                self.text(b, JigPhrase::Word(Word::Hold(HoldWord::PoolLost)), &[Hole::Number(pool)]);
            }
            HoldCategory::Procedure => self.text(b, JigPhrase::Word(Word::Hold(HoldWord::Procedure)), &[]),
        }
    }

    /// Show the exact current hold reason supplied with a moved refusal.
    pub(super) fn hold_fact(&self, b: &mut Builder, reason: &HoldReason, text_limit: u32) {
        match reason {
            HoldReason::Tries { failures } => {
                self.text(b, JigPhrase::Word(Word::Hold(HoldWord::Failures)), &[]);
                for failure in failures {
                    self.text(b, JigPhrase::Attempt, &[Hole::Number(failure.attempt)]);
                    crate::cards::chip::failure(b, failure.class, self);
                    self.text(b, JigPhrase::Reason, &[name(failure.reason.bytes())]);
                    self.wall(b, failure.at);
                }
            }
            HoldReason::Stalled { since, waiting } => {
                self.text(b, JigPhrase::StalledOn, &[name(waiting.bytes())]);
                self.text(b, JigPhrase::Since, &[Hole::Wall(self.adjusted_wall(*since))]);
            }
            HoldReason::Dependency { task, how } => {
                self.text(b, JigPhrase::DependencyEnded, &[Hole::Number(*task)]);
                self.ending(b, how);
            }
            HoldReason::Drift { part } => {
                self.text(b, JigPhrase::DriftHold, &[]);
                self.part_fact(b, part, text_limit);
            }
            HoldReason::EffectFailed { part } => {
                self.text(b, JigPhrase::EffectFailedHold, &[]);
                self.part_fact(b, part, text_limit);
            }
            HoldReason::EffectUncertain { part } => {
                self.text(b, JigPhrase::EffectUncertainHold, &[]);
                self.part_fact(b, part, text_limit);
            }
            HoldReason::Budget { spent, limit } => {
                self.text(b, JigPhrase::BudgetHold, &[Hole::Amount(*spent), Hole::Amount(*limit)]);
            }
            HoldReason::Deadline { at } => {
                self.text(b, JigPhrase::DeadlineHold, &[Hole::Wall(self.adjusted_wall(*at))]);
            }
            HoldReason::Stopped { by, at } => {
                self.text(b, JigPhrase::StoppedHold, &[name(by.name.bytes()), Hole::Wall(self.adjusted_wall(*at))]);
            }
            HoldReason::Resource { resource, holder } => {
                self.text(b, JigPhrase::ResourceHold, &[]);
                self.part_fact(b, resource, text_limit);
                self.actor(b, *holder);
            }
            HoldReason::Pool { part } => {
                self.text(b, JigPhrase::PoolHold, &[]);
                self.part_fact(b, part, text_limit);
            }
            HoldReason::Procedure { part } => {
                self.text(b, JigPhrase::ProcedureHold, &[]);
                self.part_fact(b, part, text_limit);
            }
        }
    }
    fn part_fact(&self, b: &mut Builder, part: &Part, text_limit: u32) {
        let connector = match part {
            Part::Connector { connector }
            | Part::Resource { connector, .. }
            | Part::Kind { connector, .. }
            | Part::Panel { connector, .. }
            | Part::Result { connector, .. }
            | Part::Effect { connector, .. }
            | Part::OwnerEffect { connector, .. }
            | Part::Outbox { connector, .. }
            | Part::Hold { connector, .. }
            | Part::Project { connector, .. }
            | Part::Deployment { connector } => *connector,
        };
        self.text(b, JigPhrase::PartConnector, &[Hole::Number(u64::from(connector))]);
        match part {
            Part::Connector { .. } => {}
            Part::Deployment { .. } => self.text(b, JigPhrase::Deployment, &[]),
            Part::Resource { path, .. } => {
                self.text(b, JigPhrase::ResourceSegments, &[]);
                let reserved = u32::try_from(self.say(JigPhrase::ResourceSegments, &[]).len()).expect("bounded phrase");
                crate::literal_segment::path(
                    b,
                    path.as_slice(),
                    self,
                    crate::literal_segment::PathDisplay { text: text_limit, reserved, keep_last: true },
                );
            }
            Part::Kind { kind, .. } => self.text(b, JigPhrase::PartKind, &[Hole::Number(u64::from(*kind))]),
            Part::Panel { task, .. } => {
                self.text(b, JigPhrase::PanelPart, &[]);
                self.text(b, JigPhrase::PartTask, &[Hole::Number(*task)]);
            }
            Part::Result { task, .. } => {
                self.text(b, JigPhrase::ResultPart, &[]);
                self.text(b, JigPhrase::PartTask, &[Hole::Number(*task)]);
            }
            Part::Hold { task, .. } => {
                self.text(b, JigPhrase::HoldPart, &[]);
                self.text(b, JigPhrase::PartTask, &[Hole::Number(*task)]);
            }
            Part::Outbox { scope, entry, .. } => {
                match scope {
                    jig_client::contract::PartScope::Project(project) => {
                        self.text(b, JigPhrase::PartProject, &[Hole::Number(u64::from(*project))]);
                    }
                    jig_client::contract::PartScope::Deployment => self.text(b, JigPhrase::Deployment, &[]),
                }
                self.text(b, JigPhrase::OutboxEntry, &[Hole::Number(*entry)]);
            }
            Part::Project { project, .. } => self.text(b, JigPhrase::PartProject, &[Hole::Number(u64::from(*project))]),
            Part::OwnerEffect { proposal, .. } => {
                self.text(b, JigPhrase::PartOwnerProposal, &[Hole::Number(*proposal)]);
            }
            Part::Effect { proposal, .. } => {
                let (phrase, maker) = match proposal.proposer {
                    jig_client::contract::Proposer::Task(task) => (JigPhrase::PartProposalTask, task),
                    jig_client::contract::Proposer::Party(party) => (JigPhrase::PartProposalParty, party),
                };
                self.text(b, phrase, &[Hole::Number(maker), Hole::Number(proposal.number)]);
            }
        }
    }
}
