//! English is checked data (client/domain/view.md, section 7).
use super::jig::{ALL, JigPhrase, shapes};
use alloc::boxed::Box;
use skein_lib::{List, bytes::copy_of};
use skein_ui::phrases::{
    Form, HoleKind, Invalid, Language, Phrase, PhraseLimits, Phrasebook, PluralClass, PluralRule, Segment, SpanUnit,
    WallFormat,
};
/// English words for each phrase, checked with any other loaded language.
#[must_use]
#[expect(clippy::too_many_lines, reason = "English data matches the closed phrase vocabulary whole")]
pub fn language() -> Language {
    let mut phrases = List::with_capacity(u32::try_from(ALL.len()).expect("phrase count fits"));
    for phrase in ALL {
        let segments: Box<[Segment]> = match phrase {
            JigPhrase::MovedToNewest => Box::new([
                Segment::Text(copy_of(b"Moved ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" tasks to revision ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
                Segment::Text(copy_of(b"; stayed: ")),
                Segment::Hole { index: 2, kind: HoleKind::Number },
            ]),
            JigPhrase::SettingsChanged => Box::new([
                Segment::Text(copy_of(b"Settings changed; revision ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::SettingsStale => Box::new([
                Segment::Text(copy_of(b"Settings changed; newest revision ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::SettingsFull => Box::new([
                Segment::Text(copy_of(b"Settings history is full; oldest held revision ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::SettingsFullTask => {
                Box::new([Segment::Text(copy_of(b"held by task ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::SettingsFullMore => {
                Box::new([Segment::Hole { index: 0, kind: HoleKind::Number }, Segment::Text(copy_of(b" more tasks"))])
            }
            JigPhrase::MalformedRunning => Box::new([Segment::Text(copy_of(
                b"Choose whether running work moves only when accepting a settings proposal",
            ))]),
            JigPhrase::SettingsUnchanged => {
                Box::new([Segment::Text(copy_of(b"The change leaves the settings unchanged"))])
            }
            JigPhrase::SettingCharter => {
                Box::new([Segment::Text(copy_of(b"charter ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::SettingTemplate => {
                Box::new([Segment::Text(copy_of(b"template ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::SettingPolicy => {
                Box::new([Segment::Text(copy_of(b"policy ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::SettingRules => Box::new([Segment::Text(copy_of(b"the rules"))]),
            JigPhrase::SettingFaultShape => {
                Box::new([Segment::Text(copy_of(b"Setting is out of shape or past its bound"))])
            }
            JigPhrase::SettingFaultCount => Box::new([Segment::Text(copy_of(b"Setting kind is past its count limit"))]),
            JigPhrase::SettingFaultReference => {
                Box::new([Segment::Text(copy_of(b"Setting names a charter, template, model or host not offered"))])
            }
            JigPhrase::SettingFaultRules => {
                Box::new([Segment::Text(copy_of(b"Setting policy is above the deployment rules"))])
            }
            JigPhrase::SettingFaultBudget => Box::new([Segment::Text(copy_of(
                b"Setting charter budget is below its reserve or short of one costliest completion",
            ))]),
            JigPhrase::SettingFaultGrace => {
                Box::new([Segment::Text(copy_of(b"Setting template grace is past the allowed span"))])
            }
            JigPhrase::SettingFaultOwner => {
                Box::new([Segment::Text(copy_of(b"Setting policy owner role cannot be a root requester"))])
            }

            JigPhrase::OpenProgramModel => Box::new([Segment::Text(copy_of(b"This model runs through a program"))]),
            JigPhrase::OpenProgramUnavailable => {
                Box::new([Segment::Text(copy_of(b"A chat can't stay open through a program yet"))])
            }
            JigPhrase::OpenProgramChooseModel => {
                Box::new([Segment::Text(copy_of(b"To chat now, choose a model that doesn't run through a program"))])
            }
            JigPhrase::OpenProgramSource => Box::new([
                Segment::Text(copy_of(b"Account ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" would use its program source ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
            ]),
            JigPhrase::OpenProgramNoSwitch => {
                Box::new([Segment::Text(copy_of(b"This chat does not switch to another source on its own"))])
            }
            JigPhrase::RefusedWorkerAt => Box::new([
                Segment::Text(copy_of(b"A worker was refused at ")),
                Segment::Hole { index: 0, kind: HoleKind::Wall },
            ]),
            JigPhrase::RefusedEngineAt => Box::new([
                Segment::Text(copy_of(b"An engine host was refused at ")),
                Segment::Hole { index: 0, kind: HoleKind::Wall },
            ]),
            JigPhrase::ActRename => Box::new([Segment::Text(copy_of(b"Rename"))]),
            JigPhrase::NameTaken => Box::new([Segment::Text(copy_of(b"Another project has that name"))]),
            JigPhrase::NameEmpty => Box::new([Segment::Text(copy_of(b"The project name is empty"))]),
            JigPhrase::NameLong => Box::new([Segment::Text(copy_of(b"The project name is longer than 64 bytes"))]),
            JigPhrase::NameControl => {
                Box::new([Segment::Text(copy_of(b"The project name contains a control character"))])
            }
            JigPhrase::NameSpace => Box::new([Segment::Text(copy_of(b"The project name has white space at an end"))]),
            JigPhrase::NameDigits => Box::new([Segment::Text(copy_of(b"The project name is digits alone"))]),
            JigPhrase::EngineLimitProjects => {
                Box::new([Segment::Text(copy_of(b"The deployment has no room for another project"))])
            }
            JigPhrase::MalformedEndedBy => Box::new([
                Segment::Text(copy_of(b"Requester-ended task is not an agent conversation at ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),

            JigPhrase::KeptUntil => Box::new([
                Segment::Text(copy_of(b"kept until T")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" is done")),
            ]),
            JigPhrase::DependencyRemoved => Box::new([
                Segment::Text(copy_of(b"T")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" was removed from its dependencies")),
            ]),
            JigPhrase::DependencyMet => Box::new([
                Segment::Text(copy_of(b"T")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" is done; the task can go on")),
            ]),
            JigPhrase::DependencyFailed => Box::new([
                Segment::Text(copy_of(b"dependency T")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" failed")),
            ]),
            JigPhrase::DependencyCancelled => Box::new([
                Segment::Text(copy_of(b"dependency T")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" was cancelled")),
            ]),
            JigPhrase::DependencyPending => Box::new([
                Segment::Text(copy_of(b"waiting for T")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b"")),
            ]),
            JigPhrase::DependencyDone => Box::new([
                Segment::Text(copy_of(b"T")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" is done")),
            ]),
            JigPhrase::CircleWaitsFor => Box::new([
                Segment::Text(copy_of(b"waits for T")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b"")),
            ]),
            JigPhrase::DependencyParked => Box::new([Segment::Text(copy_of(b"parked"))]),
            JigPhrase::CircleHold => Box::new([Segment::Text(copy_of(b"Held for a circle of waits"))]),
            JigPhrase::CircleLifts => {
                Box::new([Segment::Text(copy_of(b"This hold lifts on its own once the circle breaks"))])
            }
            JigPhrase::CircleRefused => Box::new([Segment::Text(copy_of(b"These tasks would wait for one another"))]),
            JigPhrase::KeptAnswer => {
                Box::new([Segment::Text(copy_of(b"This task is kept by its dependencies; answer once they are done"))])
            }
            JigPhrase::DependencyByTask => Box::new([
                Segment::Text(copy_of(b"added by T")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" at ")),
                Segment::Hole { index: 1, kind: HoleKind::Wall },
            ]),
            JigPhrase::DependencyByParty => Box::new([
                Segment::Text(copy_of(b"added by P")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" at ")),
                Segment::Hole { index: 1, kind: HoleKind::Wall },
            ]),
            JigPhrase::DependencyByDeployment => Box::new([
                Segment::Text(copy_of(b"added by deployment ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" at ")),
                Segment::Hole { index: 1, kind: HoleKind::Wall },
            ]),
            JigPhrase::Word(_) => unreachable!("public words compose their own language"),
            JigPhrase::OwnerHoldRelease => {
                Box::new([Segment::Text(copy_of(b"Release continues from the shown lifecycle; failure counts reset"))])
            }
            JigPhrase::QuestionFromTask => Box::new([
                Segment::Text(copy_of(b"Question from task ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::QuestionFromPerson => Box::new([
                Segment::Text(copy_of(b"Question from person ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::QuestionFromDeployment => Box::new([
                Segment::Text(copy_of(b"Question from deployment project ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::CachedReplies => Box::new([
                Segment::Text(copy_of(b"Cached replies ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::TasksIdle => {
                Box::new([Segment::Text(copy_of(b"Idle ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::TasksStarting => {
                Box::new([Segment::Text(copy_of(b"Starting ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::TasksBackingOff => {
                Box::new([Segment::Text(copy_of(b"Backing off ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }

            JigPhrase::PageCut => Box::new([Segment::Text(copy_of(b"This page is incomplete"))]),
            JigPhrase::NoteWriterParty => Box::new([
                Segment::Text(copy_of(b"Written by party ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::NoteWriterTask => Box::new([
                Segment::Text(copy_of(b"Written by task ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b", attempt ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
            ]),
            JigPhrase::NoteChangedNotSupplied => Box::new([Segment::Text(copy_of(b"Note changed time not supplied"))]),
            JigPhrase::TaskListsCut => Box::new([Segment::Text(copy_of(b"Some task lists are incomplete"))]),
            JigPhrase::ServedGoal => {
                Box::new([Segment::Text(copy_of(b"Serves goal ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::ResourceSegments => Box::new([Segment::Text(copy_of(b"Resource segments"))]),
            JigPhrase::LiteralGrant => Box::new([
                Segment::Text(copy_of(b"Grant kind ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" on connector ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
            ]),
            JigPhrase::LiteralGrantMissing => Box::new([
                Segment::Text(copy_of(b"Missing grant kind ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" on connector ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
            ]),
            JigPhrase::ScopedResourceNotes => Box::new([
                Segment::Text(copy_of(b"Resource notes in project ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" on connector ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
            ]),
            JigPhrase::FrameLogin => {
                Box::new([Segment::Text(copy_of(b"Login: ")), Segment::Hole { index: 0, kind: HoleKind::Name }])
            }
            JigPhrase::FrameService => Box::new([Segment::Text(copy_of(b"Service"))]),
            JigPhrase::FrameResultsSeen => Box::new([
                Segment::Text(copy_of(b"Results read through ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::FrameNeedsKinds => Box::new([
                Segment::Text(copy_of(b"Questions: ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b"; proposals: ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
                Segment::Text(copy_of(b"; escalations: ")),
                Segment::Hole { index: 2, kind: HoleKind::Number },
                Segment::Text(copy_of(b"; person tasks: ")),
                Segment::Hole { index: 3, kind: HoleKind::Number },
            ]),
            JigPhrase::FrameProjectsCut => Box::new([Segment::Text(copy_of(b"More project memberships than shown"))]),
            JigPhrase::FramePoolUnopened => Box::new([Segment::Text(copy_of(b"Pool ledger not supplied"))]),
            JigPhrase::PolicyPeriodSpend => Box::new([
                Segment::Text(copy_of(b"Period spend: ")),
                Segment::Hole { index: 0, kind: HoleKind::Amount },
            ]),
            JigPhrase::PolicyEditRole => {
                Box::new([Segment::Text(copy_of(b"Replace role ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::RequestNotKept => {
                Box::new([Segment::Text(copy_of(b"Request not kept; it was not sent. Try again."))])
            }
            JigPhrase::RequestUnconfirmed => Box::new([Segment::Text(copy_of(
                b"Party not confirmed; request was not made. Try again once connected.",
            ))]),
            JigPhrase::RequestsInFlight => Box::new([Segment::Text(copy_of(b"Too many requests in flight"))]),
            JigPhrase::RequestsDropped => Box::new([
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" requests kept from before were not sent")),
            ]),
            JigPhrase::PolicyRequests => Box::new([
                Segment::Text(copy_of(b"Request flags: ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::PolicyDecides => Box::new([
                Segment::Text(copy_of(b"Proposal decision flags: ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::PolicyRequirements => Box::new([Segment::Text(copy_of(b"Replace effect requirements"))]),
            JigPhrase::PolicyProjections => Box::new([Segment::Text(copy_of(b"Replace goal projection grants"))]),
            JigPhrase::PolicyPermissions => Box::new([Segment::Text(copy_of(b"Replace permission mappings"))]),
            JigPhrase::PolicyEffect => Box::new([
                Segment::Text(copy_of(b"Effect connector ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" kind ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
            ]),
            JigPhrase::PolicyJudge => Box::new([
                Segment::Text(copy_of(b"Judge connector ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" requirement ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
                Segment::Text(copy_of(b" parameters ")),
                Segment::Hole { index: 2, kind: HoleKind::Number },
            ]),
            JigPhrase::PolicyGuarded => Box::new([Segment::Text(copy_of(b"Verdict checked at application"))]),
            JigPhrase::PolicyObserved => Box::new([
                Segment::Text(copy_of(b"Observed verdict freshness: ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" ns")),
            ]),
            JigPhrase::PolicyMustGuard => Box::new([Segment::Text(copy_of(b"Guard is required"))]),
            JigPhrase::PolicyGuardNotRequired => Box::new([Segment::Text(copy_of(b"Guard is not required"))]),
            JigPhrase::PolicyPermission => Box::new([
                Segment::Text(copy_of(b"Connector ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" permission ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
                Segment::Text(copy_of(b" maps to role ")),
                Segment::Hole { index: 2, kind: HoleKind::Number },
            ]),

            JigPhrase::RunIdle => Box::new([Segment::Text(copy_of(b"Activation idle"))]),
            JigPhrase::RunDue => Box::new([Segment::Text(copy_of(b"Activation due"))]),
            JigPhrase::RunPreparing => Box::new([Segment::Text(copy_of(b"Preparing an activation"))]),
            JigPhrase::RunClaimed => Box::new([
                Segment::Text(copy_of(b"Claimed attempt ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::RunOwnerActive => Box::new([
                Segment::Text(copy_of(b"Active attempt ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::RunBackingOff => Box::new([
                Segment::Text(copy_of(b"Activation paused until ")),
                Segment::Hole { index: 0, kind: HoleKind::Wall },
            ]),
            JigPhrase::RunPhaseSince => {
                Box::new([Segment::Text(copy_of(b"Phase since ")), Segment::Hole { index: 0, kind: HoleKind::Wall }])
            }
            JigPhrase::RunPlacement => Box::new([
                Segment::Text(copy_of(b"Placement recorded at ")),
                Segment::Hole { index: 0, kind: HoleKind::Wall },
            ]),
            JigPhrase::RunUnobserved => Box::new([Segment::Text(copy_of(b"Run observation not supplied"))]),
            JigPhrase::RunObservedSince => Box::new([
                Segment::Text(copy_of(b"Observed work since ")),
                Segment::Hole { index: 0, kind: HoleKind::Wall },
            ]),
            JigPhrase::RunReportedAt => Box::new([
                Segment::Text(copy_of(b"Observation received at ")),
                Segment::Hole { index: 0, kind: HoleKind::Wall },
            ]),
            JigPhrase::RunActualStart => Box::new([
                Segment::Text(copy_of(b"Start observed at ")),
                Segment::Hole { index: 0, kind: HoleKind::Wall },
            ]),
            JigPhrase::RunStartUnknown => Box::new([Segment::Text(copy_of(b"Start observation not supplied"))]),
            JigPhrase::WorkPreparing => Box::new([Segment::Text(copy_of(b"Observed work: preparing"))]),
            JigPhrase::WorkStarting => Box::new([Segment::Text(copy_of(b"Observed work: starting"))]),
            JigPhrase::WorkActive => Box::new([Segment::Text(copy_of(b"Observed work: active"))]),
            JigPhrase::WorkWriting => Box::new([Segment::Text(copy_of(b"Observed work: writing"))]),
            JigPhrase::WorkTool => Box::new([
                Segment::Text(copy_of(b"Observed tool ")),
                Segment::Hole { index: 0, kind: HoleKind::Name },
                Segment::Text(copy_of(b", call ")),
                Segment::Hole { index: 1, kind: HoleKind::Name },
            ]),
            JigPhrase::WorkWaiting => Box::new([Segment::Text(copy_of(b"Observed work: waiting"))]),
            JigPhrase::WorkEnding => Box::new([Segment::Text(copy_of(b"Observed work: ending"))]),
            JigPhrase::RunRetryFailed => Box::new([
                Segment::Text(copy_of(b"Failure count ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" of allowance ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
            ]),
            JigPhrase::RunRetryRefused => Box::new([
                Segment::Text(copy_of(b"Activation refusal count ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::ClosingRun => Box::new([
                Segment::Text(copy_of(b"Waiting for attempt ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::ClosingDelegates => Box::new([Segment::Text(copy_of(b"Waiting for delegates"))]),
            JigPhrase::ClosingStageEffects => Box::new([Segment::Text(copy_of(b"Waiting for effects to settle"))]),
            JigPhrase::ClosingReleases => Box::new([Segment::Text(copy_of(b"Waiting for resource releases"))]),
            JigPhrase::ClosingSettled => Box::new([Segment::Text(copy_of(b"Closing obligations settled"))]),
            JigPhrase::ClosingStageUnknown => Box::new([Segment::Text(copy_of(b"Closing stage not supplied"))]),
            JigPhrase::ClosingConnectorsUnknown => Box::new([Segment::Text(copy_of(b"Connector waits not supplied"))]),
            JigPhrase::ClosingConnectorsEmpty => Box::new([Segment::Text(copy_of(b"No connector waits"))]),
            JigPhrase::ClosingPendingEnding => Box::new([Segment::Text(copy_of(b"Pending ending"))]),
            JigPhrase::HeldPriorLifecycle => Box::new([Segment::Text(copy_of(b"Prior lifecycle before the hold"))]),
            JigPhrase::HeldPriorWaiting => Box::new([Segment::Text(copy_of(b"Prior lifecycle: waiting"))]),

            JigPhrase::SpecificationNumber => Box::new([
                Segment::Text(copy_of(b"Parameter ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" has value ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
            ]),
            JigPhrase::SpecificationBytes => {
                Box::new([Segment::Text(copy_of(b"Parameter ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::SpecificationResource => Box::new([
                Segment::Text(copy_of(b"Parameter ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" names connector ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
                Segment::Text(copy_of(b" resource ")),
                Segment::Hole { index: 2, kind: HoleKind::Number },
            ]),
            JigPhrase::SpecificationInput => {
                Box::new([Segment::Text(copy_of(b"Input task ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::SpecificationCut => Box::new([Segment::Text(copy_of(b"The specification is incomplete"))]),

            JigPhrase::FailureRun => Box::new([Segment::Text(copy_of(b"Executor reported failure"))]),
            JigPhrase::FailureAgent => Box::new([Segment::Text(copy_of(b"Agent failed"))]),
            JigPhrase::FailureInvalid => Box::new([Segment::Text(copy_of(b"Result broke its contract"))]),

            JigPhrase::RecurringSkip => {
                Box::new([Segment::Text(copy_of(b"If the preceding batch is still live, skip this period"))])
            }
            JigPhrase::RecurringWait => {
                Box::new([Segment::Text(copy_of(b"If the preceding batch is still live, wait for it to end"))])
            }
            JigPhrase::RecurringCut => {
                Box::new([Segment::Text(copy_of(b"Some recurring template facts were omitted"))])
            }
            JigPhrase::RecurringKey => Box::new([
                Segment::Text(copy_of(b"Recurring template ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::RecurringLastPeriod => Box::new([
                Segment::Text(copy_of(b"Last batch period ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::RecurringPendingPeriod => Box::new([
                Segment::Text(copy_of(b"Pending batch period ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::PanelPart => Box::new([Segment::Text(copy_of(b"Task panel"))]),
            JigPhrase::ResultPart => Box::new([Segment::Text(copy_of(b"Connector result"))]),
            JigPhrase::HoldPart => Box::new([Segment::Text(copy_of(b"Connector hold"))]),

            JigPhrase::EngineLimitGoals => Box::new([Segment::Text(copy_of(b"Engine admission limit: goals"))]),

            JigPhrase::MalformedTwoAsAllowed => Box::new([
                Segment::Text(copy_of(b"Two AsAllowed budgets at ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::MalformedConversation => Box::new([
                Segment::Text(copy_of(b"Conversation task is not an agent at ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::MalformedProposedAsAllowed => Box::new([
                Segment::Text(copy_of(b"Proposed AsAllowed budget at ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::MalformedCycle => Box::new([
                Segment::Text(copy_of(b"Dependency cycle at ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::EngineLimitOperations => {
                Box::new([Segment::Text(copy_of(b"Engine admission limit: operations"))])
            }
            JigPhrase::MalformedProject => Box::new([
                Segment::Text(copy_of(b"Operation in another project at ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::EngineLimitGrace => Box::new([Segment::Text(copy_of(b"Engine admission limit: grace"))]),
            JigPhrase::EngineLimitWords => Box::new([Segment::Text(copy_of(b"Engine admission limit: words"))]),
            JigPhrase::EngineLimitIdentity => Box::new([Segment::Text(copy_of(b"Engine admission limit: identity"))]),
            JigPhrase::EngineLimitRoles => Box::new([Segment::Text(copy_of(b"Engine admission limit: roles"))]),
            JigPhrase::EngineLimitArithmetic => {
                Box::new([Segment::Text(copy_of(b"Engine admission limit: arithmetic"))])
            }
            JigPhrase::EngineLimitDuplicate => Box::new([Segment::Text(copy_of(b"Engine admission limit: duplicate"))]),
            JigPhrase::EngineLimitEmpty => Box::new([Segment::Text(copy_of(b"Engine admission limit: empty"))]),
            JigPhrase::EngineLimitBatch => Box::new([Segment::Text(copy_of(b"Engine admission limit: batch"))]),
            JigPhrase::EngineLimitProjectLiveTasks => {
                Box::new([Segment::Text(copy_of(b"Engine admission limit: project live tasks"))])
            }
            JigPhrase::EngineLimitLiveTasks => {
                Box::new([Segment::Text(copy_of(b"Engine admission limit: live tasks"))])
            }
            JigPhrase::EngineLimitTree => Box::new([Segment::Text(copy_of(b"Engine admission limit: tree"))]),
            JigPhrase::EngineLimitDepth => Box::new([Segment::Text(copy_of(b"Engine admission limit: depth"))]),
            JigPhrase::EngineLimitDelegates => Box::new([Segment::Text(copy_of(b"Engine admission limit: delegates"))]),
            JigPhrase::EngineLimitSubscription => {
                Box::new([Segment::Text(copy_of(b"Engine admission limit: subscription"))])
            }
            JigPhrase::EngineLimitDependencies => {
                Box::new([Segment::Text(copy_of(b"Engine admission limit: dependencies"))])
            }
            JigPhrase::EngineLimitCycle => Box::new([Segment::Text(copy_of(b"Engine admission limit: cycle"))]),
            JigPhrase::EngineLimitSpec => Box::new([Segment::Text(copy_of(b"Engine admission limit: spec"))]),
            JigPhrase::EngineLimitContract => Box::new([Segment::Text(copy_of(b"Engine admission limit: contract"))]),
            JigPhrase::EngineLimitAuthority => Box::new([Segment::Text(copy_of(b"Engine admission limit: authority"))]),
            JigPhrase::EngineLimitInputs => Box::new([Segment::Text(copy_of(b"Engine admission limit: inputs"))]),
            JigPhrase::EngineLimitAttempts => Box::new([Segment::Text(copy_of(b"Engine admission limit: attempts"))]),
            JigPhrase::EngineLimitLiveDelegates => {
                Box::new([Segment::Text(copy_of(b"Engine admission limit: live delegates"))])
            }
            JigPhrase::EngineLimitRestore => Box::new([Segment::Text(copy_of(b"Engine admission limit: restore"))]),
            JigPhrase::EngineLimitRead => Box::new([Segment::Text(copy_of(b"Engine admission limit: read"))]),
            JigPhrase::EngineLimitTurn => Box::new([Segment::Text(copy_of(b"Engine admission limit: turn"))]),
            JigPhrase::EngineLimitHoldKind => Box::new([Segment::Text(copy_of(b"Engine admission limit: hold kind"))]),
            JigPhrase::EngineLimitHoldTaken => {
                Box::new([Segment::Text(copy_of(b"Engine admission limit: hold taken"))])
            }
            JigPhrase::EngineLimitHolds => Box::new([Segment::Text(copy_of(b"Engine admission limit: holds"))]),
            JigPhrase::EngineLimitInboxFull => {
                Box::new([Segment::Text(copy_of(b"Engine admission limit: inbox full"))])
            }
            JigPhrase::EngineLimitNotes => Box::new([Segment::Text(copy_of(b"Engine admission limit: notes"))]),
            JigPhrase::EngineLimitRevision => Box::new([Segment::Text(copy_of(b"Engine admission limit: revision"))]),
            JigPhrase::EngineLimitQuestion => Box::new([Segment::Text(copy_of(b"Engine admission limit: question"))]),
            JigPhrase::EngineLimitPriority => Box::new([Segment::Text(copy_of(b"Engine admission limit: priority"))]),
            JigPhrase::EngineLimitPolicy => Box::new([Segment::Text(copy_of(b"Engine admission limit: policy"))]),
            JigPhrase::EngineLimitResource => Box::new([Segment::Text(copy_of(b"Engine admission limit: resource"))]),
            JigPhrase::EngineLimitService => Box::new([Segment::Text(copy_of(b"Engine admission limit: service"))]),
            JigPhrase::EngineLimitAccounts => Box::new([Segment::Text(copy_of(b"Engine admission limit: accounts"))]),
            JigPhrase::EngineLimitHolderAccounts => {
                Box::new([Segment::Text(copy_of(b"Engine admission limit: holder accounts"))])
            }
            JigPhrase::EngineLimitSources => Box::new([Segment::Text(copy_of(b"Engine admission limit: sources"))]),
            JigPhrase::EngineLimitPlacedHolds => {
                Box::new([Segment::Text(copy_of(b"Engine admission limit: placed holds"))])
            }
            JigPhrase::EngineLimitProjectPlacedHolds => {
                Box::new([Segment::Text(copy_of(b"Engine admission limit: project placed holds"))])
            }
            JigPhrase::EngineLimitPartyPlacedHolds => {
                Box::new([Segment::Text(copy_of(b"Engine admission limit: party placed holds"))])
            }
            JigPhrase::EngineLimitExemptions => {
                Box::new([Segment::Text(copy_of(b"Engine admission limit: exemptions"))])
            }
            JigPhrase::EngineLimitSettingsProposals => {
                Box::new([Segment::Text(copy_of(b"Engine admission limit: pending settings proposals"))])
            }
            JigPhrase::EngineLimitSignIns => {
                Box::new([Segment::Text(copy_of(b"Engine admission limit: account sign-ins"))])
            }
            JigPhrase::DecidedBare => Box::new([Segment::Text(copy_of(b"Decided"))]),
            JigPhrase::DeletedEnding => Box::new([Segment::Text(copy_of(b"Deleted"))]),
            JigPhrase::ByProjectDeployment => Box::new([
                Segment::Text(copy_of(b"By deployment in project ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),

            JigPhrase::NoHeldRole => Box::new([Segment::Text(copy_of(b"No held role was supplied"))]),
            JigPhrase::HeldOwner => Box::new([Segment::Text(copy_of(b"Held role: owner"))]),
            JigPhrase::HeldMaintainer => Box::new([Segment::Text(copy_of(b"Held role: maintainer"))]),
            JigPhrase::HeldMember => Box::new([Segment::Text(copy_of(b"Held role: member"))]),
            JigPhrase::HeldObserver => Box::new([Segment::Text(copy_of(b"Held role: observer"))]),
            JigPhrase::HeldPolicy => Box::new([
                Segment::Text(copy_of(b"Held policy role ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::DisposalMissing => Box::new([
                Segment::Text(copy_of(b"Task ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" needs a disposal")),
            ]),
            JigPhrase::DisposalUnknown => Box::new([
                Segment::Text(copy_of(b"Task ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" is not a named live delegate")),
            ]),
            JigPhrase::DisposalRepeated => Box::new([
                Segment::Text(copy_of(b"Task ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" has repeated disposals")),
            ]),

            JigPhrase::GrantKindMissing => Box::new([
                Segment::Text(copy_of(b"No grant covers operation kind ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::FundingMissing => {
                Box::new([Segment::Text(copy_of(b"No valid funding relation supports this request"))])
            }
            JigPhrase::DepthMaximum => Box::new([
                Segment::Text(copy_of(b"Permitted descendant depth: ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::DeadlineAuthorityMissing => {
                Box::new([Segment::Text(copy_of(b"The deadline is past or later than permitted"))])
            }
            JigPhrase::TasksLeft => Box::new([
                Segment::Text(copy_of(b"Lifetime tasks left: ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::ToolMissing => Box::new([Segment::Text(copy_of(b"The tool family is absent"))]),
            JigPhrase::ScopeMissing => Box::new([Segment::Text(copy_of(b"The note scope is absent"))]),
            JigPhrase::ModelMissing => Box::new([Segment::Text(copy_of(b"A requested model is absent"))]),
            JigPhrase::ReferenceMissing => {
                Box::new([Segment::Text(copy_of(b"No verified reference permits this action"))])
            }
            JigPhrase::ResourceAccessMissing => {
                Box::new([Segment::Text(copy_of(b"Current resource access forbids this action"))])
            }
            JigPhrase::ResourceUnavailable => {
                Box::new([Segment::Text(copy_of(b"A required connector resource is unavailable"))])
            }
            JigPhrase::AuthorityShapeMissing => {
                Box::new([Segment::Text(copy_of(b"The authority value has an invalid shape"))])
            }
            JigPhrase::AgainRequired => {
                Box::new([Segment::Text(copy_of(b"An unrecoverable effect choice is required"))])
            }
            JigPhrase::AgainUnexpected => {
                Box::new([Segment::Text(copy_of(b"An unrecoverable effect choice was unexpected"))])
            }
            JigPhrase::AccountMissing => Box::new([Segment::Text(copy_of(b"A required model account is unusable"))]),
            JigPhrase::WriterMissing => Box::new([Segment::Text(copy_of(b"A written resource is held elsewhere"))]),
            JigPhrase::RequiredJudgeMissing => Box::new([
                Segment::Text(copy_of(b"Connector ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b", kind ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
                Segment::Text(copy_of(b": a required judge has no usable fact")),
            ]),
            JigPhrase::FailedJudge => Box::new([
                Segment::Text(copy_of(b"Connector ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b", kind ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
                Segment::Text(copy_of(b": a required judge refused the effect")),
            ]),
            JigPhrase::UnguardedJudge => Box::new([
                Segment::Text(copy_of(b"Connector ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b", kind ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
                Segment::Text(copy_of(b": the effect cannot guard a required fact")),
            ]),
            JigPhrase::PartConnector => {
                Box::new([Segment::Text(copy_of(b"Connector ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::PartResource => {
                Box::new([Segment::Text(copy_of(b"Resource: ")), Segment::Hole { index: 0, kind: HoleKind::Name }])
            }
            JigPhrase::PartKind => {
                Box::new([Segment::Text(copy_of(b"Kind ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::PartProject => {
                Box::new([Segment::Text(copy_of(b"Project ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::OutboxEntry => {
                Box::new([Segment::Text(copy_of(b"Outbox entry ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::PartProposal => Box::new([
                Segment::Text(copy_of(b"Proposal ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b", number ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
            ]),

            JigPhrase::ConversationLog => Box::new([Segment::Text(copy_of(b"Conversation"))]),
            JigPhrase::WordsBeingSent => Box::new([Segment::Text(copy_of(b"Being sent"))]),
            JigPhrase::WordsSent => Box::new([Segment::Text(copy_of(b"Sent"))]),
            JigPhrase::WordsRead => Box::new([Segment::Text(copy_of(b"Read"))]),
            JigPhrase::RunResumed => Box::new([Segment::Text(copy_of(b"Resumed its conversation"))]),
            JigPhrase::RunFreshTail => Box::new([
                Segment::Text(copy_of(b"Started fresh; may have lost earlier detail. Tail events: ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::RunFinished => Box::new([Segment::Text(copy_of(b"Run finished"))]),
            JigPhrase::RunCancelled => Box::new([Segment::Text(copy_of(b"Run cancelled"))]),
            JigPhrase::TurnInput => Box::new([
                Segment::Text(copy_of(b"Input tokens: ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::TurnOutput => Box::new([
                Segment::Text(copy_of(b"Output tokens: ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::CacheRead => Box::new([
                Segment::Text(copy_of(b"Cache tokens read: ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::CacheWritten => Box::new([
                Segment::Text(copy_of(b"Cache tokens written: ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::TurnSpent => {
                Box::new([Segment::Text(copy_of(b"Turn spent: ")), Segment::Hole { index: 0, kind: HoleKind::Amount }])
            }
            JigPhrase::Provisional => Box::new([Segment::Text(copy_of(b"Provisional"))]),
            JigPhrase::TurnUnreadVersion => Box::new([
                Segment::Text(copy_of(b"Turn could not be read: transcript version ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::TurnUnreadMalformed => {
                Box::new([Segment::Text(copy_of(b"Turn could not be read: malformed transcript"))])
            }
            JigPhrase::TurnUnreadTooLarge => {
                Box::new([Segment::Text(copy_of(b"Turn could not be read: exceeds client bounds"))])
            }
            JigPhrase::TurnCut => Box::new([Segment::Text(copy_of(b"Some turn content did not fit"))]),
            JigPhrase::OpaqueDialect => Box::new([
                Segment::Text(copy_of(b"Uninterpreted block: ")),
                Segment::Hole { index: 0, kind: HoleKind::Name },
            ]),
            JigPhrase::OpaqueBytes => Box::new([
                Segment::Text(copy_of(b"Opaque bytes: ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::CallNumber => {
                Box::new([Segment::Text(copy_of(b"Call ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::OpenCall => Box::new([
                Segment::Text(copy_of(b"Open input and result for ")),
                Segment::Hole { index: 0, kind: HoleKind::Name },
            ]),
            JigPhrase::FoldCall => Box::new([
                Segment::Text(copy_of(b"Fold input and result for ")),
                Segment::Hole { index: 0, kind: HoleKind::Name },
            ]),
            JigPhrase::CallFailed => Box::new([Segment::Text(copy_of(b"Call failed"))]),
            JigPhrase::CallCompleted => Box::new([Segment::Text(copy_of(b"Call completed"))]),
            JigPhrase::CallPending => Box::new([Segment::Text(copy_of(b"Call in progress"))]),
            JigPhrase::NewsTopic => {
                Box::new([Segment::Text(copy_of(b"News topic ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::TurnNumber => {
                Box::new([Segment::Text(copy_of(b"Turn ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::TurnInstructions => Box::new([Segment::Text(copy_of(b"Instructions"))]),

            JigPhrase::ProposalsWaiting => Box::new([
                Segment::Text(copy_of(b"Pending proposals: ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),

            JigPhrase::ResourceScope => Box::new([
                Segment::Text(copy_of(b"Resource scope: ")),
                Segment::Hole { index: 0, kind: HoleKind::Name },
            ]),
            JigPhrase::ResultExpected => Box::new([Segment::Text(copy_of(b"Result expected"))]),
            JigPhrase::ReportExpected => Box::new([Segment::Text(copy_of(b"A report"))]),
            JigPhrase::WordsExpected => Box::new([Segment::Text(copy_of(b"Words"))]),
            JigPhrase::ApprovalExpected => Box::new([Segment::Text(copy_of(b"Approval of the named state"))]),

            JigPhrase::Overview => Box::new([Segment::Text(copy_of(b"Overview"))]),
            JigPhrase::Activity => Box::new([Segment::Text(copy_of(b"Activity"))]),
            JigPhrase::Plan => Box::new([Segment::Text(copy_of(b"Plan"))]),
            JigPhrase::History => Box::new([Segment::Text(copy_of(b"History"))]),
            JigPhrase::BudgetAuthority => Box::new([Segment::Text(copy_of(b"Budget and authority"))]),
            JigPhrase::Panels => Box::new([Segment::Text(copy_of(b"Connectors"))]),
            JigPhrase::Goal => Box::new([Segment::Text(copy_of(b"Goal"))]),
            JigPhrase::Resources => Box::new([Segment::Text(copy_of(b"Resources"))]),
            JigPhrase::Policy => Box::new([Segment::Text(copy_of(b"Policy"))]),
            JigPhrase::Roles => Box::new([Segment::Text(copy_of(b"Roles and services"))]),
            JigPhrase::Spend => Box::new([Segment::Text(copy_of(b"Spend"))]),
            JigPhrase::CurrentPeriod => Box::new([
                Segment::Text(copy_of(b"Current period ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::PeriodBegan => Box::new([
                Segment::Text(copy_of(b"Period began at ")),
                Segment::Hole { index: 0, kind: HoleKind::Wall },
            ]),
            JigPhrase::NextRun => {
                Box::new([Segment::Text(copy_of(b"Next runs at ")), Segment::Hole { index: 0, kind: HoleKind::Wall }])
            }
            JigPhrase::HoldRevision => Box::new([
                Segment::Text(copy_of(b"Hold revision ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::WaitingRoles => Box::new([
                Segment::Text(copy_of(b"Waiting at policy roles in project ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::NextPeriod => Box::new([Segment::Text(copy_of(b"Runs at the next period"))]),
            JigPhrase::Recurring => Box::new([Segment::Text(copy_of(b"Recurring tasks"))]),
            JigPhrase::Settings => Box::new([Segment::Text(copy_of(b"Settings"))]),
            JigPhrase::Goals => Box::new([Segment::Text(copy_of(b"Goals"))]),
            JigPhrase::Tasks => Box::new([Segment::Text(copy_of(b"Tasks"))]),
            JigPhrase::Notes => Box::new([Segment::Text(copy_of(b"Notes"))]),
            JigPhrase::Reserved => {
                Box::new([Segment::Text(copy_of(b"Reserved: ")), Segment::Hole { index: 0, kind: HoleKind::Amount }])
            }
            JigPhrase::Left => {
                Box::new([Segment::Text(copy_of(b"Left: ")), Segment::Hole { index: 0, kind: HoleKind::Amount }])
            }
            JigPhrase::TotalBudget => Box::new([
                Segment::Text(copy_of(b"Total budget: ")),
                Segment::Hole { index: 0, kind: HoleKind::Amount },
            ]),
            JigPhrase::OwnBudget => {
                Box::new([Segment::Text(copy_of(b"Own spend: ")), Segment::Hole { index: 0, kind: HoleKind::Amount }])
            }
            JigPhrase::EndedBudget => Box::new([
                Segment::Text(copy_of(b"Ended delegate spend: ")),
                Segment::Hole { index: 0, kind: HoleKind::Amount },
            ]),
            JigPhrase::HeldBelow => {
                Box::new([Segment::Text(copy_of(b"Held below: ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::FoldSubtree => Box::new([Segment::Text(copy_of(b"Fold subtree"))]),
            JigPhrase::OpenSubtree => Box::new([Segment::Text(copy_of(b"Open subtree"))]),
            JigPhrase::AllPlan => Box::new([Segment::Text(copy_of(b"All plan tasks"))]),
            JigPhrase::HeldPlan => Box::new([Segment::Text(copy_of(b"Held only"))]),
            JigPhrase::LivePlan => Box::new([Segment::Text(copy_of(b"Live only"))]),
            JigPhrase::OwnHistory => Box::new([Segment::Text(copy_of(b"This task only"))]),
            JigPhrase::SubtreeHistory => Box::new([Segment::Text(copy_of(b"This task and its subtree"))]),
            JigPhrase::BatchMade => Box::new([
                Segment::Text(copy_of(b"Made in batch ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::MovedWork => Box::new([Segment::Text(copy_of(b"Requester changed"))]),
            JigPhrase::GoalWords => Box::new([Segment::Text(copy_of(b"Goal specification"))]),
            JigPhrase::GoalBudget => Box::new([Segment::Text(copy_of(b"Goal budget"))]),
            JigPhrase::GoalDeadline => Box::new([Segment::Text(copy_of(b"Goal deadline (UTC, YYYY-MM-DD HH:MM)"))]),
            JigPhrase::GoalPriority => Box::new([Segment::Text(copy_of(b"Goal priority"))]),
            JigPhrase::PriorityMeaning => {
                Box::new([Segment::Text(copy_of(b"Priority orders work competing for one resource"))])
            }
            JigPhrase::PoolLeft => Box::new([
                Segment::Text(copy_of(b"Your pool has left: ")),
                Segment::Hole { index: 0, kind: HoleKind::Amount },
            ]),
            JigPhrase::GoalPropose => Box::new([Segment::Text(copy_of(b"Beyond your pool, this will be proposed"))]),
            JigPhrase::NoteSearch => Box::new([Segment::Text(copy_of(b"Search notes by description"))]),
            JigPhrase::AskedByMe => Box::new([Segment::Text(copy_of(b"Tasks I asked for"))]),
            JigPhrase::AllTasks => Box::new([Segment::Text(copy_of(b"All tasks"))]),
            JigPhrase::Period => {
                Box::new([Segment::Text(copy_of(b"Period ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::RoleNumber => {
                Box::new([Segment::Text(copy_of(b"Role ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::PartyNumber => {
                Box::new([Segment::Text(copy_of(b"Party ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::Allotment => {
                Box::new([Segment::Text(copy_of(b"Allotment: ")), Segment::Hole { index: 0, kind: HoleKind::Amount }])
            }
            JigPhrase::DecidesKind => {
                Box::new([Segment::Text(copy_of(b"Decides kind ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::PolicySpend => Box::new([
                Segment::Text(copy_of(b"Spend per period: ")),
                Segment::Hole { index: 0, kind: HoleKind::Amount },
            ]),
            JigPhrase::HostNumber => {
                Box::new([Segment::Text(copy_of(b"Host ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::HostConnected => Box::new([Segment::Text(copy_of(b"Connected"))]),
            JigPhrase::HostDisconnected => Box::new([Segment::Text(copy_of(b"Lost"))]),
            JigPhrase::SlotsUsed => {
                Box::new([Segment::Text(copy_of(b"Slots used: ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::SlotsFree => {
                Box::new([Segment::Text(copy_of(b"Slots free: ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::AccountNumber => {
                Box::new([Segment::Text(copy_of(b"LLM account ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::AccountUsable => Box::new([Segment::Text(copy_of(b"Usable"))]),
            JigPhrase::AccountUnavailable => Box::new([Segment::Text(copy_of(b"Unavailable"))]),
            JigPhrase::Capacity => Box::new([Segment::Text(copy_of(b"Capacity"))]),
            JigPhrase::TasksLive => {
                Box::new([Segment::Text(copy_of(b"Live tasks: ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::CapacityLimit => {
                Box::new([Segment::Text(copy_of(b"Task limit: ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::Grace => Box::new([
                Segment::Text(copy_of(b"Grace holds until ")),
                Segment::Hole { index: 0, kind: HoleKind::Wall },
            ]),
            JigPhrase::WaitingAccount => Box::new([Segment::Text(copy_of(b"Tasks waiting for this account"))]),

            JigPhrase::OwnSpend => {
                Box::new([Segment::Text(copy_of(b"Agent spend: ")), Segment::Hole { index: 0, kind: HoleKind::Amount }])
            }
            JigPhrase::DelegateSpend => Box::new([
                Segment::Text(copy_of(b"Delegate spend: ")),
                Segment::Hole { index: 0, kind: HoleKind::Amount },
            ]),

            JigPhrase::NeedsYou => Box::new([Segment::Text(copy_of(b"Needs you"))]),
            JigPhrase::InboxUpdates => Box::new([Segment::Text(copy_of(b"Updates"))]),
            JigPhrase::AllKinds => Box::new([Segment::Text(copy_of(b"All kinds"))]),
            JigPhrase::AllGoals => Box::new([Segment::Text(copy_of(b"All goals"))]),
            JigPhrase::OnlyGoal => {
                Box::new([Segment::Text(copy_of(b"Goal ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::NewUpdate => Box::new([Segment::Text(copy_of(b"New"))]),
            JigPhrase::OnCall => Box::new([Segment::Text(copy_of(b"For the person on call"))]),
            JigPhrase::PassesUp => {
                Box::new([Segment::Text(copy_of(b"Passes up at ")), Segment::Hole { index: 0, kind: HoleKind::Wall }])
            }
            JigPhrase::ReadEarlier => Box::new([Segment::Text(copy_of(b"Read earlier"))]),
            JigPhrase::ReadLater => Box::new([Segment::Text(copy_of(b"Read later"))]),
            JigPhrase::WindowStart => Box::new([Segment::Text(copy_of(b"Start of the list"))]),
            JigPhrase::WindowEnd => Box::new([Segment::Text(copy_of(b"End of the list"))]),
            JigPhrase::MoreRows => Box::new([Segment::Text(copy_of(b"More entries are available"))]),
            JigPhrase::NewChat => Box::new([Segment::Text(copy_of(b"First words"))]),
            JigPhrase::BeginConversation => Box::new([Segment::Text(copy_of(b"Start a conversation"))]),
            JigPhrase::Delegates => Box::new([
                Segment::Text(copy_of(b"Live delegates: ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::SignInPage => Box::new([Segment::Text(copy_of(b"Sign in"))]),
            JigPhrase::SignInWith => {
                Box::new([Segment::Text(copy_of(b"Sign in with ")), Segment::Hole { index: 0, kind: HoleKind::Name }])
            }
            JigPhrase::MissingPage => Box::new([Segment::Text(copy_of(b"Page missing"))]),
            JigPhrase::UpdateClient => Box::new([Segment::Text(copy_of(
                b"Client out of date: update or reload this client to continue; pending requests are kept",
            ))]),
            JigPhrase::SomeoneElse => {
                Box::new([Segment::Text(copy_of(b"Someone else's work: sign in as its owner to continue"))])
            }

            JigPhrase::LoadingNavigation => Box::new([Segment::Text(copy_of(b"Loading navigation"))]),
            JigPhrase::ByNamed => {
                Box::new([Segment::Text(copy_of(b"By ")), Segment::Hole { index: 0, kind: HoleKind::Name }])
            }
            JigPhrase::Refused => Box::new([Segment::Text(copy_of(b"Refused"))]),
            JigPhrase::Leaving => Box::new([Segment::Text(copy_of(b"Leaving"))]),
            JigPhrase::Live => Box::new([Segment::Text(copy_of(b"Live"))]),
            JigPhrase::Offline => Box::new([Segment::Text(copy_of(b"Offline"))]),
            JigPhrase::Starting => Box::new([Segment::Text(copy_of(b"Connecting"))]),
            JigPhrase::Behind => Box::new([Segment::Text(copy_of(b"Behind: waiting for current facts"))]),
            JigPhrase::Needs => {
                Box::new([Segment::Hole { index: 0, kind: HoleKind::Number }, Segment::Text(copy_of(b" need you"))])
            }
            JigPhrase::MoreNeeds => Box::new([
                Segment::Text(copy_of(b"More than ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" need you")),
            ]),
            JigPhrase::Updates => Box::new([
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" updates not read")),
            ]),
            JigPhrase::MoreUpdates => Box::new([
                Segment::Text(copy_of(b"More than ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" updates not read")),
            ]),
            JigPhrase::SignedIn => {
                Box::new([Segment::Text(copy_of(b"Signed in as ")), Segment::Hole { index: 0, kind: HoleKind::Name }])
            }
            JigPhrase::SignOutLost => Box::new([Segment::Text(copy_of(b"Not signed out: the engine was not reached"))]),
            JigPhrase::SignOut => Box::new([Segment::Text(copy_of(b"Sign out"))]),
            JigPhrase::SignedOut => Box::new([Segment::Text(copy_of(b"Signed out"))]),
            JigPhrase::NothingKnown => Box::new([Segment::Text(copy_of(b"Nothing known yet"))]),
            JigPhrase::LinkPart => Box::new([Segment::Text(copy_of(b"Connection"))]),
            JigPhrase::NeedsPart => Box::new([Segment::Text(copy_of(b"What needs you"))]),
            JigPhrase::PartyPart => Box::new([Segment::Text(copy_of(b"Who is signed in"))]),
            JigPhrase::ProjectsPart => Box::new([Segment::Text(copy_of(b"Projects"))]),
            JigPhrase::GoToTaskPart => Box::new([Segment::Text(copy_of(b"Go to a task"))]),
            JigPhrase::NoticesPart => Box::new([Segment::Text(copy_of(b"Notices"))]),
            JigPhrase::Me => Box::new([Segment::Text(copy_of(b"Work"))]),
            JigPhrase::Navigation => Box::new([Segment::Text(copy_of(b"Navigation"))]),
            JigPhrase::Project => {
                Box::new([Segment::Text(copy_of(b"Project: ")), Segment::Hole { index: 0, kind: HoleKind::Name }])
            }
            JigPhrase::AllProjects => Box::new([Segment::Text(copy_of(b"All projects"))]),
            JigPhrase::TaskNumber => Box::new([Segment::Text(copy_of(b"Go to task number"))]),
            JigPhrase::Inbox => Box::new([Segment::Text(copy_of(b"Inbox"))]),
            JigPhrase::Conversations => Box::new([Segment::Text(copy_of(b"Conversations"))]),
            JigPhrase::ProjectPage => Box::new([Segment::Text(copy_of(b"Project"))]),
            JigPhrase::Deployment => Box::new([Segment::Text(copy_of(b"Deployment"))]),
            JigPhrase::PartTask | JigPhrase::TaskPage | JigPhrase::ContextTask => {
                Box::new([Segment::Text(copy_of(b"Task ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::Open => Box::new([Segment::Text(copy_of(b"Open"))]),
            JigPhrase::Confirming => Box::new([Segment::Text(copy_of(b"Being decided: review the confirmation"))]),
            JigPhrase::InFlight => Box::new([Segment::Text(copy_of(b"Being decided: waiting for the engine"))]),

            JigPhrase::Parked => {
                Box::new([Segment::Text(copy_of(b"Being decided: waiting for the engine connection or your sign-in"))])
            }
            JigPhrase::Unknown => {
                Box::new([Segment::Text(copy_of(b"Outcome unknown: inspect the current object before acting again"))])
            }
            JigPhrase::MoveGoalUp => Box::new([Segment::Text(copy_of(b"Move up"))]),
            JigPhrase::MoveGoalDown => Box::new([Segment::Text(copy_of(b"Move down"))]),
            JigPhrase::ProposeInstead => Box::new([Segment::Text(copy_of(b"Propose instead"))]),
            JigPhrase::NoGoalTemplate => {
                Box::new([Segment::Text(copy_of(b"No goal template is offered in this project"))])
            }
            JigPhrase::LookAgain => Box::new([Segment::Text(copy_of(b"Inspect current object"))]),
            JigPhrase::Looking => Box::new([Segment::Text(copy_of(b"Finding how this ended"))]),
            JigPhrase::GoneUnknown => {
                Box::new([Segment::Text(copy_of(b"Ended; its final facts are no longer available"))])
            }
            JigPhrase::Changed => Box::new([Segment::Text(copy_of(b"Changed"))]),
            JigPhrase::ProposedByParty => Box::new([
                Segment::Text(copy_of(b"Proposed by party ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b", proposal ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
            ]),
            JigPhrase::PartOwnerProposal => Box::new([
                Segment::Text(copy_of(b"Connector proposal ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::PartProposalTask => Box::new([
                Segment::Text(copy_of(b"Proposal by task ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b", number ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
            ]),
            JigPhrase::PartProposalParty => Box::new([
                Segment::Text(copy_of(b"Proposal by party ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b", number ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
            ]),
            JigPhrase::Proposed => Box::new([
                Segment::Text(copy_of(b"Proposed by task ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b", proposal ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
            ]),

            JigPhrase::Failed => {
                Box::new([Segment::Text(copy_of(b"Failed: ")), Segment::Hole { index: 0, kind: HoleKind::Name }])
            }
            JigPhrase::Cancelled => {
                Box::new([Segment::Text(copy_of(b"Cancelled: ")), Segment::Hole { index: 0, kind: HoleKind::Name }])
            }
            JigPhrase::Withdrawn => {
                Box::new([Segment::Text(copy_of(b"Withdrawn: ")), Segment::Hole { index: 0, kind: HoleKind::Name }])
            }
            JigPhrase::ByParty => {
                Box::new([Segment::Text(copy_of(b"By party ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::ByTask => {
                Box::new([Segment::Text(copy_of(b"By task ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::ByDeployment => Box::new([Segment::Text(copy_of(b"By the deployment"))]),
            JigPhrase::At => {
                Box::new([Segment::Text(copy_of(b"At ")), Segment::Hole { index: 0, kind: HoleKind::Wall }])
            }
            JigPhrase::Accepted => Box::new([Segment::Text(copy_of(b"Accepted"))]),
            JigPhrase::Rejected => {
                Box::new([Segment::Text(copy_of(b"Rejected: ")), Segment::Hole { index: 0, kind: HoleKind::Name }])
            }
            JigPhrase::PassedUp => Box::new([Segment::Text(copy_of(b"Passed up"))]),
            JigPhrase::Released => Box::new([Segment::Text(copy_of(b"Released"))]),
            JigPhrase::ReleasedAgain => Box::new([Segment::Text(copy_of(b"Released: make the effect again"))]),
            JigPhrase::ReleasedWithout => Box::new([Segment::Text(copy_of(b"Released: do not make the effect again"))]),
            JigPhrase::LeftHeld => {
                Box::new([Segment::Text(copy_of(b"Left held: ")), Segment::Hole { index: 0, kind: HoleKind::Name }])
            }
            JigPhrase::DecidedFirst => Box::new([
                Segment::Text(copy_of(b"Decided first by ")),
                Segment::Hole { index: 0, kind: HoleKind::Name },
            ]),
            JigPhrase::MadeTask => {
                Box::new([Segment::Text(copy_of(b"Made task ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::MadeService => Box::new([
                Segment::Text(copy_of(b"Made service party ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::MadeResult => Box::new([
                Segment::Text(copy_of(b"Answered person task ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::MovingOut => Box::new([
                Segment::Text(copy_of(b"Account ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b": moving out, move ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
            ]),
            JigPhrase::MadeNote => {
                Box::new([Segment::Text(copy_of(b"Made note ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::RoleMissing => Box::new([
                Segment::Text(copy_of(b"Project ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b": requires role ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
            ]),
            JigPhrase::BudgetMissing => Box::new([
                Segment::Text(copy_of(b"Your pool has ")),
                Segment::Hole { index: 0, kind: HoleKind::Amount },
                Segment::Text(copy_of(b" left; this needs ")),
                Segment::Hole { index: 1, kind: HoleKind::Amount },
            ]),
            JigPhrase::GrantMissing => Box::new([
                Segment::Text(copy_of(b"Requires effect kind ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" on connector ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
                Segment::Text(copy_of(b", resource ")),
                Segment::Hole { index: 2, kind: HoleKind::Name },
            ]),
            JigPhrase::DepthMissing => Box::new([
                Segment::Text(copy_of(b"Delegation needs depth ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b"; allowed ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
            ]),
            JigPhrase::DeadlineMissing => Box::new([
                Segment::Text(copy_of(b"Deadline ")),
                Segment::Hole { index: 0, kind: HoleKind::Wall },
                Segment::Text(copy_of(b" exceeds latest allowed ")),
                Segment::Hole { index: 1, kind: HoleKind::Wall },
            ]),
            JigPhrase::TasksMissing => Box::new([
                Segment::Text(copy_of(b"Needs ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" tasks; ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
                Segment::Text(copy_of(b" left")),
            ]),
            JigPhrase::Agent => Box::new([Segment::Text(copy_of(b"Agent"))]),
            JigPhrase::Procedure => Box::new([Segment::Text(copy_of(b"Procedure"))]),
            JigPhrase::Person | JigPhrase::FramePerson => Box::new([Segment::Text(copy_of(b"Person"))]),
            JigPhrase::RoleExecutor => Box::new([Segment::Text(copy_of(b"Person holding a role"))]),
            JigPhrase::KindMissing => Box::new([Segment::Text(copy_of(b"Executor kind not allowed"))]),
            JigPhrase::LimitLive => Box::new([
                Segment::Text(copy_of(b"Project ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b": live task limit ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
            ]),
            JigPhrase::LimitInbox => Box::new([
                Segment::Text(copy_of(b"Task ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b": inbox limit ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
            ]),
            JigPhrase::LimitWords => Box::new([
                Segment::Text(copy_of(b"Words exceed ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" bytes")),
            ]),
            JigPhrase::LimitConversation => Box::new([
                Segment::Text(copy_of(b"Conversation word snapshot limit ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::LimitFields => Box::new([Segment::Text(copy_of(b"Field limit reached"))]),
            JigPhrase::LimitRecords => Box::new([Segment::Text(copy_of(b"Kept record limit reached"))]),
            JigPhrase::LimitRequests => Box::new([Segment::Text(copy_of(b"Request limit reached"))]),
            JigPhrase::LimitReads => Box::new([Segment::Text(copy_of(b"Read limit reached"))]),
            JigPhrase::LimitWatches => Box::new([Segment::Text(copy_of(b"Watch limit reached"))]),
            JigPhrase::LimitObjects => Box::new([Segment::Text(copy_of(b"Working set limit reached"))]),
            JigPhrase::MovedHold => Box::new([Segment::Text(copy_of(b"The hold or its reason changed"))]),
            JigPhrase::MovedSpec => Box::new([Segment::Text(copy_of(b"The specification changed"))]),
            JigPhrase::MovedPerson => Box::new([Segment::Text(copy_of(b"The question or its options changed"))]),
            JigPhrase::MovedNote => Box::new([Segment::Text(copy_of(b"The note changed"))]),
            JigPhrase::MovedWaiting => Box::new([Segment::Text(copy_of(b"The recipient changed"))]),
            JigPhrase::Revision => Box::new([
                Segment::Text(copy_of(b"Current revision ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),

            JigPhrase::Amount => Box::new([Segment::Hole { index: 0, kind: HoleKind::Amount }]),
            JigPhrase::WaitingSpan => Box::new([
                Segment::Text(copy_of(b"Waiting ")),
                Segment::Hole { index: 0, kind: HoleKind::Span },
                Segment::Text(copy_of(b" min")),
            ]),
            JigPhrase::Span => {
                Box::new([Segment::Hole { index: 0, kind: HoleKind::Span }, Segment::Text(copy_of(b" min"))])
            }
            JigPhrase::Object => Box::new([Segment::Text(copy_of(b"Object"))]),
            JigPhrase::Proposal => Box::new([Segment::Text(copy_of(b"Proposal"))]),
            JigPhrase::Escalation => Box::new([Segment::Text(copy_of(b"Escalation"))]),
            JigPhrase::Question => Box::new([Segment::Text(copy_of(b"Question"))]),
            JigPhrase::PersonTask => Box::new([Segment::Text(copy_of(b"Person task"))]),
            JigPhrase::Result => Box::new([Segment::Text(copy_of(b"Result"))]),
            JigPhrase::Note => Box::new([Segment::Text(copy_of(b"Note"))]),
            JigPhrase::ContextProject => {
                Box::new([Segment::Text(copy_of(b"In project ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::ChipNumber => Box::new([
                Segment::Text(copy_of(b"Task ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b": ")),
                Segment::Hole { index: 1, kind: HoleKind::Name },
            ]),

            JigPhrase::WakeAny => Box::new([Segment::Text(copy_of(b"Wakes on any news"))]),
            JigPhrase::WakeAt => Box::new([Segment::Text(copy_of(b"Wakes at the next scheduled time"))]),
            JigPhrase::WakeResult => Box::new([Segment::Text(copy_of(b"Wakes on a delegate result"))]),
            JigPhrase::WakeTopic => Box::new([Segment::Text(copy_of(b"Wakes on subscribed news"))]),
            JigPhrase::WakeInbox => Box::new([Segment::Text(copy_of(b"Wakes on inbox messages"))]),

            JigPhrase::Attempt => {
                Box::new([Segment::Text(copy_of(b"Attempt ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::RetryAt => Box::new([
                Segment::Text(copy_of(b"Retrying at ")),
                Segment::Hole { index: 0, kind: HoleKind::Wall },
                Segment::Text(copy_of(b": try ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
                Segment::Text(copy_of(b" of ")),
                Segment::Hole { index: 2, kind: HoleKind::Number },
            ]),

            JigPhrase::HostLost => Box::new([Segment::Text(copy_of(b"Host lost"))]),
            JigPhrase::Temporary => Box::new([Segment::Text(copy_of(b"Temporary failure"))]),
            JigPhrase::Permanent => Box::new([Segment::Text(copy_of(b"Permanent failure"))]),
            JigPhrase::Stall => Box::new([Segment::Text(copy_of(b"Stalled"))]),
            JigPhrase::Reason => {
                Box::new([Segment::Text(copy_of(b"Reason: ")), Segment::Hole { index: 0, kind: HoleKind::Name }])
            }
            JigPhrase::Closing => Box::new([
                Segment::Text(copy_of(b"Closing: ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" delegates")),
            ]),
            JigPhrase::ClosingConnector => Box::new([Segment::Text(copy_of(b"and "))]),
            JigPhrase::ApprovalAuthorityMissing => {
                Box::new([Segment::Text(copy_of(b"Proposal approval authority is missing"))])
            }
            JigPhrase::NobodyMayAccept => Box::new([Segment::Text(copy_of(b"Nobody in this project may accept it"))]),
            JigPhrase::ClosingEffects => Box::new([Segment::Text(copy_of(b"'s effects settling"))]),

            JigPhrase::ChipTitleCut => Box::new([Segment::Text(copy_of(b"Title is incomplete"))]),
            JigPhrase::ChipDependenciesCut => Box::new([Segment::Text(copy_of(b"More dependencies are not shown"))]),
            JigPhrase::ChipWaitingHolds => Box::new([Segment::Text(copy_of(b"Waiting for resource holds"))]),
            JigPhrase::WaitingParty => Box::new([
                Segment::Text(copy_of(b"Waiting for party ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::WaitingTask => Box::new([
                Segment::Text(copy_of(b"Waiting for task ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::WaitingRole => Box::new([
                Segment::Text(copy_of(b"Waiting for project ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b", role ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
            ]),
            JigPhrase::Since => {
                Box::new([Segment::Text(copy_of(b"Since ")), Segment::Hole { index: 0, kind: HoleKind::Wall }])
            }
            JigPhrase::Pool => Box::new([Segment::Text(copy_of(b"From this pool"))]),
            JigPhrase::ProposalWhy => Box::new([Segment::Text(copy_of(b"Why this is proposed"))]),
            JigPhrase::OwnTasks => Box::new([Segment::Text(copy_of(b"The accepted tasks will be yours"))]),
            JigPhrase::Batch => Box::new([Segment::Text(copy_of(b"Tasks acceptance would make"))]),
            JigPhrase::BatchTask => Box::new([
                Segment::Text(copy_of(b"Proposed task ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::AfterTask => Box::new([
                Segment::Text(copy_of(b"After proposed task ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::Budget => {
                Box::new([Segment::Text(copy_of(b"Budget ")), Segment::Hole { index: 0, kind: HoleKind::Amount }])
            }
            JigPhrase::Before => Box::new([Segment::Text(copy_of(b"Before"))]),
            JigPhrase::After => Box::new([Segment::Text(copy_of(b"After"))]),
            JigPhrase::Depth => Box::new([
                Segment::Text(copy_of(b"Delegation depth ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::TasksAllowed => Box::new([
                Segment::Text(copy_of(b"Tasks allowed ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::Grant => Box::new([
                Segment::Text(copy_of(b"Effect kind ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" on connector ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
                Segment::Text(copy_of(b", resource ")),
                Segment::Hole { index: 2, kind: HoleKind::Name },
            ]),
            JigPhrase::ToolAllowed => Box::new([
                Segment::Text(copy_of(b"Tool ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" allowed")),
            ]),
            JigPhrase::PassedBy => Box::new([Segment::Text(copy_of(b"Passed by this holder"))]),
            JigPhrase::Widen => Box::new([Segment::Text(copy_of(b"Widen authority"))]),
            JigPhrase::AmendTask => {
                Box::new([Segment::Text(copy_of(b"Amend task ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::Effect => Box::new([Segment::Text(copy_of(b"Effect"))]),
            JigPhrase::AnswerField => Box::new([Segment::Text(copy_of(b"Your answer"))]),
            JigPhrase::ApprovalState => Box::new([
                Segment::Text(copy_of(b"State to approve: ")),
                Segment::Hole { index: 0, kind: HoleKind::Name },
            ]),
            JigPhrase::Options | JigPhrase::MissingChoice => Box::new([Segment::Text(copy_of(b"Choose an option"))]),
            JigPhrase::CannotDo => Box::new([Segment::Text(copy_of(b"Cannot do it"))]),
            JigPhrase::TakenBy => {
                Box::new([Segment::Text(copy_of(b"Taken by ")), Segment::Hole { index: 0, kind: HoleKind::Name }])
            }
            JigPhrase::TakenByNumber => Box::new([
                Segment::Text(copy_of(b"Taken by party ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::StallSpan => Box::new([
                Segment::Text(copy_of(b"Becomes news for its requester after ")),
                Segment::Hole { index: 0, kind: HoleKind::Span },
                Segment::Text(copy_of(b" min")),
            ]),
            JigPhrase::Verdict => {
                Box::new([Segment::Text(copy_of(b"Verdict ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::ChoiceResult => Box::new([
                Segment::Text(copy_of(b"Choice ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b", by ")),
                Segment::Hole { index: 1, kind: HoleKind::Name },
            ]),
            JigPhrase::ApprovalYes => Box::new([
                Segment::Text(copy_of(b"Approved ")),
                Segment::Hole { index: 0, kind: HoleKind::Name },
                Segment::Text(copy_of(b", by ")),
                Segment::Hole { index: 1, kind: HoleKind::Name },
            ]),
            JigPhrase::ApprovalNo => Box::new([
                Segment::Text(copy_of(b"Did not approve ")),
                Segment::Hole { index: 0, kind: HoleKind::Name },
                Segment::Text(copy_of(b", by ")),
                Segment::Hole { index: 1, kind: HoleKind::Name },
            ]),
            JigPhrase::FirstUnread => Box::new([
                Segment::Text(copy_of(b"Open at first reply not read: ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::Reply => Box::new([Segment::Text(copy_of(b"Reply"))]),
            JigPhrase::LoadingPart => Box::new([Segment::Text(copy_of(b"Loading connector content"))]),
            JigPhrase::LoadingObject => Box::new([Segment::Text(copy_of(b"Loading object"))]),
            JigPhrase::ActWrite => Box::new([Segment::Text(copy_of(b"Write"))]),
            JigPhrase::ActAnswer => Box::new([Segment::Text(copy_of(b"Answer"))]),
            JigPhrase::ActTake => Box::new([Segment::Text(copy_of(b"Take it"))]),
            JigPhrase::ActHandBack => Box::new([Segment::Text(copy_of(b"Hand it back"))]),
            JigPhrase::ActAccept => Box::new([Segment::Text(copy_of(b"Accept"))]),
            JigPhrase::ActReject => Box::new([Segment::Text(copy_of(b"Reject"))]),
            JigPhrase::ActPassUp => Box::new([Segment::Text(copy_of(b"Pass up"))]),
            JigPhrase::ActRelease => Box::new([Segment::Text(copy_of(b"Release"))]),
            JigPhrase::ActLeaveHeld => Box::new([Segment::Text(copy_of(b"Leave it held"))]),
            JigPhrase::ActAmend => Box::new([Segment::Text(copy_of(b"Amend"))]),
            JigPhrase::ActAmendRelease => Box::new([Segment::Text(copy_of(b"Amend and release"))]),
            JigPhrase::ActStop => Box::new([Segment::Text(copy_of(b"Stop"))]),
            JigPhrase::ActCancel => Box::new([Segment::Text(copy_of(b"Cancel"))]),
            JigPhrase::ActClose => Box::new([Segment::Text(copy_of(b"Close"))]),
            JigPhrase::ActTakeOver => Box::new([Segment::Text(copy_of(b"Take over"))]),
            JigPhrase::ActSetGoal => Box::new([Segment::Text(copy_of(b"Set goal"))]),
            JigPhrase::ActReorder => Box::new([Segment::Text(copy_of(b"Reorder"))]),
            JigPhrase::ActMarkSeen => Box::new([Segment::Text(copy_of(b"Mark read"))]),
            JigPhrase::ActEditNote => Box::new([Segment::Text(copy_of(b"Edit note"))]),
            JigPhrase::ActDeleteNote => Box::new([Segment::Text(copy_of(b"Delete note"))]),
            JigPhrase::ActAdopt => Box::new([Segment::Text(copy_of(b"Adopt resource"))]),
            JigPhrase::ActChangePolicy => Box::new([Segment::Text(copy_of(b"Change policy"))]),
            JigPhrase::ActSetRoles => Box::new([Segment::Text(copy_of(b"Set roles"))]),
            JigPhrase::ActMakeService => Box::new([Segment::Text(copy_of(b"Make service"))]),
            JigPhrase::ActSetPool => Box::new([Segment::Text(copy_of(b"Set pool"))]),
            JigPhrase::ChatAbout => Box::new([Segment::Text(copy_of(b"Chat about it"))]),

            JigPhrase::Topic => Box::new([
                Segment::Text(copy_of(b"Subscribed topic ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::ElapsedSpan => Box::new([
                Segment::Text(copy_of(b"For ")),
                Segment::Hole { index: 0, kind: HoleKind::Span },
                Segment::Text(copy_of(b" min")),
            ]),
            JigPhrase::ReasonField => Box::new([Segment::Text(copy_of(b"Your reason"))]),
            JigPhrase::LoadingPhase => Box::new([Segment::Text(copy_of(b"Loading phase details"))]),
            JigPhrase::Deadline => {
                Box::new([Segment::Text(copy_of(b"Deadline ")), Segment::Hole { index: 0, kind: HoleKind::Wall }])
            }

            JigPhrase::StalledOn => Box::new([
                Segment::Text(copy_of(b"Held: stalled on ")),
                Segment::Hole { index: 0, kind: HoleKind::Name },
            ]),
            JigPhrase::DependencyEnded => Box::new([
                Segment::Text(copy_of(b"Held: dependency ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" ended unsuccessfully")),
            ]),
            JigPhrase::DriftHold => Box::new([Segment::Text(copy_of(b"Held: the system changed"))]),
            JigPhrase::EffectFailedHold => Box::new([Segment::Text(copy_of(b"Held: the effect failed for good"))]),
            JigPhrase::EffectUncertainHold => Box::new([Segment::Text(copy_of(
                b"Held: the effect may or may not have been made; inspect the system before deciding",
            ))]),
            JigPhrase::BudgetHold => Box::new([
                Segment::Text(copy_of(b"Held: spent ")),
                Segment::Hole { index: 0, kind: HoleKind::Amount },
                Segment::Text(copy_of(b", budget ")),
                Segment::Hole { index: 1, kind: HoleKind::Amount },
            ]),
            JigPhrase::DeadlineHold => Box::new([
                Segment::Text(copy_of(b"Held: past deadline ")),
                Segment::Hole { index: 0, kind: HoleKind::Wall },
            ]),
            JigPhrase::StoppedHold => Box::new([
                Segment::Text(copy_of(b"Held: stopped by ")),
                Segment::Hole { index: 0, kind: HoleKind::Name },
                Segment::Text(copy_of(b" at ")),
                Segment::Hole { index: 1, kind: HoleKind::Wall },
            ]),
            JigPhrase::ResourceHold => {
                Box::new([Segment::Text(copy_of(b"Held: waiting for a resource past its bound"))])
            }
            JigPhrase::PoolHold => Box::new([Segment::Text(copy_of(b"Held: the pool lost its allocation"))]),
            JigPhrase::ProcedureHold => Box::new([Segment::Text(copy_of(b"Held: the procedure needs a decision"))]),
            JigPhrase::LoadingDependency => Box::new([
                Segment::Text(copy_of(b"Loading task ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" phase facts")),
            ]),
            JigPhrase::BeforeHold => Box::new([Segment::Text(copy_of(b"Before the hold"))]),
            JigPhrase::SpentRun => Box::new([
                Segment::Text(copy_of(b"Task ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b", attempt ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
                Segment::Text(copy_of(b": spent ")),
                Segment::Hole { index: 2, kind: HoleKind::Amount },
            ]),
            JigPhrase::SpentDelegate => Box::new([
                Segment::Text(copy_of(b"Task ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b": spent ")),
                Segment::Hole { index: 1, kind: HoleKind::Amount },
            ]),
            JigPhrase::Confirmation => Box::new([Segment::Text(copy_of(b"Review before confirming"))]),
            JigPhrase::Confirm => Box::new([Segment::Text(copy_of(b"Confirm"))]),
            JigPhrase::Dismiss => Box::new([Segment::Text(copy_of(b"Dismiss"))]),
            JigPhrase::WillPropose => Box::new([Segment::Text(copy_of(b"This will be proposed to its holder"))]),
            JigPhrase::ChangedConfirmation => Box::new([Segment::Text(copy_of(
                b"The revision this action names changed; inspect the current facts and open it again",
            ))]),
            JigPhrase::MissingWords => Box::new([Segment::Text(copy_of(b"Required words are missing"))]),
            JigPhrase::MissingAgain => Box::new([Segment::Text(copy_of(b"Choose whether to make the effect again"))]),
            JigPhrase::MissingChange => Box::new([Segment::Text(copy_of(b"Supply the amendment"))]),
            JigPhrase::MissingDelegates => {
                Box::new([Segment::Text(copy_of(b"Choose what becomes of every live delegate"))])
            }
            JigPhrase::MissingPriority => Box::new([Segment::Text(copy_of(b"Supply a priority"))]),
            JigPhrase::MissingContext => {
                Box::new([Segment::Text(copy_of(b"Waiting for this object's project context"))])
            }
            JigPhrase::WillAccept => {
                Box::new([Segment::Text(copy_of(b"Acceptance makes or changes the following, funded as shown"))])
            }
            JigPhrase::WillReject => {
                Box::new([Segment::Text(copy_of(b"Reject this proposal and send its proposer your reason"))])
            }
            JigPhrase::WillPassUp => Box::new([Segment::Text(copy_of(b"Pass this decision to the holder above"))]),
            JigPhrase::WillLeaveHeld => Box::new([Segment::Text(copy_of(
                b"Remove this escalation from your inbox; the task stays held with your reason",
            ))]),
            JigPhrase::WillStop => Box::new([Segment::Text(copy_of(b"Stop the live run and hold its task"))]),
            JigPhrase::WillCancel => Box::new([Segment::Text(copy_of(
                b"Cancel this task and everything below it; already made effects remain",
            ))]),
            JigPhrase::WillClose => Box::new([Segment::Text(copy_of(
                b"Close this chat after disposing of every live delegate; pending effects settle",
            ))]),
            JigPhrase::WillTakeOver => {
                Box::new([Segment::Text(copy_of(b"Become this task's requester and move its funding"))])
            }
            JigPhrase::WillTake => {
                Box::new([Segment::Text(copy_of(b"Take this role task so others cannot answer it at once"))])
            }
            JigPhrase::WillHandBack => Box::new([Segment::Text(copy_of(b"Hand this task back to its role"))]),
            JigPhrase::WillWrite => Box::new([Segment::Text(copy_of(b"Send these words to the task"))]),
            JigPhrase::WillAnswer => {
                Box::new([Segment::Text(copy_of(b"Send the answer matching the task's contract"))])
            }
            JigPhrase::WillAmend => {
                Box::new([Segment::Text(copy_of(b"Replace the named specification with these changes"))])
            }
            JigPhrase::WillAmendRelease => Box::new([Segment::Text(copy_of(b"Amend and release in one decision"))]),
            JigPhrase::WillPrioritise => Box::new([Segment::Text(copy_of(b"Change this goal's priority"))]),
            JigPhrase::WillEditNote => Box::new([Segment::Text(copy_of(b"Replace the named note revision"))]),
            JigPhrase::WillDeleteNote => Box::new([Segment::Text(copy_of(b"Delete this note revision"))]),
            JigPhrase::WillGoal => Box::new([Segment::Text(copy_of(b"Make a goal from these words and options"))]),
            JigPhrase::ContinueClosing => {
                Box::new([Segment::Text(copy_of(b"Releasing carries on closing with the kept result"))])
            }
            JigPhrase::ReleaseWait => Box::new([Segment::Text(copy_of(b"Release to wait again"))]),
            JigPhrase::ReleaseProcedure => Box::new([Segment::Text(copy_of(b"Release with procedure counts reset"))]),
            JigPhrase::RepeatEffect => Box::new([Segment::Text(copy_of(b"Make the effect again"))]),
            JigPhrase::SkipEffect => {
                Box::new([Segment::Text(copy_of(b"Release without making the effect again; tell its executor so"))])
            }
            JigPhrase::ActWithdraw => Box::new([Segment::Text(copy_of(b"Withdraw proposal"))]),
            JigPhrase::DelegateCancel => Box::new([
                Segment::Text(copy_of(b"Cancel delegate ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" and its subtree")),
            ]),
            JigPhrase::DelegateTake => Box::new([
                Segment::Text(copy_of(b"Take over delegate ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b", reserving ")),
                Segment::Hole { index: 1, kind: HoleKind::Amount },
            ]),
            JigPhrase::DelegateChoices => Box::new([Segment::Text(copy_of(b"Choose each delegate's outcome"))]),
            JigPhrase::WriteAndRelease => Box::new([Segment::Text(copy_of(b"Write and release together"))]),
            JigPhrase::WriteOnly => Box::new([Segment::Text(copy_of(b"Write without releasing"))]),
            JigPhrase::NewSpec => Box::new([Segment::Text(copy_of(b"New specification"))]),
            JigPhrase::NewInstructions => Box::new([Segment::Text(copy_of(b"New instructions"))]),
            JigPhrase::InstructionNotSupplied => Box::new([Segment::Text(copy_of(b"Instructions were not supplied"))]),
            JigPhrase::InstructionEmpty => Box::new([Segment::Text(copy_of(b"Instructions are empty"))]),
            JigPhrase::NewBudget => Box::new([Segment::Text(copy_of(b"New budget"))]),
            JigPhrase::NewDeadline => Box::new([Segment::Text(copy_of(b"New deadline (UTC, YYYY-MM-DD HH:MM)"))]),
            JigPhrase::DropDependency => Box::new([
                Segment::Text(copy_of(b"Drop dependency ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::PriorityField => Box::new([Segment::Text(copy_of(b"New goal priority"))]),
            JigPhrase::NoteDescription => Box::new([Segment::Text(copy_of(b"Note description"))]),
            JigPhrase::NoteBody => Box::new([Segment::Text(copy_of(b"Note body"))]),
            JigPhrase::PersonAddressRevision => Box::new([
                Segment::Text(copy_of(b"Person-task revision ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::ArchiveCumulativeSpend => Box::new([
                Segment::Text(copy_of(b"Archive cumulative spend: ")),
                Segment::Hole { index: 0, kind: HoleKind::Amount },
            ]),
            JigPhrase::ArchiveAcceptedAt => Box::new([
                Segment::Text(copy_of(b"Archive accepted at: ")),
                Segment::Hole { index: 0, kind: HoleKind::Wall },
            ]),
            JigPhrase::ArchiveReadThrough => Box::new([
                Segment::Text(copy_of(b"Archive read through: ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::ArchiveReadAbsent => Box::new([Segment::Text(copy_of(b"Archive read position not supplied"))]),
            JigPhrase::ResultTimeUnknown => Box::new([Segment::Text(copy_of(b"Completion time not supplied"))]),
            JigPhrase::ChangeResultKind => Box::new([
                Segment::Text(copy_of(b"Change result kind ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::DocumentTextCut => Box::new([Segment::Text(copy_of(b"Only part of this text was supplied"))]),
            JigPhrase::NoteReferencesCut => Box::new([Segment::Text(copy_of(
                b"This note has more references; read the whole note before replacing it",
            ))]),
            JigPhrase::Charter => {
                Box::new([Segment::Text(copy_of(b"Charter ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::CreateProcedure => Box::new([
                Segment::Text(copy_of(b"Connector ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b", procedure ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
            ]),
            JigPhrase::Priority => {
                Box::new([Segment::Text(copy_of(b"Priority ")), Segment::Hole { index: 0, kind: HoleKind::Number }])
            }
            JigPhrase::Notice => Box::new([Segment::Text(copy_of(b"Notice"))]),
            JigPhrase::UnknownAction => {
                Box::new([Segment::Text(copy_of(b"Outcome unknown for this action; inspect its current facts"))])
            }
            JigPhrase::ResultsThrough => Box::new([
                Segment::Text(copy_of(b"Results through ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
            ]),
            JigPhrase::RepliesThrough => Box::new([
                Segment::Text(copy_of(b"Chat ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b", replies through ")),
                Segment::Hole { index: 1, kind: HoleKind::Number },
            ]),
            JigPhrase::FailedBare => Box::new([Segment::Text(copy_of(b"Failed"))]),
            JigPhrase::CancelledBare => Box::new([Segment::Text(copy_of(b"Cancelled"))]),
            JigPhrase::AddressParty => Box::new([Segment::Text(copy_of(b"Address to party number"))]),
            JigPhrase::AddressTask => Box::new([Segment::Text(copy_of(b"Address to task number"))]),
            JigPhrase::AddressRole => Box::new([Segment::Text(copy_of(b"Address to role number in this project"))]),
        };
        phrases
            .push(Phrase { id: phrase.id(), forms: Box::new([Form { class: PluralClass::Other, segments }]) })
            .expect("all English phrases fit");
    }
    crate::owner_words::language(Language {
        plural: PluralRule::English,
        span_unit: SpanUnit::Minutes,
        wall: WallFormat::Clock,
        phrases: phrases.into_boxed(),
    })
}
/// A ready English phrasebook whose names fit the client's text limit.
pub fn english(text: u32) -> Result<Phrasebook, Invalid> {
    let words = language();
    let mut segments = 0_u32;
    let mut text_bytes = 0_u32;
    for phrase in &words.phrases {
        for form in &phrase.forms {
            for segment in &form.segments {
                segments = segments.checked_add(1).ok_or(Invalid::Overflow)?;
                match segment {
                    Segment::Text(bytes) => {
                        let Ok(length) = u32::try_from(bytes.len()) else {
                            return Err(Invalid::Overflow);
                        };
                        text_bytes = text_bytes.checked_add(length).ok_or(Invalid::Overflow)?;
                    }
                    Segment::Hole { .. } => {}
                }
            }
        }
    }
    Phrasebook::load(
        shapes(text),
        Box::new([words]),
        &PhraseLimits {
            languages: 1,
            phrases: crate::owner_words::phrase_count(),
            holes: 4,
            segments,
            text_bytes,
            hole_bytes: text,
        },
    )
}
