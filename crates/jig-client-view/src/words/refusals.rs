//! Durable engine categories and complete refusal facts; client/domain/contract.md, 4.
use super::Words;
use crate::phrases::jig::JigPhrase;
use jig_client::contract::{AdmissionLimit, CurrentHold, HeldRole, Limit, Malformed, Moved, NameRule, Refusal};
use jig_client::words::{RefusalWord, Word};
use skein_ui::Builder;
use skein_ui::phrases::Hole;
impl Words {
    /// Say the supplied refusal, never deriving permission or numerical limits.
    pub fn refusal(&self, b: &mut Builder, refusal: &Refusal, text_limit: u32) {
        match refusal {
            Refusal::Forgotten(value) => self.text(
                b,
                JigPhrase::Word(Word::Refusal(RefusalWord::Forgotten)),
                &[Hole::Number(u64::from(value.oldest))],
            ),
            Refusal::Stale(value) => self.text(b, JigPhrase::SettingsStale, &[Hole::Number(u64::from(value.newest))]),
            Refusal::Full(value) => {
                self.text(b, JigPhrase::SettingsFull, &[Hole::Number(u64::from(value.revision))]);
                for task in &value.tasks {
                    self.text(b, JigPhrase::SettingsFullTask, &[Hole::Number(*task)]);
                }
                self.text(b, JigPhrase::SettingsFullMore, &[Hole::Number(u64::from(value.more))]);
            }
            Refusal::Circle { tasks, cut } => {
                self.text(b, JigPhrase::CircleRefused, &[]);
                for task in tasks {
                    self.text(b, JigPhrase::CircleWaitsFor, &[Hole::Number(*task)]);
                }
                if *cut {
                    self.text(b, JigPhrase::TaskListsCut, &[]);
                }
            }
            Refusal::Dependency { dependency, how } => self.text(
                b,
                match how {
                    jig_client::contract::OwnerTaskStatus::Done => JigPhrase::DependencyDone,
                    jig_client::contract::OwnerTaskStatus::Failed => JigPhrase::DependencyFailed,
                    jig_client::contract::OwnerTaskStatus::Cancelled => JigPhrase::DependencyCancelled,
                },
                &[Hole::Number(*dependency)],
            ),
            Refusal::Kept { pending } => {
                self.text(b, JigPhrase::KeptAnswer, &[]);
                for task in pending {
                    self.text(b, JigPhrase::KeptUntil, &[Hole::Number(*task)]);
                }
            }

            Refusal::NameTaken => self.text(b, JigPhrase::NameTaken, &[]),
            Refusal::Malformed { at, why } => self.malformed(b, *at, *why),
            Refusal::Role { held } => {
                self.text(b, JigPhrase::Word(Word::Refusal(RefusalWord::Role)), &[]);
                self.held_role(b, *held);
            }
            Refusal::Taken { by } => {
                self.text(b, JigPhrase::Word(Word::Refusal(RefusalWord::Taken)), &[Hole::Number(*by)]);
            }
            Refusal::NotWaiting => self.text(b, JigPhrase::Word(Word::Refusal(RefusalWord::NotWaiting)), &[]),
            Refusal::NoFurther => self.text(b, JigPhrase::Word(Word::Refusal(RefusalWord::NoFurther)), &[]),
            Refusal::Authority { lacking, proposable } => {
                self.lacking(b, lacking, text_limit);
                if let Some(false) = proposable {
                    self.text(b, JigPhrase::NobodyMayAccept, &[]);
                }
            }
            Refusal::Limit { which } => self.limit(b, *which),
            Refusal::Moved { now, what } => {
                let phrase = match what {
                    Moved::Hold { .. } => JigPhrase::MovedHold,
                    Moved::Spec => JigPhrase::MovedSpec,
                    Moved::PersonTask => JigPhrase::MovedPerson,
                    Moved::Note => JigPhrase::MovedNote,
                    Moved::Waiting => JigPhrase::MovedWaiting,
                };
                self.text(b, phrase, &[]);
                self.text(b, JigPhrase::Revision, &[Hole::Number(now.value)]);
                match what {
                    Moved::Hold { reason } => {
                        if let Some(reason) = reason {
                            match reason {
                                CurrentHold::Document(reason) => self.hold_fact(b, reason, text_limit),
                                CurrentHold::Reason(reason) => self.hold_category(b, *reason),
                            }
                        }
                    }
                    Moved::Spec | Moved::PersonTask | Moved::Note | Moved::Waiting => {}
                }
            }
            Refusal::Ended { how } => self.ending(b, how),
            Refusal::Account { account, .. } => {
                self.text(b, JigPhrase::Word(Word::refusal(refusal)), &[Hole::Number(u64::from(*account))]);
            }
            Refusal::SignInRedirect { .. } | Refusal::AccountMove { .. } => {
                self.text(b, JigPhrase::Word(Word::refusal(refusal)), &[]);
            }
            Refusal::Provider => self.text(b, JigPhrase::Word(Word::Refusal(RefusalWord::Provider)), &[]),
            Refusal::Unknown => self.text(b, JigPhrase::Word(Word::Refusal(RefusalWord::Unknown)), &[]),
            Refusal::NoEscalationHolder => {
                self.text(b, JigPhrase::Word(Word::Refusal(RefusalWord::NoEscalationHolder)), &[]);
            }
            Refusal::NotOffered => self.text(b, JigPhrase::Word(Word::Refusal(RefusalWord::NotOffered)), &[]),
            Refusal::KeyConflict => self.text(b, JigPhrase::Word(Word::Refusal(RefusalWord::KeyConflict)), &[]),
            Refusal::Llm { model, .. } => {
                self.text(b, JigPhrase::Word(Word::Refusal(RefusalWord::Llm)), &[super::name(model.bytes())]);
            }
        }
    }
    fn malformed(&self, b: &mut Builder, at: u32, why: Malformed) {
        let position = [Hole::Number(u64::from(at))];
        match why {
            Malformed::Running => self.text(b, JigPhrase::MalformedRunning, &[]),
            Malformed::SettingsUnchanged => self.text(b, JigPhrase::SettingsUnchanged, &[]),
            Malformed::Settings(value) => {
                use jig_client::contract::{Setting, SettingFault};
                match value.setting {
                    Setting::Charter(n) => {
                        self.text(b, JigPhrase::SettingCharter, &[Hole::Number(u64::from(n.number))]);
                    }
                    Setting::Template(n) => {
                        self.text(b, JigPhrase::SettingTemplate, &[Hole::Number(u64::from(n.number))]);
                    }
                    Setting::Policy(n) => self.text(b, JigPhrase::SettingPolicy, &[Hole::Number(u64::from(n.number))]),
                    Setting::Rules => self.text(b, JigPhrase::SettingRules, &[]),
                }
                let phrase = match value.fault {
                    SettingFault::Shape => JigPhrase::SettingFaultShape,
                    SettingFault::Count => JigPhrase::SettingFaultCount,
                    SettingFault::Reference => JigPhrase::SettingFaultReference,
                    SettingFault::Rules => JigPhrase::SettingFaultRules,
                    SettingFault::Budget => JigPhrase::SettingFaultBudget,
                    SettingFault::Grace => JigPhrase::SettingFaultGrace,
                    SettingFault::Owner => JigPhrase::SettingFaultOwner,
                };
                self.text(b, phrase, &[]);
            }
            Malformed::Name(rule) => {
                let phrase = match rule {
                    NameRule::Empty => JigPhrase::NameEmpty,
                    NameRule::Long => JigPhrase::NameLong,
                    NameRule::Control => JigPhrase::NameControl,
                    NameRule::Space => JigPhrase::NameSpace,
                    NameRule::Digits => JigPhrase::NameDigits,
                };
                self.text(b, phrase, &[]);
            }
            Malformed::ProjectMismatch => self.text(b, JigPhrase::MalformedProject, &position),
            Malformed::TwoAsAllowed => self.text(b, JigPhrase::MalformedTwoAsAllowed, &position),
            Malformed::EndedBy => self.text(b, JigPhrase::MalformedEndedBy, &position),
            Malformed::ConversationNotAgent => self.text(b, JigPhrase::MalformedConversation, &position),
            Malformed::Cycle => self.text(b, JigPhrase::MalformedCycle, &position),
            Malformed::ProposedAsAllowed => self.text(b, JigPhrase::MalformedProposedAsAllowed, &position),
        }
    }
    pub(crate) fn held_role(&self, b: &mut Builder, held: Option<HeldRole>) {
        match held {
            None => self.text(b, JigPhrase::NoHeldRole, &[]),
            Some(HeldRole::Owner) => self.text(b, JigPhrase::HeldOwner, &[]),
            Some(HeldRole::Maintainer) => self.text(b, JigPhrase::HeldMaintainer, &[]),
            Some(HeldRole::Member) => self.text(b, JigPhrase::HeldMember, &[]),
            Some(HeldRole::Observer) => self.text(b, JigPhrase::HeldObserver, &[]),
            Some(HeldRole::Policy { role }) => self.text(b, JigPhrase::HeldPolicy, &[Hole::Number(u64::from(role))]),
        }
    }
    fn limit(&self, b: &mut Builder, which: Limit) {
        match which {
            Limit::Admission { which } => self.engine_limit(b, which),
            Limit::LiveTasks { project, limit } => {
                self.text(b, JigPhrase::LimitLive, &[Hole::Number(u64::from(project)), Hole::Number(u64::from(limit))]);
            }
            Limit::Inbox { task, limit } => {
                self.text(b, JigPhrase::LimitInbox, &[Hole::Number(task), Hole::Number(u64::from(limit))]);
            }
            Limit::Words { limit } => self.text(b, JigPhrase::LimitWords, &[Hole::Number(u64::from(limit))]),
            Limit::Fields => self.text(b, JigPhrase::LimitFields, &[]),
            Limit::Records => self.text(b, JigPhrase::LimitRecords, &[]),
            Limit::Requests => self.text(b, JigPhrase::LimitRequests, &[]),
            Limit::Reads => self.text(b, JigPhrase::LimitReads, &[]),
            Limit::Watches => self.text(b, JigPhrase::LimitWatches, &[]),
            Limit::Objects => self.text(b, JigPhrase::LimitObjects, &[]),
        }
    }
    fn engine_limit(&self, b: &mut Builder, which: AdmissionLimit) {
        let phrase = match which {
            AdmissionLimit::Goals => JigPhrase::EngineLimitGoals,
            AdmissionLimit::Operations => JigPhrase::EngineLimitOperations,
            AdmissionLimit::Grace => JigPhrase::EngineLimitGrace,
            AdmissionLimit::Words => JigPhrase::EngineLimitWords,
            AdmissionLimit::Identity => JigPhrase::EngineLimitIdentity,
            AdmissionLimit::Roles => JigPhrase::EngineLimitRoles,
            AdmissionLimit::Arithmetic => JigPhrase::EngineLimitArithmetic,
            AdmissionLimit::Duplicate => JigPhrase::EngineLimitDuplicate,
            AdmissionLimit::Empty => JigPhrase::EngineLimitEmpty,
            AdmissionLimit::Batch => JigPhrase::EngineLimitBatch,
            AdmissionLimit::ProjectLiveTasks => JigPhrase::EngineLimitProjectLiveTasks,
            AdmissionLimit::LiveTasks => JigPhrase::EngineLimitLiveTasks,
            AdmissionLimit::Tree => JigPhrase::EngineLimitTree,
            AdmissionLimit::Depth => JigPhrase::EngineLimitDepth,
            AdmissionLimit::Delegates => JigPhrase::EngineLimitDelegates,
            AdmissionLimit::Subscription => JigPhrase::EngineLimitSubscription,
            AdmissionLimit::Dependencies => JigPhrase::EngineLimitDependencies,
            AdmissionLimit::Cycle => JigPhrase::EngineLimitCycle,
            AdmissionLimit::Spec => JigPhrase::EngineLimitSpec,
            AdmissionLimit::Contract => JigPhrase::EngineLimitContract,
            AdmissionLimit::Authority => JigPhrase::EngineLimitAuthority,
            AdmissionLimit::Inputs => JigPhrase::EngineLimitInputs,
            AdmissionLimit::Attempts => JigPhrase::EngineLimitAttempts,
            AdmissionLimit::LiveDelegates => JigPhrase::EngineLimitLiveDelegates,
            AdmissionLimit::Restore => JigPhrase::EngineLimitRestore,
            AdmissionLimit::Read => JigPhrase::EngineLimitRead,
            AdmissionLimit::Turn => JigPhrase::EngineLimitTurn,
            AdmissionLimit::HoldKind => JigPhrase::EngineLimitHoldKind,
            AdmissionLimit::HoldTaken => JigPhrase::EngineLimitHoldTaken,
            AdmissionLimit::Holds => JigPhrase::EngineLimitHolds,
            AdmissionLimit::InboxFull => JigPhrase::EngineLimitInboxFull,
            AdmissionLimit::Notes => JigPhrase::EngineLimitNotes,
            AdmissionLimit::Revision => JigPhrase::EngineLimitRevision,
            AdmissionLimit::Question => JigPhrase::EngineLimitQuestion,
            AdmissionLimit::Priority => JigPhrase::EngineLimitPriority,
            AdmissionLimit::Policy => JigPhrase::EngineLimitPolicy,
            AdmissionLimit::Resource => JigPhrase::EngineLimitResource,
            AdmissionLimit::Service => JigPhrase::EngineLimitService,
            AdmissionLimit::Accounts => JigPhrase::EngineLimitAccounts,
            AdmissionLimit::HolderAccounts => JigPhrase::EngineLimitHolderAccounts,
            AdmissionLimit::Sources => JigPhrase::EngineLimitSources,
            AdmissionLimit::Projects => JigPhrase::EngineLimitProjects,
            AdmissionLimit::SignIns => JigPhrase::EngineLimitSignIns,
            AdmissionLimit::SettingsProposals => JigPhrase::EngineLimitSettingsProposals,
        };
        self.text(b, phrase, &[]);
    }
}
